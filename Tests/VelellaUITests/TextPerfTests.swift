import AppKit
import SwiftUI
import Testing
@testable import VelellaUI

// N0-6's protocol as N1's standing bound (docs/plans/selection.md, N1 Limits and Baseline): the
// first frame and 20 scrolls over the 200-row corpus, and the adversarial rows' first frames, in a
// 1200 x 800 offscreen window with the message column's rows (`MessageRow`). Timings are wall clock
// in milliseconds, printed as `TEXTPERF ...`; the bounds are asserted in release builds only
// (`swift test -c release -Xswiftc -enable-testing --filter TextPerf`), where they mean something.

@MainActor
enum TextPerf {
    #if DEBUG
    nonisolated static let release = false
    #else
    nonisolated static let release = true
    #endif

    /// Absolute bounds (N1 Limits). The corpus bounds are 1.5 x the medians of the renderer N1
    /// replaced, measured once with this file on 919a6f1 in release (2026-10-16, on the author's
    /// machine under other load): first frame 18.3 ms, scroll 142.3 ms. Wall-clock bounds hold only
    /// on a comparable machine; a loaded one can miss them (the same run repeated at load average
    /// 150 gave 58.9 ms and 593.1 ms).
    static let adversarialLimit = 1000.0
    static let corpusFirstFrameLimit = 1.5 * 18.3
    static let corpusScrollLimit = 1.5 * 142.3

    static func ms(_ d: Duration) -> Double { Double(d.components.seconds) * 1000 + Double(d.components.attoseconds) / 1e15 }

    static func median(_ values: [Double]) -> Double {
        let s = values.sorted()
        guard !s.isEmpty else { return -1 }
        return s.count % 2 == 1 ? s[s.count / 2] : (s[s.count / 2 - 1] + s[s.count / 2]) / 2
    }

    static func fmt(_ values: [Double]) -> String { values.map { String(format: "%.1f", $0) }.joined(separator: ", ") }

    struct Result { var first = 0.0, scroll = 0.0 }

    /// One run: the first frame (root view assigned, laid out, displayed), then, with `scrolling`,
    /// 20 programmatic scrolls of 400 pt from the bottom, each followed by layout and display.
    static func run(_ messages: [MessageItem], scrolling: Bool = true) -> Result {
        NSApplication.shared.accessibilitySetValue(false, forAttribute: NSAccessibility.Attribute(rawValue: "AXEnhancedUserInterface"))
        let window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 1200, height: 800), styleMask: [.borderless], backing: .buffered, defer: false)
        window.isReleasedWhenClosed = false
        let host = NSHostingView(rootView: AnyView(EmptyView()))
        host.frame = NSRect(x: 0, y: 0, width: 1200, height: 800)
        window.contentView = host
        var r = Result()
        let t0 = ContinuousClock.now
        host.rootView = AnyView(Column(messages: messages))
        host.layoutSubtreeIfNeeded()
        host.displayIfNeeded()
        r.first = ms(ContinuousClock.now - t0)
        if scrolling, let scrollView = scrollView(in: host), let document = scrollView.documentView {
            let clip = scrollView.contentView
            clip.scroll(to: NSPoint(x: 0, y: max(0, document.frame.height - clip.bounds.height)))
            scrollView.reflectScrolledClipView(clip)
            host.layoutSubtreeIfNeeded()
            host.displayIfNeeded()
            for _ in 0..<20 {
                let s0 = ContinuousClock.now
                clip.scroll(to: NSPoint(x: 0, y: max(0, clip.bounds.origin.y - 400)))
                scrollView.reflectScrolledClipView(clip)
                host.layoutSubtreeIfNeeded()
                host.displayIfNeeded()
                r.scroll += ms(ContinuousClock.now - s0)
            }
        }
        return r
    }

    static func scrollView(in view: NSView) -> NSScrollView? {
        if let s = view as? NSScrollView { return s }
        for sub in view.subviews { if let s = scrollView(in: sub) { return s } }
        return nil
    }

    /// `MessageArea`'s scroller and lazy stack over `messages`, with its rows.
    struct Column: View {
        let messages: [MessageItem]
        var body: some View {
            ScrollView {
                LazyVStack(alignment: .leading, spacing: 0) {
                    ForEach(MessageGrouping.entries(messages)) { entry in
                        switch entry {
                        case .divider(_, let date): DateDivider(date: date)
                        case .row(let message, let startsGroup, let afterDivider):
                            MessageRow(message: message, startsGroup: startsGroup, afterDivider: afterDivider)
                        }
                    }
                }
                .padding(.bottom, DesignTokens.Spacing.lg)
            }
            .scrollIndicators(.hidden)
        }
    }

    // MARK: Corpus (N0-6)

    static let python = [
        "import os", "def f(x):", "    return x * 2 + 1  # short", "def long_function_name(argument_one, argument_two, argument_three):",
        "    values = [argument_one, argument_two, argument_three, argument_one + argument_two + argument_three]", "    pass  # filler",
        "class Thing:", "    def __init__(self):", "        self.items = {\"alpha\": 1, \"beta\": 2, \"gamma\": 3, \"delta\": 4, \"epsilon\": 5, \"zeta\": 6}",
        "        pass", "print(f(2))", "# end of a comment that is long enough to wrap around the code block at width 320",
    ]

    /// An emoji host that is not Discord's: nothing is fetched; the image comes from the seam.
    static let emojiURL = URL(string: "https://emoji.example.test/e/blob.png")!

    /// Highlight spans that spell exactly `lines`: each word in the next of four colours.
    static func highlight(_ lines: [String]) -> CodeHighlight {
        let colours: [UInt32] = [0x569CD6, 0xDCDCAA, 0xCE9178, 0xB5CEA8]
        return CodeHighlight(lines: lines.map { line in
            var spans: [CodeSpan] = []
            for (i, chunk) in line.split(separator: " ", omittingEmptySubsequences: false).enumerated() {
                spans.append(CodeSpan(text: String(chunk) + " ", rgb: colours[i % 4], bold: i % 7 == 3, italic: i % 5 == 4))
            }
            if let last = spans.popLast() { spans.append(CodeSpan(text: String(last.text.dropLast()), rgb: last.rgb, bold: last.bold, italic: last.italic)) }
            return spans
        })
    }

    /// The 20-row cycle: 6 one-line, 4 two-to-four-line, 3 blocks, 2 spoiler, 2 code, 2 link, 1 image rows.
    static let cycle = [0, 0, 1, 0, 2, 1, 0, 3, 4, 5, 0, 1, 2, 0, 1, 3, 4, 5, 2, 6]

    static func body(kind: Int, n: Int) -> (MessageBody, [ImageAttachment]) {
        let mention = [ResolvedMention(kind: .user, id: 1, name: "Miku")], emoji = [EmojiImage(id: 1, animated: false, url: emojiURL)]
        switch kind {
        case 0:
            return (MessageBody(text: "Short message number \(n)"), [])
        case 1:
            let inlines: [MarkdownInline] = [.text("Hey "), .userMention(1), .text(" look "), .customEmoji(id: 1, name: "blob", animated: false), .text(" at "),
                                             .timestamp(unix: 1_760_000_000, style: .longDateShortTime), .text(" - " + String(repeating: "lorem ipsum dolor sit amet ", count: 5 + n % 9))]
            return (MessageBody(blocks: [.paragraph(inlines)], mentions: mention, emoji: emoji), [])
        case 2:
            return (MessageBody(blocks: [
                .heading(level: UInt8(n % 3 + 1), [.text("A heading for row \(n)")]),
                .paragraph([.text("Body text under it, "), .bold([.text("with bold")]), .text(" and a "), .code("code span"), .text(".")]),
                .subtext([.text("Small print")]),
                .quote([.paragraph([.text("A quoted line that is long enough to wrap in a narrow column, as quotes do.")]),
                        .list(ordered: false, start: 1, items: [[.paragraph([.text("in a quote")])]])]),
                .list(ordered: true, start: 1, items: [
                    [.paragraph([.text("one")]), .list(ordered: false, start: 1, items: [[.paragraph([.text("nested")])], [.paragraph([.text("another")])]])],
                    [.paragraph([.text("two")])],
                ]),
                .paragraph([.text("the end"), .lineBreak]),
            ]), [])
        case 3:
            if n % 2 == 0 {
                return (MessageBody(blocks: [.paragraph([.text("Spoilers: "), .spoiler(id: 0, [.text("the butler did it")]), .text(" and "), .spoiler(id: 0, [.text("so did the maid")]), .text(".")])]), [])
            }
            return (MessageBody(blocks: [.paragraph([.text("Hidden code: "), .spoiler(id: 0, [.block(.codeBlock(language: "py", code: "print(\"secret\")", header: "Python", highlight: nil))])])]), [])
        case 4:
            return (MessageBody(blocks: [.paragraph([.text("Here is the code:")]),
                                         .codeBlock(language: "py", code: python.joined(separator: "\n"), header: "Python", highlight: highlight(python))]), [])
        case 5:
            let link = MarkdownInline.link(id: 0, url: "https://example.com/docs", host: "example.com", label: [.text("https://example.com/docs")], masked: false)
            let masked = MarkdownInline.link(id: 0, url: "https://example.org/guide", host: "example.org", label: [.text("the guide")], masked: true)
            return (MessageBody(blocks: [.paragraph([.text("Docs at "), link, .text(" and "), masked, .text(" for more.")])]), [])
        default:
            return (MessageBody(text: "Look at this one"), [ImageAttachment(seed: n, width: 400, height: 300)])
        }
    }

    static let authors = ["Miku", "Rin", "Len", "Luka", "Kaito"]

    /// 200 rows in the repeating order; rows of one author come three at a time, a minute apart.
    static func corpus(count: Int = 200) -> [MessageItem] {
        let base = Date(timeIntervalSince1970: 1_760_000_000)
        return (0..<count).map { n in
            let (body, images) = self.body(kind: cycle[n % cycle.count], n: n)
            let author = authors[(n / 3) % authors.count]
            return MessageItem(
                id: String(1_000_000 + n), authorId: author, authorName: author, avatar: .initials(String(author.prefix(2)).uppercased()),
                timestamp: base.addingTimeInterval(Double(n) * 60), text: "row \(n)", body: body,
                displayNameStyle: author == "Miku" ? Fixtures.sampleNameStyle : nil, images: images)
        }
    }

    static func seedEmoji() {
        let image = NSImage(size: NSSize(width: 48, height: 48))
        image.lockFocus()
        NSColor.systemPink.setFill()
        NSBezierPath(ovalIn: NSRect(x: 2, y: 2, width: 44, height: 44)).fill()
        image.unlockFocus()
        EmojiImages.shared.insert(image, for: emojiURL)
    }

    /// S-12's rows, each as one message's body.
    static func adversarial() -> [(name: String, body: MessageBody)] {
        let combining = "a" + String(repeating: "\u{0301}", count: 8192)
        let bidi = String(repeating: "abc \u{5E9}\u{5DC}\u{5D5}\u{5DD} ", count: 700)
        return [
            ("3200 ||a||", MessageBody(blocks: [.paragraph((0..<3200).map { _ in MarkdownInline.spoiler(id: 0, [.text("a")]) })])),
            ("2300 ```x``` blocks", MessageBody(blocks: (0..<2300).map { _ in MarkdownBlock.codeBlock(language: nil, code: "x", header: nil, highlight: nil) })),
            ("a base plus 16 KiB of combining marks", MessageBody(blocks: [.paragraph([.text(combining)])])),
            ("one 16 KiB line", MessageBody(blocks: [.paragraph([.text(String(repeating: "x", count: 16 * 1024))])])),
            ("64 emoji", MessageBody(blocks: [.paragraph((0..<64).map { _ in MarkdownInline.customEmoji(id: 1, name: "blob", animated: false) })],
                                    emoji: [EmojiImage(id: 1, animated: false, url: emojiURL)])),
            ("alternating RTL and LTR runs", MessageBody(blocks: [.paragraph([.text(bidi)])])),
        ]
    }

    static func message(_ body: MessageBody, embeds: [EmbedItem] = []) -> MessageItem {
        MessageItem(id: "perf", authorName: "Miku", avatar: .initials("MI"), timestamp: Date(timeIntervalSince1970: 1_760_000_000), text: "", body: body, embeds: embeds)
    }
}

@MainActor @Suite(.serialized) struct TextPerfTests {
    @Test(.enabled(if: TextPerf.release)) func corpusFirstFrameAndScroll() {
        TextPerf.seedEmoji()
        let messages = TextPerf.corpus()
        #expect(messages.count == 200)
        _ = TextPerf.run(messages) // warm-up, discarded
        let runs = (0..<7).map { _ in TextPerf.run(messages) }
        let first = TextPerf.median(runs.map(\.first)), scroll = TextPerf.median(runs.map(\.scroll))
        print("TEXTPERF corpus first frame ms x7: [\(TextPerf.fmt(runs.map(\.first)))] median \(String(format: "%.1f", first))")
        print("TEXTPERF corpus scroll (20 x 400 pt) ms x7: [\(TextPerf.fmt(runs.map(\.scroll)))] median \(String(format: "%.1f", scroll))")
        #expect(first <= TextPerf.corpusFirstFrameLimit, "first frame median \(first) ms")
        #expect(scroll <= TextPerf.corpusScrollLimit, "scroll median \(scroll) ms")
    }

    /// S-12: each adversarial row alone, first frame only: one warm-up, then the median of 7.
    @Test(.enabled(if: TextPerf.release), arguments: 0..<6) func adversarialRow(_ row: Int) {
        TextPerf.seedEmoji()
        let (name, body) = TextPerf.adversarial()[row]
        let message = TextPerf.message(body)
        _ = TextPerf.run([message], scrolling: false)
        let times = (0..<7).map { _ in TextPerf.run([message], scrolling: false).first }
        let median = TextPerf.median(times)
        print("TEXTPERF adversarial \(name): first frame ms x7: [\(TextPerf.fmt(times))] median \(String(format: "%.1f", median))")
        #expect(median <= TextPerf.adversarialLimit, "\(name): \(median) ms")
    }
}
