import Foundation
import Testing

@testable import LabelyKit

/// The App Intents logic against the real engine and the `sim:` B21 Pro, as the Android
/// AppFunction tests (ADR-0028).
@MainActor
struct AssistantTests {
    let app: AppState
    let assistant: Assistant

    init() throws {
        let dir = FileManager.default.temporaryDirectory.appendingPathComponent("labelykit-\(UUID().uuidString)")
        try FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
        let defaults = try #require(UserDefaults(suiteName: "labelykit-\(UUID().uuidString)"))
        app = AppState(
            engine: Engine(dataDir: dir.path, ble: nil),
            labelsDir: dir.appendingPathComponent("labels"),
            defaults: defaults,
            debug: true
        )
        assistant = Assistant(app: app, simulatorName: "Sim") { $0.name }
    }

    private func file(_ label: SavedLabel) -> URL {
        app.labelsDir.appendingPathComponent(label.labelId)
    }

    @Test func listTemplatesGivesStartersWithTheirFields() async throws {
        let templates = try await assistant.listTemplates()
        #expect(templates.count >= 10)
        let price = try #require(templates.first { $0.templateId == "starter:price-tag" })
        #expect(price.fields.map(\.kind) == ["text", "text", "barcode"])
        #expect(price.fields.contains { $0.name == "Product name" })
        #expect(try await assistant.listTemplates(query: "price").map(\.templateId) == ["starter:price-tag"])
    }

    @Test func createLabelFromTextSavesALabelTheAppLists() async throws {
        let label = try await assistant.createLabelFromText("Spices\nCumin")
        #expect(label.widthMm == 50 && label.heightMm == 30)
        #expect(label.name == "Spices")
        #expect(label.warnings.isEmpty)
        #expect(try String(contentsOf: file(label), encoding: .utf8).contains("Cumin"))
        #expect(app.recents.first?.path == file(label).path)
        #expect(assistant.savedLabels().map(\.labelId) == [label.labelId])
        #expect(assistant.savedLabels().first?.name == "Spices")
    }

    @Test func createLabelFromTextRejectsEmptyTextAndBadSizes() async {
        await #expect(throws: AssistantError.emptyText) { try await assistant.createLabelFromText("  \n") }
        await #expect(throws: AssistantError.sizeOutOfRange(widthMm: 500, heightMm: 30)) {
            try await assistant.createLabelFromText("x", widthMm: 500)
        }
    }

    @Test func createLabelFromTemplateFillsFields() async throws {
        let label = try await assistant.createLabelFromTemplate("starter:price-tag", fields: [
            FieldValue(name: "Product name", value: "Apples"), FieldValue(name: "9.99", value: "2.49"),
        ])
        let json = try String(contentsOf: file(label), encoding: .utf8)
        #expect(json.contains("Apples") && json.contains("2.49"))
        #expect(!json.contains("Product name"))
    }

    @Test func createLabelFromTemplateNamesTheFieldsOnAWrongField() async throws {
        do {
            _ = try await assistant.createLabelFromTemplate("starter:price-tag", fields: [FieldValue(name: "Price", value: "1")])
            Testing.Issue.record("no error")
        } catch AssistantError.noField(let field, let fields) {
            #expect(field == "Price" && fields.contains("Product name"))
        }
        await #expect(throws: AssistantError.noTemplate("starter:nope")) {
            try await assistant.createLabelFromTemplate("starter:nope")
        }
    }

    @Test func fieldValuesParseFromLines() {
        #expect(FieldValue.parse("Product name = Apples=red") == FieldValue(name: "Product name", value: " Apples=red"))
        #expect(FieldValue.parse("no equals") == nil)
        #expect(FieldValue.parse("=x") == nil)
    }

    @Test func printLabelNeedsTheUsersConsent() async throws {
        let label = try await assistant.createLabelFromText("Hi")
        await #expect(throws: AssistantError.printNotAllowed) {
            try await assistant.printLabel(label.labelId, printerId: AppState.simAddress)
        }
    }

    @Test func printLabelPrintsOnTheSimulator() async throws {
        let label = try await assistant.createLabelFromText("Hi")
        app.allowAgentPrint = true
        let printed = try await assistant.printLabel(label.labelId, printerId: AppState.simAddress, copies: 2)
        #expect(printed.state == "done" && printed.pagesPrinted == 2 && printed.pagesTotal == 2)
        #expect(app.history.count == 1)

        let printers = try await assistant.listPrinters()
        #expect(printers.first?.printerId == AppState.simAddress && printers.first?.connected == true)
        let status = try await assistant.printerStatus(AppState.simAddress)
        #expect(status.model.contains("B21"))
        #expect((0...100).contains(status.batteryPercent ?? 0))

        // No printer given: the connected one.
        #expect(try await assistant.printLabel(label.labelId).state == "done")
    }

    @Test func printLabelRejectsUnknownLabelsAndBadCopies() async throws {
        app.allowAgentPrint = true
        for id in ["../secret.labely", "", ".hidden", "missing.labely"] {
            await #expect(throws: AssistantError.noLabel(id)) {
                try await assistant.printLabel(id, printerId: AppState.simAddress)
            }
        }
        let label = try await assistant.createLabelFromText("Hi")
        await #expect(throws: AssistantError.badCopies(max: 999)) {
            try await assistant.printLabel(label.labelId, printerId: AppState.simAddress, copies: 1000)
        }
        await #expect(throws: AssistantError.noPrinter) { try await assistant.printLabel(label.labelId) }
    }

    @Test func listPrintersOffersTheSimulatorInDebugBuilds() async throws {
        let printers = try await assistant.listPrinters()
        #expect(printers == [PrinterInfo(printerId: AppState.simAddress, name: "Sim", connected: false, model: "")])
    }

    @Test func consentIsRemembered() throws {
        let defaults = try #require(UserDefaults(suiteName: "labelykit-\(UUID().uuidString)"))
        let a = AppState(engine: app.engine, labelsDir: app.labelsDir, defaults: defaults, debug: false)
        #expect(!a.allowAgentPrint)
        a.allowAgentPrint = true
        #expect(AppState(engine: app.engine, labelsDir: app.labelsDir, defaults: defaults, debug: false).allowAgentPrint)
    }
}
