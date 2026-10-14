import AppKit
import SwiftUI
import Testing
@testable import VelellaUI

// Behaviour of the K2 inline mapping, link/spoiler handler and paragraph host
// (docs/plans/markdown.md, K2 acceptance). ASTs here are the shapes K1 produces.

private let context = RenderContext(now: Date(timeIntervalSince1970: 0), locale: Locale(identifier: "en-US"))
private let fsi = "\u{2068}", pdi = "\u{2069}"
private let emojiURL = URL(string: "https://cdn.discordapp.com/emojis/100000000000000004.webp?size=48")!

/// Segments of `inlines` as one paragraph of a body with `mentions` and `emoji`.
private func segments(
    _ inlines: [MarkdownInline], mentions: [ResolvedMention] = [], emoji: [EmojiImage] = [], revealed: Set<Int> = []
) -> [Segment] {
    let body = MessageBody(blocks: [.paragraph(inlines)], mentions: mentions, emoji: emoji)
    guard case .paragraph(let numbered) = body.blocks.first else { return [] }
    return MarkdownText(body: body, revealed: revealed).segments(numbered, context: context)
}

private func plain(_ segments: [Segment]) -> String {
    segments.map { String($0.text.characters) }.joined()
}

private func joined(_ segments: [Segment]) -> AttributedString {
    segments.reduce(AttributedString()) { $0 + $1.text }
}

/// The attributes of `needle`'s characters (nil where they differ across them).
private func run(_ needle: String, in segments: [Segment]) -> AttributedSubstring? {
    let all = joined(segments)
    return all.range(of: needle).map { all[$0] }
}

private func links(_ segments: [Segment]) -> Set<URL?> {
    Set(joined(segments).runs.map(\.link))
}

private let body = MarkdownText.bodySize

// MARK: One mapping test per inline kind

@Test func textMapsToItsLiteralCharactersWithNoLink() {
    let s = segments([.text("**b** [a](https://x)")])
    #expect(plain(s) == "**b** [a](https://x)")
    #expect(links(s) == [nil])
    #expect(run("**b**", in: s)?.swiftUI.font == Font.system(size: body, weight: .regular))
}

@Test func rejectedLinkInputsArriveAsTextAndStayLiteral() {
    // K1 D4 leaves these as text; the mapping adds no link to any of them.
    for input in ["[a](javascript:alert(1))", "[a](file:///etc)", "https://user:pw@x.example/", "https:\\\\evil.example\\", "<ftp://x/y>"] {
        let s = segments([.text(input)])
        #expect(plain(s) == input)
        #expect(links(s) == [nil])
    }
}

@Test func boldIsSemibold() {
    #expect(run("b", in: segments([.bold([.text("b")])]))?.swiftUI.font == Font.system(size: body, weight: .semibold))
}

@Test func italicIsItalic() {
    #expect(run("i", in: segments([.italic([.text("i")])]))?.swiftUI.font == Font.system(size: body, weight: .regular).italic())
}

@Test func underlineAndStrikeSetTheirLines() {
    let s = segments([.underline([.text("u")]), .strike([.text("s")]), .bold([.italic([.underline([.strike([.text("all")])])])])])
    #expect(run("u", in: s)?.swiftUI.underlineStyle == .single)
    #expect(run("u", in: s)?.swiftUI.strikethroughStyle == nil)
    #expect(run("s", in: s)?.swiftUI.strikethroughStyle == .single)
    let all = run("all", in: s)
    #expect(all?.swiftUI.font == Font.system(size: body, weight: .semibold).italic())
    #expect(all?.swiftUI.underlineStyle == .single && all?.swiftUI.strikethroughStyle == .single)
}

@Test func codeIsMonospacedAtSevenEighthsOnTheCodeFill() {
    let code = run("x = 1", in: segments([.code("x = 1")]))
    // Discord: `.markup__75297 code.inline { font-size: 85% }`.
    #expect(code?.swiftUI.font == Font.system(size: body * 0.85, weight: .regular, design: .monospaced))
    #expect(code?.swiftUI.backgroundColor == MarkdownPalette.codeFill)
}

@Test func aLinkCarriesOnlyItsInAppURL() {
    let s = segments([.text("see "), .link(id: 9, url: "https://x.example/", host: "x.example", label: [.bold([.text("lab")])], masked: true)])
    #expect(plain(s) == "see lab")
    #expect(run("lab", in: s)?.link == URL(string: "velella-link:0"))
    #expect(run("lab", in: s)?.swiftUI.font == Font.system(size: body, weight: .semibold))
    #expect(run("see ", in: s)?.link == nil)
}

@Test func userMentionsShowTheResolvedNameIsolatedOnTheMentionFill() {
    let s = segments([.userMention(1), .text(" "), .userMention(2)], mentions: [ResolvedMention(kind: .user, id: 1, name: "Ada")])
    #expect(plain(s) == "\(fsi)@Ada\(pdi) \(fsi)@unknown-user\(pdi)")
    #expect(run("@Ada", in: s)?.swiftUI.backgroundColor == MarkdownPalette.mentionFill)
}

@Test func roleMentionsTakeTheRoleColour() {
    let s = segments(
        [.roleMention(5), .text(" "), .roleMention(6), .text(" "), .roleMention(7)],
        mentions: [ResolvedMention(kind: .role, id: 5, name: "mods", color: 0xFF0000), ResolvedMention(kind: .role, id: 6, name: "plain")]
    )
    #expect(plain(s) == "\(fsi)@mods\(pdi) \(fsi)@plain\(pdi) \(fsi)@deleted-role\(pdi)")
    let red = Color(RGB(rgb: 0xFF0000))
    #expect(run("@mods", in: s)?.swiftUI.foregroundColor == red)
    #expect(run("@mods", in: s)?.swiftUI.backgroundColor == red.opacity(0.22))
    #expect(run("@plain", in: s)?.swiftUI.backgroundColor == MarkdownPalette.mentionFill)
}

@Test func channelMentionsShowTheChannelName() {
    let s = segments([.channelMention(7), .text(" "), .channelMention(8)], mentions: [ResolvedMention(kind: .channel, id: 7, name: "general")])
    #expect(plain(s) == "\(fsi)#general\(pdi) \(fsi)#unknown\(pdi)")
    #expect(run("#general", in: s)?.swiftUI.backgroundColor == MarkdownPalette.mentionFill)
}

@Test func everyoneAndHereArePills() {
    let s = segments([.everyone, .text(" "), .here])
    #expect(plain(s) == "@everyone @here")
    #expect(run("@everyone", in: s)?.swiftUI.backgroundColor == MarkdownPalette.mentionFill)
    #expect(run("@here", in: s)?.swiftUI.backgroundColor == MarkdownPalette.mentionFill)
}

@Test func customEmojiAreImagesOnlyWithACoreURL() throws {
    let listed = EmojiImage(id: 4, animated: false, url: emojiURL)
    let s = segments([.customEmoji(id: 4, name: "cat", animated: false), .customEmoji(id: 4, name: "cat", animated: true), .customEmoji(id: 5, name: "dog", animated: false)], emoji: [listed])
    try #require(s.count == 2)
    #expect(s[0].image?.url == emojiURL)
    #expect(String(s[0].text.characters) == "\(fsi):cat:\(pdi)")
    let line = NSFont.systemFont(ofSize: body)
    #expect(s[0].image.map { $0.height > 0 && $0.height <= line.ascender - line.descender } == true)
    // Not in core's list (another URL, or past the 64 images): the names, as plain text.
    #expect(s[1].image == nil && String(s[1].text.characters) == "\(fsi):cat:\(pdi)\(fsi):dog:\(pdi)")
}

@Test func timestampsArePillsWithATip() {
    let s = segments([.timestamp(unix: 1_618_953_630, style: .longDateShortTime)])
    let date = Date(timeIntervalSince1970: 1_618_953_630)
    let enUS = Locale(identifier: "en_US")
    #expect(s.count == 1)
    #expect(plain(s) == date.formatted(Date.FormatStyle(date: .long, time: .shortened).locale(enUS)))
    #expect(s[0].hoverTip == date.formatted(Date.FormatStyle(date: .complete, time: .shortened).locale(enUS)))
    #expect(s[0].text.swiftUI.backgroundColor == MarkdownPalette.timestampFill)
}

@Test func slashCommandsAndGuildLinksArePills() {
    let s = segments([.slashCommand(name: "cmd sub", id: 6), .text(" "), .guildNavigation(guild: 1, target: .guide), .text(" "), .guildNavigation(guild: 1, target: .linkedRoles(nil))])
    #expect(plain(s) == "\(fsi)/cmd sub\(pdi) Server Guide Linked Roles")
    #expect(run("/cmd sub", in: s)?.swiftUI.backgroundColor == MarkdownPalette.mentionFill)
    #expect(run("Server Guide", in: s)?.swiftUI.backgroundColor == MarkdownPalette.mentionFill)
}

@Test func lineBreaksAreNewlines() {
    #expect(plain(segments([.text("a"), .lineBreak, .text("b")])) == "a\nb")
}

@Test func plainRunsMergeAndSpecialOnesStayApart() {
    let s = segments([.text("a"), .bold([.text("b")]), .timestamp(unix: 0, style: .relative), .text("c")])
    #expect(s.count == 3)
    #expect(String(s[0].text.characters) == "ab")
}

// MARK: Spoilers

@Test func aHiddenSpoilerCarriesOnlyItsRevealLink() {
    let evil: [MarkdownInline] = [.spoiler(id: 0, [.link(id: 0, url: "https://evil.example/", host: "evil.example", label: [.text("https://evil.example")], masked: false)])]
    let hidden = segments(evil)
    #expect(links(hidden) == [URL(string: "velella-spoiler:0")])
    #expect(hidden.allSatisfy { $0.covered })
    let shown = segments(evil, revealed: [0])
    #expect(links(shown) == [URL(string: "velella-link:0")])
    #expect(!shown.contains { $0.covered })
    #expect(run("https://evil.example", in: shown)?.swiftUI.backgroundColor == MarkdownPalette.revealedSpoiler)
}

@Test func everythingInsideAHiddenSpoilerIsInert() {
    let inner: [MarkdownInline] = [
        .text("x "), .userMention(1), .customEmoji(id: 4, name: "cat", animated: false),
        .timestamp(unix: 0, style: .relative), .code("c"), .spoiler(id: 1, [.text("inner")]),
    ]
    let s = segments([.text("a "), .spoiler(id: 0, inner)], mentions: [ResolvedMention(kind: .user, id: 1, name: "Ada")], emoji: [EmojiImage(id: 4, animated: false, url: emojiURL)])
    let hidden = s.filter(\.covered)
    #expect(!hidden.isEmpty)
    #expect(hidden.allSatisfy { $0.image == nil && $0.hoverTip == nil && $0.refreshEvery == nil })
    #expect(Set(hidden.flatMap { $0.text.runs.map(\.link) }) == [URL(string: "velella-spoiler:0")])
    // The outer reveal opens the outer spoiler only; the inner one then has its own link.
    let outerOpen = segments([.spoiler(id: 0, inner)], revealed: [0])
    #expect(run("inner", in: outerOpen)?.link == URL(string: "velella-spoiler:1"))
    #expect(outerOpen.contains { $0.hoverTip != nil })
}

@Test func aBlockInsideASpoilerSplitsTheParagraphUnderTheSameCover() {
    let body = MessageBody(blocks: [.paragraph([.text("a "), .spoiler(id: 0, [.block(.codeBlock(language: nil, code: "x"))]), .text(" b")])])
    guard case .paragraph(let inlines) = body.blocks[0] else { Issue.record("not a paragraph"); return }
    let parts = InlinePart.split(inlines)
    #expect(parts == [
        .inlines([.text("a ")]),
        .block(.codeBlock(language: nil, code: "x"), InlineStyle(spoilers: [0])),
        .inlines([.text(" b")]),
    ])
    guard case .block(_, let style) = parts[1] else { return }
    #expect(style.hiddenSpoiler(revealed: []) == 0)
    #expect(style.hiddenSpoiler(revealed: [0]) == nil)
}

@Test func splitPartsKeepTheirContainersAndDropBreaksAtTheCut() {
    let block = MarkdownBlock.heading(level: 1, [.text("h")])
    // `a\n**x ||# h\n z||**\nb`: text parts are re-wrapped in their containers; the block keeps them.
    #expect(InlinePart.split([.text("a"), .lineBreak, .bold([.text("x "), .spoiler(id: 3, [.block(block), .text(" z")])]), .lineBreak, .text("b")]) == [
        .inlines([.text("a"), .lineBreak, .bold([.text("x ")])]),
        .block(block, InlineStyle(bold: true, spoilers: [3])),
        .inlines([.bold([.spoiler(id: 3, [.text(" z")])]), .lineBreak, .text("b")]),
    ])
    // `a\n||# h||\nb`: the breaks at the cut go.
    #expect(InlinePart.split([.text("a"), .lineBreak, .spoiler(id: 0, [.block(block)]), .lineBreak, .text("b")]) == [
        .inlines([.text("a")]), .block(block, InlineStyle(spoilers: [0])), .inlines([.text("b")]),
    ])
    // Without blocks nothing is cut.
    #expect(InlinePart.split([.text("a"), .lineBreak, .bold([.text("b")])]) == [.inlines([.text("a"), .lineBreak, .bold([.text("b")])])])
}

// MARK: Bidi and traps

@Test func aBidiOverrideInANickStaysInsideItsIsolate() {
    let s = segments([.userMention(1), .text(" xyz")], mentions: [ResolvedMention(kind: .user, id: 1, name: "\u{202E}abc")])
    #expect(plain(s) == "\u{2068}@\u{202E}abc\u{2069} xyz")
}

/// K1's output for `4294967295. a`, `<t:9223372036854775807>`, `<t:-1>` and
/// `<@99999999999999999999999>` (velella-ffi `trap_inputs_arrive_as_text_or_bounded_values`),
/// plus extremes K1 never produces, through the mapping and a full offscreen render.
@MainActor @Test func extremeInputsMapAndRenderWithoutTrapping() {
    let blocks: [MarkdownBlock] = [
        .paragraph([.text("4294967295. a")]),
        .paragraph([.text("<t:9223372036854775807>")]),
        .paragraph([.timestamp(unix: -1, style: .longDateShortTime)]),
        .paragraph([.text("<@99999999999999999999999>")]),
        .paragraph([
            .timestamp(unix: 8_640_000_000_000, style: .relative), .timestamp(unix: -8_640_000_000_000, style: .shortTime),
            .timestamp(unix: .max, style: .fullDateShortTime), .timestamp(unix: .min, style: .shortDate),
            .userMention(.max), .roleMention(.max), .channelMention(.max),
            .customEmoji(id: .max, name: "big", animated: true), .slashCommand(name: "x", id: .max),
            .guildNavigation(guild: .max, target: .linkedRoles(.max)),
        ]),
        .list(ordered: true, start: .max, items: [[.paragraph([.text("a")])], [.paragraph([.text("b")])]]),
        .list(ordered: true, start: 999_999_999, items: Array(repeating: [.paragraph([.text("c")])], count: 3)),
    ]
    let body = MessageBody(blocks: blocks)
    let text = MarkdownText(body: body)
    var all = ""
    for case .paragraph(let inlines) in body.blocks {
        all += plain(text.segments(inlines, context: context)) + "\n"
    }
    #expect(all.hasPrefix("4294967295. a\n<t:9223372036854775807>\n"))
    #expect(all.contains("<@99999999999999999999999>"))
    let renderer = ImageRenderer(content: MarkdownView(content: body).frame(width: 480))
    #expect(renderer.nsImage != nil)
}

// MARK: The link and spoiler handler

private final class Recorder {
    var opened: [URL] = []
    var revealed: [Int] = []
    var confirmations: [LinkConfirmation] = []

    func router(_ body: MessageBody) -> LinkRouter {
        LinkRouter(body: body, reveal: { self.revealed.append($0) }, confirm: { self.confirmations.append($0) }, open: { self.opened.append($0) })
    }
}

private func linkBody(_ url: String, host: String, label: String, masked: Bool = false) -> MessageBody {
    MessageBody(blocks: [.paragraph([.spoiler(id: 0, [.text("s")]), .link(id: 0, url: url, host: host, label: [.text(label)], masked: masked)])])
}

@Test func aPlainLinkShowingItsURLOpensDirectly() {
    let recorder = Recorder()
    _ = recorder.router(linkBody("https://example.com/", host: "example.com", label: "https://example.com")).handle(MarkdownText.linkURL(0))
    #expect(recorder.opened == [URL(string: "https://example.com/")!])
    #expect(recorder.confirmations.isEmpty)
}

/// K5 plain links: what core produces for a typed URL (canonical `url`, typed label) shows the
/// canonical URL, as the official client does (K0: `https://discord.com/`), and the confirmation
/// rule sees the same text. Each case: (typed, url, host, shown, confirms).
@Test func plainLinksShowTheirCanonicalURL() {
    let cases: [(typed: String, url: String, host: String, shown: String, confirms: Bool)] = [
        ("https://a.b", "https://a.b/", "a.b", "https://a.b/", false),
        ("https://a.b/ñ", "https://a.b/%C3%B1", "a.b", "https://a.b/%C3%B1", false),
        ("https://例え.jp", "https://xn--r8jz45g.jp/", "xn--r8jz45g.jp", "https://xn--r8jz45g.jp/", true),
    ]
    for c in cases {
        let s = segments([.link(id: 0, url: c.url, host: c.host, label: [.text(c.typed)], masked: false)])
        #expect(plain(s) == c.shown, "\(c.typed)")
        #expect(links(s) == [MarkdownText.linkURL(0)])
        let recorder = Recorder()
        let body = MessageBody(blocks: [.paragraph([.link(id: 0, url: c.url, host: c.host, label: [.text(c.typed)], masked: false)])])
        #expect(body.links.map(\.visibleText) == [c.shown])
        _ = recorder.router(body).handle(MarkdownText.linkURL(0))
        #expect(recorder.confirmations.count == (c.confirms ? 1 : 0), "\(c.typed)")
        #expect(recorder.opened == (c.confirms ? [] : [URL(string: c.url)!]), "\(c.typed)")
    }
    // A masked link keeps its label and still confirms.
    let masked = MessageBody(blocks: [.paragraph([.link(id: 0, url: "https://a.b/", host: "a.b", label: [.text("看這裡")], masked: true)])])
    guard case .paragraph(let inlines) = masked.blocks[0] else { Issue.record("not a paragraph"); return }
    #expect(plain(MarkdownText(body: masked).segments(inlines, context: context)) == "看這裡")
    let recorder = Recorder()
    _ = recorder.router(masked).handle(MarkdownText.linkURL(0))
    #expect(recorder.confirmations.count == 1 && recorder.opened.isEmpty)
}

/// K2 verification S12: the handler answers `.handled` for every URL, so nothing reaches the
/// system's own URL handling.
@Test func theLinkHandlerAlwaysHandles() {
    let router = Recorder().router(linkBody("https://example.com/", host: "example.com", label: "https://example.com"))
    let handled = String(describing: OpenURLAction.Result.handled)
    for raw in ["velella-spoiler:0", "velella-link:0", "velella-link:9", "https://example.com/", "file:///etc/passwd", "garbage:", "x"] {
        #expect(String(describing: router.handle(URL(string: raw)!)) == handled, "\(raw)")
    }
    #expect(String(describing: OpenURLAction.Result.systemAction) != handled)
}

@Test func aSpoilerURLRevealsAndNeverOpens() {
    let recorder = Recorder()
    let router = recorder.router(linkBody("https://example.com/", host: "example.com", label: "https://example.com"))
    _ = router.handle(MarkdownText.spoilerURL(0))
    _ = router.handle(MarkdownText.spoilerURL(1)) // no such spoiler
    #expect(recorder.revealed == [0])
    #expect(recorder.opened.isEmpty && recorder.confirmations.isEmpty)
}

@Test func aHostMismatchOrABadTargetNeverOpens() {
    let cases: [(url: String, host: String)] = [
        ("https://evil.example/", "good.example"),
        ("javascript:alert(1)", ""),
        ("file:///etc/passwd", ""),
        ("https://user:pw@x.example/", "x.example"),
        ("https:\\\\evil.example\\", "evil.example"),
        ("velella-link:0", ""),
        ("ftp://x.example/", "x.example"),
    ]
    for (url, host) in cases {
        let recorder = Recorder()
        _ = recorder.router(linkBody(url, host: host, label: url)).handle(MarkdownText.linkURL(0))
        #expect(recorder.opened.isEmpty && recorder.confirmations.isEmpty, "\(url)")
    }
    // Indices and schemes the body never issued.
    let recorder = Recorder()
    let router = recorder.router(linkBody("https://example.com/", host: "example.com", label: "https://example.com"))
    for raw in ["velella-link:1", "velella-link:-1", "velella-link:", "velella-link:0x0", "https://example.com/", "velella-spoiler:-1"] {
        _ = router.handle(URL(string: raw)!)
    }
    #expect(recorder.opened.isEmpty && recorder.revealed.isEmpty && recorder.confirmations.isEmpty)
}

@Test func deceptiveLinksConfirmAndADeclinedConfirmationOpensNothing() {
    let cases = [
        linkBody("https://example.com/", host: "example.com", label: "click", masked: true),
        linkBody("https://example.com/", host: "example.com", label: "https://example.com", masked: true),
        linkBody("https://xn--80ak6aa92e.com/", host: "xn--80ak6aa92e.com", label: "https://xn--80ak6aa92e.com"),
        linkBody("https://example.com/", host: "example.com", label: "https://example.com/", masked: true),
    ]
    for body in cases {
        let recorder = Recorder()
        _ = recorder.router(body).handle(MarkdownText.linkURL(0))
        // Declined: the dialog never calls `accept`.
        #expect(recorder.opened.isEmpty)
        #expect(recorder.confirmations.count == 1)
        recorder.confirmations.first?.accept()
        #expect(recorder.opened == [URL(string: body.links[0].url)!])
    }
}

@Test func theConfirmationShowsTheHostThenTheEscapedTruncatedURL() {
    let long = "https://example.com/" + String(repeating: "a", count: 300) + "\u{202E}z"
    let message = LinkConfirmation(host: "example.com", url: long, accept: {}).message
    let lines = message.components(separatedBy: "\n")
    #expect(lines.first == "example.com")
    #expect(lines.last?.count == LinkConfirmation.urlLimit)
    #expect(lines.last?.hasPrefix("https://example.com/aaa") == true)
    #expect(lines.last?.hasSuffix("\\u{202E}z") == true)
    #expect(lines.last?.contains("\u{2026}") == true)
    #expect(LinkConfirmation.escaped("a\u{2068}b\u{7}c") == "a\\u{2068}b\\u{0007}c")
}

@Test func linksAndSpoilersAreNumberedInDocumentOrder() {
    let body = MessageBody(blocks: [
        .paragraph([.spoiler(id: 7, [.link(id: 7, url: "https://a.b/", host: "a.b", label: [.text("a")], masked: true)])]),
        .quote([.paragraph([.spoiler(id: 7, [.spoiler(id: 7, [.text("x")])]), .link(id: 7, url: "https://c.d/", host: "c.d", label: [.text("https://c.d")], masked: false)])]),
        .list(ordered: false, start: 1, items: [[.paragraph([.bold([.spoiler(id: 7, [.block(.paragraph([.link(id: 7, url: "https://e.f/", host: "e.f", label: [.text("e")], masked: true)]))])])])]]),
    ])
    #expect(body.links.map(\.url) == ["https://a.b/", "https://c.d/", "https://e.f/"])
    #expect(body.links.map(\.visibleText) == ["a", "https://c.d/", "e"])
    #expect(body.spoilerCount == 4)
    #expect(body.blocks[1] == .quote([.paragraph([.spoiler(id: 1, [.spoiler(id: 2, [.text("x")])]), .link(id: 1, url: "https://c.d/", host: "c.d", label: [.text("https://c.d/")], masked: false)])]))
}

// MARK: The paragraph host

@MainActor @Test func renderingATippedSegmentRecordsItsFrame() {
    let frames = TipFrames()
    let view = ParagraphView(frames: frames) { _ in
        [Segment(text: AttributedString("before ")), Segment(text: AttributedString("pill"), hoverTip: "stub tip"), Segment(text: AttributedString(" after"))]
    }
    let renderer = ImageRenderer(content: view.frame(width: 320))
    #expect(renderer.nsImage != nil)
    let recorded = frames.all
    #expect(recorded.map(\.tip) == ["stub tip"])
    guard let rect = recorded.first?.rect else { return }
    #expect(rect.width > 0 && rect.height > 0 && rect.minX > 0)
    #expect(frames.tip(at: CGPoint(x: rect.midX, y: rect.midY))?.tip == "stub tip")
    #expect(frames.tip(at: CGPoint(x: rect.minX / 2, y: rect.midY)) == nil)
}

/// The colour at the middle of an offscreen render of segments, as (red, green, blue).
@MainActor private func middlePixel(_ segments: [Segment]) -> (Double, Double, Double)? {
    let renderer = ImageRenderer(content: ParagraphView { _ in segments }.environment(\.colorScheme, .light))
    renderer.scale = 1
    guard let image = renderer.cgImage, let data = image.dataProvider?.data as Data?, image.bitsPerPixel == 32 else { return nil }
    let offset = (image.height / 2) * image.bytesPerRow + (image.width / 2) * 4
    let rgb = (Double(data[offset]), Double(data[offset + 1]), Double(data[offset + 2]))
    // BGRA or RGBA: compare as a set of channel values, never by order.
    return image.bitmapInfo.contains(.byteOrder32Little) ? (rgb.2, rgb.1, rgb.0) : rgb
}

/// Pills keep their fill under the segment renderer, and a hidden spoiler draws its cover, not
/// its glyphs (a red-on-red run would read red at its middle either way; the cover reads grey).
@MainActor @Test func pillFillsAndSpoilerCoversDrawUnderTheRenderer() throws {
    var red = AttributedString(String(repeating: "\u{2588}", count: 6))
    red.swiftUI.backgroundColor = .red
    red.swiftUI.foregroundColor = .red
    let filled = try #require(middlePixel([Segment(text: red)]))
    #expect(filled.0 > 200 && filled.1 < 100 && filled.2 < 100, "\(filled)")
    let covered = try #require(middlePixel([Segment(text: red, covered: true)]))
    #expect(!(covered.0 > 200 && covered.1 < 100 && covered.2 < 100), "\(covered)")
    #expect(abs(covered.0 - covered.1) < 30 && abs(covered.1 - covered.2) < 30, "\(covered)")
}

@Test func eachTimelineTickBuildsWithItsOwnDate() {
    let start = Date(timeIntervalSince1970: 1_000)
    let locale = Locale(identifier: "de-DE")
    let ticks = Array(RefreshSchedule(interval: 60).entries(from: start, mode: .normal).prefix(2))
    var seen: [RenderContext] = []
    for tick in ticks {
        _ = ParagraphView.tick(tick, locale: locale) { seen.append($0); return [] }
    }
    #expect(seen == [RenderContext(now: start, locale: locale), RenderContext(now: start + 60, locale: locale)])
    // Without a refresh the schedule is static: one entry.
    #expect(Array(RefreshSchedule(interval: nil).entries(from: start, mode: .normal)) == [start])
    #expect(Array(RefreshSchedule(interval: 0).entries(from: start, mode: .normal)) == [start])
}

/// K2 verification F1: a name holding its own PDI or isolate initiators can neither close the
/// FSI early nor leave it open, so " xyz" after it keeps its direction.
@Test func aNameCannotCloseOrHoldOpenItsIsolate() {
    let cases: [(String, String)] = [
        ("\u{2069}\u{202E}abc", "\u{2068}@\u{202E}abc\u{2069} xyz"),
        ("\u{2067}abc", "\u{2068}@\u{2067}abc\u{2069}\u{2069} xyz"),
        ("\u{2066}\u{2068}abc", "\u{2068}@\u{2066}\u{2068}abc\u{2069}\u{2069}\u{2069} xyz"),
        ("a\u{2066}b\u{2069}c", "\u{2068}@a\u{2066}b\u{2069}c\u{2069} xyz"),
    ]
    for (nick, want) in cases {
        let s = segments([.userMention(1), .text(" xyz")], mentions: [ResolvedMention(kind: .user, id: 1, name: nick)])
        #expect(plain(s) == want, "nick \(nick.unicodeScalars.map { String($0.value, radix: 16) })")
    }
}

/// K2 verification A3: VoiceOver gets "Spoiler" for each hidden run, never the hidden text.
@Test func hiddenSpoilerTextNeverReachesTheAccessibilityLabel() {
    let s = segments([.text("a "), .spoiler(id: 0, [.text("secret "), .bold([.text("more")])]), .text(" b")])
    let spoken = ParagraphView.spoken(s)
    #expect(!spoken.contains("secret") && !spoken.contains("more"))
    #expect(spoken == "a Spoiler b")
    let shown = segments([.spoiler(id: 0, [.text("secret")])], revealed: [0])
    #expect(ParagraphView.spoken(shown) == "secret")
}

/// The accessibility labels and values under `element`. SwiftUI's nodes answer the
/// `NSAccessibility` selectors without declaring the protocol to Swift, so they are sent as messages.
@MainActor private func spokenTexts(_ element: Any) -> [String] {
    guard let object = element as? NSObject else { return [] }
    func send(_ name: String) -> Any? {
        let selector = NSSelectorFromString(name)
        return object.responds(to: selector) ? object.perform(selector)?.takeUnretainedValue() : nil
    }
    let own = [send("accessibilityLabel"), send("accessibilityValue")].compactMap { $0 as? String }.filter { !$0.isEmpty }
    return own + ((send("accessibilityChildren") as? [Any]) ?? []).flatMap(spokenTexts)
}

/// K2 verification A-2: the spoken label is attached to the drawn paragraph, so VoiceOver reads
/// "Spoiler" in place of hidden text. Offscreen: the window is never ordered in.
@MainActor @Test func theDrawnParagraphSpeaksItsLabelNotHiddenText() {
    let s = segments([.text("a "), .spoiler(id: 0, [.text("secret")]), .text(" b")])
    let host = NSHostingView(rootView: ParagraphView { _ in s }.frame(width: 320))
    let window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 320, height: 80), styleMask: .borderless, backing: .buffered, defer: true)
    window.contentView = host
    // What an assistive app sets on the application; SwiftUI builds no accessibility nodes before.
    NSApplication.shared.accessibilitySetValue(true, forAttribute: NSAccessibility.Attribute(rawValue: "AXEnhancedUserInterface"))
    host.layoutSubtreeIfNeeded()
    let spoken = spokenTexts(host)
    #expect(spoken.contains("a Spoiler b"), "\(spoken)")
    #expect(!spoken.contains { $0.contains("secret") }, "\(spoken)")
}

/// P2-4/N3: only https on Discord's emoji CDN is ever fetched.
@Test func onlyTheEmojiCDNOverHTTPSIsFetched() {
    #expect(EmojiImages.fetchable(URL(string: "https://cdn.discordapp.com/emojis/1.webp?size=48")!))
    for bad in ["http://cdn.discordapp.com/emojis/1.webp", "https://cdn.discordapp.com.evil/emojis/1.webp",
                "https://evil.example/emojis/1.webp", "file:///etc/passwd"] {
        #expect(!EmojiImages.fetchable(URL(string: bad)!), "\(bad)")
    }
}
