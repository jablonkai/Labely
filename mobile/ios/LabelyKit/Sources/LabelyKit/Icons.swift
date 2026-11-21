import CoreGraphics
import Foundation

/// The stroke icons of the mobile design (`ICONS` in design/gen.py): 24-unit paths, drawn with
/// round caps and a 1.8 stroke. The same list as `Ic` on Android.
public enum Icon: String, CaseIterable, Sendable {
    case home, templates, clock, printer, settings, plus, minus, search
    case chevLeft, chevRight, chevDown, close, check, warn, undo, redo, trash, copy, layers
    case text, barcode, qr, rect, ellipse, line, image, table, border, bold
    case tLeft, tCenter, tRight, up, down, bluetooth, scan, data, open, label, folder, info
    case link, share, download, file
    case lock, unlock, eye, eyeOff, rotate, flipH, flipV, group, ungroup
    case alignLeft, alignCenter, alignRight, alignTop, alignMiddle, alignBottom, distributeH, distributeV, toFront
    case toBack, more, paste, cut, grid, italic, underline, calendar, hash
    case star, pause, play, refresh, magnet, selectAll, ruler, crop, network
    case edit

    public var svg: String {
        switch self {
        case .home: "M3 11l9-7 9 7 M5 9.5V20h14V9.5"
        case .templates: "M4 4h7v7H4z M13 4h7v7h-7z M4 13h7v7H4z M13 13h7v7h-7z"
        case .clock: "M12 3a9 9 0 1 0 0.01 0 M12 7v5l3 2"
        case .printer: "M7 9V3h10v6 M4 9h16v8H4z M7 14h10v7H7z"
        case .settings: "M4 6h9 M17 6h3 M15 4v4 M4 12h3 M11 12h9 M9 10v4 M4 18h11 M19 18h1 M17 16v4"
        case .plus: "M12 5v14 M5 12h14"
        case .minus: "M6 12h12"
        case .search: "M11 4a7 7 0 1 0 0.01 0 M16 16l4 4"
        case .chevLeft: "M15 6l-6 6 6 6"
        case .chevRight: "M9 6l6 6-6 6"
        case .chevDown: "M6 9l6 6 6-6"
        case .close: "M6 6l12 12 M18 6L6 18"
        case .check: "M5 12l5 5 9-10"
        case .warn: "M12 4l9 16H3z M12 10v4 M12 17h.01"
        case .undo: "M9 14L4 9l5-5 M4 9h10a5.5 5.5 0 0 1 0 11h-3"
        case .redo: "M15 14l5-5-5-5 M20 9H10a5.5 5.5 0 0 0 0 11h3"
        case .trash: "M4 7h16 M9 7V4h6v3 M6 7l1 13h10l1-13 M10 11v6 M14 11v6"
        case .copy: "M8 8h12v12H8z M4 16V4h12"
        case .layers: "M12 3l9 5-9 5-9-5z M3 13l9 5 9-5"
        case .text: "M5 7V5h14v2 M12 5v14 M9 19h6"
        case .barcode: "M4 5v14 M7 5v14 M10 5v14 M14 5v14 M16 5v14 M20 5v14"
        case .qr: "M4 4h6v6H4z M14 4h6v6h-6z M4 14h6v6H4z M14 14h2v2h-2z M18 18h2v2h-2z M18 14h2 M14 19h1"
        case .rect: "M4 6h16v12H4z"
        case .ellipse: "M4 12a8 6.5 0 1 0 16 0a8 6.5 0 1 0 -16 0"
        case .line: "M5 19L19 5"
        case .image: "M4 5h16v14H4z M4 16l5-5 4 4 3-3 4 4 M15 8a1.5 1.5 0 1 0 0.01 0"
        case .table: "M4 5h16v14H4z M4 10h16 M4 15h16 M10 5v14"
        case .border: "M3 6h18v12H3z M6 9h12v6H6z"
        case .bold: "M7 5h6a3.5 3.5 0 0 1 0 7H7z M7 12h7a3.5 3.5 0 0 1 0 7H7z"
        case .tLeft: "M4 6h16 M4 10h10 M4 14h16 M4 18h10"
        case .tCenter: "M4 6h16 M7 10h10 M4 14h16 M7 18h10"
        case .tRight: "M4 6h16 M10 10h10 M4 14h16 M10 18h10"
        case .up: "M12 19V5 M6 11l6-6 6 6"
        case .down: "M12 5v14 M6 13l6 6 6-6"
        case .bluetooth: "M7 7l10 10-5 4V3l5 4L7 17"
        case .scan: "M4 8V4h4 M16 4h4v4 M20 16v4h-4 M8 20H4v-4 M4 12h16"
        case .data:
            "M4 6c0-1.7 3.6-3 8-3s8 1.3 8 3-3.6 3-8 3-8-1.3-8-3z M4 6v12c0 1.7 3.6 3 8 3s8-1.3 8-3V6 M4 12c0 1.7 3.6 3 8 3s8-1.3 8-3"
        case .open: "M3 7V5h6l2 2h10v12H3z M3 11h18"
        case .label: "M3 6h18v12H3z M6 9h12v6H6z"
        case .folder: "M3 6h6l2 2h10v11H3z"
        case .info: "M12 3a9 9 0 1 0 0.01 0 M12 11v6 M12 7.5h.01"
        case .link: "M10 14a4 4 0 0 0 5.7 0l3-3a4 4 0 0 0-5.7-5.7l-1 1 M14 10a4 4 0 0 0-5.7 0l-3 3a4 4 0 0 0 5.7 5.7l1-1"
        case .share: "M12 15V4 M8 8l4-4 4 4 M5 13v7h14v-7"
        case .download: "M12 4v11 M8 11l4 4 4-4 M5 17v3h14v-3"
        case .file: "M6 3h8l4 4v14H6z M14 3v4h4"
        case .lock: "M6 11h12v9H6z M8 11V8a4 4 0 0 1 8 0v3"
        case .unlock: "M6 11h12v9H6z M8 11V8a4 4 0 0 1 7.5-2"
        case .eye: "M2 12c3-5 6.5-7 10-7s7 2 10 7c-3 5-6.5 7-10 7s-7-2-10-7z M12 9a3 3 0 1 0 0.01 0"
        case .eyeOff: "M4 4l16 16 M6.5 7.5C4.8 8.6 3.3 10.1 2 12c3 5 6.5 7 10 7 1.6 0 3.1-.4 4.5-1.2 M10 5.3c.7-.2 1.3-.3 2-.3 3.5 0 7 2 10 7-.8 1.3-1.6 2.4-2.5 3.3"
        case .rotate: "M19.5 9A8 8 0 1 0 20 12 M20 4v5h-5"
        case .flipH: "M12 3v18 M9 7l-5 5 5 5z M15 7l5 5-5 5z"
        case .flipV: "M3 12h18 M7 9l5-5 5 5z M7 15l5 5 5-5z"
        case .group: "M4 8V4h4 M16 4h4v4 M20 16v4h-4 M8 20H4v-4 M8 8h8v8H8z"
        case .ungroup: "M4 4h9v9H4z M11 11h9v9h-9z"
        case .alignLeft: "M4 3v18 M7 7h10v4H7z M7 13h6v4H7z"
        case .alignCenter: "M12 3v18 M6 7h12v4H6z M8 13h8v4H8z"
        case .alignRight: "M20 3v18 M7 7h10v4H7z M11 13h6v4h-6z"
        case .alignTop: "M3 4h18 M7 7h4v10H7z M13 7h4v6h-4z"
        case .alignMiddle: "M3 12h18 M7 6h4v12H7z M13 8h4v8h-4z"
        case .alignBottom: "M3 20h18 M7 7h4v10H7z M13 11h4v6h-4z"
        case .distributeH: "M4 3v18 M20 3v18 M10 7h4v10h-4z"
        case .distributeV: "M3 4h18 M3 20h18 M7 10h10v4H7z"
        case .toFront: "M5 4h14 M12 20V9 M7 13l5-5 5 5"
        case .toBack: "M5 20h14 M12 4v11 M7 11l5 5 5-5"
        case .more: "M5 12h.01 M12 12h.01 M19 12h.01"
        case .paste: "M8 5H6v15h12V5h-2 M9 3h6v4H9z"
        case .cut: "M6 4.5a2.5 2.5 0 1 0 0.01 0 M6 14.5a2.5 2.5 0 1 0 0.01 0 M8 8.5l12 9.5 M8 15.5l12-9.5"
        case .grid: "M4 4h16v16H4z M4 9.3h16 M4 14.7h16 M9.3 4v16 M14.7 4v16"
        case .italic: "M10 5h8 M6 19h8 M14 5l-4 14"
        case .underline: "M7 4v7a5 5 0 0 0 10 0V4 M5 20h14"
        case .calendar: "M4 6h16v14H4z M4 10h16 M8 3v5 M16 3v5"
        case .hash: "M9 4L7 20 M17 4l-2 16 M4 9h16 M4 15h16"
        case .star: "M12 3l2.7 5.8 6.3.8-4.6 4.4 1.2 6.2-5.600-3-5.6 3 1.2-6.2L3 9.6l6.3-.8z"
        case .pause: "M8 5v14 M16 5v14"
        case .play: "M7 5l12 7-12 7z"
        case .refresh: "M20 5v5h-5 M4 19v-5h5 M19.5 10a8 8 0 0 0-14-1.500 M4.5 14a8 8 0 0 0 14 1.500"
        case .magnet: "M6 4v8a6 6 0 0 0 12 0V4h-4v8a2 2 0 0 1-4 0V4z M6 8h4 M14 8h4"
        case .selectAll: "M4 4h3 M10.5 4h3 M17 4h3v3 M20 10.5v3 M20 17v3h-3 M13.5 20h-3 M7 20H4v-3 M4 13.5v-3 M4 7V4 M8 8h8v8H8z"
        case .ruler: "M3 8h18v8H3z M7 8v3 M11 8v4 M15 8v3"
        case .crop: "M7 3v14h14 M3 7h14v14"
        case .network: "M4 9a12 12 0 0 1 16 0 M7 12.5a7.5 7.5 0 0 1 10 0 M12 17h.01"
        case .edit: "M4 20l1-4L16 5l3 3L8 19z M14 7l3 3"
        }
    }

    /// The icon as a path in its 24 × 24 box. Parsed once per icon.
    public var path: CGPath {
        Self.cache.withLock { cache in
            if let p = cache[self] { return p }
            let p = (try? SvgPath.parse(svg)) ?? CGMutablePath()
            cache[self] = p
            return p
        }
    }

    private static let cache = LockedCache()
}

/// `CGPath` is immutable, so sharing it between threads is safe.
private final class LockedCache: @unchecked Sendable {
    private let lock = NSLock()
    private var paths: [Icon: CGPath] = [:]

    func withLock<T>(_ body: (inout [Icon: CGPath]) -> T) -> T {
        lock.lock()
        defer { lock.unlock() }
        return body(&paths)
    }
}

/// A parser for the SVG path syntax the design's icons use: M L H V C S A Z, absolute and
/// relative.
public enum SvgPath {
    public struct ParseError: Error, Equatable {
        public let at: Int
    }

    public static func parse(_ d: String) throws -> CGPath {
        var s = Scanner(Array(d.utf8))
        let path = CGMutablePath()
        var current = CGPoint.zero
        var start = CGPoint.zero
        var lastControl: CGPoint?
        var command: UInt8 = 0

        while true {
            s.skipSeparators()
            guard let c = s.peek() else { break }
            if Scanner.isCommand(c) {
                command = c
                s.advance()
            } else if command == 0 {
                throw ParseError(at: s.index)
            }
            let relative = command >= UInt8(ascii: "a")
            let base = relative ? current : .zero
            func point() throws -> CGPoint {
                let x = try s.number()
                let y = try s.number()
                return CGPoint(x: base.x + x, y: base.y + y)
            }
            var control: CGPoint?
            switch command | 0x20 {
            case UInt8(ascii: "m"):
                current = try point()
                start = current
                path.move(to: current)
                // Further pairs after a move are line-tos.
                command = relative ? UInt8(ascii: "l") : UInt8(ascii: "L")
            case UInt8(ascii: "l"):
                current = try point()
                path.addLine(to: current)
            case UInt8(ascii: "h"):
                let x = try s.number()
                current = CGPoint(x: (relative ? current.x : 0) + x, y: current.y)
                path.addLine(to: current)
            case UInt8(ascii: "v"):
                let y = try s.number()
                current = CGPoint(x: current.x, y: (relative ? current.y : 0) + y)
                path.addLine(to: current)
            case UInt8(ascii: "c"):
                let c1 = try point()
                let c2 = try point()
                current = try point()
                path.addCurve(to: current, control1: c1, control2: c2)
                control = c2
            case UInt8(ascii: "s"):
                let c1 = lastControl.map { CGPoint(x: 2 * current.x - $0.x, y: 2 * current.y - $0.y) } ?? current
                let c2 = try point()
                current = try point()
                path.addCurve(to: current, control1: c1, control2: c2)
                control = c2
            case UInt8(ascii: "a"):
                let rx = try s.number()
                let ry = try s.number()
                let rotation = try s.number()
                let large = try s.flag()
                let sweep = try s.flag()
                let end = try point()
                arc(path, from: current, to: end, rx: rx, ry: ry, degrees: rotation, large: large, sweep: sweep)
                current = end
            case UInt8(ascii: "z"):
                path.closeSubpath()
                current = start
                command = 0
            default:
                throw ParseError(at: s.index)
            }
            lastControl = control
        }
        return path
    }

    /// An SVG elliptical arc (SVG 1.1 appendix F.6.5, endpoint to centre parameterisation).
    static func arc(
        _ path: CGMutablePath, from p0: CGPoint, to p1: CGPoint,
        rx: Double, ry: Double, degrees: Double, large: Bool, sweep: Bool
    ) {
        var rx = abs(rx)
        var ry = abs(ry)
        if rx == 0 || ry == 0 || p0 == p1 {
            path.addLine(to: p1)
            return
        }
        let phi = degrees * .pi / 180
        let (cosPhi, sinPhi) = (cos(phi), sin(phi))
        let dx = (p0.x - p1.x) / 2
        let dy = (p0.y - p1.y) / 2
        let x1 = cosPhi * dx + sinPhi * dy
        let y1 = -sinPhi * dx + cosPhi * dy
        // Radii too small for the endpoints are scaled up.
        let lambda = (x1 * x1) / (rx * rx) + (y1 * y1) / (ry * ry)
        if lambda > 1 {
            rx *= lambda.squareRoot()
            ry *= lambda.squareRoot()
        }
        let num = rx * rx * ry * ry - rx * rx * y1 * y1 - ry * ry * x1 * x1
        let den = rx * rx * y1 * y1 + ry * ry * x1 * x1
        var coef = (max(0, num) / den).squareRoot()
        if large == sweep { coef = -coef }
        let cx1 = coef * rx * y1 / ry
        let cy1 = -coef * ry * x1 / rx
        let cx = cosPhi * cx1 - sinPhi * cy1 + (p0.x + p1.x) / 2
        let cy = sinPhi * cx1 + cosPhi * cy1 + (p0.y + p1.y) / 2

        func angle(_ ux: Double, _ uy: Double, _ vx: Double, _ vy: Double) -> Double {
            atan2(ux * vy - uy * vx, ux * vx + uy * vy)
        }
        let theta1 = angle(1, 0, (x1 - cx1) / rx, (y1 - cy1) / ry)
        var delta = angle((x1 - cx1) / rx, (y1 - cy1) / ry, (-x1 - cx1) / rx, (-y1 - cy1) / ry)
        if !sweep && delta > 0 { delta -= 2 * .pi }
        if sweep && delta < 0 { delta += 2 * .pi }

        // A unit circle mapped onto the ellipse; `clockwise: false` runs angles upwards.
        let t = CGAffineTransform(translationX: cx, y: cy).rotated(by: phi).scaledBy(x: rx, y: ry)
        path.addArc(center: .zero, radius: 1, startAngle: theta1, endAngle: theta1 + delta, clockwise: delta < 0, transform: t)
    }

    struct Scanner {
        let bytes: [UInt8]
        var index = 0

        init(_ bytes: [UInt8]) { self.bytes = bytes }

        static func isCommand(_ c: UInt8) -> Bool {
            "MmLlHhVvCcSsAaZz".utf8.contains(c)
        }

        func peek() -> UInt8? { index < bytes.count ? bytes[index] : nil }

        mutating func advance() { index += 1 }

        mutating func skipSeparators() {
            while let c = peek(), c == UInt8(ascii: " ") || c == UInt8(ascii: ",") || c == 0x0A || c == 0x09 {
                index += 1
            }
        }

        /// A number: sign, digits and at most one dot; `0.5.5` is two numbers.
        mutating func number() throws -> Double {
            skipSeparators()
            let begin = index
            if let c = peek(), c == UInt8(ascii: "-") || c == UInt8(ascii: "+") { index += 1 }
            var dot = false
            var digits = false
            while let c = peek() {
                if c >= UInt8(ascii: "0") && c <= UInt8(ascii: "9") {
                    digits = true
                } else if c == UInt8(ascii: "."), !dot {
                    dot = true
                } else {
                    break
                }
                index += 1
            }
            guard digits, let v = Double(String(decoding: bytes[begin..<index], as: UTF8.self)) else {
                throw ParseError(at: begin)
            }
            return v
        }

        /// An arc flag: a single `0` or `1`, which may touch the next number.
        mutating func flag() throws -> Bool {
            skipSeparators()
            guard let c = peek(), c == UInt8(ascii: "0") || c == UInt8(ascii: "1") else { throw ParseError(at: index) }
            index += 1
            return c == UInt8(ascii: "1")
        }
    }
}
