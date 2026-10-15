import AppKit
import SwiftUI
import VelellaFFI
import VelellaLogin
import VelellaUI

@main
struct VelellaApp: App {
    /// `swift run Velella --demo` fills the shell with fixture data and never touches the session
    /// or the keychain; without it the app restores the saved sign-in or shows "not signed in".
    @State private var model = AppModel(demo: CommandLine.arguments.contains("--demo"))

    init() {
        // `swift run` launches a bare executable; make it a regular foreground app.
        NSApplication.shared.setActivationPolicy(.regular)
        NSApplication.shared.activate()
    }

    var body: some Scene {
        WindowGroup("Velella") {
            MainWindow(model: model)
        }
        .commands {
            CommandGroup(after: .appSettings) {
                // Ends velella's session and forgets the saved sign-in; Discord's own session is
                // not revoked, so the text must not say "log out of Discord".
                if model.canSignOut {
                    Button("Sign Out") { model.signOut() }
                }
            }
        }

        Window("Sign In", id: LoginWindow.id) {
            LoginWindow(model: model)
        }
        .commandsRemoved()
        .restorationBehavior(.disabled)
        .defaultSize(width: LoginWindow.defaultWidth, height: LoginWindow.defaultHeight)
    }
}

private struct MainWindow: View {
    @Bindable var model: AppModel
    @Environment(\.openWindow) private var openWindow

    var body: some View {
        ShellView(
            state: $model.shell,
            onSignIn: model.isDemo ? nil : { openWindow(id: LoginWindow.id) },
            onSend: model.isDemo ? nil : { model.send($0, replyTo: $1) },
            onLoadOlder: model.isDemo ? nil : { model.loadOlder() }
        )
            .onChange(of: model.shell.selectedRailID) {
                model.syncSelection()
                // Always, also when syncSelection returns early (DM entry, same guild, no session).
                model.selectionChanged()
            }
            .onChange(of: model.shell.selectedChannelID) { model.selectionChanged() }
            .onChange(of: model.shell.selectedDMID) { model.selectionChanged() }
            .alert(model.notice ?? "", isPresented: Binding(
                get: { model.notice != nil }, set: { if !$0 { model.notice = nil } }
            )) {
                Button("OK") {}
            }
    }
}

/// Holds no token: the captured value goes straight to `AppModel.loginCaptured`, and the webview
/// (torn down on capture or close) is gone with the window.
private struct LoginWindow: View {
    static let id = "login"
    /// The login page lays out its form and QR code side by side at about 940 wide (user's capture,
    /// 2026-10-14), and that size read as cramped, so the default adds room around the card and
    /// the minimum keeps the side-by-side layout.
    static let minWidth: CGFloat = 940
    static let minHeight: CGFloat = 560
    static let defaultWidth: CGFloat = 1080
    static let defaultHeight: CGFloat = 800
    let model: AppModel
    @Environment(\.dismissWindow) private var dismissWindow

    var body: some View {
        LoginWebView(userAgent: userAgent()) { token in
            model.loginCaptured(token)
            dismissWindow(id: Self.id)
        }
        .frame(minWidth: Self.minWidth, minHeight: Self.minHeight)
    }
}
