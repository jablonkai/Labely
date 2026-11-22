import LabelyKit
import SwiftUI

/// M7 · Sablonkönyvtár: starter templates and the user's library, by folder. Folders are made,
/// renamed and deleted from the chips; templates are duplicated, renamed, moved and deleted
/// from their menu.
struct TemplatesScreen: View {
    private enum Filter: Hashable {
        case all, starters, mine
        case folder(String)
    }

    let state: AppState
    let onNewLabel: () -> Void
    @State private var query = ""
    @State private var filter = Filter.all
    @State private var category: String?
    @State private var naming = false
    @State private var folderName = ""
    /// The folder being renamed; none while a new one is named.
    @State private var renaming: String?
    @State private var deleting: String?

    var body: some View {
        let folders = state.templateFolders
        let shown = state.templates.filter { t in
            let inFilter: Bool =
                switch filter {
                case .all: true
                case .starters: t.starter && (category == nil || t.category == category)
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
                                .contextMenu {
                                    Button {
                                        renaming = f
                                        folderName = f
                                        naming = true
                                    } label: { Label(L("rename"), systemImage: "pencil") }
                                    Button(role: .destructive) { deleting = f } label: { Label(L("delete"), systemImage: "trash") }
                                }
                        }
                        Chip(text: "+ " + L("folder_new"), selected: false) {
                            renaming = nil
                            folderName = ""
                            naming = true
                        }
                    }
                }
                if filter == .starters {
                    ScrollView(.horizontal, showsIndicators: false) {
                        HStack(spacing: 8) {
                            Chip(text: L("filter_all"), selected: category == nil) { category = nil }
                            ForEach(starterCategories, id: \.id) { c in
                                Chip(text: c.name, selected: category == c.id) { category = c.id }
                            }
                        }
                    }
                }
                HStack {
                    Text(L("templates_count", shown.count)).style(.bodyStrong.with(color: Palette.muted))
                    Spacer()
                    if case .folder(let f) = filter {
                        Menu {
                            Button {
                                renaming = f
                                folderName = f
                                naming = true
                            } label: { Label(L("folder_rename"), systemImage: "pencil") }
                            Button(role: .destructive) { deleting = f } label: { Label(L("folder_delete"), systemImage: "trash") }
                        } label: {
                            HStack(spacing: 4) {
                                LIcon(.folder, Palette.primary, size: 16)
                                Text(f).style(.bodyStrong.with(color: Palette.primary)).lineLimit(1)
                                LIcon(.chevDown, Palette.primary, size: 14)
                            }
                        }
                    }
                }
                TemplateGrid(templates: shown, state: state, editable: true)
            }
            .padding(16)
        }
        .scrollDismissesKeyboard(.immediately)
        .alert(L(renaming == nil ? "folder_new" : "folder_rename"), isPresented: $naming) {
            TextField(L("folder_name"), text: $folderName)
            Button(L("save")) {
                let name = folderName.trimmingCharacters(in: .whitespaces)
                guard !name.isEmpty else { return }
                if let old = renaming {
                    if name != old {
                        state.renameFolder(old, to: name)
                        if filter == .folder(old) { filter = .folder(name) }
                    }
                } else {
                    state.createFolder(name)
                    filter = .folder(name)
                }
            }
            Button(L("cancel"), role: .cancel) {}
        }
        .alert(
            L("folder_delete"),
            isPresented: Binding(get: { deleting != nil }, set: { if !$0 { deleting = nil } }),
            presenting: deleting
        ) { f in
            Button(L("delete"), role: .destructive) {
                state.deleteFolder(f)
                if filter == .folder(f) { filter = .mine }
            }
            Button(L("cancel"), role: .cancel) {}
        } message: { f in
            Text(L("folder_delete_body", f, state.templates.filter { !$0.starter && $0.folder == f }.count))
        }
    }
}

/// Two-column grid of template cards. A tap opens a copy. With `editable`, a template's menu
/// duplicates it (a starter too, into the library), and renames, moves or deletes a library
/// one.
struct TemplateGrid: View {
    let templates: [Template]
    let state: AppState
    var editable = false
    @State private var deleting: Template?
    @State private var renaming: Template?
    @State private var name = ""

    var body: some View {
        LazyVGrid(columns: [GridItem(.flexible(), spacing: 12), GridItem(.flexible(), spacing: 12)], spacing: 12) {
            ForEach(templates) { t in
                Button { state.fromTemplate(t, name: displayName(t)) } label: {
                    Card {
                        VStack(alignment: .leading, spacing: 8) {
                            ThumbTile(image: state.thumbs[t.ref], widthMm: t.widthMm, heightMm: t.heightMm)
                                .aspectRatio(1.7, contentMode: .fit)
                            HStack(spacing: 4) {
                                VStack(alignment: .leading) {
                                    Text(displayName(t)).style(.bodyStrong.with(size: 16)).lineLimit(1)
                                    HStack(spacing: 6) {
                                        Text(([sizeText(t.widthMm, t.heightMm)] + (t.folder.isEmpty ? [] : [t.folder])).joined(separator: " · "))
                                            .style(.caption).lineLimit(1)
                                        if t.fits == true {
                                            Text(L("template_fits")).style(.caption.with(color: Palette.primary)).lineLimit(1)
                                        }
                                    }
                                }
                                .frame(maxWidth: .infinity, alignment: .leading)
                                if editable {
                                    Menu { menu(t) } label: {
                                        LIcon(.more, Palette.muted, size: 20).frame(width: 30, height: 36).contentShape(Rectangle())
                                    }
                                    .accessibilityLabel(L("more"))
                                }
                            }
                            .padding(.leading, 4)
                        }
                        .padding(10)
                    }
                }
                .buttonStyle(.plain)
                .contextMenu { if editable { menu(t) } }
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
        .alert(
            L("rename"),
            isPresented: Binding(get: { renaming != nil }, set: { if !$0 { renaming = nil } }),
            presenting: renaming
        ) { t in
            TextField(L("template_name"), text: $name)
            Button(L("save")) {
                let new = name.trimmingCharacters(in: .whitespaces)
                if !new.isEmpty, new != t.name { state.moveTemplate(t, name: new) }
            }
            Button(L("cancel"), role: .cancel) {}
        }
    }

    @ViewBuilder private func menu(_ t: Template) -> some View {
        Button { state.duplicateTemplate(t, name: t.starter ? displayName(t) : nil) } label: {
            Label(L(t.starter ? "template_copy_to_library" : "duplicate"), systemImage: "doc.on.doc")
        }
        if !t.starter {
            Button {
                name = t.name
                renaming = t
            } label: { Label(L("rename"), systemImage: "pencil") }
            Menu {
                if !t.folder.isEmpty {
                    Button(L("folder_default")) { state.moveTemplate(t, folder: "") }
                }
                ForEach(state.templateFolders.filter { $0 != t.folder }, id: \.self) { f in
                    Button(f) { state.moveTemplate(t, folder: f) }
                }
            } label: {
                Label(L("template_move"), systemImage: "folder")
            }
            Button(role: .destructive) { deleting = t } label: { Label(L("delete"), systemImage: "trash") }
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
    "starter:spice": "template_spice",
    "starter:freezer-bag": "template_freezer_bag",
    "starter:leftovers": "template_leftovers",
    "starter:meal-prep": "template_meal_prep",
    "starter:pantry-bin": "template_pantry_bin",
    "starter:bake-sale": "template_bake_sale",
    "starter:allergens": "template_allergens",
    "starter:opened-on": "template_opened_on",
    "starter:recipe-qr": "template_recipe_qr",
    "starter:file-folder": "template_file_folder",
    "starter:binder-spine": "template_binder_spine",
    "starter:shelf-label": "template_shelf_label",
    "starter:desk-name": "template_desk_name",
    "starter:inventory-bin": "template_inventory_bin",
    "starter:key-tag": "template_key_tag",
    "starter:return-to-owner": "template_return_to_owner",
    "starter:cable-wrap": "template_cable_wrap",
    "starter:patch-panel": "template_patch_panel",
    "starter:power-adapter": "template_power_adapter",
    "starter:network-port": "template_network_port",
    "starter:breaker-panel": "template_breaker_panel",
    "starter:server-rack": "template_server_rack",
    "starter:charger": "template_charger",
    "starter:outlet": "template_outlet",
    "starter:return-address": "template_return_address",
    "starter:fragile": "template_fragile",
    "starter:this-way-up": "template_this_way_up",
    "starter:keep-cold": "template_keep_cold",
    "starter:parcel-contents": "template_parcel_contents",
    "starter:box-of": "template_box_of",
    "starter:moving-box": "template_moving_box",
    "starter:order-tracking": "template_order_tracking",
    "starter:conference-badge": "template_conference_badge",
    "starter:table-number": "template_table_number",
    "starter:place-card": "template_place_card",
    "starter:gift-tag": "template_gift_tag",
    "starter:party-favor": "template_party_favor",
    "starter:raffle-ticket": "template_raffle_ticket",
    "starter:wedding-favor": "template_wedding_favor",
    "starter:coat-check": "template_coat_check",
]

/// Starter categories, in the order of `template.list`.
private var starterCategories: [(id: String, name: String)] {
    [
        ("kitchen", L("template_category_kitchen")),
        ("office", L("template_category_office")),
        ("cables", L("template_category_cables")),
        ("shipping", L("template_category_shipping")),
        ("events", L("template_category_events")),
    ]
}

/// Starter names are translated like in the studio; library templates keep their own name.
func displayName(_ t: Template) -> String {
    starterNames[t.ref].map(L) ?? t.name
}
