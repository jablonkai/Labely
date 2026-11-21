import LabelyKit
import SwiftUI

/// Colours of `design/labely-mobile.figcraft` (`C` in design/gen.py), as `Palette` on Android.
/// Each one follows the colour scheme in effect, so every view adapts to dark mode. The label
/// preview keeps white paper and black ink in both (it shows the print).
enum Palette {
    static let primary = Color(light: 0x4C6EF5, dark: 0x5C7CFA)
    /// Accent text on the tints.
    static let primaryDark = Color(light: 0x3B5BDB, dark: 0x91A7FF)
    static let primaryTint = Color(light: 0xEDF2FF, dark: 0x1E2640)
    static let primaryTint2 = Color(light: 0xDBE4FF, dark: 0x28345C)
    static let bg = Color(light: 0xF5F6F8, dark: 0x14171D)
    static let surface = Color(light: 0xFFFFFF, dark: 0x1C2028)
    static let border = Color(light: 0xE4E7EC, dark: 0x2E3440)
    static let borderSoft = Color(light: 0xEEF0F3, dark: 0x262B35)
    static let text = Color(light: 0x1B2130, dark: 0xE6E9EF)
    static let text2 = Color(light: 0x475066, dark: 0xB4BCCB)
    static let muted = Color(light: 0x8A93A6, dark: 0x8790A3)
    static let canvas = Color(light: 0xE8EBF0, dark: 0x101318)
    static let field = Color(light: 0xE9ECF1, dark: 0x252A34)
    static let green = Color(light: 0x12B886, dark: 0x20C997)
    static let greenTint = Color(light: 0xE6FCF5, dark: 0x0F2E26)
    static let amber = Color(light: 0xF59F00, dark: 0xFAB005)
    static let amberTint = Color(light: 0xFFF4E0, dark: 0x33270D)
    static let red = Color(light: 0xFA5252, dark: 0xFF6B6B)
}

extension Color {
    init(hex: UInt32) {
        self.init(
            red: Double((hex >> 16) & 0xFF) / 255,
            green: Double((hex >> 8) & 0xFF) / 255,
            blue: Double(hex & 0xFF) / 255
        )
    }

    /// A colour that is `light` in the light scheme and `dark` in the dark one.
    init(light: UInt32, dark: UInt32) {
        self.init(uiColor: UIColor { $0.userInterfaceStyle == .dark ? UIColor(rgb: dark) : UIColor(rgb: light) })
    }
}

extension UIColor {
    convenience init(rgb: UInt32) {
        self.init(
            red: CGFloat((rgb >> 16) & 0xFF) / 255,
            green: CGFloat((rgb >> 8) & 0xFF) / 255,
            blue: CGFloat(rgb & 0xFF) / 255,
            alpha: 1
        )
    }
}

extension Appearance {
    /// The scheme for `preferredColorScheme`: nil follows the system.
    var colorScheme: ColorScheme? {
        switch self {
        case .system: nil
        case .light: .light
        case .dark: .dark
        }
    }
}

/// Type scale of the mobile design (Inter there; the system font here).
struct TextStyle {
    var size: CGFloat
    var weight: Font.Weight = .regular
    var color: Color = Palette.text
    var tracking: CGFloat = 0

    static let largeTitle = TextStyle(size: 30, weight: .bold, tracking: -0.6)
    static let title = TextStyle(size: 22, weight: .bold)
    static let section = TextStyle(size: 17, weight: .bold)
    static let nav = TextStyle(size: 16, weight: .bold)
    static let body = TextStyle(size: 15)
    static let bodyStrong = TextStyle(size: 15, weight: .semibold)
    static let caption = TextStyle(size: 13, color: Palette.muted)
    static let small = TextStyle(size: 11, color: Palette.muted)

    func with(size: CGFloat? = nil, weight: Font.Weight? = nil, color: Color? = nil) -> TextStyle {
        var s = self
        if let size { s.size = size }
        if let weight { s.weight = weight }
        if let color { s.color = color }
        return s
    }
}

extension View {
    func style(_ s: TextStyle) -> some View {
        font(.system(size: s.size, weight: s.weight)).foregroundStyle(s.color).tracking(s.tracking)
    }
}

/// A localised string from `Localizable.xcstrings`. Keys follow the Android string names.
func L(_ key: String) -> String {
    NSLocalizedString(key, comment: "")
}

/// A localised format string; plural variants are chosen by the arguments.
func L(_ key: String, _ args: any CVarArg...) -> String {
    String(format: NSLocalizedString(key, comment: ""), locale: .current, arguments: args)
}

/// `50 × 30 mm` (or `1.969 × 1.181 in`), with decimals only where needed.
func sizeText(_ w: Double, _ h: Double) -> String {
    "\(mm(w)) × \(mm(h)) \(CurrentUnit.value.suffix)"
}

/// A length in the unit chosen in Settings, without the unit's name.
func mm(_ v: Double) -> String {
    CurrentUnit.value.format(v)
}

/// A length with its unit: `2.5 mm`.
func mmText(_ v: Double) -> String {
    "\(mm(v)) \(CurrentUnit.value.suffix)"
}

/// A plain number with up to `decimals` decimals.
func num(_ v: Double, decimals: Int = 1) -> String {
    v == v.rounded() ? String(Int64(v)) : v.formatted(.number.precision(.fractionLength(0...decimals)).grouping(.never))
}

/// One of the design's stroke icons, tinted and sized.
struct LIcon: View {
    let icon: Icon
    var tint: Color = Palette.text2
    var size: CGFloat = 22

    init(_ icon: Icon, _ tint: Color = Palette.text2, size: CGFloat = 22) {
        self.icon = icon
        self.tint = tint
        self.size = size
    }

    var body: some View {
        IconShape(icon: icon)
            .stroke(tint, style: StrokeStyle(lineWidth: 1.8 * size / 24, lineCap: .round, lineJoin: .round))
            .frame(width: size, height: size)
            .accessibilityHidden(true)
    }
}

struct IconShape: Shape {
    let icon: Icon

    func path(in rect: CGRect) -> Path {
        let s = min(rect.width, rect.height) / 24
        return Path(icon.path).applying(CGAffineTransform(scaleX: s, y: s).translatedBy(x: rect.minX / s, y: rect.minY / s))
    }
}
