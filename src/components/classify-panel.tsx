import { Check, Loader2, Tags, X } from "lucide-react"
import { AnimatePresence, motion, useReducedMotion } from "motion/react"

import type { ClassifyStatus } from "../lib/api"

type Props = {
  open: boolean
  pending: number | null
  status: ClassifyStatus | null
  onCancel: () => void
  onStart: () => void
}

/**
 * Classifying what is already in the vault, on request only.
 *
 * A screenshot that arrives is classified on the way in; everything already
 * here is left alone until this is pressed — the app rewriting a vault nobody
 * asked it to touch is not a feature.
 */
export function ClassifyPanel({ open, pending, status, onCancel, onStart }: Props) {
  const reduceMotion = useReducedMotion()
  const running = status?.phase === "running"
  const done = status?.phase === "done"
  const failed = status?.phase === "failed"

  return (
    <AnimatePresence>
      {open && (
        <motion.div
          initial={{ opacity: 0 }}
          animate={{ opacity: 1 }}
          exit={{ opacity: 0 }}
          transition={{ duration: reduceMotion ? 0 : 0.12 }}
          onClick={() => !running && onCancel()}
          className="fixed inset-0 z-50 flex items-center justify-center bg-background/80 p-6"
        >
          <motion.div
            role="dialog"
            aria-modal
            aria-label="Classificar prints"
            onClick={(event) => event.stopPropagation()}
            initial={reduceMotion ? { opacity: 0 } : { opacity: 0, scale: 0.98 }}
            animate={{ opacity: 1, scale: 1 }}
            exit={reduceMotion ? { opacity: 0 } : { opacity: 0, scale: 0.98 }}
            transition={{ duration: reduceMotion ? 0 : 0.14, ease: "easeOut" }}
            className="w-full max-w-md rounded-lg border bg-card p-5 shadow-xl"
          >
            <h2 className="flex items-center gap-2 font-medium">
              <Tags className="size-4 text-found" aria-hidden />
              Classificar prints
            </h2>

            {!running && !done && !failed && (
              <Invitation pending={pending} onCancel={onCancel} onStart={onStart} />
            )}
            {running && <Running status={status} />}
            {done && <Done status={status} onClose={onCancel} />}
            {failed && <Failed status={status} onClose={onCancel} />}
          </motion.div>
        </motion.div>
      )}
    </AnimatePresence>
  )
}

function Invitation({
  pending,
  onCancel,
  onStart,
}: {
  pending: number | null
  onCancel: () => void
  onStart: () => void
}) {
  if (pending === null) {
    return (
      <p className="mt-3 flex items-center gap-2 text-sm text-muted-foreground">
        <Loader2 className="size-3.5 animate-spin" aria-hidden />
        contando os prints…
      </p>
    )
  }

  if (pending === 0) {
    return (
      <>
        <p className="mt-3 text-sm text-muted-foreground">
          Nenhum print esperando. Os que chegam agora já são classificados na entrada.
        </p>
        <div className="mt-5 flex justify-end">
          <button
            type="button"
            onClick={onCancel}
            className="rounded-md px-3 py-1.5 text-sm text-muted-foreground transition-colors hover:text-foreground"
          >
            Fechar
          </button>
        </div>
      </>
    )
  }

  return (
    <>
      <p className="mt-3 text-sm text-muted-foreground">
        <strong className="text-foreground">{pending} prints</strong> entraram antes de existir
        classificação. Ler o texto que o OCR já guardou e marcar o que for código não refaz o
        reconhecimento — nenhuma imagem é lida de novo.
      </p>
      <p className="mt-2 text-sm text-muted-foreground">
        O que for reconhecido vira tópico automático, separado dos seus.
      </p>

      <div className="mt-5 flex justify-end gap-2">
        <button
          type="button"
          onClick={onCancel}
          className="rounded-md px-3 py-1.5 text-sm text-muted-foreground transition-colors hover:text-foreground"
        >
          Agora não
        </button>
        <button
          type="button"
          onClick={onStart}
          className="flex items-center gap-1.5 rounded-md bg-primary px-3 py-1.5 text-sm text-primary-foreground transition-opacity hover:opacity-90"
        >
          <Tags className="size-4" aria-hidden />
          Classificar {pending}
        </button>
      </div>
    </>
  )
}

function Running({ status }: { status: ClassifyStatus | null }) {
  const total = status?.total ?? 0
  const done = status?.done ?? 0
  const percent = total > 0 ? Math.round((done / total) * 100) : 0

  return (
    <>
      <p className="mt-3 flex items-center gap-2 text-sm">
        <Loader2 className="size-3.5 animate-spin text-muted-foreground" aria-hidden />
        {total > 0 ? `${done} de ${total}` : "começando…"}
      </p>
      <p className="mt-1 truncate font-mono text-xs text-muted-foreground">
        {status?.current || " "}
      </p>

      <div className="mt-3 h-1 overflow-hidden rounded-full bg-muted">
        <div
          className="h-full bg-found transition-[width] duration-200"
          style={{ width: `${percent}%` }}
        />
      </div>

      <p className="mt-3 text-xs text-muted-foreground">
        Pode deixar rodando e continuar usando o app.
      </p>
    </>
  )
}

function Done({ status, onClose }: { status: ClassifyStatus | null; onClose: () => void }) {
  const marked = status?.marked ?? 0
  const total = status?.total ?? 0

  return (
    <>
      <p className="mt-3 flex items-center gap-2 text-sm">
        <Check className="size-4 text-found" aria-hidden />
        {marked} de {total} reconhecidos
      </p>
      {total > marked && (
        <p className="mt-2 text-xs text-muted-foreground">
          Os outros {total - marked} não pareciam código, e ficaram sem marca em vez de receber um
          palpite.
        </p>
      )}

      <div className="mt-5 flex justify-end">
        <button
          type="button"
          onClick={onClose}
          className="rounded-md bg-primary px-3 py-1.5 text-sm text-primary-foreground transition-opacity hover:opacity-90"
        >
          Pronto
        </button>
      </div>
    </>
  )
}

function Failed({ status, onClose }: { status: ClassifyStatus | null; onClose: () => void }) {
  return (
    <>
      <p className="mt-3 flex items-start gap-2 text-sm text-muted-foreground">
        <X className="mt-0.5 size-4 shrink-0 text-destructive" aria-hidden />
        <span>{status?.current || "a classificação não completou"}</span>
      </p>
      <div className="mt-5 flex justify-end">
        <button
          type="button"
          onClick={onClose}
          className="rounded-md px-3 py-1.5 text-sm text-muted-foreground transition-colors hover:text-foreground"
        >
          Fechar
        </button>
      </div>
    </>
  )
}
