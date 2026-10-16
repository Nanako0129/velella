import SwiftUI

private typealias A = DesignTokens.Attachment

/// What the Modify sheet edits (OA A24, A31), apart from the view so tests drive it.
struct ModifyDraft: Equatable {
    var filename: String
    var altText: String
    var spoiler: Bool
    let original: QueuedAttachment

    init(_ attachment: QueuedAttachment) {
        original = attachment
        filename = attachment.filename
        altText = attachment.altText ?? ""
        spoiler = attachment.spoiler
    }

    /// The description is capped at the API's 1024 characters.
    mutating func setAltText(_ text: String) {
        altText = String(text.prefix(A.altTextLimit))
    }

    /// Save needs a name.
    var canSave: Bool { !filename.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty }

    /// The attachment as saved. A description never set stays unset; one set, even emptied again,
    /// is sent as is (official `!= null`, P4). Nothing else of the file changes, and nothing is sent.
    var saved: QueuedAttachment {
        var attachment = original
        attachment.filename = filename
        attachment.altText = original.altText == nil && altText.isEmpty ? nil : altText
        attachment.spoiler = spoiler
        return attachment
    }
}

/// "Modify Attachment" (OA A31): the preview, "Filename", "Description (Alt Text)", "Mark as spoiler",
/// then Cancel and Save (the default). Save updates the queued file only.
struct ModifyAttachmentSheet: View {
    let attachment: QueuedAttachment
    /// The saved attachment, nil for Cancel.
    var finish: (QueuedAttachment?) -> Void
    @State private var draft: ModifyDraft
    @Environment(\.attachmentThumbnail) private var makeThumbnail
    @Environment(\.displayScale) private var scale
    @State private var thumbnail: CGImage?

    init(attachment: QueuedAttachment, finish: @escaping (QueuedAttachment?) -> Void) {
        self.attachment = attachment
        self.finish = finish
        _draft = State(initialValue: ModifyDraft(attachment))
    }

    var body: some View {
        VStack(alignment: .leading, spacing: DesignTokens.Spacing.md) {
            Text("Modify Attachment").font(.headline)
            preview
            VStack(alignment: .leading, spacing: 4) {
                Text("Filename").font(.subheadline.weight(.semibold))
                TextField("Filename", text: $draft.filename).labelsHidden()
            }
            VStack(alignment: .leading, spacing: 4) {
                Text("Description (Alt Text)").font(.subheadline.weight(.semibold))
                TextField(
                    "Add a description",
                    text: Binding(get: { draft.altText }, set: { draft.setAltText($0) }), axis: .vertical
                )
                .labelsHidden()
                .lineLimit(3...6)
            }
            Toggle("Mark as spoiler", isOn: $draft.spoiler).toggleStyle(.checkbox)
            HStack {
                Button("Cancel") { finish(nil) }.keyboardShortcut(.cancelAction)
                Spacer()
                Button("Save") { finish(draft.saved) }.keyboardShortcut(.defaultAction).disabled(!draft.canSave)
            }
        }
        .padding(20)
        .frame(width: A.modifyWidth)
        .task { thumbnail = await makeThumbnail.make(attachment.url, scale) }
    }

    @ViewBuilder
    private var preview: some View {
        Group {
            if let thumbnail {
                Image(decorative: thumbnail, scale: scale).resizable().aspectRatio(contentMode: .fit)
            } else {
                Image(systemName: AttachmentCardFacts.symbol(contentType: attachment.contentType))
                    .font(A.symbol).foregroundStyle(.secondary)
            }
        }
        .frame(width: A.modifyPreview, height: A.modifyPreview)
        .clipShape(.rect(cornerRadius: DesignTokens.Radius.composerInnerMinimum, style: .continuous))
    }
}
