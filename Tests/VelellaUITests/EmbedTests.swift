import AppKit
import SwiftUI
import Testing
@testable import VelellaUI

// Wave 3 E (embeds and media): the card's links, spoiler cover, plain strings and clamps.

private final class Recorder {
    var opened: [URL] = []
    var confirmations: [LinkConfirmation] = []
    func confirm(_ c: LinkConfirmation) { confirmations.append(c) }
    func open(_ url: URL) { opened.append(url) }
}

private let handled = String(describing: OpenURLAction.Result.handled)

private let target = EmbedItem.Link(url: "https://example.com/post", host: "example.com")

private func embed(spoiler: Bool = false) -> EmbedItem {
    EmbedItem(
        title: "Secret title", url: target,
        description: MessageBody(blocks: [.paragraph([.text("Secret description")])]),
        authorName: "Author", authorURL: EmbedItem.Link(url: "https://a.example/", host: "a.example"),
        authorIcon: URL(string: "https://images-ext-1.discordapp.net/external/a.png"),
        footerText: "Footer", footerIcon: URL(string: "https://images-ext-2.discordapp.net/external/f.png"),
        providerName: "Provider", providerURL: EmbedItem.Link(url: "https://p.example/", host: "p.example"),
        thumbnail: EmbedItem.Image(url: URL(string: "https://media.discordapp.net/t.png")!, width: 160, height: 90),
        image: EmbedItem.Image(url: URL(string: "https://media.discordapp.net/i.png")!, width: 800, height: 600, alt: "a cat"),
        spoiler: spoiler
    )
}

// MARK: Links

@Test func aTitleTapOpensOnlyAfterAccept() throws {
    let recorder = Recorder()
    let router = EmbedView.router(embed(), confirm: recorder.confirm, open: recorder.open)
    let n = try #require(EmbedView.linkIndex(embed(), .title))
    #expect(String(describing: router.handle(MarkdownText.linkURL(n))) == handled)
    #expect(recorder.opened.isEmpty)
    #expect(recorder.confirmations.count == 1)
    recorder.confirmations[0].accept()
    #expect(recorder.opened == [URL(string: target.url)!])
}

@Test func everyEmbedLinkConfirmsFirst() throws {
    let e = embed()
    for slot in EmbedView.LinkSlot.allCases {
        let recorder = Recorder()
        let n = try #require(EmbedView.linkIndex(e, slot))
        _ = EmbedView.router(e, confirm: recorder.confirm, open: recorder.open).handle(MarkdownText.linkURL(n))
        #expect(recorder.opened.isEmpty, "\(slot)")
        #expect(recorder.confirmations.count == 1, "\(slot)")
        recorder.confirmations.first?.accept()
        let expected = [e.url, e.authorURL, e.providerURL][slot.rawValue]!.url
        #expect(recorder.opened == [URL(string: expected)!], "\(slot)")
    }
    // Only present targets are numbered, in slot order.
    let authorOnly = EmbedItem(title: "t", authorName: "a", authorURL: target)
    #expect(EmbedView.linkIndex(authorOnly, .title) == nil)
    #expect(EmbedView.linkIndex(authorOnly, .author) == 0)
    #expect(EmbedView.linkBody(authorOnly).links.map(\.masked) == [true])
}

@Test func aStrayLinkIsHandledAndDoesNothing() {
    let recorder = Recorder()
    let router = EmbedView.router(embed(), confirm: recorder.confirm, open: recorder.open)
    for url in ["velella-link:99", "velella-spoiler:0", "https://example.com/post", "velella-link:-1"] {
        #expect(String(describing: router.handle(URL(string: url)!)) == handled, "\(url)")
    }
    #expect(recorder.opened.isEmpty)
    #expect(recorder.confirmations.isEmpty)
}

@Test func aMaskedLinkInAFieldValueConfirms() {
    // A field value renders through its own `MarkdownView`, whose router is this one.
    let value = MessageBody(blocks: [.paragraph([.link(id: 0, url: "https://example.com/", host: "example.com", label: [.text("here")], masked: true)])])
    let field = EmbedItem.Field(name: "n", value: value)
    let recorder = Recorder()
    _ = LinkRouter(body: field.value, reveal: { _ in }, confirm: recorder.confirm, open: recorder.open).handle(MarkdownText.linkURL(0))
    #expect(recorder.opened.isEmpty)
    #expect(recorder.confirmations.count == 1)
}

// MARK: Spoiler cover

@Test func aCoveredCardSpeaksOnlySpoiler() {
    let covered = embed(spoiler: true)
    let label = EmbedView.spokenLabel(covered, covered: true)
    #expect(label == "Spoiler")
    #expect(label?.contains("Secret") == false)
    #expect(EmbedView.spokenLabel(covered, covered: false) == nil)
}

@Test func aSpoilerAttachmentIsNotFetchedUntilRevealed() {
    let url = URL(string: "https://media.discordapp.net/a.png")!
    let image = ImageAttachment(url: url, width: 10, height: 10, spoiler: true)
    #expect(ImageAttachmentView.fetched(image, revealed: false) == nil)
    #expect(ImageAttachmentView.fetched(image, revealed: true) == url)
    #expect(ImageAttachmentView.fetched(ImageAttachment(url: url, width: 10, height: 10), revealed: false) == url)
}

@Test func aVideoShowsItsPreviewLargeAndNothingElse() {
    var video = embed()
    video.isVideo = true
    #expect(EmbedView.slots(video).thumbnail == nil)
    #expect(EmbedView.slots(video).large == video.image)
    video.image = nil
    #expect(EmbedView.slots(video).large == embed().thumbnail)
}

// MARK: Plain strings

@Test func theFooterIsIsolatedBeforeTheTime() {
    let line = EmbedView.footerLine(text: "\u{202E}evil", time: "3:00 PM")
    #expect(line == "\u{2068}\u{202E}evil\u{2069} \u{2022} 3:00 PM")
    #expect(EmbedView.footerLine(text: "\u{202E}alone", time: nil) == "\u{202E}alone")
    #expect(EmbedView.footerLine(text: nil, time: "3:00 PM") == "3:00 PM")
    #expect(EmbedView.footerLine(text: nil, time: nil) == nil)
}

@Test func footerTimesAreClampedBeforeFormatting() {
    let locale = Locale(identifier: "en-US")
    for ms in [Int64.min, Int64.max, -1, 0] {
        #expect(EmbedView.time(ms, locale: locale, now: Date(timeIntervalSince1970: 0)) != nil)
    }
    #expect(EmbedView.time(nil, locale: locale, now: .now) == nil)
}

@Test func imageLabelsAreAltTextOrImageNeverAURL() {
    #expect(MediaImages.label("a cat") == "a cat")
    #expect(MediaImages.label("**not markup**") == "**not markup**")
    #expect(MediaImages.label(nil) == "Image")
    #expect(MediaImages.label("") == "Image")
}

@Test func inlineFieldsShareRowsOfUpToThree() {
    let f = { (inline: Bool) in EmbedItem.Field(name: "n", value: MessageBody(), inline: inline) }
    let fields = [f(true), f(true), f(true), f(true), f(false), f(true), f(true)]
    #expect(EmbedView.fieldRows(fields) == [[0, 1, 2], [3], [4], [5, 6]])
}

// MARK: Clamps

@Test func extremeBoxesStayInsideTheMaxima() {
    for (w, h) in [(1.0, 65535.0), (65535.0, 1.0), (400.0, 300.0), (16384.0, 16384.0), (10.0, 10.0)] {
        for max in [MediaBox.image, MediaBox.thumbnail] {
            let box = MediaBox.fit(width: w, height: h, in: max)
            #expect(box.width <= max.width && box.height <= max.height, "\(w)x\(h) in \(max)")
            #expect(box.width > 0 && box.height > 0)
            #expect(abs(box.width / box.height - w / h) < 1e-9 * Swift.max(1, w / h))
        }
    }
    // Never scaled up; nonsense gives an empty box.
    #expect(MediaBox.fit(width: 10, height: 10, in: MediaBox.image) == CGSize(width: 10, height: 10))
    #expect(MediaBox.fit(width: 0, height: 10, in: MediaBox.image) == .zero)
    #expect(MediaBox.fit(width: .nan, height: 10, in: MediaBox.image) == .zero)
    // Image attachments take the same clamp.
    let attachment = ImageAttachmentView(image: ImageAttachment(url: URL(string: "https://media.discordapp.net/a.png")!, width: 65535, height: 1))
    #expect(attachment.box.width <= MediaBox.image.width)
}

// MARK: Mapping and rendering

@Test func liveRowsCarryEmbedsAndSpoilerImages() {
    let item = MessageItem.live(
        id: 1, authorId: 2, authorName: "a", authorColor: nil, avatarURL: "https://cdn.discordapp.com/embed/avatars/0.png",
        timestampMs: 0, content: "", body: MessageBody(), edited: false,
        images: [("https://media.discordapp.net/a.png", 4, 3), ("https://media.discordapp.net/b.png", 4, 3)],
        files: [], replyAuthor: nil, replyContent: nil, pending: false, failure: nil,
        embeds: [embed()], imageSpoilers: [true, false]
    )
    #expect(item.images.map(\.spoiler) == [true, false])
    #expect(item.embeds == [embed()])
}

/// The embed without image or icon URLs: offscreen hosts must never start a real fetch.
private func imageless(spoiler: Bool) -> EmbedItem {
    var e = embed(spoiler: spoiler)
    e.authorIcon = nil; e.footerIcon = nil; e.thumbnail = nil; e.image = nil
    return e
}

/// The accessibility labels and values under `element` (as MarkdownTests reads them).
@MainActor private func spokenTexts(_ element: Any) -> [String] {
    guard let object = element as? NSObject else { return [] }
    func send(_ name: String) -> Any? {
        let selector = NSSelectorFromString(name)
        return object.responds(to: selector) ? object.perform(selector)?.takeUnretainedValue() : nil
    }
    let own = [send("accessibilityLabel"), send("accessibilityValue")].compactMap { $0 as? String }.filter { !$0.isEmpty }
    return own + ((send("accessibilityChildren") as? [Any]) ?? []).flatMap(spokenTexts)
}

/// What VoiceOver gets from a card drawn offscreen (the window is never ordered in).
@MainActor private func spoken(_ e: EmbedItem) -> [String] {
    let host = NSHostingView(rootView: EmbedView(embed: e, opener: { _ in }).frame(width: 520))
    let window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 520, height: 400), styleMask: .borderless, backing: .buffered, defer: true)
    window.contentView = host
    NSApplication.shared.accessibilitySetValue(true, forAttribute: NSAccessibility.Attribute(rawValue: "AXEnhancedUserInterface"))
    host.layoutSubtreeIfNeeded()
    return spokenTexts(host)
}

@MainActor @Test func aCoveredCardsAccessibilityHoldsNoTitleOrDescription() {
    let covered = spoken(imageless(spoiler: true))
    #expect(covered.contains("Spoiler"), "\(covered)")
    #expect(!covered.contains { $0.contains("Secret") || $0.contains("Author") || $0.contains("Footer") }, "\(covered)")
    // The same card uncovered speaks its content.
    let shown = spoken(imageless(spoiler: false))
    #expect(shown.contains { $0.contains("Secret title") }, "\(shown)")
}

@MainActor @Test func cardsRenderCoveredAndUncovered() {
    for e in [imageless(spoiler: false), imageless(spoiler: true)] {
        let renderer = ImageRenderer(content: EmbedView(embed: e, opener: { _ in }).frame(width: 520))
        #expect(renderer.nsImage != nil)
    }
}

/// A covered card builds no image view, so nothing is fetched until it is revealed; the same card
/// uncovered fetches its four images (all through the test seam to 127.0.0.1).
@MainActor @Test func aCoveredCardFetchesNothing() async throws {
    let server = try LocalServer(Array(repeating: response(body: png(width: 10, height: 10)), count: 8))
    let loader = MediaImages(rewrite: server.rewrite)
    func show(_ e: EmbedItem) -> NSWindow {
        let host = NSHostingView(rootView: EmbedView(embed: e, opener: { _ in }, loader: loader).frame(width: 520))
        let window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 520, height: 600), styleMask: .borderless, backing: .buffered, defer: true)
        window.contentView = host
        host.layoutSubtreeIfNeeded()
        return window
    }
    let covered = show(embed(spoiler: true))
    try await Task.sleep(for: .milliseconds(500))
    #expect(server.requests.isEmpty, "\(server.requests)")
    let shown = show(embed())
    try await Task.sleep(for: .milliseconds(1500))
    #expect(server.requests.count == 4, "\(server.requests.count)")
    _ = (covered, shown)
}
