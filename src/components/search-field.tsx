import { Search, X } from "lucide-react"
import { useEffect, useRef } from "react"

import type { Topic } from "../lib/api"
import { cn } from "../lib/utils"
import { BrandIcon } from "./brand-icon"

type Props = {
  value: string
  onChange: (value: string) => void
  onKeyDown: (event: React.KeyboardEvent<HTMLInputElement>) => void
  compact: boolean
  topic: Topic | null
  onClearTopic: () => void
}

/**
 * The app is a search that happens to have notes behind it, so this is the
 * first and only thing focused on launch. Mono on purpose: it says "tool for a
 * programmer" before any copy gets the chance to.
 */
export function SearchField({
  value,
  onChange,
  onKeyDown,
  compact,
  topic,
  onClearTopic,
}: Props) {
  const input = useRef<HTMLInputElement>(null)

  useEffect(() => {
    input.current?.focus()
  }, [])

  return (
    <label
      className={cn(
        "flex w-full items-center gap-3 rounded-lg border bg-card px-4 transition-colors",
        "focus-within:border-ring",
        compact ? "h-12" : "h-16",
      )}
    >
      <Search
        className={cn("shrink-0 text-muted-foreground", compact ? "size-4" : "size-5")}
        aria-hidden
      />

      {topic && (
        <span className="flex shrink-0 items-center gap-1.5 rounded-md bg-muted py-1 pl-2 pr-1 font-mono text-xs">
          <BrandIcon slug={topic.icon} color={topic.color} className="size-3" />
          {topic.name}
          <button
            type="button"
            onClick={onClearTopic}
            aria-label={`Sair do tópico ${topic.name}`}
            className="rounded p-0.5 text-muted-foreground hover:text-foreground"
          >
            <X className="size-3" aria-hidden />
          </button>
        </span>
      )}

      <input
        ref={input}
        value={value}
        onChange={(event) => onChange(event.target.value)}
        onKeyDown={onKeyDown}
        placeholder={topic ? "filtrar neste tópico" : "buscar no vault"}
        spellCheck={false}
        autoComplete="off"
        aria-label="Buscar no vault"
        className={cn(
          "w-full bg-transparent font-mono text-foreground outline-none",
          "placeholder:text-muted-foreground",
          compact ? "text-base" : "text-xl",
        )}
      />
    </label>
  )
}
