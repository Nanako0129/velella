import AppKit
import Foundation
import Observation
import Security
import VelellaFFI
import VelellaLogin
import VelellaTransport
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
    /// Account of the saved sign-in (`Keychain.Saved.account`, or the one just written); nil when
    /// nothing is saved. A refreshed token is written only for this account.
    @ObservationIgnored private var storedAccount: String?
    /// The READY account of the current session (its heartbeat session is forgotten at Sign Out).
    @ObservationIgnored private var sessionUser: UInt64?
    /// Focus and input observers (the web IdleStore's inputs), installed once.
    @ObservationIgnored private var observers: [Any] = []
    @ObservationIgnored private var lastActivityNote = Date.distantPast

    // Sidebar subscriptions. Dropping a SidebarSubscription stops its deliveries on the Rust side.
    @ObservationIgnored private var guildsSub: SidebarSubscription?
    @ObservationIgnored private var dmsSub: SidebarSubscription?
    @ObservationIgnored private var channelsSub: SidebarSubscription?
    /// The user's own state (`premium_type` now: the composer's message-length limit); delivered at
    /// once, then after every USER_UPDATE or settings change that alters it.
    @ObservationIgnored private var selfStateSub: SidebarSubscription?
    /// Guild of the live `channelsSub`; the only guild whose channel snapshots may be applied.
    @ObservationIgnored private var channelsGuild: UInt64?
    @ObservationIgnored private var guildNames: [UInt64: String] = [:]
    @ObservationIgnored private lazy var members = MembersController(model: self)
    /// The open conversation's messages; dropping it leaves the channel (and cancels its fetch).
    @ObservationIgnored private var messagesSub: MessageSubscription?
    /// Channel `messagesSub` opened from `shell.openTarget`: the only channel `send` posts to.
    @ObservationIgnored private var messagesChannel: UInt64?
    /// Guild of `messagesChannel` when it was opened (nil for a DM): the message menu's link scope.
    @ObservationIgnored private var messagesGuild: UInt64?
    /// Bumped by every `openMessages`; only snapshots of the latest subscription are applied, so a
    /// late one from an earlier visit to the same channel (X, Y, back to X) is dropped too.
    @ObservationIgnored private var messagesOpen: UInt64 = 0
    /// The sends with files this run started (composer plan CUA): which temp files go when a send's
    /// row leaves, and which cancelled rows are ours to put back in the composer.
    @ObservationIgnored private var pendingSends = PendingSends()
    /// ATTACH_FILES' bit number in core's permission table (menu plan F1: the one list bits are tested against).
    @ObservationIgnored private lazy var attachFilesBit: UInt8? = permissionTable().first { $0.name == "ATTACH_FILES" }?.bit

    init(demo: Bool) {
        isDemo = demo
        Self.configureLaunchIdentity(demo: demo)
        shell = demo ? .demo : .signedOut
        if !demo {
            // CU-1: files a previous run synthesized and left behind.
            AttachmentFiles.sweep()
            observeActivity()
            restore()
        }
    }

    /// Before anything reads the user agent (the login window) or connects (menu plan ID): this
    /// Mac's Safari version, the system languages, and the identity store's directory (none in
    /// demo mode, which never connects and touches no file).
    private static func configureLaunchIdentity(demo: Bool) {
        let safari = Bundle(path: "/Applications/Safari.app")?
            .object(forInfoDictionaryKey: "CFBundleShortVersionString") as? String
        let store = demo ? nil : FileManager.default
            .urls(for: .applicationSupportDirectory, in: .userDomainMask).first?
            .appendingPathComponent(Keychain.service, isDirectory: true).path
        configureIdentity(safariVersion: safari, preferredLanguages: Locale.preferredLanguages, storeDir: store)
    }

    /// Frontmost or not, and clicks, scrolls and key presses in velella's windows (throttled to one
    /// report per 0.5 s, as the web client throttles them).
    private func observeActivity() {
        let center = NotificationCenter.default
        for (name, focused) in [
            (NSApplication.didBecomeActiveNotification, true), (NSApplication.didResignActiveNotification, false),
        ] {
            observers.append(center.addObserver(forName: name, object: nil, queue: .main) { [weak self] _ in
                MainActor.assumeIsolated { self?.handle?.setAppFocused(focused: focused) }
            })
        }
        if let monitor = NSEvent.addLocalMonitorForEvents(
            matching: [.leftMouseUp, .rightMouseUp, .otherMouseUp, .scrollWheel, .keyDown],
            handler: { [weak self] event in
                MainActor.assumeIsolated { self?.noteActivity() }
                return event
            }
        ) {
            observers.append(monitor)
        }
    }

    private func noteActivity() {
        let now = Date()
        guard now.timeIntervalSince(lastActivityNote) >= 0.5 else { return }
        lastActivityNote = now
        handle?.noteUserActivity()
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
        if let sessionUser { forgetAccountSession(userId: sessionUser) }
        sessionUser = nil
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
            storedAccount = saved.account.isEmpty ? nil : saved.account
            start(token: saved.token, fresh: false)
        } else if status != errSecItemNotFound {
            notice = "Couldn't read your saved sign-in (error \(status)). Sign in again."
        }
    }

    private func start(token: String, fresh: Bool) {
        handle?.disconnect()
        dropSubscriptions()
        do {
            let listener = Listener(model: self)
            // A token from the Keychain names its account; a fresh capture's account is unknown
            // until READY, so no per-account identity value goes out for it before then.
            let handle = try VelellaFFI.connectAccount(
                token: token, savedAccount: fresh ? nil : storedAccount.flatMap { UInt64($0) },
                transport: URLSessionTransport.shared, listener: listener, identityListener: listener
            )
            self.handle = handle
            generation = handle.generation()
            selfStateSub = handle.subscribeSelfState(listener: SelfStateSnapshots(model: self, generation: handle.generation()))
            pending = fresh ? (handle.generation(), token) : nil
            notice = nil
            handle.setAppFocused(focused: NSApplication.shared.isActive)
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
            sessionUser = userID
            let token = pending?.token
            pending = nil
            if action == .write, let token {
                execute(.write, token: token, account: String(userID))
            }
        case .tokenRefreshed(let gen, let userID, let refreshed):
            // Taken here, once; never stored anywhere but the Keychain item or `pending`.
            guard let token = refreshed.take() else { return }
            let action = tokenRefreshAction(
                generation: gen, userID: String(userID), currentGeneration: generation,
                pendingGeneration: pending?.generation, storedAccount: storedAccount
            )
            if let status = applyTokenRefresh(action, token: token, pending: &pending, keychain: Keychain.Updater()) {
                notice = "Couldn't save the sign-in Discord refreshed (error \(status)). If velella asks you to sign in next time, sign in again."
            }
        case .identityNotice(let gen, let kind):
            guard gen == generation else { return }
            notice = switch kind {
            case .unsupportedCapabilities:
                "Discord's web client changed how it identifies itself. velella keeps using the last version it supports; an update of velella is needed."
            case .identityFetchFailed:
                "Couldn't check Discord's current web client version. velella is using the values it saved earlier."
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
        sessionUser = nil
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
        selfStateSub = nil
        channelsSub = nil
        channelsGuild = nil
        guildNames = [:]
        members.teardown()
        discardAttachmentFiles()
        messagesSub = nil
        messagesChannel = nil
        messagesGuild = nil
        shell.messages = []
        shell.conversation = nil
        shell.messageLoad = .idle
        shell.older = .idle
        shell.typing = []
        shell.attachPermission = .unknown
        // A reply target belongs to the conversation it was picked in.
        shell.reply = nil
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
        // An obfuscated channel or DM ("No Access") is not navigable (I-5): the selection is
        // dropped, which calls this again with nothing open.
        if let handle, shell.dropBlockedSelection({ UInt64($0).map(handle.isObfuscated) ?? false }) { return }
        noteActivity() // a channel switch is input for the idle rule
        members.retarget(handle: handle, generation: generation) // L
        // M: the message history follows `shell.openTarget` here.
        openMessages(shell.openTarget.flatMap { UInt64($0) })
    }

    /// The composer's submit: to the channel this model opened, never to a selection read now.
    /// Core sends only if that is still its open channel.
    /// `target` is the reply bar's target; one picked in another channel is dropped and the text
    /// goes out as a plain message (`ReplyTarget.replyID`).
    func send(_ text: String, target: ReplyTarget? = nil) {
        guard let handle, let channel = messagesChannel else { return }
        _ = handle.sendMessage(channelId: channel, content: text, replyTo: ReplyTarget.replyID(target, sendingTo: channel))
    }

    // MARK: Attachments (composer plan CUA)

    /// What the composer's attachments call: core's CU surface and `AttachmentFiles` (CU-1, CU-6).
    @ObservationIgnored lazy var attachmentServices = AttachmentServices(
        identity: { url in
            AttachmentFiles.identity(of: url).map {
                QueuedFileIdentity(path: $0.path(), device: $0.device(), inode: $0.inode(), size: $0.size(), mtimeNs: $0.mtimeNs())
            }
        },
        write: { try AttachmentFiles.write($0, named: $1) },
        remove: { AttachmentFiles.remove($0) },
        limits: { [unowned self] conversation, sizes, stage in
            guard let handle, let channel = UInt64(conversation) else { return nil }
            return handle.attachmentLimits(channelId: channel, sizes: sizes, at: stage == .add ? .add : .send).map(Self.limit)
        },
        conversion: { [unowned self] kind in
            guard let handle else { return .off }
            switch handle.imageConversionConfig(kind: kind == .heic ? .heic : .jxr) {
            case .enabled(let quality, let maxFileSizeBytes): return .enabled(quality: quality, maxFileSizeBytes: maxFileSizeBytes)
            case .unknown, .disabled: return .off
            }
        },
        send: { [unowned self] request in sendAttachments(request) },
        cancel: { [unowned self] id in _ = UInt64(id).map { handle?.cancelUpload(nonce: $0) } },
        retry: { [unowned self] id in _ = UInt64(id).map { handle?.retryUpload(nonce: $0) } },
        discard: { [unowned self] id in
            // The UI deletes the files (it has the row's descriptors); here the send is forgotten.
            _ = UInt64(id).map { handle?.discardUpload(nonce: $0) }
            _ = pendingSends.take(id: id)
        }
    )

    /// Files with the draft's text (or none), or Shift-drop's instant send, to `request.conversation`.
    /// Core checks the permission and the limits again and refuses what it must.
    private func sendAttachments(_ request: AttachmentSendRequest) -> AttachmentSendOutcome {
        guard let handle, let channel = UInt64(request.conversation) else { return .refused }
        let descriptors = request.files.map { file in
            AttachmentDescriptor(
                file: UploadFile(
                    path: file.file.path, device: file.file.device, inode: file.file.inode, size: file.file.size, mtimeNs: file.file.mtimeNs),
                filename: file.filename, description: file.altText, spoiler: file.spoiler, contentType: file.contentType,
                originalMd5: file.originalMD5)
        }
        let result = handle.sendWithAttachments(
            channelId: channel, content: request.text, attachments: descriptors,
            replyTo: ReplyTarget.replyID(request.reply, sendingTo: channel), instant: request.instant)
        switch result {
        case .started(let nonce):
            pendingSends.begin(id: String(nonce), conversation: request.conversation, paths: request.files.map(\.file.path))
            return .started
        case .limit(let error): return .limit(Self.limit(error))
        case .noFiles, .notAllowed, .notReady, .notOpen: return .refused
        }
    }

    private static func limit(_ error: AttachmentLimitError) -> AttachmentLimit {
        switch error {
        case .tooManyFiles(let limit): .tooManyFiles(limit: Int(limit))
        case .emptyFile: .emptyFile
        case .fileTooLarge(let limit): .fileTooLarge(limit: limit)
        case .totalTooLarge(let limit): .totalTooLarge(limit: limit)
        }
    }

    /// A pending send's row as the UI draws it: core's descriptors, progress and outcome, field for field.
    private static func upload(_ upload: PendingUpload) -> PendingUploadItem {
        let state: PendingUploadItem.State = switch upload.outcome {
        case .sending: .sending
        case .failed: .failed
        case .cancelled: .cancelled
        }
        return PendingUploadItem(
            files: upload.files.map { file in
                let descriptor = file.descriptor, identity = descriptor.file()
                return PendingUploadItem.File(
                    attachment: QueuedAttachment(
                        file: QueuedFileIdentity(
                            path: identity.path(), device: identity.device(), inode: identity.inode(), size: identity.size(),
                            mtimeNs: identity.mtimeNs()),
                        filename: descriptor.filename(), altText: descriptor.altText(), spoiler: descriptor.spoiler(),
                        contentType: descriptor.contentType(), originalMD5: descriptor.originalMd5()),
                    sent: file.sent)
            },
            replyTo: upload.replyTo, instant: upload.instant, state: state)
    }

    /// What the open conversation lets this user attach: ATTACH_FILES, always in a DM, Unknown until
    /// core knows (which hides the upload entries).
    private func refreshAttachPermission() {
        var permission = AttachPermission.unknown
        if let handle, let channel = messagesChannel {
            switch handle.channelPermissions(channelId: channel) {
            case .private: permission = .allowed
            case .bits(let bits): permission = attachFilesBit.map { AttachPermission(bits: bits, attachBit: $0) } ?? .unknown
            case .unknown: break
            }
        }
        if shell.attachPermission != permission { shell.attachPermission = permission }
    }

    /// Sign-out, a new session: every temp file still held by a queue or a pending send goes (CU-1).
    private func discardAttachmentFiles() {
        for file in shell.takeAllAttachments() { AttachmentFiles.remove(file.url) }
        for path in pendingSends.takeAll() { AttachmentFiles.remove(URL(fileURLWithPath: path)) }
    }

    /// Core's one-line reply preview of a buffered message of the open conversation (spoilers as
    /// the placeholder), flattened now; nil when it is not buffered.
    func replyPreview(_ messageID: String) -> String? {
        UInt64(messageID).flatMap { messagesSub?.preview(messageId: $0) }
    }

    // MARK: Messages

    /// The user reached the top: asks core for the page before the oldest shown. Core ignores it
    /// unless that is useful (first page in, no older GET running, start not reached).
    func loadOlder() {
        messagesSub?.loadOlder()
    }

    /// Opens `target`'s messages (nil: none) unless it is already open.
    private func openMessages(_ target: UInt64?) {
        guard !isDemo, target != messagesChannel else { return }
        messagesOpen &+= 1
        messagesSub = nil
        messagesChannel = nil
        messagesGuild = nil
        shell.messages = []
        shell.conversation = nil
        shell.messageLoad = .idle
        shell.older = .idle
        shell.typing = []
        // A reply target belongs to the conversation it was picked in.
        shell.reply = nil
        guard let target, let handle, let generation else {
            shell.attachPermission = .unknown
            return
        }
        messagesChannel = target
        // `syncSelection` ran first for a rail change, so this is the open target's guild; nil in
        // DM mode, where the rail entry is not a guild.
        messagesGuild = shell.isDMMode ? nil : channelsGuild
        shell.messageLoad = .loading
        refreshAttachPermission()
        messagesSub = handle.subscribeMessages(
            channelId: target,
            listener: MessageSnapshots(model: self, generation: generation, open: messagesOpen)
        )
    }

    fileprivate func apply(messages snapshot: MessagesSnapshot, open: UInt64, generation gen: UInt64) {
        // A late snapshot of a dropped subscription must not fill another conversation.
        guard gen == generation, open == messagesOpen else { return }
        var items = snapshot.rows.map { row in
            MessageItem.live(
                id: row.id, authorId: row.authorId, authorName: row.authorName, authorColor: row.authorColor,
                avatarURL: row.authorAvatarUrl, timestampMs: row.timestampMs, content: row.content,
                body: Self.body(row), edited: row.edited, images: row.images.map { ($0.url, $0.width, $0.height) },
                files: row.files, replyAuthor: row.reply?.authorName, replyContent: row.reply?.content,
                pending: row.pending, failure: row.failure,
                reactions: row.reactions.map { ($0.name, $0.url, $0.count, $0.me) },
                embeds: row.embeds.map { Self.embed($0, row: row) }, imageSpoilers: row.images.map(\.spoiler),
                upload: row.upload.map(Self.upload)
            )
        }
        // Cancelled sends of ours go back to the composer and their rows are discarded; the files of
        // sends whose row left (confirmed) are deleted (D14a, CU-1).
        if let channel = messagesChannel {
            let result = shell.reconcileUploads(conversation: String(channel), rows: items, sends: &pendingSends)
            result.discard.forEach { id in _ = UInt64(id).map { handle?.discardUpload(nonce: $0) } }
            result.deletePaths.forEach { AttachmentFiles.remove(URL(fileURLWithPath: $0)) }
            items.removeAll { $0.upload?.state == .cancelled }
        }
        shell.messages = items
        refreshAttachPermission()
        // With the rows, never apart from them: the menu builds a row's link from these.
        shell.conversation = ConversationIDs(channel: messagesChannel, guild: messagesGuild)
        shell.typing = snapshot.typing
        shell.messageLoad = switch snapshot.load {
        case .loading: .loading
        case .loaded: .loaded
        case .failed(let text): .failed(text)
        }
        shell.older = switch snapshot.older {
        case .idle: .idle
        case .loading: .loading
        case .reachedStart: .reachedStart
        case .failed(let text): .failed(text)
        }
    }

    // MARK: Message body (core's AST, field for field; `MessageBody` numbers links and spoilers)

    private static func body(_ row: VelellaFFI.MessageRow) -> MessageBody {
        body(row.body, row: row)
    }

    /// `blocks` with the row's resolved mentions and emoji images (the body's and its embeds').
    private static func body(_ blocks: [MdBlock], row: VelellaFFI.MessageRow) -> MessageBody {
        MessageBody(
            blocks: blocks.map(block),
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

    // MARK: Embeds (wave 3 E; field for field, URLs as core checked them)

    private static func embed(_ e: MessageEmbed, row: VelellaFFI.MessageRow) -> EmbedItem {
        func link(_ l: MessageEmbedLink?) -> EmbedItem.Link? { l.map { EmbedItem.Link(url: $0.url, host: $0.host) } }
        func image(_ i: MessageEmbedImage?) -> EmbedItem.Image? {
            i.flatMap { i in URL(string: i.url).map { EmbedItem.Image(url: $0, width: Double(i.width), height: Double(i.height), alt: i.description) } }
        }
        return EmbedItem(
            title: e.title, url: link(e.url), description: body(e.description, row: row), color: e.color,
            timestampMs: e.timestampMs, authorName: e.authorName, authorURL: link(e.authorUrl),
            authorIcon: e.authorIconUrl.flatMap { URL(string: $0) }, footerText: e.footerText,
            footerIcon: e.footerIconUrl.flatMap { URL(string: $0) },
            fields: e.fields.map { EmbedItem.Field(name: $0.name, value: body($0.value, row: row), inline: $0.inline) },
            providerName: e.providerName, providerURL: link(e.providerUrl), thumbnail: image(e.thumbnail), image: image(e.image),
            isVideo: e.isVideo, spoiler: e.spoiler
        )
    }

    private static func block(_ block: MdBlock) -> MarkdownBlock {
        switch block {
        case .paragraph(let content): .paragraph(content.map(inline))
        case .heading(let level, let content): .heading(level: level, content.map(inline))
        case .subtext(let content): .subtext(content.map(inline))
        case .quote(let blocks): .quote(blocks.map(Self.block))
        case .list(let ordered, let start, let items): .list(ordered: ordered, start: start, items: items.map { $0.map(Self.block) })
        case .codeBlock(let language, let code, let header, let highlight):
            .codeBlock(language: language, code: code, header: header, highlight: highlight.map { h in
                CodeHighlight(lines: h.lines.map { line in
                    line.map { CodeSpan(text: $0.text, rgb: $0.rgb, bold: $0.bold, italic: $0.italic, underline: $0.underline) }
                })
            })
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

    fileprivate func apply(selfState state: SelfState, generation gen: UInt64) {
        guard gen == generation else { return }
        shell.premiumType = state.premiumType
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
        // A channel obfuscated while open stops being navigable.
        if let handle { shell.dropBlockedSelection { UInt64($0).map(handle.isObfuscated) ?? false } }
    }

    fileprivate func apply(dms: [SidebarDm], friendsPending: UInt32, generation gen: UInt64) {
        guard gen == generation else { return }
        shell.applyDMs(dms.map(Self.dmEntry), friendsPending: Int(friendsPending))
        if let handle { shell.dropBlockedSelection { UInt64($0).map(handle.isObfuscated) ?? false } }
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
            storedAccount = status == errSecSuccess ? account : nil
            if keychainAction(.saved(status: status), currentGeneration: generation, freshCapture: true) == .sessionOnly {
                // The session keeps running; nothing is written anywhere else.
                notice = "Signed in for this session only: couldn't save your sign-in (error \(status))."
            }
        case .delete:
            let status = Keychain.delete()
            if status == errSecSuccess { storedAccount = nil }
            if status != errSecSuccess { notice = "Couldn't remove your saved sign-in (error \(status))." }
        case .sessionOnly, .none:
            break // .sessionOnly is acted on above, where the save status is known.
        }
    }
}

/// The refreshed token's object never prints its value, whichever way it is described.
extension RefreshedToken: CustomStringConvertible, CustomDebugStringConvertible, CustomReflectable {
    public var description: String { "RefreshedToken([REDACTED])" }
    public var debugDescription: String { description }
    public var customMirror: Mirror { Mirror(self, children: [:]) }
}

/// Receives session and identity callbacks on a `velella-rt` thread and hands them to the main
/// queue in order (core emits a refresh after its generation's READY).
private final class Listener: SessionListener, IdentityListener {
    enum Event {
        case ready(UInt64, UInt64, String)
        case resumed(UInt64)
        case reconnecting(UInt64)
        case authFailed(UInt64)
        case closed(UInt64)
        case restUnauthorized(UInt64)
        case tokenRefreshed(UInt64, UInt64, RefreshedToken)
        case identityNotice(UInt64, IdentityNotice)

        /// Event name only: no username, id or token reaches stderr.
        var name: String {
            switch self {
            case .ready: "READY"
            case .resumed: "RESUMED"
            case .reconnecting: "RECONNECTING"
            case .authFailed: "AUTH_FAILED"
            case .closed: "CLOSED"
            case .restUnauthorized: "REST_UNAUTHORIZED"
            case .tokenRefreshed: "TOKEN_REFRESHED"
            case .identityNotice: "IDENTITY_NOTICE"
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
    func onTokenRefreshed(generation: UInt64, userId: UInt64, token: RefreshedToken) {
        send(.tokenRefreshed(generation, userId, token))
    }
    func onNotice(generation: UInt64, notice: IdentityNotice) { send(.identityNotice(generation, notice)) }
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

private final class SelfStateSnapshots: SelfStateListener {
    let model: AppModel
    let generation: UInt64
    init(model: AppModel, generation: UInt64) { self.model = model; self.generation = generation }

    func onSelfState(state: SelfState) {
        DispatchQueue.main.async { [model, generation] in model.apply(selfState: state, generation: generation) }
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
