import CoreGraphics
import Foundation
import Observation

/// Print progress of the editor's document.
public struct PrintProgress: Sendable, Equatable {
    public let done: Int
    public let total: Int
    public let state: String
    public let error: String?

    public var finished: Bool { Engine.finished.contains(state) }
}

/// What a print job prints and how (`print.submit`).
public struct PrintOptions: Sendable, Equatable {
    public var copies = 1
    /// `nil` is the label's density, else the model's default.
    public var density: Int?
    /// With data: `all`, one record or a list like `1,3,5-7`.
    public var records: String?
    /// Without data: how many labels to print; serials advance on each.
    public var count: Int?
    /// A column holding each record's copy count; it replaces `copies`.
    public var copiesColumn: String?
    /// Move the print right and down, in millimetres (−10 … 10).
    public var offsetXMm = 0.0
    public var offsetYMm = 0.0
    public var mirror = false
    public var rotate180 = false

    public init() {}
}

/// One open document, as `EditorState` on Android. Every change is an engine command with this
/// document's `doc_id`; after it the document and its print raster are read back, so what the
/// canvas shows is what prints.
@MainActor
@Observable
public final class EditorState: Identifiable {
    /// The B21 Pro's own resolution, so the canvas shows the exact raster.
    public static let previewDpi = 203

    public let app: AppState
    public let docId: String
    public nonisolated var id: String { docId }

    public private(set) var doc: Doc?
    /// The print raster of the previewed record.
    public internal(set) var preview: CGImage?
    /// The attached data source, if any (`EditorData.swift`).
    public internal(set) var data: DataInfo?
    /// The previewed record, 1-based.
    public internal(set) var record = 1
    /// The rows loaded so far, from record 1; `loadMoreRows` pages the rest in.
    public internal(set) var rows: [[String]] = []
    /// File name of CSV attached as text this session: the engine keeps no origin for it.
    var dataName: String?
    /// The file the data was attached from this session (`EditorInsert.swift`).
    var dataFile: DataFile?
    /// The print job being followed, to cancel it.
    public internal(set) var jobId: String?
    /// The roll barcode the paper was applied for by itself, so it happens once per roll.
    @ObservationIgnored var autoApplied: String?
    /// True while an export is being written.
    public internal(set) var exporting = false
    /// The selected elements, in the order they were picked.
    public var selection: [Int64] = []
    /// The one selected element; `nil` with none or several. Setting it replaces the selection.
    public var selectedId: Int64? {
        get { selection.count == 1 ? selection[0] : nil }
        set { selection = newValue.map { [$0] } ?? [] }
    }
    /// Taps on the canvas add to the selection instead of replacing it.
    public var multiSelect = false
    /// The serial counters of the document (`EditorInsert.swift`).
    public internal(set) var serials: [Serial] = []
    /// The roll in the printer compared with this label, once read (`EditorPaper.swift`).
    public internal(set) var loadedPaper: PaperDetect?
    public private(set) var issues: [CheckIssue]?
    public private(set) var progress: PrintProgress?
    /// False while an edit is not yet on disk. Edits are saved as they happen.
    public internal(set) var saved = true

    @ObservationIgnored private var printTask: Task<Void, Never>?

    var engine: Engine { app.engine }

    init(app: AppState, docId: String) {
        self.app = app
        self.docId = docId
    }

    public var selected: Element? {
        guard let id = selectedId else { return nil }
        return doc?.elements.first { $0.id == id }
    }

    /// The selected elements, bottom-first.
    public var selectedElements: [Element] {
        guard let doc, !selection.isEmpty else { return [] }
        return doc.elements.filter { selection.contains($0.id) }
    }

    func refresh() async throws {
        let d = try Doc.from(try await engine.call("doc.get", ["doc_id": JSON(docId)]))
        try await refreshData()
        preview = try await render()
        doc = d
        let kept = selection.filter { id in d.elements.contains { $0.id == id } }
        if kept != selection { selection = kept }
        if selection.count < 2 && d.elements.count < 2 { multiSelect = false }
    }

    /// The print raster: of the previewed record when data is attached, else of the label.
    func render() async throws -> CGImage {
        if let data, data.rowCount > 0 {
            return try await engine.preview(dpi: Self.previewDpi, docId: docId, record: record)
        }
        return try await engine.render(dpi: Self.previewDpi, docId: docId)
    }

    /// Run a document command; with `select`, the element it creates becomes the selection.
    /// `command.batch` and library commands take no `doc_id`: pass `addDoc: false`.
    @discardableResult
    public func exec(_ id: String, _ params: JSON = [:], select: Bool = false, addDoc: Bool = true) -> Task<Void, Never> {
        Task {
            await app.report {
                let r = try await engine.call(id, addDoc ? params.with("doc_id", JSON(docId)) : params)
                if select {
                    if let new = r["id"]?.int64 ?? r["element_id"]?.int64 {
                        selectedId = new
                    } else if let ids = (r["ids"] ?? r["created"])?.array?.compactMap(\.int64), !ids.isEmpty {
                        selection = ids
                    }
                }
                saved = false
                try await refresh()
                try await save()
            }
        }
    }

    @discardableResult
    public func setProps(_ id: Int64, _ props: JSON) -> Task<Void, Never> {
        exec("element.set_props", ["id": .int(id), "props": props])
    }

    @discardableResult public func undo() -> Task<Void, Never> { exec("history.undo") }
    @discardableResult public func redo() -> Task<Void, Never> { exec("history.redo") }

    // MARK: - Adding elements

    /// A frame centred on the label, at most `w` × `h` mm.
    func frame(_ w: Double, _ h: Double) -> JSON {
        guard let l = doc?.label else {
            return ["x_mm": 0, "y_mm": 0, "width_mm": JSON(w), "height_mm": JSON(h)]
        }
        let fw = min(w, l.widthMm - 2)
        let fh = min(h, l.heightMm - 2)
        return [
            "x_mm": JSON((l.widthMm - fw) / 2), "y_mm": JSON((l.heightMm - fh) / 2),
            "width_mm": JSON(fw), "height_mm": JSON(fh),
        ]
    }

    @discardableResult
    public func addText(_ text: String) -> Task<Void, Never> {
        exec("element.add_text", frame(40, 8).with("text", JSON(text)).with("fit", "shrink"), select: true)
    }

    @discardableResult
    public func addBarcode() -> Task<Void, Never> {
        exec("element.add_barcode", frame(36, 12).with("data", "123456789012"), select: true)
    }

    @discardableResult
    public func addQr() -> Task<Void, Never> {
        exec("element.add_code2d", frame(16, 16).with("data", "https://labely.org"), select: true)
    }

    @discardableResult
    public func addRect() -> Task<Void, Never> {
        exec("element.add_rect", frame(20, 12).with("stroke_mm", 0.4), select: true)
    }

    @discardableResult
    public func addEllipse() -> Task<Void, Never> {
        exec("element.add_ellipse", frame(16, 12).with("stroke_mm", 0.4), select: true)
    }

    @discardableResult
    public func addLine() -> Task<Void, Never> {
        exec("element.add_line", frame(30, 0).with("height_mm", 0.0).with("stroke_mm", 0.4), select: true)
    }

    // MARK: - Label

    @discardableResult
    public func setLabel(widthMm: Double, heightMm: Double, media: String) -> Task<Void, Never> {
        exec("doc.set_label", ["width_mm": JSON(widthMm), "height_mm": JSON(heightMm), "media": JSON(media)])
    }

    /// Change label properties (`doc.set_label`): `name` renames the document,
    /// `corner_radius_mm`, `printer_model` (empty clears it) and `density` (0 is the model's).
    @discardableResult
    public func setLabelProps(_ props: JSON) -> Task<Void, Never> {
        exec("doc.set_label", props)
    }

    /// Save the label as a library template in `folder` (`""` is the default folder). A
    /// template of the same name there is replaced: ask first (`AppState.templateExists`).
    @discardableResult
    public func saveTemplate(_ name: String, folder: String = "") -> Task<Void, Never> {
        Task {
            await app.report {
                let r = try await engine.call("template.save", [
                    "doc_id": JSON(docId), "name": JSON(name), "folder": folder.isEmpty ? nil : JSON(folder), "overwrite": true,
                ])
                if let ref = r["ref"]?.string { app.thumbs[ref] = nil }
                await app.loadTemplates()
            }
        }
    }

    /// Save to the document's own path, or a new file in app storage.
    func save() async throws {
        guard var d = doc else { return }
        let path = d.path ?? app.newDocURL().path
        try await engine.call("doc.save", ["doc_id": JSON(docId), "path": JSON(path)])
        if d.path == nil {
            d.path = path
            doc = d
        }
        saved = true
        app.remember(Recent(path: path, name: d.name, widthMm: d.label.widthMm, heightMm: d.label.heightMm), preview: preview)
    }

    // MARK: - Printing

    @discardableResult
    public func check() -> Task<Void, Never> {
        issues = nil
        return Task {
            await app.report {
                issues = CheckIssue.list(try await engine.call("doc.check", ["doc_id": JSON(docId), "address": JSON(app.printer?.address)]))
            }
        }
    }

    /// Print the previewed record, or with `allRecords` every record of the attached data.
    /// Without data the label prints as it is.
    @discardableResult
    public func print(copies: Int, density: Int?, allRecords: Bool = false) -> Task<Void, Never>? {
        var o = PrintOptions()
        o.copies = copies
        o.density = density
        o.records = recordsParam(all: allRecords)
        return print(o)
    }

    /// Labels `o` prints, when that is known before the job starts: not with a copies column.
    public func labelCount(_ o: PrintOptions) -> Int? {
        if o.copiesColumn != nil { return nil }
        if let data, data.rowCount > 0 {
            return RecordRange.parse(o.records ?? "all", count: data.rowCount).map { $0.count * o.copies }
        }
        return max(o.count ?? 1, 1) * o.copies
    }

    /// Queue a print job on the connected printer and follow it.
    @discardableResult
    public func print(_ o: PrintOptions) -> Task<Void, Never>? {
        guard let printer = app.printer else { return nil }
        printTask?.cancel()
        let hasData = (data?.rowCount ?? 0) > 0
        let total = labelCount(o) ?? 0
        progress = PrintProgress(done: 0, total: total, state: "queued", error: nil)
        let task = Task {
            await app.report {
                do {
                    let r = try await engine.call("print.submit", [
                        "doc_id": JSON(docId), "address": JSON(printer.address), "copies": JSON(o.copies),
                        "density": JSON(o.density),
                        "records": hasData ? JSON(o.records ?? "all") : nil,
                        "count": hasData ? nil : JSON(o.count),
                        "copies_column": hasData ? JSON(o.copiesColumn) : nil,
                        "offset_x_mm": o.offsetXMm == 0 ? nil : JSON(o.offsetXMm),
                        "offset_y_mm": o.offsetYMm == 0 ? nil : JSON(o.offsetYMm),
                        "mirror": o.mirror ? true : nil, "rotate_180": o.rotate180 ? true : nil,
                        "wait": false,
                    ])
                    guard let jobId = r["job_id"]?.idString else { return }
                    self.jobId = jobId
                    // The engine pushes each change (ADR-0031); stop at the final state.
                    for await j in engine.job(jobId) {
                        let p = PrintProgress(
                            done: j["pages_done"]?.int ?? 0,
                            total: j["pages_total"]?.int ?? total,
                            state: j["state"]?.string ?? "",
                            error: j["error"]?["message"]?.string
                        )
                        progress = p
                        if p.finished { break }
                    }
                    self.jobId = nil
                } catch {
                    jobId = nil
                    progress = PrintProgress(done: 0, total: total, state: "failed", error: error.localizedDescription)
                    throw error
                }
                await app.refreshHistory()
                await app.refreshStatus()
            }
        }
        printTask = task
        return task
    }

    /// Stop the job being printed at the next label, or drop it while it is queued.
    @discardableResult
    public func cancelPrint() -> Task<Void, Never> {
        guard let jobId else { return Task {} }
        return app.cancelJob(jobId)
    }

    public func clearProgress() {
        if progress?.finished == true { progress = nil }
    }
}
