import AppKit
import SwiftUI
import Testing
@testable import VelellaUI

// Behaviour of the K3 block views (docs/plans/markdown.md, K3 acceptance): the block -> model
// mapping, and offscreen renders for what only pixels can show (quote bar, wrapping, bold
// runs, spoiler cover).

private typealias S = MarkdownBlockStyle

private let base = MarkdownText(body: MessageBody())
private func model(_ block: MarkdownBlock, depth: Int = 0) -> BlockModel { BlockModel(block, base: base, listDepth: depth) }

// MARK: Mapping

@Test func headingLevelsMapToSizeAndWeight() {
    for (level, size) in [(1, 24.0), (2, 20.0), (3, 16.0)] {
        guard case .text(_, let s, let w, let secondary, _, _) = model(.heading(level: UInt8(level), [.text("h")])) else {
            Issue.record("not text"); continue
        }
        // Discord: `h1..h6 { font-weight: var(--font-weight-bold) }`, sizes 1.5rem / 1.25rem / 1rem.
        #expect(s == size && w == .bold && !secondary)
    }
    // K1 only produces 1-3; anything else clamps rather than traps.
    for level: UInt8 in [0, 4, 255] {
        if case .text(_, let s, _, _, _, _) = model(.heading(level: level, [])) { #expect(s == (level == 0 ? 24 : 16)) }
    }
}

@Test func headingsHaveTheirOwnSpacingAndParagraphsNone() {
    let h1 = model(.heading(level: 1, [])), p = model(.paragraph([]))
    // Discord: `h1, h2, h3 { margin: 16px 0 8px }`; margins collapse, so the larger side wins.
    #expect(h1.top == 16 && h1.bottom == 8)
    #expect(p.top == 0 && p.bottom == 0)
    #expect(BlockModel.gap(after: p, before: h1) == 16)
    #expect(BlockModel.gap(after: h1, before: p) == 8)
    // Paragraphs carry no margin in the markup CSS: two stacked ones touch.
    #expect(BlockModel.gap(after: p, before: p) == 0)
    // `h1:first-child, h2:first-child { margin-top: 8px }`, `h3:first-child { margin-top: 4px }`.
    #expect([1, 2, 3].map { BlockModel.firstTop(.heading(level: UInt8($0), [])) } == [8, 8, 4])
    #expect(BlockModel.firstTop(.paragraph([])) == 0)
}

@Test func quotesAndListsAskForTheirFourPointMargins() {
    let q = model(.quote([])), l = model(.list(ordered: false, start: 1, items: []))
    // `.blockquoteContainer__75297 { margin-block: var(--space-4) }`, `ol, ul { margin-block: 4px 0 }`.
    #expect(q.top == 4 && q.bottom == 4 && l.top == 4 && l.bottom == 0)
    #expect(BlockModel.gap(after: model(.paragraph([])), before: l) == 4)
}

@Test func listIndentIsSixteenForBulletsAndGrowsWithTheOrderedMarkerWidth() {
    // `ul { margin-inline: 16px 0 }`; `ol { margin-inline-start: calc(.4em + n * .6em) }`.
    #expect(BlockModel.listIndent(markers: ["\u{2022}"], size: 16) == 16)
    #expect(BlockModel.listIndent(markers: ["1.", "2."], size: 16) == 16)
    #expect(abs(BlockModel.listIndent(markers: ["9.", "10."], size: 16) - 25.6) < 0.001)
    #expect(abs(BlockModel.listIndent(markers: ["999999999."], size: 16) - 92.8) < 0.001)
}

@Test func subtextIsFourteenPointSecondary() {
    guard case .text(_, let size, _, let secondary, _, _) = model(.subtext([.text("s")])) else { Issue.record("not text"); return }
    // Discord: `.markup__75297 small { font-size: .875rem; color: var(--text-subtle); line-height: 1.20313rem }`.
    #expect(size == 14 && secondary)
    #expect(abs(MarkdownText.lineHeight(size: 14) - 19.25) < 0.001)
}

@Test func paragraphsKeepTheSurroundingSize() {
    var small = base
    small.size = 9
    guard case .text(_, let size, let weight, let secondary, _, _) = BlockModel(.paragraph([]), base: small, listDepth: 0) else { return }
    #expect(size == 9 && weight == .regular && !secondary)
}

@Test func aQuoteIsOneBarAroundItsBlocksAndTheBarIsRounded() {
    let inner = MarkdownBlock.paragraph([.text("q")])
    #expect(model(.quote([inner, .codeBlock(language: nil, code: "c", header: nil, highlight: nil)])) == .quote([inner, .codeBlock(language: nil, code: "c", header: nil, highlight: nil)]))
    // Discord: divider 4 wide, radius 4; `blockquote { padding-inline: 12px 8px }`.
    #expect(S.quoteBarWidth == 4 && S.quoteBarRadius == 4 && S.quoteBarGap == 12 && S.quoteEndPadding == 8)
}

@Test func orderedMarkersCountFromTheStartInInt() {
    #expect(BlockModel.markers(ordered: true, start: 1, count: 3, depth: 0) == ["1.", "2.", "3."])
    #expect(BlockModel.markers(ordered: true, start: 3, count: 3, depth: 0) == ["3.", "4.", "5."])
    #expect(BlockModel.markers(ordered: true, start: 999_999_999, count: 3, depth: 0) == ["999999999.", "1000000000.", "1000000001."])
    // Past K1's clamp: still no overflow.
    #expect(BlockModel.markers(ordered: true, start: .max, count: 3, depth: 0).last == "4294967297.")
    #expect(BlockModel.markers(ordered: true, start: 1, count: 0, depth: 0) == [])
}

@Test func bulletsStepFromDiscToCircleAndThenRepeat() {
    let item = [[MarkdownBlock.paragraph([.text("x")])]]
    let glyphs = (0..<5).map { depth -> String in
        guard case .list(let markers, _) = model(.list(ordered: false, start: 1, items: item), depth: depth) else { return "" }
        return markers[0]
    }
    // Discord: `ul { list-style-type: disc }`, `ol ul, ul ul { list-style-type: circle }`.
    #expect(glyphs == ["\u{2022}", "\u{25E6}", "\u{25E6}", "\u{25E6}", "\u{25E6}"])
}

@Test func codeBlocksTakeVencordsLookUpTo2000LinesAndK3sPlainBlockBeyond() {
    let highlight = CodeHighlight(lines: [[CodeSpan(text: "a", rgb: 1)], [CodeSpan(text: "b", rgb: 2)]])
    guard case .code(let code, let header, let h, let size) = model(.codeBlock(language: "py", code: "a\nb", header: "Python", highlight: highlight)) else { Issue.record("not code"); return }
    #expect(code == "a\nb" && header == "Python" && h == highlight && size == MarkdownText.bodySize * 0.875)
    let lines = Array(repeating: "x", count: 2000).joined(separator: "\n")
    guard case .code = model(.codeBlock(language: nil, code: lines, header: nil, highlight: nil)) else { Issue.record("2000 lines"); return }
    guard case .plainCode(_, let plainSize) = model(.codeBlock(language: nil, code: lines + "\nx", header: nil, highlight: nil)) else { Issue.record("2001 lines"); return }
    #expect(plainSize == MarkdownText.bodySize * 0.875)
    // K3's plain block keeps its fill, border and radius.
    #expect(S.codeFill == MarkdownPalette.codeFill)
    #expect(S.codeBorderWidth == 0.5 && S.codeRadius == 4)
    #expect(S.codeBorder == Color(nsColor: .separatorColor).opacity(0.35))
}

@Test func unchangedRowsCompareEqualAndRevealedOnesDoNot() {
    let body = MessageBody(blocks: [.paragraph([.spoiler(id: 0, [.text("s")])])])
    let a = BlockList(blocks: body.blocks, text: MarkdownText(body: body))
    #expect(a == BlockList(blocks: body.blocks, text: MarkdownText(body: body)))
    #expect(a != BlockList(blocks: body.blocks, text: MarkdownText(body: body, revealed: [0])))
    #expect(a != BlockList(blocks: body.blocks, style: InlineStyle(bold: true), text: MarkdownText(body: body)))
}

// MARK: Renders

private struct Pixels {
    let image: CGImage
    let data: Data
    var width: Int { image.width }
    var height: Int { image.height }

    /// A hosted render (selection plan N1: message text is an AppKit view, which `ImageRenderer`
    /// does not draw), light, at 1x, RGBA.
    @MainActor init?<V: View>(hosted view: V, width: CGFloat = 240) {
        guard let render = TextHarness.render(view, width: width, scale: 1),
              let provider = CGDataProvider(data: render.data as CFData),
              let image = CGImage(width: render.width, height: render.height, bitsPerComponent: 8, bitsPerPixel: 32, bytesPerRow: render.bytesPerRow,
                                  space: CGColorSpace(name: CGColorSpace.sRGB)!, bitmapInfo: CGBitmapInfo(rawValue: CGImageAlphaInfo.premultipliedLast.rawValue),
                                  provider: provider, decode: nil, shouldInterpolate: false, intent: .defaultIntent) else { return nil }
        self.image = image
        data = render.data
    }

    /// (r, g, b, a) at a pixel, as stored.
    func at(_ x: Int, _ y: Int) -> [UInt8] {
        let o = y * image.bytesPerRow + x * 4
        return Array(data[o..<o + 4])
    }

    /// The distinct pixels well inside the image (clear of rounded corners and borders).
    var colours: Set<[UInt8]> {
        Set((4..<max(5, height - 4)).flatMap { y in (4..<max(5, width - 4)).map { x in at(x, y) } })
    }

    /// Columns holding any non-transparent pixel, as a range.
    var inkColumns: ClosedRange<Int>? {
        let cols = (0..<width).filter { x in (0..<height).contains { at(x, $0)[3] > 40 } }
        return cols.first.map { $0...cols.last! }
    }
}

@MainActor private func render(_ blocks: [MarkdownBlock], width: CGFloat = 240) -> Pixels? {
    Pixels(hosted: BlockList(blocks: blocks, text: MarkdownText(body: MessageBody(blocks: blocks))), width: width)
}

@MainActor @Test func aQuoteDrawsItsBarAlongTheLeftEdgeAndAParagraphDoesNot() throws {
    let text = [MarkdownBlock.paragraph([.text("one line")]), .paragraph([.text("another")])]
    let quoted = try #require(render([.quote(text)]))
    let plain = try #require(render(text))
    #expect(quoted.height == plain.height)
    // The bar covers the full height of the two paragraphs, 4 pt wide, then the text starts 12 pt on.
    // (Clear of the rounded ends.)
    for y in [4, quoted.height / 2, quoted.height - 5] {
        // separatorColor is faint (alpha ~0.1); the bar's pixel differs from the bare background.
        #expect(Int(quoted.at(2, y)[3]) - Int(plain.at(2, y)[3]) > 12, "no bar at row \(y)")
    }
    #expect(quoted.at(4 + 4, quoted.height / 2)[3] < 10, "gap between bar and text")
}

@MainActor @Test func listItemsWrapBesideTheirMarker() throws {
    let long = String(repeating: "word ", count: 40)
    let list = try #require(render([.list(ordered: true, start: 9, items: [[.paragraph([.text(long)])], [.paragraph([.text("b")])]])], width: 200))
    let one = try #require(render([.list(ordered: true, start: 9, items: [[.paragraph([.text("a")])], [.paragraph([.text("b")])]])], width: 200))
    #expect(list.height > one.height * 3, "a long item wraps inside the column")
    // Never wider than the column.
    #expect(list.width == 200)
}

@MainActor @Test func aCodeBlockWrapsOnTheThemeBackgroundWithItsRadius() throws {
    let short = try #require(render([.codeBlock(language: nil, code: "x = 1", header: nil, highlight: nil)], width: 200))
    let long = try #require(render([.codeBlock(language: nil, code: String(repeating: "x = 1; ", count: 40), header: nil, highlight: nil)], width: 200))
    // Vencord: `white-space: pre-wrap; overflow-wrap: anywhere`. Never wider than the column.
    #expect(long.height > short.height * 2, "a long line wraps")
    #expect(long.width == 200)
    // Dark+'s editor background (#1E1E1E) inside the right padding; the corner is cut by the radius.
    let edge = short.at(short.width - 3, short.height / 2)
    #expect(edge[0...2].allSatisfy { abs(Int($0) - 0x1E) <= 4 } && edge[3] == 255, "\(edge)")
    #expect(short.at(0, 0)[3] < 255)
    // Lines are kept.
    let two = try #require(render([.codeBlock(language: nil, code: "a\nb", header: nil, highlight: nil)], width: 200))
    #expect(two.height > short.height)
}

@MainActor @Test func headingsAreTallerThanBodyAndBoldContainersWidenBlockText() throws {
    let h = try #require(render([.heading(level: 1, [.text("Title")])]))
    let p = try #require(render([.paragraph([.text("Title")])]))
    let s = try #require(render([.subtext([.text("Title")])]))
    #expect(h.height > p.height && p.height > s.height)

    // `**# mmmmmm**` and `# mmmmmm` are both semibold; a paragraph in bold is wider than plain.
    let word = [MarkdownInline.text("mmmmmmmmmm")]
    let plain = try #require(Pixels(hosted: InlineFlow(inlines: [.block(.paragraph(word))], text: base)))
    let bold = try #require(Pixels(hosted: InlineFlow(inlines: [.bold([.block(.paragraph(word))])], text: base)))
    let widthPlain = try #require(plain.inkColumns).count, widthBold = try #require(bold.inkColumns).count
    #expect(widthBold > widthPlain, "\(widthBold) vs \(widthPlain)")
}

@MainActor @Test func aCodeBlockInABoldContainerIsUnchanged() throws {
    let code = MarkdownBlock.codeBlock(language: nil, code: "mmmmmmmmmm", header: nil, highlight: nil)
    // Hosted: `ImageRenderer` draws the AppKit code view as a placeholder that is not byte-stable.
    let plain = try #require(Pixels(hosted: InlineFlow(inlines: [.block(code)], text: base)))
    let bold = try #require(Pixels(hosted: InlineFlow(inlines: [.bold([.italic([.underline([.strike([.block(code)])])])])], text: base)))
    #expect(plain.data == bold.data)
}

@MainActor @Test func aBlockUnderAHiddenSpoilerStaysCoveredUntilRevealed() throws {
    func flow(revealed: Set<Int>) -> Pixels? {
        let inlines: [MarkdownInline] = [.spoiler(id: 0, [.block(.codeBlock(language: nil, code: "WWWWWWWWWW", header: nil, highlight: nil))])]
        let body = MessageBody(blocks: [.paragraph(inlines)])
        guard case .paragraph(let numbered) = body.blocks[0] else { return nil }
        return Pixels(hosted: InlineFlow(inlines: numbered, text: MarkdownText(body: body, revealed: revealed)), width: 200)
    }
    let hidden = try #require(flow(revealed: []))
    let shown = try #require(flow(revealed: [0]))
    // Hidden: a flat cover, no glyph edges; revealed: glyph antialiasing makes many colours.
    #expect(hidden.colours.count == 1, "\(hidden.colours.count) colours under the cover")
    #expect(shown.colours.count > 2)
    // A heading under the same cover is covered as well, not only code: at most the cover and
    // the cover over the heading's own covered runs, never glyph edges.
    let body = MessageBody(blocks: [.paragraph([.spoiler(id: 0, [.block(.heading(level: 1, [.text("WWWW")]))])])])
    guard case .paragraph(let numbered) = body.blocks[0] else { return }
    let heading = try #require(Pixels(hosted: InlineFlow(inlines: numbered, text: MarkdownText(body: body)), width: 200))
    #expect(heading.colours.count <= 2, "\(heading.colours.count) colours")
}
