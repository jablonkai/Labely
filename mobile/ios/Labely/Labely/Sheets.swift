import LabelyKit
import PhotosUI
import SwiftUI

/// Insert an element; it is centred on the label and selected.
struct InsertSheet: View {
    private enum Photo {
        case image, code
    }

    let ed: EditorState
    let onDismiss: () -> Void
    /// A barcode or 2D code is being added: the editor opens its sheet when it is there.
    let onCode: (Task<Void, Never>) -> Void
    @State private var picking = false
    @State private var kind = Photo.image
    @State private var photo: PhotosPickerItem?
    @State private var icons = false

    var body: some View {
        let basics: [(Icon, String, () -> Void)] = [
            (.text, L("kind_text"), { ed.addText(L("new_text")); onDismiss() }),
            (.barcode, L("kind_barcode"), { onCode(ed.addBarcode()) }),
            (.qr, L("kind_code2d"), { onCode(ed.addQr()) }),
            // The system photo picker; it needs no photo library permission.
            (.image, L("kind_image"), { kind = .image; picking = true }),
            (.table, L("kind_table"), { ed.addTable(); onDismiss() }),
            (.star, L("kind_icon"), { icons = true }),
            (.rect, L("kind_rect"), { ed.addRect(); onDismiss() }),
            (.ellipse, L("kind_ellipse"), { ed.addEllipse(); onDismiss() }),
            (.line, L("kind_line"), { ed.addLine(); onDismiss() }),
            (.border, L("kind_border"), { ed.addBorder(); onDismiss() }),
        ]
        let smart: [(Icon, String, () -> Void)] = [
            (.calendar, L("insert_date"), { ed.addBoundText(BoundText.date); onDismiss() }),
            (.clock, L("insert_time"), { ed.addBoundText(BoundText.time); onDismiss() }),
            (.hash, L("insert_serial"), { ed.addBoundText(BoundText.serial); onDismiss() }),
            (.scan, L("insert_code_from_image"), { kind = .code; picking = true }),
        ]
        SheetBody(title: L("tool_insert"), onClose: onDismiss) {
            grid(basics)
            GroupTitle(L("insert_smart"))
            grid(smart)
        }
        .presentationDetents([.medium, .large])
        .photosPicker(isPresented: $picking, selection: $photo, matching: .images)
        .onChange(of: photo) {
            guard let item = photo else { return }
            photo = nil
            Task {
                guard let data = try? await item.loadTransferable(type: Data.self) else {
                    ed.app.message = .text(L("image_failed"))
                    return onDismiss()
                }
                if kind == .code {
                    if await ed.addCode(fromImage: data) {
                        onCode(Task {})
                    } else {
                        if ed.app.message == nil { ed.app.message = .text(L("no_code_found")) }
                        onDismiss()
                    }
                } else {
                    ed.addImage(data)
                    onDismiss()
                }
            }
        }
        .sheet(isPresented: $icons) {
            IconPickerSheet(state: ed.app, current: nil, onDismiss: { icons = false }) { name in
                icons = false
                ed.addIcon(name)
                onDismiss()
            }
        }
    }

    private func grid(_ items: [(Icon, String, () -> Void)]) -> some View {
        LazyVGrid(columns: Array(repeating: GridItem(.flexible(), spacing: 12), count: 3), spacing: 12) {
            ForEach(items.indices, id: \.self) { i in
                let (icon, label, add) = items[i]
                Button(action: add) { tile(icon, label) }.buttonStyle(.plain)
            }
        }
    }

    private func tile(_ icon: Icon, _ label: String) -> some View {
        VStack(spacing: 8) {
            LIcon(icon, Palette.primary, size: 28)
            Text(label).style(.bodyStrong.with(size: 14)).lineLimit(2).multilineTextAlignment(.center).minimumScaleFactor(0.8)
        }
        .padding(.horizontal, 6)
        .frame(maxWidth: .infinity)
        .frame(height: 96)
        .background(Palette.bg, in: RoundedRectangle(cornerRadius: 16))
        .contentShape(RoundedRectangle(cornerRadius: 16))
    }
}

/// The engine's pictograms by category, with search; a tap picks one.
struct IconPickerSheet: View {
    let state: AppState
    let current: String?
    let onDismiss: () -> Void
    let onPick: (String) -> Void
    @State private var query = ""
    @State private var category = ""

    var body: some View {
        let categories = Array(Set(state.icons.map(\.category))).sorted()
        let active = category.isEmpty ? categories.first ?? "" : category
        let shown = state.icons.filter { i in
            query.isEmpty ? i.category == active : i.name.localizedCaseInsensitiveContains(query)
        }
        SheetBody(title: L("kind_icon"), onClose: onDismiss) {
            SearchField(text: $query, placeholder: L("icons_search"))
            if query.isEmpty {
                ScrollView(.horizontal, showsIndicators: false) {
                    HStack(spacing: 8) {
                        ForEach(categories, id: \.self) { c in
                            Chip(text: c.capitalized, selected: c == active) { category = c }
                        }
                    }
                    .padding(.vertical, 1)
                }
            }
            if state.icons.isEmpty {
                ProgressView().frame(maxWidth: .infinity).padding(.vertical, 24)
            } else if shown.isEmpty {
                Text(L("icons_none")).style(.caption).frame(maxWidth: .infinity).padding(.vertical, 24)
            }
            LazyVGrid(columns: Array(repeating: GridItem(.flexible(), spacing: 10), count: 4), spacing: 10) {
                ForEach(shown) { icon in
                    let on = icon.name == current
                    let shape = RoundedRectangle(cornerRadius: 14)
                    Button { onPick(icon.name) } label: {
                        VStack(spacing: 4) {
                            ZStack {
                                if let image = state.iconThumbs[icon.name] {
                                    Image(decorative: image, scale: 1).resizable().interpolation(.medium).aspectRatio(contentMode: .fit)
                                        .blendMode(.multiply)
                                }
                            }
                            .frame(width: 40, height: 40)
                            Text(icon.name.replacingOccurrences(of: "_", with: " ")).style(.small).lineLimit(1).minimumScaleFactor(0.7)
                        }
                        .padding(8)
                        .frame(maxWidth: .infinity)
                        .background(on ? Palette.primaryTint : Palette.bg, in: shape)
                        .overlay(shape.strokeBorder(on ? Palette.primary : .clear, lineWidth: 2))
                        .contentShape(shape)
                    }
                    .buttonStyle(.plain)
                    .accessibilityLabel(icon.name)
                }
            }
        }
        .presentationDetents([.large])
        .task { await state.loadIcons() }
        .task(id: [active, query]) {
            if query.isEmpty {
                await state.loadIconThumbs(category: active)
            } else {
                for c in Set(shown.map(\.category)) { await state.loadIconThumbs(category: c) }
            }
        }
    }
}

/// Size presets, custom size, paper type and orientation: the size part of a new label and of
/// the label settings.
private struct SizeEditor: View {
    @Binding var w: Double
    @Binding var h: Double
    @Binding var media: String

    var body: some View {
        preview
        Text(L("size")).style(.bodyStrong.with(color: Palette.text2))
        LazyVGrid(columns: Array(repeating: GridItem(.flexible(), spacing: 10), count: 3), spacing: 10) {
            ForEach(Array(AppState.presetSizes.enumerated()), id: \.offset) { _, size in
                preset(size.0, size.1)
            }
        }
        HStack(spacing: 10) {
            NumberField(label: L("width"), value: w, live: true, signed: false) { if $0 > 0 { w = $0 } }
            NumberField(label: L("height"), value: h, live: true, signed: false) { if $0 > 0 { h = $0 } }
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
    }

    private var preview: some View {
        let scale = min(200 / max(w, 0.1), 100 / max(h, 0.1))
        return VStack(spacing: 6) {
            Text(mmText(w)).style(.small)
            HStack(spacing: 8) {
                RoundedRectangle(cornerRadius: 6).fill(Palette.surface).frame(width: w * scale, height: h * scale)
                Text(mmText(h)).style(.small)
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
                .minimumScaleFactor(0.7)
                .frame(maxWidth: .infinity)
                .padding(.vertical, 12)
                .background(on ? Palette.primaryTint : Palette.surface, in: shape)
                .overlay(shape.strokeBorder(on ? Palette.primary : Palette.border, lineWidth: on ? 2 : 1))
        }
        .buttonStyle(.plain)
    }
}

/// M11 · Új címke: size presets, custom size, paper type and orientation, the roll in the
/// printer ("Use this size") and the paper library.
struct NewLabelSheet: View {
    let state: AppState
    let onDismiss: () -> Void
    /// Width, height, media and corner radius.
    let onConfirm: (Double, Double, String, Double) -> Void
    @State private var w = AppState.presetSizes[0].0
    @State private var h = AppState.presetSizes[0].1
    @State private var media = "gap"
    @State private var radius = 0.0
    @State private var papers = false
    @State private var reading = false

    var body: some View {
        SheetBody(title: L("new_label"), onClose: onDismiss) {
            SizeEditor(w: $w, h: $h, media: $media)
            ListGroup {
                if state.printer != nil {
                    ListRow(title: L("paper_read"), subtitle: loadedText, action: { Task { await read() } }) {
                        IconTile(.scan)
                    } trailing: {
                        if reading {
                            ProgressView()
                        } else if let p = state.detected?.paper {
                            Button { use(p) } label: {
                                Text(L("paper_use_size")).style(.bodyStrong.with(color: Palette.primary))
                            }
                            .buttonStyle(.plain)
                        } else {
                            LIcon(.chevRight, Palette.muted, size: 18)
                        }
                    }
                    RowDivider()
                }
                ListRow(title: L("papers"), subtitle: L("papers_sub"), action: {
                    endEditing()
                    papers = true
                }) {
                    IconTile(.label)
                } trailing: {
                    LIcon(.chevRight, Palette.muted, size: 18)
                }
            }
            HStack(spacing: 12) {
                SecondaryButton(text: L("cancel"), height: 52) { onDismiss() }
                PrimaryButton(text: L("create")) {
                    endEditing()
                    onConfirm(w, h, media, radius)
                }
            }
        }
        .presentationDetents([.large])
        .sheet(isPresented: $papers) {
            PaperSheet(state: state, onPick: use) { papers = false }
        }
        // Another size by hand is no longer that paper's corners.
        .onChange(of: [w, h]) { if let p = picked, p.widthMm != w || p.heightMm != h { radius = 0 } }
    }

    @State private var picked: Paper?

    private var loadedText: String {
        guard let d = state.detected else { return L("paper_read_sub") }
        if !d.rfid { return L("paper_no_tag") }
        return d.paper.map(paperTitle) ?? L("paper_unknown_roll")
    }

    private func read() async {
        reading = true
        await state.detectPaper()
        reading = false
    }

    private func use(_ p: Paper) {
        picked = p
        (w, h, media, radius) = (p.widthMm, p.heightMm, p.media, p.cornerRadiusMm)
    }
}

/// The label of the open document: its name, size and paper, corners, the colour it is shown
/// with, the printer model and density it is made for, the check and the canvas helpers.
struct LabelSettingsSheet: View {
    static let colors = ["FFFFFF", "FFF3BF", "FFD8A8", "FFC9C9", "D3F9D8", "D0EBFF", "E5DBFF", "E9ECEF"]

    let ed: EditorState
    let onPapers: () -> Void
    let onCheck: () -> Void
    let onDismiss: () -> Void
    @State private var w = 50.0
    @State private var h = 30.0
    @State private var media = "gap"

    var body: some View {
        let app = ed.app
        SheetBody(title: L("label_title"), onClose: close) {
            if let doc = ed.doc {
                GroupTitle(L("label_name"))
                CommitField(value: doc.name, placeholder: L("label_name")) { name in
                    let name = name.trimmingCharacters(in: .whitespaces)
                    if !name.isEmpty { ed.setLabelProps(["name": JSON(name)]) }
                }

                SizeEditor(w: $w, h: $h, media: $media)
                let changed = w != doc.label.widthMm || h != doc.label.heightMm || media != doc.label.media
                PrimaryButton(text: L("apply_size"), enabled: changed) {
                    endEditing()
                    ed.setLabel(widthMm: w, heightMm: h, media: media)
                }

                ListGroup {
                    ListRow(title: L("papers"), subtitle: L("papers_sub"), action: onPapers) {
                        IconTile(.label)
                    } trailing: {
                        LIcon(.chevRight, Palette.muted, size: 18)
                    }
                    RowDivider()
                    ListRow(title: L("check_label"), subtitle: L("check_label_sub"), action: onCheck) {
                        IconTile(.check)
                    } trailing: {
                        LIcon(.chevRight, Palette.muted, size: 18)
                    }
                }

                GroupTitle(L("label_look"))
                ListGroup {
                    StepRow(title: L("corner_radius"), value: doc.label.cornerRadiusMm, step: 0.5, range: 0...20, text: mmText(doc.label.cornerRadiusMm)) {
                        ed.setLabelProps(["corner_radius_mm": JSON($0)])
                    }
                    RowDivider()
                    VStack(alignment: .leading, spacing: 10) {
                        Text(L("label_color")).style(.bodyStrong.with(size: 16))
                        Text(L("label_color_sub")).style(.caption)
                        HStack(spacing: 10) {
                            ForEach(Self.colors, id: \.self) { c in
                                let on = app.prefs.labelColor == c
                                Button { app.prefs.labelColor = c } label: {
                                    Circle()
                                        .fill(Color(hex: UInt32(c, radix: 16) ?? 0xFFFFFF))
                                        .overlay(Circle().strokeBorder(on ? Palette.primary : Palette.border, lineWidth: on ? 2.5 : 1))
                                        .frame(width: 30, height: 30)
                                }
                                .buttonStyle(.plain)
                                .accessibilityLabel(c)
                                .accessibilityAddTraits(on ? .isSelected : [])
                            }
                        }
                    }
                    .padding(.horizontal, 16)
                    .padding(.vertical, 14)
                }

                GroupTitle(L("label_printer"))
                ListGroup {
                    let models = modelOptions(doc)
                    PickRow(
                        title: L("printer_model"), subtitle: app.model(doc.label.printerModel ?? "").map { "\($0.dpi) dpi · \(mmText($0.headMm))" },
                        value: doc.label.printerModel ?? "", options: models
                    ) { ed.setLabelProps(["printer_model": JSON($0)]) }
                    RowDivider()
                    let model = app.model(doc.label.printerModel ?? "")
                    let minD = model?.densityMin ?? 1
                    let maxD = model?.densityMax ?? 5
                    ListRow(title: L("density_default"), subtitle: doc.label.density == nil ? L("density_model") : nil) {
                        CountStepper(
                            value: doc.label.density.map(String.init) ?? "–",
                            onMinus: {
                                // Below the lowest density is "the model's".
                                let d = (doc.label.density ?? minD) - 1
                                ed.setLabelProps(["density": JSON(d < minD ? 0 : d)])
                            },
                            onPlus: { ed.setLabelProps(["density": JSON(min((doc.label.density ?? minD - 1) + 1, maxD))]) }
                        )
                    }
                }
                Text(L("printer_model_hint")).style(.caption)

                CanvasOptions(state: app)
            }
        }
        .presentationDetents([.large])
        .onAppear(perform: sync)
        .onChange(of: ed.doc?.label) { sync() }
    }

    private func sync() {
        guard let l = ed.doc?.label else { return }
        (w, h, media) = (l.widthMm, l.heightMm, l.media)
    }

    private func close() {
        endEditing()
        onDismiss()
    }

    /// "None", the label's own model and every model the engine knows.
    private func modelOptions(_ doc: Doc) -> [(String, String)] {
        var ids = ed.app.models.map(\.id)
        if let id = ed.app.printer?.model?.id, !ids.contains(id) { ids.append(id) }
        if let id = doc.label.printerModel, !ids.contains(id) { ids.append(id) }
        if !ids.contains(AppState.defaultModel) { ids.append(AppState.defaultModel) }
        return [("", L("printer_model_none"))] + ids.sorted().map { id in (id, ed.app.model(id)?.name ?? id) }
    }
}
