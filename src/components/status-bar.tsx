import type { IndexStatus, VaultStatus } from "../lib/api"

export function StatusBar({
  vault,
  status,
}: {
  vault: VaultStatus | null
  status: IndexStatus | null
}) {
  const counts = status?.counts ?? vault
  const phase = status?.phase ?? "idle"

  return (
    <footer className="flex shrink-0 items-center gap-4 border-t bg-background px-4 py-2 font-mono text-xs text-muted-foreground">
      <span>
        {counts?.notes ?? 0} notas · {counts?.images ?? 0} prints · {counts?.chunks ?? 0} trechos
      </span>

      {phase !== "idle" && (
        <span className="flex min-w-0 items-center gap-2 text-foreground">
          <span className="size-1.5 animate-pulse rounded-full bg-found" aria-hidden />
          <span className="truncate">
            {phase === "scanning" ? "indexando o vault" : "OCR"}
            {status?.current ? ` · ${status.current}` : ""}
            {status?.pending ? ` · ${status.pending} na fila` : ""}
          </span>
        </span>
      )}
    </footer>
  )
}
