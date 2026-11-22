import LabelyKit
import SwiftUI
import UniformTypeIdentifiers

enum EditorSheet: String, Identifiable {
    case insert, layers, label, print, export, code, arrange, props, table, check, papers, template, dataOptions
    var id: String { rawValue }
}

/// M2 · Szerkesztő: canvas, inspector of the selected element and the editing toolbar.
struct EditorScreen: View {
    @Bindable var ed: EditorState
    let onClose: () -> Void
    @State private var sheet: EditorSheet?
    /// The data panel takes the inspector's place (M4).
    @State private var showData = false
    @State private var pickingData = false
    @State private var pickingImages = false
    /// The data panel's camera scanner: the scanned code becomes an element.
    @State private var scanning = false
    @State private var renaming = false
    @State private var newName = ""
    @State private var savingCopy: PickedFiles?

    var body: some View {
        // The stack only hosts the keyboard's toolbar; its bar stays hidden.
        NavigationStack {
            content.toolbar(.hidden, for: .navigationBar)
        }
    }

    @ViewBuilder private var content: some View {
        if let doc = ed.doc {
            VStack(spacing: 0) {
                topBar(doc)
                ZStack(alignment: .bottom) {
                    LabelCanvas(ed: ed).padding(.bottom, 56)
                    VStack(spacing: 8) {
                        PaperNotice(ed: ed) { open(.papers) }
                        PaperBar(text: sizeText(doc.label.widthMm, doc.label.heightMm) + " · " + mediaName(doc.label.media)) {
                            open(.label)
                        }
                    }
                    .padding(.bottom, 16)
                }
                .frame(maxHeight: .infinity)
                .background(Palette.canvas)
                Divider().overlay(Palette.border)
                if showData {
                    DataPanel(ed: ed, onChoose: { pickingData = true }, onImages: { pickingImages = true }, onOptions: { open(.dataOptions) }) {
                        scanning = true
                    }
                } else {
                    Inspector(ed: ed, open: open)
                }
                Divider().overlay(Palette.border)
                toolbar
            }
            .background(Palette.surface)
            .toolbar {
                ToolbarItemGroup(placement: .keyboard) {
                    Spacer()
                    Button(L("done")) { endEditing() }
                }
            }
            .sheet(item: $sheet) { s in sheetView(s, doc) }
            .sheet(item: $savingCopy) { files in SaveView(urls: files.urls).ignoresSafeArea() }
            .fullScreenCover(isPresented: $scanning) {
                ScanSheet(onDismiss: { scanning = false }) { code in
                    scanning = false
                    Task { await ed.addCode(scanned: code) }
                }
            }
            .fileImporter(isPresented: $pickingData, allowedContentTypes: dataFileTypes) { result in
                if case .success(let url) = result { ed.attachFile(url) }
            }
            // Picking an element on the canvas brings its inspector back.
            .onChange(of: ed.selection) { if !ed.selection.isEmpty { showData = false } }
            // Data that arrives with nothing selected shows its panel (Labels from data).
            .onChange(of: ed.data != nil, initial: true) { if ed.data != nil, ed.selection.isEmpty { showData = true } }
            .background {
                Color.clear.fileImporter(isPresented: $pickingImages, allowedContentTypes: [.image], allowsMultipleSelection: true) { result in
                    if case .success(let urls) = result, !urls.isEmpty { ed.attachImages(urls) }
                }
            }
            .alert(L("rename"), isPresented: $renaming) {
                TextField(L("label_name"), text: $newName)
                Button(L("save")) {
                    let name = newName.trimmingCharacters(in: .whitespaces)
                    if !name.isEmpty, name != doc.name { ed.setLabelProps(["name": JSON(name)]) }
                }
                Button(L("cancel"), role: .cancel) {}
            }
            // The roll in the printer is read when the printer or its paper changes.
            .task(id: paperKey) { await ed.refreshLoadedPaper() }
            // Print again from the history opens the print sheet at once.
            .task(id: ed.app.reprint?.id) { if ed.app.reprint != nil { sheet = .print } }
        }
    }

    private var paperKey: String {
        [ed.app.printer?.address, ed.app.printerStatus?.paper?.barcode].compactMap { $0 }.joined(separator: "|")
    }

    @ViewBuilder private func sheetView(_ s: EditorSheet, _ doc: Doc) -> some View {
        switch s {
        case .insert:
            InsertSheet(ed: ed, onDismiss: { sheet = nil }) { adding in
                // A new barcode or 2D code opens its sheet (M6).
                Task {
                    await adding.value
                    sheet = isCode(ed.selected) ? .code : nil
                }
            }
        case .layers: LayersSheet(ed: ed) { sheet = nil }.presentationDetents([.medium, .large])
        case .label: LabelSettingsSheet(ed: ed, onPapers: { sheet = .papers }, onCheck: { sheet = .check }) { sheet = nil }
        case .print: PrintSheet(ed: ed) { sheet = nil }
        case .export: ExportSheet(ed: ed) { sheet = nil }
        case .code: BarcodeSheet(ed: ed) { sheet = nil }
        case .arrange: ArrangeSheet(ed: ed) { sheet = nil }
        case .props: PropertiesSheet(ed: ed, onTable: { sheet = .table }, onCode: { sheet = .code }) { sheet = nil }
        case .table: TableSheet(ed: ed) { sheet = nil }
        case .check: CheckSheet(ed: ed) { sheet = nil }
        case .papers: PaperSheet(state: ed.app, ed: ed) { sheet = nil }
        case .template: SaveTemplateSheet(ed: ed) { sheet = nil }
        case .dataOptions: DataOptionsSheet(ed: ed) { sheet = nil }
        }
    }

    /// Fields commit when they lose focus. Opening a sheet or leaving ends editing first, so a
    /// pending edit is never lost (the editor fix of ADR-0029, kept on iOS).
    private func open(_ s: EditorSheet) {
        endEditing()
        sheet = s
    }

    private func topBar(_ doc: Doc) -> some View {
        HStack(spacing: 0) {
            IconButton(.chevLeft, tint: Palette.text, label: L("cancel")) {
                endEditing()
                onClose()
            }
            Menu {
                Button {
                    endEditing()
                    newName = doc.name
                    renaming = true
                } label: { Label(L("rename"), systemImage: "pencil") }
                Button { open(.check) } label: { Label(L("check_label"), systemImage: "checkmark.shield") }
                Divider()
                Button { open(.export) } label: { Label(L("export"), systemImage: "square.and.arrow.up") }
                Button {
                    endEditing()
                    Task { if let url = await ed.exportCopy() { savingCopy = PickedFiles(urls: [url]) } }
                } label: { Label(L("save_copy"), systemImage: "doc.on.doc") }
                Button { open(.template) } label: { Label(L("save_template"), systemImage: "square.grid.2x2") }
            } label: {
                VStack(spacing: 0) {
                    HStack(spacing: 4) {
                        Text(doc.name).style(.nav).lineLimit(1)
                        LIcon(.chevDown, Palette.text2, size: 14)
                    }
                    Text(L(ed.saved ? "saved" : "not_saved")).style(.small)
                }
                .frame(maxWidth: .infinity)
                .contentShape(Rectangle())
            }
            .buttonStyle(.plain)
            IconButton(.undo, enabled: doc.canUndo, label: L("undo")) { ed.undo() }
            IconButton(.redo, enabled: doc.canRedo, label: L("redo")) { ed.redo() }
            IconButton(.printer, tint: Palette.primary, label: L("print")) { open(.print) }
        }
        .padding(.horizontal, 8)
        .frame(height: 56)
    }

    private var toolbar: some View {
        HStack(spacing: 0) {
            toolbarItem(.plus, L("tool_insert")) { open(.insert) }
            toolbarItem(.layers, L("tool_layers")) { open(.layers) }
            toolbarItem(.data, L("tool_data"), on: showData) {
                endEditing()
                showData.toggle()
                if showData { ed.tap(nil) }
            }
            toolbarItem(.templates, L("tool_template")) { open(.template) }
            toolbarItem(.settings, L("tool_label")) { open(.label) }
        }
        .frame(height: 64)
    }

    private func toolbarItem(_ icon: Icon, _ text: String, on: Bool = false, action: @escaping () -> Void) -> some View {
        Button(action: action) {
            VStack(spacing: 3) {
                LIcon(icon, on ? Palette.primary : Palette.text2, size: 24)
                    .frame(width: 46, height: 30)
                    .background(on ? Palette.primaryTint : .clear, in: RoundedRectangle(cornerRadius: 10))
                Text(text).style(.small.with(size: 12, weight: on ? .semibold : .medium, color: on ? Palette.primary : Palette.text2))
                    .lineLimit(1)
                    .minimumScaleFactor(0.8)
            }
            .frame(maxWidth: .infinity, maxHeight: .infinity)
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .accessibilityAddTraits(on ? .isSelected : [])
    }
}

func isCode(_ e: Element?) -> Bool {
    e?.type == "barcode" || e?.type == "code2d"
}

/// Copy the selection to the system pasteboard; with `cut`, delete it too.
@MainActor
func copySelection(_ ed: EditorState, cut: Bool = false) {
    if let text = ed.copy(cut: cut) { UIPasteboard.general.string = text }
}

/// Paste from the system pasteboard: copied elements, or plain text as a text element.
@MainActor
func pasteInto(_ ed: EditorState) {
    guard let text = UIPasteboard.general.string, !text.isEmpty else {
        ed.app.message = .text(L("paste_empty"))
        return
    }
    ed.paste(text)
}

private struct PaperBar: View {
    let text: String
    let action: () -> Void

    var body: some View {
        Button(action: action) {
            HStack(spacing: 8) {
                LIcon(.rect, Palette.text, size: 16)
                Text(text).style(.bodyStrong.with(size: 14))
                LIcon(.chevDown, Palette.muted, size: 14)
            }
            .padding(.horizontal, 14)
            .padding(.vertical, 8)
            .background(Palette.surface, in: Capsule())
        }
        .buttonStyle(.plain)
    }
}

/// What the printer's roll means for this label: another paper is loaded (with "Use it"), or the
/// roll is not known yet. Nothing when the loaded paper is the label's.
private struct PaperNotice: View {
    let ed: EditorState
    let onPapers: () -> Void

    var body: some View {
        if let d = ed.loadedPaper, d.rfid {
            if let paper = d.paper, d.matchesLabel == false {
                notice(L("paper_mismatch", paperTitle(paper)), action: L("paper_use")) { ed.applyPaper(paper.id) }
            } else if d.paper == nil, d.barcode != nil {
                notice(L("paper_unknown_roll"), action: L("paper_choose"), run: onPapers)
            }
        }
    }

    private func notice(_ text: String, action: String, run: @escaping () -> Void) -> some View {
        HStack(spacing: 8) {
            LIcon(.warn, Palette.amber, size: 16)
            Text(text).style(.bodyStrong.with(size: 13, color: Palette.amber)).lineLimit(2)
            Button(action: run) {
                Text(action).style(.bodyStrong.with(size: 13, color: Palette.primary))
                    .padding(.horizontal, 10)
                    .padding(.vertical, 5)
                    .background(Palette.surface, in: Capsule())
            }
            .buttonStyle(.plain)
        }
        .padding(.leading, 12)
        .padding(.trailing, 6)
        .padding(.vertical, 6)
        .background(Palette.amberTint, in: Capsule())
        .padding(.horizontal, 16)
    }
}

private struct Inspector: View {
    let ed: EditorState
    let open: (EditorSheet) -> Void

    var body: some View {
        let selected = ed.selectedElements
        ScrollView {
            VStack(alignment: .leading, spacing: 12) {
                if let e = ed.selected {
                    single(e)
                } else if selected.count > 1 {
                    several(selected)
                } else {
                    nothing
                }
            }
            .padding(16)
        }
        .scrollDismissesKeyboard(.interactively)
        .frame(height: 256)
        // A new selection starts with fresh fields.
        .id(ed.selection)
    }

    @ViewBuilder private func single(_ e: Element) -> some View {
        HStack(spacing: 8) {
            IconTile(kindIcon(e.type))
            VStack(alignment: .leading) {
                Text(elementTitle(e)).style(.bodyStrong.with(size: 17)).lineLimit(1)
                // "Text · bound: Price column", as in M2.
                Text(([kindName(e.type), ed.boundColumn(e).map { L("bound_to", $0) }] + [e.locked ? L("locked") : nil]).compactMap { $0 }.joined(separator: " · "))
                    .style(.caption).lineLimit(1)
            }
            .frame(maxWidth: .infinity, alignment: .leading)
            MoreMenu(ed: ed)
            IconButton(.copy, label: L("duplicate")) { ed.duplicateSelection() }
            IconButton(.trash, tint: Palette.red, label: L("delete")) { ed.deleteSelection() }
        }
        HStack(spacing: 8) {
            NumberField(label: "X", value: e.xMm, suffix: "") {
                ed.exec("element.move", ["id": .int(e.id), "x_mm": JSON($0), "y_mm": JSON(e.yMm)])
            }
            NumberField(label: "Y", value: e.yMm, suffix: "") {
                ed.exec("element.move", ["id": .int(e.id), "x_mm": JSON(e.xMm), "y_mm": JSON($0)])
            }
            NumberField(label: L("width_short"), value: e.widthMm, suffix: "") {
                ed.exec("element.resize", ["id": .int(e.id), "width_mm": JSON($0), "height_mm": JSON(e.heightMm)])
            }
            NumberField(label: L("height_short"), value: e.heightMm, suffix: "") {
                ed.exec("element.resize", ["id": .int(e.id), "width_mm": JSON(e.widthMm), "height_mm": JSON($0)])
            }
        }
        switch e.type {
        case "text": TextProps(ed: ed, e: e)
        case "barcode", "code2d":
            // The content opens the barcode sheet (M6).
            FieldRow(action: { open(.code) }) {
                Text(e.data?.isEmpty == false ? e.data ?? "" : L("data_hint"))
                    .style(.body.with(color: e.data?.isEmpty == false ? Palette.text : Palette.muted))
                    .lineLimit(1)
                    .frame(maxWidth: .infinity, alignment: .leading)
                Text(Symbology.name(e.symbology)).style(.caption)
                LIcon(.chevRight, Palette.muted, size: 18)
            }
            if ed.canBind(e) { BindingRow(ed: ed, e: e) }
        case "image": ImageProps(ed: ed, e: e)
        case "table":
            FieldRow(action: { open(.table) }) {
                LIcon(.table, Palette.primary, size: 18)
                Text(L("table_edit")).style(.bodyStrong).frame(maxWidth: .infinity, alignment: .leading)
                if let t = TableInfo.of(e) { Text("\(t.rows) × \(t.columns)").style(.caption) }
                LIcon(.chevRight, Palette.muted, size: 18)
            }
        default: EmptyView()
        }
        HStack(spacing: 8) {
            SecondaryButton(text: L("properties"), icon: .settings) { open(.props) }
            SecondaryButton(text: L("arrange"), icon: .alignLeft) { open(.arrange) }
        }
    }

    @ViewBuilder private func several(_ selected: [Element]) -> some View {
        HStack(spacing: 8) {
            IconTile(.selectAll)
            VStack(alignment: .leading) {
                Text(L("selected_count", selected.count)).style(.bodyStrong.with(size: 17)).lineLimit(1)
                Text(L("selected_hint")).style(.caption).lineLimit(1)
            }
            .frame(maxWidth: .infinity, alignment: .leading)
            MoreMenu(ed: ed)
            IconButton(.copy, label: L("duplicate")) { ed.duplicateSelection() }
            IconButton(.trash, tint: Palette.red, label: L("delete")) { ed.deleteSelection() }
        }
        AlignBar(ed: ed)
        HStack(spacing: 8) {
            SecondaryButton(text: L(ed.canUngroup && !ed.canGroupMore ? "ungroup" : "group"), icon: ed.canUngroup && !ed.canGroupMore ? .ungroup : .group) {
                if ed.canUngroup && !ed.canGroupMore { ed.ungroup() } else { ed.group() }
            }
            SecondaryButton(text: L("arrange"), icon: .alignLeft) { open(.arrange) }
        }
    }

    @ViewBuilder private var nothing: some View {
        Text(L("inspector_hint")).style(.caption).multilineTextAlignment(.center)
            .frame(maxWidth: .infinity)
            .padding(.top, 28)
            .padding(.bottom, 8)
        HStack(spacing: 8) {
            SecondaryButton(text: L("paste"), icon: .paste) { pasteInto(ed) }
            SecondaryButton(text: L("select_all"), icon: .selectAll, enabled: ed.doc?.elements.isEmpty == false) { ed.selectAll() }
        }
        SecondaryButton(text: L("check_label"), icon: .check) { open(.check) }
    }
}

extension EditorState {
    /// Whether the selection holds elements that are not all in one group yet.
    var canGroupMore: Bool {
        let groups = Set(selectedElements.map(\.group))
        return canGroup && (groups.count > 1 || groups.contains(nil))
    }
}

/// The six alignments and the two distributions, as icon buttons.
struct AlignBar: View {
    let ed: EditorState

    var body: some View {
        let edges: [(Icon, String)] = [
            (.alignLeft, "left"), (.alignCenter, "center"), (.alignRight, "right"),
            (.alignTop, "top"), (.alignMiddle, "middle"), (.alignBottom, "bottom"),
        ]
        ScrollView(.horizontal, showsIndicators: false) {
            HStack(spacing: 8) {
                ForEach(edges, id: \.1) { icon, edge in
                    IconToggle(icon: icon, enabled: !ed.selection.isEmpty, label: L("align_\(edge)")) { ed.align(edge) }
                }
                Rectangle().fill(Palette.border).frame(width: 1, height: 28)
                IconToggle(icon: .distributeH, enabled: ed.selection.count >= 3, label: L("distribute_h")) { ed.distribute("horizontal") }
                IconToggle(icon: .distributeV, enabled: ed.selection.count >= 3, label: L("distribute_v")) { ed.distribute("vertical") }
            }
        }
    }
}

/// The "···" of the inspector: clipboard, lock, hide, order, group and mirror copy.
struct MoreMenu: View {
    let ed: EditorState

    var body: some View {
        let els = ed.selectedElements
        let locked = !els.isEmpty && els.allSatisfy(\.locked)
        Menu {
            Button { copySelection(ed) } label: { Label(L("copy"), systemImage: "doc.on.doc") }
            Button { copySelection(ed, cut: true) } label: { Label(L("cut"), systemImage: "scissors") }
            Button { pasteInto(ed) } label: { Label(L("paste"), systemImage: "doc.on.clipboard") }
            Button { ed.selectAll() } label: { Label(L("select_all"), systemImage: "checkmark.circle") }
            Divider()
            Button { ed.setLocked(els.map(\.id), !locked) } label: {
                Label(L(locked ? "unlock" : "lock"), systemImage: locked ? "lock.open" : "lock")
            }
            Button { ed.setHidden(els.map(\.id), true) } label: { Label(L("hide"), systemImage: "eye.slash") }
            Divider()
            Button { ed.reorder("front") } label: { Label(L("to_front"), systemImage: "square.3.layers.3d.top.filled") }
            Button { ed.reorder("back") } label: { Label(L("to_back"), systemImage: "square.3.layers.3d.bottom.filled") }
            Divider()
            if ed.canGroup { Button { ed.group() } label: { Label(L("group"), systemImage: "square.on.square.dashed") } }
            if ed.canUngroup { Button { ed.ungroup() } label: { Label(L("ungroup"), systemImage: "square.dashed") } }
            Button { ed.mirror("horizontal") } label: { Label(L("mirror_copy_h"), systemImage: "arrow.left.and.right.righttriangle.left.righttriangle.right") }
            Button { ed.mirror("vertical") } label: { Label(L("mirror_copy_v"), systemImage: "arrow.up.and.down.righttriangle.up.righttriangle.down") }
        } label: {
            LIcon(.more, Palette.text2, size: 22).frame(width: 40, height: 40).contentShape(Circle())
        }
        .accessibilityLabel(L("more"))
    }
}

private struct TextProps: View {
    let ed: EditorState
    let e: Element

    var body: some View {
        CommitField(value: e.text ?? "", placeholder: L("text_hint"), axis: .vertical) { ed.setProps(e.id, ["text": JSON($0)]) }
        HStack(spacing: 8) {
            let size = e.fontSizePt
            CountStepper(
                value: num(size),
                onMinus: { ed.setProps(e.id, ["font_size_pt": JSON(max(size - 1, 4))]) },
                onPlus: { ed.setProps(e.id, ["font_size_pt": JSON(min(size + 1, 200))]) }
            )
            Spacer(minLength: 0)
            if ed.canBind(e) {
                BindingMenu(ed: ed, e: e) {
                    let on = ed.boundColumn(e) != nil
                    LIcon(.link, on ? Palette.primary : Palette.text2, size: 22)
                        .frame(width: 44, height: 44)
                        .background(on ? Palette.primaryTint : Palette.bg, in: RoundedRectangle(cornerRadius: 12))
                        .accessibilityLabel(L("binding"))
                }
            }
            IconToggle(icon: .bold, on: e.bold, label: L("bold")) { ed.setProps(e.id, ["bold": JSON(!e.bold)]) }
            IconToggle(icon: .italic, on: e.italic, label: L("italic")) { ed.setProps(e.id, ["italic": JSON(!e.italic)]) }
            let (icon, next): (Icon, String) =
                switch e.align {
                case "center": (.tCenter, "right")
                case "right": (.tRight, "justify")
                case "justify": (.tLeft, "left")
                default: (.tLeft, "center")
                }
            IconToggle(icon: icon, on: e.align == "justify", label: L("text_align")) { ed.setProps(e.id, ["align": JSON(next)]) }
        }
    }
}

/// What the engine has for an image's 1-bit conversion: dither mode, threshold, invert.
private struct ImageProps: View {
    let ed: EditorState
    let e: Element

    var body: some View {
        Menu {
            Picker(L("image_conversion"), selection: Binding(get: { e.dither }, set: { ed.setProps(e.id, ["dither": JSON($0)]) })) {
                ForEach(Symbology.dither, id: \.self) { Text(ditherName($0)).tag($0) }
            }
        } label: {
            FieldRow {
                Text(L("image_conversion")).style(.caption)
                Text(ditherName(e.dither)).style(.body).lineLimit(1).frame(maxWidth: .infinity, alignment: .leading)
                LIcon(.chevDown, Palette.muted, size: 18)
            }
        }
        .buttonStyle(.plain)
        HStack(spacing: 10) {
            Text(L("dither_threshold")).style(.caption)
            let t = e.threshold
            CountStepper(
                value: String(t),
                onMinus: { ed.setProps(e.id, ["threshold": JSON(max(t - 16, 0))]) },
                onPlus: { ed.setProps(e.id, ["threshold": JSON(min(t + 16, 255))]) }
            )
            Spacer()
            Chip(text: L("image_invert"), selected: e.invert) { ed.setProps(e.id, ["invert": JSON(!e.invert)]) }
        }
    }
}

/// Resign the first responder, so the focused field loses focus and commits.
@MainActor
func endEditing() {
    UIApplication.shared.sendAction(#selector(UIResponder.resignFirstResponder), to: nil, from: nil, for: nil)
}
