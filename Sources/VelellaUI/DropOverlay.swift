import AppKit
import SwiftUI
import UniformTypeIdentifiers

private typealias A = DesignTokens.Attachment

/// While files are dragged over the column (D12a, OA A27): it dims, and a blurple card with a dashed
/// inner border, three documents, the title and the subtitle shows in its middle. It takes no
/// clicks, so the drop reaches the column under it.
struct DropOverlay: View {
    let conversationName: String

    static func title(_ conversationName: String) -> String { "Upload to \"\(conversationName)\"" }
    static let subtitle = "You can add comments before uploading. Hold shift to upload directly."

    var body: some View {
        ZStack {
            A.dim
            ZStack {
                RoundedRectangle(cornerRadius: A.dropRadius, style: .continuous).fill(A.dropBlurple)
                RoundedRectangle(cornerRadius: A.dropRadius - A.dropBorderInset / 2, style: .continuous)
                    .strokeBorder(.white.opacity(0.55), style: StrokeStyle(lineWidth: 1.5, dash: A.dropDash))
                    .padding(A.dropBorderInset)
                VStack(spacing: 6) {
                    documents.frame(height: 62)
                    Text(verbatim: Self.title(conversationName))
                        .font(A.dropTitle)
                        .multilineTextAlignment(.center)
                        .lineLimit(2)
                    Text(Self.subtitle)
                        .font(A.dropSubtitle)
                        .multilineTextAlignment(.center)
                        .foregroundStyle(.white.opacity(0.85))
                }
                .foregroundStyle(.white)
                .padding(.horizontal, 24)
            }
            .frame(width: A.dropWidth, height: A.dropHeight)
        }
        .allowsHitTesting(false)
        .accessibilityElement(children: .ignore)
        .accessibilityLabel(Self.title(conversationName) + ". " + Self.subtitle)
    }

    /// Three documents fanned out, the middle one with a picture.
    private var documents: some View {
        ZStack {
            Image(systemName: "doc.text.fill").font(.system(size: 44)).foregroundStyle(.white.opacity(0.45))
                .rotationEffect(.degrees(-16)).offset(x: -34, y: 4)
            Image(systemName: "doc.richtext.fill").font(.system(size: 44)).foregroundStyle(.white.opacity(0.45))
                .rotationEffect(.degrees(16)).offset(x: 34, y: 4)
            Image(systemName: "photo.fill").font(.system(size: 40)).foregroundStyle(.white.opacity(0.9))
                .padding(8).background(.white.opacity(0.25), in: RoundedRectangle(cornerRadius: 8, style: .continuous))
        }
    }
}

/// Wires the column to `AttachmentController`: the drag state and the drop. The controller holds all
/// the rules (the gate, Shift, the conversation captured at the drop); this only reads the drop.
struct AttachmentDropDelegate: DropDelegate {
    let controller: AttachmentController
    /// The open conversation when the files fell.
    let conversation: String

    func validateDrop(info: DropInfo) -> Bool { controller.enabled && info.hasItemsConforming(to: [.fileURL]) }
    func dropEntered(info: DropInfo) { controller.dragEntered() }
    func dropExited(info: DropInfo) { controller.dragExited() }
    func dropUpdated(info: DropInfo) -> DropProposal? { DropProposal(operation: .copy) }

    func performDrop(info: DropInfo) -> Bool {
        controller.dragExited()
        guard controller.enabled else { return false }
        let providers = info.itemProviders(for: [.fileURL])
        guard !providers.isEmpty else { return false }
        // Shift is read now, as the files fall; the conversation is this one even if the selection
        // moves while the files are read.
        let shift = controller.shiftHeld()
        let conversation = conversation
        Task { @MainActor in
            await controller.dropped(DroppedFiles.urls(from: providers), shift: shift, conversation: conversation)
        }
        return true
    }
}

/// The file URLs of a drop's items. Only `public.file-url` items are read (CU-6).
@MainActor
enum DroppedFiles {
    static func urls(from providers: [NSItemProvider]) async -> [URL] {
        var urls: [URL] = []
        for provider in providers {
            let url: URL? = await withCheckedContinuation { continuation in
                _ = provider.loadObject(ofClass: URL.self) { url, _ in continuation.resume(returning: url) }
            }
            if let url, url.isFileURL { urls.append(url) }
        }
        return urls
    }
}
