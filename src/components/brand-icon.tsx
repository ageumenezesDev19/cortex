import { Hash } from "lucide-react"

import { findIcon } from "../lib/icons"
import { cn } from "../lib/utils"

/**
 * The one place a brand colour is allowed to appear. Everywhere else the
 * interface stays neutral, and amber stays reserved for search matches.
 */
export function BrandIcon({
  slug,
  color,
  className,
}: {
  slug: string | null
  color?: string | null
  className?: string
}) {
  const icon = findIcon(slug)

  if (!icon) {
    return <Hash className={cn("size-4 text-muted-foreground", className)} aria-hidden />
  }

  return (
    <svg
      viewBox="0 0 24 24"
      className={cn("size-4", className)}
      fill={color ?? icon.hex}
      aria-hidden
    >
      <path d={icon.path} />
    </svg>
  )
}
