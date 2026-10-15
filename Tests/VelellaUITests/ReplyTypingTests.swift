import Foundation
import Testing
@testable import VelellaUI

private let fsi = "\u{2068}", pdi = "\u{2069}"

// MARK: Reply

@Test func aSubmitCarriesTheReplyTargetAndClearsTheReplyBar() {
    var sent: [(String, String?)] = []
    var draft = "answer"
    var reply: ReplyPreview? = ReplyPreview(authorName: "Ada", segments: [.text("question")], messageID: "30")
    ComposerSubmit.submit(&draft, reply: &reply, hasMarkedText: false) { sent.append(($0, $1)) }
    #expect(sent.count == 1 && sent[0].0 == "answer" && sent[0].1 == "30")
    #expect(draft.isEmpty)
    #expect(reply == nil)
    // The next message is a plain one.
    draft = "next"
    ComposerSubmit.submit(&draft, reply: &reply, hasMarkedText: false) { sent.append(($0, $1)) }
    #expect(sent.count == 2 && sent[1].1 == nil)
}

@Test func aSubmitThatSendsNothingKeepsTheReplyBar() {
    var calls = 0
    let target = ReplyPreview(authorName: "Ada", segments: [], messageID: "30")
    var reply: ReplyPreview? = target
    var draft = "  "
    ComposerSubmit.submit(&draft, reply: &reply, hasMarkedText: false) { _, _ in calls += 1 }
    draft = "marked"
    ComposerSubmit.submit(&draft, reply: &reply, hasMarkedText: true) { _, _ in calls += 1 }
    #expect(calls == 0)
    #expect(reply == target)
    #expect(draft == "marked")
}

@Test func aReplyWithoutAnIdSendsAPlainMessage() {
    var target: String?? = .some("unset")
    var reply: ReplyPreview? = ReplyPreview(authorName: "Ada", segments: [])
    var draft = "hi"
    ComposerSubmit.submit(&draft, reply: &reply, hasMarkedText: false) { _, id in target = .some(id) }
    #expect(target == .some(nil))
}

@Test func theReplyBarShowsTheNameVerbatimAndIsolated() {
    // Markdown in a name stays literal; a PDI inside cannot close the isolate early.
    let markup = ReplyPreview(authorName: "**Ada** [x](https://a.test)", segments: [])
    #expect(ReplyBar.title(markup) == "Replying to \(fsi)**Ada** [x](https://a.test)\(pdi)")
    let bidi = ReplyPreview(authorName: "\u{2069}\u{202E}abc", segments: [])
    #expect(ReplyBar.title(bidi) == "Replying to \(fsi)\u{202E}abc\(pdi)")
    #expect(ReplyBar.title(markup) == "Replying to " + MarkdownText.isolated(markup.authorName))
}

// MARK: Typing

private func label(_ names: [String]) -> String {
    TypingLabel.string(names).replacingOccurrences(of: fsi, with: "").replacingOccurrences(of: pdi, with: "")
}

@Test func theTypingLabelHasDiscordsFourForms() {
    #expect(label([]) == "")
    #expect(label(["A"]) == "A is typing…")
    #expect(label(["A", "B"]) == "A and B are typing…")
    #expect(label(["A", "B", "C"]) == "A, B and C are typing…")
    #expect(label(["A", "B", "C", "D"]) == "Several people are typing…")
    #expect(label(["A", "B", "C", "D", "E", "F"]) == "Several people are typing…")
    #expect(TypingLabel.parts([]).isEmpty)
}

@Test func typingNamesAreVerbatimIsolatedAndNeverMarkup() {
    let names = ["**Ada**", "\u{2069}x"]
    // The second name's stray PDI is dropped, so only the wrapper's own PDI closes its FSI.
    #expect(TypingLabel.parts(names) == [
        .name(fsi + "**Ada**" + pdi), .text(" and "), .name(fsi + "x" + pdi), .text(" are typing…"),
    ])
    // The styled string holds exactly the label's characters: nothing was parsed or added.
    #expect(String(TypingLabel.attributed(names).characters) == TypingLabel.string(names))
    #expect(TypingLabel.string(["**Ada**"]).contains("**Ada**"))
}

@Test func theTypingDotsPulseEvery1Point2SecondsAndTrailEachOther() {
    #expect(TypingMetrics.pulse == 1.2)
    #expect(TypingMetrics.dotSize == 7 && TypingMetrics.dotGap == 2)
    for dot in 0..<3 {
        for t in stride(from: 0.0, to: 1.2, by: 0.1) {
            let a = TypingMetrics.opacity(at: t, dot: dot)
            #expect(abs(a - TypingMetrics.opacity(at: t + 1.2, dot: dot)) < 1e-9)
            #expect(a >= TypingMetrics.minOpacity - 1e-9 && a <= 1 + 1e-9)
        }
    }
    #expect(TypingMetrics.opacity(at: 0.1, dot: 0) != TypingMetrics.opacity(at: 0.1, dot: 1))
}

@Test func theShellCarriesNoTypingByDefault() {
    #expect(ShellState().typing.isEmpty)
    #expect(ShellState.signedOut.typing.isEmpty)
}
