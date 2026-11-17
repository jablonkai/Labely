import LabelyKit
import SwiftUI

/// M10 · Beállítások, reduced to what the shell has: language and Bluetooth (both in the app's
/// page of the Settings app), the assistant print consent (ADR-0033) and version information.
struct SettingsScreen: View {
    @Bindable var state: AppState
    @Environment(\.openURL) private var openURL

    var body: some View {
        let version = Bundle.main.object(forInfoDictionaryKey: "CFBundleShortVersionString") as? String ?? ""
        ScrollView {
            VStack(alignment: .leading, spacing: 16) {
                LargeTitle(title: L("tab_settings"))
                ListGroup {
                    ListRow(title: L("settings_language"), subtitle: L("settings_language_sub"), action: openSettings) {
                        IconTile(.info)
                    } trailing: {
                        LIcon(.chevRight, Palette.muted, size: 18)
                    }
                    RowDivider()
                    ListRow(title: L("settings_permissions"), subtitle: L("settings_permissions_sub"), action: openSettings) {
                        IconTile(.bluetooth)
                    } trailing: {
                        LIcon(.chevRight, Palette.muted, size: 18)
                    }
                }
                ListGroup {
                    ListRow(title: L("settings_agent_print"), subtitle: L("settings_agent_print_sub")) {
                        IconTile(.printer)
                    } trailing: {
                        Toggle("", isOn: $state.allowAgentPrint).labelsHidden()
                    }
                }
                ListGroup {
                    ListRow(title: L("settings_version")) { IconTile(.label) } trailing: { Text(version).style(.caption) }
                    RowDivider()
                    ListRow(title: L("settings_engine")) { IconTile(.data) } trailing: {
                        Text(L("engine_version", state.engine.apiVersion)).style(.caption)
                    }
                }
            }
            .padding(16)
        }
    }

    /// The app's page in the Settings app, where iOS lists its language and Bluetooth access.
    private func openSettings() {
        if let url = URL(string: UIApplication.openSettingsURLString) { openURL(url) }
    }
}
