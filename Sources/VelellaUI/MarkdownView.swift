import AppKit
import SwiftUI

private typealias T = DesignTokens

/// A message body (docs/plans/markdown.md, K2). Every `.link` in it is `velella-link:<n>` or
/// `velella-spoiler:<n>`, and this view's `openURL` handler is the only way from one to anything:
/// it reveals spoilers (per row) and opens links through `LinkRouter`. Blocks render through
/// `BlockView`, which K3 styles.
struct MarkdownView: View {
    let content: MessageBody
    /// Opens a vetted http(s) link outside the app; tests inject a recorder.
    var opener: (URL) -> Void = { _ = NSWorkspace.shared.open($0) }
    @State private var revealed: Set<Int> = []
    @State private var confirmation: LinkConfirmation?

    var body: some View {
        BlockList(blocks: content.blocks, text: MarkdownText(body: content, revealed: revealed))
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

/// Blocks stacked in order.
struct BlockList: View {
    let blocks: [MarkdownBlock]
    var style = InlineStyle()
    let text: MarkdownText

    var body: some View {
        VStack(alignment: .leading, spacing: T.Spacing.xs) {
            ForEach(Array(blocks.enumerated()), id: \.offset) { _, block in
                BlockView(block: block, style: style, text: text)
            }
        }
    }
}

/// One block, plain until K3 styles it (headings, subtext, quotes, lists, code blocks). `style`
/// holds the inline containers a block sits in; its text runs take them.
struct BlockView: View {
    let block: MarkdownBlock
    var style = InlineStyle()
    let text: MarkdownText

    var body: some View {
        switch block {
        case .paragraph(let c), .heading(_, let c), .subtext(let c):
            InlineFlow(inlines: c, style: style, text: text)
        case .quote(let blocks):
            BlockList(blocks: blocks, style: style, text: text)
        case .list(let ordered, let start, let items):
            VStack(alignment: .leading, spacing: T.Spacing.xs) {
                ForEach(Array(items.enumerated()), id: \.offset) { index, item in
                    HStack(alignment: .firstTextBaseline, spacing: T.Spacing.xs) {
                        // In Int: start ≤ 999_999_999 (K1), and UInt32.max + index cannot overflow.
                        Text(verbatim: ordered ? "\(Int(start) + index)." : "\u{2022}")
                            .font(.system(size: text.size))
                        BlockList(blocks: item, style: style, text: text)
                    }
                }
            }
        case .codeBlock(_, let code):
            Text(verbatim: code).font(.system(size: text.size * MarkdownText.codeScale, design: .monospaced))
        }
    }
}
