// swift-tools-version: 6.2
// The Labely engine for the iOS shell (ADR-0030). `cargo xtask xcframework` builds
// `LabelyFFIFFI.xcframework` and `Sources/LabelyFFI/LabelyFFI.swift`. Both are generated
// and never committed.

import PackageDescription

let package = Package(
    name: "LabelyFFI",
    platforms: [.iOS(.v18), .macOS(.v15)],
    products: [
        .library(name: "LabelyFFI", targets: ["LabelyFFI"]),
    ],
    targets: [
        .binaryTarget(name: "LabelyFFIFFI", path: "LabelyFFIFFI.xcframework"),
        .target(
            name: "LabelyFFI",
            dependencies: ["LabelyFFIFFI"],
            // Native libraries of the Rust static library (`--print native-static-libs`).
            linkerSettings: [
                .linkedFramework("CoreFoundation"),
                .linkedLibrary("iconv"),
            ]
        ),
        .testTarget(name: "LabelyFFITests", dependencies: ["LabelyFFI"]),
    ]
)
