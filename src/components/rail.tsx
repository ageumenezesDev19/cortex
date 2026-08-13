import { FolderTree, Layers, Monitor, Moon, Search, Sun } from "lucide-react"

import { useTheme, type Theme } from "./theme-provider"
import { cn } from "../lib/utils"

const THEMES: { value: Theme; icon: typeof Sun; label: string }[] = [
  { value: "light", icon: Sun, label: "Tema claro" },
  { value: "dark", icon: Moon, label: "Tema escuro" },
  { value: "system", icon: Monitor, label: "Seguir o sistema" },
]

/**
 * Topics and the folder tree arrive in M3. They sit here disabled rather than
 * absent so the shape of the app stops moving between milestones.
 */
export function Rail() {
  const { theme, setTheme } = useTheme()

  return (
    <nav className="flex w-12 shrink-0 flex-col items-center gap-1 border-r bg-card py-3">
      <RailButton icon={Search} label="Buscar" active />
      <RailButton icon={Layers} label="Tópicos (em breve)" disabled />
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
