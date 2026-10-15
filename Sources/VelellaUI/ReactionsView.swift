import AppKit
import SwiftUI

/// One emoji's reaction under a message (wave 3, track X). Read-only: velella does not add or
/// remove reactions yet.
public struct ReactionItem: Hashable, Sendable {
    /// What shows for the emoji: a custom emoji's image (URL built in core from its id), or a
    /// unicode emoji as text.
    public enum Face: Hashable, Sendable {
        case image(URL)
        case text(String)
    }

    /// The unicode emoji itself, or a custom emoji's name.
    public var name: String
    public var imageURL: URL?
    public var count: UInt32
    /// The signed-in user has reacted.
    public var me: Bool

    public init(name: String, imageURL: URL? = nil, count: UInt32, me: Bool = false) {
        self.name = name; self.imageURL = imageURL; self.count = count; self.me = me
    }

    public var face: Face { imageURL.map(Face.image) ?? .text(name) }

    /// What VoiceOver reads; the name stays verbatim, so no markup in it is ever interpreted.
    var spoken: String {
        let emoji = imageURL == nil ? name : ":\(name):"
        return "\(emoji), \(count)" + (me ? ", you reacted" : "")
    }
}

/// The reaction pill, from Discord's CSS values in the wave 3 plan (row X); 1 CSS px = 1 pt.
enum ReactionMetrics {
    static let radius: CGFloat = 8
    static let paddingVertical: CGFloat = 2
    static let paddingHorizontal: CGFloat = 6
    static let emojiSize: CGFloat = 20
    /// Unicode emoji are text. Measured with NSAttributedString: an emoji advances 23 pt at a
    /// 16 pt font (about 1.44 x), so 13.5 pt keeps it inside the `emojiSize` square (asserted in
    /// ReactionsTests). How large it then looks next to Discord's 20 px images is unchecked live.
    static let textEmojiFontSize: CGFloat = 13.5
    static let countSize: CGFloat = 14
    /// Emoji to count.
    static let countGap: CGFloat = 6
    /// Between pills, across and down.
    static let pillGap: CGFloat = 4
    /// Message text to the first row of pills (the same 4 as the gap between pills).
    static let top: CGFloat = 4
    static let borderWidth: CGFloat = 1
    static var pillHeight: CGFloat { emojiSize + 2 * paddingVertical }
}

/// Fill and border of a pill. PROVISIONAL: the plan names `me` pills "brand-tinted" without
/// values; these use Discord's blurple (#5865F2) as a 15 % fill and a solid border, and a faint
/// neutral fill with no visible border otherwise. To be checked against the official client.
enum ReactionStyle {
    static let brand = Color(red: 0x58 / 255, green: 0x65 / 255, blue: 0xF2 / 255)
    static let meFillOpacity: Double = 0.15
    static let idleFillOpacity: Double = 0.06

    static func fill(me: Bool) -> Color { me ? brand.opacity(meFillOpacity) : Color.primary.opacity(idleFillOpacity) }
    static func border(me: Bool) -> Color { me ? brand : .clear }
}

/// Wraps its children into rows, `spacing` apart in both directions.
private struct PillFlow: Layout {
    var spacing: CGFloat

    func sizeThatFits(proposal: ProposedViewSize, subviews: Subviews, cache: inout ()) -> CGSize {
        arrange(width: proposal.width ?? .infinity, subviews).size
    }

    func placeSubviews(in bounds: CGRect, proposal: ProposedViewSize, subviews: Subviews, cache: inout ()) {
        let placed = arrange(width: bounds.width, subviews)
        for (subview, origin) in zip(subviews, placed.origins) {
            subview.place(at: CGPoint(x: bounds.minX + origin.x, y: bounds.minY + origin.y), proposal: .unspecified)
        }
    }

    private func arrange(width: CGFloat, _ subviews: Subviews) -> (size: CGSize, origins: [CGPoint]) {
        var origins: [CGPoint] = []
        var x: CGFloat = 0, y: CGFloat = 0, rowHeight: CGFloat = 0, widest: CGFloat = 0
        for subview in subviews {
            let size = subview.sizeThatFits(.unspecified)
            if x > 0, x + size.width > width {
                x = 0; y += rowHeight + spacing; rowHeight = 0
            }
            origins.append(CGPoint(x: x, y: y))
            x += size.width + spacing
            rowHeight = max(rowHeight, size.height)
            widest = max(widest, x - spacing)
        }
        return (CGSize(width: widest, height: y + rowHeight), origins)
    }
}

/// The pills under a message, in the order core lists them. Nothing here is clickable.
struct ReactionsView: View {
    let reactions: [ReactionItem]

    var body: some View {
        if !reactions.isEmpty {
            PillFlow(spacing: ReactionMetrics.pillGap) {
                ForEach(Array(reactions.enumerated()), id: \.offset) { _, reaction in
                    ReactionPill(reaction: reaction)
                }
            }
            .padding(.top, ReactionMetrics.top)
        }
    }
}

struct ReactionPill: View {
    let reaction: ReactionItem
    private let images = EmojiImages.shared

    var body: some View {
        HStack(spacing: ReactionMetrics.countGap) {
            emoji
            Text(verbatim: String(reaction.count))
                .font(.system(size: ReactionMetrics.countSize, weight: .semibold))
                .foregroundStyle(.primary)
        }
        .padding(.vertical, ReactionMetrics.paddingVertical)
        .padding(.horizontal, ReactionMetrics.paddingHorizontal)
        .background(ReactionStyle.fill(me: reaction.me), in: shape)
        .overlay { shape.strokeBorder(ReactionStyle.border(me: reaction.me), lineWidth: ReactionMetrics.borderWidth) }
        .accessibilityElement(children: .ignore)
        .accessibilityLabel(Text(verbatim: reaction.spoken))
    }

    private var shape: RoundedRectangle {
        RoundedRectangle(cornerRadius: ReactionMetrics.radius, style: .continuous)
    }

    @ViewBuilder
    private var emoji: some View {
        switch reaction.face {
        case .image(let url):
            Group {
                if let image = images.loaded[url] { Image(nsImage: image).resizable().scaledToFit() } else { Color.clear }
            }
            .frame(width: ReactionMetrics.emojiSize, height: ReactionMetrics.emojiSize)
            .task { images.load(url) }
        case .text(let name):
            Text(verbatim: name)
                .font(.system(size: ReactionMetrics.textEmojiFontSize))
                .lineLimit(1)
                .frame(width: ReactionMetrics.emojiSize, height: ReactionMetrics.emojiSize)
        }
    }
}

#Preview("Reactions") {
    ReactionsView(reactions: [
        ReactionItem(name: "\u{1F44D}", count: 12, me: true),
        ReactionItem(name: "\u{1F389}", count: 3),
        ReactionItem(name: "\u{2764}\u{FE0F}", count: 1),
    ])
    .padding()
}
