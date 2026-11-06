// Renders a macOS app icon on Apple's grid from flat, full-bleed artwork:
// the center of the artwork is cropped to `crop` (fraction of its side),
// drawn into an 824pt rounded tile on a 1024pt canvas with the standard
// soft shadow, and written as a 1024px PNG.
//
//   swift packaging/make_icon.swift <artwork> <out.png> [crop]

import AppKit

let canvas: CGFloat = 1024
let tile: CGFloat = 824
// Corner radius of Apple's macOS icon template for an 824pt tile.
let radius: CGFloat = 185.4
let shadowBlur: CGFloat = 20
let shadowOffset: CGFloat = -10
let shadowAlpha: CGFloat = 0.3

let args = CommandLine.arguments
guard args.count >= 3, let art = NSImage(contentsOfFile: args[1]),
      let src = art.cgImage(forProposedRect: nil, context: nil, hints: nil) else {
    FileHandle.standardError.write("usage: make_icon.swift <artwork> <out.png> [crop]\n".data(using: .utf8)!)
    exit(1)
}
let crop = args.count > 3 ? CGFloat(Double(args[3]) ?? 1) : 1

let side = CGFloat(min(src.width, src.height)) * crop
let cropRect = CGRect(x: (CGFloat(src.width) - side) / 2, y: (CGFloat(src.height) - side) / 2,
                      width: side, height: side)
guard let cropped = src.cropping(to: cropRect),
      let ctx = CGContext(data: nil, width: Int(canvas), height: Int(canvas), bitsPerComponent: 8,
                          bytesPerRow: 0, space: CGColorSpace(name: CGColorSpace.sRGB)!,
                          bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue) else {
    exit(1)
}

let frame = CGRect(x: (canvas - tile) / 2, y: (canvas - tile) / 2, width: tile, height: tile)
let shape = CGPath(roundedRect: frame, cornerWidth: radius, cornerHeight: radius, transform: nil)

ctx.saveGState()
ctx.setShadow(offset: CGSize(width: 0, height: shadowOffset), blur: shadowBlur,
              color: CGColor(gray: 0, alpha: shadowAlpha))
ctx.addPath(shape)
ctx.setFillColor(CGColor(gray: 1, alpha: 1))
ctx.fillPath()
ctx.restoreGState()

ctx.addPath(shape)
ctx.clip()
ctx.interpolationQuality = .high
ctx.draw(cropped, in: frame)

guard let out = ctx.makeImage(),
      let png = NSBitmapImageRep(cgImage: out).representation(using: .png, properties: [:]) else {
    exit(1)
}
try png.write(to: URL(fileURLWithPath: args[2]))
