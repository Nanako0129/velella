import AppKit
import SwiftUI

@main
struct VelellaApp: App {
    @State private var model = ProbeModel()

    init() {
        // `swift run` launches a bare executable; make it a regular foreground app.
        NSApplication.shared.setActivationPolicy(.regular)
        NSApplication.shared.activate()
    }

    var body: some Scene {
        WindowGroup("Velella") {
            VStack(spacing: 12) {
                Text("Async ran on: \(model.probe)")
                Text("Ticks: \(model.ticks)").monospacedDigit()
                Button("Stop") { model.stop() }
                    .disabled(!model.isRunning)
            }
            .padding(32)
            .frame(minWidth: 320)
            .task { await model.start() }
        }
    }
}
