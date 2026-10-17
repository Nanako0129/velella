import Darwin
import Foundation
import Testing
import VelellaFFI
@testable import VelellaTransport

// Composer plan CU, Swift side: the upload operation against a local server on 127.0.0.1, with a
// test upload-only list for its port. Nothing here contacts the real upload host: the shipped
// list is exercised only on requests it refuses before sending.

let safariUA = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/27.0.1 Safari/605.1.15"

/// The upload-only list for 127.0.0.1 on `port`, any path with a query; nothing else allowed.
func uploadList(_ port: UInt16) -> AllowList {
    AllowList(
        http: [], webSocket: [],
        upload: [AllowList.Rule(scheme: "http", host: .exact("127.0.0.1"), port: Int(port), path: .anyWithQuery)],
        authorizedPrefix: "http://127.0.0.1:\(port)/api/v9/"
    )
}

/// Safari's lists with A43's values (OA A40-A43), as core composes them.
func preflightHeaders(range: Bool) -> [HttpHeader] {
    [
        ("Origin", "https://discord.com"), ("Sec-Fetch-Site", "cross-site"),
        ("Access-Control-Request-Method", "PUT"),
        ("Access-Control-Request-Headers", range ? "content-range,content-type" : "content-type"),
        ("Sec-Fetch-Mode", "cors"), ("User-Agent", safariUA), ("Referer", "https://discord.com/"),
        ("Sec-Fetch-Dest", "empty"), ("Content-Length", "0"), ("Accept", "*/*"),
        ("Accept-Language", "zh-TW,zh-Hant;q=0.9"), ("Priority", "u=3, i"),
    ].map { HttpHeader(name: $0.0, value: $0.1) }
}

func putHeaders(length: UInt64, range: String? = nil) -> [HttpHeader] {
    var list: [(String, String)] = [
        ("Accept", "*/*"), ("Content-Type", "application/octet-stream"), ("Sec-Fetch-Site", "cross-site"),
        ("Origin", "https://discord.com"),
    ]
    if let range { list.append(("Content-Range", range)) }
    list += [
        ("Sec-Fetch-Mode", "cors"), ("User-Agent", safariUA), ("Referer", "https://discord.com/"),
        ("Sec-Fetch-Dest", "empty"), ("Content-Length", String(length)), ("Accept-Language", "zh-TW,zh-Hant;q=0.9"),
        ("Priority", "u=3, i"),
    ]
    return list.map { HttpHeader(name: $0.0, value: $0.1) }
}

func probeHeaders() -> [HttpHeader] {
    [
        ("Accept", "*/*"), ("Content-Type", "application/octet-stream"), ("Origin", "https://discord.com"),
        ("Content-Range", "bytes */*"), ("Sec-Fetch-Site", "cross-site"), ("Sec-Fetch-Mode", "cors"),
        ("User-Agent", safariUA), ("Referer", "https://discord.com/"), ("Sec-Fetch-Dest", "empty"),
        ("Content-Length", "0"), ("Accept-Language", "zh-TW,zh-Hant;q=0.9"), ("Priority", "u=3, i"),
    ].map { HttpHeader(name: $0.0, value: $0.1) }
}

func uploadRequest(
    _ url: String, method: UploadMethod = .put, headers: [HttpHeader], file: UInt64? = nil, start: UInt64 = 0,
    length: UInt64 = 0, deadlineMs: UInt64 = 30_000, cap: UInt64 = 64 << 10
) throws -> UploadRequest {
    try UploadRequest(
        method: method, url: url, headers: headers, file: file, start: start, length: length,
        deadlineMs: deadlineMs, bodyCap: cap
    )
}

/// A file of `count` bytes (`i % 251`) in a fresh temporary directory, written 1 MiB at a time.
func patternFile(_ count: Int, name: String = "a.bin") throws -> URL {
    let dir = FileManager.default.temporaryDirectory.appendingPathComponent("velella-upload-tests-\(UUID().uuidString)")
    try FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
    let url = dir.appendingPathComponent(name)
    FileManager.default.createFile(atPath: url.path, contents: nil)
    let handle = try FileHandle(forWritingTo: url)
    var written = 0
    while written < count {
        let n = min(1 << 20, count - written)
        try handle.write(contentsOf: Data((written..<(written + n)).map { UInt8($0 % 251) }))
        written += n
    }
    try handle.close()
    return url
}

func pattern(_ range: Range<Int>) -> Data { Data(range.map { UInt8($0 % 251) }) }

/// Every header name on the wire, lowercased, in order.
func names(_ head: String) -> [String] {
    head.split(separator: "\r\n").dropFirst().compactMap { $0.split(separator: ":").first?.lowercased() }
}

/// Opens `url` for upload as the app would: the attach-time identity, then the transport's open.
func opened(_ transport: URLSessionTransport, _ url: URL) throws -> UInt64 {
    let identity = try #require(AttachmentFiles.identity(of: url))
    return try #require(transport.openFile(file: identity))
}

let ok204 = httpResponse("200 OK", body: Data("{}".utf8), close: false)

// MARK: Allow-list

@Test func theShippedUploadListIsTheOneHostForOptionsAndPut() {
    let list = AllowList.shipped
    let host = "discord-attachments-uploads-prd.storage.googleapis.com"
    let good = "https://\(host)/upload/storage/v1/b/x/o?uploadType=resumable&upload_id=A"
    #expect(list.allowsUpload(URL(string: good)!, method: "PUT"))
    #expect(list.allowsUpload(URL(string: good)!, method: "OPTIONS"))
    for method in ["GET", "POST", "DELETE", "PATCH", "put"] {
        #expect(!list.allowsUpload(URL(string: good)!, method: method), "\(method)")
    }
    for url in [
        "http://\(host)/u?a=1", "https://\(host):8443/u?a=1", "https://u@\(host)/u?a=1", "https://\(host)/u",
        "https://\(host)/a/../u?a=1", "https://\(host)/a/%2e%2e/u?a=1", "https://\(host).evil.test/u?a=1",
        "https://evil.test/u?a=1", "https://\(host)/u?a=1#f", "https://discord.com/api/v9/x?a=1",
    ] {
        #expect(!list.allowsUpload(URL(string: url)!, method: "PUT"), "\(url)")
    }
    // The upload host is on no other list and never authorized.
    #expect(!list.allowsHTTP(URL(string: good)!))
    #expect(!list.authorizes(good))
}

/// A PUT to the upload host that carries an Authorization can only be a plain request, and the
/// transport refuses it: the host is on the upload list alone. The upload request type has no
/// Authorization (its constructor refuses the header).
@Test func aPUTToTheUploadHostCarryingAuthorizationIsRefused() async throws {
    let good = "https://discord-attachments-uploads-prd.storage.googleapis.com/u?upload_id=A"
    guard case .failure(.rejected) = await URLSessionTransport().perform(try request(good, method: .put, body: Data("x".utf8))) else {
        Issue.record("sent"); return
    }
    #expect(throws: TransportRequestError.self) {
        try uploadRequest(good, headers: [HttpHeader(name: "Authorization", value: marker)])
    }
}

// MARK: Headers on the wire (CU-2, CU-4)

/// Each preflight, PUT, resumed PUT and probe arrives with exactly the composed headers, each
/// once with its value, plus the transport's Host, Accept-Encoding and Connection and nothing
/// else: no Cookie, Authorization, X-* or resumable-upload headers, no chunked encoding.
@Test func uploadRequestsCarryExactlyTheComposedHeaders() async throws {
    let seen = Box<[(head: String, body: Data)]>([])
    let server = try TestServer { peer in
        while let request = peer.readRequest() {
            seen.update { $0.append(request) }
            peer.write(ok204)
        }
    }
    let transport = URLSessionTransport(allowList: uploadList(server.port))
    let url = "http://127.0.0.1:\(server.port)/upload?upload_id=TEST&name=a.bin"
    let fileURL = try patternFile(1000)
    let id = try opened(transport, fileURL)
    let cases: [(UploadMethod, [HttpHeader], UInt64?, UInt64, UInt64)] = [
        (.options, preflightHeaders(range: false), nil, 0, 0),
        (.put, putHeaders(length: 1000), id, 0, 1000),
        (.options, preflightHeaders(range: true), nil, 0, 0),
        (.put, probeHeaders(), nil, 0, 0),
        (.put, putHeaders(length: 400, range: "bytes 600-999/1000"), id, 600, 400),
    ]
    for (method, headers, file, start, length) in cases {
        let outcome = await transport.performUpload(
            try uploadRequest(url, method: method, headers: headers, file: file, start: start, length: length),
            progress: UploadProgress()
        )
        guard case .response(200, _, _, false) = outcome else { Issue.record("\(outcome)"); return }
    }
    transport.closeFile(id: id)
    #expect(seen.get.count == cases.count)
    for ((method, headers, _, start, length), arrived) in zip(cases, seen.get) {
        let line = arrived.head.components(separatedBy: "\r\n")[0]
        #expect(line == "\(method == .options ? "OPTIONS" : "PUT") /upload?upload_id=TEST&name=a.bin HTTP/1.1")
        let wire = names(arrived.head)
        #expect(Set(wire).count == wire.count, "a header twice: \(wire)")
        let composed = headers.map { $0.name.lowercased() }
        #expect(Set(wire) == Set(composed + ["host", "accept-encoding", "connection"]), "\(line): \(wire)")
        for h in headers { #expect(header(h.name, in: arrived.head) == [h.value], "\(line): \(h.name)") }
        #expect(arrived.body == pattern(Int(start)..<Int(start + length)), "\(line): body")
        #expect(!arrived.head.contains("CFNetwork") && !arrived.head.contains("Darwin/"))
    }
}

/// CU-2: a header name outside Safari's list (or one twice) refuses the upload; nothing is sent.
@Test func aHeaderOutsideTheListRefusesTheUpload() async throws {
    let server = try TestServer { peer in
        _ = peer.readRequest()
        peer.write(ok204)
    }
    let transport = URLSessionTransport(allowList: uploadList(server.port))
    let url = "http://127.0.0.1:\(server.port)/u?upload_id=A"
    for extra in ["Cookie", "X-Super-Properties", "X-Discord-Locale", "X-Fingerprint", "Proxy-Authorization"] {
        for (method, base) in [(UploadMethod.put, probeHeaders()), (.options, preflightHeaders(range: false))] {
            let headers = base + [HttpHeader(name: extra, value: "v")]
            let outcome = await transport.performUpload(try uploadRequest(url, method: method, headers: headers), progress: UploadProgress())
            guard case .failure(.rejected) = outcome else { Issue.record("\(extra): \(outcome)"); continue }
        }
    }
    // Content-Range on a preflight, and a name twice.
    for (method, headers) in [
        (UploadMethod.options, preflightHeaders(range: false) + [HttpHeader(name: "Content-Range", value: "bytes */*")]),
        (.put, probeHeaders() + [HttpHeader(name: "accept", value: "*/*")]),
    ] {
        guard case .failure(.rejected) = await transport.performUpload(
            try uploadRequest(url, method: method, headers: headers), progress: UploadProgress()
        ) else { Issue.record("sent"); continue }
    }
    // A method other than OPTIONS or PUT has no constructor case; a URL off the list is refused.
    let off = await URLSessionTransport(allowList: uploadList(server.port &+ 1)).performUpload(
        try uploadRequest(url, headers: probeHeaders()), progress: UploadProgress()
    )
    guard case .failure(.rejected) = off else { Issue.record("\(off)"); return }
    try await Task.sleep(for: .milliseconds(200))
    #expect(server.accepted == 0)
}

// MARK: Streaming, progress, cancel, resume (CU-3)

func footprint() -> UInt64 {
    var info = task_vm_info_data_t()
    var count = mach_msg_type_number_t(MemoryLayout<task_vm_info_data_t>.size / MemoryLayout<natural_t>.size)
    let kr = withUnsafeMutablePointer(to: &info) {
        $0.withMemoryRebound(to: integer_t.self, capacity: Int(count)) {
            task_info(mach_task_self_, task_flavor_t(TASK_VM_INFO), $0, &count)
        }
    }
    return kr == KERN_SUCCESS ? info.phys_footprint : 0
}

/// A 50 MB file streams from its handle: the server checks every byte as it arrives (keeping
/// none), the process footprint stays far below the file's size, and progress rises to the
/// length without passing it.
@Test func aFiftyMegabyteFileStreamsWithBoundedMemory() async throws {
    let size = 50_000_000
    let fileURL = try patternFile(size)
    let result = Box<(count: Int, intact: Bool)>((0, false))
    let server = try TestServer { peer in
        guard let head = peer.readHead() else { return }
        let length = Int(header("content-length", in: head).first ?? "") ?? 0
        var offset = 0
        var intact = true
        let got = peer.drain(length) { piece in
            for (i, byte) in piece.enumerated() where byte != UInt8((offset + i) % 251) { intact = false; break }
            offset += piece.count
        }
        result.update { $0 = (got, intact) }
        peer.write(httpResponse(body: Data("{}".utf8)))
    }
    let transport = URLSessionTransport(allowList: uploadList(server.port))
    let id = try opened(transport, fileURL)
    let progress = UploadProgress()
    let before = footprint()
    let peak = Box<UInt64>(before)
    let sampler = Task {
        while !Task.isCancelled {
            let now = footprint()
            peak.update { $0 = max($0, now) }
            try? await Task.sleep(for: .milliseconds(5))
        }
    }
    let outcome = await transport.performUpload(
        try uploadRequest(
            "http://127.0.0.1:\(server.port)/u?upload_id=A", headers: putHeaders(length: UInt64(size)), file: id,
            length: UInt64(size), deadlineMs: 120_000
        ),
        progress: progress
    )
    sampler.cancel()
    transport.closeFile(id: id)
    guard case .response(200, _, _, false) = outcome else { Issue.record("\(outcome)"); return }
    #expect(result.get.count == size && result.get.intact)
    let growth = peak.get &- before
    #expect(growth < 24 << 20, "footprint grew \(growth >> 20) MiB for a 50 MB body")
    let reports = progress.reports()
    #expect(reports.count > 1)
    #expect(reports == reports.sorted(), "progress went back")
    #expect(reports.last == UInt64(size) && reports.allSatisfy { $0 <= UInt64(size) })
    #expect(transport.files.count == 0)
}

/// A resumed PUT sends exactly bytes start..<size from the opened file, also after the first
/// attempt's connection dropped part-way (core then probes and resumes; here the two PUTs).
@Test func aResumeAfterADroppedConnectionSendsExactlyTheRest() async throws {
    let fileURL = try patternFile(100_000)
    let bodies = Box<[Data]>([])
    let server = try TestServer { peer in
        guard let head = peer.readHead() else { return }
        let length = Int(header("content-length", in: head).first ?? "") ?? 0
        var body = Data()
        if header("content-range", in: head).isEmpty {
            _ = peer.drain(min(length, 60_000)) { body.append($0) }
            bodies.update { $0.append(body) }
            peer.drop()
            return
        }
        _ = peer.drain(length) { body.append($0) }
        bodies.update { $0.append(body) }
        peer.write(httpResponse(body: Data("{}".utf8)))
    }
    let transport = URLSessionTransport(allowList: uploadList(server.port))
    let id = try opened(transport, fileURL)
    let url = "http://127.0.0.1:\(server.port)/u?upload_id=A"
    let first = await transport.performUpload(
        try uploadRequest(url, headers: putHeaders(length: 100_000), file: id, length: 100_000), progress: UploadProgress()
    )
    guard case .failure(let kind) = first, kind != .connectFailed else { Issue.record("\(first)"); return }
    let held = bodies.get.first?.count ?? 0
    let rest = await transport.performUpload(
        try uploadRequest(
            url, headers: putHeaders(length: UInt64(100_000 - held), range: "bytes \(held)-99999/100000"), file: id,
            start: UInt64(held), length: UInt64(100_000 - held)
        ),
        progress: UploadProgress()
    )
    transport.closeFile(id: id)
    guard case .response(200, _, _, false) = rest else { Issue.record("\(rest)"); return }
    #expect(bodies.get.count == 2)
    #expect(bodies.get[0] == pattern(0..<held))
    #expect(bodies.get[1] == pattern(held..<100_000), "the resumed body")
}

/// Cancelling the upload (core dropping its future) tears the connection down server-side.
@Test func cancellingAnUploadTearsItDown() async throws {
    let fileURL = try patternFile(4_000_000)
    let closed = Box<Bool?>(nil)
    let server = try TestServer { peer in
        guard peer.readHead() != nil else { return }
        _ = peer.drain(10_000) { _ in }
        closed.update { $0 = peer.clientClosed(within: 8) }
    }
    let transport = URLSessionTransport(allowList: uploadList(server.port))
    let id = try opened(transport, fileURL)
    let task = Task {
        await transport.performUpload(
            try uploadRequest("http://127.0.0.1:\(server.port)/u?upload_id=A", headers: putHeaders(length: 4_000_000),
                              file: id, length: 4_000_000),
            progress: UploadProgress()
        )
    }
    #expect(await eventually { server.accepted == 1 })
    try await Task.sleep(for: .milliseconds(200))
    task.cancel()
    guard case .failure(.cancelled) = try await task.value else { Issue.record("not cancelled"); return }
    transport.closeFile(id: id)
    #expect(await eventually(9) { closed.get != nil })
    #expect(closed.get == true)
}

/// Redirects stay refused for uploads, and the response is read through the 64 KiB cap.
@Test func anUploadRedirectIsNotFollowedAndItsResponseIsCapped() async throws {
    let elsewhere = try TestServer { _ in }
    let server = try TestServer { peer in
        guard let (head, _) = peer.readRequest() else { return }
        if head.hasPrefix("OPTIONS") {
            peer.write(httpResponse(body: Data(repeating: 0x61, count: 100_000)))
        } else {
            peer.write(httpResponse("307 Temporary Redirect", headers: ["Location: http://127.0.0.1:\(elsewhere.port)/u?a=1"]))
        }
    }
    let transport = URLSessionTransport(allowList: uploadList(server.port))
    let url = "http://127.0.0.1:\(server.port)/u?upload_id=A"
    let redirect = await transport.performUpload(try uploadRequest(url, headers: probeHeaders()), progress: UploadProgress())
    guard case .response(307, _, _, false) = redirect else { Issue.record("\(redirect)"); return }
    let big = await transport.performUpload(
        try uploadRequest(url, method: .options, headers: preflightHeaders(range: false)), progress: UploadProgress()
    )
    guard case .response(200, _, let body, true) = big, body.isEmpty else { Issue.record("\(big)"); return }
    try await Task.sleep(for: .milliseconds(300))
    #expect(elsewhere.accepted == 0)
}

// MARK: Files (CU-6)

@Test func aFIFODevZeroDirectoryAndPackageAreRefusedAtAttach() throws {
    let dir = FileManager.default.temporaryDirectory.appendingPathComponent("velella-attach-\(UUID().uuidString)")
    try FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
    let fifo = dir.appendingPathComponent("pipe")
    #expect(mkfifo(fifo.path, 0o600) == 0)
    let package = dir.appendingPathComponent("Thing.app/Contents", isDirectory: true)
    try FileManager.default.createDirectory(at: package, withIntermediateDirectories: true)
    for url in [fifo, URL(fileURLWithPath: "/dev/zero"), dir, dir.appendingPathComponent("Thing.app"),
                dir.appendingPathComponent("missing"), URL(string: "https://example.com/a.png")!] {
        #expect(AttachmentFiles.identity(of: url) == nil, "\(url)")
    }
    // A regular file is attachable; a link to one records the file it points to.
    let file = try patternFile(10)
    let link = dir.appendingPathComponent("link.bin")
    try FileManager.default.createSymbolicLink(at: link, withDestinationURL: file)
    let direct = try #require(AttachmentFiles.identity(of: file))
    let linked = try #require(AttachmentFiles.identity(of: link))
    #expect(direct.size() == 10)
    #expect(linked.inode() == direct.inode() && linked.path() == file.resolvingSymlinksInPath().path)
}

/// A file swapped after attach (a link, a FIFO, or new content) fails the open: no request.
@Test func aFileSwappedAfterAttachIsRefusedWithNoPUT() async throws {
    let server = try TestServer { peer in
        _ = peer.readRequest()
        peer.write(ok204)
    }
    let transport = URLSessionTransport(allowList: uploadList(server.port))
    let other = try patternFile(10, name: "other.bin")
    for swap in ["link", "fifo", "rewrite"] {
        let file = try patternFile(10)
        let identity = try #require(AttachmentFiles.identity(of: file))
        try FileManager.default.removeItem(at: file)
        switch swap {
        case "link": try FileManager.default.createSymbolicLink(at: file, withDestinationURL: other)
        case "fifo": #expect(mkfifo(file.path, 0o600) == 0)
        default: try pattern(0..<10).write(to: file)
        }
        #expect(transport.openFile(file: identity) == nil, "\(swap)")
    }
    // The same path untouched opens.
    let file = try patternFile(10)
    let id = try opened(transport, file)
    transport.closeFile(id: id)
    // An id that is not open sends nothing.
    let outcome = await transport.performUpload(
        try uploadRequest("http://127.0.0.1:\(server.port)/u?upload_id=A", headers: putHeaders(length: 10), file: 999, length: 10),
        progress: UploadProgress()
    )
    guard case .failure(.rejected) = outcome else { Issue.record("\(outcome)"); return }
    try await Task.sleep(for: .milliseconds(200))
    #expect(server.accepted == 0)
}

// MARK: Temporary files (CU-1)

/// Two long pastes in two conversations: two `message.txt` files that never share a path, each
/// uploading its own bytes; removing one leaves the other; the launch sweep removes the rest.
@Test func twoPastesKeepTheirOwnFilesAndBytes() async throws {
    let first = try AttachmentFiles.write(Data("first conversation".utf8), named: "message.txt")
    let second = try AttachmentFiles.write(Data("second conversation".utf8), named: "message.txt")
    #expect(first != second && first.lastPathComponent == "message.txt")
    let bodies = Box<[Data]>([])
    let server = try TestServer { peer in
        while let (_, body) = peer.readRequest() {
            bodies.update { $0.append(body) }
            peer.write(ok204)
        }
    }
    let transport = URLSessionTransport(allowList: uploadList(server.port))
    for file in [first, second] {
        let identity = try #require(AttachmentFiles.identity(of: file))
        let id = try #require(transport.openFile(file: identity))
        let outcome = await transport.performUpload(
            try uploadRequest("http://127.0.0.1:\(server.port)/u?upload_id=A", headers: putHeaders(length: identity.size()),
                              file: id, length: identity.size()),
            progress: UploadProgress()
        )
        transport.closeFile(id: id)
        guard case .response(200, _, _, _) = outcome else { Issue.record("\(outcome)"); return }
    }
    #expect(bodies.get == [Data("first conversation".utf8), Data("second conversation".utf8)])
    AttachmentFiles.remove(first)
    #expect(!FileManager.default.fileExists(atPath: first.deletingLastPathComponent().path))
    #expect(FileManager.default.fileExists(atPath: second.path))
    // Only files under the root's own directories are removed.
    let outside = try patternFile(3)
    AttachmentFiles.remove(outside)
    #expect(FileManager.default.fileExists(atPath: outside.path))
    #expect(throws: (any Error).self) { try AttachmentFiles.write(Data(), named: "../x") }
    AttachmentFiles.sweep()
    #expect(!FileManager.default.fileExists(atPath: second.path))
    #expect(!FileManager.default.fileExists(atPath: AttachmentFiles.root.path))
}

/// The composer queues a file by `identity(of:)`'s resolved path, which spells the system temporary
/// directory as `/private/var/...`; removing by that path must still delete the directory (CUA).
@Test func aSynthesizedFileIsRemovedByItsResolvedPath() throws {
    let written = try AttachmentFiles.write(Data("pasted".utf8), named: "message.txt")
    let identity = try #require(AttachmentFiles.identity(of: written))
    AttachmentFiles.remove(URL(fileURLWithPath: identity.path()))
    #expect(!FileManager.default.fileExists(atPath: written.deletingLastPathComponent().path))
}

// MARK: Redaction (CU-7)

@Test func nothingThatCarriesAPathNameOrAltTextPrintsIt() throws {
    let file = UploadFile(path: "/Users/\(marker)/\(marker).png", device: 1, inode: 2, size: 3, mtimeNs: 4)
    let descriptor = AttachmentDescriptor(
        file: file, filename: "\(marker).png", description: marker, spoiler: true, contentType: "image/png",
        originalMd5: nil
    )
    let request = try uploadRequest(
        "https://discord-attachments-uploads-prd.storage.googleapis.com/\(marker)?upload_id=\(marker)",
        headers: [HttpHeader(name: "Origin", value: marker)]
    )
    let pending = PendingUploadFile(descriptor: descriptor, sent: 1, size: 3)
    let values: [Any] = [file, descriptor, request, pending]
    for value in values {
        var dumped = ""
        dump(value, to: &dumped)
        for text in [String(describing: value), String(reflecting: value), dumped] {
            #expect(!text.contains("SYNTHETIC"), "\(type(of: value)): \(text)")
        }
    }
    // Not vacuous.
    #expect(descriptor.altText() == marker && file.path().contains(marker))
}
