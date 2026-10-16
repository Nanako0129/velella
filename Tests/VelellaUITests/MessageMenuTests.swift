import AppKit
import SwiftUI
import Testing
@testable import VelellaUI

// Selection plan C1: the message menu. Its rules are pure (`MessageMenu`, `ConversationIDs`,
// `ReplyTarget`); the presenter runs offscreen: its panel is never ordered in (`show` does
// nothing), copies go to a private pasteboard, and `NSPasteboard.general` is never written.

private let guildA = ConversationIDs(channelID: "200", guildID: "100")
private let dm = ConversationIDs(channelID: "200", guildID: nil)

private func row(_ id: String = "300", text: String = "hello", delivery: MessageItem.Delivery = .sent) -> MessageItem {
    MessageItem(id: id, authorName: "Ada", avatar: .initials("A"), timestamp: Date(timeIntervalSince1970: 0), text: text, delivery: delivery)
}

private func sections(_ message: MessageItem, _ conversation: ConversationIDs?, preview: String? = "p") -> [[MessageMenuItem]] {
    MessageMenu.sections(MessageMenuContext(message: message, conversation: conversation), replyPreview: preview)
}

private func titles(_ sections: [[MessageMenuItem]]) -> [[String]] {
    sections.map { $0.map(\.title) }
}

private func link(_ sections: [[MessageMenuItem]]) -> String? {
    sections.joined().lazy.compactMap { if case .copyMessageLink(let s) = $0 { s } else { nil } }.first
}

private func copiedID(_ sections: [[MessageMenuItem]]) -> String? {
    sections.joined().lazy.compactMap { if case .copyMessageID(let s) = $0 { s } else { nil } }.first
}

// MARK: Items and gating

@Test func aSentRowGetsReplyCopiesAndTheIDInDiscordsSections() {
    #expect(titles(sections(row(), guildA)) == [["Reply"], ["Copy Text", "Copy Message Link"], ["Copy Message ID"]])
    #expect(link(sections(row(), guildA)) == "https://discord.com/channels/100/200/300")
    #expect(copiedID(sections(row(), guildA)) == "300")
}

@Test func aPendingRowWithANumericIDGetsNoReplyLinkOrID() {
    // An echo's id is its nonce, numeric like a snowflake (S-11): delivery decides, not the id.
    for delivery in [MessageItem.Delivery.pending, .failed("Not sent")] {
        #expect(titles(sections(row("1234567890123456789", delivery: delivery), guildA)) == [["Copy Text"]])
    }
}

@Test func aDMLinksThroughAtMe() {
    #expect(link(sections(row(), dm)) == "https://discord.com/channels/@me/200/300")
}

@Test func anEmptyRowHasNoCopyText() {
    #expect(titles(sections(row(text: ""), guildA)) == [["Reply"], ["Copy Message Link"], ["Copy Message ID"]])
}

@Test func idsThatAreNotUInt64HideTheLinkAndIDItems() {
    let bad = ["", "abc", "-1", "18446744073709551616", "3 00", "１２３", "0x1F"]
    for id in bad {
        // The message's, the channel's, the guild's: any one failing hides both items.
        for (message, conversation) in [
            (row(id), guildA),
            (row(), ConversationIDs(channelID: id, guildID: "100")),
            (row(), ConversationIDs(channelID: "200", guildID: id)),
        ] {
            let items = titles(sections(message, conversation)).joined()
            #expect(!items.contains("Copy Message Link") && !items.contains("Copy Message ID"), "\(id)")
        }
    }
    // A failing guild id never falls back to a DM link.
    #expect(MessageMenu.messageLink(messageID: "300", channelID: "200", guildID: "x") == nil)
    // No conversation (fixtures): no link, no ID.
    #expect(titles(sections(row(), nil)) == [["Reply"], ["Copy Text"]])
    // What passes `UInt64(_:)` is written re-formatted.
    #expect(link(sections(row("+300"), ConversationIDs(channelID: "+200", guildID: "+100"))) == "https://discord.com/channels/100/200/300")
    #expect(copiedID(sections(row("+300"), guildA)) == "300")
}

@Test func everyMessageLinkMatchesDiscordsForm() throws {
    let form = try Regex("^https://discord\\.com/channels/(@me|[0-9]+)/[0-9]+/[0-9]+$")
    let ids = ["0", "1", "+7", "18446744073709551615", "1559972879481700373"]
    for m in ids {
        for c in ids {
            for g in ids.map(Optional.some) + [nil] {
                let s = try #require(MessageMenu.messageLink(messageID: m, channelID: c, guildID: g))
                #expect(s.wholeMatch(of: form) != nil, "\(s)")
            }
        }
    }
}

@Test func theConversationIDsComeFromTheEngineIDs() {
    #expect(ConversationIDs(channel: 200, guild: 100) == guildA)
    #expect(ConversationIDs(channel: 200, guild: nil) == dm)
    #expect(ConversationIDs(channel: nil, guild: 100) == nil)
    #expect(ConversationIDs(channel: .max, guild: nil)?.channelID == "18446744073709551615")
}

@Test func theLinkUsesTheRowsConversationAfterTheSelectionMoves() {
    // Rows of guild A; the selection then moves to guild B before any snapshot of B arrives.
    var state = ShellState.demo
    state.messages = [row()]
    state.conversation = guildA
    state.selectedRailID = "999"
    state.selectedChannelID = "888"
    let context = MessageMenuContext(message: state.messages[0], conversation: state.conversation)
    #expect(link(MessageMenu.sections(context, replyPreview: nil)) == "https://discord.com/channels/100/200/300")
}

@Test func copyLinkWritesTheURLAndNeverTheLabel() {
    // W1 supplies the context link; the item is C1's: the captured canonical URL.
    let body = MessageBody(blocks: [.paragraph([.link(id: 0, url: "https://b.com/", host: "b.com", label: [.text("https://a.com")], masked: true)])])
    let router = LinkRouter(body: body, reveal: { _ in }, confirm: { _ in }, open: { _ in })
    let context = MessageMenuContext(message: row(), conversation: guildA, link: (index: 0, target: body.links[0], router: router, liveBody: { body }))
    let items = MessageMenu.sections(context, replyPreview: nil)
    #expect(titles(items) == [["Reply"], ["Copy Text", "Copy Message Link"], ["Copy Link", "Open Link"], ["Copy Message ID"]])
    #expect(items.joined().contains(.copyLink("https://b.com/")))
    // With a selection, Copy ⌘C leads (e0-menu-selection.png).
    let selecting = MessageMenuContext(message: row(), conversation: guildA, copySelection: {})
    #expect(titles(MessageMenu.sections(selecting, replyPreview: nil)).first == ["Copy"])
}

// MARK: Reply

@Test func aTargetFromAnotherChannelIsDropped() {
    let target = ReplyTarget(messageID: "300", channelID: "200")
    #expect(ReplyTarget.replyID(target, sendingTo: 200) == 300)
    #expect(ReplyTarget.replyID(target, sendingTo: 201) == nil)
    #expect(ReplyTarget.replyID(nil, sendingTo: 200) == nil)
    #expect(ReplyTarget.replyID(ReplyTarget(messageID: "x", channelID: "200"), sendingTo: 200) == nil)
    #expect(ReplyTarget.replyID(ReplyTarget(messageID: "300", channelID: ""), sendingTo: 0) == nil)
}

// MARK: Presenter

private func board() -> NSPasteboard {
    NSPasteboard(name: NSPasteboard.Name("velella-c1-test-\(UUID().uuidString)"))
}

@MainActor private final class Recorder {
    var replies: [ReplyPreview] = []
    var sends = 0
    var draft = "a draft"
}

@MainActor private func presenter(_ pasteboard: NSPasteboard) -> MessageMenuPresenter {
    MessageMenuPresenter(pasteboard: pasteboard, show: { _ in })
}

@MainActor private func present(
    _ presenter: MessageMenuPresenter, _ message: MessageItem, _ conversation: ConversationIDs? = guildA,
    preview: String? = "preview line", from window: NSWindow? = nil, recorder: Recorder = Recorder()
) {
    presenter.present(
        MessageMenuContext(message: message, conversation: conversation), body: message.body, conversation: conversation,
        replyPreview: preview, at: NSPoint(x: 100, y: 100), from: window
    ) { recorder.replies.append($0) }
}

private func key(_ type: NSEvent.EventType, _ code: UInt16, _ characters: String, repeat isARepeat: Bool = false, window: NSWindow) -> NSEvent {
    NSEvent.keyEvent(
        with: type, location: .zero, modifierFlags: [], timestamp: ProcessInfo.processInfo.systemUptime,
        windowNumber: window.windowNumber, context: nil, characters: characters, charactersIgnoringModifiers: characters,
        isARepeat: isARepeat, keyCode: code
    )!
}

private let returnKey: (UInt16, String) = (36, "\r"), downKey: (UInt16, String) = (125, "\u{F701}")

@MainActor @Test func copyTextWritesTheRawContentExactlyAsPlainText() throws {
    let general = NSPasteboard.general.changeCount
    let pasteboard = board()
    defer { pasteboard.releaseGlobally() }
    let raw = "**粗 ||交錯** 暴雷|| <t:99999999999999999:f> [看這裡](https://example.com)\r\n\u{1B}x\u{202C}\u{2068}"
    let menu = presenter(pasteboard)
    present(menu, row(text: raw))
    let item = try #require(menu.model.items.first { $0.title == "Copy Text" })
    menu.choose(item, key: nil)
    #expect(pasteboard.string(forType: .string).map { Data($0.utf8) } == Data(raw.utf8))
    // N1's type set (N0-12): the plain-text type and AppKit's alias, nothing else (D4).
    #expect(Set(pasteboard.types ?? []) == [.string, NSPasteboard.PasteboardType("NSStringPboardType")])
    #expect(!menu.isPresented)
    #expect(NSPasteboard.general.changeCount == general)
}

@MainActor @Test func replyTakesTheRowsIDAndThePreviewCapturedAtOpen() throws {
    let recorder = Recorder()
    let menu = presenter(board())
    present(menu, row("300"), preview: "a ▒▒▒ b", recorder: recorder)
    let item = try #require(menu.model.items.first { $0.title == "Reply" })
    menu.choose(item, key: nil)
    let reply = try #require(recorder.replies.first)
    #expect(reply.messageID == "300" && reply.channelID == "200" && reply.authorName == "Ada")
    #expect(reply.segments == [.text("a ▒▒▒ b")])
    #expect(reply.target == ReplyTarget(messageID: "300", channelID: "200"))
}

@MainActor @Test func theMenuIsDismissedWhenItsMessageChanges() {
    let menu = presenter(board())
    let message = row()
    var edited = message
    edited.body = MessageBody(text: "changed")
    var echo = message
    echo.delivery = .pending
    for (rows, conversation, open) in [
        ([message], guildA, true),
        ([MessageItem](), guildA, false),  // deleted, or sign-out emptied the rows
        ([row("301")], guildA, false),
        ([edited], guildA, false),
        ([echo], guildA, false),
        ([message], ConversationIDs(channelID: "201", guildID: "100"), false),  // channel switch
    ] {
        present(menu, message)
        #expect(menu.isPresented)
        menu.revalidate(messages: rows, conversation: conversation)
        #expect(menu.isPresented == open, "\(rows.map(\.id)) \(String(describing: conversation))")
        menu.dismiss()
    }
}

@MainActor @Test func theProductionPanelCanBecomeKey() {
    #expect(MessageMenuPanel().canBecomeKey)
    #expect(!MessageMenuPanel().canBecomeMain)
}

@MainActor @Test func onlyAnotherWindowBecomingKeyDismissesTheMenu() throws {
    let menu = presenter(board())
    present(menu, row())
    let panel = try #require(menu.panel)
    NotificationCenter.default.post(name: NSWindow.didBecomeKeyNotification, object: panel)
    #expect(menu.isPresented)
    let other = NSWindow(contentRect: .zero, styleMask: .borderless, backing: .buffered, defer: true)
    other.isReleasedWhenClosed = false
    NotificationCenter.default.post(name: NSWindow.didBecomeKeyNotification, object: other)
    #expect(!menu.isPresented)
}

/// S-7 / F-3: Return chosen on an item, held (two repeats), then released, with the composer
/// behind the menu focused and holding a draft. Each event goes to the window that is key at that
/// moment: the panel while the menu is open (it is key throughout, `canBecomeKey`), else the main
/// window. The last step proves the composer was armed: the same Return, once the menu is gone,
/// sends.
@MainActor @Test func keysChosenInTheMenuNeverReachTheComposer() async throws {
    let recorder = Recorder()
    let main = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 600, height: 120), styleMask: .borderless, backing: .buffered, defer: false)
    main.isReleasedWhenClosed = false
    let draft = Binding(get: { recorder.draft }, set: { recorder.draft = $0 })
    let reply = Binding<ReplyPreview?>(get: { nil }, set: { _ in })
    let host = NSHostingView(rootView: Composer(reply: reply, draft: draft) { _, _ in recorder.sends += 1 }.frame(width: 600))
    main.contentView = host
    host.layoutSubtreeIfNeeded()
    func fields(_ view: NSView) -> [NSTextField] { ((view as? NSTextField).map { [$0] } ?? []) + view.subviews.flatMap(fields) }
    let field = try #require(fields(host).first)
    #expect(main.makeFirstResponder(field))

    let pasteboard = board()
    defer { pasteboard.releaseGlobally() }
    let menu = presenter(pasteboard)
    present(menu, row(), from: main, recorder: recorder)
    func send(_ type: NSEvent.EventType, _ k: (UInt16, String), repeat isARepeat: Bool = false) {
        let target: NSWindow = menu.panel ?? main
        target.sendEvent(key(type, k.0, k.1, repeat: isARepeat, window: target))
    }
    send(.keyDown, downKey)
    send(.keyUp, downKey)
    #expect(menu.model.highlighted == 0)
    send(.keyDown, returnKey)
    #expect(recorder.replies.count == 1, "Reply ran on key-down")
    #expect(menu.isPresented, "still key while Return is held")
    send(.keyDown, returnKey, repeat: true)
    send(.keyDown, returnKey, repeat: true)
    #expect(menu.isPresented)
    send(.keyUp, returnKey)
    #expect(!menu.isPresented)
    try await Task.sleep(for: .milliseconds(50))
    #expect(recorder.sends == 0)
    #expect(recorder.draft == "a draft")

    send(.keyDown, returnKey)
    try await Task.sleep(for: .milliseconds(50))
    #expect(recorder.sends == 1, "the composer was armed")
    main.close()
}

@MainActor @Test func escapeIsSwallowedUntilItsKeyUpAndRunsNothing() throws {
    let pasteboard = board()
    defer { pasteboard.releaseGlobally() }
    let general = NSPasteboard.general.changeCount
    let menu = presenter(pasteboard)
    present(menu, row())
    let panel = try #require(menu.panel)
    panel.sendEvent(key(.keyDown, 53, "\u{1B}", window: panel))
    #expect(menu.isPresented && panel.alphaValue == 0)
    panel.sendEvent(key(.keyUp, 53, "\u{1B}", window: panel))
    #expect(!menu.isPresented)
    #expect(pasteboard.types?.isEmpty ?? true)
    #expect(NSPasteboard.general.changeCount == general)
}

@MainActor @Test func thePanelIsSizedToItsRows() {
    let model = MessageMenuModel()
    model.sections = sections(row(), guildA)
    let host = NSHostingView(rootView: MessageMenuView(model: model) { _ in })
    // D5: 3 sections of 4 rows: 8 + 4 × 36 + 2 × 17 + 8.
    #expect(MessageMenu.size(model.sections) == CGSize(width: 220, height: 194))
    #expect(host.fittingSize == MessageMenu.size(model.sections))
}

// MARK: Row trigger

/// A window that reports the event being delivered as its current one, as AppKit does for a real
/// event. An offscreen window drops mouse events given to `sendEvent` (measured in N0), so the
/// test hit-tests and delivers them itself.
private final class EventWindow: NSWindow {
    var delivering: NSEvent?
    override var currentEvent: NSEvent? { delivering ?? super.currentEvent }

    /// The view that took `event`, after delivering it the way AppKit would.
    @discardableResult
    func deliver(_ event: NSEvent) -> NSView? {
        delivering = event
        defer { delivering = nil }
        let hit = contentView?.hitTest(event.locationInWindow)
        switch event.type {
        case .rightMouseDown: hit?.rightMouseDown(with: event)
        case .leftMouseDown: hit?.mouseDown(with: event)
        default: break
        }
        return hit
    }
}

@MainActor @Test func aRightClickOnTheHeaderOrAnAttachmentOpensTheMenuWithTheIDItems() throws {
    var opened: [NSPoint] = []
    let message = MessageItem(
        id: "300", authorName: "Ada", avatar: .initials("A"), timestamp: Date(timeIntervalSince1970: 0), text: "",
        images: [ImageAttachment(seed: 1, width: 400, height: 300)]
    )
    let host = NSHostingView(rootView: MessageRow(message: message, startsGroup: true, afterDivider: true) { point, _ in
        opened.append(point)
    }.frame(width: 600))
    let height = host.fittingSize.height
    let window = EventWindow(contentRect: NSRect(x: 0, y: 0, width: 600, height: height), styleMask: .borderless, backing: .buffered, defer: false)
    window.isReleasedWhenClosed = false
    window.contentView = host
    host.layoutSubtreeIfNeeded()
    func mouse(_ type: NSEvent.EventType, x: CGFloat, fromTop y: CGFloat, flags: NSEvent.ModifierFlags = []) -> NSEvent {
        NSEvent.mouseEvent(
            with: type, location: NSPoint(x: x, y: height - y), modifierFlags: flags, timestamp: 0,
            windowNumber: window.windowNumber, context: nil, eventNumber: 0, clickCount: 1, pressure: 1
        )!
    }
    let M = DesignTokens.Message.self
    let header = M.rowPaddingVertical + M.headerLineHeight / 2
    let attachment = M.rowPaddingVertical + M.headerLineHeight + 40
    for (x, y) in [(M.contentInset + 10, header), (M.contentInset + 40, attachment), (30.0, header), (590.0, height - 1)] {
        let before = opened.count
        window.deliver(mouse(.rightMouseDown, x: x, fromTop: y))
        #expect(opened.count == before + 1, "right-click at \(x), \(y)")
        window.deliver(mouse(.leftMouseDown, x: x, fromTop: y, flags: .control))
        #expect(opened.count == before + 2, "Control-click at \(x), \(y)")
        // A plain click passes through to the row's own views.
        let hit = window.deliver(mouse(.leftMouseDown, x: x, fromTop: y))
        #expect(!(hit is MessageMenuTrigger.TriggerView) && opened.count == before + 2)
    }
    #expect(height > attachment)
    let items = titles(sections(message, guildA)).joined()
    #expect(items.contains("Copy Message ID") && items.contains("Copy Message Link") && !items.contains("Copy Text"))
    window.close()
}
