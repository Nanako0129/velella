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

/// Wires the window to `AttachmentController`: the drag state and the drop. The controller holds all
/// the rules (the gate, Shift, the conversation captured at the drop); this only reads the drop.
struct AttachmentDropDelegate: DropDelegate {
    let controller: AttachmentController
    /// The open conversation when the files fell.
    let conversation: String

    /// File URLs, and the types an `NSFilePromiseReceiver` reads (Photos, Mail), so a promise-only
    /// drag is offered to the delegate too.
    static let types: [UTType] = [.fileURL] + NSFilePromiseReceiver.readableDraggedTypes.map { UTType($0) ?? UTType(importedAs: $0) }

    func validateDrop(info: DropInfo) -> Bool {
        controller.enabled && (info.hasItemsConforming(to: [.fileURL]) || !controller.promiseSources().isEmpty)
    }
    func dropEntered(info: DropInfo) { controller.dragEntered() }
    func dropExited(info: DropInfo) { controller.dragExited() }
    func dropUpdated(info: DropInfo) -> DropProposal? { DropProposal(operation: .copy) }

    func performDrop(info: DropInfo) -> Bool {
        controller.dragExited()
        guard controller.enabled else { return false }
        let providers = info.itemProviders(for: [.fileURL])
        let promises = controller.promiseSources()
        guard !providers.isEmpty || !promises.isEmpty else { return false }
        // Shift is read now, as the files fall; the conversation is this one even if the selection
        // moves while the files are read.
        let shift = controller.shiftHeld()
        let conversation = conversation
        Task { @MainActor in
            await controller.dropped(await DroppedFiles.urls(from: providers), promises: promises, shift: shift, conversation: conversation)
        }
        return true
    }
}

extension View {
    /// Files dropped anywhere in the window attach to the open conversation (Shift sends them), and
    /// while a drag is over it the window dims under the drop card (OA A27, D12a). The message column
    /// is part of the window. Only where attaching is allowed: Unknown shows and takes nothing.
    func attachmentDropTarget(_ controller: AttachmentController, conversation: String, name: String) -> some View {
        onDrop(of: AttachmentDropDelegate.types, delegate: AttachmentDropDelegate(controller: controller, conversation: conversation))
            .overlay {
                if controller.isDropTargeted && controller.enabled { DropOverlay(conversationName: name).ignoresSafeArea() }
            }
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

// MARK: File promises (CU-6: received into CU-1's directory)

/// A drag that promises files instead of naming them (Photos, Mail). `NSFilePromiseReceiver` is the
/// real one; tests stand in for it.
protocol FilePromiseSource: Sendable {
    /// How many files the promise holds.
    var count: Int { get }
    /// Writes the promised files into `directory`, reporting each file (or its error) once, on any thread.
    func receive(into directory: URL, report: @escaping @Sendable (Result<URL, any Error>) -> Void)
}

struct ReceiverSource: FilePromiseSource, @unchecked Sendable {
    // @unchecked: the receiver is only asked to receive, once, and reports through the block.
    let receiver: NSFilePromiseReceiver
    private static let queue = OperationQueue()

    /// The promised names, or the promised types (one per file) while the names are not resolved yet.
    var count: Int { receiver.fileNames.isEmpty ? receiver.fileTypes.count : receiver.fileNames.count }

    func receive(into directory: URL, report: @escaping @Sendable (Result<URL, any Error>) -> Void) {
        receiver.receivePromisedFiles(atDestination: directory, options: [:], operationQueue: Self.queue) { url, error in
            report(error.map { .failure($0) } ?? .success(url))
        }
    }
}

enum FilePromises {
    /// The promises on a pasteboard (the drag pasteboard for a real drag).
    static func sources(on board: NSPasteboard) -> [any FilePromiseSource] {
        let receivers = board.readObjects(forClasses: [NSFilePromiseReceiver.self], options: nil) as? [NSFilePromiseReceiver] ?? []
        return receivers.map { ReceiverSource(receiver: $0) }
    }

    /// A promise that has not delivered its files by then adds nothing. Not measured: Photos may export a
    /// large video slowly; a minute is long enough to wait with a drop pending.
    static let timeout: Duration = .seconds(60)

    /// Receives `source` into a fresh CU-1 directory and returns the files. A promise that fails (any
    /// file) or does not finish in `timeout` returns nothing and leaves nothing behind: its directory is
    /// removed (a late file lands nowhere).
    static func receive(_ source: any FilePromiseSource, services: AttachmentServices, timeout: Duration) async -> [URL] {
        guard source.count > 0, let directory = try? services.makeDirectory() else { return [] }
        let files: [URL]? = await withCheckedContinuation { continuation in
            let gate = Gate(continuation)
            let arrived = Locked((urls: [URL](), reports: 0))
            let expected = source.count
            source.receive(into: directory) { result in
                switch result {
                case .failure:
                    gate.finish(nil)
                case .success(let url) where !isDirectChild(url, of: directory):
                    // AppKit names a promised file by its base name in the destination; anything else
                    // (a nested path, another directory, "." or "..") fails the promise, so the
                    // cleanup in `dropped` only ever sees direct children of this directory.
                    gate.finish(nil)
                case .success(let url):
                    let (urls, reports) = arrived.update { state in
                        state.urls.append(url)
                        state.reports += 1
                        return (state.urls, state.reports)
                    }
                    if reports == expected { gate.finish(urls) }
                }
            }
            Task {
                try? await Task.sleep(for: timeout)
                gate.finish(nil)
            }
        }
        if let files { return files }
        // Any file of the directory removes it, with the files already written.
        services.remove(directory.appendingPathComponent("promise"))
        return []
    }

    /// True when `url` names an entry directly inside `directory`, compared without resolving links.
    static func isDirectChild(_ url: URL, of directory: URL) -> Bool {
        let name = url.lastPathComponent
        guard !name.isEmpty, name != ".", name != "..", !name.contains("/") else { return false }
        return url.deletingLastPathComponent().standardizedFileURL.path == directory.standardizedFileURL.path
    }

    /// Resumes its continuation once, whoever finishes first (a report, a failure, the timeout).
    private final class Gate: @unchecked Sendable {
        private let lock = NSLock()
        private var continuation: CheckedContinuation<[URL]?, Never>?
        init(_ continuation: CheckedContinuation<[URL]?, Never>) { self.continuation = continuation }
        func finish(_ result: [URL]?) {
            let pending = lock.withLock { () -> CheckedContinuation<[URL]?, Never>? in
                defer { continuation = nil }
                return continuation
            }
            pending?.resume(returning: result)
        }
    }

    private final class Locked<Value>: @unchecked Sendable {
        private let lock = NSLock()
        private var value: Value
        init(_ value: Value) { self.value = value }
        func update<Result>(_ change: (inout Value) -> Result) -> Result { lock.withLock { change(&value) } }
    }
}
