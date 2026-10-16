// Adapted from Swiftcord (https://github.com/SwiftcordApp/Swiftcord, GPL-3.0):
// the channel-column composition of Swiftcord/Views/Server/ServerView.swift, (c) Vincent Kwok:
// channel list, current-user footer at the bottom, and a placeholder when
// there is nothing to list. Changes: ServerContext/gateway/permissions/analytics removed;
// everything is driven by `ShellState`; the DM variant of the column was added.

import SwiftUI

private typealias T = DesignTokens

/// The channel column: channel or DM list under a floating user footer. The column title lives in the
/// window toolbar (see ShellView), so the list starts directly under it.
struct ServerView: View {
    @Binding var state: ShellState
    var onSignIn: (() -> Void)?

    /// Signed in with no server chosen yet (before S4 fills the rail) is not "no channels".
    private var placeholder: (title: String, detail: String) {
        if !state.isSignedIn { return ("Not signed in", "Sign in to see your servers.") }
        if state.selectedRailID == nil { return ("No server selected", "Pick a server on the left.") }
        return ("No channels", "This server has no channels you can see.")
    }

    var body: some View {
        Group {
            if state.isDMMode {
                DMList(topRows: state.dmTopRows, dms: state.dms, selection: $state.selectedDMID)
            } else if state.categories.isEmpty {
                ContentUnavailableView {
                    Label(placeholder.title, systemImage: "number")
                } description: {
                    Text(placeholder.detail)
                } actions: {
                    if !state.isSignedIn, let onSignIn {
                        Button("Sign In…", action: onSignIn)
                    }
                }
                .frame(maxHeight: .infinity)
            } else {
                ChannelList(categories: state.categories, selection: $state.selectedChannelID)
            }
        }
        // The footer floats over the list, which scrolls on under its glass and still lets its
        // last row scroll clear of it; the list's bottom edge is soft. Swiftcord v2's
        // ChannelSidebar: safeAreaBar(edge: .bottom, spacing: 0) and
        // scrollEdgeEffectStyle(.soft, for: .bottom) (refs/swiftcord-v2-teardown.md, UserFooterBar).
        .safeAreaBar(edge: .bottom, spacing: 0) {
            if let user = state.currentUser {
                CurrentUserFooter(user: user).padding(T.Spacing.sm)
            }
        }
        .scrollEdgeEffectStyle(.soft, for: .bottom)
    }
}
