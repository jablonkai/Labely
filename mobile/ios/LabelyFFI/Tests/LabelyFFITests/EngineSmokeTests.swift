import Foundation
import Testing

@testable import LabelyFFI

/// The Rust engine links and runs commands end to end (ADR-0030), like the Android
/// `EngineSmokeTest`.
struct EngineSmokeTests {
    let dataDir: URL

    init() throws {
        dataDir = FileManager.default.temporaryDirectory
            .appendingPathComponent("labely-\(UUID().uuidString)", isDirectory: true)
        try FileManager.default.createDirectory(at: dataDir, withIntermediateDirectories: true)
    }

    func engine(allowPrint: Bool = true) -> LabelyEngine {
        LabelyEngine(
            config: EngineConfig(dataDir: dataDir.path, fileRoots: [], allowPrint: allowPrint),
            ble: nil
        )
    }

    /// Parse an envelope.
    func envelope(_ json: String) throws -> [String: Any] {
        let value = try JSONSerialization.jsonObject(with: Data(json.utf8))
        return try #require(value as? [String: Any])
    }

    /// Run a command that must succeed and return its result.
    @discardableResult
    func run(_ e: LabelyEngine, _ id: String, _ params: String = "{}") throws -> [String: Any] {
        let env = try envelope(e.execute(id: id, paramsJson: params))
        #expect(env["ok"] as? Bool == true, "\(id): \(env)")
        return env["result"] as? [String: Any] ?? [:]
    }

    func errorCode(_ json: String) throws -> String? {
        let env = try envelope(json)
        #expect(env["ok"] as? Bool == false)
        return (env["error"] as? [String: Any])?["code"] as? String
    }

    @Test func rendersAndPrintsOnTheSimulator() throws {
        let e = engine()
        #expect(e.apiVersion() == 3)
        try run(e, "doc.new", #"{"width_mm": 40, "height_mm": 12, "printer_model": "niimbot.b21_pro"}"#)
        try run(
            e, "element.add_text",
            #"{"x_mm": 1, "y_mm": 1, "width_mm": 38, "height_mm": 10, "text": "Labely"}"#)
        let png = try e.renderPng(docId: nil, dpi: 203, page: 0)
        #expect(png.prefix(4) == Data([0x89, 0x50, 0x4E, 0x47]))

        try run(e, "device.connect", #"{"address": "sim:niimbot.b21_pro"}"#)
        let job = try run(e, "print.submit", #"{"address": "sim:niimbot.b21_pro"}"#)
        #expect(job["state"] as? String == "done")
    }

    @Test func pushesJobAndDeviceEvents() throws {
        let e = engine()
        let rec = Recorder()
        e.setEvents(listener: rec)
        try run(e, "doc.new", #"{"width_mm": 20, "height_mm": 8, "printer_model": "niimbot.b21_pro"}"#)
        try run(e, "device.connect", #"{"address": "sim:niimbot.b21_pro"}"#)
        try run(e, "print.submit", #"{"address": "sim:niimbot.b21_pro", "copies": 2, "wait": false}"#)
        #expect(rec.done.wait(timeout: .now() + 10) == .success)
        e.setEvents(listener: nil)

        let states = rec.snapshot().jobs.compactMap { $0["state"] as? String }
        #expect(states.first == "queued")
        #expect(states.last == "done")
        #expect(states.contains("printing"))
        let device = try #require(rec.snapshot().devices.first)
        #expect(device["address"] as? String == "sim:niimbot.b21_pro")
        #expect(device["connected"] as? Bool == true)
    }

    @Test func errorsAndPolicyComeBackAsEnvelopes() throws {
        let e = engine(allowPrint: false)
        #expect(try errorCode(e.execute(id: "doc.new", paramsJson: "[1]")) == "bad_params")

        try run(e, "doc.new", #"{"width_mm": 20, "height_mm": 8}"#)
        let outside = e.execute(id: "doc.save", paramsJson: #"{"path": "/private/x.labely"}"#)
        #expect(try errorCode(outside) == "forbidden")
        let inside = dataDir.appendingPathComponent("a.labely").path
        try run(e, "doc.save", String(data: try JSONSerialization.data(withJSONObject: ["path": inside]), encoding: .utf8)!)

        try run(e, "device.connect", #"{"address": "sim:niimbot.b21_pro"}"#)
        let print = e.execute(id: "print.submit", paramsJson: #"{"address": "sim:niimbot.b21_pro"}"#)
        #expect(try errorCode(print) == "forbidden")
        e.setAllowPrint(allow: true)
        try run(e, "print.submit", #"{"address": "sim:niimbot.b21_pro"}"#)

        #expect(throws: EngineError.self) {
            try e.renderPng(docId: "no-such-doc", dpi: 203, page: 0)
        }
    }
}

/// Plays the app's event listener; signals `done` when a job finishes.
final class Recorder: DeviceEvents, @unchecked Sendable {
    private let lock = NSLock()
    private var jobs: [[String: Any]] = []
    private var devices: [[String: Any]] = []
    let done = DispatchSemaphore(value: 0)

    func snapshot() -> (jobs: [[String: Any]], devices: [[String: Any]]) {
        lock.withLock { (jobs, devices) }
    }

    func jobChanged(jobJson: String) {
        guard let j = Self.parse(jobJson) else { return }
        lock.withLock { jobs.append(j) }
        if ["done", "failed", "cancelled"].contains(j["state"] as? String ?? "") { done.signal() }
    }

    func deviceChanged(eventJson: String) {
        guard let d = Self.parse(eventJson) else { return }
        lock.withLock { devices.append(d) }
    }

    static func parse(_ json: String) -> [String: Any]? {
        (try? JSONSerialization.jsonObject(with: Data(json.utf8))) as? [String: Any]
    }
}
