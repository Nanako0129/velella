import AppKit
import SwiftUI
import Testing
@testable import VelellaUI

// T1 (docs/plans/markdown.md): Discord's message typography. Grouping and divider placement are
// pure; the geometry (text at 72, avatar at 16, 22 pt line pitch) is measured on offscreen renders.

private typealias M = DesignTokens.Message

private let taipei: Calendar = {
    var calendar = Calendar(identifier: .gregorian)
    calendar.timeZone = TimeZone(identifier: "Asia/Taipei")!
    return calendar
}()

/// 2026-10-14 12:00 in Taipei, then `seconds` later.
private func at(_ seconds: TimeInterval) -> Date {
    taipei.date(from: DateComponents(year: 2026, month: 10, day: 14, hour: 12))!.addingTimeInterval(seconds)
}

private func item(_ id: String, by author: String, _ time: Date, reply: Bool = false, text: String = "hi") -> MessageItem {
    MessageItem(
        id: id, authorId: author, authorName: author, avatar: .initials("AD"), timestamp: time, text: text,
        reply: reply ? ReplyPreview(authorName: "x", segments: [.text("y")]) : nil
    )
}

private func starts(_ messages: [MessageItem]) -> [Bool] {
    MessageGrouping.entries(messages, calendar: taipei).compactMap { entry in
        if case .row(_, let startsGroup, _) = entry { startsGroup } else { nil }
    }
}

// MARK: Grouping

@Test func sameAuthorWithinSevenMinutesJoinsTheGroup() {
    let rows = [item("1", by: "a", at(0)), item("2", by: "a", at(60)), item("3", by: "a", at(60 + 7 * 60))]
    // The third is exactly seven minutes after the second: still the same group.
    #expect(starts(rows) == [true, false, false])
}

@Test func sameAuthorOverSevenMinutesStartsANewGroup() {
    let rows = [item("1", by: "a", at(0)), item("2", by: "a", at(7 * 60 + 1))]
    #expect(starts(rows) == [true, true])
    #expect(M.groupWindow == 7 * 60)
}

@Test func anotherAuthorStartsANewGroupAndComingBackDoesToo() {
    let rows = [item("1", by: "a", at(0)), item("2", by: "b", at(10)), item("3", by: "a", at(20)), item("4", by: "a", at(30))]
    #expect(starts(rows) == [true, true, true, false])
}

@Test func aReplyAlwaysStartsAGroup() {
    #expect(starts([item("1", by: "a", at(0)), item("2", by: "a", at(5), reply: true)]) == [true, true])
}

@Test func aNewLocalDayStartsAGroupEvenWithinTheWindow() {
    // 23:59 and 00:01 Taipei, two minutes apart, one author.
    let late = taipei.date(from: DateComponents(year: 2026, month: 10, day: 14, hour: 23, minute: 59))!
    #expect(starts([item("1", by: "a", late), item("2", by: "a", late.addingTimeInterval(120))]) == [true, true])
}

// MARK: Dividers

@Test func aDividerSitsBetweenRowsOnDifferentLocalDaysOnly() {
    let late = taipei.date(from: DateComponents(year: 2026, month: 10, day: 14, hour: 23, minute: 59))!
    let rows = [
        item("1", by: "a", late.addingTimeInterval(-60)), item("2", by: "b", late),
        item("3", by: "b", late.addingTimeInterval(120)), item("4", by: "b", late.addingTimeInterval(180)),
    ]
    let entries = MessageGrouping.entries(rows, calendar: taipei)
    let shape = entries.map { entry -> String in
        switch entry {
        case .divider: "divider"
        case .row(let m, _, let afterDivider): m.id + (afterDivider ? "*" : "")
        }
    }
    // None before the first row; one before "3", the first row of the new day, which is flagged.
    #expect(shape == ["1", "2", "divider", "3*", "4"])
    #expect(entries[2] == .divider(id: "divider-3", rows[2].timestamp))
    #expect(Set(entries.map(\.id)).count == entries.count)
}

@Test func noDividerWithoutRowsOrWithOneRow() {
    #expect(MessageGrouping.entries([], calendar: taipei).isEmpty)
    #expect(MessageGrouping.entries([item("1", by: "a", at(0))], calendar: taipei).count == 1)
}

// MARK: Tokens

@Test func messageTokensHoldDiscordsValues() {
    // Module c19a55 (message), 75297 (markup), 5126c (group and divider spacing).
    #expect(M.rowPaddingVertical == 2 && M.avatarLeading == 16 && M.avatarTop == 2)
    #expect(M.contentInset == 72 && DesignTokens.Size.messageAvatar == 40)
    #expect(M.headerLineHeight == 22 && M.headerGap == 4 && M.authorSize == 16 && M.timestampSize == 12)
    #expect(M.bodySize == 16 && M.bodyLineHeight == 22 && MarkdownText.bodySize == 16)
    #expect(M.groupStart == 17 && M.gutterWidth == 56 && M.gutterTimeSize == 11)
    #expect(M.replyLineHeight == 18 && M.replyGap == 4)
    #expect(M.dividerAbove == 24 && M.dividerBelow == 8 && M.dividerLineWidth == 1)
    #expect(M.dividerLabelSize == 12 && M.dividerLabelLineHeight == 13)
    #expect(M.dividerLabelPaddingV == 2 && M.dividerLabelPaddingH == 4)
    #expect(MarkdownText.lineHeight(size: 16) == M.bodyLineHeight)
    #expect(MarkdownText.codeScale == 0.85)
}

@Test func lineBoxesAreOnePointThreeSevenFiveEm() {
    // Body `--chat-markup-line-height: 1.375rem` = 22; headings `line-height: 1.375em`; subtext 1.20313rem = 19.25.
    #expect([16.0, 24, 20, 14].map { MarkdownText.lineHeight(size: $0) } == [22, 33, 27.5, 19.25])
}

// MARK: Renders

private struct Ink {
    let width: Int, height: Int
    private let alpha: [UInt8]
    static let scale = 2

    @MainActor init?<V: View>(_ view: V, width: CGFloat) {
        let renderer = ImageRenderer(content: view.frame(width: width, alignment: .leading).environment(\.colorScheme, .light))
        renderer.scale = CGFloat(Ink.scale)
        guard let image = renderer.cgImage, image.bitsPerPixel == 32, let data = image.dataProvider?.data as Data? else { return nil }
        self.width = image.width
        self.height = image.height
        alpha = (0..<image.height).flatMap { y in (0..<image.width).map { x in data[y * image.bytesPerRow + x * 4 + 3] } }
    }

    /// Whether the pixel holds ink, as it is not a nearly transparent edge.
    func ink(_ x: Int, _ y: Int, above threshold: UInt8 = 100) -> Bool { alpha[y * width + x] > threshold }

    /// Points from the left of the first inked column in `columns` (points) over `rows` (points).
    func firstColumn(in columns: Range<Int>, rows: Range<Int>) -> Double? {
        let s = Ink.scale
        let found = (columns.lowerBound * s..<min(columns.upperBound * s, width)).first { x in
            (rows.lowerBound * s..<min(rows.upperBound * s, height)).contains { ink(x, $0) }
        }
        return found.map { Double($0) / Double(s) }
    }

    func firstRow(in columns: Range<Int>) -> Double? {
        let s = Ink.scale
        let found = (0..<height).first { y in (columns.lowerBound * s..<min(columns.upperBound * s, width)).contains { ink($0, y) } }
        return found.map { Double($0) / Double(s) }
    }

    /// Start rows (points) of each band of consecutive inked rows in `columns`.
    func bandStarts(in columns: Range<Int>) -> [Double] {
        let s = Ink.scale
        var starts: [Double] = [], inBand = false
        for y in 0..<height {
            let row = (columns.lowerBound * s..<min(columns.upperBound * s, width)).contains { ink($0, y) }
            if row && !inBand { starts.append(Double(y) / Double(s)) }
            inBand = row
        }
        return starts
    }
}

/// Three lines of "HHHH" (flat-topped capitals).
private func threeLines(startsGroup: Bool, edited: Bool = false) -> MessageRow {
    let line = [MarkdownInline.text("HHHH")]
    let body = MessageBody(blocks: [.paragraph(line + [.lineBreak] + line + [.lineBreak] + line)])
    var message = item("1", by: "Ada", at(0))
    message.body = body
    message.edited = edited
    return MessageRow(message: message, startsGroup: startsGroup, afterDivider: true)
}

@MainActor @Test func aGroupStartPutsTheAvatarAtSixteenAndTheTextAtSeventyTwo() throws {
    let render = try #require(Ink(threeLines(startsGroup: true), width: 400))
    // The avatar is the only ink left of the text column: its left edge is at x 16, its top 2 pt
    // below the row's content top (2 pt padding + 2 pt).
    let avatarX = try #require(render.firstColumn(in: 0..<60, rows: 0..<60))
    #expect(abs(avatarX - 16) <= 0.5, "avatar starts at \(avatarX)")
    let avatarY = try #require(render.firstRow(in: 30..<42))
    #expect(abs(avatarY - 4) <= 0.5, "avatar starts at y \(avatarY)")
    // Nothing between the avatar's right edge (56) and the text column; the text's first glyph
    // (a capital's side bearing is under 2 pt) begins at 72.
    #expect(render.firstColumn(in: 57..<72, rows: 0..<render.height / 2) == nil)
    let textX = try #require(render.firstColumn(in: 60..<200, rows: 30..<render.height / 2))
    #expect((72...74).contains(textX), "body text starts at \(textX)")
}

@MainActor @Test func bodyLinesAreTwentyTwoPointsApartAndTheRowIsWholeLineBoxes() throws {
    let render = try #require(Ink(threeLines(startsGroup: true), width: 400))
    // Header band, then three body bands.
    let bands = render.bandStarts(in: 72..<300)
    #expect(bands.count == 4, "bands: \(bands)")
    guard bands.count == 4 else { return }
    let pitch = (bands[3] - bands[1]) / 2
    #expect(abs(pitch - 22) <= 0.25, "line pitch \(pitch), bands \(bands)")
    // 2 + header 22 + 3 x 22 + 2.
    #expect(abs(Double(render.height) / Double(Ink.scale) - 92) <= 0.5, "row height \(Double(render.height) / 2)")
}

@MainActor @Test func aFollowUpRowHasNeitherAvatarNorHeaderButKeepsTheTextColumn() throws {
    let render = try #require(Ink(threeLines(startsGroup: false), width: 400))
    #expect(render.firstColumn(in: 0..<72, rows: 0..<render.height / 2) == nil, "ink left of the text column")
    let textX = try #require(render.firstColumn(in: 60..<200, rows: 0..<render.height / 2))
    #expect((72...74).contains(textX))
    // Three body lines only: 2 + 3 x 22 + 2.
    #expect(abs(Double(render.height) / Double(Ink.scale) - 70) <= 0.5)
    #expect(render.bandStarts(in: 72..<300).count == 3)
}

@MainActor @Test func blockTextHasWholeLineBoxesAtEverySize() throws {
    func height(_ block: MarkdownBlock) -> Double? {
        let body = MessageBody(blocks: [block])
        return Ink(BlockList(blocks: body.blocks, text: MarkdownText(body: body)), width: 300).map { Double($0.height) / Double(Ink.scale) }
    }
    let lines: [MarkdownInline] = [.text("H"), .lineBreak, .text("H"), .lineBreak, .text("H")]
    // Three lines are three boxes: 22 at 16, 33 / 27.5 / 22 for h1 / h2 / h3, 19.25 for subtext; a
    // first heading adds its own top margin (8 / 8 / 4), whole points from the 2x render.
    func near(_ block: MarkdownBlock, _ expected: Double) throws {
        let h = try #require(height(block))
        #expect(abs(h - expected) <= 0.5, "\(block): \(h) vs \(expected)")
    }
    try near(.paragraph(lines), 66)
    try near(.heading(level: 1, lines), 8 + 99)
    try near(.heading(level: 2, lines), 8 + 82.5)
    try near(.heading(level: 3, lines), 4 + 66)
    try near(.subtext(lines), 57.75)
}

@MainActor @Test func aDateDividerIsTwentyFourAboveAndEightBelowItsLabel() throws {
    let render = try #require(Ink(DateDivider(date: at(0)), width: 400))
    // 24 + (13 line box + 2 x 2 padding) + 8.
    #expect(abs(Double(render.height) / Double(Ink.scale) - 49) <= 0.5)
}
