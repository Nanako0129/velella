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
    /// Dimmed on the rail. A muted server carries no unread or mention state (the engine zeroes it).
    public var isMuted: Bool
    public init(id: String, name: String, icon: Avatar, hasUnread: Bool = false, mentionCount: Int = 0, isMuted: Bool = false) {
        self.id = id; self.name = name; self.icon = icon
        self.hasUnread = hasUnread; self.mentionCount = mentionCount; self.isMuted = isMuted
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

    /// What the closed folder tile shows: the children's mentions summed, and an unread mark when
    /// any child is unread. Computed from the children, so the engine sends nothing for folders.
    public var mentionCount: Int { children.reduce(0) { $0 + $1.mentionCount } }
    public var hasUnread: Bool { children.contains { $0.hasUnread } }
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
    public var mentionCount: Int
    public var isMuted: Bool
    /// Hidden from @everyone; drawn with a lock on the glyph.
    public var isPrivate: Bool
    public var occupants: [VoiceOccupant]
    /// Threads drawn under this channel; a thread's own `threads` stay empty.
    public var threads: [ChannelItem]
    public init(
        id: String, name: String, kind: ChannelKind = .text, topic: String? = nil,
        isUnread: Bool = false, mentionCount: Int = 0, isMuted: Bool = false, isPrivate: Bool = false,
        occupants: [VoiceOccupant] = [], threads: [ChannelItem] = []
    ) {
        self.id = id; self.name = name; self.kind = kind; self.topic = topic
        self.isUnread = isUnread; self.mentionCount = mentionCount
        self.isMuted = isMuted; self.isPrivate = isPrivate
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

// MARK: Messages

/// One message row. `body` renders through `MarkdownView` (core's AST only); `authorName` and the
/// reply preview are shown verbatim (plan M, F5 as restated in the markdown plan).
public struct MessageItem: Identifiable, Hashable, Sendable {
    /// A local echo is `pending` until Discord confirms it, or `failed` with fixed text.
    public enum Delivery: Hashable, Sendable {
        case sent, pending
        case failed(String)
    }

    public var id: String
    /// Who wrote it: consecutive rows of one author group under one header (T1).
    public var authorId: String
    public var authorName: String
    public var authorColor: RGB?
    public var avatar: Avatar
    public var timestamp: Date
    /// The raw content; `body` is what shows.
    public var text: String
    public var body: MessageBody
    public var displayNameStyle: DisplayNameStyle?
    public var images: [ImageAttachment]
    /// Filenames of non-image attachments.
    public var files: [String]
    /// The message this one replies to.
    public var reply: ReplyPreview?
    public var edited: Bool
    public var delivery: Delivery
    /// First-seen order, at most 20; read-only pills under the message.
    public var reactions: [ReactionItem]
    /// At most 10, under the attachments (wave 3 E).
    public var embeds: [EmbedItem]
    /// A send with files not yet confirmed (composer plan CUA): the row draws upload cards, and
    /// core's echo row is the only source of its progress and outcome.
    public var upload: PendingUploadItem?
    /// Without a `body`, `text` shows as one plain paragraph. Without an `authorId` (fixtures,
    /// previews) the name stands in for it.
    public init(
        id: String, authorId: String? = nil, authorName: String, authorColor: RGB? = nil, avatar: Avatar, timestamp: Date, text: String,
        body: MessageBody? = nil,
        displayNameStyle: DisplayNameStyle? = nil, images: [ImageAttachment] = [], files: [String] = [],
        reply: ReplyPreview? = nil, edited: Bool = false, delivery: Delivery = .sent, reactions: [ReactionItem] = [],
        embeds: [EmbedItem] = [], upload: PendingUploadItem? = nil
    ) {
        self.id = id; self.authorId = authorId ?? authorName; self.authorName = authorName; self.authorColor = authorColor
        self.avatar = avatar; self.timestamp = timestamp; self.text = text
        self.body = body ?? MessageBody(text: text)
        self.displayNameStyle = displayNameStyle; self.images = images; self.files = files
        self.reply = reply; self.edited = edited; self.delivery = delivery; self.reactions = reactions
        self.embeds = embeds; self.upload = upload
    }
}

/// An image under a message: a URL for live rows (a Discord media host and dimensions > 0, both
/// checked in core), or generated art from `seed` for fixtures (no network, no bundled files).
public struct ImageAttachment: Hashable, Sendable {
    public var seed: Int
    public var url: URL?
    public var width: Double
    public var height: Double
    /// Covered, and not fetched, until clicked (IS_SPOILER or a `SPOILER_` filename).
    public var spoiler = false
    public init(seed: Int, width: Double, height: Double) {
        self.seed = seed; self.width = width; self.height = height
    }
    public init(url: URL, width: Double, height: Double, spoiler: Bool = false) {
        self.seed = 0; self.url = url; self.width = width; self.height = height; self.spoiler = spoiler
    }
}

public extension MessageItem {
    /// One engine row, passed as plain values (VelellaUI sees no FFI types; the app maps core's
    /// AST onto `body`). Text is kept as is.
    static func live(
        id: UInt64, authorId: UInt64, authorName: String, authorColor: UInt32?, avatarURL: String, timestampMs: UInt64,
        content: String, body: MessageBody, edited: Bool, images: [(url: String, width: UInt32, height: UInt32)],
        files: [String], replyAuthor: String?, replyContent: String?, pending: Bool, failure: String?,
        reactions: [(name: String, url: String?, count: UInt32, me: Bool)] = [],
        embeds: [EmbedItem] = [], imageSpoilers: [Bool] = [], upload: PendingUploadItem? = nil
    ) -> MessageItem {
        MessageItem(
            id: String(id), authorId: String(authorId), authorName: authorName, authorColor: authorColor.map { RGB(rgb: $0) },
            avatar: URL(string: avatarURL).map(Avatar.url) ?? .initials(initials(of: authorName)),
            timestamp: Date(timeIntervalSince1970: Double(timestampMs) / 1000), text: content, body: body,
            images: images.enumerated().compactMap { i, image in
                URL(string: image.url).map {
                    ImageAttachment(url: $0, width: Double(image.width), height: Double(image.height), spoiler: imageSpoilers.indices.contains(i) && imageSpoilers[i])
                }
            },
            files: files,
            reply: replyAuthor.map { ReplyPreview(authorName: $0, segments: [.text(replyContent ?? "")]) },
            edited: edited,
            delivery: failure.map(Delivery.failed) ?? (pending ? .pending : .sent),
            reactions: reactions.map { ReactionItem(name: $0.name, imageURL: $0.url.flatMap { URL(string: $0) }, count: $0.count, me: $0.me) },
            embeds: embeds, upload: upload
        )
    }
}

// MARK: Composer submit

/// The composer's submit rule (plan M, F10): blank drafts and a Return that commits an input
/// method's marked text never send; otherwise the draft is sent once and cleared at once.
public enum ComposerSubmit {
    public static func submit(_ draft: inout String, hasMarkedText: Bool, onSend: ((String) -> Void)?) {
        guard let onSend, !hasMarkedText, !draft.allSatisfy(\.isWhitespace) else { return }
        let text = draft
        draft = ""
        onSend(text)
    }

    /// What a Return did.
    public enum Outcome: Equatable, Sendable {
        case sent
        /// Nothing to send: no handler, an input method's marked text, or a blank draft.
        case ignored
        /// Over the limit (official validator MESSAGE_TOO_LONG): nothing was sent and the draft is kept.
        case tooLong(length: Int, limit: Int)
        /// Core's attachment limits refused the send: the draft, the reply and the files are kept.
        case attachmentLimit(AttachmentLimit)
    }

    /// `submit` for a composer holding queued files (composer plan CUA, OA A11): a Return sends even
    /// with a blank draft, the files going through `onSendFiles`. Nothing is cleared unless core
    /// started the send (`.started`); the text length rule is the plain submit's. Without files this
    /// is the plain submit.
    @discardableResult
    public static func submit(
        _ draft: inout String, reply: inout ReplyPreview?, attachments: inout [QueuedAttachment], hasMarkedText: Bool,
        limit: Int = .max, onSend: ((String, ReplyTarget?) -> Void)?,
        onSendFiles: ((String, [QueuedAttachment], ReplyTarget?) -> AttachmentSendOutcome)?
    ) -> Outcome {
        guard !attachments.isEmpty else { return submit(&draft, reply: &reply, hasMarkedText: hasMarkedText, limit: limit, onSend: onSend) }
        guard let onSendFiles, !hasMarkedText else { return .ignored }
        let length = MessageLength.length(draft)
        if length > limit { return .tooLong(length: length, limit: limit) }
        switch onSendFiles(draft, attachments, reply?.target) {
        case .started:
            draft = ""
            reply = nil
            attachments = []
            return .sent
        case .limit(let limit): return .attachmentLimit(limit)
        case .refused: return .ignored
        }
    }

    /// `submit` for a composer that may be replying: the send carries the reply's whole target
    /// (message and channel), and the reply bar clears with the draft, only when something was sent.
    /// A draft longer than `limit` UTF-16 units (`MessageLength`) sends nothing and is never truncated;
    /// the length is checked before the blank test, as the official validator does.
    @discardableResult
    public static func submit(
        _ draft: inout String, reply: inout ReplyPreview?, hasMarkedText: Bool, limit: Int = .max,
        onSend: ((String, ReplyTarget?) -> Void)?
    ) -> Outcome {
        guard let onSend, !hasMarkedText else { return .ignored }
        let length = MessageLength.length(draft)
        if length > limit { return .tooLong(length: length, limit: limit) }
        guard !draft.allSatisfy(\.isWhitespace) else { return .ignored }
        let text = draft, target = reply?.target
        draft = ""
        reply = nil
        onSend(text, target)
        return .sent
    }
}

// MARK: Message length

/// The official long-text rules (composer plan CG; OC L1-L9): the limit, the remaining counter and
/// what a Return over the limit shows. Lengths are UTF-16 units of the draft as sent (D9).
public enum MessageLength {
    public static let standardLimit = 2000
    public static let nitroLimit = 4000

    /// 4000 for `premium_type` 2 (Nitro: only that tier carries INCREASED_MESSAGE_LENGTH), else 2000;
    /// Unknown (nil, before READY) counts as 2000.
    public static func limit(premiumType: UInt8?) -> Int { premiumType == 2 ? nitroLimit : standardLimit }

    public static func length(_ draft: String) -> Int { draft.utf16.count }

    public struct Counter: Equatable, Sendable {
        /// Negative over the limit; the text never shows it.
        public var remaining: Int
        public var text: String
        /// Over the limit: the critical colour.
        public var isCritical: Bool { remaining < 0 }
        /// A user who cannot type 4000 also sees the inline upsell line under the counter.
        public var showsUpsell: Bool
    }

    /// The counter for `draft`, or nil while hidden: shown when remaining is at most a tenth of the
    /// limit, or for a Nitro user once the draft passes 2000.
    public static func counter(draft: String, premiumType: UInt8?) -> Counter? {
        let max = limit(premiumType: premiumType), remaining = max - length(draft)
        let nitro = max == nitroLimit
        guard (nitro && length(draft) > standardLimit) || remaining <= max / 10 else { return nil }
        let text = remaining > 0 ? "\(remaining) characters remaining"
            : remaining == 0 ? "Message is at max length" : "Message is too long"
        return Counter(remaining: remaining, text: text, showsUpsell: !nitro)
    }

    /// What a Return over the limit shows (OC L7): Nitro users the alert, the others the upsell (D7).
    public enum TooLong: Equatable, Sendable {
        case alert(title: String, message: String)
        case upsell(title: String, message: String)
    }

    public static let tooLongTitle = "Your message is too long."
    /// OC L6, with `{premiumMaxMessageLength}` filled in; the "Learn more" that follows it is a link.
    public static let upsellCopy = "Lift off with Discord Nitro to type to your heart's content, up to \(nitroLimit) characters."
    public static let nitroURL = URL(string: "https://discord.com/nitro")!

    public static func tooLong(length: Int, limit: Int, premiumType: UInt8?) -> TooLong {
        premiumType == 2
            ? .alert(title: tooLongTitle, message: "Please make your message of \(length) characters shorter. We've set the limit at \(limit) characters to be courteous to others.")
            : .upsell(title: tooLongTitle, message: upsellCopy)
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
    /// nil draws no presence dot (group DMs).
    public var status: PresenceStatus?
    /// Server-tag pill next to the name, e.g. "DEV".
    public var serverTag: String?
    /// Activity subtitle under the name, e.g. "Playing Placeholder Quest".
    public var activity: String?
    public var displayNameStyle: DisplayNameStyle?
    /// Both zero/false while the DM is muted (the engine zeroes them).
    public var unread: Bool
    public var mentionCount: Int
    public init(id: String, name: String, avatar: Avatar, status: PresenceStatus? = .offline, serverTag: String? = nil, activity: String? = nil, displayNameStyle: DisplayNameStyle? = nil, unread: Bool = false, mentionCount: Int = 0) {
        self.id = id; self.name = name; self.avatar = avatar; self.status = status
        self.serverTag = serverTag; self.activity = activity; self.displayNameStyle = displayNameStyle
        self.unread = unread; self.mentionCount = mentionCount
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
    /// The replied message's id: what the composer sends as the reply target. nil (fixtures,
    /// previews) sends a plain message.
    public var messageID: String?
    /// The channel the replied message was picked in; the app sends the reply only there.
    public var channelID: String?
    public init(authorName: String, segments: [PreviewSegment], messageID: String? = nil, channelID: String? = nil) {
        self.authorName = authorName; self.segments = segments; self.messageID = messageID; self.channelID = channelID
    }

    /// What the composer sends with the draft: nil unless both ids are known.
    public var target: ReplyTarget? {
        guard let messageID, let channelID else { return nil }
        return ReplyTarget(messageID: messageID, channelID: channelID)
    }
}

/// A reply's target: the replied message and the channel it was picked in (selection plan C1).
public struct ReplyTarget: Hashable, Sendable {
    public var messageID: String
    public var channelID: String
    public init(messageID: String, channelID: String) {
        self.messageID = messageID; self.channelID = channelID
    }

    /// The message id to send as the reply target of a message posted to `channel`: nil (a plain
    /// message) without a target, for a target picked in another channel, or when an id is not a
    /// `UInt64`. A target never follows the draft into another conversation.
    public static func replyID(_ target: ReplyTarget?, sendingTo channel: UInt64) -> UInt64? {
        guard let target, UInt64(target.channelID) == channel else { return nil }
        return UInt64(target.messageID)
    }
}

/// The ids of the conversation the message rows were loaded for (selection plan C1, Ids source).
/// The app writes them together with the rows, so a menu opened on a row builds its message link
/// from the conversation the row belongs to, never from a selection that moved since.
public struct ConversationIDs: Hashable, Sendable {
    public var channelID: String
    /// nil for a direct message.
    public var guildID: String?
    public init(channelID: String, guildID: String?) {
        self.channelID = channelID; self.guildID = guildID
    }

    /// From the engine's ids: nil without a channel; `guild` nil for a DM.
    public init?(channel: UInt64?, guild: UInt64?) {
        guard let channel else { return nil }
        self.init(channelID: String(channel), guildID: guild.map { String($0) })
    }
}

// MARK: Message loading

/// Load state of the open channel's messages; `failed` carries fixed text for the message column.
public enum MessageLoad: Hashable, Sendable {
    case idle, loading, loaded
    case failed(String)
}

// MARK: Whole shell

public struct ShellState: Equatable, Sendable {
    public var rail: [RailItem]
    public var selectedRailID: String?
    public var serverName: String?
    public var categories: [ChannelCategory]
    public var selectedChannelID: String?
    public var messages: [MessageItem]
    /// The conversation `messages` were loaded for, written with them (nil with no rows from the
    /// engine: fixtures, or before the first snapshot); the message menu reads it.
    public var conversation: ConversationIDs?
    public var memberSections: [MemberSection]
    public var currentUser: CurrentUser?
    public var dmTopRows: [DMTopRow]
    public var dms: [DMEntry]
    public var selectedDMID: String?
    /// Set while the composer is replying to a message.
    public var reply: ReplyPreview?
    public var messageLoad: MessageLoad = .idle
    /// Display names of who is typing in the open conversation (core expires them after 10 s).
    public var typing: [String] = []
    /// Unsent composer text per conversation (`openTarget`), as the official client keeps it.
    public var drafts: [String: String] = [:]
    /// The macOS preferred language (BCP 47), for message times; ShellView puts it in the
    /// environment as `discordLocale`. Not the Discord account's: READY's legacy
    /// `user_settings.locale` was stale against the official client (P1).
    public var locale: String
    /// Older-history state of the open channel's messages (wave 3 P).
    public var older: OlderState = .idle
    /// The user's `premium_type` from core's `self_state()` (nil: Unknown, before READY); the
    /// composer's message-length limit follows it (`MessageLength.limit`).
    public var premiumType: UInt8?
    /// Files queued in each conversation's composer (`openTarget`), kept like the drafts.
    public var attachmentQueues: [String: [QueuedAttachment]] = [:]
    /// What the open conversation allows (ATTACH_FILES); Unknown until core answers.
    public var attachPermission: AttachPermission = .unknown

    public init(
        rail: [RailItem] = [], selectedRailID: String? = nil, serverName: String? = nil,
        categories: [ChannelCategory] = [], selectedChannelID: String? = nil,
        messages: [MessageItem] = [], memberSections: [MemberSection] = [],
        currentUser: CurrentUser? = nil,
        dmTopRows: [DMTopRow] = [], dms: [DMEntry] = [], selectedDMID: String? = nil,
        reply: ReplyPreview? = nil,
        preferredLanguages: [String] = Locale.preferredLanguages
    ) {
        self.locale = preferredLanguages.first ?? "en-US"
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

    /// The conversation the message column shows: the selected DM in DM mode, else the selected
    /// channel (the same rule as `header`). nil when nothing is open.
    public var openTarget: String? {
        isDMMode ? selectedDMID : selectedChannelID
    }

    /// The composer's text: the open conversation's own draft, so switching conversations never
    /// carries text (and a Return) from one to the other.
    public var draft: String {
        get { openTarget.flatMap { drafts[$0] } ?? "" }
        set { if let openTarget { drafts[openTarget] = newValue.isEmpty ? nil : newValue } }
    }

    /// The open conversation's queued files.
    public var attachmentQueue: [QueuedAttachment] {
        get { openTarget.flatMap { attachmentQueues[$0] } ?? [] }
        set { if let openTarget { attachmentQueues[openTarget] = newValue.isEmpty ? nil : newValue } }
    }

    public var header: ChannelHeader? {
        if isDMMode {
            return dms.first { $0.id == selectedDMID }.map { ChannelHeader(name: $0.name, kind: nil) }
        }
        return categories.flatMap(\.channels).flatMap { [$0] + $0.threads }.first { $0.id == selectedChannelID }
            .map { ChannelHeader(name: $0.name, topic: $0.topic, kind: $0.kind) }
    }
}

// MARK: Attachments (composer plan CUA)

/// An attached file's identity, recorded when it was attached (CU-6): the resolved path and
/// (device, inode, size, mtime), compared again when the upload opens the file.
public struct QueuedFileIdentity: Hashable, Sendable {
    public var path: String
    public var device: UInt64, inode: UInt64, size: UInt64, mtimeNs: Int64
    public init(path: String, device: UInt64, inode: UInt64, size: UInt64, mtimeNs: Int64) {
        self.path = path; self.device = device; self.inode = inode; self.size = size; self.mtimeNs = mtimeNs
    }
}

/// One file queued in a composer, or one file of a pending send: nothing is uploaded until Return (D10).
public struct QueuedAttachment: Identifiable, Hashable, Sendable {
    public var id: UUID
    public var file: QueuedFileIdentity
    /// The name the message shows (Modify edits it); the upload slot keeps the file's own name.
    public var filename: String
    /// Alt text; sent whenever set, `""` included (OA A10).
    public var altText: String?
    public var spoiler: Bool
    /// The original's MIME type, `""` when unknown (CU-5).
    public var contentType: String
    /// A converted HEIC or JXR's original MD5, lowercase hex (D16a).
    public var originalMD5: String?

    public init(
        id: UUID = UUID(), file: QueuedFileIdentity, filename: String? = nil, altText: String? = nil, spoiler: Bool = false,
        contentType: String = "", originalMD5: String? = nil
    ) {
        self.id = id; self.file = file
        self.filename = filename ?? URL(fileURLWithPath: file.path).lastPathComponent
        self.altText = altText; self.spoiler = spoiler; self.contentType = contentType; self.originalMD5 = originalMD5
    }

    public var url: URL { URL(fileURLWithPath: file.path) }
    public var size: UInt64 { file.size }
}

/// What the open conversation lets the user attach (OA A26): ATTACH_FILES in a guild channel, always
/// in a DM. Unknown (before core has the answer) hides the upload entries, as menu S-6 does.
public enum AttachPermission: Equatable, Sendable {
    case allowed, denied, unknown

    public var permitsAttaching: Bool { self == .allowed }

    /// A guild channel's permission bits; `attachBit` is ATTACH_FILES' number in the official table.
    public init(bits: UInt64, attachBit: UInt8) {
        self = bits & (1 << UInt64(attachBit)) != 0 ? .allowed : .denied
    }
}

/// A send with files, as the message row draws it (the single source is core's echo row).
public struct PendingUploadItem: Hashable, Sendable {
    public struct File: Hashable, Sendable {
        public var attachment: QueuedAttachment
        /// Bytes the upload host has.
        public var sent: UInt64
        public init(attachment: QueuedAttachment, sent: UInt64) { self.attachment = attachment; self.sent = sent }
        /// 0...1, for the progress bar.
        public var fraction: Double { attachment.size == 0 ? 0 : min(1, Double(sent) / Double(attachment.size)) }
    }
    public enum State: Hashable, Sendable { case sending, failed, cancelled }

    public var files: [File]
    public var replyTo: UInt64?
    public var instant: Bool
    public var state: State
    public init(files: [File], replyTo: UInt64? = nil, instant: Bool = false, state: State) {
        self.files = files; self.replyTo = replyTo; self.instant = instant; self.state = state
    }
}

/// The sends this run started, by their row's id: which conversation, and which files to delete once
/// the row is gone (CU-1: sent, discarded or torn down; a cancelled send's files go back to the composer).
public struct PendingSends: Sendable {
    private struct Send { var conversation: String; var paths: [String]; var seen = false }
    private var sends: [String: Send] = [:]
    public init() {}

    public mutating func begin(id: String, conversation: String, paths: [String]) {
        sends[id] = Send(conversation: conversation, paths: paths)
    }

    /// The paths of send `id`, forgotten (a discarded failed send).
    public mutating func take(id: String) -> [String] { sends.removeValue(forKey: id)?.paths ?? [] }

    /// Every tracked path (sign-out, teardown).
    public mutating func takeAll() -> [String] {
        defer { sends = [:] }
        return sends.values.flatMap(\.paths)
    }

    /// The tracked sends of `conversation` whose row was in an earlier snapshot and is not in `rows`:
    /// confirmed, or discarded elsewhere. Their paths, forgotten. A send never seen yet is kept: a
    /// snapshot taken before it began can still arrive.
    mutating func finished(conversation: String, rows: [MessageItem]) -> [String] {
        let present = Set(rows.map(\.id))
        var gone: [String] = []
        for (id, send) in sends where send.conversation == conversation {
            if present.contains(id) {
                sends[id]?.seen = true
            } else if send.seen {
                gone.append(contentsOf: send.paths)
                sends[id] = nil
            }
        }
        return gone
    }

    func tracks(_ id: String) -> Bool { sends[id] != nil }
}

public extension ShellState {
    /// Applies a fresh snapshot of `conversation`'s rows to the sends this run started (CU Flow, D14a):
    /// a cancelled send's text, reply and files go back to the composer (its row is then discarded);
    /// the files of a send whose row left (confirmed) are returned for deletion. Rows of sends this
    /// run did not start are left alone.
    mutating func reconcileUploads(
        conversation: String, rows: [MessageItem], sends: inout PendingSends
    ) -> (discard: [String], deletePaths: [String]) {
        var discard: [String] = []
        for row in rows where row.upload?.state == .cancelled && sends.tracks(row.id) {
            _ = sends.take(id: row.id)
            restoreCancelled(row, conversation: conversation)
            discard.append(row.id)
        }
        return (discard, sends.finished(conversation: conversation, rows: rows))
    }

    /// D14a: puts a cancelled send's files first in the queue, its text before whatever was typed
    /// since, and its reply back when none is set and the row still shows it.
    mutating func restoreCancelled(_ message: MessageItem, conversation: String) {
        guard let upload = message.upload else { return }
        attachmentQueues[conversation, default: []].insert(contentsOf: upload.files.map(\.attachment), at: 0)
        if !message.text.isEmpty {
            let typed = drafts[conversation] ?? ""
            drafts[conversation] = typed.isEmpty ? message.text : message.text + "\n" + typed
        }
        if reply == nil, let id = upload.replyTo, let preview = message.reply {
            reply = ReplyPreview(authorName: preview.authorName, segments: preview.segments, messageID: String(id), channelID: conversation)
        }
    }

    /// Every queued file, removed (sign-out, teardown).
    mutating func takeAllAttachments() -> [QueuedAttachment] {
        defer { attachmentQueues = [:] }
        return attachmentQueues.values.flatMap { $0 }
    }
}

// MARK: Attachment services (composer plan CUA)

/// The limit a set of files breaks (core's `attachment_limits`); the composer shows the official
/// alert for each (OA A18, A37).
public enum AttachmentLimit: Equatable, Sendable {
    case tooManyFiles(limit: Int)
    case emptyFile
    case fileTooLarge(limit: UInt64)
    case totalTooLarge(limit: UInt64)
}

/// When a limits check runs (OA A36): on add, an account with premium is checked only for count and
/// emptiness; at send, every account is checked in full.
public enum LimitStage: Sendable { case add, send }

/// The outcome of asking core to send files.
public enum AttachmentSendOutcome: Equatable, Sendable {
    /// The send is under way: its echo row appears in the message list.
    case started
    case limit(AttachmentLimit)
    /// Core sent nothing (no permission, not ready, not the open channel, no files).
    case refused
}

/// HEIC and JXR are converted to JPEG before upload exactly when the account's experiment says so (D16a).
public enum ConvertibleKind: Sendable { case heic, jxr }

public enum ImageConversionSetting: Equatable, Sendable {
    /// Disabled or not yet known: the original is uploaded.
    case off
    case enabled(quality: UInt8, maxFileSizeBytes: UInt64)
}

/// One send of files for core: the conversation is the one the files were attached to, captured when
/// the user acted (a drop, a Return), never read later.
public struct AttachmentSendRequest: Sendable {
    public var conversation: String
    public var text: String
    public var files: [QueuedAttachment]
    public var reply: ReplyTarget?
    /// Shift-drop: empty text, context location `instant_upload` (OA A35).
    public var instant: Bool
    public init(conversation: String, text: String, files: [QueuedAttachment], reply: ReplyTarget? = nil, instant: Bool = false) {
        self.conversation = conversation; self.text = text; self.files = files; self.reply = reply; self.instant = instant
    }
}

/// What the composer's attachments need from outside VelellaUI, which has no FFI: the app puts core
/// and `AttachmentFiles` behind these (the file functions are `AttachmentFiles.identity`, `write` and
/// `remove`, CU-1 and CU-6).
public struct AttachmentServices: Sendable {
    /// CU-6: the identity of a regular file (links resolved); nil for anything else.
    public var identity: @Sendable (URL) -> QueuedFileIdentity?
    /// CU-1: writes `<temporary>/velella-attachments/<UUID>/<name>`, never over another file.
    public var write: @Sendable (Data, String) throws -> URL
    /// Deletes a synthesized file with its directory; leaves any other file alone.
    public var remove: @Sendable (URL) -> Void
    /// CU-1 for received file promises: creates a fresh `<temporary>/velella-attachments/<UUID>/` and
    /// returns it; `remove` of any file inside it deletes it again.
    public var makeDirectory: @Sendable () throws -> URL
    public var limits: @MainActor @Sendable (_ conversation: String, _ sizes: [UInt64], _ stage: LimitStage) -> AttachmentLimit?
    public var conversion: @MainActor @Sendable (ConvertibleKind) -> ImageConversionSetting
    public var send: @MainActor @Sendable (AttachmentSendRequest) -> AttachmentSendOutcome
    /// Cancel, retry and discard act on a pending row by its id.
    public var cancel: @MainActor @Sendable (String) -> Void
    public var retry: @MainActor @Sendable (String) -> Void
    public var discard: @MainActor @Sendable (String) -> Void

    public init(
        identity: @escaping @Sendable (URL) -> QueuedFileIdentity?,
        write: @escaping @Sendable (Data, String) throws -> URL,
        remove: @escaping @Sendable (URL) -> Void,
        makeDirectory: @escaping @Sendable () throws -> URL,
        limits: @escaping @MainActor @Sendable (String, [UInt64], LimitStage) -> AttachmentLimit?,
        conversion: @escaping @MainActor @Sendable (ConvertibleKind) -> ImageConversionSetting,
        send: @escaping @MainActor @Sendable (AttachmentSendRequest) -> AttachmentSendOutcome,
        cancel: @escaping @MainActor @Sendable (String) -> Void,
        retry: @escaping @MainActor @Sendable (String) -> Void,
        discard: @escaping @MainActor @Sendable (String) -> Void
    ) {
        self.identity = identity; self.write = write; self.remove = remove; self.makeDirectory = makeDirectory; self.limits = limits
        self.conversion = conversion; self.send = send; self.cancel = cancel; self.retry = retry; self.discard = discard
    }
}

/// The official alert copy for each limit (OA A18, A37), read from the web client's upload-error path
/// (`openUploadError`): a per-file limit for anyone but a Nitro user (`premium_type` 2) opens the
/// upsell modal, every other case the plain one with a single "Got It".
public struct AttachmentAlert: Equatable, Sendable {
    public var title: String
    public var message: String
    /// The Nitro upsell modal: "Get Nitro" and "View All Nitro Perks" (both lead to the Nitro page,
    /// velella having no store) instead of "Got It".
    public var showsUpsell: Bool

    public static let tooPowerful = "Your files are too powerful"
    /// The upsell modal's title (`9C+41g`) and its two buttons (`pj0XBN`, `ZnqyZ2`); the primary label
    /// is the default one (a trial or discount replaces it for some accounts).
    public static let upsellTitle = "Uh oh, this file exceeds the size limit."
    public static let upsellPrimary = "Get Nitro"
    public static let upsellSecondary = "View All Nitro Perks"
    /// The plain modal's only button (`NX+WJN`).
    public static let dismiss = "Got It"
    /// What the upsell says a Nitro user may upload (`getMaxFileSizeForPremiumType(TIER_2)`): 500 MB.
    /// Unverified: under the 1 GiB rollout experiment the official client says "1GB", which the UI
    /// does not know (core's per-account answer is not passed this far).
    public static let nitroLimit: UInt64 = 524_288_000

    /// humanize.js `filesize(bytes)`, which the plain modals use (`453771.Hb`, module 796873):
    /// 1024-based, "bytes" below 1024 with no decimals, else two decimals, a space and KB/MB/GB/TB,
    /// and "," between thousands. 500 MB reads "500.00 MB", 1 GiB "1.00 GB".
    static func size(_ bytes: UInt64) -> String {
        if bytes == 0 { return "0 bytes" }
        let units = ["bytes", "KB", "MB", "GB", "TB", "PB"]
        let value = Double(bytes)
        let unit = units.indices.first { value < pow(1024, Double($0 + 1)) } ?? units.count - 1
        let decimals = unit == 0 ? 0 : 2
        let fixed = String(format: "%.\(decimals)f", value / pow(1024, Double(unit)))
        let parts = fixed.split(separator: ".", maxSplits: 1)
        var integer = String(parts[0])
        var grouped = ""
        while integer.count > 3 {
            grouped = "," + integer.suffix(3) + grouped
            integer.removeLast(3)
        }
        let fraction = parts.count > 1 ? "." + parts[1] : ""
        return integer + grouped + fraction + " " + units[unit]
    }

    /// The official `formatFileSize(bytes / 1024, {useKibibytes: true})` the upsell uses, with no
    /// `useSpace` (so no space): whole MB rounded up, and from 1024 MB whole GB. 20 MiB is "20MB".
    static func upsellSize(_ bytes: UInt64) -> String {
        let megabytes = Int((Double(bytes) / 1024 / 1024).rounded(.up))
        return megabytes < 1024 ? "\(megabytes)MB" : "\(Int((Double(megabytes) / 1024).rounded()))GB"
    }

    public init(for limit: AttachmentLimit, premiumType: UInt8?) {
        switch limit {
        case .fileTooLarge(let max) where premiumType != 2:
            // `+R2TzS` with maxSize = the effective limit and premiumMaxSize = TIER_2's.
            self.init(
                title: Self.upsellTitle,
                message: "The max file size is \(Self.upsellSize(max)). Upgrade to Discord Nitro to upload files up to \(Self.upsellSize(Self.nitroLimit)).",
                showsUpsell: true)
        case .fileTooLarge(let max):
            self.init(title: Self.tooPowerful, message: "Max file size is \(Self.size(max)) please.", showsUpsell: false)
        case .totalTooLarge(let max):
            self.init(title: Self.tooPowerful, message: "Total message size must be less than \(Self.size(max))", showsUpsell: false)
        case .emptyFile:
            self.init(title: "Upload Failed", message: "File cannot be empty", showsUpsell: false)
        case .tooManyFiles(let limit):
            self.init(title: "Too many uploads!", message: "You can only upload \(limit) files at a time!", showsUpsell: false)
        }
    }

    init(title: String, message: String, showsUpsell: Bool) {
        self.title = title; self.message = message; self.showsUpsell = showsUpsell
    }
}
