import { listen } from "@tauri-apps/api/event"
import { getCurrentWebview } from "@tauri-apps/api/webview"
import { AnimatePresence, motion, useReducedMotion } from "motion/react"
import { useCallback, useEffect, useMemo, useRef, useState } from "react"

import { ImageReader } from "./components/image-reader"
import { NoteReader } from "./components/note-reader"
import { Rail } from "./components/rail"
import { ResultCard } from "./components/result-card"
import { SearchField } from "./components/search-field"
import { StatusBar } from "./components/status-bar"
import {
  importImages,
  INDEX_STATUS_EVENT,
  search,
  vaultStatus,
  type Hit,
  type IndexStatus,
  type VaultStatus,
} from "./lib/api"
import { cn } from "./lib/utils"

/** Fast enough that the results feel like they were already there. */
const DEBOUNCE_MS = 120

export default function App() {
  const [query, setQuery] = useState("")
  const [hits, setHits] = useState<Hit[]>([])
  const [selected, setSelected] = useState(0)
  const [open, setOpen] = useState<Hit | null>(null)
  const [status, setStatus] = useState<IndexStatus | null>(null)
  const [vault, setVault] = useState<VaultStatus | null>(null)
  const [dropping, setDropping] = useState(false)

  const reduceMotion = useReducedMotion()
  const results = useRef<HTMLDivElement>(null)

  useEffect(() => {
    vaultStatus()
      .then(setVault)
      .catch(() => {})
    const unlisten = listen<IndexStatus>(INDEX_STATUS_EVENT, (event) => setStatus(event.payload))
    return () => {
      unlisten.then((stop) => stop())
    }
  }, [])

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
    if (term.length === 0) {
      setHits([])
      setOpen(null)
      return
    }
    const timer = setTimeout(() => {
      search(term)
        .then((found) => {
          setHits(found)
          setSelected(0)
        })
        .catch(() => setHits([]))
    }, DEBOUNCE_MS)

    return () => clearTimeout(timer)
    // `indexedChunks` re-runs the search when the indexer finishes new work, so
    // a screenshot dropped mid-search shows up without retyping.
  }, [query, indexedChunks])

  const move = useCallback(
    (delta: number) => {
      setSelected((current) => {
        if (hits.length === 0) return 0
        const next = Math.min(Math.max(current + delta, 0), hits.length - 1)
        results.current?.children[next]?.scrollIntoView({ block: "nearest" })
        return next
      })
    },
    [hits.length],
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
    } else if (event.key === "Escape") {
      event.preventDefault()
      if (open) setOpen(null)
      else setQuery("")
    }
  }

  const searching = query.trim().length > 0
  const transition = useMemo(
    () => ({ duration: reduceMotion ? 0 : 0.18, ease: "easeOut" as const }),
    [reduceMotion],
  )

  return (
    <div className="flex h-screen overflow-hidden bg-background">
      <Rail />

      <div className="flex min-w-0 flex-1 flex-col">
        <motion.div
          layout={!reduceMotion}
          transition={transition}
          className={cn(
            "mx-auto w-full max-w-3xl px-6",
            searching ? "pt-6" : "flex flex-1 flex-col justify-center",
          )}
        >
          <SearchField value={query} onChange={setQuery} onKeyDown={onKeyDown} compact={searching} />
          {!searching && <EmptyState vault={vault} dropping={dropping} />}
        </motion.div>

        {searching && (
          <div className="flex min-h-0 flex-1 gap-4 px-6 pb-4 pt-4">
            <div
              ref={results}
              className={cn(
                "min-h-0 space-y-2 overflow-y-auto",
                open ? "w-[22rem] shrink-0" : "mx-auto w-full max-w-3xl",
              )}
            >
              <AnimatePresence initial={false}>
                {hits.map((hit, position) => (
                  <motion.div
                    key={hit.chunkId}
                    initial={reduceMotion ? { opacity: 0 } : { opacity: 0, y: 6 }}
                    animate={{ opacity: 1, y: 0 }}
                    exit={{ opacity: 0 }}
                    transition={{
                      duration: reduceMotion ? 0 : 0.12,
                      delay: reduceMotion ? 0 : Math.min(position, 8) * 0.02,
                    }}
                  >
                    <ResultCard
                      hit={hit}
                      query={query}
                      root={vault?.root ?? ""}
                      selected={position === selected}
                      onSelect={() => setSelected(position)}
                      onOpen={() => setOpen(hit)}
                    />
                  </motion.div>
                ))}
              </AnimatePresence>

              {hits.length === 0 && (
                <p className="pt-8 text-center font-mono text-sm text-muted-foreground">
                  nada encontrado para “{query.trim()}”
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
