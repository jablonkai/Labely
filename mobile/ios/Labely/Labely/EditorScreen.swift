import LabelyKit
import SwiftUI
import UniformTypeIdentifiers

private enum EditorSheet: String, Identifiable {
    case insert, layers, label, print, export, code
    var id: String { rawValue }
}

/// What the data panel's file picker offers: CSV, TSV, TXT and spreadsheets.
private let dataFileTypes: [UTType] =
    [.commaSeparatedText, .tabSeparatedText, .plainText, .spreadsheet]
        + ["csv", "tsv", "txt", "xlsx", "xls", "ods"].compactMap { UTType(filenameExtension: $0) }

/// M2 · Szerkesztő: canvas, inspector of the selected element and the editing toolbar.
struct EditorScreen: View {
    @Bindable var ed: EditorState
    let onClose: () -> Void
    @State private var sheet: EditorSheet?
    @State private var savingTemplate = false
    @State private var templateName = ""
    /// The data panel takes the inspector's place (M4).
    @State private var showData = false
    @State private var pickingData = false

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
                    PaperBar(text: sizeText(doc.label.widthMm, doc.label.heightMm) + " · " + mediaName(doc.label.media)) {
                        open(.label)
                    }
                    .padding(.bottom, 16)
                }
                .frame(maxHeight: .infinity)
                .background(Palette.canvas)
                Divider().overlay(Palette.border)
                if showData {
                    DataPanel(ed: ed) { pickingData = true }
                } else {
                    Inspector(ed: ed) { open(.code) }
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
            .sheet(item: $sheet) { s in
                switch s {
                case .insert:
                    InsertSheet(ed: ed, onDismiss: { sheet = nil }) { adding in
                        // A new barcode or 2D code opens its sheet (M6).
                        Task {
                            await adding.value
                            sheet = isCode(ed.selected) ? .code : nil
                        }
                    }
                    .presentationDetents([.medium, .large])
                case .layers: LayersSheet(ed: ed) { sheet = nil }.presentationDetents([.medium, .large])
                case .label:
                    LabelSheet(
                        title: L("label_title"),
                        confirm: L("apply"),
                        initial: (doc.label.widthMm, doc.label.heightMm, doc.label.media),
                        onDismiss: { sheet = nil }
                    ) { w, h, media in
                        ed.setLabel(widthMm: w, heightMm: h, media: media)
                        sheet = nil
                    }
                case .print: PrintSheet(ed: ed) { sheet = nil }
                case .export: ExportSheet(ed: ed) { sheet = nil }
                case .code: BarcodeSheet(ed: ed) { sheet = nil }
                }
            }
            .fileImporter(isPresented: $pickingData, allowedContentTypes: dataFileTypes) { result in
                if case .success(let url) = result { ed.attachFile(url) }
            }
            // Picking an element on the canvas brings its inspector back.
            .onChange(of: ed.selectedId) { if ed.selectedId != nil { showData = false } }
            .alert(L("save_template"), isPresented: $savingTemplate) {
                TextField(L("template_name"), text: $templateName)
                Button(L("save")) {
                    let name = templateName.trimmingCharacters(in: .whitespaces)
                    if !name.isEmpty { ed.saveTemplate(name) }
                }
                Button(L("cancel"), role: .cancel) {}
            }
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
                Button(L("export")) { open(.export) }
                Button(L("save_template")) { saveTemplate() }
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
            IconButton(.undo, enabled: doc.canUndo, label: "Undo") { ed.undo() }
            IconButton(.redo, enabled: doc.canRedo, label: "Redo") { ed.redo() }
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
                if showData { ed.selectedId = nil }
            }
            toolbarItem(.templates, L("tool_template")) { saveTemplate() }
            toolbarItem(.settings, L("tool_label")) { open(.label) }
        }
        .frame(height: 64)
    }

    private func saveTemplate() {
        endEditing()
        templateName = ed.doc?.name ?? ""
        savingTemplate = true
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

private struct Inspector: View {
    let ed: EditorState
    /// Open the barcode sheet of the selected code.
    let onCode: () -> Void

    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            if let e = ed.selected {
                HStack(spacing: 12) {
                    IconTile(kindIcon(e.type))
                    VStack(alignment: .leading) {
                        Text(elementTitle(e)).style(.bodyStrong.with(size: 17)).lineLimit(1)
                        // "Text · bound: Price column", as in M2.
                        Text([kindName(e.type), ed.boundColumn(e).map { L("bound_to", $0) }].compactMap { $0 }.joined(separator: " · "))
                            .style(.caption).lineLimit(1)
                    }
                    .frame(maxWidth: .infinity, alignment: .leading)
                    IconButton(.copy, label: "Duplicate") {
                        ed.exec("element.duplicate", ["id": .int(e.id), "dx_mm": 1.0, "dy_mm": 1.0], select: true)
                    }
                    IconButton(.trash, tint: Palette.red, label: L("delete")) {
                        ed.exec("element.delete", ["ids": [.int(e.id)]])
                    }
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
                    FieldRow(action: onCode) {
                        Text(e.data?.isEmpty == false ? e.data ?? "" : L("data_hint"))
                            .style(.body.with(color: e.data?.isEmpty == false ? Palette.text : Palette.muted))
                            .lineLimit(1)
                            .frame(maxWidth: .infinity, alignment: .leading)
                        Text(Symbology.name(e.symbology)).style(.caption)
                        LIcon(.chevRight, Palette.muted, size: 18)
                    }
                    if ed.canBind(e) { BindingRow(ed: ed, e: e) }
                case "image": ImageProps(ed: ed, e: e)
                default: EmptyView()
                }
                Spacer(minLength: 0)
            } else {
                Text(L("inspector_hint")).style(.caption).multilineTextAlignment(.center)
                    .frame(maxWidth: .infinity, maxHeight: .infinity)
            }
        }
        .padding(16)
        .frame(height: 256)
        // A new selection starts with fresh fields.
        .id(ed.selectedId)
    }
}

private struct TextProps: View {
    let ed: EditorState
    let e: Element

    var body: some View {
        ContentField(value: e.text ?? "", placeholder: L("text_hint")) { ed.setProps(e.id, ["text": JSON($0)]) }
        HStack(spacing: 10) {
            let size = e.fontSizePt
            CountStepper(
                value: String(Int(size.rounded())),
                onMinus: { ed.setProps(e.id, ["font_size_pt": JSON(max(size - 1, 4))]) },
                onPlus: { ed.setProps(e.id, ["font_size_pt": JSON(min(size + 1, 200))]) }
            )
            Spacer()
            if ed.canBind(e) {
                BindingMenu(ed: ed, e: e) {
                    let on = ed.boundColumn(e) != nil
                    LIcon(.link, on ? Palette.primary : Palette.text2, size: 22)
                        .frame(width: 44, height: 44)
                        .background(on ? Palette.primaryTint : Palette.bg, in: RoundedRectangle(cornerRadius: 12))
                        .accessibilityLabel(L("binding"))
                }
            }
            Toggle(icon: .bold, on: e.bold) { ed.setProps(e.id, ["bold": JSON(!e.bold)]) }
            let (icon, next): (Icon, String) =
                switch e.align {
                case "center": (.tCenter, "right")
                case "right": (.tRight, "left")
                default: (.tLeft, "center")
                }
            Toggle(icon: icon, on: false) { ed.setProps(e.id, ["align": JSON(next)]) }
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

private struct Toggle: View {
    let icon: Icon
    let on: Bool
    let action: () -> Void

    var body: some View {
        Button(action: action) {
            LIcon(icon, on ? Palette.primary : Palette.text2, size: 22)
                .frame(width: 44, height: 44)
                .background(on ? Palette.primaryTint : Palette.bg, in: RoundedRectangle(cornerRadius: 12))
        }
        .buttonStyle(.plain)
        .accessibilityAddTraits(on ? .isSelected : [])
    }
}

/// Single-line content editor, committed on Return or when focus leaves.
private struct ContentField: View {
    let value: String
    let placeholder: String
    let onCommit: (String) -> Void
    @State private var text = ""
    @FocusState private var focused: Bool

    var body: some View {
        let shape = RoundedRectangle(cornerRadius: 12)
        TextField("", text: $text, prompt: Text(placeholder).foregroundStyle(Palette.muted))
            .style(.body)
            .focused($focused)
            .submitLabel(.done)
            .onSubmit(commit)
            .padding(.horizontal, 12)
            .frame(height: 44)
            .background(Palette.surface, in: shape)
            .overlay(shape.strokeBorder(focused ? Palette.primary : Palette.border))
            .onAppear { text = value }
            .onChange(of: value) { if !focused { text = value } }
            .onChange(of: focused) { if !focused { commit() } }
    }

    private func commit() {
        if text != value { onCommit(text) }
    }
}

/// Resign the first responder, so the focused field loses focus and commits.
@MainActor
func endEditing() {
    UIApplication.shared.sendAction(#selector(UIResponder.resignFirstResponder), to: nil, from: nil, for: nil)
}
