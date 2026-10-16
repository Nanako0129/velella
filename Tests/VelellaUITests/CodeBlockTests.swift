import AppKit
import SwiftUI
import Testing
@testable import VelellaUI

// The K6 code-block view (docs/plans/markdown.md, K6 rev 2 "Tests" kept by rev 3.1, and rev
// 3.1's header without colours): header, line numbers, escapes, Copy, and spans that only ever
// belong to their own block.

private typealias S = CodeBlockStyle

private let python = CodeHighlight(lines: [
    [CodeSpan(text: "x", rgb: 0x9CDCFE), CodeSpan(text: " = ", rgb: 0xD4D4D4), CodeSpan(text: "1", rgb: 0xB5CEA8)],
    [],
    [CodeSpan(text: "# c", rgb: 0x6A9955, italic: true)],
])
private let pythonCode = "x = 1\n\n# c"

private func lineTexts(_ model: CodeBlockModel) -> [String] { model.lines.map { $0.map(\.text).joined() } }

/// Every (r, g, b, a) pixel of a hosted render (selection plan N1: the code is an AppKit view).
@MainActor private func hostedPixels<V: View>(_ view: V, width: CGFloat = 240) -> (height: Int, colours: [[UInt8]])? {
    guard let render = TextHarness.render(view, width: width, scale: 1) else { return nil }
    return (render.height, (0..<render.height).flatMap { y in (0..<render.width).map { render.at($0, y) } })
}

/// The Copy button's accent (#007BC8): strongly blue, unlike anything else in a plain block.
private func hasCopyAccent(_ colours: [[UInt8]]) -> Bool {
    colours.contains { p in
        let (a, c) = (Int(p[0]), Int(p[2])) // r and b, or b and r: either order is far apart
        return abs(a - c) > 120 && min(a, c) < 30 && p[3] == 255
    }
}

/// Core sends the header for a `py` block ("Python") and none for `swift` (off its table), with or
/// without colours.
@Test func theHeaderShowsWithoutColoursAndOnlyFromCore() {
    #expect(CodeBlockModel(code: pythonCode, header: "Python", highlight: nil).name == "Python")
    #expect(CodeBlockModel(code: pythonCode, header: "Python", highlight: python).name == "Python")
    #expect(CodeBlockModel(code: "let x = 1", header: nil, highlight: nil).name == nil)
}

@MainActor @Test func theHeaderAddsARowAndColoursArriveWithoutMovingTheBlock() throws {
    func height(header: String?, highlight: CodeHighlight?) throws -> Int {
        try #require(hostedPixels(CodeBlockView(code: pythonCode, header: header, highlight: highlight, size: 14,
                                          style: InlineStyle(), revealed: []))).height
    }
    let plain = try height(header: nil, highlight: nil)
    let named = try height(header: "Python", highlight: nil)
    #expect(named > plain + 10, "\(named) vs \(plain)")
    #expect(try height(header: "Python", highlight: python) == named, "colours moved the block")
}

@Test func lineNumbersRunFromOneToN() {
    #expect(CodeBlockModel(code: pythonCode, header: "Python", highlight: python).numbers == ["1", "2", "3"])
    #expect(CodeBlockModel(code: "a\n\nb\n", header: nil, highlight: nil).numbers == ["1", "2", "3", "4"])
    #expect(lineTexts(CodeBlockModel(code: "a\n\nb\n", header: nil, highlight: nil)) == ["a", "", "b", ""])
    let many = Array(repeating: "x", count: 2000).joined(separator: "\n")
    #expect(CodeBlockModel(code: many, header: nil, highlight: nil).numbers.last == "2000")
}

@MainActor @Test func longLinesWrapInsideTheColumn() throws {
    let one = try #require(hostedPixels(CodeBlockView(code: "x", header: nil, highlight: nil, size: 14, style: InlineStyle(), revealed: []), width: 200))
    let long = try #require(hostedPixels(CodeBlockView(code: String(repeating: "word ", count: 40), header: nil, highlight: nil, size: 14, style: InlineStyle(), revealed: []), width: 200))
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
    // In a full `swift test` this test took 3.0-4.2 s before W1's suite was added, and with it
    // failed 3 runs of 10 at 6.3-9.2 s against a 5 s bound (2026-10-17); the bound only stops a hang.
    let start = ContinuousClock.now
    while feedback.copied, ContinuousClock.now - start < .seconds(30) {
        try await Task.sleep(for: .milliseconds(20))
    }
    #expect(!feedback.copied)
    #expect(ContinuousClock.now - start >= .milliseconds(100))
    // Vencord's `useCopyCooldown(1000)`.
    #expect(S.copyCooldown == .seconds(1))
}

@MainActor @Test func copyShowsOnHoverAndIsAbsentWhileCovered() throws {
    func render(spoilers: [Int], revealed: Set<Int>, hovering: Bool) -> [[UInt8]]? {
        hostedPixels(CodeBlockView(code: "x = 1\ny = 2", header: nil, highlight: nil, size: 14, style: InlineStyle(spoilers: spoilers),
                             revealed: revealed, hovering: hovering))?.colours
    }
    #expect(try hasCopyAccent(#require(render(spoilers: [], revealed: [], hovering: true))))
    #expect(try !hasCopyAccent(#require(render(spoilers: [], revealed: [], hovering: false))))
    #expect(try !hasCopyAccent(#require(render(spoilers: [0], revealed: [], hovering: true))))
    #expect(try hasCopyAccent(#require(render(spoilers: [0], revealed: [0], hovering: true))))
}

@Test func spansAreUsedOnlyWhenTheySpellTheBlocksOwnCode() {
    let used = CodeBlockModel(code: pythonCode, header: "Python", highlight: python)
    #expect(used.lines == python.lines)
    // Spans of other text: the block's own code, plain (the header is core's either way).
    for other in ["x = 2\n\n# c", "x = 1\n# c", "x = 1\n\n# c\n"] {
        let model = CodeBlockModel(code: other, header: "Python", highlight: python)
        #expect(lineTexts(model) == other.components(separatedBy: "\n"), "\(other)")
        #expect(model.lines.allSatisfy { $0.count == 1 && $0[0].rgb == S.plain })
    }
    // Byte for byte, not canonical equivalence: a decomposed é is not the precomposed one.
    let composed = CodeHighlight(lines: [[CodeSpan(text: "\u{E9}", rgb: 1)]])
    #expect(CodeBlockModel(code: "e\u{301}", header: nil, highlight: composed).lines.allSatisfy { $0[0].rgb == S.plain })
}

/// Plan K6-1: a hidden-spoiler code block before a visible one. Each block's model is built from
/// its own node, so the visible one shows its own text only, even before the spoiler is opened.
@Test func aHiddenSpoilerCodeBlockBeforeAVisibleOneLeavesTheVisibleOneItsOwnText() {
    let secret = CodeHighlight(lines: [[CodeSpan(text: "secret = 1", rgb: 0x9CDCFE)]])
    let shown = CodeHighlight(lines: [[CodeSpan(text: "shown = 2", rgb: 0x9CDCFE)]])
    let body = MessageBody(blocks: [
        .paragraph([.spoiler(id: 0, [.block(.codeBlock(language: "py", code: "secret = 1", header: "Python", highlight: secret))])]),
        .codeBlock(language: "rb", code: "shown = 2", header: "Ruby", highlight: shown),
    ])
    let base = MarkdownText(body: body)
    guard case .code(let code, let header, let highlight, _) = BlockModel(body.blocks[1], base: base, listDepth: 0) else {
        Issue.record("not code"); return
    }
    let visible = CodeBlockModel(code: code, header: header, highlight: highlight)
    #expect(lineTexts(visible) == ["shown = 2"] && visible.name == "Ruby")
    #expect(visible.lines == shown.lines)
    // The hidden block is split out with its own spoiler style and keeps its own spans.
    guard case .paragraph(let inlines) = body.blocks[0],
          case .block(.codeBlock(_, let hiddenCode, let hiddenHeader, let hiddenHighlight), let style)? = InlinePart.split(inlines).first else {
        Issue.record("no hidden block"); return
    }
    #expect(style.hiddenSpoiler(revealed: []) == 0)
    let hidden = CodeBlockModel(code: hiddenCode, header: hiddenHeader, highlight: hiddenHighlight)
    #expect(lineTexts(hidden) == ["secret = 1"] && hidden.name == "Python")
    // Even a node that carried the other block's spans would draw only its own code.
    #expect(lineTexts(CodeBlockModel(code: "shown = 2", header: "Ruby", highlight: secret)) == ["shown = 2"])
}
