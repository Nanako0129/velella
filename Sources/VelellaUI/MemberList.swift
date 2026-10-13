import SwiftUI

private typealias T = DesignTokens

/// Member list grouped by role, with a rounded card per member. Fresh code (Swiftcord legacy has
/// no matching view; layout follows the standard Discord member list as in the reference).
struct MemberList: View {
    let sections: [MemberSection]

    var body: some View {
        if sections.isEmpty {
            ContentUnavailableView("No members", systemImage: "person.2")
        } else {
            ScrollView {
                LazyVStack(alignment: .leading, spacing: T.Spacing.sm) {
                    ForEach(sections) { section in
                        Text("\(section.title) \u{2014} \(section.count)")
                            .font(T.Typography.categoryHeader)
                            .foregroundStyle(.secondary)
                            .padding(.top, T.Spacing.sm)
                        ForEach(section.members) { MemberCard(member: $0) }
                    }
                }
                .padding(T.Spacing.md)
                .background(OverlayScrollerEnforcer())
            }
        }
    }
}

struct MemberCard: View {
    let member: MemberRow

    var body: some View {
        HStack(spacing: T.Spacing.sm) {
            AvatarView(avatar: member.avatar, size: T.Size.memberAvatar, presence: member.status)
            VStack(alignment: .leading, spacing: 0) {
                Text(member.name)
                    .font(T.Typography.memberName)
                    // Inner foreground style wins, so a display-name style overrides the role colour.
                    .displayNameStyle(member.displayNameStyle)
                    .foregroundStyle(member.roleColor.map(Color.init) ?? Color.primary)
                    .lineLimit(1)
                if let status = member.customStatus {
                    Text(status).font(T.Typography.memberStatus).foregroundStyle(.secondary).lineLimit(1)
                }
            }
            Spacer(minLength: 0)
        }
        .padding(T.Spacing.sm)
        .background(T.Palette.memberCardFill, in: RoundedRectangle(cornerRadius: T.Radius.memberCard, style: .continuous))
        .opacity(member.status == .offline ? T.Palette.offlineOpacity : 1)
    }
}

#Preview("Member list") {
    MemberList(sections: Fixtures.memberSections)
        .frame(width: 240, height: 560)
}
