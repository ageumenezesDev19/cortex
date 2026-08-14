import { listen } from "@tauri-apps/api/event"
import { getCurrentWebview } from "@tauri-apps/api/webview"
import { AnimatePresence, motion, useReducedMotion } from "motion/react"
import { useCallback, useEffect, useMemo, useRef, useState } from "react"

import { ImageReader } from "./components/image-reader"
import { NoteReader } from "./components/note-reader"
import { Rail } from "./components/rail"
import { ResultGroup } from "./components/result-group"
import { SearchField } from "./components/search-field"
import { StatusBar } from "./components/status-bar"
import { TopicsPanel } from "./components/topics-panel"
import {
  addToTopic,
  createTopic,
  deleteTopic,
  importImages,
  INDEX_STATUS_EVENT,
  listTopics,
  search,
  vaultStatus,
  type Group,
  type Hit,
  type IndexStatus,
  type Topic,
  type VaultStatus,
} from "./lib/api"
import { cn } from "./lib/utils"

/** Fast enough that the results feel like they were already there. */
const DEBOUNCE_MS = 120

export default function App() {
  const [query, setQuery] = useState("")
  const [groups, setGroups] = useState<Group[]>([])
  const [selected, setSelected] = useState(0)
  const [open, setOpen] = useState<Hit | null>(null)
  const [status, setStatus] = useState<IndexStatus | null>(null)
  const [vault, setVault] = useState<VaultStatus | null>(null)
  const [dropping, setDropping] = useState(false)
  const [topics, setTopics] = useState<Topic[]>([])
  const [topic, setTopic] = useState<Topic | null>(null)
  const [topicsOpen, setTopicsOpen] = useState(false)

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

  useEffect(() => {
    vaultStatus()
      .then(setVault)
      .catch(() => {})
    refreshTopics()

    const unlisten = listen<IndexStatus>(INDEX_STATUS_EVENT, (event) => setStatus(event.payload))
    return () => {
      unlisten.then((stop) => stop())
    }
  }, [refreshTopics])

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

  const indexedChunks = status?.counts.chunks ?? vault?.chunks ?? 0

  useEffect(() => {
    const term = query.trim()
    if (term.length === 0 && !topic) {
      setGroups([])
      setOpen(null)
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

  const onKeyDown = (event: React.KeyboardEvent<HTMLInputElement>) => {
    if (event.key === "ArrowDown") {
      event.preventDefault()
      move(1)
    } else if (event.key === "ArrowUp") {
      event.preventDefault()
      move(-1)
    } else if (event.key === "Enter") {
      event.preventDefault()
      if (hits[selected]) setOpen(hits[selected])
    } else if (event.key === "Backspace" && query.length === 0 && topic) {
      event.preventDefault()
      setTopic(null)
    } else if (event.key === "Escape") {
      event.preventDefault()
      if (open) setOpen(null)
      else if (query.length > 0) setQuery("")
      else setTopic(null)
    }
  }

  const browsing = query.trim().length > 0 || topic !== null
  const transition = useMemo(
    () => ({ duration: reduceMotion ? 0 : 0.18, ease: "easeOut" as const }),
    [reduceMotion],
  )

  return (
    <div className="flex h-screen overflow-hidden bg-background">
      <Rail topicsOpen={topicsOpen} onToggleTopics={() => setTopicsOpen((value) => !value)} />

      <TopicsPanel
        open={topicsOpen}
        topics={topics}
        activeSlug={topic?.slug ?? null}
        onClose={() => setTopicsOpen(false)}
        onPick={(picked) => setTopic(picked.slug === topic?.slug ? null : picked)}
        onCreate={(name, icon, color) => {
          createTopic(name, icon, color)
            .then(refreshTopics)
            .catch(() => {})
        }}
        onDelete={(slug) => {
          deleteTopic(slug)
            .then(() => {
              if (topic?.slug === slug) setTopic(null)
              refreshTopics()
            })
            .catch(() => {})
        }}
      />

      <div className="flex min-w-0 flex-1 flex-col">
        <motion.div
          layout={!reduceMotion}
          transition={transition}
          className={cn(
            "mx-auto w-full max-w-3xl px-6",
            browsing ? "pt-6" : "flex flex-1 flex-col justify-center",
          )}
        >
          <SearchField
            value={query}
            onChange={setQuery}
            onKeyDown={onKeyDown}
            compact={browsing}
            topic={topic}
            onClearTopic={() => setTopic(null)}
          />
          {!browsing && <EmptyState vault={vault} dropping={dropping} />}
        </motion.div>

        {browsing && (
          <div className="flex min-h-0 flex-1 gap-4 px-6 pb-4 pt-4">
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
                    data-hit-group={group.key}
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
                      onOpen={setOpen}
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

            {open && (
              <div className="min-h-0 min-w-0 flex-1 rounded-lg border bg-card p-4">
                {open.kind === "image" ? (
                  <ImageReader hit={open} query={query} root={vault?.root ?? ""} />
                ) : (
                  <NoteReader hit={open} />
                )}
              </div>
            )}
          </div>
        )}

        <StatusBar vault={vault} status={status} />
      </div>
    </div>
  )
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
          ? "Arraste um print para esta janela — ele vira texto pesquisável sozinho."
          : "Digite para buscar. Prints entram arrastando; o OCR é automático."}
      </p>
      {vault && <p className="mt-2 font-mono text-xs text-muted-foreground">{vault.root}</p>}
    </div>
  )
}
