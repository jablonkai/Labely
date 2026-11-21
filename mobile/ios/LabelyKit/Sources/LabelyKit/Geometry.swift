import CoreGraphics
import Foundation

// Selection and arrange geometry without views: rotated frames, bounding boxes, snapping, the
// marquee and the rotation handle, as `canvas.rs` in the studio. All in label millimetres.

/// An axis-aligned box in millimetres.
public struct BoundsMm: Sendable, Equatable {
    public var minX: Double
    public var minY: Double
    public var maxX: Double
    public var maxY: Double

    public init(minX: Double, minY: Double, maxX: Double, maxY: Double) {
        self.minX = minX
        self.minY = minY
        self.maxX = maxX
        self.maxY = maxY
    }

    public var width: Double { maxX - minX }
    public var height: Double { maxY - minY }
    public var midX: Double { (minX + maxX) / 2 }
    public var midY: Double { (minY + maxY) / 2 }

    public func union(_ o: BoundsMm) -> BoundsMm {
        BoundsMm(minX: min(minX, o.minX), minY: min(minY, o.minY), maxX: max(maxX, o.maxX), maxY: max(maxY, o.maxY))
    }

    public func intersects(_ o: BoundsMm) -> Bool {
        minX <= o.maxX && maxX >= o.minX && minY <= o.maxY && maxY >= o.minY
    }

    public func offset(dx: Double, dy: Double) -> BoundsMm {
        BoundsMm(minX: minX + dx, minY: minY + dy, maxX: maxX + dx, maxY: maxY + dy)
    }

    /// The box of all `elements`; `nil` for none.
    public static func of(_ elements: [Element]) -> BoundsMm? {
        elements.map(\.bounds).reduce(nil) { acc, b in acc?.union(b) ?? b }
    }
}

/// A frame turned about its centre by `rotation` degrees, clockwise.
public struct RotatedFrame: Sendable, Equatable {
    /// Smallest side a rotated resize leaves, in millimetres.
    public static let minMm = 0.5

    public var x: Double
    public var y: Double
    public var width: Double
    public var height: Double
    public var rotation: Double

    public init(x: Double, y: Double, width: Double, height: Double, rotation: Double) {
        self.x = x
        self.y = y
        self.width = width
        self.height = height
        self.rotation = rotation
    }

    public init(_ f: FrameMm, rotation: Double) {
        self.init(x: f.x, y: f.y, width: f.width, height: f.height, rotation: rotation)
    }

    public var frame: FrameMm { FrameMm(x: x, y: y, width: width, height: height) }
    public var center: CGPoint { CGPoint(x: x + width / 2, y: y + height / 2) }

    /// A label point in the frame's own coordinates: origin at the centre, not rotated.
    public func toLocal(_ p: CGPoint) -> CGPoint {
        let a = -rotation * .pi / 180
        let (s, c) = (sin(a), cos(a))
        let (dx, dy) = (p.x - center.x, p.y - center.y)
        return CGPoint(x: dx * c - dy * s, y: dx * s + dy * c)
    }

    public func toWorld(_ l: CGPoint) -> CGPoint {
        let a = rotation * .pi / 180
        let (s, c) = (sin(a), cos(a))
        return CGPoint(x: center.x + l.x * c - l.y * s, y: center.y + l.x * s + l.y * c)
    }

    public func contains(_ p: CGPoint, slop: Double = 0) -> Bool {
        let l = toLocal(p)
        return abs(l.x) <= abs(width) / 2 + slop && abs(l.y) <= abs(height) / 2 + slop
    }

    /// The four corners, clockwise from the top left.
    public var corners: [CGPoint] {
        let (hw, hh) = (width / 2, height / 2)
        return [CGPoint(x: -hw, y: -hh), CGPoint(x: hw, y: -hh), CGPoint(x: hw, y: hh), CGPoint(x: -hw, y: hh)].map(toWorld)
    }

    /// The axis-aligned box around the turned frame, as the engine's `Element::bounds`.
    public var bounds: BoundsMm {
        let a = rotation * .pi / 180
        let (s, c) = (sin(a), cos(a))
        let (hw, hh) = (width / 2, height / 2)
        let ex = abs(hw * c) + abs(hh * s)
        let ey = abs(hw * s) + abs(hh * c)
        return BoundsMm(minX: center.x - ex, minY: center.y - ey, maxX: center.x + ex, maxY: center.y + ey)
    }

    /// Where `handle` sits on the turned frame.
    public func point(of handle: Handle) -> CGPoint {
        let u = handle.unit
        return toWorld(CGPoint(x: (u.x - 0.5) * width, y: (u.y - 0.5) * height))
    }

    /// The frame after dragging `handle` to the label point `p`: the opposite side stays where
    /// it is, also on a turned frame.
    public func resized(_ handle: Handle, to p: CGPoint) -> RotatedFrame {
        let l = toLocal(p)
        let u = handle.unit
        func axis(_ unit: Double, _ pos: Double, _ half: Double) -> (size: Double, centre: Double) {
            if unit == 0.5 { return (half * 2, 0) }
            let sign = unit == 0 ? -1.0 : 1.0
            let anchor = -sign * half
            var edge = pos
            // The dragged edge never crosses the fixed one.
            if (edge - anchor) * sign < Self.minMm { edge = anchor + sign * Self.minMm }
            return (abs(edge - anchor), (edge + anchor) / 2)
        }
        let (w, cx) = axis(u.x, l.x, width / 2)
        let (h, cy) = axis(u.y, l.y, height / 2)
        let c = toWorld(CGPoint(x: cx, y: cy))
        return RotatedFrame(x: round1(c.x - w / 2), y: round1(c.y - h / 2), width: round1(w), height: round1(h), rotation: rotation)
    }
}

extension Element {
    public var rotated: RotatedFrame { RotatedFrame(frame, rotation: rotationDeg) }
    public var isRotated: Bool { abs(rotationDeg) > 0.01 }
    public var flipH: Bool { raw["flip_h"]?.bool ?? false }
    public var flipV: Bool { raw["flip_v"]?.bool ?? false }
    /// The group the element belongs to.
    public var group: Int64? { raw["group"]?.int64 }
    /// The element this one is a linked mirror copy of.
    public var mirrorOf: Int64? { raw["mirror"]?["of"]?.int64 }

    /// The axis-aligned box around the element, turned as it is drawn. A line's box is the one
    /// between its two ends.
    public var bounds: BoundsMm {
        if isLine {
            return BoundsMm(
                minX: min(xMm, xMm + widthMm), minY: min(yMm, yMm + heightMm),
                maxX: max(xMm, xMm + widthMm), maxY: max(yMm, yMm + heightMm)
            )
        }
        return rotated.bounds
    }
}

/// A drag snapped to nearby lines, with the guides it snapped to.
public struct Snapped: Sendable, Equatable {
    public var dx: Double
    public var dy: Double
    /// Vertical guide lines (x positions) and horizontal ones (y positions), in millimetres.
    public var xs: [Double] = []
    public var ys: [Double] = []

    public init(dx: Double, dy: Double, xs: [Double] = [], ys: [Double] = []) {
        self.dx = dx
        self.dy = dy
        self.xs = xs
        self.ys = ys
    }
}

public enum Snap {
    /// Snap distance on screen, in points.
    public static let thresholdPt = 6.0

    /// Snap a drag of `bounds` by `(dx, dy)`: its edges and centre go to the label's edges and
    /// centre and to the edges and centres of `others` within `threshold` mm; otherwise the
    /// left and top edge go to the `grid`, when there is one.
    public static func snap(
        labelWidthMm: Double, labelHeightMm: Double, others: [BoundsMm], bounds: BoundsMm,
        dx: Double, dy: Double, threshold: Double, grid: Double?
    ) -> Snapped {
        var cx = [0, labelWidthMm / 2, labelWidthMm]
        var cy = [0, labelHeightMm / 2, labelHeightMm]
        for b in others {
            cx += [b.minX, b.midX, b.maxX]
            cy += [b.minY, b.midY, b.maxY]
        }
        func axis(_ lo: Double, _ hi: Double, _ d: Double, _ candidates: [Double]) -> (Double, Double?) {
            let mine = [lo + d, (lo + hi) / 2 + d, hi + d]
            var best: (off: Double, line: Double)?
            for c in candidates {
                for m in mine where abs(c - m) <= threshold && abs(c - m) < abs(best?.off ?? .infinity) {
                    best = (c - m, c)
                }
            }
            if let best { return (d + best.off, best.line) }
            if let grid, grid > 0 { return (((lo + d) / grid).rounded() * grid - lo, nil) }
            return (d, nil)
        }
        let (sx, gx) = axis(bounds.minX, bounds.maxX, dx, cx)
        let (sy, gy) = axis(bounds.minY, bounds.maxY, dy, cy)
        return Snapped(dx: sx, dy: sy, xs: gx.map { [$0] } ?? [], ys: gy.map { [$0] } ?? [])
    }
}

public enum Rotation {
    /// The step the rotation handle snaps to.
    public static let step = 15.0

    /// `deg` in 0 ..< 360.
    public static func normalised(_ deg: Double) -> Double {
        let r = deg.truncatingRemainder(dividingBy: 360)
        let n = r < 0 ? r + 360 : r
        return n >= 359.95 ? 0 : n
    }

    /// The rotation of a frame about `center` after dragging its rotation handle from `from` to
    /// `to`: the turn between the two directions is added to `start`. With `snap`, it lands on
    /// multiples of 15°. Rounded to 0.1°.
    public static func dragged(start: Double, center: CGPoint, from: CGPoint, to: CGPoint, snap: Bool) -> Double {
        let a0 = atan2(from.y - center.y, from.x - center.x)
        let a1 = atan2(to.y - center.y, to.x - center.x)
        var deg = normalised(start + (a1 - a0) * 180 / .pi)
        if snap { deg = (deg / step).rounded() * step }
        return normalised((deg * 10).rounded() / 10)
    }
}

extension CanvasHit {
    /// Elements the marquee `rect` touches: visible and unlocked ones, as in the studio.
    public static func marquee(_ rect: BoundsMm, in doc: Doc) -> [Int64] {
        doc.elements.filter { !$0.hidden && !$0.locked && $0.bounds.intersects(rect) }.map(\.id)
    }
}

/// What a one-finger drag on the canvas does, with several elements selected or a turned one.
public enum CanvasGesture: Sendable, Equatable {
    /// Move these elements together.
    case move([Element])
    case resize(Element, Handle)
    /// Turn the element by its rotation handle.
    case rotate(Element)
    /// Drag a selection rectangle from this label point.
    case marquee(CGPoint)

    /// Distance of the rotation handle from the frame's top edge, in points.
    public static let rotateOffsetPt = 26.0

    /// Where the rotation handle of `e` is, in label millimetres.
    public static func rotateHandle(_ e: Element, ptPerMm: Double) -> CGPoint {
        let f = e.rotated
        return f.toWorld(CGPoint(x: 0, y: -abs(f.height) / 2 - rotateOffsetPt / max(ptPerMm, 0.01)))
    }

    /// Whether `e` shows a rotation handle: one selected box that may change.
    public static func canRotate(_ e: Element) -> Bool {
        !e.locked && !e.hidden && !e.isLine
    }

    /// What a drag starting at the label point `m` does. With one element selected: its
    /// rotation handle, a resize handle, else a move. A drag starting on a selected element
    /// moves the whole selection; one on another element moves that (with its group). On empty
    /// space it draws a marquee when `marquee` is on.
    public static func begin(
        at m: CGPoint, in doc: Doc, selection: [Element], ptPerMm: Double, marquee: Bool
    ) -> CanvasGesture? {
        let slop = CanvasHit.slopMm(ptPerMm: ptPerMm)
        let radius = Handle.touchRadius / max(ptPerMm, 0.01)
        if selection.count == 1, let s = selection.first {
            if canRotate(s) {
                let h = rotateHandle(s, ptPerMm: ptPerMm)
                if hypot(m.x - h.x, m.y - h.y) <= radius { return .rotate(s) }
            }
            if let h = Handle.hit(m, on: s, radius: radius) { return .resize(s, h) }
        }
        let under = doc.elements.last { !$0.hidden && $0.contains(xMm: m.x, yMm: m.y, slop: slop) }
        let onSelection = selection.contains { $0.id == under?.id || $0.contains(xMm: m.x, yMm: m.y) }
        if onSelection {
            let movable = selection.filter { !$0.locked }
            return movable.isEmpty ? nil : .move(movable)
        }
        if let under {
            let members = doc.members(of: under).filter { !$0.locked }
            return members.isEmpty ? nil : .move(members)
        }
        return marquee ? .marquee(m) : nil
    }
}

extension Handle {
    /// The handle of `e` within `radius` mm of the label point `m`, also on a turned element.
    /// Inside the frame the target shrinks on a small element, so its middle still moves it.
    public static func hit(_ m: CGPoint, on e: Element, radius: Double) -> Handle? {
        let handles = of(e)
        guard !handles.isEmpty else { return nil }
        let f = e.rotated
        let inside = !e.isLine && f.contains(m)
        let r = inside ? min(radius, min(abs(f.width), abs(f.height)) / 4) : radius
        var best: (Handle, Double)?
        for h in handles {
            let c = e.isLine ? h.point(in: CGRect(x: f.x, y: f.y, width: f.width, height: f.height)) : f.point(of: h)
            let d = hypot(m.x - c.x, m.y - c.y)
            if d <= r, d < (best?.1 ?? .infinity) { best = (h, d) }
        }
        return best?.0
    }
}

extension Doc {
    /// `e` with the other members of its group, bottom-first.
    public func members(of e: Element) -> [Element] {
        guard let g = e.group else { return [e] }
        return elements.filter { $0.group == g }
    }

    /// `ids` with the whole group of every member, in document order.
    public func expanded(_ ids: [Int64]) -> [Int64] {
        let groups = Set(elements.filter { ids.contains($0.id) }.compactMap(\.group))
        return elements.filter { ids.contains($0.id) || $0.group.map(groups.contains) == true }.map(\.id)
    }
}
