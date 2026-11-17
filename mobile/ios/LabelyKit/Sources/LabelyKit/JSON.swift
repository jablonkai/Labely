import Foundation

/// A JSON value. Command params and results cross the engine boundary as JSON text; this keeps
/// them `Sendable` and typed on the Swift side.
public enum JSON: Sendable, Equatable, Hashable {
    case null
    case bool(Bool)
    case int(Int64)
    case double(Double)
    case string(String)
    case array([JSON])
    case object([String: JSON])

    /// Parse JSON text.
    public init(parsing text: String) throws {
        self = try JSONDecoder().decode(JSON.self, from: Data(text.utf8))
    }

    /// Compact JSON text.
    public var text: String {
        // Encoding a JSON value cannot fail: every case maps to a JSON type.
        (try? String(decoding: JSONEncoder().encode(self), as: UTF8.self)) ?? "null"
    }

    public subscript(key: String) -> JSON? {
        if case .object(let o) = self { o[key] } else { nil }
    }

    public var string: String? {
        if case .string(let s) = self { s } else { nil }
    }

    public var double: Double? {
        switch self {
        case .int(let i): Double(i)
        case .double(let d): d
        default: nil
        }
    }

    public var int: Int? {
        switch self {
        case .int(let i): Int(exactly: i)
        case .double(let d): Int(exactly: d.rounded(.towardZero))
        default: nil
        }
    }

    public var int64: Int64? {
        switch self {
        case .int(let i): i
        case .double(let d): Int64(exactly: d.rounded(.towardZero))
        default: nil
        }
    }

    public var bool: Bool? {
        if case .bool(let b) = self { b } else { nil }
    }

    public var array: [JSON]? {
        if case .array(let a) = self { a } else { nil }
    }

    public var object: [String: JSON]? {
        if case .object(let o) = self { o } else { nil }
    }

    public var isNull: Bool { self == .null }

    /// An id that may be sent as a string or a number (job and history ids), as text.
    public var idString: String? {
        switch self {
        case .string(let s): s
        case .int(let i): String(i)
        default: nil
        }
    }

    /// The objects of an array value; anything else gives none.
    public var objects: [JSON] {
        (array ?? []).filter { $0.object != nil }
    }

    /// This object with `key` set; `nil` leaves the key out, as `json()` does on Android.
    public func with(_ key: String, _ value: JSON?) -> JSON {
        var o = object ?? [:]
        o[key] = value
        return .object(o)
    }
}

extension JSON: Codable {
    public init(from decoder: Decoder) throws {
        let c = try decoder.singleValueContainer()
        if c.decodeNil() {
            self = .null
        } else if let b = try? c.decode(Bool.self) {
            self = .bool(b)
        } else if let i = try? c.decode(Int64.self) {
            self = .int(i)
        } else if let d = try? c.decode(Double.self) {
            self = .double(d)
        } else if let s = try? c.decode(String.self) {
            self = .string(s)
        } else if let a = try? c.decode([JSON].self) {
            self = .array(a)
        } else {
            self = .object(try c.decode([String: JSON].self))
        }
    }

    public func encode(to encoder: Encoder) throws {
        var c = encoder.singleValueContainer()
        switch self {
        case .null: try c.encodeNil()
        case .bool(let b): try c.encode(b)
        case .int(let i): try c.encode(i)
        case .double(let d): try c.encode(d)
        case .string(let s): try c.encode(s)
        case .array(let a): try c.encode(a)
        case .object(let o): try c.encode(o)
        }
    }
}

extension JSON: ExpressibleByDictionaryLiteral, ExpressibleByArrayLiteral, ExpressibleByStringLiteral,
    ExpressibleByIntegerLiteral, ExpressibleByFloatLiteral, ExpressibleByBooleanLiteral,
    ExpressibleByNilLiteral
{
    /// `nil` values are left out, so optional params can be written inline.
    public init(dictionaryLiteral elements: (String, JSON?)...) {
        var o: [String: JSON] = [:]
        for (k, v) in elements { if let v { o[k] = v } }
        self = .object(o)
    }

    public init(arrayLiteral elements: JSON...) { self = .array(elements) }
    public init(stringLiteral value: String) { self = .string(value) }
    public init(integerLiteral value: Int64) { self = .int(value) }
    public init(floatLiteral value: Double) { self = .double(value) }
    public init(booleanLiteral value: Bool) { self = .bool(value) }
    public init(nilLiteral: ()) { self = .null }
}

/// Conversions for building params from Swift values.
extension JSON {
    public init(_ s: String) { self = .string(s) }
    public init(_ d: Double) { self = .double(d) }
    public init(_ i: Int) { self = .int(Int64(i)) }
    public init(_ i: Int64) { self = .int(i) }
    public init(_ b: Bool) { self = .bool(b) }

    public init?(_ s: String?) {
        guard let s else { return nil }
        self = .string(s)
    }

    public init?(_ i: Int?) {
        guard let i else { return nil }
        self = .int(Int64(i))
    }
}
