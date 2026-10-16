import AppKit
import SwiftUI

// What a message text view holds (docs/plans/selection.md, N1 Content): velella's AST through
// `MarkdownText`'s segments (or a code block's model), turned into an `NSAttributedString` with
// `NSAttributedString(string:attributes:)` only. Links and spoilers are velella-private indices:
// no `.link` and no URL value ever goes into storage (S-3).

/// Velella-private attributes of the storage.
enum TextKey {
    /// Int: index into the owning body's `links`.
    static let link = NSAttributedString.Key("velella.link")
    /// Int: the spoiler a hidden run reveals.
    static let spoiler = NSAttributedString.Key("velella.spoiler")
    /// Bool: the run belongs to a hidden spoiler (`Segment.covered`): clipped out and covered when
    /// drawn (layer 1), clear glyphs (layer 2).
    static let hidden = NSAttributedString.Key("velella.hidden")
    /// String: the hover tip of the run (timestamps).
    static let tip = NSAttributedString.Key("velella.tip")
    /// `CopyText`: what a copy emits for the whole run, however much of it is selected.
    static let copy = NSAttributedString.Key("velella.copy")
    /// Bool: chrome (a code block's header), skipped by copy and speech.
    static let chrome = NSAttributedString.Key("velella.chrome")
    /// CGFloat: how far the fragment draws the paragraph's glyphs above their TextKit position.
    static let lift = NSAttributedString.Key("velella.lift")
    /// `TextMarker`: a code line number drawn by the fragment, never part of the text.
    static let marker = NSAttributedString.Key("velella.marker")
    /// Bool: a code row paragraph (`MessageTextView.settleRows`).
    static let row = NSAttributedString.Key("velella.row")
    /// Bool: inline code, whose rounded fill and border the fragment draws (T1).
    static let inlineCode = NSAttributedString.Key("velella.inlineCode")
}

/// A run copied whole. A class, so that two neighbouring runs never merge into one attribute run.
final class CopyText: NSObject {
    let text: String
    init(_ text: String) { self.text = text }
}

struct TextMarker {
    var text: String
    var font: NSFont
    var color: NSColor
    /// x of the marker's leading edge, in container coordinates.
    var x: CGFloat
}

/// An image-only attachment: no file wrapper, no contents (S-17). A bare `NSTextAttachment` given
/// an image answers with a file wrapper.
final class EmojiAttachment: NSTextAttachment {
    override var fileWrapper: FileWrapper? {
        get { nil }
        set {}
    }
    override var contents: Data? {
        get { nil }
        set {}
    }
}

/// Fill, corner radius and padding around a text view's text (code blocks).
struct TextChrome: Equatable {
    var inset: CGSize
    var background: NSColor?
    var radius: CGFloat
    /// Extra space above the first paragraph, on top of `inset`: TextKit ignores
    /// `paragraphSpacingBefore` there (measured in N0), so the view offsets its container instead.
    var top: CGFloat = 0
}

/// One text block of a body: what one text view shows, or one piece of the view past the limit.
enum TextLeaf: Equatable {
    case text(segments: [Segment], size: CGFloat, weight: Font.Weight, secondary: Bool)
    /// `hidden`: the spoiler covering the block, only past the limit (a hidden block within it stays
    /// under `SpoilerCover`).
    case code(code: String, header: String?, highlight: CodeHighlight?, size: CGFloat, hidden: Int?)
    /// A block too long for line numbers: unwrapped in its own view.
    case plain(code: String, size: CGFloat, hidden: Int?)

    var imageURLs: [URL] {
        guard case .text(let segments, _, _, _) = self else { return [] }
        return segments.compactMap(\.image?.url)
    }

    var refresh: TimeInterval? {
        guard case .text(let segments, _, _, _) = self else { return nil }
        return segments.compactMap(\.refreshEvery).min()
    }
}

@MainActor
struct MessageTextContent {
    var text: NSAttributedString
    var chrome: TextChrome?
    /// Laid out without wrapping (a plain code block in its horizontal scroller).
    var unwrapped = false
    /// The accessibility value: `ParagraphView.spoken` for text, so hidden text never reaches it.
    var spoken: String

    /// Layer 2 of S-2: hidden runs get clear glyphs. Tests switch it off for the mutation runs.
    static func build(_ leaves: [TextLeaf], clearHidden: Bool = true) -> MessageTextContent {
        if leaves.count == 1, case .code(let code, let header, let highlight, let size, nil) = leaves[0] {
            let (text, chrome) = codeBlock(code: code, header: header, highlight: highlight, size: size)
            return MessageTextContent(text: text, chrome: chrome, spoken: spokenCode(code))
        }
        if leaves.count == 1, case .plain(let code, let size, nil) = leaves[0] {
            return MessageTextContent(text: plainCode(code, size: size),
                                      chrome: TextChrome(inset: CGSize(width: MarkdownBlockStyle.codePaddingH, height: MarkdownBlockStyle.codePaddingV), radius: 0),
                                      unwrapped: true, spoken: spokenCode(code))
        }
        let out = NSMutableAttributedString()
        var spoken: [String] = []
        for leaf in leaves {
            if out.length > 0 { out.append(NSAttributedString(string: "\n", attributes: [.font: NSFont.systemFont(ofSize: MarkdownText.bodySize)])) }
            switch leaf {
            case .text(let segments, let size, let weight, let secondary):
                out.append(paragraphText(segments, size: size, weight: weight, secondary: secondary, clearHidden: clearHidden))
                spoken.append(ParagraphView.spoken(segments))
            case .code(let code, let header, let highlight, let size, let hidden):
                let text = NSMutableAttributedString(attributedString: codeBlock(code: code, header: header, highlight: highlight, size: size).0)
                out.append(hidden.map { covered(text, spoiler: $0, clearHidden: clearHidden) } ?? text)
                spoken.append(hidden == nil ? spokenCode(code) : "Spoiler")
            case .plain(let code, let size, let hidden):
                let text = plainCode(code, size: size)
                out.append(hidden.map { covered(text, spoiler: $0, clearHidden: clearHidden) } ?? text)
                spoken.append(hidden == nil ? spokenCode(code) : "Spoiler")
            }
        }
        return MessageTextContent(text: out, chrome: nil, spoken: spoken.joined(separator: "\n"))
    }

    static func paragraph(_ segments: [Segment], size: CGFloat = MarkdownText.bodySize, weight: Font.Weight = .regular, secondary: Bool = false) -> MessageTextContent {
        build([.text(segments: segments, size: size, weight: weight, secondary: secondary)])
    }

    // MARK: Fonts

    private static var fonts: [TextFont: NSFont] = [:]

    static func font(_ spec: TextFont) -> NSFont {
        if let hit = fonts[spec] { return hit }
        let w: NSFont.Weight = switch spec.weight {
        case .bold: .bold
        case .semibold: .semibold
        case .medium: .medium
        default: .regular
        }
        var font = spec.mono ? NSFont.monospacedSystemFont(ofSize: spec.size, weight: w) : NSFont.systemFont(ofSize: spec.size, weight: w)
        if spec.italic { font = NSFont(descriptor: font.fontDescriptor.withSymbolicTraits(.italic), size: spec.size) ?? font }
        fonts[spec] = font
        return font
    }

    static func font(size: CGFloat, weight: Font.Weight = .regular, mono: Bool = false) -> NSFont {
        font(TextFont(size: size, weight: weight, mono: mono, italic: false))
    }

    // MARK: Paragraphs

    /// SwiftUI's exact line height leaves the glyphs at the top of the box; TextKit puts the extra
    /// height above them (measured in N0-2: 3 pt at 16). The fragment draws the glyphs `lift` higher;
    /// line boxes, selection and hit rects stay where TextKit put them.
    static func lift(size: CGFloat, mono: Bool = false, lineHeight: CGFloat) -> CGFloat {
        let f = font(size: size, mono: mono)
        return max(0, lineHeight - (f.ascender - f.descender + f.leading).rounded(.up))
    }

    static func paragraphStyle(lineHeight: CGFloat, indent: CGFloat = 0, after: CGFloat = 0) -> NSParagraphStyle {
        let style = NSMutableParagraphStyle()
        style.minimumLineHeight = lineHeight
        style.maximumLineHeight = lineHeight
        style.firstLineHeadIndent = indent
        style.headIndent = indent
        style.paragraphSpacing = after
        return style
    }

    static func paragraphText(_ segments: [Segment], size: CGFloat, weight: Font.Weight, secondary: Bool, clearHidden: Bool = true) -> NSMutableAttributedString {
        let out = NSMutableAttributedString()
        let base = secondary ? NSColor.secondaryLabelColor : NSColor.labelColor
        for segment in segments {
            if let image = segment.image, !segment.covered, let loaded = EmojiImages.shared.loaded[image.url] {
                out.append(emoji(segment, image, loaded, size: size))
                continue
            }
            for run in segment.text.runs {
                let string = String(segment.text[run.range].characters)
                let spec = run[TextFontAttribute.self] ?? TextFont(size: size, weight: weight, mono: false, italic: false)
                var attrs: [NSAttributedString.Key: Any] = [.font: font(spec), .foregroundColor: base]
                if let color = run.swiftUI.foregroundColor { attrs[.foregroundColor] = NSColor(color) }
                if run.swiftUI.underlineStyle != nil { attrs[.underlineStyle] = NSUnderlineStyle.single.rawValue }
                if run.swiftUI.strikethroughStyle != nil { attrs[.strikethroughStyle] = NSUnderlineStyle.single.rawValue }
                if let raw = run[RawTextAttribute.self] { attrs[TextKey.copy] = CopyText(raw.text) }
                let url = run.link?.absoluteString ?? ""
                if segment.covered {
                    // A covered run carries only its spoiler index: no link, fill, tip or image.
                    attrs[TextKey.hidden] = true
                    attrs[TextKey.spoiler] = Int(url.dropFirst("velella-spoiler:".count)) ?? 0
                    attrs[.foregroundColor] = clearHidden ? NSColor.clear : base
                } else {
                    if let tip = segment.hoverTip { attrs[TextKey.tip] = tip }
                    if spec.mono {
                        attrs[TextKey.inlineCode] = true
                    } else if let fill = run.swiftUI.backgroundColor {
                        attrs[.backgroundColor] = NSColor(fill)
                    }
                    if url.hasPrefix("velella-link:"), let n = Int(url.dropFirst("velella-link:".count)) { attrs[TextKey.link] = n }
                }
                out.append(NSAttributedString(string: string, attributes: attrs))
            }
        }
        holdEmptyLines(out, font: font(size: size, weight: weight))
        let lineHeight = MarkdownText.lineHeight(size: size)
        let whole = NSRange(location: 0, length: out.length)
        out.addAttribute(TextKey.lift, value: lift(size: size, lineHeight: lineHeight), range: whole)
        out.addAttribute(.paragraphStyle, value: paragraphStyle(lineHeight: lineHeight), range: whole)
        return out
    }

    /// TextKit gives an empty paragraph no minimum or maximum line height (measured in N0): a
    /// zero-width space holds each empty line open; a copy emits nothing for it.
    private static func holdEmptyLines(_ text: NSMutableAttributedString, font: NSFont) {
        let s = text.string as NSString
        var i = s.length
        while i > 0 {
            i -= 1
            guard s.character(at: i) == 0x0A, i + 1 == s.length || s.character(at: i + 1) == 0x0A else { continue }
            text.insert(NSAttributedString(string: "\u{200B}", attributes: [.font: font, TextKey.copy: CopyText("")]), at: i + 1)
        }
        if text.length == 0 { text.append(NSAttributedString(string: "\u{200B}", attributes: [.font: font, TextKey.copy: CopyText("")])) }
    }

    /// A custom emoji: one attachment character with today's `EmojiImages.loaded` image, 18 x 18 at
    /// y -3.375 at 16 pt (`InlineImage`). A copy emits `:name:`; U+FFFC never reaches the pasteboard.
    private static func emoji(_ segment: Segment, _ image: InlineImage, _ loaded: NSImage, size: CGFloat) -> NSAttributedString {
        let attachment = EmojiAttachment()
        let aspect = loaded.size.height > 0 ? min(loaded.size.width / loaded.size.height, 4) : 1
        attachment.bounds = CGRect(x: 0, y: image.baselineOffset, width: image.height * aspect, height: image.height)
        attachment.image = loaded
        let name = segment.text.runs.first?[RawTextAttribute.self]?.text ?? String(segment.text.characters)
        return NSAttributedString(string: "\u{FFFC}", attributes: [.attachment: attachment, .font: font(size: size), TextKey.copy: CopyText(name)])
    }

    /// Every character of `text` hidden under spoiler `n` (a block past the limit): both layers apply,
    /// and its line numbers are dropped, since they would be drawn outside the covered runs.
    private static func covered(_ text: NSAttributedString, spoiler n: Int, clearHidden: Bool) -> NSAttributedString {
        let out = NSMutableAttributedString(attributedString: text)
        let whole = NSRange(location: 0, length: out.length)
        out.removeAttribute(TextKey.marker, range: whole)
        out.removeAttribute(.backgroundColor, range: whole)
        out.removeAttribute(TextKey.inlineCode, range: whole)
        out.addAttributes([TextKey.hidden: true, TextKey.spoiler: n], range: whole)
        if clearHidden { out.addAttribute(.foregroundColor, value: NSColor.clear, range: whole) }
        return out
    }

    // MARK: Code

    private static func spokenCode(_ code: String) -> String { CodeBlockModel.escaped(code) }

    private static let codePlain = NSColor(Color(RGB(rgb: CodeBlockStyle.plain)))
    private static let codeBackground = NSColor(Color(CodeBlockStyle.background))
    private static var digitWidths: [CGFloat: CGFloat] = [:]

    private static func digitWidth(_ size: CGFloat) -> CGFloat {
        if let hit = digitWidths[size] { return hit }
        let width = CodeBlockView.digitWidth(size: size)
        digitWidths[size] = width
        return width
    }

    /// A code block (N0-4): header, line numbers and wrapped rows in one text, on the theme
    /// background. Rows are `CodeBlockView.attributed`; its escapes carry their raw scalars.
    static func codeBlock(code: String, header: String?, highlight: CodeHighlight?, size: CGFloat) -> (NSAttributedString, TextChrome) {
        typealias S = CodeBlockStyle
        let model = CodeBlockModel(code: code, header: header, highlight: highlight)
        let plain = codePlain
        let numberWidth = (digitWidth(size) * CGFloat(model.numbers.last?.count ?? 1)).rounded(.up)
        let codeX = S.numberLeading + numberWidth + S.numberTrailing + S.codeLeading
        let mono = font(size: size, mono: true)
        let monoLift = lift(size: size, mono: true, lineHeight: S.lineHeight)
        let out = NSMutableAttributedString()
        if let name = model.name {
            let head = NSMutableAttributedString()
            let attachment = NSTextAttachment()
            if let symbol = NSImage(systemSymbolName: S.headerSymbol, accessibilityDescription: nil) {
                attachment.image = symbol.withSymbolConfiguration(NSImage.SymbolConfiguration(pointSize: size, weight: .bold).applying(.init(paletteColors: [plain])))
            }
            head.append(NSAttributedString(attachment: attachment))
            head.addAttribute(.kern, value: S.headerIconGap, range: NSRange(location: 0, length: 1))
            head.append(NSAttributedString(string: name + "\n"))
            head.addAttributes([
                .font: font(size: size, weight: .bold), .foregroundColor: plain, TextKey.chrome: true,
                TextKey.lift: lift(size: size, lineHeight: S.lineHeight),
                .paragraphStyle: paragraphStyle(lineHeight: S.lineHeight, indent: S.headerPaddingH, after: S.headerBottom),
            ], range: NSRange(location: 0, length: head.length))
            out.append(head)
        }
        for (i, line) in model.lines.enumerated() {
            let row = NSMutableAttributedString(attributedString: codeLine(CodeBlockView.attributed(line, size: size)))
            // An empty row needs content to keep its line height (measured: 17 instead of 19).
            if row.length == 0 { row.append(NSAttributedString(string: "\u{200B}", attributes: [.font: mono, .foregroundColor: plain, TextKey.copy: CopyText("")])) }
            if i < model.lines.count - 1 { row.append(NSAttributedString(string: "\n", attributes: [.font: mono, .foregroundColor: plain])) }
            row.addAttributes([
                // One line, the usual case, needs its 1 pt already: `settleRows` then changes only wrapped rows.
                .paragraphStyle: paragraphStyle(lineHeight: S.lineHeight, indent: codeX, after: 1), TextKey.row: true, TextKey.lift: monoLift,
                TextKey.marker: TextMarker(text: model.numbers[i], font: mono, color: plain, x: S.numberLeading),
            ], range: NSRange(location: 0, length: row.length))
            out.append(row)
        }
        let pad = size * S.paddingEm
        return (out, TextChrome(inset: CGSize(width: pad, height: pad), background: codeBackground, radius: S.radius,
                                top: model.name == nil ? max(0, S.plainTopPadding - pad) : 0))
    }

    /// A code line's runs as storage text: fonts and colours from the runs, escapes copied raw.
    private static func codeLine(_ line: AttributedString) -> NSAttributedString {
        let out = NSMutableAttributedString()
        for run in line.runs {
            let spec = run[TextFontAttribute.self] ?? TextFont(size: 14, weight: .regular, mono: true, italic: false)
            var attrs: [NSAttributedString.Key: Any] = [.font: font(spec)]
            if let color = run.swiftUI.foregroundColor { attrs[.foregroundColor] = NSColor(color) }
            if run.swiftUI.underlineStyle != nil { attrs[.underlineStyle] = NSUnderlineStyle.single.rawValue }
            if let raw = run[RawTextAttribute.self] { attrs[TextKey.copy] = CopyText(raw.text) }
            out.append(NSAttributedString(string: String(line[run.range].characters), attributes: attrs))
        }
        return out
    }

    /// K3's plain block, past the line-number cap: monospaced, escaped, unwrapped.
    static func plainCode(_ code: String, size: CGFloat) -> NSAttributedString {
        let lineHeight = MarkdownText.lineHeight(size: size)
        let out = NSMutableAttributedString(attributedString: codeLine(CodeBlockView.attributed([CodeSpan(text: code, rgb: 0)], size: size)))
        let whole = NSRange(location: 0, length: out.length)
        out.addAttributes([.foregroundColor: NSColor.labelColor, .paragraphStyle: paragraphStyle(lineHeight: lineHeight),
                           TextKey.lift: lift(size: size, mono: true, lineHeight: lineHeight)], range: whole)
        holdEmptyLines(out, font: font(size: size, mono: true))
        return out
    }

    // MARK: Copy and speech

    /// What a copy of `range` puts on the pasteboard (D2): the displayed text, hidden spoilers
    /// included, no newline added; a run with a `CopyText` (emoji, substituted text, a code escape,
    /// a zero-width line holder) emits that text once, whole, even when the selection ends inside it.
    static func copied(_ text: NSAttributedString, in range: NSRange) -> String {
        let range = NSIntersectionRange(range, NSRange(location: 0, length: text.length))
        guard range.length > 0 else { return "" }
        var out = ""
        let string = text.string as NSString
        text.enumerateAttributes(in: range) { attrs, r, _ in
            if attrs[TextKey.chrome] != nil { return }
            if let copy = attrs[TextKey.copy] as? CopyText { out += copy.text; return }
            out += string.substring(with: r)
        }
        return out
    }
}
