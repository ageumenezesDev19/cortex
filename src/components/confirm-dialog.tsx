import { AnimatePresence, motion, useReducedMotion } from "motion/react"
import { useEffect, useRef } from "react"

type Props = {
  open: boolean
  title: string
  /** What actually happens, in the person's terms — not a restatement of the title. */
  detail: string
  confirmLabel: string
  onConfirm: () => void
  onCancel: () => void
}

/**
 * Every irreversible action goes through here. Written in the app's own visual
 * language rather than the system dialog, because the app has to stand on its
 * own — and because a native sheet cannot say what this is about.
 */
export function ConfirmDialog({
  open,
  title,
  detail,
  confirmLabel,
  onConfirm,
  onCancel,
}: Props) {
  const reduceMotion = useReducedMotion()
  const confirm = useRef<HTMLButtonElement>(null)

  // Focus lands on the destructive button but Escape is always one key away,
  // so confirming is deliberate and backing out is free.
  useEffect(() => {
    if (open) confirm.current?.focus()
  }, [open])

  return (
    <AnimatePresence>
      {open && (
        <motion.div
          initial={{ opacity: 0 }}
          animate={{ opacity: 1 }}
          exit={{ opacity: 0 }}
          transition={{ duration: reduceMotion ? 0 : 0.12 }}
          onKeyDown={(event) => event.key === "Escape" && onCancel()}
          className="fixed inset-0 z-50 flex items-center justify-center bg-background/80 p-6"
        >
          <motion.div
            role="alertdialog"
            aria-modal
            aria-label={title}
            initial={reduceMotion ? { opacity: 0 } : { opacity: 0, scale: 0.98 }}
            animate={{ opacity: 1, scale: 1 }}
            exit={reduceMotion ? { opacity: 0 } : { opacity: 0, scale: 0.98 }}
            transition={{ duration: reduceMotion ? 0 : 0.14, ease: "easeOut" }}
            className="w-full max-w-sm rounded-lg border bg-card p-5 shadow-xl"
          >
            <h2 className="font-medium">{title}</h2>
            <p className="mt-2 text-sm text-muted-foreground">{detail}</p>

            <div className="mt-5 flex justify-end gap-2">
              <button
                type="button"
                onClick={onCancel}
                className="rounded-md px-3 py-1.5 text-sm text-muted-foreground transition-colors hover:text-foreground"
              >
                Cancelar
              </button>
              <button
                ref={confirm}
                type="button"
                onClick={onConfirm}
                className="rounded-md bg-destructive px-3 py-1.5 text-sm text-destructive-foreground transition-opacity hover:opacity-90"
              >
                {confirmLabel}
              </button>
            </div>
          </motion.div>
        </motion.div>
      )}
    </AnimatePresence>
  )
}