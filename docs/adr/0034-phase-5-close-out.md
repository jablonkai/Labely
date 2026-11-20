# ADR-0034: Phase 5 close-out

- Status: proposed
- Date: 2026-11-20

## Context
Phase 5 has two exit criteria: printing from an iPhone to the B21 Pro, and the intents running
from Shortcuts. ADR-0033 left one question for the phone: whether an intent that runs in the
background can reach the printer without the `bluetooth-central` background mode. The exit
audit checked ADR-0011 and ADR-0030 to ADR-0033 against `crates/ffi` and `mobile/ios`. It found
no open TODOs, one gap in the tests, two faults in the BLE error path, and two places where
the ADRs did not match what was built.

## Decision
- **Hardware checks are not run yet.** The two exit criteria are still open. No iPhone was
  available for this slice, so these three checks move to the start of Phase 6:
  - Print a 50 × 30 mm text label on the B21 Pro from the app.
  - Run the three App Shortcuts from Shortcuts and from Siri.
  - Run `PrintLabelIntent` with the app in the background. If it cannot reach the printer, the
    app gets the `bluetooth-central` background mode.
- **Failed connects leave no link behind.** `CoreBle.connect` kept the link in its table when
  `open()` failed. A connection that never opened may get no disconnect callback, so the entry
  stayed, and a late callback could reach the next link to the same printer. The entry is now
  removed when the open fails.
- **Bluetooth errors are matched by constant.** `Assistant` decided between "Bluetooth" and
  "unreachable" by looking for the word "permission" in the engine message. `CoreBle` now names
  its two details (`permissionDenied`, `poweredOff`), and `Assistant` matches those.
- **Device event test.** `AppState.onDeviceEvent` had no test. `StateTests` now checks that
  events for other printers are ignored, that a status event updates battery and paper, and
  that a lost link clears the printer and sets the `printerLost` message.
- **ADR-0011 as built.** Its table names `template.instantiate`, which does not exist:
  `createLabelFromTemplate` runs `template.new` and then `element.set_props` for each field
  (ADR-0028). It also says iOS 17 and `LongRunningIntent`; the app targets iOS 18 and has no
  `LongRunningIntent` (ADR-0033). ADR-0011 now notes both.
- **Deferred, with reasons.**
  - *Mobile parity* (ADR-0029) stays under Later, so both shells get it from one design pass.
    ADR-0032 now says so.
  - *`LongRunningIntent`* waits until the app targets iOS 27 (ADR-0033).

## Consequences
- No schema or API version change.
- The roadmap marks slice 5.5 as audited, with the hardware checks open. Phase 5 is done when
  they pass; the result is added to this ADR.
- The BLE path on a real iPhone stays untested until then (ADR-0032).
