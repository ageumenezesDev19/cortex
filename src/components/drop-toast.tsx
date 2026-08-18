import { Check, FilePlus, Image as ImageIcon, Loader2, X } from "lucide-react"
import { AnimatePresence, motion, useReducedMotion } from "motion/react"
import { useEffect } from "react"

import { assetUrl } from "../lib/api"
import { usePick } from "./language-provider"

/** Long enough to read and act on, short enough not to sit in the way. */
const DISMISS_MS = 9000

export type Dropped = {
  path: string
  /** Whether the recognition has already run for this file. */
  read: boolean
}

type Props = {
  dropped: Dropped | null
  root: string
  onDismiss: () => void
  onOpen: (path: string) => void
  onCreateNote: (path: string) => void
}

/**
 * A screenshot used to land in silence: the file went in, the recognition ran,
 * and the screen looked exactly the same. This says it arrived, and follows the
 * real pipeline rather than guessing — `read` comes from the indexer's own
 * status events.
 */
const content = {
  en: {
    saved: "Screenshot saved and already searchable",
    reading: "Reading the text in the screenshot…",
    dismiss: "Dismiss",
    newNote: "Make a note from it",
    open: "Open",
  },
  pt: {
    saved: "Print salvo e já pesquisável",
    reading: "Lendo o texto do print…",
    dismiss: "Dispensar",
    newNote: "Criar nota com ele",
    open: "Abrir",
  },
}

export function DropToast({ dropped, root, onDismiss, onOpen, onCreateNote }: Props) {
  const t = usePick()(content)
  const reduceMotion = useReducedMotion()

  useEffect(() => {
    if (!dropped?.read) return
    const timer = setTimeout(onDismiss, DISMISS_MS)
    return () => clearTimeout(timer)
  }, [dropped, onDismiss])

  return (
    <AnimatePresence>
      {dropped && (
        <motion.aside
          role="status"
          initial={reduceMotion ? { opacity: 0 } : { opacity: 0, x: 24 }}
          animate={{ opacity: 1, x: 0 }}
          exit={reduceMotion ? { opacity: 0 } : { opacity: 0, x: 24 }}
          transition={{ duration: reduceMotion ? 0 : 0.18, ease: "easeOut" }}
          className="absolute bottom-14 right-6 z-40 w-72 rounded-lg border bg-card p-3 shadow-xl"
        >
          <div className="flex gap-3">
            <span className="size-12 shrink-0 overflow-hidden rounded-md border bg-muted">
              <img
                src={assetUrl(root, dropped.path)}
                alt=""
                className="size-full object-cover object-top"
                draggable={false}
              />
            </span>

            <div className="min-w-0 flex-1">
              <p className="flex items-center gap-1.5 text-sm">
                {dropped.read ? (
                  <>
                    <Check className="size-3.5 text-found" aria-hidden />
                    {t.saved}
                  </>
                ) : (
                  <>
                    <Loader2 className="size-3.5 animate-spin text-muted-foreground" aria-hidden />
                    {t.reading}
                  </>
                )}
              </p>
              <p className="mt-0.5 truncate font-mono text-xs text-muted-foreground">
                {dropped.path}
              </p>
            </div>

            <button
              type="button"
              onClick={onDismiss}
              aria-label={t.dismiss}
              className="h-fit rounded p-1 text-muted-foreground transition-colors hover:text-foreground"
            >
              <X className="size-3.5" aria-hidden />
            </button>
          </div>

          <div className="mt-3 flex gap-2">
            <button
              type="button"
              onClick={() => onCreateNote(dropped.path)}
              className="flex flex-1 items-center justify-center gap-1.5 rounded-md bg-primary px-2 py-1.5 text-xs text-primary-foreground transition-opacity hover:opacity-90"
            >
              <FilePlus className="size-3.5" aria-hidden />
              {t.newNote}
            </button>
            <button
              type="button"
              onClick={() => onOpen(dropped.path)}
              className="flex items-center justify-center gap-1.5 rounded-md border px-2 py-1.5 text-xs text-muted-foreground transition-colors hover:text-foreground"
            >
              <ImageIcon className="size-3.5" aria-hidden />
              {t.open}
            </button>
          </div>
        </motion.aside>
      )}
    </AnimatePresence>
  )
}