import { ChevronDown, ChevronLeft, ChevronRight, FilePlus, FileText, FolderPlus, Image as ImageIcon, MoreHorizontal, X } from "lucide-react"
import { motion, useReducedMotion } from "motion/react"
import { forwardRef, useEffect, useRef, useState } from "react"

import type { TreeEntry } from "../lib/api"
import { usePick } from "./language-provider"
import { useOpenFolders } from "../lib/prefs"
import { cn } from "../lib/utils"

type Props = {
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
/**
 * The ref is forwarded because `AnimatePresence mode="popLayout"` has to reach
 * the real element to take it out of the flow while it leaves — without it the
 * panel on its way out keeps holding its column.
 */
const content = {
  en: {
    newNoteRoot: "New note at the root",
    newFolderRoot: "New folder at the root",
    close: "Close the vault",
    newNoteHere: "New note here",
    newFolderHere: "New folder here",
    rename: "Rename",
    remove: "Delete",
  },
  pt: {
    newNoteRoot: "Nova nota na raiz",
    newFolderRoot: "Nova pasta na raiz",
    close: "Fechar vault",
    newNoteHere: "Nova nota aqui",
    newFolderHere: "Nova pasta aqui",
    rename: "Renomear",
    remove: "Apagar",
  },
}

export const TreePanel = forwardRef<HTMLElement, Props>(function TreePanel(
  { tree, activePath, onClose, onOpen, onCreateNote, onCreateFolder, onRename, onDelete },
  ref,
) {
  const t = usePick()(content)
  const reduceMotion = useReducedMotion()
  const list = useScrollGuard()
  const [open, toggle] = useOpenFolders()

  return (
        <motion.aside
          ref={ref}
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
                label={t.newNoteRoot}
                onClick={() => onCreateNote("")}
              />
              <IconButton
                icon={FolderPlus}
                label={t.newFolderRoot}
                onClick={() => onCreateFolder("")}
              />
              <span className="hidden sm:block">
                <IconButton icon={X} label={t.close} onClick={onClose} />
              </span>
            </div>
          </header>

          {/* `data-scrolling` turns the rows to stone for the length of a
              scroll — see `useScrollGuard`. */}
          <div ref={list} className="scroll-y flex-1 p-2 data-[scrolling]:[&_button]:pointer-events-none">
            {tree.map((entry) => (
              <Node
                key={entry.path}
                entry={entry}
                depth={0}
                open={open}
                onToggle={toggle}
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
  )
})

/**
 * No browser recomputes `:hover` while a list scrolls under a pointer that is
 * not moving: the row that was there keeps the highlight until the mouse moves
 * again. Blocking pointer events for the length of the scroll drops the stale
 * row, and the next movement lands on the right one.
 */
function useScrollGuard() {
  const list = useRef<HTMLDivElement>(null)

  useEffect(() => {
    const node = list.current
    if (!node) return

    let timer: number | undefined
    const onScroll = () => {
      node.dataset.scrolling = "true"
      window.clearTimeout(timer)
      timer = window.setTimeout(() => delete node.dataset.scrolling, 120)
    }

    node.addEventListener("scroll", onScroll, { passive: true })
    return () => {
      node.removeEventListener("scroll", onScroll)
      window.clearTimeout(timer)
    }
  }, [])

  return list
}

function Node({
  entry,
  depth,
  open,
  onToggle,
  activePath,
  onOpen,
  onCreateNote,
  onCreateFolder,
  onRename,
  onDelete,
}: {
  entry: TreeEntry
  depth: number
  /** Which folders are open, kept by `useOpenFolders` and shared by the tree. */
  open: Set<string>
  onToggle: (path: string) => void
} & Pick<
  Props,
  "activePath" | "onOpen" | "onCreateNote" | "onCreateFolder" | "onRename" | "onDelete"
>) {
  const expanded = open.has(entry.path)
  const isFolder = entry.kind === "folder"
  const Icon = isFolder ? null : entry.kind === "image" ? ImageIcon : FileText

  return (
    <div>
      <div className="flex items-center gap-0.5">
        <button
          type="button"
          onClick={() => (isFolder ? onToggle(entry.path) : onOpen(entry))}
          aria-expanded={isFolder ? expanded : undefined}
          aria-current={activePath === entry.path}
          // The panel is 256 px wide and the names came out of Notes long: two
          // folders read as `rocketseat-trilh…` with no way to tell them apart.
          title={entry.name}
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

        {/* One button, always there, on both. The four icons revealed on hover
            were a trail of half-lit rows the moment the list scrolled, because
            hover is not recomputed while it does — and on the phone they ate
            the file name and put delete a stray tap from rename. */}
        <RowMenu
          entry={entry}
          isFolder={isFolder}
          onCreateNote={onCreateNote}
          onCreateFolder={onCreateFolder}
          onRename={onRename}
          onDelete={onDelete}
        />
      </div>

      {isFolder && expanded && (
        <div>
          {entry.children.map((child) => (
            <Node
              key={child.path}
              entry={child}
              depth={depth + 1}
              open={open}
              onToggle={onToggle}
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

/**
 * The row's actions behind one button. Deleting sits at the bottom and reads in
 * words — an icon beside rename is how the wrong file goes.
 */
function RowMenu({
  entry,
  isFolder,
  onCreateNote,
  onCreateFolder,
  onRename,
  onDelete,
}: {
  entry: TreeEntry
  isFolder: boolean
  onCreateNote: (folder: string) => void
  onCreateFolder: (parent: string) => void
  onRename: (entry: TreeEntry) => void
  onDelete: (entry: TreeEntry) => void
}) {
  const t = usePick()(content)
  const [open, setOpen] = useState(false)
  const close = () => setOpen(false)

  return (
    <span className="relative shrink-0">
      <button
        type="button"
        onClick={() => setOpen((value) => !value)}
        aria-label={`Ações de ${entry.name}`}
        aria-expanded={open}
        // 44 px for a thumb, the tighter 28 for a pointer.
        className="flex size-11 items-center justify-center rounded-md text-muted-foreground transition-colors hover:text-foreground sm:size-7"
      >
        <MoreHorizontal className="size-4" aria-hidden />
      </button>

      {open && (
        <>
          <span className="fixed inset-0 z-30" onClick={close} aria-hidden />
          <span className="absolute right-0 top-full z-40 mt-1 flex w-44 flex-col gap-0.5 rounded-md border bg-card p-1 shadow-lg">
            {isFolder && (
              <>
                <MenuItem
                  label={t.newNoteHere}
                  onClick={() => {
                    close()
                    onCreateNote(entry.path)
                  }}
                />
                <MenuItem
                  label={t.newFolderHere}
                  onClick={() => {
                    close()
                    onCreateFolder(entry.path)
                  }}
                />
              </>
            )}
            <MenuItem
              label={t.rename}
              onClick={() => {
                close()
                onRename(entry)
              }}
            />
            <span className="my-0.5 h-px bg-border" aria-hidden />
            <MenuItem
              label={t.remove}
              destructive
              onClick={() => {
                close()
                onDelete(entry)
              }}
            />
          </span>
        </>
      )}
    </span>
  )
}

function MenuItem({
  label,
  destructive = false,
  onClick,
}: {
  label: string
  destructive?: boolean
  onClick: () => void
}) {
  return (
    <button
      type="button"
      onClick={onClick}
      className={cn(
        "rounded px-2 py-2.5 text-left text-sm hover:bg-muted",
        destructive && "text-destructive",
      )}
    >
      {label}
    </button>
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