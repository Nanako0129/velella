import AppKit
import Observation
import SwiftUI
import Testing
@testable import VelellaUI

// Wave 3 P: older history (docs/plans/wave3.md, row P). Everything here runs a real MessageArea in an
// offscreen window (never ordered in), scrolled with wheel events as a user does: the request when
// the first row appears, and the rows keeping their place on screen when older ones are prepended.

/// What the app does with the request: the host's `older` goes to `.loading` when it is made.
@MainActor @Observable private final class Model {
    var messages: [MessageItem]
    var conversation = "1"
    var older: OlderState = .idle
    var requests = 0
    init(_ messages: [MessageItem]) { self.messages = messages }
}

private struct Host: View {
    let model: Model
    var body: some View {
        MessageArea(
            header: ChannelHeader(name: "general"), conversationID: model.conversation, messages: model.messages,
            load: model.messages.isEmpty ? .loading : .loaded, reply: .constant(nil), draft: .constant(""), older: model.older,
            onLoadOlder: { model.requests += 1; model.older = .loading }
        )
    }
}

/// Message `n`: its own author (so every row is a group start with a header) and a body that wraps
/// to a different number of lines, so row heights differ.
private func item(_ n: Int) -> MessageItem {
    let base = Date(timeIntervalSince1970: 1_800_000_000)
    let text = "message \(n) " + String(repeating: "word ", count: (n * 7) % 40)
    return MessageItem(
        id: String(n), authorId: "a\(n)", authorName: "author \(n)", avatar: .initials("A"),
        timestamp: base.addingTimeInterval(Double(n) * 60), text: text
    )
}

private func items(_ range: ClosedRange<Int>) -> [MessageItem] { range.map(item) }

/// The accessibility label (or value) of a node; SwiftUI's nodes answer these selectors without
/// declaring the protocol to Swift.
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

@MainActor private final class Rig {
    let model: Model
    let window: NSWindow
    let host: NSHostingView<Host>
    let scroll: NSScrollView

    /// 61 messages, 100...160, opened at the bottom.
    init(_ messages: [MessageItem] = items(100...160)) async throws {
        model = Model(messages)
        window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 700, height: 500), styleMask: .borderless, backing: .buffered, defer: false)
        host = NSHostingView(rootView: Host(model: model))
        window.contentView = host
        // What an assistive app sets on the application; SwiftUI builds no accessibility nodes before.
        NSApplication.shared.accessibilitySetValue(true, forAttribute: NSAccessibility.Attribute(rawValue: "AXEnhancedUserInterface"))
        func find(_ view: NSView) -> NSScrollView? { (view as? NSScrollView) ?? view.subviews.lazy.compactMap(find).first }
        for _ in 0..<6 {
            host.layoutSubtreeIfNeeded()
            try? await Task.sleep(for: .milliseconds(40))
        }
        scroll = try #require(find(host))
    }

    var offset: CGFloat { scroll.contentView.bounds.origin.y }

    func settle() async {
        for _ in 0..<6 {
            window.contentView?.layoutSubtreeIfNeeded()
            try? await Task.sleep(for: .milliseconds(40))
        }
    }

    /// Wheel events, as a user's (a programmatic `NSClipView.scroll(to:)` does not make SwiftUI treat
    /// the scroll as the user's, and its position tracking then ignores it), until the offset
    /// reaches `target` (0 is the top).
    func scroll(toOffset target: CGFloat) async {
        for _ in 0..<600 where offset > target {
            let dy = Int32(min(120, max(1, offset - target)))
            let cg = CGEvent(scrollWheelEvent2Source: nil, units: .pixel, wheelCount: 1, wheel1: dy, wheel2: 0, wheel3: 0)!
            scroll.scrollWheel(with: NSEvent(cgEvent: cg)!)
            window.contentView?.layoutSubtreeIfNeeded()
            try? await Task.sleep(for: .milliseconds(5))
        }
        await settle()
    }

    /// Screen position of the top of every message text now on screen, by message number.
    func positions() -> [Int: CGFloat] {
        var out: [Int: CGFloat] = [:]
        walk(host) { object in
            let text = label(object)
            guard text.hasPrefix("message "), let n = Int(text.dropFirst("message ".count).prefix { $0.isNumber }),
                  let frame = (object.value(forKey: "accessibilityFrame") as? NSValue)?.rectValue,
                  // Realized rows beyond the viewport report no usable frame; the window is 500 tall.
                  frame.minY > 0, frame.maxY < 2000 else { return }
            out[n] = frame.minY
        }
        return out
    }
}

// MARK: The request

@MainActor @Test func theTopRowAsksForOlderMessagesOnceUntilTheyArrive() async throws {
    let rig = try await Rig()
    // Opened at the bottom, 61 rows in a 500 pt column: the first row is far off screen.
    #expect(rig.model.requests == 0)
    await rig.scroll(toOffset: 0)
    #expect(rig.model.requests == 1)
    // While it loads, leaving the top and coming back asks nothing more.
    await rig.scroll(toOffset: 600)
    await rig.scroll(toOffset: 0)
    #expect(rig.model.older == .loading)
    #expect(rig.model.requests == 1)
}

@MainActor @Test func reachingTheTopAgainAfterAPageArrivedAsksAgain() async throws {
    let rig = try await Rig()
    await rig.scroll(toOffset: 0)
    #expect(rig.model.requests == 1)
    rig.model.messages = items(50...99) + rig.model.messages
    rig.model.older = .idle
    await rig.settle()
    #expect(rig.model.requests == 1, "the page landing above must not itself count as reaching the top")
    await rig.scroll(toOffset: 0)
    #expect(rig.model.requests == 2)
}

@MainActor @Test func nothingIsAskedForAtTheStartOfTheConversationWhileLoadingOrAfterAFailure() async throws {
    for state in [OlderState.reachedStart, .loading, .failed("Couldn't load older messages")] {
        let rig = try await Rig()
        rig.model.older = state
        await rig.settle()
        await rig.scroll(toOffset: 0)
        #expect(rig.model.requests == 0, "\(state)")
    }
}

// MARK: The position

/// Every row on screen before the page lands is on screen again where it was (within a point:
/// SwiftUI refines the estimated heights of rows as it lays them out; 0.2 pt measured).
@MainActor private func prependKeepsPosition(startingAt start: CGFloat?, sourceLocation: SourceLocation = #_sourceLocation) async throws {
    let rig = try await Rig()
    if let start { await rig.scroll(toOffset: start) }
    let before = rig.positions()
    #expect(before.count >= 4, "rows on screen before: \(before.count)", sourceLocation: sourceLocation)
    let offsetBefore = rig.offset
    rig.model.messages = items(50...99) + rig.model.messages
    await rig.settle()
    let after = rig.positions()
    #expect(rig.offset > offsetBefore + 1000, "the offset follows the 50 new rows: \(offsetBefore) -> \(rig.offset)", sourceLocation: sourceLocation)
    for (n, y) in before {
        let moved = after[n].map { abs($0 - y) }
        #expect(moved.map { $0 <= 1 } == true, "row \(n) moved by \(String(describing: moved)) (from \(y))", sourceLocation: sourceLocation)
    }
}

@MainActor @Test func aPrependKeepsTheVisibleRowsWhereTheyWereAtTheBottom() async throws {
    try await prependKeepsPosition(startingAt: nil)
}

@MainActor @Test func aPrependKeepsTheVisibleRowsWhereTheyWereInTheMiddle() async throws {
    try await prependKeepsPosition(startingAt: 1000)
}

/// The case that matters: the user is at the top (the first row has just appeared and asked).
@MainActor @Test func aPrependKeepsTheVisibleRowsWhereTheyWereAtTheTop() async throws {
    try await prependKeepsPosition(startingAt: 0)
}

@MainActor @Test func twoPagesInARowKeepThePositionBothTimes() async throws {
    let rig = try await Rig()
    await rig.scroll(toOffset: 0)
    rig.model.messages = items(50...99) + rig.model.messages
    await rig.settle()
    await rig.scroll(toOffset: 0)
    let second = rig.positions()
    #expect(second.keys.contains { $0 < 100 }, "scrolled up into the first page")
    rig.model.messages = items(0...49) + rig.model.messages
    await rig.settle()
    let third = rig.positions()
    #expect(second.count >= 4)
    for (n, y) in second { #expect(third[n].map { abs($0 - y) <= 1 } == true, "row \(n)") }
}

/// A conversation opened after another starts at its bottom, not at the row the last one was
/// scrolled to (a stale position would be applied to any list containing that id).
@MainActor @Test func anotherConversationOpensAtItsBottom() async throws {
    let rig = try await Rig()
    let bottom = rig.positions()
    await rig.scroll(toOffset: 500)
    #expect(rig.positions() != bottom)
    rig.model.conversation = "2"
    rig.model.messages = []
    await rig.settle()
    rig.model.conversation = "1"
    rig.model.messages = items(100...160)
    await rig.settle()
    let reopened = rig.positions()
    #expect(reopened.keys.contains(160), "\(reopened.keys.sorted())")
    for (n, y) in bottom { #expect(reopened[n].map { abs($0 - y) <= 1 } == true, "row \(n)") }
}

// MARK: The top row

@MainActor private func headerHeight(_ older: OlderState) -> CGFloat {
    NSHostingView(rootView: OlderRow(older: older, onRetry: {}).frame(width: 400)).fittingSize.height
}

/// A header that changed height would shift every row below it when the spinner came or went.
@MainActor @Test func theTopRowHasTheSameHeightInEveryState() {
    let heights = [OlderState.idle, .loading, .reachedStart, .failed("Couldn't load older messages")].map(headerHeight)
    #expect(Set(heights) == [OlderRow.height], "\(heights)")
}

@MainActor @Test func theFailureTextIsVerbatimAndRetryAsksAgain() async throws {
    let rig = try await Rig()
    rig.model.older = .failed("Couldn't load **older** messages")
    await rig.settle()
    await rig.scroll(toOffset: 0)
    var texts: [String] = []
    var retry: NSObject?
    walk(rig.host) { object in
        texts.append(label(object))
        if (send(object, "accessibilityRole") as? String) == "AXButton", label(object) == "Retry" { retry = object }
    }
    #expect(texts.contains("Couldn't load **older** messages"), "\(texts.prefix(12))")
    let button = try #require(retry)
    #expect(rig.model.requests == 0, "a failure never retries by itself")
    // AXPress, as VoiceOver's "activate" sends it.
    #expect(button.perform(NSSelectorFromString("accessibilityPerformPress")) != nil)
    #expect(rig.model.requests == 1)
}
