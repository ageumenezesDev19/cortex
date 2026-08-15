import { Loader2, Sparkles, X } from "lucide-react"
import { motion, useReducedMotion } from "motion/react"
import { Fragment } from "react"

import type { AiAnswer, Source } from "../lib/api"
import { cn } from "../lib/utils"

type Props = {
  question: string
  answer: AiAnswer | null
  error: string | null
  loading: boolean
  onOpenSource: (source: Source) => void
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
        {loading && (
          <p className="flex items-center gap-2 font-mono text-xs text-muted-foreground">
            <Loader2 className="size-3.5 animate-spin" aria-hidden />
            lendo suas notas…
          </p>
        )}

        {error && !loading && (
          <p className="font-mono text-xs text-muted-foreground">{error}</p>
        )}

        {answer && !loading && (
          <>
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
