// Dibuja lo gráfico de la versión de Windows con la estética de Kinora (la de "Sala": negro
// cálido, tinta de papel, la "K" angosta y fina y la barra ámbar del avance):
//   - los íconos en varios tamaños, que después se juntan en kinora.ico, y el de la bandeja;
//   - las imágenes del instalador: la franja del costado (bienvenida y final, 164×314) y la
//     del encabezado de cada paso (150×57).
// Uso (en la Mac): swift windows/make-assets.swift windows/assets

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

let paper = color(0x0d0a08)
let ink = color(0xe9e4dc)
let muted = color(0x918b85)
let amber = color(0xec8d38)

/// Dibuja en un bitmap de `width`×`height` y lo guarda como PNG o BMP (el BMP, con fondo lleno).
func render(_ name: String, _ width: Int, _ height: Int, draw: (CGFloat, CGFloat) -> Void) throws {
    let rep = NSBitmapImageRep(
        bitmapDataPlanes: nil, pixelsWide: width, pixelsHigh: height, bitsPerSample: 8,
        samplesPerPixel: 4, hasAlpha: true, isPlanar: false, colorSpaceName: .deviceRGB,
        bytesPerRow: 0, bitsPerPixel: 0
    )!
    NSGraphicsContext.saveGraphicsState()
    NSGraphicsContext.current = NSGraphicsContext(bitmapImageRep: rep)
    draw(CGFloat(width), CGFloat(height))
    NSGraphicsContext.restoreGraphicsState()
    guard name.hasSuffix(".bmp") else {
        try rep.representation(using: .png, properties: [:])!.write(to: folder.appendingPathComponent(name))
        return
    }
    // El BMP clásico que esperan los instaladores: 24 bits, sin transparencia, de abajo hacia
    // arriba, cada fila completada a múltiplos de 4 bytes.
    let rowBytes = (width * 3 + 3) / 4 * 4
    var bmp = Data()
    func append<T>(_ value: T) { withUnsafeBytes(of: value) { bmp.append(contentsOf: $0) } }
    bmp.append(contentsOf: [0x42, 0x4D])
    append(UInt32(54 + rowBytes * height).littleEndian)
    append(UInt32(0))
    append(UInt32(54).littleEndian)
    append(UInt32(40).littleEndian)
    append(Int32(width).littleEndian)
    append(Int32(height).littleEndian)
    append(UInt16(1).littleEndian)
    append(UInt16(24).littleEndian)
    append(UInt32(0))
    append(UInt32(rowBytes * height).littleEndian)
    append(Int32(2835).littleEndian)
    append(Int32(2835).littleEndian)
    append(UInt32(0))
    append(UInt32(0))
    for y in (0..<height).reversed() {
        var row = [UInt8](repeating: 0, count: rowBytes)
        for x in 0..<width {
            let pixel = rep.colorAt(x: x, y: y)!.usingColorSpace(.sRGB)!
            row[x * 3] = UInt8(pixel.blueComponent * 255)
            row[x * 3 + 1] = UInt8(pixel.greenComponent * 255)
            row[x * 3 + 2] = UInt8(pixel.redComponent * 255)
        }
        bmp.append(contentsOf: row)
    }
    try bmp.write(to: folder.appendingPathComponent(name))
}

/// La "K" con la barra ámbar, centrada en un cuadrado de lado `side` con origen en (x, y).
func mark(x: CGFloat, y: CGFloat, side: CGFloat) {
    let font = NSFont.systemFont(ofSize: side * 0.56, weight: .light, width: .condensed)
    let letter = NSAttributedString(string: "K", attributes: [.font: font, .foregroundColor: ink])
    let size = letter.size()
    letter.draw(at: NSPoint(x: x + (side - size.width) / 2, y: y + side * 0.31))
    amber.setFill()
    NSRect(x: x + side * 0.33, y: y + side * 0.255, width: side * 0.34, height: max(1, side * 0.022)).fill()
}

// Íconos: el cuadrado de esquinas redondeadas, como el de la Mac. Los chicos van casi al borde.
for side in [16, 24, 32, 48, 64, 128, 256] {
    try render("kinora-\(side).png", side, side) { w, _ in
        let inset = side <= 32 ? 0 : w * 0.06
        let radius = w * (side <= 32 ? 0.2 : 0.18)
        paper.setFill()
        NSBezierPath(roundedRect: NSRect(x: inset, y: inset, width: w - inset * 2, height: w - inset * 2),
                     xRadius: radius, yRadius: radius).fill()
        mark(x: 0, y: 0, side: w)
    }
}

// La franja del costado del asistente: el negro de la sala, la marca y el nombre.
try render("nsis-welcome.bmp", 164, 314) { w, h in
    paper.setFill()
    NSRect(x: 0, y: 0, width: w, height: h).fill()
    mark(x: 22, y: h - 150, side: 120)
    let font = NSFont.systemFont(ofSize: 30, weight: .light, width: .condensed)
    let name = NSAttributedString(string: "Kinora", attributes: [.font: font, .foregroundColor: ink])
    name.draw(at: NSPoint(x: 26, y: 92))
    let small = NSFont.systemFont(ofSize: 11, weight: .regular)
    let line = NSAttributedString(
        string: "Películas y series\nde la casa",
        attributes: [.font: small, .foregroundColor: muted]
    )
    line.draw(in: NSRect(x: 27, y: 40, width: w - 40, height: 40))
}

// El encabezado de cada paso: una tira oscura con la marca, sobre el blanco del asistente.
try render("nsis-header.bmp", 150, 57) { w, h in
    NSColor.white.setFill()
    NSRect(x: 0, y: 0, width: w, height: h).fill()
    paper.setFill()
    NSRect(x: w - 57, y: 0, width: 57, height: h).fill()
    mark(x: w - 57, y: 0, side: 57)
}
