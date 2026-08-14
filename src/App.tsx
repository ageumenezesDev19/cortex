import { listen } from "@tauri-apps/api/event"
import { getCurrentWebview } from "@tauri-apps/api/webview"
import { AnimatePresence, motion, useReducedMotion } from "motion/react"
import { useCallback, useEffect, useMemo, useRef, useState } from "react"

import { ConfirmDialog } from "./components/confirm-dialog"
import { ImageReader } from "./components/image-reader"
import { NotePanel } from "./components/note-panel"
import { PromptDialog } from "./components/prompt-dialog"
import { Rail } from "./components/rail"
import { ResultGroup } from "./components/result-group"
import { SearchField } from "./components/search-field"
import { StatusBar } from "./components/status-bar"
import { TopicsPanel } from "./components/topics-panel"
import { TreePanel } from "./components/tree-panel"
import {
  addToTopic,
  createFolder,
  createNote,
  createTopic,
  deleteEntry,
  deleteTopic,
  importImages,
  INDEX_STATUS_EVENT,
  listTopics,
  listTree,
  renameEntry,
  search,
  vaultStatus,
  type Group,
  type Hit,
  type IndexStatus,
  type Topic,
  type TreeEntry,
  type VaultStatus,
} from "./lib/api"
import { cn } from "./lib/utils"

/** Fast enough that the results feel like they were already there. */
const DEBOUNCE_MS = 120

type Panel = "none" | "topics" | "tree"

/**
 * What the note panel is showing, and who opened it. A note reached through a
 * result belongs to that search and leaves with it; one opened from the tree is
 * your own doing and stays put.
 */
type Source = "search" | "tree"

type Open =
  | { kind: "note"; path: string; anchorStart: number | null; from: Source }
  | { kind: "image"; hit: Hit; from: Source }
  | null

type Ask =
  | { kind: "new-note"; folder: string }
  | { kind: "new-folder"; parent: string }
  | { kind: "rename"; entry: TreeEntry }
  | null

type Confirm =
  | { kind: "topic"; topic: Topic }
  | { kind: "entry"; entry: TreeEntry }
  | null

export default function App() {
  const [query, setQuery] = useState("")
  const [groups, setGroups] = useState<Group[]>([])
  const [selected, setSelected] = useState(0)
  const [open, setOpen] = useState<Open>(null)
  const [status, setStatus] = useState<IndexStatus | null>(null)
  const [vault, setVault] = useState<VaultStatus | null>(null)
  const [dropping, setDropping] = useState(false)
  const [topics, setTopics] = useState<Topic[]>([])
  const [topic, setTopic] = useState<Topic | null>(null)
  const [tree, setTree] = useState<TreeEntry[]>([])
  const [panel, setPanel] = useState<Panel>("none")
  const [ask, setAsk] = useState<Ask>(null)
  const [confirm, setConfirm] = useState<Confirm>(null)

  const reduceMotion = useReducedMotion()
  const results = useRef<HTMLDivElement>(null)

  // Keyboard navigation runs over every passage on screen; it must not know
  // that the list is grouped.
  const hits = useMemo(() => groups.flatMap((group) => group.hits), [groups])

  const refreshTopics = useCallback(() => {
    listTopics()
      .then(setTopics)
      .catch(() => {})
  }, [])

  const refreshTree = useCallback(() => {
    listTree()
      .then(setTree)
      .catch(() => {})
  }, [])

  useEffect(() => {
    vaultStatus()
      .then(setVault)
      .catch(() => {})
    refreshTopics()
    refreshTree()

    const unlisten = listen<IndexStatus>(INDEX_STATUS_EVENT, (event) => setStatus(event.payload))
    return () => {
      unlisten.then((stop) => stop())
    }
  }, [refreshTopics, refreshTree])

  const indexedChunks = status?.counts.chunks ?? vault?.chunks ?? 0

  // The indexer reports after every change, which is also the moment the tree
  // could have gained or lost a file.
  useEffect(() => {
    refreshTree()
  }, [indexedChunks, refreshTree])

  // Screenshots dropped on the window land in the vault; the watcher takes it
  // from there, exactly as if they had been saved from the Finder.
  useEffect(() => {
    const unlisten = getCurrentWebview().onDragDropEvent((event) => {
      if (event.payload.type === "over") return setDropping(true)
      if (event.payload.type === "leave") return setDropping(false)
      if (event.payload.type !== "drop") return
      setDropping(false)
      importImages(event.payload.paths).catch(() => {})
    })
    return () => {
      unlisten.then((stop) => stop())
    }
  }, [])

  useEffect(() => {
    const term = query.trim()
    if (term.length === 0 && !topic) {
      setGroups([])
      return
    }
    const timer = setTimeout(() => {
      search(term, topic?.slug ?? null)
        .then((found) => {
          setGroups(found)
          setSelected(0)
        })
        .catch(() => setGroups([]))
    }, DEBOUNCE_MS)

    return () => clearTimeout(timer)
    // `indexedChunks` re-runs the search when the indexer finishes new work, so
    // a screenshot dropped mid-search shows up without retyping.
  }, [query, topic, indexedChunks])

  const move = useCallback(
    (delta: number) => {
      setSelected((current) => {
        if (hits.length === 0) return 0
        const next = Math.min(Math.max(current + delta, 0), hits.length - 1)
        results.current
          ?.querySelector(`[data-hit="${hits[next].chunkId}"]`)
          ?.scrollIntoView({ block: "nearest" })
        return next
      })
    },
    [hits],
  )

  const openHit = useCallback((hit: Hit) => {
    setOpen(
      hit.kind === "image"
        ? { kind: "image", hit, from: "search" }
        : { kind: "note", path: hit.path, anchorStart: hit.startOffset, from: "search" },
    )
  }, [])

  /**
   * Emptying the field returns the app to what it is when it opens: one search,
   * centred, nothing behind it. The amber frame leaves with it — amber means
   * "the search found this", so it must not outlive the search.
   */
  const changeQuery = useCallback((value: string) => {
    setQuery(value)
    if (value.trim().length > 0) return
    setOpen((current) => (current?.from === "tree" ? current : null))
  }, [])

  /** Dropping the chip with nothing typed leaves the same nothing behind. */
  const clearTopic = useCallback(() => {
    setTopic(null)
    if (query.trim().length > 0) return
    setOpen((current) => (current?.from === "tree" ? current : null))
  }, [query])

  const onKeyDown = (event: React.KeyboardEvent<HTMLInputElement>) => {
    if (event.key === "ArrowDown") {
      event.preventDefault()
      move(1)
    } else if (event.key === "ArrowUp") {
      event.preventDefault()
      move(-1)
    } else if (event.key === "Enter") {
      event.preventDefault()
      if (hits[selected]) openHit(hits[selected])
    } else if (event.key === "Backspace" && query.length === 0 && topic) {
      event.preventDefault()
      clearTopic()
    } else if (event.key === "Escape") {
      event.preventDefault()
      if (open) setOpen(null)
      else if (query.length > 0) changeQuery("")
      else clearTopic()
    }
  }

  const browsing = query.trim().length > 0 || topic !== null
  const transition = useMemo(
    () => ({ duration: reduceMotion ? 0 : 0.18, ease: "easeOut" as const }),
    [reduceMotion],
  )

  const openPath = open?.kind === "note" ? open.path : null

  return (
    <div className="flex h-screen overflow-hidden bg-background">
      <Rail
        panel={panel}
        onTogglePanel={(next) => setPanel((current) => (current === next ? "none" : next))}
      />

      <TopicsPanel
        open={panel === "topics"}
        topics={topics}
        activeSlug={topic?.slug ?? null}
        onClose={() => setPanel("none")}
        onPick={(picked) => setTopic(picked.slug === topic?.slug ? null : picked)}
        onCreate={(name, icon, color) => {
          createTopic(name, icon, color)
            .then(refreshTopics)
            .catch(() => {})
        }}
        onDelete={(target) => setConfirm({ kind: "topic", topic: target })}
      />

      <TreePanel
        open={panel === "tree"}
        tree={tree}
        activePath={openPath}
        onClose={() => setPanel("none")}
        onOpenNote={(path) => setOpen({ kind: "note", path, anchorStart: null, from: "tree" })}
        onCreateNote={(folder) => setAsk({ kind: "new-note", folder })}
        onCreateFolder={(parent) => setAsk({ kind: "new-folder", parent })}
        onRename={(entry) => setAsk({ kind: "rename", entry })}
        onDelete={(entry) => setConfirm({ kind: "entry", entry })}
      />

      <div className="flex min-w-0 flex-1 flex-col">
        <motion.div
          layout={!reduceMotion}
          transition={transition}
          className={cn(
            "mx-auto w-full max-w-3xl px-6",
            browsing || open ? "pt-6" : "flex flex-1 flex-col justify-center",
          )}
        >
          <SearchField
            value={query}
            onChange={changeQuery}
            onKeyDown={onKeyDown}
            compact={browsing || open !== null}
            topic={topic}
            onClearTopic={clearTopic}
          />
          {!browsing && !open && <EmptyState vault={vault} dropping={dropping} />}
        </motion.div>

        {(browsing || open) && (
          <div className="flex min-h-0 flex-1 gap-4 px-6 pb-4 pt-4">
            {browsing && (
              <div
                ref={results}
                className={cn(
                  "min-h-0 space-y-3 overflow-y-auto",
                  open ? "w-[24rem] shrink-0" : "mx-auto w-full max-w-3xl",
                )}
              >
                <AnimatePresence initial={false}>
                  {groups.map((group, position) => (
                    <motion.div
                      key={group.key}
                      initial={reduceMotion ? { opacity: 0 } : { opacity: 0, y: 6 }}
                      animate={{ opacity: 1, y: 0 }}
                      exit={{ opacity: 0 }}
                      transition={{
                        duration: reduceMotion ? 0 : 0.12,
                        delay: reduceMotion ? 0 : Math.min(position, 8) * 0.02,
                      }}
                    >
                      <ResultGroup
                        group={group}
                        query={query}
                        root={vault?.root ?? ""}
                        topics={topics}
                        selectedHit={hits[selected] ?? null}
                        onSelect={(hit) =>
                          setSelected(hits.findIndex((one) => one.chunkId === hit.chunkId))
                        }
                        onOpen={openHit}
                        onAddToTopic={(slug, path) => {
                          addToTopic(slug, path)
                            .then(refreshTopics)
                            .catch(() => {})
                        }}
                      />
                    </motion.div>
                  ))}
                </AnimatePresence>

                {groups.length === 0 && (
                  <p className="pt-8 text-center font-mono text-sm text-muted-foreground">
                    {topic && query.trim().length === 0
                      ? `nada em ${topic.name} ainda`
                      : `nada encontrado para “${query.trim()}”`}
                  </p>
                )}
              </div>
            )}

            {open && (
              <div className="min-h-0 min-w-0 flex-1 rounded-lg border bg-card p-4">
                {open.kind === "image" ? (
                  <ImageReader hit={open.hit} query={query} root={vault?.root ?? ""} />
                ) : (
                  <NotePanel
                    key={open.path}
                    path={open.path}
                    anchorStart={open.anchorStart}
                    root={vault?.root ?? ""}
                  />
                )}
              </div>
            )}
          </div>
        )}

        <StatusBar vault={vault} status={status} />
      </div>

      <PromptDialog
        open={ask !== null}
        title={promptTitle(ask)}
        placeholder={ask?.kind === "new-folder" ? "nome da pasta" : "título da nota"}
        initialValue={ask?.kind === "rename" ? ask.entry.name : ""}
        confirmLabel={ask?.kind === "rename" ? "Renomear" : "Criar"}
        onCancel={() => setAsk(null)}
        onConfirm={(value) => {
          const action =
            ask?.kind === "new-note"
              ? createNote(ask.folder, value).then((path) => {
                  setOpen({ kind: "note", path, anchorStart: null, from: "tree" })
                })
              : ask?.kind === "new-folder"
                ? createFolder(ask.parent, value).then(() => {})
                : ask?.kind === "rename"
                  ? renameEntry(ask.entry.path, value).then((path) => {
                      if (openPath === ask.entry.path) {
                        setOpen({ kind: "note", path, anchorStart: null, from: "tree" })
                      }
                    })
                  : Promise.resolve()

          action
            .then(() => {
              refreshTree()
              refreshTopics()
            })
            .catch(() => {})
          setAsk(null)
        }}
      />

      <ConfirmDialog
        open={confirm !== null}
        title={confirmTitle(confirm)}
        detail={confirmDetail(confirm)}
        confirmLabel={confirmLabel(confirm)}
        onCancel={() => setConfirm(null)}
        onConfirm={() => {
          if (confirm?.kind === "topic") {
            const slug = confirm.topic.slug
            deleteTopic(slug)
              .then(() => {
                if (topic?.slug === slug) setTopic(null)
                refreshTopics()
              })
              .catch(() => {})
          } else if (confirm?.kind === "entry") {
            const path = confirm.entry.path
            deleteEntry(path)
              .then(() => {
                if (openPath === path) setOpen(null)
                refreshTree()
                refreshTopics()
              })
              .catch(() => {})
          }
          setConfirm(null)
        }}
      />
    </div>
  )
}

function promptTitle(ask: Ask) {
  if (ask?.kind === "new-note") return ask.folder ? `Nova nota em ${ask.folder}` : "Nova nota"
  if (ask?.kind === "new-folder") return ask.parent ? `Nova pasta em ${ask.parent}` : "Nova pasta"
  if (ask?.kind === "rename") return `Renomear ${ask.entry.name}`
  return ""
}

function confirmTitle(confirm: Confirm) {
  if (confirm?.kind === "topic") return `Apagar o tópico ${confirm.topic.name}?`
  if (confirm?.kind === "entry") return `Apagar ${confirm.entry.name}?`
  return ""
}

/** Says what actually goes, because the fear is always about the notes. */
function confirmDetail(confirm: Confirm) {
  if (confirm?.kind === "topic") {
    const count = confirm.topic.members.length
    return `O tópico sai da lista e o manifesto é apagado. As ${count} ${
      count === 1 ? "nota ou print que ele reúne continua" : "notas e prints que ele reúne continuam"
    } no vault, intocadas.`
  }
  if (confirm?.kind === "entry") {
    return confirm.entry.kind === "folder"
      ? "A pasta e tudo dentro dela são apagados do disco. Não dá para desfazer."
      : "O arquivo é apagado do disco e sai dos tópicos em que estava. Não dá para desfazer."
  }
  return ""
}

function confirmLabel(confirm: Confirm) {
  if (confirm?.kind === "topic") return `Apagar ${confirm.topic.name}`
  if (confirm?.kind === "entry") return `Apagar ${confirm.entry.name}`
  return "Apagar"
}

/** A vault with nothing in it should say what to do, not shrug. */
function EmptyState({ vault, dropping }: { vault: VaultStatus | null; dropping: boolean }) {
  const empty = (vault?.notes ?? 0) === 0 && (vault?.images ?? 0) === 0

  return (
    <div
      className={cn(
        "mt-6 rounded-lg border border-dashed p-6 text-center transition-colors",
        dropping ? "border-found bg-found/5" : "border-border",
      )}
    >
      <p className="text-sm text-muted-foreground">
        {empty
          ? "Crie a primeira nota pelo ícone de pastas, ou arraste um print para esta janela."
          : "Digite para buscar. Prints entram arrastando ou com Cmd+V dentro de uma nota."}
      </p>
      {vault && <p className="mt-2 font-mono text-xs text-muted-foreground">{vault.root}</p>}
    </div>
  )
}