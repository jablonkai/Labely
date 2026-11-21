import CoreGraphics
import Foundation
import ImageIO
import Testing

@testable import LabelyKit

/// The canvas geometry: fitting, zoom and pan clamping, handles and hit testing.
struct CanvasTests {
    let layout = CanvasLayout(viewport: CGSize(width: 348, height: 300), labelWidthMm: 50, labelHeightMm: 30)

    private func element(_ type: String = "rect", x: Double = 10, y: Double = 10, w: Double = 20, h: Double = 10, extra: JSON = [:]) -> Element {
        var o = extra.with("id", 1).with("type", JSON(type))
        o = o.with("x_mm", JSON(x)).with("y_mm", JSON(y)).with("width_mm", JSON(w)).with("height_mm", JSON(h))
        return Element.from(o)!
    }

    @Test func fitsTheLabelWithAMargin() {
        let zoom = CanvasZoom()
        #expect(layout.fitPtPerMm == 6) // (348 - 48) / 50
        #expect(layout.labelRect(zoom) == CGRect(x: 24, y: 60, width: 300, height: 180))
        #expect(layout.mm(CGPoint(x: 24 + 60, y: 60 + 30), zoom) == CGPoint(x: 10, y: 5))
        #expect(zoom.percent == 100 && zoom.isFit)
    }

    @Test func zoomIsClampedFromFitToEightTimes() {
        var zoom = CanvasZoom()
        let centre = CGPoint(x: 174, y: 150)
        zoom.zoom(by: 0.3, around: centre, in: layout)
        #expect(zoom.scale == 1 && zoom.isFit)
        zoom.zoom(by: 1.28, around: centre, in: layout)
        #expect(zoom.percent == 128)
        zoom.zoom(by: 100, around: centre, in: layout)
        #expect(zoom.scale == 8 && zoom.percent == 800)
        #expect(CanvasZoom.clampScale(.nan) == 1)
    }

    @Test func pinchKeepsThePointUnderTheFingers() {
        var zoom = CanvasZoom()
        let anchor = CGPoint(x: 100, y: 120)
        let before = layout.mm(anchor, zoom)
        zoom.zoom(by: 3, around: anchor, in: layout)
        let after = layout.mm(anchor, zoom)
        #expect(abs(after.x - before.x) < 1e-9 && abs(after.y - before.y) < 1e-9)
    }

    @Test func panNeverLosesTheLabel() {
        var zoom = CanvasZoom()
        // A fitted label stays centred.
        zoom.pan(by: CGSize(width: 500, height: -500), in: layout)
        #expect(zoom.pan == .zero)

        zoom.zoom(by: 2, around: CGPoint(x: 174, y: 150), in: layout)
        zoom.pan(by: CGSize(width: 5000, height: 5000), in: layout)
        // 600 × 360 pt label: its left and top edges stop one margin inside the viewport.
        let r = layout.labelRect(zoom)
        #expect(r.minX == CanvasLayout.margin && r.minY == CanvasLayout.margin)
        zoom.pan(by: CGSize(width: -5000, height: -5000), in: layout)
        let r2 = layout.labelRect(zoom)
        #expect(r2.maxX == 348 - CanvasLayout.margin && r2.maxY == 300 - CanvasLayout.margin)
        #expect(r2.intersects(CGRect(origin: .zero, size: layout.viewport)))

        // Zooming back out brings it back to fit.
        zoom.zoom(by: 0.1, around: .zero, in: layout)
        #expect(zoom.isFit)
    }

    @Test func handlesResizeWithAMinimumAndRounding() {
        let f = FrameMm(x: 10, y: 10, width: 20, height: 10)
        #expect(Handle.bottomRight.resized(f, dxMm: 2.04, dyMm: -3.26) == FrameMm(x: 10, y: 10, width: 22, height: 6.7))
        #expect(Handle.topLeft.resized(f, dxMm: -1.5, dyMm: 2) == FrameMm(x: 8.5, y: 12, width: 21.5, height: 8))
        #expect(Handle.left.resized(f, dxMm: 5, dyMm: 99) == FrameMm(x: 15, y: 10, width: 15, height: 10))
        #expect(Handle.top.resized(f, dxMm: 99, dyMm: -4) == FrameMm(x: 10, y: 6, width: 20, height: 14))
        // Never smaller than 1 mm; the opposite edge stays.
        #expect(Handle.right.resized(f, dxMm: -500, dyMm: 0) == FrameMm(x: 10, y: 10, width: 1, height: 10))
        #expect(Handle.topLeft.resized(f, dxMm: 500, dyMm: 500) == FrameMm(x: 29, y: 19, width: 1, height: 1))
        #expect(Handle.bottom.resized(f, dxMm: 0, dyMm: -500) == FrameMm(x: 10, y: 10, width: 20, height: 1))
    }

    @Test func lineHandlesMoveItsEnds() {
        let f = FrameMm(x: 10, y: 15, width: 30, height: 0)
        #expect(Handle.end.resized(f, dxMm: -10, dyMm: 5) == FrameMm(x: 10, y: 15, width: 20, height: 5))
        #expect(Handle.start.resized(f, dxMm: 4, dyMm: -2) == FrameMm(x: 14, y: 13, width: 26, height: 2))
        // An end may pass the other one: the line then runs backwards.
        #expect(Handle.end.resized(f, dxMm: -40, dyMm: 0) == FrameMm(x: 10, y: 15, width: -10, height: 0))
        // At least 1 mm long, in its own direction.
        #expect(Handle.end.resized(f, dxMm: -29.6, dyMm: 0) == FrameMm(x: 10, y: 15, width: 1, height: 0))
        #expect(Handle.start.resized(f, dxMm: 29.8, dyMm: 0) == FrameMm(x: 39, y: 15, width: 1, height: 0))
    }

    @Test func handlesDependOnTheElement() {
        #expect(Handle.of(element()) == Handle.box && Handle.box.count == 8)
        #expect(Handle.of(element("line", h: 0)) == [.start, .end])
        #expect(Handle.of(element(extra: ["locked": true])).isEmpty)
        #expect(Handle.of(element(extra: ["rotation_deg": 90])).isEmpty)
    }

    @Test func handleTargetsAre32Points() {
        let rect = CGRect(x: 100, y: 100, width: 120, height: 60)
        #expect(Handle.hit(CGPoint(x: 100 - 15, y: 100), rect: rect, handles: Handle.box) == .topLeft)
        #expect(Handle.hit(CGPoint(x: 100 - 17, y: 100), rect: rect, handles: Handle.box) == nil)
        #expect(Handle.hit(CGPoint(x: 220, y: 130 + 10), rect: rect, handles: Handle.box) == .right)
        #expect(Handle.hit(CGPoint(x: 160, y: 130), rect: rect, handles: Handle.box) == nil)
        // The nearest handle wins.
        #expect(Handle.hit(CGPoint(x: 150, y: 96), rect: rect, handles: Handle.box) == .top)
        // On a small element the inside moves it; the outside still resizes.
        let small = CGRect(x: 100, y: 100, width: 24, height: 24)
        #expect(Handle.hit(CGPoint(x: 108, y: 108), rect: small, handles: Handle.box) == nil)
        #expect(Handle.hit(CGPoint(x: 92, y: 92), rect: small, handles: Handle.box) == .topLeft)
        // A line: its two ends.
        let line = CGRect(x: 100, y: 100, width: 180, height: 0)
        #expect(Handle.hit(CGPoint(x: 286, y: 108), rect: line, handles: Handle.line) == .end)
        #expect(Handle.hit(CGPoint(x: 190, y: 100), rect: line, handles: Handle.line) == nil)
    }

    @Test func dragsResizeMoveOrDoNothing() throws {
        let rect = element()
        let locked = Element.from(["id": 2, "type": "rect", "x_mm": 35, "y_mm": 2, "width_mm": 10, "height_mm": 5, "locked": true])!
        let doc = try Doc.from([
            "doc_id": "d",
            "document": ["name": "t", "label": ["width_mm": 50, "height_mm": 30], "pages": [["elements": [rect.raw, locked.raw]]]],
        ])
        var zoom = CanvasZoom()
        zoom.zoom(by: 2, around: CGPoint(x: 174, y: 150), in: layout)
        // 12 pt per mm; the label's origin is at (-126, -30).
        let corner = layout.rect(rect.frame, zoom)
        #expect(corner == CGRect(x: -6, y: 90, width: 240, height: 120))
        let p = CGPoint(x: corner.maxX + 4, y: corner.maxY + 4)
        #expect(CanvasHit.drag(at: p, in: doc, selected: rect, layout: layout, zoom: zoom) == .resize(rect, .bottomRight))
        // Without a selection the same drag moves the element under the finger.
        #expect(CanvasHit.drag(at: p, in: doc, selected: nil, layout: layout, zoom: zoom) == .move(rect))
        let middle = CGPoint(x: corner.midX, y: corner.midY)
        #expect(CanvasHit.drag(at: middle, in: doc, selected: rect, layout: layout, zoom: zoom) == .move(rect))
        #expect(CanvasHit.element(at: middle, in: doc, layout: layout, zoom: zoom)?.id == 1)
        // Locked elements can be picked but not dragged; empty paper does nothing.
        let onLocked = layout.rect(locked.frame, zoom).origin.applying(.init(translationX: 20, y: 20))
        #expect(CanvasHit.element(at: onLocked, in: doc, layout: layout, zoom: zoom)?.id == 2)
        #expect(CanvasHit.drag(at: onLocked, in: doc, selected: nil, layout: layout, zoom: zoom) == nil)
        #expect(CanvasHit.drag(at: CGPoint(x: 300, y: 280), in: doc, selected: nil, layout: layout, zoom: zoom) == nil)

        // The drag is corrected for the zoom: 24 pt are 2 mm at 200 %.
        let moved = CanvasDrag.move(rect).frame(CGSize(width: 24, height: -12), ptPerMm: layout.ptPerMm(zoom))
        #expect(moved == FrameMm(x: 12, y: 9, width: 20, height: 10))
        let resized = CanvasDrag.resize(rect, .bottomRight).frame(CGSize(width: 24, height: -12), ptPerMm: layout.ptPerMm(zoom))
        #expect(resized == FrameMm(x: 10, y: 10, width: 22, height: 9))
    }
}

struct ParityLogicTests {
    @Test func readsBindingsAsTheEngineDoes() {
        #expect(Bindings.names(in: "{{Name}} – {{ Price }} Ft {{now:%Y}}") == ["Name", "Price", "now:%Y"])
        #expect(Bindings.names(in: "plain {{open") == [])
        #expect(Bindings.replacing(in: "a {{x}} b {{y}} {{", { $0 == "x" ? "1" : nil }) == "a 1 b {{y}} {{")
        let e = Element.from(["id": 1, "type": "barcode", "data": "{{EAN}}"])!
        #expect(Bindings.column(of: e, columns: ["Name", "EAN"]) == "EAN")
        #expect(Bindings.column(of: e, columns: ["Name"]) == nil)
        #expect(Bindings.property(of: "text") == "text" && Bindings.property(of: "image") == nil)
    }

    @Test func gtinCheckDigits() {
        #expect(Gtin.checkDigit(symbology: "ean13", data: "599800123456") == 0)
        #expect(Gtin.checkDigit(symbology: "ean13", data: "5998001234560") == 0)
        // A wrong last digit has no check digit to show.
        #expect(Gtin.checkDigit(symbology: "ean13", data: "5998001234567") == nil)
        #expect(Gtin.checkDigit(symbology: "ean8", data: "9638507") == 4)
        #expect(Gtin.checkDigit(symbology: "upc_a", data: "03600029145") == 2)
        #expect(Gtin.checkDigit(symbology: "ean13", data: "{{EAN}}") == nil)
        #expect(Gtin.checkDigit(symbology: "code128", data: "599800123456") == nil)
    }

    @Test func symbologiesFitTheKind() {
        #expect(Symbology.of("barcode").prefix(3) == ["ean13", "code128", "code39"])
        #expect(Symbology.of("code2d").prefix(2) == ["qr", "data_matrix"])
        #expect(Symbology.of("text").isEmpty)
        #expect(Symbology.name("data_matrix") == "Data Matrix")
    }

    @Test func exportFileNames() {
        #expect(Export.fileName(docName: "Price tag", format: .pdf) == "Price tag.pdf")
        #expect(Export.fileName(docName: "Price tag", record: 12, format: .png) == "Price tag-12.png")
        #expect(Export.fileName(docName: "a/b:c\n", format: .png) == "a-b-c-.png")
        #expect(Export.fileName(docName: " . ", record: 1, format: .png) == "label-1.png")
    }

    @Test func imageFramesKeepTheAspectRatio() {
        // Wide picture on a 50 × 30 label: as wide as the label allows.
        #expect(ImageImport.frame(pixelWidth: 1024, pixelHeight: 512, labelWidthMm: 50, labelHeightMm: 30) == FrameMm(x: 1, y: 3, width: 48, height: 24))
        // Tall picture: as high as it allows.
        #expect(ImageImport.frame(pixelWidth: 500, pixelHeight: 1000, labelWidthMm: 50, labelHeightMm: 30) == FrameMm(x: 18, y: 1, width: 14, height: 28))
    }

    @Test func picturesAreScaledDownToPng() throws {
        let p = try ImageImport.prepare(try png(width: 2000, height: 1000))
        #expect(p.width == 1024 && p.height == 512)
        #expect(p.png.starts(with: [0x89, 0x50, 0x4E, 0x47]))
        // Small pictures keep their size.
        let small = try ImageImport.prepare(try png(width: 300, height: 400))
        #expect(small.width == 300 && small.height == 400)
        #expect(throws: CommandFailure.self) { try ImageImport.prepare(Data("not an image".utf8)) }
    }
}

/// A grey test picture with a black box, as PNG.
func png(width: Int, height: Int) throws -> Data {
    let ctx = try #require(CGContext(
        data: nil, width: width, height: height, bitsPerComponent: 8, bytesPerRow: 0,
        space: CGColorSpaceCreateDeviceRGB(), bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue
    ))
    ctx.setFillColor(CGColor(red: 0.8, green: 0.8, blue: 0.8, alpha: 1))
    ctx.fill(CGRect(x: 0, y: 0, width: width, height: height))
    ctx.setFillColor(CGColor(red: 0, green: 0, blue: 0, alpha: 1))
    ctx.fill(CGRect(x: width / 4, y: height / 4, width: width / 2, height: height / 2))
    let image = try #require(ctx.makeImage())
    let out = NSMutableData()
    let dest = try #require(CGImageDestinationCreateWithData(out, "public.png" as CFString, 1, nil))
    CGImageDestinationAddImage(dest, image, nil)
    #expect(CGImageDestinationFinalize(dest))
    return out as Data
}

/// The parity features against the real engine and the `sim:` B21 Pro.
@MainActor
struct ParityTests {
    static let csv = "Name,Price,EAN\nApple,649,5998001234561\nPear,899,5998001234562\nBanana,749,5998001234563\n"
    /// A workbook with one sheet: Name, Price / Apple, 649 / Pear, 899.
    static let xlsx =
        "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"

    let dir: URL
    let app: AppState

    init() throws {
        // The engine compares real paths: /var is a link to /private/var.
        dir = FileManager.default.temporaryDirectory.resolvingSymlinksInPath().appendingPathComponent("labelykit-\(UUID().uuidString)")
        try FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
        let defaults = try #require(UserDefaults(suiteName: "labelykit-\(UUID().uuidString)"))
        app = AppState(
            engine: Engine(dataDir: dir.path, ble: nil),
            labelsDir: dir.appendingPathComponent("labels"),
            defaults: defaults,
            debug: true
        )
    }

    private func editor(_ w: Double = 50, _ h: Double = 30) async throws -> EditorState {
        await app.newLabel(widthMm: w, heightMm: h).value
        return try #require(app.editor)
    }

    private func pixels(_ image: CGImage?) -> Data? {
        image?.dataProvider?.data as Data?
    }

    @Test func attachesCsvAndPreviewsRecords() async throws {
        let ed = try await editor()
        #expect(ed.data == nil && ed.rows.isEmpty)
        await ed.addText("x").value
        let text = try #require(ed.selected)
        #expect(!ed.canBind(text))

        await ed.attachCsv(Self.csv, name: "products.csv").value
        #expect(ed.data == DataInfo(origin: nil, columns: ["Name", "Price", "EAN"], rowCount: 3))
        #expect(ed.dataSourceName == "products.csv")
        #expect(ed.rows.count == 3 && ed.rows[1] == ["Pear", "899", "5998001234562"])
        #expect(ed.record == 1 && ed.boundColumns == 0 && ed.canBind(text))

        // Binding sets the element's string to {{column}}.
        await ed.bind(text, to: "Name").value
        let bound = try #require(ed.selected)
        #expect(bound.text == "{{Name}}" && ed.boundColumn(bound) == "Name" && ed.boundColumns == 1)

        // The canvas shows the previewed record.
        let first = pixels(ed.preview)
        await ed.setRecord(3).value
        #expect(ed.record == 3)
        #expect(first != nil && pixels(ed.preview) != first)
        await ed.setRecord(99).value
        #expect(ed.record == 3)
        await ed.setRecord(1).value
        #expect(pixels(ed.preview) == first)

        // "None" keeps what the previewed record shows.
        await ed.setRecord(2).value
        await ed.bind(bound, to: nil).value
        #expect(ed.selected?.text == "Pear" && ed.boundColumns == 0)
        #expect(ed.selected.flatMap(ed.boundColumn) == nil)

        // Removing the data is one undo step.
        await ed.detachData().value
        #expect(ed.data == nil && ed.rows.isEmpty && ed.record == 1 && ed.dataSourceName == nil)
        await ed.undo().value
        #expect(ed.data?.rowCount == 3 && ed.rows.count == 3)
        #expect(app.message == nil)
    }

    @Test func pagesRows() async throws {
        let ed = try await editor()
        let csv = "N\n" + (1...120).map(String.init).joined(separator: "\n")
        await ed.attachCsv(csv).value
        #expect(ed.data?.rowCount == 120 && ed.rows.count == 50)
        await ed.loadMoreRows().value
        #expect(ed.rows.count == 100 && ed.rows[99] == ["100"])
        // Another edit keeps the loaded pages.
        await ed.addRect().value
        #expect(ed.rows.count == 100)
        await ed.loadMoreRows().value
        await ed.loadMoreRows().value
        #expect(ed.rows.count == 120 && ed.rows.last == ["120"])
        #expect(app.message == nil)
    }

    @Test func attachesPickedFiles() async throws {
        let ed = try await editor()
        let picked = dir.appendingPathComponent("picked")
        try FileManager.default.createDirectory(at: picked, withIntermediateDirectories: true)

        let sheet = picked.appendingPathComponent("products.xlsx")
        try #require(Data(base64Encoded: Self.xlsx)).write(to: sheet)
        await ed.attachFile(sheet).value
        #expect(app.message == nil)
        #expect(ed.data == DataInfo(origin: "products.xlsx", columns: ["Name", "Price"], rowCount: 2))
        #expect(ed.rows == [["Apple", "649"], ["Pear", "899"]])
        // The copy is gone again: the data is in the document.
        #expect(!FileManager.default.fileExists(atPath: dir.appendingPathComponent("imports").path))

        // Semicolons and Windows-1250, as Excel writes CSV in Hungary.
        let csv = picked.appendingPathComponent("termekek.csv")
        try #require("Név;Ár\nKörte;899\nSzőlő;1299\n".data(using: .windowsCP1250)).write(to: csv)
        await ed.attachFile(csv).value
        #expect(ed.data?.columns == ["Név", "Ár"] && ed.rows.last == ["Szőlő", "1299"])
        #expect(ed.dataSourceName == "termekek.csv")
        #expect(app.message == nil)

        // A file that is not a spreadsheet gives a message and keeps the data.
        let bad = picked.appendingPathComponent("bad.xlsx")
        try Data("nope".utf8).write(to: bad)
        await ed.attachFile(bad).value
        #expect(app.message != nil && ed.data?.rowCount == 2)
    }

    @Test func printsTheCurrentRecordOrAll() async throws {
        await app.connect(AppState.simAddress, name: "Sim").value
        let ed = try await editor()
        await ed.addText("x").value
        await ed.attachCsv(Self.csv).value
        await ed.bind(try #require(ed.selected), to: "Name").value
        await ed.setRecord(2).value

        await ed.print(copies: 2, density: 3)?.value
        #expect(ed.progress == PrintProgress(done: 2, total: 2, state: "done", error: nil))
        await ed.print(copies: 2, density: 3, allRecords: true)?.value
        #expect(ed.progress == PrintProgress(done: 6, total: 6, state: "done", error: nil))
        #expect(app.message == nil)
    }

    @Test func exportsIntoTheDataDir() async throws {
        let ed = try await editor()
        await ed.addText("x").value
        let exports = dir.appendingPathComponent("exports")

        // Without data: one file named after the document.
        let name = try #require(ed.doc?.name)
        var files = await ed.export(format: .png, dpi: 300, allRecords: true)
        #expect(files.map(\.lastPathComponent) == ["\(name).png"])
        let image = try #require(CGImageSourceCreateWithURL(files[0] as CFURL, nil).flatMap { CGImageSourceCreateImageAtIndex($0, 0, nil) })
        #expect(image.width == 591) // 50 mm at 300 dpi
        files = await ed.export(format: .pdf, dpi: 203, allRecords: false)
        #expect(files.map(\.lastPathComponent) == ["\(name).pdf"])
        let pdf = try Data(contentsOf: files[0])
        #expect(pdf.starts(with: Data("%PDF".utf8)))
        // The folder is emptied before each export.
        let left = try FileManager.default.contentsOfDirectory(atPath: exports.path)
        #expect(left == ["\(name).pdf"])

        await ed.attachCsv(Self.csv).value
        await ed.bind(try #require(ed.selected), to: "Name").value
        await ed.setRecord(2).value
        // PNG, all: one file per record.
        files = await ed.export(format: .png, dpi: 203, allRecords: true)
        #expect(files.map(\.lastPathComponent) == ["\(name)-1.png", "\(name)-2.png", "\(name)-3.png"])
        #expect(files.allSatisfy { FileManager.default.fileExists(atPath: $0.path) })
        let firstPng = try Data(contentsOf: files[0])
        let second = try Data(contentsOf: files[1])
        #expect(firstPng != second)
        // PNG, current: the previewed record.
        files = await ed.export(format: .png, dpi: 203, allRecords: false)
        #expect(files.map(\.lastPathComponent) == ["\(name).png"])
        let current = try Data(contentsOf: files[0])
        #expect(current == second)
        // PDF: one file; all records make it longer than one.
        files = await ed.export(format: .pdf, dpi: 203, allRecords: false)
        let one = try Data(contentsOf: files[0])
        files = await ed.export(format: .pdf, dpi: 203, allRecords: true)
        let all = try Data(contentsOf: files[0])
        #expect(files.count == 1 && pdfPages(all) == 3 && pdfPages(one) == 1)
        #expect(!ed.exporting && app.message == nil)
    }

    private func pdfPages(_ data: Data) -> Int? {
        CGDataProvider(data: data as CFData).flatMap { CGPDFDocument($0) }?.numberOfPages
    }

    @Test func resizingIsOneCommandAndOneUndoStep() async throws {
        let ed = try await editor()
        await ed.addRect().value
        let rect = try #require(ed.selected)
        #expect(rect.frame == FrameMm(x: 15, y: 9, width: 20, height: 12))

        // Drag the top-left handle 30 pt left and 12 pt up at 6 pt per mm.
        let frame = CanvasDrag.resize(rect, .topLeft).frame(CGSize(width: -30, height: -12), ptPerMm: 6)
        #expect(frame == FrameMm(x: 10, y: 7, width: 25, height: 14))
        await ed.setProps(rect.id, frame.props).value
        #expect(ed.selected?.frame == frame)
        await ed.undo().value
        #expect(ed.selected?.frame == rect.frame)

        // A line keeps a zero height and may run backwards.
        await ed.addLine().value
        let line = try #require(ed.selected)
        #expect(Handle.of(line) == [.start, .end] && line.heightMm == 0)
        let back = CanvasDrag.resize(line, .end).frame(CGSize(width: -40 * 6, height: 0), ptPerMm: 6)
        await ed.setProps(line.id, back.props).value
        #expect(ed.selected?.frame == FrameMm(x: 10, y: 15, width: -10, height: 0))
        #expect(app.message == nil)
    }

    @Test func barcodeChangesAreValidated() async throws {
        let ed = try await editor()
        await ed.addBarcode().value
        let code = try #require(ed.selected)
        #expect(code.symbology == "code128" && code.humanReadable == "below")
        #expect(await ed.codeIssue(code.id) == nil)

        // EAN-13 takes the 12 digits and adds the check digit.
        #expect(await ed.setCode(code.id, ["symbology": "ean13"]) == nil)
        #expect(ed.selected?.symbology == "ean13")
        #expect(Gtin.checkDigit(symbology: "ean13", data: ed.selected?.data ?? "") == 8)

        // Letters cannot be EAN-13: the engine's reason comes back and the value stays.
        let problem = await ed.setCode(code.id, ["data": "ABC"])
        #expect(problem?.isEmpty == false)
        #expect(ed.selected?.data == "123456789012" && ed.selected?.symbology == "ean13")
        #expect(ed.doc?.canRedo == false, "a rejected value leaves no history")
        #expect(await ed.codeIssue(code.id) == nil)

        // A 1D element cannot become a 2D one.
        #expect(await ed.setCode(code.id, ["symbology": "qr"]) != nil)
        #expect(ed.selected?.symbology == "ean13")

        #expect(await ed.setCode(code.id, ["human_readable": "none"]) == nil)
        #expect(ed.selected?.humanReadable == "none")
        #expect(await ed.setCode(code.id, ["symbology": "code39", "data": "ABC-1"]) == nil)
        #expect(ed.selected?.data == "ABC-1")

        // A bound value depends on the record, so it is kept.
        await ed.attachCsv(Self.csv).value
        await ed.bind(try #require(ed.selected), to: "EAN").value
        #expect(ed.selected?.data == "{{EAN}}")
        #expect(await ed.setCode(code.id, ["symbology": "ean13"]) == nil)
        #expect(ed.selected?.symbology == "ean13" && ed.selected.flatMap(ed.boundColumn) == "EAN")

        await ed.addQr().value
        let qr = try #require(ed.selected)
        #expect(qr.symbology == "qr")
        #expect(await ed.setCode(qr.id, ["symbology": "data_matrix"]) == nil)
        #expect(ed.selected?.symbology == "data_matrix")
        #expect(app.message == nil)
    }

    @Test func insertsPicturesAndSetsTheirConversion() async throws {
        let ed = try await editor()
        await ed.addImage(try png(width: 2000, height: 1000)).value
        let image = try #require(ed.selected)
        #expect(image.type == "image")
        #expect(image.frame == FrameMm(x: 1, y: 3, width: 48, height: 24))
        #expect(image.dither == "floyd_steinberg" && image.threshold == 128 && !image.invert)
        #expect(!ed.canBind(image))

        for mode in Symbology.dither {
            await ed.setProps(image.id, ["dither": JSON(mode)]).value
            #expect(ed.selected?.dither == mode)
        }
        let before = pixels(ed.preview)
        await ed.setProps(image.id, ["threshold": 200, "invert": true]).value
        #expect(ed.selected?.threshold == 200 && ed.selected?.invert == true)
        #expect(pixels(ed.preview) != before)
        #expect(app.message == nil)

        await ed.addImage(Data("not an image".utf8)).value
        #expect(app.message != nil && ed.doc?.elements.count == 1)
    }

    @Test func readsAndWritesPrinterSettings() async throws {
        #expect(app.printerSettings == nil)
        await app.connect(AppState.simAddress, name: "Sim").value
        await app.loadPrinterSettings().value
        let s = try #require(app.printerSettings)
        #expect(!s.isEmpty)
        let level = try #require(s.autoOffLevel)
        #expect(PrinterSettings.autoOffLevels.contains(level))

        let next = level == 4 ? 1 : level + 1
        await app.setPrinterSettings(autoOffLevel: next).value
        #expect(app.printerSettings?.autoOffLevel == next)
        #expect(app.printerSettings?.soundPower == s.soundPower)
        if let power = s.soundPower {
            await app.setPrinterSettings(soundPower: !power).value
            #expect(app.printerSettings?.soundPower == !power)
        }
        if let bt = s.soundBluetooth {
            await app.setPrinterSettings(soundBluetooth: !bt).value
            #expect(app.printerSettings?.soundBluetooth == !bt)
        }
        // Read back from the printer.
        let written = app.printerSettings
        await app.loadPrinterSettings().value
        #expect(app.printerSettings == written)
        #expect(app.message == nil)

        await app.disconnect().value
        #expect(app.printerSettings == nil)
    }
}
