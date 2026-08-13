import { convertFileSrc, invoke } from "@tauri-apps/api/core"

/** `[x, y, width, height]`, normalised 0–1 with the origin at the top left. */
export type Bbox = [number, number, number, number]

export type OcrBlock = {
  text: string
  bbox: Bbox
  confidence: number
}

export type Hit = {
  chunkId: number
  kind: "text" | "image"
  path: string
  title: string
  notePath: string | null
  noteTitle: string | null
  headingPath: string
  snippet: string
  startOffset: number
  endOffset: number
  blocks: OcrBlock[]
}

export type NoteContent = {
  path: string
  title: string
  source: string
}

export type VaultStatus = {
  root: string
  notes: number
  images: number
  chunks: number
}

export type IndexStatus = {
  phase: "scanning" | "ocr" | "idle"
  counts: { notes: number; images: number; chunks: number }
  current: string | null
  pending: number
}

export const INDEX_STATUS_EVENT = "index:status"

export const search = (query: string, limit = 40) => invoke<Hit[]>("search", { query, limit })

export const getNote = (path: string) => invoke<NoteContent>("get_note", { path })

export const vaultStatus = () => invoke<VaultStatus>("vault_status")

export const importImages = (paths: string[]) => invoke<number>("import_images", { paths })

export const assetUrl = (root: string, relativePath: string) =>
  convertFileSrc(`${root}/${relativePath}`)
