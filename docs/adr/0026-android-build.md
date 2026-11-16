# ADR-0026: Android build

- Status: proposed
- Date: 2026-11-16

## Context
Slice 4.2 of Phase 4 (ADR-0025) puts the Rust engine into an Android app. The shell needs a
Gradle project that builds `liblabely_ffi.so` for the phone ABIs, generates the UniFFI Kotlin
sources and packages both, without committing generated code.

## Decision
- **Layout.** `mobile/android` is a standalone Gradle build (wrapper 9.9.1) with two modules.
  - `:ffi` is an Android library holding the engine. Its only source is generated.
  - `:app` is the application `org.labely.app`: minSdk 26, compileSdk 37.2 and targetSdk 37,
    written with the `release()` DSL. Compose BOM 2026.09 needs compileSdk 37. Until slice 4.3 it is a smoke screen that renders a label
    and prints it on the simulator.
- **Versions.** Every dependency is on its latest stable release (checked on 2026-11-16 against
  Google Maven, Maven Central and the SDK repository): Gradle 9.9.1, AGP 9.5.1 with built-in
  Kotlin, Kotlin 2.4.21 (pinned by declaring KGP at the root), Compose BOM 2026.09.00,
  activity-compose 1.13.0, androidx.test runner 1.7.0 and ext-junit 1.3.0, JNA 5.19.1 (AAR),
  NDK 30.0.16248370 and platform 37.2. Pre-releases (AGP 9.6 alpha, Kotlin 2.5 RC) are not
  used. On the desktop side `cargo update` and reqwest 0.13.5 bring every crate to its newest
  version; Rust 1.100.0 is the current stable.
- **Rust build.** For every variant, `:ffi` registers two tasks and adds their outputs with
  `variant.sources.{jniLibs,kotlin}.addGeneratedSourceDirectory`:
  - `cargoNdk<Variant>` runs `cargo ndk --platform 26 -t <abi>… build -p labely-ffi --lib`.
    The release variant adds `--release`. ABIs come from `labely.abis` in `gradle.properties`
    (arm64-v8a, armeabi-v7a, x86_64); `-Plabely.abis=arm64-v8a` narrows a local build.
  - `uniffiBindings<Variant>` runs `cargo xtask bindings kotlin <dir>` (package
    `org.labely.ffi`, `crates/ffi/uniffi.toml`).
  - Both are always rerun and let cargo decide what is up to date. `cargo` is taken from
    `$CARGO`, then `~/.cargo/bin/cargo`, so builds started from Android Studio find it.
- **R8.** `:ffi` ships consumer rules that keep JNA and `org.labely.ffi`.
- **Strings.** The app's text is in `res/values` and `res/values-hu`. `cargo xtask
  i18n-check` also checks that the two Android sets have the same keys.
- **Tests.** `:ffi` has instrumented tests (`EngineSmokeTest`): load the library, run commands,
  render a PNG, print on `sim:`, and check that bad params and the file and print policy come
  back as envelopes.
- **CI.** An `android` job on Ubuntu installs the Rust Android targets and cargo-ndk, then runs
  `./gradlew :app:assembleDebug :ffi:assembleDebugAndroidTest`. The instrumented tests run
  locally on an emulator or a phone (`./gradlew :ffi:connectedDebugAndroidTest`). An emulator
  job in CI can come later if they start to break unnoticed.

## Consequences
- A fresh checkout needs the Android SDK and NDK, the three Rust Android targets and cargo-ndk.
  `mobile/android/README.md` lists them.
- Desktop CI (`cargo xtask ci`) is unchanged, apart from the extra i18n check.
- The Kotlin package and application id are `org.labely.*`. Changing them later is a rename
  in `uniffi.toml` and the Gradle files.
