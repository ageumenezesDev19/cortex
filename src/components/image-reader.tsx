import { motion, useReducedMotion } from "motion/react"

import { assetUrl, type Hit, type OcrBlock } from "../lib/api"
import { matchingBlocks } from "../lib/snippet"

type Props = {
  hit: Hit
  query: string
  root: string
}

/**
 * The one expensive animation in the app. The frame Vision drew around the
 * recognised text draws itself around the line you searched for — it explains
 * the product without a caption, which is why nothing else gets to use it.
 */
export function ImageReader({ hit, query, root }: Props) {
  const reduceMotion = useReducedMotion()
  const matched = matchingBlocks(hit.blocks, query)

  return (
    <figure className="flex h-full flex-col gap-4">
      <div className="relative min-h-0 flex-1 overflow-hidden rounded-lg border bg-muted">
        <img
          src={assetUrl(root, hit.path)}
          alt={hit.blocks.map((block) => block.text).join(" ")}
          className="size-full object-contain"
          draggable={false}
        />
        <svg
          viewBox="0 0 1 1"
          preserveAspectRatio="none"
          className="pointer-events-none absolute inset-0 size-full"
        >
          {matched.map((block, position) => (
            <Frame key={position} block={block} order={position} reduceMotion={!!reduceMotion} />
          ))}
        </svg>
      </div>

      <figcaption className="shrink-0 font-mono text-xs text-muted-foreground">
        {hit.path}
        {matched.length > 0 && (
          <span className="ml-2 text-foreground">
            {matched.length} {matched.length === 1 ? "trecho" : "trechos"} no print
          </span>
        )}
      </figcaption>
    </figure>
  )
}

function Frame({
  block,
  order,
  reduceMotion,
}: {
  block: OcrBlock
  order: number
  reduceMotion: boolean
}) {
  const [x, y, width, height] = block.bbox

  return (
    <motion.rect
      x={x}
      y={y}
      width={width}
      height={height}
      fill="hsl(var(--found) / 0.18)"
      stroke="hsl(var(--found))"
      strokeWidth={2}
      vectorEffect="non-scaling-stroke"
      initial={reduceMotion ? { opacity: 0 } : { pathLength: 0, opacity: 1 }}
      animate={reduceMotion ? { opacity: 1 } : { pathLength: 1, opacity: 1 }}
      transition={{ duration: reduceMotion ? 0.15 : 0.3, delay: order * 0.06, ease: "easeOut" }}
    />
  )
}
