import LabelyKit
import SwiftUI

/// `Name · 50 × 30 mm`.
func paperTitle(_ p: Paper) -> String {
    p.name.isEmpty ? sizeText(p.widthMm, p.heightMm) : "\(p.name) · \(sizeText(p.widthMm, p.heightMm))"
}

/// The label paper library: the bundled sizes and the user's own, with search. Tapping a paper
/// uses it for the open label (or hands it to `onPick`, for a new label). The roll in the
/// printer is read from its RFID tag and can be linked to a paper, so it is recognised next
/// time.
struct PaperSheet: View {
    @Bindable var state: AppState
    /// The open document, when the sheet is shown from the editor.
    var ed: EditorState?
    /// For a new label: the picked paper instead of applying it.
    var onPick: ((Paper) -> Void)?
    let onDismiss: () -> Void

    @State private var query = ""
    @State private var fitting = false
    @State private var mineOnly = false
    @State private var adding = false
    @State private var deleting: Paper?
    @State private var reading = false

    var body: some View {
        let papers = state.papers.filter { !mineOnly || $0.custom }
        SheetBody(title: L("papers"), onClose: onDismiss) {
            loaded
            SearchField(text: $query, placeholder: L("papers_search"))
            HStack(spacing: 8) {
                Chip(text: L("papers_fitting"), selected: fitting) { fitting.toggle() }
                Chip(text: L("papers_mine"), selected: mineOnly) { mineOnly.toggle() }
                Spacer()
                Text(L("papers_count", papers.count)).style(.caption)
            }
            ListGroup {
                if papers.isEmpty { ListRow(title: L("papers_none")) }
                ForEach(Array(papers.enumerated()), id: \.element.id) { i, p in
                    if i > 0 { RowDivider() }
                    row(p)
                }
            }
            SecondaryButton(text: L("paper_add"), icon: .plus, height: 52) { adding = true }
            ListGroup {
                SwitchRow(title: L("paper_auto"), subtitle: L("paper_auto_sub"), icon: .scan, isOn: state.prefs.autoApplyPaper) {
                    state.prefs.autoApplyPaper = $0
                }
            }
        }
        .presentationDetents([.large])
        .task(id: [query, fitting ? "1" : "0"]) { await state.loadPapers(query: query, fitting: fitting) }
        .task(id: state.printer?.address) { await read() }
        .sheet(isPresented: $adding) {
            AddPaperSheet(state: state, barcode: unknownBarcode, initial: ed?.doc.map { ($0.label.widthMm, $0.label.heightMm, $0.label.media) }) {
                adding = false
            }
        }
        .alert(
            L("paper_delete_title"),
            isPresented: Binding(get: { deleting != nil }, set: { if !$0 { deleting = nil } }),
            presenting: deleting
        ) { p in
            Button(L("delete"), role: .destructive) { state.deletePaper(p) }
            Button(L("cancel"), role: .cancel) {}
        } message: { p in
            Text(L("paper_delete_body", paperTitle(p)))
        }
    }

    /// The roll as the document's comparison, else as the app read it.
    private var detect: PaperDetect? { ed?.loadedPaper ?? state.detected }

    /// The barcode of a loaded roll that no paper is linked to yet.
    private var unknownBarcode: String? {
        guard let d = detect, d.rfid, d.paper == nil else { return nil }
        return d.barcode
    }

    private func read() async {
        guard state.printer != nil else { return }
        reading = true
        await state.detectPaper()
        await ed?.refreshLoadedPaper()
        reading = false
    }

    /// The roll in the printer: which paper it is, how many labels are left, and whether the
    /// label matches it.
    @ViewBuilder private var loaded: some View {
        if state.printer == nil {
            Banner(text: L("paper_no_printer"), ok: false)
        } else if let d = detect {
            Card(selected: d.matchesLabel == true) {
                VStack(alignment: .leading, spacing: 10) {
                    HStack(spacing: 12) {
                        let ok = d.rfid && d.paper != nil && d.matchesLabel != false
                        IconTile(ok ? .check : .warn, tint: ok ? Palette.green : Palette.amber, fill: ok ? Palette.greenTint : Palette.amberTint)
                        VStack(alignment: .leading, spacing: 2) {
                            Text(loadedTitle(d)).style(.bodyStrong.with(size: 16))
                            Text(loadedSub(d)).style(.caption).fixedSize(horizontal: false, vertical: true)
                        }
                        .frame(maxWidth: .infinity, alignment: .leading)
                        if reading {
                            ProgressView()
                        } else {
                            IconButton(.refresh, label: L("paper_read")) { Task { await read() } }
                        }
                    }
                    if let p = d.paper, d.matchesLabel != true {
                        PrimaryButton(text: L("paper_use_size"), icon: .check) { use(p) }
                    }
                    if let p = d.paper, let barcode = d.barcode {
                        Button { state.unlinkPaper(barcode: barcode) } label: {
                            Text(L("paper_unlink", p.name)).style(.body.with(weight: .semibold, color: Palette.primary))
                        }
                        .buttonStyle(.plain)
                    }
                }
                .padding(14)
            }
        } else {
            SecondaryButton(text: L(reading ? "paper_reading" : "paper_read"), icon: .scan, enabled: !reading, height: 52) {
                Task { await read() }
            }
        }
    }

    private func loadedTitle(_ d: PaperDetect) -> String {
        if !d.rfid { return L("paper_no_tag") }
        if let p = d.paper { return L("paper_loaded_is", paperTitle(p)) }
        return L("paper_unknown_roll")
    }

    private func loadedSub(_ d: PaperDetect) -> String {
        if !d.rfid { return L("paper_no_tag_sub") }
        var parts: [String] = []
        if let media = d.media { parts.append(mediaName(media)) }
        if let n = d.remaining { parts.append(L("labels_left", n)) }
        if d.paper == nil { parts.append(L("paper_unknown_hint")) } else if d.matchesLabel == false { parts.append(L("paper_differs")) }
        return parts.joined(separator: " · ")
    }

    private func use(_ p: Paper) {
        if let onPick {
            onPick(p)
        } else {
            ed?.applyPaper(p.id)
        }
        onDismiss()
    }

    private func row(_ p: Paper) -> some View {
        let current = ed?.doc.map { $0.label.widthMm == p.widthMm && $0.label.heightMm == p.heightMm && $0.label.media == p.media } ?? false
        return ListRow(
            title: p.name.isEmpty ? sizeText(p.widthMm, p.heightMm) : p.name,
            subtitle: ([sizeText(p.widthMm, p.heightMm), mediaName(p.media)] + (p.barcodes.isEmpty ? [] : [L("paper_linked", p.barcodes.count)]))
                .joined(separator: " · "),
            action: { use(p) }
        ) {
            IconTile(p.custom ? .edit : .label, tint: current ? Palette.green : Palette.primary, fill: current ? Palette.greenTint : Palette.primaryTint)
        } trailing: {
            Menu {
                Button { use(p) } label: { Label(L("paper_use_size"), systemImage: "checkmark") }
                if let barcode = unknownBarcode {
                    Button { state.linkPaper(barcode: barcode, to: p) } label: { Label(L("paper_link_roll"), systemImage: "link") }
                }
                ForEach(p.barcodes, id: \.self) { b in
                    Button { state.unlinkPaper(barcode: b) } label: { Label(L("paper_unlink_code", b), systemImage: "link.badge.plus") }
                }
                if p.custom {
                    Button(role: .destructive) { deleting = p } label: { Label(L("delete"), systemImage: "trash") }
                }
            } label: {
                LIcon(.more, Palette.muted, size: 20).frame(width: 36, height: 36).contentShape(Rectangle())
            }
            .accessibilityLabel(L("more"))
        }
    }
}

/// Add a paper of your own: name, size, corners and type; the loaded roll can be linked to it.
private struct AddPaperSheet: View {
    let state: AppState
    /// The loaded roll's barcode, when no paper is linked to it.
    let barcode: String?
    let onDismiss: () -> Void
    @State private var name = ""
    @State private var w: Double
    @State private var h: Double
    @State private var radius = 0.0
    @State private var media: String
    @State private var link = true

    init(state: AppState, barcode: String?, initial: (Double, Double, String)?, onDismiss: @escaping () -> Void) {
        self.state = state
        self.barcode = barcode
        self.onDismiss = onDismiss
        _w = State(initialValue: initial?.0 ?? 50)
        _h = State(initialValue: initial?.1 ?? 30)
        _media = State(initialValue: initial?.2 ?? "gap")
    }

    var body: some View {
        let trimmed = name.trimmingCharacters(in: .whitespaces)
        SheetBody(title: L("paper_add"), onClose: onDismiss) {
            GroupTitle(L("paper_name"))
            LabeledInput(text: $name, placeholder: L("paper_name_hint"))
            HStack(spacing: 10) {
                NumberField(label: L("width"), value: w, live: true, signed: false) { w = $0 }
                NumberField(label: L("height"), value: h, live: true, signed: false) { h = $0 }
            }
            ListGroup {
                PickRow(title: L("paper_type"), value: media, options: mediaKinds.map { ($0, mediaName($0)) }) { media = $0 }
                RowDivider()
                StepRow(title: L("corner_radius"), value: radius, step: 0.5, range: 0...20, text: mmText(radius)) { radius = $0 }
                if barcode != nil {
                    RowDivider()
                    SwitchRow(title: L("paper_link_roll"), subtitle: L("paper_link_roll_sub"), isOn: link) { link = $0 }
                }
            }
            HStack(spacing: 12) {
                SecondaryButton(text: L("cancel"), height: 52) { onDismiss() }
                PrimaryButton(text: L("save"), enabled: !trimmed.isEmpty && w > 0 && h > 0) {
                    endEditing()
                    state.addPaper(name: trimmed, widthMm: w, heightMm: h, cornerRadiusMm: radius, media: media, barcode: link ? barcode : nil)
                    onDismiss()
                }
            }
        }
        .presentationDetents([.medium, .large])
    }
}
