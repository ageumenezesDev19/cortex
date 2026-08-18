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
  Languages,
  Search,
  Share2,
  ShieldCheck,
  Sun,
  Tags,
} from "lucide-react"
import { useState } from "react"

import { isDesktop } from "../lib/api"
import { cn } from "../lib/utils"
import { useLanguage, usePick } from "./language-provider"
import { useTheme, type Theme } from "./theme-provider"

const content = {
  en: {
    themes: { light: "Light theme", dark: "Dark theme", system: "Follow the system" },
    search: "Search",
    topics: "Topics",
    vault: "Vault",
    privacy: "AI and privacy",
    phone: "Open on the phone",
    createTitle: "Create — Cmd+N for a note",
    create: "Create",
    newNote: "New note",
    newFolder: "New folder",
    sendPhoto: "Send a photo",
    importNotes: "Import from Notes",
    classify: "Sort screenshots",
    language: "Português",
  },
  pt: {
    themes: { light: "Tema claro", dark: "Tema escuro", system: "Seguir o sistema" },
    search: "Buscar",
    topics: "Tópicos",
    vault: "Vault",
    privacy: "IA e privacidade",
    phone: "Abrir no celular",
    createTitle: "Criar — Cmd+N para nota",
    create: "Criar",
    newNote: "Nova nota",
    newFolder: "Nova pasta",
    sendPhoto: "Mandar foto",
    importNotes: "Importar do Notes",
    classify: "Classificar prints",
    language: "English",
  },
}

const THEME_ORDER: { value: Theme; icon: typeof Sun }[] = [
  { value: "light", icon: Sun },
  { value: "dark", icon: Moon },
  { value: "system", icon: Monitor },
]

export type Panel = "none" | "topics" | "tree"

export function Rail({
  panel,
  onTogglePanel,
  onNewNote,
  onNewFolder,
  onImport,
  onShare,
  onSettings,
  onSendPhoto,
  onClassify,
}: {
  panel: Panel
  onTogglePanel: (panel: Panel) => void
  onNewNote: () => void
  onNewFolder: () => void
  onImport: () => void
  onShare: () => void
  onSettings: () => void
  onSendPhoto: () => void
  onClassify: () => void
}) {
  const t = usePick()(content)
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
        onClassify={onClassify}
      />
      <span className="my-1 hidden h-px w-6 bg-border sm:block" aria-hidden />

      <RailButton
        icon={Search}
        label={t.search}
        active={panel === "none"}
        onClick={() => onTogglePanel("none")}
      />
      <RailButton
        icon={Layers}
        label={t.topics}
        active={panel === "topics"}
        onClick={() => onTogglePanel("topics")}
      />
      <RailButton
        icon={FolderTree}
        label={t.vault}
        active={panel === "tree"}
        onClick={() => onTogglePanel("tree")}
      />

      <div className="ml-auto flex items-center gap-1 sm:ml-0 sm:mt-auto sm:flex-col">
        {/* Only the machine serving the vault has an address to hand out — and
            only it decides what the AI may read, with which key. */}
        {isDesktop && (
          <>
            <RailButton icon={ShieldCheck} label={t.privacy} onClick={onSettings} />
            <RailButton icon={Share2} label={t.phone} onClick={onShare} />
          </>
        )}
        <LanguageButton />
        <ThemeButtons />
      </div>
    </nav>
  )
}

/**
 * Only the demo shows this. The installed app has one reader, and translating
 * the window he already reads would be work with no audience — see
 * `language-provider`.
 */
function LanguageButton() {
  const t = usePick()(content)
  const { language, setLanguage, switchable } = useLanguage()
  if (!switchable) return null

  return (
    <RailButton
      icon={Languages}
      label={t.language}
      onClick={() => setLanguage(language === "en" ? "pt" : "en")}
    />
  )
}

/**
 * Three buttons where there is room, one that cycles where there is not — a
 * phone has no space for a setting you touch once a month.
 */
function ThemeButtons() {
  const t = usePick()(content)
  const { theme, setTheme } = useTheme()
  const current = THEME_ORDER.find((one) => one.value === theme) ?? THEME_ORDER[2]

  return (
    <>
      <RailButton
        className="sm:hidden"
        icon={current.icon}
        label={t.themes[current.value]}
        onClick={() => {
          const at = THEME_ORDER.findIndex((one) => one.value === theme)
          setTheme(THEME_ORDER[(at + 1) % THEME_ORDER.length].value)
        }}
      />
      {THEME_ORDER.map(({ value, icon }) => (
        <RailButton
          key={value}
          className="hidden sm:flex"
          icon={icon}
          label={t.themes[value]}
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
  onClassify,
}: {
  onNewNote: () => void
  onNewFolder: () => void
  onImport: () => void
  onSendPhoto: () => void
  onClassify: () => void
}) {
  const t = usePick()(content)
  const [open, setOpen] = useState(false)

  return (
    <span className="relative">
      <button
        type="button"
        title={t.createTitle}
        aria-label={t.create}
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
              label={t.newNote}
              onClick={() => {
                setOpen(false)
                onNewNote()
              }}
            />
            <MenuItem
              icon={FolderPlus}
              label={t.newFolder}
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
                label={t.sendPhoto}
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
                  label={t.importNotes}
                  onClick={() => {
                    setOpen(false)
                    onImport()
                  }}
                />
                {/* Prints que chegam já são classificados; isto é para os que
                    entraram antes, e só quando você mandar. */}
                <MenuItem
                  icon={Tags}
                  label={t.classify}
                  onClick={() => {
                    setOpen(false)
                    onClassify()
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
