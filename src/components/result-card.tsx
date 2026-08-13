import { FileText, Image as ImageIcon } from "lucide-react"

import { assetUrl, type Hit } from "../lib/api"
import { matchingBlocks, parseSnippet } from "../lib/snippet"
import { cn } from "../lib/utils"

type Props = {
  hit: Hit
  query: string
  root: string
  selected: boolean
  onSelect: () => void
  onOpen: () => void
}

export function ResultCard({ hit, query, root, selected, onSelect, onOpen }: Props) {
  const isImage = hit.kind === "image"
  const matched = isImage ? matchingBlocks(hit.blocks, query) : []

  return (
    <button
      type="button"
      onMouseEnter={onSelect}
      onClick={onOpen}
      aria-current={selected}
      className={cn(
        "flex w-full gap-4 rounded-lg border bg-card p-4 text-left transition-colors",
        selected ? "border-ring" : "hover:border-muted-foreground/40",
      )}
    >
      {isImage ? (
        <Thumbnail hit={hit} root={root} matched={matched} />
      ) : (
        <span className="mt-0.5 flex size-9 shrink-0 items-center justify-center rounded-md bg-muted">
          <FileText className="size-4 text-muted-foreground" aria-hidden />
        </span>
      )}

      <span className="min-w-0 flex-1">
        <span className="flex items-baseline gap-2">
          <span className="truncate font-medium">{hit.title}</span>
          {isImage && (
            <span className="flex items-center gap-1 font-mono text-xs text-muted-foreground">
              <ImageIcon className="size-3" aria-hidden />
              print
            </span>
          )}
        </span>

        {hit.headingPath && (
          <span className="mt-0.5 block truncate font-mono text-xs text-muted-foreground">
            {hit.headingPath}
          </span>
        )}

        <span className="mt-2 block text-sm leading-relaxed text-muted-foreground">
          {parseSnippet(hit.snippet).map((segment, position) =>
            segment.found ? (
              <mark
                key={position}
                className="rounded-sm bg-found px-0.5 text-found-foreground"
              >
                {segment.text}
              </mark>
            ) : (
              <span key={position}>{segment.text}</span>
            ),
          )}
        </span>
      </span>
    </button>
  )
}

/**
 * The frame is drawn flat here — the animated version is the reward for opening
 * the result, and spending it on every row would make it wallpaper.
 */
function Thumbnail({ hit, root, matched }: { hit: Hit; root: string; matched: Hit["blocks"] }) {
  return (
    <span className="relative block size-16 shrink-0 overflow-hidden rounded-md border bg-muted">
      <img
        src={assetUrl(root, hit.path)}
        alt=""
        className="size-full object-cover object-top"
        draggable={false}
      />
      <svg viewBox="0 0 1 1" preserveAspectRatio="none" className="absolute inset-0 size-full">
        {matched.map((block, position) => (
          <rect
            key={position}
            x={block.bbox[0]}
            y={block.bbox[1]}
            width={block.bbox[2]}
            height={block.bbox[3]}
            fill="hsl(var(--found) / 0.25)"
            stroke="hsl(var(--found))"
            strokeWidth={0.008}
            vectorEffect="non-scaling-stroke"
          />
        ))}
      </svg>
    </span>
  )
}
