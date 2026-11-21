import LabelyKit
import SwiftUI

/// M3 · Nyomtatás: preview, printer, records, copies, density, the advanced print options, the
/// check result and the print button.
struct PrintSheet: View {
    private static let maxCopies = 999
    private static let maxCount = 5000

    private enum Records: Hashable {
        case current, all, range
    }

    let ed: EditorState
    let onDismiss: () -> Void
    @State private var copies = 1
    /// `nil` is the label's density, else the model's default.
    @State private var density: Int?
    @State private var picking = false
    @State private var records = Records.current
    @State private var range = ""
    /// Labels to print without data.
    @State private var count = 1
    @State private var copiesColumn = ""
    @State private var advanced = false
    @State private var offsetX = 0.0
    @State private var offsetY = 0.0
    @State private var mirror = false
    @State private var rotate180 = false
    @State private var jobs = false

    var body: some View {
        let app = ed.app
        let printer = app.printer
        let model = printer?.model
        let rows = ed.data.map(\.rowCount) ?? 0
        let options = options(rows)
        let labels = ed.labelCount(options)
        let rangeOk = records != .range || RecordRange.parse(range, count: rows).map { !$0.isEmpty } == true
        SheetBody(title: L("print"), onClose: close) {
            if let doc = ed.doc {
                preview(doc, rows)
                if let h = app.reprint { Banner(text: L(h.partial ? "print_continue_note" : "print_again_note", h.pages, h.pagesTotal), ok: true) }

                Button { picking.toggle() } label: {
                    Card(selected: printer != nil) {
                        ListRow(
                            title: model?.name ?? printer?.name ?? L("choose_printer"),
                            subtitle: printer != nil
                                ? [app.printerStatus?.batteryPercent.map { "\($0)%" }, sizeText(doc.label.widthMm, doc.label.heightMm)]
                                .compactMap { $0 }.joined(separator: " · ")
                                : L("no_printer_hint")
                        ) {
                            IconTile(.printer, tint: printer != nil ? Palette.green : Palette.muted, fill: printer != nil ? Palette.greenTint : Palette.bg)
                        } trailing: {
                            LIcon(.chevDown, Palette.muted, size: 18)
                        }
                    }
                }
                .buttonStyle(.plain)
                if picking || printer == nil { PrinterPicker(state: app) }

                ListGroup {
                    if rows > 0 {
                        ListRow(title: L("records")) {
                            Picker(L("records"), selection: $records) {
                                Text(L("records_current")).tag(Records.current)
                                Text(L("records_all", rows)).tag(Records.all)
                                Text(L("records_range")).tag(Records.range)
                            }
                            .pickerStyle(.segmented)
                            .fixedSize()
                        }
                        if records == .range {
                            VStack(alignment: .leading, spacing: 6) {
                                LabeledInput(text: $range, placeholder: "1,3,5-7", plain: true)
                                Text(rangeOk ? L("records_range_hint", rows) : L("records_range_bad", rows))
                                    .style(.caption.with(color: rangeOk ? Palette.muted : Palette.red))
                            }
                            .padding(.horizontal, 16)
                            .padding(.bottom, 12)
                        }
                        RowDivider()
                    } else {
                        ListRow(title: L("print_count"), subtitle: ed.serials.isEmpty ? nil : L("print_count_sub")) {
                            CountStepper(
                                value: String(count),
                                onMinus: { count = max(count - 1, 1) },
                                onPlus: { count = min(count + 1, Self.maxCount) }
                            )
                        }
                        RowDivider()
                    }
                    if copiesColumn.isEmpty {
                        ListRow(title: L("copies")) {
                            CountStepper(
                                value: String(copies),
                                onMinus: { copies = max(copies - 1, 1) },
                                onPlus: { copies = min(copies + 1, Self.maxCopies) }
                            )
                        }
                    }
                    if rows > 0, let columns = ed.data?.columns {
                        if copiesColumn.isEmpty { RowDivider() }
                        PickRow(
                            title: L("copies_column"), subtitle: L("copies_column_sub"), value: copiesColumn,
                            options: [("", L("binding_none"))] + columns.map { ($0, $0) }
                        ) { copiesColumn = $0 }
                    }
                    RowDivider()
                    densityRow(doc, model)
                }

                ListGroup {
                    SwitchRow(title: L("print_advanced"), subtitle: advancedSummary, isOn: advanced) { advanced = $0 }
                    if advanced {
                        RowDivider()
                        HStack(spacing: 10) {
                            NumberField(label: L("offset_x"), value: offsetX) { offsetX = min(max($0, -10), 10) }
                            NumberField(label: L("offset_y"), value: offsetY) { offsetY = min(max($0, -10), 10) }
                        }
                        .padding(.horizontal, 16)
                        .padding(.vertical, 12)
                        RowDivider()
                        SwitchRow(title: L("print_mirror"), subtitle: L("print_mirror_sub"), isOn: mirror) { mirror = $0 }
                        RowDivider()
                        SwitchRow(title: L("print_rotate"), subtitle: L("print_rotate_sub"), isOn: rotate180) { rotate180 = $0 }
                    }
                }

                banner(labels)

                if let p = ed.progress, !p.finished {
                    VStack(alignment: .leading, spacing: 8) {
                        Text(p.total > 0 ? L("printing_progress", p.done, p.total) : L("printing_waiting")).style(.bodyStrong)
                        ProgressView(value: p.total > 0 ? Double(p.done) / Double(p.total) : 0)
                        SecondaryButton(text: L("print_cancel"), icon: .close, enabled: ed.jobId != nil) { ed.cancelPrint() }
                    }
                } else {
                    PrimaryButton(
                        text: labels.map { L("print_labels", $0) } ?? L("print"),
                        icon: .printer,
                        enabled: printer != nil && app.busy == nil && rangeOk && labels != 0
                            && ed.issues?.contains { $0.severity == "error" } != true
                    ) {
                        endEditing()
                        ed.print(options)
                    }
                }
                Button { jobs = true } label: {
                    Text(L("print_queue")).style(.body.with(weight: .semibold, color: Palette.primary)).frame(maxWidth: .infinity)
                }
                .buttonStyle(.plain)
            }
        }
        .presentationDetents([.large])
        .sheet(isPresented: $jobs) { JobsSheet(state: app) { jobs = false } }
        .task(id: printer?.address) { await ed.check().value }
        .onChange(of: model?.id) { density = nil }
        .onAppear(perform: start)
    }

    /// Print again takes the job as it was; continuing a part-finished one starts after the
    /// pages that were printed.
    private func start() {
        guard let h = ed.app.reprint else { return }
        let rows = ed.data.map(\.rowCount) ?? 0
        copies = max(h.copies, 1)
        if rows > 0 {
            records = .all
            if h.partial {
                let done = h.pages / max(h.copies, 1)
                if done > 0, done < rows {
                    records = .range
                    range = "\(done + 1)-"
                }
            }
        } else {
            let labels = max(h.labels, 1)
            count = h.partial ? max(labels - h.pages / max(h.copies, 1), 1) : labels
        }
    }

    private func options(_ rows: Int) -> PrintOptions {
        var o = PrintOptions()
        o.copies = copies
        o.density = density
        if rows > 0 {
            switch records {
            case .current: o.records = String(ed.record)
            case .all: o.records = "all"
            case .range: o.records = range
            }
            o.copiesColumn = copiesColumn.isEmpty ? nil : copiesColumn
        } else {
            o.count = count
        }
        o.offsetXMm = offsetX
        o.offsetYMm = offsetY
        o.mirror = mirror
        o.rotate180 = rotate180
        return o
    }

    private var advancedSummary: String {
        var parts: [String] = []
        if offsetX != 0 || offsetY != 0 { parts.append("\(mm(offsetX)) / \(mmText(offsetY))") }
        if mirror { parts.append(L("print_mirror")) }
        if rotate180 { parts.append(L("print_rotate")) }
        return parts.isEmpty ? L("print_advanced_sub") : parts.joined(separator: " · ")
    }

    /// The preview; with data, the arrows step through the records.
    private func preview(_ doc: Doc, _ rows: Int) -> some View {
        VStack(spacing: 10) {
            LabelImage(image: ed.preview, widthMm: doc.label.widthMm, heightMm: doc.label.heightMm)
                .containerRelativeFrame(.horizontal) { w, _ in w * 0.6 }
                .scaleEffect(x: mirror ? -1 : 1, y: 1)
                .rotationEffect(.degrees(rotate180 ? 180 : 0))
            if rows > 0 {
                HStack(spacing: 4) {
                    IconButton(.chevLeft, tint: Palette.text, size: 34, enabled: ed.record > 1, label: L("record_previous")) { ed.setRecord(ed.record - 1) }
                    Text(L("record_of", ed.record, rows)).style(.bodyStrong).monospacedDigit()
                    IconButton(.chevRight, tint: Palette.text, size: 34, enabled: ed.record < rows, label: L("record_next")) { ed.setRecord(ed.record + 1) }
                }
            }
        }
        .padding(rows > 0 ? 16 : 24)
        .frame(maxWidth: .infinity)
        .background(Palette.canvas, in: RoundedRectangle(cornerRadius: 16))
    }

    private func densityRow(_ doc: Doc, _ model: Model?) -> some View {
        let minD = model?.densityMin ?? 1
        let maxD = model?.densityMax ?? 5
        let fallback = doc.label.density ?? model?.densityDefault ?? 3
        let current = density ?? fallback
        return VStack(alignment: .leading, spacing: 8) {
            HStack {
                Text(L("density")).style(.bodyStrong.with(size: 16))
                Spacer()
                Text("\(current) / \(maxD)").style(.bodyStrong)
            }
            HStack(spacing: 10) {
                Chip(text: L(doc.label.density == nil ? "density_model" : "density_label"), selected: density == nil) { density = nil }
                if maxD > minD {
                    Slider(
                        value: Binding(get: { Double(current) }, set: { density = Int($0.rounded()) }),
                        in: Double(minD)...Double(maxD),
                        step: 1
                    )
                }
            }
        }
        .padding(.horizontal, 16)
        .padding(.vertical, 12)
    }

    private func close() {
        ed.clearProgress()
        ed.app.reprint = nil
        onDismiss()
    }

    @ViewBuilder private func banner(_ labels: Int?) -> some View {
        if let p = ed.progress, p.state == "done" {
            Banner(text: L("print_done"), ok: true, trailing: L("labels", p.done))
        } else if let p = ed.progress, p.state == "cancelled" {
            Banner(text: L("print_cancelled", p.done, p.total), ok: false)
        } else if let p = ed.progress, p.finished {
            Banner(text: L("print_failed", p.error ?? p.state), ok: false)
        } else if let issues = ed.issues {
            if let first = issues.first(where: { $0.severity == "error" }) ?? issues.first(where: { $0.severity == "warning" }) {
                Banner(text: first.message, ok: false)
            } else {
                Banner(text: L("check_ok"), ok: true, trailing: labels.map { L("labels", $0) })
            }
        }
    }
}

/// Known printers and a scan, inside the print sheet.
private struct PrinterPicker: View {
    let state: AppState

    var body: some View {
        let known = knownPrinters(state)
        let found = (state.scanResults ?? []).filter { d in
            !known.contains { $0.address == d.address } && d.address != state.printer?.address
        }
        let rows = known.map { ($0.address, $0.name) } + found.map { ($0.address, $0.name) }
        ListGroup {
            ForEach(Array(rows.enumerated()), id: \.element.0) { i, row in
                if i > 0 { RowDivider() }
                PrinterRow(state: state, address: row.0, name: row.1)
            }
            if !rows.isEmpty { RowDivider() }
            SecondaryButton(
                text: L(state.busy == AppState.scanning ? "scanning" : "printers_search"),
                icon: .bluetooth,
                enabled: state.busy == nil
            ) {
                withBluetooth(state) { state.scan() }
            }
            .padding(12)
        }
    }
}

/// Name and colours of a job state.
@MainActor
func jobState(_ state: String) -> (text: String, fg: Color, bg: Color) {
    switch state {
    case "queued": (L("job_queued"), Palette.text2, Palette.field)
    case "printing": (L("job_printing"), Palette.primary, Palette.primaryTint)
    case "done": (L("job_done"), Palette.green, Palette.greenTint)
    case "cancelled": (L("job_cancelled"), Palette.amber, Palette.amberTint)
    default: (L("job_failed"), Palette.red, Palette.red.opacity(0.12))
    }
}

struct StatePill: View {
    let state: String

    var body: some View {
        let s = jobState(state)
        Text(s.text).style(.caption.with(weight: .semibold, color: s.fg))
            .padding(.horizontal, 10)
            .padding(.vertical, 4)
            .background(s.bg, in: Capsule())
    }
}

/// The print queue: queued, printing and finished jobs. A job can be cancelled, the queue
/// paused and resumed, and the finished jobs cleared.
struct JobsSheet: View {
    let state: AppState
    let onDismiss: () -> Void

    var body: some View {
        let jobs = Array(state.jobs.reversed())
        SheetBody(title: L("print_queue"), onClose: onDismiss) {
            if state.queuePaused { Banner(text: L("queue_paused"), ok: false) }
            HStack(spacing: 12) {
                SecondaryButton(text: L(state.queuePaused ? "queue_resume" : "queue_pause"), icon: state.queuePaused ? .play : .pause) {
                    if state.queuePaused { state.resumeQueue() } else { state.pauseQueue() }
                }
                SecondaryButton(text: L("queue_clear"), icon: .trash, enabled: jobs.contains(where: \.finished)) { state.clearJobs() }
            }
            ListGroup {
                if jobs.isEmpty { ListRow(title: L("queue_empty"), subtitle: L("queue_empty_sub")) }
                ForEach(Array(jobs.enumerated()), id: \.element.id) { i, j in
                    if i > 0 { RowDivider() }
                    VStack(alignment: .leading, spacing: 8) {
                        HStack(spacing: 12) {
                            IconTile(.printer, tint: jobState(j.state).fg, fill: jobState(j.state).bg)
                            VStack(alignment: .leading, spacing: 2) {
                                Text(j.docName.isEmpty ? j.id : j.docName).style(.bodyStrong.with(size: 16)).lineLimit(1)
                                Text(L("job_pages", j.pagesDone, j.pagesTotal) + " · " + printerName(j.address)).style(.caption).lineLimit(1)
                            }
                            .frame(maxWidth: .infinity, alignment: .leading)
                            StatePill(state: j.state)
                            if !j.finished {
                                IconButton(.close, tint: Palette.red, size: 34, label: L("print_cancel")) { state.cancelJob(j.id) }
                            }
                        }
                        if j.state == "printing", j.pagesTotal > 0 {
                            ProgressView(value: Double(j.pagesDone) / Double(j.pagesTotal))
                        }
                        if let error = j.error, j.state == "failed" { Text(error).style(.caption.with(color: Palette.red)) }
                    }
                    .padding(.horizontal, 16)
                    .padding(.vertical, 12)
                }
            }
        }
        .presentationDetents([.medium, .large])
        .task { await state.refreshJobs() }
    }

    /// The name the printer at `address` is known by.
    private func printerName(_ address: String) -> String {
        if address == state.printer?.address, let name = state.printer?.model?.name { return name }
        return state.known.first { $0.address == address }?.name ?? address
    }
}
