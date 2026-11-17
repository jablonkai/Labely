import LabelyKit
import SwiftUI

/// M1 · Kezdőlap: quick start, starter templates and recent labels.
struct HomeScreen: View {
    let state: AppState
    let onNewLabel: () -> Void
    let onOpenFile: () -> Void
    @State private var query = ""

    var body: some View {
        let starters = state.templates.filter { $0.starter && matches(displayName($0)) }
        let recents = state.recents.filter { matches($0.name) }
        ScrollView {
            VStack(alignment: .leading, spacing: 16) {
                header
                VStack(alignment: .leading, spacing: 4) {
                    Text(L("home_greeting")).style(.body.with(color: Palette.muted))
                    Text(L("home_title")).style(.largeTitle.with(size: 26))
                }
                SearchField(text: $query, placeholder: L("home_search"))
                HStack(spacing: 10) {
                    ActionCard(icon: .plus, title: L("home_new"), sub: L("home_new_sub"), primary: true, action: onNewLabel)
                    ActionCard(icon: .templates, title: L("home_from_template"), sub: L("home_from_template_sub")) {
                        state.tab = .templates
                    }
                    ActionCard(icon: .open, title: L("home_open"), sub: L("home_open_sub"), action: onOpenFile)
                }
                ScrollView(.horizontal, showsIndicators: false) {
                    HStack(spacing: 8) {
                        ForEach(Array(AppState.presetSizes.enumerated()), id: \.offset) { i, size in
                            Chip(text: sizeText(size.0, size.1), selected: i == 0) {
                                state.newLabel(widthMm: size.0, heightMm: size.1)
                            }
                        }
                    }
                }
                SectionHeader(title: L("home_starters"), action: L("see_all")) { state.tab = .templates }
                TemplateGrid(templates: Array(starters.prefix(4)), state: state)
                if !recents.isEmpty {
                    SectionHeader(title: L("home_recent"))
                    ForEach(recents) { r in
                        Button { state.openRecent(r) } label: {
                            Card {
                                HStack(spacing: 12) {
                                    ThumbTile(image: state.thumbs[r.path], widthMm: r.widthMm, heightMm: r.heightMm)
                                        .frame(width: 96, height: 60)
                                    VStack(alignment: .leading) {
                                        Text(r.name).style(.bodyStrong).lineLimit(1)
                                        Text(sizeText(r.widthMm, r.heightMm)).style(.caption)
                                    }
                                    .frame(maxWidth: .infinity, alignment: .leading)
                                    LIcon(.chevRight, Palette.muted, size: 18)
                                }
                                .padding(12)
                            }
                        }
                        .buttonStyle(.plain)
                    }
                }
            }
            .padding(16)
        }
        .scrollDismissesKeyboard(.immediately)
    }

    private func matches(_ name: String) -> Bool {
        query.isEmpty || name.localizedCaseInsensitiveContains(query)
    }

    private var header: some View {
        HStack(spacing: 10) {
            // The app mark: a primary rounded square with a dot and a bar.
            HStack(spacing: 3) {
                Circle().fill(.white).frame(width: 4, height: 4)
                Capsule().fill(.white).frame(width: 9, height: 3)
            }
            .padding(.leading, 5)
            .frame(width: 26, height: 20, alignment: .leading)
            .background(Palette.primary, in: RoundedRectangle(cornerRadius: 5))
            Text(L("app_name")).style(.title.with(size: 24))
            Spacer()
            PrinterPill(state: state)
        }
    }
}

/// Connected printer and battery, or a prompt to connect one.
struct PrinterPill: View {
    let state: AppState

    var body: some View {
        let p = state.printer
        Button { state.tab = .printers } label: {
            HStack(spacing: 6) {
                Circle().fill(p != nil ? Palette.green : Palette.muted).frame(width: 8, height: 8)
                Text(p?.model?.name ?? p?.name ?? L("no_printer")).style(.bodyStrong.with(size: 13)).lineLimit(1)
                if let b = state.printerStatus?.batteryPercent { Text("· \(b)%").style(.caption) }
            }
            .padding(.horizontal, 12)
            .padding(.vertical, 7)
            .background(p != nil ? Palette.greenTint : Palette.field, in: Capsule())
        }
        .buttonStyle(.plain)
    }
}

private struct ActionCard: View {
    let icon: Icon
    let title: String
    let sub: String
    var primary = false
    let action: () -> Void

    var body: some View {
        let shape = RoundedRectangle(cornerRadius: 18)
        Button(action: action) {
            VStack(alignment: .leading) {
                IconTile(icon, tint: primary ? .white : Palette.primary, fill: primary ? .white.opacity(0.2) : Palette.primaryTint)
                Spacer()
                Text(title).style(.bodyStrong.with(size: 16, weight: .bold, color: primary ? .white : Palette.text)).lineLimit(1)
                Text(sub).style(.caption.with(color: primary ? .white.opacity(0.85) : Palette.muted)).lineLimit(2)
            }
            .padding(14)
            .frame(maxWidth: .infinity, minHeight: 150, maxHeight: 150, alignment: .leading)
            .background {
                if primary {
                    shape.fill(LinearGradient(colors: [Palette.primary, Palette.primaryDark], startPoint: .top, endPoint: .bottom))
                } else {
                    shape.fill(Palette.surface).overlay(shape.strokeBorder(Palette.border))
                }
            }
            .contentShape(shape)
        }
        .buttonStyle(.plain)
    }
}
