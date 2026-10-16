import AppKit
import SwiftUI
import Testing
@testable import VelellaUI

// P1 (docs/plans/markdown.md): Swiftcord v2's tooltip placement, the locale from macOS, and the
// paragraph's trailing line break.

// MARK: Tooltip placement (v2 0x102a07570)

private let column = CGRect(x: 0, y: 0, width: 600, height: 2000)
private let tipSize = CGSize(width: 120, height: 33)

@Test func aTooltipIsCentredAboveItsSourceSixPointsAway() {
    let source = CGRect(x: 300, y: 400, width: 40, height: 18)
    let origin = TooltipPlacement.origin(size: tipSize, source: source, container: column)
    #expect(origin.x == source.midX - tipSize.width / 2)
    #expect(origin.y == source.minY - tipSize.height - 6)
    #expect(TooltipStyle.spacing == 6)
}

@Test func aTooltipStaysInsideTheContainerInsetByEightPoints() {
    let left = TooltipPlacement.origin(size: tipSize, source: CGRect(x: 0, y: 400, width: 20, height: 18), container: column)
    #expect(left.x == 8)
    let right = TooltipPlacement.origin(size: tipSize, source: CGRect(x: 590, y: 400, width: 10, height: 18), container: column)
    #expect(right.x == 600 - 8 - tipSize.width)
    // Wider than the container: the left edge wins.
    let wide = TooltipPlacement.origin(size: CGSize(width: 700, height: 33), source: CGRect(x: 300, y: 400, width: 10, height: 18), container: column)
    #expect(wide.x == 8)
}

@Test func aTooltipFlipsBelowWhenItDoesNotFitAbove() {
    let source = CGRect(x: 300, y: 20, width: 40, height: 18)
    let origin = TooltipPlacement.origin(size: tipSize, source: source, container: column)
    #expect(origin.y == source.maxY + 6)
    // No room either side: it ends at the container's top inset.
    let tiny = CGRect(x: 0, y: 0, width: 600, height: 40)
    #expect(TooltipPlacement.origin(size: tipSize, source: CGRect(x: 300, y: 10, width: 40, height: 18), container: tiny).y == 8)
}

@Test func tooltipSizesAreTruncatedToWholePoints() {
    let origin = TooltipPlacement.origin(size: CGSize(width: 120.9, height: 33.9), source: CGRect(x: 300, y: 400, width: 40, height: 18), container: column)
    #expect(origin == CGPoint(x: 320 - 60, y: 400 - 33 - 6))
}

// MARK: Locale

@Test func theLocaleFollowsTheInjectedPreferredLanguage() {
    #expect(ShellState(preferredLanguages: ["zh-TW", "en-US"]).locale == "zh-TW")
    #expect(ShellState(preferredLanguages: []).locale == "en-US")
    #expect(ShellState().locale == Locale.preferredLanguages.first ?? "en-US")
}

// MARK: Trailing line break

private let br = MarkdownInline.lineBreak

@Test func aParagraphDropsOneTrailingBreakAndKeepsTheRest() {
    #expect(InlinePart.paragraph([.text("a"), br]) == [.inlines([.text("a")])])
    #expect(InlinePart.paragraph([.text("a"), br, br]) == [.inlines([.text("a"), br])])
    #expect(InlinePart.paragraph([.text("a"), br, .text("b")]) == [.inlines([.text("a"), br, .text("b")])])
    #expect(InlinePart.paragraph([br]) == [])
}

/// The height of a hosted render at 1x (selection plan N1: message text is an AppKit view).
@MainActor private func height(_ blocks: [MarkdownBlock]) throws -> Int {
    let view = BlockList(blocks: blocks, text: MarkdownText(body: MessageBody(blocks: blocks)))
    return try #require(TextHarness.render(view.frame(width: 240, alignment: .topLeading), width: 240, scale: 1)).height
}

@MainActor @Test func aQuoteOfOneLineWithATrailingBreakIsOneLineHigh() throws {
    let one = try height([.quote([.paragraph([.text("單行引用")])])])
    #expect(try height([.quote([.paragraph([.text("單行引用"), br])])]) == one)
    #expect(try height([.paragraph([.text("單行引用"), br])]) == (try height([.paragraph([.text("單行引用")])])))
}

@MainActor @Test func twoTrailingBreaksRenderOneEmptyLine() throws {
    let one = try height([.paragraph([.text("a")])])
    let two = try height([.paragraph([.text("a"), br, .text("b")])])
    let blank = try height([.paragraph([.text("a"), br, br])])
    #expect(two > one)
    #expect(blank == two, "an empty line is as high as a line of text")
}

// MARK: Overlay

/// Rows and columns where a render with `request` differs from one without.
@MainActor private func inkDifference(_ request: TooltipRequest?) throws -> (rows: ClosedRange<Int>, columns: ClosedRange<Int>)? {
    func render(_ request: TooltipRequest?) throws -> (CGImage, Data) {
        let view = Text(verbatim: "host").frame(width: 60, height: 20)
            .tooltipOverlay(request)
            .padding(.horizontal, 100).padding(.vertical, 80)
            .environment(\.tooltipSurface, .solid)
        let renderer = ImageRenderer(content: view)
        renderer.scale = 1
        let image = try #require(renderer.cgImage)
        return (image, try #require(image.dataProvider?.data as Data?))
    }
    let (image, ink) = try render(request)
    let (_, bare) = try render(nil)
    var rows: [Int] = [], columns: [Int] = []
    for y in 0..<image.height {
        for x in 0..<image.width where ink[y * image.bytesPerRow + x * 4 + 3] != bare[y * image.bytesPerRow + x * 4 + 3] {
            rows.append(y); columns.append(x)
        }
    }
    guard let top = rows.min(), let bottom = rows.max(), let left = columns.min(), let right = columns.max() else { return nil }
    return (top...bottom, left...right)
}

@MainActor @Test func theOverlayDrawsTheTipAboveTheHostAndCentredOnIt() throws {
    // The host is x 100..160, y 80..100 in a 260 x 180 image.
    let drawn = try #require(try inkDifference(TooltipRequest(text: "Tuesday, April 20, 2021", rect: nil)))
    #expect(drawn.rows.upperBound < 80, "the tip sits above the host")
    #expect(abs((drawn.columns.lowerBound + drawn.columns.upperBound) / 2 - 130) <= 3, "centred on the host")
    #expect(try inkDifference(nil) == nil)
}

@MainActor @Test func aTipKeepsItsOwnLineHeightUnderAParagraphsLineBox() {
    // Placed over a paragraph, the tip inherits its environment; the paragraph's 22 pt line box
    // made it 38 pt tall. v2's capsule is 33 (14 pt medium text, 8 above and below; user's capture).
    func height(_ view: some View) -> CGFloat {
        NSHostingView(rootView: view.environment(\.tooltipSurface, .solid)).fittingSize.height
    }
    let text = "Saturday, September 13, 275760 at 8:00 AM"
    let bare = height(TooltipBubble(text: text))
    #expect(height(TooltipBubble(text: text).lineHeight(.exact(points: 22))) == bare)
    #expect(abs(bare - 33) <= 1, "\(bare)")
}

@MainActor @Test func aTipHugsItsTextEvenWhenItsHostOffersMore() throws {
    // v2 sizes a tip to its content (sizeThatFits, at most 360); `.frame(maxWidth:)` stretched it
    // to the host's width. Text 14 pt medium plus 12 on each side.
    let text = "Saturday, September 13, 275760 at 8:00 AM"
    let natural = NSAttributedString(string: text, attributes: [.font: NSFont.systemFont(ofSize: 14, weight: .medium)]).size().width
    let view = TooltipBubble(text: text).environment(\.tooltipSurface, .solid)
        .frame(width: 600, alignment: .leading)
    let renderer = ImageRenderer(content: view)
    renderer.scale = 1
    let image = try #require(renderer.cgImage)
    let data = try #require(image.dataProvider?.data as Data?)
    let y = image.height / 2
    let inked = (0..<image.width).filter { data[y * image.bytesPerRow + $0 * 4 + 3] > 0 }
    let width = CGFloat(try #require(inked.max()) - (inked.min() ?? 0) + 1)
    #expect(abs(width - (natural + 24)) <= 2, "\(width) vs \(natural + 24)")
    // A long tip still wraps at 360.
    let long = NSHostingView(rootView: TooltipBubble(text: String(repeating: "word ", count: 60)).environment(\.tooltipSurface, .solid))
    #expect(long.fittingSize.width <= TooltipStyle.maxWidth)
}
