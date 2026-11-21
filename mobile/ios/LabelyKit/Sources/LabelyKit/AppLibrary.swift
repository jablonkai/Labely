import CoreGraphics
import Foundation

/// A connect that failed, kept for the tips and Try again.
public struct ConnectFailure: Sendable, Equatable {
    public let address: String
    public let name: String
    public let message: String
}

/// A pictogram of the engine (`icon.list`).
public struct IconInfo: Sendable, Equatable, Identifiable {
    public let name: String
    public let category: String

    public var id: String { name }
}

public enum PrinterAddress {
    /// What the user typed as a printer address: a full address (`ble:`, `tcp:`, `sim:` …) is
    /// kept; a bare host, with or without a port, is a network printer. `nil` when empty.
    public static func normalise(_ text: String) -> String? {
        let t = text.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !t.isEmpty, !t.contains(" ") else { return nil }
        let schemes = ["ble:", "tcp:", "usb:", "sim:", "fake:"]
        if schemes.contains(where: { t.lowercased().hasPrefix($0) }) {
            return t.count > 4 ? t : nil
        }
        return "tcp:" + t
    }
}

/// The libraries and lists the screens show besides the open document: printer models, papers,
/// templates and their folders, recent documents, the print queue and history, fonts and icons.
extension AppState {
    // MARK: - Printer models

    /// The model `id`, if the engine knows it.
    public func model(_ id: String) -> Model? {
        if let m = printer?.model, m.id == id { return m }
        return models.first { $0.id == id }
    }

    /// Load every printer model the engine knows (`device.models`).
    public func loadModels() async {
        guard let list = try? await engine.call("device.models")["models"]?.array else { return }
        let all = list.compactMap(Model.from).filter { !$0.id.isEmpty }
        if !all.isEmpty { models = all }
    }

    func rememberModel(_ m: Model) {
        guard !m.id.isEmpty, models.first(where: { $0.id == m.id }) != m else { return }
        models = models.filter { $0.id != m.id } + [m]
        save(models, "models")
    }

    /// Connect to an address the user typed (`PrinterAddress.normalise`).
    @discardableResult
    public func connectManual(_ text: String) -> Task<Void, Never> {
        guard let address = PrinterAddress.normalise(text) else { return Task {} }
        return connect(address, name: "")
    }

    /// Try the failed connect again.
    @discardableResult
    public func retryConnect() -> Task<Void, Never> {
        guard let f = connectFailure else { return Task {} }
        return connect(f.address, name: f.name)
    }

    public func dismissConnectFailure() {
        connectFailure = nil
    }

    /// "Labels from data": a new label of the first preset size with the picked data file
    /// attached. `url` may be security-scoped.
    @discardableResult
    public func newLabelFromData(_ url: URL) -> Task<Void, Never> {
        Task {
            await newLabel(widthMm: Self.presetSizes[0].0, heightMm: Self.presetSizes[0].1).value
            await editor?.attachFile(url).value
        }
    }

    // MARK: - Recent documents

    /// Rename the document of a recent entry (the name is stored in the file).
    @discardableResult
    public func renameRecent(_ r: Recent, to name: String) -> Task<Void, Never> {
        let name = name.trimmingCharacters(in: .whitespaces)
        guard !name.isEmpty, name != r.name else { return Task {} }
        return Task {
            await report {
                guard let docId = try await engine.call("doc.open", ["path": JSON(r.path)])["doc_id"]?.string else { return }
                do {
                    try await engine.call("doc.set_label", ["doc_id": JSON(docId), "name": JSON(name)])
                    try await engine.call("doc.save", ["doc_id": JSON(docId), "path": JSON(r.path)])
                } catch {
                    _ = try? await engine.call("doc.close", ["doc_id": JSON(docId)])
                    throw error
                }
                try await engine.call("doc.close", ["doc_id": JSON(docId)])
                let renamed = Recent(path: r.path, name: name, widthMm: r.widthMm, heightMm: r.heightMm)
                recents = recents.map { $0.path == r.path ? renamed : $0 }
                save(recents, "recents")
            }
        }
    }

    /// Copy the document of a recent entry as "`name`", next to the original.
    @discardableResult
    public func duplicateRecent(_ r: Recent, name: String) -> Task<Void, Never> {
        Task {
            await report {
                let target = newDocURL()
                try FileManager.default.copyItem(atPath: r.path, toPath: target.path)
                guard let docId = try await engine.call("doc.open", ["path": JSON(target.path)])["doc_id"]?.string else { return }
                do {
                    try await engine.call("doc.set_label", ["doc_id": JSON(docId), "name": JSON(name)])
                    try await engine.call("doc.save", ["doc_id": JSON(docId), "path": JSON(target.path)])
                } catch {
                    _ = try? await engine.call("doc.close", ["doc_id": JSON(docId)])
                    try? FileManager.default.removeItem(at: target)
                    throw error
                }
                try await engine.call("doc.close", ["doc_id": JSON(docId)])
                remember(Recent(path: target.path, name: name, widthMm: r.widthMm, heightMm: r.heightMm), preview: thumbs[r.path])
            }
        }
    }

    /// Delete the document of a recent entry: the file in the app's storage goes too.
    public func deleteRecent(_ r: Recent) {
        try? FileManager.default.removeItem(atPath: r.path)
        recents = recents.filter { $0.path != r.path }
        thumbs[r.path] = nil
        save(recents, "recents")
    }

    // MARK: - Templates

    /// Whether the library has a template `name` in `folder`.
    public func templateExists(name: String, folder: String) -> Bool {
        templates.contains { !$0.starter && $0.folder == folder && $0.name.compare(name, options: .caseInsensitive) == .orderedSame }
    }

    private func templateCommand(_ id: String, _ params: JSON, forget ref: String? = nil) -> Task<Void, Never> {
        Task {
            await report {
                try await engine.call(id, params)
                if let ref { thumbs[ref] = nil }
                await loadTemplates()
            }
        }
    }

    /// Create a library folder.
    @discardableResult
    public func createFolder(_ folder: String) -> Task<Void, Never> {
        templateCommand("template.folder", ["op": "create", "folder": JSON(folder.trimmingCharacters(in: .whitespaces))])
    }

    @discardableResult
    public func renameFolder(_ folder: String, to name: String) -> Task<Void, Never> {
        templateCommand("template.folder", ["op": "rename", "folder": JSON(folder), "to": JSON(name.trimmingCharacters(in: .whitespaces))])
    }

    /// Delete a library folder with the templates in it.
    @discardableResult
    public func deleteFolder(_ folder: String) -> Task<Void, Never> {
        templateCommand("template.folder", ["op": "delete", "folder": JSON(folder), "with_templates": true])
    }

    /// Copy a template (a starter too) into the library.
    @discardableResult
    public func duplicateTemplate(_ t: Template, name: String? = nil, folder: String? = nil) -> Task<Void, Never> {
        templateCommand("template.duplicate", ["ref": JSON(t.ref), "name": JSON(name), "folder": JSON(folder)])
    }

    /// Rename a library template or move it to another folder (`""` is the default folder).
    @discardableResult
    public func moveTemplate(_ t: Template, name: String? = nil, folder: String? = nil) -> Task<Void, Never> {
        templateCommand("template.move", ["ref": JSON(t.ref), "name": JSON(name), "folder": JSON(folder)], forget: t.ref)
    }

    // MARK: - Papers

    /// List the paper library: `query` searches id, name, size and RFID barcodes; with
    /// `fitting`, only papers the current printer model can print.
    public func loadPapers(query: String = "", fitting: Bool = false) async {
        await report {
            let q = query.trimmingCharacters(in: .whitespaces)
            papers = Paper.list(try await engine.call("paper.list", [
                "query": q.isEmpty ? nil : JSON(q), "model": fitting ? JSON(modelId) : nil,
            ]))
        }
    }

    /// Add a paper of the user's own; with `barcode`, the loaded roll is linked to it.
    @discardableResult
    public func addPaper(name: String, widthMm: Double, heightMm: Double, cornerRadiusMm: Double = 0, media: String = "gap", barcode: String? = nil) -> Task<Void, Never> {
        paperCommand("paper.add", [
            "name": JSON(name.trimmingCharacters(in: .whitespaces)), "width_mm": JSON(widthMm), "height_mm": JSON(heightMm),
            "corner_radius_mm": JSON(cornerRadiusMm), "media": JSON(media), "barcode": JSON(barcode),
        ])
    }

    @discardableResult
    public func deletePaper(_ p: Paper) -> Task<Void, Never> {
        paperCommand("paper.delete", ["id": JSON(p.id)])
    }

    /// Remember that the roll with `barcode` is paper `p`.
    @discardableResult
    public func linkPaper(barcode: String, to p: Paper) -> Task<Void, Never> {
        paperCommand("paper.link", ["barcode": JSON(barcode), "id": JSON(p.id)])
    }

    @discardableResult
    public func unlinkPaper(barcode: String) -> Task<Void, Never> {
        paperCommand("paper.unlink", ["barcode": JSON(barcode)])
    }

    private func paperCommand(_ id: String, _ params: JSON) -> Task<Void, Never> {
        Task {
            await report {
                try await engine.call(id, params)
                papers = Paper.list(try await engine.call("paper.list"))
                if printer != nil { await detectPaper() }
                await editor?.refreshLoadedPaper()
            }
        }
    }

    /// Read the roll in the connected printer (`paper.detect`), without a document to compare.
    @discardableResult
    public func detectPaper() async -> PaperDetect? {
        guard let p = printer else {
            detected = nil
            return nil
        }
        // Without `doc_id` the engine compares with its active document; that is not asked for
        // here, so only the roll itself is kept.
        guard let r = try? await engine.call("paper.detect", ["address": JSON(p.address)]) else {
            detected = nil
            return nil
        }
        detected = PaperDetect.from(r)
        return detected
    }

    // MARK: - Print queue

    public func refreshJobs() async {
        guard let r = try? await engine.call("job.list") else { return }
        jobs = (r["jobs"]?.objects ?? []).compactMap(PrintJob.from)
        queuePaused = r["paused"]?.bool ?? false
    }

    private func jobCommand(_ id: String, _ params: JSON = [:]) -> Task<Void, Never> {
        Task {
            await report { try await engine.call(id, params) }
            await refreshJobs()
            await refreshHistory()
        }
    }

    /// Drop a queued job, or stop the running one at the next label.
    @discardableResult
    public func cancelJob(_ id: String) -> Task<Void, Never> { jobCommand("job.cancel", ["id": JSON(id)]) }

    /// A paused queue finishes the running job and starts no new one.
    @discardableResult
    public func pauseQueue() -> Task<Void, Never> { jobCommand("job.pause") }

    @discardableResult
    public func resumeQueue() -> Task<Void, Never> { jobCommand("job.resume") }

    /// Remove the finished jobs from the queue view.
    @discardableResult
    public func clearJobs() -> Task<Void, Never> { jobCommand("job.clear") }

    // MARK: - History

    @discardableResult
    public func deleteHistory(_ h: HistoryEntry) -> Task<Void, Never> {
        jobCommand("job.history_delete", ["ids": [JSON(h.id)]])
    }

    @discardableResult
    public func clearHistory() -> Task<Void, Never> { jobCommand("job.history_clear") }

    /// Open the printed document of `h` to print it again, or to continue a part-finished job:
    /// the editor opens its print sheet (`reprint`).
    @discardableResult
    public func printAgain(_ h: HistoryEntry) -> Task<Void, Never> {
        Task {
            await openHistory(h).value
            if editor != nil { reprint = h }
        }
    }

    // MARK: - Fonts

    /// Where added font files are kept; they are loaded again at every start.
    var fontsDir: URL { engine.dataDir.appendingPathComponent("fonts", isDirectory: true) }

    /// Load the added fonts and list the families.
    public func loadFonts() async {
        let files = (try? FileManager.default.contentsOfDirectory(at: fontsDir, includingPropertiesForKeys: nil)) ?? []
        for f in files.sorted(by: { $0.path < $1.path }) {
            _ = try? await engine.call("font.add", ["path": JSON(f.path)])
        }
        if let r = try? await engine.call("font.list") {
            fonts = (r["fonts"]?.array ?? []).compactMap(\.string)
        }
    }

    /// Add a font file (TTF, OTF, TTC) the user picked; returns the families it added. `url`
    /// may be security-scoped.
    public func addFont(_ url: URL) async -> [String] {
        var added: [String] = []
        await report {
            let scoped = url.startAccessingSecurityScopedResource()
            defer { if scoped { url.stopAccessingSecurityScopedResource() } }
            try FileManager.default.createDirectory(at: fontsDir, withIntermediateDirectories: true)
            let copy = fontsDir.appendingPathComponent(url.lastPathComponent)
            try? FileManager.default.removeItem(at: copy)
            try FileManager.default.copyItem(at: url, to: copy)
            do {
                let r = try await engine.call("font.add", ["path": JSON(copy.path)])
                added = (r["added"]?.array ?? []).compactMap(\.string)
                fonts = (r["fonts"]?.array ?? []).compactMap(\.string)
            } catch {
                try? FileManager.default.removeItem(at: copy)
                throw error
            }
        }
        return added
    }

    // MARK: - Icons

    /// Side of an icon thumbnail cell: 6 mm at 254 dpi is 60 dots.
    private static let iconCellMm = 6.0
    private static let iconDpi = 254
    private static let iconColumns = 8

    /// List the engine's pictograms (once).
    public func loadIcons() async {
        guard icons.isEmpty else { return }
        await report {
            icons = (try await engine.call("icon.list")["icons"]?.objects ?? []).compactMap { i in
                i["name"]?.string.map { IconInfo(name: $0, category: i["category"]?.string ?? "") }
            }
        }
    }

    /// Render the thumbnails of the icons of `category` that have none yet: they are laid out on
    /// a scratch label, rendered once and cut apart.
    public func loadIconThumbs(category: String) async {
        let todo = icons.filter { $0.category == category && iconThumbs[$0.name] == nil }
        guard !todo.isEmpty else { return }
        let cell = Self.iconCellMm
        let cols = Self.iconColumns
        // `command.batch` takes at most 128 steps.
        for chunk in stride(from: 0, to: todo.count, by: 96).map({ Array(todo[$0..<min($0 + 96, todo.count)]) }) {
            let rows = (chunk.count + cols - 1) / cols
            guard let docId = try? await engine.call("doc.new", ["width_mm": JSON(cell * Double(cols)), "height_mm": JSON(cell * Double(rows))])["doc_id"]?.string else { return }
            let steps: [JSON] = chunk.enumerated().map { i, icon in
                [
                    "id": "element.add_icon",
                    "params": [
                        "doc_id": JSON(docId), "icon": JSON(icon.name),
                        "x_mm": JSON(Double(i % cols) * cell + 0.5), "y_mm": JSON(Double(i / cols) * cell + 0.5),
                        "width_mm": JSON(cell - 1), "height_mm": JSON(cell - 1),
                    ],
                ]
            }
            _ = try? await engine.call("command.batch", ["steps": .array(steps), "terse": true])
            if let sheet = try? await engine.render(dpi: Self.iconDpi, docId: docId) {
                let px = Int((cell * Double(Self.iconDpi) / 25.4).rounded())
                for (i, icon) in chunk.enumerated() {
                    if let tile = sheet.cropping(to: CGRect(x: (i % cols) * px, y: (i / cols) * px, width: px, height: px)) {
                        iconThumbs[icon.name] = tile
                    }
                }
            }
            _ = try? await engine.call("doc.close", ["doc_id": JSON(docId)])
        }
    }
}
