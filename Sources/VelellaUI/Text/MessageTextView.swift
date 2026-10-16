import AppKit

// The message text view (docs/plans/selection.md, N1 View, D6 = B): an NSView driving its own
// NSTextLayoutManager + NSTextContentStorage. No NSTextView and no NSLayoutManager exist, so there
// is no TextKit 1 fallback and none of NSTextView's defaults (find, speech, services, Writing
// Tools, link handling, RTF copies) to switch off. It is a static text element and nothing else.

/// What a click on text asks for.
enum TextActivation: Equatable {
    case link(Int)
    case spoiler(Int)

    var url: URL {
        switch self {
        case .link(let n): MarkdownText.linkURL(n)
        case .spoiler(let n): MarkdownText.spoilerURL(n)
        }
    }
}

/// Where a tipped run is, in the view's coordinates, and its tip.
struct TipFrame: Equatable {
    var rect: CGRect
    var tip: String
}

@MainActor
final class MessageTextView: NSView {
    let contentStorage = NSTextContentStorage()
    let layoutManager = NSTextLayoutManager()
    private let container = NSTextContainer()
    private let fragments = TextFragmentProvider()

    /// Layer 1 of S-2 (clip hidden runs out of the glyph drawing). Tests switch it off for the mutation runs.
    var clipsHidden: Bool {
        get { fragments.clipHidden }
        set { fragments.clipHidden = newValue; needsDisplay = true }
    }

    /// Calls the owning `MarkdownView`'s router (through its `openURL`).
    var onActivate: ((TextActivation) -> Void)?
    /// The tipped run under the pointer, or nil.
    var onTip: ((TipFrame?) -> Void)?
    var pasteboard: NSPasteboard = .general
    /// The coordinator of the owning `MarkdownView`; a view outside one has its own.
    weak var coordinator: SelectionCoordinator?
    private lazy var ownCoordinator = SelectionCoordinator()
    var selection: SelectionCoordinator { coordinator ?? ownCoordinator }
    /// This view's place in its coordinator's document order: the message it belongs to (empty in a
    /// body's own coordinator) and its part of it (`PartID.headerPart`, or the body's text view number).
    var message = ""
    var participant = 0
    var partID: PartID { PartID(message: message, part: participant) }
    /// The row's message menu (W1), from the row around this view; nil outside a row.
    var openMenu: ((MessageMenuRequest) -> Void)?
    /// The owning `MarkdownView`'s router: its body numbers this view's link indices.
    var linkRouter: LinkRouter?
    /// The press a menu was last opened for: AppKit can ask `menu(for:)` and then deliver it too.
    private weak var menuEvent: NSEvent?
    /// Whether assistive technology sees this view. The header participant is behind SwiftUI text
    /// that says the same, so it is not an element (N2 Headers).
    var isAccessible = true
    /// What the content was built from, so an update can skip an unchanged one.
    var source: TextSource?
    /// The same for a header participant, whose content is not leaves.
    var contentKey: String?
    private(set) var spoken = ""
    private(set) var unwrapped = false
    private var chrome: TextChrome?
    private var shownTip: TipFrame?
    private var pointer: CGPoint?
    private var press: (point: CGPoint, activation: TextActivation?, single: Bool)?
    private var dragged = false

    var containerOrigin: CGPoint { CGPoint(x: chrome?.inset.width ?? 0, y: (chrome?.inset.height ?? 0) + (chrome?.top ?? 0)) }

    override init(frame: NSRect) {
        super.init(frame: frame)
        container.lineFragmentPadding = 0
        layoutManager.textContainer = container
        contentStorage.addTextLayoutManager(layoutManager)
        contentStorage.primaryTextLayoutManager = layoutManager
        layoutManager.delegate = fragments
        addTrackingArea(NSTrackingArea(rect: .zero, options: [.mouseMoved, .mouseEnteredAndExited, .activeInActiveApp, .inVisibleRect], owner: self))
    }

    required init?(coder: NSCoder) { fatalError("not archived") }

    override var isFlipped: Bool { true }
    override var acceptsFirstResponder: Bool { true }

    /// Replaces the text. The selection is kept as positions in the coordinator (clamped when read),
    /// and the tip under the pointer is looked up again, so a relative-time tick loses neither.
    func setContent(_ content: MessageTextContent) {
        contentStorage.textStorage = NSTextStorage(attributedString: content.text)
        measured = [:]
        settledWidth = nil
        wrappedRows = []
        hasRows = false
        content.text.enumerateAttribute(TextKey.row, in: NSRange(location: 0, length: content.text.length)) { value, _, stop in
            if value != nil { hasRows = true; stop.pointee = true }
        }
        chrome = content.chrome
        spoken = content.spoken
        unwrapped = content.unwrapped
        wantsLayer = (chrome?.radius ?? 0) > 0
        layer?.cornerRadius = chrome?.radius ?? 0
        layer?.masksToBounds = wantsLayer
        needsDisplay = true
        if let pointer {
            let next = tip(at: pointer)
            if next != shownTip {
                shownTip = next
                // Not inside SwiftUI's view update that set the content.
                DispatchQueue.main.async { [weak self] in self?.onTip?(next) }
            }
        }
    }

    var attributed: NSAttributedString { contentStorage.attributedString ?? NSAttributedString() }
    var length: Int { attributed.length }

    // MARK: Layout

    /// Sets the container's width, and settles the code rows for it (their spacing depends on the width).
    private func fit(width: CGFloat) {
        let w = max(width - 2 * containerOrigin.x, 1)
        if container.size.width != w { container.size = CGSize(width: w, height: 0) }
        if settledWidth != w {
            if hasRows { settleRows() }
            settledWidth = w
        }
    }

    private var settledWidth: CGFloat?
    /// Whether the text holds code rows, whose spacing `settleRows` keeps.
    private var hasRows = false
    /// The rows (by start offset) that wrap at the settled width, so carry no extra spacing.
    private var wrappedRows: Set<Int> = []

    /// Height and widest line per proposed width, until the text changes: SwiftUI asks the same
    /// widths again on every pass, and each answer otherwise lays the whole text out anew.
    private var measured: [CGFloat: (height: CGFloat, width: CGFloat)] = [:]

    /// The height of the text at `width` (lays it out).
    func height(forWidth width: CGFloat) -> CGFloat { measure(width).height }

    /// The widest line at `width`.
    func naturalWidth(at width: CGFloat) -> CGFloat { measure(width).width }

    /// What "no width" lays out at.
    static let unbounded: CGFloat = 1e5
    /// SwiftUI's stacks probe a minimum size with width 0, which would lay the whole text out one
    /// character per line; the probe's height is not what the view is placed at, so it is answered
    /// from this width instead. ponytail: two body line boxes, a guess; raise it if a layout ever
    /// depends on that answer.
    private static let probeFloor: CGFloat = 44

    private func measure(_ width: CGFloat) -> (height: CGFloat, width: CGFloat) {
        if let hit = measured[width] { return hit }
        // Text whose widest line fits is laid out exactly as unbounded: one layout serves every
        // width at least that wide (left-aligned text only, which is all this view draws).
        if width < Self.unbounded {
            let free = measure(Self.unbounded)
            if free.width <= width + 0.5 {
                measured[width] = free
                return free
            }
        }
        fit(width: max(width, Self.probeFloor))
        layoutManager.ensureLayout(for: layoutManager.documentRange)
        let last = length > 0 ? (attributed.attribute(.paragraphStyle, at: length - 1, effectiveRange: nil) as? NSParagraphStyle)?.paragraphSpacing ?? 0 : 0
        let usage = layoutManager.usageBoundsForTextContainer
        // TextKit leaves the last paragraph's spacing after it out of its usage bounds.
        let result = (usage.height + last + 2 * (chrome?.inset.height ?? 0) + (chrome?.top ?? 0), usage.width + 2 * containerOrigin.x)
        measured[width] = result
        return result
    }

    override func layout() {
        super.layout()
        fitBounds()
    }

    /// Measuring leaves the container at the last width SwiftUI asked about; drawing and hit-testing
    /// use the view's own, or the unbounded layout when every line fits the view (the same lines).
    private func fitBounds() {
        guard bounds.width > 0 else { return }
        let fits = unwrapped || measured[Self.unbounded].map { $0.width <= bounds.width + 0.5 } == true
        fit(width: fits ? Self.unbounded : bounds.width)
    }

    /// Row minimum of code blocks (N0-4): a one-line row is 19 tall, a wrapped one n x 18. Rows are
    /// built with the one-line row's 1 pt of spacing after them; a row that wraps loses it, and gets
    /// it back when it fits again. A change lays the text out again (TextKit cannot state a minimum
    /// per paragraph).
    private func settleRows() {
        guard let storage = contentStorage.textStorage else { return }
        layoutManager.ensureLayout(for: layoutManager.documentRange)
        var changes: [(NSRange, CGFloat)] = []
        let recheck = !wrappedRows.isEmpty
        layoutManager.enumerateTextLayoutFragments(from: layoutManager.documentRange.location, options: [.ensuresLayout]) { fragment in
            let wraps = fragment.textLineFragments.count > 1
            guard wraps || recheck, let paragraph = fragment.textElement as? NSTextParagraph, paragraph.attributedString.length > 0,
                  paragraph.attributedString.attribute(TextKey.row, at: 0, effectiveRange: nil) != nil else { return true }
            let start = offset(of: fragment.rangeInElement.location)
            guard wraps != wrappedRows.contains(start) else { return true }
            if wraps { wrappedRows.insert(start) } else { wrappedRows.remove(start) }
            changes.append((NSRange(location: start, length: offset(of: fragment.rangeInElement.endLocation) - start), wraps ? 0 : 1))
            return true
        }
        guard !changes.isEmpty else { return }
        contentStorage.performEditingTransaction {
            for (range, spacing) in changes {
                guard let style = storage.attribute(.paragraphStyle, at: range.location, effectiveRange: nil) as? NSParagraphStyle,
                      let copy = style.mutableCopy() as? NSMutableParagraphStyle else { continue }
                copy.paragraphSpacing = spacing
                storage.addAttribute(.paragraphStyle, value: copy, range: range)
            }
        }
        layoutManager.ensureLayout(for: layoutManager.documentRange)
    }

    // MARK: Geometry

    func location(_ offset: Int) -> NSTextLocation? { contentStorage.location(contentStorage.documentRange.location, offsetBy: offset) }
    func offset(of location: NSTextLocation) -> Int { contentStorage.offset(from: contentStorage.documentRange.location, to: location) }

    /// View-coordinate rects of `range`, one per line fragment, each with its baseline (from the rect's top).
    func segments(for range: NSRange, type: NSTextLayoutManager.SegmentType = .standard) -> [(rect: CGRect, baseline: CGFloat)] {
        fitBounds()
        guard let start = location(range.location), let end = location(range.location + range.length),
              let textRange = NSTextRange(location: start, end: end) else { return [] }
        var out: [(CGRect, CGFloat)] = []
        layoutManager.enumerateTextSegments(in: textRange, type: type, options: []) { _, frame, baseline, _ in
            out.append((frame.offsetBy(dx: containerOrigin.x, dy: containerOrigin.y), baseline))
            return true
        }
        return out
    }

    func rects(for range: NSRange, type: NSTextLayoutManager.SegmentType = .standard) -> [CGRect] { segments(for: range, type: type).map(\.rect) }

    /// The character boundary nearest `point`: plain nearest-character hit-testing, used only to
    /// place a selection end, never to decide what a click activates (S-13).
    func index(at point: CGPoint) -> Int {
        let local = CGPoint(x: point.x - containerOrigin.x, y: point.y - containerOrigin.y)
        fitBounds()
        layoutManager.ensureLayout(for: layoutManager.documentRange)
        guard let fragment = layoutManager.textLayoutFragment(for: local) else { return local.y < 0 ? 0 : length }
        let frame = fragment.layoutFragmentFrame
        let inFragment = CGPoint(x: local.x - frame.minX, y: local.y - frame.minY)
        let lines = fragment.textLineFragments
        guard let line = lines.first(where: { inFragment.y < $0.typographicBounds.maxY }) ?? lines.last else { return length }
        let inLine = CGPoint(x: inFragment.x - line.typographicBounds.minX, y: inFragment.y - line.typographicBounds.minY)
        // The character under the point, then the boundary after it when the point is past its middle.
        // ponytail: left-to-right only; in a right-to-left run the end can land one character off.
        var i = line.characterIndex(for: inLine)
        let end = line.characterRange.upperBound
        if i < end, (line.attributedString.string as NSString).character(at: i) != 0x0A {
            let next = (line.attributedString.string as NSString).rangeOfComposedCharacterSequence(at: i).upperBound
            if inLine.x > (line.locationForCharacter(at: i).x + line.locationForCharacter(at: min(next, end)).x) / 2 { i = next }
        }
        return min(offset(of: fragment.rangeInElement.location) + i, length)
    }

    /// The run with `key` whose segment frames contain `point` (S-13: containment, not nearness).
    func runHit(_ key: NSAttributedString.Key, at point: CGPoint) -> (value: Any, range: NSRange)? {
        let near = index(at: point)
        for i in [near, near - 1] where i >= 0 && i < length {
            var range = NSRange()
            guard let value = attributed.attribute(key, at: i, effectiveRange: &range) else { continue }
            if rects(for: range).contains(where: { $0.contains(point) }) { return (value, range) }
        }
        return nil
    }

    /// A hidden spoiler wins over a link inside it: a covered run carries only its spoiler index.
    func activation(at point: CGPoint) -> TextActivation? {
        if let hit = runHit(TextKey.spoiler, at: point), let n = hit.value as? Int { return .spoiler(n) }
        if let hit = runHit(TextKey.link, at: point), let n = hit.value as? Int { return .link(n) }
        return nil
    }

    /// The box of the run's glyphs, from its font's ascent and descent (what SwiftUI's run bounds
    /// were, which `TooltipOverlay` was placed against), in the view's coordinates.
    func glyphRects(for range: NSRange) -> [CGRect] {
        guard range.length > 0 else { return [] }
        let attrs = attributed.attributes(at: range.location, effectiveRange: nil)
        let font = attrs[.font] as? NSFont ?? .systemFont(ofSize: MarkdownText.bodySize)
        let lift = attrs[TextKey.lift] as? CGFloat ?? 0
        return segments(for: range).map { segment in
            let baseline = segment.rect.minY + segment.baseline - lift
            return CGRect(x: segment.rect.minX, y: baseline - font.ascender, width: segment.rect.width, height: font.ascender - font.descender)
        }
    }

    /// Every tipped run and where it is.
    var tipFrames: [TipFrame] {
        var out: [TipFrame] = []
        attributed.enumerateAttribute(TextKey.tip, in: NSRange(location: 0, length: length)) { value, range, _ in
            guard let tip = value as? String else { return }
            out += glyphRects(for: range).map { TipFrame(rect: $0, tip: tip) }
        }
        return out
    }

    func tip(at point: CGPoint) -> TipFrame? {
        guard let hit = runHit(TextKey.tip, at: point), let text = hit.value as? String else { return nil }
        let rects = rects(for: hit.range)
        let line = rects.firstIndex { $0.contains(point) } ?? 0
        let glyphs = glyphRects(for: hit.range)
        return glyphs.indices.contains(line) ? TipFrame(rect: glyphs[line], tip: text) : nil
    }

    // MARK: Drawing

    /// The system's selected-text background while the window is key and one of the coordinator's
    /// views is first responder; unemphasized otherwise.
    private var selectionColor: NSColor {
        let focused = selection.owns(window?.firstResponder)
        return window?.isKeyWindow == true && focused ? .selectedTextBackgroundColor : .unemphasizedSelectedTextBackgroundColor
    }

    override func draw(_ dirtyRect: NSRect) {
        guard let context = NSGraphicsContext.current?.cgContext else { return }
        fitBounds()
        if let background = chrome?.background {
            background.setFill()
            bounds.fill()
        }
        if let selected = selection.range(for: self) {
            // The selection colour skips covered runs, as the official client's does.
            let hidden = hiddenRects()
            context.saveGState()
            if !hidden.isEmpty { clipOutside(hidden, in: context) }
            context.setFillColor(selectionColor.cgColor)
            rects(for: selected, type: .selection).forEach { context.fill($0) }
            context.restoreGState()
        }
        context.saveGState()
        context.translateBy(x: containerOrigin.x, y: containerOrigin.y)
        layoutManager.enumerateTextLayoutFragments(from: layoutManager.documentRange.location, options: [.ensuresLayout]) { fragment in
            let frame = fragment.layoutFragmentFrame
            if frame.maxY + containerOrigin.y < dirtyRect.minY { return true }
            if frame.minY + containerOrigin.y > dirtyRect.maxY { return false }
            fragment.draw(at: frame.origin, in: context)
            return true
        }
        context.restoreGState()
    }

    private func hiddenRects() -> [CGRect] {
        var out: [CGRect] = []
        attributed.enumerateAttribute(TextKey.hidden, in: NSRange(location: 0, length: length)) { value, range, _ in
            if value != nil { out += rects(for: range) }
        }
        return out
    }

    override func becomeFirstResponder() -> Bool { selection.repaint(); return true }
    override func resignFirstResponder() -> Bool {
        selection.repaint()
        // After the window has settled on its next responder (or none, when this view is leaving it).
        DispatchQueue.main.async { [weak selection, weak window] in
            MainActor.assumeIsolated { selection?.reclaimResponder(in: window) }
        }
        return true
    }

    // MARK: Mouse

    /// Every press places the selection's anchor; a link or a hidden spoiler activates only on a
    /// single click whose press and release fall in the same run's segment frames, with no drag
    /// past `DesignTokens.TextView.dragThreshold` (N1 Links). The window has already made this view
    /// first responder for the press, as `NSWindow` does for a view that accepts it.
    override func mouseDown(with event: NSEvent) {
        // A Control-click is a right-click: it opens the menu and leaves the selection alone.
        if event.modifierFlags.contains(.control) { _ = menu(for: event); return }
        let point = convert(event.locationInWindow, from: nil)
        press = (point, activation(at: point), event.clickCount == 1)
        dragged = false
        selection.begin(in: self, at: index(at: point), windowPoint: event.locationInWindow)
    }

    override func mouseDragged(with event: NSEvent) {
        guard let press else { return }
        let point = convert(event.locationInWindow, from: nil)
        if hypot(point.x - press.point.x, point.y - press.point.y) > DesignTokens.TextView.dragThreshold { dragged = true }
        if dragged { selection.drag(to: event.locationInWindow) }
    }

    override func mouseUp(with event: NSEvent) {
        defer { press = nil }
        selection.finish()
        guard let press, let target = press.activation, press.single, event.clickCount == 1, !dragged,
              activation(at: convert(event.locationInWindow, from: nil)) == target else { return }
        onActivate?(target)
    }

    override func mouseMoved(with event: NSEvent) {
        let point = convert(event.locationInWindow, from: nil)
        pointer = point
        let next = tip(at: point)
        if next != shownTip { shownTip = next; onTip?(next) }
    }

    override func mouseExited(with event: NSEvent) {
        pointer = nil
        if shownTip != nil { shownTip = nil; onTip?(nil) }
    }

    // MARK: Copy, and the defaults it does not have

    /// ⌘C: the coordinator's selection, as plain text only (D4).
    @objc func copy(_ sender: Any?) { selection.copy(to: pasteboard) }

    @objc func validateUserInterfaceItem(_ item: any NSValidatedUserInterfaceItem) -> Bool {
        item.action == #selector(copy(_:)) && selection.hasSelection
    }

    override func validRequestor(forSendType sendType: NSPasteboard.PasteboardType?, returnType: NSPasteboard.PasteboardType?) -> Any? { nil }
    override func quickLook(with event: NSEvent) {}

    // MARK: Message menu (W1)

    override func rightMouseDown(with event: NSEvent) { _ = menu(for: event) }

    /// Opens the row's message menu for the press; AppKit's own menu never shows (nil).
    override func menu(for event: NSEvent) -> NSMenu? {
        guard event !== menuEvent else { return nil }
        menuEvent = event
        presentMenu(at: convert(event.locationInWindow, from: nil))
        return nil
    }

    /// VoiceOver's Show Menu: no pointer, so no link items; the menu opens at the view's corner.
    override func accessibilityPerformShowMenu() -> Bool {
        guard openMenu != nil else { return false }
        presentMenu(at: nil)
        return true
    }

    /// The row's menu for a press at `point` (view coordinates). Link items only when the point lies
    /// inside a link run's segment frames (S-13), never over a covered run, which carries only its
    /// spoiler index; Copy only while this view's selection covers something.
    func presentMenu(at point: CGPoint?) {
        guard let openMenu else { return }
        let local = point ?? bounds.origin
        let screen = window.map { $0.convertPoint(toScreen: convert(local, to: nil)) } ?? NSEvent.mouseLocation
        let coordinator = selection, board = pasteboard
        openMenu(MessageMenuRequest(
            point: screen, window: window, link: point.flatMap(menuLink(at:)),
            copySelection: coordinator.hasSelection ? { coordinator.copy(to: board) } : nil
        ))
    }

    /// The link whose segment frames contain `point`: its index, the target as the owning router's
    /// body has it now (captured), that router, and the body the router has when asked later.
    func menuLink(at point: CGPoint) -> MessageMenuLink? {
        guard case .link(let n) = activation(at: point), let router = linkRouter, router.body.links.indices.contains(n) else { return nil }
        return (n, router.body.links[n], router, { [weak self] in MainActor.assumeIsolated { self?.linkRouter?.body } })
    }

    // MARK: Accessibility (N0-10): only what is defined here is exposed.

    override func isAccessibilityElement() -> Bool { isAccessible }
    override func accessibilityRole() -> NSAccessibility.Role? { .staticText }
    /// `NSAccessibilityStaticText`'s value, narrowing NSView's `Any?`.
    func accessibilityValue() -> String? { spoken }
    override func accessibilityString(for range: NSRange) -> String? { nil }
    override func accessibilityAttributedString(for range: NSRange) -> NSAttributedString? { nil }
}

/// A static text element only: none of NSTextInputClient, NSServicesMenuRequestor,
/// NSTextFinderClient or NSAccessibilityNavigableStaticText (N1 View, F-6).
extension MessageTextView: @preconcurrency NSAccessibilityStaticText {}

/// Clips `context` to everything outside `holes`. The holes are united first: TextKit gives a run
/// that ends the text its last rect twice (measured by the N1 verifier on `a ||secret||`), and an
/// even-odd path pairs such duplicates up and clips nothing.
private func clipOutside(_ holes: [CGRect], in context: CGContext) {
    let union = CGMutablePath()
    holes.forEach { union.addRect($0) }
    let plane = CGPath(rect: CGRect(x: -1e5, y: -1e5, width: 2e5, height: 2e5), transform: nil)
    context.addPath(plane.subtracting(union, using: .winding))
    context.clip()
}

// MARK: Fragments

/// Hands out `TextFragment`s; one per layout manager, kept alive by its view.
final class TextFragmentProvider: NSObject, NSTextLayoutManagerDelegate {
    var clipHidden = true

    func textLayoutManager(_ textLayoutManager: NSTextLayoutManager, textLayoutFragmentFor location: NSTextLocation, in textElement: NSTextElement) -> NSTextLayoutFragment {
        TextFragment(textElement: textElement, range: textElement.elementRange)
    }
}

/// A paragraph that never draws its hidden glyphs: it clips each hidden run's segment rects out of
/// the glyph drawing, then fills the cover there (S-1 layer 1; covers come from the fragment's own
/// current text at draw time, S-10). It also draws inline code's rounded fill and border, and code
/// line numbers.
final class TextFragment: NSTextLayoutFragment {
    private var paragraph: NSAttributedString? { (textElement as? NSTextParagraph)?.attributedString }

    private var lift: CGFloat {
        guard let text = paragraph, text.length > 0 else { return 0 }
        return text.attribute(TextKey.lift, at: 0, effectiveRange: nil) as? CGFloat ?? 0
    }

    /// Segment rects (container coordinates, with baselines) of the runs carrying `key`, with their fonts.
    private func runs(_ key: NSAttributedString.Key) -> [(rect: CGRect, baseline: CGFloat, font: NSFont?)] {
        guard let tlm = textLayoutManager, let content = tlm.textContentManager, let text = paragraph else { return [] }
        var out: [(CGRect, CGFloat, NSFont?)] = []
        text.enumerateAttribute(key, in: NSRange(location: 0, length: text.length)) { value, run, _ in
            guard value != nil,
                  let start = content.location(rangeInElement.location, offsetBy: run.location),
                  let end = content.location(start, offsetBy: run.length),
                  let range = NSTextRange(location: start, end: end) else { return }
            let font = text.attribute(.font, at: run.location, effectiveRange: nil) as? NSFont
            tlm.enumerateTextSegments(in: range, type: .standard, options: []) { _, frame, baseline, _ in
                out.append((frame, baseline, font))
                return true
            }
        }
        return out
    }

    /// Wide enough for the line numbers that sit left of a code row's indented frame.
    override var renderingSurfaceBounds: CGRect {
        let base = super.renderingSurfaceBounds
        guard let text = paragraph, text.length > 0, text.attribute(TextKey.marker, at: 0, effectiveRange: nil) != nil else { return base }
        return base.union(CGRect(x: -layoutFragmentFrame.origin.x, y: 0, width: layoutFragmentFrame.origin.x, height: layoutFragmentFrame.height))
    }

    override func draw(at point: CGPoint, in context: CGContext) {
        let shift = CGPoint(x: point.x - layoutFragmentFrame.origin.x, y: point.y - layoutFragmentFrame.origin.y)
        let hidden = runs(TextKey.hidden).map { $0.rect.offsetBy(dx: shift.x, dy: shift.y) }
        drawInlineCode(shift: shift, in: context)
        context.saveGState()
        context.translateBy(x: 0, y: -lift)
        if (textLayoutManager?.delegate as? TextFragmentProvider)?.clipHidden ?? true, !hidden.isEmpty {
            clipOutside(hidden, in: context)
        }
        super.draw(at: point, in: context)
        context.restoreGState()
        // One path, one fill: a pixel under two of the rects is covered once, not darker.
        let covers = CGMutablePath()
        hidden.forEach { covers.addRoundedRect(in: $0, cornerWidth: MarkdownPalette.spoilerRadius, cornerHeight: MarkdownPalette.spoilerRadius) }
        context.addPath(covers)
        context.setFillColor(NSColor(MarkdownPalette.spoilerCover).cgColor)
        context.fillPath()
        drawMarker(at: point, in: context)
    }

    private func drawInlineCode(shift: CGPoint, in context: CGContext) {
        typealias V = DesignTokens.TextView
        for run in runs(TextKey.inlineCode) {
            guard let font = run.font else { continue }
            let baseline = run.rect.minY + run.baseline - lift + shift.y
            let box = CGRect(x: run.rect.minX + shift.x, y: baseline - font.ascender, width: run.rect.width, height: font.ascender - font.descender)
                .insetBy(dx: V.inlineCodeBorderWidth / 2, dy: V.inlineCodeBorderWidth / 2)
            let path = CGPath(roundedRect: box, cornerWidth: V.inlineCodeRadius, cornerHeight: V.inlineCodeRadius, transform: nil)
            context.addPath(path)
            context.setFillColor(NSColor(MarkdownPalette.codeFill).cgColor)
            context.fillPath()
            context.addPath(path)
            context.setStrokeColor(V.inlineCodeBorder.cgColor)
            context.setLineWidth(V.inlineCodeBorderWidth)
            context.strokePath()
        }
    }

    /// A code row's line number, beside its first line, from the paragraph's first character.
    private func drawMarker(at point: CGPoint, in context: CGContext) {
        guard let text = paragraph, text.length > 0, let first = textLineFragments.first,
              let marker = text.attribute(TextKey.marker, at: 0, effectiveRange: nil) as? TextMarker else { return }
        let baseline = point.y + first.typographicBounds.minY + first.glyphOrigin.y - lift
        let string = NSAttributedString(string: marker.text, attributes: [.font: marker.font, .foregroundColor: marker.color])
        NSGraphicsContext.saveGraphicsState()
        NSGraphicsContext.current = NSGraphicsContext(cgContext: context, flipped: true)
        string.draw(at: CGPoint(x: point.x - layoutFragmentFrame.origin.x + marker.x, y: baseline - marker.font.ascender.rounded(.up)))
        NSGraphicsContext.restoreGraphicsState()
    }
}
