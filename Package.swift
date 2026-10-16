// swift-tools-version: 6.2
import PackageDescription

// `make build` generates Sources/VelellaFFI*/ and builds target/release/libvelella_ffi.a first.
// Run swift from the repo root so the relative archive path resolves.
let package = Package(
    name: "Velella",
    platforms: [.macOS(.v26)],
    targets: [
        // Generated header + modulemap (module name velella_ffiFFI, as the bindings import it).
        .target(name: "VelellaFFIC", path: "Sources/VelellaFFIC"),
        .target(
            name: "VelellaFFI",
            dependencies: ["VelellaFFIC"],
            path: "Sources/VelellaFFI"
        ),
        // SwiftUI shell; plain view-state values and fixtures, deliberately NO VelellaFFI dependency.
        // Resources: the emoji catalog and the Twemoji SVGs (scripts/emoji_catalog.py generates both).
        .target(
            name: "VelellaUI",
            path: "Sources/VelellaUI",
            resources: [.copy("Resources/Emoji"), .copy("Resources/Twemoji")]
        ),
        .testTarget(
            name: "VelellaUITests",
            dependencies: ["VelellaUI"],
            path: "Tests/VelellaUITests",
            resources: [.copy("Fixtures/EmojiReference")]
        ),
        // Login webview, capture checks and the Keychain decision; also NO VelellaFFI dependency.
        .target(name: "VelellaLogin", path: "Sources/VelellaLogin"),
        .testTarget(name: "VelellaLoginTests", dependencies: ["VelellaLogin"], path: "Tests/VelellaLoginTests"),
        .executableTarget(
            name: "Velella",
            dependencies: ["VelellaFFI", "VelellaUI", "VelellaLogin"],
            path: "Sources/Velella",
            linkerSettings: [
                // Explicit archive path, NOT -l: Darwin -l prefers a dylib if one sits beside the .a.
                .unsafeFlags(["-Xlinker", "target/release/libvelella_ffi.a"]),
                .linkedFramework("Security"),
                .linkedFramework("SystemConfiguration"),
                .linkedFramework("CoreFoundation"),
                .linkedLibrary("c++"),
                .linkedLibrary("resolv"),
            ]
        ),
    ]
)
