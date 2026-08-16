import Foundation
import Vision
import AppKit

// Usage: swift ocr.swift <outdir> <image...>
let args = CommandLine.arguments
guard args.count > 2 else { print("usage: swift ocr.swift <outdir> <image...>"); exit(1) }
let outDir = args[1]
let paths = Array(args.dropFirst(2))

for p in paths {
    guard let img = NSImage(contentsOfFile: p) else {
        print("{\"file\":\"\(p)\",\"error\":\"cannot load\"}")
        continue
    }
    var rect = NSRect(origin: .zero, size: img.size)
    guard let cg = img.cgImage(forProposedRect: &rect, context: nil, hints: nil) else { continue }
    let request = VNRecognizeTextRequest()
    request.recognitionLevel = .accurate
    request.usesLanguageCorrection = false
    request.recognitionLanguages = ["zh-Hans", "en-US"]
    let handler = VNImageRequestHandler(cgImage: cg, options: [:])
    try? handler.perform([request])
    var items: [[String: Any]] = []
    for obs in request.results ?? [] {
        guard let cand = obs.topCandidates(1).first else { continue }
        let b = obs.boundingBox // normalized, origin bottom-left
        // convert to top-left pixel coords
        let x = Double(b.origin.x) * Double(cg.width)
        let y = Double(1.0 - b.origin.y - b.size.height) * Double(cg.height)
        let w = Double(b.size.width) * Double(cg.width)
        let h = Double(b.size.height) * Double(cg.height)
        items.append([
            "text": cand.string,
            "conf": cand.confidence,
            "x": Int(x.rounded()),
            "y": Int(y.rounded()),
            "w": Int(w.rounded()),
            "h": Int(h.rounded())
        ])
    }
    let data = try JSONSerialization.data(withJSONObject: [
        "file": p,
        "w": cg.width,
        "h": cg.height,
        "items": items
    ], options: [.prettyPrinted])
    let name = URL(fileURLWithPath: p).deletingPathExtension().lastPathComponent
    let out = URL(fileURLWithPath: outDir).appendingPathComponent(name + ".json")
    try data.write(to: out)
    print("done \(p): \(items.count) items -> \(out.path)")
}
