import AppKit
import SwiftUI

private typealias T = DesignTokens

/// A message body (docs/plans/markdown.md, K2). Every `.link` in it is `velella-link:<n>` or
/// `velella-spoiler:<n>`, and this view's `openURL` handler is the only way from one to anything:
/// it reveals spoilers (per row) and opens links through `LinkRouter`. Blocks render through
/// `BlockView` (K3).
struct MarkdownView: View {
    let content: MessageBody
    /// Opens a vetted http(s) link outside the app; tests inject a recorder.
    var opener: (URL) -> Void = { _ = NSWorkspace.shared.open($0) }
    @State private var revealed: Set<Int> = []
    @State private var confirmation: LinkConfirmation?

    var body: some View {
        BlockList(blocks: content.blocks, text: MarkdownText(body: content, revealed: revealed))
            .equatable()
            .environment(\.openURL, OpenURLAction { router.handle($0) })
            .confirmationDialog(
                "Open this link in your browser?",
                isPresented: Binding(get: { confirmation != nil }, set: { if !$0 { confirmation = nil } }),
                presenting: confirmation
            ) { request in
                Button("Open Link") { request.accept() }
                Button("Cancel", role: .cancel) {}
            } message: { request in
                Text(verbatim: request.message)
            }
            // An edit renumbers the spoilers: nothing stays open under a new number.
            .onChange(of: content.blocks) { revealed = [] }
    }

    private var router: LinkRouter {
        LinkRouter(body: content, reveal: { revealed.insert($0) }, confirm: { confirmation = $0 }, open: opener)
    }
}

/// The link and spoiler handler (plan K2, P1-2, P2-1, P2-2). Always `.handled`: nothing reaches
/// the system's URL handling.
struct LinkRouter {
    let body: MessageBody
    let reveal: (Int) -> Void
    let confirm: (LinkConfirmation) -> Void
    let open: (URL) -> Void

    func handle(_ url: URL) -> OpenURLAction.Result {
        let parts = url.absoluteString.split(separator: ":", maxSplits: 1)
        guard parts.count == 2, let n = Int(parts[1]), n >= 0 else { return .handled }
        switch parts[0] {
        case "velella-spoiler" where n < body.spoilerCount: reveal(n)
        case "velella-link" where n < body.links.count: follow(body.links[n])
        default: break
        }
        return .handled
    }

    /// Rebuilt from core's canonical URL and re-checked: http(s), core's host, no userinfo. Masked
    /// links, `xn--` hosts and text that is not the URL confirm first (Q5); plain links open.
    private func follow(_ link: LinkTarget) {
        guard let target = URL(string: link.url),
              let scheme = target.scheme?.lowercased(), scheme == "http" || scheme == "https",
              target.host() == link.host, target.user == nil, target.password == nil else { return }
        guard Self.needsConfirmation(link) else { return open(target) }
        confirm(LinkConfirmation(host: link.host, url: link.url) { open(target) })
    }

    static func needsConfirmation(_ link: LinkTarget) -> Bool {
        // WHATWG adds the "/" of an empty path: `https://a.b` shows as typed, canonical `https://a.b/`.
        let showsURL = link.visibleText == link.url || link.visibleText + "/" == link.url
        return link.masked || !showsURL || link.host.split(separator: ".").contains { $0.lowercased().hasPrefix("xn--") }
    }
}

/// What the confirmation shows; `accept` opens the link.
struct LinkConfirmation {
    let host: String
    let url: String
    let accept: () -> Void

    /// Longest URL shown in full; longer ones lose their middle.
    static let urlLimit = 120

    /// The ASCII host on its own line, then the canonical URL, with control and format
    /// characters escaped and the URL middle-truncated.
    var message: String { Self.escaped(host) + "\n\n" + Self.middleTruncated(Self.escaped(url), limit: Self.urlLimit) }

    static func escaped(_ s: String) -> String {
        s.unicodeScalars.map { scalar in
            switch scalar.properties.generalCategory {
            case .control, .format: String(format: "\\u{%04X}", scalar.value)
            default: String(scalar)
            }
        }.joined()
    }

    static func middleTruncated(_ s: String, limit: Int) -> String {
        guard s.count > limit else { return s }
        let head = (limit - 1) * 2 / 3
        return s.prefix(head) + "\u{2026}" + s.suffix(limit - 1 - head)
    }
}

// MARK: Block style (plan K3)

/// Every visual value of the block views. "v2" is Swiftcord v2's measured value (plan K3);
/// "provisional" values have no source yet and await the K0 capture of the official client.
enum MarkdownBlockStyle {
    /// Heading sizes for levels 1, 2, 3, semibold (v2).
    static let headingSizes: [CGFloat] = [24, 20, 16]
    static let headingWeight = Font.Weight.semibold
    /// Space above and below a heading, per level. Provisional, awaiting K0. A heading's space
    /// replaces the default gap between blocks when larger; nothing sits above a list's first block.
    static let headingTop: [CGFloat] = [12, 10, 8]
    static let headingBottom: [CGFloat] = [4, 4, 2]

    /// Subtext (`-#`): 11 pt in the secondary colour (plan K3).
    static let subtextSize: CGFloat = 11

    /// Quote bar: 4 pt wide, rounded, in the separator colour, 8 pt from the text. Provisional,
    /// awaiting the K0 capture of the official client (Discord's bar is a muted grey of this width).
    static let quoteBarWidth: CGFloat = 4
    static let quoteBarGap: CGFloat = 8
    static let quoteBar = Color(nsColor: .separatorColor)

    /// Indent of every list from its container's leading edge, so a nested list steps in by it
    /// plus the marker column. Provisional, awaiting K0.
    static let listIndent: CGFloat = 8
    /// Gap between the marker column and the item, and between items. Provisional, awaiting K0.
    static let markerGap: CGFloat = 6
    static let itemGap: CGFloat = 2
    /// Bullets by nesting level, the last repeating. Provisional, awaiting K0.
    static let bullets = ["\u{2022}", "\u{25E6}", "\u{25AA}"]

    /// Code blocks: monospaced at 0.875 x body, as inline code (plan K2/K3).
    static let codeScale = MarkdownText.codeScale
    /// The same fill as inline code (secondarySystemFill x 0.45).
    static let codeFill = MarkdownPalette.codeFill
    /// 0.5 pt separator colour x 0.35 border, radius 4 (v2).
    static let codeBorder = Color(nsColor: .separatorColor).opacity(0.35)
    static let codeBorderWidth: CGFloat = 0.5
    static let codeRadius: CGFloat = 4
    /// Padding inside the code block. Provisional, awaiting K0.
    static let codePaddingH: CGFloat = 8
    static let codePaddingV: CGFloat = 6
}

/// What a block draws as: a pure mapping from the AST, so the structure is testable without a view.
enum BlockModel: Equatable {
    /// A paragraph, heading or subtext: inlines at `size`/`weight`, in the secondary colour or not.
    case text(inlines: [MarkdownInline], size: CGFloat, weight: Font.Weight, secondary: Bool, top: CGFloat, bottom: CGFloat)
    /// One bar and indent around the blocks (K1: a quote never holds a quote).
    case quote([MarkdownBlock])
    /// `markers[i]` leads `items[i]`; the items' own lists sit one level deeper.
    case list(markers: [String], items: [[MarkdownBlock]])
    /// Monospaced, unwrapped, horizontally scrollable.
    case code(String, size: CGFloat)

    /// `base` is the surrounding text (size, weight); `listDepth` the number of lists around the block.
    init(_ block: MarkdownBlock, base: MarkdownText, listDepth: Int) {
        switch block {
        case .paragraph(let c):
            self = .text(inlines: c, size: base.size, weight: base.weight, secondary: false, top: 0, bottom: 0)
        case .heading(let level, let c):
            let i = min(max(Int(level), 1), 3) - 1
            let s = MarkdownBlockStyle.self
            self = .text(inlines: c, size: s.headingSizes[i], weight: s.headingWeight, secondary: false, top: s.headingTop[i], bottom: s.headingBottom[i])
        case .subtext(let c):
            self = .text(inlines: c, size: MarkdownBlockStyle.subtextSize, weight: base.weight, secondary: true, top: 0, bottom: 0)
        case .quote(let blocks):
            self = .quote(blocks)
        case .list(let ordered, let start, let items):
            self = .list(markers: Self.markers(ordered: ordered, start: start, count: items.count, depth: listDepth), items: items)
        case .codeBlock(_, let code):
            self = .code(code, size: base.size * MarkdownBlockStyle.codeScale)
        }
    }

    /// Numbers are computed in `Int`: `start` is at most 999_999_999 (K1) and a `UInt32` plus any
    /// item index cannot overflow it. Bullets step through `bullets` by nesting depth.
    static func markers(ordered: Bool, start: UInt32, count: Int, depth: Int) -> [String] {
        if ordered { return (0..<count).map { "\(Int(start) + $0)." } }
        let bullets = MarkdownBlockStyle.bullets
        return Array(repeating: bullets[min(depth, bullets.count - 1)], count: count)
    }

    var top: CGFloat { if case .text(_, _, _, _, let top, _) = self { top } else { 0 } }
    var bottom: CGFloat { if case .text(_, _, _, _, _, let bottom) = self { bottom } else { 0 } }

    /// The gap between two stacked blocks: the default, or a heading's own space when larger.
    static func gap(after previous: BlockModel, before next: BlockModel) -> CGFloat {
        max(T.Spacing.xs, previous.bottom, next.top)
    }
}

extension MarkdownText: Equatable {
    /// Row view models are `Equatable` so an unchanged row is skipped (plan K3).
    static func == (a: MarkdownText, b: MarkdownText) -> Bool {
        a.size == b.size && a.weight == b.weight && a.revealed == b.revealed && a.body == b.body
    }
}

// MARK: Block views

/// Blocks stacked in order. `listDepth` counts the lists around them (bullet glyph by level).
struct BlockList: View, Equatable {
    let blocks: [MarkdownBlock]
    var style = InlineStyle()
    let text: MarkdownText
    var listDepth = 0

    var body: some View {
        let models = blocks.map { BlockModel($0, base: text, listDepth: listDepth) }
        VStack(alignment: .leading, spacing: 0) {
            ForEach(Array(blocks.enumerated()), id: \.offset) { index, block in
                BlockView(block: block, style: style, text: text, listDepth: listDepth)
                    .padding(.top, index == 0 ? 0 : BlockModel.gap(after: models[index - 1], before: models[index]))
            }
        }
    }
}

/// One block (plan K3). `style` holds the inline containers a block sits in; its text runs take
/// them (code blocks do not). Run fonts are set on the runs, so sizes and weights go through
/// `MarkdownText`, not `.font`.
struct BlockView: View, Equatable {
    let block: MarkdownBlock
    var style = InlineStyle()
    let text: MarkdownText
    var listDepth = 0

    var body: some View {
        switch BlockModel(block, base: text, listDepth: listDepth) {
        case .text(let inlines, let size, let weight, let secondary, _, _):
            let sized = { var t = text; t.size = size; t.weight = weight; return t }()
            if secondary {
                InlineFlow(inlines: inlines, style: style, text: sized).foregroundStyle(.secondary)
            } else {
                InlineFlow(inlines: inlines, style: style, text: sized)
            }
        case .quote(let blocks):
            BlockList(blocks: blocks, style: style, text: text, listDepth: listDepth)
                .padding(.leading, MarkdownBlockStyle.quoteBarWidth + MarkdownBlockStyle.quoteBarGap)
                .overlay(alignment: .leading) {
                    RoundedRectangle(cornerRadius: MarkdownBlockStyle.quoteBarWidth / 2)
                        .fill(MarkdownBlockStyle.quoteBar)
                        .frame(width: MarkdownBlockStyle.quoteBarWidth)
                }
        case .list(let markers, let items):
            Grid(alignment: .topLeading, horizontalSpacing: MarkdownBlockStyle.markerGap, verticalSpacing: MarkdownBlockStyle.itemGap) {
                ForEach(Array(items.enumerated()), id: \.offset) { index, item in
                    GridRow(alignment: .firstTextBaseline) {
                        Text(verbatim: markers[index])
                            .font(.system(size: text.size, weight: text.weight).monospacedDigit())
                            .gridColumnAlignment(.trailing)
                        BlockList(blocks: item, style: style, text: text, listDepth: listDepth + 1)
                    }
                }
            }
            .padding(.leading, MarkdownBlockStyle.listIndent)
        case .code(let code, let size):
            // A line that fits is drawn in place; a wider block scrolls sideways, never wraps.
            let lines = Text(verbatim: code)
                .font(.system(size: size, design: .monospaced))
                .fixedSize(horizontal: true, vertical: true)
                .padding(.horizontal, MarkdownBlockStyle.codePaddingH)
                .padding(.vertical, MarkdownBlockStyle.codePaddingV)
            ViewThatFits(in: .horizontal) {
                lines.frame(maxWidth: .infinity, alignment: .leading)
                ScrollView(.horizontal) { lines }
            }
            .background(MarkdownBlockStyle.codeFill)
            .clipShape(RoundedRectangle(cornerRadius: MarkdownBlockStyle.codeRadius))
            .overlay {
                RoundedRectangle(cornerRadius: MarkdownBlockStyle.codeRadius)
                    .strokeBorder(MarkdownBlockStyle.codeBorder, lineWidth: MarkdownBlockStyle.codeBorderWidth)
            }
        }
    }
}

// MARK: Previews

private func blockPreview(_ blocks: [MarkdownBlock]) -> some View {
    BlockList(blocks: blocks, text: MarkdownText(body: MessageBody(blocks: blocks))).padding(T.Spacing.lg).frame(width: 360)
}

#Preview("Headings and subtext") {
    blockPreview([
        .heading(level: 1, [.text("Heading one")]), .heading(level: 2, [.text("Heading two")]),
        .heading(level: 3, [.text("Heading three")]), .paragraph([.text("Body text.")]), .subtext([.text("Subtext")]),
    ])
}

#Preview("Quote") {
    blockPreview([
        .paragraph([.text("before")]),
        .quote([.paragraph([.text("A quoted line that is long enough to wrap onto a second line in the column.")]),
                .list(ordered: false, start: 1, items: [[.paragraph([.text("in a quote")])]])]),
        .paragraph([.text("after")]),
    ])
}

#Preview("Lists") {
    blockPreview([
        .list(ordered: false, start: 1, items: [
            [.paragraph([.text("one")]), .list(ordered: false, start: 1, items: [[.paragraph([.text("nested")]), .list(ordered: false, start: 1, items: [[.paragraph([.text("deeper")])]])]])],
            [.paragraph([.text("two")])],
        ]),
        .list(ordered: true, start: 8, items: (0..<4).map { [.paragraph([.text("item \($0)")])] }),
        .list(ordered: true, start: 999_999_999, items: (0..<3).map { [.paragraph([.text("big \($0)")])] }),
    ])
}

#Preview("Code block") {
    blockPreview([
        .paragraph([.text("see:")]),
        .codeBlock(language: "swift", code: "let x = 1\nlet reallyLongLineThatDoesNotWrapButScrollsSideways = x + 2 + 3 + 4 + 5 + 6 + 7 + 8 + 9"),
    ])
}

#Preview("Blocks in containers") {
    blockPreview([.paragraph([
        .bold([.block(.heading(level: 2, [.text("bold heading")]))]),
        .italic([.block(.quote([.paragraph([.text("italic quote")])]))]),
        .spoiler(id: 0, [.block(.codeBlock(language: nil, code: "hidden code"))]),
    ])])
}
