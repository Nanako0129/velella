// Adapted from Swiftcord (https://github.com/SwiftcordApp/Swiftcord, GPL-3.0):
// the channel-column composition of Swiftcord/Views/Server/ServerView.swift, (c) Vincent Kwok:
// channel list, current-user footer at the bottom, and a placeholder when
// there is nothing to list. Changes: ServerContext/gateway/permissions/analytics removed;
// everything is driven by `ShellState`; the DM variant of the column was added.

import SwiftUI

private typealias T = DesignTokens

/// The channel column: channel or DM list, floating user footer. The column title lives in the
/// window toolbar (see ShellView), so the list starts directly under it.
struct ServerView: View {
    @Binding var state: ShellState

    var body: some View {
        VStack(spacing: 0) {
            if state.isDMMode {
                DMList(topRows: state.dmTopRows, dms: state.dms, selection: $state.selectedDMID)
            } else if state.categories.isEmpty {
                ContentUnavailableView {
                    Label(state.isSignedIn ? "No channels" : "Not signed in", systemImage: "number")
                } description: {
                    Text(state.isSignedIn ? "This server has no channels you can see." : "Sign in to see your servers.")
                }
                .frame(maxHeight: .infinity)
            } else {
                ChannelList(categories: state.categories, selection: $state.selectedChannelID)
            }

            if let user = state.currentUser {
                CurrentUserFooter(user: user).padding(T.Spacing.sm)
            }
        }
    }
}
