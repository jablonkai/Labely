import LabelyKit
import SwiftUI

/// M7 · Sablonkönyvtár: starter templates and the user's library, by folder.
struct TemplatesScreen: View {
    private enum Filter: Hashable {
        case all, starters, mine
        case folder(String)
    }

    let state: AppState
    let onNewLabel: () -> Void
    @State private var query = ""
    @State private var filter = Filter.all

    var body: some View {
        let folders = Set(state.templates.filter { !$0.starter && !$0.folder.isEmpty }.map(\.folder)).sorted()
        let shown = state.templates.filter { t in
            let inFilter: Bool =
                switch filter {
                case .all: true
                case .starters: t.starter
                case .mine: !t.starter
                case .folder(let f): !t.starter && t.folder == f
                }
            return inFilter && (query.isEmpty || displayName(t).localizedCaseInsensitiveContains(query))
        }
        ScrollView {
            VStack(alignment: .leading, spacing: 16) {
                LargeTitle(title: L("tab_templates"), sub: L("templates_sub")) {
                    IconButton(.plus, tint: .white, fill: Palette.primary, size: 52, label: L("new_label"), action: onNewLabel)
                }
                SearchField(text: $query, placeholder: L("templates_search"))
                ScrollView(.horizontal, showsIndicators: false) {
                    HStack(spacing: 8) {
                        Chip(text: L("filter_all"), selected: filter == .all) { filter = .all }
                        Chip(text: L("filter_starters"), selected: filter == .starters) { filter = .starters }
                        Chip(text: L("filter_mine"), selected: filter == .mine) { filter = .mine }
                        ForEach(folders, id: \.self) { f in
                            Chip(text: f, selected: filter == .folder(f)) { filter = .folder(f) }
                        }
                    }
                }
                Text(L("templates_count", shown.count)).style(.bodyStrong.with(color: Palette.muted))
                TemplateGrid(templates: shown, state: state, deletable: true)
            }
            .padding(16)
        }
        .scrollDismissesKeyboard(.immediately)
    }
}

/// Two-column grid of template cards. A tap opens a copy; library templates can be deleted from
/// the context menu.
struct TemplateGrid: View {
    let templates: [Template]
    let state: AppState
    var deletable = false
    @State private var deleting: Template?

    var body: some View {
        LazyVGrid(columns: [GridItem(.flexible(), spacing: 12), GridItem(.flexible(), spacing: 12)], spacing: 12) {
            ForEach(templates) { t in
                Button { state.fromTemplate(t, name: displayName(t)) } label: {
                    Card {
                        VStack(alignment: .leading, spacing: 8) {
                            ThumbTile(image: state.thumbs[t.ref], widthMm: t.widthMm, heightMm: t.heightMm)
                                .aspectRatio(1.7, contentMode: .fit)
                            VStack(alignment: .leading) {
                                Text(displayName(t)).style(.bodyStrong.with(size: 16)).lineLimit(1)
                                Text(sizeText(t.widthMm, t.heightMm)).style(.caption)
                            }
                            .padding(.horizontal, 4)
                        }
                        .padding(10)
                    }
                }
                .buttonStyle(.plain)
                .contextMenu {
                    if deletable && !t.starter {
                        Button(role: .destructive) { deleting = t } label: { Text(L("delete")) }
                    }
                }
            }
        }
        .alert(
            L("template_delete_title"),
            isPresented: Binding(get: { deleting != nil }, set: { if !$0 { deleting = nil } }),
            presenting: deleting
        ) { t in
            Button(L("delete"), role: .destructive) { state.deleteTemplate(t) }
            Button(L("cancel"), role: .cancel) {}
        } message: { t in
            Text(L("template_delete_body", displayName(t)))
        }
    }
}

private let starterNames: [String: String] = [
    "starter:name-badge": "template_name_badge",
    "starter:price-tag": "template_price_tag",
    "starter:asset-tag": "template_asset_tag",
    "starter:address": "template_address",
    "starter:qr-link": "template_qr_link",
    "starter:storage-box": "template_storage_box",
    "starter:jar": "template_jar",
    "starter:cable-flag": "template_cable_flag",
    "starter:wifi": "template_wifi",
    "starter:shipping": "template_shipping",
]

/// Starter names are translated like in the studio; library templates keep their own name.
func displayName(_ t: Template) -> String {
    starterNames[t.ref].map(L) ?? t.name
}
