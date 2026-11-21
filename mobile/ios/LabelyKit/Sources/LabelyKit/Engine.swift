import CoreGraphics
import Foundation
import ImageIO
import LabelyFFI
import Synchronization

/// A command that came back with `ok: false`. `code` is the stable error code.
public struct CommandFailure: Error, LocalizedError, Equatable {
    public let code: String
    public let message: String
    public let hint: String?

    public var errorDescription: String? { message.isEmpty ? code : message }
}

/// The Rust engine behind one serial queue (ADR-0032), as `Engine` on Android (ADR-0027).
/// Every edit is a command, so the shell holds no editing logic: it calls `call` and re-reads
/// the document. Commands run in the order they are called and never on the main thread.
///
/// The engine pushes job and printer changes (ADR-0031): `job(_:)` follows one job and
/// `devices` carries the printer events, so nothing polls.
public final class Engine: Sendable {
    public static let finished: Set<String> = ["done", "failed", "cancelled"]

    /// The engine's data directory: with no file roots, the only place its commands may read
    /// and write files.
    public let dataDir: URL
    private let raw: LabelyEngine
    private let queue = DispatchQueue(label: "labely-engine", qos: .userInitiated)
    private let jobs = JobBoard()

    /// Printer events: `{"address", "connected", "device"?, "status"?, "lost"?}` after
    /// `device.connect`, `device.disconnect`, `device.status`, or when a link drops. One reader.
    public let devices: AsyncStream<JSON>
    private let deviceSink: AsyncStream<JSON>.Continuation
    /// Every change of any print job, for the queue view. One reader.
    public let jobEvents: AsyncStream<JSON>
    private let jobSink: AsyncStream<JSON>.Continuation

    public init(dataDir: String, ble: NativeBle?) {
        self.dataDir = URL(fileURLWithPath: dataDir, isDirectory: true)
        raw = LabelyEngine(config: EngineConfig(dataDir: dataDir, fileRoots: [], allowPrint: true), ble: ble)
        (devices, deviceSink) = AsyncStream.makeStream(of: JSON.self, bufferingPolicy: .bufferingNewest(64))
        (jobEvents, jobSink) = AsyncStream.makeStream(of: JSON.self, bufferingPolicy: .bufferingNewest(64))
        raw.setEvents(listener: Listener(jobs: jobs, devices: deviceSink, jobSink: jobSink))
    }

    deinit {
        raw.setEvents(listener: nil)
        deviceSink.finish()
        jobSink.finish()
    }

    public var apiVersion: Int { Int(raw.apiVersion()) }

    /// Run `id` and return its `result`, or throw `CommandFailure`.
    @discardableResult
    public func call(_ id: String, _ params: JSON = [:]) async throws -> JSON {
        let text = params.text
        return try await run { raw in try Engine.unwrap(raw.execute(id: id, paramsJson: text)) }
    }

    /// The exact 1-bit print raster of page 0 of the active (or `docId`) document.
    public func render(dpi: Int, docId: String? = nil) async throws -> CGImage {
        let png = try await run { raw in
            do {
                return try raw.renderPng(docId: docId, dpi: UInt32(dpi), page: 0)
            } catch EngineError.Command(let code, let detail) {
                throw CommandFailure(code: code, message: detail, hint: nil)
            }
        }
        return try Self.image(png)
    }

    /// The 1-bit print raster of page 0 with the bindings filled from `record` (1-based).
    public func preview(dpi: Int, docId: String, record: Int) async throws -> CGImage {
        let r = try await call("render.preview", ["doc_id": JSON(docId), "dpi": JSON(dpi), "record": JSON(record)])
        guard let png = r["png_base64"]?.string.flatMap({ Data(base64Encoded: $0) }) else {
            throw CommandFailure(code: "internal", message: "render.preview returned no image", hint: nil)
        }
        return try Self.image(png)
    }

    static func image(_ png: Data) throws -> CGImage {
        guard let src = CGImageSourceCreateWithData(png as CFData, nil),
              let image = CGImageSourceCreateImageAtIndex(src, 0, nil)
        else { throw CommandFailure(code: "internal", message: "cannot decode the label preview", hint: nil) }
        return image
    }

    /// Every change of job `id`, starting with the latest one already seen.
    public func job(_ id: String) -> AsyncStream<JSON> {
        jobs.watch(id)
    }

    /// Wait until job `id` is done, failed or cancelled; returns its last state.
    public func finished(_ id: String) async -> JSON? {
        for await j in job(id) where Engine.finished.contains(j["state"]?.string ?? "") {
            return j
        }
        return nil
    }

    private func run<T: Sendable>(_ body: @escaping @Sendable (LabelyEngine) throws -> T) async throws -> T {
        let raw = raw
        return try await withCheckedThrowingContinuation { c in
            queue.async { c.resume(with: Result { try body(raw) }) }
        }
    }

    static func unwrap(_ envelope: String) throws -> JSON {
        let env = try JSON(parsing: envelope)
        if env["ok"]?.bool == true {
            return env["result"] ?? [:]
        }
        let err = env["error"] ?? [:]
        let hint = err["hint"]?.string
        throw CommandFailure(
            code: err["code"]?.string ?? "internal",
            message: err["message"]?.string ?? "",
            hint: hint?.isEmpty == false ? hint : nil
        )
    }
}

/// Called on the engine's event thread, in order (ADR-0031). Must not block.
private final class Listener: DeviceEvents {
    let jobs: JobBoard
    let devices: AsyncStream<JSON>.Continuation
    let jobSink: AsyncStream<JSON>.Continuation

    init(jobs: JobBoard, devices: AsyncStream<JSON>.Continuation, jobSink: AsyncStream<JSON>.Continuation) {
        self.jobs = jobs
        self.devices = devices
        self.jobSink = jobSink
    }

    func jobChanged(jobJson: String) {
        if let j = try? JSON(parsing: jobJson) {
            jobs.update(j)
            jobSink.yield(j)
        }
    }

    func deviceChanged(eventJson: String) {
        if let e = try? JSON(parsing: eventJson) { devices.yield(e) }
    }
}

/// The latest state of each job this session and the streams watching them. A lock rather
/// than an actor, so updates keep the order the engine sent them in.
final class JobBoard: Sendable {
    private static let maxJobs = 100

    private struct State {
        var latest: [String: JSON] = [:]
        var order: [String] = []
        var watchers: [String: [UUID: AsyncStream<JSON>.Continuation]] = [:]
    }

    private let state = Mutex(State())

    func update(_ job: JSON) {
        guard let id = job["id"]?.idString else { return }
        state.withLock { s in
            if s.latest[id] == job { return }
            if s.latest[id] == nil { s.order.append(id) }
            s.latest[id] = job
            while s.order.count > Self.maxJobs {
                let old = s.order.removeFirst()
                s.latest[old] = nil
            }
            for c in (s.watchers[id] ?? [:]).values { c.yield(job) }
        }
    }

    func watch(_ id: String) -> AsyncStream<JSON> {
        let (stream, c) = AsyncStream.makeStream(of: JSON.self, bufferingPolicy: .unbounded)
        let token = UUID()
        state.withLock { s in
            s.watchers[id, default: [:]][token] = c
            if let j = s.latest[id] { c.yield(j) }
        }
        c.onTermination = { [weak self] _ in
            self?.state.withLock { s in
                s.watchers[id]?[token] = nil
                if s.watchers[id]?.isEmpty == true { s.watchers[id] = nil }
            }
        }
        return stream
    }
}
