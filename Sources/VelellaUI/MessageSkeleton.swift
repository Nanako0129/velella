import AppKit
import SwiftUI

private typealias S = DesignTokens.Skeleton

/// Grey placeholder messages while a conversation's history loads, as Swiftcord v2 shows them:
/// static (no shimmer), bottom-aligned above the composer, replaced by the messages with no
/// transition. The bar widths come from the conversation id, so one conversation always shows
/// the same skeleton.
struct MessageSkeleton: View {
    let seed: String

    // ponytail: fixed count, enough to overflow any window; v2's own count was not decoded.
    static let rowCount = 24

    var body: some View {
        let rows = SkeletonRow.rows(seed: seed, count: Self.rowCount)
        Canvas { context, size in
            let total = rows.reduce(0) { $0 + $1.height + S.rowGap } - S.rowGap
            var top = size.height - total
            for row in rows {
                row.draw(in: context, top: top, width: size.width)
                top += row.height + S.rowGap
            }
        }
        .accessibilityLabel("Loading messages")
    }
}

/// One placeholder message: name and timestamp bar widths, one to four body lines (the last one
/// shorter) and, one row in five, a media block.
struct SkeletonRow: Equatable {
    var nameWidth: CGFloat
    var timeWidth: CGFloat
    var lineWidths: [CGFloat]
    var media: CGSize?

    /// Content bottom less the avatar's 2 top offset, at least the avatar (v2's row height).
    var height: CGFloat {
        let linesEnd = S.firstLineTop + S.linePitch * CGFloat(lineWidths.count)
        let bottom = media.map { linesEnd + S.mediaTop + $0.height } ?? linesEnd - (S.linePitch - S.lineHeight)
        return max(bottom, S.avatarTop + S.avatar) - S.avatarTop
    }

    /// `count` rows for a conversation id: decimal ids seed directly, anything else by a hash.
    static func rows(seed: String, count: Int) -> [SkeletonRow] {
        let base = UInt64(seed) ?? seed.utf8.reduce(UInt64(5381)) { ($0 &* 33) &+ UInt64($1) }
        return (0..<count).map { make(seed: base &+ UInt64($0) &* 0x9E37_79B9) }
    }

    /// Knuth's MMIX LCG; each draw maps to [0, 1] in 1/9999 steps, as v2 does.
    static func make(seed: UInt64) -> SkeletonRow {
        var state = seed &* 6_364_136_223_846_793_005 &+ 0x2CEA_EE21_BF46_BC00
        func unit(_ value: UInt64) -> CGFloat { CGFloat(value % 10_000) / 9_999 }
        func next() -> UInt64 {
            state = state &* 6_364_136_223_846_793_005 &+ 1_442_695_040_888_963_407
            return state
        }
        let name = 76 + 82 * unit(state)
        let time = 34 + 18 * unit(next())
        let lastLine = Int(next() & 3)
        let lines = (0...lastLine).map { $0 == lastLine ? 96 + 264 * unit(next()) : 220 + 320 * unit(next()) }
        var media: CGSize?
        if next() % 5 == 0 {
            let width = 180 + 140 * unit(next())
            media = CGSize(width: width, height: 92 + 92 * unit(next()))
        }
        return SkeletonRow(nameWidth: name, timeWidth: time, lineWidths: lines, media: media)
    }

    func draw(in context: GraphicsContext, top: CGFloat, width: CGFloat) {
        func fill(_ rect: CGRect, radius: CGFloat, alpha: CGFloat) {
            context.fill(Path(roundedRect: rect, cornerRadius: radius),
                         with: .color(Color(nsColor: .secondaryLabelColor.withAlphaComponent(alpha))))
        }
        let x = S.textLeading
        let room = max(width - x - S.inset, 0)
        fill(CGRect(x: S.inset, y: top + S.avatarTop, width: S.avatar, height: S.avatar),
             radius: S.avatar / 2, alpha: S.avatarAlpha)
        fill(CGRect(x: x, y: top + S.nameTop, width: min(nameWidth, room), height: S.nameHeight),
             radius: S.nameHeight / 2, alpha: S.barAlpha)
        let timeX = x + nameWidth + S.timeGap
        fill(CGRect(x: timeX, y: top + S.timeTop, width: min(timeWidth, max(width - timeX - S.inset, 0)), height: S.timeHeight),
             radius: S.timeHeight / 2, alpha: S.timeAlpha)
        var y = top + S.firstLineTop
        for line in lineWidths {
            fill(CGRect(x: x, y: y, width: min(line, room), height: S.lineHeight), radius: S.lineHeight / 2, alpha: S.barAlpha)
            y += S.linePitch
        }
        if let media {
            fill(CGRect(x: x, y: y + S.mediaTop, width: min(media.width, room), height: media.height),
                 radius: S.mediaRadius, alpha: S.mediaAlpha)
        }
    }
}

#Preview("Message skeleton") {
    MessageSkeleton(seed: "1100000000000000001").frame(width: 640, height: 560)
}
