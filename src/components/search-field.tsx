import { Search } from "lucide-react"
import { useEffect, useRef } from "react"

import { cn } from "../lib/utils"

type Props = {
  value: string
  onChange: (value: string) => void
  onKeyDown: (event: React.KeyboardEvent<HTMLInputElement>) => void
  compact: boolean
}

/**
 * The app is a search that happens to have notes behind it, so this is the
 * first and only thing focused on launch. Mono on purpose: it says "tool for a
 * programmer" before any copy gets the chance to.
 */
export function SearchField({ value, onChange, onKeyDown, compact }: Props) {
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
      <input
        ref={input}
        value={value}
        onChange={(event) => onChange(event.target.value)}
        onKeyDown={onKeyDown}
        placeholder="buscar no vault"
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
