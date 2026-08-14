import { readdirSync, readFileSync, statSync } from "node:fs"
import { join } from "node:path"

import { headingIndexAt, parse, serialize, survivesRoundTrip } from "../src/lib/markdown"

/**
 * The rich editor rewrites a whole note on save, so anything the schema cannot
 * represent would be written back flattened. This walks the real vault and
 * proves each note either survives the trip or is refused by the guard.
 *
 * Run with `npm run check:markdown`.
 */

const root = join(process.env.HOME ?? "", "Cortex")
let failures = 0

function check(label: string, passed: boolean) {
  console.log(`${passed ? "ok    " : "FALHOU"}  ${label}`)
  if (!passed) failures += 1
}

function notesIn(dir: string): string[] {
  let found: string[] = []
  for (const entry of readdirSync(dir)) {
    if (entry.startsWith(".")) continue
    const path = join(dir, entry)
    if (statSync(path).isDirectory()) found = found.concat(notesIn(path))
    else if (entry.endsWith(".md")) found.push(path)
  }
  return found
}

let notes: string[] = []
try {
  notes = notesIn(root)
} catch {
  console.log(`vault não encontrado em ${root}`)
}

for (const path of notes) {
  const source = readFileSync(path, "utf8")
  const survives = survivesRoundTrip(source)
  check(path.replace(`${root}/`, ""), survives)

  if (!survives) {
    console.log("  --- de volta ---")
    console.log(serialize(parse(source)))
  }
}

// The guard has to refuse what it cannot represent, or the save commits the loss.
check("tabela é refusada pela guarda", !survivesRoundTrip("# N\n\n| a | b |\n| - | - |\n| 1 | 2 |\n"))
check("nota simples passa pela guarda", survivesRoundTrip("# N\n\ntexto\n\n## Dois\n\nmais\n"))

// Heading counting drives the anchor jump, so a `#` inside a fence must not count.
const fenced = "# Um\n\ntexto\n\n```\n# não é título\n```\n\n## Dois\n\nfim\n"
const byteOffset = (text: string, at: number) => Buffer.byteLength(text.slice(0, at))
check("título 0 no começo", headingIndexAt(fenced, 0) === 0)
check(
  "título 1 no segundo heading",
  headingIndexAt(fenced, byteOffset(fenced, fenced.indexOf("## Dois"))) === 1,
)
check(
  "`#` dentro de bloco de código não conta",
  headingIndexAt(fenced, byteOffset(fenced, fenced.indexOf("fim"))) === 2,
)

console.log(failures === 0 ? "\ntudo certo" : `\n${failures} falha(s)`)
process.exit(failures === 0 ? 0 : 1)