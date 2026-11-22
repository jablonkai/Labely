import CoreGraphics
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
    public var cornerRadiusMm = 0.0
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
        if type != "line", abs(raw["rotation_deg"]?.double ?? 0) > 0.01 {
            return rotated.contains(CGPoint(x: x, y: y), slop: slop)
        }
        // A line's width and height may be negative.
        let (x0, x1) = (min(xMm, xMm + widthMm), max(xMm, xMm + widthMm))
        let (y0, y1) = (min(yMm, yMm + heightMm), max(yMm, yMm + heightMm))
        return x >= x0 - slop && x <= x1 + slop && y >= y0 - slop && y <= y1 + slop
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
    /// `{{name}}` variables of the document.
    public var variables: [String: String] = [:]
    /// The embedded image assets by key, as `element.paste` takes them.
    public var assets: [String: JSON] = [:]

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
                density: l["density"]?.int,
                cornerRadiusMm: l["corner_radius_mm"]?.double ?? 0
            ),
            elements: (page?["elements"]?.objects ?? []).compactMap(Element.from),
            variables: (d["variables"]?.object ?? [:]).compactMapValues(\.string),
            assets: d["assets"]?.object ?? [:]
        )
    }
}

public struct Model: Sendable, Equatable, Codable {
    public let id: String
    public let name: String
    public let densityMin: Int
    public let densityMax: Int
    public let densityDefault: Int
    public var dpi = 203
    /// Dots of the printhead; with `dpi`, the widest line it prints.
    public var headDots = 0
    public var maxWidthMm = 0.0
    /// Prints 16 gray levels (`grayscale` in print.submit). Optional so models saved
    /// before it still decode.
    public var gray: Bool?

    /// The model prints gray levels.
    public var printsGray: Bool { gray == true }

    /// Width the printhead covers, in millimetres; 0 when unknown.
    public var headMm: Double { dpi > 0 ? Double(headDots) * 25.4 / Double(dpi) : 0 }

    static func from(_ m: JSON) -> Model? {
        guard m.object != nil else { return nil }
        return Model(
            id: m["id"]?.string ?? "",
            name: m["name"]?.string ?? "",
            densityMin: m["density_min"]?.int ?? 1,
            densityMax: m["density_max"]?.int ?? 5,
            densityDefault: m["density_default"]?.int ?? 3,
            dpi: m["dpi"]?.int ?? 203,
            headDots: m["head_dots"]?.int ?? 0,
            maxWidthMm: m["max_width_mm"]?.double ?? 0,
            gray: m["gray"]?.bool ?? false
        )
    }
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
        let model = o["model"].flatMap(Model.from)
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

/// The RFID tag of the loaded roll, as the printer reports it.
public struct PaperTag: Sendable, Equatable {
    public let barcode: String
    public let total: Int
    public let used: Int
    public let media: String?

    public var remaining: Int { max(total - used, 0) }

    static func from(_ p: JSON?) -> PaperTag? {
        guard let p, let barcode = p["barcode"]?.string else { return nil }
        return PaperTag(barcode: barcode, total: p["total"]?.int ?? 0, used: p["used"]?.int ?? 0, media: p["media"]?.string)
    }
}

public struct DeviceStatus: Sendable, Equatable {
    public let batteryPercent: Int?
    public let paperPresent: Bool?
    public var lidClosed: Bool?
    public var paper: PaperTag?

    static func from(_ r: JSON) -> DeviceStatus {
        let s = r["status"] ?? [:]
        return DeviceStatus(
            batteryPercent: s["battery_percent"]?.int, paperPresent: s["paper_present"]?.bool,
            lidClosed: s["lid_closed"]?.bool, paper: PaperTag.from(s["paper"])
        )
    }
}

/// `device.settings`: only what the printer reports is set.
public struct PrinterSettings: Sendable, Equatable {
    /// Auto power-off level 1–4 (NIIMBOT: 15, 30, 45, 60 minutes).
    public let autoOffLevel: Int?
    public let soundPower: Bool?
    public let soundBluetooth: Bool?

    public static let autoOffLevels = 1...4

    public var isEmpty: Bool { autoOffLevel == nil && soundPower == nil && soundBluetooth == nil }

    static func from(_ r: JSON) -> PrinterSettings {
        let s = r["settings"] ?? [:]
        return PrinterSettings(
            autoOffLevel: s["auto_off_level"]?.int, soundPower: s["sound_power"]?.bool, soundBluetooth: s["sound_bluetooth"]?.bool
        )
    }
}

/// `data.get` without its rows: the attached data source.
public struct DataInfo: Sendable, Equatable {
    /// File name the data came from; none for CSV attached as text.
    public let origin: String?
    public let columns: [String]
    public let rowCount: Int

    static func from(_ r: JSON) -> DataInfo? {
        guard r["attached"]?.bool == true else { return nil }
        let origin = r["origin"]?.string
        return DataInfo(
            origin: origin?.isEmpty == false ? origin : nil,
            columns: (r["columns"]?.array ?? []).compactMap(\.string),
            rowCount: r["row_count"]?.int ?? 0
        )
    }

    static func rows(_ r: JSON) -> [[String]] {
        (r["rows"]?.array ?? []).map { ($0.array ?? []).map { $0.string ?? "" } }
    }
}

/// `doc.check` issue.
public struct CheckIssue: Sendable, Equatable {
    public let rule: String
    public let severity: String
    public let message: String
    public let elementId: Int64?
    public var hint = ""

    static func list(_ r: JSON) -> [CheckIssue] {
        (r["issues"]?.objects ?? []).map {
            CheckIssue(
                rule: $0["rule"]?.string ?? "", severity: $0["severity"]?.string ?? "", message: $0["message"]?.string ?? "",
                elementId: $0["element_id"]?.int64, hint: $0["hint"]?.string ?? ""
            )
        }
    }
}

public struct HistoryEntry: Sendable, Equatable, Identifiable {
    public let id: String
    public let printedAt: String
    public let docName: String
    public let pages: Int
    public let state: String
    public var address = ""
    public var model: String?
    public var error: String?
    /// Distinct labels of the job and copies of each: `labels × copies` pages were asked for.
    public var labels = 0
    public var copies = 1
    /// Whether the printed document was kept, so it can be opened and printed again.
    public var hasFile = false

    /// Pages the job asked for.
    public var pagesTotal: Int { max(labels * copies, pages) }
    /// A job that stopped before its last page can be continued.
    public var partial: Bool { state != "done" && pages > 0 && pages < pagesTotal }

    static func list(_ r: JSON) -> [HistoryEntry] {
        (r["entries"]?.objects ?? []).compactMap { e in
            guard let id = e["id"]?.idString else { return nil }
            return HistoryEntry(
                id: id,
                printedAt: e["printed_at"]?.string ?? "",
                docName: e["doc_name"]?.string ?? "",
                pages: e["pages_printed"]?.int ?? 0,
                state: e["state"]?.string ?? "",
                address: e["address"]?.string ?? "",
                model: e["model"]?.string,
                error: e["error"]?.string,
                labels: e["labels"]?.int ?? 0,
                copies: e["copies"]?.int ?? 1,
                hasFile: e["file"]?.string != nil
            )
        }
    }
}

/// A label paper of the library (`paper.list`): `std:` bundled, `my:` the user's.
public struct Paper: Sendable, Equatable, Identifiable {
    public let id: String
    public let name: String
    public let widthMm: Double
    public let heightMm: Double
    public let cornerRadiusMm: Double
    public let media: String
    public let custom: Bool
    /// RFID barcodes linked to this paper.
    public let barcodes: [String]

    static func from(_ p: JSON?) -> Paper? {
        guard let p, let id = p["id"]?.string else { return nil }
        return Paper(
            id: id,
            name: p["name"]?.string ?? "",
            widthMm: p["width_mm"]?.double ?? 0,
            heightMm: p["height_mm"]?.double ?? 0,
            cornerRadiusMm: p["corner_radius_mm"]?.double ?? 0,
            media: p["media"]?.string ?? "gap",
            custom: p["custom"]?.bool ?? false,
            barcodes: (p["barcodes"]?.array ?? []).compactMap(\.string)
        )
    }

    static func list(_ r: JSON) -> [Paper] {
        (r["papers"]?.objects ?? []).compactMap { from($0) }
    }
}

/// `paper.detect`: the roll in the printer and how it compares with the label.
public struct PaperDetect: Sendable, Equatable {
    public let rfid: Bool
    public let barcode: String?
    public let media: String?
    public let remaining: Int?
    /// The paper the roll's barcode is linked to; none for an unknown roll.
    public let paper: Paper?
    public let matchesLabel: Bool?

    static func from(_ r: JSON) -> PaperDetect {
        PaperDetect(
            rfid: r["rfid"]?.bool ?? false, barcode: r["barcode"]?.string, media: r["media"]?.string,
            remaining: r["remaining"]?.int, paper: Paper.from(r["paper"]), matchesLabel: r["matches_label"]?.bool
        )
    }
}

/// A job of the print queue (`job.list` and the pushed job changes).
public struct PrintJob: Sendable, Equatable, Identifiable {
    public let id: String
    public let state: String
    public let address: String
    public let docName: String
    public let pagesDone: Int
    public let pagesTotal: Int
    public let error: String?

    public var finished: Bool { Engine.finished.contains(state) }

    static func from(_ j: JSON) -> PrintJob? {
        guard let id = j["id"]?.idString else { return nil }
        return PrintJob(
            id: id, state: j["state"]?.string ?? "", address: j["address"]?.string ?? "",
            docName: j["doc_name"]?.string ?? "", pagesDone: j["pages_done"]?.int ?? 0,
            pagesTotal: j["pages_total"]?.int ?? 0, error: j["error"]?["message"]?.string
        )
    }
}

/// A serial counter of the document (`data.set_serial`).
public struct Serial: Sendable, Equatable, Identifiable {
    public var name: String
    public var start = 1
    public var step = 1
    public var digits = 0
    public var prefix = ""
    public var suffix = ""

    public var id: String { name }

    public init(name: String, start: Int = 1, step: Int = 1, digits: Int = 0, prefix: String = "", suffix: String = "") {
        self.name = name
        self.start = start
        self.step = step
        self.digits = digits
        self.prefix = prefix
        self.suffix = suffix
    }

    /// The value on label `n` (0-based), as the engine prints it.
    public func value(_ n: Int) -> String {
        let v = start + step * n
        let digitsText = String(repeating: "0", count: max(digits - String(abs(v)).count, 0)) + String(abs(v))
        return prefix + (v < 0 ? "-" : "") + digitsText + suffix
    }

    static func list(_ r: JSON) -> [Serial] {
        (r["serials"]?.object ?? [:]).map { name, s in
            Serial(
                name: name, start: s["start"]?.int ?? 1, step: s["step"]?.int ?? 1, digits: s["digits"]?.int ?? 0,
                prefix: s["prefix"]?.string ?? "", suffix: s["suffix"]?.string ?? ""
            )
        }
        .sorted { $0.name < $1.name }
    }
}
