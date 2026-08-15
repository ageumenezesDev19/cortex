import { convertFileSrc, invoke, isTauri } from "@tauri-apps/api/core"
import { listen } from "@tauri-apps/api/event"

/**
 * The same app runs in two places: inside the Tauri window on the Mac, and in
 * Safari on the phone over the tailnet. Only this file knows the difference —
 * everything above it calls the same functions either way.
 */
const insideApp = () => isTauri() || "__TAURI_INTERNALS__" in window

/**
 * Asked on every call rather than decided once when this module loaded: if the
 * window ever answers "browser" for a moment, every call goes to the dev server
 * and the app comes up with an empty vault and dead buttons.
 */
export const isDesktop = insideApp()

/**
 * `POST /api/<command>` carries exactly the arguments `invoke` would send, so a
 * command is written once and both transports reach it.
 */
async function post<T>(command: string, args: Record<string, unknown> = {}): Promise<T> {
  const response = await fetch(`/api/${command}`, {
    method: "POST",
    headers: { "content-type": "application/json" },
    // The session is a cookie: an <img> cannot send a header, and every
    // screenshot in the vault is loaded by one.
    credentials: "same-origin",
    body: JSON.stringify(args),
  })
  if (!response.ok) throw new Error((await response.text()) || response.statusText)
  return response.status === 204 ? (undefined as T) : ((await response.json()) as T)
}

const call = <T>(command: string, args: Record<string, unknown> = {}): Promise<T> =>
  insideApp() ? invoke<T>(command, args) : post<T>(command, args)

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
  /** Filled in by the app from what a screenshot turned out to be. */
  auto: boolean
  members: string[]
}

export const INDEX_STATUS_EVENT = "index:status"

export const search = (query: string, topic: string | null = null, limit = 40) =>
  call<Group[]>("search", { query, topic, limit })

export const getNote = (path: string) => call<NoteContent>("get_note", { path })

export const vaultStatus = () => call<VaultStatus>("vault_status")

/** Returns the vault-relative paths it wrote, so the notice can name them. */
export const importImages = (paths: string[]) =>
  call<string[]>("import_images", { paths })

/** An image opened from the tree, where no search result carries its boxes. */
export type AssetContent = {
  path: string
  blocks: OcrBlock[]
}

export const getAsset = (path: string) => call<AssetContent>("get_asset", { path })

export const listTree = () => call<TreeEntry[]>("list_tree")

export const IMPORT_EVENT = "import:status"

export type ImportPreview = {
  notes: number
  folders: number
  /** Brought over in an earlier run; these get skipped. */
  already: number
}

export type ImportReport = {
  imported: number
  skipped: number
  failed: number
  attachments: number
  /** Edited here after being imported, so the re-import left them alone. */
  kept: number
}

export type ImportStatus = {
  phase: "running" | "done" | "failed"
  done: number
  total: number
  current: string
  report: ImportReport | null
  error: string | null
}

export const importPreview = () => call<ImportPreview>("import_preview")

export const importNotes = () => call<void>("import_notes")

/** Where a note by this name probably belongs, judged by what the vault has. */
export type Placement = {
  folder: string | null
  topics: string[]
}

export const suggestPlacement = (title: string) =>
  call<Placement>("suggest_placement", { title })

export const createNote = (
  folder: string,
  title: string,
  topics: string[] = [],
  /** A dropped screenshot to move next to the note and embed at the top. */
  embed: string | null = null,
) => call<string>("create_note", { folder, title, topics, embed })

export const writeNote = (path: string, source: string) =>
  call<void>("write_note", { path, source })

export const createFolder = (parent: string, name: string) =>
  call<string>("create_folder", { parent, name })

export const renameEntry = (path: string, name: string) =>
  call<string>("rename_entry", { path, name })

export const deleteEntry = (path: string) => call<void>("delete_entry", { path })

/** Returns the markdown link to drop at the cursor. */
export const pasteImage = (notePath: string, bytes: number[], extension: string) =>
  call<string>("paste_image", { notePath, bytes, extension })

export const listTopics = () => call<Topic[]>("list_topics")

export const createTopic = (name: string, icon: string | null, color: string | null) =>
  call<Topic>("create_topic", { name, icon, color })

export const updateTopic = (
  slug: string,
  name: string,
  icon: string | null,
  color: string | null,
) => call<Topic>("update_topic", { slug, name, icon, color })

export const deleteTopic = (slug: string) => call<void>("delete_topic", { slug })

export const addToTopic = (slug: string, path: string) =>
  call<Topic>("add_to_topic", { slug, path })

export const removeFromTopic = (slug: string, path: string) =>
  call<Topic>("remove_from_topic", { slug, path })

/**
 * On the desktop the webview reads the file straight off disk; on the phone it
 * comes down the same session as everything else.
 */
export const assetUrl = (root: string, relativePath: string) =>
  insideApp() ? convertFileSrc(`${root}/${relativePath}`) : `/asset/${encode(relativePath)}`

const encode = (path: string) => path.split("/").map(encodeURIComponent).join("/")

/**
 * Sends a photo raw. As a JSON array of numbers — what `pasteImage` needs — a
 * 4 MB picture costs 12 MB of text on the way up.
 *
 * `note` empty drops it loose in the vault, the same place a screenshot dropped
 * on the window with no note open lands.
 */
export async function uploadImage(file: File, note = ""): Promise<string> {
  const ext = (file.name.split(".").pop() || file.type.split("/")[1] || "png").toLowerCase()
  const response = await fetch(
    `/upload?note=${encodeURIComponent(note)}&ext=${encodeURIComponent(ext)}`,
    { method: "POST", credentials: "same-origin", body: file },
  )
  if (!response.ok) throw new Error((await response.text()) || response.statusText)
  return ((await response.json()) as { path: string }).path
}

/** Where to open the vault from the phone. Desktop only: the phone is looking
 * at a server, not running one. */
export type ServerLink = {
  label: string
  url: string
  /** False means plain HTTP: the session cookie crosses that network in clear. */
  secure: boolean
  /** Whether the switch beside it is on. Off still shows, and says what to turn on. */
  live: boolean
}

export type TailscaleStatus = {
  installed: boolean
  connected: boolean
  /** What the daemon called its own state, shown as-is when it is not running. */
  state: string
  host: string | null
  serving: boolean
}

export type ServerState = {
  settings: { serving: boolean; localNetwork: boolean; tailscale: boolean }
  port: number
  tailscale: TailscaleStatus
  links: ServerLink[]
}

/** One link in the chain the phone walks, and whether it holds. */
export type TestStep = {
  label: string
  ok: boolean
  detail: string
}

/** One passage the answer may cite, numbered from 1. */
export type Source = {
  index: number
  path: string
  title: string
  headingPath: string
  /** Byte offset, so a citation jumps the same way a search result does. */
  startOffset: number
}

export type AiAnswer = {
  text: string
  sources: Source[]
  provider: "groq" | "gemini"
  /** What the question was expanded into, so retrieval is not a black box. */
  terms: string[]
}

/** Whether a key is configured — never the key itself. */
export type AiStatus = {
  keys: { groq: boolean; gemini: boolean }
  excluded: string[]
  groqModel: string
  geminiModel: string
}

export const ask = (question: string) => call<AiAnswer>("ask", { question })

export const aiStatus = () => call<AiStatus>("ai_status")

/** Desktop only: what may leave this machine is the machine's decision. */
export const setExcluded = (excluded: string[]) =>
  invoke<AiStatus>("set_excluded", { excluded })

export type ClassifyStatus = {
  phase: "running" | "done" | "failed"
  done: number
  total: number
  current: string
  /** How many turned out to be something; the rest stayed bare on purpose. */
  marked: number
}

export const CLASSIFY_EVENT = "classify:status"

/** Desktop only: reclassifying rewrites the vault, which is the machine's call. */
export const classifyPreview = () => invoke<number>("classify_preview")

export const classifyExisting = () => invoke<void>("classify_existing")

/** Desktop fires an event; the phone never runs this, so there is nothing to hear. */
export function onClassifyStatus(handler: (status: ClassifyStatus) => void): () => void {
  if (!insideApp()) return () => {}
  const stop = listen<ClassifyStatus>(CLASSIFY_EVENT, (event) => handler(event.payload))
  return () => void stop.then((off) => off())
}

export const serverState = () => invoke<ServerState>("server_state")

export const setServer = (serving: boolean, localNetwork: boolean, tailscale: boolean) =>
  invoke<ServerState>("set_server", { serving, localNetwork, tailscale })

export const testConnection = (local: boolean) =>
  invoke<TestStep[]>("test_connection", { local })

/**
 * The Notes.app import is desktop only — the library lives on this machine — so
 * on the phone there is nothing to report and nothing to stop.
 */
export function onImportStatus(handler: (status: ImportStatus) => void): () => void {
  if (!insideApp()) return () => {}
  const stop = listen<ImportStatus>(IMPORT_EVENT, (event) => handler(event.payload))
  return () => void stop.then((off) => off())
}

/**
 * Tauri events do not exist in a browser. On the phone the status is polled
 * instead — it moves while the indexer works and stands still the rest of the
 * time, so a slow poll says everything a listener would.
 */
export function onIndexStatus(handler: (status: IndexStatus) => void): () => void {
  if (insideApp()) {
    const stop = listen<IndexStatus>(INDEX_STATUS_EVENT, (event) => handler(event.payload))
    return () => void stop.then((off) => off())
  }

  let alive = true
  const tick = async () => {
    if (!alive) return
    try {
      const status = await vaultStatus()
      handler({
        phase: "idle",
        counts: { notes: status.notes, images: status.images, chunks: status.chunks },
        current: null,
        pending: 0,
      })
    } catch {
      // A blip on the tailnet is not worth a message on screen.
    }
  }
  void tick()
  const timer = setInterval(tick, 15_000)
  return () => {
    alive = false
    clearInterval(timer)
  }
}
