import AppKit
import Observation
import SwiftUI
import Testing
@testable import VelellaUI

// Composer plan CG (docs/plans/composer.md): the multi-line editor's growth and height limit, its
// keys, the long-text rules and the channel-switch transaction. Everything that hosts a view runs it in
// an offscreen window (never ordered in), and keys go to the real text view as events.

// MARK: Rig

@MainActor @Observable private final class Model {
    var draft = ""
    var reply: ReplyPreview?
    var channel = "1"
    var premium: UInt8?
    var sent: [(String, ReplyTarget?)] = []
    var opened: [URL] = []
}

private struct Host: View {
    let model: Model
    var body: some View {
        Composer(
            reply: Binding(get: { model.reply }, set: { model.reply = $0 }),
            draft: Binding(get: { model.draft }, set: { model.draft = $0 }),
            channelID: model.channel, premiumType: model.premium,
            onSend: { model.sent.append(($0, $1)) }
        )
        .frame(width: 640)
        .environment(\.openURL, OpenURLAction { model.opened.append($0); return .handled })
    }
}

private let reply = ReplyPreview(authorName: "Ada", segments: [.text("question")], messageID: "30", channelID: "1")

@MainActor private func settleLayout(_ view: NSView) async {
    for _ in 0..<6 {
        view.layoutSubtreeIfNeeded()
        try? await Task.sleep(for: .milliseconds(30))
    }
}

@MainActor private final class Rig {
    let model = Model()
    let host: NSHostingView<Host>
    let window: TextWindow?

    /// The composer in a window `windowHeight` tall, or in none (nil).
    init(windowHeight: CGFloat?, premium: UInt8? = nil) async {
        model.premium = premium
        if let windowHeight {
            let (host, window) = TextHarness.hosting(Host(model: model), size: CGSize(width: 700, height: windowHeight))
            self.host = host; self.window = window
        } else {
            host = NSHostingView(rootView: Host(model: model))
            host.frame = NSRect(x: 0, y: 0, width: 700, height: 600)
            window = nil
        }
        await settleLayout(host)
    }

    var text: ComposerTextView { TextHarness.all(ComposerTextView.self, in: host)[0] }
    var scroll: NSScrollView { text.enclosingScrollView! }
    var editorHeight: CGFloat { scroll.frame.height }
    /// The editor's bottom edge in window coordinates (y up).
    var editorY: CGFloat { scroll.convert(scroll.bounds, to: nil).minY }

    func setDraft(_ draft: String) async {
        model.draft = draft
        await settleLayout(host)
    }

    /// Return and friends, as events to the text view.
    func key(_ code: UInt16, _ characters: String, _ modifiers: NSEvent.ModifierFlags = []) {
        let event = NSEvent.keyEvent(
            with: .keyDown, location: .zero, modifierFlags: modifiers, timestamp: ProcessInfo.processInfo.systemUptime,
            windowNumber: window?.windowNumber ?? 0, context: nil, characters: characters, charactersIgnoringModifiers: characters,
            isARepeat: false, keyCode: code)!
        text.keyDown(with: event)
    }

    func returnKey(_ modifiers: NSEvent.ModifierFlags = []) { key(36, "\r", modifiers) }

    /// Typed into the text view, so the binding follows through its delegate.
    func type(_ string: String) {
        text.insertText(string, replacementRange: NSRange(location: NSNotFound, length: 0))
    }
}

private func lines(_ n: Int) -> String { (1...n).map { "line \($0)" }.joined(separator: "\n") }

// MARK: Growth

@MainActor @Test func theEditorIsTodaysSingleLineHeightWhenEmptyOrOneLine() async {
    let rig = await Rig(windowHeight: 800)
    #expect(rig.editorHeight == DesignTokens.Size.composerRow)
    await rig.setDraft("hello")
    #expect(rig.editorHeight == DesignTokens.Size.composerRow)
}

@MainActor @Test func theEditorGrowsByOneLineEachLineAndShrinksBack() async {
    let rig = await Rig(windowHeight: 1200)
    var heights: [CGFloat] = []
    for n in 1...6 {
        await rig.setDraft(lines(n))
        heights.append(rig.editorHeight)
    }
    #expect(heights[0] == DesignTokens.Size.composerRow)
    let steps = zip(heights.dropFirst(), heights).map { $0 - $1 }
    // One 13 pt line is ~16 pt; rounding up to whole points moves a step by at most a point.
    #expect(steps.allSatisfy { $0 >= 14 && $0 <= 18 }, "\(heights)")
    #expect(heights == heights.sorted())
    await rig.setDraft("")
    #expect(rig.editorHeight == DesignTokens.Size.composerRow)
    // A trailing newline is a line already: the caret sits on it.
    await rig.setDraft("a\n")
    #expect(rig.editorHeight == heights[1])
}

@MainActor @Test func theHeightStopsAtHalfTheWindowWhenThatIsLessThan320() async {
    let rig = await Rig(windowHeight: 400)
    await rig.setDraft(lines(60))
    #expect(rig.editorHeight == 200)
    await rig.setDraft(lines(3))
    #expect(rig.editorHeight < 200)
    await rig.setDraft("")
    #expect(rig.editorHeight == DesignTokens.Size.composerRow)
}

@MainActor @Test func theHeightStopsAt320InATallWindow() async {
    let rig = await Rig(windowHeight: 1200)
    await rig.setDraft(lines(60))
    #expect(rig.editorHeight == 320)
    await rig.setDraft(lines(2))
    #expect(rig.editorHeight < 320)
}

@MainActor @Test func theHeightLimitIs160WithoutAWindow() async {
    let rig = await Rig(windowHeight: nil)
    await rig.setDraft(lines(60))
    #expect(rig.editorHeight == 160)
}

@Test func theHeightRuleClampsBetweenOneLineAndTheLimit() {
    #expect(ComposerRules.heightLimit(windowSafeHeight: 400) == 200)
    #expect(ComposerRules.heightLimit(windowSafeHeight: 900) == 320)
    #expect(ComposerRules.heightLimit(windowSafeHeight: 640) == 320)
    #expect(ComposerRules.heightLimit(windowSafeHeight: nil) == 160)
    #expect(ComposerRules.height(contentMaxY: 0, limit: 320) == DesignTokens.Size.composerRow)
    let insets = 2 * DesignTokens.Size.composerInputInset
    #expect(ComposerRules.height(contentMaxY: 100.2, limit: 320) == 101 + insets)
    #expect(ComposerRules.height(contentMaxY: 100.006, limit: 320) == 100 + insets, "layout noise of 0.006 adds no point")
    #expect(ComposerRules.height(contentMaxY: 1000, limit: 200) == 200)
    // A window too small for one line still gets one line.
    #expect(ComposerRules.height(contentMaxY: 1000, limit: 10) == DesignTokens.Size.composerRow)
}

@MainActor @Test func pastTheLimitTheEditorScrollsAndKeepsTheCaretInView() async {
    let rig = await Rig(windowHeight: 400)
    await rig.setDraft(lines(60))
    let text = rig.text
    text.setSelectedRange(NSRange(location: (text.string as NSString).length, length: 0))
    await settleLayout(rig.host)
    let layout = text.textLayoutManager!
    var caretMaxY: CGFloat = 0
    layout.enumerateTextLayoutFragments(from: layout.documentRange.endLocation, options: [.reverse, .ensuresLayout]) {
        caretMaxY = $0.layoutFragmentFrame.maxY
        return false
    }
    #expect(rig.scroll.contentView.bounds.maxY >= caretMaxY - 1, "visible to \(rig.scroll.contentView.bounds.maxY), caret at \(caretMaxY)")
    #expect(rig.scroll.scrollerStyle == .overlay && rig.scroll.hasVerticalScroller)
}

@MainActor @Test func theEditorIsPlainTextOnTextKit2() async {
    let rig = await Rig(windowHeight: 800)
    let text = rig.text
    #expect(text.textLayoutManager != nil)
    #expect(!text.isRichText)
    #expect(!text.isAutomaticQuoteSubstitutionEnabled && !text.isAutomaticDashSubstitutionEnabled)
    #expect(!text.isAutomaticLinkDetectionEnabled && !text.isAutomaticTextReplacementEnabled)
    #expect(text.isContinuousSpellCheckingEnabled)
    #expect(text.textContainer?.lineFragmentPadding == 0)
    #expect(text.font == DesignTokens.Typography.composerAppKit)
    #expect(text.accessibilityLabel() == "Message")
}

@MainActor @Test func theDraftBindingFollowsTheEditorBothWays() async {
    let rig = await Rig(windowHeight: 800)
    rig.type("typed")
    #expect(rig.model.draft == "typed")
    await rig.setDraft("from outside")
    #expect(rig.text.string == "from outside")
    await rig.setDraft("")
    #expect(rig.text.string.isEmpty)
}

// MARK: Keys

@MainActor @Test func returnSendsTheDraftAndClearsIt() async {
    let rig = await Rig(windowHeight: 800)
    rig.model.reply = reply
    await settleLayout(rig.host)
    rig.type("hello")
    rig.returnKey()
    #expect(rig.model.sent.count == 1 && rig.model.sent[0].0 == "hello")
    #expect(rig.model.sent[0].1 == ReplyTarget(messageID: "30", channelID: "1"))
    await settleLayout(rig.host)
    #expect(rig.model.draft.isEmpty && rig.text.string.isEmpty && rig.model.reply == nil)
}

@MainActor @Test func returnNeverSendsABlankDraft() async {
    let rig = await Rig(windowHeight: 800)
    rig.returnKey()
    rig.type("  \n ")
    rig.returnKey()
    #expect(rig.model.sent.isEmpty)
}

@MainActor @Test func optionReturnSendsToo() async {
    let rig = await Rig(windowHeight: 800)
    rig.type("hi")
    rig.returnKey(.option)
    #expect(rig.model.sent.map(\.0) == ["hi"])
}

@MainActor @Test func shiftReturnInsertsANewlineAndSendsNothing() async {
    let rig = await Rig(windowHeight: 800)
    rig.type("a")
    rig.returnKey(.shift)
    rig.type("b")
    #expect(rig.model.sent.isEmpty)
    #expect(rig.model.draft == "a\nb")
}

@MainActor @Test func returnInsideAnOpenCodeFenceInsertsANewline() async {
    let rig = await Rig(windowHeight: 800)
    rig.type("```swift")
    rig.returnKey()
    rig.type("let x = 1")
    rig.returnKey()
    #expect(rig.model.sent.isEmpty)
    #expect(rig.model.draft == "```swift\nlet x = 1\n")
    // Closing the fence makes Return send again.
    rig.type("```")
    rig.returnKey()
    #expect(rig.model.sent.map(\.0) == ["```swift\nlet x = 1\n```"])
}

@Test func theFenceRuleCountsFencesBeforeTheCaret() {
    let text = "a ```b``` c ```d"
    #expect(!ComposerRules.hasOpenCodeFence(in: text, caret: 0))
    #expect(ComposerRules.hasOpenCodeFence(in: text, caret: 6))
    #expect(!ComposerRules.hasOpenCodeFence(in: text, caret: 10))
    #expect(ComposerRules.hasOpenCodeFence(in: text, caret: text.utf16.count))
    // The caret before an open fence in the middle of the text: closed there.
    #expect(!ComposerRules.hasOpenCodeFence(in: "x ``` open", caret: 1))
    #expect(!ComposerRules.hasOpenCodeFence(in: "", caret: 5))
    #expect(ComposerRules.returnAction([], text: "```", caret: 3) == .newline)
    #expect(ComposerRules.returnAction([.shift], text: "x", caret: 1) == .newline)
    #expect(ComposerRules.returnAction([.option], text: "```", caret: 3) == .send)
    #expect(ComposerRules.returnAction([.command], text: "x", caret: 1) == .system)
}

@MainActor @Test func returnDuringMarkedTextSendsNothing() async {
    let rig = await Rig(windowHeight: 800)
    rig.text.setMarkedText("ni", selectedRange: NSRange(location: 2, length: 0), replacementRange: NSRange(location: NSNotFound, length: 0))
    #expect(rig.text.hasMarkedText())
    rig.returnKey()
    #expect(rig.model.sent.isEmpty)
}

@MainActor @Test func escapeCancelsTheReplyOnlyWhileOneIsSet() async {
    let rig = await Rig(windowHeight: 800)
    rig.model.reply = reply
    await settleLayout(rig.host)
    rig.type("keep me")
    rig.key(53, "\u{1b}")
    #expect(rig.model.reply == nil)
    #expect(rig.model.draft == "keep me")
    // No reply: Esc is the text view's own, and changes nothing here.
    rig.key(53, "\u{1b}")
    #expect(rig.model.draft == "keep me" && rig.model.sent.isEmpty)
}

// MARK: Long text

@Test func theLimitIs4000OnlyForNitroAndUnknownCountsAs2000() {
    #expect(MessageLength.limit(premiumType: 2) == 4000)
    for tier: UInt8? in [nil, 0, 1, 3] { #expect(MessageLength.limit(premiumType: tier) == 2000, "\(String(describing: tier))") }
}

private func counter(_ length: Int, _ premium: UInt8?) -> MessageLength.Counter? {
    MessageLength.counter(draft: String(repeating: "a", count: length), premiumType: premium)
}

@Test func theCounterShowsAtATenthOfTheLimitRemaining() {
    #expect(counter(1799, nil) == nil)
    #expect(counter(1800, nil)?.text == "200 characters remaining")
    #expect(counter(1800, 0)?.text == "200 characters remaining")
    #expect(counter(2000, 2) == nil && counter(1, 2) == nil)
    #expect(counter(3600, 2)?.text == "400 characters remaining")
}

@Test func aNitroUserSeesTheCounterOnceTheDraftPasses2000() {
    #expect(counter(2001, 2)?.text == "1999 characters remaining")
    #expect(counter(3599, 2)?.text == "401 characters remaining")
}

@Test func theCounterHasThreeTextsAndNeverANegativeNumber() {
    #expect(counter(1999, nil)?.text == "1 characters remaining")
    #expect(counter(2000, nil)?.text == "Message is at max length")
    #expect(counter(2001, nil)?.text == "Message is too long")
    #expect(counter(4000, 2)?.text == "Message is at max length")
    #expect(counter(4500, 2)?.text == "Message is too long")
    for length in [2001, 2500, 9000] { #expect(counter(length, nil)?.text.contains("-") == false) }
}

@Test func theCounterIsCriticalOnlyBelowZero() {
    #expect(counter(1999, nil)?.isCritical == false)
    #expect(counter(2000, nil)?.isCritical == false)
    #expect(counter(2001, nil)?.isCritical == true)
    #expect(counter(4001, 2)?.isCritical == true)
    #expect(DesignTokens.Palette.counterCritical != DesignTokens.Palette.counterDefault)
}

@Test func theInlineUpsellIsForUsersWithoutNitroOnly() {
    #expect(counter(1900, nil)?.showsUpsell == true)
    #expect(counter(1900, 1)?.showsUpsell == true)
    #expect(counter(3900, 2)?.showsUpsell == false)
    #expect(counter(2500, 2)?.showsUpsell == false)
}

@Test func lengthIsCountedInUTF16UnitsOfTheDraftAsSent() {
    #expect(MessageLength.length("👍") == 2)
    #expect(MessageLength.length("é") == 1 && MessageLength.length("e\u{301}") == 2)
    let thumbs = String(repeating: "👍", count: 1001)
    #expect(thumbs.count == 1001 && MessageLength.length(thumbs) == 2002)
    let over = MessageLength.counter(draft: thumbs, premiumType: nil)
    #expect(over?.remaining == -2 && over?.text == "Message is too long" && over?.isCritical == true)
    #expect(MessageLength.counter(draft: String(repeating: "👍", count: 1000), premiumType: nil)?.text == "Message is at max length")
}

@MainActor @Test func aDraftOf1001ThumbsIsTooLongAndReturnSendsNothing() async {
    let rig = await Rig(windowHeight: 800)
    let thumbs = String(repeating: "👍", count: 1001)
    await rig.setDraft(thumbs)
    rig.returnKey()
    #expect(rig.model.sent.isEmpty)
    #expect(rig.model.draft == thumbs, "never truncated")
    var draft = thumbs, none: ReplyPreview?
    #expect(ComposerSubmit.submit(&draft, reply: &none, hasMarkedText: false, limit: 2000) { _, _ in } == .tooLong(length: 2002, limit: 2000))
}

@MainActor @Test func aDraftOf1000ThumbsIsAtTheLimitAndSends() async {
    let rig = await Rig(windowHeight: 800)
    let thumbs = String(repeating: "👍", count: 1000)
    await rig.setDraft(thumbs)
    rig.returnKey()
    #expect(rig.model.sent.map(\.0) == [thumbs])
}

@MainActor @Test func aNitroUserSendsUpTo4000UnitsAndNoMoreAndUnknownUses2000() async {
    let long = String(repeating: "a", count: 3000)
    let nitro = await Rig(windowHeight: 800, premium: 2)
    await nitro.setDraft(long)
    nitro.returnKey()
    #expect(nitro.model.sent.map(\.0) == [long])
    let unknown = await Rig(windowHeight: 800, premium: nil)
    await unknown.setDraft(long)
    unknown.returnKey()
    #expect(unknown.model.sent.isEmpty && unknown.model.draft == long)
    let over = String(repeating: "a", count: 4001)
    await nitro.setDraft(over)
    nitro.returnKey()
    #expect(nitro.model.sent.count == 1 && nitro.model.draft == over)
}

@Test func theOverTheLimitReturnShowsTheAlertForNitroAndTheUpsellOtherwise() {
    let alert = MessageLength.tooLong(length: 4100, limit: 4000, premiumType: 2)
    #expect(alert == .alert(
        title: "Your message is too long.",
        message: "Please make your message of 4100 characters shorter. We've set the limit at 4000 characters to be courteous to others."))
    for tier: UInt8? in [nil, 0, 1, 3] {
        #expect(MessageLength.tooLong(length: 2100, limit: 2000, premiumType: tier) == .upsell(
            title: "Your message is too long.",
            message: "Lift off with Discord Nitro to type to your heart's content, up to 4000 characters."), "\(String(describing: tier))")
    }
    #expect(MessageLength.nitroURL.absoluteString == "https://discord.com/nitro")
}

@Test func anOverLimitReturnKeepsTheReplyAndTheDraft() {
    var draft = String(repeating: "a", count: 2001)
    var preview: ReplyPreview? = reply
    var sent = 0
    let outcome = ComposerSubmit.submit(&draft, reply: &preview, hasMarkedText: false, limit: 2000) { _, _ in sent += 1 }
    #expect(outcome == .tooLong(length: 2001, limit: 2000) && sent == 0 && draft.count == 2001 && preview != nil)
}

@MainActor @Test func theCounterReservesRoomInTheInputWhileItShows() async {
    let rig = await Rig(windowHeight: 800)
    await rig.setDraft(String(repeating: "a", count: 10))
    let plain = rig.host.fittingSize.height
    await rig.setDraft(String(repeating: "a", count: 1900))
    // 1900 characters wrap to several lines; compare with the same text just under the threshold.
    let shown = rig.host.fittingSize.height
    await rig.setDraft(String(repeating: "a", count: 1700))
    let hidden = rig.host.fittingSize.height
    #expect(plain <= hidden)
    #expect(shown - hidden >= 2 * DesignTokens.Size.composerCounterLine, "upsell line included: \(shown) vs \(hidden)")
}

// MARK: Transitions

@MainActor @Test func aChangeOfChannelClearsTheReplyAnimationAndALoneReplyChangeKeepsIt() async {
    // The editor's frame moves as the card shrinks; mid-animation it is between its two ends.
    func movement(switchChannel: Bool) async -> (before: CGFloat, soon: CGFloat, after: CGFloat) {
        let rig = await Rig(windowHeight: 800)
        rig.model.reply = reply
        await settleLayout(rig.host)
        let before = rig.editorY
        if switchChannel { rig.model.channel = "2" }
        rig.model.reply = nil
        rig.host.layoutSubtreeIfNeeded()
        try? await Task.sleep(for: .milliseconds(30))
        rig.host.layoutSubtreeIfNeeded()
        let soon = rig.editorY
        await settleLayout(rig.host)
        return (before, soon, rig.editorY)
    }
    let animated = await movement(switchChannel: false)
    #expect(animated.before != animated.after)
    #expect(animated.soon != animated.after, "the spring is still running: \(animated)")
    let cleared = await movement(switchChannel: true)
    #expect(cleared.soon == cleared.after, "no animation after a channel change: \(cleared)")
}

@MainActor @Test func typingGrowthIsNotAnimated() async {
    let rig = await Rig(windowHeight: 1200)
    rig.model.draft = lines(5)
    rig.host.layoutSubtreeIfNeeded()
    let soon = rig.editorHeight
    await settleLayout(rig.host)
    #expect(soon == rig.editorHeight && soon > DesignTokens.Size.composerRow)
}

@Test func theSpringIsSwiftcordsAndNamedInTheTokens() {
    #expect(DesignTokens.Motion.composer == Animation.spring(response: 0.15, dampingFraction: 0.85, blendDuration: 0.25))
}

// MARK: The message list (D8)

@MainActor @Observable private final class ListModel {
    var messages: [MessageItem]
    var draft = ""
    init(_ messages: [MessageItem]) { self.messages = messages }
}

private struct ListHost: View {
    let model: ListModel
    var body: some View {
        MessageArea(
            header: ChannelHeader(name: "general"), conversationID: "1", messages: model.messages, load: .loaded,
            reply: .constant(nil), draft: Binding(get: { model.draft }, set: { model.draft = $0 })
        )
    }
}

private func item(_ n: Int) -> MessageItem {
    let base = Date(timeIntervalSince1970: 1_800_000_000)
    return MessageItem(
        id: String(n), authorId: "a\(n)", authorName: "author \(n)", avatar: .initials("A"),
        timestamp: base.addingTimeInterval(Double(n) * 60), text: "message \(n) " + String(repeating: "word ", count: (n * 7) % 40)
    )
}

@MainActor private func send(_ object: NSObject, _ name: String) -> Any? {
    let selector = NSSelectorFromString(name)
    return object.responds(to: selector) ? object.perform(selector)?.takeUnretainedValue() : nil
}

@MainActor private func label(_ object: NSObject) -> String {
    [send(object, "accessibilityLabel"), send(object, "accessibilityValue")].compactMap { $0 as? String }.first { !$0.isEmpty } ?? ""
}

@MainActor private func walk(_ element: Any, _ visit: (NSObject) -> Void) {
    guard let object = element as? NSObject else { return }
    visit(object)
    for child in (send(object, "accessibilityChildren") as? [Any]) ?? [] { walk(child, visit) }
}

@MainActor private final class ListRig {
    let model = ListModel((100...160).map(item))
    let window: NSWindow
    let host: NSHostingView<ListHost>

    init() async {
        window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 700, height: 500), styleMask: .borderless, backing: .buffered, defer: false)
        host = NSHostingView(rootView: ListHost(model: model))
        window.contentView = host
        NSApplication.shared.accessibilitySetValue(true, forAttribute: NSAccessibility.Attribute(rawValue: "AXEnhancedUserInterface"))
        await settleLayout(host)
    }

    var editor: NSScrollView { TextHarness.all(ComposerTextView.self, in: host)[0].enclosingScrollView! }
    /// The top edge of the editor in the same (screen, y up) space as the accessibility frames.
    var editorTop: CGFloat { window.convertToScreen(editor.convert(editor.bounds, to: nil)).maxY }
    var list: NSScrollView {
        func find(_ view: NSView) -> NSScrollView? { (view as? NSScrollView).flatMap { $0 is ComposerScrollView ? nil : $0 } ?? view.subviews.lazy.compactMap(find).first }
        return find(host)!
    }

    func settle() async { await settleLayout(host) }

    /// Wheel events, as a user's, until the list's offset reaches `target` (0 is the top).
    func scroll(toOffset target: CGFloat) async {
        for _ in 0..<600 where list.contentView.bounds.origin.y > target {
            let dy = Int32(min(120, max(1, list.contentView.bounds.origin.y - target)))
            let cg = CGEvent(scrollWheelEvent2Source: nil, units: .pixel, wheelCount: 1, wheel1: dy, wheel2: 0, wheel3: 0)!
            list.scrollWheel(with: NSEvent(cgEvent: cg)!)
            host.layoutSubtreeIfNeeded()
            try? await Task.sleep(for: .milliseconds(5))
        }
        await settle()
    }

    /// Screen frame of every message text on screen, by message number.
    func frames() -> [Int: NSRect] {
        var out: [Int: NSRect] = [:]
        walk(host) { object in
            let text = label(object)
            guard text.hasPrefix("message "), let n = Int(text.dropFirst("message ".count).prefix { $0.isNumber }),
                  let frame = (object.value(forKey: "accessibilityFrame") as? NSValue)?.rectValue, frame.minY > 0, frame.maxY < 2000 else { return }
            out[n] = frame
        }
        return out
    }
}

private func sameY(_ a: NSRect?, _ b: NSRect?) -> Bool {
    guard let a, let b else { return false }
    return abs(a.minY - b.minY) <= 1
}

@MainActor @Test func aListAtItsBottomKeepsItsLastMessageAboveAGrowingComposer() async {
    let rig = await ListRig()
    let before = rig.frames()
    let top0 = rig.editorTop
    #expect(before[160] != nil, "the last message is on screen: \(before.keys.sorted())")
    #expect(before[160].map { $0.minY >= top0 - 0.5 } == true, "last message \(String(describing: before[160])) above the editor top \(top0)")
    rig.model.draft = lines(60)
    await rig.settle()
    #expect(rig.editor.frame.height == 250)
    let grown = rig.frames()
    #expect(grown[160] != nil, "still on screen after growth: \(grown.keys.sorted())")
    #expect(grown[160].map { $0.minY >= rig.editorTop - 0.5 } == true, "last message \(String(describing: grown[160])) above the editor top \(rig.editorTop)")
    rig.model.draft = ""
    await rig.settle()
    let after = rig.frames()
    #expect(after[160] != nil && after[160]!.minY >= rig.editorTop - 0.5, "after shrinking: \(String(describing: after[160])) vs \(rig.editorTop)")
    // And it came back to where it was.
    #expect(sameY(after[160], before[160]), "\(String(describing: after[160])) vs \(String(describing: before[160]))")
}

@MainActor @Test func aListScrolledUpKeepsItsTopRowWhereItWasWhileTheComposerGrowsAndShrinks() async {
    let rig = await ListRig()
    await rig.scroll(toOffset: 1000)
    let before = rig.frames()
    #expect(before.count >= 3 && before[160] == nil, "scrolled away from the bottom: \(before.keys.sorted())")
    let topRow = before.max { $0.value.maxY < $1.value.maxY }!.key
    for draft in [lines(60), ""] {
        rig.model.draft = draft
        await rig.settle()
        let now = rig.frames()
        #expect(sameY(now[topRow], before[topRow]),
                "top row \(topRow) moved: \(String(describing: now[topRow])) vs \(String(describing: before[topRow])), draft \(draft.count)")
    }
}

// MARK: What the over-limit Return presents

/// Every string a window's views show, for telling the alert from the sheet.
@MainActor private func strings(in view: NSView) -> [String] {
    let own = [(view as? NSTextField)?.stringValue, (view as? NSButton)?.title].compactMap { $0 }
    return own + view.subviews.flatMap(strings)
}

@MainActor @Test func anOverLimitReturnPresentsTheAlertForNitroAndTheSheetOtherwise() async throws {
    for premium in [UInt8(2), nil] {
        let rig = await Rig(windowHeight: 800, premium: premium)
        await rig.setDraft(String(repeating: "a", count: premium == 2 ? 4001 : 2001))
        rig.returnKey()
        for _ in 0..<20 where rig.window?.attachedSheet == nil { await settleLayout(rig.host) }
        let sheet = try #require(rig.window?.attachedSheet, "premium \(String(describing: premium))")
        await settleLayout(sheet.contentView!)
        let shown = strings(in: sheet.contentView!)
        #expect(shown.contains("Okay"), "\(shown)")
        #expect(shown.contains("Learn more") == (premium == nil), "\(shown)")
        #expect(shown.contains { $0.hasPrefix("Please make your message of 4001 characters shorter") } == (premium == 2), "\(shown)")
        #expect(shown.contains("Lift off with Discord Nitro to type to your heart's content, up to 4000 characters.") == (premium == nil), "\(shown)")
        #expect(rig.model.sent.isEmpty && rig.model.draft.count == (premium == 2 ? 4001 : 2001))
        // "Learn more" opens the Nitro page through the environment's openURL, and closes the sheet.
        if premium == nil {
            func button(_ view: NSView) -> NSButton? { (view as? NSButton).flatMap { $0.title == "Learn more" ? $0 : nil } ?? view.subviews.lazy.compactMap(button).first }
            try #require(button(sheet.contentView!)).performClick(nil)
            #expect(rig.model.opened == [MessageLength.nitroURL])
        } else {
            rig.window?.endSheet(sheet)
        }
    }
}
