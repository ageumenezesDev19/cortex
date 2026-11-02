import { getCurrentWebview } from "@tauri-apps/api/webview"
import { ArrowLeft, FilePlus, FolderPlus, Sparkles } from "lucide-react"
import { AnimatePresence, motion, useReducedMotion } from "motion/react"
import { useCallback, useEffect, useMemo, useRef, useState } from "react"

import { ConfirmDialog } from "./components/confirm-dialog"
import { DropToast, type Dropped } from "./components/drop-toast"
import { ImportPanel } from "./components/import-panel"
import { CreateDialog } from "./components/create-dialog"
import { ImageReader } from "./components/image-reader"
import { NotePanel } from "./components/note-panel"
import { PromptDialog } from "./components/prompt-dialog"
import { Rail } from "./components/rail"
import { usePick } from "./components/language-provider"
import { AnswerPanel } from "./components/answer-panel"
import { ClassifyPanel } from "./components/classify-panel"
import { SettingsPanel } from "./components/settings-panel"
import { SharePanel } from "./components/share-panel"
import { ResultGroup } from "./components/result-group"
import { SearchField } from "./components/search-field"
import { StatusBar } from "./components/status-bar"
import { TopicsPanel } from "./components/topics-panel"
import { TreePanel } from "./components/tree-panel"
import {
  addToTopic,
  aiStatus,
  ask as askVault,
  cancelAsk,
  classifyExisting,
  classifyPreview,
  createFolder,
  createNote,
  createTopic,
  deleteEntry,
  deleteTopic,
  importImages,
  importNotes,
  uploadImage,
  importPreview,
  isDemo,
  isDesktop,
  listTopics,
  listTree,
  onClassifyStatus,
  onImportStatus,
  onIndexStatus,
  renameEntry,
  search,
  vaultStatus,
  type AiAnswer,
  type AiStatus,
  type ClassifyStatus,
  type Group,
  type Hit,
  type Source as Citation,
  type ImportPreview,
  type ImportStatus,
  type IndexStatus,
  type Topic,
  type TreeEntry,
  type VaultStatus,
} from "./lib/api"
import { DROP_ON_NOTE, noteSurfaceAt, type DropOnNote } from "./lib/drop"
import { useLocalToggle } from "./lib/prefs"
import { cn } from "./lib/utils"
import { useViewportHeight } from "./lib/viewport"

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
  | { kind: "image"; path: string; blocks?: Hit["blocks"]; from: Source }
  | null

type Ask =
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
  /** Set when nothing matched every word and the search accepted any of them. */
  const [widened, setWidened] = useState<string[] | null>(null)
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
  const [creating, setCreating] = useState<string | null>(null)
  const [dropped, setDropped] = useState<Dropped | null>(null)
  // Set only when a note is being created out of a dropped screenshot.
  const [embedding, setEmbedding] = useState<string | null>(null)
  const [importing, setImporting] = useState(false)
  const [sharing, setSharing] = useState(false)
  const [settings, setSettings] = useState(false)
  const photo = useRef<HTMLInputElement>(null)
  const [answer, setAnswer] = useState<AiAnswer | null>(null)
  const [answering, setAnswering] = useState(false)
  const [answerError, setAnswerError] = useState<string | null>(null)
  const [ai, setAi] = useState<AiStatus | null>(null)
  const [classifying, setClassifying] = useState(false)
  const [classifyPending, setClassifyPending] = useState<number | null>(null)
  const [classifyStatus, setClassifyStatus] = useState<ClassifyStatus | null>(null)
  const [preview, setPreview] = useState<ImportPreview | null>(null)
  const [importStatus, setImportStatus] = useState<ImportStatus | null>(null)

  const t = usePick()(content)
  const [aiInSearch, toggleAiInSearch] = useLocalToggle("cortex:ai-search")
  /** With the AI on, the literal list is behind a link rather than in the way. */
  const [showingResults, setShowingResults] = useState(false)

  const reduceMotion = useReducedMotion()
  const keyboardOpen = useViewportHeight()
  const results = useRef<HTMLDivElement>(null)
  /** The token of the answer being waited on, so a late one can be ignored. */
  const asking = useRef<number | null>(null)
  const searchField = useRef<HTMLInputElement>(null)

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
    aiStatus()
      .then(setAi)
      .catch(() => {})

    const stopStatus = onIndexStatus(setStatus)
    const stopImport = onImportStatus(setImportStatus)
    const stopClassify = onClassifyStatus(setClassifyStatus)
    return () => {
      stopStatus()
      stopImport()
      stopClassify()
    }
  }, [refreshTopics, refreshTree])

  const indexedChunks = status?.counts.chunks ?? vault?.chunks ?? 0

  // The indexer reports after every change, which is also the moment the tree
  // could have gained or lost a file.
  useEffect(() => {
    refreshTree()
  }, [indexedChunks, refreshTree])

  // The notice follows the real pipeline rather than a timer: the screenshot is
  // only "searchable" once the indexer has stopped working on it.
  useEffect(() => {
    if (!dropped || dropped.read) return
    if (status?.phase === "idle") setDropped({ ...dropped, read: true })
  }, [status, dropped])

  // Screenshots dropped on the window land in the vault; the watcher takes it
  // from there, exactly as if they had been saved from the Finder.
  // The browser's own drag and paste, for the demo. Tauri has its own event
  // below and never reaches this.
  useEffect(() => {
    if (!isDemo) return

    const take = async (files: FileList | null) => {
      const image = [...(files ?? [])].find((one) => one.type.startsWith("image/"))
      if (!image) return
      setDropping(false)
      setDropped({ path: await uploadImage(image), read: false })
    }

    const over = (event: DragEvent) => {
      event.preventDefault()
      setDropping(true)
    }
    const leave = () => setDropping(false)
    const drop = (event: DragEvent) => {
      event.preventDefault()
      void take(event.dataTransfer?.files ?? null).catch(() => setDropping(false))
    }
    const paste = (event: ClipboardEvent) => {
      const image = [...(event.clipboardData?.items ?? [])]
        .find((one) => one.type.startsWith("image/"))
        ?.getAsFile()
      if (image) void take({ 0: image, length: 1 } as unknown as FileList).catch(() => {})
    }

    window.addEventListener("dragover", over)
    window.addEventListener("dragleave", leave)
    window.addEventListener("drop", drop)
    window.addEventListener("paste", paste)
    return () => {
      window.removeEventListener("dragover", over)
      window.removeEventListener("dragleave", leave)
      window.removeEventListener("drop", drop)
      window.removeEventListener("paste", paste)
    }
  }, [])

  useEffect(() => {
    // Dragging a file onto the window is a desktop gesture; the phone sends a
    // photo through the picker instead.
    if (!isDesktop) return

    // Tauri reports the pointer in device pixels; the DOM is asked in CSS ones.
    const point = (position: { x: number; y: number }) => ({
      x: position.x / window.devicePixelRatio,
      y: position.y / window.devicePixelRatio,
    })

    // Which note is about to receive the print, said out loud while the file is
    // still in the air. Set on the element rather than carried down as a prop:
    // it is the element the hit test already found.
    let marked: HTMLElement | null = null
    const mark = (surface: HTMLElement | null) => {
      if (marked === surface) return
      if (marked) delete marked.dataset.dropTarget
      if (surface) surface.dataset.dropTarget = "true"
      marked = surface
    }

    const unlisten = getCurrentWebview().onDragDropEvent((event) => {
      if (event.payload.type === "leave") {
        mark(null)
        return setDropping(false)
      }
      if (event.payload.type === "over") {
        const { x, y } = point(event.payload.position)
        mark(noteSurfaceAt(x, y))
        return setDropping(true)
      }
      if (event.payload.type !== "drop") return
      setDropping(false)
      mark(null)

      // A print let go over the open note belongs in that note, at the spot it
      // was dropped. Anywhere else in the window it goes to the inbox, which is
      // where every drop used to land regardless.
      const { x, y } = point(event.payload.position)
      const { paths } = event.payload
      if (noteSurfaceAt(x, y)) {
        window.dispatchEvent(
          new CustomEvent<DropOnNote>(DROP_ON_NOTE, { detail: { paths, x, y } }),
        )
        return
      }

      importImages(paths)
        .then((written) => {
          const last = written[written.length - 1]
          if (last) setDropped({ path: last, read: false })
        })
        .catch(() => {})
    })
    return () => {
      mark(null)
      unlisten.then((stop) => stop())
    }
  }, [])

  useEffect(() => {
    const term = query.trim()
    if (term.length === 0 && !topic) {
      setGroups([])
      setWidened(null)
      return
    }
    const timer = setTimeout(() => {
      search(term, topic?.slug ?? null)
        .then((found) => {
          setGroups(found.groups)
          setWidened(found.widened ? found.terms : null)
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

  /**
   * A question is a search that ends in `?`. The literal results stay on
   * screen — the answer is an extra, never the only road.
   */
  const runAsk = useCallback((question: string) => {
    setShowingResults(false)
    // Random rather than sequential: the phone and the Mac ask the same server,
    // and a counter starting at 1 on both would let one cancel the other.
    const token = Math.floor(Math.random() * 2 ** 48)
    asking.current = token
    setAnswering(true)
    setAnswer(null)
    setAnswerError(null)
    askVault(question, token)
      .then((answer) => asking.current === token && setAnswer(answer))
      .catch((reason) => {
        // A cancelled call comes back as an error; the person already knows.
        if (asking.current !== token) return
        setAnswerError(String(reason).replace(/^Error:\s*/, ""))
      })
      .finally(() => {
        if (asking.current === token) setAnswering(false)
      })
  }, [])

  /** Kills the call, rather than leaving it running with nobody listening. */
  const cancelAnswer = useCallback(() => {
    const token = asking.current
    asking.current = null
    setAnswering(false)
    setAnswer(null)
    setAnswerError(null)
    if (token !== null) cancelAsk(token).catch(() => {})
  }, [])

  /** A citation lands on the passage through the same anchor a hit uses. */
  const openSource = useCallback((source: Citation) => {
    setOpen({ kind: "note", path: source.path, anchorStart: source.startOffset, from: "search" })
  }, [])

  const openHit = useCallback((hit: Hit) => {
    setOpen(
      hit.kind === "image"
        ? { kind: "image", path: hit.path, blocks: hit.blocks, from: "search" }
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

  /**
   * One ladder for Escape, innermost first, registered on the window so it also
   * answers with the cursor inside a note. Dialogs handle their own Escape, so
   * this stands down while one is up.
   */
  useEffect(() => {
    const onEscape = (event: KeyboardEvent) => {
      if (event.key !== "Escape" || ask || confirm || creating !== null || importing) return
      const active = document.activeElement as HTMLElement | null

      if (active?.closest("[data-note-surface]")) {
        active.blur()
        setOpen(null)
      } else if (open) {
        setOpen(null)
      } else if (query.length > 0) {
        changeQuery("")
      } else if (topic) {
        clearTopic()
      } else if (panel !== "none") {
        setPanel("none")
      } else {
        return
      }

      event.preventDefault()
      searchField.current?.focus()
    }

    window.addEventListener("keydown", onEscape)
    return () => window.removeEventListener("keydown", onEscape)
  }, [ask, confirm, creating, importing, open, query, topic, panel, changeQuery, clearTopic])

  useEffect(() => {
    const onNew = (event: KeyboardEvent) => {
      if (!(event.metaKey || event.ctrlKey) || event.key !== "n") return
      event.preventDefault()
      setCreating("")
    }
    window.addEventListener("keydown", onNew)
    return () => window.removeEventListener("keydown", onNew)
  }, [])

  const onKeyDown = (event: React.KeyboardEvent<HTMLInputElement>) => {
    if (event.key === "ArrowDown") {
      event.preventDefault()
      move(1)
    } else if (event.key === "ArrowUp") {
      event.preventDefault()
      move(-1)
    } else if (event.key === "Enter") {
      event.preventDefault()
      // With the AI on, Enter is the question — the icon in the field already
      // said the AI is doing the searching, so a second button to confirm it
      // was a step that explained nothing.
      if (aiInSearch && query.trim().length > 1) runAsk(query.trim())
      else if (hits[selected]) openHit(hits[selected])
    } else if (event.key === "Backspace" && query.length === 0 && topic) {
      event.preventDefault()
      clearTopic()
    }
    // Escape is handled on the window, so it answers from inside a note too.
  }

  const browsing = query.trim().length > 0 || topic !== null
  const transition = useMemo(
    () => ({ duration: reduceMotion ? 0 : 0.18, ease: "easeOut" as const }),
    [reduceMotion],
  )

  const openPath = open?.path ?? null

  return (
    <div className="fixed inset-x-0 top-0 flex h-[var(--app-h,100dvh)] flex-col-reverse overflow-hidden bg-background sm:static sm:h-screen sm:flex-row">
      {!keyboardOpen && (
      <Rail
        panel={panel}
        onTogglePanel={(next) => setPanel((current) => (current === next ? "none" : next))}
        onNewNote={() => setCreating("")}
        onNewFolder={() => setAsk({ kind: "new-folder", parent: "" })}
        onShare={() => setSharing(true)}
        onSettings={() => setSettings(true)}
        onClassify={() => {
          setClassifying(true)
          setClassifyStatus(null)
          setClassifyPending(null)
          classifyPreview()
            .then(setClassifyPending)
            .catch(() => setClassifyPending(0))
        }}
        onSendPhoto={() => photo.current?.click()}
        onImport={() => {
          setImporting(true)
          setImportStatus(null)
          setPreview(null)
          importPreview()
            .then(setPreview)
            .catch(() => {})
        }}
      />
      )}

      {/* One presence for both, and `popLayout` so the panel on its way out
          stops holding its 256 px while the other one arrives. With one
          AnimatePresence each, the two columns overlapped for 180 ms and the
          search field jumped 180 px and came back. */}
      <AnimatePresence mode="popLayout" initial={false}>
        {panel === "topics" && (
      <TopicsPanel
        key="topics"
        topics={topics}
        activeSlug={topic?.slug ?? null}
        onClose={() => setPanel("none")}
        onPick={(picked) => {
          setTopic(picked.slug === topic?.slug ? null : picked)
          if (narrow()) setPanel("none")
        }}
        onCreate={(name, icon, color) => {
          createTopic(name, icon, color)
            .then(refreshTopics)
            .catch(() => {})
        }}
        onDelete={(target) => setConfirm({ kind: "topic", topic: target })}
      />
        )}

        {panel === "tree" && (
      <TreePanel
        key="tree"
        tree={tree}
        activePath={openPath}
        onClose={() => setPanel("none")}
        onOpen={(entry) => {
          setOpen(
            entry.kind === "image"
              ? { kind: "image", path: entry.path, from: "tree" }
              : { kind: "note", path: entry.path, anchorStart: null, from: "tree" },
          )
          // On a phone the panel is the whole screen, so leaving it open means
          // opening a note behind a wall. On a desktop it is a column beside
          // the reader and stays where it is.
          if (narrow()) setPanel("none")
        }}
        onCreateNote={(folder) => setCreating(folder)}
        onCreateFolder={(parent) => setAsk({ kind: "new-folder", parent })}
        onRename={(entry) => setAsk({ kind: "rename", entry })}
        onDelete={(entry) => setConfirm({ kind: "entry", entry })}
      />
        )}
      </AnimatePresence>

      {/* `min-h-0` is what lets this shrink to the shell instead of growing to
          the height of the note inside it. Without it the column overflowed,
          and since the shell is `column-reverse` on a phone, the overflow was
          cut off the top: no back button, no title, and nothing left to scroll. */}
      <div className="flex min-h-0 min-w-0 flex-1 flex-col">
        <motion.div
          layout={!reduceMotion}
          transition={transition}
          className={cn(
            "mx-auto w-full max-w-3xl px-4 sm:px-6",
            // Whatever else moves, what is being typed stays on screen.
            "sticky top-0 z-20 bg-background",
            browsing || open ? "pt-6" : "flex flex-1 flex-col justify-start pt-10 sm:justify-center sm:pt-0",
          )}
        >
          <SearchField
            inputRef={searchField}
            value={query}
            onChange={changeQuery}
            onKeyDown={onKeyDown}
            compact={browsing || open !== null}
            topic={topic}
            onClearTopic={clearTopic}
            ai={!isDemo && aiInSearch}
            onToggleAi={toggleAiInSearch}
            hideAi={isDemo}
          />
          {!browsing && !open && (
            <EmptyState
              vault={vault}
              dropping={dropping}
              onNewNote={() => setCreating("")}
              onNewFolder={() => setAsk({ kind: "new-folder", parent: "" })}
              onTry={changeQuery}
            />
          )}
        </motion.div>

        {(browsing || open) && (
          <div className="flex min-h-0 flex-1 gap-4 px-4 pb-4 pt-4 sm:px-6">
            {browsing && (
              <div
                ref={results}
                className={cn(
                  "scroll-y min-w-0 space-y-3",
                  open ? "hidden sm:block sm:w-[24rem] sm:shrink-0" : "mx-auto w-full max-w-3xl",
                )}
              >
                {/* Before Enter, no list and no call: just whether there is
                    anything to answer from. It comes out of the literal search,
                    which costs nothing, so you know the AI has material before
                    you spend a request on it. */}
                {!isDemo && aiInSearch && !answer && !answering && !answerError && (
                  <Readiness
                    ready={Boolean(ai?.keys.groq || ai?.keys.gemini)}
                    notes={groups.length}
                  />
                )}

                {(answer || answering || answerError) && (
                  <AnswerPanel
                    question={query.trim()}
                    answer={answer}
                    error={answerError}
                    loading={answering}
                    onOpenSource={openSource}
                    onOpenPlace={(place) =>
                      setOpen({
                        kind: "note",
                        path: place.path,
                        anchorStart: null,
                        from: "search",
                      })
                    }
                    onCancel={cancelAnswer}
                    onClose={() => {
                      setAnswer(null)
                      setAnswerError(null)
                    }}
                  />
                )}

                {/* A search that had to be widened says so. Nothing matched
                    every word, and pretending otherwise is how the app would
                    look like it answered the question it was asked. */}
                {widened && groups.length > 0 && (
                  <p className="font-mono text-xs text-muted-foreground">
                    {t.widened(widened.join(", "))}
                  </p>
                )}

                {aiInSearch && (answer || answering) && !showingResults && (
                  <button
                    type="button"
                    onClick={() => setShowingResults(true)}
                    className="font-mono text-xs text-muted-foreground underline underline-offset-2 transition-colors hover:text-foreground"
                  >
                    ver os {groups.length} resultados da busca
                  </button>
                )}

                {(!aiInSearch || showingResults || (!answer && !answering)) &&
                  groups.map((group, position) => (
                  <motion.div
                    key={group.key}
                    initial={reduceMotion ? { opacity: 0 } : { opacity: 0, y: 6 }}
                    animate={{ opacity: 1, y: 0 }}
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

                {groups.length === 0 && !aiInSearch && (
                  <p className="pt-8 text-center font-mono text-sm text-muted-foreground">
                    {topic && query.trim().length === 0
                      ? `nada em ${topic.name} ainda`
                      : `nada encontrado para “${query.trim()}”`}
                  </p>
                )}
              </div>
            )}

            {open && (
              <div className="flex min-h-0 min-w-0 flex-1 flex-col rounded-lg border bg-card p-3 sm:p-4">
                {/* Esc does this on the desktop; a phone has no Esc. */}
                <button
                  type="button"
                  onClick={() => setOpen(null)}
                  className="mb-2 flex items-center gap-1.5 self-start rounded-md py-1 pr-2 text-sm text-muted-foreground sm:hidden"
                >
                  <ArrowLeft className="size-4" aria-hidden />
                  voltar
                </button>
                <div className="min-h-0 min-w-0 flex-1">
                {open.kind === "image" ? (
                  <ImageReader
                    key={open.path}
                    path={open.path}
                    blocks={open.blocks}
                    query={query}
                    root={vault?.root ?? ""}
                  />
                ) : (
                  <NotePanel
                    key={open.path}
                    path={open.path}
                    anchorStart={open.anchorStart}
                    root={vault?.root ?? ""}
                  />
                )}
                </div>
              </div>
            )}
          </div>
        )}

        <StatusBar vault={vault} status={status} />
      </div>

      {/* The phone's own picker, which on iOS offers the camera as well. The
          photo lands in the vault and the watcher reads it, exactly as a
          screenshot dropped on the window does. */}
      <input
        ref={photo}
        type="file"
        accept="image/*"
        className="hidden"
        onChange={(event) => {
          const file = event.target.files?.[0]
          event.target.value = ""
          if (!file) return
          uploadImage(file)
            .then((path) => setDropped({ path, read: false }))
            .catch(() => {})
        }}
      />

      <ClassifyPanel
        open={classifying}
        pending={classifyPending}
        status={classifyStatus}
        onCancel={() => setClassifying(false)}
        onStart={() => {
          setClassifyStatus(null)
          classifyExisting().catch(() => {})
        }}
      />

      <SettingsPanel open={settings} onClose={() => setSettings(false)} onChanged={setAi} />

      <SharePanel open={sharing} onClose={() => setSharing(false)} />

      <PromptDialog
        open={ask !== null}
        title={promptTitle(ask)}
        placeholder={ask?.kind === "new-folder" ? t.folderName : t.noteTitle}
        initialValue={ask?.kind === "rename" ? ask.entry.name : ""}
        confirmLabel={ask?.kind === "rename" ? t.rename : t.create}
        onCancel={() => setAsk(null)}
        onConfirm={(value) => {
          const action =
            ask?.kind === "new-folder"
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

      <ImportPanel
        open={importing}
        preview={preview}
        status={importStatus}
        onCancel={() => {
          setImporting(false)
          setImportStatus(null)
        }}
        onStart={() => {
          importNotes().catch(() => {})
        }}
      />

      <DropToast
        dropped={dropped}
        root={vault?.root ?? ""}
        onDismiss={() => setDropped(null)}
        onOpen={(path) => {
          setOpen({ kind: "image", path, from: "tree" })
          setDropped(null)
        }}
        onCreateNote={(path) => {
          setEmbedding(path)
          setCreating("")
          setDropped(null)
        }}
      />

      <CreateDialog
        open={creating !== null}
        tree={tree}
        topics={topics}
        startingFolder={creating ?? ""}
        onCancel={() => {
          setCreating(null)
          setEmbedding(null)
        }}
        onCreate={(title, folder, chosen) => {
          createNote(folder, title, chosen, embedding)
            .then((path) => {
              setOpen({ kind: "note", path, anchorStart: null, from: "tree" })
              refreshTree()
              refreshTopics()
            })
            .catch(() => {})
          setCreating(null)
          setEmbedding(null)
        }}
      />

      <ConfirmDialog
        open={confirm !== null}
        title={confirmTitle(confirm)}
        detail={confirmDetail(confirm, t)}
        confirmLabel={confirmLabel(confirm, t)}
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
  if (ask?.kind === "new-folder") return ask.parent ? `Nova pasta em ${ask.parent}` : "Nova pasta"
  if (ask?.kind === "rename") return `Renomear ${ask.entry.name}`
  return ""
}

function confirmTitle(confirm: Confirm) {
  if (confirm?.kind === "topic") return `Apagar o tópico ${confirm.topic.name}?`
  if (confirm?.kind === "entry") return `Apagar ${confirm.entry.name}?`
  return ""
}

const content = {
  en: {
    folderName: "folder name",
    noteTitle: "note title",
    rename: "Rename",
    create: "Create",
    noKey: "To answer from here, put a Groq or Gemini key in the AI panel. Without one the search keeps working — just switch the icon off in the field.",
    nothingFound: "nothing found — Enter asks anyway",
    notesSpeak: (n: number) => `${n} ${n === 1 ? "note mentions" : "notes mention"} this · Enter to ask`,
    emptyDesktop: "Empty vault. Start a note, or drag a screenshot into this window.",
    emptyPhone: "Empty vault. Start a note, or send a photo with the +.",
    howDesktop: "Type to search. Screenshots come in by dragging, or with Cmd+V inside a note.",
    howPhone: "Type to search. Photos come in through the + down here.",
    howDemo: "Ten notes on Blender, Rust, CSS, Git and bread. Search:",
    dropHere: "Screenshots come in by dragging them here, or with Cmd+V inside a note.",
    widened: (terms: string) => `nothing had every word — showing what mentions ${terms}`,
    /** Phrases, not questions: a question expects an answer, and without a
     *  provider this returns a list of notes. Each was measured against the
     *  vault; the ones that landed on the wrong note were dropped. */
    examples: [
      "flat looking scene",
      "wrong commit message",
      "columns that resize on their own",
      "borrow checker complaints",
    ],
    newNote: "New note",
    newFolder: "New folder",
    topicEmpty: "The topic gathers nothing yet, so nothing more is lost with it.",
    topicKeeps: (n: number) =>
      `The ${n} ${n === 1 ? "note or screenshot stays" : "notes and screenshots stay"} in the vault, untouched — but the list of what belongs to this topic is gone, and rebuilding it is by hand.`,
    folderGone: "The folder and everything in it are deleted from disk. There is no undo.",
    fileGone: "The file is deleted from disk and leaves the topics it was in. There is no undo.",
    remove: (name: string) => `Delete ${name}`,
    removePlain: "Delete",
  },
  pt: {
    folderName: "nome da pasta",
    noteTitle: "título da nota",
    rename: "Renomear",
    create: "Criar",
    noKey: "Para responder daqui, ponha uma chave do Groq ou do Gemini no painel de IA. Sem ela a busca segue funcionando — é só desligar o ícone no campo.",
    nothingFound: "nada encontrado — Enter pergunta assim mesmo",
    notesSpeak: (n: number) => `${n} ${n === 1 ? "nota fala" : "notas falam"} disso · Enter para perguntar`,
    emptyDesktop: "Vault vazio. Comece uma nota, ou arraste um print para esta janela.",
    emptyPhone: "Vault vazio. Comece uma nota, ou mande uma foto pelo +.",
    howDesktop: "Digite para buscar. Prints entram arrastando ou com Cmd+V dentro de uma nota.",
    howPhone: "Digite para buscar. Fotos entram pelo + aqui embaixo.",
    howDemo: "Dez notas sobre Blender, Rust, CSS, Git e pão. Busque:",
    dropHere: "Prints entram arrastando para cá, ou com Cmd+V dentro de uma nota.",
    widened: (terms: string) => `nada com todas as palavras — mostrando o que fala de ${terms}`,
    examples: [
      "mensagem de commit errada",
      "colunas que mudam de tamanho sozinhas",
      "reclamação do compilador",
      "perder o foco quando interrompem",
    ],
    newNote: "Nova nota",
    newFolder: "Nova pasta",
    topicEmpty: "O tópico não reúne nada ainda, então nada mais se perde com ele.",
    topicKeeps: (n: number) =>
      `As ${n} ${n === 1 ? "nota ou print continua" : "notas e prints continuam"} no vault, intocadas — mas a lista de que ${n === 1 ? "pertence" : "pertencem"} a este tópico se perde, e refazer é na mão.`,
    folderGone: "A pasta e tudo dentro dela são apagados do disco. Não dá para desfazer.",
    fileGone: "O arquivo é apagado do disco e sai dos tópicos em que estava. Não dá para desfazer.",
    remove: (name: string) => `Apagar ${name}`,
    removePlain: "Apagar",
  },
}

/**
 * Says what actually goes. The fear is always about the notes, and they stay —
 * but the curation is the work, and that is what leaves. No word from the code
 * ("manifest", "index") belongs in a sentence someone reads under pressure.
 */
function confirmDetail(confirm: Confirm, t: (typeof content)["en"]) {
  if (confirm?.kind === "topic") {
    const count = confirm.topic.members.length
    return count === 0 ? t.topicEmpty : t.topicKeeps(count)
  }
  if (confirm?.kind === "entry") {
    return confirm.entry.kind === "folder" ? t.folderGone : t.fileGone
  }
  return ""
}

function confirmLabel(confirm: Confirm, t: (typeof content)["en"]) {
  if (confirm?.kind === "topic") return t.remove(confirm.topic.name)
  if (confirm?.kind === "entry") return t.remove(confirm.entry.name)
  return t.removePlain
}

/** A vault with nothing in it should say what to do, not shrug — and the way to
 *  start writing belongs here, not inside a panel you have to know about. */
/** Whether the panels are covering the screen rather than sitting beside it. */
const narrow = () => !window.matchMedia("(min-width: 640px)").matches

/**
 * What the AI would have to work with, before it is asked. The count comes from
 * the search that already ran for free, so this line promises material rather
 * than describing a button.
 */
function Readiness({ ready, notes }: { ready: boolean; notes: number }) {
  const t = usePick()(content)

  if (!ready) {
    return (
      <p className="rounded-lg border border-dashed px-4 py-3 text-xs text-muted-foreground">
        {t.noKey}
      </p>
    )
  }

  return (
    <p className="flex items-center gap-2 font-mono text-xs text-muted-foreground">
      <Sparkles className="size-3.5 text-found" aria-hidden />
      {notes === 0 ? t.nothingFound : t.notesSpeak(notes)}
    </p>
  )
}

function EmptyState({
  vault,
  dropping,
  onNewNote,
  onNewFolder,
  onTry,
}: {
  vault: VaultStatus | null
  dropping: boolean
  onNewNote: () => void
  onNewFolder: () => void
  /** Demo only: nobody can search a vault they have never read without being
   *  handed a question that works. */
  onTry: (query: string) => void
}) {
  const t = usePick()(content)
  const empty = (vault?.notes ?? 0) === 0 && (vault?.images ?? 0) === 0

  return (
    <div
      className={cn(
        "mt-6 rounded-lg border border-dashed p-6 text-center transition-colors",
        dropping ? "border-found bg-found/5" : "border-border",
      )}
    >
      <p className="text-sm text-muted-foreground">
        {isDemo
          ? t.howDemo
          : empty
            ? isDesktop
              ? t.emptyDesktop
              : t.emptyPhone
            : isDesktop
              ? t.howDesktop
              : t.howPhone}
      </p>

      {isDemo && (
        <div className="mt-4 flex flex-wrap justify-center gap-2">
          {t.examples.map((example) => (
            <button
              key={example}
              type="button"
              onClick={() => onTry(example)}
              className="rounded-full border px-3 py-1.5 font-mono text-xs text-muted-foreground transition-colors hover:border-primary/50 hover:text-foreground"
            >
              {example}
            </button>
          ))}
        </div>
      )}

      {isDemo && <p className="mt-6 text-xs text-muted-foreground">{t.dropHere}</p>}

      <div className="mt-6 flex justify-center gap-4 border-t border-dashed pt-6">
        <button
          type="button"
          onClick={onNewNote}
          className="flex items-center gap-1.5 rounded-md bg-primary px-3 py-1.5 text-sm text-primary-foreground transition-opacity hover:opacity-90"
        >
          <FilePlus className="size-4" aria-hidden />
          {t.newNote}
        </button>
        <button
          type="button"
          onClick={onNewFolder}
          className="flex items-center gap-1.5 rounded-md border px-3 py-1.5 text-sm text-muted-foreground transition-colors hover:text-foreground"
        >
          <FolderPlus className="size-4" aria-hidden />
          {t.newFolder}
        </button>
      </div>

      {vault && isDesktop && (
        <p className="mt-4 font-mono text-xs text-muted-foreground">{vault.root}</p>
      )}
    </div>
  )
}