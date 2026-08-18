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

/**
 * The web demo: no Tauri, no server, everything in the page.
 *
 * A build-time flag rather than a runtime check, so the demo's data and the
 * embedding model are absent from the app's own bundle — a desktop build must
 * not carry a vault of invented notes.
 */
export const isDemo = import.meta.env.VITE_DEMO === "1"

const call = <T>(command: string, args: Record<string, unknown> = {}): Promise<T> => {
  if (isDemo) return demoCall<T>(command, args)
  return insideApp() ? invoke<T>(command, args) : post<T>(command, args)
}

/** Loaded on demand: naming it at the top would pull the demo into every
 *  bundle, flag or no flag. */
const demoCall = async <T>(command: string, args: Record<string, unknown>): Promise<T> => {
  const { call: answer } = await import("../demo/shim")
  return answer<T>(command, args, demoLanguage)
}

/**
 * The language the demo is showing, read by the shim to pick a vault.
 *
 * A module variable rather than a hook argument because `call` is reached from
 * plain functions all over `api.ts`, and threading React state through every
 * one of them would put the provider inside the transport.
 */
let demoLanguage: "en" | "pt" = "en"

export const setDemoLanguage = (language: "en" | "pt") => {
  demoLanguage = language
}

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
  phase: "scanning" | "ocr" | "embedding" | "idle"
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

/** What a search found, and whether it had to widen to find it. */
export type Found = {
  groups: Group[]
  /** Nothing matched every word, so any of them was accepted. Said out loud. */
  widened: boolean
  /** The words actually searched, once the filler was dropped. */
  terms: string[]
}

export const search = (query: string, topic: string | null = null, limit = 40) =>
  call<Found>("search", { query, topic, limit })

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
  isDemo
    ? demoAssetUrl(relativePath)
    : insideApp()
      ? convertFileSrc(`${root}/${relativePath}`)
      : `/asset/${encode(relativePath)}`

/**
 * A screenshot pasted into the demo lives in IndexedDB, so it needs a blob URL.
 *
 * The map is filled when the vault loads and when an image is written, never
 * here: `<img src>` is read during render and cannot wait for a promise, so
 * resolving lazily would hand back an empty string and never re-render.
 */
const blobUrls = new Map<string, string>()

export const rememberAsset = (path: string, blob: Blob) => {
  if (!blobUrls.has(path)) blobUrls.set(path, URL.createObjectURL(blob))
}

const demoAssetUrl = (path: string) => blobUrls.get(path) ?? ""

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
  if (isDemo) {
    const bytes = [...new Uint8Array(await file.arrayBuffer())]
    return call<string>("upload_image", { bytes, extension: ext, notePath: note })
  }
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

/** Where a note the subject lives in sits, and how much of it matched. */
export type Place = {
  path: string
  folder: string
  title: string
  passages: number
}

export type AiAnswer = {
  text: string
  sources: Source[]
  provider: "groq" | "gemini"
  /** What the question was expanded into, so retrieval is not a black box. */
  terms: string[]
  /** Asking where something is and what it says are different questions. */
  kind: "locate" | "explain"
  places: Place[]
}

export type Provider = "groq" | "gemini"

/**
 * Where the key in use came from. The app reads the environment first, so a key
 * typed here sits behind an exported variable — the panel says which one is
 * answering rather than leaving that a mystery.
 */
export type KeySource = "environment" | "app"

/** What a provider said about its own limits, when it says anything at all. */
export type Limits = {
  limitRequests: number | null
  remainingRequests: number | null
  resetRequests: string | null
  limitTokens: number | null
  remainingTokens: number | null
  resetTokens: string | null
}

/**
 * Groq reports its own numbers in every reply; Gemini reports none, so its half
 * is this app counting its own calls against a limit written down.
 */
export type Quota = {
  groq: Limits
  groqCalls: number
  geminiCalls: number
  geminiDailyLimit: number
  /** Seconds to midnight Pacific, when Gemini's day turns. */
  geminiResetsIn: number
}

/** Whether a key is configured and where it came from — never the key itself. */
export type AiStatus = {
  keys: { groq: KeySource | null; gemini: KeySource | null }
  excluded: string[]
  quota: Quota
  /** How much of the vault can be searched by meaning, and whether the app
   *  keeps that current on its own. */
  coverage: { chunks: number; embedded: number }
  autoEmbed: boolean
  /** Whether the weights are on this machine. Nothing about meaning works
   *  without them, and only the panel's button may put them there. */
  modelInstalled: boolean
  groqModel: string
  geminiModel: string
  geminiExpandModel: string
}

export const KEY_NAMES: Record<Provider, string> = {
  groq: "GROQ_API_KEY",
  gemini: "GEMINI_API_KEY",
}

/**
 * The token is this call's handle: `cancelAsk` kills the request with it rather
 * than merely stopping the wait. A question is two provider calls under the
 * same token, so cancelling catches whichever is in the air.
 */
export const ask = (question: string, token: number) =>
  call<AiAnswer>("ask", { question, token })

export const cancelAsk = (token: number) => call<void>("cancel_ask", { token })

export const aiStatus = () => call<AiStatus>("ai_status")

// Desktop only, and deliberately not on the server: what may leave this machine
// — and with what key — is decided at the machine, not over plain HTTP.

export const setExcluded = (excluded: string[]) =>
  invoke<AiStatus>("set_excluded", { excluded })

/** `null` removes the key. Nothing here ever reads one back. */
export const setKey = (name: string, value: string | null) =>
  invoke<AiStatus>("set_key", { name, value })

export const testKey = (provider: Provider) => invoke<void>("test_key", { provider })

/** Spends one Groq request to hear its numbers. Never offered for Gemini. */
export const refreshQuota = () => invoke<AiStatus>("refresh_quota")

export const EMBED_EVENT = "embed:status"

export type EmbedStatus = {
  phase: "running" | "done" | "failed"
  done: number
  total: number
  error: string | null
}

/** Desktop only, and never automatic — the model runs here, but the pass still
 *  costs minutes of someone's laptop. */
export const embedVault = () => invoke<void>("embed_vault")

export const cancelEmbedding = () => invoke<void>("cancel_embedding")

export const setAutoEmbed = (on: boolean) => invoke<AiStatus>("set_auto_embed", { on })

export function onEmbedStatus(handler: (status: EmbedStatus) => void): () => void {
  if (!insideApp()) return () => {}
  const stop = listen<EmbedStatus>(EMBED_EVENT, (event) => handler(event.payload))
  return () => void stop.then((off) => off())
}

export const MODEL_EVENT = "model:status"

export type ModelStatus = {
  phase: "downloading" | "ready" | "failed"
  /** Bytes, not passages: this is the download, not the embedding. */
  done: number
  total: number
  error: string | null
}

/** The one moment this app reaches the network for search to work. Only the
 *  button in the panel may call it. */
export const downloadModel = () => invoke<void>("download_model")

/** Deletes the weights. The vectors already made stay — what is embedded stays
 *  searchable, and only new passages wait for the model to come back. */
export const deleteModel = () => invoke<AiStatus>("delete_model")

export function onModelStatus(handler: (status: ModelStatus) => void): () => void {
  if (!insideApp()) return () => {}
  const stop = listen<ModelStatus>(MODEL_EVENT, (event) => handler(event.payload))
  return () => void stop.then((off) => off())
}

export const setGeminiLimit = (limit: number) =>
  invoke<AiStatus>("set_gemini_limit", { limit })

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
