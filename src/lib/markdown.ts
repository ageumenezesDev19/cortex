import type { Node as ProseNode } from "prosemirror-model"
import {
  defaultMarkdownParser,
  defaultMarkdownSerializer,
  MarkdownSerializer,
  schema,
} from "prosemirror-markdown"

export { schema }

/**
 * `-` for bullets because that is what the vault already uses; the default `*`
 * would rewrite every list the first time a note is saved, for nothing.
 */
export const serializer = new MarkdownSerializer(
  {
    ...defaultMarkdownSerializer.nodes,
    bullet_list(state, node) {
      state.renderList(node, "  ", () => "- ")
    },
  },
  defaultMarkdownSerializer.marks,
)

export const parse = (source: string): ProseNode =>
  defaultMarkdownParser.parse(source) ?? schema.node("doc", null, [schema.node("paragraph")])

export const serialize = (doc: ProseNode): string => serializer.serialize(doc)

/**
 * A rich editor rewrites the whole file on save. Anything the schema does not
 * know — a GFM table is the obvious one — would come back as flattened text and
 * the save would commit that loss.
 *
 * So no note enters the rich editor without proving it survives the trip first.
 * Trailing whitespace is not a loss; anything else is.
 */
export type RoundTrip =
  | { survives: true }
  /** `line` is 1-based, counted in the file as written. */
  | { survives: false; line: number; text: string }

export function roundTripReport(source: string): RoundTrip {
  let back: string
  try {
    back = serialize(parse(source))
  } catch {
    return { survives: false, line: 1, text: source.split("\n")[0] ?? "" }
  }

  const before = normalise(source).split("\n")
  const after = normalise(back).split("\n")
  const at = before.findIndex((line, index) => line !== after[index])
  if (at === -1 && before.length === after.length) return { survives: true }

  // The comparison runs on the normalised text, so the line number is reported
  // against the file the person will actually look at.
  const culprit = before[at] ?? before[before.length - 1] ?? ""
  const original = source.split("\n")
  const line = original.findIndex((raw) => raw.trim() === culprit.trim())

  return { survives: false, line: (line === -1 ? at : line) + 1, text: culprit.trim() }
}

export const survivesRoundTrip = (source: string) => roundTripReport(source).survives

/**
 * Compares meaning, not bytes. A paragraph hand-wrapped at 80 columns and the
 * same paragraph on one long line are the same markdown, and the serializer does
 * not re-wrap — without this every hand-wrapped note would be called lossy and
 * pushed into raw mode.
 *
 * Loss still shows: anything the schema drops comes back with escapes (a table's
 * pipes become `\|`), which no amount of joining hides.
 */
function normalise(markdown: string) {
  const lines = markdown.split("\n")
  const out: string[] = []
  let inFence = false

  for (const raw of lines) {
    const line = raw.replace(/\s+$/, "")
    if (/^\s*(```|~~~)/.test(line)) {
      inFence = !inFence
      out.push(line)
      continue
    }
    // Inside a fence every newline counts. Outside, a soft wrap is a space —
    // but only for a line that continues a paragraph. A line opening a block
    // keeps its own line, or a table's rows would fold into one string and the
    // guard would stop seeing the table disappear.
    const previous = out[out.length - 1]
    const continues =
      !inFence && line !== "" && previous !== undefined && previous !== "" && !isBlockStart(line)

    if (continues && !isFence(previous)) {
      out[out.length - 1] = `${previous} ${line.trim()}`
    } else {
      out.push(line)
    }
  }

  return out.join("\n").replace(/\n{3,}/g, "\n\n").trim()
}

const isFence = (line: string) => /^\s*(```|~~~)/.test(line)

const isBlockStart = (line: string) =>
  /^\s*(#{1,6}\s|>|[-*+]\s|\d+[.)]\s|\||-{3,}\s*$|={3,}\s*$)/.test(line)

/**
 * Byte offsets index the file; they mean nothing inside the document. Headings
 * survive the conversion in order, so counting them is exact and needs no text
 * comparison: the nth heading in the source is the nth heading in the document.
 */
export function headingIndexAt(source: string, byteOffset: number): number {
  const upTo = new TextDecoder().decode(new TextEncoder().encode(source).slice(0, byteOffset))
  let index = 0
  let inFence = false

  for (const line of upTo.split("\n")) {
    if (/^\s*(```|~~~)/.test(line)) inFence = !inFence
    else if (!inFence && /^#{1,6}\s/.test(line)) index += 1
  }
  return index
}

/** The span of the nth heading and everything under it, in document positions. */
export function headingSpan(doc: ProseNode, headingIndex: number): { from: number; to: number } {
  if (headingIndex === 0) {
    const firstHeading = findHeadingPositions(doc)[0]
    return { from: 0, to: firstHeading ?? doc.content.size }
  }

  const headings = findHeadingPositions(doc)
  const from = headings[headingIndex - 1]
  if (from === undefined) return { from: 0, to: 0 }

  return { from, to: headings[headingIndex] ?? doc.content.size }
}

function findHeadingPositions(doc: ProseNode): number[] {
  const positions: number[] = []
  doc.descendants((node, pos) => {
    if (node.type.name === "heading") positions.push(pos)
    return false
  })
  return positions
}