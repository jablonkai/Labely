import LabelyKit
import SwiftUI

/// The label as the printer will print it (the engine's 1-bit raster), fitted to the space,
/// with the selection and its handles drawn on top. Tap selects (a long press starts selecting
/// several); a one-finger drag moves the selection with snapping, resizes or turns it by a
/// handle, or draws a selection rectangle on empty space; two fingers zoom and pan. The
/// geometry is `Canvas.swift` and `Geometry.swift` in LabelyKit.
struct LabelCanvas: View {
    let ed: EditorState
    @State private var zoom = CanvasZoom()
    @State private var gesture: CanvasGesture?
    /// Where the finger went down, in label millimetres.
    @State private var start = CGPoint.zero
    @State private var translation = CGSize.zero

    /// What the drag in progress shows before it is committed.
    private struct Live {
        var frames: [Int64: RotatedFrame] = [:]
        var guides = Snapped(dx: 0, dy: 0)
        var marquee: BoundsMm?
        var angle: Double?
    }

    var body: some View {
        GeometryReader { geo in
            if let doc = ed.doc {
                let prefs = ed.app.prefs
                let layout = CanvasLayout(viewport: geo.size, labelWidthMm: doc.label.widthMm, labelHeightMm: doc.label.heightMm)
                let zoom = clamped(layout)
                let rect = layout.labelRect(zoom)
                let s = layout.ptPerMm(zoom)
                let live = live(doc, s, prefs)
                let radius = doc.label.cornerRadiusMm > 0 ? doc.label.cornerRadiusMm * s : min(doc.label.heightMm * 0.04, 12) * zoom.scale
                ZStack(alignment: .topLeading) {
                    ZStack {
                        Color(hex: UInt32(prefs.labelColor, radix: 16) ?? 0xFFFFFF)
                        if let preview = ed.preview {
                            // The raster is black on white: multiplied, the label colour shows through.
                            Image(decorative: preview, scale: 1).resizable().interpolation(.none).blendMode(.multiply)
                        }
                        if let span = printable(doc) {
                            OutsideHead(leftMm: span.0, rightMm: span.1, widthMm: doc.label.widthMm)
                        }
                        if prefs.showGrid { GridLines(stepMm: prefs.gridMm, ptPerMm: s) }
                    }
                    .frame(width: rect.width, height: rect.height)
                    .clipShape(RoundedRectangle(cornerRadius: radius))
                    .shadow(color: .black.opacity(0.12), radius: 10, y: 3)
                    .position(x: rect.midX, y: rect.midY)

                    Guides(snapped: live.guides, labelRect: rect, ptPerMm: s)

                    let selected = ed.selectedElements
                    ForEach(selected) { e in
                        SelectionFrame(
                            frame: live.frames[e.id] ?? e.rotated, isLine: e.isLine, origin: rect.origin, ptPerMm: s,
                            handles: selected.count == 1 ? Handle.of(e) : [],
                            rotateHandle: selected.count == 1 && CanvasGesture.canRotate(e),
                            locked: e.locked
                        )
                    }
                    if let m = live.marquee {
                        Rectangle()
                            .fill(Palette.primary.opacity(0.08))
                            .overlay(Rectangle().stroke(Palette.primary, style: StrokeStyle(lineWidth: 1, dash: [4, 3])))
                            .frame(width: m.width * s, height: m.height * s)
                            .position(x: rect.minX + m.midX * s, y: rect.minY + m.midY * s)
                            .allowsHitTesting(false)
                    }

                    CanvasTouches(
                        onTap: { p in
                            // A field being edited commits for the old selection first.
                            endEditing()
                            ed.tap(CanvasHit.element(at: p, in: doc, layout: layout, zoom: zoom)?.id)
                        },
                        onLongPress: { p in
                            endEditing()
                            UIImpactFeedbackGenerator(style: .medium).impactOccurred()
                            let hit = CanvasHit.element(at: p, in: doc, layout: layout, zoom: zoom)
                            if let hit, !ed.selection.contains(hit.id) || !ed.multiSelect {
                                ed.multiSelect = true
                                if !ed.selection.contains(hit.id) { ed.tap(hit.id, add: true) }
                            } else if hit == nil {
                                ed.multiSelect.toggle()
                            }
                        },
                        onDrag: { phase in onDrag(phase, doc, layout, zoom) },
                        onTransform: { factor, pan, anchor in
                            var z = zoom
                            z.zoom(by: factor, around: anchor, in: layout)
                            z.pan(by: pan, in: layout)
                            self.zoom = z
                        }
                    )

                    if prefs.rulers { Rulers(labelRect: rect, ptPerMm: s, unit: prefs.unit, size: geo.size) }

                    HStack(alignment: .top) {
                        if ed.multiSelect || ed.selection.count > 1 { SelectionBadge(ed: ed) }
                        Spacer()
                        VStack(alignment: .trailing, spacing: 8) {
                            ZoomBadge(percent: zoom.percent) { self.zoom = CanvasZoom() }
                            if let a = live.angle { AngleBadge(deg: a) }
                        }
                    }
                    .padding(.top, prefs.rulers ? 26 : 12)
                    .padding(.leading, prefs.rulers ? 26 : 12)
                    .padding(.trailing, 12)
                }
                .frame(width: geo.size.width, height: geo.size.height)
            }
        }
        .clipped()
        .background(Palette.canvas)
    }

    /// The zoom, kept valid when the viewport or the label size changed.
    private func clamped(_ layout: CanvasLayout) -> CanvasZoom {
        var z = zoom
        z.clamp(in: layout)
        return z
    }

    /// The part of the label the printhead reaches, when the label is wider than the head.
    private func printable(_ doc: Doc) -> (Double, Double)? {
        guard let id = doc.label.printerModel, let head = ed.app.model(id)?.headMm, head > 0, doc.label.widthMm > head + 0.01 else { return nil }
        let left = (doc.label.widthMm - head) / 2
        return (left, left + head)
    }

    private func live(_ doc: Doc, _ s: Double, _ prefs: Prefs) -> Live {
        var live = Live()
        guard let gesture else { return live }
        let dx = translation.width / s
        let dy = translation.height / s
        switch gesture {
        case .move(let els):
            let snapped = snap(els, doc, dx, dy, s, prefs)
            live.guides = snapped
            for e in els {
                var f = e.rotated
                f.x += snapped.dx
                f.y += snapped.dy
                live.frames[e.id] = f
            }
        case .resize(let e, let h):
            live.frames[e.id] = resized(e, h, dx, dy)
        case .rotate(let e):
            let deg = turned(e, dx, dy, prefs)
            var f = e.rotated
            f.rotation = deg
            live.frames[e.id] = f
            live.angle = deg
        case .marquee(let p):
            live.marquee = BoundsMm(minX: min(p.x, p.x + dx), minY: min(p.y, p.y + dy), maxX: max(p.x, p.x + dx), maxY: max(p.y, p.y + dy))
        }
        return live
    }

    private func snap(_ els: [Element], _ doc: Doc, _ dx: Double, _ dy: Double, _ s: Double, _ prefs: Prefs) -> Snapped {
        guard prefs.snap, let bounds = BoundsMm.of(els) else { return Snapped(dx: dx, dy: dy) }
        let moving = Set(els.map(\.id))
        return Snap.snap(
            labelWidthMm: doc.label.widthMm, labelHeightMm: doc.label.heightMm,
            others: doc.elements.filter { !$0.hidden && !moving.contains($0.id) }.map(\.bounds),
            bounds: bounds, dx: dx, dy: dy, threshold: Snap.thresholdPt / s, grid: prefs.showGrid ? prefs.gridMm : nil
        )
    }

    private func resized(_ e: Element, _ h: Handle, _ dx: Double, _ dy: Double) -> RotatedFrame {
        if e.isLine || !e.isRotated { return RotatedFrame(h.resized(e.frame, dxMm: dx, dyMm: dy), rotation: e.rotationDeg) }
        let from = e.rotated.point(of: h)
        return e.rotated.resized(h, to: CGPoint(x: from.x + dx, y: from.y + dy))
    }

    private func turned(_ e: Element, _ dx: Double, _ dy: Double, _ prefs: Prefs) -> Double {
        Rotation.dragged(
            start: e.rotationDeg, center: e.rotated.center, from: start, to: CGPoint(x: start.x + dx, y: start.y + dy),
            snap: prefs.rotateSnap
        )
    }

    private func onDrag(_ phase: CanvasTouches.Drag, _ doc: Doc, _ layout: CanvasLayout, _ zoom: CanvasZoom) {
        let s = layout.ptPerMm(zoom)
        let prefs = ed.app.prefs
        switch phase {
        case .began(let p):
            endEditing()
            translation = .zero
            start = layout.mm(p, zoom)
            gesture = CanvasGesture.begin(at: start, in: doc, selection: ed.selectedElements, ptPerMm: s, marquee: true)
            // Dragging an element that is not selected selects it (with its group).
            if case .move(let els) = gesture, !els.contains(where: { ed.selection.contains($0.id) }) {
                ed.selection = els.map(\.id)
            }
        case .changed(let t):
            translation = t
        case .ended:
            let dx = translation.width / s
            let dy = translation.height / s
            switch gesture {
            case .move(let els):
                let snapped = snap(els, doc, dx, dy, s, prefs)
                if abs(snapped.dx) >= 0.05 || abs(snapped.dy) >= 0.05 { ed.move(els, dx: snapped.dx, dy: snapped.dy) }
            case .resize(let e, let h):
                let f = resized(e, h, dx, dy).frame
                // The whole frame in one command, so a resize is one undo step.
                if f != e.frame { ed.setProps(e.id, f.props) }
            case .rotate(let e):
                let deg = turned(e, dx, dy, prefs)
                if abs(deg - e.rotationDeg) > 0.01 { ed.rotate(e, to: deg) }
            case .marquee(let p):
                let box = BoundsMm(minX: min(p.x, p.x + dx), minY: min(p.y, p.y + dy), maxX: max(p.x, p.x + dx), maxY: max(p.y, p.y + dy))
                let hits = CanvasHit.marquee(box, in: doc)
                let base = ed.multiSelect ? ed.selection : []
                ed.select(base + hits.filter { !base.contains($0) })
                if ed.selection.count > 1 { ed.multiSelect = true }
            case nil: break
            }
            gesture = nil
            translation = .zero
        case .cancelled:
            gesture = nil
            translation = .zero
        }
    }
}

/// The zoom percent in the canvas corner; tapping it returns to the fitted label.
private struct ZoomBadge: View {
    let percent: Int
    let action: () -> Void

    var body: some View {
        Button(action: action) {
            Text("\(percent)%")
                .style(.bodyStrong.with(size: 13, color: Palette.text2))
                .monospacedDigit()
                .padding(.horizontal, 12)
                .padding(.vertical, 6)
                .background(Palette.surface.opacity(0.9), in: Capsule())
                .contentShape(Capsule())
        }
        .buttonStyle(.plain)
        .accessibilityLabel(L("zoom_fit"))
        .accessibilityValue("\(percent)%")
    }
}

/// The angle of the element being turned.
private struct AngleBadge: View {
    let deg: Double

    var body: some View {
        Text("\(deg.formatted(.number.precision(.fractionLength(0...1))))°")
            .style(.bodyStrong.with(size: 13, color: .white))
            .monospacedDigit()
            .padding(.horizontal, 12)
            .padding(.vertical, 6)
            .background(Palette.primary, in: Capsule())
            .allowsHitTesting(false)
    }
}

/// "3 selected" while several elements are being picked, with Select all and a way out.
private struct SelectionBadge: View {
    let ed: EditorState

    var body: some View {
        HStack(spacing: 2) {
            Text(L("selected_count", ed.selection.count))
                .style(.bodyStrong.with(size: 13, color: Palette.primaryDark))
                .monospacedDigit()
                .padding(.leading, 12)
            IconButton(.selectAll, tint: Palette.primaryDark, size: 30, label: L("select_all")) { ed.selectAll() }
            IconButton(.close, tint: Palette.primaryDark, size: 30, label: L("done")) { ed.tap(nil) }
        }
        .padding(.trailing, 2)
        .background(Palette.primaryTint, in: Capsule())
        .overlay(Capsule().strokeBorder(Palette.primaryTint2))
    }
}

/// The selection outline with its handles: eight on a box (turned with it), the two ends on a
/// line, and the rotation handle above the top edge.
private struct SelectionFrame: View {
    let frame: RotatedFrame
    let isLine: Bool
    /// The label's top-left corner in the viewport.
    let origin: CGPoint
    let ptPerMm: Double
    let handles: [Handle]
    let rotateHandle: Bool
    let locked: Bool

    private func pt(_ m: CGPoint) -> CGPoint {
        CGPoint(x: origin.x + m.x * ptPerMm, y: origin.y + m.y * ptPerMm)
    }

    private func point(_ h: Handle) -> CGPoint {
        isLine ? pt(h.point(in: CGRect(x: frame.x, y: frame.y, width: frame.width, height: frame.height))) : pt(frame.point(of: h))
    }

    var body: some View {
        let top = pt(frame.toWorld(CGPoint(x: 0, y: -abs(frame.height) / 2)))
        let knob = pt(frame.toWorld(CGPoint(x: 0, y: -abs(frame.height) / 2 - CanvasGesture.rotateOffsetPt / ptPerMm)))
        ZStack(alignment: .topLeading) {
            Path { p in
                if isLine {
                    p.move(to: pt(CGPoint(x: frame.x, y: frame.y)))
                    p.addLine(to: pt(CGPoint(x: frame.x + frame.width, y: frame.y + frame.height)))
                } else {
                    p.addLines(frame.corners.map(pt))
                    p.closeSubpath()
                }
                if rotateHandle {
                    p.move(to: top)
                    p.addLine(to: knob)
                }
            }
            .stroke(Palette.primary, style: StrokeStyle(lineWidth: 1.5, dash: locked ? [4, 3] : []))
            ForEach(handles.indices, id: \.self) { i in
                let c = point(handles[i])
                Rectangle()
                    .fill(.white)
                    .overlay(Rectangle().stroke(Palette.primary, lineWidth: 1.5))
                    .frame(width: 8, height: 8)
                    .rotationEffect(.degrees(isLine ? 0 : frame.rotation))
                    .position(x: c.x, y: c.y)
            }
            if rotateHandle {
                Circle()
                    .fill(.white)
                    .overlay(Circle().stroke(Palette.primary, lineWidth: 1.5))
                    .overlay(LIcon(.rotate, Palette.primary, size: 12))
                    .frame(width: 20, height: 20)
                    .position(x: knob.x, y: knob.y)
            }
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
        .allowsHitTesting(false)
    }
}

/// The lines a drag snapped to, drawn across the label.
private struct Guides: View {
    let snapped: Snapped
    let labelRect: CGRect
    let ptPerMm: Double

    var body: some View {
        Path { p in
            for x in snapped.xs {
                p.move(to: CGPoint(x: labelRect.minX + x * ptPerMm, y: labelRect.minY - 8))
                p.addLine(to: CGPoint(x: labelRect.minX + x * ptPerMm, y: labelRect.maxY + 8))
            }
            for y in snapped.ys {
                p.move(to: CGPoint(x: labelRect.minX - 8, y: labelRect.minY + y * ptPerMm))
                p.addLine(to: CGPoint(x: labelRect.maxX + 8, y: labelRect.minY + y * ptPerMm))
            }
        }
        .stroke(Palette.red, lineWidth: 1)
        .allowsHitTesting(false)
    }
}

/// The grid over the label. Dense grids are thinned so the lines stay at least 6 pt apart.
private struct GridLines: View {
    let stepMm: Double
    let ptPerMm: Double

    var body: some View {
        Canvas { ctx, size in
            guard stepMm > 0, ptPerMm > 0 else { return }
            var step = stepMm * ptPerMm
            while step < 6 { step *= 2 }
            var p = Path()
            var x = step
            while x < size.width - 0.5 {
                p.move(to: CGPoint(x: x, y: 0))
                p.addLine(to: CGPoint(x: x, y: size.height))
                x += step
            }
            var y = step
            while y < size.height - 0.5 {
                p.move(to: CGPoint(x: 0, y: y))
                p.addLine(to: CGPoint(x: size.width, y: y))
                y += step
            }
            ctx.stroke(p, with: .color(.black.opacity(0.11)), lineWidth: 0.5)
        }
        .allowsHitTesting(false)
    }
}

/// The strips of a too-wide label that the printhead does not reach.
private struct OutsideHead: View {
    let leftMm: Double
    let rightMm: Double
    let widthMm: Double

    var body: some View {
        GeometryReader { geo in
            let k = geo.size.width / max(widthMm, 0.1)
            HStack(spacing: 0) {
                Palette.amber.opacity(0.22).frame(width: leftMm * k)
                Color.clear
                Palette.amber.opacity(0.22).frame(width: (widthMm - rightMm) * k)
            }
        }
        .allowsHitTesting(false)
        .accessibilityLabel(L("outside_head"))
    }
}

/// Rulers along the top and left edge, in the chosen unit, following zoom and pan.
private struct Rulers: View {
    static let thickness = 18.0

    let labelRect: CGRect
    let ptPerMm: Double
    let unit: LengthUnit
    let size: CGSize

    var body: some View {
        Canvas { ctx, size in
            let t = Self.thickness
            ctx.fill(Path(CGRect(x: 0, y: 0, width: size.width, height: t)), with: .color(Palette.surface.opacity(0.92)))
            ctx.fill(Path(CGRect(x: 0, y: t, width: t, height: size.height - t)), with: .color(Palette.surface.opacity(0.92)))
            let (minor, major) = Ruler.steps(unit: unit, ptPerMm: ptPerMm)
            var ticks = Path()
            func draw(horizontal: Bool) {
                let origin = horizontal ? labelRect.minX : labelRect.minY
                let length = horizontal ? size.width : size.height
                let first = ((t - origin) / ptPerMm / minor).rounded(.up)
                var i = first
                while origin + i * minor * ptPerMm <= length {
                    let mm = i * minor
                    let pos = origin + mm * ptPerMm
                    let isMajor = abs((mm / major).rounded() - mm / major) < 1e-6
                    let len = isMajor ? t * 0.6 : t * 0.3
                    if horizontal {
                        ticks.move(to: CGPoint(x: pos, y: t - len))
                        ticks.addLine(to: CGPoint(x: pos, y: t))
                    } else {
                        ticks.move(to: CGPoint(x: t - len, y: pos))
                        ticks.addLine(to: CGPoint(x: t, y: pos))
                    }
                    if isMajor {
                        let text = Text(unit.format(mm)).font(.system(size: 8)).foregroundStyle(Palette.muted)
                        if horizontal {
                            ctx.draw(text, at: CGPoint(x: pos + 2, y: 5), anchor: .leading)
                        } else {
                            ctx.draw(text, at: CGPoint(x: 1, y: pos + 6), anchor: .leading)
                        }
                    }
                    i += 1
                }
            }
            draw(horizontal: true)
            draw(horizontal: false)
            ctx.stroke(ticks, with: .color(Palette.muted), lineWidth: 0.5)
        }
        .frame(width: size.width, height: size.height)
        .allowsHitTesting(false)
    }
}

/// The canvas gestures. UIKit recognisers, because SwiftUI has no two-finger pan: one finger
/// taps, long-presses and drags; two fingers pinch and pan at once, and starting them cancels a
/// drag.
private struct CanvasTouches: UIViewRepresentable {
    enum Drag {
        /// Where the finger went down.
        case began(CGPoint)
        /// Movement since then.
        case changed(CGSize)
        case ended
        case cancelled
    }

    var onTap: (CGPoint) -> Void
    var onLongPress: (CGPoint) -> Void
    var onDrag: (Drag) -> Void
    /// Zoom factor, pan and the centre of the fingers since the last call.
    var onTransform: (CGFloat, CGSize, CGPoint) -> Void

    func makeCoordinator() -> Coordinator { Coordinator(self) }

    func makeUIView(context: Context) -> UIView {
        let view = UIView()
        view.backgroundColor = .clear
        view.isMultipleTouchEnabled = true
        let c = context.coordinator
        let tap = UITapGestureRecognizer(target: c, action: #selector(Coordinator.tap))
        let hold = UILongPressGestureRecognizer(target: c, action: #selector(Coordinator.hold))
        let drag = UIPanGestureRecognizer(target: c, action: #selector(Coordinator.drag))
        drag.maximumNumberOfTouches = 1
        let pinch = UIPinchGestureRecognizer(target: c, action: #selector(Coordinator.pinch))
        let pan = UIPanGestureRecognizer(target: c, action: #selector(Coordinator.pan))
        pan.minimumNumberOfTouches = 2
        pan.maximumNumberOfTouches = 2
        c.oneFinger = drag
        c.twoFingers = [pinch, pan]
        for g in [tap, hold, drag, pinch, pan] as [UIGestureRecognizer] {
            g.delegate = c
            view.addGestureRecognizer(g)
        }
        return view
    }

    func updateUIView(_ view: UIView, context: Context) {
        context.coordinator.parent = self
    }

    @MainActor
    final class Coordinator: NSObject, UIGestureRecognizerDelegate {
        var parent: CanvasTouches
        var oneFinger: UIPanGestureRecognizer?
        var twoFingers: [UIGestureRecognizer] = []

        init(_ parent: CanvasTouches) {
            self.parent = parent
        }

        @objc func tap(_ g: UITapGestureRecognizer) {
            if g.state == .ended { parent.onTap(g.location(in: g.view)) }
        }

        @objc func hold(_ g: UILongPressGestureRecognizer) {
            if g.state == .began { parent.onLongPress(g.location(in: g.view)) }
        }

        @objc func drag(_ g: UIPanGestureRecognizer) {
            let t = g.translation(in: g.view)
            switch g.state {
            case .began:
                // The recogniser begins after some movement: go back to the touch-down point.
                let p = g.location(in: g.view)
                parent.onDrag(.began(CGPoint(x: p.x - t.x, y: p.y - t.y)))
                parent.onDrag(.changed(CGSize(width: t.x, height: t.y)))
            case .changed: parent.onDrag(.changed(CGSize(width: t.x, height: t.y)))
            case .ended: parent.onDrag(.ended)
            case .cancelled, .failed: parent.onDrag(.cancelled)
            default: break
            }
        }

        @objc func pinch(_ g: UIPinchGestureRecognizer) {
            if g.state == .began { cancelDrag() }
            guard g.state == .began || g.state == .changed, g.numberOfTouches >= 2 else { return }
            parent.onTransform(g.scale, .zero, g.location(in: g.view))
            g.scale = 1
        }

        @objc func pan(_ g: UIPanGestureRecognizer) {
            if g.state == .began { cancelDrag() }
            guard g.state == .began || g.state == .changed else { return }
            let t = g.translation(in: g.view)
            parent.onTransform(1, CGSize(width: t.x, height: t.y), g.location(in: g.view))
            g.setTranslation(.zero, in: g.view)
        }

        /// A second finger turns a drag into a zoom: the element stays where it was.
        private func cancelDrag() {
            guard let g = oneFinger, g.state == .began || g.state == .changed else { return }
            g.isEnabled = false
            g.isEnabled = true
        }

        func gestureRecognizer(_ g: UIGestureRecognizer, shouldRecognizeSimultaneouslyWith other: UIGestureRecognizer) -> Bool {
            twoFingers.contains(g) && twoFingers.contains(other)
        }
    }
}
