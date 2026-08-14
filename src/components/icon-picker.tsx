import { useMemo, useState } from "react"

import { searchIcons } from "../lib/icons"
import { cn } from "../lib/utils"

type Props = {
  value: string | null
  onChange: (slug: string, hex: string) => void
}

export function IconPicker({ value, onChange }: Props) {
  const [query, setQuery] = useState("")
  const icons = useMemo(() => searchIcons(query), [query])

  return (
    <div className="space-y-2">
      <input
        value={query}
        onChange={(event) => setQuery(event.target.value)}
        placeholder="buscar ícone"
        spellCheck={false}
        aria-label="Buscar ícone"
        className="h-8 w-full rounded-md border bg-background px-2 font-mono text-xs outline-none focus:border-ring"
      />

      <div className="grid max-h-40 grid-cols-8 gap-1 overflow-y-auto">
        {icons.map((icon) => (
          <button
            key={icon.slug}
            type="button"
            title={icon.title}
            aria-label={icon.title}
            aria-pressed={value === icon.slug}
            onClick={() => onChange(icon.slug, icon.hex)}
            className={cn(
              "flex aspect-square items-center justify-center rounded-md border transition-colors",
              value === icon.slug ? "border-ring bg-muted" : "border-transparent hover:bg-muted",
            )}
          >
            <svg viewBox="0 0 24 24" className="size-4" fill={icon.hex}>
              <path d={icon.path} />
            </svg>
          </button>
        ))}

        {icons.length === 0 && (
          <p className="col-span-8 py-3 text-center font-mono text-xs text-muted-foreground">
            nenhum ícone para “{query}”
          </p>
        )}
      </div>
    </div>
  )
}
