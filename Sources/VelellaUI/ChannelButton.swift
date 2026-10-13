// Adapted from Swiftcord (https://github.com/SwiftcordApp/Swiftcord, GPL-3.0):
// Swiftcord/Views/Server/ChannelButton.swift (GuildChButton) and
// Swiftcord/ButtonStyles/DiscordChannelButton.swift, (c) Vincent Kwok.
// Changes: DiscordKit `Channel`/ServerContext replaced by `ChannelItem`; DM buttons dropped (DMs
// arrive as rail entries in this shell); row geometry, fonts, colours, the trailing unread dot,
// the private-channel lock and the thread connector follow Swiftcord v2's SidebarRow as recorded
// in refs/swiftcord-v2-teardown.md, kept out of git.

import SwiftUI

private typealias T = DesignTokens

/// One channel or thread row, laid out in row coordinates like v2's SidebarRow: glyph box at
/// x 20-40, name from x 48 to 20 before the trailing edge, unread dot at that trailing edge.
struct ChannelButton: View {
    let channel: ChannelItem
    let isSelected: Bool
    /// nil for a channel; for a thread, whether it is the last one under its parent (└ instead of ├).
    var threadIsLast: Bool?
    let onSelect: () -> Void
    @State private var hovered = false

    private var showsUnreadDot: Bool { channel.isUnread && !channel.isMuted }

    var body: some View {
        Button(action: onSelect) {
            HStack(spacing: 0) {
                Group {
                    if let threadIsLast {
                        ThreadConnector(isLast: threadIsLast)
                            .stroke(T.Palette.threadLine, style: StrokeStyle(lineWidth: T.Size.threadLineWidth, lineCap: .butt))
                    } else {
                        ChannelGlyph(kind: channel.kind, isPrivate: channel.isPrivate)
                    }
                }
                .frame(width: T.Size.channelIcon)
                .padding(.leading, T.Size.channelIconLeading)

                Text(channel.name)
                    .font(T.Typography.channelName)
                    .foregroundStyle(isSelected || showsUnreadDot ? T.Palette.unread : T.Palette.readChannel)
                    .opacity(channel.isMuted ? T.Palette.mutedOpacity : 1)
                    .lineLimit(1)
                    .padding(.leading, T.Size.channelTitleLeading - T.Size.channelIconLeading - T.Size.channelIcon)

                Spacer(minLength: 0)

                if showsUnreadDot {
                    Circle().fill(T.Palette.unread)
                        .frame(width: T.Size.channelUnreadDot, height: T.Size.channelUnreadDot)
                        .padding(.leading, T.Size.channelBadgeGap)
                }
            }
            .padding(.trailing, T.Size.channelTrailing)
            .frame(maxWidth: .infinity, alignment: .leading)
            .frame(height: T.Size.channelRowHeight)
            .contentShape(Rectangle())
        }
        .buttonStyle(DiscordChannelButton(isSelected: isSelected, isHovered: hovered))
        .onHover { hovered = $0 }
    }
}

/// Channel kind glyph in the 20 pt box. A private channel gets an 8 pt lock in the box's
/// top-trailing corner and the glyph is cut away 1 pt around the lock's frame (v2 SidebarRow).
///
/// v2's 20 pt UIImageView renders the symbol at about 16x17 pt (user's 2x capture, 2026-10-13),
/// because a UIKit symbol image carries padding; a resizable SwiftUI symbol has none, so the glyph
/// is fit to `channelGlyph` inside the box to land at the same size.
struct ChannelGlyph: View {
    let kind: ChannelKind
    let isPrivate: Bool

    var body: some View {
        let glyph = Image(systemName: kind.symbol).resizable().scaledToFit()
            .frame(width: T.Size.channelGlyph, height: T.Size.channelGlyph)
            .foregroundStyle(T.Palette.channelIcon)
            .frame(width: T.Size.channelIcon, height: T.Size.channelIcon)
        if isPrivate {
            glyph
                .mask {
                    Rectangle().overlay(alignment: .topTrailing) {
                        RoundedRectangle(cornerRadius: T.Radius.channelLockCutout)
                            .frame(width: T.Size.channelLock + 2 * T.Size.channelLockKnockout,
                                   height: T.Size.channelLock + 2 * T.Size.channelLockKnockout)
                            .offset(x: T.Size.channelLockKnockout, y: -T.Size.channelLockKnockout)
                            .blendMode(.destinationOut)
                    }
                    .compositingGroup()
                }
                .overlay(alignment: .topTrailing) {
                    Image(systemName: "lock.fill").resizable().scaledToFit()
                        .frame(width: T.Size.channelLock, height: T.Size.channelLock)
                        .foregroundStyle(T.Palette.channelIcon)
                }
        } else {
            glyph
        }
    }
}

/// Tree line of a thread row in the 20 pt glyph box (v2: x 29 in the row = 9 in the box): a
/// vertical from the top to 4 above the middle, a quadratic elbow to (13, middle), then a line to
/// the box's trailing edge; when another thread follows, the vertical also runs on to the bottom.
struct ThreadConnector: Shape {
    let isLast: Bool

    func path(in rect: CGRect) -> Path {
        let x = rect.minX + T.Size.threadSpineX, y = rect.midY, r = T.Size.threadElbowRadius
        var path = Path()
        path.move(to: CGPoint(x: x, y: rect.minY))
        path.addLine(to: CGPoint(x: x, y: y - r))
        path.addQuadCurve(to: CGPoint(x: x + r, y: y), control: CGPoint(x: x, y: y))
        path.addLine(to: CGPoint(x: rect.maxX, y: y))
        if !isLast {
            path.move(to: CGPoint(x: x, y: y - r))
            path.addLine(to: CGPoint(x: x, y: rect.maxY))
        }
        return path
    }
}

/// Selection and hover background, inset 12 at both sides with corner 8 (v2 SidebarRow).
struct DiscordChannelButton: ButtonStyle {
    let isSelected: Bool
    let isHovered: Bool

    func makeBody(configuration: Configuration) -> some View {
        configuration.label
            .background {
                RoundedRectangle(cornerRadius: T.Radius.row)
                    .fill(isSelected ? T.Palette.channelSelectedFill : (isHovered ? T.Palette.channelHoverFill : .clear))
                    .padding(.horizontal, T.Size.channelSelectionInset)
            }
    }
}

#Preview("Channel buttons") {
    VStack(spacing: 0) {
        ChannelButton(channel: ChannelItem(id: "1", name: "general", isUnread: true), isSelected: false) {}
        ChannelButton(channel: ChannelItem(id: "2", name: "random"), isSelected: true) {}
        ChannelButton(channel: ChannelItem(id: "3", name: "noise", isMuted: true), isSelected: false) {}
        ChannelButton(channel: ChannelItem(id: "4", name: "mods", isPrivate: true), isSelected: false) {}
    }
    .frame(width: 260)
    .padding()
}
