import {
  Download,
  FilePlus,
  FolderPlus,
  FolderTree,
  Layers,
  Monitor,
  Moon,
  Plus,
  ImagePlus,
  Search,
  Share2,
  Sun,
} from "lucide-react"
import { useState } from "react"

import { isDesktop } from "../lib/api"
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
  onImport,
  onShare,
  onSendPhoto,
}: {
  panel: Panel
  onTogglePanel: (panel: Panel) => void
  onNewNote: () => void
  onNewFolder: () => void
  onImport: () => void
  onShare: () => void
  onSendPhoto: () => void
}) {
  return (
    // A column on the side of a window, a bar under a thumb on a phone. The
    // padding at the end is the home indicator on an iPhone.
    <nav
      className={cn(
        // Above the content, and above it by more than a tie: the sticky search
        // field is later in the DOM, so at the same z-index it won and painted
        // over the menu that opens out of this bar.
        "relative z-30 flex shrink-0 items-center gap-1 border-t bg-card px-2 py-1",
        "sm:w-12 sm:flex-col sm:border-r sm:border-t-0 sm:px-0 sm:py-3",
      )}
      style={{ paddingBottom: "max(0.25rem, env(safe-area-inset-bottom))" }}
    >
      {/* Above the panels on purpose: creating has to be reachable from every
          screen, not from inside a panel you must know to open first. */}
      <NewMenu
        onNewNote={onNewNote}
        onNewFolder={onNewFolder}
        onImport={onImport}
        onSendPhoto={onSendPhoto}
      />
      <span className="my-1 hidden h-px w-6 bg-border sm:block" aria-hidden />

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

      <div className="ml-auto flex items-center gap-1 sm:ml-0 sm:mt-auto sm:flex-col">
        {/* Only the machine serving the vault has an address to hand out. */}
        {isDesktop && <RailButton icon={Share2} label="Abrir no celular" onClick={onShare} />}
        <ThemeButtons />
      </div>
    </nav>
  )
}

/**
 * Three buttons where there is room, one that cycles where there is not — a
 * phone has no space for a setting you touch once a month.
 */
function ThemeButtons() {
  const { theme, setTheme } = useTheme()
  const current = THEMES.find((one) => one.value === theme) ?? THEMES[2]

  return (
    <>
      <RailButton
        className="sm:hidden"
        icon={current.icon}
        label={current.label}
        onClick={() => {
          const at = THEMES.findIndex((one) => one.value === theme)
          setTheme(THEMES[(at + 1) % THEMES.length].value)
        }}
      />
      {THEMES.map(({ value, icon, label }) => (
        <RailButton
          key={value}
          className="hidden sm:flex"
          icon={icon}
          label={label}
          active={theme === value}
          onClick={() => setTheme(value)}
        />
      ))}
    </>
  )
}

function NewMenu({
  onNewNote,
  onNewFolder,
  onImport,
  onSendPhoto,
}: {
  onNewNote: () => void
  onNewFolder: () => void
  onImport: () => void
  onSendPhoto: () => void
}) {
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
          "flex size-11 items-center justify-center rounded-md transition-colors sm:size-8",
          open ? "bg-muted text-foreground" : "text-muted-foreground hover:text-foreground",
        )}
      >
        <Plus className="size-5 sm:size-4" aria-hidden />
      </button>

      {open && (
        <>
          <span className="fixed inset-0 z-10" onClick={() => setOpen(false)} aria-hidden />
          {/* Upwards from a bottom bar, sideways from a side rail. */}
          <span className="absolute bottom-full left-0 z-20 mb-2 flex w-44 flex-col gap-0.5 rounded-md border bg-card p-1 shadow-lg sm:bottom-auto sm:left-full sm:top-0 sm:mb-0 sm:ml-2 sm:w-40">
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
            {/* The phone has a camera and a photo library; the Mac has the
                Notes.app library. Each gets the door it can use. */}
            {!isDesktop && (
              <MenuItem
                icon={ImagePlus}
                label="Mandar foto"
                onClick={() => {
                  setOpen(false)
                  onSendPhoto()
                }}
              />
            )}
            {isDesktop && (
              <>
                <span className="my-0.5 h-px bg-border" aria-hidden />
                <MenuItem
                  icon={Download}
                  label="Importar do Notes"
                  onClick={() => {
                    setOpen(false)
                    onImport()
                  }}
                />
              </>
            )}
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
      className="flex items-center gap-2 rounded px-2 py-2.5 text-left text-sm hover:bg-muted sm:py-1.5"
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
  className,
}: {
  icon: typeof Sun
  label: string
  active?: boolean
  onClick?: () => void
  className?: string
}) {
  return (
    <button
      type="button"
      title={label}
      aria-label={label}
      aria-pressed={active}
      onClick={onClick}
      // 44 px is Apple's floor for something you hit with a thumb; the mouse
      // gets the tighter 32.
      className={cn(
        "flex size-11 items-center justify-center rounded-md transition-colors sm:size-8",
        active ? "bg-muted text-foreground" : "text-muted-foreground hover:text-foreground",
        className,
      )}
    >
      <Icon className="size-5 sm:size-4" aria-hidden />
    </button>
  )
}
