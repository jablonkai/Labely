# ADR-0033: iOS App Intents

- Status: proposed
- Date: 2026-11-17

## Context
Slice 5.4 of Phase 5 (ADR-0030) gives Siri, Shortcuts and Apple Intelligence the six functions
of ADR-0011, as the AppFunctions do on Android (ADR-0028). The APIs were checked again against
the iOS 27.2 SDK (Xcode 27.2): App Intents, `AppEntity`, `EntityQuery` and
`AppShortcutsProvider` are unchanged. `LongRunningIntent` exists, but only from iOS 27, and a
conformance cannot be guarded by `#available` while the app targets iOS 18.

## Decision
- **Two parts, as in ADR-0032.** `LabelyKit.Assistant` holds the six functions as plain
  `async throws` methods on the app's own `AppState` and engine, so `swift test` runs them on
  macOS against the real engine and the `sim:` B21 Pro. `Intents.swift` in the app only
  describes them to the system: entities, intents, shortcuts, and localised results and errors.
- **One engine.** `Shared` in the app owns the `AppState` and the `Assistant`. The scene and
  the intents use it, and an intent can run before any scene exists. Intents run in the app's
  process (no extension), so labels an intent makes appear in the recents, and its prints in
  the history.
- **Functions.** The intents use the Android names and run the same commands (ADR-0028):

| Intent | Parameters | Returns |
|---|---|---|
| `CreateLabelFromTextIntent` | text, width and height (50 × 30 mm by default) | `LabelEntity`, dialog |
| `CreateLabelFromTemplateIntent` | `TemplateEntity`, fields as `name=value` lines | `LabelEntity`, dialog |
| `ListTemplatesIntent` | search text | `[TemplateEntity]` |
| `ListPrintersIntent` | search nearby | `[PrinterEntity]` |
| `GetPrinterStatusIntent` | `PrinterEntity` (optional) | `PrinterEntity` with battery and paper, dialog |
| `PrintLabelIntent` | `LabelEntity`, `PrinterEntity` (optional), copies 1–999 | dialog |

  - **Entities.** A `LabelEntity` id is the file name in the label folder, as the Android label
    id; its query lists the saved labels, newest first. A `TemplateEntity` id is the template
    ref, and its string query searches by name. A `PrinterEntity` id is the printer address;
    an id the app does not list still resolves, so a saved shortcut keeps its printer.
  - **Fields** are a list of `name=value` texts, because an intent parameter cannot be a
    dictionary. The first `=` splits. Field names follow ADR-0028.
  - **No printer** given uses the connected printer, else the only known one.
- **Print consent.** `printLabel` fails until the user turns on "Allow assistants to print" in
  Settings. It is stored in `UserDefaults` and is off by default, as on Android.
- **Errors.** `Assistant` throws `AssistantError`, whose cases carry the data and no text. The
  app turns them into localised messages through `CustomLocalizedStringResourceConvertible`.
  Other engine failures pass their message through.
- **Shortcuts.** `LabelyShortcuts` offers three: make a text label, print a label and check the
  printer. Their phrases are in `AppShortcuts.xcstrings`, English and Hungarian.
- **Language.** Intent titles, parameters, properties and messages are `intent_…` keys in
  `Localizable.xcstrings`. `cargo xtask i18n-check` now finds `"intent_…"` literals in the app's
  sources, as it does `L("key")`, and checks `AppShortcuts.xcstrings`.
- **Tests.** `AssistantTests` covers every function, the consent error, label id checks, the
  field errors and a two-copy print on the simulator. CI already runs them with the other
  `LabelyKit` tests, and builds the app, which runs the App Intents metadata processor.

## Consequences
- Shortcuts and Siri can make and print labels without opening the app. A print still needs the
  user's consent once.
- No `LongRunningIntent`: a print waits up to five minutes for its job. When the app targets
  iOS 27, `PrintLabelIntent` can adopt it.
- The app has no `bluetooth-central` background mode. Whether an intent run in the background
  can reach the printer is checked on an iPhone in 5.5, together with Siri and Shortcuts.
- No command schema changed, so `API_VERSION` stays 3.
