import LabelyKit
import SwiftUI

/// Insert an element; it is centred on the label and selected.
struct InsertSheet: View {
    let ed: EditorState
    let onDismiss: () -> Void

    var body: some View {
        let items: [(Icon, String, () -> Void)] = [
            (.text, L("kind_text"), { ed.addText(L("new_text")) }),
            (.barcode, L("kind_barcode"), { ed.addBarcode() }),
            (.qr, L("kind_code2d"), { ed.addQr() }),
            (.rect, L("kind_rect"), { ed.addRect() }),
            (.ellipse, L("kind_ellipse"), { ed.addEllipse() }),
            (.line, L("kind_line"), { ed.addLine() }),
        ]
        SheetBody(title: L("tool_insert"), onClose: onDismiss) {
            LazyVGrid(columns: Array(repeating: GridItem(.flexible(), spacing: 12), count: 3), spacing: 12) {
                ForEach(items.indices, id: \.self) { i in
                    let (icon, label, add) = items[i]
                    Button {
                        add()
                        onDismiss()
                    } label: {
                        VStack(spacing: 8) {
                            LIcon(icon, Palette.primary, size: 28)
                            Text(label).style(.bodyStrong.with(size: 14)).lineLimit(1)
                        }
                        .frame(maxWidth: .infinity)
                        .frame(height: 96)
                        .background(Palette.bg, in: RoundedRectangle(cornerRadius: 16))
                        .contentShape(RoundedRectangle(cornerRadius: 16))
                    }
                    .buttonStyle(.plain)
                }
            }
        }
    }
}

/// M5 · Rétegek: elements top-first; tap selects, arrows reorder.
struct LayersSheet: View {
    let ed: EditorState
    let onDismiss: () -> Void

    var body: some View {
        let elements = Array((ed.doc?.elements ?? []).reversed())
        SheetBody(title: L("tool_layers"), onClose: onDismiss) {
            ListGroup {
                if elements.isEmpty { ListRow(title: L("layers_empty")) }
                ForEach(Array(elements.enumerated()), id: \.element.id) { i, e in
                    if i > 0 { RowDivider() }
                    let on = e.id == ed.selectedId
                    ListRow(title: elementTitle(e), subtitle: kindName(e.type), action: {
                        ed.selectedId = e.id
                        onDismiss()
                    }) {
                        IconTile(kindIcon(e.type), tint: on ? Palette.primary : Palette.text2, fill: on ? Palette.primaryTint : Palette.bg)
                    } trailing: {
                        IconButton(.up, enabled: i > 0, label: "Up") {
                            ed.exec("element.reorder", ["id": .int(e.id), "to": "forward"])
                        }
                        IconButton(.down, enabled: i < elements.count - 1, label: "Down") {
                            ed.exec("element.reorder", ["id": .int(e.id), "to": "backward"])
                        }
                    }
                }
            }
        }
    }
}

/// M11 · Új címke: size presets, custom size, paper type and orientation. Used for a new label
/// and, with another title, for changing the open one.
struct LabelSheet: View {
    let title: String
    let confirm: String
    let onDismiss: () -> Void
    let onConfirm: (Double, Double, String) -> Void
    @State private var w: Double
    @State private var h: Double
    @State private var media: String

    init(
        title: String, confirm: String, initial: (Double, Double, String),
        onDismiss: @escaping () -> Void, onConfirm: @escaping (Double, Double, String) -> Void
    ) {
        self.title = title
        self.confirm = confirm
        self.onDismiss = onDismiss
        self.onConfirm = onConfirm
        _w = State(initialValue: initial.0)
        _h = State(initialValue: initial.1)
        _media = State(initialValue: initial.2)
    }

    var body: some View {
        SheetBody(title: title, onClose: onDismiss) {
            preview
            Text(L("size")).style(.bodyStrong.with(color: Palette.text2))
            LazyVGrid(columns: Array(repeating: GridItem(.flexible(), spacing: 10), count: 3), spacing: 10) {
                ForEach(Array(AppState.presetSizes.enumerated()), id: \.offset) { _, size in
                    preset(size.0, size.1)
                }
            }
            HStack(spacing: 10) {
                NumberField(label: L("width"), value: w, live: true) { if $0 > 0 { w = $0 } }
                NumberField(label: L("height"), value: h, live: true) { if $0 > 0 { h = $0 } }
            }
            ListGroup {
                ListRow(title: L("paper_type")) {
                    Menu {
                        Picker(L("paper_type"), selection: $media) {
                            ForEach(mediaKinds, id: \.self) { Text(mediaName($0)).tag($0) }
                        }
                    } label: {
                        HStack(spacing: 4) {
                            Text(mediaName(media)).style(.body.with(color: Palette.muted))
                            LIcon(.chevRight, Palette.muted, size: 18)
                        }
                    }
                }
                RowDivider()
                ListRow(title: L("orientation")) {
                    Picker(L("orientation"), selection: Binding(get: { w >= h }, set: { landscape in
                        if landscape != (w >= h) { swap(&w, &h) }
                    })) {
                        Text(L("landscape")).tag(true)
                        Text(L("portrait")).tag(false)
                    }
                    .pickerStyle(.segmented)
                    .fixedSize()
                }
            }
            HStack(spacing: 12) {
                SecondaryButton(text: L("cancel"), height: 52) { onDismiss() }
                PrimaryButton(text: confirm) {
                    endEditing()
                    onConfirm(w, h, media)
                }
            }
        }
        .presentationDetents([.large])
    }

    private var preview: some View {
        let scale = min(200 / w, 100 / h)
        return VStack(spacing: 6) {
            Text("\(mm(w)) mm").style(.small)
            HStack(spacing: 8) {
                RoundedRectangle(cornerRadius: 6).fill(Palette.surface).frame(width: w * scale, height: h * scale)
                Text("\(mm(h)) mm").style(.small)
            }
        }
        .frame(maxWidth: .infinity)
        .frame(height: 160)
        .background(Palette.canvas, in: RoundedRectangle(cornerRadius: 16))
    }

    private func preset(_ pw: Double, _ ph: Double) -> some View {
        let on = (pw == w && ph == h) || (pw == h && ph == w)
        let shape = RoundedRectangle(cornerRadius: 12)
        return Button {
            // Keep the chosen orientation.
            if w < h { (w, h) = (ph, pw) } else { (w, h) = (pw, ph) }
        } label: {
            Text(sizeText(pw, ph))
                .style(.bodyStrong.with(size: 14, color: on ? Palette.primaryDark : Palette.text))
                .lineLimit(1)
                .minimumScaleFactor(0.8)
                .frame(maxWidth: .infinity)
                .padding(.vertical, 12)
                .background(on ? Palette.primaryTint : Palette.surface, in: shape)
                .overlay(shape.strokeBorder(on ? Palette.primary : Palette.border, lineWidth: on ? 2 : 1))
        }
        .buttonStyle(.plain)
    }
}
