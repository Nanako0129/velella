import Foundation

/// Plain value types for what the shell shows. No FFI types: a later slice maps engine
/// snapshots onto these.

public struct RGB: Hashable, Sendable {
    public var red: Double, green: Double, blue: Double
    public init(_ red: Double, _ green: Double, _ blue: Double) {
        self.red = red; self.green = green; self.blue = blue
    }
}

public enum Avatar: Hashable, Sendable {
    case url(URL)
    case initials(String)
    case symbol(String)
}

// TODO(display-name-styles): field names unconfirmed; verify from live payload in S3
/// Nitro display-name styling. nil on a user/member means the default rendering.
public struct DisplayNameStyle: Hashable, Sendable {
    public enum Font: Hashable, Sendable { case `default`, serif, rounded, monospaced }
    public enum Effect: Hashable, Sendable { case none, glow }
    public var font: Font
    /// One colour is solid, two or more form a gradient.
    public var colors: [RGB]
    public var effect: Effect
    public init(font: Font = .default, colors: [RGB] = [], effect: Effect = .none) {
        self.font = font; self.colors = colors; self.effect = effect
    }
}

public enum PresenceStatus: Hashable, Sendable {
    case online, idle, doNotDisturb, offline
}

// MARK: Guild rail

public struct RailGuild: Identifiable, Hashable, Sendable {
    public var id: String
    public var name: String
    public var icon: Avatar
    public var hasUnread: Bool
    public var mentionCount: Int
    public init(id: String, name: String, icon: Avatar, hasUnread: Bool = false, mentionCount: Int = 0) {
        self.id = id; self.name = name; self.icon = icon
        self.hasUnread = hasUnread; self.mentionCount = mentionCount
    }
}

public struct RailFolder: Identifiable, Hashable, Sendable {
    public var id: String
    public var name: String?
    public var color: RGB
    public var children: [RailGuild]
    public init(id: String, name: String? = nil, color: RGB, children: [RailGuild]) {
        self.id = id; self.name = name; self.color = color; self.children = children
    }
}

public struct RailDM: Identifiable, Hashable, Sendable {
    public var id: String
    public var name: String
    public var icon: Avatar
    public var hasUnread: Bool
    public var mentionCount: Int
    public init(id: String, name: String, icon: Avatar, hasUnread: Bool = false, mentionCount: Int = 0) {
        self.id = id; self.name = name; self.icon = icon
        self.hasUnread = hasUnread; self.mentionCount = mentionCount
    }
}

public enum RailItem: Identifiable, Hashable, Sendable {
    case guild(RailGuild)
    case folder(RailFolder)
    case dm(RailDM)

    public var id: String {
        switch self {
        case .guild(let g): g.id
        case .folder(let f): f.id
        case .dm(let d): d.id
        }
    }
}

// MARK: Channels

public enum ChannelKind: Hashable, Sendable {
    case text, announcement, forum, voice, stage

    /// SF Symbol shown before the channel name: Swiftcord legacy's filled set (ChannelButton.swift
    /// `chIcons`), plus the filled forum and stage symbols Swiftcord v2 draws.
    public var symbol: String {
        switch self {
        case .text: "number"
        case .announcement: "megaphone.fill"
        case .forum: "bubble.left.and.bubble.right.fill"
        case .voice: "speaker.wave.2.fill"
        case .stage: "person.wave.2.fill"
        }
    }
}

public struct VoiceOccupant: Identifiable, Hashable, Sendable {
    public var id: String
    public var name: String
    public var avatar: Avatar
    public init(id: String, name: String, avatar: Avatar) {
        self.id = id; self.name = name; self.avatar = avatar
    }
}

public struct ChannelItem: Identifiable, Hashable, Sendable {
    public var id: String
    public var name: String
    public var kind: ChannelKind
    public var topic: String?
    public var isUnread: Bool
    public var isMuted: Bool
    /// Hidden from @everyone; drawn with a lock on the glyph.
    public var isPrivate: Bool
    public var occupants: [VoiceOccupant]
    /// Threads drawn under this channel; a thread's own `threads` stay empty.
    public var threads: [ChannelItem]
    public init(
        id: String, name: String, kind: ChannelKind = .text, topic: String? = nil,
        isUnread: Bool = false, isMuted: Bool = false, isPrivate: Bool = false,
        occupants: [VoiceOccupant] = [], threads: [ChannelItem] = []
    ) {
        self.id = id; self.name = name; self.kind = kind; self.topic = topic
        self.isUnread = isUnread; self.isMuted = isMuted; self.isPrivate = isPrivate
        self.occupants = occupants; self.threads = threads
    }
}

public struct ChannelCategory: Identifiable, Hashable, Sendable {
    public var id: String
    /// nil for channels that sit above every category.
    public var name: String?
    public var channels: [ChannelItem]
    public init(id: String, name: String?, channels: [ChannelItem]) {
        self.id = id; self.name = name; self.channels = channels
    }
}

public struct ChannelHeader: Hashable, Sendable {
    public var name: String
    public var topic: String?
    /// nil for direct messages, whose title carries no channel glyph.
    public var kind: ChannelKind?
    public init(name: String, topic: String? = nil, kind: ChannelKind? = .text) {
        self.name = name; self.topic = topic; self.kind = kind
    }
}

// MARK: Messages (placeholder rows; real message rows come in a later slice)

public struct MessagePlaceholder: Identifiable, Hashable, Sendable {
    public var id: String
    public var authorName: String
    public var authorColor: RGB?
    public var avatar: Avatar
    public var timestamp: Date
    public var text: String
    public var displayNameStyle: DisplayNameStyle?
    public var image: ImageAttachment?
    public init(id: String, authorName: String, authorColor: RGB? = nil, avatar: Avatar, timestamp: Date, text: String, displayNameStyle: DisplayNameStyle? = nil, image: ImageAttachment? = nil) {
        self.id = id; self.authorName = authorName; self.authorColor = authorColor
        self.avatar = avatar; self.timestamp = timestamp; self.text = text
        self.displayNameStyle = displayNameStyle; self.image = image
    }
}

/// An image under a message. Fixtures draw generated art from `seed` (no network, no bundled
/// files); real rows will carry a URL once messages are wired.
public struct ImageAttachment: Hashable, Sendable {
    public var seed: Int
    public var width: Double
    public var height: Double
    public init(seed: Int, width: Double, height: Double) {
        self.seed = seed; self.width = width; self.height = height
    }
}

// MARK: Members

public struct MemberRow: Identifiable, Hashable, Sendable {
    public var id: String
    public var name: String
    public var avatar: Avatar
    public var roleColor: RGB?
    public var status: PresenceStatus
    public var customStatus: String?
    public var displayNameStyle: DisplayNameStyle?
    public init(id: String, name: String, avatar: Avatar, roleColor: RGB? = nil, status: PresenceStatus = .online, customStatus: String? = nil, displayNameStyle: DisplayNameStyle? = nil) {
        self.id = id; self.name = name; self.avatar = avatar; self.roleColor = roleColor
        self.status = status; self.customStatus = customStatus; self.displayNameStyle = displayNameStyle
    }
}

public struct MemberSection: Identifiable, Hashable, Sendable {
    public var id: String
    public var title: String
    /// Total members in the role; may exceed `members.count` while the list is partially loaded.
    public var count: Int
    public var members: [MemberRow]
    public init(id: String, title: String, count: Int, members: [MemberRow]) {
        self.id = id; self.title = title; self.count = count; self.members = members
    }
}

// MARK: Current user

public struct CurrentUser: Hashable, Sendable {
    public var displayName: String
    public var username: String
    public var avatar: Avatar
    public var status: PresenceStatus
    public var displayNameStyle: DisplayNameStyle?
    public init(displayName: String, username: String, avatar: Avatar, status: PresenceStatus = .online, displayNameStyle: DisplayNameStyle? = nil) {
        self.displayName = displayName; self.username = username; self.avatar = avatar; self.status = status
        self.displayNameStyle = displayNameStyle
    }
}

// MARK: Direct messages

/// Top rows of the DM column (Friends, Message Requests, the app's own entry).
public struct DMTopRow: Identifiable, Hashable, Sendable {
    public var id: String
    public var title: String
    public var symbol: String
    public var badgeCount: Int
    public init(id: String, title: String, symbol: String, badgeCount: Int = 0) {
        self.id = id; self.title = title; self.symbol = symbol; self.badgeCount = badgeCount
    }
}

public struct DMEntry: Identifiable, Hashable, Sendable {
    public var id: String
    public var name: String
    public var avatar: Avatar
    /// nil draws no presence dot (live DMs: presence is not wired yet).
    public var status: PresenceStatus?
    /// Server-tag pill next to the name, e.g. "DEV".
    public var serverTag: String?
    /// Activity subtitle under the name, e.g. "Playing Placeholder Quest".
    public var activity: String?
    public var displayNameStyle: DisplayNameStyle?
    public init(id: String, name: String, avatar: Avatar, status: PresenceStatus? = .offline, serverTag: String? = nil, activity: String? = nil, displayNameStyle: DisplayNameStyle? = nil) {
        self.id = id; self.name = name; self.avatar = avatar; self.status = status
        self.serverTag = serverTag; self.activity = activity; self.displayNameStyle = displayNameStyle
    }
}

// MARK: Composer reply bar

/// One piece of a one-line message preview. Custom emoji are their own segment so the reply bar
/// draws an image instead of printing raw `<:name:id>` markup.
public enum PreviewSegment: Hashable, Sendable {
    case text(String)
    case customEmoji(name: String, url: URL?)
}

public struct ReplyPreview: Hashable, Sendable {
    public var authorName: String
    public var segments: [PreviewSegment]
    public init(authorName: String, segments: [PreviewSegment]) {
        self.authorName = authorName; self.segments = segments
    }
}

// MARK: Whole shell

public struct ShellState: Equatable, Sendable {
    public var rail: [RailItem]
    public var selectedRailID: String?
    public var serverName: String?
    public var categories: [ChannelCategory]
    public var selectedChannelID: String?
    public var messages: [MessagePlaceholder]
    public var memberSections: [MemberSection]
    public var currentUser: CurrentUser?
    public var dmTopRows: [DMTopRow]
    public var dms: [DMEntry]
    public var selectedDMID: String?
    /// Set while the composer is replying to a message.
    public var reply: ReplyPreview?

    public init(
        rail: [RailItem] = [], selectedRailID: String? = nil, serverName: String? = nil,
        categories: [ChannelCategory] = [], selectedChannelID: String? = nil,
        messages: [MessagePlaceholder] = [], memberSections: [MemberSection] = [],
        currentUser: CurrentUser? = nil,
        dmTopRows: [DMTopRow] = [], dms: [DMEntry] = [], selectedDMID: String? = nil,
        reply: ReplyPreview? = nil
    ) {
        self.rail = rail; self.selectedRailID = selectedRailID; self.serverName = serverName
        self.categories = categories; self.selectedChannelID = selectedChannelID
        self.messages = messages; self.memberSections = memberSections; self.currentUser = currentUser
        self.dmTopRows = dmTopRows; self.dms = dms; self.selectedDMID = selectedDMID; self.reply = reply
    }

    /// Nothing loaded: the "not signed in" shell.
    public static let signedOut = ShellState()

    public var isSignedIn: Bool { currentUser != nil }

    /// True while the selected rail entry is the direct-messages entry.
    public var isDMMode: Bool {
        rail.contains { if case .dm(let dm) = $0 { dm.id == selectedRailID } else { false } }
    }

    public var header: ChannelHeader? {
        if isDMMode {
            return dms.first { $0.id == selectedDMID }.map { ChannelHeader(name: $0.name, kind: nil) }
        }
        return categories.flatMap(\.channels).flatMap { [$0] + $0.threads }.first { $0.id == selectedChannelID }
            .map { ChannelHeader(name: $0.name, topic: $0.topic, kind: $0.kind) }
    }
}
