// swift-tools-version: 6.2
import PackageDescription

// `make build` generates Sources/VelellaFFI*/ and builds target/release/libvelella_ffi.a first.
// Run swift from the repo root so the relative archive path resolves.

/// The Rust archive and what it links against, for every product that calls into it.
let rustArchive: [LinkerSetting] = [
    // Explicit archive path, NOT -l: Darwin -l prefers a dylib if one sits beside the .a.
    .unsafeFlags(["-Xlinker", "target/release/libvelella_ffi.a"]),
    .linkedFramework("Security"),
    .linkedFramework("SystemConfiguration"),
    .linkedFramework("CoreFoundation"),
    .linkedLibrary("c++"),
    .linkedLibrary("resolv"),
]

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
        .target(name: "VelellaUI", path: "Sources/VelellaUI"),
        .testTarget(name: "VelellaUITests", dependencies: ["VelellaUI"], path: "Tests/VelellaUITests"),
        // Login webview, capture checks and the Keychain decision; also NO VelellaFFI dependency.
        .target(name: "VelellaLogin", path: "Sources/VelellaLogin"),
        .testTarget(name: "VelellaLoginTests", dependencies: ["VelellaLogin"], path: "Tests/VelellaLoginTests"),
        // Core's transport on URLSession (plan NET); its own target so the wire tests can import it.
        .target(name: "VelellaTransport", dependencies: ["VelellaFFI"], path: "Sources/Velella/Transport"),
        .testTarget(
            name: "VelellaTransportTests",
            dependencies: ["VelellaTransport", "VelellaFFI"],
            path: "Tests/VelellaTransportTests",
            linkerSettings: rustArchive
        ),
        .executableTarget(
            name: "Velella",
            dependencies: ["VelellaFFI", "VelellaUI", "VelellaLogin", "VelellaTransport"],
            path: "Sources/Velella",
            exclude: ["Transport"],
            linkerSettings: rustArchive
        ),
    ]
)
