# Labely for iOS

The iOS shell (Phase 5, ADR-0030, ADR-0032). The engine is Rust: `cargo xtask xcframework`
builds it as `LabelyFFIFFI.xcframework` and generates its Swift bindings into the `LabelyFFI`
package.

| Folder | What it is |
|---|---|
| `LabelyFFI` | The engine: the XCFramework and its generated Swift bindings |
| `LabelyKit` | Everything but the views: engine wrapper, app and editor state, App Intents logic, CoreBluetooth, icons. Builds for macOS too, so `swift test` runs it on the host |
| `Labely` | The app: `Labely.xcodeproj` with the SwiftUI screens and the string catalogs |

## Requirements

- Xcode 27 or newer (Swift 6.2 tools)
- `rustup target add aarch64-apple-ios aarch64-apple-ios-sim`

## Build and test

Run these from the repository root:

```sh
cargo xtask xcframework            # release build of the device, simulator and macOS slices
cargo xtask xcframework --debug    # faster, larger
(cd mobile/ios/LabelyFFI && swift test)   # engine smoke tests on the macOS slice
(cd mobile/ios/LabelyFFI && xcodebuild build -quiet -scheme LabelyFFI \
  -destination 'generic/platform=iOS Simulator')   # compile the bindings for iOS
(cd mobile/ios/LabelyKit && swift test)   # app and editor state on the engine and sim: printer
(cd mobile/ios/Labely && xcodebuild build -quiet -scheme Labely \
  -destination 'generic/platform=iOS Simulator' CODE_SIGNING_ALLOWED=NO)   # the app
```

To run it on an iPhone, open `Labely/Labely.xcodeproj` in Xcode, pick your team under
Signing & Capabilities and run. Debug builds also list the B21 Pro simulator, so the app works
without a printer. New Swift files in `Labely/Labely` join the target by themselves (a
synchronized folder). App strings go in `Localizable.xcstrings` with English and Hungarian;
`cargo xtask i18n-check` checks both.

The App Intents are in `Labely/Intents.swift` (ADR-0033); their logic is
`LabelyKit/Sources/LabelyKit/Assistant.swift`. Intent strings are `intent_…` keys in
`Localizable.xcstrings`, and the App Shortcuts phrases are in `AppShortcuts.xcstrings`. To try
them, run the app once, turn on "Allow assistants to print" in Settings, and find Labely's
actions in the Shortcuts app.

The xcframework and `Sources/LabelyFFI/LabelyFFI.swift` are generated. Never commit them.
