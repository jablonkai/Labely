# ADR-0027: Android UI and BLE

- Status: proposed
- Date: 2026-11-17

## Context
Slice 4.3 of Phase 4 (ADR-0025) turns the Android smoke screen of ADR-0026 into an app: a
Kotlin `NativeBle` so the engine can reach a real printer, and Compose screens for editing,
templates and printing. The screens follow `design/labely-mobile.figcraft` (exports in
`design/png-mobile/`), as the studio follows the desktop design.

## Decision
- **BLE.** `AndroidBle` implements `NativeBle` and `GattLink` implements `NativeBleLink`.
  - Rust calls them on its own threads and every call blocks. Android allows one outstanding
    GATT operation per connection, so `GattLink` runs each operation (connect, discover, MTU,
    CCCD write, characteristic write) under one lock and waits for its callback, with a timeout.
  - `connect` discovers services, picks the characteristic from the same known pairs as
    `labely_transport::gatt`, asks for a 247-byte MTU and enables notifications. A failed MTU
    request keeps the 20-byte default payload.
  - `write` is write-without-response. It waits for `onCharacteristicWrite` (flow control) and
    retries while the controller reports busy. API 33+ uses the new `writeCharacteristic` and
    `writeDescriptor` overloads; older versions use the deprecated ones.
  - A dropped link calls `BleNotifySink.disconnected`. A revoked permission becomes
    `BleError::Unavailable`.
  - Permissions: `BLUETOOTH_SCAN` (`neverForLocation`) and `BLUETOOTH_CONNECT` on Android 12+,
    and fine location on 8–11. They are asked for when the user first searches or connects.
- **Engine thread.** `Engine` runs every `execute` and `render_png` on one worker thread
  (`Dispatchers` from a single-thread executor), so commands keep their order and the UI thread
  never blocks. Envelopes with `ok: false` become `CommandFailure` (code, message, hint), which
  the UI shows in a snackbar.
- **State.** `LabelyApplication` owns the engine and an `AppState` with Compose state, so a
  rotation keeps the open document. No ViewModel or navigation library is added: there are
  five tabs and one editor, switched by state, and the back gesture is handled with
  `BackHandler`.
- **Editor.** `EditorState` addresses its document by `doc_id` in every call, so template
  thumbnails rendered in the background never touch it. After each command it reads `doc.get`
  and `render_png` at 203 dpi. The canvas shows that raster unfiltered, so the screen shows the
  exact dots that print. Tap selects the top-most element and dragging moves it with
  `element.move`. The inspector edits position, size, text, data, font size, bold and
  alignment. Sheets cover insert, layers, label size and paper, and save as template.
- **Saving.** Every edit saves the document to `files/labels/` (or its own path), so there is
  no unsaved state to lose. A template opened and left unedited is not saved. Recent documents
  and known printers are kept in `SharedPreferences`.
- **Printing.** The print sheet runs `doc.check` for the chosen printer, then
  `print.submit {wait: false}` and polls `job.get` for progress. The density range comes from
  the connected model. Debug builds also list the `sim:` B21 Pro, so the app works without a
  printer.
- **Design.** Colours, type sizes and icons come from the mobile design (`design/gen.py`). The
  stroke icons are its SVG paths parsed with `PathParser`, so no icon library is added.
- **Language.** English and Hungarian strings, with the same keys (`cargo xtask i18n-check`).
  Starter template names reuse the studio's translations. `generateLocaleConfig` enables the
  per-app language setting of Android 13+.
- **Deferred.** Data merge (M4), the barcode screen (M6), export (M12), printer settings, pinch
  zoom, resize handles and images wait for a later slice. The design's "From data" home card is
  replaced by "Open", which imports a `.labely` file.

## Consequences
- No new dependencies: Compose, activity-compose and the platform BLE API are enough.
- The BLE code is not covered by CI. It is tested by hand on a phone with the B21 Pro, here and
  in 4.5. The engine path behind it is covered by the `SimBleHost` tests of ADR-0025.
- Saving on every edit writes a small JSON file per change. That is cheap for labels.
