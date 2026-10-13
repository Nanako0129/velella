// The window composition is fresh code, rebuilt on NavigationSplitView and `.inspector` instead
// of Swiftcord/Views/Server/ServerView.swift's NavigationView and AppKit sidebar toggle (GPL-3.0,
// (c) Vincent Kwok). Toolbar layout (column title and round glass buttons in the sidebar
// toolbar, the channel name as the window title over the content) follows the Swiftcord v2
// reference screens.

import SwiftUI

private typealias T = DesignTokens

/// The main window: sidebar (server rail + channel column), message column, member list.
public struct ShellView: View {
    /// Owned by the app (or a fixture), so its later changes reach the window.
    @Binding private var state: ShellState
    private let onSignIn: (() -> Void)?
    @State private var showMembers = true
    @State private var search = ""
    @State private var sidebarWidth: CGFloat = 0

    /// - Parameter onSignIn: shown as "Sign In…" on the signed-out placeholder when set.
    public init(state: Binding<ShellState>, onSignIn: (() -> Void)? = nil) {
        _state = state
        self.onSignIn = onSignIn
    }

    public var body: some View {
        NavigationSplitView {
            // Rail, a 1 pt line, channel list, spacing 0 (v2's sidebar HStack).
            HStack(spacing: 0) {
                ServerRail(items: state.rail, selection: $state.selectedRailID)
                Rectangle().fill(T.Palette.railColumnDivider).frame(width: T.Size.railColumnDivider)
                ServerView(state: $state, onSignIn: onSignIn)
            }
            .onGeometryChange(for: CGFloat.self) { $0.size.width } action: { sidebarWidth = $0 }
            .navigationSplitViewColumnWidth(
                min: T.Size.railWidth + T.Size.railColumnDivider + T.Size.channelColumnMin,
                ideal: T.Size.railWidth + T.Size.railColumnDivider + T.Size.channelColumnIdeal,
                max: T.Size.railWidth + T.Size.railColumnDivider + T.Size.channelColumnMax
            )
            .toolbar {
                // Column title right of the traffic lights (no glass behind it), then the column
                // button; the system sidebar toggle follows. Each is its own item so the toolbar
                // draws v2's round glass behind the button and the toggle. The title is a
                // `.primaryAction` pushed left by the flexible spacer because, observed 2026-10-13
                // (macOS 27.0.1), a `.navigation` title lands after the toggle over the detail
                // column, and `DefaultToolbarItem(kind: .sidebarToggle)` draws an empty capsule.
                ToolbarItem(placement: .primaryAction) { sidebarTitle }
                    .sharedBackgroundVisibility(.hidden)
                ToolbarSpacer(.flexible, placement: .primaryAction)
                ToolbarItem(placement: .primaryAction) { sidebarButton }
            }
        } detail: {
            MessageArea(header: state.header, isSignedIn: state.isSignedIn, messages: state.messages, reply: $state.reply)
                // No minimum of its own: SwiftUI's `.inspector` adds the floating sidebar's current
                // width to this column's minimum twice (measured 2026-10-13, macOS 27.0.1,
                // offscreen: 1486 = 343 × 2 + 320 + 480), so any minimum here made the window
                // minimum follow the column widths: dividers grew the window, locked it at 1686,
                // and on screen the oscillating minimum threw NSGenericException "more Update
                // Constraints in Window passes than there are views". The window minimum below
                // is a constant above anything that sum can reach.
                .frame(minWidth: 0, maxWidth: .infinity)
                // The system title, not a custom toolbar item: it gets the leading inset Swiftcord v2
                // shows (title 20-22 pt after the sidebar edge; a hidden-glass custom item sat at 8).
                .navigationTitle(state.header?.name ?? "")
                .toolbar {
                    // Glass button groups as in Swiftcord v2: pin | call + video | members.
                    // The buttons have no actions until their features exist.
                    ToolbarItem(placement: .primaryAction) {
                        Button {} label: { Image(systemName: "pin") }.help("Pinned messages")
                    }
                    ToolbarSpacer(.fixed, placement: .primaryAction)
                    ToolbarItemGroup(placement: .primaryAction) {
                        if state.isDMMode {
                            Button {} label: { Image(systemName: "phone") }.help("Start voice call")
                            Button {} label: { Image(systemName: "video") }.help("Start video call")
                        }
                    }
                    ToolbarSpacer(.fixed, placement: .primaryAction)
                    ToolbarItem(placement: .primaryAction) {
                        Button { showMembers.toggle() } label: { Image(systemName: "person.2") }
                            .help("Toggle member list")
                    }
                }
                .searchable(text: $search, placement: .toolbar, prompt: "Search")
                .inspector(isPresented: $showMembers) {
                    // Fixed width: v2's member list does not resize.
                    MemberList(sections: state.memberSections).inspectorColumnWidth(T.Size.memberColumnWidth)
                }
        }
        // A constant window minimum that fits the widest sidebar, the narrowest message column and
        // the member list, so no divider position can push the window wider.
        .frame(minWidth: T.Size.windowMinWidth, minHeight: T.Size.windowMinHeight)
        .containerBackground(T.Palette.windowMaterial, for: .window)
    }

    /// DM mode: search; a server: its menu, whose "⌄" is the native toolbar menu indicator
    /// (a-toolbar-guild.png). The menu stays empty until server actions exist.
    @ViewBuilder
    private var sidebarButton: some View {
        if state.isDMMode {
            Button {} label: { Image(systemName: "magnifyingglass") }.help("Find or start a conversation")
        } else if state.isSignedIn {
            Menu {
                Button("Invite People") {}.disabled(true)
                Button("Server Settings") {}.disabled(true)
            } label: { Image(systemName: "ellipsis") }.help("Server menu")
        }
    }

    @ViewBuilder
    private var sidebarTitle: some View {
        if let title = state.isDMMode ? "Direct Messages" : state.serverName {
            Text(title).font(T.Typography.toolbarTitle).lineLimit(1)
                .frame(maxWidth: max(0, sidebarWidth - T.Size.sidebarToolbarReserved), alignment: .leading)
        }
    }
}

#Preview("Shell, demo", traits: .fixedLayout(width: 1200, height: 720)) {
    @Previewable @State var state = ShellState.demo
    ShellView(state: $state)
}

#Preview("Shell, DM mode", traits: .fixedLayout(width: 1200, height: 720)) {
    @Previewable @State var state = { var state = ShellState.demo; state.selectedRailID = "dm"; return state }()
    ShellView(state: $state)
}

#Preview("Shell, not signed in", traits: .fixedLayout(width: 1200, height: 720)) {
    ShellView(state: .constant(.signedOut), onSignIn: {})
}
