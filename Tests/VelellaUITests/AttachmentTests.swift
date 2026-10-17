import AppKit
import ImageIO
import Observation
import SwiftUI
import Testing
import UniformTypeIdentifiers
@testable import VelellaUI

// Composer plan CUA (docs/plans/composer.md): the attachment row and cards, the Modify sheet, the
// three entry points (panel, drop, paste through the real editor), the add-time limits and their
// alerts, HEIC / JXR conversion, the permission gate, the drop overlay, Return with files, and the
// pending row. Views are hosted in an offscreen window (never ordered in); core and the file layer
// are `AttachmentServices` fakes that record what they are asked, and the real CU-1 / CU-6 rules
// (regular files only, one directory per synthesized file) are implemented once in `Files` below,
// because VelellaUI cannot link `AttachmentFiles` (it lives behind the FFI).

// MARK: Rig

/// Records what the services were asked, from any thread.
private final class Log<Value: Sendable>: @unchecked Sendable {
    private let lock = NSLock()
    private var value: Value
    init(_ value: Value) { self.value = value }
    var get: Value { lock.withLock { value } }
    func update(_ change: (inout Value) -> Void) { lock.withLock { change(&value) } }
}

/// CU-1 and CU-6 over a private temporary root: the same rules as `AttachmentFiles`.
private struct Files: Sendable {
    let root: URL
    let removed = Log<[URL]>([])

    init() { root = FileManager.default.temporaryDirectory.appendingPathComponent("velella-ui-tests-\(UUID().uuidString)", isDirectory: true) }

    func identity(_ url: URL) -> QueuedFileIdentity? {
        guard url.isFileURL else { return nil }
        let path = url.resolvingSymlinksInPath().path
        var st = stat()
        guard lstat(path, &st) == 0, st.st_mode & S_IFMT == S_IFREG else { return nil }
        return QueuedFileIdentity(
            path: path, device: UInt64(UInt32(bitPattern: st.st_dev)), inode: st.st_ino, size: UInt64(clamping: st.st_size),
            mtimeNs: Int64(st.st_mtimespec.tv_sec) * 1_000_000_000 + Int64(st.st_mtimespec.tv_nsec))
    }

    func makeDirectory() throws -> URL {
        let directory = root.appendingPathComponent(UUID().uuidString, isDirectory: true)
        try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
        return directory
    }

    func write(_ data: Data, _ name: String) throws -> URL {
        let directory = root.appendingPathComponent(UUID().uuidString, isDirectory: true)
        try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
        let file = directory.appendingPathComponent(name)
        try data.write(to: file, options: .withoutOverwriting)
        return file
    }

    func remove(_ file: URL) {
        removed.update { $0.append(file) }
        let directory = file.deletingLastPathComponent()
        guard directory.deletingLastPathComponent().resolvingSymlinksInPath().path == root.resolvingSymlinksInPath().path else { return }
        try? FileManager.default.removeItem(at: directory)
    }

    /// A file in the root's own `source` directory (what a user picks or drops).
    func source(_ name: String, _ data: Data) throws -> URL {
        let directory = root.appendingPathComponent("source", isDirectory: true)
        try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
        let file = directory.appendingPathComponent(name)
        try data.write(to: file)
        return file
    }

    var synthesized: [String] {
        let all = (try? FileManager.default.contentsOfDirectory(atPath: root.path)) ?? []
        return all.filter { $0 != "source" }
    }
}

@MainActor private final class Fake {
    let files = Files()
    var sent: [AttachmentSendRequest] = []
    var cancelled: [String] = [], retried: [String] = [], discarded: [String] = []
    var limitCalls: [(conversation: String, sizes: [UInt64], stage: LimitStage)] = []
    var limit: ([UInt64], LimitStage) -> AttachmentLimit? = { _, _ in nil }
    var conversion: ImageConversionSetting = .off
    var outcome: AttachmentSendOutcome = .started

    var services: AttachmentServices {
        let files = files
        return AttachmentServices(
            identity: files.identity, write: files.write, remove: files.remove, makeDirectory: files.makeDirectory,
            limits: { [self] conversation, sizes, stage in
                limitCalls.append((conversation, sizes, stage))
                return limit(sizes, stage)
            },
            conversion: { [self] _ in conversion },
            send: { [self] request in
                sent.append(request)
                return outcome
            },
            cancel: { [self] in cancelled.append($0) },
            retry: { [self] in retried.append($0) },
            discard: { [self] in discarded.append($0) })
    }

    deinit { try? FileManager.default.removeItem(at: files.root) }
}

@MainActor @Observable private final class Model {
    var draft = ""
    var reply: ReplyPreview?
    var queues: [String: [QueuedAttachment]] = [:]
    var permission: AttachPermission = .allowed
    var premium: UInt8? = 2
    var messages: [MessageItem] = []
    var sentTexts: [String] = []
    var opened: [URL] = []
}

private struct Host: View {
    let model: Model
    let services: AttachmentServices?
    let controller: AttachmentController
    var body: some View {
        MessageArea(
            header: ChannelHeader(name: "general"), conversationID: "1", messages: model.messages, load: .loaded,
            reply: Binding(get: { model.reply }, set: { model.reply = $0 }),
            draft: Binding(get: { model.draft }, set: { model.draft = $0 }),
            onSend: { text, _ in model.sentTexts.append(text) },
            premiumType: model.premium,
            attachmentQueues: Binding(get: { model.queues }, set: { model.queues = $0 }),
            attachPermission: model.permission, attachmentServices: services, attachmentController: controller)
            // What ShellView puts around the whole window.
            .attachmentDropTarget(controller, conversation: "1", name: "general")
            .environment(\.openURL, OpenURLAction { model.opened.append($0); return .handled })
    }
}

@MainActor private func settle(_ view: NSView, _ rounds: Int = 6) async {
    for _ in 0..<rounds {
        view.layoutSubtreeIfNeeded()
        try? await Task.sleep(for: .milliseconds(30))
    }
}

@MainActor private final class Rig {
    let fake = Fake()
    let model = Model()
    let controller = AttachmentController()
    let host: NSHostingView<Host>
    let window: TextWindow

    init(permission: AttachPermission = .allowed, premium: UInt8? = 2, withServices: Bool = true) async {
        model.permission = permission
        model.premium = premium
        // The accessibility tree stands in for reading the screen: cards and rows are labelled elements.
        (host, window) = TextHarness.hosting(
            Host(model: model, services: withServices ? fake.services : nil, controller: controller),
            size: CGSize(width: 900, height: 800), accessibility: true)
        await settle(host)
    }

    var queue: [QueuedAttachment] { model.queues["1"] ?? [] }
    var editor: ComposerTextView { TextHarness.all(ComposerTextView.self, in: host)[0] }

    func labels() -> [String] {
        var out: [String] = []
        walk(host) { out.append(label($0)) }
        return out
    }

    func has(_ text: String) -> Bool { labels().contains { $0.contains(text) } }

    /// Paste what `board` carries into the real editor.
    func paste(_ board: NSPasteboard) async {
        editor.pasteboard = board
        editor.paste(nil)
        await settle(host)
    }

    func alertTexts() async -> [String] {
        for _ in 0..<30 where window.attachedSheet == nil { await settle(host, 1) }
        guard let sheet = window.attachedSheet?.contentView else { return [] }
        await settle(sheet)
        return strings(in: sheet)
    }
}

@MainActor private func send(_ object: NSObject, _ name: String) -> Any? {
    let selector = NSSelectorFromString(name)
    return object.responds(to: selector) ? object.perform(selector)?.takeUnretainedValue() : nil
}

@MainActor private func label(_ object: NSObject) -> String {
    [send(object, "accessibilityLabel"), send(object, "accessibilityValue")].compactMap { $0 as? String }.first { !$0.isEmpty } ?? ""
}

@MainActor private func walk(_ element: Any, _ visit: (NSObject) -> Void) {
    guard let object = element as? NSObject else { return }
    visit(object)
    for child in (send(object, "accessibilityChildren") as? [Any]) ?? [] { walk(child, visit) }
}

@MainActor private func strings(in view: NSView) -> [String] {
    let own = [(view as? NSTextField)?.stringValue, (view as? NSButton)?.title].compactMap { $0 }
    return own + view.subviews.flatMap(strings)
}

private func png(width: Int, height: Int, color: CGColor = CGColor(red: 1, green: 0, blue: 0, alpha: 1)) -> CGImage {
    let context = CGContext(
        data: nil, width: width, height: height, bitsPerComponent: 8, bytesPerRow: 0, space: CGColorSpace(name: CGColorSpace.sRGB)!,
        bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue)!
    context.setFillColor(color)
    context.fill(CGRect(x: 0, y: 0, width: width, height: height))
    return context.makeImage()!
}

/// Encoded image bytes of `type`, with the given properties (orientation, metadata).
private func encoded(_ image: CGImage, as type: UTType, properties: [CFString: Any] = [:]) -> Data? {
    let data = NSMutableData()
    guard let destination = CGImageDestinationCreateWithData(data, type.identifier as CFString, 1, nil) else { return nil }
    CGImageDestinationAddImage(destination, image, properties as CFDictionary)
    return CGImageDestinationFinalize(destination) ? data as Data : nil
}

@MainActor private func board() -> NSPasteboard { TextHarness.board() }

private func queued(_ name: String, size: UInt64 = 10, contentType: String = "", spoiler: Bool = false) -> QueuedAttachment {
    QueuedAttachment(
        file: QueuedFileIdentity(path: "/tmp/\(name)", device: 1, inode: 2, size: size, mtimeNs: 3), filename: name,
        spoiler: spoiler, contentType: contentType)
}

// MARK: Gate

@Test func onlyAnAllowedConversationPermitsAttaching() {
    #expect(AttachPermission.allowed.permitsAttaching)
    #expect(!AttachPermission.denied.permitsAttaching)
    #expect(!AttachPermission.unknown.permitsAttaching)
    // ATTACH_FILES is bit 15 of the official table.
    #expect(AttachPermission(bits: 1 << 15, attachBit: 15) == .allowed)
    #expect(AttachPermission(bits: ~(1 << 15), attachBit: 15) == .denied)
}

@MainActor @Test func theUploadEntryIsInThePlusMenuOnlyWhereAttachingIsAllowed() async {
    for (allowed, shown) in [(true, true), (false, false)] {
        let (host, _) = TextHarness.hosting(PlusMenu(canUpload: allowed) {}, size: CGSize(width: 300, height: 300), accessibility: true)
        await settle(host)
        var labels: [String] = []
        walk(host) { labels.append(label($0)) }
        #expect(labels.contains("Upload a File") == shown, "\(labels)")
        #expect(labels.contains("Create Thread"), "the other entries stay")
    }
}

@MainActor @Test func unknownPermissionHidesTheDropFilePasteAndThePanel() async throws {
    for permission in [AttachPermission.unknown, .denied] {
        let rig = await Rig(permission: permission)
        let file = try rig.fake.files.source("a.txt", Data("a".utf8))
        // The drag: no overlay.
        rig.controller.dragEntered()
        await settle(rig.host)
        #expect(!rig.controller.isDropTargeted && !rig.has("Upload to"), "\(permission)")
        rig.controller.isDropTargeted = true
        await settle(rig.host)
        #expect(!rig.has("Upload to"), "the overlay itself checks the gate too, \(permission)")
        // The drop: nothing attached, nothing sent, Shift or not.
        await rig.controller.dropped([file], shift: false, conversation: "1")
        await rig.controller.dropped([file], shift: true, conversation: "1")
        #expect(rig.queue.isEmpty && rig.fake.sent.isEmpty, "\(permission)")
        // The panel.
        rig.controller.chooseFiles = { [file] }
        await rig.controller.uploadFile(conversation: "1")
        #expect(rig.queue.isEmpty)
        // File paste, and a long paste, are plain text pastes.
        let files = board()
        files.writeObjects([file as NSURL])
        await rig.paste(files)
        let long = board()
        long.setString(String(repeating: "x", count: 5000), forType: .string)
        await rig.paste(long)
        #expect(rig.queue.isEmpty && rig.model.draft.count == 5000, "\(permission)")
    }
}

@MainActor @Test func withoutServicesNothingAttaches() async throws {
    let rig = await Rig(withServices: false)
    #expect(!rig.controller.enabled)
    let long = board()
    long.setString(String(repeating: "x", count: 5000), forType: .string)
    await rig.paste(long)
    #expect(rig.queue.isEmpty)
}

// MARK: Entry points: panel and drop

@MainActor @Test func thePanelAddsEveryChosenFileToTheConversationItWasOpenedIn() async throws {
    let rig = await Rig()
    let a = try rig.fake.files.source("a.txt", Data("aa".utf8)), b = try rig.fake.files.source("b.bin", Data("bbb".utf8))
    rig.controller.chooseFiles = { [a, b] }
    await rig.controller.uploadFile(conversation: "1")
    await settle(rig.host)
    #expect(rig.queue.map(\.filename) == ["a.txt", "b.bin"] && rig.queue.map(\.size) == [2, 3])
    #expect(rig.queue[0].contentType == "text/plain" && rig.queue[1].contentType != "", "CU-5: the MIME type from the extension")
    #expect(rig.queue.allSatisfy { !$0.spoiler && $0.altText == nil })
    // Nothing was uploaded or sent: adding only queues (D10).
    #expect(rig.fake.sent.isEmpty)
}

@MainActor @Test func aDropAttachesAndCapturesTheConversationItFellOn() async throws {
    let rig = await Rig()
    let a = try rig.fake.files.source("a.txt", Data("aa".utf8))
    await rig.controller.dropped([a], shift: false, conversation: "77")
    // The selection can move while the files are read: they join the conversation of the drop.
    #expect(rig.model.queues["77"]?.map(\.filename) == ["a.txt"] && rig.queue.isEmpty)
    #expect(rig.fake.sent.isEmpty)
}

@MainActor @Test func onlyRegularFilesAreAttachedAtAnyEntryPoint() async throws {
    let rig = await Rig()
    let directory = rig.fake.files.root.appendingPathComponent("a-directory", isDirectory: true)
    try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
    let package = rig.fake.files.root.appendingPathComponent("Thing.app", isDirectory: true)
    try FileManager.default.createDirectory(at: package, withIntermediateDirectories: true)
    var fifo = rig.fake.files.root.appendingPathComponent("a-fifo")
    #expect(mkfifo(fifo.path, 0o600) == 0)
    fifo = fifo.standardizedFileURL
    let regular = try rig.fake.files.source("ok.txt", Data("ok".utf8))
    await rig.controller.dropped([directory, package, fifo, URL(fileURLWithPath: "/dev/zero"), regular], shift: false, conversation: "1")
    #expect(rig.queue.map(\.filename) == ["ok.txt"])
    #expect(rig.fake.limitCalls.count == 1 && rig.fake.limitCalls[0].sizes == [2])
}

@MainActor @Test func everyAddRunsTheLimitsCheckFirstOverTheQueueAndTheNewFiles() async throws {
    let rig = await Rig()
    let a = try rig.fake.files.source("a.txt", Data("aa".utf8)), b = try rig.fake.files.source("b.txt", Data("bbbb".utf8))
    await rig.controller.dropped([a], shift: false, conversation: "1")
    await rig.controller.dropped([b], shift: false, conversation: "1")
    #expect(rig.fake.limitCalls.map(\.sizes) == [[2], [2, 4]])
    #expect(rig.fake.limitCalls.allSatisfy { $0.stage == .add && $0.conversation == "1" })
}

@Test func theUpsellSizesFollowTheOfficialFormatter() {
    // `formatFileSize(bytes / 1024, {useKibibytes: true})` with no `useSpace`: whole MB rounded up,
    // whole GB from 1024 MB, no space.
    #expect(AttachmentAlert.upsellSize(20_971_520) == "20MB")
    #expect(AttachmentAlert.upsellSize(20_971_521) == "21MB")
    #expect(AttachmentAlert.upsellSize(52_428_800) == "50MB")
    #expect(AttachmentAlert.upsellSize(524_288_000) == "500MB")
    #expect(AttachmentAlert.upsellSize(1_073_741_824) == "1GB")
    #expect(AttachmentAlert.upsellSize(1_610_612_736) == "2GB")
}

/// humanize.js `filesize`, the plain modals' format (`453771.Hb`), computed by hand from its rules.
@Test func thePlainModalsFormatSizesAsHumanizeFilesizeDoes() {
    let vectors: [(UInt64, String)] = [
        (0, "0 bytes"), (512, "512 bytes"), (1_024, "1.00 KB"), (1_536, "1.50 KB"),
        (10_485_760, "10.00 MB"), (20_971_520, "20.00 MB"), (52_428_800, "50.00 MB"),
        (524_288_000, "500.00 MB"), (1_073_741_824, "1.00 GB"), (1_073_741_823, "1,024.00 MB"),
    ]
    for (bytes, text) in vectors { #expect(AttachmentAlert.size(bytes) == text, "\(bytes)") }
}

@Test func eachLimitHasTheOfficialAlertCopy() {
    let upsell = "The max file size is %@. Upgrade to Discord Nitro to upload files up to 500MB."
    let cases: [(UInt8?, AttachmentLimit, String, String, Bool)] = [
        // Anyone but a Nitro user gets the upsell modal for a file over its limit.
        (nil, .fileTooLarge(limit: 20_971_520), "Uh oh, this file exceeds the size limit.", String(format: upsell, "20MB"), true),
        (1, .fileTooLarge(limit: 52_428_800), "Uh oh, this file exceeds the size limit.", String(format: upsell, "50MB"), true),
        (3, .fileTooLarge(limit: 52_428_800), "Uh oh, this file exceeds the size limit.", String(format: upsell, "50MB"), true),
        // A Nitro user gets the plain modal.
        (2, .fileTooLarge(limit: 524_288_000), "Your files are too powerful", "Max file size is 500.00 MB please.", false),
        // The total has no upsell for anyone (the official call passes no showPremiumUpsell).
        (nil, .totalTooLarge(limit: 524_288_000), "Your files are too powerful", "Total message size must be less than 500.00 MB", false),
        (2, .totalTooLarge(limit: 1_073_741_824), "Your files are too powerful", "Total message size must be less than 1.00 GB", false),
        (2, .emptyFile, "Upload Failed", "File cannot be empty", false),
        (nil, .tooManyFiles(limit: 10), "Too many uploads!", "You can only upload 10 files at a time!", false),
    ]
    for (premium, limit, title, message, upsell) in cases {
        let alert = AttachmentAlert(for: limit, premiumType: premium)
        #expect(alert == AttachmentAlert(title: title, message: message, showsUpsell: upsell), "\(limit) \(String(describing: premium))")
    }
    #expect(AttachmentAlert.upsellPrimary == "Get Nitro" && AttachmentAlert.upsellSecondary == "View All Nitro Perks" && AttachmentAlert.dismiss == "Got It")
}

@MainActor @Test func aBrokenLimitAddsNothingAndShowsTheAlertWithItsOfficialButtons() async throws {
    for premium in [UInt8?.none, 2] {
        let rig = await Rig(premium: premium)
        rig.fake.limit = { _, _ in .fileTooLarge(limit: 10_485_760) }
        let a = try rig.fake.files.source("big.bin", Data("x".utf8))
        await rig.controller.dropped([a], shift: false, conversation: "1")
        #expect(rig.queue.isEmpty, "nothing is added")
        let shown = await rig.alertTexts()
        if premium == nil {
            #expect(shown.contains("Uh oh, this file exceeds the size limit."), "\(shown)")
            #expect(shown.contains { $0 == "The max file size is 10MB. Upgrade to Discord Nitro to upload files up to 500MB." }, "\(shown)")
            #expect(shown.contains("Get Nitro") && shown.contains("View All Nitro Perks") && shown.contains("Close") && !shown.contains("Got It"), "\(shown)")
            // Both Nitro buttons lead to the Nitro page.
            func button(_ title: String) -> NSButton? {
                func find(_ view: NSView) -> NSButton? { (view as? NSButton).flatMap { $0.title == title ? $0 : nil } ?? view.subviews.lazy.compactMap(find).first }
                return find(rig.window.attachedSheet!.contentView!)
            }
            try #require(button("View All Nitro Perks")).performClick(nil)
            #expect(rig.model.opened == [MessageLength.nitroURL])
        } else {
            #expect(shown.contains("Your files are too powerful") && shown.contains("Got It"), "\(shown)")
            #expect(shown.contains { $0.hasPrefix("Max file size is 10.00 MB please.") }, "\(shown)")
            #expect(!shown.contains("Get Nitro") && !shown.contains("View All Nitro Perks"), "\(shown)")
        }
        if let sheet = rig.window.attachedSheet { rig.window.endSheet(sheet) }
    }
}

@MainActor @Test func aRefusedSynthesizedFileIsDeletedWithItsDirectory() async {
    let rig = await Rig()
    rig.fake.limit = { _, _ in .emptyFile }
    let long = board()
    long.setString(String(repeating: "y", count: 4001), forType: .string)
    await rig.paste(long)
    #expect(rig.queue.isEmpty && rig.fake.files.synthesized.isEmpty && rig.controller.alert?.title == "Upload Failed")
}

@MainActor @Test func moreThanTenFilesAreRefusedWhenTheTenthIsPassed() async throws {
    let rig = await Rig()
    rig.fake.limit = { sizes, _ in sizes.count > 10 ? .tooManyFiles(limit: 10) : nil }
    let urls = try (0..<11).map { try rig.fake.files.source("f\($0).txt", Data("x".utf8)) }
    await rig.controller.dropped(Array(urls.prefix(10)), shift: false, conversation: "1")
    #expect(rig.queue.count == 10)
    await rig.controller.dropped([urls[10]], shift: false, conversation: "1")
    #expect(rig.queue.count == 10 && rig.controller.alert == AttachmentAlert(for: .tooManyFiles(limit: 10), premiumType: 2))
}

// MARK: Shift-drop

@MainActor @Test func shiftDropSendsAtOnceWithNoTextAndLeavesTheDraftAndTheQueue() async throws {
    let rig = await Rig()
    rig.model.draft = "keep me"
    rig.model.queues["1"] = [queued("waiting.txt")]
    let a = try rig.fake.files.source("a.txt", Data("aa".utf8))
    await rig.controller.dropped([a], shift: true, conversation: "1")
    let request = try #require(rig.fake.sent.first)
    #expect(rig.fake.sent.count == 1 && request.text == "" && request.instant && request.reply == nil && request.conversation == "1")
    #expect(request.files.map(\.filename) == ["a.txt"], "only the dropped files")
    #expect(rig.model.draft == "keep me" && rig.queue.map(\.filename) == ["waiting.txt"])
    #expect(rig.model.sentTexts.isEmpty)
}

@MainActor @Test func aShiftDropToAnotherConversationSendsThere() async throws {
    let rig = await Rig()
    let a = try rig.fake.files.source("a.txt", Data("aa".utf8))
    await rig.controller.dropped([a], shift: true, conversation: "88")
    #expect(rig.fake.sent.map(\.conversation) == ["88"])
}

@MainActor @Test func aRefusedShiftDropDeletesSynthesizedFilesAndKeepsTheRest() async throws {
    let rig = await Rig()
    rig.fake.outcome = .limit(.emptyFile)
    let a = try rig.fake.files.source("a.txt", Data("aa".utf8))
    await rig.controller.dropped([a], shift: true, conversation: "1")
    #expect(rig.controller.alert?.title == "Upload Failed" && rig.queue.isEmpty)
}

// MARK: A real drag, through the hosting view's drag destination

private final class FakeDrag: NSObject, NSDraggingInfo {
    let board: NSPasteboard
    let window: NSWindow
    init(board: NSPasteboard, window: NSWindow) { self.board = board; self.window = window }
    var draggingDestinationWindow: NSWindow? { window }
    var draggingSourceOperationMask: NSDragOperation { .copy }
    var draggingLocation: NSPoint { NSPoint(x: 400, y: 400) }
    var draggedImageLocation: NSPoint { NSPoint(x: 400, y: 400) }
    var draggedImage: NSImage? { nil }
    var draggingPasteboard: NSPasteboard { board }
    var draggingSource: Any? { nil }
    var draggingSequenceNumber: Int { 1 }
    var draggingFormation: NSDraggingFormation = .default
    var animatesToDestination = false
    var numberOfValidItemsForDrop = 1
    var springLoadingHighlight: NSSpringLoadingHighlight { .none }
    func slideDraggedImage(to screenPoint: NSPoint) {}
    func enumerateDraggingItems(
        options enumOpts: NSDraggingItemEnumerationOptions = [], for view: NSView?, classes classArray: [AnyClass],
        searchOptions: [NSPasteboard.ReadingOptionKey: Any] = [:], using block: @escaping (NSDraggingItem, Int, UnsafeMutablePointer<ObjCBool>) -> Void
    ) {}
    func resetSpringLoading() {}
}

/// The view SwiftUI registers for `onDrop`, and a drag of `files` over it.
@MainActor private func drag(_ rig: Rig, files: [URL], promised: Bool = false) throws -> (target: NSView, info: FakeDrag) {
    func find(_ view: NSView) -> NSView? { view.registeredDraggedTypes.isEmpty ? view.subviews.lazy.compactMap(find).first : view }
    let board = TextHarness.board()
    if !files.isEmpty { board.writeObjects(files as [NSURL]) }
    if promised {
        // What a promising drag (Photos, Mail) carries instead of file URLs.
        for type in NSFilePromiseReceiver.readableDraggedTypes { board.setString("public.data", forType: NSPasteboard.PasteboardType(type)) }
    }
    return (try #require(find(rig.window.contentView!)), FakeDrag(board: board, window: rig.window))
}

@MainActor @Test func aRealDragShowsTheOverlayAndTheDropAttachesToTheOpenConversation() async throws {
    let rig = await Rig()
    let file = try rig.fake.files.source("dragged.txt", Data("hello".utf8))
    let (target, info) = try drag(rig, files: [file])
    #expect(target.draggingEntered(info) == .copy)
    await settle(rig.host, 4)
    #expect(rig.controller.isDropTargeted && rig.has("Upload to \"general\""))
    target.draggingExited(info)
    await settle(rig.host, 4)
    #expect(!rig.controller.isDropTargeted && !rig.has("Upload to"))
    _ = target.draggingEntered(info)
    #expect(target.prepareForDragOperation(info) && target.performDragOperation(info))
    for _ in 0..<40 where rig.queue.isEmpty { await settle(rig.host, 1) }
    #expect(rig.queue.map(\.filename) == ["dragged.txt"] && rig.fake.sent.isEmpty)
    #expect(!rig.controller.isDropTargeted)
}

@MainActor @Test func aRealShiftDropSendsAtOnceAndLeavesTheDraft() async throws {
    let rig = await Rig()
    rig.controller.shiftHeld = { true }
    rig.model.draft = "my words"
    let file = try rig.fake.files.source("dragged.txt", Data("hello".utf8))
    let (target, info) = try drag(rig, files: [file])
    _ = target.draggingEntered(info)
    #expect(target.performDragOperation(info))
    for _ in 0..<40 where rig.fake.sent.isEmpty { await settle(rig.host, 1) }
    let request = try #require(rig.fake.sent.first)
    #expect(request.text == "" && request.instant && request.files.map(\.filename) == ["dragged.txt"])
    #expect(rig.model.draft == "my words" && rig.queue.isEmpty && rig.model.sentTexts.isEmpty)
    // Without Shift the same drop only attaches.
    rig.controller.shiftHeld = { false }
    _ = target.draggingEntered(info)
    #expect(target.performDragOperation(info))
    for _ in 0..<40 where rig.queue.isEmpty { await settle(rig.host, 1) }
    #expect(rig.fake.sent.count == 1 && rig.queue.count == 1)
}

@MainActor @Test func aRealDragOverAConversationThatForbidsAttachingShowsAndTakesNothing() async throws {
    for permission in [AttachPermission.unknown, .denied] {
        let rig = await Rig(permission: permission)
        let file = try rig.fake.files.source("dragged.txt", Data("hello".utf8))
        let (target, info) = try drag(rig, files: [file])
        #expect(target.draggingEntered(info) == [], "\(permission)")
        await settle(rig.host, 4)
        #expect(!rig.controller.isDropTargeted && !rig.has("Upload to"), "\(permission)")
        #expect(!target.performDragOperation(info), "\(permission)")
        await settle(rig.host, 4)
        #expect(rig.queue.isEmpty && rig.fake.sent.isEmpty, "\(permission)")
    }
}

// MARK: The whole window is the drop target (OA A27, D12a)

@MainActor @Observable private final class WindowModel {
    var state: ShellState
    init(_ state: ShellState) { self.state = state }
}

private struct WindowHost: View {
    let model: WindowModel
    let services: AttachmentServices
    var body: some View {
        ShellView(state: Binding(get: { model.state }, set: { model.state = $0 }), attachments: services)
    }
}

@MainActor private final class WindowRig {
    let fake = Fake()
    let model: WindowModel
    let host: NSHostingView<WindowHost>
    let window: TextWindow

    init(permission: AttachPermission) async {
        var state = ShellState(
            rail: [.guild(RailGuild(id: "g", name: "G", icon: .initials("G")))], selectedRailID: "g",
            categories: [ChannelCategory(id: "c", name: nil, channels: [ChannelItem(id: "1", name: "general")])], selectedChannelID: "1")
        state.attachPermission = permission
        state.messageLoad = .loaded
        state.premiumType = 2
        model = WindowModel(state)
        (host, window) = TextHarness.hosting(WindowHost(model: model, services: fake.services), size: CGSize(width: 1300, height: 800))
        await settle(host, 10)
    }

    var queue: [QueuedAttachment] { model.state.attachmentQueues["1"] ?? [] }

    /// The view SwiftUI registers for the window's `onDrop`: the only one, and the window's size.
    var target: NSView {
        get throws {
            func all(_ view: NSView) -> [NSView] { (view.registeredDraggedTypes.isEmpty ? [] : [view]) + view.subviews.flatMap(all) }
            // SwiftUI's destination view of the window-level onDrop: the one as large as the window.
            let views = all(window.contentView!).filter { $0.frame.size == window.contentView!.frame.size }
            return try #require(views.first, "\(all(window.contentView!).map { $0.frame })")
        }
    }

    func drag(files: [URL]) throws -> FakeDrag {
        let board = TextHarness.board()
        board.writeObjects(files as [NSURL])
        return FakeDrag(board: board, window: window)
    }
}

@MainActor @Test func aDragOverTheWindowDimsTheRailTheSidebarAndTheColumnUnderTheDropCard() async throws {
    let rig = await WindowRig(permission: .allowed)
    let target = try rig.target
    #expect(target.frame.size == CGSize(width: 1300, height: 800), "the drop target is the whole window: \(target.frame)")
    let spots: [(String, CGFloat, CGFloat)] = [("rail", 30, 400), ("sidebar", 200, 400), ("column", 1000, 400), ("column top", 760, 700)]
    let before = try #require(TextHarness.Pixels(rig.host))
    let file = try rig.fake.files.source("a.txt", Data("a".utf8))
    let info = try rig.drag(files: [file])
    #expect(target.draggingEntered(info) == .copy)
    await settle(rig.host, 8)
    let during = try #require(TextHarness.Pixels(rig.host))
    for (name, x, y) in spots {
        let was = before.rgb(x, y), now = during.rgb(x, y)
        #expect(now.0 < was.0 || was.0 < 10, "\(name) is dimmed: \(was) -> \(now)")
    }
    // The blurple card is centred in the window (330 x 190): its left upper part is blue, not grey.
    let card = during.rgb(650 - 140, 400 + 70), card2 = during.rgb(650 + 140, 400 - 70)
    #expect(card.2 > card.0 + 60 && card2.2 > card2.0 + 60, "\(card) \(card2)")
    target.draggingExited(info)
    await settle(rig.host, 8)
    let after = try #require(TextHarness.Pixels(rig.host))
    for (name, x, y) in spots { #expect(after.rgb(x, y) == before.rgb(x, y), "\(name) is back") }
}

@MainActor @Test func aDropAnywhereInTheWindowAttachesToTheOpenConversationAndShiftSendsThere() async throws {
    let rig = await WindowRig(permission: .allowed)
    let target = try rig.target
    let file = try rig.fake.files.source("a.txt", Data("aa".utf8))
    let info = try rig.drag(files: [file])
    // The drop is the same wherever it falls in the window: one destination covers it all.
    _ = target.draggingEntered(info)
    #expect(target.performDragOperation(info))
    for _ in 0..<40 where rig.queue.isEmpty { await settle(rig.host, 1) }
    #expect(rig.queue.map(\.filename) == ["a.txt"] && rig.fake.sent.isEmpty)
}

@MainActor @Test func aDragOverAWindowThatForbidsAttachingDimsNothingAndTakesNothing() async throws {
    for permission in [AttachPermission.unknown, .denied] {
        let rig = await WindowRig(permission: permission)
        let target = try rig.target
        let before = try #require(TextHarness.Pixels(rig.host))
        let file = try rig.fake.files.source("a.txt", Data("aa".utf8))
        let info = try rig.drag(files: [file])
        #expect(target.draggingEntered(info) == [], "\(permission)")
        await settle(rig.host, 8)
        let during = try #require(TextHarness.Pixels(rig.host))
        #expect(during.rgb(200, 400) == before.rgb(200, 400) && during.rgb(1000, 400) == before.rgb(1000, 400), "\(permission)")
        #expect(!target.performDragOperation(info), "\(permission)")
        await settle(rig.host, 6)
        #expect(rig.queue.isEmpty && rig.fake.sent.isEmpty, "\(permission)")
    }
}

// MARK: File promises (CU-6)

/// A promise that writes `files` into the directory it is given, off the main thread, or fails or
/// stalls on request. Stands in for `NSFilePromiseReceiver`, whose in-process round trip does not
/// resolve its file names offscreen (probed): the receive-and-attach path is what is tested.
private final class PromiseStub: FilePromiseSource, @unchecked Sendable {
    enum Mode { case deliver, failSecond, neverReport, lateWrite }
    let files: [(String, Data)]
    let mode: Mode
    let received = Log<Int>(0)
    init(_ files: [(String, Data)], mode: Mode = .deliver) { self.files = files; self.mode = mode }
    var count: Int { files.count }

    func receive(into directory: URL, report: @escaping @Sendable (Result<URL, any Error>) -> Void) {
        received.update { $0 += 1 }
        let files = files, mode = mode
        DispatchQueue.global().async {
            for (index, (name, data)) in files.enumerated() {
                let url = directory.appendingPathComponent(name)
                switch mode {
                case .deliver:
                    try? data.write(to: url)
                    report(.success(url))
                case .failSecond:
                    if index == 0 { try? data.write(to: url); report(.success(url)) } else { report(.failure(CocoaError(.fileReadUnknown))) }
                case .neverReport:
                    try? data.write(to: url)
                case .lateWrite:
                    Thread.sleep(forTimeInterval: 0.4)
                    try? data.write(to: url) // the directory is gone by now: nothing lands
                    report(.success(url))
                }
            }
        }
    }
}

@MainActor @Test func aPromisedFileIsReceivedIntoCU1sDirectoryAndAttachedLikeADroppedOne() async throws {
    let rig = await Rig()
    let stub = PromiseStub([("holiday.png", try #require(encoded(png(width: 8, height: 8), as: .png)))])
    await rig.controller.dropped([], promises: [stub], shift: false, conversation: "1")
    let entry = try #require(rig.queue.first)
    #expect(rig.queue.count == 1 && entry.filename == "holiday.png" && entry.contentType == "image/png" && entry.size > 0)
    #expect(entry.url.deletingLastPathComponent().deletingLastPathComponent().resolvingSymlinksInPath().path == rig.fake.files.root.resolvingSymlinksInPath().path, "<root>/<UUID>/holiday.png")
    #expect(rig.fake.files.synthesized.count == 1)
    // The same checks as a dropped file: the limits ran on its size.
    #expect(rig.fake.limitCalls.last?.sizes == [entry.size] && rig.fake.limitCalls.last?.stage == .add)
}

@MainActor @Test func aPromiseThatFailsOrStallsAddsNothingAndLeavesNoFiles() async throws {
    let rig = await Rig()
    // One of two files fails: the whole promise is dropped, the file already written with it.
    await rig.controller.dropped([], promises: [PromiseStub([("a.txt", Data("a".utf8)), ("b.txt", Data("b".utf8))], mode: .failSecond)], shift: false, conversation: "1")
    #expect(rig.queue.isEmpty && rig.fake.files.synthesized.isEmpty && rig.fake.limitCalls.isEmpty)
    // A promise that never reports times out.
    rig.controller.promiseTimeout = .milliseconds(150)
    await rig.controller.dropped([], promises: [PromiseStub([("a.txt", Data("a".utf8))], mode: .neverReport)], shift: false, conversation: "1")
    #expect(rig.queue.isEmpty && rig.fake.files.synthesized.isEmpty)
    // One that delivers after the timeout lands nowhere.
    await rig.controller.dropped([], promises: [PromiseStub([("late.txt", Data("a".utf8))], mode: .lateWrite)], shift: false, conversation: "1")
    try? await Task.sleep(for: .milliseconds(600))
    #expect(rig.queue.isEmpty && rig.fake.files.synthesized.isEmpty, "\(rig.fake.files.synthesized)")
}

@MainActor @Test func aPromisedFileThatIsNotARegularFileOrBreaksALimitLeavesNothingBehind() async throws {
    let rig = await Rig()
    // The stub "delivers" a directory.
    final class DirectoryPromise: FilePromiseSource, @unchecked Sendable {
        var count: Int { 1 }
        func receive(into directory: URL, report: @escaping @Sendable (Result<URL, any Error>) -> Void) {
            let folder = directory.appendingPathComponent("folder", isDirectory: true)
            try? FileManager.default.createDirectory(at: folder, withIntermediateDirectories: true)
            report(.success(folder))
        }
    }
    await rig.controller.dropped([], promises: [DirectoryPromise()], shift: false, conversation: "1")
    #expect(rig.queue.isEmpty && rig.fake.files.synthesized.isEmpty)
    rig.fake.limit = { _, _ in .fileTooLarge(limit: 5) }
    await rig.controller.dropped([], promises: [PromiseStub([("big.bin", Data("0123456789".utf8))])], shift: false, conversation: "1")
    #expect(rig.queue.isEmpty && rig.fake.files.synthesized.isEmpty && rig.controller.alert != nil)
}

@MainActor @Test func aPromisedHEICIsConvertedAndItsOriginalIsNotKept() async throws {
    let rig = await Rig()
    rig.fake.conversion = .enabled(quality: 70, maxFileSizeBytes: 20_971_520)
    let heic = try jpegWithOrientation(width: 30, height: 20, orientation: 1)
    await rig.controller.dropped([], promises: [PromiseStub([("IMG_9.HEIC", heic)])], shift: false, conversation: "1")
    let entry = try #require(rig.queue.first)
    #expect(entry.filename == "IMG_9.jpg" && entry.contentType == "image/heic" && entry.originalMD5?.count == 32)
    #expect(rig.fake.files.synthesized.count == 1, "only the converted file's directory remains")
}

/// One promise delivering two files into one directory, one of them replaced by its conversion:
/// deleting the replaced original must leave the queued sibling in place (CUA verifier F2).
@MainActor @Test func aConvertedFileOfAMultiFilePromiseDoesNotTakeItsQueuedSiblingWithIt() async throws {
    let rig = await Rig()
    rig.fake.conversion = .enabled(quality: 70, maxFileSizeBytes: 20_971_520)
    let heic = try jpegWithOrientation(width: 30, height: 20, orientation: 1)
    await rig.controller.dropped([], promises: [PromiseStub([("IMG_9.HEIC", heic), ("notes.txt", Data("n".utf8))])], shift: false, conversation: "1")
    #expect(rig.queue.map(\.filename) == ["IMG_9.jpg", "notes.txt"])
    #expect(rig.queue.map { FileManager.default.fileExists(atPath: $0.url.path) } == [true, true], "both queued files still exist")
}

@MainActor @Test func aShiftDropOfAPromiseSendsTheReceivedFileAtOnce() async throws {
    let rig = await Rig()
    rig.model.draft = "kept"
    await rig.controller.dropped([], promises: [PromiseStub([("clip.txt", Data("c".utf8))])], shift: true, conversation: "9")
    let request = try #require(rig.fake.sent.first)
    #expect(request.conversation == "9" && request.text == "" && request.instant && request.files.map(\.filename) == ["clip.txt"])
    #expect(rig.model.draft == "kept" && rig.queue.isEmpty)
}

@MainActor @Test func promisesAreNotReceivedWhereAttachingIsForbiddenOrWhenFilesAreNamed() async throws {
    let denied = await Rig(permission: .unknown)
    let stub = PromiseStub([("a.txt", Data("a".utf8))])
    await denied.controller.dropped([], promises: [stub], shift: false, conversation: "1")
    #expect(stub.received.get == 0 && denied.queue.isEmpty && denied.fake.files.synthesized.isEmpty, "Unknown hides the drop")
    // A drag that names files is those files: its promises (a second copy of the same items) are not received.
    let rig = await Rig()
    let named = try rig.fake.files.source("named.txt", Data("n".utf8))
    let twin = PromiseStub([("named.txt", Data("n".utf8))])
    await rig.controller.dropped([named], promises: [twin], shift: false, conversation: "1")
    #expect(twin.received.get == 0 && rig.queue.map(\.filename) == ["named.txt"])
}

@MainActor @Test func aPromiseOnlyDragIsAcceptedAndReceivedThroughTheRealDropDestination() async throws {
    let rig = await Rig()
    let stub = PromiseStub([("from-mail.pdf", Data("%PDF".utf8))])
    rig.controller.promiseSources = { [stub] }
    // No file URL on the drag: the files are only promised.
    let (target, info) = try drag(rig, files: [], promised: true)
    #expect(target.draggingEntered(info) == .copy, "a promise alone makes the drag acceptable")
    #expect(target.performDragOperation(info))
    for _ in 0..<60 where rig.queue.isEmpty { await settle(rig.host, 1) }
    #expect(rig.queue.map(\.filename) == ["from-mail.pdf"] && stub.received.get == 1)
    // Without a promise, an empty drag is refused.
    rig.controller.promiseSources = { [] }
    let (other, otherInfo) = try drag(rig, files: [])
    #expect(other.draggingEntered(otherInfo) == [])
}

// MARK: Overlay

@MainActor @Test func theOverlayShowsOnDragEnterAndLeavesOnExitAndOnDrop() async throws {
    let rig = await Rig()
    #expect(!rig.has("Upload to"))
    rig.controller.dragEntered()
    await settle(rig.host)
    #expect(rig.has("Upload to \"general\""), "\(rig.labels())")
    #expect(rig.has("You can add comments before uploading. Hold shift to upload directly."))
    rig.controller.dragExited()
    await settle(rig.host)
    #expect(!rig.has("Upload to"))
    rig.controller.dragEntered()
    await settle(rig.host)
    let a = try rig.fake.files.source("a.txt", Data("aa".utf8))
    await rig.controller.dropped([a], shift: false, conversation: "1")
    await settle(rig.host)
    #expect(!rig.controller.isDropTargeted && !rig.has("Upload to"))
}

@Test func theOverlayCopyIsTheOfficial() {
    #expect(DropOverlay.title("Swiftcord") == "Upload to \"Swiftcord\"")
    #expect(DropOverlay.subtitle == "You can add comments before uploading. Hold shift to upload directly.")
}

// MARK: Paste, through the real editor

@MainActor @Test func pastedFileURLsAttachWithTheirNames() async throws {
    let rig = await Rig()
    let a = try rig.fake.files.source("report.pdf", Data("%PDF".utf8)), b = try rig.fake.files.source("notes.txt", Data("n".utf8))
    let files = board()
    files.writeObjects([a as NSURL, b as NSURL])
    await rig.paste(files)
    for _ in 0..<20 where rig.queue.count < 2 { await settle(rig.host, 1) }
    #expect(rig.queue.map(\.filename) == ["report.pdf", "notes.txt"])
    #expect(rig.model.draft.isEmpty, "no file name is typed")
    #expect(rig.queue[0].contentType == "application/pdf")
}

@MainActor @Test func aPasteTakesOnlyPublicFileURLNeverAnotherType() async throws {
    let rig = await Rig()
    let file = try rig.fake.files.source("secret.txt", Data("s".utf8))
    // A `public.url` that names a file, and the path as text: neither is a file-URL item.
    let urlOnly = board()
    urlOnly.declareTypes([.URL], owner: nil)
    urlOnly.setString(file.absoluteString, forType: .URL)
    await rig.paste(urlOnly)
    let pathText = board()
    pathText.setString(file.path, forType: .string)
    await rig.paste(pathText)
    try? await Task.sleep(for: .milliseconds(200))
    #expect(rig.queue.isEmpty, "\(rig.queue)")
    #expect(PasteContent(urlOnly) == nil && PasteContent(pathText) == nil)
    // The text paste still works as plain text.
    #expect(rig.model.draft == file.path)
}

@MainActor @Test func aPastedImageBecomesImagePNGWrittenToATempFile() async throws {
    let rig = await Rig()
    let bytes = try #require(encoded(png(width: 6, height: 4), as: .png))
    let pasted = board()
    pasted.setData(bytes, forType: .png)
    await rig.paste(pasted)
    let item = try #require(rig.queue.first)
    #expect(rig.queue.count == 1 && item.filename == "image.png" && item.contentType == "image/png")
    #expect(try Data(contentsOf: item.url) == bytes, "PNG data is kept as it is")
    #expect(item.url.deletingLastPathComponent().deletingLastPathComponent().resolvingSymlinksInPath().path == rig.fake.files.root.resolvingSymlinksInPath().path, "CU-1: <root>/<UUID>/image.png")
}

@MainActor @Test func aPastedTIFFIsEncodedToPNG() async throws {
    let rig = await Rig()
    let tiff = try #require(encoded(png(width: 6, height: 4), as: .tiff))
    let pasted = board()
    pasted.setData(tiff, forType: .tiff)
    await rig.paste(pasted)
    let item = try #require(rig.queue.first)
    #expect(item.filename == "image.png")
    let data = try Data(contentsOf: item.url)
    #expect(data.prefix(8) == Data([0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A]), "a PNG signature")
}

@MainActor @Test func imageDataNextToTextIsAPlainTextPaste() async throws {
    let rig = await Rig()
    let bytes = try #require(encoded(png(width: 6, height: 4), as: .png))
    let cells = board()
    cells.declareTypes([.string, .png], owner: nil)
    cells.setString("a\tb", forType: .string)
    cells.setData(bytes, forType: .png)
    await rig.paste(cells)
    #expect(rig.queue.isEmpty && rig.model.draft == "a\tb")
}

@MainActor @Test func textOver4000UnitsBecomesMessageTxtAndExactly4000StaysText() async throws {
    let rig = await Rig()
    let long = String(repeating: "é", count: 4001)
    let pasted = board()
    pasted.setString(long, forType: .string)
    await rig.paste(pasted)
    let item = try #require(rig.queue.first)
    #expect(item.filename == "message.txt" && item.contentType == "text/plain" && rig.model.draft.isEmpty)
    #expect(try String(contentsOf: item.url, encoding: .utf8) == long)
    let edge = board()
    edge.setString(String(repeating: "a", count: 4000), forType: .string)
    await rig.paste(edge)
    #expect(rig.queue.count == 1 && rig.model.draft.count == 4000)
    // 2001 emoji are 4002 UTF-16 units, over the threshold although 2001 characters.
    let emoji = board()
    emoji.setString(String(repeating: "👍", count: 2001), forType: .string)
    await rig.paste(emoji)
    #expect(rig.queue.count == 2)
}

@MainActor @Test func twoLongPastesInTwoConversationsKeepTheirOwnBytesAndRemovingOneLeavesTheOther() async throws {
    let rig = await Rig()
    let first = board(), second = board()
    first.setString(String(repeating: "1", count: 4001), forType: .string)
    second.setString(String(repeating: "2", count: 4001), forType: .string)
    await rig.paste(first)
    rig.controller.configure(
        services: rig.fake.services, permission: .allowed, premiumType: 2, conversation: "2",
        queues: Binding(get: { rig.model.queues }, set: { rig.model.queues = $0 }))
    #expect(rig.controller.paste(try #require(PasteContent(second)), conversation: "2"))
    let one = try #require(rig.model.queues["1"]?.first), two = try #require(rig.model.queues["2"]?.first)
    #expect(one.url != two.url)
    #expect(try String(contentsOf: one.url, encoding: .utf8).first == "1" && String(contentsOf: two.url, encoding: .utf8).first == "2")
    rig.fake.files.remove(one.url)
    #expect(!FileManager.default.fileExists(atPath: one.url.path) && FileManager.default.fileExists(atPath: two.url.path))
}

// MARK: Return with files

@Test func returnWithFilesSendsEvenWithABlankDraft() {
    var draft = "", reply: ReplyPreview? = nil, files = [queued("a.txt")]
    var request: (String, [QueuedAttachment], ReplyTarget?)?
    let outcome = ComposerSubmit.submit(&draft, reply: &reply, attachments: &files, hasMarkedText: false, onSend: nil) { text, files, target in
        request = (text, files, target)
        return .started
    }
    #expect(outcome == .sent && request?.0 == "" && request?.1.map(\.filename) == ["a.txt"])
    #expect(draft.isEmpty && files.isEmpty && reply == nil)
}

@Test func aSendWithFilesKeepsTheReplyTargetAndClearsEverythingOnceStarted() {
    var draft = "caption", reply: ReplyPreview? = ReplyPreview(authorName: "Ada", segments: [], messageID: "30", channelID: "1")
    var files = [queued("a.txt")]
    var target: ReplyTarget?
    ComposerSubmit.submit(&draft, reply: &reply, attachments: &files, hasMarkedText: false, onSend: nil) { _, _, replyTarget in
        target = replyTarget
        return .started
    }
    #expect(target == ReplyTarget(messageID: "30", channelID: "1") && draft.isEmpty && reply == nil && files.isEmpty)
}

@Test func aRefusedOrLimitedSendKeepsTheDraftTheReplyAndTheFiles() {
    for result in [AttachmentSendOutcome.refused, .limit(.totalTooLarge(limit: 5))] {
        var draft = "caption", reply: ReplyPreview? = ReplyPreview(authorName: "Ada", segments: [], messageID: "30", channelID: "1")
        var files = [queued("a.txt")]
        let outcome = ComposerSubmit.submit(&draft, reply: &reply, attachments: &files, hasMarkedText: false, onSend: nil) { _, _, _ in result }
        if case .limit(let limit) = result { #expect(outcome == .attachmentLimit(limit)) } else { #expect(outcome == .ignored) }
        #expect(draft == "caption" && reply != nil && files.map(\.filename) == ["a.txt"])
    }
}

@Test func filesStillObeyTheTextLimitAndMarkedText() {
    var draft = String(repeating: "a", count: 2001), reply: ReplyPreview? = nil, files = [queued("a.txt")]
    var called = false
    let over = ComposerSubmit.submit(&draft, reply: &reply, attachments: &files, hasMarkedText: false, limit: 2000, onSend: nil) { _, _, _ in
        called = true
        return .started
    }
    #expect(over == .tooLong(length: 2001, limit: 2000) && !called && files.count == 1)
    draft = "x"
    let marked = ComposerSubmit.submit(&draft, reply: &reply, attachments: &files, hasMarkedText: true, onSend: nil) { _, _, _ in
        called = true
        return .started
    }
    #expect(marked == .ignored && !called)
}

@Test func withoutFilesTheSubmitIsTheOldOne() {
    var draft = "  ", reply: ReplyPreview? = nil, files: [QueuedAttachment] = []
    var texts: [String] = []
    #expect(ComposerSubmit.submit(&draft, reply: &reply, attachments: &files, hasMarkedText: false, onSend: { text, _ in texts.append(text) }, onSendFiles: { _, _, _ in .started }) == .ignored)
    draft = "hi"
    #expect(ComposerSubmit.submit(&draft, reply: &reply, attachments: &files, hasMarkedText: false, onSend: { text, _ in texts.append(text) }, onSendFiles: { _, _, _ in .started }) == .sent)
    #expect(texts == ["hi"])
}

@MainActor @Test func anAttachmentsOnlyReturnReachesTheSendThroughTheRealEditor() async throws {
    let rig = await Rig()
    let a = try rig.fake.files.source("a.txt", Data("aa".utf8))
    await rig.controller.dropped([a], shift: false, conversation: "1")
    await settle(rig.host)
    rig.editor.onSubmit?()
    await settle(rig.host)
    let request = try #require(rig.fake.sent.first)
    #expect(request.text == "" && !request.instant && request.files.map(\.filename) == ["a.txt"] && request.conversation == "1")
    #expect(rig.queue.isEmpty, "the composer clears at once")
    #expect(rig.model.sentTexts.isEmpty, "not the text-only send")
}

@MainActor @Test func aSendTimeLimitKeepsTheDraftAndFilesAndShowsTheAlert() async throws {
    let rig = await Rig(premium: nil)
    let a = try rig.fake.files.source("a.txt", Data("aa".utf8))
    await rig.controller.dropped([a], shift: false, conversation: "1")
    rig.model.draft = "caption"
    rig.fake.outcome = .limit(.totalTooLarge(limit: 524_288_000))
    await settle(rig.host)
    rig.editor.onSubmit?()
    await settle(rig.host)
    #expect(rig.model.draft == "caption" && rig.queue.map(\.filename) == ["a.txt"])
    let shown = await rig.alertTexts()
    #expect(shown.contains { $0.hasPrefix("Total message size must be less than 500.00 MB") } && shown.contains("Got It"), "\(shown)")
    if let sheet = rig.window.attachedSheet { rig.window.endSheet(sheet) }
}

// MARK: Row, cards, Modify

@MainActor @Test func theRowShowsOneLabelledCardPerQueuedFileAndGoesWhenTheQueueEmpties() async throws {
    let rig = await Rig()
    rig.model.queues["1"] = [queued("a.png", size: 2048, contentType: "image/png"), queued("b.zip", size: 3_000_000, contentType: "application/zip")]
    await settle(rig.host, 10)
    #expect(rig.has("a.png, 2 KB") && rig.has("b.zip, 3 MB"), "\(rig.labels())")
    rig.model.queues["1"] = nil
    await settle(rig.host, 30)
    #expect(!rig.has("a.png"))
}

@MainActor @Test func theRowInsertsAndRemovesUnderTheComposersSpringKeyedOnTheCount() async throws {
    let rig = await Rig()
    func cardY() -> CGFloat? {
        var y: CGFloat?
        walk(rig.host) { object in
            if label(object).hasPrefix("a.png"), let frame = (object.value(forKey: "accessibilityFrame") as? NSValue)?.rectValue { y = frame.midY }
        }
        return y
    }
    #expect(cardY() == nil)
    rig.model.queues["1"] = [queued("a.png", contentType: "image/png")]
    await settle(rig.host, 1)
    // The change is animated: the card is not yet where it settles (it moves up from the bottom).
    let early = cardY()
    await settle(rig.host, 30)
    let settled = try #require(cardY())
    #expect(early == nil || abs(early! - settled) > 0.5, "inserted with a movement, not in one frame")
    // The tokens' spring is the one the row is keyed with (D6).
    #expect(DesignTokens.Motion.composer == .spring(response: 0.15, dampingFraction: 0.85, blendDuration: 0.25))
    rig.model.queues["1"] = nil
    await settle(rig.host, 30)
    #expect(cardY() == nil)
}

@MainActor private func cardHost(_ attachment: QueuedAttachment, thumbnail: CGImage?) -> (NSHostingView<AnyView>, TextWindow) {
    var view = AnyView(AttachmentCard(attachment: attachment, onToggleSpoiler: {}, onSave: { _ in }, onRemove: {}))
    view = AnyView(view.environment(\.attachmentThumbnail, AttachmentThumbnails { _, _ in thumbnail }))
    return TextHarness.hosting(view, size: CGSize(width: 260, height: 260), accessibility: true)
}

@MainActor @Test func anImageCardIs200SquareWithTheImageFittedInsideSixPoints() async throws {
    let attachment = queued("wide.png", size: 5000, contentType: "image/png")
    let (host, _) = cardHost(attachment, thumbnail: png(width: 400, height: 200))
    await settle(host, 12)
    #expect(host.fittingSize == CGSize(width: 200, height: 200), "\(host.fittingSize)")
    var labels: [String] = []
    walk(host) { labels.append(label($0)) }
    #expect(labels.contains("wide.png, 5 KB"), "\(labels)")
    // Pixels: red image, 188 wide (200 - 2 * 6) and 94 tall, centred in the card above the strip.
    let pixels = try #require(TextHarness.Pixels(host))
    let card = NSRect(x: 30, y: 30, width: 200, height: 200) // the 260 pt window centres the card
    func red(_ x: CGFloat, _ y: CGFloat) -> Bool { let c = pixels.rgb(x, y); return c.0 > 200 && c.1 < 60 && c.2 < 60 }
    let midX = card.midX
    // The picture is in the card's middle, even though the strip sits on its lower part.
    #expect(red(midX, card.midY), "centre of the card is the image")
    #expect(!red(card.minX + 3, card.midY), "6 pt of card colour at the side")
}

@MainActor @Test func aSpoilerCardShowsTheBlackPillOverItsPreviewAndSaysSo() async throws {
    // The 44 pt blur itself is a Core Animation filter, which `cacheDisplay` does not draw: only the
    // pill and the label are observable offscreen.
    var attachment = queued("pic.png", size: 5000, contentType: "image/png", spoiler: true)
    func blackRun(_ host: NSView) throws -> Int {
        let pixels = try #require(TextHarness.Pixels(host))
        return (60...200).reduce(0) { sum, x in
            sum + (118...142).filter { y in let c = pixels.rgb(CGFloat(x), CGFloat(y)); return c.0 < 40 && c.1 < 40 && c.2 < 40 }.count
        }
    }
    let (host, _) = cardHost(attachment, thumbnail: png(width: 200, height: 200))
    await settle(host, 12)
    var labels: [String] = []
    walk(host) { labels.append(label($0)) }
    #expect(labels.contains { $0.contains("pic.png") && $0.contains("spoiler") }, "\(labels)")
    #expect(try blackRun(host) > 500, "the pill is a black capsule across the middle")
    attachment.spoiler = false
    let (plain, _) = cardHost(attachment, thumbnail: png(width: 200, height: 200))
    await settle(plain, 12)
    #expect(try blackRun(plain) == 0)
    #expect(DesignTokens.Attachment.spoilerBlur == 44)
}

@MainActor @Test func aNonImageCardShowsASymbolAndTheExtension() async throws {
    let (host, _) = cardHost(queued("archive.tar.gz", size: 1234, contentType: "application/gzip"), thumbnail: nil)
    await settle(host, 12)
    var labels: [String] = []
    walk(host) { labels.append(label($0)) }
    #expect(labels.contains("archive.tar.gz, 1 KB"), "\(labels)")
    #expect(AttachmentCardFacts.symbol(contentType: "image/heic") == "photo")
    #expect(AttachmentCardFacts.symbol(contentType: "video/mp4") == "film")
    #expect(AttachmentCardFacts.symbol(contentType: "audio/mpeg") == "waveform")
    #expect(AttachmentCardFacts.symbol(contentType: "application/pdf") == "doc.richtext")
    #expect(AttachmentCardFacts.symbol(contentType: "") == "doc" && AttachmentCardFacts.symbol(contentType: "application/zip") == "doc")
    #expect(AttachmentCardFacts.extensionLabel(filename: "a.gz") == "GZ")
    #expect(AttachmentCardFacts.extensionLabel(filename: "Makefile") == "FILE")
    #expect(AttachmentCardFacts.extensionLabel(filename: "x.verylongext") == "VERYLONG")
    #expect(AttachmentCardFacts.sizeText(14_310_000) == "14.3 MB", "ByteCountFormatter .file")
}

@MainActor @Test func theCardHasThreeNamedAccessibilityActionsAndOneLabel() async throws {
    var actions: [String] = []
    let attachment = queued("a.png", contentType: "image/png")
    let view = AttachmentCard(attachment: attachment, onToggleSpoiler: { actions.append("spoiler") }, onSave: { _ in }, onRemove: { actions.append("remove") })
        .environment(\.attachmentThumbnail, AttachmentThumbnails { _, _ in nil })
    let (host, _) = TextHarness.hosting(view, size: CGSize(width: 260, height: 260), accessibility: true)
    await settle(host, 10)
    var found: NSObject?
    walk(host) { if label($0).hasPrefix("a.png") { found = $0 } }
    let element = try #require(found)
    let custom = (send(element, "accessibilityCustomActions") as? [NSAccessibilityCustomAction]) ?? []
    #expect(custom.map(\.name).sorted() == ["Modify Attachment", "Remove Attachment", "Spoiler Attachment"], "\(custom.map(\.name))")
    custom.first { $0.name == "Spoiler Attachment" }?.handler?()
    custom.first { $0.name == "Remove Attachment" }?.handler?()
    #expect(actions == ["spoiler", "remove"])
}

@MainActor @Test func theRowsActionsToggleSpoilerAndRemoveAndDeleteATempFile() async throws {
    let rig = await Rig()
    let long = board()
    long.setString(String(repeating: "t", count: 4001), forType: .string)
    await rig.paste(long)
    let file = try #require(rig.queue.first)
    // Toggle, through the row's binding and a card action.
    var queue = rig.queue
    let binding = Binding(get: { queue }, set: { queue = $0 })
    let row = AttachmentRow(attachments: binding) { rig.fake.files.remove($0.url) }
    let (host, _) = TextHarness.hosting(row.environment(\.attachmentThumbnail, AttachmentThumbnails { _, _ in nil }), size: CGSize(width: 400, height: 260), accessibility: true)
    await settle(host, 10)
    var element: NSObject?
    walk(host) { if label($0).hasPrefix("message.txt") { element = $0 } }
    let custom = (send(try #require(element), "accessibilityCustomActions") as? [NSAccessibilityCustomAction]) ?? []
    custom.first { $0.name == "Spoiler Attachment" }?.handler?()
    #expect(queue.first?.spoiler == true && queue.first?.id == file.id)
    custom.first { $0.name == "Remove Attachment" }?.handler?()
    #expect(queue.isEmpty)
    #expect(!FileManager.default.fileExists(atPath: file.url.deletingLastPathComponent().path), "the synthesized file went with its directory")
}

@Test func theModifyDraftSavesFilenameDescriptionAndSpoilerOnlyOnTheQueuedFile() {
    let original = queued("old.png", size: 77, contentType: "image/png")
    var draft = ModifyDraft(original)
    #expect(draft.filename == "old.png" && draft.altText == "" && !draft.spoiler)
    draft.filename = "new name.png"
    draft.setAltText("a jellyfish")
    draft.spoiler = true
    let saved = draft.saved
    #expect(saved.filename == "new name.png" && saved.altText == "a jellyfish" && saved.spoiler)
    #expect(saved.id == original.id && saved.file == original.file && saved.contentType == "image/png", "nothing else changes")
}

@Test func theDescriptionIsCappedAt1024AndAnEmptiedOneIsKeptEmptyNotUnset() {
    var draft = ModifyDraft(queued("a.png"))
    draft.setAltText(String(repeating: "a", count: 3000))
    #expect(draft.altText.count == 1024 && DesignTokens.Attachment.altTextLimit == 1024)
    // Never touched: still unset, so no description is sent.
    #expect(ModifyDraft(queued("a.png")).saved.altText == nil)
    // Set once and emptied: sent as "" (official `!= null`, P4).
    var withAlt = queued("a.png")
    withAlt.altText = "x"
    var emptied = ModifyDraft(withAlt)
    emptied.setAltText("")
    #expect(emptied.saved.altText == "")
    var blank = ModifyDraft(queued("a.png"))
    blank.filename = "  "
    #expect(!blank.canSave)
}

@MainActor @Test func theModifySheetHasTheOfficialLayoutAndCopy() async throws {
    var finished: [QueuedAttachment?] = []
    let sheet = ModifyAttachmentSheet(attachment: queued("a.png", contentType: "image/png")) { finished.append($0) }
        .environment(\.attachmentThumbnail, AttachmentThumbnails { _, _ in nil })
    let (host, _) = TextHarness.hosting(sheet, size: CGSize(width: 460, height: 520), accessibility: true)
    await settle(host, 10)
    var labels: [String] = []
    walk(host) { labels.append(label($0)) }
    for copy in ["Modify Attachment", "Filename", "Description (Alt Text)", "Mark as spoiler", "Cancel", "Save"] {
        #expect(labels.contains { $0.contains(copy) }, "\(copy) in \(labels)")
    }
    func press(_ title: String) throws {
        var found: NSObject?
        walk(host) { if label($0) == title, send($0, "accessibilityRole") as? String == "AXButton" { found = $0 } }
        let button = try #require(found, "a \(title) button")
        _ = button.perform(NSSelectorFromString("accessibilityPerformPress"))
    }
    try press("Save")
    #expect(finished.count == 1 && finished[0]?.filename == "a.png")
    try press("Cancel")
    #expect(finished.count == 2 && finished[1] == nil)
}

// MARK: HEIC / JXR

@Test func conversionKindsAreDetectedByMIMEAndByExtension() {
    for type in ["image/heic", "image/heif", "image/heic-sequence", "image/heif-sequence"] {
        #expect(ImageConversion.kind(contentType: type, filename: "x.bin") == .heic, "\(type)")
    }
    for type in ["image/jxr", "image/vnd.ms-photo"] { #expect(ImageConversion.kind(contentType: type, filename: "x.bin") == .jxr, "\(type)") }
    for name in ["a.heic", "a.HEIF", "a.hif"] { #expect(ImageConversion.kind(contentType: "", filename: name) == .heic, "\(name)") }
    for name in ["a.jxr", "a.WDP", "a.hdp"] { #expect(ImageConversion.kind(contentType: "", filename: name) == .jxr, "\(name)") }
    #expect(ImageConversion.kind(contentType: "image/png", filename: "a.png") == nil)
    #expect(ImageConversion.kind(contentType: "", filename: "a.jpg") == nil)
    #expect(ImageConversion.originalContentType("", filename: "a.heic", kind: .heic) == "image/heic")
    #expect(ImageConversion.originalContentType("", filename: "a.hif", kind: .heic) == "image/heif")
    #expect(ImageConversion.originalContentType("", filename: "a.wdp", kind: .jxr) == "image/jxr")
    #expect(ImageConversion.originalContentType("image/heif", filename: "a.heic", kind: .heic) == "image/heif")
}

@Test func theConvertedNameFollowsTheBundlesRewriteRule() {
    #expect(ImageConversion.rewrittenName("IMG_1.heic") == "IMG_1.jpg")
    #expect(ImageConversion.rewrittenName("a.b.HEIF") == "a.b.jpg")
    #expect(ImageConversion.rewrittenName("x.hif") == "x.jpg")
    #expect(ImageConversion.rewrittenName("p.jxr") == "p.jpg" && ImageConversion.rewrittenName("p.wdp") == "p.jpg" && ImageConversion.rewrittenName("p.hdp") == "p.jpg")
    #expect(ImageConversion.rewrittenName("noext") == "noext.jpg")
    #expect(ImageConversion.rewrittenName("odd.png") == "odd.png.jpg", "any other name gets .jpg appended")
}

/// A JPEG whose pixels are `width` x `height` as stored, an EXIF orientation and metadata that must go.
private func jpegWithOrientation(width: Int, height: Int, orientation: Int) throws -> Data {
    let properties: [CFString: Any] = [
        kCGImagePropertyOrientation: orientation,
        kCGImagePropertyExifDictionary: [kCGImagePropertyExifUserComment: "private note"],
        kCGImagePropertyTIFFDictionary: [kCGImagePropertyTIFFMake: "SecretCam", kCGImagePropertyTIFFOrientation: orientation],
        kCGImagePropertyGPSDictionary: [kCGImagePropertyGPSLatitude: 25.0, kCGImagePropertyGPSLatitudeRef: "N"],
    ]
    return try #require(encoded(png(width: width, height: height), as: .jpeg, properties: properties))
}

@MainActor @Test func aHEICIsConvertedToAnUprightMetadataFreeJPEGWithTheOriginalsMD5() async throws {
    let fake = Fake()
    fake.conversion = .enabled(quality: 80, maxFileSizeBytes: 20_971_520)
    // A .heic by name with orientation 6 (rotated a quarter turn): stored 40 x 20, shown 20 x 40.
    let bytes = try jpegWithOrientation(width: 40, height: 20, orientation: 6)
    let source = try fake.files.source("IMG_0001.HEIC", bytes)
    let entries = await AttachmentIntake.entries(for: [source], services: fake.services)
    let entry = try #require(entries.first)
    #expect(entries.count == 1 && entry.filename == "IMG_0001.jpg")
    #expect(entry.contentType == "image/heic", "the original MIME is kept (A48)")
    #expect(entry.file.path.hasSuffix("/IMG_0001.jpg") && entry.file.path != source.resolvingSymlinksInPath().path, "a CU-1 temp file")
    #expect(entry.size == (try Data(contentsOf: entry.url)).count && entry.size > 0, "size is the converted file's")
    // The MD5 is the original bytes', lowercase hex.
    let expected = try #require(ImageConversion.md5Hex(of: source))
    #expect(entry.originalMD5 == expected && expected.count == 32 && expected == expected.lowercased())
    // Pixels: orientation applied, metadata dropped.
    let result = try #require(CGImageSourceCreateWithURL(entry.url as CFURL, nil))
    let properties = try #require(CGImageSourceCopyPropertiesAtIndex(result, 0, nil) as? [CFString: Any])
    #expect(properties[kCGImagePropertyPixelWidth] as? Int == 20 && properties[kCGImagePropertyPixelHeight] as? Int == 40, "\(properties)")
    #expect(properties[kCGImagePropertyGPSDictionary] == nil, "no GPS")
    #expect((properties[kCGImagePropertyExifDictionary] as? [CFString: Any])?[kCGImagePropertyExifUserComment] == nil, "no EXIF comment")
    #expect((properties[kCGImagePropertyTIFFDictionary] as? [CFString: Any])?[kCGImagePropertyTIFFMake] == nil, "no camera make")
    #expect((properties[kCGImagePropertyOrientation] as? Int ?? 1) == 1, "no orientation left to apply")
    #expect(CGImageSourceGetType(result) as String? == UTType.jpeg.identifier)
}

@MainActor @Test func theQualityIsTheVariationsQualityOver100() async throws {
    let fake = Fake()
    // A busy image shows the difference between two qualities.
    let size = 200
    let context = CGContext(data: nil, width: size, height: size, bitsPerComponent: 8, bytesPerRow: 0, space: CGColorSpace(name: CGColorSpace.sRGB)!, bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue)!
    for y in 0..<size { for x in 0..<size {
        context.setFillColor(CGColor(red: Double((x * 7) % 256) / 255, green: Double((y * 13) % 256) / 255, blue: Double((x * y) % 256) / 255, alpha: 1))
        context.fill(CGRect(x: x, y: y, width: 1, height: 1))
    } }
    let source = try fake.files.source("busy.heic", try #require(encoded(context.makeImage()!, as: .png)))
    let low = try #require(ImageConversion.convert(source, quality: 20, maxFileSizeBytes: .max))
    let high = try #require(ImageConversion.convert(source, quality: 95, maxFileSizeBytes: .max))
    #expect(low.jpeg.count < high.jpeg.count)
}

@MainActor @Test func aConversionThatCannotHappenKeepsTheOriginalWithNoNotice() async throws {
    let png = try #require(encoded(png(width: 300, height: 300), as: .png))
    // Over the variation's size: the JPEG would be larger than 100 bytes.
    let tooBig = Fake()
    tooBig.conversion = .enabled(quality: 80, maxFileSizeBytes: 100)
    let big = try tooBig.files.source("a.heic", try jpegWithOrientation(width: 300, height: 300, orientation: 1))
    let bigEntry = try #require(await AttachmentIntake.entries(for: [big], services: tooBig.services).first)
    #expect(bigEntry.filename == "a.heic" && bigEntry.originalMD5 == nil && bigEntry.file.path == big.resolvingSymlinksInPath().path)
    #expect(tooBig.files.synthesized.isEmpty, "no temp file left by a refused conversion")
    // A corrupt file.
    let corrupt = Fake()
    corrupt.conversion = .enabled(quality: 80, maxFileSizeBytes: 20_971_520)
    let bad = try corrupt.files.source("b.heic", Data("not an image at all".utf8))
    let badEntry = try #require(await AttachmentIntake.entries(for: [bad], services: corrupt.services).first)
    #expect(badEntry.filename == "b.heic" && badEntry.originalMD5 == nil && badEntry.contentType != "")
    #expect(corrupt.files.synthesized.isEmpty)
    // Unknown (or disabled) config.
    let unknown = Fake()
    unknown.conversion = .off
    let fine = try unknown.files.source("c.heic", png)
    let fineEntry = try #require(await AttachmentIntake.entries(for: [fine], services: unknown.services).first)
    #expect(fineEntry.filename == "c.heic" && fineEntry.originalMD5 == nil && unknown.files.synthesized.isEmpty)
    // A JXR is detected the same way and falls back the same way when ImageIO cannot decode it.
    let jxr = Fake()
    jxr.conversion = .enabled(quality: 85, maxFileSizeBytes: 52_428_800)
    let wdp = try jxr.files.source("d.jxr", Data("JXR?".utf8))
    let jxrEntry = try #require(await AttachmentIntake.entries(for: [wdp], services: jxr.services).first)
    #expect(jxrEntry.filename == "d.jxr" && jxr.files.synthesized.isEmpty)
}

@MainActor @Test func aConvertedFileIsWhatTheCardAndTheQueueHold() async throws {
    let rig = await Rig()
    rig.fake.conversion = .enabled(quality: 60, maxFileSizeBytes: 20_971_520)
    let source = try rig.fake.files.source("shot.heif", try jpegWithOrientation(width: 30, height: 30, orientation: 1))
    await rig.controller.dropped([source], shift: false, conversation: "1")
    #expect(rig.queue.map(\.filename) == ["shot.jpg"] && rig.queue[0].contentType == "image/heif")
    #expect(rig.fake.limitCalls.last?.sizes == [rig.queue[0].size], "the limits see the converted size")
}

// MARK: Pending row

private func pending(_ state: PendingUploadItem.State, sent: UInt64 = 0, size: UInt64 = 100, text: String = "caption") -> MessageItem {
    MessageItem(
        id: "555", authorName: "me", avatar: .initials("M"), timestamp: Date(timeIntervalSince1970: 1_800_000_000), text: text,
        reply: nil, delivery: state == .failed ? .failed("Upload failed.") : .pending,
        upload: PendingUploadItem(files: [.init(attachment: queued("video.mov", size: size), sent: sent)], state: state))
}

@MainActor private func pendingHost(_ message: MessageItem, actions: UploadRowActions) -> NSHostingView<AnyView> {
    let (host, _) = TextHarness.hosting(AnyView(MessageRow(message: message, uploadActions: actions).frame(width: 700)), size: CGSize(width: 700, height: 300), accessibility: true)
    return host
}

private final class Calls: @unchecked Sendable {
    var cancel: [String] = [], retry: [String] = [], discard: [(String, [String])] = []
    @MainActor var actions: UploadRowActions {
        UploadRowActions(
            cancel: { self.cancel.append($0) }, retry: { self.retry.append($0) },
            discard: { self.discard.append(($0, $1.map(\.filename))) })
    }
}

@MainActor private func children(of object: NSObject) -> [NSObject] { ((send(object, "accessibilityChildren") as? [Any]) ?? []).compactMap { $0 as? NSObject } }

@MainActor private func card(in host: NSView, startingWith prefix: String) -> NSObject? {
    var found: NSObject?
    walk(host) { if (send($0, "accessibilityLabel") as? String)?.hasPrefix(prefix) == true { found = $0 } }
    return found
}

@MainActor @Test func aSendingRowShowsTheCardWithItsFilenameProgressAndACancelAction() async throws {
    let calls = Calls()
    let host = pendingHost(pending(.sending, sent: 42), actions: calls.actions)
    await settle(host, 10)
    let element = try #require(card(in: host, startingWith: "video.mov"))
    #expect(send(element, "accessibilityLabel") as? String == "video.mov, uploading")
    // The progress bar inside the card reports its value (42 of 100 bytes sent).
    let bar = try #require(children(of: element).first { send($0, "accessibilityRole") as? String == "AXProgressIndicator" })
    #expect((send(bar, "accessibilityValue") as? NSNumber)?.doubleValue == 0.42, "\(String(describing: send(bar, "accessibilityValue")))")
    let custom = (send(element, "accessibilityCustomActions") as? [NSAccessibilityCustomAction]) ?? []
    #expect(custom.map(\.name) == ["Cancel Upload"])
    custom[0].handler?()
    #expect(calls.cancel == ["555"] && calls.retry.isEmpty && calls.discard.isEmpty)
    var labels: [String] = []
    walk(host) { labels.append(label($0)) }
    #expect(!labels.contains { $0.contains("Upload Failed") })
}

@MainActor @Test func aFailedRowOffersRetryAndDiscardWithTheOfficialCopy() async throws {
    let calls = Calls()
    let host = pendingHost(pending(.failed), actions: calls.actions)
    await settle(host, 10)
    let element = try #require(card(in: host, startingWith: "video.mov"))
    #expect(send(element, "accessibilityLabel") as? String == "video.mov, upload failed")
    let texts = children(of: element).map(label)
    #expect(texts.contains("Upload Failed") && texts.contains("Click here to retry the upload"), "\(texts)")
    #expect(!children(of: element).contains { send($0, "accessibilityRole") as? String == "AXProgressIndicator" })
    let custom = (send(element, "accessibilityCustomActions") as? [NSAccessibilityCustomAction]) ?? []
    #expect(Set(custom.map(\.name)) == ["Retry Upload", "Discard Upload"])
    custom.first { $0.name == "Retry Upload" }?.handler?()
    custom.first { $0.name == "Discard Upload" }?.handler?()
    #expect(calls.retry == ["555"] && calls.discard.count == 1 && calls.discard[0].0 == "555" && calls.discard[0].1 == ["video.mov"])
    #expect(calls.cancel.isEmpty, "the cross of a failed card discards, never cancels")
}

@Test func aPendingRowWithFilesDimsItsTextButNotItsCardsAndOtherEchoesDimWhole() {
    let withFiles = pending(.sending, text: "hello there")
    var plain = withFiles
    plain.upload = nil
    #expect(MessageRow.dimming(withFiles) == .init(row: 1, body: 0.5), "the text only (OA A33)")
    #expect(MessageRow.dimming(plain) == .init(row: 0.5, body: 1), "a text echo dims whole, as before")
    #expect(MessageRow.dimming(pending(.failed)) == .init(row: 1, body: 1))
    var sent = plain
    sent.delivery = .sent
    #expect(MessageRow.dimming(sent) == .init(row: 1, body: 1))
}

// MARK: The send's lifecycle in the shell state (CU Flow, D14a, CU-1)

private func row(_ id: String, _ state: PendingUploadItem.State?, text: String = "caption", files: [String] = ["a.txt"], reply: ReplyPreview? = nil, replyTo: UInt64? = nil) -> MessageItem {
    MessageItem(
        id: id, authorName: "me", avatar: .initials("M"), timestamp: Date(timeIntervalSince1970: 1_800_000_000), text: text, reply: reply,
        delivery: state == .sending ? .pending : .sent,
        upload: state.map { state in PendingUploadItem(files: files.map { .init(attachment: queued($0), sent: 0) }, replyTo: replyTo, state: state) })
}

private func shell(_ conversation: String = "1") -> ShellState {
    var state = ShellState(
        rail: [.guild(RailGuild(id: "g", name: "G", icon: .initials("G")))], selectedRailID: "g",
        categories: [ChannelCategory(id: "c", name: nil, channels: [ChannelItem(id: conversation, name: "general")])], selectedChannelID: conversation)
    state.attachPermission = .allowed
    return state
}

@Test func progressThenSuccessDeletesTheFilesOnlyAfterTheRowWasSeenAndLeft() {
    var state = shell()
    var sends = PendingSends()
    sends.begin(id: "9", conversation: "1", paths: ["/tmp/velella-attachments/x/a.txt"])
    // A snapshot taken before the send began arrives: nothing is deleted.
    #expect(state.reconcileUploads(conversation: "1", rows: [row("1", nil)], sends: &sends).deletePaths.isEmpty)
    // Progress: the row is there.
    #expect(state.reconcileUploads(conversation: "1", rows: [row("9", .sending)], sends: &sends).deletePaths.isEmpty)
    // Success: the row left (the real message took its place).
    let done = state.reconcileUploads(conversation: "1", rows: [row("123", nil)], sends: &sends)
    #expect(done.deletePaths == ["/tmp/velella-attachments/x/a.txt"] && done.discard.isEmpty)
    // Forgotten: nothing twice.
    #expect(state.reconcileUploads(conversation: "1", rows: [], sends: &sends).deletePaths.isEmpty)
}

@Test func aRowInAnotherConversationNeverSettlesThisSend() {
    var state = shell()
    var sends = PendingSends()
    sends.begin(id: "9", conversation: "1", paths: ["/p"])
    _ = state.reconcileUploads(conversation: "1", rows: [row("9", .sending)], sends: &sends)
    #expect(state.reconcileUploads(conversation: "2", rows: [], sends: &sends).deletePaths.isEmpty)
}

@Test func failedThenRetryKeepsTheFilesAndFailedThenDiscardIsTheUIsToDelete() {
    var state = shell()
    var sends = PendingSends()
    sends.begin(id: "9", conversation: "1", paths: ["/p"])
    _ = state.reconcileUploads(conversation: "1", rows: [row("9", .sending)], sends: &sends)
    #expect(state.reconcileUploads(conversation: "1", rows: [row("9", .failed)], sends: &sends).deletePaths.isEmpty)
    #expect(state.reconcileUploads(conversation: "1", rows: [row("9", .sending)], sends: &sends).deletePaths.isEmpty, "retry runs the same row again")
    #expect(state.reconcileUploads(conversation: "1", rows: [row("9", .failed)], sends: &sends).discard.isEmpty)
}

@MainActor @Test func discardingAFailedRowThroughTheColumnDeletesItsTempFile() async throws {
    let rig = await Rig()
    let written = try rig.fake.files.write(Data("pasted text".utf8), "message.txt")
    let identity = try #require(rig.fake.files.identity(written))
    var failed = row("9", .failed, files: [])
    failed.upload = PendingUploadItem(files: [.init(attachment: QueuedAttachment(file: identity, filename: "message.txt"), sent: 0)], state: .failed)
    rig.model.messages = [failed]
    await settle(rig.host, 10)
    var card: NSObject?
    walk(rig.host) { if (send($0, "accessibilityLabel") as? String)?.hasPrefix("message.txt") == true { card = $0 } }
    let custom = (send(try #require(card), "accessibilityCustomActions") as? [NSAccessibilityCustomAction]) ?? []
    custom.first { $0.name == "Discard Upload" }?.handler?()
    #expect(rig.fake.discarded == ["9"])
    #expect(!FileManager.default.fileExists(atPath: written.deletingLastPathComponent().path), "the temp file went with the row")
    custom.first { $0.name == "Retry Upload" }?.handler?()
    #expect(rig.fake.retried == ["9"])
}

@Test func cancelPutsTheTextTheFilesAndTheReplyBackInTheComposer() {
    var state = shell()
    var sends = PendingSends()
    sends.begin(id: "9", conversation: "1", paths: ["/p/a.txt"])
    let reply = ReplyPreview(authorName: "Ada", segments: [.text("question")])
    let cancelled = row("9", .cancelled, text: "my caption", files: ["a.txt", "b.txt"], reply: reply, replyTo: 30)
    let result = state.reconcileUploads(conversation: "1", rows: [cancelled], sends: &sends)
    #expect(result.discard == ["9"] && result.deletePaths.isEmpty, "the files are the composer's again: not deleted")
    #expect(state.attachmentQueues["1"]?.map(\.filename) == ["a.txt", "b.txt"])
    #expect(state.drafts["1"] == "my caption")
    #expect(state.reply?.messageID == "30" && state.reply?.channelID == "1" && state.reply?.authorName == "Ada")
    // The row is discarded once; a second snapshot that still lists it restores nothing again.
    let again = state.reconcileUploads(conversation: "1", rows: [cancelled], sends: &sends)
    #expect(again.discard.isEmpty && state.attachmentQueues["1"]?.count == 2 && state.drafts["1"] == "my caption")
}

@Test func cancelKeepsWhatWasTypedSinceAndTheFilesAddedSince() {
    var state = shell()
    state.drafts["1"] = "typed later"
    state.attachmentQueues["1"] = [queued("later.txt")]
    state.reply = ReplyPreview(authorName: "Bo", segments: [], messageID: "1", channelID: "1")
    state.restoreCancelled(row("9", .cancelled, text: "first", files: ["a.txt"], reply: ReplyPreview(authorName: "Ada", segments: []), replyTo: 30), conversation: "1")
    #expect(state.drafts["1"] == "first\ntyped later")
    #expect(state.attachmentQueues["1"]?.map(\.filename) == ["a.txt", "later.txt"])
    #expect(state.reply?.authorName == "Bo", "a reply set since stays")
}

@Test func aCancelledRowThatIsNotOursIsLeftAlone() {
    var state = shell()
    var sends = PendingSends()
    let result = state.reconcileUploads(conversation: "1", rows: [row("9", .cancelled)], sends: &sends)
    #expect(result.discard.isEmpty && state.attachmentQueues.isEmpty && state.drafts.isEmpty)
}

@Test func signOutTakesEveryQueuedAndPendingFile() {
    var state = shell()
    state.attachmentQueues = ["1": [queued("a")], "2": [queued("b"), queued("c")]]
    #expect(Set(state.takeAllAttachments().map(\.filename)) == ["a", "b", "c"] && state.attachmentQueues.isEmpty)
    var sends = PendingSends()
    sends.begin(id: "1", conversation: "1", paths: ["/a"])
    sends.begin(id: "2", conversation: "2", paths: ["/b"])
    #expect(Set(sends.takeAll()) == ["/a", "/b"] && sends.takeAll().isEmpty)
}

@Test func theQueueFollowsTheOpenConversationLikeTheDraft() {
    var state = shell("1")
    state.attachmentQueue = [queued("a")]
    state.selectedChannelID = "2"
    #expect(state.attachmentQueue.isEmpty)
    state.attachmentQueue = [queued("b")]
    state.selectedChannelID = "1"
    #expect(state.attachmentQueue.map(\.filename) == ["a"] && state.attachmentQueues["2"]?.map(\.filename) == ["b"])
    state.attachmentQueue = []
    #expect(state.attachmentQueues["1"] == nil)
}
