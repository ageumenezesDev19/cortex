import { ChevronLeft, Plus, Trash2, X } from "lucide-react"
import { AnimatePresence, motion, useReducedMotion } from "motion/react"
import { Fragment, useState } from "react"

import type { Topic } from "../lib/api"
import { cn } from "../lib/utils"
import { BrandIcon } from "./brand-icon"
import { IconPicker } from "./icon-picker"

type Props = {
  open: boolean
  topics: Topic[]
  activeSlug: string | null
  onClose: () => void
  onPick: (topic: Topic) => void
  onCreate: (name: string, icon: string | null, color: string | null) => void
  /** Hands the whole topic over: the confirmation names it before it goes. */
  onDelete: (topic: Topic) => void
}

/**
 * Picking a topic does not navigate — it drops a chip in the search field. That
 * is the whole design: browsing and searching are the same gesture, so this
 * panel never becomes a second place to be.
 */
/**
 * Yours, then the app's, with the boundary marked. Sorting rather than two
 * lists keeps one scroll and one keyboard order.
 */
function ordered(topics: Topic[]) {
  const sorted = [...topics].sort((a, b) => Number(a.auto) - Number(b.auto))
  return sorted.map((topic, at) => ({
    topic,
    startsAuto: topic.auto && !sorted[at - 1]?.auto,
  }))
}

export function TopicsPanel({
  open,
  topics,
  activeSlug,
  onClose,
  onPick,
  onCreate,
  onDelete,
}: Props) {
  const reduceMotion = useReducedMotion()
  const [creating, setCreating] = useState(false)

  return (
    <AnimatePresence>
      {open && (
        <motion.aside
          initial={reduceMotion ? { opacity: 0 } : { opacity: 0, x: -12 }}
          animate={{ opacity: 1, x: 0 }}
          exit={reduceMotion ? { opacity: 0 } : { opacity: 0, x: -12 }}
          transition={{ duration: reduceMotion ? 0 : 0.18, ease: "easeOut" }}
          className="fixed inset-x-0 top-0 bottom-[var(--rail)] z-40 flex w-full flex-col border-r bg-card sm:static sm:bottom-auto sm:z-auto sm:w-64 sm:shrink-0"
        >
          <header className="flex items-center justify-between border-b px-3 py-2">
            {/* On a phone the panel is a screen, and a screen is left by a
                button you can hit, not by a 16 px cross in the corner. */}
            <button
              type="button"
              onClick={onClose}
              className="-ml-1 flex h-11 items-center gap-1 rounded-md pl-1 pr-2 text-sm text-muted-foreground transition-colors hover:text-foreground sm:hidden"
            >
              <ChevronLeft className="size-5" aria-hidden />
              voltar
            </button>
            <h2 className="font-mono text-xs uppercase tracking-wide text-muted-foreground">
              Tópicos
            </h2>
            <button
              type="button"
              onClick={onClose}
              aria-label="Fechar tópicos"
              className="hidden rounded p-1 text-muted-foreground hover:text-foreground sm:block"
            >
              <X className="size-4" aria-hidden />
            </button>
          </header>

          <div className="min-h-0 flex-1 space-y-1 overflow-y-auto p-2">
            {/* Yours first, always. A dozen marks the app made for itself must
                not bury the handful you curated. */}
            {ordered(topics).map(({ topic, startsAuto }, position) => (
              <Fragment key={topic.slug}>
                {startsAuto && (
                  <p className="px-2 pb-1 pt-3 font-mono text-xs uppercase tracking-wide text-muted-foreground">
                    automáticos
                  </p>
                )}
              <motion.div
                initial={reduceMotion ? { opacity: 0 } : { opacity: 0, y: 4 }}
                animate={{ opacity: 1, y: 0 }}
                transition={{
                  duration: reduceMotion ? 0 : 0.12,
                  delay: reduceMotion ? 0 : Math.min(position, 8) * 0.02,
                }}
                className="group flex items-center gap-1"
              >
                <button
                  type="button"
                  onClick={() => onPick(topic)}
                  aria-pressed={activeSlug === topic.slug}
                  className={cn(
                    "flex min-w-0 flex-1 items-center gap-2 rounded-md px-2 py-1.5 text-left text-sm transition-colors",
                    activeSlug === topic.slug ? "bg-muted" : "hover:bg-muted/60",
                  )}
                >
                  <BrandIcon slug={topic.icon} color={topic.color} />
                  <span className="truncate">{topic.name}</span>
                  <span className="ml-auto font-mono text-xs text-muted-foreground">
                    {topic.members.length}
                  </span>
                </button>

                <button
                  type="button"
                  onClick={() => onDelete(topic)}
                  aria-label={`Apagar ${topic.name}`}
                  className="rounded p-1 text-muted-foreground opacity-0 transition-opacity hover:text-foreground focus-visible:opacity-100 group-hover:opacity-100"
                >
                  <Trash2 className="size-3.5" aria-hidden />
                </button>
              </motion.div>
              </Fragment>
            ))}

            {topics.length === 0 && !creating && (
              <p className="px-2 py-6 text-center text-sm text-muted-foreground">
                Um tópico junta o que está espalhado por várias pastas.
              </p>
            )}
          </div>

          <div className="border-t p-2">
            {creating ? (
              <NewTopicForm
                onCancel={() => setCreating(false)}
                onCreate={(name, icon, color) => {
                  onCreate(name, icon, color)
                  setCreating(false)
                }}
              />
            ) : (
              <button
                type="button"
                onClick={() => setCreating(true)}
                className="flex w-full items-center gap-2 rounded-md px-2 py-1.5 text-sm text-muted-foreground transition-colors hover:bg-muted hover:text-foreground"
              >
                <Plus className="size-4" aria-hidden />
                Criar tópico
              </button>
            )}
          </div>
        </motion.aside>
      )}
    </AnimatePresence>
  )
}

function NewTopicForm({
  onCreate,
  onCancel,
}: {
  onCreate: (name: string, icon: string | null, color: string | null) => void
  onCancel: () => void
}) {
  const [name, setName] = useState("")
  const [icon, setIcon] = useState<string | null>(null)
  const [color, setColor] = useState<string | null>(null)

  return (
    <form
      onSubmit={(event) => {
        event.preventDefault()
        if (name.trim()) onCreate(name, icon, color)
      }}
      onKeyDown={(event) => event.key === "Escape" && onCancel()}
      className="space-y-2"
    >
      <input
        value={name}
        onChange={(event) => setName(event.target.value)}
        placeholder="nome do tópico"
        spellCheck={false}
        autoFocus
        aria-label="Nome do tópico"
        className="h-8 w-full rounded-md border bg-background px-2 text-sm outline-none focus:border-ring"
      />

      <IconPicker
        value={icon}
        onChange={(slug, hex) => {
          setIcon(slug)
          setColor(hex)
        }}
      />

      <div className="flex gap-2">
        <button
          type="submit"
          disabled={name.trim().length === 0}
          className="flex-1 rounded-md bg-primary px-2 py-1.5 text-sm text-primary-foreground disabled:opacity-40"
        >
          Criar
        </button>
        <button
          type="button"
          onClick={onCancel}
          className="rounded-md px-2 py-1.5 text-sm text-muted-foreground hover:text-foreground"
        >
          Cancelar
        </button>
      </div>
    </form>
  )
}
