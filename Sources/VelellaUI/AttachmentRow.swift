import AppKit
import QuickLookThumbnailing
import SwiftUI
import UniformTypeIdentifiers

private typealias T = DesignTokens
private typealias A = DesignTokens.Attachment

// MARK: Intake

/// Turns files into queue entries (composer plan CUA Entry points, HEIC / JXR conversion). The file
/// layer is `AttachmentServices`' (CU-1, CU-6).
@MainActor
enum AttachmentIntake {
    /// CU-5: the MIME type from the extension, `""` when the system has none.
    static func contentType(forName name: String) -> String {
        UTType(filenameExtension: (name as NSString).pathExtension)?.preferredMIMEType ?? ""
    }

    /// One entry per regular file (CU-6: directories, packages, FIFOs and devices are refused here, with
    /// no request). A HEIC or JXR is replaced by its JPEG when the account's config enables it and the
    /// conversion succeeds within the size; otherwise, with no notice, the original stays (D16a, A50).
    static func entries(for urls: [URL], services: AttachmentServices) async -> [QueuedAttachment] {
        var entries: [QueuedAttachment] = []
        for url in urls {
            guard let identity = services.identity(url) else { continue }
            let name = url.lastPathComponent
            let type = contentType(forName: name)
            if let kind = ImageConversion.kind(contentType: type, filename: name),
               case .enabled(let quality, let maxBytes) = services.conversion(kind),
               let converted = await convert(identity, name: name, type: type, kind: kind, quality: quality, maxBytes: maxBytes, services: services) {
                entries.append(converted)
            } else {
                entries.append(QueuedAttachment(file: identity, filename: name, contentType: type))
            }
        }
        return entries
    }

    private static func convert(
        _ identity: QueuedFileIdentity, name: String, type: String, kind: ConvertibleKind, quality: UInt8, maxBytes: UInt64,
        services: AttachmentServices
    ) async -> QueuedAttachment? {
        let source = URL(fileURLWithPath: identity.path)
        let converted = await Task.detached(priority: .userInitiated) {
            ImageConversion.convert(source, quality: quality, maxFileSizeBytes: maxBytes)
        }.value
        guard let converted else { return nil }
        let newName = ImageConversion.rewrittenName(name)
        guard let file = try? services.write(converted.jpeg, newName) else { return nil }
        guard let id = services.identity(file) else {
            services.remove(file)
            return nil
        }
        return QueuedAttachment(
            file: id, filename: newName, contentType: ImageConversion.originalContentType(type, filename: name, kind: kind),
            originalMD5: converted.originalMD5)
    }

    /// A pasted image or long text: bytes written to a CU-1 temp file, then attached like any file.
    static func synthesized(_ data: Data, named name: String, contentType: String, services: AttachmentServices) -> QueuedAttachment? {
        guard let url = try? services.write(data, name) else { return nil }
        guard let identity = services.identity(url) else {
            services.remove(url)
            return nil
        }
        return QueuedAttachment(file: identity, filename: name, contentType: contentType)
    }
}

// MARK: Controller

/// What one message column's attachments share: the gate, the alert, the drag state and the adds that
/// outlive a view update (a drop or a panel finishes later, into the conversation the user acted in).
/// The view refreshes the unobserved fields on every update (`configure`).
@MainActor @Observable
final class AttachmentController {
    var alert: AttachmentAlert?
    /// Files are being dragged over the column (the drop overlay).
    var isDropTargeted = false

    @ObservationIgnored private(set) var services: AttachmentServices?
    @ObservationIgnored private(set) var permission: AttachPermission = .unknown
    @ObservationIgnored private(set) var premiumType: UInt8?
    @ObservationIgnored private(set) var conversation = ""
    @ObservationIgnored private var queues: Binding<[String: [QueuedAttachment]]> = .constant([:])
    /// The "Upload a File" panel; tests stand in for it.
    @ObservationIgnored var chooseFiles: @MainActor () async -> [URL] = AttachmentOpenPanel.choose
    /// Whether Shift is held as files fall (a drop carries no modifiers, so the keyboard's state at
    /// that moment); tests stand in for it.
    @ObservationIgnored var shiftHeld: @MainActor () -> Bool = { NSEvent.modifierFlags.contains(.shift) }

    func configure(
        services: AttachmentServices?, permission: AttachPermission, premiumType: UInt8?, conversation: String,
        queues: Binding<[String: [QueuedAttachment]]>
    ) {
        self.services = services; self.permission = permission; self.premiumType = premiumType
        self.conversation = conversation; self.queues = queues
    }

    /// The upload entry, the drop target and file paste exist only while this holds (OA A26, F1);
    /// Unknown does not pass.
    var enabled: Bool { services != nil && permission.permitsAttaching }

    /// A drag came over the column: the overlay shows only where a drop would attach.
    func dragEntered() { if enabled { isDropTargeted = true } }
    func dragExited() { isDropTargeted = false }

    /// The long paste's rule (OA A21): the user's `uploadLongMessages` setting where velella can read
    /// it, else the official default, on. velella reads no such setting, so it is on.
    static let uploadLongMessages = true

    // MARK: Adds

    /// "+" → "Upload a File".
    func uploadFile(conversation: String) async {
        guard enabled else { return }
        await add(await chooseFiles(), to: conversation)
    }

    /// Adds files to `conversation`'s queue, after the limits check (OA A36, A37): nothing is added
    /// when one is broken.
    func add(_ urls: [URL], to conversation: String) async {
        guard enabled, let services else { return }
        admit(await AttachmentIntake.entries(for: urls, services: services), to: conversation)
    }

    func admit(_ entries: [QueuedAttachment], to conversation: String) {
        guard let services, !entries.isEmpty else { return }
        let existing = queues.wrappedValue[conversation] ?? []
        if let limit = services.limits(conversation, (existing + entries).map(\.size), .add) {
            refuse(entries, limit)
        } else {
            queues.wrappedValue[conversation] = existing + entries
        }
    }

    private func refuse(_ entries: [QueuedAttachment], _ limit: AttachmentLimit) {
        // A synthesized file nobody holds any more goes with its directory.
        entries.forEach { services?.remove($0.url) }
        alert = AttachmentAlert(for: limit, premiumType: premiumType)
    }

    /// A drop on the column. `conversation` was captured when the files fell; `shift` was read from
    /// the drop. Shift sends the files at once with no text, leaving the draft and the queue alone
    /// (OA A35, D12a); without it they join the queue.
    func dropped(_ urls: [URL], shift: Bool, conversation: String) async {
        isDropTargeted = false
        guard enabled, let services else { return }
        let entries = await AttachmentIntake.entries(for: urls, services: services)
        guard !entries.isEmpty else { return }
        guard shift else { return admit(entries, to: conversation) }
        if let limit = services.limits(conversation, entries.map(\.size), .add) { return refuse(entries, limit) }
        switch services.send(AttachmentSendRequest(conversation: conversation, text: "", files: entries, instant: true)) {
        case .started: break
        case .limit(let limit): refuse(entries, limit)
        case .refused: entries.forEach { services.remove($0.url) }
        }
    }

    /// A paste the editor offered. True when it was taken (files, an image, a long text, each only
    /// while attaching is allowed); false leaves the paste to the text view.
    func paste(_ content: PasteContent, conversation: String) -> Bool {
        guard enabled, let services else { return false }
        switch content {
        case .files(let urls):
            Task { await add(urls, to: conversation) }
            return true
        case .image(let png):
            guard let entry = AttachmentIntake.synthesized(png, named: "image.png", contentType: "image/png", services: services) else { return false }
            admit([entry], to: conversation)
            return true
        case .longText(let text):
            guard Self.uploadLongMessages,
                  let entry = AttachmentIntake.synthesized(Data(text.utf8), named: "message.txt", contentType: "text/plain", services: services)
            else { return false }
            admit([entry], to: conversation)
            return true
        }
    }
}

/// The "Upload a File" panel: several files of any type.
@MainActor
enum AttachmentOpenPanel {
    static func choose() async -> [URL] {
        let panel = NSOpenPanel()
        panel.canChooseFiles = true
        panel.canChooseDirectories = false
        panel.allowsMultipleSelection = true
        panel.prompt = "Upload"
        return await withCheckedContinuation { continuation in
            let finish: (NSApplication.ModalResponse) -> Void = { continuation.resume(returning: $0 == .OK ? panel.urls : []) }
            if let window = NSApp.keyWindow { panel.beginSheetModal(for: window, completionHandler: finish) } else { panel.begin(completionHandler: finish) }
        }
    }
}

// MARK: Thumbnails

/// How a card gets its thumbnail (SA: `QLThumbnailGenerator`, a real thumbnail only, 200 x 200).
struct AttachmentThumbnails: Sendable {
    var make: @Sendable (URL, CGFloat) async -> CGImage?

    init(_ make: @escaping @Sendable (URL, CGFloat) async -> CGImage?) { self.make = make }

    static let quickLook = AttachmentThumbnails { url, scale in
        let request = QLThumbnailGenerator.Request(
            fileAt: url, size: CGSize(width: A.cardSize, height: A.cardSize), scale: scale, representationTypes: .thumbnail)
        return try? await QLThumbnailGenerator.shared.generateBestRepresentation(for: request).cgImage
    }
}

extension EnvironmentValues {
    @Entry var attachmentThumbnail = AttachmentThumbnails.quickLook
}

// MARK: Row

/// The queued files, above the input row inside the composer's glass card (SA: a horizontal scroll
/// view with no indicators, 8 between cards, 6 either side). Its insertion and removal transition
/// belongs to the composer, which keys it on the count.
struct AttachmentRow: View {
    @Binding var attachments: [QueuedAttachment]
    /// Called with a removed file, so a synthesized one is deleted (CU-1).
    var onRemove: (QueuedAttachment) -> Void = { _ in }

    var body: some View {
        ScrollView(.horizontal) {
            HStack(spacing: A.rowSpacing) {
                ForEach(attachments) { attachment in
                    AttachmentCard(
                        attachment: attachment,
                        onToggleSpoiler: { update(attachment.id) { $0.spoiler.toggle() } },
                        onSave: { saved in update(attachment.id) { $0 = saved } },
                        onRemove: { remove(attachment) })
                }
            }
            .padding(.horizontal, A.rowInset)
        }
        .scrollIndicators(.hidden)
    }

    private func update(_ id: UUID, _ change: (inout QueuedAttachment) -> Void) {
        guard let i = attachments.firstIndex(where: { $0.id == id }) else { return }
        change(&attachments[i])
    }

    private func remove(_ attachment: QueuedAttachment) {
        attachments.removeAll { $0.id == attachment.id }
        onRemove(attachment)
    }
}
