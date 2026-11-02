import { readdirSync, readFileSync, statSync } from "node:fs"
import { join } from "node:path"

import {
  altWithWidth,
  headingIndexAt,
  imageWidth,
  roundTripReport,
  survivesRoundTrip,
} from "../src/lib/markdown"

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

// The vault is the person's data, not a fixture: it changes under us, and a note
// that cannot round-trip is a finding to report, not a broken build. The
// assertions below are what actually pass or fail.
console.log("vault:")
for (const path of notes) {
  const report = roundTripReport(readFileSync(path, "utf8"))
  const name = path.replace(`${root}/`, "")

  if (report.survives) {
    console.log(`  ok      ${name}`)
  } else {
    console.log(`  abre no markdown  ${name}`)
    console.log(`     linha ${report.line}: ${report.text}`)
  }
}
console.log()

// The guard has to refuse what it cannot represent, or the save commits the loss.
check("tabela é refusada pela guarda", !survivesRoundTrip("# N\n\n| a | b |\n| - | - |\n| 1 | 2 |\n"))
check("nota simples passa pela guarda", survivesRoundTrip("# N\n\ntexto\n\n## Dois\n\nmais\n"))

// An image's width lives in its alt text. If the serializer ever escaped the
// pipe, the guard would call every sized note lossy and lock it out of the
// editor — so the round trip is asserted, not assumed.
check("imagem com largura passa pela guarda", survivesRoundTrip("![|320](_assets/print.png)\n"))
check(
  "largura com legenda passa pela guarda",
  survivesRoundTrip("![Uma legenda|320](_assets/print.png)\n"),
)
check("a largura é lida do alt", imageWidth("|320") === 320 && imageWidth("legenda") === null)
check("original tira a largura", altWithWidth("legenda|320", null) === "legenda")
check("sem legenda e sem largura o alt é vazio", altWithWidth("|320", null) === null)
check("trocar de tamanho não duplica o sufixo", altWithWidth("|320", 880) === "|880")

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