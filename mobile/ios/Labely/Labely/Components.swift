import LabelyKit
import SwiftUI

// The shared pieces of the mobile design, as `Components.kt` on Android.

/// White rounded card with a hairline border; a selected card has a primary border.
struct Card<Content: View>: View {
    var selected = false
    @ViewBuilder let content: Content

    var body: some View {
        let shape = RoundedRectangle(cornerRadius: 18)
        VStack(alignment: .leading, spacing: 0) { content }
            .background(Palette.surface, in: shape)
            .overlay(shape.strokeBorder(selected ? Palette.primary : Palette.border, lineWidth: selected ? 2 : 1))
            .clipShape(shape)
    }
}

/// Grouped list container.
struct ListGroup<Content: View>: View {
    @ViewBuilder let content: Content

    var body: some View {
        Card { content }.frame(maxWidth: .infinity)
    }
}

struct SectionHeader: View {
    let title: String
    var action: String?
    var onAction: () -> Void = {}

    var body: some View {
        HStack {
            Text(title).style(.title.with(size: 20))
            Spacer()
            if let action {
                Button(action: onAction) { Text(action).style(.body.with(weight: .semibold, color: Palette.primary)) }
                    .buttonStyle(.plain)
            }
        }
        .padding(.top, 8)
    }
}

struct LargeTitle<Trailing: View>: View {
    let title: String
    var sub: String?
    @ViewBuilder var trailing: Trailing

    var body: some View {
        HStack(alignment: .bottom) {
            VStack(alignment: .leading, spacing: 4) {
                Text(title).style(.largeTitle)
                if let sub { Text(sub).style(.caption) }
            }
            Spacer()
            trailing
        }
        .padding(.top, 8)
    }
}

extension LargeTitle where Trailing == EmptyView {
    init(title: String, sub: String? = nil) {
        self.init(title: title, sub: sub) { EmptyView() }
    }
}

struct SearchField: View {
    @Binding var text: String
    let placeholder: String

    var body: some View {
        HStack(spacing: 8) {
            LIcon(.search, Palette.muted, size: 18)
            TextField("", text: $text, prompt: Text(placeholder).foregroundStyle(Palette.muted))
                .style(.body)
                .autocorrectionDisabled()
                .submitLabel(.search)
            if !text.isEmpty {
                Button { text = "" } label: { LIcon(.close, Palette.muted, size: 16) }.buttonStyle(.plain)
            }
        }
        .padding(.horizontal, 12)
        .frame(height: 44)
        .background(Palette.field, in: RoundedRectangle(cornerRadius: 12))
    }
}

struct Chip: View {
    let text: String
    let selected: Bool
    let action: () -> Void

    var body: some View {
        Button(action: action) {
            Text(text)
                .style(.body.with(weight: selected ? .semibold : .medium, color: selected ? Palette.primaryDark : Palette.text))
                .padding(.horizontal, 14)
                .padding(.vertical, 8)
                .background(selected ? Palette.primaryTint : Palette.surface, in: Capsule())
                .overlay(Capsule().strokeBorder(selected ? Palette.primaryTint2 : Palette.border))
        }
        .buttonStyle(.plain)
    }
}

/// Round icon button; `tint` colours the glyph, `fill` the circle.
struct IconButton: View {
    let icon: Icon
    var tint: Color = Palette.text2
    var fill: Color = .clear
    var size: CGFloat = 40
    var enabled = true
    var label: String?
    let action: () -> Void

    init(
        _ icon: Icon, tint: Color = Palette.text2, fill: Color = .clear, size: CGFloat = 40,
        enabled: Bool = true, label: String? = nil, action: @escaping () -> Void
    ) {
        self.icon = icon
        self.tint = tint
        self.fill = fill
        self.size = size
        self.enabled = enabled
        self.label = label
        self.action = action
    }

    var body: some View {
        Button(action: action) {
            LIcon(icon, enabled ? tint : Palette.border, size: size * 0.55)
                .frame(width: size, height: size)
                .background(fill, in: Circle())
                .contentShape(Circle())
        }
        .buttonStyle(.plain)
        .disabled(!enabled)
        .accessibilityLabel(label ?? icon.rawValue)
    }
}

struct PrimaryButton: View {
    let text: String
    var icon: Icon?
    var enabled = true
    let action: () -> Void

    var body: some View {
        Button(action: action) {
            HStack(spacing: 8) {
                if let icon { LIcon(icon, .white, size: 20) }
                Text(text).style(.bodyStrong.with(size: 17, color: .white))
            }
            .frame(maxWidth: .infinity)
            .frame(height: 52)
            .background(enabled ? Palette.primary : Palette.primaryTint2, in: RoundedRectangle(cornerRadius: 14))
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .disabled(!enabled)
    }
}

struct SecondaryButton: View {
    let text: String
    var icon: Icon?
    var enabled = true
    var height: CGFloat = 44
    let action: () -> Void

    var body: some View {
        let shape = RoundedRectangle(cornerRadius: 12)
        Button(action: action) {
            HStack(spacing: 6) {
                if let icon { LIcon(icon, Palette.text, size: 18) }
                Text(text).style(.bodyStrong.with(color: enabled ? Palette.text : Palette.muted))
            }
            .frame(maxWidth: .infinity)
            .frame(height: height)
            .padding(.horizontal, 12)
            .background(Palette.surface, in: shape)
            .overlay(shape.strokeBorder(Palette.border))
            .contentShape(shape)
        }
        .buttonStyle(.plain)
        .disabled(!enabled)
    }
}

/// A row of a grouped list: leading content, title, optional subtitle, trailing content.
struct ListRow<Leading: View, Trailing: View>: View {
    let title: String
    var subtitle: String?
    var action: (() -> Void)?
    @ViewBuilder var leading: Leading
    @ViewBuilder var trailing: Trailing

    var body: some View {
        let row = HStack(spacing: 12) {
            leading
            VStack(alignment: .leading, spacing: 2) {
                Text(title).style(.bodyStrong.with(size: 16)).lineLimit(1)
                if let subtitle { Text(subtitle).style(.caption).lineLimit(1) }
            }
            .frame(maxWidth: .infinity, alignment: .leading)
            trailing
        }
        .padding(.horizontal, 16)
        .padding(.vertical, 14)
        .contentShape(Rectangle())
        if let action {
            Button(action: action) { row }.buttonStyle(.plain)
        } else {
            row
        }
    }
}

extension ListRow where Leading == EmptyView, Trailing == EmptyView {
    init(title: String, subtitle: String? = nil, action: (() -> Void)? = nil) {
        self.init(title: title, subtitle: subtitle, action: action, leading: { EmptyView() }, trailing: { EmptyView() })
    }
}

extension ListRow where Leading == EmptyView {
    init(title: String, subtitle: String? = nil, action: (() -> Void)? = nil, @ViewBuilder trailing: () -> Trailing) {
        self.init(title: title, subtitle: subtitle, action: action, leading: { EmptyView() }, trailing: trailing)
    }
}

struct RowDivider: View {
    var body: some View {
        Rectangle().fill(Palette.borderSoft).frame(height: 1)
    }
}

/// Icon in a tinted rounded square, as in list rows and cards.
struct IconTile: View {
    let icon: Icon
    var tint: Color = Palette.primary
    var fill: Color = Palette.primaryTint
    var size: CGFloat = 40

    init(_ icon: Icon, tint: Color = Palette.primary, fill: Color = Palette.primaryTint, size: CGFloat = 40) {
        self.icon = icon
        self.tint = tint
        self.fill = fill
        self.size = size
    }

    var body: some View {
        LIcon(icon, tint, size: size * 0.5)
            .frame(width: size, height: size)
            .background(fill, in: RoundedRectangle(cornerRadius: 10))
    }
}

/// − value + stepper.
struct CountStepper: View {
    let value: String
    let onMinus: () -> Void
    let onPlus: () -> Void

    var body: some View {
        HStack(spacing: 0) {
            IconButton(.minus, size: 36, label: "−", action: onMinus)
            Text(value).style(.bodyStrong.with(size: 17)).lineLimit(1).frame(minWidth: 36).padding(.horizontal, 4)
            IconButton(.plus, size: 36, label: "+", action: onPlus)
        }
        .padding(4)
        .background(Palette.bg, in: RoundedRectangle(cornerRadius: 12))
    }
}

/// Bordered numeric field with a leading label and a unit suffix. The value is committed when
/// the field loses focus (the keyboard's Done, another field, or a sheet opening); with `live`,
/// on every valid change, for local state that a button reads right away.
struct NumberField: View {
    let label: String
    let value: Double
    var suffix = "mm"
    var live = false
    let onCommit: (Double) -> Void

    @State private var text = ""
    @FocusState private var focused: Bool

    var body: some View {
        let shape = RoundedRectangle(cornerRadius: 12)
        HStack(spacing: 8) {
            Text(label).style(.caption).lineLimit(1).fixedSize()
            TextField("", text: $text)
                .style(.body)
                .keyboardType(.decimalPad)
                .focused($focused)
                .onSubmit(commit)
            if !suffix.isEmpty { Text(suffix).style(.caption) }
        }
        .padding(.horizontal, 12)
        .frame(height: 44)
        .background(Palette.surface, in: shape)
        .overlay(shape.strokeBorder(focused ? Palette.primary : Palette.border))
        .onAppear { text = mm(value) }
        .onChange(of: value) { if !focused { text = mm(value) } }
        .onChange(of: focused) { if !focused { commit() } }
        .onChange(of: text) {
            if live, let v = parse(), v > 0, v != value { onCommit(v) }
        }
    }

    private func parse() -> Double? {
        Double(text.replacingOccurrences(of: ",", with: "."))
    }

    private func commit() {
        guard let v = parse() else {
            text = mm(value)
            return
        }
        if v != value { onCommit(v) }
    }
}

/// A label raster on a white card, sized to its aspect ratio. The canvas draws the raster
/// unfiltered, so the printer's dots stay sharp.
struct LabelImage: View {
    let image: CGImage?
    let widthMm: Double
    let heightMm: Double
    var sharp = false

    var body: some View {
        let ratio = min(max(widthMm / max(heightMm, 0.1), 0.2), 5)
        ZStack {
            Color.white
            if let image {
                Image(decorative: image, scale: 1)
                    .resizable()
                    .interpolation(sharp ? .none : .medium)
                    .aspectRatio(contentMode: .fit)
            }
        }
        .aspectRatio(ratio, contentMode: .fit)
        .clipShape(RoundedRectangle(cornerRadius: 8))
        .shadow(color: .black.opacity(0.08), radius: 2, y: 1)
    }
}

/// Grey tile holding a label thumbnail, as in the template cards.
struct ThumbTile: View {
    let image: CGImage?
    let widthMm: Double
    let heightMm: Double

    var body: some View {
        LabelImage(image: image, widthMm: widthMm, heightMm: heightMm)
            .padding(12)
            .frame(maxWidth: .infinity, maxHeight: .infinity)
            .background(Palette.canvas, in: RoundedRectangle(cornerRadius: 12))
    }
}

/// Header of a sheet: title and a round close button.
struct SheetHeader: View {
    let title: String
    let onClose: () -> Void

    var body: some View {
        HStack {
            Text(title).style(.title)
            Spacer()
            IconButton(.close, fill: Palette.bg, size: 36, label: L("cancel"), action: onClose)
        }
        .padding(.bottom, 8)
    }
}

/// A sheet in the design's style: header, then scrolling content.
struct SheetBody<Content: View>: View {
    let title: String
    let onClose: () -> Void
    @ViewBuilder let content: Content

    var body: some View {
        VStack(spacing: 0) {
            SheetHeader(title: title, onClose: onClose).padding(.horizontal, 20).padding(.top, 20)
            ScrollView {
                VStack(alignment: .leading, spacing: 16) { content }
                    .padding(.horizontal, 20)
                    .padding(.bottom, 16)
            }
            .scrollDismissesKeyboard(.interactively)
        }
        .background(Palette.surface)
        .presentationDragIndicator(.visible)
    }
}

/// Green or amber status line with an icon, e.g. the check result.
struct Banner: View {
    let text: String
    let ok: Bool
    var trailing: String?

    var body: some View {
        let fg = ok ? Palette.green : Palette.amber
        HStack(spacing: 10) {
            LIcon(ok ? .check : .warn, fg, size: 18)
            Text(text).style(.bodyStrong.with(color: fg)).frame(maxWidth: .infinity, alignment: .leading)
            if let trailing { Text(trailing).style(.bodyStrong) }
        }
        .padding(14)
        .background(ok ? Palette.greenTint : Palette.amberTint, in: RoundedRectangle(cornerRadius: 12))
    }
}
