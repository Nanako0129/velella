import AppKit
import SwiftUI

private typealias T = DesignTokens
private typealias M = DesignTokens.Message

/// Message column: message rows, their load state and the composer. Fresh code (Swiftcord legacy
/// has no matching view; its message list is a separate, DiscordKit-bound subsystem). The body
/// renders through `MarkdownView`, built only from core's AST; every other payload string (names,
/// reply previews, file names, fixed failure texts) goes through `Text(verbatim:)` (plan M, F5).
struct MessageArea: View {
    let header: ChannelHeader?
    /// The open conversation's id: seeds the loading skeleton.
    var conversationID = ""
    var isSignedIn = true
    let messages: [MessageItem]
    var load: MessageLoad = .idle
    @Binding var reply: ReplyPreview?
    @Binding var draft: String
    /// Receives the draft and the reply's target (nil without a reply).
    var onSend: ((String, ReplyTarget?) -> Void)?
    /// The conversation `messages` were loaded for (`ShellState.conversation`); the message menu
    /// builds its links from it.
    var conversation: ConversationIDs?
    /// Core's reply preview line for a message id (the app's per-message preview call); nil
    /// (fixtures) leaves the reply bar's line empty.
    var replyPreview: ((String) -> String?)?
    /// Older-history state and the request for the next page (wave 3 P); see `OlderMessages.swift`.
    var older: OlderState = .idle
    var onLoadOlder: (() -> Void)?
    /// Who is typing in this conversation, shown under the composer.
    var typing: [String] = []
    @State private var columnWidth: CGFloat = 0
    /// The topmost message row (or divider) in view, kept by SwiftUI as the list scrolls. Binding it
    /// makes the scroll view hold that row at its place when older rows are prepended.
    @State private var topID: String?
    /// The open message menu, if any (selection plan C1).
    @State private var menu = MessageMenuPresenter()
    /// The selection across the column's messages (selection plan N2).
    @State private var selection = SelectionCoordinator()
    @Environment(\.discordLocale) private var discordLocale

    var body: some View {
        Group { content }
            // N2 Model: the coordinator follows the rows in display order, so a selection reaches rows no view shows.
            .onChange(of: messages, initial: true) {
                selection.locale = discordLocale
                selection.setRows(MessageGrouping.entries(messages).compactMap { entry in
                    if case .row(let message, let startsGroup, _) = entry { SelectionRow(message: message, startsGroup: startsGroup) } else { nil }
                })
            }
            // C1 Dismissal: the message left the rows (deleted, a switch or sign-out emptied them),
            // its body or delivery changed, or the conversation did.
            .onChange(of: messages) { menu.revalidate(messages: messages, conversation: conversation) }
            .onChange(of: conversation) { menu.revalidate(messages: messages, conversation: conversation) }
            .onDisappear {
                menu.dismiss()
                selection.clear()
            }
    }

    /// Opens the menu for `message` (C1 Trigger); link and selection come with W1.
    private func openMenu(_ message: MessageItem, at point: NSPoint, in window: NSWindow?) {
        let context = MessageMenuContext(message: message, conversation: conversation)
        // Core flattens the preview now, once, and only when Reply can show (C1 Reply).
        let preview = context.delivery == .sent ? replyPreview?(message.id) : nil
        menu.present(
            context, body: message.body, conversation: conversation, replyPreview: preview, at: point, from: window
        ) { [reply = $reply] in reply.wrappedValue = $0 }
    }

    @ViewBuilder
    private var content: some View {
        if header != nil {
            Group {
                // Until the first page arrives (a switch empties the rows at once), as in v2.
                if load == .loading && messages.isEmpty {
                    MessageSkeleton(seed: conversationID)
                } else {
                    ScrollView {
                        // Rows carry their own 16 pt inset, padding and group spacing (T1).
                        VStack(alignment: .leading, spacing: 0) {
                            // Outside the scroll target layout: a position kept across a prepend is
                            // always a message row's, never this fixed-height header's.
                            LoadStatus(load: load)
                            OlderRow(older: older, onRetry: onLoadOlder)
                            LazyVStack(alignment: .leading, spacing: 0) {
                                let entries = MessageGrouping.entries(messages)
                                ForEach(entries) { entry in
                                    switch entry {
                                    case .divider(_, let date): DateDivider(date: date)
                                    case .row(let message, let startsGroup, let afterDivider):
                                        MessageRow(message: message, startsGroup: startsGroup, afterDivider: afterDivider) { point, window in
                                            openMenu(message, at: point, in: window)
                                        }
                                            .onAppear { if entry.id == entries.first?.id, older == .idle { onLoadOlder?() } }
                                    }
                                }
                            }
                            .scrollTargetLayout()
                        }
                        .padding(.bottom, T.Spacing.lg)
                        .background(OverlayScrollerEnforcer())
                        // Tooltips (v2) stay inside the column: its width, from the top of the content.
                        .coordinateSpace(.named(TooltipSpace.name))
                        .onGeometryChange(for: CGFloat.self, of: { $0.size.width }) { columnWidth = $0 }
                        .environment(\.tooltipContainer, CGRect(x: 0, y: 0, width: columnWidth, height: 1e7))
                        .environment(\.selectionColumn, CoordinatorBox(coordinator: selection))
                    }
                    .background { SelectionResponder(coordinator: selection) }
                    .defaultScrollAnchor(.bottom)
                    .scrollPosition(id: $topID, anchor: .top)
                    // Messages pass under the toolbar through a glass band with a hard lower edge, on a
                    // darker pane than the translucent sidebar (Swiftcord v2, user's captures 2026-10-13).
                    .scrollEdgeEffectStyle(.hard, for: .top)
                }
            }
            // Another conversation starts at its bottom, not at a row of the last one.
            .onChange(of: conversationID) { topID = nil }
            .safeAreaInset(edge: .bottom) {
                VStack(spacing: 0) {
                    Composer(reply: $reply, draft: $draft, onSend: onSend)
                    TypingIndicator(names: typing)
                        .padding(.horizontal, T.Spacing.lg + T.Spacing.md)
                        // Up into the composer's own bottom gutter, which already sits under the card.
                        .padding(.top, -T.Spacing.md)
                        .padding(.bottom, T.Spacing.xs)
                }
            }
            // Under the toolbar too, so the toolbar band carries the pane's tint (a band of bare
            // window material over the messages read as a separate, lighter strip).
            .background(T.Palette.messagePane)
        } else if isSignedIn {
            ContentUnavailableView("No channel selected", systemImage: "number", description: Text("Pick a channel on the left."))
        } else {
            ContentUnavailableView {
                Label("Not signed in", systemImage: "person.crop.circle.badge.questionmark")
            } description: {
                Text("Sign in to read and send messages.")
            }
        }
    }
}

/// Above the rows: the fixed failure text when a load failed.
private struct LoadStatus: View {
    let load: MessageLoad

    var body: some View {
        if case .failed(let text) = load {
            Label { Text(verbatim: text) } icon: { Image(systemName: "exclamationmark.triangle.fill") }
                .padding(.horizontal, T.Spacing.lg)
                .font(T.Typography.messageBody)
                .foregroundStyle(.secondary)
        }
    }
}

/// What the message column lists: rows, with a date divider before the first row of a new local day.
enum MessageEntry: Identifiable, Equatable {
    case divider(id: String, Date)
    /// `afterDivider`: the row follows a divider, which already spaces it.
    case row(MessageItem, startsGroup: Bool, afterDivider: Bool)

    var id: String {
        switch self {
        case .divider(let id, _): id
        case .row(let message, _, _): message.id
        }
    }
}

enum MessageGrouping {
    /// A row starts a new group (avatar and header) when the previous row is from another author,
    /// is more than `M.groupWindow` older, sits on another local day, or is a reply (its reply line
    /// belongs above a header). `calendar` decides the day; only tests pass another.
    static func entries(_ messages: [MessageItem], calendar: Calendar = .current) -> [MessageEntry] {
        var out: [MessageEntry] = []
        var previous: MessageItem?
        for message in messages {
            let newDay = previous.map { !calendar.isDate($0.timestamp, inSameDayAs: message.timestamp) } ?? false
            if newDay { out.append(.divider(id: "divider-" + message.id, message.timestamp)) }
            let startsGroup = newDay || message.reply != nil || previous.map {
                $0.authorId != message.authorId || message.timestamp.timeIntervalSince($0.timestamp) > M.groupWindow
            } ?? true
            out.append(.row(message, startsGroup: startsGroup, afterDivider: newDay))
            previous = message
        }
        return out
    }
}

/// A hairline either side of the long date (`D` style, the shell locale). Discord draws one
/// line under the label; the label sits on the chat background, which here is translucent, so the
/// line stops at the label instead.
struct DateDivider: View {
    let date: Date
    @Environment(\.discordLocale) private var discordLocale

    var body: some View {
        let label = DiscordTime.format(date: date, style: .longDate, locale: discordLocale, now: .now)
        HStack(spacing: 0) {
            line
            Text(verbatim: label)
                .font(T.Typography.dividerLabel)
                .foregroundStyle(.secondary)
                .frame(height: M.dividerLabelLineHeight)
                .padding(.horizontal, M.dividerLabelPaddingH)
                .padding(.vertical, M.dividerLabelPaddingV)
            line
        }
        .padding(.horizontal, M.avatarLeading)
        .padding(.top, M.dividerAbove)
        .padding(.bottom, M.dividerBelow)
        .accessibilityElement(children: .ignore)
        .accessibilityLabel(Text(verbatim: label))
        .accessibilityAddTraits(.isHeader)
    }

    private var line: some View {
        Rectangle().fill(Color(nsColor: .separatorColor)).frame(height: M.dividerLineWidth)
    }
}

/// One message (T1): the text column starts at 72; a group's first row adds the avatar at 16 and
/// the header, a follow-up shows its time in the 56 pt gutter on hover.
struct MessageRow: View {
    let message: MessageItem
    var startsGroup = true
    var afterDivider = false
    /// A right-click anywhere on the row (C1 Trigger): its screen point and window.
    var onContextMenu: ((NSPoint, NSWindow?) -> Void)?
    @Environment(\.discordLocale) private var discordLocale
    @Environment(\.selectionColumn) private var column
    @State private var hovered = false

    /// The context the header time formats with (K4); body paragraphs build their own per tick.
    private var renderContext: RenderContext { RenderContext(now: .now, locale: discordLocale) }

    /// Above the header when the row replies to another.
    private var replyHeight: CGFloat { message.reply == nil ? 0 : M.replyLineHeight + M.replyGap }

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            if startsGroup {
                if let reply = message.reply {
                    HStack(spacing: T.Spacing.xs) {
                        Image(systemName: "arrowshape.turn.up.left.fill")
                        Text(verbatim: reply.authorName).fontWeight(.semibold)
                        Text(verbatim: reply.segments.map { if case .text(let text) = $0 { text } else { "" } }.joined())
                    }
                    .font(T.Typography.reply)
                    .foregroundStyle(.secondary)
                    .lineLimit(1)
                    .frame(height: M.replyLineHeight)
                    .padding(.bottom, M.replyGap)
                }
                let time = DiscordTime.header(date: message.timestamp, now: renderContext.now, locale: renderContext.locale)
                HStack(alignment: .firstTextBaseline, spacing: M.headerGap) {
                    Text(verbatim: message.authorName)
                        .font(T.Typography.authorName)
                        .displayNameStyle(message.displayNameStyle)
                        .foregroundStyle(message.authorColor.map(Color.init) ?? Color.primary)
                        // A press on the name reaches the header participant behind it (N2 Headers). The
                        // time keeps its hover (the tooltip), so a press on it starts no selection.
                        .allowsHitTesting(column.coordinator == nil)
                    Text(verbatim: time)
                        .font(T.Typography.timestamp)
                        .foregroundStyle(.secondary)
                        .tooltip(DiscordTime.tooltip(date: message.timestamp, locale: renderContext.locale, now: renderContext.now))
                }
                .frame(height: M.headerLineHeight)
                // N2 Headers: the header's selectable twin, behind the SwiftUI text.
                .background(alignment: .topLeading) {
                    if column.coordinator != nil {
                        HeaderTextRep(name: message.authorName, time: time, font: message.displayNameStyle?.font)
                            .environment(\.textMessage, message.id)
                    }
                }
            }
            if !message.body.blocks.isEmpty {
                MarkdownView(content: message.body)
                    .environment(\.textMessage, message.id)
            }
            // After the text, so a follow-up row (no header) shows it too.
            if message.edited {
                Text("(edited)").font(T.Typography.edited).foregroundStyle(.secondary)
            }
            ForEach(Array(message.images.enumerated()), id: \.offset) { _, image in
                ImageAttachmentView(image: image).padding(.top, T.Spacing.xs)
            }
            ForEach(Array(message.files.enumerated()), id: \.offset) { _, file in
                Label { Text(verbatim: file) } icon: { Image(systemName: "doc") }
                    .font(T.Typography.messageBody)
                    .foregroundStyle(.secondary)
            }
            // After the attachments, before the reactions (wave 3 E).
            ForEach(Array(message.embeds.enumerated()), id: \.offset) { _, embed in
                EmbedView(embed: embed).padding(.top, T.Spacing.xs)
            }
            if case .failed(let reason) = message.delivery {
                Text(verbatim: reason).font(T.Typography.timestamp).foregroundStyle(.red)
            }
            // Last in the text column: after the text, attachments and any failure line.
            ReactionsView(reactions: message.reactions)
        }
        .padding(.leading, M.contentInset)
        .padding(.trailing, M.trailingInset)
        .padding(.vertical, M.rowPaddingVertical)
        // The avatar hangs beside the content: it never adds to the row's height, so a row with
        // little text still clears it.
        .frame(minHeight: startsGroup ? M.rowPaddingVertical * 2 + M.avatarTop + T.Size.messageAvatar : nil, alignment: .topLeading)
        .frame(maxWidth: .infinity, alignment: .leading)
        .overlay(alignment: .topLeading) {
            if startsGroup {
                AvatarView(avatar: message.avatar, size: T.Size.messageAvatar)
                    .padding(.leading, M.avatarLeading)
                    .padding(.top, M.rowPaddingVertical + replyHeight + M.avatarTop)
            } else if hovered {
                Text(verbatim: DiscordTime.format(date: message.timestamp, style: .shortTime, locale: renderContext.locale, now: renderContext.now))
                    .font(T.Typography.gutterTime)
                    .foregroundStyle(.secondary)
                    .frame(width: M.gutterWidth, height: M.headerLineHeight, alignment: .trailing)
                    .padding(.top, M.rowPaddingVertical)
                    .tooltip(DiscordTime.tooltip(date: message.timestamp, locale: renderContext.locale, now: renderContext.now))
            }
        }
        .onHover { hovered = $0 }
        // Over the whole row, avatar and paddings included; it takes only right- and Control-clicks.
        .overlay { if let onContextMenu { MessageMenuTrigger(open: onContextMenu) } }
        .padding(.top, startsGroup && !afterDivider ? M.groupStart : 0)
        // An unconfirmed echo is dimmed until Discord confirms it.
        .opacity(message.delivery == .pending ? 0.5 : 1)
    }
}

/// An image under a message, fitted inside `MediaBox.image` and rounded. Live images load through
/// `MediaImages` (wave 3 E; a spoiler stays covered, and unfetched, until clicked); fixture images
/// are generated art, a mesh gradient whose hues come from the seed.
struct ImageAttachmentView: View {
    let image: ImageAttachment
    @State private var revealed = false

    /// The display box: the image's size fitted inside `MediaBox.image`.
    nonisolated var box: CGSize { MediaBox.fit(width: image.width, height: image.height, in: MediaBox.image) }

    /// What the view fetches: nothing for fixtures or while a spoiler is covered.
    nonisolated static func fetched(_ image: ImageAttachment, revealed: Bool) -> URL? {
        image.spoiler && !revealed ? nil : image.url
    }

    var body: some View {
        Group {
            if let url = Self.fetched(image, revealed: revealed) {
                MediaImageView(url: url, box: box)
            } else if image.url != nil {
                SpoilerMediaCover(box: box) { revealed = true }
            } else {
                generated.frame(width: box.width, height: box.height)
            }
        }
        .clipShape(RoundedRectangle(cornerRadius: T.Radius.messageImage, style: .continuous))
    }

    private var generated: some View {
        let hue = Double(image.seed % 12) / 12
        let colors = (0..<9).map { i in
            Color(hue: (hue + Double(i) * 0.07).truncatingRemainder(dividingBy: 1),
                  saturation: 0.7, brightness: i % 2 == 0 ? 0.95 : 0.6)
        }
        return MeshGradient(
            width: 3, height: 3,
            points: [[0, 0], [0.5, 0], [1, 0], [0, 0.5], [0.6, 0.4], [1, 0.5], [0, 1], [0.5, 1], [1, 1]],
            colors: colors
        )
    }
}

// MARK: Composer

/// Glass card: optional reply bar above one input row with a "+" on the left and GIF, sticker
/// and emoji glyphs on the right (Swiftcord v2 layout). The card is the container shape, so the
/// "+" and the reply bar are concentric with its corners, as in v2 (ConcentricRectangle). GIF,
/// sticker and emoji are decoration until their pickers exist.
struct Composer: View {
    @Binding var reply: ReplyPreview?
    /// The open conversation's draft (`ShellState.draft`).
    @Binding var draft: String
    /// Receives the draft and the reply target on Return (`ComposerSubmit`: never blank, never an
    /// IME commit); the reply bar clears with the draft.
    var onSend: ((String, ReplyTarget?) -> Void)?
    @State private var plusMenuOpen = false

    var body: some View {
        VStack(spacing: T.Spacing.sm) {
            if let reply {
                ReplyBar(reply: reply) { self.reply = nil }
            }
            HStack(spacing: T.Spacing.md) {
                Button { plusMenuOpen = true } label: {
                    Image(systemName: "plus")
                        .font(T.Typography.composerPlusGlyph)
                        .frame(width: T.Size.composerPlus, height: T.Size.composerPlus)
                        .background(T.Palette.plusButtonFill, in: ComposerInnerShape.shape)
                }
                .buttonStyle(.plain)
                .help("Add to message")
                .popover(isPresented: $plusMenuOpen, arrowEdge: .bottom) {
                    PlusMenu { plusMenuOpen = false }
                }

                TextField("Message", text: $draft)
                    .textFieldStyle(.plain)
                    .font(T.Typography.composer)
                    .onSubmit {
                        // The field editor still holding marked text means this Return belongs to
                        // the input method (it commits a candidate), not to the composer.
                        let editor = NSApp.keyWindow?.firstResponder as? NSTextView
                        ComposerSubmit.submit(&draft, reply: &reply, hasMarkedText: editor?.hasMarkedText() ?? false, onSend: onSend)
                    }

                // White glyphs, as in v2.
                Group {
                    Text("GIF").font(T.Typography.composerGif)
                    Image(systemName: "square.on.square")
                    Image(systemName: "face.smiling.inverse")
                }
                .font(T.Typography.composerGlyph)
                .foregroundStyle(.primary)
            }
            .frame(minHeight: T.Size.composerHeight - 2 * T.Spacing.sm)
        }
        .padding(T.Spacing.sm)
        .containerShape(RoundedRectangle(cornerRadius: T.Radius.composer, style: .continuous))
        .glassEffect(.regular, in: .rect(cornerRadius: T.Radius.composer))
        .padding(T.Spacing.lg)
    }
}

/// Shape of the composer's inner pieces: concentric with the card, never below the minimum.
enum ComposerInnerShape {
    static let shape = ConcentricRectangle(
        corners: .concentric(minimum: .fixed(T.Radius.composerInnerMinimum)), isUniform: true
    )
}

struct ReplyBar: View {
    let reply: ReplyPreview
    let onClose: () -> Void

    /// The name is payload text: shown verbatim, isolated so its bidi controls stay inside it.
    static func title(_ reply: ReplyPreview) -> String {
        "Replying to " + MarkdownText.isolated(reply.authorName)
    }

    var body: some View {
        HStack(spacing: T.Spacing.md) {
            Image(systemName: "arrowshape.turn.up.left.fill").font(T.Typography.replyGlyph)
            VStack(alignment: .leading, spacing: 0) {
                Text(verbatim: Self.title(reply)).font(T.Typography.replyAuthor)
                HStack(spacing: 2) {
                    ForEach(Array(reply.segments.enumerated()), id: \.offset) { _, segment in
                        switch segment {
                        case .text(let text):
                            Text(text)
                        case .customEmoji(let name, let url):
                            emoji(name: name, url: url)
                        }
                    }
                }
                .font(T.Typography.replyPreview)
                .foregroundStyle(.secondary)
                .lineLimit(1)
            }
            Spacer()
            Button(action: onClose) { Image(systemName: "xmark").font(T.Typography.replyClose) }
                .buttonStyle(.plain)
                .help("Cancel reply")
        }
        .padding(.horizontal, T.Spacing.md)
        .padding(.vertical, T.Spacing.sm)
        .background(T.Palette.replyBarFill, in: ComposerInnerShape.shape)
    }

    /// Custom emoji render as an image, never as raw `<:name:id>` markup. Without a URL (fixtures)
    /// a placeholder glyph stands in.
    @ViewBuilder
    private func emoji(name: String, url: URL?) -> some View {
        Group {
            if let url {
                AsyncImage(url: url) { phase in
                    if let image = phase.image { image.resizable().scaledToFit() } else { Color.clear }
                }
            } else {
                Image(systemName: "face.smiling")
            }
        }
        .frame(width: T.Size.replyEmoji, height: T.Size.replyEmoji)
        .accessibilityLabel(":\(name):")
    }
}

/// Wide Discord-style "+" menu: leading icon, tall rows, generous spacing, in the system's glass
/// popover material rather than a narrow native menu (user decision 2026-10-13). Entries only
/// dismiss for now; their actions arrive with the features they open.
struct PlusMenu: View {
    let dismiss: () -> Void

    private let entries: [(title: String, symbol: String)] = [
        ("Upload a File", "arrow.up.doc"),
        ("Create Thread", "bubble.left.and.text.bubble.right"),
        ("Use Apps", "square.grid.2x2"),
        ("Create Poll", "chart.bar.xaxis"),
    ]

    var body: some View {
        VStack(spacing: T.Spacing.xs) {
            ForEach(entries, id: \.title) { entry in
                PlusMenuRow(title: entry.title, symbol: entry.symbol, action: dismiss)
            }
        }
        .padding(T.Spacing.sm)
        .frame(width: T.Size.plusMenuWidth)
    }
}

private struct PlusMenuRow: View {
    let title: String
    let symbol: String
    let action: () -> Void
    @State private var hovered = false

    var body: some View {
        Button(action: action) {
            HStack(spacing: T.Spacing.md) {
                Image(systemName: symbol).frame(width: T.Size.plusMenuIcon)
                Text(title).font(T.Typography.plusMenuRow)
                Spacer()
            }
            .padding(.horizontal, T.Spacing.md)
            .frame(height: T.Size.plusMenuRowHeight)
            .background(hovered ? T.Palette.plusMenuRowHover : .clear, in: RoundedRectangle(cornerRadius: T.Radius.plusMenuRow, style: .continuous))
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .onHover { hovered = $0 }
    }
}

#Preview("Composer with reply") {
    Composer(reply: .constant(Fixtures.reply), draft: .constant("")).frame(width: 640)
}

#Preview("Plus menu") {
    PlusMenu {}.padding()
}
