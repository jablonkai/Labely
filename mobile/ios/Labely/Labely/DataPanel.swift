import LabelyKit
import SwiftUI

/// M4 · Adatok és rekordok: the attached data source, the record stepper and the rows. It takes
/// the inspector's place; the canvas above shows the previewed record.
struct DataPanel: View {
    let ed: EditorState
    /// Open the file picker.
    let onChoose: () -> Void

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
            IconTile(.data, size: 48)
            Text(L("data_empty_title")).style(.bodyStrong.with(size: 17))
            Text(L("data_empty_hint")).style(.caption).multilineTextAlignment(.center)
            PrimaryButton(text: L("data_choose"), icon: .open, action: onChoose).padding(.top, 4)
        }
        .padding(16)
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
                Button(L("data_replace"), action: onChoose)
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
