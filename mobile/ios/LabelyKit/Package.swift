// swift-tools-version: 6.2
// The iOS shell's logic without its views (ADR-0032): the engine wrapper, the app and editor
// state, CoreBluetooth and the design's icons. It builds for macOS too, so `swift test` runs it
// against the real engine and the `sim:` printer without a simulator.

import PackageDescription

let package = Package(
    name: "LabelyKit",
    platforms: [.iOS(.v18), .macOS(.v15)],
    products: [
        .library(name: "LabelyKit", targets: ["LabelyKit"]),
    ],
    dependencies: [
        .package(path: "../LabelyFFI"),
    ],
    targets: [
        .target(name: "LabelyKit", dependencies: ["LabelyFFI"]),
        .testTarget(name: "LabelyKitTests", dependencies: ["LabelyKit"]),
    ]
)
