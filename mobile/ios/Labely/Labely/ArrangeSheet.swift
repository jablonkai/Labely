import LabelyKit
import SwiftUI

/// Arrange the selection: align and distribute, order, rotation and flipping, nudging, grouping,
/// lock and hide, mirror copies, the clipboard and the element's name. The canvas above stays
/// visible at the medium height, so every change shows at once.
struct ArrangeSheet: View {
    let ed: EditorState
    let onDismiss: () -> Void
    @State private var nudge = EditorState.nudgeSmall

    var body: some View {
        let els = ed.selectedElements
        let one = ed.selected
        SheetBody(title: els.count > 1 ? L("selected_count", els.count) : L("arrange"), onClose: close) {
            if els.isEmpty {
                Text(L("inspector_hint")).style(.caption).frame(maxWidth: .infinity).padding(.vertical, 24)
            } else {
                if let one {
                    GroupTitle(L("element_name"))
                    CommitField(value: one.name ?? "", placeholder: kindName(one.type)) { ed.rename(one, to: $0) }
                }

                GroupTitle(L(els.count > 1 ? "align" : "align_to_label"))
                AlignBar(ed: ed)

                GroupTitle(L("order"))
                HStack(spacing: 8) {
                    IconToggle(icon: .toFront, label: L("to_front")) { ed.reorder("front") }
                    IconToggle(icon: .up, label: L("forward")) { ed.reorder("forward") }
                    IconToggle(icon: .down, label: L("backward")) { ed.reorder("backward") }
                    IconToggle(icon: .toBack, label: L("to_back")) { ed.reorder("back") }
                    Spacer()
                }

                GroupTitle(L("rotation"))
                HStack(spacing: 8) {
                    if let one {
                        NumberField(label: "", value: one.rotationDeg, suffix: "°", length: false) { ed.rotate(one, to: $0) }
                            .frame(maxWidth: 110)
                    }
                    IconToggle(icon: .rotate, label: L("turn_90")) { ed.turn90() }
                    IconToggle(icon: .flipH, on: one?.flipH ?? false, label: L("flip_h")) { ed.flip(horizontal: true) }
                    IconToggle(icon: .flipV, on: one?.flipV ?? false, label: L("flip_v")) { ed.flip(horizontal: false) }
                    Spacer()
                }

                GroupTitle(L("nudge"))
                HStack(spacing: 8) {
                    IconToggle(icon: .chevLeft, label: L("nudge_left")) { ed.nudge(dx: -nudge, dy: 0) }
                    IconToggle(icon: .up, label: L("nudge_up")) { ed.nudge(dx: 0, dy: -nudge) }
                    IconToggle(icon: .down, label: L("nudge_down")) { ed.nudge(dx: 0, dy: nudge) }
                    IconToggle(icon: .chevRight, label: L("nudge_right")) { ed.nudge(dx: nudge, dy: 0) }
                    Spacer()
                    Picker(L("nudge"), selection: $nudge) {
                        Text(mmText(EditorState.nudgeSmall)).tag(EditorState.nudgeSmall)
                        Text(mmText(EditorState.nudgeLarge)).tag(EditorState.nudgeLarge)
                    }
                    .pickerStyle(.segmented)
                    .fixedSize()
                }

                let locked = els.allSatisfy(\.locked)
                ListGroup {
                    if els.count > 1 || ed.canUngroup {
                        HStack(spacing: 12) {
                            SecondaryButton(text: L("group"), icon: .group, enabled: ed.canGroupMore) { ed.group() }
                            SecondaryButton(text: L("ungroup"), icon: .ungroup, enabled: ed.canUngroup) { ed.ungroup() }
                        }
                        .padding(12)
                        RowDivider()
                    }
                    SwitchRow(title: L("lock"), subtitle: L("lock_sub"), icon: locked ? .lock : .unlock, isOn: locked) {
                        ed.setLocked(els.map(\.id), $0)
                    }
                    RowDivider()
                    ListRow(title: L("hide"), subtitle: L("hide_sub"), action: {
                        ed.setHidden(els.map(\.id), true)
                        onDismiss()
                    }) {
                        IconTile(.eyeOff)
                    } trailing: {
                        LIcon(.chevRight, Palette.muted, size: 18)
                    }
                }

                GroupTitle(L("mirror_copy"))
                Text(L("mirror_copy_sub")).style(.caption)
                HStack(spacing: 12) {
                    SecondaryButton(text: L("mirror_h"), icon: .flipH) { ed.mirror("horizontal") }
                    SecondaryButton(text: L("mirror_v"), icon: .flipV) { ed.mirror("vertical") }
                }
                if let one, one.mirrorOf != nil {
                    SecondaryButton(text: L("mirror_unlink"), icon: .link) { ed.unlinkMirror(one) }
                }

                GroupTitle(L("clipboard"))
                HStack(spacing: 8) {
                    SecondaryButton(text: L("copy"), icon: .copy) { copySelection(ed) }
                    SecondaryButton(text: L("cut"), icon: .cut) { copySelection(ed, cut: true) }
                    SecondaryButton(text: L("paste"), icon: .paste) { pasteInto(ed) }
                }
            }
            CanvasOptions(state: ed.app)
        }
        .presentationDetents([.medium, .large])
        .presentationBackgroundInteraction(.enabled(upThrough: .medium))
    }

    private func close() {
        endEditing()
        onDismiss()
    }
}

/// Grid, snapping, rulers and the rotation step: the same settings as in Settings, where they
/// are the defaults of every label.
struct CanvasOptions: View {
    @Bindable var state: AppState

    var body: some View {
        GroupTitle(L("canvas_helpers"))
        ListGroup {
            SwitchRow(title: L("snap"), subtitle: L("snap_sub"), icon: .magnet, isOn: state.prefs.snap) { state.prefs.snap = $0 }
            RowDivider()
            SwitchRow(title: L("grid"), subtitle: L("grid_sub"), icon: .grid, isOn: state.prefs.showGrid) { state.prefs.showGrid = $0 }
            RowDivider()
            SegmentRow(
                title: L("grid_step"), value: state.prefs.gridMm,
                options: Prefs.gridSteps.map { ($0, mmText($0)) }
            ) { state.prefs.gridMm = $0 }
            RowDivider()
            SwitchRow(title: L("rulers"), icon: .ruler, isOn: state.prefs.rulers) { state.prefs.rulers = $0 }
            RowDivider()
            SwitchRow(title: L("rotate_snap"), subtitle: L("rotate_snap_sub"), icon: .rotate, isOn: state.prefs.rotateSnap) {
                state.prefs.rotateSnap = $0
            }
        }
    }
}

/// M5 · Rétegek: elements top-first with their lock and visibility; a tap selects, several can
/// be picked, and the bar below reorders, groups, duplicates and deletes the selection.
struct LayersSheet: View {
    let ed: EditorState
    let onDismiss: () -> Void
    @State private var query = ""

    var body: some View {
        let all = Array((ed.doc?.elements ?? []).reversed())
        let elements = all.filter { e in
            query.isEmpty || elementTitle(e).localizedCaseInsensitiveContains(query) || kindName(e.type).localizedCaseInsensitiveContains(query)
        }
        VStack(spacing: 0) {
            SheetBody(title: L("tool_layers"), onClose: onDismiss) {
                if all.count > 5 || !query.isEmpty { SearchField(text: $query, placeholder: L("layers_search")) }
                HStack {
                    Chip(text: L("select_several"), selected: ed.multiSelect) {
                        ed.multiSelect.toggle()
                        if !ed.multiSelect, ed.selection.count > 1 { ed.selection = Array(ed.selection.prefix(1)) }
                    }
                    Chip(text: L("select_all"), selected: false) { ed.selectAll() }
                    Spacer()
                    if ed.selection.count > 1 { Text(L("selected_count", ed.selection.count)).style(.caption) }
                }
                VStack(spacing: 2) {
                    if elements.isEmpty { ListRow(title: L(all.isEmpty ? "layers_empty" : "layers_no_match")) }
                    ForEach(elements) { e in LayerRow(ed: ed, e: e) }
                }
            }
            Divider().overlay(Palette.border)
            actions
        }
        .background(Palette.surface)
    }

    private var actions: some View {
        let any = !ed.selection.isEmpty
        return HStack(spacing: 4) {
            IconButton(.toFront, enabled: any, label: L("to_front")) { ed.reorder("front") }
            IconButton(.up, enabled: any, label: L("forward")) { ed.reorder("forward") }
            IconButton(.down, enabled: any, label: L("backward")) { ed.reorder("backward") }
            IconButton(.toBack, enabled: any, label: L("to_back")) { ed.reorder("back") }
            IconButton(ed.canGroupMore || !ed.canUngroup ? .group : .ungroup, enabled: ed.canGroupMore || ed.canUngroup, label: L(ed.canGroupMore || !ed.canUngroup ? "group" : "ungroup")) {
                if ed.canGroupMore { ed.group() } else { ed.ungroup() }
            }
            IconButton(.copy, enabled: any, label: L("duplicate")) { ed.duplicateSelection() }
            Spacer()
            IconButton(.trash, tint: Palette.red, enabled: any, label: L("delete")) { ed.deleteSelection() }
        }
        .padding(.horizontal, 16)
        .frame(height: 56)
    }
}

private struct LayerRow: View {
    let ed: EditorState
    let e: Element

    var body: some View {
        let on = ed.selection.contains(e.id)
        HStack(spacing: 10) {
            Button {
                ed.tap(e.id)
            } label: {
                HStack(spacing: 10) {
                    LIcon(kindIcon(e.type), on ? Palette.primary : Palette.text2, size: 20)
                    Text(elementTitle(e))
                        .style(.bodyStrong.with(size: 16, color: e.hidden ? Palette.muted : on ? Palette.primary : Palette.text))
                        .lineLimit(1)
                    if e.group != nil { LIcon(.group, Palette.muted, size: 14) }
                    Spacer(minLength: 0)
                    if let column = ed.boundColumn(e) {
                        HStack(spacing: 4) {
                            LIcon(.link, Palette.primary, size: 13)
                            Text(column).style(.caption.with(weight: .semibold, color: Palette.primary)).lineLimit(1)
                        }
                        .padding(.horizontal, 8)
                        .padding(.vertical, 4)
                        .background(Palette.primaryTint, in: Capsule())
                    }
                }
                .contentShape(Rectangle())
            }
            .buttonStyle(.plain)
            IconButton(e.hidden ? .eyeOff : .eye, tint: e.hidden ? Palette.muted : Palette.text2, size: 34, label: L(e.hidden ? "show" : "hide")) {
                ed.setHidden([e.id], !e.hidden)
            }
            IconButton(e.locked ? .lock : .unlock, tint: e.locked ? Palette.text : Palette.muted, size: 34, label: L(e.locked ? "unlock" : "lock")) {
                ed.setLocked([e.id], !e.locked)
            }
        }
        .padding(.leading, 12)
        .padding(.trailing, 4)
        .frame(height: 46)
        .background(on ? Palette.primaryTint : .clear, in: RoundedRectangle(cornerRadius: 12))
        .accessibilityAddTraits(on ? .isSelected : [])
    }
}

/// "Check label": what `doc.check` finds, worst first. Tapping a problem selects its element.
struct CheckSheet: View {
    let ed: EditorState
    let onDismiss: () -> Void

    var body: some View {
        SheetBody(title: L("check_label"), onClose: onDismiss) {
            if let issues = ed.issues {
                let sorted = issues.sorted { rank($0.severity) < rank($1.severity) }
                if sorted.isEmpty {
                    Banner(text: L("check_ok"), ok: true)
                } else {
                    Text(L("check_count", sorted.count)).style(.bodyStrong.with(color: Palette.muted))
                    ListGroup {
                        ForEach(Array(sorted.enumerated()), id: \.offset) { i, issue in
                            if i > 0 { RowDivider() }
                            row(issue)
                        }
                    }
                }
            } else {
                ProgressView().frame(maxWidth: .infinity).padding(.vertical, 24)
            }
            SecondaryButton(text: L("check_again"), icon: .refresh) { ed.check() }
        }
        .presentationDetents([.medium, .large])
        .task { await ed.check().value }
    }

    private func rank(_ severity: String) -> Int {
        switch severity {
        case "error": 0
        case "warning": 1
        default: 2
        }
    }

    private func row(_ issue: CheckIssue) -> some View {
        let (icon, tint, fill): (Icon, Color, Color) =
            switch issue.severity {
            case "error": (.warn, Palette.red, Palette.red.opacity(0.12))
            case "warning": (.warn, Palette.amber, Palette.amberTint)
            default: (.info, Palette.primary, Palette.primaryTint)
            }
        let target = issue.elementId.flatMap { id in ed.doc?.elements.first { $0.id == id } }
        return Button {
            if let target {
                ed.multiSelect = false
                ed.selection = [target.id]
                onDismiss()
            }
        } label: {
            HStack(alignment: .top, spacing: 12) {
                IconTile(icon, tint: tint, fill: fill)
                VStack(alignment: .leading, spacing: 2) {
                    Text(issue.message).style(.bodyStrong).fixedSize(horizontal: false, vertical: true)
                    if !issue.hint.isEmpty { Text(issue.hint).style(.caption).fixedSize(horizontal: false, vertical: true) }
                    if let target { Text(elementTitle(target)).style(.caption.with(weight: .semibold, color: Palette.primary)) }
                }
                .frame(maxWidth: .infinity, alignment: .leading)
                if target != nil { LIcon(.chevRight, Palette.muted, size: 18) }
            }
            .padding(.horizontal, 16)
            .padding(.vertical, 12)
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .disabled(target == nil)
    }
}

/// Save the label as a template: its name and the library folder. A template of the same name
/// in that folder is replaced only after asking.
struct SaveTemplateSheet: View {
    let ed: EditorState
    let onDismiss: () -> Void
    @State private var name = ""
    @State private var folder = ""
    @State private var newFolder = ""
    @State private var addingFolder = false
    @State private var replacing = false

    var body: some View {
        let app = ed.app
        let trimmed = name.trimmingCharacters(in: .whitespaces)
        SheetBody(title: L("save_template"), onClose: onDismiss) {
            GroupTitle(L("template_name"))
            LabeledInput(text: $name, placeholder: L("template_name"))
            GroupTitle(L("template_folder"))
            FlowChips {
                Chip(text: L("folder_default"), selected: folder.isEmpty) { folder = "" }
                ForEach(folders(app), id: \.self) { f in
                    Chip(text: f, selected: folder == f) { folder = f }
                }
                Chip(text: "+ " + L("folder_new"), selected: false) {
                    newFolder = ""
                    addingFolder = true
                }
            }
            HStack(spacing: 12) {
                SecondaryButton(text: L("cancel"), height: 52) { onDismiss() }
                PrimaryButton(text: L("save"), enabled: !trimmed.isEmpty) {
                    endEditing()
                    if app.templateExists(name: trimmed, folder: folder) { replacing = true } else { save(trimmed) }
                }
            }
        }
        .presentationDetents([.medium, .large])
        .onAppear { name = ed.doc?.name ?? "" }
        .alert(L("folder_new"), isPresented: $addingFolder) {
            TextField(L("folder_name"), text: $newFolder)
            Button(L("create")) {
                let f = newFolder.trimmingCharacters(in: .whitespaces)
                if !f.isEmpty { folder = f }
            }
            Button(L("cancel"), role: .cancel) {}
        }
        .alert(L("template_replace_title"), isPresented: $replacing) {
            Button(L("replace"), role: .destructive) { save(trimmed) }
            Button(L("cancel"), role: .cancel) {}
        } message: {
            Text(L("template_replace_body", trimmed))
        }
    }

    /// The library's folders, and the one just named (it is created on saving).
    private func folders(_ app: AppState) -> [String] {
        folder.isEmpty || app.templateFolders.contains(folder) ? app.templateFolders : app.templateFolders + [folder]
    }

    private func save(_ name: String) {
        ed.saveTemplate(name, folder: folder)
        onDismiss()
    }
}

/// A bordered single-line text input bound to local state.
struct LabeledInput: View {
    @Binding var text: String
    let placeholder: String
    var plain = false
    @FocusState private var focused: Bool

    var body: some View {
        let shape = RoundedRectangle(cornerRadius: 12)
        TextField("", text: $text, prompt: Text(placeholder).foregroundStyle(Palette.muted))
            .style(.body)
            .focused($focused)
            .autocorrectionDisabled(plain)
            .textInputAutocapitalization(plain ? .never : .sentences)
            .submitLabel(.done)
            .padding(.horizontal, 12)
            .frame(height: 48)
            .background(Palette.surface, in: shape)
            .overlay(shape.strokeBorder(focused ? Palette.primary : Palette.border))
    }
}

/// Chips that wrap onto further lines.
struct FlowChips<Content: View>: View {
    @ViewBuilder let content: Content

    var body: some View {
        FlowLayout(spacing: 8) { content }
    }
}

struct FlowLayout: Layout {
    var spacing: CGFloat = 8

    func sizeThatFits(proposal: ProposedViewSize, subviews: Subviews, cache: inout ()) -> CGSize {
        place(subviews, width: proposal.width ?? .infinity).size
    }

    func placeSubviews(in bounds: CGRect, proposal: ProposedViewSize, subviews: Subviews, cache: inout ()) {
        for (i, p) in place(subviews, width: bounds.width).points.enumerated() {
            subviews[i].place(at: CGPoint(x: bounds.minX + p.x, y: bounds.minY + p.y), proposal: .unspecified)
        }
    }

    private func place(_ subviews: Subviews, width: CGFloat) -> (size: CGSize, points: [CGPoint]) {
        var points: [CGPoint] = []
        var x: CGFloat = 0
        var y: CGFloat = 0
        var row: CGFloat = 0
        var widest: CGFloat = 0
        for v in subviews {
            let s = v.sizeThatFits(.unspecified)
            if x > 0, x + s.width > width {
                x = 0
                y += row + spacing
                row = 0
            }
            points.append(CGPoint(x: x, y: y))
            x += s.width + spacing
            row = max(row, s.height)
            widest = max(widest, x - spacing)
        }
        return (CGSize(width: widest, height: y + row), points)
    }
}
