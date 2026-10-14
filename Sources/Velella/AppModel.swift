import Foundation
import Observation
import Security
import VelellaFFI
import VelellaLogin
import VelellaUI

/// Owns the session, its generation and the pending freshly captured token, and executes the
/// Keychain decisions `keychainAction` makes. The token is never observable state: it lives in
/// `@ObservationIgnored` storage only until READY or failure, and never enters `ShellState`.
@MainActor @Observable
final class AppModel {
    var shell: ShellState
    /// Shown as an alert over the main window; never contains a token.
    var notice: String?
    let isDemo: Bool

    @ObservationIgnored private var handle: SessionHandle?
    @ObservationIgnored private var generation: UInt64?
    @ObservationIgnored private var pending: (generation: UInt64, token: String)?

    // Sidebar subscriptions. Dropping a SidebarSubscription stops its deliveries on the Rust side.
    @ObservationIgnored private var guildsSub: SidebarSubscription?
    @ObservationIgnored private var dmsSub: SidebarSubscription?
    @ObservationIgnored private var channelsSub: SidebarSubscription?
    /// Guild of the live `channelsSub`; the only guild whose channel snapshots may be applied.
    @ObservationIgnored private var channelsGuild: UInt64?
    @ObservationIgnored private var guildNames: [UInt64: String] = [:]
    @ObservationIgnored private lazy var members = MembersController(model: self)
    /// The open conversation's messages; dropping it leaves the channel (and cancels its fetch).
    @ObservationIgnored private var messagesSub: MessageSubscription?
    /// Channel `messagesSub` opened from `shell.openTarget`: the only channel `send` posts to.
    @ObservationIgnored private var messagesChannel: UInt64?
    /// Bumped by every `openMessages`; only snapshots of the latest subscription are applied, so a
    /// late one from an earlier visit to the same channel (X, Y, back to X) is dropped too.
    @ObservationIgnored private var messagesOpen: UInt64 = 0

    init(demo: Bool) {
        isDemo = demo
        shell = demo ? .demo : .signedOut
        if !demo { restore() }
    }

    var canSignOut: Bool { !isDemo && shell.isSignedIn }

    /// Called by the login window, once, with the captured token.
    func loginCaptured(_ token: String) {
        guard !isDemo else { return }
        start(token: token, fresh: true)
    }

    func signOut() {
        guard !isDemo else { return }
        handle?.disconnect()
        handle = nil
        generation = nil
        pending = nil
        dropSubscriptions()
        shell = .signedOut
        notice = nil
        execute(keychainAction(.signedOut, currentGeneration: nil, freshCapture: false))
    }

    // MARK: Session

    private func restore() {
        let (saved, status) = Keychain.load()
        // A read error never deletes (the decision for `.loaded` is always `.none`).
        execute(keychainAction(.loaded(status: status), currentGeneration: generation, freshCapture: false))
        if let saved {
            start(token: saved.token, fresh: false)
        } else if status != errSecItemNotFound {
            notice = "Couldn't read your saved sign-in (error \(status)). Sign in again."
        }
    }

    private func start(token: String, fresh: Bool) {
        handle?.disconnect()
        dropSubscriptions()
        do {
            let handle = try VelellaFFI.connect(token: token, listener: Listener(model: self))
            self.handle = handle
            generation = handle.generation()
            pending = fresh ? (handle.generation(), token) : nil
            notice = nil
        } catch {
            endSession()
            notice = "That sign-in couldn't be used. Try signing in again."
        }
    }

    fileprivate func receive(_ event: Listener.Event) {
        FileHandle.standardError.write(Data("velella: \(event.name)\n".utf8))
        switch event {
        case .ready(let gen, let userID, let username):
            let fresh = pending?.generation == gen
            let action = keychainAction(.ready(generation: gen), currentGeneration: generation, freshCapture: fresh)
            guard gen == generation else { return }
            shell.currentUser = CurrentUser(
                displayName: username, username: username, avatar: .initials(String(username.prefix(1)).uppercased())
            )
            subscribeSidebar(generation: gen)
            let token = pending?.token
            pending = nil
            if action == .write, let token {
                execute(.write, token: token, account: String(userID))
            }
        case .authFailed(let gen):
            let action = keychainAction(.authFailed(generation: gen), currentGeneration: generation, freshCapture: false)
            guard gen == generation else { return }
            endSession()
            notice = "Discord didn't accept this sign-in. Sign in again."
            execute(action)
        case .closed(let gen):
            guard gen == generation else { return }
            endSession()
            notice = "Disconnected."
        case .restUnauthorized(let gen):
            // Non-destructive (plan Q3): the session and the saved sign-in stay; only REST is off.
            // A revoked token also fails IDENTIFY with 4004, which is `.authFailed` above.
            guard gen == generation else { return }
            notice = "Discord didn't accept this sign-in for loading or sending messages. Sign out and sign in again if this continues."
        case .resumed, .reconnecting:
            break
        }
    }

    private func endSession() {
        handle = nil
        generation = nil
        pending = nil
        dropSubscriptions()
        shell = .signedOut
    }

    // MARK: Sidebar

    private static let dmRailID = RailDM.directMessages.id

    /// On READY: guilds and DMs for the whole session (a repeated READY reuses them: the engine
    /// replaces its state in place and pushes fresh snapshots).
    private func subscribeSidebar(generation gen: UInt64) {
        guard let handle, guildsSub == nil else { return }
        shell.rail = [.dm(.directMessages)]
        guildsSub = handle.subscribeGuilds(listener: GuildsSnapshots(model: self, generation: gen))
        dmsSub = handle.subscribeDms(listener: DMSnapshots(model: self, generation: gen))
        if shell.selectedRailID == nil { shell.selectedRailID = Self.dmRailID }
    }

    private func dropSubscriptions() {
        guildsSub = nil
        dmsSub = nil
        channelsSub = nil
        channelsGuild = nil
        guildNames = [:]
        members.teardown()
        messagesSub = nil
        messagesChannel = nil
        shell.messages = []
        shell.messageLoad = .idle
    }

    /// Follows the shell's own selection binding (the window calls this when it changes).
    func syncSelection() {
        guard let handle, let generation else { return }
        guard let guild = shell.selectedRailID.flatMap({ UInt64($0) }) else {
            channelsSub = nil
            channelsGuild = nil
            shell.categories = []
            return
        }
        guard guild != channelsGuild else { return }
        channelsSub = nil
        channelsGuild = guild
        shell.serverName = guildNames[guild]
        shell.selectedChannelID = nil
        shell.categories = []
        channelsSub = handle.subscribeChannels(
            guildId: guild, listener: ChannelSnapshots(model: self, generation: generation)
        )
    }

    /// Runs after every change of the selected channel, DM or rail entry (the window calls it, after
    /// `syncSelection()` for a rail change). `shell.openTarget` is the conversation now open.
    func selectionChanged() {
        members.retarget(handle: handle, generation: generation) // L
        // M: the message history follows `shell.openTarget` here.
        openMessages(shell.openTarget.flatMap { UInt64($0) })
    }

    /// The composer's submit: to the channel this model opened, never to a selection read now.
    /// Core sends only if that is still its open channel.
    func send(_ text: String) {
        guard let handle, let channel = messagesChannel else { return }
        _ = handle.sendMessage(channelId: channel, content: text)
    }

    // MARK: Messages

    /// Opens `target`'s messages (nil: none) unless it is already open.
    private func openMessages(_ target: UInt64?) {
        guard !isDemo, target != messagesChannel else { return }
        messagesOpen &+= 1
        messagesSub = nil
        messagesChannel = nil
        shell.messages = []
        shell.messageLoad = .idle
        guard let target, let handle, let generation else { return }
        messagesChannel = target
        shell.messageLoad = .loading
        messagesSub = handle.subscribeMessages(
            channelId: target,
            listener: MessageSnapshots(model: self, generation: generation, open: messagesOpen)
        )
    }

    fileprivate func apply(messages snapshot: MessagesSnapshot, open: UInt64, generation gen: UInt64) {
        // A late snapshot of a dropped subscription must not fill another conversation.
        guard gen == generation, open == messagesOpen else { return }
        shell.messages = snapshot.rows.map { row in
            MessageItem.live(
                id: row.id, authorName: row.authorName, authorColor: row.authorColor,
                avatarURL: row.authorAvatarUrl, timestampMs: row.timestampMs, content: row.content,
                body: Self.body(row), edited: row.edited, images: row.images.map { ($0.url, $0.width, $0.height) },
                files: row.files, replyAuthor: row.reply?.authorName, replyContent: row.reply?.content,
                pending: row.pending, failure: row.failure
            )
        }
        shell.messageLoad = switch snapshot.load {
        case .loading: .loading
        case .loaded: .loaded
        case .failed(let text): .failed(text)
        }
    }

    // MARK: Message body (core's AST, field for field; `MessageBody` numbers links and spoilers)

    private static func body(_ row: VelellaFFI.MessageRow) -> MessageBody {
        MessageBody(
            blocks: row.body.map(block),
            mentions: row.mentions.map { mention in
                let kind: MentionKind = switch mention.kind {
                case .user: .user
                case .role: .role
                case .channel: .channel
                }
                return ResolvedMention(kind: kind, id: mention.id, name: mention.name, color: mention.color)
            },
            emoji: row.emoji.compactMap { e in URL(string: e.url).map { EmojiImage(id: e.id, animated: e.animated, url: $0) } }
        )
    }

    private static func block(_ block: MdBlock) -> MarkdownBlock {
        switch block {
        case .paragraph(let content): .paragraph(content.map(inline))
        case .heading(let level, let content): .heading(level: level, content.map(inline))
        case .subtext(let content): .subtext(content.map(inline))
        case .quote(let blocks): .quote(blocks.map(Self.block))
        case .list(let ordered, let start, let items): .list(ordered: ordered, start: start, items: items.map { $0.map(Self.block) })
        case .codeBlock(let language, let code): .codeBlock(language: language, code: code)
        }
    }

    private static func inline(_ inline: MdInline) -> MarkdownInline {
        switch inline {
        case .text(let text): .text(text)
        case .bold(let content): .bold(content.map(Self.inline))
        case .italic(let content): .italic(content.map(Self.inline))
        case .underline(let content): .underline(content.map(Self.inline))
        case .strike(let content): .strike(content.map(Self.inline))
        case .spoiler(let content): .spoiler(id: 0, content.map(Self.inline))
        case .code(let text): .code(text)
        case .link(let url, let host, let label, let masked):
            .link(id: 0, url: url, host: host, label: label.map(Self.inline), masked: masked)
        case .userMention(let id): .userMention(id)
        case .roleMention(let id): .roleMention(id)
        case .channelMention(let id): .channelMention(id)
        case .everyone: .everyone
        case .here: .here
        case .customEmoji(let id, let name, let animated): .customEmoji(id: id, name: name, animated: animated)
        case .timestamp(let unix, let style): .timestamp(unix: unix, style: timestampStyle(style))
        case .slashCommand(let name, let id): .slashCommand(name: name, id: id)
        case .guildNavigation(let guild, let target): .guildNavigation(guild: guild, target: navTarget(target))
        case .lineBreak: .lineBreak
        case .block(let block): .block(Self.block(block))
        }
    }

    private static func timestampStyle(_ style: MdTimestampStyle) -> TimestampStyle {
        switch style {
        case .shortTime: .shortTime
        case .mediumTime: .mediumTime
        case .shortDate: .shortDate
        case .longDate: .longDate
        case .longDateShortTime: .longDateShortTime
        case .fullDateShortTime: .fullDateShortTime
        case .shortDateShortTime: .shortDateShortTime
        case .shortDateMediumTime: .shortDateMediumTime
        case .relative: .relative
        }
    }

    private static func navTarget(_ target: MdNavTarget) -> NavTarget {
        switch target {
        case .customize: .customize
        case .browse: .browse
        case .guide: .guide
        case .linkedRoles(let role): .linkedRoles(role)
        }
    }

    fileprivate func apply(guilds entries: [SidebarEntry], generation gen: UInt64) {
        guard gen == generation else { return }
        var names: [UInt64: String] = [:]
        var rail: [RailItem] = []
        for entry in entries {
            switch entry {
            case .guild(let guild):
                names[guild.id] = guild.name
                rail.append(.guild(Self.railGuild(guild)))
            case .folder(let id, let name, let color, let guilds):
                guard let first = guilds.first else { continue }
                for guild in guilds { names[guild.id] = guild.name }
                rail.append(.folder(RailFolder(
                    id: "folder-\(id ?? first.id)", name: name,
                    color: color.map { RGB(rgb: $0) } ?? .defaultFolder,
                    children: guilds.map(Self.railGuild)
                )))
            }
        }
        guildNames = names
        shell.applyGuildRail(rail)
        // The selected guild vanished (GUILD_DELETE, or this account left it): back to DMs.
        if let selected = shell.selectedRailID.flatMap({ UInt64($0) }) {
            if let name = names[selected] { shell.serverName = name } else { shell.selectedRailID = Self.dmRailID }
        }
        syncSelection()
    }

    fileprivate func apply(channels categories: [SidebarCategory], guild: UInt64, generation gen: UInt64) {
        // Second check: a snapshot of a dropped subscription can still be in flight after the
        // selection moved to another guild; it must not overwrite that guild's channels or clear
        // its selected channel.
        guard gen == generation, guild == channelsGuild else { return }
        let mapped = categories.map { category in
            ChannelCategory(
                id: category.id.map { "cat-\($0)" } ?? "root", name: category.name,
                channels: category.channels.map(Self.channelItem)
            )
        }
        shell.categories = mapped
        let listed = mapped.flatMap(\.channels).flatMap { [$0] + $0.threads }
        if let selected = shell.selectedChannelID, !listed.contains(where: { $0.id == selected }) {
            shell.selectedChannelID = nil
        }
    }

    fileprivate func apply(dms: [SidebarDm], friendsPending: UInt32, generation gen: UInt64) {
        guard gen == generation else { return }
        shell.applyDMs(dms.map(Self.dmEntry), friendsPending: Int(friendsPending))
    }

    private static func avatar(url: String?, name: String) -> Avatar {
        url.flatMap { URL(string: $0) }.map(Avatar.url) ?? .initials(initials(of: name))
    }

    private static func railGuild(_ guild: SidebarGuild) -> RailGuild {
        RailGuild(
            id: String(guild.id), name: guild.name, icon: avatar(url: guild.iconUrl, name: guild.name),
            hasUnread: guild.unread, mentionCount: Int(guild.mentionCount), isMuted: guild.muted
        )
    }

    private static func channelItem(_ channel: SidebarChannel) -> ChannelItem {
        ChannelItem(
            id: String(channel.id), name: channel.name, kind: channelKind(channel.kind),
            isUnread: channel.unread, mentionCount: Int(channel.mentionCount),
            isMuted: channel.muted, isPrivate: channel.`private`,
            occupants: channel.occupants.map {
                VoiceOccupant(id: String($0.id), name: $0.name, avatar: avatar(url: $0.iconUrl, name: $0.name))
            },
            threads: channel.threads.map {
                ChannelItem(
                    id: String($0.id), name: $0.name, isUnread: $0.unread,
                    mentionCount: Int($0.mentionCount), isMuted: $0.muted
                )
            }
        )
    }

    private static func dmEntry(_ dm: SidebarDm) -> DMEntry {
        DMEntry(
            id: String(dm.id), name: dm.name, avatar: avatar(url: dm.iconUrl, name: dm.name),
            status: dm.status.map(presence), unread: dm.unread, mentionCount: Int(dm.mentionCount)
        )
    }

    private static func presence(_ status: SidebarStatus) -> PresenceStatus {
        switch status {
        case .online: .online
        case .idle: .idle
        case .doNotDisturb: .doNotDisturb
        case .offline: .offline
        }
    }

    private static func channelKind(_ kind: UInt8) -> ChannelKind {
        switch kind {
        case 5: .announcement
        case 15, 16: .forum
        case 2: .voice
        case 13: .stage
        default: .text
        }
    }

    // MARK: Keychain

    /// The only place that acts on a `KeychainAction`.
    private func execute(_ action: KeychainAction, token: String? = nil, account: String? = nil) {
        switch action {
        case .write:
            guard let token, let account else { return }
            let status = Keychain.save(token: token, account: account)
            if keychainAction(.saved(status: status), currentGeneration: generation, freshCapture: true) == .sessionOnly {
                // The session keeps running; nothing is written anywhere else.
                notice = "Signed in for this session only: couldn't save your sign-in (error \(status))."
            }
        case .delete:
            let status = Keychain.delete()
            if status != errSecSuccess { notice = "Couldn't remove your saved sign-in (error \(status))." }
        case .sessionOnly, .none:
            break // .sessionOnly is acted on above, where the save status is known.
        }
    }
}

/// Receives session callbacks on a `velella-rt` thread and hands them to the main queue in order.
private final class Listener: SessionListener {
    enum Event {
        case ready(UInt64, UInt64, String)
        case resumed(UInt64)
        case reconnecting(UInt64)
        case authFailed(UInt64)
        case closed(UInt64)
        case restUnauthorized(UInt64)

        /// Event name only: no username, id or token reaches stderr.
        var name: String {
            switch self {
            case .ready: "READY"
            case .resumed: "RESUMED"
            case .reconnecting: "RECONNECTING"
            case .authFailed: "AUTH_FAILED"
            case .closed: "CLOSED"
            case .restUnauthorized: "REST_UNAUTHORIZED"
            }
        }
    }

    let model: AppModel
    init(model: AppModel) { self.model = model }

    private func send(_ event: Event) {
        DispatchQueue.main.async { [model] in model.receive(event) }
    }

    func onReady(generation: UInt64, userId: UInt64, username: String) { send(.ready(generation, userId, username)) }
    func onResumed(generation: UInt64) { send(.resumed(generation)) }
    func onReconnecting(generation: UInt64) { send(.reconnecting(generation)) }
    func onAuthFailed(generation: UInt64) { send(.authFailed(generation)) }
    func onClosed(generation: UInt64) { send(.closed(generation)) }
    func onRestUnauthorized(generation: UInt64) { send(.restUnauthorized(generation)) }
}

/// Sidebar callbacks arrive on `velella-rt` threads; each goes to the main queue in order and is
/// tagged with the session generation the subscription was made for (checked in AppModel).
private final class GuildsSnapshots: GuildsListener {
    let model: AppModel
    let generation: UInt64
    init(model: AppModel, generation: UInt64) { self.model = model; self.generation = generation }

    func onGuilds(entries: [SidebarEntry]) {
        DispatchQueue.main.async { [model, generation] in model.apply(guilds: entries, generation: generation) }
    }
}

private final class ChannelSnapshots: ChannelsListener {
    let model: AppModel
    let generation: UInt64
    init(model: AppModel, generation: UInt64) { self.model = model; self.generation = generation }

    func onChannels(guildId: UInt64, categories: [SidebarCategory]) {
        DispatchQueue.main.async { [model, generation] in
            model.apply(channels: categories, guild: guildId, generation: generation)
        }
    }
}

private final class MessageSnapshots: MessagesListener {
    let model: AppModel
    let generation: UInt64
    let open: UInt64
    init(model: AppModel, generation: UInt64, open: UInt64) {
        self.model = model; self.generation = generation; self.open = open
    }

    func onMessages(channelId: UInt64, snapshot: MessagesSnapshot) {
        DispatchQueue.main.async { [model, generation, open] in
            model.apply(messages: snapshot, open: open, generation: generation)
        }
    }
}

private final class DMSnapshots: DmsListener {
    let model: AppModel
    let generation: UInt64
    init(model: AppModel, generation: UInt64) { self.model = model; self.generation = generation }

    func onDms(dms: [SidebarDm], friendsPending: UInt32) {
        DispatchQueue.main.async { [model, generation] in
            model.apply(dms: dms, friendsPending: friendsPending, generation: generation)
        }
    }
}
