# Labely for Android

The Android shell (Phase 4, ADR-0025 to ADR-0028). The engine is Rust: `:ffi` builds it with
cargo-ndk and generates its Kotlin bindings, and `:app` is the UI.

## Requirements

- Android SDK with platform 37.2 and NDK 30.0.16248370 (`local.properties` or `ANDROID_HOME`)
- JDK 17 or newer
- `rustup target add aarch64-linux-android armv7-linux-androideabi x86_64-linux-android`
- `cargo install cargo-ndk --locked`

## Build and test

```sh
./gradlew :app:assembleDebug                        # debug APK, all three ABIs
./gradlew :app:assembleDebug -Plabely.abis=arm64-v8a  # one ABI, faster
./gradlew :app:testDebugUnitTest                    # AppFunctions on Robolectric, with the engine built for the host
./gradlew :ffi:connectedDebugAndroidTest            # engine smoke tests on a device or emulator
./gradlew :app:bundleRelease :app:assembleRelease   # Play bundle and APK, see docs/releasing.md
```

The Rust libraries and the Kotlin bindings are generated into `build/`. Never commit them.

The version is the workspace version of `Cargo.toml`. A release build signs with the upload key
of the `LABELY_KEYSTORE_FILE`, `LABELY_KEYSTORE_PASSWORD`, `LABELY_KEY_ALIAS` and
`LABELY_KEY_PASSWORD` variables, and is unsigned without them (ADR-0039).

## App

The screens follow `design/labely-mobile.figcraft` (ADR-0027). Debug builds list the B21 Pro
simulator under Printers, so editing and printing work without hardware. A real printer needs
Bluetooth on and the Nearby devices permission, which the app asks for on the first search.

## AppFunctions

On Android 16 and later, assistants can create, list and print labels through the six
AppFunctions of `LabelyFunctions` (ADR-0028). Printing stays off until the user turns on
"Allow assistants to print" in Settings. On a device:

```sh
adb shell cmd app_function list-app-functions | grep org.labely.app
adb shell cmd app_function execute-app-function --package org.labely.app \
  --function 'org.labely.app.appfunctions.LabelyFunctions#createLabelFromText' \
  --parameters '{"text": "Hello"}'
```
