import LabelyKit
import SwiftUI

/// M4 · Adatok és rekordok: the attached data source, the record stepper and the rows. It takes
/// the inspector's place; the canvas above shows the previewed record.
struct DataPanel: View {
    let ed: EditorState
    /// Open the file picker.
    let onChoose: () -> Void
    /// Pick images: one record per image.
    let onImages: () -> Void
    /// Open the data options: sheet, header row, columns and serial numbers.
    let onOptions: () -> Void
    /// Scan a code with the camera; it is added as an element.
    let onScan: () -> Void

    var body: some View {
        VStack(spacing: 0) {
            if let data = ed.data {
                header(data)
                Divider().overlay(Palette.border)
                rows(data)
            } else {
                empty
            }
        }
        .frame(height: 256)
    }

    private var empty: some View {
        VStack(spacing: 10) {
            Text(L("data_empty_title")).style(.bodyStrong.with(size: 17))
            Text(L("data_empty_hint")).style(.caption).multilineTextAlignment(.center)
            PrimaryButton(text: L("data_choose"), icon: .open, action: onChoose).padding(.top, 4)
            HStack(spacing: 8) {
                SecondaryButton(text: L("data_images"), icon: .image, action: onImages)
                SecondaryButton(text: ed.serials.isEmpty ? L("data_serials") : L("data_serials_n", ed.serials.count), icon: .hash, action: onOptions)
            }
            SecondaryButton(text: L("scan_add"), icon: .scan, action: onScan)
        }
        .padding(.horizontal, 16)
        .padding(.vertical, 8)
        .frame(maxWidth: .infinity, maxHeight: .infinity)
    }

    private func header(_ data: DataInfo) -> some View {
        HStack(spacing: 10) {
            IconTile(.file, tint: Palette.green, fill: Palette.greenTint)
            VStack(alignment: .leading, spacing: 2) {
                Text(ed.dataSourceName ?? L("data_unnamed")).style(.bodyStrong.with(size: 16)).lineLimit(1)
                Text(L("data_summary", data.rowCount, data.columns.count, ed.boundColumns)).style(.caption).lineLimit(1)
                    .minimumScaleFactor(0.8)
            }
            .frame(maxWidth: .infinity, alignment: .leading)
            Menu {
                Button(L("data_options"), action: onOptions)
                Button(L("data_replace"), action: onChoose)
                Button(L("data_images"), action: onImages)
                Button(L("scan_add"), action: onScan)
                Button(L("data_remove"), role: .destructive) { ed.detachData() }
            } label: {
                LIcon(.settings, Palette.text2, size: 20).frame(width: 36, height: 36).contentShape(Rectangle())
            }
            .accessibilityLabel(L("tool_data"))
            if data.rowCount > 0 {
                HStack(spacing: 0) {
                    IconButton(.chevLeft, tint: Palette.text, size: 32, enabled: ed.record > 1, label: "‹") { ed.setRecord(ed.record - 1) }
                    Text("\(ed.record) / \(data.rowCount)").style(.bodyStrong).monospacedDigit().lineLimit(1).fixedSize()
                    IconButton(.chevRight, tint: Palette.text, size: 32, enabled: ed.record < data.rowCount, label: "›") { ed.setRecord(ed.record + 1) }
                }
            }
        }
        .padding(.leading, 16)
        .padding(.trailing, 8)
        .frame(height: 64)
    }

    private func rows(_ data: DataInfo) -> some View {
        ScrollViewReader { proxy in
            ScrollView {
                LazyVStack(spacing: 0) {
                    ForEach(Array(ed.rows.enumerated()), id: \.offset) { i, row in
                        RecordRow(number: i + 1, row: row, selected: ed.record == i + 1) { ed.setRecord(i + 1) }
                            .id(i + 1)
                            .onAppear { if i == ed.rows.count - 1 { ed.loadMoreRows() } }
                        RowDivider()
                    }
                }
            }
            // The stepper may move to a record that is out of sight.
            .onChange(of: ed.record) { withAnimation { proxy.scrollTo(ed.record) } }
        }
    }
}

/// One record: its number, the first column, and the next two below it.
private struct RecordRow: View {
    let number: Int
    let row: [String]
    let selected: Bool
    let action: () -> Void

    var body: some View {
        Button(action: action) {
            HStack(spacing: 12) {
                Text("\(number)").style(.caption.with(size: 14)).monospacedDigit().frame(minWidth: 22, alignment: .trailing)
                VStack(alignment: .leading, spacing: 1) {
                    Text(row.first ?? "").style(.bodyStrong.with(size: 16)).lineLimit(1)
                    let rest = row.dropFirst().prefix(2).filter { !$0.isEmpty }.joined(separator: " · ")
                    if !rest.isEmpty { Text(rest).style(.caption).lineLimit(1) }
                }
                .frame(maxWidth: .infinity, alignment: .leading)
            }
            .padding(.horizontal, 16)
            .frame(minHeight: 52)
            .background(selected ? Palette.primaryTint : Palette.surface)
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .accessibilityAddTraits(selected ? .isSelected : [])
    }
}

/// The column picker of an element's binding: the columns of the data, and "None".
struct BindingMenu<Label: View>: View {
    let ed: EditorState
    let e: Element
    @ViewBuilder let label: Label

    var body: some View {
        Menu {
            Picker(L("binding"), selection: Binding(
                get: { ed.boundColumn(e) ?? "" },
                set: { ed.bind(e, to: $0.isEmpty ? nil : $0) }
            )) {
                Text(L("binding_none")).tag("")
                ForEach(ed.data?.columns ?? [], id: \.self) { Text($0).tag($0) }
            }
        } label: {
            label
        }
        .buttonStyle(.plain)
    }
}

/// The "Binding" row of the inspector: the bound column, opening the column picker.
struct BindingRow: View {
    let ed: EditorState
    let e: Element

    var body: some View {
        BindingMenu(ed: ed, e: e) {
            FieldRow {
                LIcon(.link, Palette.primary, size: 18)
                Text(L("binding")).style(.bodyStrong)
                Text(ed.boundColumn(e).map { L("binding_column", $0) } ?? L("binding_none"))
                    .style(.body.with(color: Palette.muted)).lineLimit(1)
                    .frame(maxWidth: .infinity, alignment: .trailing)
                LIcon(.chevRight, Palette.muted, size: 18)
            }
        }
    }
}

/// A bordered row in the inspector's field style; a button with `action`.
struct FieldRow<Content: View>: View {
    var action: (() -> Void)?
    @ViewBuilder let content: Content

    var body: some View {
        let shape = RoundedRectangle(cornerRadius: 12)
        let row = HStack(spacing: 8) { content }
            .padding(.horizontal, 12)
            .frame(height: 44)
            .background(Palette.surface, in: shape)
            .overlay(shape.strokeBorder(Palette.border))
            .contentShape(shape)
        if let action {
            Button(action: action) { row }.buttonStyle(.plain)
        } else {
            row
        }
    }
}

/// The data options: the sheet and header row of the attached file, an element for each
/// column, and the serial numbers.
struct DataOptionsSheet: View {
    let ed: EditorState
    let onDismiss: () -> Void
    @State private var editing: Serial?

    var body: some View {
        SheetBody(title: L("data_options"), onClose: onDismiss) {
            if let data = ed.data {
                GroupTitle(ed.dataSourceName ?? L("data_unnamed"))
                if ed.dataHeader != nil || !ed.dataSheets.isEmpty {
                    ListGroup {
                        if ed.dataSheets.count > 0, let sheet = ed.dataSheet {
                            PickRow(title: L("data_sheet"), value: sheet, options: ed.dataSheets.map { ($0, $0) }) { ed.reattachData(sheet: $0) }
                            RowDivider()
                        }
                        if let header = ed.dataHeader {
                            SwitchRow(title: L("data_header"), subtitle: L("data_header_sub"), isOn: header) { ed.reattachData(header: $0) }
                        }
                    }
                } else {
                    Text(L("data_reopen_hint")).style(.caption)
                }

                GroupTitle(L("data_columns"))
                Text(L("data_columns_hint")).style(.caption)
                ListGroup {
                    ForEach(Array(data.columns.enumerated()), id: \.offset) { i, column in
                        if i > 0 { RowDivider() }
                        ListRow(title: column, subtitle: ed.rows.first.flatMap { i < $0.count ? $0[i] : nil }) {
                            addMenu(column)
                        }
                    }
                }
            }

            GroupTitle(L("data_serials"))
            Text(L("serial_hint")).style(.caption)
            ListGroup {
                if ed.serials.isEmpty { ListRow(title: L("serial_none")) }
                ForEach(Array(ed.serials.enumerated()), id: \.element.id) { i, s in
                    if i > 0 { RowDivider() }
                    ListRow(title: "{{\(s.name)}}", subtitle: (0..<3).map(s.value).joined(separator: ", ") + " …", action: { editing = s }) {
                        IconTile(.hash)
                    } trailing: {
                        addMenu(s.name)
                    }
                }
            }
            SecondaryButton(text: L("serial_add"), icon: .plus, height: 52) { editing = Serial(name: ed.freeSerialName) }
        }
        .presentationDetents([.large])
        .sheet(item: $editing) { s in
            SerialSheet(ed: ed, serial: s, isNew: !ed.serials.contains { $0.name == s.name }) { editing = nil }
        }
    }

    /// "Add an element bound to this column": a text, a barcode or a 2D code.
    private func addMenu(_ column: String) -> some View {
        Menu {
            Button { add(column, "text") } label: { Label(L("kind_text"), systemImage: "textformat") }
            Button { add(column, "barcode") } label: { Label(L("kind_barcode"), systemImage: "barcode") }
            Button { add(column, "code2d") } label: { Label(L("kind_code2d"), systemImage: "qrcode") }
        } label: {
            HStack(spacing: 4) {
                LIcon(.plus, Palette.primary, size: 16)
                Text(L("data_add_bound")).style(.bodyStrong.with(size: 14, color: Palette.primary))
            }
            .padding(.horizontal, 10)
            .padding(.vertical, 6)
            .background(Palette.primaryTint, in: Capsule())
        }
        .buttonStyle(.plain)
    }

    private func add(_ column: String, _ type: String) {
        ed.addBound(column: column, type: type)
        onDismiss()
    }
}

/// One serial counter: its name, first value, step, digits, prefix and suffix, with a sample.
private struct SerialSheet: View {
    let ed: EditorState
    let isNew: Bool
    let onDismiss: () -> Void
    @State private var s: Serial

    init(ed: EditorState, serial: Serial, isNew: Bool, onDismiss: @escaping () -> Void) {
        self.ed = ed
        self.isNew = isNew
        self.onDismiss = onDismiss
        _s = State(initialValue: serial)
    }

    var body: some View {
        SheetBody(title: L("serial_title"), onClose: onDismiss) {
            Banner(text: (0..<3).map(s.value).joined(separator: ", ") + " …", ok: true)
            if isNew {
                GroupTitle(L("serial_name"))
                LabeledInput(text: $s.name, placeholder: "serial", plain: true)
            } else {
                GroupTitle("{{\(s.name)}}")
            }
            ListGroup {
                ListRow(title: L("serial_start")) {
                    NumberField(label: "", value: Double(s.start), suffix: "", length: false) { s.start = Int($0.rounded()) }.frame(width: 140)
                }
                RowDivider()
                StepRow(title: L("serial_step"), value: Double(s.step), range: -1000...1000, text: String(s.step)) { s.step = Int($0) }
                RowDivider()
                StepRow(title: L("serial_digits"), subtitle: L("serial_digits_sub"), value: Double(s.digits), range: 0...18, text: String(s.digits)) {
                    s.digits = Int($0)
                }
            }
            HStack(spacing: 10) {
                VStack(alignment: .leading, spacing: 6) {
                    GroupTitle(L("serial_prefix"))
                    LabeledInput(text: $s.prefix, placeholder: "", plain: true)
                }
                VStack(alignment: .leading, spacing: 6) {
                    GroupTitle(L("serial_suffix"))
                    LabeledInput(text: $s.suffix, placeholder: "", plain: true)
                }
            }
            HStack(spacing: 12) {
                if !isNew {
                    SecondaryButton(text: L("delete"), icon: .trash, height: 52) {
                        ed.removeSerial(s.name)
                        onDismiss()
                    }
                }
                PrimaryButton(text: L("save"), enabled: !s.name.trimmingCharacters(in: .whitespaces).isEmpty) {
                    endEditing()
                    s.name = s.name.trimmingCharacters(in: .whitespaces)
                    ed.setSerial(s)
                    onDismiss()
                }
            }
            if !isNew {
                SecondaryButton(text: L("serial_add_text"), icon: .text) {
                    ed.addBound(column: s.name)
                    onDismiss()
                }
            }
        }
        .presentationDetents([.large])
    }
}
