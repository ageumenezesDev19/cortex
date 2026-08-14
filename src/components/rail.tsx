import { FolderTree, Layers, Monitor, Moon, Search, Sun } from "lucide-react"

import { cn } from "../lib/utils"
import { useTheme, type Theme } from "./theme-provider"

const THEMES: { value: Theme; icon: typeof Sun; label: string }[] = [
  { value: "light", icon: Sun, label: "Tema claro" },
  { value: "dark", icon: Moon, label: "Tema escuro" },
  { value: "system", icon: Monitor, label: "Seguir o sistema" },
]

/** The folder tree arrives in M6; it sits here disabled so the shape holds. */
export function Rail({
  topicsOpen,
  onToggleTopics,
}: {
  topicsOpen: boolean
  onToggleTopics: () => void
}) {
  const { theme, setTheme } = useTheme()

  return (
    <nav className="flex w-12 shrink-0 flex-col items-center gap-1 border-r bg-card py-3">
      <RailButton icon={Search} label="Buscar" active={!topicsOpen} />
      <RailButton
        icon={Layers}
        label="Tópicos"
        active={topicsOpen}
        onClick={onToggleTopics}
      />
      <RailButton icon={FolderTree} label="Pastas (em breve)" disabled />

      <div className="mt-auto flex flex-col gap-1">
        {THEMES.map(({ value, icon, label }) => (
          <RailButton
            key={value}
            icon={icon}
            label={label}
            active={theme === value}
            onClick={() => setTheme(value)}
          />
        ))}
      </div>
    </nav>
  )
}

function RailButton({
  icon: Icon,
  label,
  active = false,
  disabled = false,
  onClick,
}: {
  icon: typeof Sun
  label: string
  active?: boolean
  disabled?: boolean
  onClick?: () => void
}) {
  return (
    <button
      type="button"
      title={label}
      aria-label={label}
      aria-pressed={active}
      disabled={disabled}
      onClick={onClick}
      className={cn(
        "flex size-8 items-center justify-center rounded-md transition-colors",
        active ? "bg-muted text-foreground" : "text-muted-foreground hover:text-foreground",
        disabled && "cursor-not-allowed opacity-40 hover:text-muted-foreground",
      )}
    >
      <Icon className="size-4" aria-hidden />
    </button>
  )
}
