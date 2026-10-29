/**
 * A screenshot dropped on the note that is open, rather than on the window.
 *
 * Tauri reports a drag on the window and hands over a position, never a target,
 * so where the file landed is decided once — in `App` — and announced to
 * whichever editor happens to be mounted, the rich one or the markdown textarea.
 */
export const DROP_ON_NOTE = "cortex:drop-on-note"

export type DropOnNote = {
  paths: string[]
  /** Where the pointer let go, in CSS pixels. */
  x: number
  y: number
}

/** The note surface under a point, or null when the drop missed one. */
export function noteSurfaceAt(x: number, y: number) {
  const element = document.elementFromPoint(x, y)
  return element instanceof Element
    ? element.closest<HTMLElement>("[data-note-surface]")
    : null
}

export function onDropOnNote(take: (drop: DropOnNote) => void) {
  const listener = (event: Event) => take((event as CustomEvent<DropOnNote>).detail)
  window.addEventListener(DROP_ON_NOTE, listener)
  return () => window.removeEventListener(DROP_ON_NOTE, listener)
}
