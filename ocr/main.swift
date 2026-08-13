import CoreGraphics
import Foundation
import ImageIO
import Vision

// Standalone CLI over Apple's Vision framework: Rust cannot call Swift APIs
// directly, so the app spawns this as a sidecar and reads JSON from stdout.
// On-device, free, and no model kept in RAM.

struct Block: Encodable {
    let text: String
    let bbox: [Double]
    let confidence: Double
}

struct OcrResult: Encodable {
    let text: String
    let blocks: [Block]
    let lang: String
    let ocr_at: String
}

func fail(_ message: String) -> Never {
    FileHandle.standardError.write(Data((message + "\n").utf8))
    exit(1)
}

let arguments = CommandLine.arguments
guard arguments.count > 1 else { fail("usage: cortex-ocr <image>") }

let url = URL(fileURLWithPath: arguments[1])
guard let source = CGImageSourceCreateWithURL(url as CFURL, nil),
      let image = CGImageSourceCreateImageAtIndex(source, 0, nil)
else {
    fail("could not read image at \(url.path)")
}

let languages = ["pt-BR", "en-US"]
let request = VNRecognizeTextRequest()
request.recognitionLevel = .accurate
request.usesLanguageCorrection = true
request.recognitionLanguages = languages

do {
    try VNImageRequestHandler(cgImage: image, options: [:]).perform([request])
} catch {
    fail("recognition failed: \(error.localizedDescription)")
}

var blocks: [Block] = []
var lines: [String] = []

for observation in request.results ?? [] {
    guard let candidate = observation.topCandidates(1).first else { continue }
    let box = observation.boundingBox
    // Vision measures from the bottom-left, CSS from the top-left. Flipping it
    // here means nothing downstream has to know which convention it holds.
    blocks.append(
        Block(
            text: candidate.string,
            bbox: [box.origin.x, 1 - box.origin.y - box.height, box.width, box.height],
            confidence: Double(candidate.confidence)
        )
    )
    lines.append(candidate.string)
}

let result = OcrResult(
    text: lines.joined(separator: "\n"),
    blocks: blocks,
    lang: languages[0],
    ocr_at: ISO8601DateFormatter().string(from: Date())
)

do {
    FileHandle.standardOutput.write(try JSONEncoder().encode(result))
} catch {
    fail("could not encode result: \(error.localizedDescription)")
}
