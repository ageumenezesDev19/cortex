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

/** One matching passage inside a group. The group header carries the title. */
export function ResultCard({ hit, query, root, selected, onSelect, onOpen }: Props) {
  const isImage = hit.kind === "image"
  const matched = isImage ? matchingBlocks(hit.blocks, query) : []

  return (
    <button
      type="button"
      data-hit={hit.chunkId}
      onMouseEnter={onSelect}
      onClick={onOpen}
      aria-current={selected}
      className={cn(
        "flex w-full gap-3 rounded-md border border-transparent p-2 text-left transition-colors",
        selected ? "border-ring bg-muted/40" : "hover:bg-muted/30",
      )}
    >
      {isImage && <Thumbnail hit={hit} root={root} matched={matched} />}

      <span className="min-w-0 flex-1">
        {hit.headingPath && (
          <span className="block truncate font-mono text-xs text-muted-foreground">
            {hit.headingPath}
          </span>
        )}
        {/* A pasted URL has no spaces to break at, so it sets a minimum width
            for the whole list and pushes it off the screen — which on a phone
            costs the vertical scroll, not just the looks. */}
        <span className="mt-1 block break-words text-sm leading-relaxed text-muted-foreground">
          {parseSnippet(hit.snippet).map((segment, position) =>
            segment.found ? (
              <mark key={position} className="rounded-sm bg-found px-0.5 text-found-foreground">
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
    <span className="relative block size-14 shrink-0 overflow-hidden rounded-md border bg-muted">
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
            strokeWidth={1}
            vectorEffect="non-scaling-stroke"
          />
        ))}
      </svg>
    </span>
  )
}
