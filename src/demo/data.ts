/**
 * The demo's vault, built at compile time by `cargo run --example gerar_demo`.
 *
 * Passages were split by the app's own chunker and embedded by the same
 * multilingual-e5-small the desktop runs, so a vector here and a vector there
 * live in the same space. Nothing in this file is real: the notes are written
 * for the demo.
 */

import type { NoteContent, Topic, TreeEntry, VaultStatus } from "../lib/api"

export type Chunk = {
  chunkId: number
  path: string
  headingPath: string
  text: string
  start: number
  end: number
  vector: number[]
}

export type DemoData = {
  vault: VaultStatus
  topics: (Topic & { members: string[] })[]
  tree: TreeEntry[]
  notes: Record<string, NoteContent>
  chunks: Chunk[]
  dimensions: number
}

/** One per language, kept apart. The two vaults hold the same notes written in
 *  each language rather than one bilingual vault, because the model ranks a
 *  same-language passage above a same-meaning one — see `gerar_demo`. */
const loaded = new Map<string, Promise<DemoData>>()

/** Fetched rather than imported: 255 KB of vectors should not sit inside the
 *  bundle that has to parse before anything renders. */
export function data(language: "en" | "pt"): Promise<DemoData> {
  let waiting = loaded.get(language)
  if (!waiting) {
    waiting = fetch(`/demo-data-${language}.json`).then((response) => {
      if (!response.ok) throw new Error("could not load the demo vault")
      return response.json() as Promise<DemoData>
    })
    loaded.set(language, waiting)
  }
  return waiting
}
