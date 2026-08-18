/**
 * The demo's ranking, reimplemented for the browser.
 *
 * This is NOT the app's search. The desktop ranks with SQLite FTS5 and fuses
 * that with the vectors in Rust; none of it compiles to a web page. What is
 * shared is everything before the ranking — the passages were split by the
 * app's own `parse::chunk_markdown` and embedded by the same model — and the
 * fusion, which is the same reciprocal rank formula with the same constant.
 *
 * So the demo finds the same passages; it may order two near-ties differently
 * than the app would. For thirty passages that difference is invisible, and
 * pretending otherwise in a portfolio piece would be the wrong trade.
 */

/** All the ranking needs. Structural on purpose: the passages come from the
 *  seeded file on the first visit and from IndexedDB after that. */
type Chunk = { text: string; vector: number[] }

/** The constant from `embed::fuse`. 60 is what the original paper used. */
const RRF_K = 60

/** Accents folded the way SQLite's `remove_diacritics 2` folds them, so that
 *  searching "producao" still finds "produção" — the vault is Portuguese. */
export function fold(text: string): string {
  return text
    .toLowerCase()
    .normalize("NFD")
    .replace(/[̀-ͯ]/g, "")
}

const words = (text: string) => fold(text).split(/[^\p{L}\p{N}]+/u).filter(Boolean)

/** Words that carry no search intent. The app drops these too — that is what
 *  `Found.terms` means by "once the filler was dropped". */
const FILLER = new Set([
  "a", "o", "as", "os", "um", "uma", "de", "do", "da", "dos", "das", "em", "no", "na",
  "por", "que", "como", "para", "pra", "com", "meu", "minha", "eu", "e", "ou", "se",
  "nao", "muito", "mais", "ser", "esta", "estou", "fica", "ficou", "tem", "ter",
  "the", "an", "of", "to", "in", "on", "for", "and", "or", "is", "are", "my", "i",
  "why", "how", "what", "does", "do", "did", "so", "much", "many", "should", "would",
  "not", "no", "it", "its", "that", "this", "be", "was", "were", "have", "has",
])

export const meaningful = (query: string) => words(query).filter((word) => !FILLER.has(word))

/**
 * Passages ranked by the words themselves, and whether the query had to be
 * loosened to find any.
 *
 * Every term first, any term only if that found nothing — the same order the
 * desktop uses, and the reason it reports `widened`. Accepting any term from
 * the start is what made "custa caro" rank a note about fixed costs above the
 * one actually about switching tasks: one loose match earned the top rank and
 * then beat the right answer on the tie.
 */
function byWords(chunks: Chunk[], query: string): { order: number[]; widened: boolean } {
  const asked = meaningful(query)
  if (asked.length === 0) return { order: [], widened: false }

  const score = (chunk: Chunk, terms: string[]) => {
    const inside = words(chunk.text)
    if (inside.length === 0) return { hits: 0, score: 0 }

    let hits = 0
    let score = 0
    for (const term of terms) {
      // A prefix only counts from four letters up: "custa" reaching "custo"
      // is noise, "ilumin" reaching "iluminação" is the point.
      const found = inside.filter(
        (word) => word === term || (term.length >= 4 && word.startsWith(term)),
      ).length
      if (found > 0) {
        hits += 1
        score += (1 + Math.log(found)) / Math.log(inside.length + 2)
      }
    }
    return { hits, score }
  }

  const scored = chunks.map((chunk, at) => ({ at, ...score(chunk, asked) }))
  const every = scored.filter((one) => one.hits === asked.length)
  const chosen = every.length > 0 ? every : scored.filter((one) => one.hits > 0)

  return {
    order: chosen.sort((a, b) => b.score - a.score).map((one) => one.at),
    widened: every.length === 0 && chosen.length > 0,
  }
}

function cosine(a: Float32Array | number[], b: Float32Array | number[]): number {
  let dot = 0
  let left = 0
  let right = 0
  for (let at = 0; at < a.length; at++) {
    dot += a[at] * b[at]
    left += a[at] * a[at]
    right += b[at] * b[at]
  }
  const scale = Math.sqrt(left) * Math.sqrt(right)
  return scale === 0 ? 0 : dot / scale
}

/** Passage indices nearest the question's vector, best first. */
function byMeaning(chunks: Chunk[], asked: Float32Array): number[] {
  return chunks
    .map((chunk, at) => ({ at, score: cosine(asked, chunk.vector) }))
    .sort((a, b) => b.score - a.score)
    .map((one) => one.at)
}

/**
 * Merges the two rankings by position rather than by score.
 *
 * A cosine and a word count are not comparable numbers; their *ranks* are.
 * A passage near the top of either list ends up near the top; one in both
 * wins. Ported from `embed::fuse`.
 */
export function fuse(lists: { order: number[]; weight: number }[]): number[] {
  const scored = new Map<number, number>()
  for (const { order, weight } of lists) {
    order.forEach((at, rank) => {
      scored.set(at, (scored.get(at) ?? 0) + weight / (RRF_K + rank + 1))
    })
  }
  return [...scored.entries()].sort((a, b) => b[1] - a[1]).map(([at]) => at)
}

/**
 * The whole ranking. `asked` is null when the model has not loaded yet, and
 * the words alone still answer — the search box works while the 100 MB of
 * weights are still arriving.
 */
export function rank(
  chunks: Chunk[],
  query: string,
  asked: Float32Array | null,
): { order: number[]; widened: boolean } {
  const { order: words, widened } = byWords(chunks, query)
  if (!asked) return { order: words, widened }

  // A third of the vault rather than two thirds: past that the list is no
  // longer "what this could mean", it is everything, and RRF hands real points
  // to passages that merely failed to be last.
  const meaning = byMeaning(chunks, asked).slice(0, Math.max(5, Math.ceil(chunks.length / 3)))

  // A widened word list is the search saying "nothing matched everything you
  // asked, here is what shares a word". That is a guess, and giving a guess the
  // same authority as a strong vector match is what put a note about saved
  // screenshots above the one about typography: both were rank one, and the
  // guess was listed first.
  return {
    order: fuse([
      { order: words, weight: widened ? 0.35 : 1 },
      { order: meaning, weight: 1 },
    ]),
    widened,
  }
}

