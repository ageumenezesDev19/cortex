import { execFileSync } from "node:child_process"
import { mkdtempSync, readFileSync, writeFileSync } from "node:fs"
import { tmpdir } from "node:os"
import { join } from "node:path"
import { fileURLToPath } from "node:url"
import { deflateSync, inflateSync } from "node:zlib"

/**
 * Turns icon/cortex.svg into the 1024px source `tauri icon` expects.
 *
 * The only SVG rasteriser on a stock Mac is Quick Look, and it paints onto
 * opaque white — which is why the app first shipped with a white square around
 * it in the Dock. So the alpha is not taken from the render: it is computed from
 * the rounded rectangle the artwork is drawn inside, which is known exactly.
 *
 * Edge pixels get their colour from just inside the shape rather than from the
 * render, because a rendered edge is already blended with that white and would
 * leave a pale fringe once made translucent.
 */

// Must match the <rect> in icon/cortex.svg.
const SIZE = 1024
const INSET = 96
const RADIUS = 186
const SAMPLES = 4

// `fileURLToPath`, not `.pathname`: this project sits under a folder with a
// space in its name, and `.pathname` hands the %20 back still encoded.
const root = fileURLToPath(new URL("..", import.meta.url))
const svg = join(root, "icon/cortex.svg")
const out = join(root, "icon/cortex-1024.png")

const scratch = mkdtempSync(join(tmpdir(), "cortex-icon-"))
execFileSync("qlmanage", ["-t", "-s", String(SIZE), "-o", scratch, svg], { stdio: "ignore" })

const { width, height, pixels } = decode(readFileSync(join(scratch, "cortex.svg.png")))
if (width !== SIZE || height !== SIZE) {
  throw new Error(`Quick Look devolveu ${width}×${height}, esperado ${SIZE}`)
}

const centre = SIZE / 2
for (let y = 0; y < height; y += 1) {
  for (let x = 0; x < width; x += 1) {
    const coverage = cover(x, y)
    const at = (y * width + x) * 4

    if (coverage >= 0.999) continue
    if (coverage <= 0.001) {
      pixels[at + 3] = 0
      continue
    }

    // Pull the colour from a few pixels further in, where nothing is blended
    // with the background.
    const inward = 4
    const dx = Math.sign(centre - x)
    const dy = Math.sign(centre - y)
    const source = ((y + dy * inward) * width + (x + dx * inward)) * 4
    pixels[at] = pixels[source]
    pixels[at + 1] = pixels[source + 1]
    pixels[at + 2] = pixels[source + 2]
    pixels[at + 3] = Math.round(coverage * 255)
  }
}

writeFileSync(out, encode(width, height, pixels))
console.log(`escrito ${out} (${SIZE}×${SIZE}, com transparência)`)

/** Fraction of the pixel that falls inside the rounded rectangle. */
function cover(x, y) {
  let inside = 0
  for (let sy = 0; sy < SAMPLES; sy += 1) {
    for (let sx = 0; sx < SAMPLES; sx += 1) {
      const px = x + (sx + 0.5) / SAMPLES
      const py = y + (sy + 0.5) / SAMPLES
      if (isInside(px, py)) inside += 1
    }
  }
  return inside / (SAMPLES * SAMPLES)
}

function isInside(x, y) {
  const min = INSET
  const max = SIZE - INSET
  if (x < min || x > max || y < min || y > max) return false

  // Only the corners are curved; everything else is a plain rectangle.
  const cx = x < min + RADIUS ? min + RADIUS : x > max - RADIUS ? max - RADIUS : null
  const cy = y < min + RADIUS ? min + RADIUS : y > max - RADIUS ? max - RADIUS : null
  if (cx === null || cy === null) return true

  return Math.hypot(x - cx, y - cy) <= RADIUS
}

function decode(file) {
  let offset = 8
  let width = 0
  let height = 0
  const parts = []

  while (offset < file.length) {
    const length = file.readUInt32BE(offset)
    const type = file.toString("ascii", offset + 4, offset + 8)
    const body = file.subarray(offset + 8, offset + 8 + length)

    if (type === "IHDR") {
      width = body.readUInt32BE(0)
      height = body.readUInt32BE(4)
      if (body[8] !== 8 || body[9] !== 6) {
        throw new Error("esperado PNG RGBA de 8 bits")
      }
    }
    if (type === "IDAT") parts.push(body)
    offset += 12 + length
  }

  return { width, height, pixels: unfilter(inflateSync(Buffer.concat(parts)), width, height) }
}

function unfilter(raw, width, height) {
  const bpp = 4
  const stride = width * bpp
  const pixels = Buffer.alloc(height * stride)

  for (let y = 0; y < height; y += 1) {
    const filter = raw[y * (stride + 1)]
    const line = raw.subarray(y * (stride + 1) + 1, (y + 1) * (stride + 1))
    const target = y * stride

    for (let i = 0; i < stride; i += 1) {
      const a = i >= bpp ? pixels[target + i - bpp] : 0
      const b = y > 0 ? pixels[target - stride + i] : 0
      const c = y > 0 && i >= bpp ? pixels[target - stride + i - bpp] : 0
      let value = line[i]

      if (filter === 1) value += a
      else if (filter === 2) value += b
      else if (filter === 3) value += (a + b) >> 1
      else if (filter === 4) value += paeth(a, b, c)
      pixels[target + i] = value & 0xff
    }
  }
  return pixels
}

function paeth(a, b, c) {
  const p = a + b - c
  const pa = Math.abs(p - a)
  const pb = Math.abs(p - b)
  const pc = Math.abs(p - c)
  return pa <= pb && pa <= pc ? a : pb <= pc ? b : c
}

function encode(width, height, pixels) {
  const stride = width * 4
  const raw = Buffer.alloc(height * (stride + 1))
  for (let y = 0; y < height; y += 1) {
    raw[y * (stride + 1)] = 0
    pixels.copy(raw, y * (stride + 1) + 1, y * stride, (y + 1) * stride)
  }

  const header = Buffer.alloc(13)
  header.writeUInt32BE(width, 0)
  header.writeUInt32BE(height, 4)
  header[8] = 8
  header[9] = 6

  return Buffer.concat([
    Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]),
    chunk("IHDR", header),
    chunk("IDAT", deflateSync(raw)),
    chunk("IEND", Buffer.alloc(0)),
  ])
}

function chunk(type, body) {
  const length = Buffer.alloc(4)
  length.writeUInt32BE(body.length)
  const payload = Buffer.concat([Buffer.from(type, "ascii"), body])
  const crc = Buffer.alloc(4)
  crc.writeUInt32BE(crc32(payload) >>> 0)
  return Buffer.concat([length, payload, crc])
}

function crc32(buffer) {
  let crc = ~0
  for (const byte of buffer) {
    crc ^= byte
    for (let bit = 0; bit < 8; bit += 1) {
      crc = (crc >>> 1) ^ (0xedb88320 & -(crc & 1))
    }
  }
  return ~crc
}