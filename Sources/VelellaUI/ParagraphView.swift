import AppKit
import Observation
import SwiftUI

// The paragraph host (docs/plans/markdown.md, K2; docs/plans/selection.md, N1): a paragraph's
// segments in one message text view, inside a `TimelineView` that rebuilds them at the smallest
// refresh they ask for.

/// What a paragraph's segments are built from on each timeline tick.
struct RenderContext: Equatable, Sendable {
    var now: Date
    /// The shell locale (`ShellState.locale`: the macOS preferred language).
    var locale: Locale
}

extension EnvironmentValues {
    /// `ShellState.locale`, set by ShellView; read into every `RenderContext`.
    @Entry var discordLocale = Locale(identifier: "en-US")
}

/// Every `interval` from the start, or a single entry (static) without one.
struct RefreshSchedule: TimelineSchedule {
    var interval: TimeInterval?

    func entries(from start: Date, mode: TimelineScheduleMode) -> AnyIterator<Date> {
        guard let interval, interval > 0 else { return AnyIterator(CollectionOfOne(start).makeIterator()) }
        var next = start
        return AnyIterator {
            defer { next += interval }
            return next
        }
    }
}

struct ParagraphView: View {
    let build: (RenderContext) -> [Segment]
    /// The block's base size and weight (`MarkdownText`): its line box and first baseline.
    var size: CGFloat = MarkdownText.bodySize
    var weight: Font.Weight = .regular
    /// The text view's place in its `MarkdownView`'s document order.
    var participant = 0
    @Environment(\.discordLocale) private var locale
    @Environment(\.textSecondary) private var secondary

    init(size: CGFloat = MarkdownText.bodySize, weight: Font.Weight = .regular, participant: Int = 0, build: @escaping (RenderContext) -> [Segment]) {
        self.size = size
        self.weight = weight
        self.participant = participant
        self.build = build
    }

    var body: some View {
        let refresh = build(RenderContext(now: .now, locale: locale)).compactMap(\.refreshEvery).min()
        TimelineView(RefreshSchedule(interval: refresh)) { timeline in
            MessageTextBlock(leaves: [.text(segments: Self.tick(timeline.date, locale: locale, build: build), size: size, weight: weight, secondary: secondary)],
                             participant: participant)
        }
        .alignmentGuide(.firstTextBaseline) { [size, weight] _ in MainActor.assumeIsolated { TextMetrics.firstBaseline(size: size, weight: weight) } }
    }

    /// What VoiceOver reads: each run of covered (hidden spoiler) segments becomes one "Spoiler",
    /// so hidden text never reaches the accessibility tree (K2 verification A3).
    nonisolated static func spoken(_ segments: [Segment]) -> String {
        var out = ""
        var inCover = false
        for segment in segments {
            if segment.covered {
                if !inCover { out += "Spoiler" }
                inCover = true
            } else {
                inCover = false
                out += String(segment.text.characters)
            }
        }
        return out
    }

    /// What one timeline tick renders: `build` at the tick's date.
    nonisolated static func tick(_ date: Date, locale: Locale, build: (RenderContext) -> [Segment]) -> [Segment] {
        build(RenderContext(now: date, locale: locale))
    }
}

private typealias T = DesignTokens

// MARK: Blocks inside inline containers

/// A paragraph cut at the blocks inside its inline containers (`a ||```x```|| b`).
enum InlinePart: Equatable {
    /// Text, re-wrapped in the containers it sat in.
    case inlines([MarkdownInline])
    /// A block with the containers around it, so K3 can style it and a spoiler can cover it.
    case block(MarkdownBlock, InlineStyle)

    static func split(_ inlines: [MarkdownInline], style: InlineStyle = InlineStyle()) -> [InlinePart] {
        var parts: [InlinePart] = []
        func add(_ inline: MarkdownInline) {
            if case .inlines(let run)? = parts.last {
                parts[parts.count - 1] = .inlines(run + [inline])
            } else {
                parts.append(.inlines([inline]))
            }
        }
        func lift(_ content: [MarkdownInline], _ change: (inout InlineStyle) -> Void, wrap: ([MarkdownInline]) -> MarkdownInline) {
            var inner = style
            change(&inner)
            for part in split(content, style: inner) {
                switch part {
                case .inlines(let run): add(wrap(run))
                case .block: parts.append(part)
                }
            }
        }
        for node in inlines {
            guard node.containsBlock else { add(node); continue }
            switch node {
            case .block(let block): parts.append(.block(block, style))
            case .bold(let c): lift(c, { $0.bold = true }, wrap: MarkdownInline.bold)
            case .italic(let c): lift(c, { $0.italic = true }, wrap: MarkdownInline.italic)
            case .underline(let c): lift(c, { $0.underline = true }, wrap: MarkdownInline.underline)
            case .strike(let c): lift(c, { $0.strike = true }, wrap: MarkdownInline.strike)
            case .spoiler(let id, let c): lift(c, { $0.spoilers.append(id) }, wrap: { .spoiler(id: id, $0) })
            default: add(node) // link labels never hold blocks (K1 D7)
            }
        }
        return trimmingBreaks(parts)
    }

    /// The parts a paragraph renders. HTML ignores a lone `<br>` at the end of a block, so the
    /// final line break is not drawn (P1): `> single\n` is one line, two trailing breaks are one
    /// empty line. Render side only; the core AST keeps the break.
    static func paragraph(_ inlines: [MarkdownInline], style: InlineStyle = InlineStyle()) -> [InlinePart] {
        var parts = split(inlines, style: style)
        if case .inlines(var run)? = parts.last, run.last == .lineBreak {
            run.removeLast()
            if run.isEmpty { parts.removeLast() } else { parts[parts.count - 1] = .inlines(run) }
        }
        return parts
    }

    /// A line break where the paragraph is cut is dropped (plan, block grouping): K1 drops those
    /// beside an `Inline::Block`, this the ones beside a container holding one (`a\n||```x```||`).
    private static func trimmingBreaks(_ parts: [InlinePart]) -> [InlinePart] {
        parts.indices.compactMap { i in
            guard case .inlines(var run) = parts[i] else { return parts[i] }
            if i > 0, run.first == .lineBreak { run.removeFirst() }
            if i + 1 < parts.count, run.last == .lineBreak { run.removeLast() }
            return run.isEmpty ? nil : .inlines(run)
        }
    }
}

private extension MarkdownInline {
    var containsBlock: Bool {
        switch self {
        case .block: true
        case .bold(let c), .italic(let c), .underline(let c), .strike(let c), .spoiler(_, let c): c.contains { $0.containsBlock }
        default: false
        }
    }
}

/// A paragraph's inlines: one `ParagraphView`, or a vertical stack when blocks sit inside it.
/// `leaf` numbers its first text view; parts at or past the view limit are left to the remainder.
struct InlineFlow: View {
    let inlines: [MarkdownInline]
    var style = InlineStyle()
    let text: MarkdownText
    var leaf = 0

    var body: some View {
        let parts = InlinePart.paragraph(inlines, style: style)
        let starts = Self.starts(parts, from: leaf)
        VStack(alignment: .leading, spacing: T.Spacing.xs) {
            ForEach(Array(parts.enumerated()).filter { starts[$0.offset] < TextLeaves.limit }, id: \.offset) { index, part in
                switch part {
                case .inlines(let run):
                    ParagraphView(size: text.size, weight: text.weight, participant: starts[index]) { [text, style] context in
                        text.segments(run, style: style, context: context)
                    }
                case .block(let block, let blockStyle):
                    SpoilerCover(hidden: blockStyle.hiddenSpoiler(revealed: text.revealed)) {
                        BlockView(block: block, style: blockStyle, text: text, leaf: starts[index])
                    }
                }
            }
        }
    }

    static func starts(_ parts: [InlinePart], from first: Int) -> [Int] {
        var next = first
        return parts.map { part in
            defer {
                switch part {
                case .inlines: next += 1
                case .block(let block, _): next += TextLeaves.count(block)
                }
            }
            return next
        }
    }
}

/// A block under a hidden spoiler: hidden behind the spoiler's cover, which reveals that spoiler
/// (the same reveal unit as its text) through the body's link handler.
struct SpoilerCover<Content: View>: View {
    let hidden: Int?
    @ViewBuilder let content: Content
    @Environment(\.openURL) private var openURL

    var body: some View {
        if let hidden {
            content.hidden()
                .overlay {
                    RoundedRectangle(cornerRadius: MarkdownPalette.spoilerRadius).fill(MarkdownPalette.spoilerCover)
                }
                .contentShape(Rectangle())
                .onTapGesture { openURL(MarkdownText.spoilerURL(hidden)) }
        } else {
            content
        }
    }
}

// MARK: Emoji images

/// Custom-emoji images, loaded once per URL without cookies or credentials, only from Discord's
/// CDN (the URLs come from core; this re-checks the host).
@MainActor @Observable
final class EmojiImages {
    static let shared = EmojiImages()
    // ponytail: never evicted (48 px images, one per distinct emoji seen); add an LRU if memory shows it.
    private(set) var loaded: [URL: NSImage] = [:]
    /// Never retried after a failure within this run.
    @ObservationIgnored private var started: Set<URL> = []

    private static let session: URLSession = {
        let config = URLSessionConfiguration.ephemeral
        config.httpCookieStorage = nil
        config.httpShouldSetCookies = false
        config.urlCredentialStorage = nil
        return URLSession(configuration: config)
    }()

    /// Only Discord's emoji CDN over https is ever fetched (P2-4, N3).
    nonisolated static func fetchable(_ url: URL) -> Bool {
        url.scheme == "https" && url.host() == "cdn.discordapp.com"
    }

    /// Test seam: an image for `url` without any fetch.
    func insert(_ image: NSImage, for url: URL) { loaded[url] = image }

    func load(_ url: URL) {
        guard Self.fetchable(url), started.insert(url).inserted else { return }
        Task {
            guard let (data, response) = try? await Self.session.data(from: url),
                  (response as? HTTPURLResponse)?.statusCode == 200,
                  let image = NSImage(data: data) else { return }
            loaded[url] = image
        }
    }
}
