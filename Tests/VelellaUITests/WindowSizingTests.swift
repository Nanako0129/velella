import AppKit
import SwiftUI
import Testing
@testable import VelellaUI

private func splitViews(in view: NSView) -> [NSSplitView] {
    ((view as? NSSplitView).map { [$0] } ?? []) + view.subviews.flatMap { splitViews(in: $0) }
}

/// The 2026-10-13 crash began with widening the sidebar in a window at its minimum width: with a
/// minimum width on the message column under SwiftUI's `.inspector`, that grew the window by twice
/// the drag (1486 → 1606 for +60) and the minimum followed the column widths. Driven through
/// NSSplitView in an offscreen window, the window must keep its size and minimum instead, and the
/// member list its fixed width. With that message-column minimum put back, this test's process
/// dies with the same NSGenericException as the app (observed 2026-10-13).
@MainActor @Test func dividerDragsNeverGrowTheWindow() async throws {
    let minWidth = DesignTokens.Size.windowMinWidth
    let window = NSWindow(
        contentRect: NSRect(x: 0, y: 0, width: minWidth, height: 800),
        styleMask: [.titled, .resizable, .fullSizeContentView], backing: .buffered, defer: false
    )
    window.contentViewController = NSHostingController(rootView: ShellView(state: .constant(.demo)))
    // Suspends instead of pumping RunLoop.main: a pump inside this main-actor job ran other
    // main-actor tests' jobs nested in it, and the suite crashed (EXC_BAD_ACCESS in an
    // autorelease pool pop, 2026-10-14). While suspended the run loop lays out as before.
    func settle() async {
        for _ in 0..<5 {
            window.contentView?.layoutSubtreeIfNeeded()
            try? await Task.sleep(for: .milliseconds(30))
        }
    }
    await settle()
    let content = try #require(window.contentView)
    let splits = splitViews(in: content)
    let sidebar = try #require(splits.first), members = try #require(splits.last)
    try #require(sidebar !== members)
    sidebar.setPosition(1000, ofDividerAt: 0)
    members.setPosition(0, ofDividerAt: 0)
    await settle()
    // v2's member list does not resize: the divider drag leaves it at its fixed width.
    #expect(members.arrangedSubviews[1].frame.width == DesignTokens.Size.memberColumnWidth)
    #expect(sidebar.arrangedSubviews[0].frame.width == DesignTokens.Size.railWidth
        + DesignTokens.Size.railColumnDivider + DesignTokens.Size.channelColumnMax)
    #expect(window.frame.width == minWidth)
    #expect(window.contentMinSize.width == minWidth)
    window.close()
}
