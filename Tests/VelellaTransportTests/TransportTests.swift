import Foundation
import Testing
import VelellaFFI
@testable import VelellaTransport

// Plan NET, Swift side: the URLSession transport against a local server on 127.0.0.1. Each test
// builds the transport with a test-only allow-list for its own port; nothing here contacts
// discord.com (the shipped list is only exercised on requests it refuses before sending).

let marker = "SYNTHETIC_TOKEN_MARKER_0123456789"
let ua = VelellaFFI.userAgent()

/// http and ws to 127.0.0.1 on these ports, any path; Authorization only under `/api/v9/`.
func local(_ ports: UInt16...) -> AllowList {
    let rules = { (scheme: String) in
        ports.map { AllowList.Rule(scheme: scheme, host: .exact("127.0.0.1"), port: Int($0), path: .prefix("/")) }
    }
    return AllowList(http: rules("http"), webSocket: rules("ws"), authorizedPrefix: "http://127.0.0.1:\(ports[0])/api/v9/")
}

func request(
    _ url: String, method: HttpMethod = .get, body: Data? = nil, auth: String? = marker,
    cap: UInt64 = 4 << 20, deadlineMs: UInt64 = 10_000
) throws -> HttpRequest {
    try HttpRequest(
        method: method, url: url, headers: [HttpHeader(name: "User-Agent", value: ua)], body: body,
        authorization: auth, deadlineMs: deadlineMs, bodyCap: cap
    )
}

func header(_ name: String, in head: String) -> [String] {
    head.split(separator: "\r\n").dropFirst().compactMap { line in
        let parts = line.split(separator: ":", maxSplits: 1)
        guard parts.count == 2, parts[0].lowercased() == name.lowercased() else { return nil }
        return String(parts[1].dropFirst(parts[1].hasPrefix(" ") ? 1 : 0))
    }
}

// MARK: Cookies, redirects

@Test func noCookieIsSentBackAfterSetCookie() async throws {
    let heads = Box<[String]>([])
    let server = try TestServer { peer in
        guard let (head, _) = peer.readRequest() else { return }
        heads.update { $0.append(head) }
        peer.write(httpResponse(headers: ["Set-Cookie: session=SECRET; Path=/"], body: Data("{}".utf8)))
    }
    let transport = URLSessionTransport(allowList: local(server.port))
    for _ in 0..<2 {
        guard case .response(200, _, _, false) = await transport.perform(try request("http://127.0.0.1:\(server.port)/api/v9/a")) else {
            Issue.record("no response"); return
        }
    }
    #expect(heads.get.count == 2)
    #expect(heads.get.allSatisfy { header("cookie", in: $0).isEmpty })
}

@Test func a302IsReturnedUnfollowed() async throws {
    let elsewhere = try TestServer { _ in }
    let server = try TestServer { peer in
        guard peer.readRequest() != nil else { return }
        peer.write(httpResponse("302 Found", headers: ["Location: http://127.0.0.1:\(elsewhere.port)/api/v9/b"]))
    }
    let transport = URLSessionTransport(allowList: local(server.port, elsewhere.port))
    let outcome = await transport.perform(try request("http://127.0.0.1:\(server.port)/api/v9/a"))
    guard case .response(let status, let headers, _, false) = outcome else {
        Issue.record("\(outcome)"); return
    }
    #expect(status == 302)
    #expect(headers.contains { $0.name.lowercased() == "location" })
    try await Task.sleep(for: .milliseconds(300))
    #expect(elsewhere.accepted == 0, "the redirect target was contacted")
}

@Test func aWebSocketUpgrade3xxIsReturnedUnfollowed() async throws {
    let elsewhere = try TestServer { _ in }
    let server = try TestServer { peer in
        guard peer.readRequest() != nil else { return }
        peer.write(httpResponse("302 Found", headers: ["Location: ws://127.0.0.1:\(elsewhere.port)/"]))
    }
    let transport = URLSessionTransport(allowList: local(server.port, elsewhere.port))
    let outcome = await transport.open(try WebSocketRequest(
        url: "ws://127.0.0.1:\(server.port)/?v=9", headers: [], maxMessageSize: 1 << 20, deadlineMs: 5000
    ))
    guard case .rejected(302) = outcome else { Issue.record("\(outcome)"); return }
    try await Task.sleep(for: .milliseconds(300))
    #expect(elsewhere.accepted == 0, "the redirect target was contacted")
}

// MARK: Body caps

/// The declared length over the cap stops it at once; an undeclared body is cut at cap + 1, so
/// the server cannot finish writing; a body exactly at the cap arrives whole.
func capCase(cap: Int) async throws {
    let full = Box<[Bool]>([])
    let server = try TestServer { peer in
        guard let (head, _) = peer.readRequest() else { return }
        let path = head.split(separator: " ")[1]
        switch path {
        case "/api/v9/declared":
            peer.write(httpResponse(body: Data(count: cap + 1)))
        case "/api/v9/streamed":
            // No length: the body ends at close. Far more than the cap, in chunks.
            peer.write(httpResponse(length: false))
            var wrote = true
            for _ in 0..<64 where wrote { wrote = peer.write(Data(count: max(cap / 4, 16 << 10))) }
            full.update { $0.append(wrote) }
        default:
            peer.write(httpResponse(body: Data(repeating: 0x61, count: cap)))
        }
    }
    let transport = URLSessionTransport(allowList: local(server.port))
    let base = "http://127.0.0.1:\(server.port)/api/v9/"
    let declared = await transport.perform(try request(base + "declared", cap: UInt64(cap)))
    guard case .response(200, _, let body, true) = declared, body.isEmpty else { Issue.record("\(declared)"); return }
    let streamed = await transport.perform(try request(base + "streamed", cap: UInt64(cap)))
    guard case .response(200, _, let body2, true) = streamed, body2.isEmpty else { Issue.record("\(streamed)"); return }
    #expect(await eventually { full.get.count == 1 })
    #expect(full.get == [false], "the server wrote the whole oversized body: the client kept reading")
    let atCap = await transport.perform(try request(base + "exact", cap: UInt64(cap)))
    guard case .response(200, _, let body3, false) = atCap else { Issue.record("\(atCap)"); return }
    #expect(body3.count == cap)
}

@Test func anOverCapAPIBodyIsStopped() async throws { try await capCase(cap: 4 << 20) }

/// D12's script (the fast-connect chunk, about 33 KB) gets a cap of this order.
@Test func anOverCapScriptBodyIsStopped() async throws { try await capCase(cap: 256 << 10) }

// MARK: Bytes on the wire

@Test func theUserAgentAndAuthorizationGoOnTheWireExactly() async throws {
    let raw = Box<String>("")
    let server = try TestServer { peer in
        guard let (head, _) = peer.readRequest() else { return }
        raw.update { $0 = head }
        peer.write(httpResponse(body: Data("{}".utf8)))
    }
    let transport = URLSessionTransport(allowList: local(server.port))
    let composed = try HttpRequest(
        method: .get, url: "http://127.0.0.1:\(server.port)/api/v9/users/@me",
        headers: [HttpHeader(name: "User-Agent", value: ua), HttpHeader(name: "Accept-Language", value: "en-US,en;q=0.9")],
        body: nil, authorization: marker, deadlineMs: 10_000, bodyCap: 1 << 20
    )
    _ = await transport.perform(composed)
    let head = raw.get
    #expect(header("user-agent", in: head) == [ua])
    // Core's value replaces the one URLSession would take from the system locale.
    #expect(header("accept-language", in: head) == ["en-US,en;q=0.9"])
    #expect(!head.contains("CFNetwork") && !head.contains("Darwin/"))
    #expect(head.contains("\r\nAuthorization: \(marker)\r\n"))
    #expect(head.components(separatedBy: marker).count == 2, "the token appears more than once")
    #expect(header("cookie", in: head).isEmpty)
}

@Test func anEmojiPathAndPercent20QueryArriveByteForByte() async throws {
    let line = Box<String>("")
    let server = try TestServer { peer in
        guard let (head, _) = peer.readRequest() else { return }
        line.update { $0 = head.components(separatedBy: "\r\n")[0] }
        peer.write(httpResponse(body: Data("{}".utf8)))
    }
    let transport = URLSessionTransport(allowList: local(server.port))
    let target = "/api/v9/channels/1/messages/2/reactions/%F0%9F%91%8D/name:123/@me?location=Message%20Reaction&burst=false"
    let outcome = await transport.perform(try request("http://127.0.0.1:\(server.port)\(target)"))
    guard case .response(200, _, _, _) = outcome else { Issue.record("\(outcome)"); return }
    #expect(line.get == "GET \(target) HTTP/1.1")
}

// MARK: Failures and deadlines

/// T-3: a POST the server read and then dropped without answering is not `connectFailed` (core
/// would never resend it), and it reached the server exactly once, also on a reused connection.
@Test func aPOSTReadThenClosedIsNotConnectFailedAndArrivesOnce() async throws {
    let posts = Box(0)
    let server = try TestServer { peer in
        while let (head, _) = peer.readRequest() {
            if head.hasPrefix("POST") {
                posts.update { $0 += 1 }
                peer.drop()
                return
            }
            // A keep-alive answer, so the POST may go out on this same connection.
            peer.write(httpResponse(body: Data("{}".utf8), close: false))
        }
    }
    let transport = URLSessionTransport(allowList: local(server.port))
    let base = "http://127.0.0.1:\(server.port)/api/v9/channels/1/messages"
    _ = await transport.perform(try request(base))
    let outcome = await transport.perform(try request(base, method: .post, body: Data(#"{"content":"hi"}"#.utf8)))
    guard case .failure(let kind) = outcome else { Issue.record("\(outcome)"); return }
    #expect(kind != .connectFailed)
    try await Task.sleep(for: .milliseconds(500))
    #expect(posts.get == 1, "the POST was sent \(posts.get) times")
}

@Test func aRefusedPortIsConnectFailed() async throws {
    let port = try TestServer.refusedPort()
    let transport = URLSessionTransport(allowList: local(port))
    let outcome = await transport.perform(try request("http://127.0.0.1:\(port)/api/v9/a"))
    guard case .failure(.connectFailed) = outcome else { Issue.record("\(outcome)"); return }
}

/// T-4: the request's deadline (and core's cancel, which is what dropping its future does) tears
/// the connection down: the server sees it close.
@Test func theDeadlineTearsTheConnectionDownServerSide() async throws {
    let closed = Box<[Bool]>([])
    let server = try TestServer { peer in
        guard peer.readRequest() != nil else { return }
        closed.update { $0.append(peer.clientClosed(within: 8)) }
    }
    let transport = URLSessionTransport(allowList: local(server.port))
    let started = Date()
    let outcome = await transport.perform(try request("http://127.0.0.1:\(server.port)/api/v9/a", deadlineMs: 1000))
    guard case .failure(.timeout) = outcome else { Issue.record("\(outcome)"); return }
    #expect(Date().timeIntervalSince(started) < 4)
    #expect(await eventually(9) { closed.get.count == 1 })
    #expect(closed.get == [true])

    let task = Task { await transport.perform(try request("http://127.0.0.1:\(server.port)/api/v9/b")) }
    #expect(await eventually { server.accepted == 2 })
    try await Task.sleep(for: .milliseconds(200))
    task.cancel()
    guard case .failure(.cancelled) = try await task.value else { Issue.record("not cancelled"); return }
    #expect(await eventually(9) { closed.get.count == 2 })
    #expect(closed.get == [true, true])
}

@Test func urlErrorsMapToTheClosedSet() {
    let connect: [URLError.Code] = [
        .cannotFindHost, .cannotConnectToHost, .dnsLookupFailed, .notConnectedToInternet, .secureConnectionFailed,
        .serverCertificateUntrusted, .serverCertificateHasBadDate, .serverCertificateHasUnknownRoot,
        .serverCertificateNotYetValid, .clientCertificateRejected, .clientCertificateRequired,
    ]
    for code in connect { #expect(transportErrorKind(URLError(code)) == .connectFailed, "\(code)") }
    #expect(transportErrorKind(URLError(.timedOut)) == .timeout)
    #expect(transportErrorKind(URLError(.cancelled)) == .cancelled)
    for code: URLError.Code in [.networkConnectionLost, .badServerResponse, .unknown, .zeroByteResource, URLError.Code(rawValue: -98765)] {
        #expect(transportErrorKind(URLError(code)) == .other, "\(code)")
    }
    #expect(transportErrorKind(POSIXError(.ECONNREFUSED)) == .other)
}

// MARK: WebSocket

/// A WebSocket server: answers the upgrade, then runs `then`.
func webSocketServer(_ then: @escaping @Sendable (Peer) -> Void) throws -> TestServer {
    try TestServer { peer in
        guard let (head, _) = peer.readRequest() else { return }
        peer.acceptWebSocket(head)
        then(peer)
    }
}

func openSocket(_ transport: URLSessionTransport, port: UInt16, max: UInt64 = 64 << 20) async throws -> WebSocketSocket? {
    let outcome = await transport.open(try WebSocketRequest(
        url: "ws://127.0.0.1:\(port)/?v=9&encoding=json&compress=zlib-stream",
        headers: [HttpHeader(name: "User-Agent", value: ua)], maxMessageSize: max, deadlineMs: 5000
    ))
    guard case .opened(let socket) = outcome else { Issue.record("\(outcome)"); return nil }
    return socket
}

@Test func aTwoMiBBinaryFrameIsReceived() async throws {
    let payload = Data((0..<(2 << 20)).map { UInt8($0 & 0xFF) })
    let server = try webSocketServer { peer in
        peer.sendFrame(opcode: 0x2, payload)
        _ = peer.readFrame()
    }
    let transport = URLSessionTransport(allowList: local(server.port))
    guard let socket = try await openSocket(transport, port: server.port) else { return }
    guard case .binary(let data) = await socket.receive() else { Issue.record("no binary frame"); return }
    #expect(data == payload)
    socket.close(code: 1000)
}

/// The upgrade's deadline covers the upgrade only: an open socket quiet for longer still gets the
/// next message (Discord's gateway is quiet for up to a heartbeat interval, about 41 s).
@Test func anOpenSocketOutlivesTheUpgradeDeadline() async throws {
    let server = try webSocketServer { peer in
        Thread.sleep(forTimeInterval: 2.5)
        peer.sendFrame(opcode: 0x1, Data("late".utf8))
        _ = peer.readFrame()
    }
    let transport = URLSessionTransport(allowList: local(server.port))
    let outcome = await transport.open(try WebSocketRequest(
        url: "ws://127.0.0.1:\(server.port)/", headers: [], maxMessageSize: 1 << 20, deadlineMs: 1000
    ))
    guard case .opened(let socket) = outcome else { Issue.record("\(outcome)"); return }
    let message = await socket.receive()
    guard case .text("late") = message else { Issue.record("\(message)"); return }
    socket.close(code: 1000)
}

/// T-6: a 4004 close frame (Discord: the token is bad, the sign-in is deleted) and a TCP drop
/// (resume) must never be confused.
@Test func a4004CloseFrameAndAnAbruptDropGiveDifferentResults() async throws {
    let server = try webSocketServer { peer in
        peer.sendClose(4004)
        _ = peer.readFrame()
    }
    let dropper = try webSocketServer { peer in peer.drop() }
    let transport = URLSessionTransport(allowList: local(server.port, dropper.port))
    guard let socket = try await openSocket(transport, port: server.port) else { return }
    let closed = await socket.receive()
    guard case .closed(4004) = closed else { Issue.record("\(closed)"); return }
    guard let dropped = try await openSocket(transport, port: dropper.port) else { return }
    let lost = await dropped.receive()
    guard case .lost = lost else { Issue.record("\(lost)"); return }
}

/// T-4: core's close code (4000 keeps a Discord session resumable) reaches the server, and the
/// upgrade carries core's User-Agent.
@Test func theClientsCloseCodeAndUserAgentReachTheServer() async throws {
    let seen = Box<(head: String, close: Data?)>(("", nil))
    let server = try TestServer { peer in
        guard let (head, _) = peer.readRequest() else { return }
        peer.acceptWebSocket(head)
        seen.update { $0.head = head }
        while let frame = peer.readFrame() {
            if frame.opcode == 0x8 { seen.update { $0.close = frame.payload }; return }
        }
    }
    let transport = URLSessionTransport(allowList: local(server.port))
    guard let socket = try await openSocket(transport, port: server.port) else { return }
    let sent = await withCheckedContinuation { c in socket.send(.string("{\"op\":1,\"d\":null}")) { c.resume(returning: $0) } }
    #expect(sent == nil)
    socket.close(code: 4000)
    #expect(await eventually { seen.get.close != nil })
    #expect(seen.get.close.map { Array($0.prefix(2)) } == [0x0F, 0xA0])
    #expect(header("user-agent", in: seen.get.head) == [ua])
}

// MARK: Allow-list and URL fidelity

@Test func aURLThePlatformWouldChangeIsRejectedAndNothingIsSent() async throws {
    let server = try TestServer { peer in
        _ = peer.readRequest()
        peer.write(httpResponse(body: Data("{}".utf8)))
    }
    let transport = URLSessionTransport(allowList: local(server.port))
    let base = "http://127.0.0.1:\(server.port)/api/v9/"
    for changed in [base + "a b", base + "\u{1F44D}", base + "a|b", base + "a?q=a b", base + "%zz"] {
        let outcome = await transport.perform(try request(changed))
        guard case .failure(.rejected) = outcome else { Issue.record("\(changed): \(outcome)"); continue }
    }
    let ws = await transport.open(try WebSocketRequest(url: "ws://127.0.0.1:\(server.port)/a b", headers: [], maxMessageSize: 1, deadlineMs: 1000))
    guard case .failure(.rejected) = ws else { Issue.record("\(ws)"); return }
    try await Task.sleep(for: .milliseconds(200))
    #expect(server.accepted == 0)
}

@Test func anOffListHostIsRejectedAndNothingIsSent() async throws {
    let server = try TestServer { peer in _ = peer.readRequest() }
    let url = "http://127.0.0.1:\(server.port)/api/v9/a"
    // The shipped list, and a test list for another port.
    for transport in [URLSessionTransport(), URLSessionTransport(allowList: local(server.port &+ 1))] {
        let outcome = await transport.perform(try request(url))
        guard case .failure(.rejected) = outcome else { Issue.record("\(outcome)"); return }
        let ws = await transport.open(try WebSocketRequest(url: "ws://127.0.0.1:\(server.port)/", headers: [], maxMessageSize: 1, deadlineMs: 1000))
        guard case .failure(.rejected) = ws else { Issue.record("\(ws)"); return }
    }
    try await Task.sleep(for: .milliseconds(200))
    #expect(server.accepted == 0)
}

/// Authorization goes only to the API prefix; a request elsewhere that carries one is refused.
@Test func authorizationGoesOnlyToTheAPI() async throws {
    let heads = Box<[String]>([])
    let server = try TestServer { peer in
        guard let (head, _) = peer.readRequest() else { return }
        heads.update { $0.append(head) }
        peer.write(httpResponse(body: Data("{}".utf8)))
    }
    let transport = URLSessionTransport(allowList: local(server.port))
    let app = "http://127.0.0.1:\(server.port)/app"
    guard case .failure(.rejected) = await transport.perform(try request(app)) else { Issue.record("sent"); return }
    #expect(server.accepted == 0)
    guard case .response(200, _, _, _) = await transport.perform(try request(app, auth: nil)) else { Issue.record("not sent"); return }
    #expect(heads.get.count == 1 && header("authorization", in: heads.get[0]).isEmpty)
    // The shipped list refuses the identity URLs with a token before anything leaves.
    for url in ["https://discord.com/app", "https://discord.com/assets/fast-connect.0123456789abcdef.js"] {
        guard case .failure(.rejected) = await URLSessionTransport().perform(try request(url)) else {
            Issue.record("\(url) with a token was not refused"); return
        }
    }
}

@Test func theShippedAllowListIsHostAndPathPairs() {
    let list = AllowList.shipped
    let http = [
        "https://discord.com/api/v9/users/@me": true,
        "https://discord.com/api/v9/channels/1/messages?limit=50": true,
        "https://discord.com/app": true,
        "https://discord.com/assets/fast-connect.0123456789abcdef.js": true,
        "https://discord.com/app?x=1": false,
        "https://discord.com/app/": false,
        "https://discord.com/assets/fast-connect.0123456789ABCDEF.js": false,
        "https://discord.com/assets/fast-connect.0123456789abcde.js": false,
        "https://discord.com/assets/fast-connect.0123456789abcdef0.js": false,
        "https://discord.com/assets/fast-connect.0123456789abcdef.js?v=1": false,
        "https://discord.com/assets/web.0123456789abcdef.js": false,
        "https://discord.com/api/v10/users/@me": false,
        "https://discord.com/oauth2/authorize": false,
        "http://discord.com/api/v9/users/@me": false,
        "https://discord.com:8443/api/v9/users/@me": false,
        "https://user@discord.com/api/v9/users/@me": false,
        "https://discord.com.evil.test/api/v9/x": false,
        "https://cdn.discordapp.com/api/v9/x": false,
        "https://gateway.discord.gg/api/v9/x": false,
    ]
    for (url, want) in http { #expect(list.allowsHTTP(URL(string: url)!) == want, "\(url)") }
    let ws = [
        "wss://gateway.discord.gg/?v=9&encoding=json&compress=zlib-stream": true,
        "wss://gateway-us-east1-b.discord.gg/?v=9&encoding=json&compress=zlib-stream": true,
        "wss://gateway.discord.gg/path": false,
        "wss://gateway-.discord.gg/": false,
        "wss://gateway-a.b.discord.gg/": false,
        "wss://gateway.discord.gg.evil.test/": false,
        "wss://gateway.discord.gg:8443/": false,
        "ws://gateway.discord.gg/": false,
        "wss://discord.com/": false,
        "https://gateway.discord.gg/": false,
    ]
    for (url, want) in ws { #expect(list.allowsWebSocket(URL(string: url)!) == want, "\(url)") }
    #expect(list.authorizes("https://discord.com/api/v9/users/@me"))
    #expect(!list.authorizes("https://discord.com/app"))
    #expect(!list.authorizes("https://discord.com/api/v9"))
}

// MARK: Redaction

@Test func nothingThatCarriesTheTokenPrintsIt() throws {
    let req = try HttpRequest(
        method: .post, url: "https://discord.com/api/v9/x?\(marker)", headers: [HttpHeader(name: "X-Test", value: marker)],
        body: Data(marker.utf8), authorization: marker, deadlineMs: 1000, bodyCap: 8
    )
    let frame = WebSocketFrame.newText(text: #"{"op":2,"d":{"token":"\#(marker)"}}"#)
    let ws = try WebSocketRequest(url: "wss://gateway.discord.gg/?\(marker)", headers: [], maxMessageSize: 1, deadlineMs: 1)
    let outcome = HTTPOutcome.response(status: 200, headers: [HttpHeader(name: "X", value: marker)], body: Data(marker.utf8), overCap: false)
    let message = ReceivedMessage.text(marker)
    let values: [Any] = [req, frame, ws, outcome, message]
    for value in values {
        var dumped = ""
        dump(value, to: &dumped)
        for text in [String(describing: value), String(reflecting: value), dumped] {
            #expect(!text.contains("SYNTHETIC"), "\(type(of: value))")
        }
    }
    // Not vacuous: the values do carry it.
    #expect(frame.text()?.contains(marker) == true)
    #expect(req.takeAuthorization() == marker)
    #expect(req.takeAuthorization() == nil, "Authorization handed out twice")
}
