import { Check, Copy, ShieldAlert, TriangleAlert } from "lucide-react"
import { AnimatePresence, motion, useReducedMotion } from "motion/react"
import QRCode from "qrcode"
import { useEffect, useState } from "react"

import { serverAddress, type ServerLink } from "../lib/api"
import { cn } from "../lib/utils"

/**
 * The address to open on the phone, as a QR code. The link carries the session
 * token, and typing sixty-four characters on a phone keyboard is not a thing
 * anyone would do twice.
 */
export function SharePanel({ open, onClose }: { open: boolean; onClose: () => void }) {
  const reduceMotion = useReducedMotion()
  const [links, setLinks] = useState<ServerLink[] | null>(null)
  const [picked, setPicked] = useState(0)
  const [image, setImage] = useState("")
  const [copied, setCopied] = useState(false)

  useEffect(() => {
    if (!open) return
    setLinks(null)
    setPicked(0)
    serverAddress()
      .then(setLinks)
      .catch(() => setLinks([]))
  }, [open])

  const link = links?.[picked]

  useEffect(() => {
    if (!link) return setImage("")
    // Rendered locally: the address would be a session for anyone who saw it.
    QRCode.toDataURL(link.url, { margin: 1, width: 320 })
      .then(setImage)
      .catch(() => setImage(""))
  }, [link])

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
            className="w-full max-w-sm rounded-lg border bg-card p-5 shadow-xl"
          >
            <h2 className="font-medium">Abrir no celular</h2>
            <p className="mt-1 text-sm text-muted-foreground">
              Aponte a câmera. O vault continua só aqui — o telefone é uma janela.
            </p>

            {links === null && (
              <p className="mt-4 font-mono text-xs text-muted-foreground">procurando o endereço…</p>
            )}

            {links?.length === 0 && (
              <p className="mt-4 flex items-start gap-2 text-sm text-muted-foreground">
                <ShieldAlert className="mt-0.5 size-4 shrink-0" aria-hidden />
                Não achei nenhum endereço. O servidor não subiu, ou esta máquina está sem rede.
              </p>
            )}

            {links && links.length > 1 && (
              <div className="mt-4 flex gap-1 rounded-md bg-muted p-1">
                {links.map((one, at) => (
                  <button
                    key={one.label}
                    type="button"
                    onClick={() => setPicked(at)}
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

            {link && (
              <>
                {image && (
                  <img
                    src={image}
                    alt={`QR code para ${link.label}`}
                    // White behind it in both themes: a dark QR on a dark card
                    // is a QR no camera reads.
                    className="mx-auto mt-4 w-56 rounded-md bg-white p-2"
                  />
                )}

                <button
                  type="button"
                  onClick={() => {
                    void navigator.clipboard.writeText(link.url)
                    setCopied(true)
                    setTimeout(() => setCopied(false), 1500)
                  }}
                  className="mt-3 flex w-full items-center justify-center gap-1.5 rounded-md border px-3 py-1.5 text-xs text-muted-foreground transition-colors hover:text-foreground"
                >
                  {copied ? (
                    <Check className="size-3.5 text-found" aria-hidden />
                  ) : (
                    <Copy className="size-3.5" aria-hidden />
                  )}
                  {copied ? "copiado" : "copiar o endereço"}
                </button>

                {!link.secure && (
                  <p className="mt-3 flex items-start gap-2 rounded-md border border-found/40 bg-found/10 px-3 py-2 text-xs text-muted-foreground">
                    <TriangleAlert className="mt-0.5 size-3.5 shrink-0 text-found" aria-hidden />
                    <span>
                      Na rede local é HTTP puro: quem estiver no mesmo Wi-Fi consegue ver a sessão
                      passar. Em casa tudo bem, em rede que não é sua use o Tailscale.
                    </span>
                  </p>
                )}
              </>
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
