import { Check, Copy, Loader2, TriangleAlert, X } from "lucide-react"
import { AnimatePresence, motion, useReducedMotion } from "motion/react"
import QRCode from "qrcode"
import { useCallback, useEffect, useState } from "react"

import {
  serverState,
  setServer,
  testConnection,
  type ServerLink,
  type ServerState,
  type TestStep,
} from "../lib/api"
import { cn } from "../lib/utils"

/**
 * Where the vault is opened up to the phone, and where it is closed again.
 *
 * Everything here used to need a terminal, which an installed app does not
 * have — so the switches live next to the address each one lights up.
 */
export function SharePanel({ open, onClose }: { open: boolean; onClose: () => void }) {
  const reduceMotion = useReducedMotion()
  const [state, setState] = useState<ServerState | null>(null)
  const [busy, setBusy] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const [picked, setPicked] = useState(0)
  const [steps, setSteps] = useState<TestStep[] | null>(null)
  const [testing, setTesting] = useState(false)

  useEffect(() => {
    if (!open) return
    setState(null)
    setSteps(null)
    setError(null)
    serverState()
      .then(setState)
      .catch((reason) => setError(String(reason)))
  }, [open])

  const change = useCallback(
    (patch: Partial<ServerState["settings"]>) => {
      if (!state) return
      setBusy(true)
      setError(null)
      setSteps(null)
      const next = { ...state.settings, ...patch }
      setServer(next.serving, next.localNetwork, next.tailscale)
        .then(setState)
        .catch((reason) => setError(String(reason)))
        .finally(() => setBusy(false))
    },
    [state],
  )

  const link = state?.links[picked] ?? null

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
            aria-label="Abrir no celular"
            onClick={(event) => event.stopPropagation()}
            initial={reduceMotion ? { opacity: 0 } : { opacity: 0, scale: 0.98 }}
            animate={{ opacity: 1, scale: 1 }}
            exit={reduceMotion ? { opacity: 0 } : { opacity: 0, scale: 0.98 }}
            transition={{ duration: reduceMotion ? 0 : 0.14, ease: "easeOut" }}
            className="max-h-[90vh] w-full max-w-sm overflow-y-auto rounded-lg border bg-card p-5 shadow-xl"
          >
            <h2 className="font-medium">Abrir no celular</h2>
            <p className="mt-1 text-sm text-muted-foreground">
              O vault continua só aqui — o telefone é uma janela.
            </p>

            {!state && !error && (
              <p className="mt-4 flex items-center gap-2 font-mono text-xs text-muted-foreground">
                <Loader2 className="size-3.5 animate-spin" aria-hidden />
                lendo o estado…
              </p>
            )}

            {state && (
              <>
                <div className="mt-4 space-y-1">
                  <Switch
                    label="Abrir para o celular"
                    hint="com isto desligado, a porta fica fechada"
                    on={state.settings.serving}
                    busy={busy}
                    onToggle={() => change({ serving: !state.settings.serving })}
                  />
                  <Switch
                    label="Tailscale (HTTPS)"
                    hint={tailscaleHint(state)}
                    on={state.settings.tailscale}
                    busy={busy}
                    disabled={!state.settings.serving || !state.tailscale.connected}
                    onToggle={() => change({ tailscale: !state.settings.tailscale })}
                  />
                  <Switch
                    label="Rede local (HTTP)"
                    hint="qualquer aparelho no mesmo Wi-Fi alcança"
                    on={state.settings.localNetwork}
                    busy={busy}
                    disabled={!state.settings.serving}
                    onToggle={() => change({ localNetwork: !state.settings.localNetwork })}
                  />
                </div>

                {state.links.length > 1 && (
                  <div className="mt-4 flex gap-1 rounded-md bg-muted p-1">
                    {state.links.map((one, at) => (
                      <button
                        key={one.label}
                        type="button"
                        onClick={() => {
                          setPicked(at)
                          setSteps(null)
                        }}
                        className={cn(
                          "flex-1 rounded px-2 py-1 text-xs transition-colors",
                          at === picked
                            ? "bg-card text-foreground shadow-sm"
                            : "text-muted-foreground hover:text-foreground",
                        )}
                      >
                        {one.label}
                      </button>
                    ))}
                  </div>
                )}

                {link ? (
                  <Address
                    link={link}
                    onTest={() => {
                      setTesting(true)
                      setSteps(null)
                      testConnection(link.label === "Rede local")
                        .then(setSteps)
                        .finally(() => setTesting(false))
                    }}
                    testing={testing}
                    steps={steps}
                  />
                ) : (
                  <p className="mt-4 text-sm text-muted-foreground">
                    Nenhum endereço disponível. Sem Tailscale conectado e sem rede, só esta máquina
                    alcança o vault.
                  </p>
                )}
              </>
            )}

            {error && (
              <p className="mt-3 flex items-start gap-2 rounded-md border border-destructive/40 px-3 py-2 text-xs text-muted-foreground">
                <X className="mt-0.5 size-3.5 shrink-0 text-destructive" aria-hidden />
                <span className="font-mono">{error}</span>
              </p>
            )}

            <div className="mt-5 flex justify-end">
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

/** The four states Tailscale can be in, each said out loud rather than hidden. */
function tailscaleHint(state: ServerState) {
  if (!state.tailscale.installed) return "Tailscale não encontrado — instale para alcançar de fora"
  if (!state.tailscale.connected) return `o daemon está em ${state.tailscale.state}; rode tailscale up`
  return state.tailscale.host ?? "conectado"
}

function Address({
  link,
  onTest,
  testing,
  steps,
}: {
  link: ServerLink
  onTest: () => void
  testing: boolean
  steps: TestStep[] | null
}) {
  const [image, setImage] = useState("")
  const [copied, setCopied] = useState(false)

  useEffect(() => {
    if (!link.live) return setImage("")
    // Rendered here: the address is a session for anyone who sees it.
    QRCode.toDataURL(link.url, { margin: 1, width: 320 })
      .then(setImage)
      .catch(() => setImage(""))
  }, [link])

  if (!link.live) {
    return (
      <p className="mt-4 rounded-md border border-dashed px-3 py-4 text-center text-sm text-muted-foreground">
        {link.label} está desligado. Ligue o interruptor acima para ver o código.
      </p>
    )
  }

  return (
    <>
      {image && (
        <img
          src={image}
          alt={`QR code para ${link.label}`}
          // White behind it in both themes: a dark QR on a dark card is a QR no
          // camera reads.
          className="mx-auto mt-4 w-52 rounded-md bg-white p-2"
        />
      )}

      <div className="mt-3 flex gap-2">
        <button
          type="button"
          onClick={() => {
            void navigator.clipboard.writeText(link.url)
            setCopied(true)
            setTimeout(() => setCopied(false), 1500)
          }}
          className="flex flex-1 items-center justify-center gap-1.5 rounded-md border px-3 py-1.5 text-xs text-muted-foreground transition-colors hover:text-foreground"
        >
          {copied ? (
            <Check className="size-3.5 text-found" aria-hidden />
          ) : (
            <Copy className="size-3.5" aria-hidden />
          )}
          {copied ? "copiado" : "copiar"}
        </button>
        <button
          type="button"
          onClick={onTest}
          disabled={testing}
          className="flex flex-1 items-center justify-center gap-1.5 rounded-md border px-3 py-1.5 text-xs text-muted-foreground transition-colors hover:text-foreground disabled:opacity-60"
        >
          {testing && <Loader2 className="size-3.5 animate-spin" aria-hidden />}
          testar conexão
        </button>
      </div>

      {steps && (
        <ul className="mt-3 space-y-1.5">
          {steps.map((step) => (
            <li key={step.label} className="flex items-start gap-2 text-xs">
              {step.ok ? (
                <Check className="mt-0.5 size-3.5 shrink-0 text-found" aria-hidden />
              ) : (
                <X className="mt-0.5 size-3.5 shrink-0 text-destructive" aria-hidden />
              )}
              <span className="min-w-0">
                {step.label}
                {step.detail && (
                  <span className="block truncate font-mono text-muted-foreground">
                    {step.detail}
                  </span>
                )}
              </span>
            </li>
          ))}
        </ul>
      )}

      {!link.secure && (
        <p className="mt-3 flex items-start gap-2 rounded-md border border-found/40 bg-found/10 px-3 py-2 text-xs text-muted-foreground">
          <TriangleAlert className="mt-0.5 size-3.5 shrink-0 text-found" aria-hidden />
          <span>
            Na rede local é HTTP puro: quem estiver no mesmo Wi-Fi consegue ver a sessão passar. Em
            casa tudo bem, em rede que não é sua use o Tailscale.
          </span>
        </p>
      )}
    </>
  )
}

function Switch({
  label,
  hint,
  on,
  busy,
  disabled = false,
  onToggle,
}: {
  label: string
  hint: string
  on: boolean
  busy: boolean
  disabled?: boolean
  onToggle: () => void
}) {
  return (
    <button
      type="button"
      role="switch"
      aria-checked={on}
      disabled={disabled || busy}
      onClick={onToggle}
      className="flex w-full items-center gap-3 rounded-md px-2 py-2 text-left transition-colors hover:bg-muted disabled:opacity-50 disabled:hover:bg-transparent"
    >
      <span className="min-w-0 flex-1">
        <span className="block text-sm">{label}</span>
        <span className="block truncate text-xs text-muted-foreground">{hint}</span>
      </span>
      <span
        className={cn(
          "relative h-5 w-9 shrink-0 rounded-full transition-colors",
          on ? "bg-found" : "bg-muted-foreground/30",
        )}
        aria-hidden
      >
        <span
          className={cn(
            "absolute top-0.5 size-4 rounded-full bg-card transition-[left]",
            on ? "left-[1.125rem]" : "left-0.5",
          )}
        />
      </span>
    </button>
  )
}