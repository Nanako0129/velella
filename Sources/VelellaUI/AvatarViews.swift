import SwiftUI

private typealias T = DesignTokens

extension Color {
    init(_ rgb: RGB) { self.init(red: rgb.red, green: rgb.green, blue: rgb.blue) }
}

extension PresenceStatus {
    var color: Color {
        switch self {
        case .online: T.Palette.online
        case .idle: T.Palette.idle
        case .doNotDisturb: T.Palette.doNotDisturb
        case .offline: T.Palette.offline
        }
    }

    var label: String {
        switch self {
        case .online: "Online"
        case .idle: "Idle"
        case .doNotDisturb: "Do Not Disturb"
        case .offline: "Offline"
        }
    }
}

/// Artwork, initials or symbol, with no background or clipping; callers frame and clip it.
/// Network images use plain `AsyncImage`; fixtures never use `.url`.
struct AvatarGlyph: View {
    let avatar: Avatar
    let initialsFont: Font
    var symbolFont: Font = .body
    var minimumScale: CGFloat = T.Typography.railInitialsMinScale

    var body: some View {
        switch avatar {
        case .url(let url):
            AsyncImage(url: url) { phase in
                if let image = phase.image {
                    image.resizable().scaledToFill()
                } else {
                    Color.clear
                }
            }
        case .initials(let text):
            Text(text).font(initialsFont).lineLimit(1).minimumScaleFactor(minimumScale)
        case .symbol(let name):
            Image(systemName: name).font(symbolFont)
        }
    }
}

/// Circular avatar on a name-derived colour, optional presence dot. Fresh code (Swiftcord's
/// AvatarWithPresence depends on DiscordKit types).
struct AvatarView: View {
    let avatar: Avatar
    let size: CGFloat
    var presence: PresenceStatus?
    var dotSize: CGFloat = T.Size.statusDot
    var dotRingWidth: CGFloat = T.Size.statusDotRingWidth

    var body: some View {
        let background = Self.background(for: avatar)
        ZStack {
            Color(background)
            AvatarGlyph(
                avatar: avatar,
                initialsFont: .system(size: size * T.Typography.initialsScale, weight: .medium),
                symbolFont: .system(size: size * T.Typography.initialsScale)
            )
            .foregroundStyle(ContrastText.color(on: background))
        }
        .frame(width: size, height: size)
        .clipShape(Circle())
        .overlay(alignment: .bottomTrailing) {
            if let presence { PresenceDot(status: presence, size: dotSize, ringWidth: dotRingWidth) }
        }
    }

    /// Stable across launches: Swift's `hashValue` is randomised per process, so hash by hand.
    static func background(for avatar: Avatar) -> RGB {
        guard case .initials(let text) = avatar else { return RGB(0.45, 0.45, 0.47) }
        let hash = text.unicodeScalars.reduce(UInt32(5381)) { ($0 &* 33) &+ $1.value }
        return RGB(
            hue: Double(hash % 360) / 360,
            saturation: T.Palette.avatarSaturation, brightness: T.Palette.avatarBrightness
        )
    }
}

struct PresenceDot: View {
    let status: PresenceStatus
    var size: CGFloat = T.Size.statusDot
    var ringWidth: CGFloat = T.Size.statusDotRingWidth
    var body: some View {
        Circle()
            .fill(status.color)
            .frame(width: size, height: size)
            .overlay(Circle().stroke(T.Palette.statusDotRing, lineWidth: ringWidth))
            .offset(x: T.Size.statusDotOffset, y: T.Size.statusDotOffset)
            .accessibilityLabel(status.label)
    }
}

/// Red capsule with a ring in the window colour, as on Swiftcord v2's rail.
struct MentionBadge: View {
    let count: Int
    var body: some View {
        Text(count > 99 ? "99+" : "\(count)")
            .font(T.Typography.badge)
            .foregroundStyle(T.Palette.mentionBadgeText)
            .padding(.horizontal, T.Spacing.xs)
            .frame(minWidth: T.Size.mentionBadgeMin, minHeight: T.Size.mentionBadgeMin)
            .background(T.Palette.mentionBadge, in: Capsule())
            .overlay(Capsule().stroke(T.Palette.statusDotRing, lineWidth: T.Size.mentionBadgeRingWidth))
    }
}

// MARK: Display name styles

// TODO(display-name-styles): field names unconfirmed; verify from live payload in S3
extension View {
    /// Renders a Nitro display-name style; nil leaves the view exactly as it was.
    @ViewBuilder
    func displayNameStyle(_ style: DisplayNameStyle?) -> some View {
        if let style {
            self
                .fontDesign(style.font.design)
                .foregroundStyle(style.foreground)
                .shadow(color: style.effect == .glow ? (style.colors.first.map(Color.init) ?? .clear) : .clear,
                        radius: style.effect == .glow ? T.Palette.nameGlowRadius : 0)
        } else {
            self
        }
    }
}

private extension DisplayNameStyle.Font {
    /// System design stand-ins; custom font files are not shipped (open question for S3).
    var design: Font.Design {
        switch self {
        case .default: .default
        case .serif: .serif
        case .rounded: .rounded
        case .monospaced: .monospaced
        }
    }
}

private extension DisplayNameStyle {
    var foreground: AnyShapeStyle {
        switch colors.count {
        case 0: AnyShapeStyle(.primary)
        case 1: AnyShapeStyle(Color(colors[0]))
        default: AnyShapeStyle(LinearGradient(colors: colors.map(Color.init), startPoint: .leading, endPoint: .trailing))
        }
    }
}
