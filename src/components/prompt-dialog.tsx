import { AnimatePresence, motion, useReducedMotion } from "motion/react"
import { useEffect, useState } from "react"

type Props = {
  open: boolean
  title: string
  placeholder: string
  initialValue?: string
  confirmLabel: string
  onConfirm: (value: string) => void
  onCancel: () => void
}

/** Naming a note or a folder, without leaving the app to do it. */
export function PromptDialog({
  open,
  title,
  placeholder,
  initialValue = "",
  confirmLabel,
  onConfirm,
  onCancel,
}: Props) {
  const reduceMotion = useReducedMotion()
  const [value, setValue] = useState(initialValue)

  useEffect(() => {
    if (open) setValue(initialValue)
  }, [open, initialValue])

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
            aria-label={title}
            initial={reduceMotion ? { opacity: 0 } : { opacity: 0, scale: 0.98 }}
            animate={{ opacity: 1, scale: 1 }}
            exit={reduceMotion ? { opacity: 0 } : { opacity: 0, scale: 0.98 }}
            transition={{ duration: reduceMotion ? 0 : 0.14, ease: "easeOut" }}
            onSubmit={(event) => {
              event.preventDefault()
              if (value.trim()) onConfirm(value.trim())
            }}
            onKeyDown={(event) => event.key === "Escape" && onCancel()}
            className="w-full max-w-sm rounded-lg border bg-card p-5 shadow-xl"
          >
            <h2 className="font-medium">{title}</h2>

            <input
              value={value}
              onChange={(event) => setValue(event.target.value)}
              placeholder={placeholder}
              spellCheck={false}
              autoFocus
              aria-label={title}
              className="mt-3 h-9 w-full rounded-md border bg-background px-2 text-sm outline-none focus:border-ring"
            />

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
                disabled={value.trim().length === 0}
                className="rounded-md bg-primary px-3 py-1.5 text-sm text-primary-foreground transition-opacity hover:opacity-90 disabled:opacity-40"
              >
                {confirmLabel}
              </button>
            </div>
          </motion.form>
        </motion.div>
      )}
    </AnimatePresence>
  )
}