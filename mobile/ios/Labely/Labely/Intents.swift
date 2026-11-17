import AppIntents
import LabelyKit

// The six assistant functions of ADR-0011 as App Intents (ADR-0033). The logic is
// `LabelyKit.Assistant`, shared with the tests; these types only describe the functions to the
// system and turn results and errors into what Shortcuts and Siri show. Every resource key
// starts with `intent_` so `cargo xtask i18n-check` finds it in `Localizable.xcstrings`.

// MARK: - Entities

struct LabelEntity: AppEntity {
    static let typeDisplayRepresentation = TypeDisplayRepresentation(name: "intent_t_label")
    static let defaultQuery = LabelQuery()

    /// The file name in the label folder.
    let id: String
    @Property(title: "intent_prop_name") var name: String
    @Property(title: "intent_p_width") var widthMm: Double?
    @Property(title: "intent_p_height") var heightMm: Double?
    @Property(title: "intent_prop_warnings") var warnings: [String]

    init(id: String, name: String, widthMm: Double?, heightMm: Double?, warnings: [String] = []) {
        self.id = id
        self.name = name
        self.widthMm = widthMm
        self.heightMm = heightMm
        self.warnings = warnings
    }

    init(_ l: SavedLabel) {
        self.init(id: l.labelId, name: l.name, widthMm: l.widthMm, heightMm: l.heightMm, warnings: l.warnings)
    }

    init(_ f: SavedLabelFile) {
        self.init(id: f.labelId, name: f.name, widthMm: f.widthMm, heightMm: f.heightMm)
    }

    var displayRepresentation: DisplayRepresentation {
        guard let w = widthMm, let h = heightMm else { return DisplayRepresentation(title: "\(name)") }
        return DisplayRepresentation(title: "\(name)", subtitle: "\(sizeText(w, h))")
    }
}

struct LabelQuery: EntityQuery {
    @MainActor
    func entities(for identifiers: [String]) async throws -> [LabelEntity] {
        let saved = Shared.assistant.savedLabels()
        return identifiers.compactMap { id in saved.first { $0.labelId == id }.map(LabelEntity.init) }
    }

    @MainActor
    func suggestedEntities() async throws -> [LabelEntity] {
        Shared.assistant.savedLabels().map(LabelEntity.init)
    }
}

struct TemplateEntity: AppEntity {
    static let typeDisplayRepresentation = TypeDisplayRepresentation(name: "intent_t_template")
    static let defaultQuery = TemplateQuery()

    /// The template ref, such as `starter:price-tag`.
    let id: String
    @Property(title: "intent_prop_name") var name: String
    @Property(title: "intent_prop_folder") var folder: String
    @Property(title: "intent_p_width") var widthMm: Double
    @Property(title: "intent_p_height") var heightMm: Double
    /// The field names, for `name=value` lines.
    @Property(title: "intent_p_fields") var fields: [String]

    init(_ t: LabelTemplate) {
        id = t.templateId
        name = t.name
        folder = t.folder
        widthMm = t.widthMm
        heightMm = t.heightMm
        fields = t.fields.map(\.name)
    }

    var displayRepresentation: DisplayRepresentation {
        DisplayRepresentation(title: "\(name)", subtitle: "\(sizeText(widthMm, heightMm))")
    }
}

struct TemplateQuery: EntityStringQuery {
    @MainActor
    func entities(for identifiers: [String]) async throws -> [TemplateEntity] {
        let all = try await intentCall { try await Shared.assistant.listTemplates() }
        return identifiers.compactMap { id in all.first { $0.templateId == id }.map(TemplateEntity.init) }
    }

    @MainActor
    func entities(matching string: String) async throws -> [TemplateEntity] {
        try await intentCall { try await Shared.assistant.listTemplates(query: string) }.map(TemplateEntity.init)
    }

    @MainActor
    func suggestedEntities() async throws -> [TemplateEntity] {
        try await intentCall { try await Shared.assistant.listTemplates() }.map(TemplateEntity.init)
    }
}

struct PrinterEntity: AppEntity {
    static let typeDisplayRepresentation = TypeDisplayRepresentation(name: "intent_t_printer")
    static let defaultQuery = PrinterQuery()

    /// The printer address, such as `ble:<uuid>`.
    let id: String
    @Property(title: "intent_prop_name") var name: String
    @Property(title: "intent_prop_connected") var connected: Bool
    @Property(title: "intent_prop_model") var model: String
    /// Set by "Get Printer Status".
    @Property(title: "intent_prop_battery") var batteryPercent: Int?
    @Property(title: "intent_prop_paper") var paper: String?

    init(id: String, name: String, connected: Bool, model: String, batteryPercent: Int? = nil, paper: String? = nil) {
        self.id = id
        self.name = name
        self.connected = connected
        self.model = model
        self.batteryPercent = batteryPercent
        self.paper = paper
    }

    init(_ p: PrinterInfo) {
        self.init(id: p.printerId, name: p.name, connected: p.connected, model: p.model)
    }

    var displayRepresentation: DisplayRepresentation {
        DisplayRepresentation(title: "\(name.isEmpty ? id : name)", subtitle: model.isEmpty ? nil : "\(model)")
    }
}

struct PrinterQuery: EntityQuery {
    /// Printers the app does not list (yet) still resolve, so a saved shortcut keeps working.
    @MainActor
    func entities(for identifiers: [String]) async throws -> [PrinterEntity] {
        let all = try await intentCall { try await Shared.assistant.listPrinters() }
        return identifiers.map { id in
            all.first { $0.printerId == id }.map(PrinterEntity.init) ?? PrinterEntity(id: id, name: id, connected: false, model: "")
        }
    }

    @MainActor
    func suggestedEntities() async throws -> [PrinterEntity] {
        try await intentCall { try await Shared.assistant.listPrinters() }.map(PrinterEntity.init)
    }
}

// MARK: - Intents

struct CreateLabelFromTextIntent: AppIntent {
    static let title: LocalizedStringResource = "intent_text_title"
    static let description = IntentDescription("intent_text_desc")

    @Parameter(title: "intent_p_text", inputOptions: String.IntentInputOptions(multiline: true))
    var text: String

    @Parameter(title: "intent_p_width", default: 50, inclusiveRange: (10, 120))
    var widthMm: Double

    @Parameter(title: "intent_p_height", default: 30, inclusiveRange: (5, 120))
    var heightMm: Double

    @MainActor
    func perform() async throws -> some IntentResult & ReturnsValue<LabelEntity> & ProvidesDialog {
        let label = try await intentCall { try await Shared.assistant.createLabelFromText(text, widthMm: widthMm, heightMm: heightMm) }
        return .result(value: LabelEntity(label), dialog: savedDialog(label))
    }
}

struct CreateLabelFromTemplateIntent: AppIntent {
    static let title: LocalizedStringResource = "intent_template_title"
    static let description = IntentDescription("intent_template_desc")

    @Parameter(title: "intent_p_template")
    var template: TemplateEntity

    /// `name=value` lines; fields not given keep the template's sample value.
    @Parameter(title: "intent_p_fields")
    var fields: [String]?

    @MainActor
    func perform() async throws -> some IntentResult & ReturnsValue<LabelEntity> & ProvidesDialog {
        var values: [FieldValue] = []
        for line in fields ?? [] where !line.trimmingCharacters(in: .whitespaces).isEmpty {
            guard let v = FieldValue.parse(line) else { throw IntentFailure(L("intent_err_field_line", line)) }
            values.append(v)
        }
        let label = try await intentCall { try await Shared.assistant.createLabelFromTemplate(template.id, fields: values) }
        return .result(value: LabelEntity(label), dialog: savedDialog(label))
    }
}

struct ListTemplatesIntent: AppIntent {
    static let title: LocalizedStringResource = "intent_templates_title"
    static let description = IntentDescription("intent_templates_desc")

    @Parameter(title: "intent_p_query")
    var query: String?

    @MainActor
    func perform() async throws -> some IntentResult & ReturnsValue<[TemplateEntity]> {
        .result(value: try await intentCall { try await Shared.assistant.listTemplates(query: query) }.map(TemplateEntity.init))
    }
}

struct ListPrintersIntent: AppIntent {
    static let title: LocalizedStringResource = "intent_printers_title"
    static let description = IntentDescription("intent_printers_desc")

    @Parameter(title: "intent_p_scan", default: false)
    var scan: Bool

    @MainActor
    func perform() async throws -> some IntentResult & ReturnsValue<[PrinterEntity]> {
        .result(value: try await intentCall { try await Shared.assistant.listPrinters(scan: scan) }.map(PrinterEntity.init))
    }
}

struct GetPrinterStatusIntent: AppIntent {
    static let title: LocalizedStringResource = "intent_status_title"
    static let description = IntentDescription("intent_status_desc")

    /// No printer uses the connected one, else the only known one.
    @Parameter(title: "intent_p_printer")
    var printer: PrinterEntity?

    @MainActor
    func perform() async throws -> some IntentResult & ReturnsValue<PrinterEntity> & ProvidesDialog {
        let s = try await intentCall {
            guard let id = printer?.id ?? Shared.assistant.defaultPrinterId() else { throw AssistantError.noPrinter }
            return try await Shared.assistant.printerStatus(id)
        }
        let paper = switch s.paperPresent {
        case true?: L("paper_loaded")
        case false?: L("paper_missing")
        case nil: L("intent_unknown")
        }
        let battery = s.batteryPercent.map { "\($0)%" } ?? L("intent_unknown")
        let entity = PrinterEntity(id: s.printerId, name: s.name, connected: true, model: s.model, batteryPercent: s.batteryPercent, paper: paper)
        return .result(value: entity, dialog: dialog(L("intent_status_dialog", s.name, battery, paper)))
    }
}

struct PrintLabelIntent: AppIntent {
    static let title: LocalizedStringResource = "intent_print_title"
    static let description = IntentDescription("intent_print_desc")

    @Parameter(title: "intent_p_label")
    var label: LabelEntity

    /// No printer uses the connected one, else the only known one.
    @Parameter(title: "intent_p_printer")
    var printer: PrinterEntity?

    @Parameter(title: "intent_p_copies", default: 1, inclusiveRange: (1, 999))
    var copies: Int

    @MainActor
    func perform() async throws -> some IntentResult & ProvidesDialog {
        let r = try await intentCall { try await Shared.assistant.printLabel(label.id, printerId: printer?.id, copies: copies) }
        return .result(dialog: dialog(L("intent_printed_dialog", label.name, r.pagesPrinted, r.pagesTotal)))
    }
}

// MARK: - Shortcuts

struct LabelyShortcuts: AppShortcutsProvider {
    static var appShortcuts: [AppShortcut] {
        AppShortcut(
            intent: CreateLabelFromTextIntent(),
            phrases: ["Make a label in \(.applicationName)", "Create a \(.applicationName) label"],
            shortTitle: "intent_short_text",
            systemImageName: "textformat"
        )
        AppShortcut(
            intent: PrintLabelIntent(),
            phrases: ["Print a label with \(.applicationName)"],
            shortTitle: "intent_print_title",
            systemImageName: "printer"
        )
        AppShortcut(
            intent: GetPrinterStatusIntent(),
            phrases: ["Check my \(.applicationName) printer"],
            shortTitle: "intent_status_title",
            systemImageName: "battery.75percent"
        )
    }
}

// MARK: - Results and errors

/// A message Shortcuts and Siri show when an intent fails.
struct IntentFailure: Error, CustomLocalizedStringResourceConvertible {
    let message: String

    init(_ message: String) { self.message = message }

    var localizedStringResource: LocalizedStringResource { LocalizedStringResource(stringLiteral: message) }
}

/// Run an assistant function; its errors become localised messages.
@MainActor
func intentCall<T>(_ body: () async throws -> T) async throws -> T {
    do {
        return try await body()
    } catch let e as AssistantError {
        throw IntentFailure(message(e))
    }
}

private func message(_ e: AssistantError) -> String {
    switch e {
    case .emptyText: L("intent_err_empty_text")
    case .sizeOutOfRange(let w, let h): L("intent_err_size", sizeText(w, h))
    case .noTemplate(let id): L("intent_err_no_template", id)
    case .noField(let field, let fields): L("intent_err_no_field", field, fields.joined(separator: ", "))
    case .noLabel(let id): L("intent_err_no_label", id)
    case .noPrinter: L("intent_err_no_printer")
    case .badCopies(let max): L("intent_err_copies", max)
    case .printNotAllowed: L("intent_err_print_consent", L("settings_agent_print"))
    case .printTimeout: L("intent_err_timeout")
    case .printFailed(_, let reason): L("intent_err_print_failed", reason)
    case .bluetooth: L("intent_err_bluetooth")
    case .unreachable: L("intent_err_unreachable")
    case .engine(_, let message): message
    }
}

private func dialog(_ text: String) -> IntentDialog {
    IntentDialog(LocalizedStringResource(stringLiteral: text))
}

private func savedDialog(_ l: SavedLabel) -> IntentDialog {
    let saved = L("intent_saved_dialog", l.name)
    return dialog(l.warnings.isEmpty ? saved : ([saved] + l.warnings).joined(separator: "\n"))
}
