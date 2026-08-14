import { FilePlus, FolderPlus, FolderTree, Layers, Monitor, Moon, Plus, Search, Sun } from "lucide-react"
import { useState } from "react"

import { cn } from "../lib/utils"
import { useTheme, type Theme } from "./theme-provider"

const THEMES: { value: Theme; icon: typeof Sun; label: string }[] = [
  { value: "light", icon: Sun, label: "Tema claro" },
  { value: "dark", icon: Moon, label: "Tema escuro" },
  { value: "system", icon: Monitor, label: "Seguir o sistema" },
]

export type Panel = "none" | "topics" | "tree"

export function Rail({
  panel,
  onTogglePanel,
  onNewNote,
  onNewFolder,
}: {
  panel: Panel
  onTogglePanel: (panel: Panel) => void
  onNewNote: () => void
  onNewFolder: () => void
}) {
  const { theme, setTheme } = useTheme()

  return (
    <nav className="flex w-12 shrink-0 flex-col items-center gap-1 border-r bg-card py-3">
      {/* Above the panels on purpose: creating has to be reachable from every
          screen, not from inside a panel you must know to open first. */}
      <NewMenu onNewNote={onNewNote} onNewFolder={onNewFolder} />
      <span className="my-1 h-px w-6 bg-border" aria-hidden />

      <RailButton
        icon={Search}
        label="Buscar"
        active={panel === "none"}
        onClick={() => onTogglePanel("none")}
      />
      <RailButton
        icon={Layers}
        label="Tópicos"
        active={panel === "topics"}
        onClick={() => onTogglePanel("topics")}
      />
      <RailButton
        icon={FolderTree}
        label="Vault"
        active={panel === "tree"}
        onClick={() => onTogglePanel("tree")}
      />

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

function NewMenu({ onNewNote, onNewFolder }: { onNewNote: () => void; onNewFolder: () => void }) {
  const [open, setOpen] = useState(false)

  return (
    <span className="relative">
      <button
        type="button"
        title="Criar — Cmd+N para nota"
        aria-label="Criar"
        aria-expanded={open}
        onClick={() => setOpen((value) => !value)}
        className={cn(
          "flex size-8 items-center justify-center rounded-md transition-colors",
          open ? "bg-muted text-foreground" : "text-muted-foreground hover:text-foreground",
        )}
      >
        <Plus className="size-4" aria-hidden />
      </button>

      {open && (
        <>
          <span className="fixed inset-0 z-10" onClick={() => setOpen(false)} aria-hidden />
          <span className="absolute left-full top-0 z-20 ml-2 flex w-40 flex-col gap-0.5 rounded-md border bg-card p-1 shadow-lg">
            <MenuItem
              icon={FilePlus}
              label="Nova nota"
              onClick={() => {
                setOpen(false)
                onNewNote()
              }}
            />
            <MenuItem
              icon={FolderPlus}
              label="Nova pasta"
              onClick={() => {
                setOpen(false)
                onNewFolder()
              }}
            />
          </span>
        </>
      )}
    </span>
  )
}

function MenuItem({
  icon: Icon,
  label,
  onClick,
}: {
  icon: typeof Sun
  label: string
  onClick: () => void
}) {
  return (
    <button
      type="button"
      onClick={onClick}
      className="flex items-center gap-2 rounded px-2 py-1.5 text-left text-sm hover:bg-muted"
    >
      <Icon className="size-4 text-muted-foreground" aria-hidden />
      {label}
    </button>
  )
}

function RailButton({
  icon: Icon,
  label,
  active = false,
  onClick,
}: {
  icon: typeof Sun
  label: string
  active?: boolean
  onClick?: () => void
}) {
  return (
    <button
      type="button"
      title={label}
      aria-label={label}
      aria-pressed={active}
      onClick={onClick}
      className={cn(
        "flex size-8 items-center justify-center rounded-md transition-colors",
        active ? "bg-muted text-foreground" : "text-muted-foreground hover:text-foreground",
      )}
    >
      <Icon className="size-4" aria-hidden />
    </button>
  )
}