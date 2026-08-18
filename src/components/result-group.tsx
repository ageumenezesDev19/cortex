import { FileText, Image as ImageIcon, Plus } from "lucide-react"
import { useState } from "react"

import type { Group, Hit, Topic } from "../lib/api"
import { cn } from "../lib/utils"
import { usePick } from "./language-provider"
import { BrandIcon } from "./brand-icon"
import { ResultCard } from "./result-card"

type Props = {
  group: Group
  query: string
  root: string
  topics: Topic[]
  selectedHit: Hit | null
  onSelect: (hit: Hit) => void
  onOpen: (hit: Hit) => void
  onAddToTopic: (slug: string, path: string) => void
}

/**
 * A note appears once, with its matching passages inside it. Before this, a long
 * note filled the list with copies of its own title.
 */
const content = {
  en: {
    more: (n: number) => `+${n} ${n === 1 ? "passage" : "passages"} in this note`,
    addedTo: (name: string) => `Added to ${name}`,
    topic: "topic",
  },
  pt: {
    more: (n: number) => `+${n} ${n === 1 ? "trecho" : "trechos"} nesta nota`,
    addedTo: (name: string) => `Adicionado a ${name}`,
    topic: "tópico",
  },
}

export function ResultGroup({
  group,
  query,
  root,
  topics,
  selectedHit,
  onSelect,
  onOpen,
  onAddToTopic,
}: Props) {
  const t = usePick()(content)
  const hidden = group.total - group.hits.length

  return (
    <article className="rounded-lg border bg-card">
      <header className="flex items-center gap-3 border-b px-4 py-3">
        <span className="flex size-8 shrink-0 items-center justify-center rounded-md bg-muted">
          {group.kind === "image" ? (
            <ImageIcon className="size-4 text-muted-foreground" aria-hidden />
          ) : (
            <FileText className="size-4 text-muted-foreground" aria-hidden />
          )}
        </span>

        <span className="min-w-0 flex-1">
          <span className="block truncate font-medium">{group.title}</span>
          {group.folder && (
            <span className="block truncate font-mono text-xs text-muted-foreground">
              {group.folder}
            </span>
          )}
        </span>

        <AddToTopic topics={topics} path={group.path} onAdd={onAddToTopic} />
      </header>

      <div className="space-y-1 p-2">
        {group.hits.map((hit) => (
          <ResultCard
            key={hit.chunkId}
            hit={hit}
            query={query}
            root={root}
            selected={selectedHit?.chunkId === hit.chunkId}
            onSelect={() => onSelect(hit)}
            onOpen={() => onOpen(hit)}
          />
        ))}

        {hidden > 0 && (
          <p className="px-2 pb-1 font-mono text-xs text-muted-foreground">{t.more(hidden)}</p>
        )}
      </div>
    </article>
  )
}

/** The button says what happens, and afterwards says that it happened. */
function AddToTopic({
  topics,
  path,
  onAdd,
}: {
  topics: Topic[]
  path: string
  onAdd: (slug: string, path: string) => void
}) {
  const t = usePick()(content)
  const [open, setOpen] = useState(false)
  const [added, setAdded] = useState<string | null>(null)

  if (topics.length === 0) return null

  if (added) {
    return (
      <span className="shrink-0 font-mono text-xs text-muted-foreground">{t.addedTo(added)}</span>
    )
  }

  return (
    <span className="relative shrink-0">
      <button
        type="button"
        onClick={() => setOpen((value) => !value)}
        aria-expanded={open}
        className={cn(
          "flex items-center gap-1 rounded-md px-2 py-1 font-mono text-xs transition-colors",
          "text-muted-foreground hover:bg-muted hover:text-foreground",
        )}
      >
        <Plus className="size-3" aria-hidden />
        {t.topic}
      </button>

      {open && (
        <span className="absolute right-0 top-full z-10 mt-1 flex w-48 flex-col gap-0.5 rounded-md border bg-card p-1 shadow-lg">
          {topics.map((topic) => (
            <button
              key={topic.slug}
              type="button"
              onClick={() => {
                onAdd(topic.slug, path)
                setAdded(topic.name)
                setOpen(false)
              }}
              className="flex items-center gap-2 rounded px-2 py-1.5 text-left text-sm hover:bg-muted"
            >
              <BrandIcon slug={topic.icon} color={topic.color} />
              <span className="truncate">{topic.name}</span>
            </button>
          ))}
        </span>
      )}
    </span>
  )
}
