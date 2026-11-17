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
    public private(set) var preview: CGImage?
    public var selectedId: Int64?
    public private(set) var issues: [CheckIssue]?
    public private(set) var progress: PrintProgress?
    /// False while an edit is not yet on disk. Edits are saved as they happen.
    public private(set) var saved = true

    @ObservationIgnored private var printTask: Task<Void, Never>?

    private var engine: Engine { app.engine }

    init(app: AppState, docId: String) {
        self.app = app
        self.docId = docId
    }

    public var selected: Element? {
        guard let id = selectedId else { return nil }
        return doc?.elements.first { $0.id == id }
    }

    func refresh() async throws {
        let d = try Doc.from(try await engine.call("doc.get", ["doc_id": JSON(docId)]))
        preview = try await engine.render(dpi: Self.previewDpi, docId: docId)
        doc = d
        if let id = selectedId, !d.elements.contains(where: { $0.id == id }) { selectedId = nil }
    }

    /// Run a document command; with `select`, the element it creates becomes the selection.
    @discardableResult
    public func exec(_ id: String, _ params: JSON = [:], select: Bool = false) -> Task<Void, Never> {
        Task {
            await app.report {
                let r = try await engine.call(id, params.with("doc_id", JSON(docId)))
                if select, let new = r["id"]?.int64 { selectedId = new }
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
    private func frame(_ w: Double, _ h: Double) -> JSON {
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

    @discardableResult
    public func saveTemplate(_ name: String) -> Task<Void, Never> {
        Task {
            await app.report {
                try await engine.call("template.save", ["doc_id": JSON(docId), "name": JSON(name), "overwrite": true])
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

    @discardableResult
    public func print(copies: Int, density: Int?) -> Task<Void, Never>? {
        guard let printer = app.printer else { return nil }
        printTask?.cancel()
        progress = PrintProgress(done: 0, total: copies, state: "queued", error: nil)
        let task = Task {
            await app.report {
                do {
                    let r = try await engine.call("print.submit", [
                        "doc_id": JSON(docId), "address": JSON(printer.address), "copies": JSON(copies),
                        "density": JSON(density), "wait": false,
                    ])
                    guard let jobId = r["job_id"]?.idString else { return }
                    // The engine pushes each change (ADR-0031); stop at the final state.
                    for await j in engine.job(jobId) {
                        let p = PrintProgress(
                            done: j["pages_done"]?.int ?? 0,
                            total: j["pages_total"]?.int ?? copies,
                            state: j["state"]?.string ?? "",
                            error: j["error"]?["message"]?.string
                        )
                        progress = p
                        if p.finished { break }
                    }
                } catch {
                    progress = PrintProgress(done: 0, total: copies, state: "failed", error: error.localizedDescription)
                    throw error
                }
                await app.refreshHistory()
                await app.refreshStatus()
            }
        }
        printTask = task
        return task
    }

    public func clearProgress() {
        if progress?.finished == true { progress = nil }
    }
}
