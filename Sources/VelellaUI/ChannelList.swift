// Adapted from Swiftcord (https://github.com/SwiftcordApp/Swiftcord, GPL-3.0):
// Swiftcord/Views/Server/ChannelList.swift, (c) Vincent Kwok.
// Changes: gateway/permissions/read-state/nsfw filtering replaced by ready-made
// `ChannelCategory` values; the sidebar `List` with Introspect scroller tweaks replaced by a
// `ScrollView` (see `SidebarScroll`) laid out with Swiftcord v2's row metrics; voice occupants
// and threads added; category headers follow Swiftcord v2 ("Name" plus a trailing chevron,
// normal case).

import SwiftUI

private typealias T = DesignTokens

/// Channel list of the selected server.
struct ChannelList: View {
    let categories: [ChannelCategory]
    @Binding var selection: String?

    /// One button row. `threadIsLast` is nil for a channel.
    private func row(for channel: ChannelItem, threadIsLast: Bool? = nil) -> some View {
        ChannelButton(channel: channel, isSelected: selection == channel.id, threadIsLast: threadIsLast) {
            selection = channel.id
        }
    }

    @ViewBuilder
    private func item(for channel: ChannelItem) -> some View {
        row(for: channel)
        ForEach(channel.occupants) { occupant in
            Label {
                Text(occupant.name).font(T.Typography.occupantName)
            } icon: {
                AvatarView(avatar: occupant.avatar, size: T.Size.occupantAvatar)
            }
            .foregroundStyle(T.Palette.readChannel)
            // Occupants start in the name column, where threads' names start too.
            .padding(.leading, T.Size.channelTitleLeading)
            .frame(height: T.Size.channelRowHeight)
        }
        ForEach(Array(channel.threads.enumerated()), id: \.element.id) { index, thread in
            row(for: thread, threadIsLast: index == channel.threads.count - 1)
        }
    }

    var body: some View {
        ScrollView {
            LazyVStack(alignment: .leading, spacing: 0) {
                ForEach(categories) { category in
                    if let name = category.name {
                        // A category header takes one row's height (c-channels-guild.png: row
                        // centres 32 pt apart, the header's included).
                        // Name then chevron from x 20, 13 pt (v2 SidebarRow header mode).
                        HStack(spacing: T.Spacing.xs) {
                            Text(name).font(T.Typography.channelCategory).lineLimit(1)
                            Image(systemName: "chevron.down").font(T.Typography.categoryChevron)
                        }
                        .foregroundStyle(T.Palette.readChannel)
                        .padding(.leading, T.Size.categoryHeaderLeading)
                        .padding(.trailing, T.Size.channelTrailing)
                        .frame(height: T.Size.channelRowHeight)
                    }
                    ForEach(category.channels) { item(for: $0) }
                }
            }
            .background(OverlayScrollerEnforcer())
        }
        .sidebarScroll()
    }
}

#Preview("Channel list") {
    ChannelList(categories: Fixtures.categories, selection: .constant("c-general"))
        .frame(width: 240, height: 560)
}

extension View {
    /// Sidebar lists start below the toolbar and scroll on under it at full strength, with the
    /// scroller below the toolbar, as in Swiftcord v2 (its top row under the traffic lights
    /// measures as bright as rows below it; its scroller starts at the toolbar's lower edge; user's
    /// captures 2026-10-13).
    func sidebarScroll() -> some View { modifier(SidebarScroll()) }
}

/// Observed 2026-10-13 (macOS 27.0.1): any scroll view whose safe area holds the toolbar fades
/// what scrolls under it, and `scrollEdgeEffectHidden` does not stop that. So the scroll view
/// leaves the safe area and takes the toolbar height back as content and scroller margins. A
/// sidebar `List` was no option: it ignores `contentMargins` (and `scrollClipDisabled`).
private struct SidebarScroll: ViewModifier {
    @State private var toolbarHeight: CGFloat = 0

    func body(content: Content) -> some View {
        content
            .contentMargins(.top, toolbarHeight, for: .scrollContent)
            .contentMargins(.top, toolbarHeight, for: .scrollIndicators)
            .ignoresSafeArea(.container, edges: .top)
            .onGeometryChange(for: CGFloat.self) { $0.safeAreaInsets.top } action: { toolbarHeight = $0 }
    }
}
