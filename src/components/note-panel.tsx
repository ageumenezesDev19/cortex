import { Code2, TriangleAlert, Type } from "lucide-react"
import { useCallback, useEffect, useMemo, useRef, useState } from "react"

import { getNote, pasteImage, writeNote, type NoteContent } from "../lib/api"
import { headingIndexAt, survivesRoundTrip } from "../lib/markdown"
import { RichEditor } from "./rich-editor"

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
  const [raw, setRaw] = useState(false)
  const [lossy, setLossy] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const [dirty, setDirty] = useState(false)

  const editor = useRef<HTMLTextAreaElement>(null)
  const latest = useRef("")
  latest.current = draft

  useEffect(() => {
    let active = true
    setNote(null)

    getNote(path)
      .then((content) => {
        if (!active) return
        // A note the round trip would damage never enters the rich editor.
        const survives = survivesRoundTrip(content.source)
        setNote(content)
        setDraft(content.source)
        setLossy(!survives)
        setRaw(!survives)
        setDirty(false)
        setError(null)
      })
      .catch((reason) => active && setError(String(reason)))

    return () => {
      active = false
    }
  }, [path])

  const save = useCallback(async () => {
    try {
      await writeNote(path, latest.current)
      setDirty(false)
    } catch (reason) {
      setError(String(reason))
    }
  }, [path])

  useEffect(() => {
    if (!dirty) return
    const timer = setTimeout(save, AUTOSAVE_MS)
    return () => clearTimeout(timer)
  }, [dirty, draft, save])

  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      if (!(event.metaKey || event.ctrlKey)) return
      if (event.key === "e" && !lossy) {
        event.preventDefault()
        setRaw((value) => !value)
      } else if (event.key === "s") {
        event.preventDefault()
        void save()
      }
    }
    window.addEventListener("keydown", onKeyDown)
    return () => window.removeEventListener("keydown", onKeyDown)
  }, [save, lossy])

  const change = useCallback((markdown: string) => {
    setDraft(markdown)
    setDirty(true)
  }, [])

  const onPasteImage = useCallback(
    async (file: File) => {
      const bytes = Array.from(new Uint8Array(await file.arrayBuffer()))
      const extension = file.type.split("/")[1] ?? "png"
      try {
        const markdown = await pasteImage(path, bytes, extension)
        // `![](_assets/print-1.png)` — the editor only needs what is in the parens.
        return markdown.slice(markdown.indexOf("(") + 1, markdown.lastIndexOf(")"))
      } catch (reason) {
        setError(String(reason))
        return null
      }
    },
    [path],
  )

  const anchorHeading = useMemo(
    () => (note && anchorStart !== null ? headingIndexAt(note.source, anchorStart) : null),
    [note, anchorStart],
  )

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

        {!lossy && (
          <button
            type="button"
            onClick={() => setRaw((value) => !value)}
            title={raw ? "Texto (Cmd+E)" : "Markdown (Cmd+E)"}
            aria-label={raw ? "Ver como texto" : "Ver o markdown"}
            className="flex items-center gap-1.5 rounded-md px-2 py-1 font-mono text-xs text-muted-foreground transition-colors hover:bg-muted hover:text-foreground"
          >
            {raw ? <Type className="size-3.5" aria-hidden /> : <Code2 className="size-3.5" aria-hidden />}
            {raw ? "texto" : "markdown"}
          </button>
        )}
      </header>

      {lossy && (
        <p className="mb-3 flex shrink-0 items-start gap-2 rounded-md border border-found/40 bg-found/10 px-3 py-2 text-xs text-muted-foreground">
          <TriangleAlert className="mt-0.5 size-3.5 shrink-0 text-found" aria-hidden />
          <span>
            Esta nota tem marcação que o editor de texto não sabe reescrever sem estragar — uma
            tabela, provavelmente. Ela abre no markdown, que preserva tudo.
          </span>
        </p>
      )}

      {raw ? (
        <textarea
          ref={editor}
          value={draft}
          onChange={(event) => change(event.target.value)}
          spellCheck={false}
          data-note-surface="true"
          aria-label={`Markdown de ${note.title}`}
          className="min-h-0 flex-1 resize-none rounded-md border bg-background p-3 font-mono text-sm leading-relaxed outline-none focus:border-ring"
        />
      ) : (
        <div className="min-h-0 flex-1">
          <RichEditor
            key={path}
            markdown={note.source}
            root={root}
            noteFolder={folderOf(note.path)}
            anchorHeading={anchorHeading}
            onChange={change}
            onPasteImage={onPasteImage}
          />
        </div>
      )}
    </div>
  )
}

function folderOf(path: string) {
  const cut = path.lastIndexOf("/")
  return cut === -1 ? "" : path.slice(0, cut)
}