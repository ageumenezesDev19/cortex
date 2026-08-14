import { motion, useReducedMotion } from "motion/react"
import { useEffect, useState } from "react"

import { assetUrl, getAsset, type OcrBlock } from "../lib/api"
import { matchingBlocks } from "../lib/snippet"

type Props = {
  path: string
  root: string
  /** Boxes already in hand from a search result; loaded by path otherwise. */
  blocks?: OcrBlock[]
  /** What was searched, if anything. Nothing searched, nothing framed. */
  query?: string
}

/**
 * The one expensive animation in the app. The frame Vision drew around the
 * recognised text draws itself around the line you searched for — it explains
 * the product without a caption, which is why nothing else gets to use it.
 */
export function ImageReader({ path, root, blocks, query = "" }: Props) {
  const reduceMotion = useReducedMotion()
  const [loaded, setLoaded] = useState<OcrBlock[]>(blocks ?? [])

  useEffect(() => {
    if (blocks) return setLoaded(blocks)

    let active = true
    getAsset(path)
      .then((asset) => active && setLoaded(asset.blocks))
      .catch(() => active && setLoaded([]))
    return () => {
      active = false
    }
  }, [path, blocks])

  const matched = query.trim() ? matchingBlocks(loaded, query) : []

  return (
    <figure className="flex h-full flex-col gap-4">
      <div className="relative min-h-0 flex-1 overflow-hidden rounded-lg border bg-muted">
        <img
          src={assetUrl(root, path)}
          alt={loaded.map((block) => block.text).join(" ")}
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
        {path}
        {matched.length > 0 && (
          <span className="ml-2 text-foreground">
            {matched.length} {matched.length === 1 ? "trecho" : "trechos"} no print
          </span>
        )}
        {matched.length === 0 && loaded.length > 0 && (
          <span className="ml-2">
            {loaded.length} {loaded.length === 1 ? "linha lida" : "linhas lidas"}
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