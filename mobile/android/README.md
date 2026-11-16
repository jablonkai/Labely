# Labely for Android

The Android shell (Phase 4, ADR-0025 and ADR-0026). The engine is Rust: `:ffi` builds it with
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
./gradlew :ffi:connectedDebugAndroidTest            # engine smoke tests on a device or emulator
```

The Rust libraries and the Kotlin bindings are generated into `build/`. Never commit them.
