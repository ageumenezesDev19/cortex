import {
  Check,
  ChevronDown,
  ChevronRight,
  FileText,
  Image as ImageIcon,
  Loader2,
  ShieldCheck,
  X,
} from "lucide-react"
import { AnimatePresence, motion, useReducedMotion } from "motion/react"
import { useCallback, useEffect, useState } from "react"

import {
  type ModelStatus,
  aiStatus,
  cancelEmbedding,
  deleteModel,
  downloadModel,
  embedVault,
  KEY_NAMES,
  listTree,
  onEmbedStatus,
  refreshQuota,
  onModelStatus,
  setAutoEmbed,
  setExcluded,
  setGeminiLimit,
  setKey,
  testKey,
  type AiStatus,
  type EmbedStatus,
  type Provider,
  type TreeEntry,
} from "../lib/api"
import { readable } from "../lib/failures"
import { cn } from "../lib/utils"
import { ProgressLine } from "./progress-line"

type Props = {
  open: boolean
  onClose: () => void
  /** So the rest of the app learns a key arrived without asking again. */
  onChanged: (status: AiStatus) => void
}

/**
 * The two halves of letting a provider read the vault: which key it uses, and
 * how much of the vault it may see. Both live on this machine — the phone can
 * ask questions, it does not get to decide what leaves.
 */
export function SettingsPanel({ open, onClose, onChanged }: Props) {
  const reduceMotion = useReducedMotion()
  const [status, setStatus] = useState<AiStatus | null>(null)
  const [tree, setTree] = useState<TreeEntry[]>([])
  const [error, setError] = useState<string | null>(null)

  useEffect(() => {
    if (!open) return
    setError(null)
    aiStatus()
      .then(setStatus)
      .catch((reason) => setError(String(reason)))
    listTree()
      .then(setTree)
      .catch(() => setTree([]))
  }, [open])

  const apply = useCallback(
    (next: AiStatus) => {
      setStatus(next)
      onChanged(next)
    },
    [onChanged],
  )

  const changeExcluded = useCallback(
    (excluded: string[]) => {
      setError(null)
      setExcluded(excluded)
        .then(apply)
        .catch((reason) => setError(String(reason)))
    },
    [apply],
  )

  return (
    <AnimatePresence>
      {open && (
        <motion.div
          initial={{ opacity: 0 }}
          animate={{ opacity: 1 }}
          exit={{ opacity: 0 }}
          transition={{ duration: reduceMotion ? 0 : 0.12 }}
          onClick={onClose}
          className="fixed inset-0 z-50 flex items-center justify-center bg-background/80 p-6"
        >
          <motion.div
            role="dialog"
            aria-modal
            aria-label="IA e privacidade"
            onClick={(event) => event.stopPropagation()}
            initial={reduceMotion ? { opacity: 0 } : { opacity: 0, scale: 0.98 }}
            animate={{ opacity: 1, scale: 1 }}
            exit={reduceMotion ? { opacity: 0 } : { opacity: 0, scale: 0.98 }}
            transition={{ duration: reduceMotion ? 0 : 0.14, ease: "easeOut" }}
            className="flex max-h-[90vh] w-full max-w-md flex-col rounded-lg border bg-card shadow-xl"
          >
            <header className="shrink-0 border-b p-5 pb-4">
              <h2 className="flex items-center gap-2 font-medium">
                <ShieldCheck className="size-4 text-found" aria-hidden />
                IA e privacidade
              </h2>
              <p className="mt-1 text-sm text-muted-foreground">
                A chave fica nesta máquina e o vault continua aqui. O que sai é só o que você
                deixar sair.
              </p>
            </header>

            <div className="scroll-y p-5">
              {!status && !error && <ProgressLine label="lendo as configurações…" />}

              {/* Three questions, three sections: com que chave, quanto já
                  foi, até onde ela chega. */}
              {status && (
                <>
                  <Keys status={status} onApply={apply} onError={setError} />
                  <Usage status={status} onApply={apply} onError={setError} />
                  <Meaning
                    status={status}
                    onApply={apply}
                    onDone={() => {
                      aiStatus().then(apply).catch(() => {})
                    }}
                  />
                  <Folders
                    tree={tree}
                    excluded={status.excluded}
                    onChange={changeExcluded}
                  />
                </>
              )}

              {error && (
                <p className="mt-4 flex items-start gap-2 rounded-md border border-destructive/40 px-3 py-2 text-xs text-muted-foreground">
                  <X className="mt-0.5 size-3.5 shrink-0 text-destructive" aria-hidden />
                  <span className="font-mono">{error}</span>
                </p>
              )}
            </div>

            <div className="flex shrink-0 justify-end border-t p-3">
              <button
                type="button"
                onClick={onClose}
                className="rounded-md px-3 py-1.5 text-sm text-muted-foreground transition-colors hover:text-foreground"
              >
                Fechar
              </button>
            </div>
          </motion.div>
        </motion.div>
      )}
    </AnimatePresence>
  )
}

function Keys({
  status,
  onApply,
  onError,
}: {
  status: AiStatus
  onApply: (status: AiStatus) => void
  onError: (message: string | null) => void
}) {
  return (
    <section>
      <h3 className="text-sm font-medium">Chaves</h3>
      <p className="mt-1 text-xs text-muted-foreground">
        Groq responde primeiro, e o Gemini entra quando ele esbarra no limite do minuto. Uma só
        já funciona — e nenhuma das duas é usada pela busca por significado, que roda aqui.
        Os limites de cada uma aparecem em <em>Uso de hoje</em>, lidos da resposta do provedor.
      </p>

      <div className="mt-3 space-y-2">
        <KeyRow provider="groq" label="Groq" status={status} onApply={onApply} onError={onError} />
        <KeyRow
          provider="gemini"
          label="Gemini"
          status={status}
          onApply={onApply}
          onError={onError}
        />
      </div>

      <p className="mt-3 text-xs text-muted-foreground">
        Guardada em <code className="font-mono">.server/env</code>, dentro do vault, legível só
        pelo seu usuário. Nunca é enviada para o celular e nunca volta para esta tela — para
        conferir se está certa, use o testar.
      </p>
    </section>
  )
}

function KeyRow({
  provider,
  label,
  status,
  onApply,
  onError,
}: {
  provider: Provider
  label: string
  status: AiStatus
  onApply: (status: AiStatus) => void
  onError: (message: string | null) => void
}) {
  const [value, setValue] = useState("")
  const [busy, setBusy] = useState(false)
  const [result, setResult] = useState<{ ok: boolean; message: string } | null>(null)
  const source = status.keys[provider]

  const run = (work: Promise<unknown>, after: () => void) => {
    setBusy(true)
    setResult(null)
    onError(null)
    work
      .then(after)
      .catch((reason) => onError(String(reason).replace(/^Error:\s*/, "")))
      .finally(() => setBusy(false))
  }

  const save = () => {
    const typed = value.trim()
    if (!typed) return
    run(setKey(KEY_NAMES[provider], typed).then(onApply), () => setValue(""))
  }

  return (
    <div className="rounded-md border p-3">
      <div className="flex items-center gap-2">
        <span className="flex-1 text-sm">{label}</span>
        <span className="font-mono text-xs text-muted-foreground">{describe(source)}</span>
      </div>

      <div className="mt-2 flex gap-2">
        <input
          type="password"
          value={value}
          onChange={(event) => setValue(event.target.value)}
          onKeyDown={(event) => event.key === "Enter" && save()}
          placeholder={source ? "trocar a chave" : `chave do ${label}`}
          aria-label={`Chave do ${label}`}
          autoComplete="off"
          spellCheck={false}
          className="min-w-0 flex-1 rounded-md border bg-background px-2 py-1.5 font-mono text-xs outline-none focus:border-ring"
        />
        <button
          type="button"
          onClick={save}
          disabled={busy || value.trim().length === 0}
          className="rounded-md bg-primary px-3 py-1.5 text-xs text-primary-foreground transition-opacity hover:opacity-90 disabled:opacity-40"
        >
          Salvar
        </button>
      </div>

      {source && (
        <div className="mt-2 flex items-center gap-3">
          <button
            type="button"
            disabled={busy}
            onClick={() =>
              run(
                testKey(provider).then(
                  () => setResult({ ok: true, message: "respondeu" }),
                  (reason) =>
                    setResult({ ok: false, message: String(reason).replace(/^Error:\s*/, "") }),
                ),
                () => {},
              )
            }
            className="flex items-center gap-1.5 text-xs text-muted-foreground transition-colors hover:text-foreground disabled:opacity-60"
          >
            {busy && <Loader2 className="size-3 animate-spin" aria-hidden />}
            testar
          </button>
          {source === "app" && (
            <button
              type="button"
              disabled={busy}
              onClick={() => run(setKey(KEY_NAMES[provider], null).then(onApply), () => {})}
              className="text-xs text-muted-foreground transition-colors hover:text-destructive disabled:opacity-60"
            >
              remover
            </button>
          )}
        </div>
      )}

      {result && (
        <p
          className={cn(
            "mt-2 flex items-start gap-1.5 text-xs",
            result.ok ? "text-found" : "text-muted-foreground",
          )}
        >
          {result.ok ? (
            <Check className="mt-0.5 size-3 shrink-0" aria-hidden />
          ) : (
            <X className="mt-0.5 size-3 shrink-0 text-destructive" aria-hidden />
          )}
          <span className="min-w-0 break-words">{result.message}</span>
        </p>
      )}

      {/* `key()` reads the environment first, so a variable exported in the
          shell answers even with a key saved here. */}
      {source === "environment" && (
        <p className="mt-2 text-xs text-muted-foreground">
          Esta vem do ambiente do terminal que abriu o app. Uma chave salva aqui só passa a valer
          quando essa variável sair.
        </p>
      )}
    </div>
  )
}

const describe = (source: AiStatus["keys"][Provider]) => {
  if (source === "environment") return "do ambiente"
  if (source === "app") return "salva no app"
  return "sem chave"
}

/**
 * How much of the day is gone, from whoever knows it.
 *
 * The two halves are not the same kind of number and the section says so: Groq
 * states its own totals in every reply, Gemini states none, so its bar is this
 * app counting against a limit that had to be written down.
 */
function Usage({
  status,
  onApply,
  onError,
}: {
  status: AiStatus
  onApply: (status: AiStatus) => void
  onError: (message: string | null) => void
}) {
  const [busy, setBusy] = useState(false)
  const [editing, setEditing] = useState(false)
  const { quota } = status
  const groqUsed =
    quota.groq.limitRequests !== null && quota.groq.remainingRequests !== null
      ? quota.groq.limitRequests - quota.groq.remainingRequests
      : null

  return (
    <section className="mt-6">
      <div className="flex items-baseline gap-2">
        <h3 className="flex-1 text-sm font-medium">Uso de hoje</h3>
        <button
          type="button"
          disabled={busy || status.keys.groq === null}
          onClick={() => {
            setBusy(true)
            onError(null)
            refreshQuota()
              .then(onApply)
              .catch((reason) => onError(String(reason).replace(/^Error:\s*/, "")))
              .finally(() => setBusy(false))
          }}
          title="Gasta um pedido do Groq para ouvir os números dele"
          className="text-xs text-muted-foreground transition-colors hover:text-foreground disabled:opacity-40"
        >
          atualizar
        </button>
      </div>

      <div className="mt-3 space-y-3 rounded-md border p-3">
        {groqUsed === null ? (
          <p className="text-xs text-muted-foreground">
            Groq — {quota.groqCalls} chamadas hoje. Os números dele chegam junto da primeira
            resposta do dia, ou pelo atualizar aí em cima (custa um pedido).
          </p>
        ) : (
          <>
            <ProgressLine
              label="Groq · pedidos"
              value={{ used: groqUsed, total: quota.groq.limitRequests ?? 0 }}
              detail={`${groqUsed} de ${quota.groq.limitRequests}${
                quota.groq.resetRequests ? ` · renova em ${quota.groq.resetRequests}` : ""
              }`}
            />
            {quota.groq.limitTokens !== null && quota.groq.remainingTokens !== null && (
              <ProgressLine
                label="Groq · tokens"
                value={{
                  used: quota.groq.limitTokens - quota.groq.remainingTokens,
                  total: quota.groq.limitTokens,
                }}
                detail={`${quota.groq.limitTokens - quota.groq.remainingTokens} de ${
                  quota.groq.limitTokens
                }${quota.groq.resetTokens ? ` · renova em ${quota.groq.resetTokens}` : ""}`}
              />
            )}
          </>
        )}

        <ProgressLine
          label="Gemini · pedidos"
          value={{ used: quota.geminiCalls, total: quota.geminiDailyLimit }}
          detail={`${quota.geminiCalls} de ${quota.geminiDailyLimit} · zera em ${countdown(
            quota.geminiResetsIn,
          )}`}
        />
      </div>

      {/* The one number in this panel that nobody can verify from a response,
          so it says where it came from and can be corrected. */}
      <p className="mt-2 text-xs text-muted-foreground">
        O Gemini não informa cota nenhuma, então essa barra é a contagem deste app contra o
        limite do free tier.{" "}
        {editing ? (
          <input
            type="number"
            min={1}
            defaultValue={quota.geminiDailyLimit}
            autoFocus
            onBlur={(event) => {
              const limit = Number(event.target.value)
              setEditing(false)
              if (limit > 0 && limit !== quota.geminiDailyLimit) {
                setGeminiLimit(limit)
                  .then(onApply)
                  .catch((reason) => onError(String(reason)))
              }
            }}
            className="w-16 rounded border bg-background px-1 font-mono outline-none focus:border-ring"
          />
        ) : (
          <button
            type="button"
            onClick={() => setEditing(true)}
            className="underline underline-offset-2 transition-colors hover:text-foreground"
          >
            mudar o limite
          </button>
        )}
      </p>
    </section>
  )
}

/**
 * Searching by meaning, which runs on this machine. Optional by nature: with
 * nothing embedded the app searches by words, which is what it always did.
 *
 * The weights are 465 MB, so downloading them is a button and never a side
 * effect — see `embed::wanted`, which will not let anything else start one.
 */
function Meaning({
  status,
  onApply,
  onDone,
}: {
  status: AiStatus
  onApply: (status: AiStatus) => void
  onDone: () => void
}) {
  const [progress, setProgress] = useState<EmbedStatus | null>(null)
  const [download, setDownload] = useState<ModelStatus | null>(null)
  const [confirmingRemoval, setConfirmingRemoval] = useState(false)

  useEffect(() => onEmbedStatus(setProgress), [])
  useEffect(() => onModelStatus(setDownload), [])
  useEffect(() => {
    if (progress?.phase === "done") onDone()
  }, [progress?.phase, onDone])
  useEffect(() => {
    // The status carries `modelInstalled`, so a finished download has to be
    // read back rather than assumed from the event.
    if (download?.phase === "ready") aiStatus().then(onApply).catch(() => {})
  }, [download?.phase, onApply])

  const { chunks, embedded } = status.coverage
  const missing = Math.max(0, chunks - embedded)
  const running = progress?.phase === "running"
  const downloading = download?.phase === "downloading" && !status.modelInstalled

  return (
    <section className="mt-6">
      <div className="flex items-baseline gap-2">
        <h3 className="flex-1 text-sm font-medium">Busca por significado</h3>
        {status.modelInstalled && (
          <button
            type="button"
            role="switch"
            aria-checked={status.autoEmbed}
            aria-label="Embutir automaticamente"
            onClick={() => {
              setAutoEmbed(!status.autoEmbed)
                .then(onApply)
                .catch(() => {})
            }}
            className="text-xs text-muted-foreground transition-colors hover:text-foreground"
          >
            {status.autoEmbed ? "automático · ligado" : "automático · desligado"}
          </button>
        )}
      </div>
      <p className="mt-1 text-xs text-muted-foreground">
        Com os trechos embutidos, perguntar deixa de depender da palavra exata: *anotações do
        blender* acha a nota do Blender porque ela fala do assunto. O app cuida disso junto com o
        índice, como já faz com o OCR dos prints — e o modelo roda aqui, sem chave e sem enviar
        nada para fora.
      </p>

      <div className="mt-3 rounded-md border p-3">
        {!status.modelInstalled ? (
          downloading ? (
            <ProgressLine
              label="baixando o modelo…"
              value={{ used: download.done, total: Math.max(download.total, 1) }}
              detail={`${megabytes(download.done)} de ${megabytes(download.total)}`}
            />
          ) : (
            <div className="flex items-center gap-3">
              <span className="flex-1 text-xs text-muted-foreground">
                Precisa do modelo, que ocupa 465 MB e é baixado uma vez. Depois disso funciona
                offline, e nenhum trecho do vault sai desta máquina.
              </span>
              <button
                type="button"
                onClick={() => {
                  downloadModel().catch(() => {})
                }}
                className="shrink-0 rounded-md border px-3 py-1.5 text-xs text-muted-foreground transition-colors hover:text-foreground"
              >
                baixar modelo
              </button>
            </div>
          )
        ) : running ? (
          <ProgressLine
            label="embutindo os trechos…"
            value={{ used: progress.done, total: Math.max(progress.total, 1) }}
            detail={`${progress.done} de ${progress.total}`}
            onCancel={() => {
              cancelEmbedding().catch(() => {})
            }}
          />
        ) : (
          <>
            <ProgressLine
              label="trechos com vetor"
              value={{ used: embedded, total: Math.max(chunks, 1) }}
              detail={`${embedded} de ${chunks}`}
            />
            <div className="mt-3 flex items-center gap-3">
              <span className="flex-1 text-xs text-muted-foreground">
                {missing === 0
                  ? "tudo em dia"
                  : status.autoEmbed
                    ? `${missing} na fila`
                    : `${missing} esperando`}
              </span>
              {/* Only when nobody is doing it automatically, or when a pass
                  stopped and waiting for the next change is too long. */}
              {missing > 0 && (
                <button
                  type="button"
                  onClick={() => {
                    embedVault().catch(() => {})
                  }}
                  className="rounded-md border px-3 py-1.5 text-xs text-muted-foreground transition-colors hover:text-foreground"
                >
                  embutir agora
                </button>
              )}
            </div>

            <div className="mt-3 flex items-center gap-3 border-t pt-3">
              {confirmingRemoval ? (
                <>
                  <span className="flex-1 text-xs text-muted-foreground">
                    Apaga os 465 MB do modelo. Os {embedded} trechos já embutidos continuam
                    valendo e seguem sendo encontrados — só passagem nova fica esperando o
                    modelo voltar.
                  </span>
                  <button
                    type="button"
                    onClick={() => setConfirmingRemoval(false)}
                    className="shrink-0 rounded-md border px-3 py-1.5 text-xs text-muted-foreground transition-colors hover:text-foreground"
                  >
                    cancelar
                  </button>
                  <button
                    type="button"
                    onClick={() => {
                      setConfirmingRemoval(false)
                      setDownload(null)
                      deleteModel().then(onApply).catch(() => {})
                    }}
                    className="shrink-0 rounded-md border border-destructive/50 px-3 py-1.5 text-xs text-destructive transition-colors hover:bg-destructive/10"
                  >
                    remover mesmo assim
                  </button>
                </>
              ) : (
                <>
                  <span className="flex-1 text-xs text-muted-foreground">
                    modelo instalado · 465 MB
                  </span>
                  <button
                    type="button"
                    onClick={() => setConfirmingRemoval(true)}
                    className="shrink-0 rounded-md border border-destructive/50 px-3 py-1.5 text-xs text-destructive transition-colors hover:bg-destructive/10"
                  >
                    remover
                  </button>
                </>
              )}
            </div>
          </>
        )}

        {progress?.phase === "failed" && progress.error && (
          <p className="mt-2 text-xs text-muted-foreground">{readable(progress.error)}</p>
        )}
        {download?.phase === "failed" && (
          <p className="mt-2 text-xs text-muted-foreground">
            O download não terminou. Costuma ser a rede — tentar de novo continua de onde parou.
          </p>
        )}
      </div>
    </section>
  )
}

/** Bytes are unreadable at this size; the share is what the bar is for. */
function megabytes(bytes: number) {
  return `${Math.round(bytes / (1024 * 1024))} MB`
}

/** `6h14` reads faster than a wall-clock time when the question is "how long". */
function countdown(seconds: number) {
  if (seconds <= 0) return "instantes"
  const hours = Math.floor(seconds / 3600)
  const minutes = Math.floor((seconds % 3600) / 60)
  return hours > 0 ? `${hours}h${String(minutes).padStart(2, "0")}` : `${minutes}min`
}

function Folders({
  tree,
  excluded,
  onChange,
}: {
  tree: TreeEntry[]
  excluded: string[]
  onChange: (excluded: string[]) => void
}) {
  const allow = (path: string) => onChange(excluded.filter((one) => one !== path))

  // Unchecking a folder covers everything under it, so the entries it swallows
  // leave the list — otherwise checking it back would half-uncover the folder.
  const block = (path: string) =>
    onChange([...excluded.filter((one) => !under(one, path)), path])

  return (
    <section className="mt-6">
      <div className="flex items-baseline gap-2">
        <h3 className="flex-1 text-sm font-medium">O que a IA pode ler</h3>
        <button
          type="button"
          onClick={() => onChange([])}
          className="text-xs text-muted-foreground transition-colors hover:text-foreground"
        >
          marcar tudo
        </button>
        <button
          type="button"
          onClick={() => onChange(tree.map((entry) => entry.path))}
          className="text-xs text-muted-foreground transition-colors hover:text-foreground"
        >
          desmarcar tudo
        </button>
      </div>

      <p className="mt-1 text-xs text-muted-foreground">
        Desmarcar uma pasta tira ela e tudo que está dentro. Sua pergunta sempre chega ao
        provedor — o que isto controla são os trechos que o Cortex anexa a ela.
      </p>

      <div className="mt-3 rounded-md border p-1">
        {tree.length === 0 ? (
          <p className="px-2 py-4 text-center text-xs text-muted-foreground">vault vazio</p>
        ) : (
          tree.map((entry) => (
            <Row
              key={entry.path}
              entry={entry}
              depth={0}
              excluded={excluded}
              onAllow={allow}
              onBlock={block}
            />
          ))
        )}
      </div>

    </section>
  )
}

function Row({
  entry,
  depth,
  excluded,
  onAllow,
  onBlock,
}: {
  entry: TreeEntry
  depth: number
  excluded: string[]
  onAllow: (path: string) => void
  onBlock: (path: string) => void
}) {
  // Opens itself when something inside is out. A folder that hides an unchecked
  // box makes the tree lie about what the AI reads.
  const [expanded, setExpanded] = useState(() =>
    excluded.some((one) => one !== entry.path && under(one, entry.path)),
  )
  const isFolder = entry.kind === "folder"
  const blocker = excluded.find((prefix) => under(entry.path, prefix)) ?? null
  const inherited = blocker !== null && blocker !== entry.path
  const allowed = blocker === null
  const Icon = entry.kind === "image" ? ImageIcon : FileText

  return (
    <div>
      <div className="flex items-center gap-1" style={{ paddingLeft: `${depth * 12}px` }}>
        <button
          type="button"
          role="checkbox"
          aria-checked={allowed}
          // A folder inside a folder that is already out has nothing to decide.
          disabled={inherited}
          onClick={() => (allowed ? onBlock(entry.path) : onAllow(entry.path))}
          className="flex min-w-0 flex-1 items-center gap-2 rounded px-1 py-1.5 text-left text-sm transition-colors hover:bg-muted disabled:opacity-40 disabled:hover:bg-transparent"
        >
          <span
            className={cn(
              "flex size-4 shrink-0 items-center justify-center rounded border",
              allowed ? "border-found bg-found text-found-foreground" : "border-muted-foreground/40",
            )}
            aria-hidden
          >
            {allowed && <Check className="size-3" />}
          </span>
          {!isFolder && <Icon className="size-3.5 shrink-0 text-muted-foreground" aria-hidden />}
          <span className="min-w-0 truncate">{entry.name}</span>
        </button>

        {isFolder && entry.children.length > 0 && (
          <button
            type="button"
            onClick={() => setExpanded((value) => !value)}
            aria-expanded={expanded}
            aria-label={`${expanded ? "Fechar" : "Abrir"} ${entry.name}`}
            className="rounded p-1 text-muted-foreground transition-colors hover:text-foreground"
          >
            {expanded ? (
              <ChevronDown className="size-3.5" aria-hidden />
            ) : (
              <ChevronRight className="size-3.5" aria-hidden />
            )}
          </button>
        )}
      </div>

      {isFolder &&
        expanded &&
        entry.children.map((child) => (
          <Row
            key={child.path}
            entry={child}
            depth={depth + 1}
            excluded={excluded}
            onAllow={onAllow}
            onBlock={onBlock}
          />
        ))}
    </div>
  )
}

/** The same prefix rule the back end applies when it gathers the passages. */
const under = (path: string, prefix: string) =>
  path === prefix || path.startsWith(`${prefix}/`)
