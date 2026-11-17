import Foundation

// Typed views of the engine's JSON results, as `Models.kt` on Android. Only what the shell
// displays is read.

public struct Template: Sendable, Equatable, Identifiable {
    public let ref: String
    public let name: String
    public let folder: String
    public let starter: Bool
    public let widthMm: Double
    public let heightMm: Double

    public var id: String { ref }

    static func list(_ r: JSON) -> [Template] {
        (r["templates"]?.objects ?? []).compactMap { t in
            guard let ref = t["ref"]?.string else { return nil }
            return Template(
                ref: ref,
                name: t["name"]?.string ?? "",
                folder: t["folder"]?.string ?? "",
                starter: t["starter"]?.bool ?? false,
                widthMm: t["width_mm"]?.double ?? 0,
                heightMm: t["height_mm"]?.double ?? 0
            )
        }
    }
}

public struct LabelInfo: Sendable, Equatable {
    public let widthMm: Double
    public let heightMm: Double
    public let media: String
    public let printerModel: String?
    public let density: Int?
}

/// One element of the document, with the raw JSON for kind-specific properties.
public struct Element: Sendable, Equatable, Identifiable {
    public let id: Int64
    public let type: String
    public let name: String?
    public let xMm: Double
    public let yMm: Double
    public let widthMm: Double
    public let heightMm: Double
    public let locked: Bool
    public let hidden: Bool
    public let raw: JSON

    public var text: String? { raw["text"]?.string }
    public var data: String? { raw["data"]?.string }
    public var fontSizePt: Double { raw["font_size_pt"]?.double ?? 10 }
    public var bold: Bool { raw["bold"]?.bool ?? false }
    public var align: String { raw["align"]?.string ?? "left" }

    public func contains(xMm x: Double, yMm y: Double, slop: Double = 0) -> Bool {
        x >= xMm - slop && x <= xMm + widthMm + slop && y >= yMm - slop && y <= yMm + heightMm + slop
    }

    static func from(_ o: JSON) -> Element? {
        guard let id = o["id"]?.int64 else { return nil }
        let name = o["name"]?.string
        return Element(
            id: id,
            type: o["type"]?.string ?? "",
            name: name?.isEmpty == false ? name : nil,
            xMm: o["x_mm"]?.double ?? 0,
            yMm: o["y_mm"]?.double ?? 0,
            widthMm: o["width_mm"]?.double ?? 0,
            heightMm: o["height_mm"]?.double ?? 0,
            locked: o["locked"]?.bool ?? false,
            hidden: o["hidden"]?.bool ?? false,
            raw: o
        )
    }
}

/// `doc.get` result. Elements are bottom-first, as drawn.
public struct Doc: Sendable, Equatable {
    public let docId: String
    public let name: String
    public var path: String?
    public let dirty: Bool
    public let canUndo: Bool
    public let canRedo: Bool
    public let label: LabelInfo
    public let elements: [Element]

    static func from(_ r: JSON) throws -> Doc {
        guard let docId = r["doc_id"]?.string, let d = r["document"], let l = d["label"] else {
            throw CommandFailure(code: "internal", message: "doc.get returned no document", hint: nil)
        }
        let page = d["pages"]?.objects.first
        let path = r["path"]?.string
        let model = l["printer_model"]?.string
        return Doc(
            docId: docId,
            name: d["name"]?.string ?? "",
            path: path?.isEmpty == false ? path : nil,
            dirty: r["dirty"]?.bool ?? false,
            canUndo: r["can_undo"]?.bool ?? false,
            canRedo: r["can_redo"]?.bool ?? false,
            label: LabelInfo(
                widthMm: l["width_mm"]?.double ?? 0,
                heightMm: l["height_mm"]?.double ?? 0,
                media: l["media"]?.string ?? "gap",
                printerModel: model?.isEmpty == false ? model : nil,
                density: l["density"]?.int
            ),
            elements: (page?["elements"]?.objects ?? []).compactMap(Element.from)
        )
    }
}

public struct Model: Sendable, Equatable {
    public let id: String
    public let name: String
    public let densityMin: Int
    public let densityMax: Int
    public let densityDefault: Int
}

/// `device.connect` / `device.list` entry.
public struct Device: Sendable, Equatable {
    public let address: String
    public let name: String
    public let connected: Bool
    public let model: Model?
    public let firmware: String?
    public let serial: String?

    static func from(_ o: JSON) -> Device? {
        guard let address = o["address"]?.string else { return nil }
        let model = o["model"].flatMap { m -> Model? in
            guard m.object != nil else { return nil }
            return Model(
                id: m["id"]?.string ?? "",
                name: m["name"]?.string ?? "",
                densityMin: m["density_min"]?.int ?? 1,
                densityMax: m["density_max"]?.int ?? 5,
                densityDefault: m["density_default"]?.int ?? 3
            )
        }
        let firmware = o["firmware"]?.string
        let serial = o["serial"]?.string
        return Device(
            address: address,
            name: o["name"]?.string ?? "",
            connected: o["connected"]?.bool ?? false,
            model: model,
            firmware: firmware?.isEmpty == false ? firmware : nil,
            serial: serial?.isEmpty == false ? serial : nil
        )
    }
}

public struct DeviceStatus: Sendable, Equatable {
    public let batteryPercent: Int?
    public let paperPresent: Bool?

    static func from(_ r: JSON) -> DeviceStatus {
        let s = r["status"] ?? [:]
        return DeviceStatus(batteryPercent: s["battery_percent"]?.int, paperPresent: s["paper_present"]?.bool)
    }
}

/// `doc.check` issue.
public struct CheckIssue: Sendable, Equatable {
    public let rule: String
    public let severity: String
    public let message: String

    static func list(_ r: JSON) -> [CheckIssue] {
        (r["issues"]?.objects ?? []).map {
            CheckIssue(rule: $0["rule"]?.string ?? "", severity: $0["severity"]?.string ?? "", message: $0["message"]?.string ?? "")
        }
    }
}

public struct HistoryEntry: Sendable, Equatable, Identifiable {
    public let id: String
    public let printedAt: String
    public let docName: String
    public let pages: Int
    public let state: String

    static func list(_ r: JSON) -> [HistoryEntry] {
        (r["entries"]?.objects ?? []).compactMap { e in
            guard let id = e["id"]?.idString else { return nil }
            return HistoryEntry(
                id: id,
                printedAt: e["printed_at"]?.string ?? "",
                docName: e["doc_name"]?.string ?? "",
                pages: e["pages_printed"]?.int ?? 0,
                state: e["state"]?.string ?? ""
            )
        }
    }
}
