import AppKit
import SwiftUI

// What a message offers the column's selection (docs/plans/selection.md, N2 Parts and Headers):
// its header and its body's text blocks, nothing else. The same strings the views are built from,
// so the coordinator can copy rows no view shows.

/// The header participant's text: the name, then the time, laid out on one line as the SwiftUI
/// header lays them out, with clear glyphs (the SwiftUI text above them is what shows).
@MainActor
enum HeaderText {
    /// Stands between the name and the time: a space kerned to be as wide as the header's gap,
    /// copying as the official client's two lines, `name` then ` — time` (D3).
    static let separator = " "
    static let copied = "\n — "

    static func content(name: String, time: String, font design: DisplayNameStyle.Font? = nil) -> MessageTextContent {
        typealias M = DesignTokens.Message
        let paragraph = MessageTextContent.paragraphStyle(lineHeight: M.headerLineHeight)
        func run(_ string: String, _ font: NSFont, extra: [NSAttributedString.Key: Any] = [:]) -> NSAttributedString {
            var attributes: [NSAttributedString.Key: Any] = [.font: font, .foregroundColor: NSColor.clear, .paragraphStyle: paragraph]
            attributes.merge(extra) { $1 }
            return NSAttributedString(string: string, attributes: attributes)
        }
        let out = NSMutableAttributedString()
        out.append(run(name, nameFont(design)))
        let font = nameFont(design)
        let space = (separator as NSString).size(withAttributes: [.font: font]).width
        out.append(run(separator, font, extra: [.kern: M.headerGap - space, TextKey.copy: CopyText(copied)]))
        out.append(run(time, MessageTextContent.font(size: M.timestampSize, weight: .medium)))
        return MessageTextContent(text: out, chrome: nil, unwrapped: true, spoken: "")
    }

    /// `T.Typography.authorName` (16, medium) in the name style's design.
    private static func nameFont(_ design: DisplayNameStyle.Font?) -> NSFont {
        let base = MessageTextContent.font(size: DesignTokens.Message.authorSize, weight: .medium)
        let target: NSFontDescriptor.SystemDesign? = switch design {
        case .serif: .serif
        case .rounded: .rounded
        case .monospaced: .monospaced
        default: nil
        }
        guard let target, let styled = base.fontDescriptor.withDesign(target) else { return base }
        return NSFont(descriptor: styled, size: base.pointSize) ?? base
    }
}

@MainActor
enum MessageParts {
    /// A message's parts by number, as its views show them now: the header (`PartID.headerPart`)
    /// when its row has one, then each body text view in document order, the remainder past the
    /// view limit as one part numbered `viewLimit`.
    static func parts(of row: SelectionRow, locale: Locale, now: Date = .now) -> [(Int, NSAttributedString)] {
        var out: [(Int, NSAttributedString)] = []
        let message = row.message
        if row.startsGroup {
            let time = DiscordTime.header(date: message.timestamp, now: now, locale: locale)
            out.append((PartID.headerPart, HeaderText.content(name: message.authorName, time: time, font: message.displayNameStyle?.font).text))
        }
        let leaves = TextLeaves.all(message.body.blocks, text: MarkdownText(body: message.body), context: RenderContext(now: now, locale: locale))
        let limit = TextLeaves.limit
        for (i, leaf) in leaves.prefix(limit).enumerated() { out.append((i, MessageTextContent.build([leaf]).text)) }
        if leaves.count > limit { out.append((limit, MessageTextContent.build(Array(leaves[limit...])).text)) }
        return out
    }

    /// Whether two texts copy alike, which is what "the same text" means across a recycled view
    /// (an emoji whose image has loaded is one character, not `:name:`).
    static func same(_ a: NSAttributedString, _ b: NSAttributedString) -> Bool {
        MessageTextContent.copied(a, in: NSRange(location: 0, length: a.length)) == MessageTextContent.copied(b, in: NSRange(location: 0, length: b.length))
    }
}
