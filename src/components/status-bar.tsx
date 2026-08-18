import { isDemo, type IndexStatus, type VaultStatus } from "../lib/api"
import { useLanguage } from "./language-provider"
import { usePick } from "./language-provider"

const content = {
  en: {
    counts: (notes: number, images: number, chunks: number) =>
      `${notes} notes · ${images} screenshots · ${chunks} passages`,
    scanning: "indexing the vault",
    embedding: "embedding",
    queued: (n: number) => ` · ${n} queued`,
    restore: "start over",
    confirmRestore: "Delete the notes you wrote here and put the original vault back?",
  },
  pt: {
    counts: (notes: number, images: number, chunks: number) =>
      `${notes} notas · ${images} prints · ${chunks} trechos`,
    scanning: "indexando o vault",
    embedding: "embutindo",
    queued: (n: number) => ` · ${n} na fila`,
    restore: "recomeçar",
    confirmRestore: "Apagar as notas que você escreveu aqui e devolver o vault original?",
  },
}

export function StatusBar({
  vault,
  status,
}: {
  vault: VaultStatus | null
  status: IndexStatus | null
}) {
  const t = usePick()(content)
  const { language } = useLanguage()
  const counts = status?.counts ?? vault
  const phase = status?.phase ?? "idle"

  return (
    <footer className="flex shrink-0 items-center gap-4 border-t bg-background px-4 py-2 font-mono text-xs text-muted-foreground">
      <span>{t.counts(counts?.notes ?? 0, counts?.images ?? 0, counts?.chunks ?? 0)}</span>

      {/* Only the demo can be put back: the desktop vault is real files. */}
      {isDemo && (
        <button
          type="button"
          onClick={() => {
            if (!window.confirm(t.confirmRestore)) return
            void import("../demo/shim").then(async ({ restore }) => {
              await restore(language)
              window.location.reload()
            })
          }}
          className="ml-auto transition-colors hover:text-foreground"
        >
          {t.restore}
        </button>
      )}

      {phase !== "idle" && (
        <span className="flex min-w-0 items-center gap-2 text-foreground">
          <span className="size-1.5 animate-pulse rounded-full bg-found" aria-hidden />
          <span className="truncate">
            {phase === "scanning" ? t.scanning : phase === "embedding" ? t.embedding : "OCR"}
            {status?.current ? ` · ${status.current}` : ""}
            {status?.pending ? t.queued(status.pending) : ""}
          </span>
        </span>
      )}
    </footer>
  )
}
