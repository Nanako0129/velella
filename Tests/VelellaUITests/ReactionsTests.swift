import AppKit
import SwiftUI
import Testing
@testable import VelellaUI

// Wave 3, track X: read-only reactions. Pill geometry is checked as constants (Discord's values
// from the plan) and on offscreen renders; the mapping from engine rows is pure.

private typealias R = ReactionMetrics

@MainActor private func render(_ reactions: [ReactionItem], width: CGFloat) -> CGImage? {
    let renderer = ImageRenderer(content: ReactionsView(reactions: reactions).frame(width: width, alignment: .leading)
        .environment(\.colorScheme, .light))
    renderer.scale = 1
    return renderer.cgImage
}

private func pixel(_ image: CGImage, _ x: Int, _ y: Int) -> [UInt8]? {
    guard image.bitsPerPixel == 32, let data = image.dataProvider?.data as Data? else { return nil }
    let o = y * image.bytesPerRow + x * 4
    return Array(data[o..<o + 4])
}

@Test func pillValuesAreDiscordsFromThePlan() {
    #expect(R.radius == 8)
    #expect((R.paddingVertical, R.paddingHorizontal) == (2, 6))
    #expect(R.emojiSize == 20)
    #expect(R.countSize == 14)
    #expect(R.countGap == 6)
    #expect(R.pillGap == 4)
    #expect(R.pillHeight == 24)
}

@MainActor @Test func oneRowIsOnePillHighAndPillsWrapWithTheGapBetweenRows() throws {
    let three = (1...3).map { ReactionItem(name: "\u{1F44D}", count: UInt32($0)) }
    // Wide: one row, the top gap above it.
    let wide = try #require(render(three, width: 400))
    #expect(CGFloat(wide.height) == R.top + R.pillHeight)
    // A single-digit pill is about 47 pt wide; 110 holds two (98) but not three.
    let narrow = try #require(render(three, width: 110))
    #expect(CGFloat(narrow.height) == R.top + 2 * R.pillHeight + R.pillGap)
    // No reactions, no view, no top gap.
    #expect(render([], width: 400) == nil || render([], width: 400)!.height == 0)
}

@MainActor @Test func aPillIsEmojiThenCountWithTheSpecifiedWidth() throws {
    // Unicode: 6 + 20 + 6 + count + 6. A custom emoji pill has the same frame around its image.
    let text = try #require(render([ReactionItem(name: "\u{1F44D}", count: 5)], width: 200))
    let image = try #require(render([ReactionItem(name: "x", imageURL: URL(string: "https://cdn.discordapp.com/emojis/1.webp?size=48"), count: 5)], width: 200))
    let digit = NSAttributedString(string: "5", attributes: [.font: NSFont.systemFont(ofSize: R.countSize, weight: .semibold)]).size().width
    // The pill's right edge is the last non-transparent column of the render.
    func rightEdge(_ img: CGImage) -> Int {
        (0..<img.width).last { x in (0..<img.height).contains { (pixel(img, x, $0)?[3] ?? 0) > 0 } } ?? -1
    }
    let expected = R.paddingHorizontal + R.emojiSize + R.countGap + digit + R.paddingHorizontal
    #expect(abs(CGFloat(rightEdge(text) + 1) - expected) <= 1.5, "\(rightEdge(text) + 1) vs \(expected)")
    #expect(rightEdge(text) == rightEdge(image))
}

@MainActor @Test func myPillHasTheBrandBorderAndTheOthersNone() throws {
    let mine = try #require(render([ReactionItem(name: "a", count: 1, me: true)], width: 120))
    let other = try #require(render([ReactionItem(name: "a", count: 1, me: false)], width: 120))
    let y = Int(R.top + R.pillHeight / 2)
    let border = try #require(pixel(mine, 0, y))
    #expect(border[3] == 255)
    #expect(abs(Int(border[0]) - Int(border[2])) > 100, "a blue-ish border: \(border)")
    // The idle pill's edge is only the faint fill, never a solid border.
    #expect(try #require(pixel(other, 0, y))[3] < 60)
    // The tint inside differs from the idle fill.
    #expect(ReactionStyle.fill(me: true) != ReactionStyle.fill(me: false))
    #expect(ReactionStyle.border(me: false) == .clear)
    #expect(ReactionStyle.border(me: true) == ReactionStyle.brand)
}

@Test func aUnicodeEmojiIsItsNameVerbatimAndACustomOneIsItsImage() {
    // Markup in a name is never interpreted: the face is the exact string.
    for name in ["**x**", "[a](https://x.test)", "<@1>", "\u{202E}x", "\u{1F44D}"] {
        #expect(ReactionItem(name: name, count: 1).face == .text(name))
    }
    let url = URL(string: "https://cdn.discordapp.com/emojis/7.gif?size=48")!
    #expect(ReactionItem(name: "party", imageURL: url, count: 1).face == .image(url))
    // The spoken label shows a custom emoji as :name: and never reads markup differently.
    #expect(ReactionItem(name: "party", imageURL: url, count: 3, me: true).spoken == ":party:, 3, you reacted")
    #expect(ReactionItem(name: "**x**", count: 2).spoken == "**x**, 2")
}

@Test func aUnicodeEmojiGlyphFitsItsSquare() {
    let font = NSFont.systemFont(ofSize: R.textEmojiFontSize)
    for emoji in ["\u{1F44D}", "\u{2764}\u{FE0F}", "\u{1F468}\u{200D}\u{1F469}\u{200D}\u{1F467}\u{200D}\u{1F466}", "\u{1F1F9}\u{1F1FC}", "\u{1F44D}\u{1F3FD}"] {
        let width = NSAttributedString(string: emoji, attributes: [.font: font]).size().width
        #expect(width <= R.emojiSize, "\(emoji) is \(width) wide")
    }
}

@Test func liveRowsMapReactionsInOrder() {
    let item = MessageItem.live(
        id: 1, authorId: 2, authorName: "A", authorColor: nil, avatarURL: "", timestampMs: 0, content: "x",
        body: MessageBody(text: "x"), edited: false, images: [], files: [], replyAuthor: nil, replyContent: nil,
        pending: false, failure: nil,
        reactions: [
            (name: "\u{1F44D}", url: nil, count: 3, me: true),
            (name: "party", url: "https://cdn.discordapp.com/emojis/7.webp?size=48", count: 1, me: false),
        ]
    )
    #expect(item.reactions == [
        ReactionItem(name: "\u{1F44D}", count: 3, me: true),
        ReactionItem(name: "party", imageURL: URL(string: "https://cdn.discordapp.com/emojis/7.webp?size=48"), count: 1),
    ])
    // Without reactions, the default.
    #expect(MessageItem(id: "1", authorName: "A", avatar: .initials("A"), timestamp: .now, text: "x").reactions.isEmpty)
}
