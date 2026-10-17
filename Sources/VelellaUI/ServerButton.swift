// Adapted from Swiftcord (https://github.com/SwiftcordApp/Swiftcord, GPL-3.0):
// Swiftcord/Views/Server/ServerButton.swift, (c) Vincent Kwok.
// Changes: DiscordKit/CachedAsyncImage/SwiftyGif icon sources replaced by the `Avatar`
// view-state type and plain AsyncImage; hover handled by the button itself; geometry follows
// Swiftcord v2's GuildRailRow / GuildIndicator / GuildMentionBadge as recorded in
// refs/swiftcord-v2-teardown.md, kept out of git (46 pt squircle icon centred in the 82 pt rail, half-hidden
// indicator, badge inside the icon corner with a cut-out ring).

import SwiftUI

private typealias T = DesignTokens

/// Bar on the rail's leading edge: 46 tall when selected, 23 on hover, an 8 pt dot when unread.
/// Shown it sits at x = -4 so the rail's edge hides half of it; hidden it slides to -6 and fades.
struct RailIndicator: View {
    let height: CGFloat
    let visible: Bool

    var body: some View {
        Capsule()
            .fill(T.Palette.unread)
            .frame(width: T.Size.indicatorWidth, height: visible ? height : 0)
            .opacity(visible ? 1 : 0)
            .offset(x: visible ? T.Size.indicatorShownX : T.Size.indicatorHiddenX)
            .animation(T.Motion.railSpring, value: height)
            .animation(T.Motion.railSpring, value: visible)
    }
}

struct ServerButton: View {
    let selected: Bool
    let name: String
    let icon: Avatar
    var hasUnread = false
    var mentionCount = 0
    var isMuted = false
    let onSelect: () -> Void
    @State private var hovered = false

    private var indicatorHeight: CGFloat {
        selected ? T.Size.indicatorSelectedHeight
            : hovered ? T.Size.indicatorHoverHeight : T.Size.indicatorRestHeight
    }

    var body: some View {
        Button(name, action: onSelect)
            .buttonStyle(ServerButtonStyle(selected: selected, icon: icon, hovered: hovered, mentionCount: mentionCount))
            .opacity(isMuted ? T.Palette.mutedOpacity : 1)
            .onHover { hovered = $0 }
            .help(name)
            .frame(width: T.Size.railWidth)
            .overlay(alignment: .leading) {
                RailIndicator(height: indicatorHeight, visible: selected || hovered || hasUnread)
            }
            .accessibilityElement(children: .combine)
            .accessibilityLabel(mentionCount > 0 ? "\(name), \(mentionCount) mentions" : name)
    }
}

extension ServerButton {
    /// The one place a `RailGuild` becomes a rail button, shared by `ServerRail` and
    /// `ServerFolder` so a muted server is dimmed whether it sits in a folder or not.
    init(guild: RailGuild, selected: Bool, onSelect: @escaping () -> Void) {
        self.init(
            selected: selected, name: guild.name, icon: guild.icon, hasUnread: guild.hasUnread,
            mentionCount: guild.mentionCount, isMuted: guild.isMuted, onSelect: onSelect
        )
    }
}

struct ServerButtonStyle: ButtonStyle {
    let selected: Bool
    let icon: Avatar
    let hovered: Bool
    var mentionCount = 0

    private var active: Bool { hovered || selected }

    private var hasArtwork: Bool {
        if case .url = icon { return true }
        return false
    }

    /// Artwork fills the squircle edge to edge with nothing behind it, so its transparent pixels
    /// show the rail (Swiftcord v2, user's capture 2026-10-17); initials and symbols sit on a tile.
    private var fill: Color {
        if hasArtwork { return .clear }
        return active ? Color.accentColor : T.Palette.railIconIdleFill
    }

    func makeBody(configuration: Configuration) -> some View {
        AvatarGlyph(
            avatar: icon,
            initialsFont: .system(size: max(T.Size.railIcon * T.Typography.railInitialsScale,
                                            T.Typography.railInitialsMinSize)),
            symbolFont: T.Typography.railSymbol
        )
        .padding(hasArtwork ? 0 : T.Spacing.xs)
        .frame(width: T.Size.railIcon, height: T.Size.railIcon)
        .foregroundStyle(active ? Color.white : Color.primary)
        .background(fill)
        .clipShape(RoundedRectangle(cornerRadius: T.Radius.railIcon, style: .continuous))
        .mentionBadge(mentionCount)
        .offset(y: configuration.isPressed ? 1 : 0)
        .animation(.none, value: configuration.isPressed)
        .animation(T.Motion.railSpring, value: active)
    }
}

/// Rail mention counter: 16 pt red capsule inside the icon's bottom-trailing corner; the icon is
/// cut away 2 pt around it instead of drawing a ring, so the rail shows through (v2).
struct RailMentionBadge: View {
    let count: Int

    var body: some View {
        Text(count > 99 ? "99+" : "\(count)")
            .font(T.Typography.channelBadge)
            .foregroundStyle(T.Palette.mentionBadgeText)
            .padding(.horizontal, T.Size.channelBadgePadding)
            .frame(minWidth: T.Size.railBadgeHeight, minHeight: T.Size.railBadgeHeight)
            .background(T.Palette.mentionBadge, in: Capsule())
            .fixedSize()
    }
}

extension View {
    /// Puts a `RailMentionBadge` in the bottom-trailing corner and cuts the view away around it.
    @ViewBuilder
    func mentionBadge(_ count: Int) -> some View {
        if count > 0 {
            self
                .mask {
                    Rectangle().overlay(alignment: .bottomTrailing) {
                        RailMentionBadge(count: count).hidden()
                            .padding(T.Size.railBadgeCutout)
                            .background(Color.black, in: RoundedRectangle(cornerRadius: T.Radius.railBadgeCutout))
                            .offset(x: T.Size.railBadgeCutout, y: T.Size.railBadgeCutout)
                            .blendMode(.destinationOut)
                    }
                    .compositingGroup()
                }
                .overlay(alignment: .bottomTrailing) { RailMentionBadge(count: count) }
        } else {
            self
        }
    }
}

#Preview("Server button") {
    VStack {
        ServerButton(selected: true, name: "Lorem Ipsum Club", icon: .initials("LI")) {}
        ServerButton(selected: false, name: "Example Workshop", icon: .initials("EW"), hasUnread: true, mentionCount: 3) {}
    }
    .padding()
}
