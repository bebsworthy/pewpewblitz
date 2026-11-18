// Builds the Android adaptive launcher icon layers from the original PewPew Blitz lockup
// (BRL-06pz). Run from the repository root on macOS:
//
//     swift asset_src/android/build_launcher_icons.swift
//
// Writes `ic_launcher_foreground.png` (the full-color lockup) and `ic_launcher_monochrome.png`
// (the lockup's bright letters, blaster, and bolt as one alpha mask for themed icons) into each
// `mipmap-*dpi` folder of the Android shell. Both are 108 dp adaptive-icon layers with the lockup
// centered inside the 66 dp safe circle; the background layer is a vector drawable.
import AppKit

let lockupPath = "assets/brawler/ui/branding/pewpew-blitz-lockup.png"
let resources = "packages/brawler-android/android/app/src/main/res"
let densities: [(String, CGFloat)] = [
    ("mdpi", 1), ("hdpi", 1.5), ("xhdpi", 2), ("xxhdpi", 3), ("xxxhdpi", 4),
]
/// Adaptive icon canvas and the lockup width inside it, in dp. At 64 dp wide (about 33 dp tall)
/// the letters, blaster, and spark stay inside the 66 dp circle every launcher mask preserves.
let canvasDp: CGFloat = 108
let lockupWidthDp: CGFloat = 64

guard let lockup = NSImage(contentsOfFile: lockupPath),
    let lockupImage = lockup.cgImage(forProposedRect: nil, context: nil, hints: nil)
else {
    fatalError("cannot read \(lockupPath); run from the repository root")
}

func render(size: Int, draw: (CGContext) -> Void) -> CGImage {
    let context = CGContext(
        data: nil, width: size, height: size, bitsPerComponent: 8, bytesPerRow: 0,
        space: CGColorSpace(name: CGColorSpace.sRGB)!,
        bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue)!
    context.interpolationQuality = .high
    draw(context)
    return context.makeImage()!
}

func write(_ image: CGImage, to path: String) {
    let rep = NSBitmapImageRep(cgImage: image)
    guard let data = rep.representation(using: .png, properties: [:]) else {
        fatalError("cannot encode \(path)")
    }
    try! data.write(to: URL(fileURLWithPath: path))
    print("wrote \(path)")
}

/// The lockup's bright artwork as coverage: white and yellow letters, the blaster, and the bolt
/// stay; the blue outline and navy shading drop out.
func monochrome(from foreground: CGImage) -> CGImage {
    let size = foreground.width
    let bytesPerRow = size * 4
    var pixels = [UInt8](repeating: 0, count: bytesPerRow * size)
    let context = CGContext(
        data: &pixels, width: size, height: size, bitsPerComponent: 8, bytesPerRow: bytesPerRow,
        space: CGColorSpace(name: CGColorSpace.sRGB)!,
        bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue)!
    context.draw(foreground, in: CGRect(x: 0, y: 0, width: size, height: size))
    for index in stride(from: 0, to: pixels.count, by: 4) {
        let alpha = Double(pixels[index + 3]) / 255
        guard alpha > 0 else { continue }
        let red = Double(pixels[index]) / 255 / alpha
        let green = Double(pixels[index + 1]) / 255 / alpha
        // Red plus green separates the white and yellow artwork from the blue outline.
        let warmth = (red + green) / 2
        let edge = min(max((warmth - 0.45) / 0.2, 0), 1)
        let coverage = edge * edge * (3 - 2 * edge) * alpha
        let value = UInt8((coverage * 255).rounded())
        pixels[index] = value
        pixels[index + 1] = value
        pixels[index + 2] = value
        pixels[index + 3] = value
    }
    return context.makeImage()!
}

for (folder, scale) in densities {
    let size = Int((canvasDp * scale).rounded())
    let width = lockupWidthDp * scale
    let height = width * CGFloat(lockupImage.height) / CGFloat(lockupImage.width)
    let foreground = render(size: size) { context in
        let rect = CGRect(
            x: (CGFloat(size) - width) / 2, y: (CGFloat(size) - height) / 2, width: width,
            height: height)
        context.draw(lockupImage, in: rect)
    }
    let directory = "\(resources)/mipmap-\(folder)"
    write(foreground, to: "\(directory)/ic_launcher_foreground.png")
    write(monochrome(from: foreground), to: "\(directory)/ic_launcher_monochrome.png")
}
