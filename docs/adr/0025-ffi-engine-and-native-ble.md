# ADR-0025: FFI engine and native BLE host (Phase 4 start)

- Status: proposed
- Date: 2026-11-16

## Context
Phase 4 puts the engine on Android. ADR-0006 put mobile BLE in the shell behind a UniFFI
callback, and the architecture (§7) sketched a `LabelyEngine` object with `execute`. Before the
Android shell can start, the Rust side needs that crate, a way for the device hub to use the
shell's BLE stack, and a test path that needs no phone or printer. UniFFI 0.32.2 is still the
latest release (checked 2026-11-16).

## Decision
- **Crate.** `labely-ffi` (L3, ADR-0003) uses UniFFI 0.32 in proc-macro mode. It builds as
  `lib` and `cdylib`; `staticlib` for iOS is added in Phase 5. It is the only crate allowed to
  use `uniffi`, and it allows `unsafe_code` for the generated scaffolding.
- **Engine.** `LabelyEngine` wraps one `Session`.
  - `new(EngineConfig { data_dir, file_roots, allow_print }, ble?)`. Templates, history and
    the paper library live under `data_dir`. The policy is restricted to `data_dir` plus
    `file_roots`, so file commands cannot leave the app's storage (ADR-0012).
  - `execute(id, params_json) -> envelope_json` takes the same ids and envelopes as MCP. Bad
    JSON or non-object params come back as a `bad_params` envelope, never as an exception.
  - `commands_json()` returns `command.list`. `api_version()` returns `API_VERSION`.
  - `set_allow_print(bool)` is the switch for the AppFunctions one-time consent (ADR-0011).
  - `render_png(doc_id?, dpi, page) -> bytes` renders the print raster without a base64 round
    trip, for the canvas.
- **BLE, split in two.** ADR-0006 named one interface `NativeBleLink {scan, connect, write, mtu,
  close}`. It is split, so each connection is its own object and several printers can be open:
  - `NativeBle { scan(timeout_ms, name_prefixes), connect(id, sink) -> NativeBleLink }`
  - `NativeBleLink { write(bytes), mtu(), name(), disconnect() }`
  - `BleNotifySink { push(bytes), disconnected() }` is created by Rust **before** `connect`, so
    notifications that arrive while the link opens are not lost.
  - `BleError` has `Unavailable` (Bluetooth off, no permission), `NotFound` and `Failed`.
- **Hub.** `labely-transport` gains the UniFFI-free `NativeBleHost` trait and
  `CallbackTransport::connect_with`. `Hub::with_native_ble(host)` sends `ble:` addresses and
  scans through it instead of btleplug, and `labely_printers::mobile_services(host)` builds the
  services. Addresses stay `ble:<id>` (MAC on Android, peripheral UUID on iOS), so print history
  and saved documents look the same as on desktop.
- **Simulator host.** `labely_printers::sim::SimBleHost` is a `NativeBleHost` that runs the
  B21 Pro simulator behind a 20-byte MTU. The FFI tests drive the whole path (scan, connect,
  print) through a Rust stand-in for the shell.
- **Bindings.** Generated sources are not committed. `cargo xtask bindings [kotlin|swift]
  [out-dir]` builds the host library and runs `uniffi-bindgen` (feature `bindgen`) in library
  mode. The Android build (4.2) calls it.
- **Deferred.** A device-event callback (printer state pushed to the shell) waits for the
  Compose UI (4.3). Until then the shell polls `device.status` and `job.status`.

## Phase 4 slices

| Slice | Scope |
|---|---|
| 4.1 FFI | `labely-ffi`, `NativeBleHost`, `SimBleHost`, `xtask bindings` (this ADR) |
| 4.2 Android build | `mobile/android` Gradle project (AGP 9, Kotlin 2.4), cargo-ndk for arm64-v8a, armeabi-v7a, x86_64, bindings from `xtask`, a smoke instrumented test, CI job |
| 4.3 Android UI and BLE | Kotlin `NativeBle` (`BluetoothGatt` with a queued op scheduler, permissions), Compose editor, templates and print screens after `design/labely-mobile.figcraft` |
| 4.4 AppFunctions | the six functions of ADR-0011 (API version re-checked), print consent, `appfunctions-testing` |
| 4.5 Close-out | print from a phone to the B21 Pro; exit audit |

## Consequences
- The shells only call `execute`, `render_png` and the BLE callbacks. No editing logic moves
  to Kotlin or Swift.
- The Rust BLE path is tested on every CI run through `SimBleHost`. The real GATT code is tested
  on a phone in 4.3 and 4.5.
- No command schema changed, so `API_VERSION` stays 3.
