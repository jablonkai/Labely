# ADR-0029: Phase 4 close-out

- Status: proposed
- Date: 2026-11-17

## Context
Phase 4 has two exit criteria: printing from a phone to the B21 Pro, and AppFunctions tested
with `appfunctions-testing`. The exit audit checked ADR-0011 and ADR-0025 to ADR-0028 against
`crates/ffi` and `mobile/android`. It found no open TODOs and two gaps in the tests. It also
found that ADR-0025 says one thing about print consent and the code does another. A few
deferred items still had no reason.

## Decision
- **Hardware.** A Pixel 11 Pro (Android 17) printed a 50 × 30 mm text label on the B21 Pro
  (firmware 3.13) over BLE through `GattLink`. The app connected from the known-printer list,
  showed battery and paper, and the job finished as "printed". The label came out correct.
- **Editor fix found on the phone.** Text and number fields commit on Done or when they lose
  focus. Hiding the keyboard with Back left focus in the field, so the edit was not committed,
  and Print would have printed the old text. The editor now clears focus when the keyboard
  hides and before it opens a sheet.
- **AppFunctions tests.** `LabelyFunctionsTest` covers all six functions through
  `AppFunctionTestRule`, and CI runs it. Two tests are added:
  - `listPrinters` with `scan` and no Bluetooth permission gives
    `AppFunctionPermissionRequiredException`.
  - With permission and Bluetooth on, the scan runs and lists the known printers.
  - The `forbidden` → `AppFunctionDeniedException` mapping has no test. No function can reach
    it: label ids are bare file names in the app's label folder, which `printLabel` checks
    first (that check is tested).
- **Print consent stays in the shell.** The app's own UI and its AppFunctions share one
  engine, so the engine keeps `allow_print = true`. If "Allow assistants to print" switched
  `set_allow_print`, it would also block the user's own printing. The consent is checked in
  `LabelyFunctions.printLabel` before the engine is called. `set_allow_print` remains for
  shells that give agents their own engine. ADR-0025 is amended to match.
- **Function signatures.** Android uses `createLabelFromText(text, widthMm, heightMm)` and
  `printLabel(labelId, printerId?, copies)` (ADR-0028). ADR-0011's table now notes this. iOS
  follows the Android names in Phase 5.
- **Deferred, with reasons.**
  - *Device events.* Pushing printer state to the shell (ADR-0025) is not built. Polling
    `device.status` and `job.get` is enough for one printer and a few-second job. It is
    revisited in Phase 5, so Android and iOS share one callback design.
  - *Mobile editor gaps* (ADR-0027): data merge, the barcode screen, export, printer
    settings, pinch zoom, resize handles and images. The phone covers making a text, code or
    template label and printing it. These items move to "Mobile parity" under Later in the
    roadmap, so both shells get them from one design pass on `design/labely-mobile.figcraft`.
  - *Emulator CI job* (ADR-0026). CI builds `EngineSmokeTest` but does not run it. The
    Robolectric AppFunction tests already drive the real engine (host build through JNA) in
    CI. Only the Android ABI packaging is left untested in CI, and the APK build covers that.
    The job is added if a packaging regression slips through.

## Consequences
- No schema or API version change.
- The roadmap marks Phase 4 done and lists "Mobile parity" under Later.
