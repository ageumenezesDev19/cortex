/**
 * Rust indexes the file in UTF-8 bytes; JavaScript slices strings in UTF-16
 * code units. Slicing with a raw byte offset lands mid-word on the first accent,
 * so every offset crossing the IPC boundary is converted here.
 */
export function byteToCharIndex(source: string, byteOffset: number): number {
  if (byteOffset <= 0) return 0

  let bytes = 0
  let index = 0

  while (index < source.length) {
    if (bytes >= byteOffset) return index
    const codePoint = source.codePointAt(index)!
    bytes += utf8Length(codePoint)
    index += codePoint > 0xffff ? 2 : 1
  }
  return source.length
}

function utf8Length(codePoint: number) {
  if (codePoint < 0x80) return 1
  if (codePoint < 0x800) return 2
  if (codePoint < 0x10000) return 3
  return 4
}
