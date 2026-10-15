import Foundation

/// Deterministic fixture data with obviously fake names; no real people or servers.
/// Used by `--demo` and the previews. No randomness, no `Date.now`.
public enum Fixtures {
    private static let orange = RGB(0.90, 0.50, 0.20)
    private static let blue = RGB(0.25, 0.60, 0.90)
    private static let green = RGB(0.30, 0.70, 0.40)
    private static let purple = RGB(0.70, 0.45, 0.90)
    private static let pink = RGB(0.90, 0.40, 0.60)

    /// Fixed instant (2025-10-09 08:53:20 UTC) so rendered timestamps never change between runs.
    private static let baseTime = Date(timeIntervalSince1970: 1_760_000_000)

    public static let currentUser = CurrentUser(
        displayName: "Test User", username: "test.user", avatar: .initials("TU"), status: .online
    )

    /// Style used by previews to show the display-name slot rendering (no fixture uses it by default).
    public static let sampleNameStyle = DisplayNameStyle(
        font: .serif, colors: [RGB(0.95, 0.45, 0.35), RGB(0.95, 0.75, 0.25)], effect: .glow
    )

    public static let rail: [RailItem] = [
        .dm(RailDM(id: "dm", name: "Direct Messages", icon: .symbol("bubble.left.fill"), mentionCount: 2)),
        .guild(RailGuild(id: "g-lorem", name: "Lorem Ipsum Club", icon: .initials("LI"))),
        .guild(RailGuild(id: "g-example", name: "Example Workshop", icon: .initials("EW"), hasUnread: true)),
        .folder(RailFolder(id: "f-sample", name: "Sample Folder", color: purple, children: [
            RailGuild(id: "g-foo", name: "Foo Bar Baz", icon: .initials("FBB"), hasUnread: true),
            RailGuild(id: "g-dummy", name: "Dummy Data Hub", icon: .initials("DDH")),
            RailGuild(id: "g-test", name: "Test Guild", icon: .initials("TG"), mentionCount: 3),
        ])),
        .guild(RailGuild(id: "g-mock", name: "Mockup Lounge", icon: .initials("ML"))),
        .guild(RailGuild(id: "g-placeholder", name: "Placeholder Plaza", icon: .initials("PP"))),
    ]

    private static func person(_ id: String, _ name: String) -> VoiceOccupant {
        VoiceOccupant(id: id, name: name, avatar: .initials(String(name.prefix(2)).uppercased()))
    }

    public static let categories: [ChannelCategory] = [
        ChannelCategory(id: "cat-top", name: nil, channels: [
            ChannelItem(id: "c-welcome", name: "welcome", topic: "Fake server, fake channels, fake everything."),
        ]),
        ChannelCategory(id: "cat-info", name: "Info", channels: [
            ChannelItem(id: "c-announcements", name: "announcements", kind: .announcement, topic: "Lorem ipsum dolor sit amet.", isUnread: true),
            ChannelItem(id: "c-rules", name: "rules", topic: "Be excellent to each other. (Fixture.)"),
            ChannelItem(id: "c-releases", name: "releases", kind: .announcement, isMuted: true),
        ]),
        ChannelCategory(id: "cat-chat", name: "Chat", channels: [
            ChannelItem(id: "c-general", name: "general", topic: "General chatter about nothing in particular.", isUnread: true, threads: [
                ChannelItem(id: "t-sync", name: "Weekly sync - 2026/10/13"),
                ChannelItem(id: "t-ideas", name: "Lorem ipsum ideas"),
                ChannelItem(id: "t-fixture", name: "Fixture thread with a rather long name"),
            ]),
            ChannelItem(id: "c-random", name: "random", topic: "Placeholder topic for the random channel.", threads: [
                ChannelItem(id: "t-pics", name: "Placeholder pictures"),
            ]),
            ChannelItem(id: "c-help", name: "help-forum", kind: .forum, topic: "Ask a fake question."),
            ChannelItem(id: "c-mods", name: "mod-chat", isPrivate: true),
            ChannelItem(id: "c-mod-forum", name: "mod-forum", kind: .forum, isPrivate: true),
            ChannelItem(id: "c-noise", name: "background-noise", isMuted: true),
        ]),
        ChannelCategory(id: "cat-voice", name: "Voice", channels: [
            ChannelItem(id: "c-lounge", name: "Lounge", kind: .voice, occupants: [
                person("o1", "Ada Placeholder"), person("o2", "Bob Example"),
            ]),
            ChannelItem(id: "c-stage", name: "Town Hall", kind: .stage),
            ChannelItem(id: "c-staff", name: "Staff Room", kind: .voice, isPrivate: true),
        ]),
    ]

    /// Enough rows to scroll, with images among them, so the toolbar band and the composer glass
    /// have colourful content passing under them.
    public static let messages: [MessageItem] = {
        let authors: [(String, RGB?)] = [
            ("Ada Placeholder", orange), ("Bob Example", blue), ("Cleo Sample", green),
            ("Dex Dummy", nil), ("Test User", purple), ("Hana Ipsum", pink),
        ]
        let lines = [
            "Lorem ipsum dolor sit amet, consectetur adipiscing elit.",
            "Sed do eiusmod tempor incididunt ut labore et dolore magna aliqua.",
            "Ut enim ad minim veniam, quis nostrud exercitation ullamco laboris nisi ut aliquip ex ea commodo consequat.",
            "This is a fixture message; real rows arrive in a later slice.",
            "Duis aute irure dolor in reprehenderit in voluptate velit esse cillum dolore eu fugiat nulla pariatur.",
            "Excepteur sint occaecat cupidatat non proident.",
            "Short one.",
            "Curabitur pretium tincidunt lacus. Nulla gravida orci a odio. Nullam varius, turpis et commodo pharetra, est eros bibendum elit, nec luctus magna felis sollicitudin mauris. Integer in mauris eu nibh euismod gravida.",
            "Look at this:",
        ]
        let images: [Int: ImageAttachment] = [
            3: ImageAttachment(seed: 1, width: 480, height: 270),
            8: ImageAttachment(seed: 2, width: 300, height: 400),
            14: ImageAttachment(seed: 3, width: 400, height: 400),
            21: ImageAttachment(seed: 4, width: 520, height: 300),
            27: ImageAttachment(seed: 5, width: 360, height: 240),
            33: ImageAttachment(seed: 6, width: 480, height: 320),
        ]
        // Unicode only: a fixture makes no network request. The first pill is the user's own.
        let reactions: [Int: [ReactionItem]] = [
            2: [ReactionItem(name: "\u{1F44D}", count: 12, me: true), ReactionItem(name: "\u{1F389}", count: 3), ReactionItem(name: "\u{2764}\u{FE0F}", count: 1)],
            9: [ReactionItem(name: "\u{1F602}", count: 2)],
        ]
        return (0..<36).map { index in
            let author = authors[(index * 5 + index / 3) % authors.count]
            let markdown = index == 6
            return MessageItem(
                id: "m\(index)", authorName: author.0, authorColor: author.1,
                avatar: .initials(String(author.0.prefix(2)).uppercased()),
                timestamp: baseTime.addingTimeInterval(Double(index) * 240),
                text: markdown ? markdownSource : images[index] != nil ? lines[8] : lines[index % 8],
                body: markdown ? markdownBody : nil,
                images: images[index].map { [$0] } ?? [],
                reactions: reactions[index] ?? []
            )
        }
    }()

    /// One row of inline markdown: styles, a mention, a spoiler, a masked link (confirms before
    /// opening), a timestamp with its hover tip, and a code block under a spoiler.
    private static let markdownSource =
        "**Bold**, *italic*, __underline__, ~~strike~~, `code`, <@1>, ||a spoiler||, [a masked link](https://example.com) and <t:1618953630:f>\n||```\nhidden block\n```||"

    private static let markdownBody = MessageBody(
        blocks: [
            .paragraph([
                .bold([.text("Bold")]), .text(", "), .italic([.text("italic")]), .text(", "),
                .underline([.text("underline")]), .text(", "), .strike([.text("strike")]), .text(", "),
                .code("code"), .text(", "), .userMention(1), .text(", "), .spoiler(id: 0, [.text("a spoiler")]),
                .text(", "),
                .link(id: 0, url: "https://example.com/", host: "example.com", label: [.text("a masked link")], masked: true),
                .text(" and "), .timestamp(unix: 1_618_953_630, style: .longDateShortTime), .lineBreak,
                // K1 keeps this break (it is not beside the block itself); the split drops it.
                .spoiler(id: 0, [.block(.codeBlock(language: nil, code: "hidden block", header: nil, highlight: nil))]),
            ]),
        ],
        mentions: [ResolvedMention(kind: .user, id: 1, name: "Bob Example")]
    )

    private static func member(_ id: String, _ name: String, _ color: RGB?, _ status: PresenceStatus = .online, _ custom: String? = nil) -> MemberRow {
        MemberRow(id: id, name: name, avatar: .initials(String(name.prefix(2)).uppercased()), roleColor: color, status: status, customStatus: custom)
    }

    public static let memberSections: [MemberSection] = [
        MemberSection(id: "r-admin", title: "Admin", count: 2, members: [
            member("u1", "Ada Placeholder", orange, .online, "Writing lorem ipsum"),
            member("u2", "Bob Example", orange, .idle),
        ]),
        MemberSection(id: "r-mod", title: "Moderator", count: 3, members: [
            member("u3", "Cleo Sample", green, .doNotDisturb, "In a fake meeting"),
            member("u4", "Dex Dummy", green),
            member("u5", "Eve Mockup", green, .idle),
        ]),
        MemberSection(id: "r-bots", title: "Bots", count: 1, members: [
            member("u6", "Fixture Bot", blue),
        ]),
        MemberSection(id: "r-online", title: "Online", count: 187, members: [
            member("u7", "Gus Lorem", nil),
            member("u8", "Hana Ipsum", pink),
            member("u9", "Test User", nil),
        ]),
        MemberSection(id: "r-offline", title: "Offline", count: 412, members: [
            member("u10", "Ivy Filler", nil, .offline),
            member("u11", "Jin Stub", nil, .offline),
        ]),
    ]

    public static let dmTopRows: [DMTopRow] = [
        DMTopRow(id: "dm-friends", title: "Friends", symbol: "person.2.fill", badgeCount: 12),
        DMTopRow(id: "dm-requests", title: "Message Requests", symbol: "tray.fill", badgeCount: 6),
        DMTopRow(id: "dm-app", title: "Velella", symbol: "sparkles"),
    ]

    public static let dms: [DMEntry] = [
        DMEntry(id: "d1", name: "Ada Placeholder", avatar: .initials("AP"), status: .online),
        DMEntry(id: "d2", name: "Bob Example", avatar: .initials("BE"), status: .idle, serverTag: "DEV"),
        DMEntry(id: "d3", name: "Cleo Sample", avatar: .initials("CS"), status: .online),
        DMEntry(id: "d4", name: "Dex Dummy", avatar: .initials("DD"), status: .idle, serverTag: "TEST", activity: "Playing Placeholder Quest"),
        DMEntry(id: "d5", name: "Eve Mockup", avatar: .initials("EM"), status: .offline),
        DMEntry(id: "d6", name: "Fig Fixture", avatar: .initials("FF"), status: .doNotDisturb),
    ]

    /// Reply bar with a custom emoji segment (`url` nil: the view draws its placeholder glyph).
    public static let reply = ReplyPreview(authorName: "Alpha Tester", segments: [
        .text("lorem ipsum "), .customEmoji(name: "cat_example", url: nil),
    ])

    public static let demo = ShellState(
        rail: rail, selectedRailID: "g-lorem", serverName: "Lorem Ipsum Club",
        categories: categories, selectedChannelID: "c-general",
        messages: messages, memberSections: memberSections, currentUser: currentUser,
        dmTopRows: dmTopRows, dms: dms, selectedDMID: "d1", reply: reply
    )
}

public extension ShellState {
    static var demo: ShellState { Fixtures.demo }
}
