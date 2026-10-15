import AppKit
import SwiftUI

// Message embeds (wave 3 E, Display and Embed links rows). Every string here is payload text:
// plain strings are `Text(verbatim:)` (isolated where they share a line), descriptions and field
// values are core's AST through `MarkdownView`, images come only through `MediaImages`, and the
// card's links reach the system only through a `LinkRouter` whose targets are all masked, so each
// one asks first.

/// One embed, as plain values (VelellaUI sees no FFI types; the app maps core's row onto this).
public struct EmbedItem: Hashable, Sendable {
    /// A checked http(s) target from core: canonical URL and ASCII host.
    public struct Link: Hashable, Sendable {
        public var url: String
        public var host: String
        public init(url: String, host: String) { self.url = url; self.host = host }
    }

    public struct Image: Hashable, Sendable {
        public var url: URL
        public var width: Double
        public var height: Double
        /// Alt text, verbatim.
        public var alt: String?
        public init(url: URL, width: Double, height: Double, alt: String? = nil) {
            self.url = url; self.width = width; self.height = height; self.alt = alt
        }
    }

    public struct Field: Hashable, Sendable {
        public var name: String
        public var value: MessageBody
        public var inline: Bool
        public init(name: String, value: MessageBody, inline: Bool = false) {
            self.name = name; self.value = value; self.inline = inline
        }
    }

    public var title: String?
    public var url: Link?
    public var description: MessageBody
    /// 0xRRGGBB; nil draws the default bar.
    public var color: UInt32?
    public var timestampMs: Int64?
    public var authorName: String?
    public var authorURL: Link?
    public var authorIcon: URL?
    public var footerText: String?
    public var footerIcon: URL?
    public var fields: [Field]
    public var providerName: String?
    public var providerURL: Link?
    public var thumbnail: Image?
    public var image: Image?
    /// A video or gifv: its preview shows, nothing plays.
    public var isVideo: Bool
    /// The body hides this embed's link in a spoiler: the card is covered as one unit.
    public var spoiler: Bool

    public init(
        title: String? = nil, url: Link? = nil, description: MessageBody = MessageBody(), color: UInt32? = nil,
        timestampMs: Int64? = nil, authorName: String? = nil, authorURL: Link? = nil, authorIcon: URL? = nil,
        footerText: String? = nil, footerIcon: URL? = nil, fields: [Field] = [], providerName: String? = nil,
        providerURL: Link? = nil, thumbnail: Image? = nil, image: Image? = nil, isVideo: Bool = false, spoiler: Bool = false
    ) {
        self.title = title; self.url = url; self.description = description; self.color = color
        self.timestampMs = timestampMs; self.authorName = authorName; self.authorURL = authorURL; self.authorIcon = authorIcon
        self.footerText = footerText; self.footerIcon = footerIcon; self.fields = fields; self.providerName = providerName
        self.providerURL = providerURL; self.thumbnail = thumbnail; self.image = image; self.isVideo = isVideo; self.spoiler = spoiler
    }
}

/// The card's look, from Discord's embed CSS (`.embedFull`, `.embedAuthor`, `.embedTitle`, ...);
/// 1 CSS px = 1 pt. Not checked against a capture yet: provisional.
enum EmbedStyle {
    /// `.embedFull { border-left: 4px solid; border-radius: 4px; max-width: 520px }`.
    static let barWidth: CGFloat = 4
    static let radius: CGFloat = 4
    static let maxWidth: CGFloat = 520
    /// `padding: .5rem 1rem 1rem .75rem`.
    static let padding = EdgeInsets(top: 8, leading: 12, bottom: 16, trailing: 16)
    /// Between rows (`margin-top: 8px` on author, title, description, fields, footer).
    static let rowGap: CGFloat = 8
    /// Thumbnail to text (`.embedThumbnail { margin-left: 16px }`); image above (`margin-top: 16px`).
    static let thumbnailGap: CGFloat = 16
    static let imageTop: CGFloat = 16
    /// Between field columns and rows (`.embedFields { gap: 8px }`).
    static let fieldGap: CGFloat = 8
    /// Inline fields per row.
    static let fieldColumns = 3
    /// Icon to its text (`margin-right: 8px`).
    static let iconGap: CGFloat = 8
    /// Author name and field names: .875rem semibold; title 1rem semibold; footer and provider .75rem.
    static let authorFont = Font.system(size: 14, weight: .semibold)
    static let titleFont = Font.system(size: 16, weight: .semibold)
    static let fieldNameFont = Font.system(size: 14, weight: .semibold)
    static let smallFont = Font.system(size: 12, weight: .medium)
    /// Card fill and default bar: no capture; quiet neutrals that sit on the message pane.
    static let fill = Color.primary.opacity(0.06)
    static let defaultBar = Color(nsColor: .separatorColor)
    /// Discord's link colour for titles.
    static let link = Color(nsColor: .linkColor)
    /// A covered card's height (its content is not laid out under the cover). Provisional.
    static let coveredHeight: CGFloat = 80
}

/// The card. Links go through `router` (all masked, so each confirms); the root's `openURL`
/// answers `.handled` for anything else, so nothing reaches the system's URL handling.
struct EmbedView: View {
    let embed: EmbedItem
    /// Opens a vetted link; nil uses the surrounding (system) `openURL`. Tests inject a recorder.
    var opener: ((URL) -> Void)?
    /// Where the card's images load from; tests point it at a local server.
    var loader: MediaImages = .shared
    @Environment(\.openURL) private var outerOpenURL
    @Environment(\.discordLocale) private var locale
    @State private var revealed = false
    @State private var confirmation: LinkConfirmation?

    private var covered: Bool { embed.spoiler && !revealed }

    var body: some View {
        Group {
            if covered {
                // Nothing of the card is built under the cover: no text, no links, no image or
                // emoji fetch, until the click.
                RoundedRectangle(cornerRadius: EmbedStyle.radius)
                    .fill(MarkdownPalette.spoilerCover)
                    .overlay { Text("SPOILER").font(.caption.weight(.bold)).foregroundStyle(.white) }
                    .frame(maxWidth: EmbedStyle.maxWidth)
                    .frame(height: EmbedStyle.coveredHeight)
                    .contentShape(Rectangle())
                    .onTapGesture { revealed = true }
                    .accessibilityElement(children: .ignore)
                    .accessibilityLabel(Text(verbatim: Self.spokenLabel(embed, covered: true) ?? ""))
                    .accessibilityAddTraits(.isButton)
            } else {
                card
            }
        }
        .environment(\.openURL, OpenURLAction { covered ? .handled : router.handle($0) })
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
    }

    private var router: LinkRouter {
        Self.router(embed, confirm: { confirmation = $0 }, open: opener ?? { outerOpenURL($0) })
    }

    // MARK: Links

    enum LinkSlot: Int, CaseIterable { case title, author, provider }

    /// The card's link targets, all masked (so every one confirms), numbered in slot order.
    nonisolated static func linkBody(_ embed: EmbedItem) -> MessageBody {
        let targets = [embed.url, embed.authorURL, embed.providerURL].compactMap { $0 }
        return MessageBody(blocks: [.paragraph(targets.map { .link(id: 0, url: $0.url, host: $0.host, label: [.text($0.url)], masked: true) })])
    }

    /// `velella-link:<n>` of a slot, or nil without a target.
    nonisolated static func linkIndex(_ embed: EmbedItem, _ slot: LinkSlot) -> Int? {
        let targets = [embed.url, embed.authorURL, embed.providerURL]
        guard targets[slot.rawValue] != nil else { return nil }
        return targets[..<slot.rawValue].compactMap { $0 }.count
    }

    nonisolated static func router(_ embed: EmbedItem, confirm: @escaping (LinkConfirmation) -> Void, open: @escaping (URL) -> Void) -> LinkRouter {
        LinkRouter(body: linkBody(embed), reveal: { _ in }, confirm: confirm, open: open)
    }

    /// A slot's text: a plain-looking button sending its in-app link through the router when it has
    /// a target, else plain text.
    @ViewBuilder
    private func linked(_ text: String, _ slot: LinkSlot, font: Font, color: Color? = nil) -> some View {
        let label = Text(verbatim: text).font(font).foregroundStyle(color ?? .primary)
        if let n = Self.linkIndex(embed, slot) {
            Button { _ = router.handle(MarkdownText.linkURL(n)) } label: { label.foregroundStyle(EmbedStyle.link) }
                .buttonStyle(.plain)
        } else {
            label
        }
    }

    // MARK: Text helpers

    /// What VoiceOver reads for the whole card: "Spoiler" while covered (nothing of the content),
    /// otherwise nil (the parts speak for themselves).
    nonisolated static func spokenLabel(_ embed: EmbedItem, covered: Bool) -> String? {
        covered ? "Spoiler" : nil
    }

    /// The footer line: the footer text isolated (it shares the line with the time), a bullet,
    /// the time; either part alone when the other is missing.
    nonisolated static func footerLine(text: String?, time: String?) -> String? {
        switch (text, time) {
        case let (text?, time?): MarkdownText.isolated(text) + " \u{2022} " + time
        case let (text?, nil): text
        case let (nil, time?): time
        case (nil, nil): nil
        }
    }

    /// The footer time, clamped to the JS date range before any conversion (K4 rule).
    nonisolated static func time(_ ms: Int64?, locale: Locale, now: Date) -> String? {
        guard let ms else { return nil }
        let limit = DiscordTime.unixLimit * 1000
        let clamped = min(max(ms, -limit), limit)
        return DiscordTime.header(date: Date(timeIntervalSince1970: Double(clamped) / 1000), now: now, locale: locale)
    }

    /// Fields laid out as Discord does: up to `fieldColumns` consecutive inline fields share a
    /// row; any other field takes a row of its own. Indices into `fields`.
    nonisolated static func fieldRows(_ fields: [EmbedItem.Field]) -> [[Int]] {
        var rows: [[Int]] = []
        for (i, field) in fields.enumerated() {
            if field.inline, let last = rows.last, last.count < EmbedStyle.fieldColumns, last.allSatisfy({ fields[$0].inline }) {
                rows[rows.count - 1].append(i)
            } else {
                rows.append([i])
            }
        }
        return rows
    }

    /// (top-right thumbnail, large image): a video shows its preview large and no thumbnail.
    nonisolated static func slots(_ embed: EmbedItem) -> (thumbnail: EmbedItem.Image?, large: EmbedItem.Image?) {
        embed.isVideo ? (nil, embed.image ?? embed.thumbnail) : (embed.thumbnail, embed.image)
    }

    // MARK: Card

    private var card: some View {
        let (thumbnail, large) = Self.slots(embed)
        return HStack(spacing: 0) {
            Rectangle()
                .fill(embed.color.map { Color(RGB(rgb: $0)) } ?? EmbedStyle.defaultBar)
                .frame(width: EmbedStyle.barWidth)
            VStack(alignment: .leading, spacing: 0) {
                HStack(alignment: .top, spacing: EmbedStyle.thumbnailGap) {
                    VStack(alignment: .leading, spacing: EmbedStyle.rowGap) {
                        if let name = embed.providerName {
                            linked(name, .provider, font: EmbedStyle.smallFont, color: .secondary)
                        }
                        if let name = embed.authorName {
                            HStack(spacing: EmbedStyle.iconGap) {
                                if let icon = embed.authorIcon {
                                    media(icon, box: MediaBox.authorIcon, alt: nil).clipShape(Circle())
                                }
                                linked(name, .author, font: EmbedStyle.authorFont)
                            }
                        }
                        if let title = embed.title {
                            linked(title, .title, font: EmbedStyle.titleFont)
                        }
                        if !embed.description.blocks.isEmpty {
                            MarkdownView(content: embed.description)
                        }
                        if !embed.fields.isEmpty {
                            fields
                        }
                    }
                    .frame(maxWidth: .infinity, alignment: .leading)
                    if let thumbnail {
                        let box = MediaBox.fit(width: thumbnail.width, height: thumbnail.height, in: MediaBox.thumbnail)
                        media(thumbnail.url, box: box, alt: thumbnail.alt)
                            .clipShape(RoundedRectangle(cornerRadius: EmbedStyle.radius))
                    }
                }
                if let large {
                    let box = MediaBox.fit(width: large.width, height: large.height, in: MediaBox.image)
                    media(large.url, box: box, alt: large.alt)
                        .clipShape(RoundedRectangle(cornerRadius: EmbedStyle.radius))
                        .overlay {
                            if embed.isVideo {
                                // A still preview: nothing plays (no AVKit, no web view).
                                Image(systemName: "play.circle.fill").font(.largeTitle).foregroundStyle(.white.opacity(0.85))
                            }
                        }
                        .padding(.top, EmbedStyle.imageTop)
                }
                if let line = Self.footerLine(text: embed.footerText, time: Self.time(embed.timestampMs, locale: locale, now: .now)) {
                    HStack(spacing: EmbedStyle.iconGap) {
                        if let icon = embed.footerIcon {
                            media(icon, box: MediaBox.footerIcon, alt: nil).clipShape(Circle())
                        }
                        Text(verbatim: line).font(EmbedStyle.smallFont).foregroundStyle(.secondary)
                    }
                    .padding(.top, EmbedStyle.rowGap)
                }
            }
            .padding(EmbedStyle.padding)
        }
        .fixedSize(horizontal: false, vertical: true)
        .background(EmbedStyle.fill)
        .clipShape(RoundedRectangle(cornerRadius: EmbedStyle.radius))
        .frame(maxWidth: EmbedStyle.maxWidth, alignment: .leading)
    }

    private var fields: some View {
        VStack(alignment: .leading, spacing: EmbedStyle.fieldGap) {
            ForEach(Array(Self.fieldRows(embed.fields).enumerated()), id: \.offset) { _, row in
                HStack(alignment: .top, spacing: EmbedStyle.fieldGap) {
                    ForEach(row, id: \.self) { i in
                        let field = embed.fields[i]
                        VStack(alignment: .leading, spacing: 2) {
                            Text(verbatim: field.name).font(EmbedStyle.fieldNameFont)
                            if !field.value.blocks.isEmpty {
                                MarkdownView(content: field.value)
                            }
                        }
                        .frame(maxWidth: .infinity, alignment: .leading)
                    }
                }
            }
        }
    }

    private func media(_ url: URL, box: CGSize, alt: String?) -> some View {
        MediaImageView(url: url, box: box, alt: alt, loader: loader)
    }
}
