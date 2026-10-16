import AppKit
import Observation
import SwiftUI
import Testing
@testable import VelellaUI

// The message menu wired to the text views (docs/plans/selection.md, W1 Tests), against the real
// column (`MessageArea`) or a hosted `MarkdownView` in an offscreen window that is never ordered
// in. The presenter is injected: it never shows its panel and copies to a private board, and
// `NSPasteboard.general` is checked not to move. No link is ever opened outside the test: Open Link
// goes to a recording opener.

/// Hands each press to the view under it as AppKit does, with the press as the window's current
/// event (the row trigger reads it to decide whether to take the press). An offscreen window drops
/// mouse events given to `sendEvent` (N0), so delivery is by hit-testing.
private final class MenuWindow: NSWindow {
    var delivering: NSEvent?
    override var currentEvent: NSEvent? { delivering ?? super.currentEvent }
    override var canBecomeKey: Bool { true }
    override var isKeyWindow: Bool { true }

    @discardableResult
    func deliver(_ event: NSEvent) -> NSView? {
        delivering = event
        defer { delivering = nil }
        guard let content = contentView else { return nil }
        let hit = content.hitTest(content.superview?.convert(event.locationInWindow, from: nil) ?? event.locationInWindow)
        switch event.type {
        case .rightMouseDown: hit?.rightMouseDown(with: event)
        case .leftMouseDown: hit?.mouseDown(with: event)
        default: break
        }
        return hit
    }
}

@MainActor @Observable private final class Rows {
    var messages: [MessageItem]
    init(_ messages: [MessageItem]) { self.messages = messages }
}

private struct Column: View {
    let rows: Rows
    let board: NSPasteboard
    let presenter: MessageMenuPresenter
    var body: some View {
        MessageArea(
            header: ChannelHeader(name: "general"), conversationID: "1", messages: rows.messages, load: .loaded,
            reply: .constant(nil), draft: .constant(""), menuPresenter: presenter
        )
        .environment(\.textPasteboard, PasteboardBox(board: board))
    }
}

private let epoch = Date(timeIntervalSince1970: 1_735_732_800)

private func message(_ n: Int, text: String? = nil, body: MessageBody? = nil, embeds: [EmbedItem] = []) -> MessageItem {
    MessageItem(
        id: "m\(n)", authorId: "a\(n)", authorName: "author\(n)", avatar: .initials("A"),
        timestamp: epoch.addingTimeInterval(Double(n) * 3600), text: text ?? "message \(n) body", body: body, embeds: embeds
    )
}

private func link(_ url: String, host: String, label: [MarkdownInline]? = nil) -> MarkdownInline {
    .link(id: 0, url: url, host: host, label: label ?? [.text(url)], masked: label != nil)
}

@MainActor private func range(of key: NSAttributedString.Key, in view: MessageTextView) -> NSRange? {
    var found: NSRange?
    view.attributed.enumerateAttribute(key, in: NSRange(location: 0, length: view.length)) { value, r, stop in
        if value != nil { found = r; stop.pointee = true }
    }
    return found
}

@MainActor private func center(_ view: MessageTextView, _ range: NSRange) -> CGPoint {
    let r = view.rects(for: range)[0]
    return view.convert(CGPoint(x: r.midX, y: r.midY), to: nil)
}

private func hasLinkItems(_ items: [MessageMenuItem]) -> Bool {
    items.contains(.openLink) || items.contains { if case .copyLink = $0 { true } else { false } }
}

@MainActor private final class ColumnRig {
    let rows: Rows
    let board = TextHarness.board()
    let presenter: MessageMenuPresenter
    let window: MenuWindow
    let host: NSHostingView<Column>

    init(_ messages: [MessageItem]) async {
        rows = Rows(messages)
        presenter = MessageMenuPresenter(pasteboard: board, show: { _ in })
        window = MenuWindow(contentRect: NSRect(x: 0, y: 0, width: 700, height: 600), styleMask: .borderless, backing: .buffered, defer: false)
        window.isReleasedWhenClosed = false
        host = NSHostingView(rootView: Column(rows: rows, board: board, presenter: presenter))
        window.contentView = host
        for _ in 0..<6 {
            host.layoutSubtreeIfNeeded()
            try? await Task.sleep(for: .milliseconds(40))
        }
    }

    var views: [MessageTextView] { TextHarness.textViews(in: host) }
    func body(_ n: Int) -> MessageTextView? { views.first { $0.message == "m\(n)" && $0.participant == 0 } }

    func select(in view: MessageTextView, from start: Int, to end: Int) {
        let a = view.rects(for: NSRange(location: start, length: 1))[0], b = view.rects(for: NSRange(location: end, length: 1))[0]
        TextHarness.send(TextHarness.mouse(.leftMouseDown, at: CGPoint(x: a.minX + 0.5, y: a.midY), in: view), to: window)
        TextHarness.send(TextHarness.mouse(.leftMouseDragged, at: CGPoint(x: b.minX + 0.5, y: b.midY), in: view), to: window)
        TextHarness.send(TextHarness.mouse(.leftMouseUp, at: CGPoint(x: b.minX + 0.5, y: b.midY), in: view), to: window)
    }

    func press(_ type: NSEvent.EventType, at point: CGPoint, control: Bool = false) -> NSEvent {
        NSEvent.mouseEvent(
            with: type, location: point, modifierFlags: control ? .control : [], timestamp: ProcessInfo.processInfo.systemUptime,
            windowNumber: window.windowNumber, context: nil, eventNumber: 700, clickCount: 1, pressure: 1)!
    }

    /// A right-click at `point` (window coordinates): the view that took it, and the menu's items.
    func rightClick(at point: CGPoint) -> (hit: NSView?, items: [MessageMenuItem]) {
        presenter.dismiss()
        let hit = window.deliver(press(.rightMouseDown, at: point))
        return (hit, presenter.isPresented ? presenter.model.items : [])
    }
}

@MainActor @Suite(.serialized) struct MenuWiringTests {
    // MARK: Trigger and selection

    /// N2's monitor cleared the selection on any left press outside its text, Control-click included,
    /// so the menu's Copy never saw it. A Control-click and a right-click on the row padding keep the
    /// selection, and the row's menu offers Copy, which copies it.
    @Test func aControlClickOrARightClickOnTheRowPaddingKeepsTheSelectionForCopy() async throws {
        let general = NSPasteboard.general.changeCount
        let rig = await ColumnRig([message(1, text: "alpha bravo"), message(2, text: "delta echo")])
        defer { rig.presenter.dismiss(); rig.window.close() }
        let body = try #require(rig.body(1))
        let coordinator = body.selection
        rig.select(in: body, from: 0, to: 5)
        #expect(coordinator.hasSelection)
        // Left of the avatar, level with the body: the row's padding, no text there.
        let padding = CGPoint(x: 6, y: body.convert(body.bounds, to: nil).midY)
        for control in [true, false] {
            let event = rig.press(control ? .leftMouseDown : .rightMouseDown, at: padding, control: control)
            coordinator.handle(event)  // as the monitor sees the press first
            #expect(coordinator.hasSelection, "control: \(control)")
            rig.presenter.dismiss()
            let hit = rig.window.deliver(event)
            #expect(hit is MessageMenuTrigger.TriggerView, "control: \(control)")
            #expect(rig.presenter.context?.messageID == "m1")
            #expect(rig.presenter.model.items.first == .copySelection, "control: \(control)")
            rig.board.clearContents()
            rig.presenter.choose(.copySelection, key: nil)
            #expect(rig.board.string(forType: .string) == "alpha", "control: \(control)")
        }
        // An other-button press is no plain press either.
        coordinator.handle(rig.press(.otherMouseDown, at: padding))
        #expect(coordinator.hasSelection)
        // A plain press there still clears.
        coordinator.handle(rig.press(.leftMouseDown, at: padding))
        #expect(!coordinator.hasSelection)
        #expect(NSPasteboard.general.changeCount == general)
    }

    @Test func withASelectionInOneMessageARightClickOnAnotherTargetsTheOneUnderThePointer() async throws {
        let rig = await ColumnRig([message(1, text: "alpha bravo"), message(2, text: "delta echo")])
        defer { rig.presenter.dismiss(); rig.window.close() }
        let x = try #require(rig.body(1)), y = try #require(rig.body(2))
        rig.select(in: y, from: 0, to: 5)
        #expect(x.selection.hasSelection)
        let point = center(x, NSRange(location: 2, length: 1))
        let (hit, items) = rig.rightClick(at: point)
        #expect(hit === x, "the text view takes its own right-click, not the row trigger")
        #expect(rig.presenter.context?.messageID == "m1")
        #expect(items.first == .copySelection)
        // A Control-click on the same text: the same menu, and the selection is still there.
        rig.presenter.dismiss()
        let control = rig.press(.leftMouseDown, at: point, control: true)
        x.selection.handle(control)
        rig.window.deliver(control)
        #expect(rig.presenter.context?.messageID == "m1" && x.selection.hasSelection)
        rig.board.clearContents()
        rig.presenter.choose(.copySelection, key: nil)
        #expect(rig.board.string(forType: .string) == "delta")
    }

    // MARK: Link hits

    @Test func aRightClickFortyPointsPastALineEndingInALinkGivesNoLinkItems() async throws {
        let body = MessageBody(blocks: [.paragraph([.text("see "), link("https://example.com/", host: "example.com")])])
        let rig = await ColumnRig([message(1, text: "see https://example.com/", body: body)])
        defer { rig.presenter.dismiss(); rig.window.close() }
        let view = try #require(rig.body(1))
        let linkRange = try #require(range(of: TextKey.link, in: view))
        let end = try #require(view.rects(for: linkRange).last)
        let past = view.convert(CGPoint(x: end.maxX + 40, y: end.midY), to: nil)
        let off = rig.rightClick(at: past)
        #expect(off.hit === view)
        #expect(rig.presenter.isPresented && !hasLinkItems(off.items), "\(off.items)")
        let on = rig.rightClick(at: center(view, linkRange))
        #expect(on.items.contains(.copyLink("https://example.com/")) && on.items.contains(.openLink))
    }

    @Test func aRightClickOnAHiddenSpoilerInsideAMaskedLinkGivesNoLinkItems() async throws {
        let label: [MarkdownInline] = [.spoiler(id: 0, [.text("secret")]), .text(" shown")]
        let body = MessageBody(blocks: [.paragraph([link("https://b.example/", host: "b.example", label: label)])])
        let rig = await ColumnRig([message(1, text: "[||secret|| shown](https://b.example/)", body: body)])
        defer { rig.presenter.dismiss(); rig.window.close() }
        let view = try #require(rig.body(1))
        let hidden = try #require(range(of: TextKey.hidden, in: view))
        let covered = rig.rightClick(at: center(view, hidden))
        #expect(covered.hit === view && rig.presenter.isPresented && !hasLinkItems(covered.items), "\(covered.items)")
        let shown = rig.rightClick(at: center(view, try #require(range(of: TextKey.link, in: view))))
        #expect(shown.items.contains(.openLink))
    }

    /// The embed description is its own `MarkdownView`, numbering its own links: link 0 there is the
    /// embed's, not the body's. Open Link runs the embed router's rule (a plain link opens without a
    /// confirmation) on the embed's URL. The router's opener is the system's, so the captured router's
    /// body and rule are run through a recording opener.
    @Test func anEmbedDescriptionsLinkUsesTheEmbedsRouterAndNumbering() async throws {
        let body = MessageBody(blocks: [.paragraph([link("https://body.example/", host: "body.example", label: [.text("body")])])])
        let embed = EmbedItem(description: MessageBody(blocks: [.paragraph([.text("read "), link("https://embed.example/", host: "embed.example")])]))
        let rig = await ColumnRig([message(1, text: "[body](https://body.example/)", body: body, embeds: [embed])])
        defer { rig.presenter.dismiss(); rig.window.close() }
        let view = try #require(rig.views.first { $0.attributed.string.contains("embed.example") })
        #expect(view.message.isEmpty, "the embed's text is not part of the column's selection")
        let (hit, items) = rig.rightClick(at: center(view, try #require(range(of: TextKey.link, in: view))))
        #expect(hit === view)
        #expect(rig.presenter.context?.messageID == "m1")
        #expect(items.contains(.copyLink("https://embed.example/")))
        let captured = try #require(rig.presenter.context?.link)
        #expect(captured.index == 0 && captured.router.body.links.map(\.url) == ["https://embed.example/"])

        var opened: [URL] = [], confirmed: [String] = []
        let recording = LinkRouter(body: captured.router.body, reveal: captured.router.reveal, confirm: { confirmed.append($0.url) }, open: { opened.append($0) })
        let context = MessageMenuContext(message: rig.rows.messages[0], conversation: nil, link: (captured.index, captured.target, recording, captured.liveBody))
        rig.presenter.present(context, body: rig.rows.messages[0].body, conversation: nil, replyPreview: nil, at: .zero, from: rig.window) { _ in }
        rig.presenter.choose(.openLink, key: nil)
        #expect(opened == [URL(string: "https://embed.example/")!] && confirmed.isEmpty, "\(opened) \(confirmed)")
    }

    // MARK: Choose time

    @Test func aBodyChangeBetweenOpenAndChooseOpensNothing() async throws {
        var opened: [URL] = []
        var requests: [MessageMenuRequest] = []
        let model = BodyModel(MessageBody(blocks: [.paragraph([.text("go "), link("https://a.example/", host: "a.example")])]))
        let host = BodyHost(model: model, opener: { opened.append($0) })
            .environment(\.messageMenu, MessageMenuOpener { requests.append($0) })
            .frame(width: 400)
        let (hosting, window) = TextHarness.hosting(host, size: CGSize(width: 400, height: 60))
        defer { window.close() }
        let presenter = MessageMenuPresenter(pasteboard: TextHarness.board(), show: { _ in })
        let item = message(1, text: "go https://a.example/", body: model.body)
        func openMenu() throws {
            let view = try #require(TextHarness.textViews(in: hosting).first)
            let at = try #require(range(of: TextKey.link, in: view))
            view.rightMouseDown(with: TextHarness.mouse(.rightMouseDown, at: view.convert(center(view, at), from: nil), in: view))
            let request = try #require(requests.last)
            presenter.present(
                MessageMenuContext(message: item, conversation: nil, link: request.link, copySelection: request.copySelection),
                body: item.body, conversation: nil, replyPreview: nil, at: request.point, from: window
            ) { _ in }
            #expect(presenter.model.items.contains(.openLink))
        }
        try openMenu()
        presenter.choose(.openLink, key: nil)
        #expect(opened == [URL(string: "https://a.example/")!], "unchanged: opens once")

        try openMenu()
        // The row's revalidation (C1 Dismissal) is bypassed: only the choose-time check stands.
        model.body = MessageBody(blocks: [.paragraph([.text("go "), link("https://evil.example/", host: "evil.example")])])
        hosting.layoutSubtreeIfNeeded()
        #expect(presenter.isPresented)
        presenter.choose(.openLink, key: nil)
        #expect(opened.count == 1, "\(opened)")

        // VoiceOver's Show Menu has no pointer: the row's menu without link items.
        requests = []
        let view = try #require(TextHarness.textViews(in: hosting).first)
        #expect(view.accessibilityPerformShowMenu())
        #expect(requests.count == 1 && requests[0].link == nil)
    }

    @Test func copyLinkOnAMaskedLinkWritesItsURLNotItsLabel() throws {
        let general = NSPasteboard.general.changeCount
        var requests: [MessageMenuRequest] = []
        let body = MessageBody(blocks: [.paragraph([link("https://b.com/", host: "b.com", label: [.text("https://a.com")])])])
        let (hosting, window) = TextHarness.hosting(
            BodyHost(model: BodyModel(body)).environment(\.messageMenu, MessageMenuOpener { requests.append($0) }).frame(width: 400),
            size: CGSize(width: 400, height: 60))
        defer { window.close() }
        let view = try #require(TextHarness.textViews(in: hosting).first)
        #expect(view.attributed.string == "https://a.com")
        let at = view.convert(center(view, try #require(range(of: TextKey.link, in: view))), from: nil)
        view.rightMouseDown(with: TextHarness.mouse(.rightMouseDown, at: at, in: view))
        let request = try #require(requests.last)
        let board = TextHarness.board()
        defer { board.releaseGlobally() }
        let presenter = MessageMenuPresenter(pasteboard: board, show: { _ in })
        let item = message(1, text: "[https://a.com](https://b.com)", body: body)
        presenter.present(MessageMenuContext(message: item, conversation: nil, link: request.link), body: body, conversation: nil,
                          replyPreview: nil, at: request.point, from: window) { _ in }
        let copy = try #require(presenter.model.items.first { if case .copyLink = $0 { true } else { false } })
        presenter.choose(copy, key: nil)
        #expect(board.string(forType: .string) == "https://b.com/")
        #expect(NSPasteboard.general.changeCount == general)
    }
}
