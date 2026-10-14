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
    /// links, `xn--` hosts and text that is not the URL confirm first (Q5); plain links, which
    /// show their canonical URL (`Numbering`), open.
    private func follow(_ link: LinkTarget) {
        guard let target = URL(string: link.url),
              let scheme = target.scheme?.lowercased(), scheme == "http" || scheme == "https",
              target.host() == link.host, target.user == nil, target.password == nil else { return }
        guard Self.needsConfirmation(link) else { return open(target) }
        confirm(LinkConfirmation(host: link.host, url: link.url) { open(target) })
    }

    static func needsConfirmation(_ link: LinkTarget) -> Bool {
        link.masked || link.visibleText != link.url || link.host.split(separator: ".").contains { $0.lowercased().hasPrefix("xn--") }
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

/// Every visual value of the block views, from Discord's markup CSS (module 75297; plan T1):
/// each constant cites its selector. Code blocks keep their K3 values until K6 replaces them.
enum MarkdownBlockStyle {
    /// Heading sizes for levels 1, 2, 3 (`.markup__75297 h1/h2/h3`: 1.5rem, 1.25rem, 1rem), bold
    /// (`h1..h6`: `font-weight: var(--font-weight-bold)`, `line-height: 1.375em`).
    static let headingSizes: [CGFloat] = [24, 20, 16]
    static let headingWeight = Font.Weight.bold
    /// `h1, h2, h3 { margin: 16px 0 8px }`. A heading's space replaces the gap between blocks when
    /// larger (CSS margins collapse).
    static let headingTop: CGFloat = 16
    static let headingBottom: CGFloat = 8
    /// `h1:first-child, h2:first-child { margin-top: 8px }`, `h3:first-child { margin-top: 4px }`,
    /// per level.
    static let headingFirstTop: [CGFloat] = [8, 8, 4]

    /// The gap between two stacked blocks that ask for none of their own. Paragraphs and subtext
    /// carry no margin in the markup CSS (a newline is a line break); lists, quotes and headings
    /// state theirs below.
    static let blockGap: CGFloat = 0

    /// Subtext (`-#`): `.markup__75297 small:not(.inlineFormat__75297)`: font-size .875rem, in
    /// `--text-subtle` (the secondary colour); its `line-height: 1.20313rem` is 19.25, which is the
    /// 1.375 ratio at 14.
    static let subtextSize: CGFloat = 14

    /// Quote: `.blockquoteContainer__75297 .blockquoteDivider__75297`: bar 4 wide, `border-radius:
    /// 4px`; `blockquote { padding-inline: 12px 8px }`; text in `--text-subtle` (the secondary
    /// colour); the container's `margin-top/bottom: var(--space-4)` (4) above and below.
    static let quoteBarWidth: CGFloat = 4
    static let quoteBarRadius: CGFloat = 4
    static let quoteBarGap: CGFloat = 12
    static let quoteEndPadding: CGFloat = 8
    static let quoteMargin: CGFloat = 4
    static let quoteBar = Color(nsColor: .separatorColor)

    /// `ul { margin-inline: 16px 0 }`: a bullet list's content sits 16 from its container; the
    /// marker hangs in that space (`list-style-position: outside`).
    static let listIndent: CGFloat = 16
    /// `ol { margin-inline-start: calc(.4em + var(--totalCharacters) * .6em) }`: the indent of an
    /// ordered list, in em of the list's font, for `--totalCharacters` marker characters. Taken as
    /// the digit count of the largest number (one digit gives 1 em = 16, as `ul`); not verified.
    static let orderedIndentBase: CGFloat = 0.4
    static let orderedIndentPerCharacter: CGFloat = 0.6
    /// `ol, ul { margin-block: 4px 0 }`.
    static let listTop: CGFloat = 4
    /// Space between items (plan T1: each item 4 below).
    static let itemGap: CGFloat = 4
    /// Marker to the item's text. Not read from the CSS: the browser's own outside-marker offset,
    /// approximated.
    static let markerGap: CGFloat = 4
    /// Bullets by nesting level, the last repeating (`ul { list-style-type: disc }`, `ol ul, ul ul
    /// { list-style-type: circle }`).
    static let bullets = ["\u{2022}", "\u{25E6}"]

    /// Code blocks: monospaced at 0.875 x body (K3, until K6).
    static let codeScale: CGFloat = 0.875
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
            self = .text(inlines: c, size: s.headingSizes[i], weight: s.headingWeight, secondary: false, top: s.headingTop, bottom: s.headingBottom)
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

    /// Numbers are computed in `Int`: core caps `start` at 1_000_000_000 (K5), and any `UInt32`
    /// plus an item index cannot overflow it. Bullets step through `bullets` by nesting depth.
    static func markers(ordered: Bool, start: UInt32, count: Int, depth: Int) -> [String] {
        if ordered { return (0..<count).map { "\(Int(start) + $0)." } }
        let bullets = MarkdownBlockStyle.bullets
        return Array(repeating: bullets[min(depth, bullets.count - 1)], count: count)
    }

    /// The block's own margins; code keeps K3's 4 until K6.
    var top: CGFloat {
        switch self {
        case .text(_, _, _, _, let top, _): top
        case .quote: MarkdownBlockStyle.quoteMargin
        case .list: MarkdownBlockStyle.listTop
        case .code: T.Spacing.xs
        }
    }
    var bottom: CGFloat {
        switch self {
        case .text(_, _, _, _, _, let bottom): bottom
        case .quote: MarkdownBlockStyle.quoteMargin
        case .list: 0
        case .code: T.Spacing.xs
        }
    }

    /// The gap between two stacked blocks: the default, or the blocks' own margins when larger.
    static func gap(after previous: BlockModel, before next: BlockModel) -> CGFloat {
        max(MarkdownBlockStyle.blockGap, previous.bottom, next.top)
    }

    /// Space above a container's first block: only headings ask for any (`h1:first-child` ...).
    static func firstTop(_ block: MarkdownBlock) -> CGFloat {
        guard case .heading(let level, _) = block else { return 0 }
        return MarkdownBlockStyle.headingFirstTop[min(max(Int(level), 1), 3) - 1]
    }

    /// Where a list's text starts from its container: 16 for bullets, and for an ordered list
    /// .4 em + n x .6 em for the n digits of its largest number.
    static func listIndent(markers: [String], size: CGFloat) -> CGFloat {
        let digits = markers.map { $0.filter(\.isNumber).count }.max() ?? 0
        guard digits > 0 else { return MarkdownBlockStyle.listIndent }
        return (MarkdownBlockStyle.orderedIndentBase + CGFloat(digits) * MarkdownBlockStyle.orderedIndentPerCharacter) * size
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
                    .padding(.top, index == 0 ? BlockModel.firstTop(block) : BlockModel.gap(after: models[index - 1], before: models[index]))
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
            // Every line is a whole line box (22 at 16): n lines are n boxes tall.
            let box = AttributedString.LineHeight.exact(points: MarkdownText.lineHeight(size: size))
            if secondary {
                InlineFlow(inlines: inlines, style: style, text: sized).foregroundStyle(.secondary).lineHeight(box)
            } else {
                InlineFlow(inlines: inlines, style: style, text: sized).lineHeight(box)
            }
        case .quote(let blocks):
            BlockList(blocks: blocks, style: style, text: text, listDepth: listDepth)
                .foregroundStyle(.secondary)
                .padding(.leading, MarkdownBlockStyle.quoteBarWidth + MarkdownBlockStyle.quoteBarGap)
                .padding(.trailing, MarkdownBlockStyle.quoteEndPadding)
                .overlay(alignment: .leading) {
                    RoundedRectangle(cornerRadius: MarkdownBlockStyle.quoteBarRadius)
                        .fill(MarkdownBlockStyle.quoteBar)
                        .frame(width: MarkdownBlockStyle.quoteBarWidth)
                }
        case .list(let markers, let items):
            // The marker hangs in the indent, right-aligned beside the item's first line.
            let indent = BlockModel.listIndent(markers: markers, size: text.size)
            VStack(alignment: .leading, spacing: MarkdownBlockStyle.itemGap) {
                ForEach(Array(items.enumerated()), id: \.offset) { index, item in
                    HStack(alignment: .firstTextBaseline, spacing: MarkdownBlockStyle.markerGap) {
                        Text(verbatim: markers[index])
                            .font(.system(size: text.size, weight: text.weight).monospacedDigit())
                            .frame(width: indent - MarkdownBlockStyle.markerGap, alignment: .trailing)
                        BlockList(blocks: item, style: style, text: text, listDepth: listDepth + 1)
                    }
                }
            }
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
