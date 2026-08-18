/**
 * The demo answers the commands the app would send to Rust.
 *
 * Everything the desktop does to a file, this does to IndexedDB — including
 * chunking and embedding a note as it is saved, which works because the model
 * is already in the page to embed questions with.
 */

import type { Found, Group, Hit, NoteContent, Topic } from "../lib/api"
import type { Language } from "../components/language-provider"
import { rememberAsset } from "../lib/api"
import { data } from "./data"
import { chunkMarkdown } from "./chunk"
import { embedQuestion, embedPassages } from "./embedder"
import { textIn } from "./ocr"
import { fold, meaningful, rank } from "./search"
import * as store from "./store"

/** `search::CHUNKS_PER_GROUP`. */
const CHUNKS_PER_GROUP = 3

const NOT_HERE = "Isso não existe na demonstração."

/** Seeded once per language, then the store is the vault. */
const ready = new Map<Language, Promise<void>>()

function prepared(language: Language): Promise<void> {
  let waiting = ready.get(language)
  if (!waiting) {
    waiting = data(language)
      .then((seed) => store.seed(language, seed))
      .then(async () => {
        // Blob URLs before the first render that needs one — see `assetUrl`.
        for (const one of await store.assets(language)) rememberAsset(one.path, one.blob)
      })
    ready.set(language, waiting)
  }
  return waiting
}

export async function call<T>(
  command: string,
  args: Record<string, unknown>,
  language: Language,
): Promise<T> {
  await prepared(language)
  const text = (name: string) => String(args[name] ?? "")

  switch (command) {
    case "vault_status":
      return (await store.status(language)) as T
    case "list_tree":
      return (await store.tree(language)) as T

    case "get_note": {
      const note = await store.note(language, text("path"))
      if (!note) throw new Error("essa nota não existe")
      return note as T
    }

    case "search":
      return (await search(
        language,
        text("query"),
        (args.topic as string | null) ?? null,
        Number(args.limit ?? 40),
      )) as T

    case "list_topics":
      return (await store.topics(language)).map((one) => one.topic) as T

    case "ai_status":
      return status(await store.status(language)) as T

    // ── writing ───────────────────────────────────────────────────────────

    case "create_note": {
      const folder = text("folder")
      const title = text("title") || "sem título"
      const path = `${folder ? `${folder}/` : ""}${slug(title)}.md`
      await save(language, path, `# ${title}\n\n`)
      return path as T
    }

    case "write_note":
      await save(language, text("path"), text("source"))
      return undefined as T

    case "create_folder": {
      const parent = text("parent")
      const path = `${parent ? `${parent}/` : ""}${slug(text("name"))}`
      await store.writeFolder(language, path)
      return path as T
    }

    case "rename_entry": {
      const from = text("path")
      const name = text("name")
      const at = from.lastIndexOf("/")
      const to = `${at === -1 ? "" : `${from.slice(0, at)}/`}${name}${from.endsWith(".md") ? ".md" : ""}`
      const note = await store.note(language, from)
      if (note) {
        await store.remove(language, from)
        await save(language, to, note.source)
      }
      return to as T
    }

    case "delete_entry":
      await store.remove(language, text("path"))
      return undefined as T

    case "create_topic": {
      const topic: Topic = {
        slug: slug(text("name")),
        name: text("name"),
        icon: (args.icon as string) ?? null,
        color: (args.color as string) ?? null,
        auto: false,
        members: [],
      }
      await store.writeTopic(language, topic, [])
      return topic as T
    }

    case "update_topic": {
      const found = (await store.topics(language)).find((one) => one.topic.slug === text("slug"))
      if (!found) throw new Error(NOT_HERE)
      const topic = {
        ...found.topic,
        name: text("name") || found.topic.name,
        icon: (args.icon as string) ?? found.topic.icon,
        color: (args.color as string) ?? found.topic.color,
      }
      await store.writeTopic(language, topic, found.members)
      return topic as T
    }

    case "delete_topic":
      await store.removeTopic(language, text("slug"))
      return undefined as T

    case "add_to_topic":
    case "remove_from_topic": {
      const found = (await store.topics(language)).find((one) => one.topic.slug === text("slug"))
      if (!found) throw new Error(NOT_HERE)
      const path = text("path")
      const members =
        command === "add_to_topic"
          ? [...new Set([...found.members, path])]
          : found.members.filter((one) => one !== path)
      await store.writeTopic(language, { ...found.topic, members }, members)
      return undefined as T
    }

    case "paste_image":
    case "upload_image": {
      const bytes = new Uint8Array(args.bytes as number[])
      const extension = text("extension") || "png"
      const blob = new Blob([bytes], { type: `image/${extension === "jpg" ? "jpeg" : extension}` })
      return (await addImage(language, blob, extension)) as T
    }

    case "get_asset":
      return { path: text("path"), blocks: [] } as T

    default:
      throw new Error(NOT_HERE)
  }
}

export async function restore(language: Language): Promise<void> {
  await store.reset(language)
  ready.delete(language)
  await prepared(language)
}

// ── indexing ────────────────────────────────────────────────────────────────

/** Writes a note and rebuilds everything indexed from it. */
async function save(language: Language, path: string, source: string): Promise<void> {
  await store.writeNote(language, path, source)

  const pieces = chunkMarkdown(source)
  if (pieces.length === 0) return await store.reindex(language, path, [])

  const vectors = await embedPassages(pieces.map((one) => one.text))
  await store.reindex(
    language,
    path,
    pieces.map((piece, at) => ({
      id: `${language}:${path}:${piece.start}`,
      language,
      path,
      headingPath: piece.headingPath,
      text: piece.text,
      start: piece.start,
      end: piece.end,
      vector: vectors[at],
      kind: "text" as const,
    })),
  )
}

/** Stores a screenshot and indexes whatever OCR read out of it. */
async function addImage(language: Language, blob: Blob, extension: string): Promise<string> {
  const path = `_assets/print-${Date.now()}.${extension}`
  await store.writeAsset(language, path, blob)
  rememberAsset(path, blob)

  const read = await textIn(blob, language)
  if (read.length === 0) return path

  const [vector] = await embedPassages([read])
  await store.reindex(language, path, [
    {
      id: `${language}:${path}:0`,
      language,
      path,
      headingPath: path.split("/").pop() ?? path,
      text: read,
      start: 0,
      end: read.length,
      vector,
      kind: "image" as const,
    },
  ])
  return path
}

const slug = (name: string) =>
  fold(name).replace(/[^a-z0-9]+/g, "-").replace(/^-|-$/g, "") || "sem-titulo"

// ── search ──────────────────────────────────────────────────────────────────

function status(vault: { chunks: number }) {
  return {
    keys: { groq: null, gemini: null },
    excluded: [],
    quota: {
      groq: { limitRequests: null, remainingRequests: null, limitTokens: null, remainingTokens: null },
      groqCalls: 0,
      geminiCalls: 0,
      geminiDailyLimit: 0,
      geminiResetsIn: 0,
    },
    coverage: { chunks: vault.chunks, embedded: vault.chunks },
    autoEmbed: false,
    modelInstalled: true,
    groqModel: "",
    geminiModel: "",
    geminiExpandModel: "",
  }
}

async function search(
  language: Language,
  query: string,
  topic: string | null,
  limit: number,
): Promise<Found> {
  const terms = meaningful(query)

  // Inside a topic and with nothing typed, the topic itself is the query —
  // `search::run` does the same before it looks at any word.
  const inTopic = topic
    ? ((await store.topics(language)).find((one) => one.topic.slug === topic)?.members ?? [])
    : null

  const all = await store.chunks(language)
  const chunks = inTopic ? all.filter((one) => inTopic.includes(one.path)) : all

  if (terms.length === 0) {
    if (!inTopic) return { groups: [], widened: false, terms }
    return { groups: gather(chunks, await titlesOf(language), query).slice(0, limit), widened: false, terms }
  }
  const asked = await embedQuestion(query)
  const { order, widened } = rank(chunks, query, asked)
  const titles = await titlesOf(language)
  const groups = new Map<string, Group>()

  for (const at of order) {
    const chunk = chunks[at]
    let group = groups.get(chunk.path)
    if (!group) {
      const folder = chunk.path.includes("/") ? chunk.path.slice(0, chunk.path.lastIndexOf("/")) : ""
      group = {
        key: chunk.path,
        kind: chunk.kind === "image" ? "image" : "note",
        path: chunk.path,
        title: titles.get(chunk.path) ?? (chunk.path.split("/").pop() ?? chunk.path),
        folder,
        total: 0,
        hits: [],
      }
      groups.set(chunk.path, group)
    }
    group.total += 1
    if (group.hits.length < CHUNKS_PER_GROUP) group.hits.push(hitOf(chunk, query))
  }

  return { groups: [...groups.values()].slice(0, limit), widened, terms }
}

const titlesOf = async (language: Language) =>
  new Map((await store.notes(language)).map((one) => [one.path, store.titleOf(one.source, one.path)]))

/** Every passage, grouped by note and in the order given — what a topic shows
 *  when nothing has been typed into it. */
function gather(chunks: store.StoredChunk[], titles: Map<string, string>, query: string): Group[] {
  const groups = new Map<string, Group>()
  for (const chunk of chunks) {
    let group = groups.get(chunk.path)
    if (!group) {
      const at = chunk.path.lastIndexOf("/")
      group = {
        key: chunk.path,
        kind: chunk.kind === "image" ? "image" : "note",
        path: chunk.path,
        title: titles.get(chunk.path) ?? chunk.path.slice(at + 1),
        folder: at === -1 ? "" : chunk.path.slice(0, at),
        total: 0,
        hits: [],
      }
      groups.set(chunk.path, group)
    }
    group.total += 1
    if (group.hits.length < CHUNKS_PER_GROUP) group.hits.push(hitOf(chunk, query))
  }
  return [...groups.values()]
}

function hitOf(chunk: store.StoredChunk, query: string): Hit {
  return {
    chunkId: Number(chunk.start),
    kind: chunk.kind,
    path: chunk.path,
    headingPath: chunk.headingPath,
    snippet: snippetOf(chunk.text, query),
    startOffset: chunk.start,
    endOffset: chunk.end,
    blocks: [],
  }
}

/** A window around the first word that matched, so the line shown explains why
 *  the result is there. */
function snippetOf(text: string, query: string): string {
  const flat = text.replace(/\s+/g, " ").trim()
  const terms = meaningful(query)
  const folded = fold(flat)

  const at = terms.map((term) => folded.indexOf(term)).filter((n) => n >= 0).sort((a, b) => a - b)[0]
  if (at === undefined || flat.length <= 180) return flat.slice(0, 180)

  const from = Math.max(0, at - 60)
  return (from > 0 ? "…" : "") + flat.slice(from, from + 180) + "…"
}

export type { NoteContent }
