# ADR-0006: BLE in Rust on desktop, in the native shell on mobile

- Status: proposed
- Date: 2026-11-14

## Context
- btleplug 0.13.5 works on macOS, Windows, Linux and iOS (CoreBluetooth), and on Android through
  JNI.
- Android needs a JNI class-loader setup, bundled Java classes and R8 keep rules. There are open
  MTU issues: #504, where peripheral-initiated MTU exchanges are ignored, and #503. #501 says there
  is no minimal Android example.
- iOS cannot pair/bond through btleplug (#426).
- Mobile BLE also depends on permission flows and background lifecycle, which belong to the
  platform.

## Decision
- **Desktop and CLI.** `labely-transport::ble` uses btleplug.
- **Android and iOS.** BLE is implemented natively (Kotlin `BluetoothGatt` with a queued GATT op
  scheduler; Swift CoreBluetooth). It is exposed to Rust as a UniFFI callback interface,
  `NativeBleLink {scan, connect, write, mtu, close}`, with notifications pushed into Rust.
- **Adapter.** `labely-transport::callback` adapts it to the `Transport` trait, so framing,
  chunking by `mtu()`, protocol and drivers stay in Rust.

## Consequences
- About 300 lines of BLE glue per mobile platform. No btleplug Java or R8 setup.
- MTU is under platform control.
- The Rust driver code is identical on every platform.
- Revisit if btleplug's Android MTU issues are fixed.
