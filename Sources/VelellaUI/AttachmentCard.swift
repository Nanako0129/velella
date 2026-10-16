import AppKit
import SwiftUI

private typealias A = DesignTokens.Attachment

/// What a card's preview shows: the file's name-derived facts, kept free of views so tests read them.
enum AttachmentCardFacts {
    /// SA: the SF Symbol of a non-image preview, by MIME type.
    static func symbol(contentType: String) -> String {
        let type = contentType.lowercased()
        if type.hasPrefix("image/") { return "photo" }
        if type.hasPrefix("video/") { return "film" }
        if type.hasPrefix("audio/") { return "waveform" }
        if type == "application/pdf" { return "doc.richtext" }
        return "doc"
    }

    /// SA: the uppercase extension, at most 8 characters, "FILE" without one.
    static func extensionLabel(filename: String) -> String {
        let ext = (filename as NSString).pathExtension.uppercased()
        return ext.isEmpty ? "FILE" : String(ext.prefix(A.extensionMaxLength))
    }

    static func sizeText(_ bytes: UInt64) -> String {
        ByteCountFormatter.string(fromByteCount: Int64(clamping: bytes), countStyle: .file)
    }

    static func accessibilityLabel(_ attachment: QueuedAttachment) -> String {
        "\(attachment.filename), \(sizeText(attachment.size))" + (attachment.spoiler ? ", spoiler" : "")
    }
}

/// One queued file (D11a; SA card): 200 x 200 on `.quinary`, the preview, an info strip on thin
/// material, and three glass circles for the spoiler, Modify and remove (OA A22).
struct AttachmentCard: View {
    let attachment: QueuedAttachment
    var onToggleSpoiler: () -> Void
    var onSave: (QueuedAttachment) -> Void
    var onRemove: () -> Void
    @Environment(\.attachmentThumbnail) private var makeThumbnail
    @Environment(\.displayScale) private var scale
    @State private var thumbnail: Thumbnail = .generating
    @State private var modifying = false

    enum Thumbnail { case generating, image(CGImage), none }

    var body: some View {
        ZStack(alignment: .topTrailing) {
            preview
                .blur(radius: attachment.spoiler ? A.spoilerBlur : 0)
                .clipShape(.rect(cornerRadius: A.previewRadius, style: .continuous))
                .padding(A.previewInset)
                .frame(maxWidth: .infinity, maxHeight: .infinity)
                .overlay {
                    if attachment.spoiler {
                        Text("SPOILER")
                            .font(A.spoilerPill)
                            .foregroundStyle(.white)
                            .padding(.horizontal, 10)
                            .padding(.vertical, 5)
                            .background(.black, in: Capsule())
                    }
                }
            VStack(spacing: 0) {
                Spacer(minLength: 0)
                infoStrip
            }
            actions.padding(A.actionInset)
        }
        .frame(width: A.cardSize, height: A.cardSize)
        .background(.quinary, in: ComposerInnerShape.shape)
        .clipShape(ComposerInnerShape.shape)
        .help(attachment.filename)
        .task(id: attachment.file) {
            thumbnail = .generating
            let image = await makeThumbnail.make(attachment.url, scale)
            thumbnail = image.map(Thumbnail.image) ?? .none
        }
        .sheet(isPresented: $modifying) {
            ModifyAttachmentSheet(attachment: attachment) { saved in
                modifying = false
                if let saved { onSave(saved) }
            }
        }
        // One element with its filename and state; the three actions are named actions (the circles
        // stay clickable for the pointer).
        .accessibilityElement(children: .ignore)
        .accessibilityLabel(AttachmentCardFacts.accessibilityLabel(attachment))
        .accessibilityAction(named: "Spoiler Attachment", onToggleSpoiler)
        .accessibilityAction(named: "Modify Attachment") { modifying = true }
        .accessibilityAction(named: "Remove Attachment", onRemove)
    }

    @ViewBuilder
    private var preview: some View {
        switch thumbnail {
        case .generating:
            HStack(spacing: A.processingGap) {
                ProgressView().progressViewStyle(.circular).controlSize(.small).accessibilityHidden(true)
                Text("Processing").font(.caption)
            }
            .foregroundStyle(.secondary)
            .frame(maxWidth: .infinity, maxHeight: .infinity)
        case .image(let image):
            Image(decorative: image, scale: scale)
                .resizable()
                .aspectRatio(contentMode: .fit)
        case .none:
            VStack(spacing: A.symbolGap) {
                Image(systemName: AttachmentCardFacts.symbol(contentType: attachment.contentType))
                    .font(A.symbol)
                Text(AttachmentCardFacts.extensionLabel(filename: attachment.filename))
                    .font(A.symbolExtension)
                    .lineLimit(1)
                    .minimumScaleFactor(0.7)
                    .frame(maxWidth: A.symbolExtensionWidth)
            }
            .foregroundStyle(.secondary)
            .frame(maxWidth: .infinity, maxHeight: .infinity)
        }
    }

    private var infoStrip: some View {
        VStack(alignment: .leading, spacing: A.stripGap) {
            Text(verbatim: attachment.filename)
                .font(A.name)
                .lineLimit(1)
                .truncationMode(.middle)
                .foregroundStyle(.primary)
            Text(verbatim: AttachmentCardFacts.sizeText(attachment.size))
                .font(A.size)
                .foregroundStyle(.secondary)
        }
        .padding(.horizontal, A.stripPaddingH)
        .padding(.vertical, A.stripPaddingV)
        .frame(maxWidth: .infinity, alignment: .leading)
        .background(Material.thin)
    }

    private var actions: some View {
        HStack(spacing: A.actionSpacing) {
            action(attachment.spoiler ? "eye.slash" : "eye", help: "Spoiler Attachment", onToggleSpoiler)
            action("pencil", help: "Modify Attachment") { modifying = true }
            action("xmark", help: "Remove Attachment", onRemove)
        }
    }

    private func action(_ symbol: String, help: String, _ run: @escaping () -> Void) -> some View {
        Button(action: run) {
            Image(systemName: symbol)
                .font(A.actionGlyph)
                .foregroundStyle(.primary)
                .frame(width: A.actionSize, height: A.actionSize)
                .glassEffect(.regular, in: Circle())
                .contentShape(Circle())
        }
        .buttonStyle(.plain)
        .help(help)
        .accessibilityHidden(true)
    }
}
