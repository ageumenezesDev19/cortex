import { Sparkles } from "lucide-react"
import { AnimatePresence, motion, useReducedMotion } from "motion/react"
import { useEffect, useMemo, useRef, useState } from "react"

import { suggestPlacement, type Topic, type TreeEntry } from "../lib/api"
import { cn } from "../lib/utils"
import { BrandIcon } from "./brand-icon"

/** Same beat as the search field: fast enough to feel like it already knew. */
const SUGGEST_MS = 120

type Props = {
  open: boolean
  tree: TreeEntry[]
  topics: Topic[]
  /** Folder the action started from; empty string is the vault root. */
  startingFolder: string
  onCancel: () => void
  onCreate: (title: string, folder: string, topics: string[]) => void
}

export function CreateDialog({
  open,
  tree,
  topics,
  startingFolder,
  onCancel,
  onCreate,
}: Props) {
  const reduceMotion = useReducedMotion()
  const [title, setTitle] = useState("")
  const [folder, setFolder] = useState(startingFolder)
  const [chosen, setChosen] = useState<string[]>([])
  const [suggested, setSuggested] = useState(false)

  // Once you touch the filing, the app stops second-guessing it.
  const [touched, setTouched] = useState(false)

  const folders = useMemo(() => foldersIn(tree), [tree])
  const name = useRef<HTMLInputElement>(null)

  useEffect(() => {
    if (!open) return
    setTitle("")
    setFolder(startingFolder)
    setChosen([])
    setSuggested(false)
    setTouched(false)
    name.current?.focus()
  }, [open, startingFolder])

  useEffect(() => {
    if (!open || touched || title.trim().length < 3) return

    const timer = setTimeout(() => {
      suggestPlacement(title)
        .then((placement) => {
          if (touched) return
          setFolder(placement.folder ?? startingFolder)
          setChosen(placement.topics)
          setSuggested(placement.folder !== null || placement.topics.length > 0)
        })
        .catch(() => {})
    }, SUGGEST_MS)

    return () => clearTimeout(timer)
  }, [open, title, touched, startingFolder])

  const toggleTopic = (slug: string) => {
    setTouched(true)
    setSuggested(false)
    setChosen((current) =>
      current.includes(slug) ? current.filter((one) => one !== slug) : [...current, slug],
    )
  }

  return (
    <AnimatePresence>
      {open && (
        <motion.div
          initial={{ opacity: 0 }}
          animate={{ opacity: 1 }}
          exit={{ opacity: 0 }}
          transition={{ duration: reduceMotion ? 0 : 0.12 }}
          className="fixed inset-0 z-50 flex items-center justify-center bg-background/80 p-6"
        >
          <motion.form
            aria-label="Nova nota"
            initial={reduceMotion ? { opacity: 0 } : { opacity: 0, scale: 0.98 }}
            animate={{ opacity: 1, scale: 1 }}
            exit={reduceMotion ? { opacity: 0 } : { opacity: 0, scale: 0.98 }}
            transition={{ duration: reduceMotion ? 0 : 0.14, ease: "easeOut" }}
            onSubmit={(event) => {
              event.preventDefault()
              if (title.trim()) onCreate(title.trim(), folder, chosen)
            }}
            onKeyDown={(event) => event.key === "Escape" && onCancel()}
            className="w-full max-w-md rounded-lg border bg-card p-5 shadow-xl"
          >
            <h2 className="font-medium">Nova nota</h2>

            <label className="mt-4 block">
              <span className="font-mono text-xs text-muted-foreground">Nome</span>
              <input
                ref={name}
                value={title}
                onChange={(event) => setTitle(event.target.value)}
                placeholder="do que trata esta nota"
                spellCheck={false}
                autoFocus
                className="mt-1 h-9 w-full rounded-md border bg-background px-2 text-sm outline-none focus:border-ring"
              />
            </label>

            <label className="mt-3 block">
              <span className="flex items-center gap-1.5 font-mono text-xs text-muted-foreground">
                Pasta
                {suggested && <Suggested />}
              </span>
              <select
                value={folder}
                onChange={(event) => {
                  setTouched(true)
                  setSuggested(false)
                  setFolder(event.target.value)
                }}
                className="mt-1 h-9 w-full rounded-md border bg-background px-2 text-sm outline-none focus:border-ring"
              >
                <option value="">/ (raiz do vault)</option>
                {folders.map((path) => (
                  <option key={path} value={path}>
                    {path}
                  </option>
                ))}
              </select>
            </label>

            {topics.length > 0 && (
              <div className="mt-3">
                <span className="flex items-center gap-1.5 font-mono text-xs text-muted-foreground">
                  Tópicos
                  {suggested && chosen.length > 0 && <Suggested />}
                </span>
                <div className="mt-1.5 flex flex-wrap gap-1.5">
                  {topics.map((topic) => (
                    <button
                      key={topic.slug}
                      type="button"
                      onClick={() => toggleTopic(topic.slug)}
                      aria-pressed={chosen.includes(topic.slug)}
                      className={cn(
                        "flex items-center gap-1.5 rounded-md border px-2 py-1 text-xs transition-colors",
                        chosen.includes(topic.slug)
                          ? "border-ring bg-muted text-foreground"
                          : "text-muted-foreground hover:text-foreground",
                      )}
                    >
                      <BrandIcon slug={topic.icon} color={topic.color} className="size-3" />
                      {topic.name}
                    </button>
                  ))}
                </div>
              </div>
            )}

            <div className="mt-5 flex justify-end gap-2">
              <button
                type="button"
                onClick={onCancel}
                className="rounded-md px-3 py-1.5 text-sm text-muted-foreground transition-colors hover:text-foreground"
              >
                Cancelar
              </button>
              <button
                type="submit"
                disabled={title.trim().length === 0}
                className="rounded-md bg-primary px-3 py-1.5 text-sm text-primary-foreground transition-opacity hover:opacity-90 disabled:opacity-40"
              >
                Criar
              </button>
            </div>
          </motion.form>
        </motion.div>
      )}
    </AnimatePresence>
  )
}

/** Marks a field the app filled in, so it never passes for your own choice. */
function Suggested() {
  return (
    <span className="flex items-center gap-1 text-found">
      <Sparkles className="size-3" aria-hidden />
      sugerido
    </span>
  )
}

function foldersIn(tree: TreeEntry[]): string[] {
  return tree.flatMap((entry) =>
    entry.kind === "folder" ? [entry.path, ...foldersIn(entry.children)] : [],
  )
}