import Foundation
import Testing
@testable import VelellaUI

private func live(
    content: String = "hi", authorName: String = "Ada", authorColor: UInt32? = nil,
    avatarURL: String = "https://cdn.discordapp.com/embed/avatars/0.png",
    images: [(url: String, width: UInt32, height: UInt32)] = [], files: [String] = [],
    replyAuthor: String? = nil, replyContent: String? = nil, edited: Bool = false,
    pending: Bool = false, failure: String? = nil
) -> MessageItem {
    .live(
        id: 42, authorName: authorName, authorColor: authorColor, avatarURL: avatarURL, timestampMs: 1_760_000_000_123,
        content: content, edited: edited, images: images, files: files, replyAuthor: replyAuthor,
        replyContent: replyContent, pending: pending, failure: failure
    )
}

@Test func markdownLinkContentStaysLiteralText() {
    let item = live(content: "[a](file:///etc)", authorName: "**bold**", replyAuthor: "[r](https://x.test)", replyContent: "[a](file:///etc)")
    #expect(item.text == "[a](file:///etc)")
    #expect(item.authorName == "**bold**")
    #expect(item.reply == ReplyPreview(authorName: "[r](https://x.test)", segments: [.text("[a](file:///etc)")]))
}

@Test func liveRowMapsEveryField() {
    let item = live(
        authorColor: 0xFF0000,
        images: [("https://cdn.discordapp.com/a.png", 640, 480)], files: ["notes.txt"], edited: true
    )
    #expect(item.id == "42")
    #expect(item.authorColor == RGB(1, 0, 0))
    #expect(item.avatar == .url(URL(string: "https://cdn.discordapp.com/embed/avatars/0.png")!))
    #expect(item.timestamp == Date(timeIntervalSince1970: 1_760_000_000.123))
    #expect(item.images == [ImageAttachment(url: URL(string: "https://cdn.discordapp.com/a.png")!, width: 640, height: 480)])
    #expect(item.files == ["notes.txt"])
    #expect(item.edited)
    #expect(item.reply == nil)
    #expect(item.delivery == .sent)
    #expect(live(authorColor: nil).authorColor == nil)
    #expect(live(avatarURL: "").avatar == .initials("A"))
    #expect(live(pending: true).delivery == .pending)
    #expect(live(pending: false, failure: "Not sent").delivery == .failed("Not sent"))
}

@Test func composerSendsANonBlankDraftOnceAndClearsIt() {
    var sent: [String] = []
    var draft = "hello there"
    ComposerSubmit.submit(&draft, hasMarkedText: false) { sent.append($0) }
    #expect(sent == ["hello there"])
    #expect(draft.isEmpty)
    // A second Return on the now empty draft sends nothing more.
    ComposerSubmit.submit(&draft, hasMarkedText: false) { sent.append($0) }
    #expect(sent == ["hello there"])
}

@Test func composerNeverSendsAWhitespaceOnlyDraft() {
    var calls = 0
    for blank in ["", " ", " \t ", "\u{3000}"] {
        var draft = blank
        ComposerSubmit.submit(&draft, hasMarkedText: false) { _ in calls += 1 }
    }
    #expect(calls == 0)
}

@Test func composerIgnoresAReturnThatCommitsMarkedText() {
    var calls = 0
    var draft = "你好"
    ComposerSubmit.submit(&draft, hasMarkedText: true) { _ in calls += 1 }
    #expect(calls == 0)
    #expect(draft == "你好")
}
