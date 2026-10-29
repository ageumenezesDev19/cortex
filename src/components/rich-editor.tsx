import { baseKeymap, chainCommands, toggleMark } from "prosemirror-commands"
import { history, redo, undo } from "prosemirror-history"
import {
  InputRule,
  inputRules,
  textblockTypeInputRule,
  wrappingInputRule,
} from "prosemirror-inputrules"
import { keymap } from "prosemirror-keymap"
import type { MarkType, Node as ProseNode } from "prosemirror-model"
import { liftListItem, sinkListItem, splitListItem } from "prosemirror-schema-list"
import { EditorState, NodeSelection, Plugin, PluginKey, TextSelection } from "prosemirror-state"
import { Decoration, DecorationSet, EditorView } from "prosemirror-view"
import { useEffect, useRef } from "react"

import { assetUrl } from "../lib/api"
import { noteSurfaceAt, onDropOnNote } from "../lib/drop"
import { altWithWidth, headingSpan, imageWidth, parse, schema, serialize } from "../lib/markdown"
import { usePick } from "./language-provider"

const content = {
  en: { small: "S", medium: "M", large: "L", original: "Original", size: "Image size" },
  pt: { small: "P", medium: "M", large: "G", original: "Original", size: "Tamanho da imagem" },
}

type Labels = (typeof content)["pt"]

type Props = {
  /** Only the initial document; typing does not flow back through this. */
  markdown: string
  root: string
  noteFolder: string
  /** Heading index to frame on arrival, or null when nothing was searched. */
  anchorHeading: number | null
  onChange: (markdown: string) => void
  onPasteImage: (file: File) => Promise<string | null>
  /** Copies screenshots dropped from outside in, and answers with their sources. */
  onAttachImages: (paths: string[]) => Promise<string[]>
}

const anchorKey = new PluginKey<DecorationSet>("cortex-anchor")

export function RichEditor({
  markdown,
  root,
  noteFolder,
  anchorHeading,
  onChange,
  onPasteImage,
  onAttachImages,
}: Props) {
  const host = useRef<HTMLDivElement>(null)
  const view = useRef<EditorView | null>(null)

  // Callbacks change on every render of the parent; the view is built once, so
  // it reads them through a ref instead of being rebuilt.
  const handlers = useRef({ onChange, onPasteImage, onAttachImages })
  handlers.current = { onChange, onPasteImage, onAttachImages }

  // Same reason as the handlers: a node view outlives the render that made it,
  // and a language switch has to reach the bar it draws.
  const t = usePick()(content)
  const labels = useRef<Labels>(t)
  labels.current = t

  useEffect(() => {
    if (!host.current) return

    const doc = parse(markdown)
    const state = EditorState.create({
      doc,
      plugins: [
        anchorPlugin(doc, anchorHeading),
        history(),
        inputRules({ rules: markdownRules() }),
        keymap(editingKeymap()),
        keymap(baseKeymap),
      ],
    })

    const editor = new EditorView(host.current, {
      state,
      attributes: { class: "cortex-prose", "data-note-surface": "true" },
      nodeViews: {
        image: (node, editorView, getPos) =>
          new ImageView(node, editorView, getPos, root, noteFolder, labels),
      },
      dispatchTransaction(transaction) {
        const next = editor.state.apply(transaction)
        editor.updateState(next)
        if (transaction.docChanged) handlers.current.onChange(serialize(next.doc))
      },
      handleDOMEvents: {
        // The amber frame means "the search found this". The moment you put the
        // cursor in the text you are writing, not searching, so it goes.
        mousedown: clearAnchor,
        keydown: clearAnchor,
      },
      handlePaste(_, event) {
        const file = imageFrom(event.clipboardData)
        if (!file) return false

        handlers.current.onPasteImage(file).then((source) => {
          if (!source || !view.current) return
          const image = schema.nodes.image.create({ src: source })
          view.current.dispatch(view.current.state.tr.replaceSelectionWith(image))
        })
        return true
      },
    })

    view.current = editor
    scrollToAnchor(editor)
    return () => {
      editor.destroy()
      view.current = null
    }
    // Rebuilding on `markdown` would throw away the cursor on every keystroke:
    // the document is seeded once, and the note panel remounts on a path change.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [])

  // A print dragged from the Finder onto the note. It lands where it was let
  // go, not at whatever the cursor was doing before the drag started.
  useEffect(
    () =>
      onDropOnNote(({ paths, x, y }) => {
        // The surface the drop was measured against is this view's own element,
        // which is how a drop meant for the markdown textarea is left alone.
        const editor = view.current
        if (!editor || noteSurfaceAt(x, y) !== editor.dom) return

        // `posAtCoords` can still come back empty over a margin. The end of the
        // note is a worse place than the pointer, and a far better one than
        // nowhere: a file that was dropped has to end up somewhere visible.
        const at = editor.posAtCoords({ left: x, top: y })

        void (async () => {
          const sources = await handlers.current.onAttachImages(paths)
          let dropPoint: number | null = at?.pos ?? editor.state.doc.content.size

          for (const source of sources) {
            if (!view.current) return

            const { tr, doc } = view.current.state
            // Only the first image is placed; the ones after it follow the
            // selection the insert leaves behind, so two files keep their order.
            if (dropPoint !== null) {
              tr.setSelection(
                TextSelection.near(doc.resolve(Math.min(dropPoint, doc.content.size))),
              )
              dropPoint = null
            }
            view.current.dispatch(
              tr.replaceSelectionWith(schema.nodes.image.create({ src: source })),
            )
          }
        })()
      }),
    [],
  )

  return <div ref={host} className="scroll-y h-full" />
}

function clearAnchor(view: EditorView) {
  if (anchorKey.getState(view.state)?.find().length) {
    view.dispatch(view.state.tr.setMeta(anchorKey, "clear"))
  }
  return false
}

function scrollToAnchor(view: EditorView) {
  const decoration = anchorKey.getState(view.state)?.find()[0]
  if (!decoration) return
  view.domAtPos(decoration.from).node instanceof HTMLElement &&
    (view.domAtPos(decoration.from).node as HTMLElement).scrollIntoView({ block: "center" })
}

function anchorPlugin(doc: ProseNode, headingIndex: number | null) {
  const decorations =
    headingIndex === null ? [] : frameFor(doc, headingSpan(doc, headingIndex))

  return new Plugin<DecorationSet>({
    key: anchorKey,
    state: {
      init: () => DecorationSet.create(doc, decorations),
      apply(transaction, set) {
        if (transaction.getMeta(anchorKey) === "clear") return DecorationSet.empty
        return set.map(transaction.mapping, transaction.doc)
      },
    },
    props: {
      decorations: (state) => anchorKey.getState(state),
    },
  })
}

/** One decoration per top-level block in the span, so the frame wraps blocks. */
function frameFor(doc: ProseNode, span: { from: number; to: number }) {
  const decorations: Decoration[] = []
  doc.forEach((node, offset) => {
    if (offset >= span.from && offset < span.to) {
      decorations.push(Decoration.node(offset, offset + node.nodeSize, { class: "cortex-found" }))
    }
  })
  return decorations
}

function markdownRules() {
  return [
    textblockTypeInputRule(/^(#{1,6})\s$/, schema.nodes.heading, (match) => ({
      level: match[1].length,
    })),
    textblockTypeInputRule(/^```$/, schema.nodes.code_block),
    wrappingInputRule(/^\s*>\s$/, schema.nodes.blockquote),
    wrappingInputRule(/^\s*([-+*])\s$/, schema.nodes.bullet_list),
    wrappingInputRule(
      /^(\d+)\.\s$/,
      schema.nodes.ordered_list,
      (match) => ({ order: Number(match[1]) }),
      (match, node) => node.childCount + node.attrs.order === Number(match[1]),
    ),
    markRule(/(?:\*\*)([^*]+)(?:\*\*)$/, schema.marks.strong),
    markRule(/(?:^|[^*])(?:\*)([^*]+)(?:\*)$/, schema.marks.em),
    markRule(/(?:`)([^`]+)(?:`)$/, schema.marks.code),
  ]
}

/**
 * `prosemirror-inputrules` ships rules for blocks but none for marks, so the
 * closing delimiter is matched by hand and the text between is re-inserted
 * carrying the mark.
 */
function markRule(pattern: RegExp, mark: MarkType) {
  return new InputRule(pattern, (state, match, start, end) => {
    const [full, content] = match
    const from = start + full.indexOf(content)
    const transaction = state.tr

    if (content) {
      transaction.replaceWith(from, end, schema.text(content, [mark.create()]))
    }
    return transaction.removeStoredMark(mark)
  })
}

function editingKeymap() {
  return {
    "Mod-b": toggleMark(schema.marks.strong),
    "Mod-i": toggleMark(schema.marks.em),
    "Mod-`": toggleMark(schema.marks.code),
    "Mod-z": undo,
    "Shift-Mod-z": redo,
    "Mod-y": redo,
    Enter: chainCommands(splitListItem(schema.nodes.list_item), baseKeymap.Enter),
    Tab: sinkListItem(schema.nodes.list_item),
    "Shift-Tab": liftListItem(schema.nodes.list_item),
  }
}

function imageFrom(data: DataTransfer | null) {
  return (
    Array.from(data?.items ?? [])
      .find((item) => item.type.startsWith("image/"))
      ?.getAsFile() ?? null
  )
}

/** What the bar offers. `null` is "original": no width written at all. */
const sizes: { label: keyof typeof content.pt; width: number | null }[] = [
  { label: "small", width: 320 },
  { label: "medium", width: 560 },
  { label: "large", width: 880 },
  { label: "original", width: null },
]

const sizeKey = (width: number | null) => (width === null ? "original" : String(width))

/**
 * The document keeps the relative path that belongs on disk; only what the
 * webview loads is rewritten to an asset URL.
 *
 * Selecting the image puts a bar of sizes over it. The choice is written into
 * the note as `![|400](…)`, so it is an ordinary edit: undo takes it back and
 * the autosave carries it to disk.
 */
class ImageView {
  dom: HTMLSpanElement
  private image: HTMLImageElement
  private bar: HTMLElement | null = null

  constructor(
    private node: ProseNode,
    private view: EditorView,
    private getPos: () => number | undefined,
    private root: string,
    private noteFolder: string,
    private labels: { current: Labels },
  ) {
    this.dom = document.createElement("span")
    this.dom.className = "cortex-image"

    this.image = document.createElement("img")
    this.image.draggable = false
    this.dom.append(this.image)
    this.render()
  }

  update(node: ProseNode) {
    if (node.type !== this.node.type) return false
    this.node = node
    this.render()
    this.markChosen()
    return true
  }

  selectNode() {
    this.dom.dataset.selected = "true"
    if (!this.bar) {
      this.bar = this.buildBar()
      this.dom.append(this.bar)
    }
    this.markChosen()
  }

  deselectNode() {
    delete this.dom.dataset.selected
    this.bar?.remove()
    this.bar = null
  }

  /** The bar is furniture, not text: its clicks and its DOM are not the note's. */
  stopEvent(event: Event) {
    return this.bar?.contains(event.target as Node) ?? false
  }

  ignoreMutation() {
    return true
  }

  private render() {
    const source = String(this.node.attrs.src ?? "")
    this.image.src = source.startsWith("http")
      ? source
      : assetUrl(this.root, this.noteFolder ? `${this.noteFolder}/${source}` : source)

    const alt = this.node.attrs.alt as string | null
    this.image.alt = (alt ?? "").replace(/\|\d+$/, "")

    const width = imageWidth(alt)
    this.image.style.width = width === null ? "" : `${width}px`
    // The ceiling in the stylesheet is a default. A size picked by hand beats it,
    // or choosing "grande" on a tall screenshot would do nothing at all.
    this.image.style.maxHeight = width === null ? "" : "none"
  }

  private buildBar() {
    const bar = document.createElement("span")
    bar.className = "cortex-image-bar"
    bar.contentEditable = "false"

    for (const { label, width } of sizes) {
      const button = document.createElement("button")
      button.type = "button"
      button.textContent = this.labels.current[label]
      button.title = this.labels.current.size
      button.dataset.size = sizeKey(width)
      // On mousedown, and refused: a click would first take the selection off
      // the image, and the bar would be gone before it arrived.
      button.addEventListener("mousedown", (event) => {
        event.preventDefault()
        this.resize(width)
      })
      bar.append(button)
    }
    return bar
  }

  private markChosen() {
    const chosen = sizeKey(imageWidth(this.node.attrs.alt))
    for (const button of this.bar?.querySelectorAll("button") ?? []) {
      button.toggleAttribute("data-on", button.dataset.size === chosen)
    }
  }

  private resize(width: number | null) {
    const at = this.getPos()
    if (at === undefined) return

    const { tr } = this.view.state
    tr.setNodeMarkup(at, null, {
      ...this.node.attrs,
      alt: altWithWidth(this.node.attrs.alt as string | null, width),
    })
    // Keeps the image selected, so the four sizes can be tried one after another.
    tr.setSelection(NodeSelection.create(tr.doc, at))
    this.view.dispatch(tr)
    this.view.focus()
  }
}