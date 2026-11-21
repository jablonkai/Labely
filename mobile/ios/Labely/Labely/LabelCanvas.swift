import LabelyKit
import SwiftUI

/// The label as the printer will print it (the engine's 1-bit raster), fitted to the space,
/// with the selection and its handles drawn on top. Tap selects; a one-finger drag moves an
/// element (`element.move`) or resizes it by a handle (one `element.set_props`); two fingers
/// zoom and pan. The geometry is `Canvas.swift` in LabelyKit.
struct LabelCanvas: View {
    let ed: EditorState
    @State private var zoom = CanvasZoom()
    @State private var drag: CanvasDrag?
    @State private var translation = CGSize.zero

    var body: some View {
        GeometryReader { geo in
            if let doc = ed.doc {
                let layout = CanvasLayout(viewport: geo.size, labelWidthMm: doc.label.widthMm, labelHeightMm: doc.label.heightMm)
                let zoom = clamped(layout)
                let rect = layout.labelRect(zoom)
                ZStack(alignment: .topTrailing) {
                    ZStack {
                        Color.white
                        if let preview = ed.preview {
                            Image(decorative: preview, scale: 1).resizable().interpolation(.none)
                        }
                    }
                    .frame(width: rect.width, height: rect.height)
                    .clipShape(RoundedRectangle(cornerRadius: min(doc.label.heightMm * 0.04, 12) * zoom.scale))
                    .shadow(color: .black.opacity(0.12), radius: 10, y: 3)
                    .position(x: rect.midX, y: rect.midY)

                    if let sel = ed.selected {
                        let frame = drag?.element.id == sel.id ? drag?.frame(translation, ptPerMm: layout.ptPerMm(zoom)) : nil
                        SelectionFrame(rect: layout.rect(frame ?? sel.frame, zoom), isLine: sel.isLine, handles: Handle.of(sel))
                    }

                    CanvasTouches(
                        onTap: { p in
                            // A field being edited commits for the old selection first.
                            endEditing()
                            ed.selectedId = CanvasHit.element(at: p, in: doc, layout: layout, zoom: zoom)?.id
                        },
                        onDrag: { phase in onDrag(phase, doc, layout, zoom) },
                        onTransform: { factor, pan, anchor in
                            var z = zoom
                            z.zoom(by: factor, around: anchor, in: layout)
                            z.pan(by: pan, in: layout)
                            self.zoom = z
                        }
                    )

                    ZoomBadge(percent: zoom.percent) { self.zoom = CanvasZoom() }
                        .padding(12)
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

    private func onDrag(_ phase: CanvasTouches.Drag, _ doc: Doc, _ layout: CanvasLayout, _ zoom: CanvasZoom) {
        switch phase {
        case .began(let p):
            endEditing()
            translation = .zero
            drag = CanvasHit.drag(at: p, in: doc, selected: ed.selected, layout: layout, zoom: zoom)
            if let drag { ed.selectedId = drag.element.id }
        case .changed(let t):
            translation = t
        case .ended:
            if let drag {
                let e = drag.element
                let f = drag.frame(translation, ptPerMm: layout.ptPerMm(zoom))
                if f != e.frame {
                    switch drag {
                    case .move: ed.exec("element.move", ["id": .int(e.id), "x_mm": JSON(f.x), "y_mm": JSON(f.y)])
                    // The whole frame in one command, so a resize is one undo step.
                    case .resize: ed.setProps(e.id, f.props)
                    }
                }
            }
            drag = nil
            translation = .zero
        case .cancelled:
            drag = nil
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

/// The selection outline with its handles: eight on a box, the two ends on a line, none on a
/// locked or rotated element.
private struct SelectionFrame: View {
    let rect: CGRect
    let isLine: Bool
    let handles: [Handle]

    var body: some View {
        ZStack(alignment: .topLeading) {
            Path { p in
                if isLine {
                    p.move(to: rect.origin)
                    p.addLine(to: CGPoint(x: rect.origin.x + rect.size.width, y: rect.origin.y + rect.size.height))
                } else {
                    let box = rect.standardized
                    p.addRect(CGRect(x: box.minX, y: box.minY, width: max(box.width, 1), height: max(box.height, 1)))
                }
            }
            .stroke(Palette.primary, lineWidth: 1.5)
            ForEach(handles.indices, id: \.self) { i in
                let c = handles[i].point(in: rect)
                Rectangle()
                    .fill(.white)
                    .overlay(Rectangle().stroke(Palette.primary, lineWidth: 1.5))
                    .frame(width: 8, height: 8)
                    .position(x: c.x, y: c.y)
            }
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
        .allowsHitTesting(false)
    }
}

/// The canvas gestures. UIKit recognisers, because SwiftUI has no two-finger pan: one finger
/// taps and drags; two fingers pinch and pan at once, and starting them cancels a drag.
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
        let drag = UIPanGestureRecognizer(target: c, action: #selector(Coordinator.drag))
        drag.maximumNumberOfTouches = 1
        let pinch = UIPinchGestureRecognizer(target: c, action: #selector(Coordinator.pinch))
        let pan = UIPanGestureRecognizer(target: c, action: #selector(Coordinator.pan))
        pan.minimumNumberOfTouches = 2
        pan.maximumNumberOfTouches = 2
        c.oneFinger = drag
        c.twoFingers = [pinch, pan]
        for g in [tap, drag, pinch, pan] as [UIGestureRecognizer] {
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
