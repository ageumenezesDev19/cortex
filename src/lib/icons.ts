import {
  siAndroid,
  siAngular,
  siApple,
  siAstro,
  siBun,
  siC,
  siCloudflare,
  siCplusplus,
  siCss,
  siCypress,
  siDeno,
  siDocker,
  siDrizzle,
  siEslint,
  siExpo,
  siExpress,
  siFastapi,
  siFigma,
  siFirebase,
  siFlutter,
  siGit,
  siGithub,
  siGo,
  siGraphql,
  siHtml5,
  siJavascript,
  siJest,
  siKubernetes,
  siLinux,
  siMongodb,
  siMysql,
  siNestjs,
  siNextdotjs,
  siNodedotjs,
  siNpm,
  siPhp,
  siPostgresql,
  siPrisma,
  siPython,
  siReact,
  siRedis,
  siRedux,
  siRust,
  siSass,
  siSqlite,
  siStorybook,
  siStripe,
  siSupabase,
  siSwift,
  siTailwindcss,
  siTauri,
  siTestinglibrary,
  siTypescript,
  siVercel,
  siVite,
  siVitest,
  siVuedotjs,
  siWebassembly,
  siZod,
} from "simple-icons"

export type BrandIcon = {
  slug: string
  title: string
  /** SVG path data, drawn in a 24×24 viewBox. */
  path: string
  hex: string
}

/**
 * A curated set rather than all ~3300 Simple Icons: named imports let the
 * bundler drop the rest, and a picker over sixty relevant icons beats one over
 * three thousand. Adding a stack is one line.
 */
const SOURCE = [
  siReact, siNextdotjs, siTypescript, siJavascript, siNodedotjs, siRust, siTauri,
  siTailwindcss, siVite, siVitest, siJest, siTestinglibrary, siCypress,
  siPostgresql, siSqlite, siMysql, siMongodb, siRedis, siPrisma, siDrizzle,
  siSupabase, siFirebase, siDocker, siKubernetes, siCloudflare, siVercel,
  siGit, siGithub, siNpm, siBun, siDeno, siEslint, siStorybook, siFigma,
  siHtml5, siCss, siSass, siPython, siGo, siPhp, siC, siCplusplus, siSwift,
  siAndroid, siApple, siLinux, siFlutter, siExpo, siVuedotjs, siAngular,
  siAstro, siNestjs, siExpress, siFastapi, siGraphql, siRedux, siZod, siStripe,
  siWebassembly,
]

export const BRAND_ICONS: BrandIcon[] = SOURCE.map((icon) => ({
  slug: icon.slug,
  title: icon.title,
  path: icon.path,
  hex: `#${icon.hex}`,
}))

const BY_SLUG = new Map(BRAND_ICONS.map((icon) => [icon.slug, icon]))

export const findIcon = (slug: string | null | undefined) =>
  slug ? BY_SLUG.get(slug) ?? null : null

/** Accent-insensitive substring match, same folding the search field uses. */
export function searchIcons(query: string): BrandIcon[] {
  const term = fold(query)
  if (term.length === 0) return BRAND_ICONS
  return BRAND_ICONS.filter(
    (icon) => fold(icon.title).includes(term) || icon.slug.includes(term),
  )
}

function fold(value: string) {
  return value
    .normalize("NFD")
    .replace(/\p{Diacritic}/gu, "")
    .toLowerCase()
}
