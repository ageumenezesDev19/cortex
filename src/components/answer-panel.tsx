import { FolderOpen, Sparkles, X } from "lucide-react"
import { motion, useReducedMotion } from "motion/react"
import { Fragment } from "react"

import { readable } from "../lib/failures"
import type { AiAnswer, Place, Source } from "../lib/api"
import { cn } from "../lib/utils"
import { ProgressLine } from "./progress-line"

type Props = {
  question: string
  answer: AiAnswer | null
  error: string | null
  loading: boolean
  onOpenSource: (source: Source) => void
  /** A place is a note: opening one is opening it, not filtering by it. */
  onOpenPlace: (place: Place) => void
  /** Kills the call. Not "stop showing it" — see `provider::cancel`. */
  onCancel: () => void
  onClose: () => void
}

/**
 * The answer, with every claim carrying the passage it came from.
 *
 * A citation is the point of this panel, not decoration: clicking one opens the
 * note at the passage through the same anchor jump a search result uses. Prose
 * without a citation is prose the vault cannot back up.
 */
export function AnswerPanel({
  question,
  answer,
  error,
  loading,
  onOpenSource,
  onOpenPlace,
  onCancel,
  onClose,
}: Props) {
  const reduceMotion = useReducedMotion()

  return (
    <motion.div
      initial={reduceMotion ? { opacity: 0 } : { opacity: 0, y: 6 }}
      animate={{ opacity: 1, y: 0 }}
      transition={{ duration: reduceMotion ? 0 : 0.14 }}
      className="rounded-lg border bg-card"
    >
      <header className="flex items-start gap-3 border-b px-4 py-3">
        <Sparkles className="mt-0.5 size-4 shrink-0 text-found" aria-hidden />
        <span className="min-w-0 flex-1">
          <span className="block truncate text-sm">{question}</span>
          {answer && (
            <span className="mt-0.5 block truncate font-mono text-xs text-muted-foreground">
              {answer.provider} · buscou por {answer.terms.join(", ")}
            </span>
          )}
        </span>
        <button
          type="button"
          onClick={onClose}
          aria-label="Fechar resposta"
          className="rounded p-1 text-muted-foreground transition-colors hover:text-foreground"
        >
          <X className="size-4" aria-hidden />
        </button>
      </header>

      <div className="px-4 py-3">
        {loading && <ProgressLine label="lendo suas notas…" onCancel={onCancel} />}

        {error && !loading && (
          <p className="text-xs text-muted-foreground">{readable(error)}</p>
        )}

        {answer && !loading && (
          <>
            {/* Asked where something is, the places come before the prose:
                they are the answer, and they came out of the search rather
                than out of a model. */}
            {answer.kind === "locate" && answer.places.length > 0 && (
              <ul className="mb-3 space-y-1">
                {answer.places.map((place) => (
                  <li key={place.path}>
                    <button
                      type="button"
                      onClick={() => onOpenPlace(place)}
                      className="flex w-full items-baseline gap-2 rounded px-1 py-1 text-left transition-colors hover:bg-muted"
                    >
                      <FolderOpen className="size-3.5 shrink-0 text-muted-foreground" aria-hidden />
                      <span className="min-w-0 flex-1 truncate text-sm">{place.title}</span>
                      <span className="shrink-0 font-mono text-xs text-muted-foreground">
                        {place.folder || "raiz"} · {place.passages}
                      </span>
                    </button>
                  </li>
                ))}
              </ul>
            )}

            <p className="whitespace-pre-wrap text-sm leading-relaxed">
              <Cited text={answer.text} sources={answer.sources} onOpen={onOpenSource} />
            </p>

            <ul className="mt-4 space-y-1 border-t pt-3">
              {answer.sources.map((source) => (
                <li key={source.index}>
                  <button
                    type="button"
                    onClick={() => onOpenSource(source)}
                    className="flex w-full items-baseline gap-2 rounded px-1 py-1 text-left transition-colors hover:bg-muted"
                  >
                    <span className="font-mono text-xs text-found">[{source.index}]</span>
                    <span className="min-w-0 flex-1">
                      <span className="block truncate text-sm">{source.title}</span>
                      <span className="block truncate font-mono text-xs text-muted-foreground">
                        {source.headingPath || source.path}
                      </span>
                    </span>
                  </button>
                </li>
              ))}
            </ul>
          </>
        )}
      </div>
    </motion.div>
  )
}

/**
 * Turns every `[n]` in the answer into the passage it points at. A number with
 * no matching source stays plain text — an invented citation must not look
 * clickable.
 */
function Cited({
  text,
  sources,
  onOpen,
}: {
  text: string
  sources: Source[]
  onOpen: (source: Source) => void
}) {
  const parts = text.split(/(\[\d+\])/g)

  return (
    <>
      {parts.map((part, at) => {
        const match = /^\[(\d+)\]$/.exec(part)
        const source = match ? sources.find((one) => one.index === Number(match[1])) : undefined
        if (!source) return <Fragment key={at}>{part}</Fragment>

        return (
          <button
            key={at}
            type="button"
            onClick={() => onOpen(source)}
            title={source.title}
            className={cn(
              "mx-0.5 rounded px-1 font-mono text-xs align-baseline",
              "bg-found/15 text-found transition-colors hover:bg-found/30",
            )}
          >
            {source.index}
          </button>
        )
      })}
    </>
  )
}
