import CryptoKit
import Foundation

// A small blocking TCP server on 127.0.0.1 for the transport's wire tests: each accepted
// connection runs the test's handler on its own thread, which reads and writes raw bytes (HTTP/1.1
// and WebSocket framing by hand). Nothing here leaves the machine.

final class TestServer: @unchecked Sendable {
    // @unchecked: `count` is behind `lock`; the rest is immutable.
    let port: UInt16
    private let fd: Int32
    private let lock = NSLock()
    private var count = 0

    /// Connections accepted so far.
    var accepted: Int { lock.withLock { count } }

    init(_ handle: @escaping @Sendable (Peer) -> Void) throws {
        let (fd, port) = try Self.listen()
        self.fd = fd
        self.port = port
        Thread { [self] in
            while true {
                let c = accept(fd, nil, nil)
                guard c >= 0 else { return }
                lock.withLock { count += 1 }
                Thread { handle(Peer(c)) }.start()
            }
        }.start()
    }

    deinit {
        shutdown(fd, SHUT_RDWR)
        Darwin.close(fd)
    }

    /// A listening socket on 127.0.0.1 and its port.
    static func listen() throws -> (Int32, UInt16) {
        let s = socket(AF_INET, SOCK_STREAM, 0)
        var addr = sockaddr_in()
        addr.sin_len = UInt8(MemoryLayout<sockaddr_in>.size)
        addr.sin_family = sa_family_t(AF_INET)
        addr.sin_addr.s_addr = inet_addr("127.0.0.1")
        var len = socklen_t(MemoryLayout<sockaddr_in>.size)
        let ok = withUnsafeMutablePointer(to: &addr) {
            $0.withMemoryRebound(to: sockaddr.self, capacity: 1) { p in
                bind(s, p, len) == 0 && Darwin.listen(s, 16) == 0 && getsockname(s, p, &len) == 0
            }
        }
        guard ok else {
            Darwin.close(s)
            throw POSIXError(.EADDRNOTAVAIL)
        }
        return (s, UInt16(bigEndian: addr.sin_port))
    }

    /// A port nothing listens on (bound, then closed).
    static func refusedPort() throws -> UInt16 {
        let s = socket(AF_INET, SOCK_STREAM, 0)
        var addr = sockaddr_in()
        addr.sin_len = UInt8(MemoryLayout<sockaddr_in>.size)
        addr.sin_family = sa_family_t(AF_INET)
        addr.sin_addr.s_addr = inet_addr("127.0.0.1")
        var len = socklen_t(MemoryLayout<sockaddr_in>.size)
        let ok = withUnsafeMutablePointer(to: &addr) {
            $0.withMemoryRebound(to: sockaddr.self, capacity: 1) { p in
                bind(s, p, len) == 0 && getsockname(s, p, &len) == 0
            }
        }
        Darwin.close(s)
        guard ok else { throw POSIXError(.EADDRNOTAVAIL) }
        return UInt16(bigEndian: addr.sin_port)
    }
}

/// One accepted connection.
final class Peer: @unchecked Sendable {
    // @unchecked: used by one handler thread only.
    let fd: Int32
    private var buffer = Data()

    init(_ fd: Int32) {
        self.fd = fd
        var one: Int32 = 1
        setsockopt(fd, SOL_SOCKET, SO_NOSIGPIPE, &one, socklen_t(MemoryLayout<Int32>.size))
        setTimeout(10)
    }

    deinit { Darwin.close(fd) }

    func setTimeout(_ seconds: Int) {
        var tv = timeval(tv_sec: seconds, tv_usec: 0)
        setsockopt(fd, SOL_SOCKET, SO_RCVTIMEO, &tv, socklen_t(MemoryLayout<timeval>.size))
    }

    /// One read into the buffer; false on end of stream, error or timeout.
    @discardableResult
    func fill() -> Bool {
        var chunk = [UInt8](repeating: 0, count: 65536)
        let n = read(fd, &chunk, chunk.count)
        guard n > 0 else { return false }
        buffer.append(contentsOf: chunk[..<n])
        return true
    }

    func take(_ count: Int) -> Data? {
        while buffer.count < count { guard fill() else { return nil } }
        let out = buffer.prefix(count)
        buffer.removeFirst(count)
        return Data(out)
    }

    /// The raw head (through the blank line) and the Content-Length body.
    func readRequest() -> (head: String, body: Data)? {
        let blank = Data("\r\n\r\n".utf8)
        while buffer.range(of: blank) == nil { guard fill() else { return nil } }
        let end = buffer.range(of: blank)!.upperBound
        let head = String(decoding: buffer[buffer.startIndex..<end], as: UTF8.self)
        buffer.removeSubrange(buffer.startIndex..<end)
        let length = head.split(separator: "\r\n").lazy.compactMap { line -> Int? in
            let parts = line.split(separator: ":", maxSplits: 1)
            guard parts.count == 2, parts[0].lowercased() == "content-length" else { return nil }
            return Int(parts[1].trimmingCharacters(in: .whitespaces))
        }.first ?? 0
        guard let body = take(length) else { return nil }
        return (head, body)
    }

    /// The raw head only; the body stays unread (`drain` reads it piece by piece).
    func readHead() -> String? {
        let blank = Data("\r\n\r\n".utf8)
        while buffer.range(of: blank) == nil { guard fill() else { return nil } }
        let end = buffer.range(of: blank)!.upperBound
        let head = String(decoding: buffer[buffer.startIndex..<end], as: UTF8.self)
        buffer.removeSubrange(buffer.startIndex..<end)
        return head
    }

    /// Reads up to `count` body bytes without keeping them, handing each piece to `piece`;
    /// returns how many arrived before the end of stream. Reads past the buffer straight from
    /// the socket: `Data.removeFirst` keeps its storage, so a buffer drained and refilled grows.
    func drain(_ count: Int, _ piece: (Data) -> Void) -> Int {
        var got = min(buffer.count, count)
        if got > 0 {
            piece(Data(buffer.prefix(got)))
            buffer = Data(buffer.dropFirst(got))
        }
        var chunk = [UInt8](repeating: 0, count: 65536)
        while got < count {
            let n = read(fd, &chunk, min(chunk.count, count - got))
            guard n > 0 else { break }
            piece(Data(chunk[..<n]))
            got += n
        }
        return got
    }


    /// False if the peer went away before everything was written.
    @discardableResult
    func write(_ data: Data) -> Bool {
        data.withUnsafeBytes { raw in
            var offset = 0
            while offset < raw.count {
                let n = Darwin.write(fd, raw.baseAddress! + offset, raw.count - offset)
                if n <= 0 { return false }
                offset += n
            }
            return true
        }
    }

    func write(_ text: String) { write(Data(text.utf8)) }

    /// True once the client closed its end (read gives end of stream or reset) within `seconds`.
    func clientClosed(within seconds: Int) -> Bool {
        setTimeout(seconds)
        let start = Date()
        var chunk = [UInt8](repeating: 0, count: 65536)
        while Date().timeIntervalSince(start) < Double(seconds) {
            let n = read(fd, &chunk, chunk.count)
            if n == 0 { return true }
            if n < 0 { return errno == ECONNRESET }
        }
        return false
    }

    /// TCP close without anything else.
    func drop() { shutdown(fd, SHUT_RDWR) }

    // MARK: WebSocket

    /// Answers an upgrade request with 101.
    func acceptWebSocket(_ head: String) {
        let key = head.split(separator: "\r\n").compactMap { line -> String? in
            let parts = line.split(separator: ":", maxSplits: 1)
            guard parts.count == 2, parts[0].lowercased() == "sec-websocket-key" else { return nil }
            return parts[1].trimmingCharacters(in: .whitespaces)
        }.first ?? ""
        let digest = Insecure.SHA1.hash(data: Data((key + "258EAFA5-E914-47DA-95CA-C5AB0DC85B11").utf8))
        write("HTTP/1.1 101 Switching Protocols\r\nUpgrade: websocket\r\nConnection: Upgrade\r\n"
            + "Sec-WebSocket-Accept: \(Data(digest).base64EncodedString())\r\n\r\n")
    }

    /// One unmasked server frame.
    func sendFrame(opcode: UInt8, _ payload: Data) {
        var frame = Data([0x80 | opcode])
        switch payload.count {
        case ..<126: frame.append(UInt8(payload.count))
        case ..<65536: frame.append(126); frame.append(contentsOf: [UInt8(payload.count >> 8), UInt8(payload.count & 0xFF)])
        default:
            frame.append(127)
            frame.append(contentsOf: (0..<8).reversed().map { UInt8((payload.count >> ($0 * 8)) & 0xFF) })
        }
        write(frame + payload)
    }

    func sendClose(_ code: UInt16) { sendFrame(opcode: 0x8, Data([UInt8(code >> 8), UInt8(code & 0xFF)])) }

    /// One client frame (opcode, unmasked payload).
    func readFrame() -> (opcode: UInt8, payload: Data)? {
        guard let head = take(2) else { return nil }
        let opcode = head[head.startIndex] & 0x0F
        var length = Int(head[head.startIndex + 1] & 0x7F)
        if length == 126 {
            guard let ext = take(2) else { return nil }
            length = ext.reduce(0) { $0 << 8 | Int($1) }
        } else if length == 127 {
            guard let ext = take(8) else { return nil }
            length = ext.reduce(0) { $0 << 8 | Int($1) }
        }
        let masked = head[head.startIndex + 1] & 0x80 != 0
        let mask = masked ? take(4).map(Array.init) ?? [] : []
        guard let payload = take(length) else { return nil }
        let bytes = masked ? payload.enumerated().map { $0.element ^ mask[$0.offset % 4] } : Array(payload)
        return (opcode, Data(bytes))
    }
}

func httpResponse(_ status: String = "200 OK", headers: [String] = [], body: Data = Data(), length: Bool = true, close: Bool = true) -> Data {
    var head = "HTTP/1.1 \(status)\r\n"
    if close { head += "Connection: close\r\n" }
    if length { head += "Content-Length: \(body.count)\r\n" }
    for h in headers { head += h + "\r\n" }
    return Data((head + "\r\n").utf8) + body
}

/// A thread-safe box the server threads fill and the test reads.
final class Box<T: Sendable>: @unchecked Sendable {
    private let lock = NSLock()
    private var value: T
    init(_ value: T) { self.value = value }
    var get: T { lock.withLock { value } }
    func update(_ change: (inout T) -> Void) { lock.withLock { change(&value) } }
}

/// Polls `condition` for up to `seconds`.
func eventually(_ seconds: Double = 5, _ condition: () -> Bool) async -> Bool {
    let start = Date()
    while Date().timeIntervalSince(start) < seconds {
        if condition() { return true }
        try? await Task.sleep(for: .milliseconds(20))
    }
    return condition()
}
