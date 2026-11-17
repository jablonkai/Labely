import LabelyKit
import SwiftUI

/// M3 · Nyomtatás: preview, printer, copies, density, the check result and the print button.
struct PrintSheet: View {
    private static let maxCopies = 999

    let ed: EditorState
    let onDismiss: () -> Void
    @State private var copies = 1
    @State private var density: Double?
    @State private var picking = false

    var body: some View {
        let app = ed.app
        let printer = app.printer
        let model = printer?.model
        let minD = model?.densityMin ?? 1
        let maxD = model?.densityMax ?? 5
        let current = Int(density ?? Double(ed.doc?.label.density ?? model?.densityDefault ?? 3))
        SheetBody(title: L("print"), onClose: close) {
            if let doc = ed.doc {
                LabelImage(image: ed.preview, widthMm: doc.label.widthMm, heightMm: doc.label.heightMm)
                    .containerRelativeFrame(.horizontal) { w, _ in w * 0.6 }
                    .padding(24)
                    .frame(maxWidth: .infinity)
                    .background(Palette.canvas, in: RoundedRectangle(cornerRadius: 16))

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
                    ListRow(title: L("copies")) {
                        CountStepper(
                            value: String(copies),
                            onMinus: { copies = max(copies - 1, 1) },
                            onPlus: { copies = min(copies + 1, Self.maxCopies) }
                        )
                    }
                    RowDivider()
                    ListRow(title: L("density")) {
                        if maxD > minD {
                            Slider(
                                value: Binding(get: { Double(current) }, set: { density = $0.rounded() }),
                                in: Double(minD)...Double(maxD),
                                step: 1
                            )
                            .frame(width: 140)
                        }
                        Text("\(current) / \(maxD)").style(.bodyStrong)
                    }
                }

                banner

                if let p = ed.progress, !p.finished {
                    VStack(alignment: .leading, spacing: 8) {
                        Text(L("printing_progress", p.done, p.total)).style(.bodyStrong)
                        ProgressView(value: p.total > 0 ? Double(p.done) / Double(p.total) : 0)
                    }
                } else {
                    PrimaryButton(
                        text: L("print_labels", copies),
                        icon: .printer,
                        enabled: printer != nil && app.busy == nil && ed.issues?.contains { $0.severity == "error" } != true
                    ) {
                        ed.print(copies: copies, density: current)
                    }
                }
            }
        }
        .presentationDetents([.large])
        .task(id: printer?.address) { await ed.check().value }
        .onChange(of: model?.id) { density = nil }
    }

    private func close() {
        ed.clearProgress()
        onDismiss()
    }

    @ViewBuilder private var banner: some View {
        if let p = ed.progress, p.state == "done" {
            Banner(text: L("print_done"), ok: true, trailing: L("labels", p.done))
        } else if let p = ed.progress, p.finished {
            Banner(text: L("print_failed", p.error ?? p.state), ok: false)
        } else if let issues = ed.issues {
            if let first = issues.first(where: { $0.severity == "error" }) ?? issues.first(where: { $0.severity == "warning" }) {
                Banner(text: first.message, ok: false)
            } else {
                Banner(text: L("check_ok"), ok: true, trailing: L("labels", copies))
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
