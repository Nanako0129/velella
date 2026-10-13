import SwiftUI

private typealias T = DesignTokens

/// Message column: placeholder rows and the composer. Fresh code (Swiftcord legacy has no
/// matching view; its message list is a separate, DiscordKit-bound subsystem). Real message rows
/// replace `MessageRow` in a later slice.
struct MessageArea: View {
    let header: ChannelHeader?
    var isSignedIn = true
    let messages: [MessagePlaceholder]
    /// Not drawn yet: the message slice shows its loading and failure states.
    var load: MessageLoad = .idle
    @Binding var reply: ReplyPreview?
    var onSend: ((String) -> Void)?

    var body: some View {
        if header != nil {
            ScrollView {
                LazyVStack(alignment: .leading, spacing: T.Spacing.lg) {
                    ForEach(messages) { MessageRow(message: $0) }
                }
                .padding(T.Spacing.lg)
                .background(OverlayScrollerEnforcer())
            }
            .defaultScrollAnchor(.bottom)
            // Messages pass under the toolbar through a glass band with a hard lower edge, on a
            // darker pane than the translucent sidebar (Swiftcord v2, user's captures 2026-10-13).
            .scrollEdgeEffectStyle(.hard, for: .top)
            .safeAreaInset(edge: .bottom) { Composer(reply: $reply, onSend: onSend) }
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

struct MessageRow: View {
    let message: MessagePlaceholder

    var body: some View {
        HStack(alignment: .top, spacing: T.Spacing.md) {
            AvatarView(avatar: message.avatar, size: T.Size.messageAvatar)
            VStack(alignment: .leading, spacing: 0) {
                HStack(alignment: .firstTextBaseline, spacing: T.Spacing.sm) {
                    Text(message.authorName)
                        .font(T.Typography.authorName)
                        .displayNameStyle(message.displayNameStyle)
                        .foregroundStyle(message.authorColor.map(Color.init) ?? Color.primary)
                    Text(message.timestamp.formatted(date: .omitted, time: .shortened))
                        .font(T.Typography.timestamp)
                        .foregroundStyle(.secondary)
                }
                Text(message.text).font(T.Typography.messageBody)
                if let image = message.image {
                    ImageAttachmentView(image: image).padding(.top, T.Spacing.xs)
                }
            }
        }
    }
}

/// An image under a message, scaled down to the column's image width and rounded. Fixture images
/// are generated art: a mesh gradient whose hues come from the seed.
struct ImageAttachmentView: View {
    let image: ImageAttachment

    var body: some View {
        let hue = Double(image.seed % 12) / 12
        let colors = (0..<9).map { i in
            Color(hue: (hue + Double(i) * 0.07).truncatingRemainder(dividingBy: 1),
                  saturation: 0.7, brightness: i % 2 == 0 ? 0.95 : 0.6)
        }
        MeshGradient(
            width: 3, height: 3,
            points: [[0, 0], [0.5, 0], [1, 0], [0, 0.5], [0.6, 0.4], [1, 0.5], [0, 1], [0.5, 1], [1, 1]],
            colors: colors
        )
        .aspectRatio(image.width / image.height, contentMode: .fit)
        .frame(maxWidth: min(image.width, T.Size.messageImageMaxWidth), alignment: .leading)
        .clipShape(RoundedRectangle(cornerRadius: T.Radius.messageImage, style: .continuous))
    }
}

// MARK: Composer

/// Glass card: optional reply bar above one input row with a "+" on the left and GIF, sticker
/// and emoji glyphs on the right (Swiftcord v2 layout). The card is the container shape, so the
/// "+" and the reply bar are concentric with its corners, as in v2 (ConcentricRectangle). GIF,
/// sticker and emoji are decoration until their pickers exist.
struct Composer: View {
    @Binding var reply: ReplyPreview?
    /// Not called yet: the message slice wires submit (and its empty-draft and IME rules).
    var onSend: ((String) -> Void)?
    @State private var draft = ""
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

    var body: some View {
        HStack(spacing: T.Spacing.md) {
            Image(systemName: "arrowshape.turn.up.left.fill").font(T.Typography.replyGlyph)
            VStack(alignment: .leading, spacing: 0) {
                Text("Replying to \(reply.authorName)").font(T.Typography.replyAuthor)
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
    Composer(reply: .constant(Fixtures.reply)).frame(width: 640)
}

#Preview("Plus menu") {
    PlusMenu {}.padding()
}
