import Foundation
import Synchronization

// Pure helpers of the shell: units, record ranges, the element clipboard and text spans.

/// The colour scheme the app is shown in: the system's (the default), or always light or dark.
/// The label preview stays white paper and black ink in either.
public enum Appearance: String, Sendable, CaseIterable, Codable {
    case system, light, dark
}

/// The unit sizes are shown and typed in. The engine and the files always use millimetres.
public enum LengthUnit: String, Sendable, CaseIterable, Codable {
    case mm, inch

    public static let mmPerInch = 25.4

    public var suffix: String { self == .mm ? "mm" : "in" }

    /// Decimals shown: 0.1 mm or 0.001 in.
    public var decimals: Int { self == .mm ? 1 : 3 }

    /// `mm` in this unit.
    public func value(_ mm: Double) -> Double {
        self == .mm ? mm : mm / Self.mmPerInch
    }

    /// A value in this unit as millimetres, rounded to 0.01 mm.
    public func mm(_ value: Double) -> Double {
        self == .mm ? value : (value * Self.mmPerInch * 100).rounded() / 100
    }

    /// `mm` in this unit, with only the decimals it needs.
    public func format(_ mm: Double, locale: Locale = .current) -> String {
        let p = pow(10, Double(decimals))
        let v = (value(mm) * p).rounded() / p
        if v == v.rounded() { return String(Int64(v)) }
        return v.formatted(.number.precision(.fractionLength(0...decimals)).grouping(.never).locale(locale))
    }

    /// What the user typed, as millimetres; a decimal comma is fine.
    public func parse(_ text: String) -> Double? {
        let t = text.trimmingCharacters(in: .whitespaces).replacingOccurrences(of: ",", with: ".")
        guard let v = Double(t), v.isFinite else { return nil }
        return mm(v)
    }
}

/// The record list of `print.submit`: `all`, `3`, `1,4,6-9`, `5-` (to the last), as the engine
/// reads it.
public enum RecordRange {
    /// The records `spec` names among `count`, 1-based and in the order given; `nil` when it is
    /// not a valid list.
    public static func parse(_ spec: String, count: Int) -> [Int]? {
        let spec = spec.trimmingCharacters(in: .whitespaces)
        if spec.isEmpty || spec.lowercased() == "all" { return count > 0 ? Array(1...count) : [] }
        func num(_ s: Substring) -> Int? {
            guard let n = Int(s.trimmingCharacters(in: .whitespaces)), n >= 1, n <= count else { return nil }
            return n
        }
        var out: [Int] = []
        for part in spec.split(separator: ",", omittingEmptySubsequences: false) {
            if let dash = part.firstIndex(where: { $0 == "-" || $0 == "–" }) {
                guard let a = num(part[..<dash]) else { return nil }
                let rest = part[part.index(after: dash)...]
                let b: Int
                if rest.trimmingCharacters(in: .whitespaces).isEmpty {
                    b = count
                } else {
                    guard let n = num(rest) else { return nil }
                    b = n
                }
                out += a <= b ? Array(a...b) : Array((b...a).reversed())
            } else {
                guard let n = num(part) else { return nil }
                out.append(n)
            }
        }
        return out
    }
}

/// Elements on the pasteboard, in the studio's format, so they paste between documents and
/// between the two apps.
public enum ElementClipboard {
    public static let format = "labely/elements"

    /// The pasteboard text of `elements` with the image assets they use.
    public static func encode(_ elements: [Element], assets: [String: JSON]) -> String {
        var used: [String: JSON] = [:]
        for e in elements where e.type == "image" {
            if let key = e.raw["asset"]?.string, let a = assets[key] { used[key] = a }
        }
        let o: JSON = ["format": JSON(format), "elements": .array(elements.map(\.raw)), "assets": .object(used)]
        return o.text
    }

    /// The elements and assets of pasteboard text; `nil` when it is anything else.
    public static func decode(_ text: String) -> (elements: [JSON], assets: JSON)? {
        guard let v = try? JSON(parsing: text), v["format"]?.string == format, let elements = v["elements"]?.array else { return nil }
        return (elements, v["assets"] ?? [:])
    }

    /// `text` with every element moved by `d` mm, for pasting again a step further.
    public static func shifted(_ text: String, by d: Double) -> String {
        guard let (elements, assets) = decode(text) else { return text }
        let moved = elements.map { e in
            e.with("x_mm", JSON((e["x_mm"]?.double ?? 0) + d)).with("y_mm", JSON((e["y_mm"]?.double ?? 0) + d))
        }
        let o: JSON = ["format": JSON(format), "elements": .array(moved), "assets": assets]
        return o.text
    }
}

/// The style of a range of a text element (`spans` of `element.set_props`). Positions count
/// Unicode scalars, as the engine does.
public struct TextSpan: Sendable, Equatable {
    public var start: Int
    public var end: Int
    public var bold: Bool?
    public var italic: Bool?
    public var underline: Bool?
    public var fontSizePt: Double?

    public init(start: Int, end: Int, bold: Bool? = nil, italic: Bool? = nil, underline: Bool? = nil, fontSizePt: Double? = nil) {
        self.start = start
        self.end = end
        self.bold = bold
        self.italic = italic
        self.underline = underline
        self.fontSizePt = fontSizePt
    }

    var isPlain: Bool { bold == nil && italic == nil && underline == nil && fontSizePt == nil }

    public var json: JSON {
        [
            "start": JSON(start), "end": JSON(end), "bold": bold.map { JSON($0) }, "italic": italic.map { JSON($0) },
            "underline": underline.map { JSON($0) }, "font_size_pt": fontSizePt.map { JSON($0) },
        ]
    }

    static func from(_ o: JSON) -> TextSpan? {
        guard let start = o["start"]?.int, let end = o["end"]?.int else { return nil }
        return TextSpan(
            start: start, end: end, bold: o["bold"]?.bool, italic: o["italic"]?.bool, underline: o["underline"]?.bool,
            fontSizePt: o["font_size_pt"]?.double
        )
    }
}

public enum TextSpans {
    /// The engine keeps at most this many spans.
    public static let maxSpans = 256

    public static func of(_ e: Element) -> [TextSpan] {
        (e.raw["spans"]?.objects ?? []).compactMap(TextSpan.from)
    }

    /// The scalar range of a UTF-16 range of `text`, as a text view reports its selection.
    public static func scalarRange(of utf16: Range<Int>, in text: String) -> Range<Int> {
        let u = text.utf16
        func scalars(_ offset: Int) -> Int {
            let i = u.index(u.startIndex, offsetBy: min(max(offset, 0), u.count))
            return text.unicodeScalars.distance(from: text.unicodeScalars.startIndex, to: i.samePosition(in: text.unicodeScalars) ?? text.unicodeScalars.endIndex)
        }
        let a = scalars(utf16.lowerBound)
        return a..<max(scalars(utf16.upperBound), a)
    }

    /// The style in effect at scalar `i`: later spans win, unset fields come from `base`.
    public static func style(at i: Int, spans: [TextSpan], base: TextSpan) -> TextSpan {
        var s = base
        for span in spans where span.start <= i && i < span.end {
            if let v = span.bold { s.bold = v }
            if let v = span.italic { s.italic = v }
            if let v = span.underline { s.underline = v }
            if let v = span.fontSizePt { s.fontSizePt = v }
        }
        return s
    }

    /// `spans` with `change` applied on top of `range`. The spans are cut at the range's ends,
    /// so the result never overlaps; neighbours with the same style are joined.
    public static func applying(_ spans: [TextSpan], range: Range<Int>, _ change: (inout TextSpan) -> Void) -> [TextSpan] {
        guard !range.isEmpty else { return spans }
        // Flatten into runs at every boundary.
        var cuts = Set([range.lowerBound, range.upperBound])
        for s in spans {
            cuts.insert(s.start)
            cuts.insert(s.end)
        }
        let points = cuts.sorted()
        var out: [TextSpan] = []
        for (a, b) in zip(points, points.dropFirst()) {
            var run = style(at: a, spans: spans, base: TextSpan(start: a, end: b))
            run.start = a
            run.end = b
            if a >= range.lowerBound && b <= range.upperBound { change(&run) }
            if run.isPlain { continue }
            if var last = out.last, last.end == a, same(last, run) {
                last.end = b
                out[out.count - 1] = last
            } else {
                out.append(run)
            }
        }
        return Array(out.prefix(maxSpans))
    }

    /// `spans` without any style inside `range`.
    public static func clearing(_ spans: [TextSpan], range: Range<Int>) -> [TextSpan] {
        applying(spans, range: range) { s in
            s.bold = nil
            s.italic = nil
            s.underline = nil
            s.fontSizePt = nil
        }
    }

    private static func same(_ a: TextSpan, _ b: TextSpan) -> Bool {
        a.bold == b.bold && a.italic == b.italic && a.underline == b.underline && a.fontSizePt == b.fontSizePt
    }
}

/// What the user set up in the app, kept across launches: units, the canvas helpers and the
/// paper behaviour. The studio keeps the same in its preferences.
public struct Prefs: Sendable, Equatable, Codable {
    public static let gridSteps = [0.5, 1, 2, 5]

    public var unit = LengthUnit.mm
    public var showGrid = false
    public var snap = true
    public var gridMm = 1.0
    public var rulers = false
    /// The rotation handle lands on 15° steps.
    public var rotateSnap = true
    /// Use the paper the printer reports for the open label by itself.
    public var autoApplyPaper = false
    /// Colour the label is drawn with on screen, `RRGGBB`; the print is not affected.
    public var labelColor = "FFFFFF"
    /// Light, dark or the system's colour scheme.
    public var appearance = Appearance.system

    public init() {}

    public init(from decoder: Decoder) throws {
        let c = try decoder.container(keyedBy: CodingKeys.self)
        unit = try c.decodeIfPresent(LengthUnit.self, forKey: .unit) ?? .mm
        showGrid = try c.decodeIfPresent(Bool.self, forKey: .showGrid) ?? false
        snap = try c.decodeIfPresent(Bool.self, forKey: .snap) ?? true
        gridMm = try c.decodeIfPresent(Double.self, forKey: .gridMm) ?? 1
        rulers = try c.decodeIfPresent(Bool.self, forKey: .rulers) ?? false
        rotateSnap = try c.decodeIfPresent(Bool.self, forKey: .rotateSnap) ?? true
        autoApplyPaper = try c.decodeIfPresent(Bool.self, forKey: .autoApplyPaper) ?? false
        labelColor = try c.decodeIfPresent(String.self, forKey: .labelColor) ?? "FFFFFF"
        appearance = try c.decodeIfPresent(Appearance.self, forKey: .appearance) ?? .system
    }
}

/// Ruler ticks: minor ticks at least 5 pt apart and labelled ones at least 40 pt, as the
/// studio's rulers.
public enum Ruler {
    public static func steps(unit: LengthUnit, ptPerMm: Double) -> (minor: Double, major: Double) {
        let per = unit == .mm ? 1.0 : LengthUnit.mmPerInch
        let candidates: [Double] = unit == .mm ? [0.5, 1, 2, 5, 10, 20, 50, 100] : [1.0 / 16, 0.125, 0.25, 0.5, 1, 2, 5]
        let minor = candidates.first { $0 * per * ptPerMm >= 5 } ?? candidates[candidates.count - 1]
        let major = candidates.first { $0 * per * ptPerMm >= 40 } ?? candidates[candidates.count - 1]
        return (minor * per, max(major, minor) * per)
    }
}

/// The unit in effect, for formatting outside the views (App Intents run off the main actor).
/// `AppState` keeps it equal to `prefs.unit`.
public enum CurrentUnit {
    private static let store = Mutex(LengthUnit.mm)

    public static var value: LengthUnit {
        get { store.withLock { $0 } }
        set { store.withLock { $0 = newValue } }
    }
}
