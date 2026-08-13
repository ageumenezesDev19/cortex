/** FTS5 wraps matched terms in these; they are not markdown, just our markers. */
const OPEN = "<<<"
const CLOSE = ">>>"

export type Segment = { text: string; found: boolean }

export function parseSnippet(snippet: string): Segment[] {
  const segments: Segment[] = []
  let rest = snippet

  while (rest.length > 0) {
    const start = rest.indexOf(OPEN)
    if (start === -1) break

    const end = rest.indexOf(CLOSE, start)
    if (end === -1) break

    if (start > 0) segments.push({ text: rest.slice(0, start), found: false })
    segments.push({ text: rest.slice(start + OPEN.length, end), found: true })
    rest = rest.slice(end + CLOSE.length)
  }
  if (rest.length > 0) segments.push({ text: rest, found: false })
  return segments
}

/**
 * Which OCR blocks the query landed in. Accent-insensitive to match the SQLite
 * tokenizer, so typing "codigo" still frames the line that reads "código".
 */
export function matchingBlocks<T extends { text: string }>(blocks: T[], query: string): T[] {
  const terms = query
    .split(/\s+/)
    .map(fold)
    .filter((term) => term.length > 0)
  if (terms.length === 0) return []

  return blocks.filter((block) => {
    const text = fold(block.text)
    return terms.some((term) => text.includes(term))
  })
}

function fold(value: string) {
  return value
    .normalize("NFD")
    .replace(/\p{Diacritic}/gu, "")
    .toLowerCase()
}
