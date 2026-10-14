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
        content: content, body: MessageBody(text: content), edited: edited, images: images, files: files, replyAuthor: replyAuthor,
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

@Test func skeletonRowsAreStablePerConversationAndStayInV2Ranges() {
    #expect(SkeletonRow.rows(seed: "1556391419621671022", count: 8) == SkeletonRow.rows(seed: "1556391419621671022", count: 8))
    #expect(SkeletonRow.rows(seed: "1", count: 8) != SkeletonRow.rows(seed: "2", count: 8))
    let rows = SkeletonRow.rows(seed: "1523004250152501341", count: 500)
    for row in rows {
        #expect((76...158).contains(row.nameWidth))
        #expect((34...52).contains(row.timeWidth))
        #expect((1...4).contains(row.lineWidths.count))
        #expect((96...360).contains(row.lineWidths.last!))
        #expect(row.lineWidths.dropLast().allSatisfy { (220...540).contains($0) })
        if let media = row.media {
            #expect((180...320).contains(media.width) && (92...184).contains(media.height))
        }
    }
    // One row in five carries media.
    let withMedia = rows.filter { $0.media != nil }.count
    #expect((60...140).contains(withMedia))
}

@Test func skeletonRowHeightsMatchTheV2Capture() {
    func row(lines: Int, media: CGSize? = nil) -> SkeletonRow {
        SkeletonRow(nameWidth: 100, timeWidth: 40, lineWidths: Array(repeating: 200, count: lines), media: media)
    }
    // Two lines measured 50 tall in the capture; one line is held at the avatar's 40.
    #expect(row(lines: 2).height == 50)
    #expect(row(lines: 1).height == 40)
    // v2: 17 × lines + 16, plus the media height + 8.
    #expect(row(lines: 1, media: CGSize(width: 200, height: 100)).height == 141)
}
