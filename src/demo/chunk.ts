/**
 * Splitting a note at its headings, for the browser.
 *
 * A port of `parse::chunk_markdown`, not a share: that one runs pulldown-cmark
 * in Rust. A note written in the demo can therefore land on slightly different
 * boundaries than the desktop would give it — the same divergence the ranking
 * already has, and worth knowing rather than hiding.
 */

/** `parse::HEADING_SEPARATOR`. */
const SEPARATOR = " › "

export type Chunk = {
  headingPath: string
  /** UTF-8 bytes, like Rust — `byteToCharIndex` converts at the other end. */
  start: number
  end: number
  text: string
}

const encoder = new TextEncoder()
const bytes = (text: string) => encoder.encode(text).length

/** Markdown syntax removed, which is what gets indexed and shown in snippets. */
function plain(line: string): string {
  return line
    .replace(/^\s{0,3}>\s?/, "")
    .replace(/^\s*([-*+]|\d+[.)])\s+/, "")
    .replace(/!\[([^\]]*)\]\([^)]*\)/g, "$1")
    .replace(/\[([^\]]*)\]\([^)]*\)/g, "$1")
    .replace(/`([^`]*)`/g, "$1")
    .replace(/(\*\*|__)(.*?)\1/g, "$2")
    .replace(/(\*|_)(.*?)\1/g, "$2")
    .replace(/~~(.*?)~~/g, "$1")
    .trim()
}

export function chunkMarkdown(source: string): Chunk[] {
  const chunks: Chunk[] = []
  const stack: { level: number; text: string }[] = []

  let headingPath = ""
  let openedWith = ""
  let start = 0
  let collected: string[] = []
  let fenced = false
  let at = 0

  const flush = (end: number) => {
    const text = collected.join("\n").trim()
    collected = []
    // A chunk that is nothing but its own heading says nothing.
    if (text.length === 0 || end <= start || text === openedWith) return
    chunks.push({ headingPath, start, end, text })
  }

  for (const line of source.split("\n")) {
    const width = bytes(line) + 1
    if (/^\s*(```|~~~)/.test(line)) fenced = !fenced

    const heading = fenced ? null : /^(#{1,6})\s+(.*)$/.exec(line)
    if (heading) {
      flush(at)
      start = at

      const level = heading[1].length
      const text = plain(heading[2])
      while (stack.length > 0 && stack[stack.length - 1].level >= level) stack.pop()
      stack.push({ level, text })
      headingPath = stack.map((one) => one.text).join(SEPARATOR)

      // The heading is part of its own chunk's searchable text.
      collected.push(text)
      openedWith = text
    } else {
      const text = plain(line)
      if (text.length > 0) collected.push(text)
    }
    at += width
  }

  flush(bytes(source))
  return chunks
}
