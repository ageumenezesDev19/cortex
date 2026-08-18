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
import { EditorState, Plugin, PluginKey } from "prosemirror-state"
import { Decoration, DecorationSet, EditorView } from "prosemirror-view"
import { useEffect, useRef } from "react"

import { assetUrl } from "../lib/api"
import { headingSpan, parse, schema, serialize } from "../lib/markdown"

type Props = {
  /** Only the initial document; typing does not flow back through this. */
  markdown: string
  root: string
  noteFolder: string
  /** Heading index to frame on arrival, or null when nothing was searched. */
  anchorHeading: number | null
  onChange: (markdown: string) => void
  onPasteImage: (file: File) => Promise<string | null>
}

const anchorKey = new PluginKey<DecorationSet>("cortex-anchor")

export function RichEditor({
  markdown,
  root,
  noteFolder,
  anchorHeading,
  onChange,
  onPasteImage,
}: Props) {
  const host = useRef<HTMLDivElement>(null)
  const view = useRef<EditorView | null>(null)

  // Callbacks change on every render of the parent; the view is built once, so
  // it reads them through a ref instead of being rebuilt.
  const handlers = useRef({ onChange, onPasteImage })
  handlers.current = { onChange, onPasteImage }

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
        image: (node) => new ImageView(node, root, noteFolder),
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

/**
 * The document keeps the relative path that belongs on disk; only what the
 * webview loads is rewritten to an asset URL.
 */
class ImageView {
  dom: HTMLImageElement

  constructor(node: ProseNode, root: string, noteFolder: string) {
    const source = String(node.attrs.src ?? "")
    this.dom = document.createElement("img")
    this.dom.src = source.startsWith("http")
      ? source
      : assetUrl(root, noteFolder ? `${noteFolder}/${source}` : source)
    this.dom.alt = String(node.attrs.alt ?? "")
    this.dom.draggable = false
  }
}