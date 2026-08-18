/**
 * The demo's vault, kept in the browser.
 *
 * IndexedDB rather than localStorage: a single screenshot passes the ~5 MB cap
 * localStorage has, and blobs cannot go in it at all. Seeded from the built
 * data file on first visit; after that what is stored is what counts.
 */

import type { NoteContent, Topic, TreeEntry, VaultStatus } from "../lib/api"
import { chunkMarkdown } from "./chunk"
import type { Language } from "../components/language-provider"

const DB = "cortex-demo"
const VERSION = 1

export type StoredChunk = {
  id: string
  language: Language
  path: string
  headingPath: string
  text: string
  start: number
  end: number
  vector: number[]
  /** Screenshots carry the text OCR read out of them. */
  kind: "text" | "image"
}

export type StoredNote = { key: string; language: Language; path: string; source: string }
export type StoredFolder = { key: string; language: Language; path: string }
export type StoredAsset = { key: string; language: Language; path: string; blob: Blob }
export type StoredTopic = { key: string; language: Language; topic: Topic; members: string[] }

let opening: Promise<IDBDatabase> | null = null

function open(): Promise<IDBDatabase> {
  opening ??= new Promise((resolve, reject) => {
    const request = indexedDB.open(DB, VERSION)
    request.onupgradeneeded = () => {
      const db = request.result
      for (const name of ["notes", "folders", "assets", "topics", "meta"]) {
        if (!db.objectStoreNames.contains(name)) db.createObjectStore(name, { keyPath: "key" })
      }
      if (!db.objectStoreNames.contains("chunks")) {
        const chunks = db.createObjectStore("chunks", { keyPath: "id" })
        chunks.createIndex("byPath", ["language", "path"])
        chunks.createIndex("byLanguage", "language")
      }
    }
    request.onsuccess = () => resolve(request.result)
    request.onerror = () => reject(request.error)
  })
  return opening
}

/** The browser sets the ceiling and it varies with the visitor's disk; all we
 *  choose is what happens when it runs out. */
export class OutOfRoom extends Error {}

const readable = (error: DOMException | null) =>
  error?.name === "QuotaExceededError"
    ? new OutOfRoom("o navegador ficou sem espaço para guardar isto")
    : (error ?? new Error("não consegui gravar"))

function run<T>(store: string, mode: IDBTransactionMode, work: (s: IDBObjectStore) => IDBRequest): Promise<T> {
  return open().then(
    (db) =>
      new Promise<T>((resolve, reject) => {
        const transaction = db.transaction(store, mode)
        const request = work(transaction.objectStore(store))
        request.onsuccess = () => resolve(request.result as T)
        transaction.onerror = () => reject(readable(transaction.error))
        request.onerror = () => reject(readable(request.error))
      }),
  )
}

const all = <T>(store: string) => run<T[]>(store, "readonly", (s) => s.getAll())
const put = <T>(store: string, value: T) => run<IDBValidKey>(store, "readwrite", (s) => s.put(value))
const drop = (store: string, key: string) => run<undefined>(store, "readwrite", (s) => s.delete(key))

const key = (language: Language, path: string) => `${language}:${path}`

// ── seeding ────────────────────────────────────────────────────────────────

/** How long a visitor's own notes survive. Coming back a month later should
 *  show the vault this is meant to show, not their leftovers. */
const KEEP_FOR_DAYS = 7

/** Bump on any change to `demo/vault-*` or the seeded topics, renames included.
 *  Forgetting is silent: the build is right and old browsers keep the old vault. */
const SEED_VERSION = 7

/** Whether this language's vault has been laid down yet. */
async function seeded(language: Language): Promise<boolean> {
  const marks = await all<{ key: string }>("meta")
  return marks.some((one) => one.key === `seeded:${language}:${SEED_VERSION}`)
}

/** Wipes a vault nobody has opened in a while, so the next visit starts clean. */
async function expire(language: Language): Promise<void> {
  const marks = await all<{ key: string; at?: number }>("meta")
  const seen = marks.find((one) => one.key === `seen:${language}`)?.at
  const old = seen !== undefined && Date.now() - seen > KEEP_FOR_DAYS * 86_400_000
  if (old) await reset(language)
  await put("meta", { key: `seen:${language}`, at: Date.now() })
}

export async function seed(language: Language, data: SeedData): Promise<void> {
  await expire(language)
  if (await seeded(language)) return

  // A newer seed replaces the old one rather than landing on top of it: the
  // notes carry the same keys, but a topic that was dropped upstream would
  // otherwise stay for the people who had it.
  await reset(language)

  for (const [path, note] of Object.entries(data.notes)) {
    await put<StoredNote>("notes", { key: key(language, path), language, path, source: note.source })
  }
  for (const chunk of data.chunks) {
    await put<StoredChunk>("chunks", {
      id: `${language}:${chunk.path}:${chunk.start}`,
      language,
      path: chunk.path,
      headingPath: chunk.headingPath,
      text: chunk.text,
      start: chunk.start,
      end: chunk.end,
      vector: chunk.vector,
      kind: "text",
    })
  }
  for (const topic of data.topics ?? []) {
    const { members, ...rest } = topic
    await put<StoredTopic>("topics", {
      key: key(language, topic.slug),
      language,
      topic: { ...rest, members },
      members,
    })
  }
  await put("meta", { key: `seeded:${language}:${SEED_VERSION}` })
}

export type SeedData = {
  notes: Record<string, NoteContent>
  topics?: (Topic & { members: string[] })[]
  chunks: {
    path: string
    headingPath: string
    text: string
    start: number
    end: number
    vector: number[]
  }[]
}

/** Wipes this language's vault so the next load seeds it again. */
export async function reset(language: Language): Promise<void> {
  for (const store of ["notes", "folders", "assets", "topics"] as const) {
    for (const one of await all<{ key: string; language: Language }>(store)) {
      if (one.language === language) await drop(store, one.key)
    }
  }
  for (const chunk of await chunks(language)) await drop("chunks", chunk.id)
  for (const mark of await all<{ key: string }>("meta")) {
    if (mark.key.startsWith(`seeded:${language}`)) await drop("meta", mark.key)
  }
}

// ── reading ────────────────────────────────────────────────────────────────

const mine = <T extends { language: Language }>(rows: T[], language: Language) =>
  rows.filter((one) => one.language === language)

export const notes = async (language: Language) =>
  mine(await all<StoredNote>("notes"), language)

export const chunks = async (language: Language) =>
  mine(await all<StoredChunk>("chunks"), language)

export const folders = async (language: Language) =>
  mine(await all<StoredFolder>("folders"), language)

export const assets = async (language: Language) =>
  mine(await all<StoredAsset>("assets"), language)

export const topics = async (language: Language) =>
  mine(await all<StoredTopic>("topics"), language)

export async function note(language: Language, path: string): Promise<NoteContent | null> {
  const rows = await notes(language)
  const found = rows.find((one) => one.path === path)
  if (!found) return null
  return {
    path,
    title: titleOf(found.source, path),
    source: found.source,
    chunks: chunkMarkdown(found.source).map((c) => ({
      headingPath: c.headingPath,
      start: c.start,
      end: c.end,
    })),
  }
}

/** `parse::title_of`: the first H1, or the file name. */
export function titleOf(source: string, fallback: string): string {
  const heading = /^#\s+(.+)$/m.exec(source)
  if (heading) return heading[1].trim()
  return fallback.split("/").pop()?.replace(/\.md$/, "") ?? fallback
}

export async function status(language: Language): Promise<VaultStatus> {
  const [n, a, c] = await Promise.all([notes(language), assets(language), chunks(language)])
  return { root: "Cortex (demo)", notes: n.length, images: a.length, chunks: c.length }
}

/** Folders come from the note paths as well as from explicitly created ones, so
 *  a folder holding notes exists even if nobody made it by hand. */
export async function tree(language: Language): Promise<TreeEntry[]> {
  const [n, f, a] = await Promise.all([notes(language), folders(language), assets(language)])
  const paths = [
    ...n.map((one) => ({ path: one.path, kind: "note" as const })),
    ...a.map((one) => ({ path: one.path, kind: "image" as const })),
  ]

  const root: TreeEntry[] = []
  const folderAt = new Map<string, TreeEntry>()

  const ensureFolder = (path: string): TreeEntry[] => {
    if (path === "") return root
    const existing = folderAt.get(path)
    if (existing) return existing.children
    const at = path.lastIndexOf("/")
    const parent = ensureFolder(at === -1 ? "" : path.slice(0, at))
    const entry: TreeEntry = {
      path,
      name: path.slice(at + 1),
      kind: "folder",
      children: [],
    }
    folderAt.set(path, entry)
    parent.push(entry)
    return entry.children
  }

  for (const one of f) ensureFolder(one.path)
  for (const one of paths) {
    const at = one.path.lastIndexOf("/")
    const into = ensureFolder(at === -1 ? "" : one.path.slice(0, at))
    into.push({ path: one.path, name: one.path.slice(at + 1), kind: one.kind, children: [] })
  }

  const sort = (entries: TreeEntry[]): TreeEntry[] =>
    entries
      .sort((a, b) =>
        a.kind === b.kind || (a.kind !== "folder" && b.kind !== "folder")
          ? a.name.localeCompare(b.name)
          : a.kind === "folder"
            ? -1
            : 1,
      )
      .map((entry) => ({ ...entry, children: sort(entry.children) }))

  return sort(root)
}

// ── writing ────────────────────────────────────────────────────────────────

export const writeNote = (language: Language, path: string, source: string) =>
  put<StoredNote>("notes", { key: key(language, path), language, path, source })

export const writeFolder = (language: Language, path: string) =>
  put<StoredFolder>("folders", { key: key(language, path), language, path })

export const writeAsset = (language: Language, path: string, blob: Blob) =>
  put<StoredAsset>("assets", { key: key(language, path), language, path, blob })

export const writeTopic = (language: Language, topic: Topic, members: string[]) =>
  put<StoredTopic>("topics", { key: key(language, topic.slug), language, topic, members })

export const removeTopic = (language: Language, slug: string) => drop("topics", key(language, slug))

/** Removes a note or image and everything indexed from it. */
export async function remove(language: Language, path: string): Promise<void> {
  for (const store of ["notes", "assets", "folders"] as const) {
    await drop(store, key(language, path))
  }
  for (const chunk of await chunks(language)) {
    if (chunk.path === path || chunk.path.startsWith(`${path}/`)) await drop("chunks", chunk.id)
  }
}

/** Replaces everything indexed from one file. */
export async function reindex(language: Language, path: string, fresh: StoredChunk[]): Promise<void> {
  for (const chunk of await chunks(language)) {
    if (chunk.path === path) await drop("chunks", chunk.id)
  }
  for (const chunk of fresh) await put<StoredChunk>("chunks", chunk)
}
