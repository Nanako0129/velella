import AppKit
import SwiftUI

private typealias Menu = DesignTokens.Menu

// The message context menu (docs/plans/selection.md, C1): official Discord's layout
// (docs/element-map.md, e0-menu-other.png) on a glass panel. Fresh code.

/// The link under the pointer (W1 Link): its index in `router`'s body, the target captured at open,
/// the router that owns the text (the nearest `MarkdownView`'s), and that view's current body.
typealias MessageMenuLink = (index: Int, target: LinkTarget, router: LinkRouter, liveBody: () -> MessageBody?)

/// What a right-click hands the row's menu (W1 Trigger): the screen point, the window, and what the
/// text view under the pointer adds. The row trigger leaves `link` and `copySelection` nil.
struct MessageMenuRequest {
    var point: NSPoint
    var window: NSWindow?
    var link: MessageMenuLink?
    var copySelection: (() -> Void)?
}

struct MessageMenuOpener: @unchecked Sendable { var open: ((MessageMenuRequest) -> Void)? }
struct LinkRouterBox: @unchecked Sendable { var router: LinkRouter? }

extension EnvironmentValues {
    /// The row's menu (set by `MessageRow` over the whole row, embeds included): a text view opens
    /// it for its own row, never for the selection's anchor.
    @Entry var messageMenu = MessageMenuOpener()
    /// The nearest `MarkdownView`'s router: its body numbers the link indices of the text below it.
    @Entry var textLinks = LinkRouterBox()
}

/// What a menu acts on, captured when it opens (C1 Context): nothing here is read again when an
/// item is chosen, except `link.liveBody`, which Open Link compares against.
struct MessageMenuContext {
    let messageID: String
    let delivery: MessageItem.Delivery
    let authorName: String
    /// The raw content, what Copy Text writes byte for byte (D1).
    let content: String
    /// The conversation's channel; empty when the rows came without one (fixtures).
    let channelID: String
    /// nil for a direct message.
    let guildID: String?
    /// The link under the pointer and the router that owns it (W1 supplies it).
    let link: MessageMenuLink?
    /// Copies the current selection (W1 supplies it).
    let copySelection: (() -> Void)?

    /// The row trigger's context (C1): the row's own values and the conversation its rows were
    /// loaded for, never the current selection.
    init(
        message: MessageItem, conversation: ConversationIDs?,
        link: MessageMenuLink? = nil,
        copySelection: (() -> Void)? = nil
    ) {
        messageID = message.id
        delivery = message.delivery
        authorName = message.authorName
        content = message.text
        channelID = conversation?.channelID ?? ""
        guildID = conversation?.guildID
        self.link = link
        self.copySelection = copySelection
    }
}

/// One menu row. The payload each case carries is captured when the menu opens.
enum MessageMenuItem: Equatable {
    case copySelection
    case reply(ReplyPreview)
    case copyText(String)
    case copyMessageLink(String)
    /// The link's canonical URL, never its label (F-13).
    case copyLink(String)
    case openLink
    case copyMessageID(String)

    var title: String {
        switch self {
        case .copySelection: "Copy"
        case .reply: "Reply"
        case .copyText: "Copy Text"
        case .copyMessageLink: "Copy Message Link"
        case .copyLink: "Copy Link"
        case .openLink: "Open Link"
        case .copyMessageID: "Copy Message ID"
        }
    }

    /// SF Symbols standing in for Discord's icons.
    var symbol: String {
        switch self {
        case .copySelection, .copyText: "doc.on.doc.fill"
        case .reply: "arrow.turn.up.left"
        case .copyMessageLink, .copyLink: "link"
        case .openLink: "macwindow"
        case .copyMessageID: "number.square.fill"
        }
    }
}

/// The menu's content rules (C1 Items and Gating), pure so the tests target them.
enum MessageMenu {
    /// Sections in e0-menu-other.png's order, items velella cannot do left out: Copy ⌘C (with a
    /// selection, as in e0-menu-selection.png) | Reply | Copy Text, Copy Message Link | Copy Link,
    /// Open Link (e0-menu-link.png) | Copy Message ID. Empty sections are dropped. `replyPreview`
    /// is the preview line core flattened when the menu opened.
    static func sections(_ context: MessageMenuContext, replyPreview: String?) -> [[MessageMenuItem]] {
        // An echo carries a nonce for its id until Discord confirms it (S-11, F-10).
        let sent = context.delivery == .sent
        let link = sent ? messageLink(messageID: context.messageID, channelID: context.channelID, guildID: context.guildID) : nil
        let id = link == nil ? nil : canonicalID(context.messageID)
        var copies: [MessageMenuItem] = context.content.isEmpty ? [] : [.copyText(context.content)]
        if let link { copies.append(.copyMessageLink(link)) }
        let reply = ReplyPreview(
            authorName: context.authorName, segments: [.text(replyPreview ?? "")],
            messageID: context.messageID, channelID: context.channelID
        )
        let sections: [[MessageMenuItem]] = [
            context.copySelection == nil ? [] : [.copySelection],
            sent ? [.reply(reply)] : [],
            copies,
            context.link.map { [.copyLink($0.target.url), .openLink] } ?? [],
            id.map { [.copyMessageID($0)] } ?? [],
        ]
        return sections.filter { !$0.isEmpty }
    }

    /// `s` when it is a `UInt64`, re-formatted (so `+5` becomes `5`); otherwise nil (S-11).
    static func canonicalID(_ s: String) -> String? {
        UInt64(s).map { String($0) }
    }

    /// Discord's message link, or nil when any id fails `canonicalID`. A DM (no guild) uses
    /// `@me`; a guild id that fails hides the link rather than falling back to `@me`.
    static func messageLink(messageID: String, channelID: String, guildID: String?) -> String? {
        guard let message = canonicalID(messageID), let channel = canonicalID(channelID) else { return nil }
        let scope: String
        if let guildID {
            guard let guild = canonicalID(guildID) else { return nil }
            scope = guild
        } else {
            scope = "@me"
        }
        return "https://discord.com/channels/\(scope)/\(channel)/\(message)"
    }

    /// The panel's size for `sections`, from the D5 metrics.
    static func size(_ sections: [[MessageMenuItem]]) -> CGSize {
        let rows = CGFloat(sections.reduce(0) { $0 + $1.count })
        let breaks = CGFloat(max(sections.count - 1, 0))
        return CGSize(
            width: Menu.width,
            height: 2 * Menu.panelPadding + rows * (Menu.rowHeight + 2 * Menu.rowMargin)
                + breaks * (Menu.separatorHeight + 2 * Menu.separatorGap)
        )
    }

    /// The panel's frame: its top-left corner at the pointer, flipped left or up where it would
    /// leave `visible`, then kept inside it.
    static func frame(size: CGSize, at point: NSPoint, in visible: NSRect) -> NSRect {
        var x = point.x, y = point.y - size.height
        if x + size.width > visible.maxX { x = point.x - size.width }
        if y < visible.minY { y = point.y }
        x = min(max(x, visible.minX), visible.maxX - size.width)
        y = min(max(y, visible.minY), visible.maxY - size.height)
        return NSRect(origin: NSPoint(x: x, y: y), size: size)
    }
}

// MARK: Panel

/// The menu's window. Borderless, so it says it can become key (a borderless window cannot by
/// default): while the menu is open it is the key window, and keys never reach the composer
/// behind it (S-7, F-3). Every key event goes to `onKey`.
final class MessageMenuPanel: NSPanel {
    var onKey: ((NSEvent) -> Void)?

    init() {
        super.init(contentRect: .zero, styleMask: [.borderless], backing: .buffered, defer: true)
        isFloatingPanel = true
        level = .popUpMenu
        isOpaque = false
        backgroundColor = .clear
        hasShadow = true
        isReleasedWhenClosed = false
        hidesOnDeactivate = false
        becomesKeyOnlyIfNeeded = false
        collectionBehavior = [.transient, .ignoresCycle, .fullScreenAuxiliary]
    }

    override var canBecomeKey: Bool { true }
    override var canBecomeMain: Bool { false }

    override func sendEvent(_ event: NSEvent) {
        switch event.type {
        case .keyDown, .keyUp: onKey?(event)
        default: super.sendEvent(event)
        }
    }
}

/// Rows, separators and the highlighted row, shared by the presenter and the view.
@MainActor @Observable
final class MessageMenuModel {
    var sections: [[MessageMenuItem]] = []
    /// Index into `items`.
    var highlighted: Int?

    var items: [MessageMenuItem] { sections.flatMap { $0 } }

    /// Down (+1) or Up (-1), wrapping; from no highlight, Down starts at the first row and Up at
    /// the last.
    func move(_ step: Int) {
        let count = items.count
        guard count > 0 else { return }
        highlighted = highlighted.map { ($0 + step + count) % count } ?? (step > 0 ? 0 : count - 1)
    }
}

/// Opens one menu at a time and runs its items (C1 Focus and keys, Dismissal, Pasteboard).
@MainActor
final class MessageMenuPresenter {
    private(set) var panel: MessageMenuPanel?
    private(set) var context: MessageMenuContext?
    let model = MessageMenuModel()
    /// Copies go here (production: `.general`; tests inject a private board).
    let pasteboard: NSPasteboard
    /// Orders the panel in and makes it key; tests pass one that leaves it offscreen.
    private let show: (NSPanel) -> Void
    private var body: MessageBody?
    private var conversation: ConversationIDs?
    private var setReply: (ReplyPreview) -> Void = { _ in }
    /// The key an item was chosen with: swallowed, repeats included, until its key-up.
    private var held: UInt16?
    /// Runs once the panel is gone and the responder is back (Open Link's dialog).
    private var afterClose: (() -> Void)?
    private var restoreWindow: NSWindow?
    private var restoreResponder: NSResponder?
    private var observers: [NSObjectProtocol] = []
    private var monitor: Any?

    init(pasteboard: NSPasteboard = .general, show: @escaping (NSPanel) -> Void = { $0.makeKeyAndOrderFront(nil) }) {
        self.pasteboard = pasteboard
        self.show = show
    }

    /// Open: shown, or invisible while it swallows the key an item was chosen with. The panel is
    /// the key window throughout.
    var isPresented: Bool { panel != nil }

    /// Opens the menu for `context` at `point` (screen coordinates) over `window`, whose first
    /// responder comes back when it closes. `body` and `conversation` are the row's at open:
    /// `revalidate` dismisses when either changes.
    func present(
        _ context: MessageMenuContext, body: MessageBody, conversation: ConversationIDs?, replyPreview: String?,
        at point: NSPoint, from window: NSWindow?, setReply: @escaping (ReplyPreview) -> Void
    ) {
        finish(restore: false)
        let sections = MessageMenu.sections(context, replyPreview: replyPreview)
        guard !sections.isEmpty else { return }
        self.context = context
        self.body = body
        self.conversation = conversation
        self.setReply = setReply
        model.sections = sections
        model.highlighted = nil
        restoreWindow = window
        restoreResponder = window?.firstResponder

        let panel = MessageMenuPanel()
        panel.onKey = { [weak self] in self?.key($0) }
        let size = MessageMenu.size(sections)
        panel.contentView = NSHostingView(rootView: MessageMenuView(model: model) { [weak self] in self?.choose($0, key: nil) })
        let screen = NSScreen.screens.first { $0.frame.contains(point) } ?? window?.screen ?? NSScreen.main
        panel.setFrame(MessageMenu.frame(size: size, at: point, in: screen?.visibleFrame ?? NSRect(origin: point, size: size)), display: false)
        self.panel = panel
        observe(window)
        show(panel)
    }

    /// Dismissal by data (C1 Dismissal): the message left the rows (deleted, a channel switch or
    /// sign-out emptied them), its body, content or delivery changed, or the conversation did.
    func revalidate(messages: [MessageItem], conversation: ConversationIDs?) {
        guard let context, let body else { return }
        let row = messages.first { $0.id == context.messageID }
        let current = conversation == self.conversation && row.map {
            $0.body == body && $0.text == context.content && $0.delivery == context.delivery
        } == true
        guard !current else { return }
        if held != nil {
            // Already invisible, its item already run: keep swallowing the held key, but a
            // deferred action no longer applies to what is shown.
            afterClose = nil
        } else {
            finish(restore: true)
        }
    }

    /// Closes the menu and gives the key status back.
    func dismiss() {
        afterClose = nil
        finish(restore: true)
    }

    /// Runs `item`. Chosen with a key, the panel stays key but invisible and swallows that key
    /// until its key-up; only then does it close and restore the responder (S-7, S-16).
    func choose(_ item: MessageMenuItem, key: UInt16?) {
        guard let panel, held == nil, let context else { return }
        switch item {
        case .copySelection: context.copySelection?()
        case .reply(let preview): setReply(preview)
        case .copyText(let s), .copyMessageLink(let s), .copyLink(let s), .copyMessageID(let s):
            CopyFeedback().copy(s, to: pasteboard)
        case .openLink:
            afterClose = { Self.openLink(context) }
        }
        guard let key else { return finish(restore: true) }
        hold(key, panel)
    }

    // MARK: Keys

    private func key(_ event: NSEvent) {
        if let held {
            if event.type == .keyUp && event.keyCode == held { finish(restore: true) }
            return
        }
        guard event.type == .keyDown, let panel else { return }
        switch event.keyCode {
        case KeyCode.down: model.move(1)
        case KeyCode.up: model.move(-1)
        case KeyCode.escape: afterClose = nil; hold(event.keyCode, panel)
        case KeyCode.returnKey, KeyCode.keypadEnter, KeyCode.space:
            // A repeat belongs to a key that went down before the menu had the keys.
            guard !event.isARepeat, let i = model.highlighted, model.items.indices.contains(i) else { return }
            choose(model.items[i], key: event.keyCode)
        default: break
        }
    }

    private func hold(_ key: UInt16, _ panel: MessageMenuPanel) {
        held = key
        panel.alphaValue = 0
        panel.ignoresMouseEvents = true
    }

    private enum KeyCode {
        static let returnKey: UInt16 = 36, keypadEnter: UInt16 = 76, space: UInt16 = 49
        static let escape: UInt16 = 53, down: UInt16 = 125, up: UInt16 = 126
    }

    // MARK: Closing

    /// Removes the panel; with `restore`, the window it opened over becomes key again with its
    /// first responder. Without it (another window took the keys, the app resigned active, the
    /// window is closing) nothing is moved. A deferred action runs last.
    private func finish(restore: Bool) {
        guard let panel else { return }
        observers.forEach { NotificationCenter.default.removeObserver($0) }
        observers = []
        if let monitor { NSEvent.removeMonitor(monitor) }
        monitor = nil
        self.panel = nil
        context = nil
        body = nil
        held = nil
        panel.onKey = nil
        panel.orderOut(nil)
        if restore, let window = restoreWindow, window.isVisible {
            window.makeKey()
            if let responder = restoreResponder, window.firstResponder !== responder { window.makeFirstResponder(responder) }
        }
        restoreWindow = nil
        restoreResponder = nil
        let after = afterClose
        afterClose = nil
        after?()
    }

    /// Dismissal by the system (C1 Dismissal): the app resigning active, the window closing,
    /// another window becoming key, a scroll, a click outside (consumed).
    private func observe(_ window: NSWindow?) {
        let center = NotificationCenter.default
        /// `handle` gets the notification's object, by identity.
        func on(_ name: Notification.Name, _ object: AnyObject? = nil, _ handle: @escaping @MainActor (ObjectIdentifier?) -> Void) {
            observers.append(center.addObserver(forName: name, object: object, queue: nil) { note in
                let sender = (note.object as AnyObject?).map(ObjectIdentifier.init)
                MainActor.assumeIsolated { handle(sender) }
            })
        }
        on(NSApplication.didResignActiveNotification) { [weak self] _ in self?.finish(restore: false) }
        on(NSWindow.didBecomeKeyNotification) { [weak self] sender in
            guard let self, let panel = self.panel, sender != ObjectIdentifier(panel) else { return }
            self.finish(restore: false)
        }
        if let window {
            on(NSWindow.willCloseNotification, window) { [weak self] _ in self?.finish(restore: false) }
        }
        monitor = NSEvent.addLocalMonitorForEvents(matching: [.leftMouseDown, .rightMouseDown, .otherMouseDown, .scrollWheel]) { [weak self] event in
            let type = event.type, number = event.windowNumber
            let consume = MainActor.assumeIsolated { self?.outside(type, windowNumber: number) ?? false }
            return consume ? nil : event
        }
    }

    /// A click or scroll seen by the event monitor: one outside the panel dismisses (once the
    /// menu is invisible it changes nothing); a click is consumed (true), a scroll goes on.
    private func outside(_ type: NSEvent.EventType, windowNumber: Int) -> Bool {
        guard let panel, windowNumber != panel.windowNumber else { return false }
        if held == nil { dismiss() }
        return type != .scrollWheel
    }

    /// Open Link (C1 Items, W1 Link): only while the owning view's current body still has the
    /// captured target at its index; then that router's checks run on it.
    private static func openLink(_ context: MessageMenuContext) {
        guard let link = context.link, let live = link.liveBody(), live.links.indices.contains(link.index),
              live.links[link.index] == link.target, link.router.body.links.indices.contains(link.index),
              link.router.body.links[link.index] == link.target,
              let url = URL(string: "velella-link:\(link.index)") else { return }
        _ = link.router.handle(url)
    }
}

// MARK: View

/// The panel's content: glass, Discord's rows (D5).
struct MessageMenuView: View {
    let model: MessageMenuModel
    let choose: (MessageMenuItem) -> Void

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            ForEach(Array(model.sections.enumerated()), id: \.offset) { s, section in
                if s > 0 {
                    Rectangle().fill(Menu.separator).frame(height: Menu.separatorHeight).padding(.vertical, Menu.separatorGap)
                }
                let start = model.sections[..<s].reduce(0) { $0 + $1.count }
                ForEach(Array(section.enumerated()), id: \.offset) { i, item in
                    row(item, index: start + i)
                }
            }
        }
        .padding(Menu.panelPadding)
        .frame(width: Menu.width, alignment: .leading)
        .glassEffect(.regular, in: .rect(cornerRadius: Menu.panelRadius))
    }

    private func row(_ item: MessageMenuItem, index: Int) -> some View {
        Button { choose(item) } label: {
            HStack(spacing: Menu.iconGap) {
                Image(systemName: item.symbol)
                    .font(Menu.icon)
                    .foregroundStyle(.secondary)
                    .frame(width: Menu.iconBox, height: Menu.iconBox)
                    .accessibilityHidden(true)
                Text(item.title).font(Menu.label).foregroundStyle(.primary).lineLimit(1)
                Spacer(minLength: 0)
                switch item {
                case .copySelection: Text(verbatim: "⌘C").font(Menu.trailing).foregroundStyle(.secondary)
                case .openLink: Image(systemName: "arrow.up.right").font(Menu.trailing).foregroundStyle(.secondary)
                default: EmptyView()
                }
            }
            .padding(.horizontal, Menu.itemPadding)
            .frame(maxWidth: .infinity, minHeight: Menu.rowHeight, maxHeight: Menu.rowHeight, alignment: .leading)
            .background(model.highlighted == index ? Menu.rowHighlight : .clear, in: RoundedRectangle(cornerRadius: Menu.itemRadius))
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .padding(.vertical, Menu.rowMargin)
        .onHover { inside in
            if inside { model.highlighted = index } else if model.highlighted == index { model.highlighted = nil }
        }
    }
}

// MARK: Row trigger

/// Laid over a message row (C1 Trigger): takes a right-click, or a Control-click, anywhere on
/// the row and hands its screen point and window to `open`. Every other event passes through:
/// for those `hitTest` answers nil, so the row's own views get them. So does a right-click on a
/// message text view (W1 Trigger): the text view opens the menu itself, with its link and selection.
struct MessageMenuTrigger: NSViewRepresentable {
    let open: (MessageMenuRequest) -> Void

    func makeNSView(context: Context) -> TriggerView {
        let view = TriggerView()
        view.open = open
        return view
    }

    func updateNSView(_ view: TriggerView, context: Context) {
        view.open = open
    }

    final class TriggerView: NSView {
        var open: (MessageMenuRequest) -> Void = { _ in }
        /// Set while asking the window what lies under this overlay.
        private var lookingThrough = false

        static func opensMenu(_ event: NSEvent) -> Bool {
            event.type == .rightMouseDown || (event.type == .leftMouseDown && event.modifierFlags.contains(.control))
        }

        override func hitTest(_ point: NSPoint) -> NSView? {
            guard !lookingThrough, let event = window?.currentEvent, Self.opensMenu(event), let hit = super.hitTest(point) else { return nil }
            return textUnder(point) ? nil : hit
        }

        /// Whether a message text view is what the press would reach without this overlay.
        private func textUnder(_ point: NSPoint) -> Bool {
            guard let content = window?.contentView else { return false }
            lookingThrough = true
            defer { lookingThrough = false }
            let inWindow = superview?.convert(point, to: nil) ?? point
            return content.hitTest(content.superview?.convert(inWindow, from: nil) ?? inWindow) is MessageTextView
        }

        override func rightMouseDown(with event: NSEvent) { fire(event) }

        override func mouseDown(with event: NSEvent) {
            if Self.opensMenu(event) { fire(event) } else { super.mouseDown(with: event) }
        }

        /// AppKit's own context menu never shows here.
        override func menu(for event: NSEvent) -> NSMenu? { nil }

        private func fire(_ event: NSEvent) {
            let point = window.map { $0.convertPoint(toScreen: event.locationInWindow) } ?? NSEvent.mouseLocation
            open(MessageMenuRequest(point: point, window: window))
        }
    }
}
