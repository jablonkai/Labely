import LabelyKit
import SwiftUI

/// M9 · Előzmények: the print history with printer and status. A tap reopens the printed
/// document; a job can be printed again, a part-finished one continued, and entries deleted.
struct HistoryScreen: View {
    let state: AppState
    @State private var jobs = false
    @State private var clearing = false

    var body: some View {
        let active = state.jobs.filter { !$0.finished }.count
        ScrollView {
            VStack(alignment: .leading, spacing: 16) {
                LargeTitle(title: L("tab_history"), sub: L("history_sub")) {
                    IconButton(.printer, tint: Palette.primary, fill: Palette.primaryTint, size: 52, label: L("print_queue")) { jobs = true }
                        .overlay(alignment: .topTrailing) {
                            if active > 0 {
                                Text("\(active)").style(.small.with(weight: .bold, color: .white))
                                    .frame(minWidth: 18, minHeight: 18)
                                    .background(Palette.red, in: Circle())
                            }
                        }
                }
                if active > 0 || state.queuePaused {
                    Button { jobs = true } label: {
                        Banner(text: state.queuePaused ? L("queue_paused") : L("queue_active", active), ok: !state.queuePaused)
                    }
                    .buttonStyle(.plain)
                }
                ListGroup {
                    if state.history.isEmpty { ListRow(title: L("history_empty")) }
                    ForEach(Array(state.history.enumerated()), id: \.element.id) { i, h in
                        if i > 0 { RowDivider() }
                        row(h)
                    }
                }
                if !state.history.isEmpty {
                    SecondaryButton(text: L("history_clear"), icon: .trash) { clearing = true }
                }
            }
            .padding(16)
        }
        .task {
            await state.refreshHistory()
            await state.refreshJobs()
        }
        .refreshable { await state.refreshHistory() }
        .sheet(isPresented: $jobs) { JobsSheet(state: state) { jobs = false } }
        .alert(L("history_clear_title"), isPresented: $clearing) {
            Button(L("history_clear"), role: .destructive) { state.clearHistory() }
            Button(L("cancel"), role: .cancel) {}
        } message: {
            Text(L("history_clear_body"))
        }
    }

    private func row(_ h: HistoryEntry) -> some View {
        let ok = h.state == "done"
        let printer = state.known.first { $0.address == h.address }?.name ?? h.model ?? transportName(h.address)
        let pages = h.partial ? L("job_pages", h.pages, h.pagesTotal) : L("labels", h.pages)
        return HStack(spacing: 12) {
            Button {
                if h.hasFile { state.openHistory(h) }
            } label: {
                HStack(spacing: 12) {
                    IconTile(ok ? .printer : .warn, tint: ok ? Palette.primary : Palette.amber, fill: ok ? Palette.primaryTint : Palette.amberTint)
                    VStack(alignment: .leading, spacing: 2) {
                        Text(h.docName).style(.bodyStrong.with(size: 16)).lineLimit(1)
                        Text(String(h.printedAt.prefix(16)).replacingOccurrences(of: "T", with: " ") + " · " + pages).style(.caption).lineLimit(1)
                        Text(printer).style(.caption).lineLimit(1)
                    }
                    .frame(maxWidth: .infinity, alignment: .leading)
                    StatePill(state: h.state)
                }
                .contentShape(Rectangle())
            }
            .buttonStyle(.plain)
            Menu {
                if h.hasFile {
                    Button { state.openHistory(h) } label: { Label(L("history_open"), systemImage: "doc") }
                    Button { state.printAgain(h) } label: {
                        Label(L(h.partial ? "history_continue" : "history_reprint"), systemImage: "printer")
                    }
                }
                Button(role: .destructive) { state.deleteHistory(h) } label: { Label(L("delete"), systemImage: "trash") }
            } label: {
                LIcon(.more, Palette.muted, size: 20).frame(width: 36, height: 44).contentShape(Rectangle())
            }
            .accessibilityLabel(L("more"))
        }
        .padding(.leading, 16)
        .padding(.trailing, 6)
        .padding(.vertical, 10)
    }
}
