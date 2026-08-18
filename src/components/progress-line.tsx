import { Loader2 } from "lucide-react"
import { motion, useReducedMotion } from "motion/react"

type Props = {
  label: string
  /** Given only when the work can really be stopped, never as decoration. */
  onCancel?: () => void
  /** Turns the bar from "something is happening" into "this much of it". */
  value?: { used: number; total: number }
  /** What sits at the right of the label — a count, a countdown. */
  detail?: string
}

/**
 * Something is happening and nobody knows for how long.
 *
 * Indeterminate on purpose: a bar that fakes a percentage is a bar that lies
 * twice, once while it crawls and once when it jumps to the end. The cancel is
 * only here when pressing it stops the work — see `provider::cancel`.
 */
export function ProgressLine({ label, onCancel, value, detail }: Props) {
  const reduceMotion = useReducedMotion()
  const percent = value && value.total > 0 ? Math.min(100, (value.used / value.total) * 100) : null

  return (
    <div>
      <div className="flex items-center gap-2">
        {percent === null && (
          <Loader2 className="size-3.5 shrink-0 animate-spin text-muted-foreground" aria-hidden />
        )}
        <span className="min-w-0 flex-1 truncate font-mono text-xs text-muted-foreground">
          {label}
        </span>
        {detail && (
          <span className="shrink-0 font-mono text-xs text-muted-foreground">{detail}</span>
        )}
        {onCancel && (
          <button
            type="button"
            onClick={onCancel}
            className="shrink-0 rounded px-2 py-1 text-xs text-muted-foreground transition-colors hover:text-destructive"
          >
            cancelar
          </button>
        )}
      </div>

      <div
        role="progressbar"
        aria-label={label}
        aria-valuenow={percent === null ? undefined : Math.round(percent)}
        className="mt-2 h-0.5 overflow-hidden rounded-full bg-muted"
      >
        {percent !== null ? (
          <div
            className="h-full bg-muted-foreground/60 transition-[width] duration-300"
            style={{ width: `${Math.max(percent, 2)}%` }}
          />
        ) : reduceMotion ? (
          <div className="h-full w-1/3 bg-muted-foreground/40" />
        ) : (
          <motion.div
            className="h-full w-1/3 bg-muted-foreground/40"
            animate={{ x: ["-100%", "300%"] }}
            transition={{ duration: 1.4, repeat: Infinity, ease: "easeInOut" }}
          />
        )}
      </div>
    </div>
  )
}
