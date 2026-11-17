# ADR-0032: iOS UI and BLE

- Status: proposed
- Date: 2026-11-17

## Context
Slice 5.3 of Phase 5 (ADR-0030) turns the engine package into an iPhone app: a CoreBluetooth
`NativeBle` so the engine can reach a real printer, and SwiftUI screens for editing, templates
and printing. The screens follow `design/labely-mobile.figcraft`, as the Android ones do
(ADR-0027). The Android shell already settled what the screens do, so iOS ports it. No iOS
simulator runtime is installed locally or on CI, so as much as possible must be testable
without one.

## Decision
- **Two parts.** `mobile/ios/LabelyKit` is a Swift package with everything except the views:
  the engine wrapper, the models, the app and editor state, CoreBluetooth, the GATT rules and
  the design's icons. It depends on `LabelyFFI` by path and builds for macOS too, so
  `swift test` runs it against the real engine and the `sim:` B21 Pro. `mobile/ios/Labely` is
  the app: SwiftUI views only, in `Labely.xcodeproj`.
- **Xcode project.** It is written by hand and kept small: a file-system synchronized group
  (`objectVersion` 77), so new Swift files need no project edit, a local package reference
  to `../LabelyKit`, a shared `Labely` scheme and a generated Info.plist. No XcodeGen or
  Tuist. iPhone only, portrait, light, iOS 18. Simulators build for `arm64` only, as the
  XCFramework has no Intel simulator slice (ADR-0030).
- **Engine.** `Engine` runs every command on one serial queue, so commands keep their order
  and the main thread never blocks; `call` is `async` and throws `CommandFailure`. Params and
  results are a `Sendable` `JSON` enum, which keeps integers integers on the wire. Job events
  go to a lock-guarded board rather than an actor, so they keep the engine's order; `job(id)`
  is an `AsyncStream` that starts with the latest state. Device events are one `AsyncStream`.
- **State.** `AppState` and `EditorState` are `@MainActor @Observable` ports of their Android
  counterparts: the same commands, saving on every edit, recents and known printers in
  `UserDefaults`, the `sim:` printer in debug builds. Actions return their `Task`, so tests
  await them. Fixed messages are a `Notice` enum the app localises; the kit holds no strings.
  Data lives in Application Support, labels in its `labels` folder.
- **BLE.** `CoreBle` implements `NativeBle` and `CoreBleLink` implements `NativeBleLink`.
  - All CoreBluetooth state lives on one serial queue, which is also the delegate queue. Rust
    threads enter it with `queue.sync` and wait for callbacks on semaphores, never on it.
  - One operation at a time per link (connect, service and characteristic discovery,
    subscribe), each with a timeout, as `GattLink` does on Android.
  - The device id is the peripheral UUID (`ble:<uuid>`). Scans keep the peripherals they see;
    `connect` falls back to `retrievePeripherals`. The scan filter and the characteristic
    choice use the same rules as `Gatt.kt`; CoreBluetooth's short UUIDs are expanded first.
  - iOS negotiates the MTU itself; `mtu()` is `maximumWriteValueLength(.withoutResponse)`.
    Writes are write-without-response and wait for `peripheralIsReady` when the buffer is full.
  - A dropped link, or Bluetooth turning off, calls `BleNotifySink.disconnected` once. A denied
    permission or Bluetooth off becomes `BleError.Unavailable`.
  - The central is created on the first scan or connect, which is when iOS asks for the
    permission. A denied permission is caught before a scan and shown as a message.
- **Views.** Colours, type sizes and the stroke icons come from the mobile design. The icons
  are the design's SVG paths, parsed by a small SVG path parser into `CGPath` (no icon
  library). The canvas shows the 203 dpi raster without interpolation, tap selects and drag
  moves, as on Android. Fields commit when they lose focus; opening a sheet, tapping the
  canvas or leaving the editor resigns the first responder first, so an edit is never lost
  (the editor fix of ADR-0029).
- **Language.** `Localizable.xcstrings` uses the Android string names as keys, with English
  and Hungarian, and was generated once from the Android resources. `InfoPlist.xcstrings`
  localises the Bluetooth prompt. `cargo xtask i18n-check` now checks that every catalog key
  has an English and a Hungarian value, and that every `L("key")` literal in the app exists.
- **CI.** The `ios` job also runs `swift test` in `LabelyKit` and builds the app for the
  simulator and the device without signing.
- **Deferred.** The assistant print consent comes with App Intents (5.4). The mobile parity
  items of ADR-0029 stay deferred. Printing from an iPhone to the B21 Pro is checked in 5.5.

## Consequences
- No new dependencies: SwiftUI, Observation, CoreBluetooth and the generated bindings.
- The views and the CoreBluetooth code are not run in CI. The logic behind them is, on macOS
  through the real engine. The BLE path is tested by hand on an iPhone in 5.5.
- No command schema changed, so `API_VERSION` stays 3.
