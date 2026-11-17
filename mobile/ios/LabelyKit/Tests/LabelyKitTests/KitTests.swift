import CoreGraphics
import Foundation
import Testing

@testable import LabelyKit

struct JSONTests {
    @Test func roundTripsAndKeepsIntegers() throws {
        let j: JSON = ["id": 7, "x_mm": 1.5, "name": "a", "ok": true, "none": nil, "list": [1, "b"]]
        #expect(j["none"] == nil)
        let back = try JSON(parsing: j.text)
        #expect(back == j)
        #expect(back["id"]?.int64 == 7)
        // Integers stay integers on the wire: serde rejects `7.0` for a u64.
        #expect(JSON.int(7).text == "7")
        #expect(back["x_mm"]?.double == 1.5)
    }

    @Test func envelopesBecomeResultsOrFailures() throws {
        #expect(try Engine.unwrap(#"{"ok": true, "result": {"a": 1}}"#)["a"]?.int == 1)
        #expect(throws: CommandFailure(code: "bad_params", message: "nope", hint: "try")) {
            try Engine.unwrap(#"{"ok": false, "error": {"code": "bad_params", "message": "nope", "hint": "try"}}"#)
        }
    }
}

struct IconTests {
    @Test func everyIconParses() throws {
        for icon in Icon.allCases {
            let p = try SvgPath.parse(icon.svg)
            #expect(!p.isEmpty, "\(icon)")
            let box = p.boundingBoxOfPath
            #expect(box.minX >= 0 && box.minY >= 0 && box.maxX <= 24 && box.maxY <= 24, "\(icon): \(box)")
        }
    }

    @Test func arcsDrawWholeCircles() throws {
        // `a9 9 0 1 0 0.01 0` from (12, 3) is the clock's circle around (12, 12).
        let box = try SvgPath.parse(Icon.clock.svg).boundingBoxOfPath
        #expect(abs(box.minX - 3) < 0.05 && abs(box.maxX - 21) < 0.05)
        #expect(abs(box.minY - 3) < 0.05 && abs(box.maxY - 21) < 0.05)
    }

    @Test func numbersMaySplitOnSignsAndDots() throws {
        let p = try SvgPath.parse("M0 0l-3.6-3 .5.5h.01")
        #expect(p.currentPoint.x.isApproximately(-3.09) && p.currentPoint.y.isApproximately(-2.5))
        #expect(throws: SvgPath.ParseError.self) { try SvgPath.parse("M0 0 X1") }
    }
}

struct GattTests {
    @Test func matchesAliasesAndFullUuids() {
        #expect(Gatt.uuidMatches("FFE1", "ffe1"))
        #expect(Gatt.uuidMatches("0000FFE1-3C17-D293-8E48-14FE2E4DA212", "ffe1"))
        #expect(Gatt.uuidMatches("E7810A71-73AE-499D-8C15-FAA9AEF0C3F2", Gatt.niimbotService))
        #expect(!Gatt.uuidMatches("FFE2", "ffe1"))
    }

    @Test func scanMatchesByNameOrService() {
        #expect(Gatt.scanMatches(name: "B21_Pro-1234", services: [], prefixes: ["b21"]))
        #expect(Gatt.scanMatches(name: "", services: ["E7810A71-73AE-499D-8C15-FAA9AEF0C3F2"], prefixes: ["b21"]))
        #expect(!Gatt.scanMatches(name: "Speaker", services: ["180F"], prefixes: ["b21"]))
        #expect(Gatt.scanMatches(name: "Speaker", services: [], prefixes: []))
    }

    @Test func picksThePreferredUsableCharacteristic() {
        let candidates: [(service: String, characteristic: String, usable: Bool)] = [
            ("0000E0FF-3C17-D293-8E48-14FE2E4DA212", "FFE1", true),
            (Gatt.niimbotService.uppercased(), "BEF8D6C9-9C21-4C9E-B632-BD58C1009F9F", false),
            (Gatt.niimbotService.uppercased(), "BEF8D6C9-9C21-4C9E-B632-BD58C1009F9F", true),
        ]
        #expect(Gatt.pick(candidates) == 2)
        #expect(Gatt.pick(Array(candidates.prefix(2))) == 0)
        #expect(Gatt.pick([]) == nil)
    }
}

/// The app and editor state against the real engine and the `sim:` B21 Pro, as the Android
/// AppFunction tests drive the engine on the host.
@MainActor
struct StateTests {
    let dir: URL
    let app: AppState

    init() throws {
        dir = FileManager.default.temporaryDirectory.appendingPathComponent("labelykit-\(UUID().uuidString)")
        try FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
        let defaults = try #require(UserDefaults(suiteName: "labelykit-\(UUID().uuidString)"))
        app = AppState(
            engine: Engine(dataDir: dir.path, ble: nil),
            labelsDir: dir.appendingPathComponent("labels"),
            defaults: defaults,
            debug: true
        )
    }

    @Test func loadsStarterTemplatesWithThumbnails() async {
        await app.start().value
        #expect(app.templates.filter(\.starter).count == 10)
        #expect(app.templates.allSatisfy { app.thumbs[$0.ref] != nil })
        #expect(app.message == nil)
    }

    @Test func editsSaveAsTheyHappen() async throws {
        await app.newLabel(widthMm: 50, heightMm: 30).value
        let ed = try #require(app.editor)
        #expect(ed.doc?.label.widthMm == 50)
        #expect(ed.preview?.width == 400) // 50 mm at 203 dpi

        await ed.addText("Labely").value
        let text = try #require(ed.selected)
        #expect(text.type == "text" && text.text == "Labely")
        #expect(ed.saved)
        let path = try #require(ed.doc?.path)
        #expect(FileManager.default.fileExists(atPath: path))
        #expect(app.recents.first?.path == path)

        await ed.setProps(text.id, ["text": "Changed", "bold": true]).value
        #expect(ed.selected?.text == "Changed" && ed.selected?.bold == true)
        await ed.undo().value
        #expect(ed.selected?.text == "Labely")

        await ed.exec("element.delete", ["ids": [.int(text.id)]]).value
        #expect(ed.selectedId == nil && ed.doc?.elements.isEmpty == true)

        await app.closeEditor().value
        #expect(app.editor == nil && app.message == nil)
    }

    @Test func printsOnTheSimulatorWithPushedProgress() async throws {
        await app.connect(AppState.simAddress, name: "Sim").value
        #expect(app.printer?.model?.id == "niimbot.b21_pro")
        #expect(app.known.first == KnownPrinter(address: AppState.simAddress, name: "Sim"))

        await app.newLabel(widthMm: 40, heightMm: 12).value
        let ed = try #require(app.editor)
        await ed.addQr().value
        await ed.check().value
        #expect(ed.issues?.contains { $0.severity == "error" } == false)

        await ed.print(copies: 2, density: 3)?.value
        #expect(ed.progress == PrintProgress(done: 2, total: 2, state: "done", error: nil))
        #expect(app.history.first?.state == "done")

        // A disconnect event clears the printer.
        await app.disconnect().value
        #expect(app.printer == nil)
    }

    @Test func failuresBecomeMessages() async {
        await app.connect("sim:no.such_model", name: "").value
        #expect(app.printer == nil)
        guard case .text(let m) = app.message else {
            Testing.Issue.record("no message: \(String(describing: app.message))")
            return
        }
        #expect(!m.isEmpty)
    }
}

extension CGFloat {
    func isApproximately(_ v: Double) -> Bool { abs(Double(self) - v) < 1e-6 }
}
