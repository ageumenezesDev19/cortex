import { Search, X } from "lucide-react"
import { useEffect } from "react"

import type { Topic } from "../lib/api"
import { cn } from "../lib/utils"
import { BrandIcon } from "./brand-icon"

type Props = {
  /** Held by the app so Escape can hand focus back from anywhere. */
  inputRef: React.RefObject<HTMLInputElement | null>
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
  inputRef,
  value,
  onChange,
  onKeyDown,
  compact,
  topic,
  onClearTopic,
}: Props) {
  useEffect(() => {
    // Not on a phone: focusing on load throws the keyboard up over the screen
    // before anyone asked to type.
    if (window.matchMedia("(min-width: 640px)").matches) inputRef.current?.focus()
  }, [inputRef])

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
        ref={inputRef}
        value={value}
        onChange={(event) => onChange(event.target.value)}
        onKeyDown={onKeyDown}
        placeholder={topic ? "filtrar neste tópico" : "buscar no vault"}
        spellCheck={false}
        autoComplete="off"
        autoCapitalize="off"
        autoCorrect="off"
        enterKeyHint="search"
        aria-label="Buscar no vault"
        className={cn(
          "w-full min-w-0 bg-transparent font-mono text-foreground outline-none",
          "placeholder:text-muted-foreground",
          compact ? "text-base" : "text-xl",
        )}
      />

      {/* Ours, not the browser's. `type="search"` draws a clear button on a Mac
          and none on an iPhone, which left emptying the field to the backspace
          key — and put a Safari address suggestion over the field for free. */}
      {value.length > 0 && (
        <button
          type="button"
          onClick={() => {
            onChange("")
            inputRef.current?.focus()
          }}
          aria-label="Limpar a busca"
          className="-mr-2 flex size-11 shrink-0 items-center justify-center rounded-md text-muted-foreground transition-colors hover:text-foreground sm:size-8"
        >
          <X className="size-4" aria-hidden />
        </button>
      )}
    </label>
  )
}
