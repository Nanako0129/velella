import Foundation
import VelellaFFI

// Core's transport on URLSession (plan NET, D13, D15). Core composes every request (URL, headers,
// deadline, body cap) and keeps every protection above the trait; this file only moves bytes,
// with these rules:
// - One ephemeral session: no cookie storage or cookies, no URL cache, no credential storage;
//   the system proxy and the system's and user's trusted roots apply, as in Safari (D15).
// - The URL is sent exactly as core passed it: a URL the platform would change by a byte, or one
//   off the allow-list, is refused before anything is sent (T-10). An Authorization goes only to
//   `https://discord.com/api/v9/`; a request elsewhere carrying one is refused.
// - Redirects are never followed, for HTTP or the WebSocket upgrade; the 3xx goes back to core.
// - Each request's deadline is enforced here too, and cancellation from core cancels the task
//   (T-4). Bodies stream: the declared length is checked first, and the task is cancelled at
//   cap + 1 bytes of decoded body (T-5).
// - WebSocket: the message-size limit is core's cap; a close code is reported only when a close
//   frame arrived, otherwise the connection is lost (T-6).
// - Nothing here logs or formats a request, header, body or frame (T-2, T-16).
// - Uploads (composer plan CU): a separate operation for the upload host's preflight and PUT,
//   checked against the upload-only allow-list and Safari's closed header names; the body
//   streams from a file opened once and verified against its attach-time identity (CU-6). It is
//   a data task with a body stream: an upload task adds `Upload-Draft-Interop-Version: 6` and
//   `Upload-Complete: ?1` (measured on macOS 27, CFNetwork 3896.100, 2026-10-17), which Safari's
//   XHR never sends (CU-4). CFNetwork orders the headers itself: the composed order does not
//   survive to the wire.

/// The app's one transport, shared by every session.
public final class URLSessionTransport: Transport, @unchecked Sendable {
    // @unchecked: every stored property is immutable and thread-safe (`URLSession`, the delegate
    // guards its own state).

    public static let shared = URLSessionTransport()

    let allowList: AllowList
    private let delegate = SessionDelegate()
    private let session: URLSession
    let files = OpenFiles()

    public convenience init() { self.init(allowList: .shipped) }

    /// Tests pass a list for their local server; the app always uses `.shipped`.
    init(allowList: AllowList) {
        self.allowList = allowList
        let config = URLSessionConfiguration.ephemeral
        config.httpCookieStorage = nil
        config.httpCookieAcceptPolicy = .never
        config.httpShouldSetCookies = false
        config.urlCredentialStorage = nil
        config.urlCache = nil
        config.requestCachePolicy = .reloadIgnoringLocalCacheData
        session = URLSession(configuration: config, delegate: delegate, delegateQueue: nil)
    }

    // MARK: Foreign trait

    public func http(request: HttpRequest, completion: HttpCompletion) -> TransportTask {
        TaskHandle(Task { [self] in
            switch await perform(request) {
            case .response(let status, let headers, let body, let overCap):
                completion.succeed(status: status, headers: headers, body: body, bodyOverCap: overCap)
            case .failure(let kind):
                completion.fail(kind: kind)
            }
        })
    }

    public func websocket(request: WebSocketRequest, completion: WebSocketCompletion) -> TransportTask {
        TaskHandle(Task { [self] in
            switch await open(request) {
            case .opened(let socket): completion.opened(connection: socket)
            case .rejected(let status): completion.rejected(status: status)
            case .failure(let kind): completion.failed(kind: kind)
            }
        })
    }

    public func openFile(file: UploadFile) -> UInt64? {
        files.open(file)
    }

    public func closeFile(id: UInt64) {
        files.close(id)
    }

    public func upload(request: UploadRequest, progress: UploadProgress, completion: HttpCompletion) -> TransportTask {
        TaskHandle(Task { [self] in
            switch await performUpload(request, progress: progress) {
            case .response(let status, let headers, let body, let overCap):
                completion.succeed(status: status, headers: headers, body: body, bodyOverCap: overCap)
            case .failure(let kind):
                completion.fail(kind: kind)
            }
        })
    }

    // MARK: HTTP

    /// One exchange. Cancelling the calling task cancels it.
    func perform(_ request: HttpRequest) async -> HTTPOutcome {
        let target = request.url()
        guard let url = URL(string: target), url.absoluteString == target, allowList.allowsHTTP(url) else {
            return .failure(.rejected)
        }
        let deadline = Double(request.deadlineMs()) / 1000
        var built = URLRequest(url: url, cachePolicy: .reloadIgnoringLocalCacheData, timeoutInterval: deadline)
        built.httpMethod = request.method()
        built.httpShouldHandleCookies = false
        for header in request.headers() {
            built.setValue(header.value, forHTTPHeaderField: header.name)
        }
        if let authorization = request.takeAuthorization() {
            guard allowList.authorizes(target) else { return .failure(.rejected) }
            built.setValue(authorization, forHTTPHeaderField: "Authorization")
        }
        built.httpBody = request.body()
        let task = session.dataTask(with: built)
        let exchange = Exchange(cap: Int(clamping: request.bodyCap()))
        return await run(task, exchange: exchange, deadline: deadline) {
            await withCheckedContinuation { exchange.start($0) }
        }
    }

    // MARK: Upload

    /// One upload exchange: the URL exactly as given and on the upload-only list for its method,
    /// only Safari's header names (each once), the body from the opened file, redirects refused,
    /// the response through the request's cap. Cancelling the calling task cancels it.
    func performUpload(_ request: UploadRequest, progress: UploadProgress) async -> HTTPOutcome {
        let target = request.url()
        let method = request.method()
        guard let url = URL(string: target), url.absoluteString == target,
              allowList.allowsUpload(url, method: method) else { return .failure(.rejected) }
        let headers = request.headers()
        let names = headers.map { $0.name.lowercased() }
        let allowed = method == "OPTIONS" ? UploadHeaders.preflight : UploadHeaders.put
        guard names.allSatisfy(allowed.contains), Set(names).count == names.count else { return .failure(.rejected) }
        let deadline = Double(request.deadlineMs()) / 1000
        var built = URLRequest(url: url, cachePolicy: .reloadIgnoringLocalCacheData, timeoutInterval: deadline)
        built.httpMethod = method
        built.httpShouldHandleCookies = false
        for header in headers {
            built.setValue(header.value, forHTTPHeaderField: header.name)
        }
        if let id = request.file() {
            // The declared length is the streamed length, or the request is never sent.
            let declared = headers.first { $0.name.lowercased() == "content-length" }?.value
            guard method == "PUT", declared == String(request.bodyLength()), let fd = files.descriptor(id),
                  let stream = FileBody.stream(fd: fd, start: request.bodyStart(), length: request.bodyLength())
            else { return .failure(.rejected) }
            built.httpBodyStream = stream
        } else if method == "PUT" {
            built.httpBody = Data()
        }
        let task = session.dataTask(with: built)
        let exchange = Exchange(cap: Int(clamping: request.bodyCap())) { progress.sent(bytes: UInt64(clamping: $0)) }
        return await run(task, exchange: exchange, deadline: deadline) {
            await withCheckedContinuation { exchange.start($0) }
        }
    }

    // MARK: WebSocket

    func open(_ request: WebSocketRequest) async -> OpenOutcome {
        let target = request.url()
        guard let url = URL(string: target), url.absoluteString == target, allowList.allowsWebSocket(url) else {
            return .failure(.rejected)
        }
        let deadline = Double(request.deadlineMs()) / 1000
        var built = URLRequest(url: url, cachePolicy: .reloadIgnoringLocalCacheData, timeoutInterval: deadline)
        built.httpShouldHandleCookies = false
        for header in request.headers() {
            built.setValue(header.value, forHTTPHeaderField: header.name)
        }
        let task = session.webSocketTask(with: built)
        task.maximumMessageSize = Int(clamping: request.maxMessageSize())
        let upgrade = Upgrade()
        return await run(task, exchange: upgrade, deadline: deadline) {
            await withCheckedContinuation { upgrade.start($0) }
        }
    }

    /// Registers `task` with the delegate, resumes it, and waits for `body`'s outcome; tears the
    /// task down on cancellation or when `deadline` passes.
    private func run<T>(
        _ task: URLSessionTask, exchange: some TaskState, deadline: TimeInterval,
        _ body: @escaping @Sendable () async -> T
    ) async -> T {
        delegate.register(task, exchange)
        return await withTaskCancellationHandler {
            async let outcome = body()
            task.resume()
            let timer = Task {
                try await Task.sleep(for: .seconds(deadline))
                exchange.deadlinePassed()
                task.cancel()
            }
            let result = await outcome
            timer.cancel()
            return result
        } onCancel: {
            task.cancel()
        }
    }
}

/// Cancels the Swift task behind one foreign call.
private final class TaskHandle: TransportTask, @unchecked Sendable {
    let task: Task<Void, Never>
    init(_ task: Task<Void, Never>) { self.task = task }
    func cancel() { task.cancel() }
}

// MARK: Outcomes

enum HTTPOutcome: Sendable {
    /// Any status, a 3xx included. `overCap`: the body passed the cap and was dropped.
    case response(status: UInt16, headers: [HttpHeader], body: Data, overCap: Bool)
    case failure(TransportErrorKind)
}

enum ReceivedMessage: Sendable {
    case text(String)
    case binary(Data)
    /// A close frame with this code arrived.
    case closed(UInt16)
    /// The connection ended without a close frame, or failed.
    case lost
}

enum OpenOutcome: Sendable {
    case opened(WebSocketSocket)
    /// The upgrade was answered with this status instead of 101.
    case rejected(UInt16)
    case failure(TransportErrorKind)
}

/// The closed mapping of T-3. Only errors raised before a request byte can have been written are
/// `connectFailed`: core retries a send on nothing else (D11).
func transportErrorKind(_ error: Error) -> TransportErrorKind {
    guard let error = error as? URLError else { return .other }
    switch error.code {
    case .cannotFindHost, .cannotConnectToHost, .dnsLookupFailed, .notConnectedToInternet,
         .secureConnectionFailed, .serverCertificateHasBadDate, .serverCertificateUntrusted,
         .serverCertificateHasUnknownRoot, .serverCertificateNotYetValid, .clientCertificateRejected,
         .clientCertificateRequired:
        return .connectFailed
    case .timedOut: return .timeout
    case .cancelled: return .cancelled
    default: return .other
    }
}

// MARK: Per-task state

/// What the session delegate tracks for one task.
protocol TaskState: AnyObject, Sendable {
    func deadlinePassed()
    func completed(_ task: URLSessionTask, error: Error?)
}

/// One HTTP exchange: status and headers, the body up to the cap, the outcome once.
final class Exchange: TaskState, @unchecked Sendable {
    // @unchecked: all mutable state is behind `lock`.
    private let lock = NSLock()
    private let cap: Int
    private var continuation: CheckedContinuation<HTTPOutcome, Never>?
    private var pending: HTTPOutcome?
    private var status: UInt16 = 0
    private var headers: [HttpHeader] = []
    private var body = Data()
    private var overCap = false
    private var timedOut = false
    /// The bytes of the request body sent so far, for an upload.
    private let onSent: (@Sendable (Int64) -> Void)?

    init(cap: Int, onSent: (@Sendable (Int64) -> Void)? = nil) {
        self.cap = cap
        self.onSent = onSent
    }

    func sent(_ total: Int64) { onSent?(total) }

    func start(_ continuation: CheckedContinuation<HTTPOutcome, Never>) {
        let ready: HTTPOutcome? = lock.withLock {
            if let pending { return pending }
            self.continuation = continuation
            return nil
        }
        if let ready { continuation.resume(returning: ready) }
    }

    private func finish(_ outcome: HTTPOutcome) {
        let waiting: CheckedContinuation<HTTPOutcome, Never>? = lock.withLock {
            guard pending == nil else { return nil }
            pending = outcome
            defer { continuation = nil }
            return continuation
        }
        waiting?.resume(returning: outcome)
    }

    /// False when the declared length is already over the cap.
    func received(_ response: URLResponse) -> Bool {
        lock.withLock {
            let http = response as? HTTPURLResponse
            status = UInt16(clamping: http?.statusCode ?? 0)
            headers = (http?.allHeaderFields ?? [:]).compactMap { key, value in
                guard let name = key as? String, let value = value as? String else { return nil }
                return HttpHeader(name: name, value: value)
            }
            if response.expectedContentLength > Int64(cap) { overCap = true }
            return !overCap
        }
    }

    /// False once the body passed the cap; the caller cancels the task.
    func received(_ data: Data) -> Bool {
        lock.withLock {
            guard !overCap else { return false }
            if body.count + data.count > cap {
                overCap = true
                body = Data()
                return false
            }
            body.append(data)
            return true
        }
    }

    func deadlinePassed() { lock.withLock { timedOut = true } }

    func completed(_ task: URLSessionTask, error: Error?) {
        let outcome: HTTPOutcome = lock.withLock {
            if overCap { return .response(status: status, headers: headers, body: Data(), overCap: true) }
            if let error {
                return .failure(timedOut && (error as? URLError)?.code == .cancelled ? .timeout : transportErrorKind(error))
            }
            return .response(status: status, headers: headers, body: body, overCap: false)
        }
        finish(outcome)
    }
}

/// One WebSocket upgrade until it opens or fails; then the open socket's close-frame record.
final class Upgrade: TaskState, @unchecked Sendable {
    // @unchecked: all mutable state is behind `lock`.
    private let lock = NSLock()
    private var continuation: CheckedContinuation<OpenOutcome, Never>?
    private var pending: OpenOutcome?
    private var timedOut = false
    /// Set by `didCloseWith` when a close frame arrived.
    private var closeCode: UInt16?

    func start(_ continuation: CheckedContinuation<OpenOutcome, Never>) {
        let ready: OpenOutcome? = lock.withLock {
            if let pending { return pending }
            self.continuation = continuation
            return nil
        }
        if let ready { continuation.resume(returning: ready) }
    }

    private func finish(_ outcome: OpenOutcome) {
        let waiting: CheckedContinuation<OpenOutcome, Never>? = lock.withLock {
            guard pending == nil else { return nil }
            pending = outcome
            defer { continuation = nil }
            return continuation
        }
        waiting?.resume(returning: outcome)
    }

    func opened(_ task: URLSessionWebSocketTask) { finish(.opened(WebSocketSocket(task: task, upgrade: self))) }

    func closed(with code: URLSessionWebSocketTask.CloseCode) {
        guard code != .invalid else { return }
        lock.withLock { closeCode = UInt16(clamping: code.rawValue) }
    }

    var receivedCloseCode: UInt16? { lock.withLock { closeCode } }

    func deadlinePassed() { lock.withLock { timedOut = true } }

    func completed(_ task: URLSessionTask, error: Error?) {
        let timedOut = lock.withLock { self.timedOut }
        if let status = (task.response as? HTTPURLResponse)?.statusCode, status != 101 {
            finish(.rejected(UInt16(clamping: status)))
        } else if let error {
            finish(.failure(timedOut && (error as? URLError)?.code == .cancelled ? .timeout : transportErrorKind(error)))
        } else {
            finish(.failure(.other))
        }
    }
}

/// One open WebSocket, as core's `WebSocketConnection`.
final class WebSocketSocket: WebSocketConnection, @unchecked Sendable {
    // @unchecked: immutable references to thread-safe objects; `closedByClient` is behind `lock`.
    private let task: URLSessionWebSocketTask
    private let upgrade: Upgrade
    private let lock = NSLock()
    private var closedByClient = false

    init(task: URLSessionWebSocketTask, upgrade: Upgrade) {
        self.task = task
        self.upgrade = upgrade
    }

    func send(frame: WebSocketFrame, completion: SendCompletion) {
        let message: URLSessionWebSocketTask.Message
        if let text = frame.text() {
            message = .string(text)
        } else {
            message = .data(frame.binary() ?? Data())
        }
        send(message) { failure in
            if let failure { completion.failed(kind: failure) } else { completion.sent() }
        }
    }

    func send(_ message: URLSessionWebSocketTask.Message, _ handler: @escaping @Sendable (TransportErrorKind?) -> Void) {
        task.send(message) { error in handler(error.map(transportErrorKind)) }
    }

    func receive(completion: ReceiveCompletion) {
        receive { message in
            switch message {
            case .text(let text): completion.text(text: text)
            case .binary(let data): completion.binary(data: data)
            case .closed(let code): completion.closed(code: code)
            case .lost: completion.lost()
            }
        }
    }

    /// One message, or the end. A close code is reported only for a close frame the server sent
    /// (T-6). URLSession fails the pending receive before it calls `didCloseWith`, so at that
    /// moment the delegate's record is usually still empty while the task's `closeCode` is
    /// already set from the received frame; the record is used when it did arrive first. The
    /// task's `closeCode` also holds a code the client sent itself, so once `close(code:)` ran the
    /// end is reported as lost, never as that code. (A server close frame racing the client's own
    /// close is then reported as lost too; core is already closing.)
    func receive(_ handler: @escaping @Sendable (ReceivedMessage) -> Void) {
        task.receive { [self] result in
            switch result {
            case .success(.string(let text)): handler(.text(text))
            case .success(.data(let data)): handler(.binary(data))
            case .success: handler(.lost)
            case .failure:
                guard !lock.withLock({ closedByClient }) else { return handler(.lost) }
                let code = upgrade.receivedCloseCode
                    ?? (task.closeCode != .invalid ? UInt16(clamping: task.closeCode.rawValue) : nil)
                handler(code.map(ReceivedMessage.closed) ?? .lost)
            }
        }
    }

    func receive() async -> ReceivedMessage {
        await withCheckedContinuation { continuation in receive { continuation.resume(returning: $0) } }
    }

    func close(code: UInt16) {
        lock.withLock { closedByClient = true }
        task.cancel(with: URLSessionWebSocketTask.CloseCode(rawValue: Int(code)) ?? .normalClosure, reason: nil)
    }
}

// MARK: Upload files and bodies

/// Safari's header names for the upload's preflight and PUT (composer plan D15a, OA A40-A42),
/// lowercased. Any other name refuses the upload before anything is sent (CU-2).
enum UploadHeaders {
    static let preflight: Set<String> = [
        "origin", "sec-fetch-site", "access-control-request-method", "access-control-request-headers",
        "sec-fetch-mode", "user-agent", "referer", "sec-fetch-dest", "content-length", "accept",
        "accept-language", "priority",
    ]
    static let put: Set<String> = [
        "accept", "content-type", "sec-fetch-site", "origin", "content-range", "sec-fetch-mode", "user-agent",
        "referer", "sec-fetch-dest", "content-length", "accept-language", "priority",
    ]
}

/// Files opened for upload, by id (CU-6): opened once without following a link and without
/// blocking (a FIFO swapped in cannot hang the open), then kept only when `fstat` shows a regular
/// file with the identity recorded at attach time.
final class OpenFiles: @unchecked Sendable {
    // @unchecked: all mutable state is behind `lock`.
    private let lock = NSLock()
    private var handles: [UInt64: Int32] = [:]
    private var next: UInt64 = 0

    func open(_ file: UploadFile) -> UInt64? {
        let fd = Darwin.open(file.path(), O_RDONLY | O_NOFOLLOW | O_NONBLOCK | O_CLOEXEC)
        guard fd >= 0 else { return nil }
        var st = stat()
        guard fstat(fd, &st) == 0, FileIdentity(st) == FileIdentity(file) else {
            Darwin.close(fd)
            return nil
        }
        return lock.withLock {
            next += 1
            handles[next] = fd
            return next
        }
    }

    func descriptor(_ id: UInt64) -> Int32? { lock.withLock { handles[id] } }

    func close(_ id: UInt64) {
        if let fd = lock.withLock({ handles.removeValue(forKey: id) }) { Darwin.close(fd) }
    }

    /// Ids still open (tests).
    var count: Int { lock.withLock { handles.count } }
}

/// (device, inode, size, mtime) of a regular file, as recorded at attach and compared at send.
struct FileIdentity: Equatable {
    let device: UInt64, inode: UInt64, size: UInt64, mtimeNs: Int64

    /// nil unless `st` is a regular file.
    init?(_ st: stat) {
        guard st.st_mode & S_IFMT == S_IFREG else { return nil }
        device = UInt64(UInt32(bitPattern: st.st_dev))
        inode = st.st_ino
        size = UInt64(clamping: st.st_size)
        mtimeNs = Int64(st.st_mtimespec.tv_sec) * 1_000_000_000 + Int64(st.st_mtimespec.tv_nsec)
    }

    init(_ file: UploadFile) {
        device = file.device()
        inode = file.inode()
        size = file.size()
        mtimeNs = file.mtimeNs()
    }
}

/// The body of one upload PUT: `length` bytes of an opened file from `start`, read in 64 KiB
/// pieces on a thread of its own into a bound stream pair, so neither the file nor the body is
/// ever in memory whole. The thread reads its own duplicate of the descriptor, so core closing
/// the file after a cancel cannot hand it another file's descriptor.
final class FileBody: @unchecked Sendable {
    // @unchecked: `output` and `fd` are used by the writer thread only, after `init`.
    static let piece = 64 << 10
    private let output: OutputStream
    private let fd: Int32
    private let start: UInt64
    private let length: UInt64

    private init(output: OutputStream, fd: Int32, start: UInt64, length: UInt64) {
        self.output = output
        self.fd = fd
        self.start = start
        self.length = length
    }

    static func stream(fd: Int32, start: UInt64, length: UInt64) -> InputStream? {
        let own = dup(fd)
        guard own >= 0 else { return nil }
        var input: InputStream?
        var output: OutputStream?
        Stream.getBoundStreams(withBufferSize: piece, inputStream: &input, outputStream: &output)
        guard let input, let output else {
            Darwin.close(own)
            return nil
        }
        let body = FileBody(output: output, fd: own, start: start, length: length)
        Thread { body.write() }.start()
        return input
    }

    /// Ends early when the reader goes away (the task finished or was cancelled) or the file is
    /// shorter than promised (the host then sees a short body and the PUT fails).
    private func write() {
        defer {
            output.close()
            Darwin.close(fd)
        }
        output.open()
        var buffer = [UInt8](repeating: 0, count: Self.piece)
        var offset = start
        var left = length
        while left > 0 {
            let want = Int(min(UInt64(Self.piece), left))
            let n = buffer.withUnsafeMutableBytes { pread(fd, $0.baseAddress, want, off_t(offset)) }
            guard n > 0 else { return }
            var written = 0
            while written < n {
                let w = buffer.withUnsafeBufferPointer { output.write($0.baseAddress! + written, maxLength: n - written) }
                guard w > 0 else { return }
                written += w
            }
            offset += UInt64(n)
            left -= UInt64(n)
        }
    }
}

// MARK: Session delegate

/// Routes URLSession's callbacks to each task's state.
private final class SessionDelegate: NSObject, URLSessionDataDelegate, URLSessionWebSocketDelegate, @unchecked Sendable {
    // @unchecked: `states` is behind `lock`.
    private let lock = NSLock()
    private var states: [Int: any TaskState] = [:]

    func register(_ task: URLSessionTask, _ state: any TaskState) {
        lock.withLock { states[task.taskIdentifier] = state }
    }

    private func state(_ task: URLSessionTask) -> (any TaskState)? {
        lock.withLock { states[task.taskIdentifier] }
    }

    /// Never follows: the 3xx becomes the response.
    func urlSession(
        _ session: URLSession, task: URLSessionTask, willPerformHTTPRedirection response: HTTPURLResponse,
        newRequest request: URLRequest, completionHandler: @escaping (URLRequest?) -> Void
    ) {
        completionHandler(nil)
    }

    func urlSession(
        _ session: URLSession, dataTask: URLSessionDataTask, didReceive response: URLResponse,
        completionHandler: @escaping (URLSession.ResponseDisposition) -> Void
    ) {
        let within = (state(dataTask) as? Exchange)?.received(response) ?? false
        completionHandler(within ? .allow : .cancel)
    }

    func urlSession(_ session: URLSession, dataTask: URLSessionDataTask, didReceive data: Data) {
        if (state(dataTask) as? Exchange)?.received(data) == false { dataTask.cancel() }
    }

    func urlSession(
        _ session: URLSession, task: URLSessionTask, didSendBodyData bytesSent: Int64, totalBytesSent: Int64,
        totalBytesExpectedToSend: Int64
    ) {
        (state(task) as? Exchange)?.sent(totalBytesSent)
    }

    func urlSession(_ session: URLSession, webSocketTask: URLSessionWebSocketTask, didOpenWithProtocol protocol: String?) {
        (state(webSocketTask) as? Upgrade)?.opened(webSocketTask)
    }

    func urlSession(
        _ session: URLSession, webSocketTask: URLSessionWebSocketTask,
        didCloseWith closeCode: URLSessionWebSocketTask.CloseCode, reason: Data?
    ) {
        (state(webSocketTask) as? Upgrade)?.closed(with: closeCode)
    }

    func urlSession(_ session: URLSession, task: URLSessionTask, didCompleteWithError error: Error?) {
        let state = lock.withLock { states.removeValue(forKey: task.taskIdentifier) }
        state?.completed(task, error: error)
    }

    /// No task metrics are collected.
    func urlSession(_ session: URLSession, task: URLSessionTask, didFinishCollecting metrics: URLSessionTaskMetrics) {}
}

// MARK: Redaction (T-2, T-16)

extension HTTPOutcome: CustomStringConvertible, CustomDebugStringConvertible, CustomReflectable {
    var description: String {
        switch self {
        case .response(let status, _, _, let overCap): "HTTPOutcome(\(status), overCap: \(overCap), [REDACTED])"
        case .failure(let kind): "HTTPOutcome(\(kind))"
        }
    }
    var debugDescription: String { description }
    var customMirror: Mirror { Mirror(self, children: [], displayStyle: .enum) }
}

/// Holds response headers and the body while they arrive.
extension Exchange: CustomStringConvertible, CustomDebugStringConvertible, CustomReflectable {
    var description: String { "Exchange([REDACTED])" }
    var debugDescription: String { description }
    var customMirror: Mirror { Mirror(self, children: [], displayStyle: .class) }
}

extension ReceivedMessage: CustomStringConvertible, CustomDebugStringConvertible, CustomReflectable {
    var description: String {
        switch self {
        case .text(let text): "ReceivedMessage.text(\(text.utf8.count) bytes)"
        case .binary(let data): "ReceivedMessage.binary(\(data.count) bytes)"
        case .closed(let code): "ReceivedMessage.closed(\(code))"
        case .lost: "ReceivedMessage.lost"
        }
    }
    var debugDescription: String { description }
    var customMirror: Mirror { Mirror(self, children: [], displayStyle: .enum) }
}

extension HttpRequest: CustomReflectable {
    public var customMirror: Mirror { Mirror(self, children: [], displayStyle: .class) }
}

extension WebSocketRequest: CustomReflectable {
    public var customMirror: Mirror { Mirror(self, children: [], displayStyle: .class) }
}

extension WebSocketFrame: CustomReflectable {
    public var customMirror: Mirror { Mirror(self, children: [], displayStyle: .class) }
}

extension UploadRequest: CustomReflectable {
    public var customMirror: Mirror { Mirror(self, children: [], displayStyle: .class) }
}

