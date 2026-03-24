#!/usr/bin/env swift

import AppKit
import Foundation

struct IconSpec {
    let fileName: String
    let pixels: Int
}

let specs: [IconSpec] = [
    .init(fileName: "icon_16x16.png", pixels: 16),
    .init(fileName: "icon_16x16@2x.png", pixels: 32),
    .init(fileName: "icon_32x32.png", pixels: 32),
    .init(fileName: "icon_32x32@2x.png", pixels: 64),
    .init(fileName: "icon_128x128.png", pixels: 128),
    .init(fileName: "icon_128x128@2x.png", pixels: 256),
    .init(fileName: "icon_256x256.png", pixels: 256),
    .init(fileName: "icon_256x256@2x.png", pixels: 512),
    .init(fileName: "icon_512x512.png", pixels: 512),
    .init(fileName: "icon_512x512@2x.png", pixels: 1024),
]

let outputPath = CommandLine.arguments.dropFirst().first ?? "assets/app.iconset"
let outputURL = URL(fileURLWithPath: outputPath, isDirectory: true)

func color(_ hex: Int, alpha: CGFloat = 1.0) -> NSColor {
    NSColor(
        calibratedRed: CGFloat((hex >> 16) & 0xff) / 255.0,
        green: CGFloat((hex >> 8) & 0xff) / 255.0,
        blue: CGFloat(hex & 0xff) / 255.0,
        alpha: alpha
    )
}

func roundedRectPath(_ rect: CGRect, radius: CGFloat) -> NSBezierPath {
    NSBezierPath(roundedRect: rect, xRadius: radius, yRadius: radius)
}

func fillBackground(in rect: CGRect) {
    let basePath = roundedRectPath(rect, radius: rect.width * 0.235)
    let gradient = NSGradient(colors: [
        color(0x10131d),
        color(0x23293b),
        color(0x2d354b),
    ])!
    gradient.draw(in: basePath, angle: -35.0)

    color(0xffa11a, alpha: 0.18).setStroke()
    basePath.lineWidth = max(1.0, rect.width * 0.02)
    basePath.stroke()

    let highlight = NSBezierPath()
    let inset = rect.insetBy(dx: rect.width * 0.10, dy: rect.height * 0.10)
    highlight.appendOval(in: CGRect(
        x: inset.minX,
        y: inset.midY,
        width: inset.width * 0.92,
        height: inset.height * 0.64
    ))
    color(0xffffff, alpha: 0.05).setFill()
    highlight.fill()
}

func strokeRing(in rect: CGRect) {
    let ringRect = rect.insetBy(dx: rect.width * 0.205, dy: rect.height * 0.205)
    let ring = NSBezierPath()
    ring.appendArc(
        withCenter: NSPoint(x: ringRect.midX, y: ringRect.midY),
        radius: ringRect.width * 0.5,
        startAngle: 32,
        endAngle: 328,
        clockwise: false
    )
    ring.lineCapStyle = .round
    ring.lineWidth = rect.width * 0.095
    color(0xff9f1c).setStroke()
    ring.stroke()

    let dotSize = rect.width * 0.085
    let dot = NSBezierPath(ovalIn: CGRect(
        x: rect.midX + rect.width * 0.245 - dotSize * 0.5,
        y: rect.midY + rect.height * 0.295 - dotSize * 0.5,
        width: dotSize,
        height: dotSize
    ))
    color(0xffc45a).setFill()
    dot.fill()
}

func fillBolt(in rect: CGRect) {
    let bolt = NSBezierPath()
    bolt.move(to: NSPoint(x: rect.minX + rect.width * 0.57, y: rect.minY + rect.height * 0.17))
    bolt.line(to: NSPoint(x: rect.minX + rect.width * 0.34, y: rect.minY + rect.height * 0.53))
    bolt.line(to: NSPoint(x: rect.minX + rect.width * 0.49, y: rect.minY + rect.height * 0.53))
    bolt.line(to: NSPoint(x: rect.minX + rect.width * 0.40, y: rect.minY + rect.height * 0.84))
    bolt.line(to: NSPoint(x: rect.minX + rect.width * 0.68, y: rect.minY + rect.height * 0.42))
    bolt.line(to: NSPoint(x: rect.minX + rect.width * 0.53, y: rect.minY + rect.height * 0.42))
    bolt.close()

    let shadow = NSShadow()
    shadow.shadowBlurRadius = rect.width * 0.03
    shadow.shadowOffset = NSSize(width: 0, height: -rect.width * 0.012)
    shadow.shadowColor = color(0x000000, alpha: 0.28)
    shadow.set()

    color(0xfefefe).setFill()
    bolt.fill()
}

func renderIcon(size: Int) -> Data {
    guard let bitmap = NSBitmapImageRep(
        bitmapDataPlanes: nil,
        pixelsWide: size,
        pixelsHigh: size,
        bitsPerSample: 8,
        samplesPerPixel: 4,
        hasAlpha: true,
        isPlanar: false,
        colorSpaceName: .deviceRGB,
        bytesPerRow: 0,
        bitsPerPixel: 0
    ) else {
        fatalError("Failed to allocate bitmap")
    }

    bitmap.size = NSSize(width: size, height: size)

    NSGraphicsContext.saveGraphicsState()
    guard let context = NSGraphicsContext(bitmapImageRep: bitmap) else {
        fatalError("Failed to create graphics context")
    }

    NSGraphicsContext.current = context
    context.cgContext.setAllowsAntialiasing(true)
    context.imageInterpolation = .high

    let canvas = CGRect(x: 0, y: 0, width: size, height: size).insetBy(
        dx: CGFloat(size) * 0.06,
        dy: CGFloat(size) * 0.06
    )

    fillBackground(in: canvas)
    strokeRing(in: canvas)
    fillBolt(in: canvas)

    context.flushGraphics()
    NSGraphicsContext.restoreGraphicsState()

    guard let png = bitmap.representation(using: .png, properties: [:]) else {
        fatalError("Failed to encode PNG")
    }

    return png
}

let fileManager = FileManager.default
try fileManager.createDirectory(at: outputURL, withIntermediateDirectories: true)

for spec in specs {
    let destination = outputURL.appendingPathComponent(spec.fileName)
    let data = renderIcon(size: spec.pixels)
    try data.write(to: destination, options: .atomic)
    print("Wrote \(destination.path)")
}
