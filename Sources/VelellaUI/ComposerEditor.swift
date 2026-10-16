import AppKit
import SwiftUI

private typealias T = DesignTokens

/// The rules the editor's keys and height follow, kept free of AppKit state so tests can drive them.
enum ComposerRules {
    /// The official `hasOpenCodeBlock`: an odd number of ``` fences before the caret (UTF-16 offset).
    static func hasOpenCodeFence(in text: String, caret: Int) -> Bool {
        let before = (text as NSString).substring(to: min(max(caret, 0), (text as NSString).length))
        return (before.components(separatedBy: "```").count - 1) % 2 == 1
    }

    enum ReturnAction: Equatable { case send, newline, system }

    /// What a Return does. Plain Return and Option-Return send, Shift-Return and Return inside an open
    /// code fence insert a newline; any other modifier is left to the text view.
    static func returnAction(_ modifiers: NSEvent.ModifierFlags, text: String, caret: Int) -> ReturnAction {
        switch modifiers.intersection([.shift, .option, .control, .command]) {
        case [.shift]: .newline
        case [.option]: .send
        case []: hasOpenCodeFence(in: text, caret: caret) ? .newline : .send
        default: .system
        }
    }

    /// The editor's limit (plan D2): `composerMaxHeight`, or half the window's safe-area height when
    /// that is smaller; `composerNoWindowHeight` before the view has a window.
    static func heightLimit(windowSafeHeight: CGFloat?) -> CGFloat {
        min(T.Size.composerMaxHeight, windowSafeHeight.map { $0 * T.Size.composerMaxWindowFraction } ?? T.Size.composerNoWindowHeight)
    }

    /// The editor's height: the content's lowest layout-fragment edge plus both insets, rounded up,
    /// never below one line's row and never above `limit` (the floor wins in a tiny window). TextKit
    /// lays one 13 pt line out 16.006 pt tall (measured); the 0.05 keeps that noise from adding a point.
    static func height(contentMaxY: CGFloat, limit: CGFloat) -> CGFloat {
        max(T.Size.composerRow, min(limit, ceil(contentMaxY + 2 * T.Size.composerInputInset - 0.05)))
    }
}

/// What a paste can attach (composer plan CUA, CU-6, CU-10): read from the pasteboard here, in the
/// composer's editor only.
enum PasteContent: Equatable {
    case files([URL])
    /// PNG bytes.
    case image(Data)
    case longText(String)

    /// Official `uploadPromptCharacterCount` (OA A21): UTF-16 units.
    static let longTextThreshold = 4000

    /// Files come only from `public.file-url` items (never `public.url`, text that looks like a path,
    /// or a promise). Without files, image data attaches when no text is on the board (PNG preferred,
    /// TIFF encoded to PNG), and text over 4000 UTF-16 units becomes `message.txt` (OA A21). Anything
    /// else is nil: a plain text paste.
    init?(_ board: NSPasteboard) {
        let urls = (board.pasteboardItems ?? []).compactMap { $0.string(forType: .fileURL) }.compactMap { URL(string: $0) }.filter(\.isFileURL)
        if !urls.isEmpty {
            self = .files(urls)
            return
        }
        let text = board.string(forType: .string)
        if text == nil {
            if let png = board.data(forType: .png) {
                self = .image(png)
                return
            }
            if let tiff = board.data(forType: .tiff), let png = NSBitmapImageRep(data: tiff)?.representation(using: .png, properties: [:]) {
                self = .image(png)
                return
            }
        }
        if let text, text.utf16.count > Self.longTextThreshold {
            self = .longText(text)
            return
        }
        return nil
    }
}

/// The composer's text view: plain text, TextKit 2, and the composer's keys.
final class ComposerTextView: NSTextView {
    /// Return, Option-Return (when it sends).
    var onSubmit: (() -> Void)?
    /// Esc; nil while no reply is set.
    var onCancelReply: (() -> Void)?
    /// The window was resized (the height limit follows it).
    var onWindowResize: (() -> Void)?
    private var resizeObserver: NSObjectProtocol?

    static func make() -> ComposerTextView {
        let view = ComposerTextView(usingTextLayoutManager: true)
        view.isRichText = false
        view.importsGraphics = false
        view.allowsUndo = true
        view.drawsBackground = false
        view.font = T.Typography.composerAppKit
        view.textColor = .labelColor
        view.isAutomaticQuoteSubstitutionEnabled = false
        view.isAutomaticDashSubstitutionEnabled = false
        view.isAutomaticLinkDetectionEnabled = false
        view.isAutomaticDataDetectionEnabled = false
        view.isAutomaticTextReplacementEnabled = false
        view.isAutomaticSpellingCorrectionEnabled = false
        view.isAutomaticTextCompletionEnabled = false
        view.smartInsertDeleteEnabled = false
        view.isContinuousSpellCheckingEnabled = true
        view.textContainerInset = NSSize(width: 0, height: T.Size.composerInputInset)
        view.textContainer?.lineFragmentPadding = 0
        view.isVerticallyResizable = true
        view.isHorizontallyResizable = false
        view.autoresizingMask = [.width]
        view.minSize = .zero
        view.maxSize = NSSize(width: CGFloat.greatestFiniteMagnitude, height: CGFloat.greatestFiniteMagnitude)
        view.textContainer?.widthTracksTextView = true
        return view
    }

    override func keyDown(with event: NSEvent) {
        // Marked text belongs to the input method: its Return commits a candidate.
        guard !hasMarkedText() else { return super.keyDown(with: event) }
        switch event.keyCode {
        case 36, 76: // Return, keypad Enter
            switch ComposerRules.returnAction(event.modifierFlags, text: string, caret: selectedRange().location) {
            case .send: onSubmit?()
            case .newline: insertNewline(nil)
            case .system: super.keyDown(with: event)
            }
        case 53 where onCancelReply != nil: // Esc
            onCancelReply?()
        default:
            super.keyDown(with: event)
        }
    }

    /// Offered what a paste carries before the text view reads it (composer plan CUA, OA A21); true
    /// when taken. nil while attaching is not allowed or not set up.
    var onPaste: ((PasteContent) -> Bool)?
    /// What a paste reads; tests give it their own board.
    var pasteboard: NSPasteboard = .general

    /// Files, an image or a long text become attachments when the composer takes them; anything else
    /// is plain text, whatever the pasteboard carries.
    override func paste(_ sender: Any?) {
        if let onPaste, let content = PasteContent(pasteboard), onPaste(content) { return }
        if pasteboard === NSPasteboard.general {
            pasteAsPlainText(sender)
        } else if let text = pasteboard.string(forType: .string) {
            insertText(text, replacementRange: NSRange(location: NSNotFound, length: 0))
        }
    }

    /// The content's height at `width`: the lowest edge of the layout fragments, with the extra line
    /// fragment of a trailing newline (the SC measure).
    func contentMaxY(width: CGFloat) -> CGFloat {
        guard let layout = textLayoutManager, let container = textContainer else { return 0 }
        let saved = container.size
        container.size = NSSize(width: width, height: saved.height)
        defer { container.size = saved }
        layout.ensureLayout(for: layout.documentRange)
        var maxY: CGFloat = 0
        layout.enumerateTextLayoutFragments(from: layout.documentRange.location, options: [.ensuresLayout, .ensuresExtraLineFragment]) {
            maxY = max(maxY, $0.layoutFragmentFrame.maxY)
            return true
        }
        return maxY
    }

    override func viewDidMoveToWindow() {
        super.viewDidMoveToWindow()
        resizeObserver.map(NotificationCenter.default.removeObserver)
        resizeObserver = window.map {
            NotificationCenter.default.addObserver(forName: NSWindow.didResizeNotification, object: $0, queue: .main) { [weak self] _ in
                MainActor.assumeIsolated { self?.onWindowResize?() }
            }
        }
        // Its height limit depends on the window, now known.
        DispatchQueue.main.async { [weak self] in self?.onWindowResize?() }
    }
}

/// Keeps the caret in view when the editor's frame changes (growth to the limit, then scrolling).
final class ComposerScrollView: NSScrollView {
    override func layout() {
        super.layout()
        if let text = documentView as? NSTextView { text.scrollRangeToVisible(text.selectedRange()) }
    }
}

/// The composer's multi-line editor (plan CG): an `NSScrollView` around a TextKit 2 text view. The
/// binding is the source of truth; its height follows the content up to `ComposerRules.heightLimit`,
/// then the overlay scroller takes over.
struct ComposerEditor: NSViewRepresentable {
    @Binding var text: String
    /// Bumped when the window resizes: the height limit changed although no proposal did.
    @Binding var windowTick: Int
    var label: String
    var onSubmit: () -> Void
    /// nil while no reply is set.
    var onCancelReply: (() -> Void)?
    /// nil while attaching is not allowed: every paste is plain text.
    var onPaste: ((PasteContent) -> Bool)?
    var pasteboard: NSPasteboard = .general

    func makeCoordinator() -> Coordinator { Coordinator() }

    func makeNSView(context: Context) -> NSScrollView {
        let scroll = ComposerScrollView()
        let view = ComposerTextView.make()
        view.delegate = context.coordinator
        view.setAccessibilityLabel(label)
        scroll.documentView = view
        // The system's "always show scroll bars" would pin the legacy track: the same enforcer as the
        // message list, placed inside the scroll view so it finds this one.
        view.addSubview(OverlayScrollerEnforcer.EnforcerView())
        scroll.drawsBackground = false
        scroll.borderType = .noBorder
        scroll.hasVerticalScroller = true
        scroll.hasHorizontalScroller = false
        scroll.autohidesScrollers = true
        scroll.scrollerStyle = .overlay
        scroll.automaticallyAdjustsContentInsets = false
        scroll.contentInsets = NSEdgeInsetsZero
        return scroll
    }

    func updateNSView(_ scroll: NSScrollView, context: Context) {
        guard let view = scroll.documentView as? ComposerTextView else { return }
        context.coordinator.parent = self
        view.onSubmit = onSubmit
        view.onCancelReply = onCancelReply
        view.onPaste = onPaste
        view.pasteboard = pasteboard
        view.onWindowResize = { [coordinator = context.coordinator] in coordinator.parent.windowTick &+= 1 }
        view.setAccessibilityLabel(label)
        // An outside change (a send cleared the draft, another conversation's draft): the caret
        // goes to its end. Never while the input method holds marked text.
        if view.string != text, !view.hasMarkedText() {
            view.string = text
            view.setSelectedRange(NSRange(location: (text as NSString).length, length: 0))
            view.undoManager?.removeAllActions()
        }
    }

    func sizeThatFits(_ proposal: ProposedViewSize, nsView scroll: NSScrollView, context: Context) -> CGSize? {
        guard let view = scroll.documentView as? ComposerTextView else { return nil }
        // HStack probes with a zero or infinite width to learn its flexibility: no layout of a long
        // draft for that.
        guard let width = proposal.width, width.isFinite, width >= 1 else {
            return CGSize(width: proposal.width.flatMap { $0.isFinite ? max($0, 0) : nil } ?? 0, height: T.Size.composerRow)
        }
        let limit = ComposerRules.heightLimit(windowSafeHeight: scroll.window?.contentView.map { $0.safeAreaRect.height })
        return CGSize(width: width, height: ComposerRules.height(contentMaxY: view.contentMaxY(width: width), limit: limit))
    }

    @MainActor final class Coordinator: NSObject, NSTextViewDelegate {
        var parent: ComposerEditor!

        func textDidChange(_ notification: Notification) {
            guard let view = notification.object as? NSTextView else { return }
            if parent.text != view.string { parent.text = view.string }
        }
    }
}
