import SwiftUI

private typealias T = DesignTokens

/// Channel column in direct-message mode: Friends / Message Requests / app rows with badges,
/// then a "Direct Messages" section of conversations. Fresh code, laid out after Swiftcord v2's
/// DM column (Swiftcord legacy's DM buttons are bound to DiscordKit channels). The top rows are
/// display-only until the engine slices give them destinations.
struct DMList: View {
    let topRows: [DMTopRow]
    let dms: [DMEntry]
    @Binding var selection: String?

    var body: some View {
        ScrollView {
            LazyVStack(alignment: .leading, spacing: 0) {
                ForEach(topRows) { row in
                    HStack(spacing: T.Spacing.md) {
                        Image(systemName: row.symbol)
                            .font(.system(size: T.Size.dmTopIcon * 0.6))
                            .frame(width: T.Size.dmTopIcon)
                        Text(row.title).font(T.Typography.dmName)
                        Spacer()
                        if row.badgeCount > 0 { MentionBadge(count: row.badgeCount) }
                    }
                    .foregroundStyle(.secondary)
                    .padding(.leading, T.Size.channelIconLeading)
                    .padding(.trailing, T.Size.channelTrailing)
                    .frame(height: T.Size.dmRowHeight)
                    .padding(.vertical, T.Size.dmRowGap / 2)
                }
                Text("Direct Messages")
                    .font(T.Typography.dmSectionHeader)
                    .foregroundStyle(.secondary)
                    .padding(.leading, T.Size.categoryHeaderLeading)
                    .frame(height: T.Size.dmSectionHeaderHeight)
                ForEach(dms) { dm in
                    DMRow(dm: dm, isSelected: selection == dm.id) { selection = dm.id }
                }
            }
            .background(OverlayScrollerEnforcer())
        }
        .sidebarScroll()
    }
}

struct DMRow: View {
    let dm: DMEntry
    let isSelected: Bool
    let onSelect: () -> Void
    @State private var hovered = false

    var body: some View {
        Button(action: onSelect) {
            // v2 SidebarRow's avatar mode: avatar at x 18, name at 62, background inset 12.
            HStack(spacing: T.Size.dmTitleLeading - T.Size.dmAvatarLeading - T.Size.dmAvatar) {
                AvatarView(avatar: dm.avatar, size: T.Size.dmAvatar, presence: dm.status)
                VStack(alignment: .leading, spacing: 0) {
                    HStack(spacing: T.Spacing.xs) {
                        Text(dm.name).displayNameStyle(dm.displayNameStyle).lineLimit(1)
                            .fontWeight(dm.unread ? .semibold : nil)
                        if let tag = dm.serverTag {
                            Text(tag)
                                .font(T.Typography.dmTag)
                                .padding(.horizontal, T.Spacing.xs)
                                .background(T.Palette.tagPillFill, in: RoundedRectangle(cornerRadius: T.Radius.tagPill))
                        }
                    }
                    .font(T.Typography.dmName)
                    if let activity = dm.activity {
                        Text(activity).font(T.Typography.dmActivity).lineLimit(1)
                    }
                }
                Spacer(minLength: 0)
                if dm.mentionCount > 0 {
                    MentionBadge(count: dm.mentionCount)
                } else if dm.unread {
                    Circle().fill(T.Palette.unread)
                        .frame(width: T.Size.channelUnreadDot, height: T.Size.channelUnreadDot)
                }
            }
            .foregroundStyle(isSelected || dm.unread ? T.Palette.unread : T.Palette.readChannel)
            .padding(.leading, T.Size.dmAvatarLeading)
            .padding(.trailing, T.Size.channelTrailing)
            .frame(height: T.Size.dmRowHeight)
            .background {
                RoundedRectangle(cornerRadius: T.Radius.row, style: .continuous)
                    .fill(isSelected ? T.Palette.channelSelectedFill : (hovered ? T.Palette.channelHoverFill : .clear))
                    .padding(.horizontal, T.Size.channelSelectionInset)
            }
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .onHover { hovered = $0 }
        .padding(.vertical, T.Size.dmRowGap / 2)
    }
}

#Preview("DM list") {
    DMList(topRows: Fixtures.dmTopRows, dms: Fixtures.dms, selection: .constant("d2"))
        .frame(width: 280, height: 560)
}
