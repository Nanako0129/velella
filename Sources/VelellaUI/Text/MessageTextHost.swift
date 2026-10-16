import AppKit
import SwiftUI

// How the block views host message text views (docs/plans/selection.md, N1 Block layout and
// Limits): today's SwiftUI block layout stays, each text block is one `MessageTextView`, and a
// `MarkdownView` instance holds at most `DesignTokens.TextView.viewLimit` of them; the rest of its
// text, in document order, goes into one more view at its end.

struct CoordinatorBox: @unchecked Sendable { weak var coordinator: SelectionCoordinator? }
struct PasteboardBox: @unchecked Sendable { var board: NSPasteboard? }

extension EnvironmentValues {
    /// The owning `MarkdownView`'s selection.
    @Entry var textCoordinator = CoordinatorBox()
    /// The message column's selection (N2), set by `MessageArea`; a text view uses it only when
    /// `textMessage` is set too.
    @Entry var selectionColumn = CoordinatorBox()
    /// The message whose body (and header) the text views below belong to. Set on a row's body only:
    /// embeds and attachments leave it unset, so they take no part in the column's selection.
    @Entry var textMessage: String?
    /// Where a copy goes; nil is the general pasteboard. Tests inject their own.
    @Entry var textPasteboard = PasteboardBox()
    /// Text in the secondary colour (quotes, subtext).
    @Entry var textSecondary = false
}

/// What a view's text was built from: its leaves and which of their emoji images had loaded.
struct TextSource: Equatable {
    var leaves: [TextLeaf]
    var images: Set<URL>
}

// MARK: Representable

/// Which coordinator a text view reports to and under which message: the column's for a row's
/// body or header, else the nearest `MarkdownView`'s own (a message id of "").
@MainActor
func selectionTarget(_ environment: EnvironmentValues) -> (coordinator: SelectionCoordinator?, message: String) {
    if let column = environment.selectionColumn.coordinator, let message = environment.textMessage { return (column, message) }
    return (environment.textCoordinator.coordinator, "")
}

struct MessageTextRep: NSViewRepresentable {
    let leaves: [TextLeaf]
    let images: Set<URL>
    let participant: Int
    var onTip: (TipFrame?) -> Void = { _ in }

    func makeNSView(context: Context) -> MessageTextView { MessageTextView(frame: .zero) }

    func updateNSView(_ view: MessageTextView, context: Context) {
        let environment = context.environment
        let openURL = environment.openURL
        view.onActivate = { openURL($0.url) }
        view.onTip = onTip
        view.pasteboard = environment.textPasteboard.board ?? .general
        let source = TextSource(leaves: leaves, images: images)
        if view.source != source {
            view.setContent(.build(leaves))
            view.source = source
            view.invalidateIntrinsicContentSize()
            // Each URL is fetched once per run, and only from Discord's CDN (`EmojiImages.load`).
            for url in leaves.flatMap(\.imageURLs) { EmojiImages.shared.load(url) }
        }
        let (coordinator, message) = selectionTarget(environment)
        if view.coordinator !== coordinator || view.participant != participant || view.message != message {
            view.coordinator?.detach(view)
            view.coordinator = coordinator
            view.participant = participant
            view.message = message
        }
        coordinator?.attach(view)
    }

    static func dismantleNSView(_ view: MessageTextView, coordinator: ()) {
        view.coordinator?.detach(view)
    }

    func sizeThatFits(_ proposal: ProposedViewSize, nsView view: MessageTextView, context: Context) -> CGSize? {
        // No proposal is the text's natural width: laid out unbounded.
        let natural = view.unwrapped || proposal.width == nil || proposal.width?.isFinite == false
        let width = natural ? MessageTextView.unbounded : proposal.width!
        // SwiftUI's own Text rounds its size up to a whole pixel; so does this.
        let scale = max(context.environment.displayScale, 1)
        let height = (view.height(forWidth: width) * scale).rounded(.up) / scale
        return CGSize(width: natural ? view.naturalWidth(at: width) : width, height: height)
    }
}

/// The header participant (N2 Headers): a text view behind the SwiftUI header, holding the name and
/// the time with clear glyphs. It takes part in hit-testing and paints the selection, and is not an
/// accessibility element: the SwiftUI text above it says the same.
struct HeaderTextRep: NSViewRepresentable {
    let name: String
    let time: String
    var font: DisplayNameStyle.Font?

    func makeNSView(context: Context) -> MessageTextView {
        let view = MessageTextView(frame: .zero)
        view.isAccessible = false
        return view
    }

    func updateNSView(_ view: MessageTextView, context: Context) {
        view.pasteboard = context.environment.textPasteboard.board ?? .general
        let key = name + HeaderText.separator + time + String(describing: font)
        if view.contentKey != key {
            view.setContent(HeaderText.content(name: name, time: time, font: font))
            view.contentKey = key
            view.invalidateIntrinsicContentSize()
        }
        let (coordinator, message) = selectionTarget(context.environment)
        if view.coordinator !== coordinator || view.participant != PartID.headerPart || view.message != message {
            view.coordinator?.detach(view)
            view.coordinator = coordinator
            view.participant = PartID.headerPart
            view.message = message
        }
        coordinator?.attach(view)
    }

    static func dismantleNSView(_ view: MessageTextView, coordinator: ()) {
        view.coordinator?.detach(view)
    }

    /// Fills the header it sits behind.
    func sizeThatFits(_ proposal: ProposedViewSize, nsView view: MessageTextView, context: Context) -> CGSize? {
        CGSize(width: proposal.width ?? view.naturalWidth(at: MessageTextView.unbounded), height: proposal.height ?? DesignTokens.Message.headerLineHeight)
    }
}

/// One text view and its hover tip, shown at once (`TooltipStyle`).
struct MessageTextBlock: View {
    let leaves: [TextLeaf]
    let participant: Int
    @State private var shown: TipFrame?
    private let images = EmojiImages.shared

    var body: some View {
        // Reading `loaded` here makes the body depend on it: an image arriving rebuilds the text.
        MessageTextRep(leaves: leaves, images: Set(leaves.flatMap(\.imageURLs).filter { images.loaded[$0] != nil }), participant: participant) { shown = $0 }
            .tooltipOverlay(shown.map { TooltipRequest(text: $0.tip, rect: $0.rect) })
    }
}

/// The text past the view limit, in one view at the end of its `MarkdownView` (F-4).
struct TextRemainder: View {
    let blocks: [MarkdownBlock]
    let text: MarkdownText
    @Environment(\.discordLocale) private var locale

    var body: some View {
        let now = TextLeaves.remainder(blocks, text: text, context: RenderContext(now: .now, locale: locale))
        let refresh = now.compactMap(\.refresh).min()
        TimelineView(RefreshSchedule(interval: refresh)) { timeline in
            MessageTextBlock(leaves: refresh == nil ? now : TextLeaves.remainder(blocks, text: text, context: RenderContext(now: timeline.date, locale: locale)),
                             participant: DesignTokens.TextView.viewLimit)
        }
    }
}

// MARK: Leaves

/// Text views in document order, as the block views make them: each inline part of a paragraph,
/// heading or subtext, and each code block, is one.
enum TextLeaves {
    static let limit = DesignTokens.TextView.viewLimit

    static func count(_ blocks: [MarkdownBlock]) -> Int { blocks.reduce(0) { $0 + count($1) } }

    static func count(_ block: MarkdownBlock) -> Int {
        switch block {
        case .paragraph(let c), .heading(_, let c), .subtext(let c): count(c)
        case .quote(let blocks): count(blocks)
        case .list(_, _, let items): items.reduce(0) { $0 + count($1) }
        case .codeBlock: 1
        }
    }

    static func count(_ inlines: [MarkdownInline]) -> Int {
        InlinePart.paragraph(inlines).reduce(0) { n, part in
            switch part {
            case .inlines: n + 1
            case .block(let block, _): n + count(block)
            }
        }
    }

    /// The first leaf of each of `blocks`, from `first`.
    static func starts(_ blocks: [MarkdownBlock], from first: Int) -> [Int] {
        var next = first
        return blocks.map { block in
            defer { next += count(block) }
            return next
        }
    }

    /// The leaves numbered `limit` and later, in document order, with every rule the views keep:
    /// covered runs, the spoiler of a covered block, the code escapes.
    static func remainder(_ blocks: [MarkdownBlock], text: MarkdownText, context: RenderContext) -> [TextLeaf] {
        leaves(blocks, from: limit, text: text, context: context)
    }

    /// Every leaf, the ones the views show one each and the remainder alike (N2 builds the parts of
    /// rows that no view shows from these).
    static func all(_ blocks: [MarkdownBlock], text: MarkdownText, context: RenderContext) -> [TextLeaf] {
        leaves(blocks, from: 0, text: text, context: context)
    }

    private static func leaves(_ blocks: [MarkdownBlock], from first: Int, text: MarkdownText, context: RenderContext) -> [TextLeaf] {
        var out: [TextLeaf] = []
        var next = 0
        collect(blocks, from: first, style: InlineStyle(), text: text, listDepth: 0, secondary: false, context: context, next: &next, into: &out)
        return out
    }

    private static func collect(_ blocks: [MarkdownBlock], from first: Int, style: InlineStyle, text: MarkdownText, listDepth: Int, secondary: Bool,
                                context: RenderContext, next: inout Int, into out: inout [TextLeaf]) {
        for block in blocks {
            let n = count(block)
            if next + n <= first { next += n; continue }
            switch BlockModel(block, base: text, listDepth: listDepth) {
            case .text(let inlines, let size, let weight, let sec, _, _):
                var sized = text
                sized.size = size
                sized.weight = weight
                for part in InlinePart.paragraph(inlines, style: style) {
                    switch part {
                    case .inlines(let run):
                        if next >= first { out.append(.text(segments: sized.segments(run, style: style, context: context), size: size, weight: weight, secondary: secondary || sec)) }
                        next += 1
                    case .block(let inner, let innerStyle):
                        collect([inner], from: first, style: innerStyle, text: sized, listDepth: listDepth, secondary: secondary || sec, context: context, next: &next, into: &out)
                    }
                }
            case .quote(let inner):
                collect(inner, from: first, style: style, text: text, listDepth: listDepth, secondary: true, context: context, next: &next, into: &out)
            case .list(_, let items):
                for item in items {
                    collect(item, from: first, style: style, text: text, listDepth: listDepth + 1, secondary: secondary, context: context, next: &next, into: &out)
                }
            case .code(let code, let header, let highlight, let size):
                if next >= first { out.append(.code(code: code, header: header, highlight: highlight, size: size, hidden: style.hiddenSpoiler(revealed: text.revealed))) }
                next += 1
            case .plainCode(let code, let size):
                if next >= first { out.append(.plain(code: code, size: size, hidden: style.hiddenSpoiler(revealed: text.revealed))) }
                next += 1
            }
        }
    }
}

// MARK: Metrics

/// The first baseline TextKit gives a text (after the lift): what SwiftUI's `.firstTextBaseline`
/// alignment needs from a representable, which reports none (measured in N0: without it a list
/// item's marker aligns to the bottom of the text and rows grow).
@MainActor
enum TextMetrics {
    private struct Key: Hashable { var size: CGFloat; var weight: Font.Weight?; var header: Bool }
    private static var cache: [Key: CGFloat] = [:]

    static func firstBaseline(size: CGFloat, weight: Font.Weight) -> CGFloat {
        cached(Key(size: size, weight: weight, header: false)) {
            var text = AttributedString("H")
            text[TextFontAttribute.self] = TextFont(size: size, weight: weight, mono: false, italic: false)
            return .paragraph([Segment(text: text)], size: size, weight: weight)
        }
    }

    static func firstBaseline(codeSize size: CGFloat, header: Bool) -> CGFloat {
        cached(Key(size: size, weight: nil, header: header)) { .build([.code(code: "x", header: header ? "x" : nil, highlight: nil, size: size, hidden: nil)]) }
    }

    private static func cached(_ key: Key, _ content: () -> MessageTextContent) -> CGFloat {
        if let hit = cache[key] { return hit }
        let view = MessageTextView(frame: .zero)
        view.setContent(content())
        _ = view.height(forWidth: 500)
        var baseline: CGFloat = 0
        view.layoutManager.enumerateTextLayoutFragments(from: view.layoutManager.documentRange.location, options: [.ensuresLayout]) { fragment in
            guard let line = fragment.textLineFragments.first else { return true }
            let text = (fragment.textElement as? NSTextParagraph)?.attributedString
            let lift = text.flatMap { $0.length > 0 ? $0.attribute(TextKey.lift, at: 0, effectiveRange: nil) as? CGFloat : nil } ?? 0
            baseline = view.containerOrigin.y + fragment.layoutFragmentFrame.minY + line.typographicBounds.minY + line.glyphOrigin.y - lift
            return false
        }
        cache[key] = baseline
        return baseline
    }
}
