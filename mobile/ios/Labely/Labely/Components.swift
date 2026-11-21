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
/// on every valid change, for local state that a button reads right away. A length (the
/// default) is in millimetres and shown in the unit chosen in Settings; with `length: false`
/// the number is shown as it is (points, degrees).
struct NumberField: View {
    let label: String
    let value: Double
    var suffix = "mm"
    var live = false
    var length = true
    /// Whether zero and negative numbers may be typed.
    var signed = true
    let onCommit: (Double) -> Void

    @State private var text = ""
    @FocusState private var focused: Bool

    var body: some View {
        let shape = RoundedRectangle(cornerRadius: 12)
        HStack(spacing: 8) {
            if !label.isEmpty { Text(label).style(.caption).lineLimit(1).fixedSize() }
            TextField("", text: $text)
                .style(.body)
                .keyboardType(signed ? .numbersAndPunctuation : .decimalPad)
                .autocorrectionDisabled()
                .focused($focused)
                .onSubmit(commit)
            if !suffix.isEmpty { Text(length && suffix == "mm" ? CurrentUnit.value.suffix : suffix).style(.caption) }
        }
        .padding(.horizontal, 12)
        .frame(height: 44)
        .background(Palette.surface, in: shape)
        .overlay(shape.strokeBorder(focused ? Palette.primary : Palette.border))
        .onAppear { text = shown }
        .onChange(of: value) { if !focused { text = shown } }
        .onChange(of: focused) { if !focused { commit() } }
        .onChange(of: text) {
            if live, text != shown, let v = parse(), v > 0, v != value { onCommit(v) }
        }
    }

    private var shown: String { length ? mm(value) : num(value, decimals: 2) }

    private func parse() -> Double? {
        length ? CurrentUnit.value.parse(text) : Double(text.replacingOccurrences(of: ",", with: "."))
    }

    private func commit() {
        // Text the user did not change is not a new value (a rounded inch is not exact).
        guard text != shown else { return }
        guard let v = parse(), signed || v > 0 else {
            text = shown
            return
        }
        if v != value { onCommit(v) } else { text = shown }
    }
}

/// Square icon button that shows a state: the bold or lock toggles of the inspector.
struct IconToggle: View {
    let icon: Icon
    var on = false
    var enabled = true
    var label: String?
    let action: () -> Void

    var body: some View {
        Button(action: action) {
            LIcon(icon, !enabled ? Palette.border : on ? Palette.primary : Palette.text2, size: 22)
                .frame(width: 44, height: 44)
                .background(on ? Palette.primaryTint : Palette.bg, in: RoundedRectangle(cornerRadius: 12))
        }
        .buttonStyle(.plain)
        .disabled(!enabled)
        .accessibilityLabel(label ?? icon.rawValue)
        .accessibilityAddTraits(on ? .isSelected : [])
    }
}

/// A list row with a switch.
struct SwitchRow: View {
    let title: String
    var subtitle: String?
    var icon: Icon?
    let isOn: Bool
    let onChange: (Bool) -> Void

    var body: some View {
        ListRow(title: title, subtitle: subtitle) {
            if let icon { IconTile(icon) }
        } trailing: {
            SwiftUI.Toggle("", isOn: Binding(get: { isOn }, set: onChange)).labelsHidden().tint(Palette.primary)
        }
    }
}

/// A list row whose value opens a menu of choices.
struct PickRow<T: Hashable>: View {
    let title: String
    var subtitle: String?
    let value: T
    let options: [(T, String)]
    let onPick: (T) -> Void

    var body: some View {
        ListRow(title: title, subtitle: subtitle) {
            Menu {
                Picker(title, selection: Binding(get: { value }, set: onPick)) {
                    ForEach(options, id: \.0) { Text($0.1).tag($0.0) }
                }
            } label: {
                HStack(spacing: 4) {
                    Text(options.first { $0.0 == value }?.1 ?? "").style(.body.with(color: Palette.muted)).lineLimit(1)
                    LIcon(.chevDown, Palette.muted, size: 16)
                }
                .contentShape(Rectangle())
            }
            .buttonStyle(.plain)
        }
    }
}

/// A list row with a segmented choice.
struct SegmentRow<T: Hashable>: View {
    let title: String
    let value: T
    let options: [(T, String)]
    let onPick: (T) -> Void

    var body: some View {
        ListRow(title: title) {
            Picker(title, selection: Binding(get: { value }, set: onPick)) {
                ForEach(options, id: \.0) { Text($0.1).tag($0.0) }
            }
            .pickerStyle(.segmented)
            .fixedSize()
        }
    }
}

/// A list row with a − value + stepper over a number.
struct StepRow: View {
    let title: String
    var subtitle: String?
    let value: Double
    var step = 1.0
    var range: ClosedRange<Double> = 0...1000
    var text: String?
    let onChange: (Double) -> Void

    var body: some View {
        ListRow(title: title, subtitle: subtitle) {
            CountStepper(
                value: text ?? num(value, decimals: 2),
                onMinus: { onChange(clamp(value - step)) },
                onPlus: { onChange(clamp(value + step)) }
            )
        }
    }

    private func clamp(_ v: Double) -> Double {
        // Steps of 0.1 would otherwise drift.
        min(max((v * 1000).rounded() / 1000, range.lowerBound), range.upperBound)
    }
}

/// A list row with a slider; the value is committed when the finger lifts.
struct SliderRow: View {
    let title: String
    let value: Double
    let range: ClosedRange<Double>
    var text: (Double) -> String = { num($0, decimals: 2) }
    let onCommit: (Double) -> Void
    @State private var live: Double?

    var body: some View {
        VStack(alignment: .leading, spacing: 6) {
            HStack {
                Text(title).style(.bodyStrong.with(size: 16))
                Spacer()
                Text(text(live ?? value)).style(.body.with(color: Palette.muted)).monospacedDigit()
            }
            Slider(value: Binding(get: { live ?? value }, set: { live = $0 }), in: range) { editing in
                if !editing, let v = live {
                    live = nil
                    if v != value { onCommit(v) }
                }
            }
        }
        .padding(.horizontal, 16)
        .padding(.vertical, 12)
    }
}

/// Title of a group of controls in a sheet.
struct GroupTitle: View {
    let text: String

    init(_ text: String) {
        self.text = text
    }

    var body: some View {
        Text(text).style(.bodyStrong.with(color: Palette.text2))
    }
}

/// A text editor committed when focus leaves (and on Return when single-line). `axis:
/// .vertical` grows with its lines, for multi-line text.
struct CommitField: View {
    let value: String
    let placeholder: String
    var axis = Axis.horizontal
    var plain = false
    let onCommit: (String) -> Void
    @State private var text = ""
    @FocusState private var focused: Bool

    var body: some View {
        let shape = RoundedRectangle(cornerRadius: 12)
        TextField("", text: $text, prompt: Text(placeholder).foregroundStyle(Palette.muted), axis: axis)
            .style(.body)
            .lineLimit(axis == .vertical ? 1...6 : 1...1)
            .focused($focused)
            .autocorrectionDisabled(plain)
            .textInputAutocapitalization(plain ? .never : .sentences)
            .submitLabel(axis == .vertical ? .return : .done)
            .onSubmit { if axis == .horizontal { commit() } }
            .padding(.horizontal, 12)
            .padding(.vertical, axis == .vertical ? 11 : 0)
            .frame(minHeight: 44)
            .background(Palette.surface, in: shape)
            .overlay(shape.strokeBorder(focused ? Palette.primary : Palette.border))
            .onAppear { text = value }
            .onChange(of: value) { if !focused { text = value } }
            .onChange(of: focused) { if !focused { commit() } }
    }

    private func commit() {
        if text != value { onCommit(text) }
    }
}

/// The system "save to" picker: copies files to the place the user picks.
struct SaveView: UIViewControllerRepresentable {
    let urls: [URL]

    func makeUIViewController(context: Context) -> UIDocumentPickerViewController {
        UIDocumentPickerViewController(forExporting: urls, asCopy: true)
    }

    func updateUIViewController(_ controller: UIDocumentPickerViewController, context: Context) {}
}

/// Files handed to a system sheet.
struct PickedFiles: Identifiable {
    let id = UUID()
    let urls: [URL]
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
