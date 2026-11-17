# Labely for iOS

The iOS shell (Phase 5, ADR-0030). The engine is Rust: `cargo xtask xcframework` builds it as
`LabelyFFIFFI.xcframework` and generates its Swift bindings into the `LabelyFFI` package.

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
```

The xcframework and `Sources/LabelyFFI/LabelyFFI.swift` are generated. Never commit them.
