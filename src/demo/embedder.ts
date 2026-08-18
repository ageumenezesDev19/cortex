/**
 * Embedding the question, in the browser.
 *
 * Only the question — the passages were embedded at build time by the Rust
 * side, because a page that made a visitor wait for 30 passages before showing
 * a result would be a worse demo than one that showed nothing.
 *
 * The model is the ONNX port of the same `intfloat/multilingual-e5-small` the
 * desktop runs, so the vector this produces is comparable with the ones in
 * `demo-data.json`. A different model would return the same shape and rank
 * nonsense, which is the failure this comment exists to prevent.
 */

import { pipeline, type FeatureExtractionPipeline } from "@huggingface/transformers"

const MODEL = "Xenova/multilingual-e5-small"

let loading: Promise<FeatureExtractionPipeline> | null = null
/** Set once the weights are here. Read synchronously so a search can decide to
 *  answer now with words instead of waiting — see `embedQuestion`. */
let loaded: FeatureExtractionPipeline | null = null

/** Starts the download. Called when the page opens rather than on the first
 *  keystroke, so the weights are usually there before anyone types. */
export function warm(): void {
  loading ??= pipeline("feature-extraction", MODEL, {
    // Quantised: a quarter of the download for a difference in ranking that
    // thirty passages cannot show.
    dtype: "q8",
  }).then((ready) => {
    loaded = ready
    return ready
  })
}

/**
 * The question as a vector, or `null` while the model is still arriving.
 *
 * Returning null rather than waiting is deliberate: the word search answers
 * immediately and the results sharpen once the model lands. A spinner over an
 * empty page for the length of a model download is not a demo.
 */
export async function embedQuestion(question: string): Promise<Float32Array | null> {
  warm()
  if (!loaded) return null

  // `query: ` is not decoration — E5 was trained with it, and a question
  // embedded as though it were a passage lands somewhere slightly else.
  const output = await loaded(`query: ${question}`, { pooling: "mean", normalize: true })
  return output.data as Float32Array
}

/**
 * Passages, for something the visitor just wrote.
 *
 * `passage: ` where the question gets `query: ` — E5 was trained with both, and
 * mixing them puts a note somewhere its neighbours are not. Waits for the model
 * instead of returning null: an unembedded note would be silently missing from
 * every search by meaning.
 */
export async function embedPassages(texts: string[]): Promise<number[][]> {
  warm()
  const ready = await whenReady()
  if (!ready || !loaded) throw new Error("o modelo de busca não carregou")

  const out: number[][] = []
  // Sixteen at a time, for the reason `embed::BATCH` documents: a wide batch is
  // padded up to its longest text.
  for (let at = 0; at < texts.length; at += 16) {
    const slice = texts.slice(at, at + 16).map((text) => `passage: ${text}`)
    const result = await loaded(slice, { pooling: "mean", normalize: true })
    const rows = result.tolist() as number[][]
    out.push(...rows)
  }
  return out
}

/** Whether the weights have finished arriving, for the notice in the UI. */
export const ready = (): boolean => loaded !== null

/** Resolves when the model is usable, so a caller can re-run a search that
 *  answered with words alone. */
export function whenReady(): Promise<unknown> {
  warm()
  return loading ?? Promise.resolve()
}
