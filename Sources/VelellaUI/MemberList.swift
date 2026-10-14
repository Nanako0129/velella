import SwiftUI

private typealias T = DesignTokens

/// Member list grouped by role: each section is one rounded card of rows with separators between
/// them (Swiftcord v2's member list). Fresh code (Swiftcord legacy has no matching view). Rows stay
/// in one lazy stack, each drawing its own slice of the card, so a section of thousands of members
/// still loads lazily.
struct MemberList: View {
    let sections: [MemberSection]

    var body: some View {
        if sections.isEmpty {
            ContentUnavailableView("No members", systemImage: "person.2")
        } else {
            ScrollView {
                LazyVStack(alignment: .leading, spacing: 0) {
                    ForEach(sections) { section in
                        Text(verbatim: "\(section.title) \u{2014} \(section.count)")
                            .font(T.Typography.memberSectionHeader)
                            .foregroundStyle(.secondary)
                            .lineLimit(1)
                            .padding(.leading, T.Size.memberRowInset)
                            .padding(.vertical, T.Size.memberSectionHeaderPadding)
                        ForEach(section.members) { member in
                            MemberCard(
                                member: member,
                                isFirst: member.id == section.members.first?.id,
                                isLast: member.id == section.members.last?.id
                            )
                        }
                    }
                }
                .padding([.horizontal, .bottom], T.Spacing.md)
                .background(OverlayScrollerEnforcer())
            }
        }
    }
}

/// One row of a section card: rounds the card's top corners on the first row and its bottom
/// corners on the last, and draws the separator under every row but the last.
struct MemberCard: View {
    let member: MemberRow
    var isFirst = true
    var isLast = true

    var body: some View {
        HStack(spacing: T.Size.memberAvatarGap) {
            MemberAvatar(avatar: member.avatar, status: member.status)
            VStack(alignment: .leading, spacing: -T.Size.memberStatusOverlap) {
                Text(verbatim: member.name)
                    .font(T.Typography.memberName)
                    // Inner foreground style wins, so a display-name style overrides the role colour.
                    .displayNameStyle(member.displayNameStyle)
                    .foregroundStyle(member.roleColor.map(Color.init) ?? Color.primary)
                    .lineLimit(1)
                    .frame(height: T.Size.memberNameLine)
                if let status = member.customStatus {
                    Text(verbatim: status)
                        .font(T.Typography.memberStatus)
                        .foregroundStyle(.secondary)
                        .lineLimit(1)
                        .frame(height: T.Size.memberStatusLine)
                }
            }
            Spacer(minLength: 0)
        }
        .padding(.horizontal, T.Size.memberRowInset)
        .frame(height: T.Size.memberRowHeight)
        // Only the row's content dims: the card and its separators stay as they are.
        .opacity(member.status == .offline ? T.Palette.offlineOpacity : 1)
        .background(
            T.Palette.memberCardFill,
            in: UnevenRoundedRectangle(
                topLeadingRadius: isFirst ? T.Radius.memberCard : 0,
                bottomLeadingRadius: isLast ? T.Radius.memberCard : 0,
                bottomTrailingRadius: isLast ? T.Radius.memberCard : 0,
                topTrailingRadius: isFirst ? T.Radius.memberCard : 0,
                style: .continuous
            )
        )
        .overlay(alignment: .bottom) {
            if !isLast {
                Divider().padding(.leading, T.Size.memberRowInset + T.Size.memberAvatar + T.Size.memberAvatarGap)
            }
        }
    }
}

/// Avatar with its presence dot on the rim, cut out of the avatar (v2 MemberPresenceBadge).
private struct MemberAvatar: View {
    let avatar: Avatar
    let status: PresenceStatus

    var body: some View {
        let size = T.Size.memberAvatar
        let centre = size * T.Size.memberPresenceCentre
        let dot = T.Size.statusDot
        let hole = dot * T.Size.memberPresenceCutout
        AvatarView(avatar: avatar, size: size)
            .mask {
                Path { path in
                    path.addRect(CGRect(x: 0, y: 0, width: size, height: size))
                    path.addEllipse(in: CGRect(x: centre - hole / 2, y: centre - hole / 2, width: hole, height: hole))
                }
                .fill(style: FillStyle(eoFill: true))
            }
            .overlay(alignment: .topLeading) {
                Circle()
                    .fill(status.color)
                    .frame(width: dot, height: dot)
                    .offset(x: centre - dot / 2, y: centre - dot / 2)
                    .accessibilityLabel(status.label)
            }
    }
}

#Preview("Member list") {
    MemberList(sections: Fixtures.memberSections)
        .frame(width: 256, height: 560)
}
