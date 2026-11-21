import Foundation

/// `{{name}}` bindings in an element's string, as the engine's `interpolate` reads them.
public enum Bindings {
    /// The property that holds an element's bindable string, as `data.bind` picks it; `nil`
    /// for kinds that cannot be bound.
    public static func property(of type: String) -> String? {
        switch type {
        case "text": "text"
        case "barcode", "code2d": "data"
        default: nil
        }
    }

    public static func string(of e: Element) -> String? {
        property(of: e.type).flatMap { e.raw[$0]?.string }
    }

    /// The binding names in `s`, in order.
    public static func names(in s: String) -> [String] {
        var out: [String] = []
        replacing(in: s) { name in
            out.append(name)
            return nil
        }
        return out
    }

    /// `s` with each `{{name}}` replaced by `value(name)`; `nil` keeps the binding.
    @discardableResult
    public static func replacing(in s: String, _ value: (String) -> String?) -> String {
        var out = ""
        var rest = Substring(s)
        while let open = rest.range(of: "{{") {
            out += rest[..<open.lowerBound]
            let after = rest[open.upperBound...]
            guard let close = after.range(of: "}}") else {
                out += rest[open.lowerBound...]
                rest = ""
                break
            }
            let name = after[..<close.lowerBound].trimmingCharacters(in: .whitespaces)
            out += value(name) ?? String(rest[open.lowerBound..<close.upperBound])
            rest = after[close.upperBound...]
        }
        return out + rest
    }

    /// The first of `columns` that `e` is bound to.
    public static func column(of e: Element, columns: [String]) -> String? {
        string(of: e).flatMap { names(in: $0).first(where: columns.contains) }
    }
}

/// Modulo-10 check digits of EAN-13, EAN-8 and UPC-A, as the studio's inspector shows them.
public enum Gtin {
    static let lengths = ["ean13": 13, "ean8": 8, "upc_a": 12]

    /// The check digit of `data` when it is complete: all digits, with or without the last one.
    /// `nil` for other symbologies, bound or incomplete data, and a wrong last digit.
    public static func checkDigit(symbology: String, data: String) -> Int? {
        guard let len = lengths[symbology] else { return nil }
        let digits = data.compactMap(\.wholeNumberValue)
        guard digits.count == data.count, data.allSatisfy(\.isASCII), digits.count == len || digits.count == len - 1 else { return nil }
        var sum = 0
        for (i, d) in digits.prefix(len - 1).reversed().enumerated() {
            sum += i % 2 == 0 ? d * 3 : d
        }
        let check = (10 - sum % 10) % 10
        return digits.count == len && digits[len - 1] != check ? nil : check
    }
}

/// The symbologies of the engine (`Symbology1d`, `Symbology2d`), the common ones first.
public enum Symbology {
    public static let oneD = [
        "ean13", "code128", "code39", "ean8", "upc_a", "upc_e", "itf", "code93", "codabar",
        "gs1_128", "code128_a", "code128_b", "code128_c",
    ]
    public static let twoD = ["qr", "data_matrix", "pdf417", "aztec"]
    /// `human_readable` of a barcode.
    public static let humanReadable = ["none", "below", "above"]
    /// `dither` of an image.
    public static let dither = ["floyd_steinberg", "atkinson", "bayer", "threshold"]

    /// The symbologies an element of `type` may switch among.
    public static func of(_ type: String) -> [String] {
        switch type {
        case "barcode": oneD
        case "code2d": twoD
        default: []
        }
    }

    /// Symbology names are the same in every language.
    public static func name(_ id: String) -> String {
        switch id {
        case "ean13": "EAN-13"
        case "ean8": "EAN-8"
        case "upc_a": "UPC-A"
        case "upc_e": "UPC-E"
        case "code128": "Code 128"
        case "code128_a": "Code 128 A"
        case "code128_b": "Code 128 B"
        case "code128_c": "Code 128 C"
        case "gs1_128": "GS1-128"
        case "code39": "Code 39"
        case "code93": "Code 93"
        case "itf": "ITF"
        case "codabar": "Codabar"
        case "qr": "QR"
        case "data_matrix": "Data Matrix"
        case "pdf417": "PDF417"
        case "aztec": "Aztec"
        default: id
        }
    }
}

extension Element {
    public var symbology: String { raw["symbology"]?.string ?? (type == "code2d" ? "qr" : "code128") }
    public var humanReadable: String { raw["human_readable"]?.string ?? "below" }
    public var dither: String { raw["dither"]?.string ?? "floyd_steinberg" }
    public var threshold: Int { raw["threshold"]?.int ?? 128 }
    public var invert: Bool { raw["invert"]?.bool ?? false }
}

/// Data merge: the attached source, the previewed record, bindings and code validation.
extension EditorState {
    static let pageRows = 50
    /// Spreadsheets go through a file in the data directory; CSV is attached as text.
    static let sheetExtensions: Set<String> = ["xlsx", "xlsm", "xls", "ods"]

    /// Name of the data source for the panel's header.
    public var dataSourceName: String? { data?.origin ?? dataName }

    /// How many columns some element is bound to.
    public var boundColumns: Int {
        guard let data, let doc else { return 0 }
        var used = Set<String>()
        for e in doc.elements {
            for n in Bindings.string(of: e).map(Bindings.names(in:)) ?? [] where data.columns.contains(n) { used.insert(n) }
        }
        return used.count
    }

    /// The column `e` is bound to, if it can be bound.
    public func boundColumn(_ e: Element) -> String? {
        Bindings.column(of: e, columns: data?.columns ?? [])
    }

    /// Whether `e` may get a binding: `data.bind` takes text, barcodes and 2D codes.
    public func canBind(_ e: Element) -> Bool {
        data != nil && Bindings.property(of: e.type) != nil
    }

    /// Read the data source and the first rows, and keep the previewed record in range.
    func refreshData() async throws {
        let r = try await engine.call("data.get", ["doc_id": JSON(docId), "from": 1, "limit": JSON(Self.pageRows)])
        let info = DataInfo.from(r)
        let first = DataInfo.rows(r)
        // Loaded pages stay while the data is the same.
        if info != data || Array(rows.prefix(first.count)) != first { rows = first }
        if info == nil {
            dataName = nil
            dataFile = nil
        }
        serials = Serial.list(r)
        // Other data starts at its first record.
        if info != data { record = 1 }
        data = info
        record = min(max(record, 1), max(info?.rowCount ?? 1, 1))
    }

    /// Load the next page of rows, if there are more.
    @discardableResult
    public func loadMoreRows() -> Task<Void, Never> {
        Task {
            guard let data, rows.count < data.rowCount else { return }
            let from = rows.count + 1
            await app.report {
                let r = try await engine.call("data.get", ["doc_id": JSON(docId), "from": JSON(from), "limit": JSON(Self.pageRows)])
                // An edit may have replaced the rows meanwhile.
                if rows.count + 1 == from { rows += DataInfo.rows(r) }
            }
        }
    }

    /// Preview record `n` (1-based, clamped): the canvas shows it and Print → Current prints it.
    @discardableResult
    public func setRecord(_ n: Int) -> Task<Void, Never> {
        let n = min(max(n, 1), max(data?.rowCount ?? 1, 1))
        guard n != record else { return Task {} }
        record = n
        return Task {
            await app.report {
                let image = try await render()
                if record == n { preview = image }
            }
        }
    }

    /// `records` of `print.submit` and `export.pdf`: the previewed record or all; `nil`
    /// without data, where it is not allowed.
    func recordsParam(all: Bool) -> String? {
        guard let data, data.rowCount > 0 else { return nil }
        return all ? "all" : String(record)
    }

    /// Attach CSV text (`data.attach_csv {text}`); `name` is shown as the source.
    @discardableResult
    public func attachCsv(_ text: String, name: String? = nil) -> Task<Void, Never> {
        cleanUp()
        return attach(DataFile(kind: .csv(text), name: name))
    }

    /// Attach the file the user picked: CSV, TSV and TXT as text; a spreadsheet is copied into
    /// `<data dir>/imports`, the only place the engine may read, and attached by path. The data
    /// is embedded in the document; the copy is removed when the editor closes. `url` may be
    /// security-scoped.
    @discardableResult
    public func attachFile(_ url: URL) -> Task<Void, Never> {
        Task {
            let name = url.lastPathComponent
            let scoped = url.startAccessingSecurityScopedResource()
            defer { if scoped { url.stopAccessingSecurityScopedResource() } }
            if Self.sheetExtensions.contains(url.pathExtension.lowercased()) {
                let dir = engine.dataDir.appendingPathComponent("imports", isDirectory: true)
                let copy = dir.appendingPathComponent(name)
                var copied = false
                await app.report {
                    try? FileManager.default.removeItem(at: dir)
                    try FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
                    try FileManager.default.copyItem(at: url, to: copy)
                    copied = true
                }
                // The copy stays for the session, so the sheet and the header row can be changed.
                if copied { await attach(DataFile(kind: .sheet(copy), name: name)).value }
                // A file the engine could not read is not kept.
                if dataFile?.kind != .sheet(copy) { try? FileManager.default.removeItem(at: dir) }
            } else {
                var text: String?
                await app.report { text = try Self.readText(url) }
                if let text { await attachCsv(text, name: name).value }
            }
        }
    }

    /// The text of a CSV file: UTF-8 (the engine drops a BOM), UTF-16 with a BOM, else Windows-1250,
    /// which Hungarian spreadsheet exports use.
    static func readText(_ url: URL) throws -> String {
        let bytes = try Data(contentsOf: url)
        if let s = String(data: bytes, encoding: .utf8) { return s }
        if bytes.starts(with: [0xFF, 0xFE]) || bytes.starts(with: [0xFE, 0xFF]), let s = String(data: bytes, encoding: .utf16) { return s }
        if let s = String(data: bytes, encoding: .windowsCP1250) { return s }
        throw CommandFailure(code: "bad_params", message: "\(url.lastPathComponent) is not text", hint: nil)
    }

    @discardableResult
    public func detachData() -> Task<Void, Never> {
        cleanUp()
        return exec("data.detach")
    }

    /// Bind `e` to `column` (`data.bind`). `nil` removes the binding: the binding is
    /// `{{column}}` in the element's string, so the string is set to what the previewed record
    /// shows there.
    @discardableResult
    public func bind(_ e: Element, to column: String?) -> Task<Void, Never> {
        if let column {
            return exec("data.bind", ["id": .int(e.id), "column": JSON(column)])
        }
        guard let property = Bindings.property(of: e.type), let s = e.raw[property]?.string, let columns = data?.columns else {
            return Task {}
        }
        return Task {
            var values: [String: JSON] = [:]
            await app.report {
                values = try await engine.call("data.preview_record", ["doc_id": JSON(docId), "record": JSON(record)])["values"]?.object ?? [:]
            }
            let plain = Bindings.replacing(in: s) { name in
                columns.contains(name) ? values[name]?.string ?? "" : nil
            }
            if plain != s { await setProps(e.id, [property: JSON(plain)]).value }
        }
    }

    /// The reason the engine cannot encode the barcode or 2D code `id` (`doc.check`'s
    /// `encode_failed`), or `nil` when it is valid.
    public func codeIssue(_ id: Int64) async -> String? {
        guard let r = try? await engine.call("doc.check", ["doc_id": JSON(docId)]) else { return nil }
        guard let issue = CheckIssue.list(r).first(where: { $0.rule == "encode_failed" && $0.elementId == id }) else { return nil }
        // "element 3 cannot be encoded: <reason>"
        if let cut = issue.message.range(of: "cannot be encoded: ") { return String(issue.message[cut.upperBound...]) }
        return issue.message
    }

    /// The reason `data` cannot be encoded as `symbology` in the frame of `e`, or `nil`.
    /// `element.set_props` takes any code data and the engine reports a bad value only in
    /// `doc.check`, so the value is tried on a scratch document. A rejected value never
    /// enters this document or its undo history.
    private func encodeError(_ e: Element, symbology: String, data: String) async throws -> String? {
        guard let l = doc?.label else { return nil }
        let r = try await engine.call("doc.new", ["width_mm": JSON(l.widthMm), "height_mm": JSON(l.heightMm)])
        guard let scratch = r["doc_id"]?.string else { return nil }
        do {
            try await engine.call(e.type == "barcode" ? "element.add_barcode" : "element.add_code2d", [
                "doc_id": JSON(scratch), "x_mm": JSON(e.xMm), "y_mm": JSON(e.yMm),
                "width_mm": JSON(e.widthMm), "height_mm": JSON(e.heightMm),
                "data": JSON(data), "symbology": JSON(symbology),
            ])
            let check = try await engine.call("doc.check", ["doc_id": JSON(scratch)])
            _ = try? await engine.call("doc.close", ["doc_id": JSON(scratch)])
            guard let issue = CheckIssue.list(check).first(where: { $0.rule == "encode_failed" }) else { return nil }
            if let cut = issue.message.range(of: "cannot be encoded: ") { return String(issue.message[cut.upperBound...]) }
            return issue.message
        } catch {
            _ = try? await engine.call("doc.close", ["doc_id": JSON(scratch)])
            throw error
        }
    }

    /// Set properties of a barcode or 2D code. A value or symbology that the engine cannot
    /// encode is not applied, and the engine's message is returned. A bound value depends on
    /// the record, so it is applied unchecked.
    public func setCode(_ id: Int64, _ props: JSON) async -> String? {
        var problem: String?
        await app.report {
            do {
                if let e = doc?.elements.first(where: { $0.id == id }),
                   props["data"] != nil || props["symbology"] != nil {
                    let data = props["data"]?.string ?? e.data ?? ""
                    let symbology = props["symbology"]?.string ?? e.symbology
                    if Bindings.names(in: data).isEmpty,
                       let issue = try await encodeError(e, symbology: symbology, data: data) {
                        problem = issue
                        return
                    }
                }
                try await engine.call("element.set_props", ["doc_id": JSON(docId), "id": .int(id), "props": props])
            } catch let e as CommandFailure {
                problem = e.localizedDescription
                return
            }
            saved = false
            try await refresh()
            try await save()
        }
        return problem
    }
}

/// The roll in the printer and the label paper library, for the open document.
extension EditorState {
    /// Read the roll in the connected printer and compare it with this label (`paper.detect`).
    /// With "use the loaded paper by itself" on, a known roll of another size is applied once.
    public func refreshLoadedPaper() async {
        guard let p = app.printer else {
            loadedPaper = nil
            return
        }
        guard let r = try? await engine.call("paper.detect", ["address": JSON(p.address), "doc_id": JSON(docId)]) else {
            loadedPaper = nil
            return
        }
        let d = PaperDetect.from(r)
        loadedPaper = d
        if app.prefs.autoApplyPaper, d.matchesLabel == false, let paper = d.paper, let barcode = d.barcode, autoApplied != barcode {
            autoApplied = barcode
            await applyPaper(paper.id).value
        }
    }

    /// Give the label the size, corners and media of paper `id` (`paper.apply`).
    @discardableResult
    public func applyPaper(_ id: String) -> Task<Void, Never> {
        Task {
            await exec("paper.apply", ["id": JSON(id)]).value
            if app.printer != nil, let r = try? await engine.call("paper.detect", ["address": JSON(app.printer?.address), "doc_id": JSON(docId)]) {
                loadedPaper = PaperDetect.from(r)
            }
        }
    }
}
