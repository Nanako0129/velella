import AppKit
import Observation
import SwiftUI
import Testing
@testable import VelellaUI

// The message text view (docs/plans/selection.md, N1 Tests): the N0 rows kept for B (N0-7, N0-10,
// N0-12, N0-13, N0-15) and N1's own cases. Every copy goes to an injected pasteboard; the suite
// checks that the general pasteboard never changes.

/// A body that can change under a hosted `MarkdownView` without replacing the view.
@MainActor @Observable
final class BodyModel {
    var body: MessageBody
    init(_ body: MessageBody) { self.body = body }
}

struct BodyHost: View {
    let model: BodyModel
    var opener: (URL) -> Void = { _ in }
    var board: NSPasteboard?
    var body: some View {
        MarkdownView(content: model.body, opener: opener).environment(\.textPasteboard, PasteboardBox(board: board))
    }
}

@MainActor
private final class ClickRecorder {
    var opened: [URL] = []
    var confirmed: [LinkConfirmation] = []
    var revealed: [Int] = []

    func router(_ body: MessageBody) -> LinkRouter {
        LinkRouter(body: body, reveal: { self.revealed.append($0) }, confirm: { self.confirmed.append($0) }, open: { self.opened.append($0) })
    }

    /// The view's clicks go through a recording router, as `MarkdownView`'s `openURL` routes them.
    func attach(_ view: MessageTextView, _ body: MessageBody) {
        let router = router(body)
        view.onActivate = { _ = router.handle($0.url) }
    }
}

/// `SpoilerCover` without its `.hidden()`: the mutation of N0-7's block case.
private struct UnhiddenCover<Content: View>: View {
    @ViewBuilder let content: Content
    var body: some View {
        content.overlay { RoundedRectangle(cornerRadius: MarkdownPalette.spoilerRadius).fill(MarkdownPalette.spoilerCover) }
    }
}

private let hiddenInlines: [MarkdownInline] = [.text("a "), .spoiler(id: 0, [.text("secret")]), .text(" b")]

@MainActor
private func range(of key: NSAttributedString.Key, in view: MessageTextView, value: Int? = nil) -> NSRange {
    var found = NSRange(location: NSNotFound, length: 0)
    view.attributed.enumerateAttribute(key, in: NSRange(location: 0, length: view.length)) { v, r, stop in
        guard v != nil, value == nil || v as? Int == value else { return }
        found = r
        stop.pointee = true
    }
    return found
}

@MainActor
private func hiddenRects(_ view: MessageTextView) -> [CGRect] {
    var out: [CGRect] = []
    view.attributed.enumerateAttribute(TextKey.hidden, in: NSRange(location: 0, length: view.length)) { v, r, _ in
        if v != nil { out += view.rects(for: r) }
    }
    return out
}

/// The largest channel spread over each hidden rect of `view`, inset by the cover radius.
@MainActor
private func coverSpread(_ view: MessageTextView, in window: NSWindow) -> Int {
    guard let backdrop = window.contentView, let pixels = TextHarness.Pixels(backdrop) else { return -2 }
    let rects = hiddenRects(view).map { view.convert($0, to: backdrop).insetBy(dx: MarkdownPalette.spoilerRadius, dy: MarkdownPalette.spoilerRadius) }
    return rects.map { pixels.spread($0) }.max() ?? -3
}

/// Selects all of `view` by a synthesized drag from its first to its last character.
@MainActor
private func selectAll(_ view: MessageTextView) {
    let first = view.rects(for: NSRange(location: 0, length: 1))[0]
    let last = view.rects(for: NSRange(location: view.length - 1, length: 1)).last!
    TextHarness.drag(view, from: CGPoint(x: first.minX + 0.5, y: first.midY), to: CGPoint(x: last.maxX + 2, y: last.midY))
}

/// What a select-all and ⌘C of `view` writes to its board.
@MainActor
private func copyAll(_ view: MessageTextView) -> String? {
    selectAll(view)
    view.copy(nil)
    return view.pasteboard.string(forType: .string)
}

private func hasIsolates(_ s: String) -> Bool { s.unicodeScalars.contains { (0x2066...0x2069).contains($0.value) } }

@MainActor @Suite(.serialized) struct MessageTextTests {
    // MARK: N0-7: hidden spoilers are never drawn

    @Test func n0_7_coversAreUniformInFourStatesAndEachLayerHoldsAlone() {
        TextHarness.guardingGeneral {
            let segments = TextHarness.segments(hiddenInlines).segments
            @MainActor func spreads(clip: Bool, clear: Bool) -> (unselected: Int, key: Int, notKey: Int, hover: Int, keyPixel: (Int, Int, Int)?, notKeyPixel: (Int, Int, Int)?) {
                let (view, window) = TextHarness.paragraph(segments, clearHidden: clear)
                view.clipsHidden = clip
                #expect(window.isKeyWindow && window.firstResponder === view)
                let unselected = coverSpread(view, in: window)
                selectAll(view)
                #expect(window.isKeyWindow && window.firstResponder === view, "key state or first responder lost")
                let key = coverSpread(view, in: window)
                let keyPixel = window.contentView.flatMap { TextHarness.Pixels($0) }?.rgb(4, 11)
                window.keyState = false
                view.needsDisplay = true
                #expect(!window.isKeyWindow)
                let notKey = coverSpread(view, in: window)
                let notKeyPixel = window.contentView.flatMap { TextHarness.Pixels($0) }?.rgb(4, 11)
                window.keyState = true
                view.needsDisplay = true
                let cover = hiddenRects(view)[0]
                TextHarness.send(TextHarness.mouse(.mouseMoved, at: CGPoint(x: cover.midX, y: cover.midY), in: view), to: window)
                #expect(window.isKeyWindow)
                return (unselected, key, notKey, coverSpread(view, in: window), keyPixel, notKeyPixel)
            }
            let both = spreads(clip: true, clear: true)
            for spread in [both.unselected, both.key, both.notKey, both.hover] { #expect((0...2).contains(spread), "both layers: \(both)") }
            #expect(both.keyPixel != nil && both.notKeyPixel != nil && both.keyPixel! != both.notKeyPixel!, "selection colour should follow the key state")
            let layer1 = spreads(clip: true, clear: false)
            for spread in [layer1.unselected, layer1.key, layer1.notKey, layer1.hover] { #expect((0...2).contains(spread), "layer 1 alone: \(layer1)") }
            let layer2 = spreads(clip: false, clear: true)
            for spread in [layer2.unselected, layer2.key, layer2.notKey, layer2.hover] { #expect((0...2).contains(spread), "layer 2 alone: \(layer2)") }
            let neither = spreads(clip: false, clear: false)
            for spread in [neither.unselected, neither.key, neither.notKey] { #expect(spread > 2, "neither layer must show the glyphs: \(neither)") }
        }
    }

    @Test func n0_7_aHiddenCodeBlockStaysUnderItsCover() throws {
        func render<V: View>(_ view: V) -> (pixels: TextHarness.Pixels?, code: CGRect?) {
            let (host, window) = TextHarness.hosting(view.frame(width: 300).padding(10), size: CGSize(width: 320, height: 90))
            let code = TextHarness.textViews(in: host).first.map { $0.convert($0.bounds, to: window.contentView) }
            return (window.contentView.flatMap { TextHarness.Pixels($0) }, code)
        }
        func code() -> CodeBlockView { CodeBlockView(code: "secret = 1", header: nil, highlight: nil, size: 14, style: InlineStyle(spoilers: [0]), revealed: []) }
        let unhidden = render(UnhiddenCover { code() })
        let rect = try #require(unhidden.code).insetBy(dx: MarkdownPalette.spoilerRadius, dy: MarkdownPalette.spoilerRadius)
        let hidden = render(SpoilerCover(hidden: 0) { code() })
        #expect((0...2).contains(hidden.pixels?.spread(rect) ?? -1), "hidden block not uniform")
        #expect((unhidden.pixels?.spread(rect) ?? -1) > 2, "the un-hidden block must fail")
    }

    // MARK: N0-10: accessibility

    @Test func n0_10_noAccessibilityOutputHoldsHiddenText() {
        TextHarness.guardingGeneral {
            let (view, window) = TextHarness.paragraph(TextHarness.segments(hiddenInlines).segments)
            selectAll(view)
            var seen = Set<ObjectIdentifier>(), records: [TextAX.Record] = []
            TextAX.walk(view, seen: &seen, into: &records)
            #expect(TextAX.leaks(records).isEmpty, "\(TextAX.leaks(records).map { "\($0.attribute): \($0.output)" })")
            #expect(records.contains { $0.attribute == "accessibilityValue" && $0.output == "\"a Spoiler b\"" }, "the value is the spoken text")
            _ = window
            // Hosted: an inline and a block spoiler in a body.
            let body = MessageBody(blocks: [
                .paragraph(hiddenInlines),
                .paragraph([.spoiler(id: 0, [.block(.codeBlock(language: nil, code: "secret", header: nil, highlight: nil))])]),
            ])
            let (host, hostWindow) = TextHarness.hosting(MarkdownView(content: body).frame(width: 300), size: CGSize(width: 320, height: 160), accessibility: true)
            for view in TextHarness.textViews(in: host) where view.length > 0 { selectAll(view) }
            var hostSeen = Set<ObjectIdentifier>(), hosted: [TextAX.Record] = []
            TextAX.walk(host, seen: &hostSeen, into: &hosted)
            #expect(TextAX.leaks(hosted).isEmpty, "\(TextAX.leaks(hosted).map { "\($0.path): \($0.attribute): \($0.output)" })")
            #expect(hosted.contains { $0.output.contains("a Spoiler b") }, "the walk reached the text view")
            _ = hostWindow
        }
    }

    // MARK: N0-12: the pasteboard

    @Test func n0_12_copyWritesOnePlainString() {
        TextHarness.guardingGeneral {
            let (view, _) = TextHarness.paragraph(TextHarness.segments([.text("a "), .bold([.text("bold")]), .text(" "), .spoiler(id: 0, [.text("secret")]), .text(" b")]).segments)
            view.pasteboard.declareTypes([.rtf, .html], owner: nil)
            view.pasteboard.setString("<b>old</b>", forType: .html)
            #expect(copyAll(view) == "a bold secret b")
            #expect(Set(view.pasteboard.types?.map(\.rawValue) ?? []) == TextHarness.stringFlavours)
        }
    }

    // MARK: N0-13: links without `.link`

    @Test func n0_13_linksAreIndicesAndActivateOnlyOnAClick() throws {
        try TextHarness.guardingGeneral {
            let inlines: [MarkdownInline] = [
                .text("see "), .link(id: 0, url: "https://example.com/a", host: "example.com", label: [.text("https://example.com/a")], masked: false),
                .text(" and "), .link(id: 0, url: "https://masked.example/", host: "masked.example", label: [.text("click me")], masked: true),
                .text(" and "), .spoiler(id: 0, [.link(id: 0, url: "https://hidden.example/", host: "hidden.example", label: [.text("secret")], masked: true)]), .text(" end"),
            ]
            let (segments, body) = TextHarness.segments(inlines)
            let (view, window) = TextHarness.paragraph(segments, width: 600)
            let recorder = ClickRecorder()
            recorder.attach(view, body)
            var keys = Set<String>(), urls = 0
            view.attributed.enumerateAttributes(in: NSRange(location: 0, length: view.length)) { attrs, _, _ in
                for (key, value) in attrs {
                    keys.insert(key.rawValue)
                    if value is URL || value is NSURL { urls += 1 }
                }
            }
            #expect(!keys.contains(NSAttributedString.Key.link.rawValue) && urls == 0, "\(keys)")
            // A covered run carries only its spoiler index.
            let spoiler = range(of: TextKey.spoiler, in: view)
            #expect(view.attributed.attribute(TextKey.link, at: spoiler.location, effectiveRange: nil) == nil)
            let plain = range(of: TextKey.link, in: view, value: 0), masked = range(of: TextKey.link, in: view, value: 1)
            TextHarness.click(view, at: TextHarness.center(view, plain))
            #expect(recorder.opened.map(\.absoluteString) == ["https://example.com/a"] && recorder.confirmed.isEmpty)
            TextHarness.click(view, at: TextHarness.center(view, masked))
            #expect(recorder.opened.count == 1 && recorder.confirmed.count == 1)
            TextHarness.click(view, at: TextHarness.center(view, spoiler))
            #expect(recorder.revealed == [0] && recorder.opened.count == 1 && recorder.confirmed.count == 1)
            // A drag that starts on a link opens nothing.
            let start = TextHarness.center(view, plain)
            TextHarness.drag(view, from: start, to: CGPoint(x: start.x + 60, y: start.y))
            #expect(recorder.opened.count == 1 && recorder.confirmed.count == 1 && recorder.revealed == [0])
            try #require(recorder.confirmed.first).accept()
            #expect(recorder.opened.count == 2)
            _ = window
        }
    }

    @Test func aClickFortyPointsPastALinkOpensNothing() {
        let (segments, body) = TextHarness.segments([.text("see "), .link(id: 0, url: "https://example.com/a", host: "example.com", label: [.text("https://example.com/a")], masked: false)])
        let (view, _) = TextHarness.paragraph(segments, width: 500)
        let recorder = ClickRecorder()
        recorder.attach(view, body)
        let end = view.rects(for: range(of: TextKey.link, in: view)).last!
        let past = CGPoint(x: end.maxX + 40, y: end.midY)
        #expect(view.activation(at: past) == nil && view.activation(at: CGPoint(x: end.midX, y: end.midY)) == .link(0))
        TextHarness.click(view, at: past)
        #expect(recorder.opened.isEmpty)
    }

    @Test func aDoubleClickOnAPlainLinkOpensAtMostOnce() {
        TextHarness.guardingGeneral {
            var opened: [URL] = []
            let body = MessageBody(blocks: [.paragraph([.text("go "), .link(id: 0, url: "https://example.com/", host: "example.com", label: [.text("https://example.com/")], masked: false)])])
            let (host, window) = TextHarness.hosting(BodyHost(model: BodyModel(body), opener: { opened.append($0) }).frame(width: 400), size: CGSize(width: 400, height: 60))
            guard let view = TextHarness.textViews(in: host).first else { Issue.record("no text view"); return }
            let point = TextHarness.center(view, range(of: TextKey.link, in: view))
            TextHarness.click(view, at: point, count: 1)
            TextHarness.click(view, at: point, count: 2)
            #expect(opened == [URL(string: "https://example.com/")!], "\(opened)")
            _ = window
        }
    }

    @Test func aDragFromAHiddenSpoilerRevealsNothing() {
        let (segments, body) = TextHarness.segments(hiddenInlines)
        let (view, _) = TextHarness.paragraph(segments)
        let recorder = ClickRecorder()
        recorder.attach(view, body)
        let start = TextHarness.center(view, range(of: TextKey.hidden, in: view))
        TextHarness.drag(view, from: start, to: CGPoint(x: start.x + 60, y: start.y))
        #expect(recorder.revealed.isEmpty)
        TextHarness.click(view, at: start)
        #expect(recorder.revealed == [0], "a click still reveals")
    }

    // MARK: N0-15: edits under covers and selections

    @Test func n0_15_aCoverFollowsAnEditOfTheSameView() {
        TextHarness.guardingGeneral {
            let (view, window) = TextHarness.paragraph(TextHarness.segments([.text("a "), .spoiler(id: 0, [.text("secret")])]).segments)
            selectAll(view)
            #expect((0...2).contains(coverSpread(view, in: window)), "drawn once before the edit")
            view.setContent(.paragraph(TextHarness.segments([.text("aaaaaaaa "), .spoiler(id: 0, [.text("secret")])]).segments))
            view.frame.size.height = view.height(forWidth: 320)
            #expect((0...2).contains(coverSpread(view, in: window)))
            // The cover is drawn where the run now is, not where it was.
            let rect = hiddenRects(view)[0]
            #expect(rect.minX > 40, "the run moved")
            let pixels = window.contentView.flatMap { TextHarness.Pixels($0) }
            #expect(pixels?.rgb(rect.midX, rect.midY) != nil && pixels!.rgb(rect.midX, rect.midY) != pixels!.rgb(318, rect.midY), "no cover at the run")
        }
    }

    @Test func n0_15_anEditUnderASelectionAcrossViewsNeverTraps() throws {
        try TextHarness.guardingGeneral {
            let board = TextHarness.board()
            let model = BodyModel(MessageBody(blocks: [.paragraph([.text("first paragraph")]), .heading(level: 2, [.text("second part")])]))
            let (host, window) = TextHarness.hosting(BodyHost(model: model, board: board).frame(width: 400), size: CGSize(width: 400, height: 200))
            let views = TextHarness.textViews(in: host)
            try #require(views.count == 2)
            let a = views[0], b = views[1]
            let start = a.rects(for: NSRange(location: 6, length: 1))[0]
            let end = b.rects(for: NSRange(location: 5, length: 1))[0]
            TextHarness.send(TextHarness.mouse(.leftMouseDown, at: CGPoint(x: start.minX + 0.5, y: start.midY), in: a), to: window)
            TextHarness.send(TextHarness.mouse(.leftMouseDragged, at: CGPoint(x: end.maxX - 0.5, y: end.midY), in: b), to: window)
            TextHarness.send(TextHarness.mouse(.leftMouseUp, at: CGPoint(x: end.maxX - 0.5, y: end.midY), in: b), to: window)
            a.copy(nil)
            #expect(board.string(forType: .string) == "paragraph\nsecond", "\(String(describing: board.string(forType: .string)))")
            // The second view's text goes: no trap, only the first view's part.
            model.body = MessageBody(blocks: [.paragraph([.text("first paragraph")]), .heading(level: 2, [])])
            host.layoutSubtreeIfNeeded()
            a.copy(nil)
            #expect(board.string(forType: .string) == "paragraph")
            // A shorter text whose only character spans the old end: clamped, then snapped to the
            // whole character, never a lone surrogate.
            model.body = MessageBody(blocks: [.paragraph([.text("first paragraph")]), .heading(level: 2, [.text("👩‍👩‍👧")])])
            host.layoutSubtreeIfNeeded()
            a.copy(nil)
            #expect(board.string(forType: .string) == "paragraph\n👩‍👩‍👧", "\(String(describing: board.string(forType: .string)))")
        }
    }

    // MARK: The view itself

    @Test func aFullRenderHasNoTextViewAndEachViewOwnsItsStorage() throws {
        let body = MessageBody(blocks: [
            .heading(level: 1, [.text("Title")]), .paragraph([.text("Body "), .code("code"), .text(" "), .spoiler(id: 0, [.text("hidden")])]),
            .subtext([.text("small")]), .quote([.paragraph([.text("quoted")])]),
            .list(ordered: true, start: 1, items: [[.paragraph([.text("one")])], [.paragraph([.text("two")])]]),
            .codeBlock(language: "py", code: "x = 1\ny = 2", header: "Python", highlight: nil),
            .paragraph([.spoiler(id: 0, [.block(.codeBlock(language: nil, code: "covered", header: nil, highlight: nil))])]),
        ])
        let (host, window) = TextHarness.hosting(MarkdownView(content: body).frame(width: 400), size: CGSize(width: 400, height: 600))
        #expect(TextHarness.all(NSTextView.self, in: host).isEmpty)
        let views = TextHarness.textViews(in: host)
        // Seven text blocks; the code block under a hidden spoiler makes no view at all under `.hidden()`.
        #expect(views.count == 7, "\(views.count)")
        #expect(!views.contains { $0.attributed.string.contains("covered") })
        for view in views {
            #expect(view.layoutManager.textContentManager === view.contentStorage)
            #expect(view.contentStorage.textStorage?.layoutManagers.isEmpty == true)
            #expect(view.layoutManager.textContainer?.layoutManager == nil)
        }
        #expect(Set(views.map { ObjectIdentifier($0.contentStorage) }).count == views.count)
        _ = window
    }

    @Test func theTextViewIsAStaticTextElementAndNothingElse() {
        let (view, _) = TextHarness.paragraph(TextHarness.segments([.text("plain words here")]).segments)
        #expect(view.conforms(to: NSAccessibilityStaticText.self))
        #expect(!view.conforms(to: NSTextInputClient.self))
        #expect(!view.conforms(to: NSServicesMenuRequestor.self))
        #expect(!view.conforms(to: NSTextFinderClient.self))
        #expect(!view.conforms(to: NSAccessibilityNavigableStaticText.self))
        selectAll(view)
        #expect(view.selection.hasSelection)
        #expect(view.validRequestor(forSendType: .string, returnType: .string) == nil)
        #expect(view.validRequestor(forSendType: .string, returnType: nil) == nil)
        let event = TextHarness.mouse(.rightMouseDown, at: CGPoint(x: 5, y: 5), in: view)
        #expect(view.menu(for: event) == nil)
        view.quickLook(with: event)
        let valid = ["copy:", "selectAll:", "cut:", "paste:", "delete:", "print:", "startSpeaking:", "takeFindStringFromSelection:",
                     "performFindPanelAction:", "performTextFinderAction:", "showWritingTools:", "orderFrontSharingServicePicker:"].filter {
            view.validateUserInterfaceItem(NSMenuItem(title: $0, action: NSSelectorFromString($0), keyEquivalent: ""))
        }
        #expect(valid == ["copy:"])
        #expect(view.accessibilityRole() == .staticText)
    }

    @Test func aPressMakesTheTextViewFirstResponderAndCopyFollowsTheResponderChain() throws {
        try TextHarness.guardingGeneral {
            let board = TextHarness.board()
            let model = BodyModel(MessageBody(blocks: [.paragraph([.text("hello responder chain")])]))
            let (host, window) = TextHarness.hosting(BodyHost(model: model, board: board).frame(width: 400), size: CGSize(width: 400, height: 60))
            let view = try #require(TextHarness.textViews(in: host).first)
            window.makeFirstResponder(nil)
            selectAll(view)
            #expect(window.isKeyWindow)
            #expect(window.firstResponder === view)
            #expect(window.firstResponder?.tryToPerform(#selector(MessageTextView.copy(_:)), with: nil) == true)
            #expect(board.string(forType: .string) == "hello responder chain")
        }
    }

    // MARK: Copy mapping (D2, S-14, S-15, E1)

    @Test func substitutedRunsCopyTheirRawTextWithoutIsolates() {
        TextHarness.guardingGeneral {
            let url = TextHarness.emojiURL()
            EmojiImages.shared.insert(TextHarness.image(), for: url)
            let (segments, _) = TextHarness.segments(
                [.slashCommand(name: "cmd sub", id: 1), .text(" "), .customEmoji(id: 9, name: "nolist", animated: false), .text(" "),
                 .spoiler(id: 0, [.customEmoji(id: 4, name: "hid", animated: false)]), .text(" "), .userMention(1), .userMention(1), .text(" "),
                 .customEmoji(id: 4, name: "img", animated: false), .text(" author\u{2068}own")],
                mentions: [ResolvedMention(kind: .user, id: 1, name: "\u{202E}Ada")], emoji: [EmojiImage(id: 4, animated: false, url: url)])
            let (view, _) = TextHarness.paragraph(segments, width: 600)
            #expect(view.attributed.string.contains("\u{FFFC}"), "the loaded emoji is an attachment")
            let copied = copyAll(view) ?? ""
            #expect(copied == "/cmd sub :nolist: :hid: @\u{202E}Ada@\u{202E}Ada :img: author\u{2068}own", "\(copied.unicodeScalars.map { String($0.value, radix: 16) })")
            #expect(!copied.contains("\u{FFFC}"))
            #expect(!hasIsolates(copied.replacingOccurrences(of: "author\u{2068}own", with: "")))
        }
    }

    @Test func codeCopiesItsRawScalarsAndAnEscapeIsCopiedWholeOrNotAtAll() throws {
        TextHarness.guardingGeneral {
            let code = "a\u{1B}b\u{202E}c"
            let (codeText, chrome) = MessageTextContent.codeBlock(code: code, header: "Python", highlight: nil, size: 14)
            let (view, _) = TextHarness.view(MessageTextContent(text: codeText, chrome: chrome, spoken: ""), width: 320)
            #expect(view.attributed.string.contains("\\u{202E}") && !view.attributed.string.contains("\u{202E}"), "drawn escaped")
            #expect(copyAll(view) == code)
            // A selection ending inside the ESC escape: the whole character or none of it.
            let display = view.attributed.string as NSString
            let start = display.range(of: "a").location
            let inside = display.range(of: "\\u{001B}").location + 3
            let from = view.rects(for: NSRange(location: start, length: 1))[0]
            let to = view.rects(for: NSRange(location: inside, length: 1))[0]
            TextHarness.drag(view, from: CGPoint(x: from.minX + 0.5, y: from.midY), to: CGPoint(x: to.minX + 0.5, y: to.midY))
            view.copy(nil)
            let partial = view.pasteboard.string(forType: .string)
            #expect(partial == "a\u{1B}" || partial == "a", "\(String(describing: partial?.debugDescription))")
        }
    }

    /// N0-4's rows: one line is 19 tall (18 + 1), a wrapped row n x 18. Padding 7 (0.5 em at 14) each side plus 1 on top.
    @Test func codeRowsAreNineteenForOneLineAndEighteenPerWrappedLine() {
        func view(_ code: String) -> MessageTextView {
            let (text, chrome) = MessageTextContent.codeBlock(code: code, header: nil, highlight: nil, size: 14)
            return TextHarness.view(MessageTextContent(text: text, chrome: chrome, spoken: ""), width: 600).view
        }
        #expect(abs(view("x").height(forWidth: 600) - 34) < 0.01)
        let long = view(String(repeating: "word ", count: 30))
        let wide = long.height(forWidth: 2000)
        let narrow = long.height(forWidth: 300)
        #expect(abs(wide - 34) < 0.01, "\(wide)")
        let rows = (narrow - 15) / 18
        #expect(rows >= 2 && abs(rows - rows.rounded()) < 0.01, "\(narrow)")
    }

    @Test func anAuthorsEscapeAndPopSurviveACopy() {
        TextHarness.guardingGeneral {
            let (view, _) = TextHarness.paragraph(TextHarness.segments([.text("x\u{1B}[31my\u{202C}z")]).segments)
            #expect(copyAll(view) == "x\u{1B}[31my\u{202C}z")
        }
    }

    @Test func aCoveredEmojiMakesNoAttachmentAndNoFetch() {
        let url = URL(string: "https://cdn.discordapp.com/emojis/100000000000000004.webp?size=48")!
        EmojiImages.shared.insert(TextHarness.image(), for: url)
        let (segments, _) = TextHarness.segments([.spoiler(id: 0, [.customEmoji(id: 4, name: "cat", animated: false)])], emoji: [EmojiImage(id: 4, animated: false, url: url)])
        // Only a segment's image starts a load (`TextLeaf.imageURLs` feeds `EmojiImages.load`).
        #expect(TextLeaf.text(segments: segments, size: 16, weight: .regular, secondary: false).imageURLs.isEmpty)
        let (view, _) = TextHarness.paragraph(segments)
        var attachments = 0
        view.attributed.enumerateAttribute(.attachment, in: NSRange(location: 0, length: view.length)) { v, _, _ in if v != nil { attachments += 1 } }
        #expect(attachments == 0)
        #expect(view.spoken == "Spoiler")
    }

    // MARK: Spoilers over time

    @Test func aRevealThenAnEditShowsTheNewCoverAtOnce() throws {
        let model = BodyModel(MessageBody(blocks: [.paragraph([.text("x "), .spoiler(id: 0, [.text("first secret")])])]))
        let (host, window) = TextHarness.hosting(BodyHost(model: model).frame(width: 400), size: CGSize(width: 400, height: 60))
        let view = try #require(TextHarness.textViews(in: host).first)
        TextHarness.click(view, at: TextHarness.center(view, range(of: TextKey.hidden, in: view)))
        host.layoutSubtreeIfNeeded()
        #expect(range(of: TextKey.hidden, in: view).location == NSNotFound, "revealed")
        model.body = MessageBody(blocks: [.paragraph([.text("x "), .spoiler(id: 0, [.text("other secret")])])])
        host.layoutSubtreeIfNeeded()
        let edited = try #require(TextHarness.textViews(in: host).first)
        #expect(range(of: TextKey.hidden, in: edited).location != NSNotFound, "the new spoiler is covered on its first render")
        #expect((0...2).contains(coverSpread(edited, in: window)))
        #expect(edited.spoken == "x Spoiler")
    }

    @Test func aResizeKeepsAWrappedCoverUniform() throws {
        let words = String(repeating: "secret words ", count: 12)
        let (host, window) = TextHarness.hosting(MarkdownView(content: MessageBody(blocks: [.paragraph([.text("lead "), .spoiler(id: 0, [.text(words)])])])),
                                                 size: CGSize(width: 600, height: 200))
        window.setContentSize(NSSize(width: 320, height: 200))
        host.frame = NSRect(x: 0, y: 0, width: 320, height: 200)
        window.contentView?.frame = host.frame
        host.layoutSubtreeIfNeeded()
        let view = try #require(TextHarness.textViews(in: host).first)
        #expect(view.bounds.width <= 320 && hiddenRects(view).count > 1, "the spoiler wraps at 320")
        #expect((0...2).contains(coverSpread(view, in: window)))
    }

    // MARK: Limits (F-4)

    static func limitRows() -> [(name: String, message: MessageItem)] {
        let code = MarkdownBlock.codeBlock(language: nil, code: "x", header: nil, highlight: nil)
        let codeBody = MessageBody(blocks: Array(repeating: code, count: 850))
        let embeds = (0..<10).map { _ in EmbedItem(title: "t", description: codeBody) }
        return [
            ("2300 code blocks", TextPerf.message(MessageBody(blocks: Array(repeating: code, count: 2300)))),
            ("code blocks interleaved with text", TextPerf.message(MessageBody(blocks: (0..<2000).map { $0 % 2 == 0 ? code : .paragraph([.text("text \($0)")]) }))),
            ("1000 list items", TextPerf.message(MessageBody(blocks: [.list(ordered: false, start: 1, items: (0..<1000).map { [.paragraph([.text("item \($0)")])] })]))),
            ("1000 headings", TextPerf.message(MessageBody(blocks: (0..<1000).map { .heading(level: 2, [.text("heading \($0)")]) }))),
            ("10 embeds of code blocks", TextPerf.message(MessageBody(text: "embeds"), embeds: embeds)),
        ]
    }

    @Test(arguments: 0..<5) func aRowHoldsAtMostSixtyFiveViewsPerBody(_ row: Int) {
        let (name, message) = Self.limitRows()[row]
        let window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 1200, height: 800), styleMask: [.borderless], backing: .buffered, defer: false)
        window.isReleasedWhenClosed = false
        let host = NSHostingView(rootView: AnyView(EmptyView()))
        host.frame = NSRect(x: 0, y: 0, width: 1200, height: 800)
        window.contentView = host
        let t0 = ContinuousClock.now
        host.rootView = AnyView(TextPerf.Column(messages: [message]))
        host.layoutSubtreeIfNeeded()
        host.displayIfNeeded()
        let ms = TextPerf.ms(ContinuousClock.now - t0)
        let views = TextHarness.textViews(in: host)
        let perBody = Dictionary(grouping: views) { ObjectIdentifier($0.selection) }.values.map(\.count)
        print("TEXTPERF limit \(name): first frame \(String(format: "%.1f", ms)) ms, \(views.count) text views, at most \(perBody.max() ?? 0) per body")
        #expect((perBody.max() ?? 0) <= DesignTokens.TextView.viewLimit + 1, "\(name): \(perBody.max() ?? 0) views in one body")
        #expect(views.contains { $0.participant == DesignTokens.TextView.viewLimit }, "\(name): no remainder view")
        if TextPerf.release { #expect(ms <= TextPerf.adversarialLimit, "\(name): \(ms) ms") }
    }

    @Test func theRemainderKeepsCoversAccessibilityAndEscapes() throws {
        let code = MarkdownBlock.codeBlock(language: nil, code: "x", header: nil, highlight: nil)
        let body = MessageBody(blocks: Array(repeating: code, count: 64) + [
            .paragraph([.spoiler(id: 0, [.block(.codeBlock(language: nil, code: "secret", header: nil, highlight: nil))])]),
            .codeBlock(language: nil, code: "\u{202E}x", header: nil, highlight: nil),
        ])
        let height = NSHostingView(rootView: MarkdownView(content: body).frame(width: 400)).fittingSize.height
        let (host, window) = TextHarness.hosting(MarkdownView(content: body).frame(width: 400), size: CGSize(width: 400, height: height), accessibility: true)
        let views = TextHarness.textViews(in: host)
        #expect(views.count == 65)
        let rest = try #require(views.first { $0.participant == DesignTokens.TextView.viewLimit })
        #expect(rest.attributed.string.contains("\\u{202E}x") && !rest.attributed.string.contains("\u{202E}"), "drawn escaped")
        #expect(!hiddenRects(rest).isEmpty)
        #expect((0...2).contains(coverSpread(rest, in: window)))
        var seen = Set<ObjectIdentifier>(), records: [TextAX.Record] = []
        TextAX.walk(host, seen: &seen, into: &records)
        #expect(TextAX.leaks(records).isEmpty, "\(TextAX.leaks(records).map(\.output))")
        #expect(rest.spoken == "Spoiler\n\\u{202E}x")
    }
}
