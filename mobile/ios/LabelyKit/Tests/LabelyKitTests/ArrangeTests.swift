import CoreGraphics
import Foundation
import Testing

@testable import LabelyKit

private func element(_ id: Int64 = 1, _ type: String = "rect", x: Double = 10, y: Double = 10, w: Double = 20, h: Double = 10, extra: JSON = [:]) -> Element {
    var o = extra.with("id", .int(id)).with("type", JSON(type))
    o = o.with("x_mm", JSON(x)).with("y_mm", JSON(y)).with("width_mm", JSON(w)).with("height_mm", JSON(h))
    return Element.from(o)!
}

/// Rotated frames, snapping, the rotation handle and the marquee.
struct GeometryTests {
    @Test func boundsFollowTheRotation() {
        let e = element(extra: ["rotation_deg": 90])
        let b = e.bounds
        #expect(abs(b.minX - 15) < 1e-9 && abs(b.maxX - 25) < 1e-9 && abs(b.minY - 5) < 1e-9 && abs(b.maxY - 25) < 1e-9)
        // A quarter turn: the point right of the unrotated frame is outside, the one above is inside.
        #expect(!e.contains(xMm: 29, yMm: 15) && e.contains(xMm: 20, yMm: 6))
        #expect(element().bounds == BoundsMm(minX: 10, minY: 10, maxX: 30, maxY: 20))
        #expect(element(1, "line", w: -5, h: 0).bounds == BoundsMm(minX: 5, minY: 10, maxX: 10, maxY: 10))
    }

    @Test func aTurnedFrameResizesAgainstItsOppositeSide() {
        let f = RotatedFrame(x: 10, y: 10, width: 20, height: 10, rotation: 0)
        #expect(f.resized(.bottomRight, to: CGPoint(x: 35, y: 25)).frame == FrameMm(x: 10, y: 10, width: 25, height: 15))
        // Never smaller than the minimum, and the fixed corner stays.
        #expect(f.resized(.bottomRight, to: CGPoint(x: 0, y: 0)).frame == FrameMm(x: 10, y: 10, width: 0.5, height: 0.5))

        let turned = RotatedFrame(x: 10, y: 10, width: 20, height: 10, rotation: 90)
        let fixed = turned.point(of: .topLeft)
        let r = turned.resized(.bottomRight, to: CGPoint(x: 10, y: 30))
        let after = r.point(of: .topLeft)
        #expect(abs(after.x - fixed.x) < 0.11 && abs(after.y - fixed.y) < 0.11)
        #expect(r.width > 20 && r.rotation == 90)
    }

    @Test func handlesOfATurnedElementTurnWithIt() {
        let e = element(extra: ["rotation_deg": 90])
        #expect(Handle.of(e) == Handle.box)
        // The top-left handle of the turned frame is at its top-right corner on screen.
        let p = e.rotated.point(of: .topLeft)
        #expect(abs(p.x - 25) < 1e-9 && abs(p.y - 5) < 1e-9)
        #expect(Handle.hit(CGPoint(x: 25.2, y: 5.1), on: e, radius: 2) == .topLeft)
        #expect(Handle.hit(CGPoint(x: 20, y: 15), on: e, radius: 2) == nil)
    }

    @Test func snappingPrefersNearbyLinesThenTheGrid() {
        let other = BoundsMm(minX: 30, minY: 0, maxX: 36, maxY: 6)
        let me = BoundsMm(minX: 4, minY: 10, maxX: 14, maxY: 14)
        // The right edge 14 + 15.7 snaps to the other's left edge at 30.
        let a = Snap.snap(labelWidthMm: 80, labelHeightMm: 30, others: [other], bounds: me, dx: 15.7, dy: 0.2, threshold: 0.5, grid: nil)
        #expect(abs(a.dx - 16) < 1e-9 && a.xs == [30] && a.dy == 0.2 && a.ys.isEmpty)
        // The centre 9 + 10.9 snaps to the label's centre at 20.
        let b = Snap.snap(labelWidthMm: 40, labelHeightMm: 30, others: [], bounds: me, dx: 10.9, dy: 0, threshold: 0.5, grid: nil)
        #expect(abs(b.dx - 11) < 1e-9 && b.xs == [20])
        // Nothing near: the grid rounds the left and top edge.
        let c = Snap.snap(labelWidthMm: 40, labelHeightMm: 30, others: [], bounds: me, dx: 3.3, dy: 3.3, threshold: 0.1, grid: 1)
        #expect(abs(c.dx - 3) < 1e-9 && abs(c.dy - 3) < 1e-9 && c.xs.isEmpty)
        let d = Snap.snap(labelWidthMm: 40, labelHeightMm: 30, others: [], bounds: me, dx: 3.3, dy: 3.3, threshold: 0.1, grid: nil)
        #expect(d.dx == 3.3 && d.dy == 3.3)
    }

    @Test func theRotationHandleTurnsAndSnaps() {
        let c = CGPoint(x: 20, y: 15)
        // From straight up to the right: a quarter turn clockwise.
        #expect(Rotation.dragged(start: 0, center: c, from: CGPoint(x: 20, y: 5), to: CGPoint(x: 30, y: 15), snap: false) == 90)
        #expect(Rotation.dragged(start: 0, center: c, from: CGPoint(x: 20, y: 5), to: CGPoint(x: 21.9, y: 5), snap: true) == 15)
        #expect(Rotation.dragged(start: 350, center: c, from: CGPoint(x: 20, y: 5), to: CGPoint(x: 30, y: 15), snap: false) == 80)
        #expect(Rotation.normalised(-90) == 270 && Rotation.normalised(720) == 0)
        let e = element()
        let h = CanvasGesture.rotateHandle(e, ptPerMm: 13)
        #expect(abs(h.x - 20) < 1e-9 && abs(h.y - 8) < 1e-9)
    }
}

struct ToolsTests {
    @Test func recordRangesReadLikeTheEngine() {
        #expect(RecordRange.parse("1,3,5-7", count: 9) == [1, 3, 5, 6, 7])
        #expect(RecordRange.parse("all", count: 3) == [1, 2, 3] && RecordRange.parse(" ", count: 2) == [1, 2])
        #expect(RecordRange.parse("7-", count: 9) == [7, 8, 9] && RecordRange.parse("3-1", count: 9) == [3, 2, 1])
        #expect(RecordRange.parse("2 , 2", count: 9) == [2, 2])
        #expect(RecordRange.parse("0", count: 9) == nil && RecordRange.parse("10", count: 9) == nil)
        #expect(RecordRange.parse("1,,2", count: 9) == nil && RecordRange.parse("a-b", count: 9) == nil)
    }

    @Test func unitsConvertBothWays() {
        let en = Locale(identifier: "en_US")
        #expect(LengthUnit.mm.format(50, locale: en) == "50" && LengthUnit.mm.format(12.34, locale: en) == "12.3")
        #expect(LengthUnit.inch.format(50.8, locale: en) == "2" && LengthUnit.inch.format(30, locale: en) == "1.181")
        #expect(LengthUnit.mm.format(2.5, locale: Locale(identifier: "hu_HU")) == "2,5")
        #expect(LengthUnit.inch.parse("2") == 50.8 && LengthUnit.inch.parse("0,5") == 12.7)
        #expect(LengthUnit.mm.parse("12,5") == 12.5 && LengthUnit.mm.parse("x") == nil)
        let (minor, major) = Ruler.steps(unit: .mm, ptPerMm: 6)
        #expect(minor == 1 && major == 10)
    }

    @Test func spansAreCutAndJoined() {
        var spans = TextSpans.applying([], range: 2..<6) { $0.bold = true }
        #expect(spans == [TextSpan(start: 2, end: 6, bold: true)])
        spans = TextSpans.applying(spans, range: 4..<8) { $0.italic = true }
        #expect(spans == [
            TextSpan(start: 2, end: 4, bold: true), TextSpan(start: 4, end: 6, bold: true, italic: true),
            TextSpan(start: 6, end: 8, italic: true),
        ])
        #expect(TextSpans.style(at: 5, spans: spans, base: TextSpan(start: 0, end: 0, bold: false, fontSizePt: 12)).italic == true)
        spans = TextSpans.applying(spans, range: 2..<4) { $0.italic = true }
        spans = TextSpans.applying(spans, range: 6..<8) { $0.bold = true }
        #expect(spans == [TextSpan(start: 2, end: 8, bold: true, italic: true)])
        #expect(TextSpans.clearing(spans, range: 3..<5) == [
            TextSpan(start: 2, end: 3, bold: true, italic: true), TextSpan(start: 5, end: 8, bold: true, italic: true),
        ])
        // "é😀x": the emoji is two UTF-16 units and one scalar.
        #expect(TextSpans.scalarRange(of: 1..<3, in: "é😀x") == 1..<2)
        #expect(TextSpans.scalarRange(of: 3..<4, in: "é😀x") == 2..<3)
    }

    @Test func theClipboardKeepsElementsAndTheirAssets() throws {
        let image = element(2, "image", extra: ["asset": "abc"])
        let text = ElementClipboard.encode([element(), image], assets: ["abc": ["mime": "image/png", "data_base64": "AA=="], "unused": [:]])
        let (elements, assets) = try #require(ElementClipboard.decode(text))
        #expect(elements.count == 2 && assets.object?.keys.sorted() == ["abc"])
        #expect(ElementClipboard.decode("hello") == nil && ElementClipboard.decode("{\"format\":\"x\"}") == nil)
        let shifted = try #require(ElementClipboard.decode(ElementClipboard.shifted(text, by: 1)))
        #expect(shifted.elements[0]["x_mm"]?.double == 11)
    }

    @Test func serialsCountLikeTheEngine() {
        #expect(Serial(name: "s", start: 1, step: 1, digits: 4, prefix: "N").value(2) == "N0003")
        #expect(Serial(name: "s", start: 5, step: -10).value(1) == "-5")
    }
}

/// Selection and arranging against the real engine.
@MainActor
struct ArrangeTests {
    let app: AppState

    init() throws {
        let dir = FileManager.default.temporaryDirectory.appendingPathComponent("labelykit-\(UUID().uuidString)")
        try FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
        let defaults = try #require(UserDefaults(suiteName: "labelykit-\(UUID().uuidString)"))
        app = AppState(engine: Engine(dataDir: dir.path, ble: nil), labelsDir: dir.appendingPathComponent("labels"), defaults: defaults, debug: true)
    }

    private func editor() async throws -> EditorState {
        await app.newLabel(widthMm: 50, heightMm: 30).value
        return try #require(app.editor)
    }

    @Test func severalElementsMoveAlignAndGroupAsOneStep() async throws {
        let ed = try await editor()
        await ed.addRect().value
        let a = try #require(ed.selected)
        await ed.addEllipse().value
        let b = try #require(ed.selected)
        await ed.addText("T").value
        let c = try #require(ed.selected)

        ed.tap(a.id)
        ed.multiSelect = true
        ed.tap(b.id)
        #expect(ed.selection == [a.id, b.id] && ed.selected == nil && ed.selectedElements.count == 2)

        await ed.move(ed.selectedElements, dx: 2.04, dy: -1).value
        #expect(ed.doc?.elements.first { $0.id == a.id }?.xMm == round1(a.xMm + 2.04))
        #expect(ed.doc?.elements.first { $0.id == b.id }?.yMm == round1(b.yMm - 1))
        // One undo step for both.
        await ed.undo().value
        #expect(ed.doc?.elements.first { $0.id == a.id }?.frame == a.frame && ed.doc?.elements.first { $0.id == b.id }?.frame == b.frame)

        await ed.align("left").value
        let xs = ed.selectedElements.map(\.xMm)
        #expect(xs[0] == xs[1])

        ed.tap(c.id)
        await ed.distribute("horizontal").value
        #expect(app.message == nil)

        await ed.group().value
        #expect(ed.selectedElements.allSatisfy { $0.group != nil } && ed.canUngroup)
        // A tap on one member selects the whole group.
        ed.multiSelect = false
        ed.tap(nil)
        ed.tap(a.id)
        #expect(Set(ed.selection) == Set([a.id, b.id, c.id]))
        await ed.ungroup().value
        #expect(ed.selectedElements.allSatisfy { $0.group == nil })

        ed.selectAll()
        #expect(ed.selection.count == 3 && ed.multiSelect)
        await ed.deleteSelection().value
        #expect(ed.doc?.elements.isEmpty == true && ed.selection.isEmpty)
    }

    @Test func rotatesFlipsLocksHidesAndReorders() async throws {
        let ed = try await editor()
        await ed.addRect().value
        let a = try #require(ed.selected)
        await ed.addEllipse().value
        let b = try #require(ed.selected)

        await ed.rotate(b, to: -90).value
        #expect(ed.selected?.rotationDeg == 270)
        await ed.turn90().value
        #expect(ed.selected?.rotationDeg == 0)
        await ed.flip(horizontal: true).value
        #expect(ed.selected?.flipH == true && ed.selected?.flipV == false)
        await ed.rename(b, to: " Dot ").value
        #expect(ed.selected?.name == "Dot")
        await ed.rename(b, to: "").value
        #expect(ed.selected?.name == nil)

        await ed.reorder("back").value
        #expect(ed.doc?.elements.map(\.id) == [b.id, a.id])
        await ed.reorder("front").value
        #expect(ed.doc?.elements.map(\.id) == [a.id, b.id])

        await ed.setLocked([b.id], true).value
        #expect(ed.selected?.locked == true)
        await ed.setLocked([b.id], false).value
        await ed.setHidden([a.id, b.id], true).value
        #expect(ed.doc?.elements.allSatisfy(\.hidden) == true)
        await ed.setHidden([a.id, b.id], false).value

        await ed.mirror("horizontal").value
        let copy = try #require(ed.selected)
        #expect(copy.mirrorOf == b.id && ed.doc?.elements.count == 3)
        await ed.unlinkMirror(copy).value
        #expect(ed.selected?.mirrorOf == nil && app.message == nil)
    }

    @Test func inkColourGoesRedAndBack() async throws {
        let ed = try await editor()
        await ed.addRect().value
        let a = try #require(ed.selected)
        await ed.addEllipse().value
        let b = try #require(ed.selected)
        // No two-colour printer yet: the picker is off.
        #expect(!ed.redPrinter)
        await app.loadModels()
        await ed.exec("doc.set_label", ["printer_model": "niimbot.b21_c2b"]).value
        #expect(ed.redPrinter)

        ed.selectAll()
        await ed.setRed(true).value
        #expect(ed.doc?.elements.allSatisfy(\.isRed) == true)
        await ed.undo().value
        #expect(ed.doc?.elements.contains(where: \.isRed) == false)
        ed.tap(nil)
        ed.multiSelect = false
        ed.tap(a.id)
        await ed.setRed(true).value
        #expect(ed.selected?.isRed == true && ed.doc?.elements.first { $0.id == b.id }?.isRed == false)
        await ed.setRed(false).value
        #expect(ed.selected?.isRed == false && app.message == nil)
    }

    @Test func copiesAndPastesAlsoBetweenDocuments() async throws {
        let ed = try await editor()
        await ed.addText("Copy me").value
        await ed.addRect().value
        ed.selectAll()
        let text = try #require(ed.copy())
        await ed.paste(text).value
        #expect(ed.doc?.elements.count == 4 && ed.selection.count == 2)
        // The copies land 1 mm off the originals.
        #expect(ed.selectedElements.first?.xMm == (ed.doc?.elements.first?.xMm ?? 0) + 1)

        // Plain text becomes a text element.
        await ed.paste("  Hello  ").value
        #expect(ed.selected?.text == "Hello")
        // Cutting is a copy and a delete.
        #expect(ed.copy() != nil)
        await ed.deleteSelection().value
        await ed.undo().value

        await app.closeEditor().value
        let other = try await editor()
        await other.paste(text).value
        #expect(other.doc?.elements.map(\.type) == ["text", "rect"] && app.message == nil)
        #expect(other.doc?.elements.first?.text == "Copy me")
    }
}
