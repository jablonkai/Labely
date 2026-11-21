import CoreGraphics
import Foundation
import Testing

@testable import LabelyKit

/// The further element kinds, tables, serials, papers, templates, the queue and the history
/// against the real engine and the `sim:` printer.
@MainActor
struct FeatureTests {
    let dir: URL
    let app: AppState

    init() throws {
        dir = FileManager.default.temporaryDirectory.appendingPathComponent("labelykit-\(UUID().uuidString)")
        try FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
        let defaults = try #require(UserDefaults(suiteName: "labelykit-\(UUID().uuidString)"))
        app = AppState(engine: Engine(dataDir: dir.path, ble: nil), labelsDir: dir.appendingPathComponent("labels"), defaults: defaults, debug: true)
    }

    private func editor(_ w: Double = 50, _ h: Double = 30) async throws -> EditorState {
        await app.newLabel(widthMm: w, heightMm: h).value
        return try #require(app.editor)
    }

    @Test func insertsTablesIconsBordersAndBoundTexts() async throws {
        let ed = try await editor()
        await ed.addTable(rows: 2, columns: 3).value
        let table = try #require(ed.selected)
        var info = try #require(TableInfo.of(table))
        #expect(info.rows == 2 && info.columns == 3)

        await ed.tableEdit(table.id, ["op": "set_cell", "row": 0, "col": 1, "text": "Hi", "bold": true, "align": "left", "invert": true]).value
        await ed.tableEdit(table.id, ["op": "insert_row", "at": 2]).value
        await ed.tableEdit(table.id, ["op": "merge", "row": 0, "col": 0, "rows": 1, "cols": 2]).value
        await ed.tableEdit(table.id, ["op": "resize_column", "at": 2, "width": 2.0]).value
        info = try #require(ed.selected.flatMap(TableInfo.of))
        #expect(info.rows == 3 && info.cell(0, 1).text == "Hi" && info.cell(0, 1).bold && info.cell(0, 1).invert)
        #expect(info.merge(at: 0, 1) != nil && info.isCovered(0, 1) && !info.isCovered(0, 0) && info.columnWidths[2] == 2)
        await ed.tableEdit(table.id, ["op": "split", "row": 0, "col": 0]).value
        await ed.tableEdit(table.id, ["op": "delete_column", "at": 0]).value
        await ed.tableEdit(table.id, ["op": "clear"]).value
        info = try #require(ed.selected.flatMap(TableInfo.of))
        #expect(info.merges.isEmpty && info.columns == 2 && info.cell(0, 0).text.isEmpty)

        await app.loadIcons()
        let icon = try #require(app.icons.first)
        await app.loadIconThumbs(category: icon.category)
        #expect(app.iconThumbs[icon.name]?.width == 60)
        await ed.addIcon(icon.name).value
        #expect(ed.selected?.type == "icon" && ed.selected?.iconName == icon.name)

        for style in BorderStyles.all {
            await ed.addBorder(style).value
            #expect(ed.selected?.borderStyle == style && ed.selected?.widthMm == 50)
        }

        await ed.addBoundText(BoundText.serial).value
        #expect(ed.selected?.text == "{{serial}}" && ed.serials.map(\.name) == ["serial"])
        await ed.setSerial(Serial(name: "serial", start: 7, step: 2, digits: 3, prefix: "A", suffix: "!")).value
        #expect(ed.serials.first == Serial(name: "serial", start: 7, step: 2, digits: 3, prefix: "A", suffix: "!"))
        #expect(ed.freeSerialName == "serial2")
        await ed.addBoundText(BoundText.date).value
        #expect(ed.selected?.text == BoundText.date)
        await ed.removeSerial("serial").value
        #expect(ed.serials.isEmpty && app.message == nil)
    }

    @Test func textShapeAndImagePropertiesRoundTrip() async throws {
        let ed = try await editor()
        await app.loadFonts()
        #expect(app.fonts.contains("Noto Sans"))
        await ed.addText("Hello world").value
        let t = try #require(ed.selected)
        await ed.setProps(t.id, [
            "italic": true, "underline": true, "align": "justify", "valign": "middle", "fit": "fill", "line_spacing": 1.5,
            "letter_spacing_mm": 0.2, "wrap": false, "invert": true, "direction": "vertical", "max_length_mm": 30.0,
            "spans": .array(TextSpans.applying([], range: 0..<5) { $0.bold = true }.map(\.json)),
        ]).value
        let e = try #require(ed.selected)
        #expect(e.italic && e.underline && e.align == "justify" && e.valign == "middle" && e.fit == "fill")
        #expect(e.lineSpacing == 1.5 && e.letterSpacingMm == 0.2 && !e.wrap && e.invert && e.direction == "vertical" && e.maxLengthMm == 30)
        #expect(TextSpans.of(e) == [TextSpan(start: 0, end: 5, bold: true)])

        await ed.addLine().value
        let line = try #require(ed.selected)
        await ed.setProps(line.id, ["dash_mm": .array(LineStyle.dashed.dash(strokeMm: 0.4)!.map { JSON($0) })]).value
        #expect(LineStyle.of(ed.selected?.dashMm) == .dashed)
        await ed.setProps(line.id, .object(["dash_mm": .null])).value
        #expect(LineStyle.of(ed.selected?.dashMm) == .solid && LineStyle.of(LineStyle.dotted.dash(strokeMm: 0.3)) == .dotted)

        await ed.addRect().value
        let rect = try #require(ed.selected)
        await ed.setProps(rect.id, ["fill": true, "corner_radius_mm": 2.0, "stroke_mm": 1.0]).value
        #expect(ed.selected?.fill == true && ed.selected?.cornerRadiusMm == 2 && ed.selected?.strokeMm == 1)

        await ed.addBarcode().value
        let barcode = try #require(ed.selected)
        await ed.setProps(barcode.id, ["text_size_pt": 6.0]).value
        #expect(ed.selected?.textSizePt == 6)
        await ed.addQr().value
        let qr = try #require(ed.selected)
        await ed.setProps(qr.id, ["ecc": "H"]).value
        #expect(ed.selected?.ecc == "H" && app.message == nil)
    }

    @Test func labelSettingsCheckAndSaveACopy() async throws {
        let ed = try await editor()
        await ed.setLabelProps(["name": "Shelf", "corner_radius_mm": 2.0, "density": 4]).value
        #expect(ed.doc?.name == "Shelf" && ed.doc?.label.cornerRadiusMm == 2 && ed.doc?.label.density == 4)
        #expect(app.recents.first?.name == "Shelf")
        await ed.setLabelProps(["density": 0]).value
        #expect(ed.doc?.label.density == nil)

        // An element outside the label is a problem that names its element.
        await ed.addRect().value
        let rect = try #require(ed.selected)
        await ed.exec("element.move", ["id": .int(rect.id), "x_mm": 45.0, "y_mm": 5.0]).value
        await ed.check().value
        #expect(ed.issues?.contains { $0.rule == "out_of_bounds" && $0.elementId == rect.id } == true)

        let copy = try #require(await ed.exportCopy())
        #expect(copy.lastPathComponent == "Shelf.labely" && FileManager.default.fileExists(atPath: copy.path))

        // Recents: rename, duplicate, delete.
        await app.closeEditor().value
        let recent = try #require(app.recents.first)
        await app.renameRecent(recent, to: "Bin").value
        #expect(app.recents.first?.name == "Bin")
        await app.duplicateRecent(try #require(app.recents.first), name: "Bin 2").value
        #expect(app.recents.map(\.name) == ["Bin 2", "Bin"])
        await app.openRecent(try #require(app.recents.first)).value
        #expect(app.editor?.doc?.name == "Bin 2" && app.editor?.doc?.elements.count == 1)
        await app.closeEditor().value
        app.deleteRecent(try #require(app.recents.first))
        #expect(app.recents.map(\.name) == ["Bin"] && app.message == nil)
    }

    @Test func templatesHaveFolders() async throws {
        let ed = try await editor()
        await ed.addText("T").value
        await ed.saveTemplate("Mine", folder: "Shop").value
        #expect(app.templateFolders == ["Shop"] && app.templateExists(name: "mine", folder: "Shop"))
        #expect(!app.templateExists(name: "Mine", folder: ""))
        let t = try #require(app.templates.first { $0.name == "Mine" })
        await app.duplicateTemplate(t, name: "Copy").value
        #expect(app.templates.contains { $0.name == "Copy" && $0.folder == "Shop" })
        await app.moveTemplate(t, name: "Renamed", folder: "").value
        #expect(app.templates.contains { $0.name == "Renamed" && $0.folder.isEmpty })
        await app.createFolder("Empty").value
        await app.renameFolder("Empty", to: "Later").value
        #expect(app.templateFolders == ["Later", "Shop"])
        await app.deleteFolder("Shop").value
        #expect(app.templateFolders == ["Later"] && !app.templates.contains { $0.name == "Copy" })
        // A starter can be copied into the library.
        let starter = try #require(app.templates.first { $0.starter })
        await app.duplicateTemplate(starter, folder: "Later").value
        #expect(app.templates.contains { !$0.starter && $0.folder == "Later" } && app.message == nil)
    }

    @Test func papersAreListedAddedAndApplied() async throws {
        await app.loadPapers()
        #expect(!app.papers.isEmpty && app.papers.allSatisfy { !$0.custom })
        await app.loadPapers(query: "50x30")
        #expect(app.papers.contains { $0.widthMm == 50 && $0.heightMm == 30 })
        await app.addPaper(name: "My roll", widthMm: 33, heightMm: 22, cornerRadiusMm: 1, media: "gap").value
        let mine = try #require(app.papers.first { $0.custom })
        #expect(mine.id.hasPrefix("my:") && mine.name == "My roll")

        let ed = try await editor()
        await ed.applyPaper(mine.id).value
        #expect(ed.doc?.label.widthMm == 33 && ed.doc?.label.heightMm == 22 && ed.doc?.label.cornerRadiusMm == 1)

        // The simulator's roll: read it, link it, and the label matches after applying it.
        await app.connect(AppState.simAddress, name: "Sim").value
        #expect(app.model("niimbot.b21_pro")?.dpi == 300 && (app.model("niimbot.b21_pro")?.headMm ?? 0) > 49)
        await ed.refreshLoadedPaper()
        if let barcode = ed.loadedPaper?.barcode {
            await app.linkPaper(barcode: barcode, to: mine).value
            #expect(ed.loadedPaper?.paper?.id == mine.id && ed.loadedPaper?.matchesLabel == true)
            #expect(app.detected?.paper?.id == mine.id)
            await app.unlinkPaper(barcode: barcode).value
            #expect(ed.loadedPaper?.paper?.id != mine.id)
        }
        await app.deletePaper(mine).value
        #expect(!app.papers.contains(where: \.custom) && app.message == nil)
    }

    @Test func printsRangesWithOptionsAndKeepsTheQueueAndHistory() async throws {
        await app.start().value
        await app.connect(AppState.simAddress, name: "Sim").value
        let ed = try await editor(40, 12)
        await ed.attachCsv("Name,Qty\nApple,2\nPear,0\nPlum,1\nFig,1\n").value
        #expect(ed.dataHeader == true)
        await ed.addBound(column: "Name").value
        #expect(ed.selected?.text == "{{Name}}")

        var o = PrintOptions()
        o.records = "1,3-4"
        o.copies = 2
        o.rotate180 = true
        o.mirror = true
        o.offsetXMm = 1
        #expect(ed.labelCount(o) == 6)
        o.records = "9"
        #expect(ed.labelCount(o) == nil)
        o.records = "1,3-4"
        await ed.print(o)?.value
        #expect(ed.progress == PrintProgress(done: 6, total: 6, state: "done", error: nil) && ed.jobId == nil)

        // Copies from a column: 2 + 0 + 1 + 1.
        var c = PrintOptions()
        c.copiesColumn = "Qty"
        #expect(ed.labelCount(c) == nil)
        await ed.print(c)?.value
        #expect(ed.progress?.done == 4 && ed.progress?.state == "done")

        await app.refreshJobs()
        #expect(app.jobs.count == 2 && app.jobs.allSatisfy(\.finished) && !app.queuePaused)
        await app.pauseQueue().value
        #expect(app.queuePaused)
        await app.resumeQueue().value
        await app.clearJobs().value
        #expect(app.jobs.isEmpty && !app.queuePaused)

        #expect(app.history.count == 2 && app.history.allSatisfy { $0.hasFile && $0.address == AppState.simAddress })
        await app.closeEditor().value
        await app.printAgain(try #require(app.history.first)).value
        #expect(app.editor != nil && app.reprint?.id == app.history.first?.id)
        await app.closeEditor().value
        await app.deleteHistory(try #require(app.history.last)).value
        #expect(app.history.count == 1)
        await app.clearHistory().value
        #expect(app.history.isEmpty && app.message == nil)

        // The header row can be turned off: the first row becomes a record.
        let again = try await editor()
        await again.attachCsv("Name\nApple\n").value
        #expect(again.data?.rowCount == 1)
        await again.reattachData(header: false).value
        #expect(again.data?.rowCount == 2 && again.dataHeader == false && app.message == nil)
    }

    @Test func manualAddressesAndFailedConnects() async {
        #expect(PrinterAddress.normalise(" 192.168.1.50 ") == "tcp:192.168.1.50")
        #expect(PrinterAddress.normalise("printer.local:9100") == "tcp:printer.local:9100")
        #expect(PrinterAddress.normalise("sim:niimbot.b21_pro") == "sim:niimbot.b21_pro")
        #expect(PrinterAddress.normalise("") == nil && PrinterAddress.normalise("tcp:") == nil && PrinterAddress.normalise("a b") == nil)

        await app.connect("sim:no.such_model", name: "Ghost").value
        #expect(app.connectFailure?.address == "sim:no.such_model" && app.connectFailure?.name == "Ghost")
        await app.connectManual(AppState.simAddress).value
        #expect(app.printer != nil && app.connectFailure == nil)
        #expect(app.printerStatus?.lidClosed != nil || app.printerStatus?.batteryPercent != nil)
    }

    @Test func prefsPersist() throws {
        let defaults = try #require(UserDefaults(suiteName: "labelykit-\(UUID().uuidString)"))
        let a = AppState(engine: app.engine, labelsDir: dir.appendingPathComponent("labels"), defaults: defaults, debug: true)
        #expect(a.prefs == Prefs() && a.prefs.snap && !a.prefs.showGrid)
        a.prefs.unit = .inch
        a.prefs.gridMm = 2
        a.prefs.showGrid = true
        #expect(CurrentUnit.value == .inch)
        let b = AppState(engine: app.engine, labelsDir: dir.appendingPathComponent("labels"), defaults: defaults, debug: true)
        #expect(b.prefs.unit == .inch && b.prefs.gridMm == 2 && b.prefs.showGrid)
        b.prefs.unit = .mm
    }
}
