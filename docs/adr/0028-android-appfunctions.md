# ADR-0028: Android AppFunctions

- Status: proposed
- Date: 2026-11-17

## Context
Slice 4.4 of Phase 4 (ADR-0025) gives assistants on the phone the six functions of ADR-0011.
At the start of the slice the versions were checked again (2026-11-17):
`androidx.appfunctions` 1.0.0-rc01 is still the newest release, so ADR-0011 holds.

## Decision
- **Library.** `appfunctions` and `appfunctions-compiler` 1.0.0-rc01 through KSP 2.3.14. There
  is no separate `appfunctions-service`: it ended at alpha09. `LabelyFunctions` is an abstract
  `AppFunctionService` with `@AppFunctionServiceEntryPoint`. KSP generates
  `LabelyAppFunctionService` and `assets/labely_app_functions.xml`, and the manifest declares
  the service with `BIND_APP_FUNCTION_SERVICE`. `res/xml/app_metadata.xml` describes the app
  and how the functions fit together.
- **One engine.** The service uses the app's own `AppState` and engine from
  `LabelyApplication`. There is no second session, so a label an assistant makes appears in the
  app's recent labels, and its prints appear in the history.
- **Label ids.** Labels are saved to `files/labels/`, and a label id is the file name there. A
  `doc_id` would be lost when the process dies between two calls. `printLabel` accepts only a
  bare file name in that folder.
- **Functions.** The names follow ADR-0011. Each runs plain commands with a `doc_id`, closing
  its document in `finally`:

| Function | Commands |
|---|---|
| `createLabelFromText(text, widthMm, heightMm)` | `doc.new` → `element.add_text` (centred, `fit: shrink`) → `doc.save` → `doc.check` |
| `createLabelFromTemplate(templateId, fields)` | `template.new` → `element.set_props` per field → `doc.save` → `doc.check` |
| `listTemplates(query)` | `template.list`, and `template.new` + `doc.get` for each template's fields |
| `listPrinters(scan)` | `device.list`, the known printers, the simulator in debug builds, `device.scan` when asked |
| `getPrinterStatus(printerId)` | `device.connect` when needed → `device.status` |
| `printLabel(labelId, printerId, copies)` | `doc.open` → `print.submit {wait: false}` → `job.get` until finished |

  - A batch (ADR-0011) is not used: a failed step must still close its document, and the
    service does that in `finally`.
  - **Template fields** are the text, barcode and QR elements. A field's name is the element's
    name, or else its sample text, because the starter templates do not name their elements.
    `data.bind` is not needed for this. A wrong field name fails and lists the template's fields.
  - **Optional parameters** use the type's empty value (`0`, `null`, `false`, an empty list),
    because the library ignores Kotlin defaults. Results use `-1` and `"unknown"` for missing
    values instead of nullable numbers.
- **Print consent.** `printLabel` throws `AppFunctionPermissionRequiredException` until the
  user turns on "Allow assistants to print" in Settings (Android 16 and later). The setting is
  stored in `SharedPreferences` and is off by default, like the studio's "Allow agents to
  print". The KDoc also asks the assistant to confirm with the user before printing.
- **Errors.** Engine error codes map to AppFunction exceptions: `bad_params` to invalid
  argument, `not_found` to element not found, `forbidden` to denied, and a missing Bluetooth
  permission to permission required. Other errors become an app error whose message says what
  the user can do.
- **Tests.** `appfunctions-testing`'s `AppFunctionTestRule` runs under Robolectric 4.18
  (SDK 36). The tests drive the real engine: `cargoHost` builds `labely-ffi` for the host, and
  JNA loads it from `target/debug`. They cover registration, every function, the consent error
  and a print of two copies on the B21 Pro simulator. CI runs `:app:testDebugUnitTest`.

## Consequences
- Assistants can make and print labels without the UI. A print still needs the user's consent
  once.
- `listTemplates` opens every template to read its fields. With a few dozen templates this is
  fast.
- AppFunctions run only on Android 16 and later. Older phones get the app without them, and
  the Settings switch is hidden there.
- The tests need a host Rust build, so CI builds `labely-ffi` twice: for Android and for the
  host.
