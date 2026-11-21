import LabelyKit
import SwiftUI

/// M6 · Vonalkód: symbology, value with its validity, the human-readable text and the binding
/// of the selected barcode or 2D code.
struct BarcodeSheet: View {
    let ed: EditorState
    let onDismiss: () -> Void
    @State private var value = ""
    /// The engine's reason when it cannot encode the value.
    @State private var problem: String?
    @FocusState private var focused: Bool

    var body: some View {
        let e = isCode(ed.selected) ? ed.selected : nil
        SheetBody(title: L(e?.type == "code2d" ? "kind_code2d" : "kind_barcode"), onClose: close) {
            if let e {
                symbologies(e)
                Text(L("code_value")).style(.bodyStrong.with(color: Palette.text2))
                VStack(alignment: .leading, spacing: 8) {
                    field(e)
                    validity(e)
                }
                ListGroup {
                    if e.type == "barcode" {
                        let shown = e.humanReadable != "none"
                        ListRow(title: L("human_readable"), subtitle: L("human_readable_sub")) {
                            Toggle("", isOn: Binding(
                                get: { shown },
                                set: { ed.setProps(e.id, ["human_readable": $0 ? "below" : "none"]) }
                            ))
                            .labelsHidden()
                            .tint(Palette.primary)
                        }
                        if ed.canBind(e) {
                            RowDivider()
                            binding(e)
                        }
                        if shown {
                            RowDivider()
                            ListRow(title: L("text_position")) {
                                Picker(L("text_position"), selection: Binding(
                                    get: { e.humanReadable },
                                    set: { ed.setProps(e.id, ["human_readable": JSON($0)]) }
                                )) {
                                    Text(L("position_below")).tag("below")
                                    Text(L("position_above")).tag("above")
                                }
                                .pickerStyle(.segmented)
                                .fixedSize()
                            }
                        }
                    } else if ed.canBind(e) {
                        binding(e)
                    }
                }
            }
        }
        .presentationDetents([.fraction(0.65), .large])
        .task(id: e?.id) {
            guard let e else { return onDismiss() }
            value = e.data ?? ""
            problem = await ed.codeIssue(e.id)
        }
        // A binding or an undo changed the value underneath.
        .onChange(of: e?.data) { if !focused, let data = e?.data { value = data } }
    }

    private func close() {
        // The field commits when it loses focus.
        endEditing()
        onDismiss()
    }

    /// The symbologies this element may switch to: 1D among 1D, 2D among 2D.
    private func symbologies(_ e: Element) -> some View {
        ScrollView(.horizontal, showsIndicators: false) {
            HStack(spacing: 8) {
                ForEach(Symbology.of(e.type), id: \.self) { s in
                    Chip(text: Symbology.name(s), selected: e.symbology == s) {
                        guard e.symbology != s else { return }
                        Task { problem = await ed.setCode(e.id, ["symbology": JSON(s)]) }
                    }
                }
            }
            .padding(.vertical, 1)
        }
    }

    private func field(_ e: Element) -> some View {
        let shape = RoundedRectangle(cornerRadius: 12)
        return HStack(spacing: 8) {
            TextField("", text: $value, prompt: Text(L("data_hint")).foregroundStyle(Palette.muted))
                .style(.body)
                .autocorrectionDisabled()
                .textInputAutocapitalization(.never)
                .focused($focused)
                .submitLabel(.done)
                .onSubmit { commit(e) }
            if ed.boundColumn(e) != nil { LIcon(.link, Palette.primary, size: 18) }
        }
        .padding(.horizontal, 12)
        .frame(height: 48)
        .background(Palette.surface, in: shape)
        .overlay(shape.strokeBorder(problem != nil ? Palette.red : focused ? Palette.primary : Palette.border))
        .onChange(of: focused) { if !focused { commit(e) } }
    }

    /// Apply the value; when the engine cannot encode it, the old one stays and its reason shows.
    private func commit(_ e: Element) {
        guard value != e.data else { return }
        let new = value
        Task { problem = await ed.setCode(e.id, ["data": JSON(new)]) }
    }

    @ViewBuilder private func validity(_ e: Element) -> some View {
        if let problem {
            HStack(alignment: .top, spacing: 6) {
                LIcon(.warn, Palette.red, size: 16)
                Text(problem).style(.caption.with(color: Palette.red))
            }
        } else if let column = ed.boundColumn(e) {
            HStack(spacing: 6) {
                LIcon(.link, Palette.muted, size: 16)
                Text(L("code_bound", column)).style(.caption)
            }
        } else {
            HStack(spacing: 6) {
                LIcon(.check, Palette.green, size: 16)
                let digit = Gtin.checkDigit(symbology: e.symbology, data: e.data ?? "")
                Text(digit.map { L("code_valid_check", $0) } ?? L("code_valid")).style(.caption.with(color: Palette.green))
            }
        }
    }

    private func binding(_ e: Element) -> some View {
        BindingMenu(ed: ed, e: e) {
            ListRow(
                title: L("binding"),
                subtitle: ed.boundColumn(e).map { column in
                    [L("binding_column", column), ed.dataSourceName].compactMap { $0 }.joined(separator: " · ")
                } ?? L("binding_none")
            ) {
                IconTile(.link)
            } trailing: {
                LIcon(.chevRight, Palette.muted, size: 18)
            }
        }
    }
}
