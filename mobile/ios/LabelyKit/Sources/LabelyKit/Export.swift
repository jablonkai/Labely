import CoreGraphics
import Foundation
import ImageIO
import UniformTypeIdentifiers

public enum ExportFormat: String, Sendable, CaseIterable {
    case png, pdf
}

/// Export file naming. The engine writes PNG and PDF only, as the 1-bit print raster.
public enum Export {
    public static let dpis = [203, 300, 600]
    public static let defaultDpi = 300

    /// The document name as a file name: path separators and control characters become `-`.
    public static func baseName(_ docName: String) -> String {
        let bad = CharacterSet(charactersIn: "/\\:*?\"<>|").union(.controlCharacters).union(.newlines)
        let name = String(docName.unicodeScalars.map { bad.contains($0) ? "-" : Character($0) })
            .trimmingCharacters(in: .whitespaces.union(CharacterSet(charactersIn: ".")))
        return name.isEmpty ? "label" : name
    }

    /// `<doc name>.<ext>`, or `<doc name>-<record>.<ext>` for one file of a record series.
    public static func fileName(docName: String, record: Int? = nil, format: ExportFormat) -> String {
        baseName(docName) + (record.map { "-\($0)" } ?? "") + "." + format.rawValue
    }
}

extension EditorState {
    /// Records an export of everything writes: the rows of the data, 1 without data.
    public var recordCount: Int { max(data?.rowCount ?? 1, 1) }

    /// Write the label into `<data dir>/exports` (emptied first; the engine may write only
    /// below its data directory) and return the files, for the share sheet or the save picker.
    /// PDF is one file with a page per record; PNG with `allRecords` is one file per record.
    /// A failure becomes the toast message and gives no files.
    public func export(format: ExportFormat, dpi: Int, allRecords: Bool) async -> [URL] {
        guard let doc, !exporting else { return [] }
        exporting = true
        defer { exporting = false }
        let fm = FileManager.default
        let dir = engine.dataDir.appendingPathComponent("exports", isDirectory: true)
        let hasData = (data?.rowCount ?? 0) > 0
        var files: [URL] = []
        await app.report {
            try? fm.removeItem(at: dir)
            try fm.createDirectory(at: dir, withIntermediateDirectories: true)
            switch format {
            case .pdf:
                let url = dir.appendingPathComponent(Export.fileName(docName: doc.name, format: .pdf))
                try await engine.call("export.pdf", [
                    "doc_id": JSON(docId), "path": JSON(url.path), "dpi": JSON(dpi),
                    "records": JSON(recordsParam(all: allRecords)), "count": hasData ? nil : 1,
                ])
                files = [url]
            case .png:
                let series = allRecords && hasData
                var out: [URL] = []
                for n in series ? Array(1...recordCount) : [record] {
                    let url = dir.appendingPathComponent(Export.fileName(docName: doc.name, record: series ? n : nil, format: .png))
                    try await engine.call("export.png", [
                        "doc_id": JSON(docId), "path": JSON(url.path), "dpi": JSON(dpi), "record": hasData ? JSON(n) : nil,
                    ])
                    out.append(url)
                }
                files = out
            }
        }
        return files
    }
}

/// A picked photo made ready for `element.add_image`.
public enum ImageImport {
    /// The longer side of an embedded image, in pixels.
    public static let maxPixels = 1024

    public struct Prepared: Sendable, Equatable {
        public let png: Data
        public let width: Int
        public let height: Int
    }

    /// Decode `data` (any format ImageIO reads), apply its orientation, scale it down so the
    /// longer side is at most `maxPixels`, and encode it as PNG.
    public static func prepare(_ data: Data) throws -> Prepared {
        let options: [CFString: Any] = [
            kCGImageSourceCreateThumbnailFromImageAlways: true,
            kCGImageSourceCreateThumbnailWithTransform: true,
            kCGImageSourceThumbnailMaxPixelSize: maxPixels,
        ]
        guard let src = CGImageSourceCreateWithData(data as CFData, nil),
              let image = CGImageSourceCreateThumbnailAtIndex(src, 0, options as CFDictionary)
        else { throw CommandFailure(code: "bad_params", message: "cannot read the image", hint: nil) }
        let out = NSMutableData()
        guard let dest = CGImageDestinationCreateWithData(out, UTType.png.identifier as CFString, 1, nil) else {
            throw CommandFailure(code: "internal", message: "cannot encode the image", hint: nil)
        }
        CGImageDestinationAddImage(dest, image, nil)
        guard CGImageDestinationFinalize(dest) else {
            throw CommandFailure(code: "internal", message: "cannot encode the image", hint: nil)
        }
        return Prepared(png: out as Data, width: image.width, height: image.height)
    }

    /// The largest frame with the picture's aspect ratio inside the label, 1 mm from its edges,
    /// centred.
    public static func frame(pixelWidth: Int, pixelHeight: Int, labelWidthMm: Double, labelHeightMm: Double) -> FrameMm {
        let maxW = max(labelWidthMm - 2, Handle.minMm)
        let maxH = max(labelHeightMm - 2, Handle.minMm)
        let ratio = Double(max(pixelWidth, 1)) / Double(max(pixelHeight, 1))
        var w = maxW
        var h = w / ratio
        if h > maxH {
            h = maxH
            w = h * ratio
        }
        w = max(round1(w), 0.1)
        h = max(round1(h), 0.1)
        return FrameMm(x: round1((labelWidthMm - w) / 2), y: round1((labelHeightMm - h) / 2), width: w, height: h)
    }
}

extension EditorState {
    /// Add a picked image: scaled down, embedded as PNG, selected.
    @discardableResult
    public func addImage(_ data: Data) -> Task<Void, Never> {
        Task {
            guard let label = doc?.label else { return }
            var prepared: ImageImport.Prepared?
            // Decoding a photo takes a moment: off the main actor.
            await app.report { prepared = try await Task.detached { try ImageImport.prepare(data) }.value }
            guard let p = prepared else { return }
            let frame = ImageImport.frame(pixelWidth: p.width, pixelHeight: p.height, labelWidthMm: label.widthMm, labelHeightMm: label.heightMm)
            await exec(
                "element.add_image",
                frame.props.with("data_base64", JSON(p.png.base64EncodedString())).with("mime", "image/png"),
                select: true
            ).value
        }
    }
}

extension EditorState {
    /// A copy of the document as `<name>.labely` in `<data dir>/exports`, for the system's
    /// "save to" picker or the share sheet (Save a copy / Save As). `nil` after a failure.
    public func exportCopy() async -> URL? {
        guard let doc else { return nil }
        var out: URL?
        await app.report {
            if !saved || doc.path == nil { try await save() }
            guard let path = self.doc?.path else { return }
            let fm = FileManager.default
            let dir = engine.dataDir.appendingPathComponent("exports", isDirectory: true)
            try? fm.removeItem(at: dir)
            try fm.createDirectory(at: dir, withIntermediateDirectories: true)
            let url = dir.appendingPathComponent(Export.baseName(doc.name) + ".labely")
            try fm.copyItem(atPath: path, toPath: url.path)
            out = url
        }
        return out
    }
}
