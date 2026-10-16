import AppKit

// The selection over the message column (docs/plans/selection.md, N2): one coordinator per column
// (or per `MarkdownView` outside one, as in N1). It holds the selection as model positions (message
// id, part, UTF-16 offset), not as views, so rows can leave and re-enter the lazy stack mid-drag
// and the copy still reaches rows no view shows. It tracks the drag itself (autoscroll, and the
// gesture when the view that took the press has been recycled) and writes the copy.

/// One selectable part: a message's header, or one of its body's text views.
struct PartID: Hashable {
    var message: String
    var part: Int
}

extension PartID {
    /// The header's number: before every body part of its message.
    static let headerPart = -1
}

/// One row of the column's model, in display order.
struct SelectionRow {
    var message: MessageItem
    /// The row shows its header (and so has a header part).
    var startsGroup: Bool
}

@MainActor
final class SelectionCoordinator {
    /// A selection end. `snapshot` is the part's text when the end was placed: in a column the offset
    /// means nothing in any other string, so an end whose part now shows different text is dropped
    /// (a relative-time refresh or a later name resolution counts).
    struct Position {
        var message: String
        var part: Int
        var offset: Int
        var snapshot: NSAttributedString
    }

    private final class Weak { weak var view: MessageTextView?; init(_ view: MessageTextView) { self.view = view } }
    private var views: [PartID: Weak] = [:]
    private(set) var anchor: Position?
    private(set) var focus: Position?
    private let feedback = CopyFeedback()

    // MARK: Model

    private(set) var rows: [SelectionRow] = []
    private var rank: [String: Int] = [:]
    var locale = Locale(identifier: "en-US")

    /// The column's rows, in order. A selection end in a message that left (deleted, or a switch
    /// emptied the rows) moves to its nearest surviving neighbour: the start of the next message, else
    /// the end of the previous one, so nothing dangles; with none left the selection clears. An edit
    /// changes the part's text, which `slice` notices by itself.
    func setRows(_ new: [SelectionRow]) {
        let old = rows
        rows = new
        rank = Dictionary(new.enumerated().map { ($1.message.id, $0) }, uniquingKeysWith: { first, _ in first })
        guard !old.isEmpty else {
            if let a = anchor, rank[a.message] == nil { clear() }
            return
        }
        let oldRank = Dictionary(old.enumerated().map { ($1.message.id, $0) }, uniquingKeysWith: { first, _ in first })
        func repaired(_ p: Position) -> Position? {
            guard rank[p.message] == nil else { return p }
            guard let at = oldRank[p.message] else { return nil }
            if let next = old[at...].first(where: { rank[$0.message.id] != nil }) {
                return Position(message: next.message.id, part: .min, offset: 0, snapshot: NSAttributedString())
            }
            if let previous = old[..<at].last(where: { rank[$0.message.id] != nil }) {
                return Position(message: previous.message.id, part: .max, offset: 0, snapshot: NSAttributedString())
            }
            return nil
        }
        guard let a = anchor, let f = focus else { return }
        if let a2 = repaired(a), let f2 = repaired(f) {
            anchor = a2
            focus = f2
            repaint()
        } else {
            clear()
        }
    }

    /// A message's place, or nil for one the column does not list. Without a model (a body's own
    /// coordinator) every message is the same one.
    private func place(_ message: String) -> Int? { rows.isEmpty ? 0 : rank[message] }

    // MARK: Views

    func attach(_ view: MessageTextView) {
        guard views[view.partID]?.view !== view else { return }
        views[view.partID] = Weak(view)
        view.needsDisplay = true
    }

    func detach(_ view: MessageTextView) {
        // A newer view may already hold the part (SwiftUI makes before it dismantles).
        if let held = views[view.partID], held.view === view || held.view == nil { views[view.partID] = nil }
    }

    /// A text view resigned: when that left the window with no first responder at all (its row was
    /// recycled or deleted), the column's own takes the place, so ⌘C still reaches the selection. A
    /// focus that moved to another control stays there.
    func reclaimResponder(in window: NSWindow?) {
        guard hasSelection, let responder, let window, responder.window === window, window.firstResponder === window else { return }
        window.makeFirstResponder(responder)
    }

    /// The column's own first responder (`SelectionResponderView`): ⌘C still reaches the selection
    /// after every text view it was made in has been recycled.
    weak var responder: NSView?

    /// Whether `object` is a view that makes this selection's colour the focused one.
    func owns(_ object: NSResponder?) -> Bool {
        if let object, object === responder { return true }
        return (object as? MessageTextView)?.selection === self
    }

    // MARK: Gesture

    /// Only one selection shows at a time: a press in another body clears this one.
    private static weak var active: SelectionCoordinator?
    private(set) var dragging = false
    private var press: CGPoint?
    private var moved = false
    private var lastPoint: CGPoint?
    private weak var scroller: NSScrollView?
    private(set) var timer: Timer?
    private var monitor: Any?
    /// How a monitor is installed and removed; tests swap these to count the installed ones.
    static var addMonitor: (@escaping (NSEvent) -> Void) -> Any? = { handler in
        NSEvent.addLocalMonitorForEvents(matching: [.leftMouseDown, .leftMouseDragged, .leftMouseUp]) { event in
            handler(event)
            return event
        }
    }
    static var removeMonitor: (Any) -> Void = { NSEvent.removeMonitor($0) }
    /// The selection's colour follows the window's key state: repaint when it changes, while there is one.
    private var keyObservers: [NSObjectProtocol] = []
    /// Points scrolled per autoscroll step while the pointer is past the column's edge.
    var autoscrollStep: CGFloat = 40
    private(set) var autoscrolls = 0

    func begin(in view: MessageTextView, at offset: Int, windowPoint: CGPoint) {
        if Self.active !== self { Self.active?.clear() }
        Self.active = self
        if views[view.partID]?.view !== view { attach(view) }
        guard place(view.message) != nil else { return clear() }
        anchor = Position(message: view.message, part: view.participant, offset: offset, snapshot: NSAttributedString(attributedString: view.attributed))
        focus = anchor
        dragging = true
        press = windowPoint
        lastPoint = windowPoint
        moved = false
        scroller = view.enclosingScrollView ?? scroller
        if keyObservers.isEmpty, let window = view.window {
            keyObservers = [NSWindow.didBecomeKeyNotification, NSWindow.didResignKeyNotification].map { name in
                NotificationCenter.default.addObserver(forName: name, object: window, queue: .main) { [weak self] _ in
                    MainActor.assumeIsolated { self?.repaint() }
                }
            }
        }
        if monitor == nil {
            // The gesture belongs to the coordinator, not to the view that took the press: that view
            // can be recycled while the pointer is past the edge, and AppKit then drops the drag.
            // The same monitor clears the selection on a press anywhere but in its own text.
            monitor = Self.addMonitor { [weak self] event in
                MainActor.assumeIsolated { self?.handle(event) }
            }
        }
        timer?.invalidate()
        timer = Timer.scheduledTimer(withTimeInterval: 0.05, repeats: true) { [weak self] _ in
            MainActor.assumeIsolated { self?.tick() }
        }
        repaint()
    }

    /// What the monitor does with an event (tests call it as the monitor would).
    func handle(_ event: NSEvent) {
        switch event.type {
        case .leftMouseDragged: drag(to: event.locationInWindow)
        case .leftMouseUp: finish()
        case .leftMouseDown:
            // A Control-click opens the message menu, whose Copy acts on this selection (W1); a
            // right- or other-button press never reaches here (the monitor's mask, and `default`).
            guard !event.modifierFlags.contains(.control) else { return }
            // A press in this selection's own text places the next one (`begin`); anywhere else clears.
            guard let content = (event.window ?? views.values.compactMap { $0.view?.window }.first)?.contentView else { return }
            let hit = content.hitTest(content.superview?.convert(event.locationInWindow, from: nil) ?? event.locationInWindow)
            if (hit as? MessageTextView)?.selection !== self { clear() }
        default: break
        }
    }

    /// The drag has reached `windowPoint` (window coordinates): scrolls when it is past the
    /// column's edge, then moves the selection's far end to the part under it.
    func drag(to windowPoint: CGPoint) {
        guard dragging, anchor != nil else { return }
        lastPoint = windowPoint
        if !moved, let press, hypot(windowPoint.x - press.x, windowPoint.y - press.y) <= DesignTokens.TextView.dragThreshold { return }
        moved = true
        tick()
    }

    /// One step of the drag: autoscroll, then resolve the pointer again (the rows moved under it).
    func tick() {
        guard dragging, moved, let point = lastPoint else { return }
        autoscroll(for: point)
        if let position = resolve(point) {
            focus = position
            repaint()
        }
    }

    /// The press is released. ⌘C needs a responder: when the view the press began in is gone, the
    /// column's own takes the place.
    func finish() {
        dragging = false
        timer?.invalidate()
        timer = nil
        // A click that selected nothing has no use for the monitor.
        if !hasSelection { clear() }
        guard hasSelection, let responder, let window = responder.window, !owns(window.firstResponder) else { return }
        window.makeFirstResponder(responder)
    }

    func clear() {
        anchor = nil
        focus = nil
        dragging = false
        releaseResources()
        repaint()
    }

    /// The timer, the monitor and the key observers: alive only while there is a selection.
    private func releaseResources() {
        timer?.invalidate()
        timer = nil
        if let monitor { Self.removeMonitor(monitor) }
        monitor = nil
        keyObservers.forEach { NotificationCenter.default.removeObserver($0) }
        keyObservers = []
    }

    isolated deinit { releaseResources() }

    func repaint() { views.values.forEach { $0.view?.needsDisplay = true } }

    private func autoscroll(for point: CGPoint) {
        guard let scroll = scroller, let document = scroll.documentView else { return }
        let clip = scroll.contentView
        let local = clip.convert(point, from: nil)
        let insets = scroll.contentInsets
        // The column's edges, in the clip's coordinates: its bounds less what the toolbar and the composer cover.
        let top = clip.isFlipped ? clip.bounds.minY + insets.top : clip.bounds.maxY - insets.top
        let bottom = clip.isFlipped ? clip.bounds.maxY - insets.bottom : clip.bounds.minY + insets.bottom
        let beyondTop = clip.isFlipped ? top - local.y : local.y - top
        let beyondBottom = clip.isFlipped ? local.y - bottom : bottom - local.y
        var delta: CGFloat = 0
        if beyondTop > 0 { delta = -min(autoscrollStep, beyondTop + 8) } else if beyondBottom > 0 { delta = min(autoscrollStep, beyondBottom + 8) }
        guard delta != 0 else { return }
        let sign: CGFloat = clip.isFlipped ? 1 : -1
        let low = document.frame.minY - insets.top, high = max(document.frame.maxY - clip.bounds.height + insets.bottom, low)
        let y = min(max(clip.bounds.origin.y + delta * sign, low), high)
        guard y != clip.bounds.origin.y else { return }
        clip.scroll(to: NSPoint(x: clip.bounds.origin.x, y: y))
        scroll.reflectScrolledClipView(clip)
        autoscrolls += 1
        // Rows enter and leave the lazy stack on layout.
        scroll.window?.contentView?.layoutSubtreeIfNeeded()
    }

    // MARK: Ranges

    private func key(_ p: Position) -> (Int, Int)? { place(p.message).map { ($0, p.part) } }

    private func before(_ a: (Int, Int), _ b: (Int, Int)) -> Bool { a.0 != b.0 ? a.0 < b.0 : a.1 < b.1 }

    private var ordered: (start: Position, end: Position, startKey: (Int, Int), endKey: (Int, Int))? {
        guard let anchor, let focus, let a = key(anchor), let f = key(focus) else { return nil }
        let swap = before(f, a) || (a == f && focus.offset < anchor.offset)
        return swap ? (focus, anchor, f, a) : (anchor, focus, a, f)
    }

    /// The part of a part's text the selection covers, or nil: the part is outside it, or is an end
    /// whose text is no longer the one the end was placed in (dropped). Snapped to composed characters.
    private func slice(of text: NSAttributedString, at place: (Int, Int)) -> NSRange? {
        guard let (start, end, startKey, endKey) = ordered, !before(place, startKey), !before(endKey, place) else { return nil }
        let length = text.length
        var from = 0, to = length
        // A column drops an end whose text changed; a body's own coordinator (no model) keeps N1's
        // clamp and snap, which the offsets get here either way.
        let drops = !rows.isEmpty
        if place == startKey {
            guard !drops || start.snapshot.string == text.string else { return nil }
            from = min(start.offset, length)
        }
        if place == endKey {
            guard !drops || end.snapshot.string == text.string else { return nil }
            to = min(end.offset, length)
        }
        guard to > from else { return nil }
        let snapped = (text.string as NSString).rangeOfComposedCharacterSequences(for: NSRange(location: from, length: to - from))
        return snapped.length > 0 ? snapped : nil
    }

    /// The part of `view`'s text the selection covers.
    func range(for view: MessageTextView) -> NSRange? {
        guard views[view.partID]?.view === view, let p = place(view.message) else { return nil }
        return slice(of: view.attributed, at: (p, view.participant))
    }

    /// The selection's displayed text (D2, D3): each part's slice, one newline between parts. The
    /// parts come from the views that show them, else from the model, so rows scrolled out of the
    /// lazy stack still count; an end whose view is gone slices the text it was placed in, when the
    /// model still shows the same text there.
    var selectedText: String {
        guard let (start, end, startKey, endKey) = ordered else { return "" }
        var out: [String] = []
        var live: [Int: [Int: NSAttributedString]] = [:]
        for (id, held) in views {
            guard let view = held.view, let p = place(id.message) else { continue }
            live[p, default: [:]][id.part] = view.attributed
        }
        let span = rows.isEmpty ? [0] : Array(startKey.0...endKey.0)
        for p in span {
            var parts = live[p] ?? [:]
            if !rows.isEmpty {
                // ponytail: model parts are rebuilt for every row of the span on each copy; cache them
                // by message if a copy over thousands of rows is ever slow.
                for (number, text) in MessageParts.parts(of: rows[p], locale: locale) where parts[number] == nil {
                    // A model text stands in for a part no view shows. An end slices the text it was placed
                    // in (the model's may differ in an attachment), and only while the model still shows it.
                    if (p, number) == startKey {
                        if MessageParts.same(text, start.snapshot) { parts[number] = start.snapshot }
                    } else if (p, number) == endKey {
                        if MessageParts.same(text, end.snapshot) { parts[number] = end.snapshot }
                    } else {
                        parts[number] = text
                    }
                }
            }
            for number in parts.keys.sorted() {
                guard let text = parts[number], let range = slice(of: text, at: (p, number)) else { continue }
                out.append(MessageTextContent.copied(text, in: range))
            }
        }
        return out.joined(separator: "\n")
    }

    /// Whether the selection covers anything: its ends differ. (A drop of every part it covers
    /// leaves a selection that copies nothing, which `copy` leaves alone.)
    var hasSelection: Bool {
        guard let a = anchor, let f = focus else { return false }
        return a.message != f.message || a.part != f.part || a.offset != f.offset
    }

    /// Through the one plain-text writer (D4): clear, then one string.
    func copy(to pasteboard: NSPasteboard) {
        let text = selectedText
        guard !text.isEmpty else { return }
        feedback.copy(text, to: pasteboard)
    }

    /// The part under `point` (window coordinates), else the nearest one vertically: its start when
    /// the point is below it, its end when above (the window's y grows upward).
    private func resolve(_ point: CGPoint) -> Position? {
        var best: (view: MessageTextView, place: (Int, Int), distance: CGFloat, offset: Int)?
        for (id, held) in views {
            guard let view = held.view, view.window != nil, !view.isHiddenOrHasHiddenAncestor, let p = place(id.message) else { continue }
            let frame = view.convert(view.bounds, to: nil)
            let distance = point.y > frame.maxY ? point.y - frame.maxY : (point.y < frame.minY ? frame.minY - point.y : 0)
            let offset = point.y > frame.maxY ? 0 : (point.y < frame.minY ? view.length : view.index(at: view.convert(point, from: nil)))
            if best == nil || distance < best!.distance || (distance == best!.distance && before((p, id.part), best!.place)) {
                best = (view, (p, id.part), distance, offset)
            }
        }
        return best.map {
            Position(message: $0.view.message, part: $0.view.participant, offset: $0.offset, snapshot: NSAttributedString(attributedString: $0.view.attributed))
        }
    }
}
