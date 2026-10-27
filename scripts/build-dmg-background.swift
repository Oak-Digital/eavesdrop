// Regenerate with: swift scripts/build-dmg-background.swift assets/dmg-background.tiff
// Draws the installation artwork only; Finder supplies the two draggable icons.
import AppKit

guard CommandLine.arguments.count == 2 else {
    fatalError("Usage: swift build-dmg-background.swift output.tiff")
}

let width = 640
let height = 360
func render(scale: Int) -> NSBitmapImageRep {
    let bitmap = NSBitmapImageRep(
        bitmapDataPlanes: nil, pixelsWide: width * scale, pixelsHigh: height * scale,
        bitsPerSample: 8, samplesPerPixel: 4, hasAlpha: true,
        isPlanar: false, colorSpaceName: .deviceRGB, bytesPerRow: 0, bitsPerPixel: 0
    )!
    bitmap.size = NSSize(width: width, height: height)
    NSGraphicsContext.saveGraphicsState()
    NSGraphicsContext.current = NSGraphicsContext(bitmapImageRep: bitmap)
    NSColor(calibratedWhite: 0.975, alpha: 1).setFill()
    NSRect(x: 0, y: 0, width: width, height: height).fill()

    func text(_ value: String, top: CGFloat, size: CGFloat, weight: NSFont.Weight, gray: CGFloat) {
        let paragraph = NSMutableParagraphStyle()
        paragraph.alignment = .center
        (value as NSString).draw(
            in: NSRect(x: 24, y: CGFloat(height) - top - 32, width: 592, height: 32),
            withAttributes: [
                .font: NSFont.systemFont(ofSize: size, weight: weight),
                .foregroundColor: NSColor(calibratedWhite: gray, alpha: 1),
                .paragraphStyle: paragraph,
            ]
        )
    }

    text("Install Eavesdrop", top: 34, size: 25, weight: .semibold, gray: 0.10)
    text("Drag Eavesdrop to the Applications folder.", top: 73, size: 14, weight: .regular, gray: 0.32)

    // The icon centers are (170, 180) and (470, 180) in Finder's top-down space.
    let arrow = NSBezierPath()
    arrow.lineWidth = 3
    arrow.lineCapStyle = .round
    arrow.lineJoinStyle = .round
    arrow.move(to: NSPoint(x: 284, y: 180))
    arrow.line(to: NSPoint(x: 356, y: 180))
    arrow.move(to: NSPoint(x: 344, y: 192))
    arrow.line(to: NSPoint(x: 356, y: 180))
    arrow.line(to: NSPoint(x: 344, y: 168))
    NSColor(calibratedWhite: 0.45, alpha: 1).setStroke()
    arrow.stroke()

    text("When copying finishes, eject this disk image.", top: 283, size: 12, weight: .regular, gray: 0.38)
    text("Open Eavesdrop from Applications to get started.", top: 304, size: 12, weight: .regular, gray: 0.38)
    NSGraphicsContext.restoreGraphicsState()
    return bitmap
}

let image = NSImage(size: NSSize(width: width, height: height))
image.addRepresentation(render(scale: 1))
image.addRepresentation(render(scale: 2))
try image.tiffRepresentation(using: .lzw, factor: 1)!.write(
    to: URL(fileURLWithPath: CommandLine.arguments[1])
)
