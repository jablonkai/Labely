import CoreGraphics
import Foundation
import Observation

public enum AppTab: Sendable, CaseIterable {
    case home, templates, history, printers, settings
}

/// A document the user opened or saved recently.
public struct Recent: Sendable, Equatable, Codable, Identifiable {
    public let path: String
    public let name: String
    public let widthMm: Double
    public let heightMm: Double

    public var id: String { path }

    public init(path: String, name: String, widthMm: Double, heightMm: Double) {
        self.path = path
        self.name = name
        self.widthMm = widthMm
        self.heightMm = heightMm
    }
}

/// A printer the user connected to before, remembered across launches.
public struct KnownPrinter: Sendable, Equatable, Codable, Identifiable {
    public let address: String
    public let name: String

    public var id: String { address }

    public init(address: String, name: String) {
        self.address = address
        self.name = name
    }
}

/// A one-line message for the toast. The shell localises the fixed ones.
public enum Notice: Sendable, Equatable {
    case text(String)
    case printerLost
    case calibrated
}

/// Everything the screens show, read from the engine, as `AppState` on Android (ADR-0027).
/// The SwiftUI views only read it and call its methods.
@MainActor
@Observable
public final class AppState {
    /// Label sizes offered on the home screen and in the new-label sheet, in millimetres.
    public static let presetSizes: [(Double, Double)] = [(50, 30), (40, 30), (30, 15), (50, 50), (40, 12), (70, 40)]
    /// The model new labels are made for until a printer is connected.
    public static let defaultModel = "niimbot.b21_pro"
    /// The B21 Pro simulator, offered in debug builds so the app works without a printer.
    public static let simAddress = "sim:niimbot.b21_pro"
    /// `busy` while scanning.
    public static let scanning = "scan"
    public static let thumbDpi = 203

    private static let scanMs = 5000
    private static let maxRecents = 12
    private static let maxKnown = 8

    public let engine: Engine
    /// Where labels are saved; App Intents address labels by their file name here (ADR-0033).
    public let labelsDir: URL
    public let debug: Bool
    /// Name of a new blank label, in the user's language.
    public var newLabelName = "Label"
    @ObservationIgnored private let defaults: UserDefaults

    public var tab: AppTab = .home
    public internal(set) var templates: [Template] = []
    public internal(set) var thumbs: [String: CGImage] = [:]
    public internal(set) var recents: [Recent] = []
    public internal(set) var known: [KnownPrinter] = []
    public internal(set) var history: [HistoryEntry] = []

    /// The connected printer and its last status.
    public internal(set) var printer: Device? {
        didSet {
            if printer?.address != oldValue?.address { printerSettings = nil }
            if let m = printer?.model { rememberModel(m) }
        }
    }
    public internal(set) var printerStatus: DeviceStatus?
    /// The connected printer's own settings, once `loadPrinterSettings` has read them.
    public internal(set) var printerSettings: PrinterSettings?

    /// Address being connected, or `scanning` while scanning.
    public internal(set) var busy: String?
    public internal(set) var scanResults: [Device]?

    public var message: Notice?

    public internal(set) var editor: EditorState?

    /// Whether App Intents may print (ADR-0033). Off until the user turns it on, like the
    /// studio's "Allow agents to print".
    public var allowAgentPrint = false {
        didSet { defaults.set(allowAgentPrint, forKey: "allow_agent_print") }
    }

    /// Units, grid, snapping and the paper behaviour, kept across launches.
    public var prefs = Prefs() {
        didSet {
            CurrentUnit.value = prefs.unit
            if prefs != oldValue { save(prefs, "prefs") }
        }
    }

    // MARK: Library state (`AppLibrary.swift`)

    /// The printer models the engine knows (cached from earlier runs until they load), with their
    /// resolution and head width.
    public internal(set) var models: [Model] = []
    /// Folders of the template library besides the default one.
    public internal(set) var templateFolders: [String] = []
    /// The paper library as last listed.
    public internal(set) var papers: [Paper] = []
    /// The roll in the connected printer, once read with `detectPaper`.
    public internal(set) var detected: PaperDetect?
    /// The print queue: queued, printing and recently finished jobs, oldest first.
    public internal(set) var jobs: [PrintJob] = []
    public internal(set) var queuePaused = false
    /// The connect that failed last, for the tips and Try again.
    public internal(set) var connectFailure: ConnectFailure?
    /// Font families the engine has (`font.list`).
    public internal(set) var fonts: [String] = []
    /// The engine's pictograms (`icon.list`) and their thumbnails by name.
    public internal(set) var icons: [IconInfo] = []
    public internal(set) var iconThumbs: [String: CGImage] = [:]
    /// The history entry being printed again: the editor opens its print sheet for it.
    public var reprint: HistoryEntry?

    /// The elements copied last (`EditorArrange.swift`).
    @ObservationIgnored var clip: Clip?

    /// The model new documents and checks use.
    public var modelId: String { printer?.model?.id ?? Self.defaultModel }

    public init(engine: Engine, labelsDir: URL, defaults: UserDefaults = .standard, debug: Bool) {
        self.engine = engine
        self.labelsDir = labelsDir
        self.defaults = defaults
        self.debug = debug
        try? FileManager.default.createDirectory(at: labelsDir, withIntermediateDirectories: true)
        recents = loadRecents()
        known = load([KnownPrinter].self, "known") ?? []
        allowAgentPrint = defaults.bool(forKey: "allow_agent_print")
        prefs = load(Prefs.self, "prefs") ?? Prefs()
        CurrentUnit.value = prefs.unit
        models = load([Model].self, "models") ?? []
    }

    /// Follow the engine's printer events and load the lists. Returns the loading task.
    @discardableResult
    public func start() -> Task<Void, Never> {
        let events = engine.devices
        Task { [weak self] in
            for await e in events {
                self?.onDeviceEvent(e)
            }
        }
        let jobEvents = engine.jobEvents
        Task { [weak self] in
            for await _ in jobEvents {
                await self?.refreshJobs()
            }
        }
        return Task {
            await loadModels()
            await loadFonts()
            await loadTemplates()
            await refreshHistory()
        }
    }

    /// A printer event pushed by the engine (ADR-0031): a dropped link or a new status.
    func onDeviceEvent(_ e: JSON) {
        guard let p = printer, e["address"]?.string == p.address else { return }
        if e["connected"]?.bool != true {
            printer = nil
            printerStatus = nil
            if e["lost"]?.bool == true { message = .printerLost }
        } else if e["status"] != nil {
            printerStatus = DeviceStatus.from(e)
        }
    }

    // MARK: - Templates

    public func loadTemplates() async {
        await report {
            let r = try await engine.call("template.list")
            templates = Template.list(r)
            templateFolders = (r["folders"]?.array ?? []).compactMap(\.string)
            for t in templates where thumbs[t.ref] == nil {
                await thumbnail(t.ref) { try await self.engine.call("template.new", ["ref": .string(t.ref)]) }
            }
            for r in recents where thumbs[r.path] == nil {
                await thumbnail(r.path) { try await self.engine.call("doc.open", ["path": .string(r.path)]) }
            }
        }
    }

    /// Open a document with `open`, render it small, close it. Failures leave no thumbnail.
    func thumbnail(_ key: String, open: () async throws -> JSON) async {
        guard let docId = try? await open()["doc_id"]?.string else { return }
        if let image = try? await engine.render(dpi: Self.thumbDpi, docId: docId) {
            thumbs[key] = image
        }
        _ = try? await engine.call("doc.close", ["doc_id": .string(docId)])
    }

    @discardableResult
    public func deleteTemplate(_ t: Template) -> Task<Void, Never> {
        Task {
            await report {
                try await engine.call("template.delete", ["ref": .string(t.ref)])
                thumbs[t.ref] = nil
                await loadTemplates()
            }
        }
    }

    // MARK: - Documents

    @discardableResult
    public func newLabel(widthMm: Double, heightMm: Double, media: String = "gap", cornerRadiusMm: Double = 0) -> Task<Void, Never> {
        openWith {
            try await self.engine.call("doc.new", [
                "width_mm": JSON(widthMm),
                "height_mm": JSON(heightMm),
                "media": JSON(media),
                "corner_radius_mm": cornerRadiusMm > 0 ? JSON(cornerRadiusMm) : nil,
                "printer_model": JSON(self.modelId),
                "name": JSON(self.newLabelName),
            ])
        }
    }

    /// Open a template as a new document named `name` (starter names are translated).
    @discardableResult
    public func fromTemplate(_ t: Template, name: String) -> Task<Void, Never> {
        openWith { try await self.engine.call("template.new", ["ref": JSON(t.ref), "name": JSON(name)]) }
    }

    @discardableResult
    public func openRecent(_ r: Recent) -> Task<Void, Never> {
        openWith { try await self.engine.call("doc.open", ["path": JSON(r.path)]) }
    }

    @discardableResult
    public func openHistory(_ h: HistoryEntry) -> Task<Void, Never> {
        openWith { try await self.engine.call("job.history_open", ["id": JSON(h.id)]) }
    }

    /// Copy a `.labely` file the user picked into app storage and open it. `url` may be
    /// security-scoped (from the file importer).
    @discardableResult
    public func openFile(_ url: URL) -> Task<Void, Never> {
        openWith {
            let target = self.newDocURL()
            let scoped = url.startAccessingSecurityScopedResource()
            defer { if scoped { url.stopAccessingSecurityScopedResource() } }
            try FileManager.default.copyItem(at: url, to: target)
            return try await self.engine.call("doc.open", ["path": JSON(target.path)])
        }
    }

    func openWith(_ open: @escaping () async throws -> JSON) -> Task<Void, Never> {
        Task {
            await report {
                guard let docId = try await open()["doc_id"]?.string else { return }
                let ed = EditorState(app: self, docId: docId)
                try await ed.refresh()
                editor = ed
            }
        }
    }

    /// Close the editor's document. Edits were saved as they happened; an untouched one is dropped.
    @discardableResult
    public func closeEditor() -> Task<Void, Never> {
        guard let ed = editor else { return Task {} }
        editor = nil
        reprint = nil
        return Task {
            await report {
                ed.cleanUp()
                if !ed.saved { try await ed.save() }
                try await engine.call("doc.close", ["doc_id": JSON(ed.docId)])
            }
        }
    }

    /// Where a document without a path is saved.
    func newDocURL() -> URL {
        labelsDir.appendingPathComponent("\(Self.stamp()).labely")
    }

    func remember(_ r: Recent, preview: CGImage?) {
        recents = Array(([r] + recents.filter { $0.path != r.path }).prefix(Self.maxRecents))
        if let preview { thumbs[r.path] = preview }
        save(recents, "recents")
    }

    private func loadRecents() -> [Recent] {
        (load([Recent].self, "recents") ?? []).filter { FileManager.default.fileExists(atPath: $0.path) }
    }

    // MARK: - History

    public func refreshHistory() async {
        await report { history = HistoryEntry.list(try await engine.call("job.history", ["limit": 100])) }
    }

    // MARK: - Printers

    /// Scan for printers. The view checks the Bluetooth permission first.
    @discardableResult
    public func scan() -> Task<Void, Never> {
        busy = Self.scanning
        return Task {
            defer { busy = nil }
            await report {
                let r = try await engine.call("device.scan", ["timeout_ms": JSON(Self.scanMs)])
                scanResults = (r["devices"]?.objects ?? []).compactMap(Device.from).filter { $0.address.hasPrefix("ble:") }
            }
        }
    }

    @discardableResult
    public func connect(_ address: String, name: String) -> Task<Void, Never> {
        busy = address
        connectFailure = nil
        return Task {
            defer { busy = nil }
            await report {
                do {
                    _ = try await connectNow(address, name: name)
                } catch {
                    connectFailure = ConnectFailure(address: address, name: name, message: error.localizedDescription)
                    throw error
                }
            }
        }
    }

    /// Connect to `address` unless it is already the connected printer, and make it the app's
    /// printer. Throws `CommandFailure`; used by the UI and by App Intents.
    public func connectNow(_ address: String, name: String = "") async throws -> Device {
        if let p = printer {
            if p.address == address { return p }
            await disconnectQuietly(p.address)
        }
        guard let dev = Device.from(try await engine.call("device.connect", ["address": JSON(address)])) else {
            throw CommandFailure(code: "internal", message: "device.connect returned no device", hint: nil)
        }
        printer = dev
        let entry = KnownPrinter(address: address, name: name.isEmpty ? dev.name : name)
        known = Array(([entry] + known.filter { $0.address != address }).prefix(Self.maxKnown))
        save(known, "known")
        await refreshStatus()
        return dev
    }

    @discardableResult
    public func disconnect() -> Task<Void, Never> {
        Task {
            guard let p = printer else { return }
            await disconnectQuietly(p.address)
        }
    }

    func disconnectQuietly(_ address: String) async {
        _ = try? await engine.call("device.disconnect", ["address": JSON(address)])
        printer = nil
        printerStatus = nil
    }

    public func refreshStatus() async {
        guard let p = printer else { return }
        do {
            printerStatus = DeviceStatus.from(try await engine.call("device.status", ["address": JSON(p.address)]))
        } catch let e as CommandFailure where e.code == "device_unavailable" || e.code == "not_found" {
            printer = nil
            printerStatus = nil
        } catch {}
    }

    @discardableResult
    public func calibrate() -> Task<Void, Never> {
        guard let p = printer else { return Task {} }
        busy = p.address
        return Task {
            defer { busy = nil }
            await report {
                try await engine.call("device.calibrate", ["address": JSON(p.address)])
                message = .calibrated
            }
        }
    }

    /// Read the settings the connected printer reports (`device.settings`).
    @discardableResult
    public func loadPrinterSettings() -> Task<Void, Never> {
        guard let p = printer else { return Task {} }
        return Task {
            await report {
                let r = try await engine.call("device.settings", ["address": JSON(p.address)])
                if printer?.address == p.address { printerSettings = PrinterSettings.from(r) }
            }
        }
    }

    /// Change the given settings on the connected printer (`device.set_settings`).
    @discardableResult
    public func setPrinterSettings(autoOffLevel: Int? = nil, soundPower: Bool? = nil, soundBluetooth: Bool? = nil) -> Task<Void, Never> {
        guard let p = printer else { return Task {} }
        return Task {
            await report {
                let r = try await engine.call("device.set_settings", [
                    "address": JSON(p.address), "auto_off_level": JSON(autoOffLevel),
                    "sound_power": soundPower.map { JSON($0) }, "sound_bluetooth": soundBluetooth.map { JSON($0) },
                ])
                if printer?.address == p.address { printerSettings = PrinterSettings.from(r) }
            }
        }
    }

    public func forget(_ k: KnownPrinter) {
        known = known.filter { $0.address != k.address }
        save(known, "known")
    }

    // MARK: - Errors and storage

    /// Run `body`; a failure becomes the toast message instead of a crash.
    func report(_ body: () async throws -> Void) async {
        do {
            try await body()
        } catch is CancellationError {
        } catch {
            message = .text(error.localizedDescription)
        }
    }

    func save<T: Encodable>(_ value: T, _ key: String) {
        if let data = try? JSONEncoder().encode(value) { defaults.set(data, forKey: key) }
    }

    func load<T: Decodable>(_ type: T.Type, _ key: String) -> T? {
        defaults.data(forKey: key).flatMap { try? JSONDecoder().decode(type, from: $0) }
    }

    private static func stamp() -> String {
        let f = DateFormatter()
        f.locale = Locale(identifier: "en_US_POSIX")
        f.dateFormat = "yyyyMMdd-HHmmss-SSS"
        return f.string(from: Date())
    }
}
