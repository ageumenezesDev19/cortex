/**
 * Stands in for Transformers.js in the desktop build.
 *
 * The app embeds through fastembed in Rust; the browser library is the demo's
 * only. Aliasing it away keeps 22 MB of WebAssembly out of the .app, and the
 * throw is here so a future import from app code fails loudly at the first call
 * rather than silently returning nothing.
 */
export function pipeline(): never {
  throw new Error("Transformers.js só existe no demo web — o app embute pelo Rust")
}

export type FeatureExtractionPipeline = never
