import { Check, Download, Loader2, TriangleAlert, X } from "lucide-react"
import { AnimatePresence, motion, useReducedMotion } from "motion/react"

import type { ImportPreview, ImportStatus } from "../lib/api"

type Props = {
  open: boolean
  preview: ImportPreview | null
  status: ImportStatus | null
  onCancel: () => void
  onStart: () => void
}

/**
 * The one operation in the app that reaches outside the vault and takes minutes.
 * It says what it will do before doing it, and shows real progress while it runs
 * — 232 notes with no feedback would be indistinguishable from a hang.
 */
export function ImportPanel({ open, preview, status, onCancel, onStart }: Props) {
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
          onKeyDown={(event) => event.key === "Escape" && !running && onCancel()}
          className="fixed inset-0 z-50 flex items-center justify-center bg-background/80 p-6"
        >
          <motion.div
            role="dialog"
            aria-modal
            aria-label="Importar do Notes"
            initial={reduceMotion ? { opacity: 0 } : { opacity: 0, scale: 0.98 }}
            animate={{ opacity: 1, scale: 1 }}
            exit={reduceMotion ? { opacity: 0 } : { opacity: 0, scale: 0.98 }}
            transition={{ duration: reduceMotion ? 0 : 0.14, ease: "easeOut" }}
            className="w-full max-w-md rounded-lg border bg-card p-5 shadow-xl"
          >
            <h2 className="font-medium">Importar do Notes</h2>

            {!running && !done && !failed && (
              <Invitation preview={preview} onCancel={onCancel} onStart={onStart} />
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
  preview,
  onCancel,
  onStart,
}: {
  preview: ImportPreview | null
  onCancel: () => void
  onStart: () => void
}) {
  if (!preview) {
    return (
      <p className="mt-3 flex items-center gap-2 text-sm text-muted-foreground">
        <Loader2 className="size-3.5 animate-spin" aria-hidden />
        Perguntando ao Notes o que existe lá…
      </p>
    )
  }

  const pending = preview.notes - preview.already

  return (
    <>
      <p className="mt-3 text-sm text-muted-foreground">
        O Notes tem <strong className="text-foreground">{preview.notes} notas</strong> em{" "}
        {preview.folders} pastas. Elas entram como markdown em{" "}
        <code className="font-mono text-xs">_importado/</code>, mantendo as pastas.
      </p>

      {preview.already > 0 && (
        <p className="mt-2 text-sm text-muted-foreground">
          {preview.already} já vieram antes e serão puladas — {pending} a trazer.
        </p>
      )}

      <p className="mt-3 flex items-start gap-2 rounded-md border border-found/40 bg-found/10 px-3 py-2 text-xs text-muted-foreground">
        <TriangleAlert className="mt-0.5 size-3.5 shrink-0 text-found" aria-hidden />
        <span>
          Só o texto vem nesta passada. Onde havia imagem fica uma marca, e o caminho de volta
          fica guardado — nada é apagado do Notes.
        </span>
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
          <Download className="size-4" aria-hidden />
          Trazer {pending} notas
        </button>
      </div>
    </>
  )
}

function Running({ status }: { status: ImportStatus | null }) {
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
        {status?.current || " "}
      </p>

      <div className="mt-3 h-1 overflow-hidden rounded-full bg-muted">
        <div
          className="h-full bg-found transition-[width] duration-200"
          style={{ width: `${percent}%` }}
        />
      </div>

      <p className="mt-3 text-xs text-muted-foreground">
        Pode deixar rodando e continuar usando o app — as notas aparecem na busca conforme chegam.
      </p>
    </>
  )
}

function Done({ status, onClose }: { status: ImportStatus | null; onClose: () => void }) {
  const report = status?.report

  return (
    <>
      <p className="mt-3 flex items-center gap-2 text-sm">
        <Check className="size-4 text-found" aria-hidden />
        {report?.imported ?? 0} notas trazidas
      </p>

      <ul className="mt-3 space-y-1 font-mono text-xs text-muted-foreground">
        {(report?.skipped ?? 0) > 0 && <li>{report?.skipped} já estavam aqui, puladas</li>}
        {(report?.attachments ?? 0) > 0 && (
          <li>{report?.attachments} imagens marcadas, ainda no Notes</li>
        )}
        {(report?.failed ?? 0) > 0 && <li className="text-found">{report?.failed} falharam</li>}
      </ul>

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

function Failed({ status, onClose }: { status: ImportStatus | null; onClose: () => void }) {
  return (
    <>
      <p className="mt-3 flex items-start gap-2 text-sm text-muted-foreground">
        <X className="mt-0.5 size-4 shrink-0 text-destructive" aria-hidden />
        <span>{status?.error ?? "o import não completou"}</span>
      </p>
      <p className="mt-2 text-xs text-muted-foreground">
        Se o macOS pediu permissão para o Cortex controlar o Notes, autorize e tente de novo.
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