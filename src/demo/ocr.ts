/**
 * Reading the text inside a screenshot, in the browser.
 *
 * The desktop hands this to Apple Vision through a Swift sidecar. On the web
 * the only thing that works everywhere is Tesseract compiled to WebAssembly —
 * the native `TextDetector` has been broken on iOS since 18.
 */

const LANGUAGES = { en: "eng", pt: "por" } as const

let worker: Promise<unknown> | null = null

/** Loaded on first use: ~2 MB of engine and training data nobody who only
 *  searches should have to download. */
async function ready(language: keyof typeof LANGUAGES) {
  worker ??= import("tesseract.js").then((tesseract) =>
    tesseract.createWorker(LANGUAGES[language]),
  )
  return worker as Promise<{
    recognize: (image: Blob) => Promise<{ data: { text: string } }>
  }>
}

/**
 * The text in an image, or empty when there is none to find.
 *
 * Failure is not an error here: a photo of a cat has no text, and a screenshot
 * that OCR cannot read is still worth keeping in the vault.
 */
export async function textIn(image: Blob, language: keyof typeof LANGUAGES): Promise<string> {
  try {
    const engine = await ready(language)
    const { data } = await engine.recognize(image)
    return data.text.trim()
  } catch {
    return ""
  }
}
