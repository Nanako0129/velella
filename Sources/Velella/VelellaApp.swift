import AppKit
import SwiftUI
import VelellaUI

@main
struct VelellaApp: App {
    /// `swift run Velella --demo` fills the shell with fixture data; without it the shell is
    /// shown in its "not signed in" state.
    private let state: ShellState = CommandLine.arguments.contains("--demo") ? .demo : .signedOut

    init() {
        // `swift run` launches a bare executable; make it a regular foreground app.
        NSApplication.shared.setActivationPolicy(.regular)
        NSApplication.shared.activate()
    }

    var body: some Scene {
        WindowGroup("Velella") {
            ShellView(state: state)
        }
    }
}
