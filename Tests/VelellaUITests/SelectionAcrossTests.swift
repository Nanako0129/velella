import AppKit
import Observation
import SwiftUI
import Testing
@testable import VelellaUI

// Selection across messages (docs/plans/selection.md, N2 Tests): N0-16's cases against the real
// message column (`MessageArea`) in an offscreen window that is never ordered in; edits, deletes
// and a relative-time refresh under a selection; the header participant's accessibility. Every copy
// goes to an injected pasteboard, and `NSPasteboard.general` is checked not to move.

@MainActor @Observable private final class Model {
    var messages: [MessageItem]
    init(_ messages: [MessageItem]) { self.messages = messages }
}

private struct Host: View {
    let model: Model
    let board: NSPasteboard
    var body: some View {
        MessageArea(
            header: ChannelHeader(name: "general"), conversationID: "1", messages: model.messages, load: .loaded,
            reply: .constant(nil), draft: .constant("")
        )
        .environment(\.textPasteboard, PasteboardBox(board: board))
    }
}

private let locale = Locale(identifier: "en-US")
/// January 2025: far from any test clock, so the header time is the older-day form on every run.
private let epoch = Date(timeIntervalSince1970: 1_735_732_800)

/// Message `n`: its own author, so every row has a header.
private func message(_ n: Int, text: String? = nil, body: MessageBody? = nil) -> MessageItem {
    MessageItem(
        id: "m\(n)", authorId: "a\(n)", authorName: "author\(n)", avatar: .initials("A"),
        timestamp: epoch.addingTimeInterval(Double(n) * 3600), text: text ?? "message \(n) body", body: body
    )
}

private func time(_ n: Int) -> String {
    DiscordTime.header(date: epoch.addingTimeInterval(Double(n) * 3600), now: .now, locale: locale)
}

/// What a copy emits for message `n`'s header: `name` then ` — time` on two lines (D3).
private func header(_ n: Int) -> String { "author\(n)\n — " + time(n) }

private func column(_ count: Int) -> [MessageItem] { (1...count).map { message($0) } }

/// Whether `s` holds a replacement character, which is what a lone surrogate becomes in a Swift string.
private func hasLoneSurrogate(_ s: String) -> Bool { s.unicodeScalars.contains("\u{FFFD}") }

@MainActor private final class Rig {
    let model: Model
    let board = TextHarness.board()
    let window: TextWindow
    let host: NSHostingView<Host>
    let scroll: NSScrollView

    init(_ messages: [MessageItem], height: CGFloat = 500, key: Bool = true) async throws {
        model = Model(messages)
        window = TextWindow(width: 700, height: height)
        window.keyState = key
        host = NSHostingView(rootView: Host(model: model, board: board))
        window.contentView = host
        func find(_ view: NSView) -> NSScrollView? { (view as? NSScrollView) ?? view.subviews.lazy.compactMap(find).first }
        for _ in 0..<6 {
            host.layoutSubtreeIfNeeded()
            try? await Task.sleep(for: .milliseconds(40))
        }
        scroll = try #require(find(host))
    }

    func settle() async {
        for _ in 0..<4 {
            host.layoutSubtreeIfNeeded()
            try? await Task.sleep(for: .milliseconds(30))
        }
    }

    var views: [MessageTextView] { TextHarness.textViews(in: host) }

    func body(_ n: Int, _ part: Int = 0) -> MessageTextView? { views.first { $0.message == "m\(n)" && $0.participant == part } }
    func header(_ n: Int) -> MessageTextView? { body(n, PartID.headerPart) }

    var coordinator: SelectionCoordinator? { views.first?.selection }

    /// A press in `view` at character `offset`, a drag to `target` at `end`, and the release.
    func select(from view: MessageTextView, _ offset: Int, to target: MessageTextView, _ end: Int) {
        let a = view.rects(for: NSRange(location: offset, length: 1))[0]
        let b = target.rects(for: NSRange(location: end, length: 1))[0]
        TextHarness.send(TextHarness.mouse(.leftMouseDown, at: CGPoint(x: a.minX + 0.5, y: a.midY), in: view), to: window)
        TextHarness.send(TextHarness.mouse(.leftMouseDragged, at: CGPoint(x: b.minX + 0.5, y: b.midY), in: target), to: window)
        TextHarness.send(TextHarness.mouse(.leftMouseUp, at: CGPoint(x: b.minX + 0.5, y: b.midY), in: target), to: window)
    }

    /// ⌘C as the responder chain delivers it: `copy:` to the first responder.
    func copy() -> String? {
        board.clearContents()
        #expect(window.firstResponder?.tryToPerform(#selector(MessageTextView.copy(_:)), with: nil) == true, "nothing handles ⌘C")
        return board.string(forType: .string)
    }

    /// An event at a window point, as the coordinator's monitor receives it.
    func event(_ type: NSEvent.EventType, at point: CGPoint) -> NSEvent {
        NSEvent.mouseEvent(
            with: type, location: point, modifierFlags: [], timestamp: ProcessInfo.processInfo.systemUptime,
            windowNumber: window.windowNumber, context: nil, eventNumber: 900, clickCount: 1, pressure: 1)!
    }
}

@MainActor @Suite(.serialized) struct SelectionAcrossTests {
    private func threeMessages() -> [MessageItem] { [message(1, text: "alpha bravo"), message(2, text: "delta echo"), message(3, text: "golf hotel")] }

    // MARK: N0-16: body to body, with headers

    @Test func aDragFromOneBodyToTheNextCopiesTheTailTheHeaderAndTheHead() async throws {
        let before = NSPasteboard.general.changeCount
        let rig = try await Rig([message(1, text: "alpha bravo charlie"), message(2, text: "delta echo foxtrot")])
        let a = try #require(rig.body(1)), b = try #require(rig.body(2))
        rig.select(from: a, 6, to: b, 5)
        #expect(rig.window.firstResponder === a)
        #expect(rig.copy() == "bravo charlie\n" + header(2) + "\ndelta")
        #expect(NSPasteboard.general.changeCount == before)
    }

    @Test func aSelectionAcrossThreeMessagesTakesEveryHeaderAndBodyBetween() async throws {
        let rig = try await Rig(threeMessages())
        rig.select(from: try #require(rig.body(1)), 6, to: try #require(rig.body(3)), 4)
        #expect(rig.copy() == "bravo\n" + header(2) + "\ndelta echo\n" + header(3) + "\ngolf")
    }

    @Test func aDragUpwardCopiesTheSameText() async throws {
        let rig = try await Rig([message(1, text: "alpha bravo"), message(2, text: "delta echo")])
        rig.select(from: try #require(rig.body(2)), 5, to: try #require(rig.body(1)), 6)
        #expect(rig.copy() == "bravo\n" + header(2) + "\ndelta")
    }

    @Test func aSelectionMayStartInAHeader() async throws {
        let rig = try await Rig([message(1, text: "alpha bravo"), message(2, text: "delta echo")])
        let h = try #require(rig.header(1))
        // The header view lays the name out first, then the gap and the time.
        #expect(h.attributed.string.hasPrefix("author1"))
        rig.select(from: h, 3, to: try #require(rig.body(1)), 5)
        #expect(rig.copy() == "hor1\n — " + time(1) + "\nalpha")
    }

    @Test func aFollowUpRowHasNoHeaderToCopy() async throws {
        let one = MessageItem(id: "m1", authorId: "a", authorName: "author1", avatar: .initials("A"), timestamp: epoch, text: "first line")
        let two = MessageItem(id: "m2", authorId: "a", authorName: "author1", avatar: .initials("A"), timestamp: epoch.addingTimeInterval(60), text: "second line")
        let rig = try await Rig([one, two])
        #expect(rig.header(2) == nil)
        rig.select(from: try #require(rig.body(1)), 6, to: try #require(rig.body(2)), 6)
        #expect(rig.copy() == "line\nsecond")
    }

    @Test func theHeaderParticipantTakesPartInHitTestingAndIsNotAnAccessibilityElement() async throws {
        NSApplication.shared.accessibilitySetValue(true, forAttribute: NSAccessibility.Attribute(rawValue: "AXEnhancedUserInterface"))
        let rig = try await Rig([message(1, text: "alpha bravo"), message(2, text: "delta echo")])
        let h = try #require(rig.header(1))
        #expect(!h.isAccessibilityElement())
        // A press on the name reaches the participant, not SwiftUI's text above it.
        let centre = h.convert(CGPoint(x: h.rects(for: NSRange(location: 1, length: 1))[0].midX, y: h.bounds.midY), to: nil)
        let content = try #require(rig.window.contentView)
        let hit = content.hitTest(content.superview?.convert(centre, from: nil) ?? centre)
        #expect(hit === h, "\(String(describing: hit))")
        // The accessibility tree holds the body texts and no header participant.
        var seen: [MessageTextView] = []
        func walk(_ element: Any) {
            guard let object = element as? NSObject else { return }
            if let view = object as? MessageTextView { seen.append(view) }
            let children = (TextAX.send(object, "accessibilityChildren") as? [Any]) ?? []
            children.forEach(walk)
        }
        walk(rig.host)
        #expect(seen.contains { $0.participant == 0 }, "the walk found no text view at all")
        #expect(!seen.contains { $0.participant == PartID.headerPart })
    }

    @Test func theHeaderParticipantLaysTheNameAndTheTimeWhereTheSwiftUITextIs() async throws {
        let rig = try await Rig([message(1, text: "alpha bravo")])
        let h = try #require(rig.header(1))
        let name = "author1"
        func width(_ text: String, _ font: Font) -> CGFloat { NSHostingView(rootView: Text(verbatim: text).font(font).fixedSize()).fittingSize.width }
        let nameRect = h.rects(for: NSRange(location: 0, length: (name as NSString).length))
        let timeStart = (name as NSString).length + 1
        let timeRect = h.rects(for: NSRange(location: timeStart, length: (time(1) as NSString).length))
        // (TextKit gives a run that ends the text its last rect twice.)
        #expect(!nameRect.isEmpty && !timeRect.isEmpty, "\(nameRect) \(timeRect)")
        #expect(abs(nameRect[0].width - width(name, DesignTokens.Typography.authorName)) < 1, "name \(nameRect[0].width)")
        #expect(abs(timeRect[0].width - width(time(1), DesignTokens.Typography.timestamp)) < 1, "time \(timeRect[0].width)")
        // The time starts one gap after the name (the SwiftUI header's spacing).
        #expect(abs(timeRect[0].minX - (nameRect[0].maxX + DesignTokens.Message.headerGap)) < 1, "name \(nameRect) time \(timeRect)")
    }

    // MARK: N0-16: a drag past the column's edge until the origin is recycled

    @Test func anAutoscrollingDragSurvivesTheOriginBeingRecycledAndCopiesEveryRowBetween() async throws {
        let before = NSPasteboard.general.changeCount
        let rig = try await Rig(column(60))
        // Opened at the bottom: the origin is the last message's body.
        let origin = try #require(rig.body(60))
        let coordinator = origin.selection
        let a = origin.rects(for: NSRange(location: 8, length: 1))[0]
        TextHarness.send(TextHarness.mouse(.leftMouseDown, at: CGPoint(x: a.minX + 0.5, y: a.midY), in: origin), to: rig.window)
        // AppKit sends a drag to the view that took the press only while it is in the window; once it is
        // recycled the coordinator's monitor reads the rest. Both end in `handle`, so the test delivers
        // the drags there, as the monitor does (a monitor is not reachable under swift test).
        let top = rig.scroll.convert(rig.scroll.bounds, to: nil).maxY
        var steps = 0
        while origin.window != nil, steps < 300 {
            coordinator.handle(rig.event(.leftMouseDragged, at: CGPoint(x: 300, y: top + 40)))
            await rig.settle()
            steps += 1
        }
        #expect(origin.window == nil, "the origin was never recycled (\(steps) steps, \(coordinator.autoscrolls) scrolls)")
        #expect(coordinator.autoscrolls > 0)
        // Now onto a body in view.
        // (well inside the column: not under the composer, and not in the band an autoscroll still reads as past the edge)
        let visible = rig.views.filter {
            let frame = $0.convert($0.bounds, to: nil)
            return $0.participant == 0 && $0.window != nil && frame.minY > 150 && frame.maxY < 430
        }
        let target = try #require(visible.min { Int($0.message.dropFirst())! < Int($1.message.dropFirst())! })
        let k = Int(target.message.dropFirst())!
        let b = target.rects(for: NSRange(location: 8, length: 1))[0]
        let point = target.convert(CGPoint(x: b.minX + 0.5, y: b.midY), to: nil)
        coordinator.handle(rig.event(.leftMouseDragged, at: point))
        coordinator.handle(rig.event(.leftMouseUp, at: point))
        // No text view that took a press is left: the column's own responder takes ⌘C.
        #expect(rig.window.firstResponder is SelectionResponderView, "\(String(describing: rig.window.firstResponder))")
        let between = ((k + 1)..<60).map { header($0) + "\nmessage \($0) body" }
        let expected = (["\(String("message \(k) body".dropFirst(8)))"] + between + [header(60) + "\nmessage "]).joined(separator: "\n")
        #expect(rig.copy() == expected)
        #expect(NSPasteboard.general.changeCount == before)
    }

    // MARK: Edits and deletes under a selection

    @Test func editingTheMessageAtEitherEndDropsThatPartAndAnInteriorEditShows() async throws {
        let rig = try await Rig(threeMessages())
        rig.select(from: try #require(rig.body(1)), 6, to: try #require(rig.body(3)), 4)
        #expect(rig.copy() == "bravo\n" + header(2) + "\ndelta echo\n" + header(3) + "\ngolf")
        // The end's body text changes: that part is dropped, the rest stays.
        rig.model.messages[2] = message(3, text: "gold rush")
        await rig.settle()
        #expect(rig.copy() == "bravo\n" + header(2) + "\ndelta echo\n" + header(3))
        // An interior edit is simply the new text.
        rig.model.messages[1] = message(2, text: "delta CHANGED")
        await rig.settle()
        #expect(rig.copy() == "bravo\n" + header(2) + "\ndelta CHANGED\n" + header(3))
        // The start's body text changes: dropped too.
        rig.model.messages[0] = message(1, text: "alpha bravo!")
        await rig.settle()
        #expect(rig.copy() == header(2) + "\ndelta CHANGED\n" + header(3))
    }

    @Test func deletingAMessageUnderTheSelectionMovesTheEndToItsNeighbour() async throws {
        // An interior message: the rest of the selection stays.
        var rig = try await Rig(threeMessages())
        rig.select(from: try #require(rig.body(1)), 6, to: try #require(rig.body(3)), 4)
        rig.model.messages.remove(at: 1)
        await rig.settle()
        #expect(rig.copy() == "bravo\n" + header(3) + "\ngolf")
        // The end's own message: the end moves to the start of the next one, or the end of the previous.
        rig = try await Rig(threeMessages())
        rig.select(from: try #require(rig.body(1)), 6, to: try #require(rig.body(2)), 4)
        rig.model.messages.remove(at: 1)
        await rig.settle()
        #expect(rig.copy() == "bravo")
        rig = try await Rig(threeMessages())
        rig.select(from: try #require(rig.body(1)), 6, to: try #require(rig.body(3)), 4)
        rig.model.messages.removeLast()
        await rig.settle()
        #expect(rig.copy() == "bravo\n" + header(2) + "\ndelta echo")
        // The start's message: the start moves to the next message's beginning.
        rig = try await Rig(threeMessages())
        rig.select(from: try #require(rig.body(1)), 6, to: try #require(rig.body(3)), 4)
        rig.model.messages.removeFirst()
        await rig.settle()
        #expect(rig.copy() == header(2) + "\ndelta echo\n" + header(3) + "\ngolf")
        // Every message: nothing left to select.
        let coordinator = try #require(rig.coordinator)
        rig.model.messages = []
        await rig.settle()
        #expect(!coordinator.hasSelection)
        #expect(coordinator.selectedText.isEmpty)
    }

    // MARK: A relative timestamp's refresh under a selection

    @Test func aRefreshOfARelativeTimestampDropsItsPartWithoutATrapOrALoneSurrogate() async throws {
        let before = NSPasteboard.general.changeCount
        let inlines: [MarkdownInline] = [
            .text("due "), .timestamp(unix: Int64(epoch.timeIntervalSince1970), style: .relative),
            .text(" \u{1F469}\u{200D}\u{1F469}\u{200D}\u{1F467} end"),
        ]
        let body = MessageBody(blocks: [.paragraph(inlines)])
        let rig = try await Rig([message(1, text: "alpha bravo"), message(2, text: "", body: body)])
        let a = try #require(rig.body(1)), b = try #require(rig.body(2))
        let end = (b.attributed.string as NSString).range(of: "end").location
        rig.select(from: a, 6, to: b, end + 2)
        let first = rig.copy() ?? ""
        #expect(first.hasSuffix("en"), "\(first.debugDescription)")
        #expect(!hasLoneSurrogate(first))
        // The timestamp's refresh: another `now`, so another string (and another length) in the same view.
        func refreshed(_ now: Date) -> MessageTextContent {
            guard case .paragraph(let numbered) = body.blocks[0], case .inlines(let run)? = InlinePart.paragraph(numbered).first else { return .paragraph([]) }
            return .paragraph(MarkdownText(body: body).segments(run, context: RenderContext(now: now, locale: locale)))
        }
        let shown = b.attributed.string
        b.setContent(refreshed(epoch.addingTimeInterval(86_400 * 800)))
        #expect(b.attributed.string != shown, "the refresh changed nothing, so the test proves nothing")
        let after = rig.copy() ?? ""
        #expect(after == "bravo\n" + header(2), "\(after.debugDescription)")
        #expect(!hasLoneSurrogate(after))
        // The same text again is the same part, shown again.
        b.setContent(refreshed(.now))
        #expect(rig.copy()?.hasSuffix("en") == true)
        #expect(NSPasteboard.general.changeCount == before)
    }

    // MARK: Clicking elsewhere

    @Test func aPressOutsideTheSelectionsTextClearsItAndOneInItPlacesTheNext() async throws {
        let rig = try await Rig(threeMessages())
        let coordinator = try #require(rig.body(1)?.selection)
        rig.select(from: try #require(rig.body(1)), 0, to: try #require(rig.body(2)), 5)
        #expect(coordinator.hasSelection)
        // A press on text of this column: the monitor leaves it to the view, which begins a new selection.
        let text = try #require(rig.body(3))
        let r = text.rects(for: NSRange(location: 1, length: 1))[0]
        coordinator.handle(rig.event(.leftMouseDown, at: text.convert(CGPoint(x: r.midX, y: r.midY), to: nil)))
        #expect(coordinator.hasSelection)
        TextHarness.click(text, at: CGPoint(x: r.midX, y: r.midY))
        #expect(!coordinator.hasSelection, "a click is a new, empty selection")
        // A press anywhere else clears.
        rig.select(from: try #require(rig.body(1)), 0, to: try #require(rig.body(2)), 5)
        #expect(coordinator.hasSelection)
        coordinator.handle(rig.event(.leftMouseDown, at: CGPoint(x: 3, y: 3)))
        #expect(!coordinator.hasSelection)
    }
}
