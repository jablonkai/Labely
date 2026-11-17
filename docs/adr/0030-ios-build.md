# ADR-0030: iOS build (Phase 5 start)

- Status: proposed
- Date: 2026-11-17

## Context
Phase 5 puts the engine on iOS (roadmap). `labely-ffi` (ADR-0025) already builds for Android,
and its UniFFI config names the Swift module `LabelyFFI`. ADR-0025 left the iOS `staticlib` for
this phase. The Swift shell needs a package that links the Rust engine for the iPhone and the
simulator, and does not commit generated code. UniFFI 0.32.2 is still the latest release
(checked 2026-11-17). The local toolchain is Xcode 27.2 with Swift 6.4.2.

## Decision
- **Static library without a crate-type change.** `cargo xtask xcframework` runs
  `cargo rustc -p labely-ffi --lib --crate-type staticlib --release` per target. `crate-type`
  in `Cargo.toml` stays `lib` and `cdylib`, so desktop and Android builds do not build a
  65 MB archive they never use. `--debug` builds without `--release`.
- **Targets.** `aarch64-apple-ios` (device), `aarch64-apple-ios-sim` (Apple-silicon
  simulator) and `aarch64-apple-darwin`. The macOS slice lets `swift test` run the engine on
  the host without a simulator. Intel simulators are not built. The deployment targets are
  iOS 18 and macOS 15.
- **XCFramework.** The task generates the Swift bindings (`xtask bindings swift`), copies
  `LabelyFFIFFI.h` and the modulemap (as `module.modulemap`) into one headers folder, and
  runs `xcodebuild -create-xcframework`. The result is `LabelyFFIFFI.xcframework`, named
  after the C module UniFFI generates.
- **Swift package.** `mobile/ios/LabelyFFI` is a local package. `LabelyFFIFFI` is a
  `binaryTarget` with a path, and `LabelyFFI` is a Swift target whose only source,
  `LabelyFFI.swift`, is generated. The target links `CoreFoundation` and `iconv`, the
  native libraries `--print native-static-libs` reports. The xcframework and the generated
  Swift are git-ignored. The app project (5.3) depends on the package by path.
- **Tests.** `LabelyFFITests` (Swift Testing) mirrors the Android `EngineSmokeTest`. It
  checks the API version, renders a PNG, prints on `sim:`, and checks that bad params and
  the file and print policy come back as envelopes. `swift test` runs it on the macOS slice.
- **CI.** An `ios` job on `macos-latest` adds the two iOS targets, runs
  `cargo xtask xcframework`, then `swift test`. It also runs `xcodebuild build` for
  `generic/platform=iOS Simulator`, which compiles the Swift bindings for iOS without a
  simulator runtime. Linking the iOS slices is checked by the app build in 5.3.

## Phase 5 slices

| Slice | Scope |
|---|---|
| 5.1 iOS build | staticlib, `xtask xcframework`, `mobile/ios/LabelyFFI` package, Swift smoke tests, CI job (this ADR) |
| 5.2 Device events | a `DeviceEvents` callback in `labely-ffi` that pushes printer and job state to the shell (ADR-0029); Android drops its polling |
| 5.3 iOS UI and BLE | Xcode app project, CoreBluetooth `NativeBle`, SwiftUI editor, templates and print screens after `design/labely-mobile.figcraft` |
| 5.4 App Intents | the six functions under the Android names (ADR-0029), App Shortcuts, print consent |
| 5.5 Close-out | print from an iPhone to the B21 Pro; the intents run from Shortcuts; exit audit |

## Consequences
- A fresh checkout needs Xcode and `rustup target add aarch64-apple-ios aarch64-apple-ios-sim`.
  `mobile/ios/README.md` lists them.
- Desktop CI and the Android build are unchanged.
- No command schema changed, so `API_VERSION` stays 3.
