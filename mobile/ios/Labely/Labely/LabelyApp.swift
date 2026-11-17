import LabelyKit
import SwiftUI
import UniformTypeIdentifiers

@main
struct LabelyApp: App {
    @State private var state: AppState = Shared.state

    var body: some Scene {
        WindowGroup {
            RootView(state: state)
                // The design is light only (as on Android).
                .preferredColorScheme(.light)
                .tint(Palette.primary)
        }
    }
}

/// The engine and the UI state for the life of the process (ADR-0032), shared by the screens
/// and the App Intents (ADR-0033), which may run before any scene exists.
@MainActor
enum Shared {
    static let state = makeState()
    static let assistant = Assistant(app: state, simulatorName: L("simulator"), templateName: displayName)

    /// The engine's data lives in Application Support; labels are saved in its `labels` folder.
    private static func makeState() -> AppState {
        let fm = FileManager.default
        let support = fm.urls(for: .applicationSupportDirectory, in: .userDomainMask)[0]
        try? fm.createDirectory(at: support, withIntermediateDirectories: true)
        #if DEBUG
            let debug = true
        #else
            let debug = false
        #endif
        let state = AppState(
            engine: Engine(dataDir: support.path, ble: CoreBle()),
            labelsDir: support.appendingPathComponent("labels", isDirectory: true),
            debug: debug
        )
        state.newLabelName = L("new_label_name")
        state.start()
        return state
    }
}

struct RootView: View {
    @Bindable var state: AppState
    @State private var newLabel = false
    @State private var importing = false

    var body: some View {
        ZStack(alignment: .bottom) {
            if let editor = state.editor {
                EditorScreen(ed: editor, onClose: { state.closeEditor() })
                    .transition(.move(edge: .trailing))
            } else {
                VStack(spacing: 0) {
                    ZStack {
                        switch state.tab {
                        case .home: HomeScreen(state: state, onNewLabel: { newLabel = true }, onOpenFile: { importing = true })
                        case .templates: TemplatesScreen(state: state, onNewLabel: { newLabel = true })
                        case .history: HistoryScreen(state: state)
                        case .printers: PrintersScreen(state: state)
                        case .settings: SettingsScreen(state: state)
                        }
                    }
                    .frame(maxHeight: .infinity)
                    TabBar(current: $state.tab)
                }
                .background(Palette.bg)
            }
            Toast(state: state).padding(.bottom, 80)
        }
        .animation(.easeInOut(duration: 0.25), value: state.editor?.docId)
        .sheet(isPresented: $newLabel) {
            LabelSheet(
                title: L("new_label"),
                confirm: L("create"),
                initial: (AppState.presetSizes[0].0, AppState.presetSizes[0].1, "gap"),
                onDismiss: { newLabel = false }
            ) { w, h, media in
                newLabel = false
                state.newLabel(widthMm: w, heightMm: h, media: media)
            }
        }
        .fileImporter(isPresented: $importing, allowedContentTypes: [.data]) { result in
            switch result {
            case .success(let url): state.openFile(url)
            case .failure: state.message = .text(L("open_failed"))
            }
        }
    }
}

private struct TabBar: View {
    @Binding var current: AppTab

    var body: some View {
        VStack(spacing: 0) {
            Rectangle().fill(Palette.border).frame(height: 1)
            HStack(spacing: 0) {
                item(.home, .home, "tab_home")
                item(.templates, .templates, "tab_templates")
                item(.history, .clock, "tab_history")
                item(.printers, .printer, "tab_printers")
                item(.settings, .settings, "tab_settings")
            }
            .frame(height: 56)
        }
        .background(Palette.surface.ignoresSafeArea(edges: .bottom))
    }

    private func item(_ tab: AppTab, _ icon: Icon, _ key: String) -> some View {
        let on = current == tab
        return Button {
            current = tab
        } label: {
            VStack(spacing: 3) {
                LIcon(icon, on ? Palette.primary : Palette.muted, size: 24)
                Text(L(key))
                    .style(.small.with(weight: on ? .semibold : .medium, color: on ? Palette.primaryDark : Palette.muted))
                    .lineLimit(1)
            }
            .frame(maxWidth: .infinity, maxHeight: .infinity)
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .accessibilityAddTraits(on ? .isSelected : [])
    }
}

/// The one-line message, shown for a few seconds above the tab bar.
private struct Toast: View {
    @Bindable var state: AppState

    var body: some View {
        if let notice = state.message {
            Text(text(notice))
                .style(.bodyStrong.with(color: .white))
                .padding(.horizontal, 16)
                .padding(.vertical, 12)
                .background(Palette.text.opacity(0.92), in: RoundedRectangle(cornerRadius: 12))
                .padding(.horizontal, 16)
                .transition(.move(edge: .bottom).combined(with: .opacity))
                .onTapGesture { state.message = nil }
                .task(id: notice) {
                    try? await Task.sleep(for: .seconds(3.5))
                    if state.message == notice { state.message = nil }
                }
        }
    }

    private func text(_ n: Notice) -> String {
        switch n {
        case .text(let t): t
        case .printerLost: L("printer_lost")
        case .calibrated: L("calibrated")
        }
    }
}
