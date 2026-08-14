import { Eye, Pencil } from "lucide-react"
import { useCallback, useEffect, useMemo, useRef, useState } from "react"
import ReactMarkdown from "react-markdown"
import remarkGfm from "remark-gfm"

import { assetUrl, getNote, pasteImage, writeNote, type NoteContent } from "../lib/api"
import { byteToCharIndex } from "../lib/offsets"
import { cn } from "../lib/utils"

/** Long enough not to write on every keystroke, short enough to forget about. */
const AUTOSAVE_MS = 600

type Props = {
  path: string
  /** Byte offset of the passage a search result pointed at, if it came from one. */
  anchorStart: number | null
  root: string
}

export function NotePanel({ path, anchorStart, root }: Props) {
  const [note, setNote] = useState<NoteContent | null>(null)
  const [draft, setDraft] = useState("")
  const [editing, setEditing] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const [dirty, setDirty] = useState(false)

  const editor = useRef<HTMLTextAreaElement>(null)
  const anchor = useRef<HTMLDivElement>(null)

  const load = useCallback(() => {
    getNote(path)
      .then((content) => {
        setNote(content)
        setDraft(content.source)
        setDirty(false)
        setError(null)
      })
      .catch((reason) => setError(String(reason)))
  }, [path])

  useEffect(() => {
    setNote(null)
    setEditing(false)
    load()
  }, [path, load])

  useEffect(() => {
    if (!editing) anchor.current?.scrollIntoView({ block: "center" })
  }, [note, editing])

  const save = useCallback(async () => {
    if (!dirty) return
    try {
      await writeNote(path, draft)
      setDirty(false)
    } catch (reason) {
      setError(String(reason))
    }
  }, [dirty, draft, path])

  useEffect(() => {
    if (!dirty) return
    const timer = setTimeout(save, AUTOSAVE_MS)
    return () => clearTimeout(timer)
  }, [dirty, draft, save])

  // Leaving edit mode reloads the note: the chunk boundaries that reading mode
  // anchors on were computed from the text before the edit.
  const toggleMode = useCallback(async () => {
    if (editing) {
      await save()
      load()
    }
    setEditing((value) => !value)
  }, [editing, load, save])

  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      if (!(event.metaKey || event.ctrlKey)) return
      if (event.key === "e") {
        event.preventDefault()
        void toggleMode()
      } else if (event.key === "s") {
        event.preventDefault()
        void save()
      }
    }
    window.addEventListener("keydown", onKeyDown)
    return () => window.removeEventListener("keydown", onKeyDown)
  }, [toggleMode, save])

  const onPaste = async (event: React.ClipboardEvent<HTMLTextAreaElement>) => {
    const file = Array.from(event.clipboardData.items)
      .find((item) => item.type.startsWith("image/"))
      ?.getAsFile()
    if (!file) return

    event.preventDefault()
    const bytes = Array.from(new Uint8Array(await file.arrayBuffer()))
    const extension = file.type.split("/")[1] ?? "png"

    try {
      const markdown = await pasteImage(path, bytes, extension)
      const field = editor.current
      const at = field?.selectionStart ?? draft.length
      setDraft(`${draft.slice(0, at)}${markdown}\n${draft.slice(at)}`)
      setDirty(true)
    } catch (reason) {
      setError(String(reason))
    }
  }

  if (error && !note) {
    return <p className="font-mono text-sm text-muted-foreground">{error}</p>
  }
  if (!note) return null

  return (
    <div className="flex h-full flex-col">
      <header className="mb-3 flex shrink-0 items-center gap-3">
        <span className="min-w-0 flex-1">
          <span className="block truncate font-medium">{note.title}</span>
          <span className="block truncate font-mono text-xs text-muted-foreground">
            {note.path}
          </span>
        </span>

        <span className="font-mono text-xs text-muted-foreground">
          {dirty ? "salvando…" : "salvo"}
        </span>

        <button
          type="button"
          onClick={() => void toggleMode()}
          title={editing ? "Ler (Cmd+E)" : "Editar (Cmd+E)"}
          aria-label={editing ? "Ler" : "Editar"}
          className="flex items-center gap-1.5 rounded-md px-2 py-1 font-mono text-xs text-muted-foreground transition-colors hover:bg-muted hover:text-foreground"
        >
          {editing ? <Eye className="size-3.5" aria-hidden /> : <Pencil className="size-3.5" aria-hidden />}
          {editing ? "ler" : "editar"}
        </button>
      </header>

      {editing ? (
        <textarea
          ref={editor}
          value={draft}
          onChange={(event) => {
            setDraft(event.target.value)
            setDirty(true)
          }}
          onPaste={onPaste}
          spellCheck={false}
          aria-label={`Editando ${note.title}`}
          className="min-h-0 flex-1 resize-none rounded-md border bg-background p-3 font-mono text-sm leading-relaxed outline-none focus:border-ring"
        />
      ) : (
        <article className="min-h-0 flex-1 overflow-y-auto pr-1">
          {note.chunks.map((chunk) => {
            const found = anchorStart !== null && chunk.start === anchorStart
            const slice = note.source.slice(
              byteToCharIndex(note.source, chunk.start),
              byteToCharIndex(note.source, chunk.end),
            )

            return (
              <div
                key={chunk.start}
                ref={found ? anchor : undefined}
                className={cn(
                  "rounded-md px-2 py-1",
                  found && "bg-found/10 shadow-[inset_0_0_0_1px_hsl(var(--found))]",
                )}
              >
                <Markdown source={slice} noteFolder={folderOf(note.path)} root={root} />
              </div>
            )
          })}
        </article>
      )}
    </div>
  )
}

/**
 * `rehype-raw` is deliberately absent: a note is often pasted from somewhere
 * else, and third-party HTML does not get to become live markup here.
 */
function Markdown({
  source,
  noteFolder,
  root,
}: {
  source: string
  noteFolder: string
  root: string
}) {
  const components = useMemo(
    () => ({
      h1: (props: object) => <h1 className="mb-3 mt-2 text-2xl font-semibold" {...props} />,
      h2: (props: object) => <h2 className="mb-2 mt-5 text-lg font-semibold" {...props} />,
      h3: (props: object) => <h3 className="mb-2 mt-4 font-medium" {...props} />,
      p: (props: object) => <p className="my-2 leading-relaxed" {...props} />,
      ul: (props: object) => <ul className="my-2 list-disc space-y-1 pl-5" {...props} />,
      ol: (props: object) => <ol className="my-2 list-decimal space-y-1 pl-5" {...props} />,
      blockquote: (props: object) => (
        <blockquote className="my-3 border-l-2 pl-3 text-muted-foreground" {...props} />
      ),
      code: (props: object) => (
        <code className="rounded bg-muted px-1 py-0.5 font-mono text-[0.85em]" {...props} />
      ),
      pre: (props: object) => (
        <pre
          className="my-3 overflow-x-auto rounded-md border bg-muted/50 p-3 font-mono text-xs leading-relaxed [&_code]:bg-transparent [&_code]:p-0"
          {...props}
        />
      ),
      table: (props: object) => (
        <div className="my-3 overflow-x-auto">
          <table className="w-full text-sm" {...props} />
        </div>
      ),
      th: (props: object) => <th className="border-b px-2 py-1 text-left font-medium" {...props} />,
      td: (props: object) => <td className="border-b px-2 py-1" {...props} />,
      // Links stay text: opening a browser would be another app doing the work.
      a: (props: object) => <span className="text-found underline underline-offset-2" {...props} />,
      img: ({ src, alt }: { src?: string; alt?: string }) => {
        if (!src) return null
        const resolved = src.startsWith("http")
          ? src
          : assetUrl(root, noteFolder ? `${noteFolder}/${src}` : src)
        return (
          <img
            src={resolved}
            alt={alt ?? ""}
            className="my-3 max-w-full rounded-md border"
            draggable={false}
          />
        )
      },
    }),
    [noteFolder, root],
  )

  return (
    <ReactMarkdown remarkPlugins={[remarkGfm]} components={components}>
      {source}
    </ReactMarkdown>
  )
}

function folderOf(path: string) {
  const cut = path.lastIndexOf("/")
  return cut === -1 ? "" : path.slice(0, cut)
}