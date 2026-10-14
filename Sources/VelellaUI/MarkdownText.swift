import AppKit
import SwiftUI

// Message-body markdown (docs/plans/markdown.md, K2): a plain mirror of core's AST, which the app
// maps FFI rows onto, and the inline mapping from it to `Segment`s. Every string in a segment
// comes from the AST, from core's resolved names or from fixed text, and attributes are set run
// by run: never `AttributedString(markdown:)`, `LocalizedStringKey` of a payload, or HTML (F5).

// MARK: AST

public enum TimestampStyle: Hashable, Sendable {
    /// `t`, `T`, `d`, `D`, `f` (the default), `F`, `s`, `S`, `R`.
    case shortTime, mediumTime, shortDate, longDate, longDateShortTime, fullDateShortTime,
         shortDateShortTime, shortDateMediumTime, relative
}

public enum NavTarget: Hashable, Sendable {
    case customize, browse, guide
    case linkedRoles(UInt64?)
}

public indirect enum MarkdownBlock: Hashable, Sendable {
    case paragraph([MarkdownInline])
    /// `level` is 1-3.
    case heading(level: UInt8, [MarkdownInline])
    case subtext([MarkdownInline])
    /// Never contains a quote.
    case quote([MarkdownBlock])
    case list(ordered: Bool, start: UInt32, items: [[MarkdownBlock]])
    case codeBlock(language: String?, code: String)
}

public indirect enum MarkdownInline: Hashable, Sendable {
    case text(String)
    case bold([MarkdownInline]), italic([MarkdownInline]), underline([MarkdownInline]), strike([MarkdownInline])
    /// `id` numbers the row's spoilers in document order; `MessageBody` assigns it.
    case spoiler(id: Int, [MarkdownInline])
    case code(String)
    /// `url` is canonical http(s), `host` its ASCII host, both from core. `id` numbers the row's
    /// links in document order; `MessageBody` assigns it.
    case link(id: Int, url: String, host: String, label: [MarkdownInline], masked: Bool)
    case userMention(UInt64), roleMention(UInt64), channelMention(UInt64)
    case everyone, here
    case customEmoji(id: UInt64, name: String, animated: Bool)
    case timestamp(unix: Int64, style: TimestampStyle)
    case slashCommand(name: String, id: UInt64)
    case guildNavigation(guild: UInt64, target: NavTarget)
    case lineBreak
    /// A block inside an inline container, kept in place (`InlinePart.split` lifts it out).
    case block(MarkdownBlock)
}

public enum MentionKind: Hashable, Sendable { case user, role, channel }

/// A mention resolved in core: `name` nil for an unknown user or channel or a deleted role;
/// `color` a role's 0xRRGGBB (0 for none).
public struct ResolvedMention: Hashable, Sendable {
    public var kind: MentionKind
    public var id: UInt64
    public var name: String?
    public var color: UInt32
    public init(kind: MentionKind, id: UInt64, name: String?, color: UInt32 = 0) {
        self.kind = kind; self.id = id; self.name = name; self.color = color
    }
}

/// A custom emoji's image; the URL is built in core from the id (CDN host, fixed size).
public struct EmojiImage: Hashable, Sendable {
    public var id: UInt64
    public var animated: Bool
    public var url: URL
    public init(id: UInt64, animated: Bool, url: URL) {
        self.id = id; self.animated = animated; self.url = url
    }
}

/// What `velella-link:<id>` stands for.
public struct LinkTarget: Hashable, Sendable {
    public var url: String
    public var host: String
    public var masked: Bool
    /// The label's text, compared with `url` by the confirmation rule.
    public var visibleText: String
}

/// A row's body with its side data. Links and spoilers are numbered here, in document order, so
/// `velella-link:<n>` and `velella-spoiler:<n>` index `links` and the spoilers.
public struct MessageBody: Hashable, Sendable {
    public private(set) var blocks: [MarkdownBlock] = []
    public private(set) var mentions: [ResolvedMention] = []
    /// At most 64 (core's limit); an emoji without an entry renders as `:name:`.
    public private(set) var emoji: [EmojiImage] = []
    public private(set) var links: [LinkTarget] = []
    public private(set) var spoilerCount = 0

    /// Any `id`s in `blocks` are replaced by the numbering.
    public init(blocks: [MarkdownBlock] = [], mentions: [ResolvedMention] = [], emoji: [EmojiImage] = []) {
        var numbering = Numbering()
        self.blocks = numbering.blocks(blocks)
        self.mentions = mentions
        self.emoji = emoji
        links = numbering.links
        spoilerCount = numbering.spoilers
    }

    /// Plain text as one paragraph (fixtures, previews).
    public init(text: String) {
        self.init(blocks: text.isEmpty ? [] : [.paragraph([.text(text)])])
    }

    func mention(_ kind: MentionKind, _ id: UInt64) -> ResolvedMention? {
        mentions.first { $0.kind == kind && $0.id == id }
    }

    func emojiURL(_ id: UInt64, animated: Bool) -> URL? {
        emoji.first { $0.id == id && $0.animated == animated }?.url
    }
}

private struct Numbering {
    var links: [LinkTarget] = []
    var spoilers = 0

    mutating func blocks(_ blocks: [MarkdownBlock]) -> [MarkdownBlock] { blocks.map { block($0) } }

    mutating func block(_ block: MarkdownBlock) -> MarkdownBlock {
        switch block {
        case .paragraph(let c): .paragraph(inlines(c))
        case .heading(let level, let c): .heading(level: level, inlines(c))
        case .subtext(let c): .subtext(inlines(c))
        case .quote(let b): .quote(blocks(b))
        case .list(let ordered, let start, let items): .list(ordered: ordered, start: start, items: items.map { blocks($0) })
        case .codeBlock: block
        }
    }

    mutating func inlines(_ inlines: [MarkdownInline]) -> [MarkdownInline] { inlines.map { inline($0) } }

    mutating func inline(_ inline: MarkdownInline) -> MarkdownInline {
        switch inline {
        case .bold(let c): return .bold(inlines(c))
        case .italic(let c): return .italic(inlines(c))
        case .underline(let c): return .underline(inlines(c))
        case .strike(let c): return .strike(inlines(c))
        case .spoiler(_, let c):
            let id = spoilers
            spoilers += 1
            return .spoiler(id: id, inlines(c))
        case .link(_, let url, let host, let label, let masked):
            let id = links.count
            // A plain link shows its canonical URL, as the official client does (K0); the one
            // label feeds both what is drawn and the confirmation rule.
            let shown = masked ? inlines(label) : [.text(url)]
            links.append(LinkTarget(url: url, host: host, masked: masked, visibleText: MarkdownText.plainText(shown)))
            return .link(id: id, url: url, host: host, label: shown, masked: masked)
        case .block(let b): return .block(block(b))
        default: return inline
        }
    }
}

// MARK: Inline mapping

/// One piece of a paragraph (the plan's `Segment`). `ParagraphView` turns each into a `Text`.
struct Segment: Equatable {
    var text: AttributedString
    /// Shown on hover after 0.5 s (timestamps).
    var hoverTip: String?
    /// Rebuild the paragraph this often (relative timestamps).
    var refreshEvery: TimeInterval?
    /// A custom emoji's image, drawn in place of `text` (its alt text) once loaded.
    var image: InlineImage?
    /// Part of a hidden spoiler: drawn as a cover, never as glyphs.
    var covered = false

    /// Plain runs merge with their neighbours.
    fileprivate var isPlain: Bool { hoverTip == nil && refreshEvery == nil && image == nil }
}

struct InlineImage: Equatable {
    var url: URL
    /// At most the line height of the run's font.
    var height: CGFloat
    var baselineOffset: CGFloat
}

/// The containers around a run. Text runs take bold, italic, underline and strike from it; a
/// block inside containers gets it too (K3 applies it to the block's text).
struct InlineStyle: Hashable {
    var bold = false, italic = false, underline = false, strike = false
    /// Enclosing spoilers, outermost first.
    var spoilers: [Int] = []
    /// The enclosing link's id.
    var link: Int?

    /// The outermost spoiler still hidden: it covers the run, and its reveal is the run's only link.
    func hiddenSpoiler(revealed: Set<Int>) -> Int? { spoilers.first { !revealed.contains($0) } }
}

/// Run colours (plan K2 values; v2 has no markdown source to measure).
enum MarkdownPalette {
    /// Inline code: secondarySystemFill × 0.45 (plan K2; K3's code blocks use the same).
    static let codeFill = Color(nsColor: .secondarySystemFill).opacity(0.45)
    /// Mentions, @everyone/@here, slash commands and guild links: systemBlue × 0.22 (plan K2).
    static let mentionFill = Color(nsColor: .systemBlue).opacity(0.22)
    /// A role mention's own colour at the mention fill's strength.
    static let roleFillOpacity = 0.22
    /// Timestamps sit on the code fill until K0 shows the official chip.
    static let timestampFill = codeFill
    /// A hidden spoiler's cover. Provisional until K0 captures the official fill.
    static let spoilerCover = Color(nsColor: .tertiaryLabelColor)
    static let spoilerRadius: CGFloat = 3
    /// A revealed spoiler keeps a faint chip, as the official client does. Provisional.
    static let revealedSpoiler = Color.primary.opacity(0.08)
}

/// Maps inlines to segments (plan K2 "Inline mapping"). `revealed` holds the row's opened spoilers.
struct MarkdownText {
    var body: MessageBody
    var revealed: Set<Int> = []
    /// Base font of the runs; K3 sets these for headings and subtext.
    var size: CGFloat = MarkdownText.bodySize
    var weight: Font.Weight = .regular

    /// The message body's size: 16 (`.markup__75297`: font-size 1rem; `DesignTokens.Message.bodySize`).
    static let bodySize = DesignTokens.Message.bodySize
    /// Inline code is monospaced at 85 % of the run's size (`.markup__75297 code.inline`: font-size 85%).
    static let codeScale: CGFloat = 0.85

    /// A line box is 1.375 em tall: the body's `--chat-markup-line-height: 1.375rem` (22 at 16), a
    /// heading's `line-height: 1.375em`, and subtext's `1.20313rem` (19.25 at 14, the same ratio).
    static let lineHeightRatio: CGFloat = 1.375

    /// The line box of text at `size`; `BlockView` applies it with `.lineHeight(.exact)`. Measured
    /// on SwiftUI's own layout (T1 report): that makes n lines exactly n boxes tall. The plan's
    /// first idea, line spacing = box - NSFont's ascender - descender + leading (18.84 at 16), is
    /// off, because SwiftUI lays a 16 pt line out 19 tall: lines came 22.16 apart and drifted.
    static func lineHeight(size: CGFloat) -> CGFloat { size * lineHeightRatio }

    func segments(_ inlines: [MarkdownInline], style: InlineStyle = InlineStyle(), context: RenderContext) -> [Segment] {
        var out: [Segment] = []
        for node in inlines { append(node, style, context, &out) }
        return Self.coalesced(out)
    }

    private func append(_ node: MarkdownInline, _ style: InlineStyle, _ context: RenderContext, _ out: inout [Segment]) {
        func nested(_ content: [MarkdownInline], _ change: (inout InlineStyle) -> Void) {
            var inner = style
            change(&inner)
            for node in content { append(node, inner, context, &out) }
        }
        switch node {
        case .text(let text): out.append(run(text, style))
        case .bold(let c): nested(c) { $0.bold = true }
        case .italic(let c): nested(c) { $0.italic = true }
        case .underline(let c): nested(c) { $0.underline = true }
        case .strike(let c): nested(c) { $0.strike = true }
        case .spoiler(let id, let c): nested(c) { $0.spoilers.append(id) }
        case .link(let id, _, _, let label, _): nested(label) { $0.link = id }
        case .code(let text): out.append(run(text, style, code: true))
        case .userMention(let id): out.append(mention(.user, id, style))
        case .roleMention(let id): out.append(mention(.role, id, style))
        case .channelMention(let id): out.append(mention(.channel, id, style))
        case .everyone: out.append(run("@everyone", style, fill: MarkdownPalette.mentionFill))
        case .here: out.append(run("@here", style, fill: MarkdownPalette.mentionFill))
        case .customEmoji(let id, let name, let animated):
            var segment = run(Self.isolated(":\(name):"), style)
            if !segment.covered, let url = body.emojiURL(id, animated: animated) {
                let font = NSFont.systemFont(ofSize: size)
                segment.image = InlineImage(url: url, height: (font.ascender - font.descender).rounded(.down), baselineOffset: font.descender)
            }
            out.append(segment)
        case .timestamp(let unix, let timeStyle):
            let shown = Self.timestamp(unix: unix, style: timeStyle, context: context)
            var segment = run(shown.text, style, fill: MarkdownPalette.timestampFill)
            if !segment.covered {
                segment.hoverTip = shown.hoverTip
                segment.refreshEvery = shown.refreshEvery
            }
            out.append(segment)
        case .slashCommand(let name, _): out.append(run(Self.isolated("/\(name)"), style, fill: MarkdownPalette.mentionFill))
        case .guildNavigation(_, let target): out.append(run(Self.label(target), style, fill: MarkdownPalette.mentionFill))
        case .lineBreak: out.append(run("\n", style))
        // `InlinePart.split` lifts blocks out before mapping; a direct caller still gets the text.
        case .block(let block): out.append(run(Self.plainText([.block(block)]), style))
        }
    }

    private func mention(_ kind: MentionKind, _ id: UInt64, _ style: InlineStyle) -> Segment {
        let resolved = body.mention(kind, id)
        let text = switch kind {
        case .user: "@" + (resolved?.name ?? "unknown-user")
        case .role: "@" + (resolved?.name ?? "deleted-role")
        case .channel: "#" + (resolved?.name ?? "unknown")
        }
        guard let color = resolved?.color, color != 0 else {
            return run(Self.isolated(text), style, fill: MarkdownPalette.mentionFill)
        }
        let role = Color(RGB(rgb: color))
        return run(Self.isolated(text), style, fill: role.opacity(MarkdownPalette.roleFillOpacity), color: role)
    }

    /// One run with the containers' attributes. Inside a hidden spoiler the run carries only the
    /// spoiler's reveal link (no inner link, fill, tip or image) and is marked covered.
    private func run(_ string: String, _ style: InlineStyle, code: Bool = false, fill: Color? = nil, color: Color? = nil) -> Segment {
        var text = AttributedString(string)
        var font: Font = code
            ? .system(size: size * Self.codeScale, weight: style.bold ? .semibold : weight, design: .monospaced)
            : .system(size: size, weight: style.bold ? .semibold : weight)
        if style.italic { font = font.italic() }
        text.swiftUI.font = font
        if style.underline { text.swiftUI.underlineStyle = .single }
        if style.strike { text.swiftUI.strikethroughStyle = .single }
        if let hidden = style.hiddenSpoiler(revealed: revealed) {
            text.link = Self.spoilerURL(hidden)
            text.swiftUI.foregroundColor = .clear
            text.swiftUI.backgroundColor = MarkdownPalette.spoilerCover
            return Segment(text: text, covered: true)
        }
        if let background = code ? MarkdownPalette.codeFill : fill {
            text.swiftUI.backgroundColor = background
        } else if !style.spoilers.isEmpty {
            text.swiftUI.backgroundColor = MarkdownPalette.revealedSpoiler
        }
        if let color { text.swiftUI.foregroundColor = color }
        if let link = style.link { text.link = Self.linkURL(link) }
        return Segment(text: text)
    }

    static func coalesced(_ segments: [Segment]) -> [Segment] {
        var out: [Segment] = []
        for segment in segments {
            if let last = out.last, last.isPlain, segment.isPlain, last.covered == segment.covered {
                out[out.count - 1].text += segment.text
            } else {
                out.append(segment)
            }
        }
        return out
    }

    // MARK: The timestamp case (K4)

    /// A timestamp's text in the shell locale, its hover tip (the `F` style) and, for `R`, how
    /// often to rebuild it.
    static func timestamp(unix: Int64, style: TimestampStyle, context: RenderContext) -> (text: String, hoverTip: String?, refreshEvery: TimeInterval?) {
        (
            DiscordTime.format(unix: unix, style: style, locale: context.locale, now: context.now),
            DiscordTime.tooltip(unix: unix, locale: context.locale, now: context.now),
            style == .relative ? DiscordTime.refreshInterval(for: unix, now: context.now) : nil
        )
    }

    // MARK: Helpers

    static func linkURL(_ id: Int) -> URL { URL(string: "velella-link:\(id)")! }
    static func spoilerURL(_ id: Int) -> URL { URL(string: "velella-spoiler:\(id)")! }

    /// FSI…PDI around a substituted string, so bidi controls in it cannot reorder the line (P2-3).
    /// Inside, a PDI with no isolate of the string's own to close is dropped (it would close the
    /// FSI early), and every isolate the string leaves open is closed, so the final PDI always
    /// closes the FSI (K2 verification F1). Embeddings and overrides end at that PDI (UAX #9).
    static func isolated(_ s: String) -> String {
        var out = String.UnicodeScalarView(["\u{2068}"])
        var open = 0
        for scalar in s.unicodeScalars {
            switch scalar.value {
            case 0x2066...0x2068: open += 1
            case 0x2069:
                guard open > 0 else { continue }
                open -= 1
            default: break
            }
            out.append(scalar)
        }
        out.append(contentsOf: Array(repeating: Unicode.Scalar(0x2069)!, count: open + 1))
        return String(out)
    }

    /// Discord's labels for guild navigation links.
    static func label(_ target: NavTarget) -> String {
        switch target {
        case .customize: "Channels & Roles"
        case .browse: "Browse Channels"
        case .guide: "Server Guide"
        case .linkedRoles: "Linked Roles"
        }
    }

    /// The text and code of `inlines`, blocks included; markup and tokens dropped.
    static func plainText(_ inlines: [MarkdownInline]) -> String {
        inlines.map { inline in
            switch inline {
            case .text(let s), .code(let s): s
            case .bold(let c), .italic(let c), .underline(let c), .strike(let c), .spoiler(_, let c): plainText(c)
            case .link(_, _, _, let label, _): plainText(label)
            case .lineBreak: "\n"
            case .block(let block): plainText(block)
            default: ""
            }
        }.joined()
    }

    static func plainText(_ block: MarkdownBlock) -> String {
        switch block {
        case .paragraph(let c), .heading(_, let c), .subtext(let c): plainText(c)
        case .quote(let blocks): blocks.map(plainText).joined(separator: "\n")
        case .list(_, _, let items): items.map { $0.map(plainText).joined(separator: "\n") }.joined(separator: "\n")
        case .codeBlock(_, let code): code
        }
    }
}
