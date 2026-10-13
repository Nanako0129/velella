// Adapted from Swiftcord (https://github.com/SwiftcordApp/Swiftcord, GPL-3.0):
// Swiftcord/Views/User/CurrentUserFooter.swift, (c) Vincent Kwok.
// Changes: DiscordKit user/presence, status popover, account switcher, custom-status and login
// sheets removed; the avatar, name/username and gear layout is kept on `CurrentUser`. The
// gear is enabled but does nothing until the app supplies a settings action (there is no
// settings scene yet). Sizes, the light glass rim and the small status dot follow Swiftcord v2.

import SwiftUI

private typealias T = DesignTokens

struct CurrentUserFooter: View {
    let user: CurrentUser
    var onOpenSettings: (() -> Void)?

    var body: some View {
        HStack(spacing: T.Spacing.sm) {
            AvatarView(
                avatar: user.avatar, size: T.Size.footerAvatar, presence: user.status,
                dotSize: T.Size.footerStatusDot, dotRingWidth: T.Size.footerStatusDotRingWidth
            )

            VStack(alignment: .leading, spacing: 0) {
                Text(user.displayName).font(T.Typography.footerName).displayNameStyle(user.displayNameStyle).lineLimit(1)
                Text(user.username).font(T.Typography.footerUsername).foregroundStyle(.secondary).lineLimit(1)
            }

            Spacer()

            Button { onOpenSettings?() } label: {
                Image(systemName: "gearshape.fill").font(.system(size: T.Size.footerGear))
            }
            .buttonStyle(.plain)
            .foregroundStyle(T.Palette.footerGear)
            .help("Settings")
        }
        .frame(height: T.Size.footerHeight)
        .padding(.horizontal, T.Spacing.sm)
        .glassEffect(.regular, in: .rect(cornerRadius: T.Radius.footer))
        .overlay {
            RoundedRectangle(cornerRadius: T.Radius.footer)
                .strokeBorder(T.Palette.footerRim, lineWidth: T.Size.footerRimWidth)
        }
    }
}

#Preview("Footer") {
    CurrentUserFooter(user: Fixtures.currentUser)
        .frame(width: 240)
        .padding()
}
