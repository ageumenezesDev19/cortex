import { convertFileSrc, invoke } from "@tauri-apps/api/core"

/** `[x, y, width, height]`, normalised 0–1 with the origin at the top left. */
export type Bbox = [number, number, number, number]

export type OcrBlock = {
  text: string
  bbox: Bbox
  confidence: number
}

/** One matching passage — a stretch of a note, or the text inside a screenshot. */
export type Hit = {
  chunkId: number
  kind: "text" | "image"
  path: string
  headingPath: string
  snippet: string
  startOffset: number
  endOffset: number
  blocks: OcrBlock[]
}

/** The passages of one note (or one orphan screenshot), gathered under it. */
export type Group = {
  key: string
  kind: "note" | "image"
  path: string
  title: string
  folder: string
  /** Everything that matched, not only what `hits` carries. */
  total: number
  hits: Hit[]
}

/** A heading-delimited slice of the raw source, as the index sees it. */
export type ChunkRange = {
  headingPath: string
  start: number
  end: number
}

export type NoteContent = {
  path: string
  title: string
  source: string
  chunks: ChunkRange[]
}

export type TreeEntry = {
  path: string
  name: string
  kind: "folder" | "note" | "image"
  children: TreeEntry[]
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

export type Topic = {
  slug: string
  name: string
  icon: string | null
  color: string | null
  members: string[]
}

export const INDEX_STATUS_EVENT = "index:status"

export const search = (query: string, topic: string | null = null, limit = 40) =>
  invoke<Group[]>("search", { query, topic, limit })

export const getNote = (path: string) => invoke<NoteContent>("get_note", { path })

export const vaultStatus = () => invoke<VaultStatus>("vault_status")

export const importImages = (paths: string[]) => invoke<number>("import_images", { paths })

export const listTree = () => invoke<TreeEntry[]>("list_tree")

export const createNote = (folder: string, title: string) =>
  invoke<string>("create_note", { folder, title })

export const writeNote = (path: string, source: string) =>
  invoke<void>("write_note", { path, source })

export const createFolder = (parent: string, name: string) =>
  invoke<string>("create_folder", { parent, name })

export const renameEntry = (path: string, name: string) =>
  invoke<string>("rename_entry", { path, name })

export const deleteEntry = (path: string) => invoke<void>("delete_entry", { path })

/** Returns the markdown link to drop at the cursor. */
export const pasteImage = (notePath: string, bytes: number[], extension: string) =>
  invoke<string>("paste_image", { notePath, bytes, extension })

export const listTopics = () => invoke<Topic[]>("list_topics")

export const createTopic = (name: string, icon: string | null, color: string | null) =>
  invoke<Topic>("create_topic", { name, icon, color })

export const updateTopic = (
  slug: string,
  name: string,
  icon: string | null,
  color: string | null,
) => invoke<Topic>("update_topic", { slug, name, icon, color })

export const deleteTopic = (slug: string) => invoke<void>("delete_topic", { slug })

export const addToTopic = (slug: string, path: string) =>
  invoke<Topic>("add_to_topic", { slug, path })

export const removeFromTopic = (slug: string, path: string) =>
  invoke<Topic>("remove_from_topic", { slug, path })

export const assetUrl = (root: string, relativePath: string) =>
  convertFileSrc(`${root}/${relativePath}`)
