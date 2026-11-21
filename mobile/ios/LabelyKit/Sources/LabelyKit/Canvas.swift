import CoreGraphics
import Foundation

// The canvas geometry without its view: fitting, pinch zoom and pan, hit testing and the resize
// handles. Points are view points in the canvas viewport; frames are millimetres on the label.

/// Round to 0.1 mm, as moves and resizes are sent.
public func round1(_ v: Double) -> Double {
    (v * 10).rounded() / 10
}

/// An element frame in millimetres. A line runs from `(x, y)` to `(x + width, y + height)`, so
/// its width and height may be negative.
public struct FrameMm: Sendable, Equatable {
    public var x: Double
    public var y: Double
    public var width: Double
    public var height: Double

    public init(x: Double, y: Double, width: Double, height: Double) {
        self.x = x
        self.y = y
        self.width = width
        self.height = height
    }

    /// `props` of `element.set_props`: the whole frame in one command, so one undo step.
    public var props: JSON {
        ["x_mm": JSON(x), "y_mm": JSON(y), "width_mm": JSON(width), "height_mm": JSON(height)]
    }
}

extension Element {
    public var frame: FrameMm { FrameMm(x: xMm, y: yMm, width: widthMm, height: heightMm) }
    public var rotationDeg: Double { raw["rotation_deg"]?.double ?? 0 }
    public var isLine: Bool { type == "line" }
}

/// The label fitted into the canvas viewport.
public struct CanvasLayout: Sendable, Equatable {
    /// Free space kept around the fitted label, in points.
    public static let margin = 24.0

    public let viewport: CGSize
    public let labelWidthMm: Double
    public let labelHeightMm: Double

    public init(viewport: CGSize, labelWidthMm: Double, labelHeightMm: Double) {
        self.viewport = viewport
        self.labelWidthMm = max(labelWidthMm, 0.1)
        self.labelHeightMm = max(labelHeightMm, 0.1)
    }

    /// Points per millimetre when the whole label fits (zoom 100 %).
    public var fitPtPerMm: Double {
        let w = max(viewport.width - 2 * Self.margin, 1)
        let h = max(viewport.height - 2 * Self.margin, 1)
        return min(w / labelWidthMm, h / labelHeightMm)
    }

    public func ptPerMm(_ zoom: CanvasZoom) -> Double { fitPtPerMm * zoom.scale }

    /// Where the label is drawn in the viewport.
    public func labelRect(_ zoom: CanvasZoom) -> CGRect {
        let s = ptPerMm(zoom)
        let w = labelWidthMm * s
        let h = labelHeightMm * s
        return CGRect(
            x: (viewport.width - w) / 2 + zoom.pan.width, y: (viewport.height - h) / 2 + zoom.pan.height,
            width: w, height: h
        )
    }

    /// A viewport point in label millimetres.
    public func mm(_ p: CGPoint, _ zoom: CanvasZoom) -> CGPoint {
        let r = labelRect(zoom)
        let s = ptPerMm(zoom)
        return CGPoint(x: (p.x - r.minX) / s, y: (p.y - r.minY) / s)
    }

    /// A frame in viewport points; a line's rectangle may have a negative size.
    public func rect(_ f: FrameMm, _ zoom: CanvasZoom) -> CGRect {
        let r = labelRect(zoom)
        let s = ptPerMm(zoom)
        return CGRect(x: r.minX + f.x * s, y: r.minY + f.y * s, width: f.width * s, height: f.height * s)
    }
}

/// Pinch zoom and pan of the canvas. 1 is the fitted label and the minimum.
public struct CanvasZoom: Sendable, Equatable {
    public static let maxScale = 8.0

    public private(set) var scale = 1.0
    /// Offset of the label's centre from the viewport's centre, in points.
    public private(set) var pan = CGSize.zero

    public init() {}

    public var percent: Int { Int((scale * 100).rounded()) }
    public var isFit: Bool { scale == 1 && pan == .zero }

    public static func clampScale(_ s: Double) -> Double {
        s.isFinite ? min(max(s, 1), maxScale) : 1
    }

    /// Zoom by `factor`, keeping the label point under `anchor` (the pinch centre) in place.
    public mutating func zoom(by factor: Double, around anchor: CGPoint, in layout: CanvasLayout) {
        let new = Self.clampScale(scale * factor)
        let k = new / scale
        // The anchor relative to the viewport centre, where `pan` is measured from.
        let ax = anchor.x - layout.viewport.width / 2
        let ay = anchor.y - layout.viewport.height / 2
        pan = CGSize(width: ax - (ax - pan.width) * k, height: ay - (ay - pan.height) * k)
        scale = new
        clamp(in: layout)
    }

    public mutating func pan(by d: CGSize, in layout: CanvasLayout) {
        pan = CGSize(width: pan.width + d.width, height: pan.height + d.height)
        clamp(in: layout)
    }

    /// Keep the label on screen: an axis on which the label fits stays centred; a larger one
    /// pans until the label's edge is `CanvasLayout.margin` inside the viewport.
    public mutating func clamp(in layout: CanvasLayout) {
        scale = Self.clampScale(scale)
        let s = layout.fitPtPerMm * scale
        let limX = max((layout.labelWidthMm * s - layout.viewport.width) / 2 + CanvasLayout.margin, 0)
        let limY = max((layout.labelHeightMm * s - layout.viewport.height) / 2 + CanvasLayout.margin, 0)
        pan = CGSize(width: min(max(pan.width, -limX), limX), height: min(max(pan.height, -limY), limY))
    }
}

/// A resize handle of the selection: the eight of a box, or the two ends of a line.
public enum Handle: Sendable, Equatable, CaseIterable {
    case topLeft, top, topRight, left, right, bottomLeft, bottom, bottomRight
    case start, end

    /// Smallest size of a resized element, and length of a line, in millimetres.
    public static let minMm = 1.0
    /// Half of the touch target, in points (the target is 32 pt).
    public static let touchRadius = 16.0

    public static let box: [Handle] = [.topLeft, .top, .topRight, .left, .right, .bottomLeft, .bottom, .bottomRight]
    public static let line: [Handle] = [.start, .end]

    /// The handles `e` shows: none when it is locked or hidden. A turned box keeps its eight
    /// handles, turned with it.
    public static func of(_ e: Element) -> [Handle] {
        if e.locked || e.hidden { return [] }
        return e.isLine ? line : box
    }

    /// Position in the frame, as fractions of its width and height.
    var unit: (x: Double, y: Double) {
        switch self {
        case .topLeft, .start: (0, 0)
        case .top: (0.5, 0)
        case .topRight: (1, 0)
        case .left: (0, 0.5)
        case .right: (1, 0.5)
        case .bottomLeft: (0, 1)
        case .bottom: (0.5, 1)
        case .bottomRight, .end: (1, 1)
        }
    }

    /// Centre of the handle on `rect` (points or millimetres).
    public func point(in rect: CGRect) -> CGPoint {
        let u = unit
        return CGPoint(x: rect.origin.x + rect.size.width * u.x, y: rect.origin.y + rect.size.height * u.y)
    }

    /// `f` after dragging this handle by `(dxMm, dyMm)`: the moved edges are rounded to 0.1 mm
    /// and the size stays at least `minMm`.
    public func resized(_ f: FrameMm, dxMm: Double, dyMm: Double) -> FrameMm {
        switch self {
        case .start, .end: return resizedLine(f, dxMm, dyMm)
        default: break
        }
        var l = f.x
        var t = f.y
        var r = f.x + f.width
        var b = f.y + f.height
        let u = unit
        if u.x == 0 { l = min(round1(l + dxMm), r - Self.minMm) }
        if u.x == 1 { r = max(round1(r + dxMm), l + Self.minMm) }
        if u.y == 0 { t = min(round1(t + dyMm), b - Self.minMm) }
        if u.y == 1 { b = max(round1(b + dyMm), t + Self.minMm) }
        return FrameMm(x: round1(l), y: round1(t), width: round1(r - l), height: round1(b - t))
    }

    private func resizedLine(_ f: FrameMm, _ dx: Double, _ dy: Double) -> FrameMm {
        var x0 = f.x
        var y0 = f.y
        var x1 = f.x + f.width
        var y1 = f.y + f.height
        if self == .start {
            x0 = round1(x0 + dx)
            y0 = round1(y0 + dy)
        } else {
            x1 = round1(x1 + dx)
            y1 = round1(y1 + dy)
        }
        var w = round1(x1 - x0)
        var h = round1(y1 - y0)
        if (w * w + h * h).squareRoot() < Self.minMm {
            // Too short: keep the line's direction at the minimum length.
            let len = (f.width * f.width + f.height * f.height).squareRoot()
            let ux = len > 0 ? f.width / len : 1
            let uy = len > 0 ? f.height / len : 0
            w = round1(ux * Self.minMm)
            h = round1(uy * Self.minMm)
            if self == .start {
                x0 = round1(x1 - w)
                y0 = round1(y1 - h)
            }
        }
        return FrameMm(x: x0, y: y0, width: w, height: h)
    }

    /// The handle of `handles` under `p`, for a selection drawn at `rect` (points). Outside the
    /// frame the target is 32 pt; inside it shrinks on a small element, so its middle still
    /// moves it.
    public static func hit(_ p: CGPoint, rect: CGRect, handles: [Handle]) -> Handle? {
        let box = rect.standardized
        let isLine = handles == line
        let inside = !isLine && box.contains(p)
        let radius = inside ? min(touchRadius, min(box.width, box.height) / 4) : touchRadius
        var best: (Handle, Double)?
        for h in handles {
            let c = h.point(in: rect)
            let d = ((p.x - c.x) * (p.x - c.x) + (p.y - c.y) * (p.y - c.y)).squareRoot()
            if d <= radius, d < (best?.1 ?? .infinity) { best = (h, d) }
        }
        return best?.0
    }
}

/// What a one-finger drag on the canvas does.
public enum CanvasDrag: Sendable, Equatable {
    case move(Element)
    case resize(Element, Handle)

    public var element: Element {
        switch self {
        case .move(let e), .resize(let e, _): e
        }
    }

    /// The element's frame after dragging by `translation` points at `ptPerMm`.
    public func frame(_ translation: CGSize, ptPerMm: Double) -> FrameMm {
        let dx = translation.width / ptPerMm
        let dy = translation.height / ptPerMm
        switch self {
        case .move(let e):
            return FrameMm(x: round1(e.xMm + dx), y: round1(e.yMm + dy), width: e.widthMm, height: e.heightMm)
        case .resize(let e, let h):
            return h.resized(e.frame, dxMm: dx, dyMm: dy)
        }
    }
}

/// Hit testing on the canvas.
public enum CanvasHit {
    /// Touch slop around elements, so thin lines can be picked: 1.5 mm, less when zoomed in.
    public static func slopMm(ptPerMm: Double) -> Double {
        min(1.5, 10 / max(ptPerMm, 0.01))
    }

    /// The top-most visible element under `p`.
    public static func element(at p: CGPoint, in doc: Doc, layout: CanvasLayout, zoom: CanvasZoom) -> Element? {
        let m = layout.mm(p, zoom)
        let slop = slopMm(ptPerMm: layout.ptPerMm(zoom))
        return doc.elements.last { !$0.hidden && $0.contains(xMm: m.x, yMm: m.y, slop: slop) }
    }

    /// What a drag starting at `p` does: resize by a handle of the selection, else move the
    /// selection when the drag starts on it, else move the element under the finger. Locked
    /// elements do not move.
    public static func drag(at p: CGPoint, in doc: Doc, selected: Element?, layout: CanvasLayout, zoom: CanvasZoom) -> CanvasDrag? {
        if let s = selected {
            let handles = Handle.of(s)
            if let h = Handle.hit(p, rect: layout.rect(s.frame, zoom), handles: handles) {
                return .resize(s, h)
            }
        }
        let m = layout.mm(p, zoom)
        let under = element(at: p, in: doc, layout: layout, zoom: zoom)
        let target = selected.flatMap { s in under?.id == s.id || s.contains(xMm: m.x, yMm: m.y) ? s : nil } ?? under
        guard let target, !target.locked else { return nil }
        return .move(target)
    }
}
