import Foundation

// The six assistant functions of ADR-0011 (ADR-0033), as `LabelyFunctions` on Android
// (ADR-0028). The app's App Intents are thin wrappers around them, so the logic runs under
// `swift test` against the real engine. Each function is a few commands on the app's own engine
// and state, so what an assistant makes shows up in the app.

/// A label saved in the label folder, ready to print.
public struct SavedLabel: Sendable, Equatable {
    /// The file name in the label folder. It stays valid after the app restarts.
    public let labelId: String
    public let name: String
    public let widthMm: Double
    public let heightMm: Double
    /// `doc.check` warnings and errors: problems that would spoil the print.
    public let warnings: [String]
}

/// A text, barcode or QR element a template field fills in.
public struct TemplateField: Sendable, Equatable {
    /// The element's name, else its sample text.
    public let name: String
    /// `text`, `barcode` or `qr`.
    public let kind: String
    public let value: String
}

public struct LabelTemplate: Sendable, Equatable {
    public let templateId: String
    public let name: String
    public let folder: String
    public let widthMm: Double
    public let heightMm: Double
    public let fields: [TemplateField]
}

public struct FieldValue: Sendable, Equatable {
    public let name: String
    public let value: String

    public init(name: String, value: String) {
        self.name = name
        self.value = value
    }

    /// `name=value`, as a Shortcuts list item gives it. The first `=` splits.
    public static func parse(_ line: String) -> FieldValue? {
        guard let eq = line.firstIndex(of: "=") else { return nil }
        let name = line[..<eq].trimmingCharacters(in: .whitespaces)
        guard !name.isEmpty else { return nil }
        return FieldValue(name: name, value: String(line[line.index(after: eq)...]))
    }
}

public struct PrinterInfo: Sendable, Equatable {
    public let printerId: String
    public let name: String
    public let connected: Bool
    /// Empty until the app has connected once.
    public let model: String
}

public struct PrinterReport: Sendable, Equatable {
    public let printerId: String
    public let name: String
    public let model: String
    public let batteryPercent: Int?
    /// `true` loaded, `false` empty, `nil` not reported.
    public let paperPresent: Bool?
}

public struct PrintResult: Sendable, Equatable {
    public let jobId: String
    public let state: String
    public let pagesPrinted: Int
    public let pagesTotal: Int
}

/// Why an assistant function failed. The app turns each case into a localised message; the
/// kit holds no strings (ADR-0032).
public enum AssistantError: Error, Equatable {
    case emptyText
    case sizeOutOfRange(widthMm: Double, heightMm: Double)
    case noTemplate(String)
    case noField(field: String, fields: [String])
    case noLabel(String)
    case noPrinter
    case badCopies(max: Int)
    /// The user has not turned on "Allow assistants to print".
    case printNotAllowed
    case printTimeout(jobId: String)
    case printFailed(state: String, reason: String)
    /// Bluetooth is off or not allowed.
    case bluetooth(String)
    /// The printer does not answer.
    case unreachable(String)
    /// Any other engine failure: its message and hint.
    case engine(code: String, message: String)
}

@MainActor
public final class Assistant {
    public static let defaultWidthMm = 50.0
    public static let defaultHeightMm = 30.0
    public static let maxCopies = 999
    private static let marginMm = 2.0
    private static let textPt = 28.0
    private static let nameChars = 40
    private static let scanMs = 5000
    private static let printTimeout: Duration = .seconds(5 * 60)

    public let app: AppState
    /// A template's name in the user's language (starters are translated by the app).
    private let templateName: (Template) -> String
    private let simulatorName: String

    private var engine: Engine { app.engine }

    public init(app: AppState, simulatorName: String, templateName: @escaping (Template) -> String) {
        self.app = app
        self.simulatorName = simulatorName
        self.templateName = templateName
    }

    /// A label with `text` centred, shrinking to fit. A size of 0 (or `nil`) uses 50 × 30 mm.
    public func createLabelFromText(_ text: String, widthMm: Double? = nil, heightMm: Double? = nil) async throws -> SavedLabel {
        try await mapped {
            let trimmed = text.trimmingCharacters(in: .whitespacesAndNewlines)
            guard !trimmed.isEmpty else { throw AssistantError.emptyText }
            let w = widthMm.flatMap { $0 > 0 ? $0 : nil } ?? Self.defaultWidthMm
            let h = heightMm.flatMap { $0 > 0 ? $0 : nil } ?? Self.defaultHeightMm
            guard (10...120).contains(w), (5...120).contains(h) else {
                throw AssistantError.sizeOutOfRange(widthMm: w, heightMm: h)
            }
            let firstLine = trimmed.split(whereSeparator: \.isNewline).first.map(String.init) ?? trimmed
            let name = String(firstLine.trimmingCharacters(in: .whitespaces).prefix(Self.nameChars))
            let docId = try await docId(engine.call("doc.new", [
                "width_mm": JSON(w), "height_mm": JSON(h), "media": "gap",
                "printer_model": JSON(app.modelId), "name": JSON(name),
            ]))
            return try await withDoc(docId) {
                let m = min(Self.marginMm, min(w, h) / 10)
                try await self.engine.call("element.add_text", [
                    "doc_id": JSON(docId), "x_mm": JSON(m), "y_mm": JSON(m),
                    "width_mm": JSON(w - 2 * m), "height_mm": JSON(h - 2 * m),
                    "text": JSON(trimmed), "font_size_pt": JSON(Self.textPt), "fit": "shrink", "wrap": true,
                    "align": "center", "valign": "middle",
                ])
                return try await self.save(docId)
            }
        }
    }

    /// A label from template `templateId` with `fields` filled in; others keep their sample.
    public func createLabelFromTemplate(_ templateId: String, fields: [FieldValue] = []) async throws -> SavedLabel {
        try await mapped {
            guard let t = Template.list(try await engine.call("template.list")).first(where: { $0.ref == templateId }) else {
                throw AssistantError.noTemplate(templateId)
            }
            let docId = try await docId(engine.call("template.new", ["ref": JSON(templateId), "name": JSON(templateName(t))]))
            return try await withDoc(docId) {
                let elements = Self.fieldElements(try await self.doc(docId))
                for f in fields {
                    let targets = elements.filter { $0.name == f.name }
                    if targets.isEmpty {
                        throw AssistantError.noField(field: f.name, fields: Self.unique(elements.map(\.name)))
                    }
                    for (_, e) in targets {
                        try await self.engine.call("element.set_props", [
                            "doc_id": JSON(docId), "id": JSON(e.id), "props": [e.type == "text" ? "text" : "data": JSON(f.value)],
                        ])
                    }
                }
                return try await self.save(docId)
            }
        }
    }

    /// The starter and library templates matching `query` (all when empty), with their fields.
    public func listTemplates(query: String? = nil) async throws -> [LabelTemplate] {
        try await mapped {
            let q = query?.trimmingCharacters(in: .whitespaces)
            var out: [LabelTemplate] = []
            for t in Template.list(try await engine.call("template.list", ["query": JSON(q?.isEmpty == false ? q : nil)])) {
                let docId = try await docId(engine.call("template.new", ["ref": JSON(t.ref)]))
                let fields = try await withDoc(docId) { Self.fieldElements(try await self.doc(docId)) }
                var seen = Set<String>()
                out.append(LabelTemplate(
                    templateId: t.ref,
                    name: templateName(t),
                    folder: t.folder,
                    widthMm: t.widthMm,
                    heightMm: t.heightMm,
                    fields: fields.filter { seen.insert($0.name).inserted }.map { name, e in
                        TemplateField(name: name, kind: e.type, value: e.text ?? e.data ?? "")
                    }
                ))
            }
            return out
        }
    }

    /// The connected printer, the known ones, the simulator in debug builds and, with `scan`,
    /// the printers nearby. Connected first.
    public func listPrinters(scan: Bool = false) async throws -> [PrinterInfo] {
        try await mapped {
            var found: [PrinterInfo] = []
            func add(_ p: PrinterInfo) {
                if !found.contains(where: { $0.printerId == p.printerId }) { found.append(p) }
            }
            for d in (try await engine.call("device.list")["devices"]?.objects ?? []).compactMap(Device.from) where d.connected {
                add(Self.info(d))
            }
            for k in app.known { add(PrinterInfo(printerId: k.address, name: k.name, connected: false, model: "")) }
            if app.debug { add(PrinterInfo(printerId: AppState.simAddress, name: simulatorName, connected: false, model: "")) }
            if scan {
                let r = try await engine.call("device.scan", ["timeout_ms": JSON(Self.scanMs)])
                for d in (r["devices"]?.objects ?? []).compactMap(Device.from) where d.address.hasPrefix("ble:") {
                    add(Self.info(d))
                }
            }
            return found.filter(\.connected) + found.filter { !$0.connected }
        }
    }

    /// Battery and paper of `printerId`, connecting to it first if needed.
    public func printerStatus(_ printerId: String) async throws -> PrinterReport {
        try await mapped {
            let dev = try await app.connectNow(printerId)
            let s = DeviceStatus.from(try await engine.call("device.status", ["address": JSON(printerId)]))
            return PrinterReport(
                printerId: dev.address,
                name: dev.name,
                model: dev.model?.name ?? "",
                batteryPercent: s.batteryPercent,
                paperPresent: s.paperPresent
            )
        }
    }

    /// Print the saved label `labelId` and wait until it has printed. Needs the user's consent.
    /// No `printerId` uses the connected printer, else the only known one.
    public func printLabel(_ labelId: String, printerId: String? = nil, copies: Int = 1) async throws -> PrintResult {
        try await mapped {
            guard app.allowAgentPrint else { throw AssistantError.printNotAllowed }
            let n = copies == 0 ? 1 : copies
            guard (1...Self.maxCopies).contains(n) else { throw AssistantError.badCopies(max: Self.maxCopies) }
            let file = try labelFile(labelId)
            let chosen = printerId.flatMap { $0.isEmpty ? nil : $0 }
            guard let address = chosen ?? defaultPrinterId() else { throw AssistantError.noPrinter }
            _ = try await app.connectNow(address)
            let docId = try await docId(engine.call("doc.open", ["path": JSON(file.path)]))
            let job = try await withDoc(docId) {
                let r = try await self.engine.call("print.submit", [
                    "doc_id": JSON(docId), "address": JSON(address), "copies": JSON(n), "wait": false,
                ])
                guard let jobId = r["job_id"]?.idString else {
                    throw AssistantError.engine(code: "internal", message: "print.submit returned no job")
                }
                guard let job = await Self.timeout(Self.printTimeout, { await self.engine.finished(jobId) }) else {
                    throw AssistantError.printTimeout(jobId: jobId)
                }
                return job
            }
            await app.refreshHistory()
            await app.refreshStatus()
            let state = job["state"]?.string ?? ""
            guard state == "done" else {
                let why = job["error"]?["message"]?.string ?? ""
                throw AssistantError.printFailed(state: state, reason: why.isEmpty ? state : why)
            }
            return PrintResult(
                jobId: job["id"]?.idString ?? "",
                state: state,
                pagesPrinted: job["pages_done"]?.int ?? 0,
                pagesTotal: job["pages_total"]?.int ?? 0
            )
        }
    }

    /// The printer used when none is given: the connected one, else the only known one.
    public func defaultPrinterId() -> String? {
        app.printer?.address ?? (app.known.count == 1 ? app.known[0].address : nil)
    }

    // MARK: - Saved labels

    /// The saved labels, newest first, for the label picker.
    public func savedLabels() -> [SavedLabelFile] {
        let fm = FileManager.default
        let files = (try? fm.contentsOfDirectory(at: app.labelsDir, includingPropertiesForKeys: [.contentModificationDateKey])) ?? []
        let names = Dictionary(app.recents.map { (URL(fileURLWithPath: $0.path).lastPathComponent, $0) }, uniquingKeysWith: { a, _ in a })
        return files
            .filter { $0.pathExtension == "labely" }
            .map { url -> (SavedLabelFile, Date) in
                let id = url.lastPathComponent
                let r = names[id]
                let date = (try? url.resourceValues(forKeys: [.contentModificationDateKey]).contentModificationDate) ?? .distantPast
                return (SavedLabelFile(labelId: id, name: r?.name ?? url.deletingPathExtension().lastPathComponent,
                                       widthMm: r?.widthMm, heightMm: r?.heightMm), date)
            }
            .sorted { $0.1 > $1.1 }
            .map(\.0)
    }

    // MARK: - Helpers

    /// Run `body` on an open document and close it afterwards, whatever happens.
    private func withDoc<T>(_ docId: String, _ body: () async throws -> T) async throws -> T {
        do {
            let v = try await body()
            _ = try? await engine.call("doc.close", ["doc_id": JSON(docId)])
            return v
        } catch {
            _ = try? await engine.call("doc.close", ["doc_id": JSON(docId)])
            throw error
        }
    }

    private func docId(_ r: JSON) throws -> String {
        guard let id = r["doc_id"]?.string else {
            throw AssistantError.engine(code: "internal", message: "no doc_id in the result")
        }
        return id
    }

    private func doc(_ docId: String) async throws -> Doc {
        try Doc.from(try await engine.call("doc.get", ["doc_id": JSON(docId)]))
    }

    /// Save `docId` to the label folder, list it in the app's recents and check it.
    private func save(_ docId: String) async throws -> SavedLabel {
        let url = app.newDocURL()
        try await engine.call("doc.save", ["doc_id": JSON(docId), "path": JSON(url.path)])
        let d = try await doc(docId)
        let issues = CheckIssue.list(try await engine.call("doc.check", ["doc_id": JSON(docId)]))
        let thumb = try? await engine.render(dpi: AppState.thumbDpi, docId: docId)
        app.remember(Recent(path: url.path, name: d.name, widthMm: d.label.widthMm, heightMm: d.label.heightMm), preview: thumb)
        return SavedLabel(
            labelId: url.lastPathComponent,
            name: d.name,
            widthMm: d.label.widthMm,
            heightMm: d.label.heightMm,
            warnings: issues.filter { $0.severity != "info" }.map(\.message)
        )
    }

    /// The file `labelId` names; ids are bare file names in the label folder.
    private func labelFile(_ labelId: String) throws -> URL {
        let url = app.labelsDir.appendingPathComponent(labelId)
        var dir: ObjCBool = false
        guard !labelId.isEmpty, !labelId.contains("/"), !labelId.hasPrefix("."),
              FileManager.default.fileExists(atPath: url.path, isDirectory: &dir), !dir.boolValue
        else { throw AssistantError.noLabel(labelId) }
        return url
    }

    /// The elements a field can set, by field name: visible text, barcode and QR elements.
    static func fieldElements(_ doc: Doc) -> [(name: String, element: Element)] {
        doc.elements.compactMap { e in
            guard ["text", "barcode", "qr"].contains(e.type), !e.hidden else { return nil }
            let name = e.name ?? (e.type == "text" ? e.text : e.data) ?? ""
            return name.trimmingCharacters(in: .whitespaces).isEmpty ? nil : (name, e)
        }
    }

    private static func unique(_ names: [String]) -> [String] {
        var seen = Set<String>()
        return names.filter { seen.insert($0).inserted }
    }

    private static func info(_ d: Device) -> PrinterInfo {
        PrinterInfo(printerId: d.address, name: d.name, connected: d.connected, model: d.model?.name ?? "")
    }

    /// `body`'s value, or `nil` when it takes longer than `limit`.
    private static func timeout<T: Sendable>(_ limit: Duration, _ body: @escaping @Sendable () async -> T?) async -> T? {
        await withTaskGroup(of: T?.self) { group in
            group.addTask { await body() }
            group.addTask {
                try? await Task.sleep(for: limit)
                return nil
            }
            let first = await group.next() ?? nil
            group.cancelAll()
            return first
        }
    }

    /// Turn engine failures into the errors an assistant can act on.
    private func mapped<T>(_ body: () async throws -> T) async throws -> T {
        do {
            return try await body()
        } catch let e as AssistantError {
            throw e
        } catch let e as CommandFailure {
            let msg = [e.message.isEmpty ? e.code : e.message, e.hint].compactMap { $0 }.joined(separator: "; ")
            switch e.code {
            case "device_unavailable":
                if msg.localizedCaseInsensitiveContains("permission") || msg.contains("Bluetooth is off") {
                    throw AssistantError.bluetooth(msg)
                }
                throw AssistantError.unreachable(msg)
            default:
                throw AssistantError.engine(code: e.code, message: msg)
            }
        }
    }
}

/// A file in the label folder, for the App Intents label picker.
public struct SavedLabelFile: Sendable, Equatable {
    public let labelId: String
    public let name: String
    public let widthMm: Double?
    public let heightMm: Double?
}
