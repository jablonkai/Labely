import LabelyKit
import SwiftUI

/// The label as the printer will print it (the engine's 1-bit raster), fitted to the space,
/// with the selection drawn on top. Tap selects; dragging an element moves it with
/// `element.move`.
struct LabelCanvas: View {
    /// Extra touch slop around elements, in millimetres, so thin lines can be picked.
    private static let hitSlopMm = 1.5

    let ed: EditorState
    @State private var dragging: Element?
    @State private var drag = CGSize.zero

    var body: some View {
        GeometryReader { geo in
            if let doc = ed.doc {
                let label = doc.label
                let ratio = label.widthMm / label.heightMm
                let w = max(min(geo.size.width - 48, (geo.size.height - 48) * ratio), 1)
                let h = w / ratio
                let pxPerMm = w / label.widthMm
                ZStack(alignment: .topLeading) {
                    Color.white
                    if let preview = ed.preview {
                        Image(decorative: preview, scale: 1).resizable().interpolation(.none)
                    }
                    if let sel = ed.selected {
                        let offset = dragging?.id == sel.id ? drag : .zero
                        SelectionFrame()
                            .frame(width: max(sel.widthMm * pxPerMm, 1), height: max(sel.heightMm * pxPerMm, 1))
                            .offset(x: sel.xMm * pxPerMm + offset.width, y: sel.yMm * pxPerMm + offset.height)
                    }
                }
                .frame(width: w, height: h)
                .clipShape(RoundedRectangle(cornerRadius: min(label.heightMm * 0.04, 12)))
                .shadow(color: .black.opacity(0.12), radius: 10, y: 3)
                .contentShape(Rectangle())
                .onTapGesture { p in
                    // A field being edited commits for the old selection first.
                    endEditing()
                    ed.selectedId = hit(p, pxPerMm)?.id
                }
                .gesture(
                    DragGesture(minimumDistance: 6)
                        .onChanged { v in
                            if dragging == nil {
                                endEditing()
                                let p = v.startLocation
                                let sel = ed.selected
                                let target = sel.flatMap { s in
                                    hit(p, pxPerMm)?.id == s.id || s.contains(xMm: p.x / pxPerMm, yMm: p.y / pxPerMm) ? s : nil
                                } ?? hit(p, pxPerMm)
                                guard let target, !target.locked else { return }
                                ed.selectedId = target.id
                                dragging = target
                            }
                            drag = v.translation
                        }
                        .onEnded { _ in
                            if let e = dragging {
                                let x = round1(e.xMm + drag.width / pxPerMm)
                                let y = round1(e.yMm + drag.height / pxPerMm)
                                ed.exec("element.move", ["id": .int(e.id), "x_mm": JSON(x), "y_mm": JSON(y)])
                            }
                            dragging = nil
                            drag = .zero
                        }
                )
                .frame(maxWidth: .infinity, maxHeight: .infinity)
            }
        }
        .background(Palette.canvas)
    }

    /// The top-most visible element under `p` (points from the label's top-left).
    private func hit(_ p: CGPoint, _ pxPerMm: Double) -> Element? {
        let x = p.x / pxPerMm
        let y = p.y / pxPerMm
        return ed.doc?.elements.last { !$0.hidden && $0.contains(xMm: x, yMm: y, slop: Self.hitSlopMm) }
    }

    private func round1(_ v: Double) -> Double {
        (v * 10).rounded() / 10
    }
}

/// The selection rectangle with its eight handles.
private struct SelectionFrame: View {
    var body: some View {
        GeometryReader { g in
            let w = g.size.width
            let h = g.size.height
            ZStack(alignment: .topLeading) {
                Rectangle().stroke(Palette.primary, lineWidth: 1.5)
                ForEach(0..<9) { i in
                    if i != 4 {
                        Rectangle()
                            .fill(.white)
                            .overlay(Rectangle().stroke(Palette.primary, lineWidth: 1.5))
                            .frame(width: 8, height: 8)
                            .position(x: w * Double(i % 3) / 2, y: h * Double(i / 3) / 2)
                    }
                }
            }
        }
        .allowsHitTesting(false)
    }
}
