# ADR-0011: Android AppFunctions and iOS App Intents

- Status: proposed (re-verify APIs at Phase 4/5 start)
- Date: 2026-11-14

## Decision
- **Android.** `androidx.appfunctions` 1.0.0-rc01, with the KSP compiler and
  `appfunctions-testing`.
  - The app's minSdk is 26. The AppFunctions service is `@RequiresApi(36)`, using
    `AppFunctionService` and `@AppFunctionServiceEntryPoint`.
  - Functions use `@AppFunctionDeclaration(isDescribedByKDoc = true)` and are documented in KDoc.
- **iOS.** App Intents (min iOS 17) with `AppEntity` for templates and printers, plus
  `AppShortcutsProvider`. Newer APIs such as `LongRunningIntent` for printing are guarded by
  `#available`.
- **Functions.** Both platforms expose the same six, each mapped to registry commands through
  `LabelyEngine.execute`:

| Function | Commands |
|---|---|
| createLabelFromText(text, size?) | batch: doc.new → element.add_text → doc.save |
| createLabelFromTemplate(templateId, fields) | template.instantiate (+ data.bind) |
| listTemplates() | template.list |
| listPrinters() | device.scan / device.list |
| getPrinterStatus(printerId) | device.status |
| printLabel(docId, printerId, copies) | print.submit (needs a one-time user consent) |

Android names the parameters `widthMm, heightMm` and `labelId`, and print consent is a
setting the user turns on once (ADR-0028, ADR-0029).

## Consequences
- The shells hold no editing logic.
- If the AppFunctions API changes, only one Android module is affected.
