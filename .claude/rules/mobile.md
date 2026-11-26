---
paths:
  - "mobile/**"
---

# Mobile shells (Android, iOS)

- **Design.** Screens follow `design/labely-mobile.figcraft` (ADR-0027, ADR-0032): colours, type
  sizes and the stroke icons come from it. Do not invent layouts; check the design first.
- **Engine only.** Both shells call the engine through `LabelyEngine.execute` / `Engine.call`.
  No new FFI method or command for a UI feature (ADR-0042). Files stay inside the app data dir
  (ADR-0012): CSV as text, pictures as `data_base64`, exports to `exports/`.
- **Strings.** No hard-coded UI text.
  - Android: `mobile/android/app/src/main/res/values/` (English) and `values-<code>/` per locale.
  - iOS: `mobile/ios/Labely/Labely/*.xcstrings`. Every `L("key")` must exist in
    `Localizable.xcstrings` with a value for every shipped locale (ADR-0032); keys use the
    Android names. LabelyKit holds no strings (it returns a `Notice` the app localises).
  - Every locale has the same keys as English: `cargo xtask i18n-check`. Locales and how to
    add one: `docs/i18n/README.md`.
- **Generated, never edit or commit:** `mobile/ios/LabelyFFI/Sources/LabelyFFI/LabelyFFI.swift`,
  `mobile/ios/LabelyFFI/LabelyFFIFFI.xcframework/` (`cargo xtask xcframework`),
  `target/bindings/` and the Android `build/` dirs (`cargo xtask bindings kotlin`). Change the
  Rust side in `crates/ffi` and regenerate.
- **Parity (ADR-0042).** A feature added to one shell gets the same behaviour on the other, or
  a reason in the ADR. Check with the `parity-checker` agent.
- **Build and test:**
  - `(cd mobile/android && ./gradlew :app:assembleDebug)`, `./gradlew :app:testDebugUnitTest`
  - `cargo xtask xcframework`, then `(cd mobile/ios/LabelyFFI && swift test)` and
    `(cd mobile/ios/LabelyKit && swift test)`
  - Details: `mobile/android/README.md`, `mobile/ios/README.md`.
