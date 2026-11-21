import CoreGraphics
import Foundation

/// The frame styles of `element.add_border`.
public enum BorderStyles {
    public static let all = ["single", "double", "thick_thin", "dashed", "dotted", "corners", "ticket", "scallop"]
}

/// The date, time and serial bindings the Insert sheet offers, as the studio's Insert menu.
public enum BoundText {
    public static let date = "{{now:%Y-%m-%d}}"
    public static let time = "{{now:%H:%M}}"
    public static let serial = "{{serial}}"
}

/// The line styles of the inspector: `dash_mm` of a line by its stroke width.
public enum LineStyle: String, Sendable, CaseIterable {
    case solid, dashed, dotted

    /// `dash_mm` for a line `strokeMm` wide; `nil` is solid.
    public func dash(strokeMm: Double) -> [Double]? {
        let w = max(strokeMm, 0.1)
        switch self {
        case .solid: return nil
        case .dashed: return [round1(max(w * 4, 1)), round1(max(w * 2, 0.5))]
        case .dotted: return [round1(w), round1(w)]
        }
    }

    /// The style a `dash_mm` shows as: equal dash and gap is dotted.
    public static func of(_ dash: [Double]?) -> LineStyle {
        guard let dash, dash.count == 2 else { return .solid }
        return abs(dash[0] - dash[1]) < 0.05 ? .dotted : .dashed
    }
}

/// A table element's grid, read from its JSON.
public struct TableInfo: Sendable, Equatable {
    public struct Cell: Sendable, Equatable {
        public var text = ""
        public var bold = false
        public var align: String?
        public var fontSizePt: Double?
        public var invert = false
    }

    public struct Merge: Sendable, Equatable {
        public let row: Int
        public let col: Int
        public let rows: Int
        public let cols: Int

        public func contains(_ r: Int, _ c: Int) -> Bool {
            (row..<row + rows).contains(r) && (col..<col + cols).contains(c)
        }
    }

    public let columnWidths: [Double]
    public let rowHeights: [Double]
    public let cells: [[Cell]]
    public let merges: [Merge]
    public let fontSizePt: Double
    public let align: String
    public let strokeMm: Double

    public var rows: Int { rowHeights.count }
    public var columns: Int { columnWidths.count }

    public func cell(_ r: Int, _ c: Int) -> Cell {
        r < cells.count && c < cells[r].count ? cells[r][c] : Cell()
    }

    /// The merge covering `(r, c)`.
    public func merge(at r: Int, _ c: Int) -> Merge? {
        merges.first { $0.contains(r, c) }
    }

    /// Whether `(r, c)` is covered by a merge without being its top-left cell.
    public func isCovered(_ r: Int, _ c: Int) -> Bool {
        merge(at: r, c).map { $0.row != r || $0.col != c } ?? false
    }

    public static func of(_ e: Element) -> TableInfo? {
        guard e.type == "table" else { return nil }
        let cells: [[Cell]] = (e.raw["cells"]?.array ?? []).map { row in
            (row.array ?? []).map { c in
                Cell(
                    text: c["text"]?.string ?? "", bold: c["bold"]?.bool ?? false, align: c["align"]?.string,
                    fontSizePt: c["font_size_pt"]?.double, invert: c["invert"]?.bool ?? false
                )
            }
        }
        return TableInfo(
            columnWidths: (e.raw["column_widths"]?.array ?? []).compactMap(\.double),
            rowHeights: (e.raw["row_heights"]?.array ?? []).compactMap(\.double),
            cells: cells,
            merges: (e.raw["merges"]?.objects ?? []).map {
                Merge(row: $0["row"]?.int ?? 0, col: $0["col"]?.int ?? 0, rows: $0["rows"]?.int ?? 1, cols: $0["cols"]?.int ?? 1)
            },
            fontSizePt: e.raw["font_size_pt"]?.double ?? 8,
            align: e.raw["align"]?.string ?? "center",
            strokeMm: e.raw["stroke_mm"]?.double ?? 0.3
        )
    }
}

extension Element {
    public var italic: Bool { raw["italic"]?.bool ?? false }
    public var underline: Bool { raw["underline"]?.bool ?? false }
    public var fontFamily: String { raw["font_family"]?.string ?? "Noto Sans" }
    public var valign: String { raw["valign"]?.string ?? "top" }
    public var fit: String { raw["fit"]?.string ?? (type == "image" ? "contain" : "none") }
    public var lineSpacing: Double { raw["line_spacing"]?.double ?? 1 }
    public var letterSpacingMm: Double { raw["letter_spacing_mm"]?.double ?? 0 }
    public var wrap: Bool { raw["wrap"]?.bool ?? true }
    public var direction: String { raw["direction"]?.string ?? "horizontal" }
    public var maxLengthMm: Double? { raw["max_length_mm"]?.double }
    public var textSizePt: Double { raw["text_size_pt"]?.double ?? 8 }
    public var ecc: String { raw["ecc"]?.string ?? "M" }
    public var brightness: Double { raw["brightness"]?.double ?? 0 }
    public var contrast: Double { raw["contrast"]?.double ?? 0 }
    public var strokeMm: Double { raw["stroke_mm"]?.double ?? (type == "border" ? 0.5 : 0.3) }
    public var fill: Bool { raw["fill"]?.bool ?? false }
    public var cornerRadiusMm: Double { raw["corner_radius_mm"]?.double ?? 0 }
    public var dashMm: [Double]? { raw["dash_mm"]?.array?.compactMap(\.double) }
    public var keepAspect: Bool { raw["keep_aspect"]?.bool ?? true }
    public var borderStyle: String { raw["style"]?.string ?? "single" }
    public var iconName: String { raw["icon"]?.string ?? "" }
    /// The crop of an image as left, top, right, bottom in 0–1; `nil` shows the whole image.
    public var crop: (left: Double, top: Double, right: Double, bottom: Double)? {
        guard let c = raw["crop"], c.object != nil else { return nil }
        return (c["left"]?.double ?? 0, c["top"]?.double ?? 0, c["right"]?.double ?? 1, c["bottom"]?.double ?? 1)
    }
}

/// The file the attached data came from, kept for the session so the sheet and the header row
/// can be changed without picking it again.
struct DataFile: Equatable {
    enum Kind: Equatable {
        case csv(String)
        case sheet(URL)
    }

    var kind: Kind
    var name: String?
    var sheets: [String] = []
    var sheet: String?
    var header = true
}

/// Inserting the further element kinds, tables, serials and variables.
extension EditorState {
    /// A frame centred on the label: `w` × `h` mm, at most 90 % of the label, as the studio's.
    func centred(_ w: Double, _ h: Double) -> JSON {
        guard let l = doc?.label else { return ["x_mm": 0, "y_mm": 0, "width_mm": JSON(w), "height_mm": JSON(h)] }
        let fw = round1(min(w, l.widthMm * 0.9))
        let fh = round1(min(h, l.heightMm * 0.9))
        return [
            "x_mm": JSON(round1((l.widthMm - fw) / 2)), "y_mm": JSON(round1((l.heightMm - fh) / 2)),
            "width_mm": JSON(fw), "height_mm": JSON(fh),
        ]
    }

    @discardableResult
    public func addTable(rows: Int = 3, columns: Int = 3) -> Task<Void, Never> {
        guard let l = doc?.label else { return Task {} }
        return exec(
            "element.add_table",
            centred(l.widthMm * 0.8, l.heightMm * 0.6).with("row_count", JSON(rows)).with("column_count", JSON(columns)),
            select: true
        )
    }

    @discardableResult
    public func addIcon(_ name: String) -> Task<Void, Never> {
        guard let l = doc?.label else { return Task {} }
        let s = min(l.heightMm * 0.5, l.widthMm * 0.5)
        return exec("element.add_icon", centred(s, s).with("icon", JSON(name)), select: true)
    }

    /// A frame around the whole label.
    @discardableResult
    public func addBorder(_ style: String = "single") -> Task<Void, Never> {
        guard let l = doc?.label else { return Task {} }
        return exec("element.add_border", [
            "x_mm": 0.0, "y_mm": 0.0, "width_mm": JSON(l.widthMm), "height_mm": JSON(l.heightMm), "style": JSON(style),
        ], select: true)
    }

    /// Add a text holding a binding (`BoundText`). `{{serial}}` makes the `serial` counter
    /// first when the document has none.
    @discardableResult
    public func addBoundText(_ text: String) -> Task<Void, Never> {
        let add = ("element.add_text", frame(40, 8).with("text", JSON(text)).with("fit", "shrink"))
        let needsSerial = text.contains(BoundText.serial) && !serials.contains { $0.name == "serial" }
            && data?.columns.contains("serial") != true
        return batch((needsSerial ? [("data.set_serial", JSON.object([:]))] : []) + [add], select: true)
    }

    /// Add a text, barcode or 2D code bound to `column` (a data column, serial or variable).
    @discardableResult
    public func addBound(column: String, type: String = "text") -> Task<Void, Never> {
        guard let l = doc?.label else { return Task {} }
        let binding = JSON("{{\(column)}}")
        switch type {
        case "barcode": return exec("element.add_barcode", centred(l.widthMm * 0.7, l.heightMm * 0.4).with("data", binding), select: true)
        case "code2d":
            let s = min(l.heightMm * 0.6, l.widthMm * 0.6)
            return exec("element.add_code2d", centred(s, s).with("data", binding), select: true)
        default: return exec("element.add_text", frame(40, 8).with("text", binding).with("fit", "shrink"), select: true)
        }
    }

    /// Read the barcode or 2D code in a picked photo (`code.decode`) and add it as an element.
    /// Returns false when the photo holds no code the engine reads.
    public func addCode(fromImage data: Data) async -> Bool {
        guard let l = doc?.label else { return false }
        var found: JSON?
        await app.report {
            let p = try await Task.detached { try ImageImport.prepare(data) }.value
            let r = try await engine.call("code.decode", ["data_base64": JSON(p.png.base64EncodedString())])
            found = r["codes"]?.objects.first
        }
        guard let code = found, let text = code["text"]?.string else { return false }
        let two = code["kind"]?.string == "code2d"
        let s = min(l.heightMm * 0.6, l.widthMm * 0.6)
        let frame = two ? centred(s, s) : centred(l.widthMm * 0.7, l.heightMm * 0.4)
        await exec(
            two ? "element.add_code2d" : "element.add_barcode",
            frame.with("data", JSON(text)).with("symbology", code["symbology"]),
            select: true
        ).value
        return true
    }

    // MARK: - Tables

    /// One edit of the table `id` (`element.table_edit`); `op` is tagged by its `op` field.
    @discardableResult
    public func tableEdit(_ id: Int64, _ op: JSON) -> Task<Void, Never> {
        exec("element.table_edit", ["id": .int(id), "edit": op])
    }

    // MARK: - Serials and variables

    /// Make or change a serial counter; it is used as `{{name}}`.
    @discardableResult
    public func setSerial(_ s: Serial) -> Task<Void, Never> {
        exec("data.set_serial", [
            "name": JSON(s.name), "start": JSON(s.start), "step": JSON(s.step), "digits": JSON(min(max(s.digits, 0), 18)),
            "prefix": JSON(s.prefix), "suffix": JSON(s.suffix),
        ])
    }

    @discardableResult
    public func removeSerial(_ name: String) -> Task<Void, Never> {
        exec("data.remove_serial", ["name": JSON(name)])
    }

    /// A name for a new serial that is not taken: `serial`, `serial2`, …
    public var freeSerialName: String {
        var i = 1
        while true {
            let name = i == 1 ? "serial" : "serial\(i)"
            if !serials.contains(where: { $0.name == name }) { return name }
            i += 1
        }
    }

    @discardableResult
    public func setVariable(_ name: String, _ value: String) -> Task<Void, Never> {
        exec("variable.set", ["name": JSON(name), "value": JSON(value)])
    }

    // MARK: - Data files

    /// The sheets of the attached spreadsheet, the one read, and whether the first row holds
    /// the column names; none when the data was not attached from a file this session.
    public var dataSheets: [String] { dataFile?.sheets ?? [] }
    public var dataSheet: String? { dataFile?.sheet }
    public var dataHeader: Bool? { dataFile?.header }

    /// Read the attached file again with another sheet or header setting.
    @discardableResult
    public func reattachData(sheet: String? = nil, header: Bool? = nil) -> Task<Void, Never> {
        guard var f = dataFile else { return Task {} }
        if let sheet { f.sheet = sheet }
        if let header { f.header = header }
        return attach(f)
    }

    func attach(_ f: DataFile) -> Task<Void, Never> {
        Task {
            await app.report {
                var file = f
                switch f.kind {
                case .csv(let text):
                    try await engine.call("data.attach_csv", ["doc_id": JSON(docId), "text": JSON(text), "header": JSON(f.header)])
                case .sheet(let url):
                    let r = try await engine.call("data.attach_xlsx", [
                        "doc_id": JSON(docId), "path": JSON(url.path), "sheet": JSON(f.sheet), "header": JSON(f.header),
                    ])
                    file.sheets = (r["sheets"]?.array ?? []).compactMap(\.string)
                    file.sheet = r["sheet"]?.string ?? f.sheet
                }
                dataFile = file
                saved = false
                try await refresh()
                if data != nil, data?.origin == nil { dataName = f.name }
                try await save()
            }
        }
    }

    /// One record per picked image (`data.attach_images`): the images go into the `image`
    /// column and an image element bound to it is placed. The files are copied into the data
    /// directory for the engine to read and removed again; `urls` may be security-scoped.
    @discardableResult
    public func attachImages(_ urls: [URL]) -> Task<Void, Never> {
        Task {
            let fm = FileManager.default
            let dir = engine.dataDir.appendingPathComponent("imports", isDirectory: true)
            var paths: [JSON] = []
            await app.report {
                try? fm.removeItem(at: dir)
                try fm.createDirectory(at: dir, withIntermediateDirectories: true)
                for (i, url) in urls.prefix(500).enumerated() {
                    let scoped = url.startAccessingSecurityScopedResource()
                    defer { if scoped { url.stopAccessingSecurityScopedResource() } }
                    // Numbered, so two files of the same name both arrive.
                    let copy = dir.appendingPathComponent("\(i + 1)-\(url.lastPathComponent)")
                    try fm.copyItem(at: url, to: copy)
                    paths.append(JSON(copy.path))
                }
            }
            if !paths.isEmpty {
                dataFile = nil
                await exec("data.attach_images", ["paths": .array(paths)], select: true).value
            }
            try? fm.removeItem(at: dir)
        }
    }

    /// Remove what the editor left in the data directory.
    func cleanUp() {
        if case .sheet(let url)? = dataFile?.kind { try? FileManager.default.removeItem(at: url.deletingLastPathComponent()) }
        dataFile = nil
    }
}
