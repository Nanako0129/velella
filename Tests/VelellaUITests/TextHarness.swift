import AppKit
import SwiftUI
import Testing
@testable import VelellaUI

// The N0 harness kept for the message text view (docs/plans/selection.md, N1 Tests): an offscreen
// window whose key state is switchable and asserted, an injected pasteboard, pixels read with
// `cacheDisplay(in:to:)`, mouse events dispatched by hit-testing, never through `NSApp`.

/// Offscreen: never ordered in. `canBecomeKey` and `isKeyWindow` follow `keyState`.
final class TextWindow: NSWindow {
    var keyState = true
    override var canBecomeKey: Bool { keyState }
    override var isKeyWindow: Bool { keyState }
    override var isMainWindow: Bool { keyState }

    @MainActor convenience init(width: CGFloat, height: CGFloat) {
        self.init(contentRect: NSRect(x: 0, y: 0, width: width, height: height), styleMask: [.borderless], backing: .buffered, defer: false)
        appearance = NSAppearance(named: .darkAqua)
        isReleasedWhenClosed = false
        acceptsMouseMovedEvents = true
    }
}

/// An opaque pane behind the text, so a translucent cover reads against a real background.
final class TextBackdrop: NSView {
    override var isFlipped: Bool { true }
    override func draw(_ dirtyRect: NSRect) {
        NSColor.windowBackgroundColor.setFill()
        dirtyRect.fill()
    }
}

@MainActor
enum TextHarness {
    /// A throwaway named pasteboard; the production default (`.general`) is never written.
    static func board() -> NSPasteboard { NSPasteboard(name: NSPasteboard.Name("velella.tests.\(UUID().uuidString)")) }

    /// What AppKit puts on a board for one `setString(_:forType: .string)` (N0-12).
    static let stringFlavours: Set<String> = [NSPasteboard.PasteboardType.string.rawValue, "NSStringPboardType"]

    static let context = RenderContext(now: Date(timeIntervalSince1970: 1_760_000_000), locale: Locale(identifier: "en-US"))

    /// A 48 x 48 square standing in for an emoji.
    static func image(_ color: NSColor = .systemPink) -> NSImage {
        let image = NSImage(size: NSSize(width: 48, height: 48))
        image.lockFocus()
        color.setFill()
        NSRect(x: 0, y: 0, width: 48, height: 48).fill()
        image.unlockFocus()
        return image
    }

    /// An emoji host that is not Discord's: `EmojiImages.fetchable` is false, so nothing is fetched.
    static func emojiURL(_ name: String = UUID().uuidString) -> URL { URL(string: "https://emoji.example.test/e/\(name).png")! }

    // MARK: Hosting

    /// A SwiftUI view in the offscreen window, on an opaque backdrop.
    static func hosting<V: View>(_ view: V, size: CGSize, key: Bool = true, accessibility: Bool = false) -> (host: NSHostingView<V>, window: TextWindow) {
        let window = TextWindow(width: size.width, height: size.height)
        window.keyState = key
        let backdrop = TextBackdrop(frame: NSRect(origin: .zero, size: size))
        let host = NSHostingView(rootView: view)
        host.frame = backdrop.bounds
        backdrop.addSubview(host)
        window.contentView = backdrop
        // What an assistive app sets on the application; SwiftUI builds no accessibility nodes without it.
        if accessibility { NSApplication.shared.accessibilitySetValue(true, forAttribute: NSAccessibility.Attribute(rawValue: "AXEnhancedUserInterface")) }
        host.layoutSubtreeIfNeeded()
        return (host, window)
    }

    /// `view` laid out at `width` with its own height, in the offscreen window. The window is as tall as the view.
    static func fitted<V: View>(_ view: V, width: CGFloat, key: Bool = true) -> (host: NSHostingView<AnyView>, window: TextWindow) {
        let probe = NSHostingView(rootView: AnyView(view.frame(width: width, alignment: .topLeading)))
        let height = max(probe.fittingSize.height, 1)
        return hosting(AnyView(view.frame(width: width, alignment: .topLeading).frame(width: width, height: height, alignment: .topLeading)),
                       size: CGSize(width: width, height: height), key: key)
    }

    /// Every message text view under `view`, in subview order.
    static func textViews(in view: NSView) -> [MessageTextView] {
        ((view as? MessageTextView).map { [$0] } ?? []) + view.subviews.flatMap { textViews(in: $0) }
    }

    static func all<T: NSView>(_ type: T.Type, in view: NSView) -> [T] {
        ((view as? T).map { [$0] } ?? []) + view.subviews.flatMap { all(type, in: $0) }
    }

    // MARK: Events

    nonisolated(unsafe) private static var eventNumber = 100

    /// A mouse event at `point` in `view`'s coordinates, addressed to the view's window.
    static func mouse(_ type: NSEvent.EventType, at point: CGPoint, in view: NSView, clickCount: Int = 1) -> NSEvent {
        eventNumber += 1
        return NSEvent.mouseEvent(
            with: type, location: view.convert(point, to: nil), modifierFlags: [], timestamp: ProcessInfo.processInfo.systemUptime,
            windowNumber: view.window?.windowNumber ?? 0, context: nil, eventNumber: eventNumber, clickCount: clickCount, pressure: 1)!
    }

    /// The view a left button went down on: the capture of a drag (as AppKit's window keeps it).
    nonisolated(unsafe) static var captured: NSView?

    /// Delivers `event` as `NSWindow.sendEvent` would to a visible window: by hit-testing, with the
    /// view of a mouse down capturing the drag and the up. (An offscreen window that was never
    /// ordered in drops mouse events given to `sendEvent`, measured in N0.)
    static func send(_ event: NSEvent, to window: NSWindow) {
        guard let content = window.contentView else { return }
        let hit = content.hitTest(content.superview?.convert(event.locationInWindow, from: nil) ?? event.locationInWindow)
        switch event.type {
        case .leftMouseDown:
            captured = hit
            // What AppKit documents `NSWindow.sendEvent` doing before a mouse down (not observable
            // offscreen): the hit view becomes first responder when it accepts that.
            if let hit, hit.acceptsFirstResponder, window.firstResponder !== hit { window.makeFirstResponder(hit) }
            hit?.mouseDown(with: event)
        case .leftMouseDragged: captured?.mouseDragged(with: event)
        case .leftMouseUp: captured?.mouseUp(with: event); captured = nil
        case .mouseMoved: hit?.mouseMoved(with: event)
        default: window.sendEvent(event)
        }
    }

    static func click(_ view: NSView, at point: CGPoint, count: Int = 1) {
        guard let window = view.window else { return }
        send(mouse(.leftMouseDown, at: point, in: view, clickCount: count), to: window)
        send(mouse(.leftMouseUp, at: point, in: view, clickCount: count), to: window)
    }

    static func drag(_ view: NSView, from start: CGPoint, to end: CGPoint) {
        guard let window = view.window else { return }
        send(mouse(.leftMouseDown, at: start, in: view), to: window)
        send(mouse(.leftMouseDragged, at: CGPoint(x: (start.x + end.x) / 2, y: (start.y + end.y) / 2), in: view), to: window)
        send(mouse(.leftMouseDragged, at: end, in: view), to: window)
        send(mouse(.leftMouseUp, at: end, in: view), to: window)
    }

    /// The centre of the first segment frame of `range` in `view`.
    static func center(_ view: MessageTextView, _ range: NSRange) -> CGPoint {
        let r = view.rects(for: range)[0]
        return CGPoint(x: r.midX, y: r.midY)
    }

    // MARK: Renders

    /// An offscreen render of `view` in a hosted window (what `ImageRenderer` cannot draw: AppKit
    /// views), as the moved layout tests read it: `width` x the view's own height, at `scale`
    /// pixels per point, RGBA, transparent where nothing draws.
    struct Render {
        let width: Int, height: Int
        let bytesPerRow: Int
        let data: Data

        /// (r, g, b, a) at a pixel.
        func at(_ x: Int, _ y: Int) -> [UInt8] {
            let o = y * bytesPerRow + x * 4
            return Array(data[o..<o + 4])
        }
    }

    /// `width` nil renders at the view's natural size.
    static func render<V: View>(_ view: V, width: CGFloat?, scale: CGFloat, light: Bool = true) -> Render? {
        let root = width.map { AnyView(view.frame(width: $0, alignment: .leading)) } ?? AnyView(view.fixedSize())
        let size = NSHostingView(rootView: root).fittingSize
        guard size.width > 0, size.height > 0 else { return nil }
        let window = TextWindow(width: size.width, height: size.height)
        window.appearance = NSAppearance(named: light ? .aqua : .darkAqua)
        let host = NSHostingView(rootView: root)
        host.frame = NSRect(origin: .zero, size: size)
        window.contentView = host
        host.layoutSubtreeIfNeeded()
        let w = Int((size.width * scale).rounded(.up)), h = Int((size.height * scale).rounded(.up))
        guard let blank = NSBitmapImageRep(bitmapDataPlanes: nil, pixelsWide: w, pixelsHigh: h, bitsPerSample: 8, samplesPerPixel: 4,
                                           hasAlpha: true, isPlanar: false, colorSpaceName: .deviceRGB, bytesPerRow: 0, bitsPerPixel: 32),
              let rep = blank.retagging(with: .sRGB) else { return nil }
        rep.size = size
        host.cacheDisplay(in: host.bounds, to: rep)
        guard let bytes = rep.bitmapData else { return nil }
        return Render(width: w, height: h, bytesPerRow: rep.bytesPerRow, data: Data(bytes: bytes, count: rep.bytesPerRow * h))
    }

    // MARK: Pixels

    @MainActor struct Pixels {
        let width: Int, height: Int, scale: CGFloat
        private let data: [UInt8]
        private let origin: CGPoint

        init?(_ view: NSView, rect: NSRect? = nil) {
            let rect = rect ?? view.bounds
            guard let rep = view.bitmapImageRepForCachingDisplay(in: rect) else { return nil }
            view.cacheDisplay(in: rect, to: rep)
            guard let bytes = rep.bitmapData, rep.samplesPerPixel >= 3, rep.bitsPerSample == 8 else { return nil }
            width = rep.pixelsWide
            height = rep.pixelsHigh
            scale = CGFloat(rep.pixelsWide) / max(rect.width, 1)
            let spp = rep.samplesPerPixel
            var out = [UInt8](repeating: 0, count: width * height * 4)
            for y in 0..<height {
                for x in 0..<width {
                    let o = y * rep.bytesPerRow + x * spp
                    for c in 0..<3 { out[(y * width + x) * 4 + c] = bytes[o + c] }
                    out[(y * width + x) * 4 + 3] = spp > 3 ? bytes[o + 3] : 255
                }
            }
            data = out
            origin = rect.origin
        }

        /// (r, g, b, a) at a pixel (pixel coordinates).
        func at(_ x: Int, _ y: Int) -> [UInt8] {
            let o = (y * width + x) * 4
            return Array(data[o..<o + 4])
        }

        /// RGB at a point in view coordinates.
        func rgb(_ x: CGFloat, _ y: CGFloat) -> (Int, Int, Int) {
            let px = min(max(Int((x - origin.x) * scale), 0), width - 1), py = min(max(Int((y - origin.y) * scale), 0), height - 1)
            let p = at(px, py)
            return (Int(p[0]), Int(p[1]), Int(p[2]))
        }

        /// Largest channel difference between any two pixels of `rect` (view coordinates).
        func spread(_ rect: CGRect) -> Int {
            var lo = [255, 255, 255], hi = [0, 0, 0]
            let x0 = Int(((rect.minX - origin.x) * scale).rounded(.up)), x1 = Int(((rect.maxX - origin.x) * scale).rounded(.down))
            let y0 = Int(((rect.minY - origin.y) * scale).rounded(.up)), y1 = Int(((rect.maxY - origin.y) * scale).rounded(.down))
            guard x1 > x0, y1 > y0 else { return -1 }
            for y in max(y0, 0)..<min(y1, height) {
                for x in max(x0, 0)..<min(x1, width) {
                    let p = at(x, y)
                    for c in 0..<3 { lo[c] = min(lo[c], Int(p[c])); hi[c] = max(hi[c], Int(p[c])) }
                }
            }
            return (0..<3).map { hi[$0] - lo[$0] }.max() ?? 0
        }

        /// Whether the pixel differs from `background` by more than `tolerance` (summed channels).
        func ink(_ x: Int, _ y: Int, against background: [UInt8], tolerance: Int = 60) -> Bool {
            let p = at(x, y)
            return (0..<3).map { abs(Int(p[$0]) - Int(background[$0])) }.reduce(0, +) > tolerance
        }
    }

    // MARK: Text

    /// The segments of `inlines` as one paragraph of a body, and the body.
    static func segments(_ inlines: [MarkdownInline], mentions: [ResolvedMention] = [], emoji: [EmojiImage] = [],
                         revealed: Set<Int> = []) -> (segments: [Segment], body: MessageBody) {
        let body = MessageBody(blocks: [.paragraph(inlines)], mentions: mentions, emoji: emoji)
        guard case .paragraph(let numbered) = body.blocks[0], case .inlines(let run)? = InlinePart.paragraph(numbered).first else { return ([], body) }
        return (MarkdownText(body: body, revealed: revealed).segments(run, context: context), body)
    }

    /// A standalone text view of `segments` at `width`, first responder in a key offscreen window.
    /// `clearHidden` false switches off layer 2 (the mutation runs of N0-7).
    static func paragraph(_ segments: [Segment], width: CGFloat = 320, key: Bool = true, clearHidden: Bool = true) -> (view: MessageTextView, window: TextWindow) {
        view(.build([.text(segments: segments, size: MarkdownText.bodySize, weight: .regular, secondary: false)], clearHidden: clearHidden), width: width, key: key)
    }

    static func view(_ content: MessageTextContent, width: CGFloat = 320, key: Bool = true) -> (view: MessageTextView, window: TextWindow) {
        let view = MessageTextView(frame: .zero)
        view.pasteboard = board()
        view.setContent(content)
        let h = view.height(forWidth: width)
        view.frame = NSRect(x: 0, y: 0, width: width, height: h)
        let window = TextWindow(width: width, height: h)
        window.keyState = key
        window.contentView = TextBackdrop(frame: NSRect(x: 0, y: 0, width: width, height: h))
        window.contentView?.addSubview(view)
        window.makeFirstResponder(view)
        view.layoutSubtreeIfNeeded()
        return (view, window)
    }

    /// Fails if `body` changed the general pasteboard (every copy here goes to an injected board).
    static func guardingGeneral<T>(_ body: @MainActor () throws -> T) rethrows -> T {
        let before = NSPasteboard.general.changeCount
        let result = try body()
        #expect(NSPasteboard.general.changeCount == before, "NSPasteboard.general.changeCount moved")
        return result
    }
}

// MARK: Accessibility walk (N0-10)

@MainActor
enum TextAX {
    struct Record { var path: String; var attribute: String; var output: String }

    static func send(_ object: NSObject, _ name: String) -> Any? {
        let selector = NSSelectorFromString(name)
        return object.responds(to: selector) ? object.perform(selector)?.takeUnretainedValue() : nil
    }

    static func value(_ object: NSObject, _ attribute: String) -> Any? {
        let selector = NSSelectorFromString("accessibilityAttributeValue:")
        return object.responds(to: selector) ? object.perform(selector, with: attribute)?.takeUnretainedValue() : nil
    }

    static func parameterized(_ object: NSObject, _ attribute: String, _ parameter: Any) -> Any? {
        let selector = NSSelectorFromString("accessibilityAttributeValue:forParameter:")
        return object.responds(to: selector) ? object.perform(selector, with: attribute, with: parameter)?.takeUnretainedValue() : nil
    }

    /// Everything printable in a value: strings, attributed strings, the text inside RTF and UTF-8 data.
    static func describe(_ value: Any?) -> String {
        switch value {
        case nil: return "nil"
        case let s as String: return "\"\(s)\""
        case let a as NSAttributedString: return "attributed\"\(a.string)\""
        case let d as Data:
            let rtf = (try? NSAttributedString(data: d, options: [:], documentAttributes: nil))?.string ?? ""
            return "data[\(d.count)] text=\"\(rtf)\" raw=\"\(String(decoding: d, as: UTF8.self).prefix(200))\""
        case let n as NSNumber: return "\(n)"
        case let v as NSValue: return "\(v)"
        case let a as [Any]: return "[" + a.map { describe($0) }.joined(separator: ", ") + "]"
        case let o as NSObject: return "<\(type(of: o))>"
        default: return "\(value!)"
        }
    }

    /// Every attribute, parameterized attribute and child of `root`, recursively.
    static func walk(_ root: NSObject, path: String = "root", seen: inout Set<ObjectIdentifier>, into records: inout [Record]) {
        guard seen.insert(ObjectIdentifier(root)).inserted else { return }
        let me = "\(path)/\(type(of: root))"
        for name in ["accessibilityRole", "accessibilityLabel", "accessibilityValue", "accessibilityTitle", "accessibilityHelp",
                     "accessibilityRoleDescription", "accessibilityIdentifier", "accessibilitySelectedText",
                     "accessibilityNumberOfCharacters", "accessibilityVisibleCharacterRange"] {
            records.append(Record(path: me, attribute: name, output: describe(send(root, name))))
        }
        let names = (send(root, "accessibilityAttributeNames") as? [String]) ?? []
        for name in names where !["AXChildren", "AXParent", "AXWindow", "AXTopLevelUIElement"].contains(name) {
            records.append(Record(path: me, attribute: name, output: describe(value(root, name))))
        }
        let full = NSRange(location: 0, length: 1000)
        for name in (send(root, "accessibilityParameterizedAttributeNames") as? [String]) ?? [] {
            for (how, parameter) in [("range", NSValue(range: full) as Any), ("index 0", NSNumber(value: 0)), ("point", NSValue(point: .zero))] {
                records.append(Record(path: me, attribute: "param:\(name) [\(how)]", output: describe(parameterized(root, name, parameter))))
            }
        }
        if let element = root as? NSView {
            records.append(Record(path: me, attribute: "accessibilityString(for:)", output: describe(element.accessibilityString(for: full))))
            records.append(Record(path: me, attribute: "accessibilityAttributedString(for:)", output: describe(element.accessibilityAttributedString(for: full))))
        }
        var children = (send(root, "accessibilityChildren") as? [Any]) ?? []
        children += ((value(root, "AXChildren") as? [Any]) ?? [])
        for child in children {
            if let object = child as? NSObject { walk(object, path: me, seen: &seen, into: &records) }
        }
    }

    static func leaks(_ records: [Record], word: String = "secret") -> [Record] {
        records.filter { $0.output.localizedCaseInsensitiveContains(word) }
    }
}
