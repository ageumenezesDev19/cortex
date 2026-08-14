import { ChevronDown, ChevronLeft, ChevronRight, FilePlus, FileText, FolderPlus, Image as ImageIcon, Pencil, Trash2, X } from "lucide-react"
import { AnimatePresence, motion, useReducedMotion } from "motion/react"
import { useState } from "react"

import type { TreeEntry } from "../lib/api"
import { cn } from "../lib/utils"

type Props = {
  open: boolean
  tree: TreeEntry[]
  activePath: string | null
  onClose: () => void
  /** The whole entry, because opening a note and opening an image differ. */
  onOpen: (entry: TreeEntry) => void
  onCreateNote: (folder: string) => void
  onCreateFolder: (parent: string) => void
  onRename: (entry: TreeEntry) => void
  onDelete: (entry: TreeEntry) => void
}

/**
 * The vault as it sits on disk. With an empty search field there was nothing to
 * look at before this — the app could find a note but never show you what you
 * had.
 */
export function TreePanel({
  open,
  tree,
  activePath,
  onClose,
  onOpen,
  onCreateNote,
  onCreateFolder,
  onRename,
  onDelete,
}: Props) {
  const reduceMotion = useReducedMotion()

  return (
    <AnimatePresence>
      {open && (
        <motion.aside
          initial={reduceMotion ? { opacity: 0 } : { opacity: 0, x: -12 }}
          animate={{ opacity: 1, x: 0 }}
          exit={reduceMotion ? { opacity: 0 } : { opacity: 0, x: -12 }}
          transition={{ duration: reduceMotion ? 0 : 0.18, ease: "easeOut" }}
          className="fixed inset-x-0 top-0 bottom-[var(--rail)] z-40 flex w-full flex-col border-r bg-card sm:static sm:bottom-auto sm:z-auto sm:w-64 sm:shrink-0"
        >
          <header className="flex items-center justify-between border-b px-3 py-2">
            {/* On a phone the panel is a screen, and a screen is left by a
                button you can hit, not by a 16 px cross in the corner. */}
            <button
              type="button"
              onClick={onClose}
              className="-ml-1 flex h-11 items-center gap-1 rounded-md pl-1 pr-2 text-sm text-muted-foreground transition-colors hover:text-foreground sm:hidden"
            >
              <ChevronLeft className="size-5" aria-hidden />
              voltar
            </button>
            <h2 className="font-mono text-xs uppercase tracking-wide text-muted-foreground">
              Vault
            </h2>
            <div className="flex items-center gap-1">
              <IconButton
                icon={FilePlus}
                label="Nova nota na raiz"
                onClick={() => onCreateNote("")}
              />
              <IconButton
                icon={FolderPlus}
                label="Nova pasta na raiz"
                onClick={() => onCreateFolder("")}
              />
              <span className="hidden sm:block">
                <IconButton icon={X} label="Fechar vault" onClick={onClose} />
              </span>
            </div>
          </header>

          <div className="min-h-0 flex-1 overflow-y-auto p-2">
            {tree.map((entry) => (
              <Node
                key={entry.path}
                entry={entry}
                depth={0}
                activePath={activePath}
                onOpen={onOpen}
                onCreateNote={onCreateNote}
                onCreateFolder={onCreateFolder}
                onRename={onRename}
                onDelete={onDelete}
              />
            ))}

            {tree.length === 0 && (
              <p className="px-2 py-6 text-center text-sm text-muted-foreground">
                Vault vazio. Crie a primeira nota pelo ícone acima.
              </p>
            )}
          </div>
        </motion.aside>
      )}
    </AnimatePresence>
  )
}

function Node({
  entry,
  depth,
  activePath,
  onOpen,
  onCreateNote,
  onCreateFolder,
  onRename,
  onDelete,
}: {
  entry: TreeEntry
  depth: number
} & Pick<
  Props,
  "activePath" | "onOpen" | "onCreateNote" | "onCreateFolder" | "onRename" | "onDelete"
>) {
  const [expanded, setExpanded] = useState(depth === 0)
  const isFolder = entry.kind === "folder"
  const Icon = isFolder ? null : entry.kind === "image" ? ImageIcon : FileText

  return (
    <div>
      <div className="group flex items-center gap-0.5">
        <button
          type="button"
          onClick={() => (isFolder ? setExpanded((value) => !value) : onOpen(entry))}
          aria-expanded={isFolder ? expanded : undefined}
          aria-current={activePath === entry.path}
          style={{ paddingLeft: `${depth * 12 + 4}px` }}
          className={cn(
            "flex min-w-0 flex-1 items-center gap-1.5 rounded-md py-1 pr-1 text-left text-sm transition-colors",
            activePath === entry.path ? "bg-muted" : "hover:bg-muted/60",
          )}
        >
          {isFolder ? (
            expanded ? (
              <ChevronDown className="size-3.5 shrink-0 text-muted-foreground" aria-hidden />
            ) : (
              <ChevronRight className="size-3.5 shrink-0 text-muted-foreground" aria-hidden />
            )
          ) : (
            Icon && <Icon className="size-3.5 shrink-0 text-muted-foreground" aria-hidden />
          )}
          <span className="min-w-0 truncate">{entry.name}</span>
        </button>

        <span className="flex shrink-0 items-center transition-opacity focus-within:opacity-100 [@media(hover:hover)]:opacity-0 [@media(hover:hover)]:group-hover:opacity-100">
          {isFolder && (
            <>
              <IconButton
                icon={FilePlus}
                label={`Nova nota em ${entry.name}`}
                onClick={() => onCreateNote(entry.path)}
              />
              <IconButton
                icon={FolderPlus}
                label={`Nova pasta em ${entry.name}`}
                onClick={() => onCreateFolder(entry.path)}
              />
            </>
          )}
          <IconButton
            icon={Pencil}
            label={`Renomear ${entry.name}`}
            onClick={() => onRename(entry)}
          />
          <IconButton
            icon={Trash2}
            label={`Apagar ${entry.name}`}
            onClick={() => onDelete(entry)}
          />
        </span>
      </div>

      {isFolder && expanded && (
        <div>
          {entry.children.map((child) => (
            <Node
              key={child.path}
              entry={child}
              depth={depth + 1}
              activePath={activePath}
              onOpen={onOpen}
              onCreateNote={onCreateNote}
              onCreateFolder={onCreateFolder}
              onRename={onRename}
              onDelete={onDelete}
            />
          ))}
        </div>
      )}
    </div>
  )
}

function IconButton({
  icon: Icon,
  label,
  onClick,
}: {
  icon: typeof FileText
  label: string
  onClick: () => void
}) {
  return (
    <button
      type="button"
      title={label}
      aria-label={label}
      onClick={onClick}
      className="rounded p-1 text-muted-foreground transition-colors hover:text-foreground"
    >
      <Icon className="size-3.5" aria-hidden />
    </button>
  )
}