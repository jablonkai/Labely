import LabelyKit
import SwiftUI

/// M10 · Beállítások: units, the canvas defaults, the paper behaviour, language and Bluetooth
/// (both in the app's page of the Settings app), the assistant print consent (ADR-0033) and
/// About.
struct SettingsScreen: View {
    static let repository = "https://github.com/jablonkai/Labely"
    static let licence = "MIT OR Apache-2.0"

    @Bindable var state: AppState
    @Environment(\.openURL) private var openURL
    @State private var about = false

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 16) {
                LargeTitle(title: L("tab_settings"))
                ListGroup {
                    SegmentRow(title: L("settings_units"), value: state.prefs.unit, options: [
                        (.mm, L("unit_mm")), (.inch, L("unit_inch")),
                    ]) { state.prefs.unit = $0 }
                }
                CanvasOptions(state: state)
                GroupTitle(L("papers"))
                ListGroup {
                    SwitchRow(title: L("paper_auto"), subtitle: L("paper_auto_sub"), icon: .scan, isOn: state.prefs.autoApplyPaper) {
                        state.prefs.autoApplyPaper = $0
                    }
                }
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
                    ListRow(title: L("about"), subtitle: L("about_sub", version), action: { about = true }) {
                        IconTile(.label)
                    } trailing: {
                        LIcon(.chevRight, Palette.muted, size: 18)
                    }
                }
            }
            .padding(16)
        }
        .sheet(isPresented: $about) { aboutSheet }
    }

    private var version: String {
        Bundle.main.object(forInfoDictionaryKey: "CFBundleShortVersionString") as? String ?? ""
    }

    private var aboutSheet: some View {
        SheetBody(title: L("about"), onClose: { about = false }) {
            VStack(spacing: 6) {
                Text(L("app_name")).style(.largeTitle)
                Text(L("about_tagline")).style(.caption).multilineTextAlignment(.center)
            }
            .frame(maxWidth: .infinity)
            .padding(.vertical, 12)
            ListGroup {
                ListRow(title: L("settings_version")) { IconTile(.label) } trailing: { Text(version).style(.caption) }
                RowDivider()
                ListRow(title: L("settings_engine")) { IconTile(.data) } trailing: {
                    Text(L("engine_version", state.engine.apiVersion)).style(.caption)
                }
                RowDivider()
                ListRow(title: L("about_licence"), subtitle: L("about_licence_sub")) { IconTile(.file) } trailing: {
                    Text(Self.licence).style(.caption)
                }
                RowDivider()
                ListRow(title: L("about_source"), subtitle: Self.repository, action: {
                    if let url = URL(string: Self.repository) { openURL(url) }
                }) {
                    IconTile(.link)
                } trailing: {
                    LIcon(.chevRight, Palette.muted, size: 18)
                }
            }
        }
        .presentationDetents([.medium, .large])
    }

    /// The app's page in the Settings app, where iOS lists its language and Bluetooth access.
    private func openSettings() {
        if let url = URL(string: UIApplication.openSettingsURLString) { openURL(url) }
    }
}
