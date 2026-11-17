import LabelyKit
import SwiftUI

/// M9 · Előzmények: the print history; a tap reopens the printed document.
struct HistoryScreen: View {
    let state: AppState

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 16) {
                LargeTitle(title: L("tab_history"), sub: L("history_sub"))
                ListGroup {
                    if state.history.isEmpty { ListRow(title: L("history_empty")) }
                    ForEach(Array(state.history.enumerated()), id: \.element.id) { i, h in
                        if i > 0 { RowDivider() }
                        let ok = h.state == "done"
                        ListRow(
                            title: h.docName,
                            subtitle: String(h.printedAt.prefix(16)).replacingOccurrences(of: "T", with: " ") + " · " + L("labels", h.pages),
                            action: { state.openHistory(h) }
                        ) {
                            IconTile(ok ? .printer : .warn, tint: ok ? Palette.primary : Palette.amber, fill: ok ? Palette.primaryTint : Palette.amberTint)
                        } trailing: {
                            if !ok { Text(L("history_failed")).style(.caption.with(color: Palette.amber)) }
                        }
                    }
                }
            }
            .padding(16)
        }
        .task { await state.refreshHistory() }
        .refreshable { await state.refreshHistory() }
    }
}
