import Foundation

/// The NIIMBOT GATT knowledge of `labely_transport::gatt` (docs/protocol.md §3.1), mirrored for
/// the shell, which picks the characteristic before Rust sees the link. Same rules as `Gatt.kt`.
public enum Gatt {
    public static let niimbotService = "e7810a71-73ae-499d-8c15-faa9aef0c3f2"

    /// Known (service, characteristic) pairs in preference order; `ffe1` is a 16-bit alias.
    public static let knownPairs: [(service: String, characteristic: String)] = [
        (niimbotService, "bef8d6c9-9c21-4c9e-b632-bd58c1009f9f"),
        ("0000e0ff-3c17-d293-8e48-14fe2e4da212", "ffe1"),
        ("49535343-fe7d-4ae5-8fa9-9fafd205e455", "49535343-8841-43f4-a8d4-ecbe34729bb3"),
    ]

    private static let baseSuffix = "-0000-1000-8000-00805f9b34fb"

    /// A UUID as 36 lowercase characters. CoreBluetooth prints SIG-base UUIDs in their 16- or
    /// 32-bit form (`FFE1`), which this expands.
    public static func canonical(_ uuid: String) -> String {
        let u = uuid.lowercased()
        switch u.count {
        case 4: return "0000\(u)\(baseSuffix)"
        case 8: return "\(u)\(baseSuffix)"
        default: return u
        }
    }

    /// A UUID against a full UUID or a 16-bit alias, case-insensitively. A 16-bit alias
    /// matches `0000xxxx-` on any base, as on Android.
    public static func uuidMatches(_ uuid: String, _ pattern: String) -> Bool {
        let u = canonical(uuid)
        let p = pattern.lowercased()
        return p.count == 4 ? u.hasPrefix("0000\(p)-") : u == canonical(p)
    }

    /// Same rule as `gatt::scan_matches`.
    public static func scanMatches(name: String, services: [String], prefixes: [String]) -> Bool {
        prefixes.isEmpty
            || services.contains { uuidMatches($0, niimbotService) }
            || prefixes.contains { name.lowercased().hasPrefix($0.lowercased()) }
    }

    /// Whether `service` is one the printer characteristic can live in.
    public static func isKnownService(_ service: String) -> Bool {
        knownPairs.contains { uuidMatches(service, $0.service) }
    }

    /// Index of the preferred usable characteristic among `candidates`, each a
    /// (service UUID, characteristic UUID, usable) triple.
    public static func pick(_ candidates: [(service: String, characteristic: String, usable: Bool)]) -> Int? {
        for pair in knownPairs {
            if let i = candidates.firstIndex(where: {
                $0.usable && uuidMatches($0.service, pair.service) && uuidMatches($0.characteristic, pair.characteristic)
            }) {
                return i
            }
        }
        return nil
    }
}
