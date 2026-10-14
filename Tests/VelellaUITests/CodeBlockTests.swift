import AppKit
import SwiftUI
import Testing
@testable import VelellaUI

// The K6 code-block view (docs/plans/markdown.md, K6 "Tests", Swift): header, line numbers,
// escapes, Copy, and spans that only ever belong to their own block.

private typealias S = CodeBlockStyle

private let python = CodeHighlight(name: "Python", lines: [
    [CodeSpan(text: "x", rgb: 0x9CDCFE), CodeSpan(text: " = ", rgb: 0xD4D4D4), CodeSpan(text: "1", rgb: 0xB5CEA8)],
    [],
    [CodeSpan(text: "# c", rgb: 0x6A9955, italic: true)],
])
private let pythonCode = "x = 1\n\n# c"

private func lineTexts(_ model: CodeBlockModel) -> [String] { model.lines.map { $0.map(\.text).joined() } }

/// Distinct (r, g, b, a) pixels of an offscreen render.
@MainActor private func pixels<V: View>(_ view: V, width: CGFloat = 240) -> (height: Int, colours: [[UInt8]])? {
    let renderer = ImageRenderer(content: view.frame(width: width).environment(\.colorScheme, .light))
    renderer.scale = 1
    guard let image = renderer.cgImage, image.bitsPerPixel == 32, let data = image.dataProvider?.data as Data? else { return nil }
    var out: [[UInt8]] = []
    for y in 0..<image.height {
        for x in 0..<image.width {
            let o = y * image.bytesPerRow + x * 4
            out.append(Array(data[o..<o + 4]))
        }
    }
    return (image.height, out)
}

/// The Copy button's accent (#007BC8): strongly blue, unlike anything else in a plain block.
private func hasCopyAccent(_ colours: [[UInt8]]) -> Bool {
    colours.contains { p in
        let (a, c) = (Int(p[0]), Int(p[2])) // r and b, or b and r: either order is far apart
        return abs(a - c) > 120 && min(a, c) < 30 && p[3] == 255
    }
}

@Test func theHeaderShowsOnlyWithAResolvedLanguage() {
    #expect(CodeBlockModel(code: pythonCode, highlight: python).name == "Python")
    // No language, or one core's table does not know: core sends no highlight, so no header.
    #expect(CodeBlockModel(code: pythonCode, highlight: nil).name == nil)
}

@MainActor @Test func theHeaderAddsARowAboveTheCode() throws {
    let plain = try #require(pixels(CodeBlockView(code: pythonCode, highlight: nil, size: 14, style: InlineStyle(), revealed: [])))
    let named = try #require(pixels(CodeBlockView(code: pythonCode, highlight: python, size: 14, style: InlineStyle(), revealed: [])))
    #expect(named.height > plain.height + 10, "\(named.height) vs \(plain.height)")
}

@Test func lineNumbersRunFromOneToN() {
    #expect(CodeBlockModel(code: pythonCode, highlight: python).numbers == ["1", "2", "3"])
    #expect(CodeBlockModel(code: "a\n\nb\n", highlight: nil).numbers == ["1", "2", "3", "4"])
    #expect(lineTexts(CodeBlockModel(code: "a\n\nb\n", highlight: nil)) == ["a", "", "b", ""])
    let many = Array(repeating: "x", count: 2000).joined(separator: "\n")
    #expect(CodeBlockModel(code: many, highlight: nil).numbers.last == "2000")
}

@MainActor @Test func longLinesWrapInsideTheColumn() throws {
    let one = try #require(pixels(CodeBlockView(code: "x", highlight: nil, size: 14, style: InlineStyle(), revealed: []), width: 200))
    let long = try #require(pixels(CodeBlockView(code: String(repeating: "word ", count: 40), highlight: nil, size: 14, style: InlineStyle(), revealed: []), width: 200))
    #expect(long.height > one.height + 3 * Int(S.lineHeight))
}

@Test func controlAndBidiCharactersAreShownEscapedAndZWJIsNot() {
    let raw = "a\u{0}\u{7}\t\u{1F}\u{7F}\u{85}\u{9F}\u{61C}\u{200E}\u{200F}\u{202A}\u{202E}\u{2066}\u{2069}\u{200D}\u{200B}é"
    let shown = #"a\u{0000}\u{0007}"# + "\t" + #"\u{001F}\u{007F}\u{0085}\u{009F}\u{061C}\u{200E}\u{200F}\u{202A}\u{202E}\u{2066}\u{2069}"# + "\u{200D}\u{200B}é"
    #expect(CodeBlockModel.escaped(raw) == shown)
    // What the view draws is the escaped text.
    let drawn = CodeBlockView.attributed([CodeSpan(text: "x\u{202E}y", rgb: 0)], size: 14)
    #expect(String(drawn.characters) == #"x\u{202E}y"#)
}

@MainActor @Test func copyWritesTheExactCodeAndCoolsDown() async throws {
    let board = NSPasteboard(name: NSPasteboard.Name("velella.tests.\(UUID().uuidString)"))
    defer { board.releaseGlobally() }
    board.declareTypes([.html, .string], owner: nil)
    board.setString("<b>old</b>", forType: .html)
    let code = "a\u{202E}b\u{7}\n\tc  "
    let feedback = CopyFeedback()
    feedback.copy(code, to: board, cooldown: .milliseconds(100))
    #expect(board.string(forType: .string) == code)
    #expect(board.string(forType: .html) == nil, "the old contents are cleared")
    #expect(feedback.copied)
    // Other main-actor tests may hold the actor past the cooldown: wait for the reset, bounded.
    let start = ContinuousClock.now
    while feedback.copied, ContinuousClock.now - start < .seconds(5) {
        try await Task.sleep(for: .milliseconds(20))
    }
    #expect(!feedback.copied)
    #expect(ContinuousClock.now - start >= .milliseconds(100))
    // Vencord's `useCopyCooldown(1000)`.
    #expect(S.copyCooldown == .seconds(1))
}

@MainActor @Test func copyShowsOnHoverAndIsAbsentWhileCovered() throws {
    func render(spoilers: [Int], revealed: Set<Int>, hovering: Bool) -> [[UInt8]]? {
        pixels(CodeBlockView(code: "x = 1\ny = 2", highlight: nil, size: 14, style: InlineStyle(spoilers: spoilers),
                             revealed: revealed, hovering: hovering))?.colours
    }
    #expect(try hasCopyAccent(#require(render(spoilers: [], revealed: [], hovering: true))))
    #expect(try !hasCopyAccent(#require(render(spoilers: [], revealed: [], hovering: false))))
    #expect(try !hasCopyAccent(#require(render(spoilers: [0], revealed: [], hovering: true))))
    #expect(try hasCopyAccent(#require(render(spoilers: [0], revealed: [0], hovering: true))))
}

@Test func spansAreUsedOnlyWhenTheySpellTheBlocksOwnCode() {
    let used = CodeBlockModel(code: pythonCode, highlight: python)
    #expect(used.lines == python.lines)
    // Spans of other text: the block's own code, plain, with no header.
    for other in ["x = 2\n\n# c", "x = 1\n# c", "x = 1\n\n# c\n"] {
        let model = CodeBlockModel(code: other, highlight: python)
        #expect(model.name == nil && lineTexts(model) == other.components(separatedBy: "\n"), "\(other)")
        #expect(model.lines.allSatisfy { $0.count == 1 && $0[0].rgb == S.plain })
    }
    // Byte for byte, not canonical equivalence: a decomposed é is not the precomposed one.
    let composed = CodeHighlight(name: "Python", lines: [[CodeSpan(text: "\u{E9}", rgb: 1)]])
    #expect(CodeBlockModel(code: "e\u{301}", highlight: composed).name == nil)
}

/// Plan K6-1: a hidden-spoiler code block before a visible one. Each block's model is built from
/// its own node, so the visible one shows its own text only, even before the spoiler is opened.
@Test func aHiddenSpoilerCodeBlockBeforeAVisibleOneLeavesTheVisibleOneItsOwnText() {
    let secret = CodeHighlight(name: "Python", lines: [[CodeSpan(text: "secret = 1", rgb: 0x9CDCFE)]])
    let shown = CodeHighlight(name: "Python", lines: [[CodeSpan(text: "shown = 2", rgb: 0x9CDCFE)]])
    let body = MessageBody(blocks: [
        .paragraph([.spoiler(id: 0, [.block(.codeBlock(language: "py", code: "secret = 1", highlight: secret))])]),
        .codeBlock(language: "py", code: "shown = 2", highlight: shown),
    ])
    let base = MarkdownText(body: body)
    guard case .code(let code, let highlight, _) = BlockModel(body.blocks[1], base: base, listDepth: 0) else {
        Issue.record("not code"); return
    }
    let visible = CodeBlockModel(code: code, highlight: highlight)
    #expect(lineTexts(visible) == ["shown = 2"] && visible.name == "Python")
    // The hidden block is split out with its own spoiler style and keeps its own spans.
    guard case .paragraph(let inlines) = body.blocks[0],
          case .block(.codeBlock(_, let hiddenCode, let hiddenHighlight), let style)? = InlinePart.split(inlines).first else {
        Issue.record("no hidden block"); return
    }
    #expect(style.hiddenSpoiler(revealed: []) == 0)
    #expect(lineTexts(CodeBlockModel(code: hiddenCode, highlight: hiddenHighlight)) == ["secret = 1"])
    // Even a node that carried the other block's spans would draw only its own code.
    #expect(lineTexts(CodeBlockModel(code: "shown = 2", highlight: secret)) == ["shown = 2"])
}
