// Dibuja el ícono de Kinora.app en todos los tamaños de un .iconset (después lo junta iconutil).
// La "K" angosta y fina en tinta sobre el negro cálido, con la barra ámbar del avance: lo mismo
// que el Player. Uso: swift make-icon.swift <carpeta.iconset>

import AppKit

let folder = URL(fileURLWithPath: CommandLine.arguments[1])
try FileManager.default.createDirectory(at: folder, withIntermediateDirectories: true)

func color(_ hex: UInt32) -> NSColor {
    NSColor(
        srgbRed: CGFloat((hex >> 16) & 0xff) / 255,
        green: CGFloat((hex >> 8) & 0xff) / 255,
        blue: CGFloat(hex & 0xff) / 255,
        alpha: 1
    )
}

func draw(size: Int) -> Data {
    let side = CGFloat(size)
    let image = NSImage(size: NSSize(width: side, height: side), flipped: false) { _ in
        // La grilla de íconos de macOS: el cuadrado ocupa 824 de 1024, con esquinas de 185.
        let inset = side * 100 / 1024
        let tile = NSRect(x: inset, y: inset, width: side - inset * 2, height: side - inset * 2)
        let radius = side * 185 / 1024
        color(0x0d0a08).setFill()
        NSBezierPath(roundedRect: tile, xRadius: radius, yRadius: radius).fill()

        let font = NSFont.systemFont(ofSize: side * 0.56, weight: .light, width: .condensed)
        let letter = NSAttributedString(
            string: "K",
            attributes: [.font: font, .foregroundColor: color(0xe9e4dc)]
        )
        let bounds = letter.size()
        letter.draw(at: NSPoint(x: (side - bounds.width) / 2, y: side * 0.31))

        let bar = NSRect(x: side * 0.33, y: side * 0.255, width: side * 0.34, height: max(1, side * 0.022))
        color(0xec8d38).setFill()
        bar.fill()
        return true
    }
    let rep = NSBitmapImageRep(
        bitmapDataPlanes: nil, pixelsWide: size, pixelsHigh: size, bitsPerSample: 8,
        samplesPerPixel: 4, hasAlpha: true, isPlanar: false, colorSpaceName: .deviceRGB,
        bytesPerRow: 0, bitsPerPixel: 0
    )!
    rep.size = NSSize(width: side, height: side)
    NSGraphicsContext.saveGraphicsState()
    NSGraphicsContext.current = NSGraphicsContext(bitmapImageRep: rep)
    image.draw(in: NSRect(x: 0, y: 0, width: side, height: side))
    NSGraphicsContext.restoreGraphicsState()
    return rep.representation(using: .png, properties: [:])!
}

for base in [16, 32, 128, 256, 512] {
    try draw(size: base).write(to: folder.appendingPathComponent("icon_\(base)x\(base).png"))
    try draw(size: base * 2).write(to: folder.appendingPathComponent("icon_\(base)x\(base)@2x.png"))
}
