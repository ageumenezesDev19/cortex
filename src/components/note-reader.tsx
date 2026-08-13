import { useEffect, useRef, useState } from "react"

import { getNote, type Hit, type NoteContent } from "../lib/api"
import { byteToCharIndex } from "../lib/offsets"

/**
 * Reading is raw markdown in mono until M6 decides on a reading face. That is
 * not a placeholder: the file on disk is the source of truth, and showing it
 * verbatim is what makes the anchor jump verifiable.
 */
export function NoteReader({ hit }: { hit: Hit }) {
  const [note, setNote] = useState<NoteContent | null>(null)
  const [error, setError] = useState<string | null>(null)
  const mark = useRef<HTMLElement>(null)

  const path = hit.notePath ?? hit.path

  useEffect(() => {
    let active = true
    setNote(null)
    setError(null)

    getNote(path)
      .then((content) => active && setNote(content))
      .catch((reason) => active && setError(String(reason)))

    return () => {
      active = false
    }
  }, [path])

  useEffect(() => {
    mark.current?.scrollIntoView({ block: "center" })
  }, [note])

  if (error) {
    return <p className="font-mono text-sm text-muted-foreground">{error}</p>
  }
  if (!note) return null

  const start = byteToCharIndex(note.source, hit.startOffset)
  const end = byteToCharIndex(note.source, hit.endOffset)

  return (
    <article className="h-full overflow-y-auto">
      <pre className="whitespace-pre-wrap break-words px-1 font-mono text-sm leading-relaxed">
        <span className="text-muted-foreground">{note.source.slice(0, start)}</span>
        <mark
          ref={mark}
          className="rounded-sm bg-found/15 text-foreground shadow-[inset_0_0_0_1px_hsl(var(--found))]"
        >
          {note.source.slice(start, end)}
        </mark>
        <span className="text-muted-foreground">{note.source.slice(end)}</span>
      </pre>
    </article>
  )
}
