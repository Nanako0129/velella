import SwiftUI

private typealias A = DesignTokens.Attachment
private typealias T = DesignTokens

/// What a pending send's cards can do, by the row's id (core's echo row; composer plan CUA).
struct UploadRowActions {
    var cancel: (String) -> Void
    var retry: (String) -> Void
    /// The row's id and its files, whose temp files go with it (CU-1).
    var discard: (String, [QueuedAttachment]) -> Void
}

/// The upload cards under a pending message (OA A33): one per file, a document icon, the filename and
/// a thin progress bar from core's row, and a cross that cancels (D14a: the text and files return to
/// the composer). A failed send shows "Upload Failed" and "Click here to retry the upload" (A25); its
/// cross discards the row.
struct PendingUploadCards: View {
    let messageID: String
    let upload: PendingUploadItem
    var actions: UploadRowActions?

    var body: some View {
        VStack(alignment: .leading, spacing: T.Spacing.sm) {
            // By position: a snapshot rebuilds the files, so their ids do not last.
            ForEach(Array(upload.files.enumerated()), id: \.offset) { _, file in
                PendingUploadCard(file: file, state: upload.state, actions: actions.map { actions in
                    CardActions(
                        retry: { actions.retry(messageID) },
                        close: { upload.state == .failed ? actions.discard(messageID, upload.files.map(\.attachment)) : actions.cancel(messageID) })
                })
            }
        }
    }

    struct CardActions {
        var retry: () -> Void
        var close: () -> Void
    }
}

private struct PendingUploadCard: View {
    let file: PendingUploadItem.File
    let state: PendingUploadItem.State
    var actions: PendingUploadCards.CardActions?

    private var failed: Bool { state == .failed }

    var body: some View {
        HStack(spacing: A.pendingGap) {
            Image(systemName: "doc.fill").font(A.pendingIcon).foregroundStyle(A.documentIcon).accessibilityHidden(true)
            VStack(alignment: .leading, spacing: 6) {
                Text(verbatim: file.attachment.filename).font(.system(size: 14)).lineLimit(1).truncationMode(.middle)
                if failed {
                    VStack(alignment: .leading, spacing: 0) {
                        Text("Upload Failed").font(.caption.weight(.semibold)).foregroundStyle(.red)
                        Text("Click here to retry the upload").font(.caption).foregroundStyle(.secondary)
                    }
                } else {
                    ProgressView(value: file.fraction)
                        .progressViewStyle(.linear)
                        .controlSize(.mini)
                        .frame(height: A.pendingBarHeight)
                }
            }
            Spacer(minLength: 0)
            Button { actions?.close() } label: {
                Image(systemName: "xmark").font(.system(size: 12, weight: .semibold)).foregroundStyle(.secondary)
                    .frame(width: 24, height: 24).contentShape(Rectangle())
            }
            .buttonStyle(.plain)
            .help(failed ? "Discard Upload" : "Cancel Upload")
            .accessibilityHidden(true)
        }
        .padding(A.pendingPadding)
        .frame(maxWidth: A.pendingWidth, minHeight: A.pendingHeight, alignment: .leading)
        .background(A.pendingFill, in: RoundedRectangle(cornerRadius: A.pendingRadius, style: .continuous))
        .overlay(RoundedRectangle(cornerRadius: A.pendingRadius, style: .continuous).stroke(A.pendingStroke))
        .contentShape(Rectangle())
        .onTapGesture { if failed { actions?.retry() } }
        // One element labelled with the filename and state; its progress bar stays inside it and
        // reports its value.
        .accessibilityElement(children: .contain)
        .accessibilityLabel(Self.label(file, state))
        .modifier(CardAccessibilityActions(failed: failed, actions: actions))
    }

    static func label(_ file: PendingUploadItem.File, _ state: PendingUploadItem.State) -> String {
        file.attachment.filename + (state == .failed ? ", upload failed" : state == .sending ? ", uploading" : "")
    }
}

private struct CardAccessibilityActions: ViewModifier {
    let failed: Bool
    let actions: PendingUploadCards.CardActions?

    func body(content: Content) -> some View {
        if failed {
            content
                .accessibilityAction(named: "Retry Upload") { actions?.retry() }
                .accessibilityAction(named: "Discard Upload") { actions?.close() }
        } else {
            content.accessibilityAction(named: "Cancel Upload") { actions?.close() }
        }
    }
}
