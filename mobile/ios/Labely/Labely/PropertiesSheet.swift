import LabelyKit
import SwiftUI
import UniformTypeIdentifiers

/// Every property of the selected element, by its kind: the long form of the inspector.
struct PropertiesSheet: View {
    let ed: EditorState
    let onTable: () -> Void
    let onCode: () -> Void
    let onDismiss: () -> Void

    var body: some View {
        SheetBody(title: ed.selected.map { kindName($0.type) } ?? L("properties"), onClose: close) {
            if let e = ed.selected {
                switch e.type {
                case "text": TextSheet(ed: ed, e: e)
                case "barcode", "code2d": CodeProps(ed: ed, e: e, onCode: onCode)
                case "image": ImageSheet(ed: ed, e: e)
                case "rect", "ellipse", "line": ShapeProps(ed: ed, e: e)
                case "icon": IconProps(ed: ed, e: e)
                case "border": BorderProps(ed: ed, e: e)
                case "table": TableProps(ed: ed, e: e, onTable: onTable)
                default: EmptyView()
                }
                GroupTitle(L("element_name"))
                CommitField(value: e.name ?? "", placeholder: kindName(e.type)) { ed.rename(e, to: $0) }
            } else {
                Text(L("inspector_hint")).style(.caption).frame(maxWidth: .infinity).padding(.vertical, 24)
            }
        }
        .presentationDetents([.medium, .large])
        .presentationBackgroundInteraction(.enabled(upThrough: .medium))
        .onChange(of: ed.selected == nil) { if ed.selected == nil { onDismiss() } }
    }

    private func close() {
        endEditing()
        onDismiss()
    }
}

@MainActor
private func setProp(_ ed: EditorState, _ e: Element, _ key: String, _ value: JSON) {
    ed.setProps(e.id, .object([key: value]))
}

// MARK: - Text

/// The text with its font, style, layout and the styling of a selected range.
private struct TextSheet: View {
    let ed: EditorState
    let e: Element
    @State private var text = ""
    @State private var selection: TextSelection?
    @State private var pickingFont = false
    @FocusState private var focused: Bool

    var body: some View {
        editor
            .fileImporter(isPresented: $pickingFont, allowedContentTypes: fontTypes) { result in
                guard case .success(let url) = result else { return }
                Task {
                    if let family = await ed.app.addFont(url).first { setProp(ed, e, "font_family", JSON(family)) }
                }
            }
        rangeBar
        ListGroup {
            ListRow(title: L("font")) {
                Menu {
                    Picker(L("font"), selection: Binding(get: { e.fontFamily }, set: { setProp(ed, e, "font_family", JSON($0)) })) {
                        ForEach(fonts, id: \.self) { Text($0).tag($0) }
                    }
                    Divider()
                    Button { pickingFont = true } label: { Label(L("font_add"), systemImage: "plus") }
                } label: {
                    HStack(spacing: 4) {
                        Text(e.fontFamily).style(.body.with(color: Palette.muted)).lineLimit(1)
                        LIcon(.chevDown, Palette.muted, size: 16)
                    }
                    .contentShape(Rectangle())
                }
                .buttonStyle(.plain)
            }
            RowDivider()
            StepRow(title: L("font_size"), value: e.fontSizePt, range: 4...200, text: "\(num(e.fontSizePt)) pt") { setProp(ed, e, "font_size_pt", JSON($0)) }
            RowDivider()
            ListRow(title: L("text_style")) {
                HStack(spacing: 8) {
                    IconToggle(icon: .bold, on: e.bold, label: L("bold")) { setProp(ed, e, "bold", JSON(!e.bold)) }
                    IconToggle(icon: .italic, on: e.italic, label: L("italic")) { setProp(ed, e, "italic", JSON(!e.italic)) }
                    IconToggle(icon: .underline, on: e.underline, label: L("underline")) { setProp(ed, e, "underline", JSON(!e.underline)) }
                }
            }
        }
        ListGroup {
            SegmentRow(title: L("text_align"), value: e.align, options: [
                ("left", L("align_left")), ("center", L("align_center")), ("right", L("align_right")), ("justify", L("align_justify")),
            ]) { setProp(ed, e, "align", JSON($0)) }
            RowDivider()
            SegmentRow(title: L("text_valign"), value: e.valign, options: [
                ("top", L("align_top")), ("middle", L("align_middle")), ("bottom", L("align_bottom")),
            ]) { setProp(ed, e, "valign", JSON($0)) }
            RowDivider()
            SegmentRow(title: L("text_fit"), value: e.fit, options: [
                ("none", L("fit_fixed")), ("shrink", L("fit_shrink")), ("fill", L("fit_fill")),
            ]) { setProp(ed, e, "fit", JSON($0)) }
            RowDivider()
            StepRow(title: L("line_spacing"), value: e.lineSpacing, step: 0.1, range: 0.5...3, text: num(e.lineSpacing, decimals: 1)) {
                setProp(ed, e, "line_spacing", JSON($0))
            }
            RowDivider()
            StepRow(title: L("letter_spacing"), value: e.letterSpacingMm, step: 0.1, range: -1...5, text: mmText(e.letterSpacingMm)) {
                setProp(ed, e, "letter_spacing_mm", JSON($0))
            }
        }
        ListGroup {
            SwitchRow(title: L("text_wrap"), subtitle: L("text_wrap_sub"), isOn: e.wrap) { setProp(ed, e, "wrap", JSON($0)) }
            RowDivider()
            SwitchRow(title: L("text_invert"), subtitle: L("text_invert_sub"), isOn: e.invert) { setProp(ed, e, "invert", JSON($0)) }
            RowDivider()
            SwitchRow(title: L("text_vertical"), subtitle: L("text_vertical_sub"), isOn: e.direction == "vertical") {
                setProp(ed, e, "direction", $0 ? "vertical" : "horizontal")
            }
            RowDivider()
            ListRow(title: L("max_length"), subtitle: L("max_length_sub")) {
                NumberField(label: "", value: e.maxLengthMm ?? 0) { v in
                    setProp(ed, e, "max_length_mm", v > 0 ? JSON(v) : .null)
                }
                .frame(width: 120)
            }
        }
        if ed.canBind(e) { BindingRow(ed: ed, e: e) }
    }

    private var fontTypes: [UTType] {
        [.font] + ["ttf", "otf", "ttc"].compactMap { UTType(filenameExtension: $0) }
    }

    /// The engine's families; the element's own is listed even when it is not installed.
    private var fonts: [String] {
        ed.app.fonts.contains(e.fontFamily) ? ed.app.fonts : [e.fontFamily] + ed.app.fonts
    }

    /// Several lines; the selection is kept for the range bar below.
    private var editor: some View {
        let shape = RoundedRectangle(cornerRadius: 12)
        return TextEditor(text: $text, selection: $selection)
            .style(.body)
            .focused($focused)
            .scrollContentBackground(.hidden)
            .padding(.horizontal, 8)
            .padding(.vertical, 4)
            .frame(minHeight: 88, maxHeight: 140)
            .background(Palette.surface, in: shape)
            .overlay(shape.strokeBorder(focused ? Palette.primary : Palette.border))
            .onAppear { text = e.text ?? "" }
            .onChange(of: e.text) { if !focused { text = e.text ?? "" } }
            .onChange(of: focused) { if !focused { commit() } }
    }

    /// Apply the edited text. The spans count characters of the old text, so they are cut to
    /// the new length.
    private func commit() {
        guard text != (e.text ?? "") else { return }
        let length = text.unicodeScalars.count
        let spans = TextSpans.of(e).compactMap { s -> TextSpan? in
            var s = s
            s.end = min(s.end, length)
            return s.start < s.end ? s : nil
        }
        var props: [String: JSON] = ["text": JSON(text)]
        if !TextSpans.of(e).isEmpty { props["spans"] = .array(spans.map(\.json)) }
        ed.setProps(e.id, .object(props))
    }

    /// The selected characters as a range of Unicode scalars, when the text is the saved one.
    private var range: Range<Int>? {
        guard text == (e.text ?? ""), case .selection(let r)? = selection?.indices, !r.isEmpty else { return nil }
        let scalars = text.unicodeScalars
        guard let lo = r.lowerBound.samePosition(in: scalars), let hi = r.upperBound.samePosition(in: scalars) else { return nil }
        return scalars.distance(from: scalars.startIndex, to: lo)..<scalars.distance(from: scalars.startIndex, to: hi)
    }

    /// Bold, italic, underline and size of the selected characters only (`spans`).
    private var rangeBar: some View {
        let spans = TextSpans.of(e)
        let range = range
        let base = TextSpan(start: 0, end: 0, bold: e.bold, italic: e.italic, underline: e.underline, fontSizePt: e.fontSizePt)
        let at = range.map { TextSpans.style(at: $0.lowerBound, spans: spans, base: base) } ?? base
        func apply(_ change: @escaping (inout TextSpan) -> Void) {
            guard let range else { return }
            setProp(ed, e, "spans", .array(TextSpans.applying(spans, range: range, change).map(\.json)))
        }
        return VStack(alignment: .leading, spacing: 8) {
            Text(L(range == nil ? "range_hint" : "range_style")).style(.caption)
            HStack(spacing: 8) {
                IconToggle(icon: .bold, on: range != nil && at.bold == true, enabled: range != nil, label: L("bold")) {
                    apply { $0.bold = !(at.bold ?? false) }
                }
                IconToggle(icon: .italic, on: range != nil && at.italic == true, enabled: range != nil, label: L("italic")) {
                    apply { $0.italic = !(at.italic ?? false) }
                }
                IconToggle(icon: .underline, on: range != nil && at.underline == true, enabled: range != nil, label: L("underline")) {
                    apply { $0.underline = !(at.underline ?? false) }
                }
                IconToggle(icon: .minus, enabled: range != nil, label: L("range_smaller")) {
                    apply { $0.fontSizePt = max((at.fontSizePt ?? e.fontSizePt) - 1, 4) }
                }
                IconToggle(icon: .plus, enabled: range != nil, label: L("range_larger")) {
                    apply { $0.fontSizePt = min((at.fontSizePt ?? e.fontSizePt) + 1, 200) }
                }
                Spacer()
                Button {
                    if let range {
                        setProp(ed, e, "spans", .array(TextSpans.clearing(spans, range: range).map(\.json)))
                    } else {
                        setProp(ed, e, "spans", .array([]))
                    }
                } label: {
                    Text(L("range_clear")).style(.bodyStrong.with(color: spans.isEmpty ? Palette.muted : Palette.primary))
                }
                .buttonStyle(.plain)
                .disabled(spans.isEmpty)
            }
        }
    }
}

// MARK: - Codes

private struct CodeProps: View {
    let ed: EditorState
    let e: Element
    let onCode: () -> Void

    var body: some View {
        FieldRow(action: onCode) {
            Text(e.data?.isEmpty == false ? e.data ?? "" : L("data_hint"))
                .style(.body.with(color: e.data?.isEmpty == false ? Palette.text : Palette.muted))
                .lineLimit(1)
                .frame(maxWidth: .infinity, alignment: .leading)
            Text(Symbology.name(e.symbology)).style(.caption)
            LIcon(.chevRight, Palette.muted, size: 18)
        }
        ListGroup {
            if e.type == "barcode" {
                SegmentRow(title: L("human_readable"), value: e.humanReadable, options: [
                    ("none", L("position_none")), ("below", L("position_below")), ("above", L("position_above")),
                ]) { setProp(ed, e, "human_readable", JSON($0)) }
                RowDivider()
                StepRow(title: L("readable_size"), value: e.textSizePt, range: 4...40, text: "\(num(e.textSizePt)) pt") {
                    setProp(ed, e, "text_size_pt", JSON($0))
                }
            } else {
                SegmentRow(title: L("ecc"), value: e.ecc, options: ["L", "M", "Q", "H"].map { ($0, $0) }) { setProp(ed, e, "ecc", JSON($0)) }
                RowDivider()
                ListRow(title: L("ecc_about"), subtitle: L("ecc_sub"))
            }
        }
        if ed.canBind(e) { BindingRow(ed: ed, e: e) }
    }
}

// MARK: - Image

private struct ImageSheet: View {
    let ed: EditorState
    let e: Element

    var body: some View {
        ListGroup {
            SegmentRow(title: L("image_fit"), value: e.fit, options: [
                ("contain", L("fit_contain")), ("cover", L("fit_cover")), ("stretch", L("fit_stretch")),
            ]) { setProp(ed, e, "fit", JSON($0)) }
            RowDivider()
            PickRow(title: L("image_conversion"), value: e.dither, options: Symbology.dither.map { ($0, ditherName($0)) }) {
                setProp(ed, e, "dither", JSON($0))
            }
            RowDivider()
            SliderRow(title: L("dither_threshold"), value: Double(e.threshold), range: 0...255, text: { String(Int($0)) }) {
                setProp(ed, e, "threshold", JSON(Int($0.rounded())))
            }
            RowDivider()
            SliderRow(title: L("brightness"), value: e.brightness, range: -1...1, text: percent) { setProp(ed, e, "brightness", JSON(round2($0))) }
            RowDivider()
            SliderRow(title: L("contrast"), value: e.contrast, range: -1...1, text: percent) { setProp(ed, e, "contrast", JSON(round2($0))) }
            RowDivider()
            SwitchRow(title: L("image_invert"), isOn: e.invert) { setProp(ed, e, "invert", JSON($0)) }
        }
        GroupTitle(L("crop"))
        let c = e.crop ?? (0, 0, 1, 1)
        ListGroup {
            SliderRow(title: L("crop_left"), value: c.left, range: 0...0.95, text: percent) { crop(min($0, c.right - 0.05), c.top, c.right, c.bottom) }
            RowDivider()
            SliderRow(title: L("crop_right"), value: 1 - c.right, range: 0...0.95, text: percent) { crop(c.left, c.top, max(1 - $0, c.left + 0.05), c.bottom) }
            RowDivider()
            SliderRow(title: L("crop_top"), value: c.top, range: 0...0.95, text: percent) { crop(c.left, min($0, c.bottom - 0.05), c.right, c.bottom) }
            RowDivider()
            SliderRow(title: L("crop_bottom"), value: 1 - c.bottom, range: 0...0.95, text: percent) { crop(c.left, c.top, c.right, max(1 - $0, c.top + 0.05)) }
        }
        SecondaryButton(text: L("crop_reset"), icon: .crop, enabled: e.crop != nil) { setProp(ed, e, "crop", .null) }
    }

    private func percent(_ v: Double) -> String { "\(Int((v * 100).rounded())) %" }
    private func round2(_ v: Double) -> Double { (v * 100).rounded() / 100 }

    private func crop(_ l: Double, _ t: Double, _ r: Double, _ b: Double) {
        if l <= 0.001, t <= 0.001, r >= 0.999, b >= 0.999 {
            setProp(ed, e, "crop", .null)
        } else {
            setProp(ed, e, "crop", ["left": JSON(round2(l)), "top": JSON(round2(t)), "right": JSON(round2(r)), "bottom": JSON(round2(b))])
        }
    }
}

// MARK: - Shapes, icons, borders, tables

private struct ShapeProps: View {
    let ed: EditorState
    let e: Element

    var body: some View {
        ListGroup {
            StepRow(title: L("stroke_width"), value: e.strokeMm, step: 0.1, range: (e.isLine ? 0.1 : 0)...10, text: mmText(e.strokeMm)) { w in
                if e.isLine, let dash = LineStyle.of(e.dashMm).dash(strokeMm: w) {
                    // The dashes follow the width.
                    ed.setProps(e.id, ["stroke_mm": JSON(w), "dash_mm": .array(dash.map { JSON($0) })])
                } else {
                    setProp(ed, e, "stroke_mm", JSON(w))
                }
            }
            if e.isLine {
                RowDivider()
                SegmentRow(title: L("line_style"), value: LineStyle.of(e.dashMm), options: [
                    (.solid, L("line_solid")), (.dashed, L("line_dashed")), (.dotted, L("line_dotted")),
                ]) { style in
                    setProp(ed, e, "dash_mm", style.dash(strokeMm: e.strokeMm).map { .array($0.map { JSON($0) }) } ?? .null)
                }
            } else {
                RowDivider()
                SwitchRow(title: L("fill"), subtitle: L("fill_sub"), isOn: e.fill) { setProp(ed, e, "fill", JSON($0)) }
                if e.type == "rect" {
                    RowDivider()
                    StepRow(title: L("corner_radius"), value: e.cornerRadiusMm, step: 0.5, range: 0...50, text: mmText(e.cornerRadiusMm)) {
                        setProp(ed, e, "corner_radius_mm", JSON($0))
                    }
                }
            }
        }
    }
}

private struct IconProps: View {
    let ed: EditorState
    let e: Element
    @State private var picking = false

    var body: some View {
        ListGroup {
            ListRow(title: L("kind_icon"), subtitle: e.iconName.replacingOccurrences(of: "_", with: " "), action: { picking = true }) {
                IconTile(.star)
            } trailing: {
                LIcon(.chevRight, Palette.muted, size: 18)
            }
            RowDivider()
            SwitchRow(title: L("keep_aspect"), isOn: e.keepAspect) { setProp(ed, e, "keep_aspect", JSON($0)) }
            RowDivider()
            SwitchRow(title: L("image_invert"), isOn: e.invert) { setProp(ed, e, "invert", JSON($0)) }
        }
        .sheet(isPresented: $picking) {
            IconPickerSheet(state: ed.app, current: e.iconName, onDismiss: { picking = false }) { name in
                picking = false
                setProp(ed, e, "icon", JSON(name))
            }
        }
    }
}

func borderStyleName(_ style: String) -> String {
    L("border_\(BorderStyles.all.contains(style) ? style : "single")")
}

private struct BorderProps: View {
    let ed: EditorState
    let e: Element

    var body: some View {
        ListGroup {
            PickRow(title: L("border_style"), value: e.borderStyle, options: BorderStyles.all.map { ($0, borderStyleName($0)) }) {
                setProp(ed, e, "style", JSON($0))
            }
            RowDivider()
            StepRow(title: L("stroke_width"), value: e.strokeMm, step: 0.1, range: 0.1...10, text: mmText(e.strokeMm)) {
                setProp(ed, e, "stroke_mm", JSON($0))
            }
            RowDivider()
            StepRow(title: L("corner_radius"), value: e.cornerRadiusMm, step: 0.5, range: 0...50, text: mmText(e.cornerRadiusMm)) {
                setProp(ed, e, "corner_radius_mm", JSON($0))
            }
        }
    }
}

private struct TableProps: View {
    let ed: EditorState
    let e: Element
    let onTable: () -> Void

    var body: some View {
        if let t = TableInfo.of(e) {
            PrimaryButton(text: L("table_edit"), icon: .table, action: onTable)
            ListGroup {
                StepRow(title: L("stroke_width"), subtitle: L("table_stroke_sub"), value: t.strokeMm, step: 0.1, range: 0...5, text: mmText(t.strokeMm)) {
                    setProp(ed, e, "stroke_mm", JSON($0))
                }
                RowDivider()
                StepRow(title: L("font_size"), value: t.fontSizePt, range: 4...100, text: "\(num(t.fontSizePt)) pt") { setProp(ed, e, "font_size_pt", JSON($0)) }
                RowDivider()
                SegmentRow(title: L("text_align"), value: t.align, options: [
                    ("left", L("align_left")), ("center", L("align_center")), ("right", L("align_right")),
                ]) { setProp(ed, e, "align", JSON($0)) }
                RowDivider()
                PickRow(title: L("font"), value: e.fontFamily, options: (ed.app.fonts.contains(e.fontFamily) ? ed.app.fonts : [e.fontFamily] + ed.app.fonts).map { ($0, $0) }) {
                    setProp(ed, e, "font_family", JSON($0))
                }
            }
        }
    }
}

/// The table editor: pick a cell in the grid, edit its text and style, and add, remove, merge
/// and size rows and columns (`element.table_edit`).
struct TableSheet: View {
    let ed: EditorState
    let onDismiss: () -> Void
    @State private var row = 0
    @State private var col = 0

    var body: some View {
        let e = ed.selected
        SheetBody(title: L("table_edit"), onClose: close) {
            if let e, let t = TableInfo.of(e) {
                let r = min(row, t.rows - 1)
                let c = min(col, t.columns - 1)
                // A covered cell edits the merge's top-left cell.
                let m = t.merge(at: r, c)
                let (cr, cc) = (m?.row ?? r, m?.col ?? c)
                let cell = t.cell(cr, cc)
                grid(t, cr, cc)

                GroupTitle(L("table_cell", cr + 1, cc + 1))
                CommitField(value: cell.text, placeholder: L("table_cell_text"), axis: .vertical) {
                    edit(e, ["op": "set_cell", "row": JSON(cr), "col": JSON(cc), "text": JSON($0)])
                }
                .id("\(cr)-\(cc)")
                HStack(spacing: 8) {
                    IconToggle(icon: .bold, on: cell.bold, label: L("bold")) {
                        edit(e, ["op": "set_cell", "row": JSON(cr), "col": JSON(cc), "bold": JSON(!cell.bold)])
                    }
                    let align = cell.align ?? t.align
                    ForEach([("left", Icon.tLeft), ("center", .tCenter), ("right", .tRight)], id: \.0) { a, icon in
                        IconToggle(icon: icon, on: align == a, label: L("align_\(a)")) {
                            edit(e, ["op": "set_cell", "row": JSON(cr), "col": JSON(cc), "align": JSON(a)])
                        }
                    }
                    IconToggle(icon: .rect, on: cell.invert, label: L("text_invert")) {
                        edit(e, ["op": "set_cell", "row": JSON(cr), "col": JSON(cc), "invert": JSON(!cell.invert)])
                    }
                    Spacer()
                    let size = cell.fontSizePt ?? t.fontSizePt
                    CountStepper(
                        value: num(size),
                        onMinus: { edit(e, ["op": "set_cell", "row": JSON(cr), "col": JSON(cc), "font_size_pt": JSON(max(size - 1, 4))]) },
                        onPlus: { edit(e, ["op": "set_cell", "row": JSON(cr), "col": JSON(cc), "font_size_pt": JSON(min(size + 1, 100))]) }
                    )
                }

                GroupTitle(L("table_merge"))
                HStack(spacing: 8) {
                    let (mr, mc) = (m?.rows ?? 1, m?.cols ?? 1)
                    SecondaryButton(text: L("merge_right"), enabled: cc + mc < t.columns) {
                        edit(e, ["op": "merge", "row": JSON(cr), "col": JSON(cc), "rows": JSON(mr), "cols": JSON(mc + 1)])
                    }
                    SecondaryButton(text: L("merge_down"), enabled: cr + mr < t.rows) {
                        edit(e, ["op": "merge", "row": JSON(cr), "col": JSON(cc), "rows": JSON(mr + 1), "cols": JSON(mc)])
                    }
                    SecondaryButton(text: L("split"), enabled: m != nil) {
                        edit(e, ["op": "split", "row": JSON(cr), "col": JSON(cc)])
                    }
                }

                ListGroup {
                    ListRow(title: L("table_row", r + 1)) {
                        HStack(spacing: 4) {
                            IconButton(.up, label: L("row_insert_above")) {
                                edit(e, ["op": "insert_row", "at": JSON(r)])
                                row = r + 1
                            }
                            IconButton(.down, label: L("row_insert_below")) { edit(e, ["op": "insert_row", "at": JSON(r + 1)]) }
                            IconButton(.trash, tint: Palette.red, enabled: t.rows > 1, label: L("row_delete")) {
                                edit(e, ["op": "delete_row", "at": JSON(r)])
                                row = max(r - 1, 0)
                            }
                        }
                    }
                    RowDivider()
                    StepRow(title: L("row_height"), subtitle: L("table_relative"), value: t.rowHeights[r], step: 0.25, range: 0.25...10) {
                        edit(e, ["op": "resize_row", "at": JSON(r), "height": JSON($0)])
                    }
                    RowDivider()
                    ListRow(title: L("table_column", c + 1)) {
                        HStack(spacing: 4) {
                            IconButton(.chevLeft, label: L("column_insert_before")) {
                                edit(e, ["op": "insert_column", "at": JSON(c)])
                                col = c + 1
                            }
                            IconButton(.chevRight, label: L("column_insert_after")) { edit(e, ["op": "insert_column", "at": JSON(c + 1)]) }
                            IconButton(.trash, tint: Palette.red, enabled: t.columns > 1, label: L("column_delete")) {
                                edit(e, ["op": "delete_column", "at": JSON(c)])
                                col = max(c - 1, 0)
                            }
                        }
                    }
                    RowDivider()
                    StepRow(title: L("column_width"), subtitle: L("table_relative"), value: t.columnWidths[c], step: 0.25, range: 0.25...10) {
                        edit(e, ["op": "resize_column", "at": JSON(c), "width": JSON($0)])
                    }
                }
                SecondaryButton(text: L("table_clear"), icon: .trash) { edit(e, ["op": "clear"]) }
            } else {
                Text(L("inspector_hint")).style(.caption).frame(maxWidth: .infinity).padding(.vertical, 24)
            }
        }
        .presentationDetents([.large])
        .onChange(of: e == nil) { if e == nil { onDismiss() } }
    }

    private func close() {
        endEditing()
        onDismiss()
    }

    private func edit(_ e: Element, _ op: JSON) {
        endEditing()
        ed.tableEdit(e.id, op)
    }

    /// The cells as buttons, sized by the relative widths and heights; merged cells span.
    private func grid(_ t: TableInfo, _ sr: Int, _ sc: Int) -> some View {
        GeometryReader { geo in
            let totalW = max(t.columnWidths.reduce(0, +), 0.01)
            let totalH = max(t.rowHeights.reduce(0, +), 0.01)
            let height = geo.size.height
            let xs = t.columnWidths.reduce(into: [0.0]) { $0.append($0[$0.count - 1] + $1 / totalW * geo.size.width) }
            let ys = t.rowHeights.reduce(into: [0.0]) { $0.append($0[$0.count - 1] + $1 / totalH * height) }
            ZStack(alignment: .topLeading) {
                ForEach(0..<t.rows, id: \.self) { r in
                    ForEach(0..<t.columns, id: \.self) { c in
                        if !t.isCovered(r, c) {
                            let m = t.merge(at: r, c)
                            let w = xs[c + (m?.cols ?? 1)] - xs[c]
                            let h = ys[r + (m?.rows ?? 1)] - ys[r]
                            let cell = t.cell(r, c)
                            let on = r == sr && c == sc
                            Button {
                                endEditing()
                                row = r
                                col = c
                            } label: {
                                Text(cell.text)
                                    .style(.caption.with(size: 12, weight: cell.bold ? .bold : .regular, color: cell.invert ? Palette.surface : Palette.text))
                                    .lineLimit(2)
                                    .minimumScaleFactor(0.6)
                                    .padding(2)
                                    .frame(width: max(w, 1), height: max(h, 1))
                                    .background(cell.invert ? Palette.text : on ? Palette.primaryTint : Palette.surface)
                                    .overlay(Rectangle().strokeBorder(on ? Palette.primary : Palette.border, lineWidth: on ? 2 : 0.5))
                                    .contentShape(Rectangle())
                            }
                            .buttonStyle(.plain)
                            .offset(x: xs[c], y: ys[r])
                            .accessibilityLabel(L("table_cell", r + 1, c + 1))
                            .accessibilityAddTraits(on ? .isSelected : [])
                        }
                    }
                }
            }
        }
        .frame(height: min(max(Double(t.rows) * 40, 80), 220))
        .clipShape(RoundedRectangle(cornerRadius: 8))
        .overlay(RoundedRectangle(cornerRadius: 8).strokeBorder(Palette.border))
    }
}
