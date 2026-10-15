import AppKit
import Foundation
import ImageIO
import Testing
import UniformTypeIdentifiers
@testable import VelellaUI

// Wave 3 E: the media loader against a local HTTP server on 127.0.0.1 (the test-only seam maps a
// checked Discord URL to it). Nothing here leaves the machine.

// MARK: Host check (mirrors core's `media_url` table)

@Test func onlyDiscordMediaHostsOverHTTPSAreFetchable() {
    let accepted = [
        "https://media.discordapp.net/a.png", "https://images-ext-1.discordapp.net/a.png",
        "https://images-ext-2.discordapp.net/a.png", "https://cdn.discordapp.com/a.png",
        "https://MEDIA.discordapp.net/a.png",
    ]
    for raw in accepted { #expect(MediaImages.fetchable(URL(string: raw)!), "\(raw)") }
    // Core drops a default `:443`; Swift sees no port then, and refuses any explicit one.
    let rejected = [
        "http://media.discordapp.net/a.png", "https://media.discordapp.net:8443/a.png",
        "https://media.discordapp.net:443/a.png",
        "https://user@media.discordapp.net/a.png", "https://user:pw@media.discordapp.net/a.png",
        "https://media.discordapp.net./a.png", "https://media.discordapp.net.evil.test/a.png",
        "https://m\u{0435}dia.discordapp.net/a.png", "//media.discordapp.net/a.png",
        "file:///etc/passwd", "data:image/png;base64,AAAA", "https://discordapp.net/a.png",
        "https://evil.test/https://media.discordapp.net/a.png",
    ]
    for raw in rejected {
        if let url = URL(string: raw) { #expect(!MediaImages.fetchable(url), "\(raw)") }
    }
    // Not even a URL in Swift: these never reach a fetch.
    #expect(URL(string: "") == nil)
    if let url = URL(string: "https://evil.test\\@media.discordapp.net/") { #expect(!MediaImages.fetchable(url)) }
}

@Test func theProxyIsAskedForTheDisplaySize() {
    let url = URL(string: "https://media.discordapp.net/attachments/1/2/a.png?ex=1&width=9")!
    let requested = MediaImages.request(url, box: CGSize(width: 100, height: 50.2), scale: 2)
    let items = URLComponents(url: requested, resolvingAgainstBaseURL: false)?.queryItems ?? []
    #expect(items.map(\.name) == ["ex", "width", "height"])
    #expect(items.first { $0.name == "width" }?.value == "200")
    #expect(items.first { $0.name == "height" }?.value == "101")
    // The CDN is not a resizing proxy: its URL is kept as it is.
    let cdn = URL(string: "https://cdn.discordapp.com/emojis/1.webp?size=48")!
    #expect(MediaImages.request(cdn, box: CGSize(width: 10, height: 10), scale: 2) == cdn)
    // Extreme or broken boxes never trap.
    #expect(MediaImages.request(url, box: CGSize(width: .infinity, height: 1e300), scale: 2) != url)
}

// MARK: Local server

/// A one-thread HTTP/1.1 server on 127.0.0.1 answering each connection with the next canned
/// response, then closing it. Records each request's head.
final class LocalServer: @unchecked Sendable {
    let port: UInt16
    private let fd: Int32
    private let lock = NSLock()
    private var heads: [String] = []
    var requests: [String] { lock.withLock { heads } }

    init(_ responses: [Data]) throws {
        let s = socket(AF_INET, SOCK_STREAM, 0)
        var addr = sockaddr_in()
        addr.sin_len = UInt8(MemoryLayout<sockaddr_in>.size)
        addr.sin_family = sa_family_t(AF_INET)
        addr.sin_port = 0
        addr.sin_addr.s_addr = inet_addr("127.0.0.1")
        var len = socklen_t(MemoryLayout<sockaddr_in>.size)
        let bound = withUnsafeMutablePointer(to: &addr) {
            $0.withMemoryRebound(to: sockaddr.self, capacity: 1) { p in
                bind(s, p, len) == 0 && listen(s, 8) == 0 && getsockname(s, p, &len) == 0
            }
        }
        guard bound else {
            Darwin.close(s)
            throw POSIXError(.EADDRNOTAVAIL)
        }
        fd = s
        port = UInt16(bigEndian: addr.sin_port)
        Thread { [self] in serve(responses) }.start()
    }

    private func serve(_ responses: [Data]) {
        for response in responses {
            let c = accept(fd, nil, nil)
            guard c >= 0 else { return }
            var one: Int32 = 1
            setsockopt(c, SOL_SOCKET, SO_NOSIGPIPE, &one, socklen_t(MemoryLayout<Int32>.size))
            let head = Self.readHead(c)
            lock.withLock { heads.append(head) }
            Self.write(response, to: c)
            Darwin.close(c)
        }
    }

    deinit { Darwin.close(fd) }

    private static func readHead(_ c: Int32) -> String {
        var data = Data()
        var buffer = [UInt8](repeating: 0, count: 4096)
        while !data.contains("\r\n\r\n".data(using: .utf8)!) {
            let n = read(c, &buffer, buffer.count)
            if n <= 0 { break }
            data.append(buffer, count: n)
        }
        return String(decoding: data, as: UTF8.self)
    }

    private static func write(_ data: Data, to c: Int32) {
        data.withUnsafeBytes { raw in
            var offset = 0
            while offset < raw.count {
                let n = Darwin.write(c, raw.baseAddress! + offset, raw.count - offset)
                if n <= 0 { return }
                offset += n
            }
        }
    }

    /// `https://<media host>/path?query` → this server, same path and query.
    var rewrite: @Sendable (URL) -> URL {
        let port = port
        return { url in
            var parts = URLComponents(url: url, resolvingAgainstBaseURL: false)!
            parts.scheme = "http"; parts.host = "127.0.0.1"; parts.port = Int(port)
            return parts.url!
        }
    }
}

func response(_ status: String = "200 OK", headers: [String] = [], body: Data, length: Bool = true) -> Data {
    var head = "HTTP/1.1 \(status)\r\nConnection: close\r\nContent-Type: image/png\r\n"
    if length { head += "Content-Length: \(body.count)\r\n" }
    for h in headers { head += h + "\r\n" }
    return head.data(using: .utf8)! + "\r\n".data(using: .utf8)! + body
}

func png(width: Int, height: Int) -> Data {
    let context = CGContext(data: nil, width: width, height: height, bitsPerComponent: 8, bytesPerRow: 0,
                            space: CGColorSpaceCreateDeviceGray(), bitmapInfo: CGImageAlphaInfo.none.rawValue)!
    let image = context.makeImage()!
    let out = NSMutableData()
    let destination = CGImageDestinationCreateWithData(out, UTType.png.identifier as CFString, 1, nil)!
    CGImageDestinationAddImage(destination, image, nil)
    CGImageDestinationFinalize(destination)
    return out as Data
}

/// A PNG whose header declares `width` × `height` with no image data behind it.
private func declaredPNG(width: UInt32, height: UInt32) -> Data {
    func crc(_ bytes: [UInt8]) -> UInt32 {
        var c: UInt32 = 0xFFFF_FFFF
        for b in bytes {
            c ^= UInt32(b)
            for _ in 0..<8 { c = c & 1 == 1 ? 0xEDB8_8320 ^ (c >> 1) : c >> 1 }
        }
        return ~c
    }
    func be(_ v: UInt32) -> [UInt8] { [UInt8(v >> 24), UInt8(v >> 16 & 0xFF), UInt8(v >> 8 & 0xFF), UInt8(v & 0xFF)] }
    func chunk(_ type: String, _ body: [UInt8]) -> [UInt8] {
        let typed = Array(type.utf8) + body
        return be(UInt32(body.count)) + typed + be(crc(typed))
    }
    let ihdr = be(width) + be(height) + [8, 0, 0, 0, 0]
    return Data([0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A] + chunk("IHDR", ihdr) + chunk("IEND", []))
}

private let media = URL(string: "https://media.discordapp.net/attachments/1/2/a.png")!
private let box = CGSize(width: 100, height: 50)

// MARK: Loader

@Test func aSmallImageLoadsAndTheRequestCarriesTheDisplaySize() async throws {
    let server = try LocalServer([response(body: png(width: 300, height: 150))])
    let loader = MediaImages(rewrite: server.rewrite)
    let image = await loader.image(media, box: box, scale: 2)
    #expect(image != nil)
    // A thumbnail at most twice the box's longest side.
    #expect((image?.width ?? 0) <= 200)
    let head = try #require(server.requests.first)
    #expect(head.hasPrefix("GET /attachments/1/2/a.png?width=200&height=100 HTTP/1.1\r\n"), "\(head)")
    // No cookie, credential or added header.
    #expect(!head.lowercased().contains("cookie"))
    #expect(!head.lowercased().contains("authorization"))
    // Cached: a second call sends nothing (the server has no second answer).
    #expect(await loader.image(media, box: box, scale: 2) != nil)
    #expect(server.requests.count == 1)
}

@Test func aRedirectToAForeignHostNeverContactsIt() async throws {
    let foreign = try LocalServer([response(body: png(width: 10, height: 10))])
    let server = try LocalServer([
        response("302 Found", headers: ["Location: http://127.0.0.1:\(foreign.port)/x.png"], body: Data()),
        response("302 Found", headers: ["Location: https://media.discordapp.net/b.png"], body: Data()),
        response(body: png(width: 10, height: 10)),
    ])
    let loader = MediaImages(rewrite: server.rewrite)
    #expect(await loader.image(media, box: box, scale: 2) == nil)
    try await Task.sleep(for: .milliseconds(200))
    #expect(foreign.requests.isEmpty)
    // A redirect to a media host is followed (through the seam).
    let other = URL(string: "https://media.discordapp.net/attachments/1/2/c.png")!
    #expect(await loader.image(other, box: box, scale: 2) != nil)
    #expect(server.requests.count == 3)
    #expect(server.requests.last?.hasPrefix("GET /b.png HTTP/1.1") == true)
}

@Test func aSetCookieBringsNoCookieToTheNextRequest() async throws {
    let server = try LocalServer([
        response(headers: ["Set-Cookie: s=secret; Path=/"], body: png(width: 10, height: 10)),
        response(body: png(width: 10, height: 10)),
    ])
    let loader = MediaImages(rewrite: server.rewrite)
    #expect(await loader.image(media, box: box, scale: 2) != nil)
    #expect(await loader.image(URL(string: "https://media.discordapp.net/b.png")!, box: box, scale: 2) != nil)
    #expect(server.requests.count == 2)
    #expect(!server.requests[1].lowercased().contains("cookie"), "\(server.requests[1])")
}

@Test func a17MiBBodyGivesNoImage() async throws {
    // A real image with 17 MiB behind it: without the cap it would decode.
    let body = png(width: 10, height: 10) + Data(count: 17 << 20)
    let server = try LocalServer([response(body: body), response(body: body, length: false)])
    let loader = MediaImages(rewrite: server.rewrite)
    // Declared length over the cap.
    #expect(await loader.image(media, box: box, scale: 2) == nil)
    // No declared length: refused while streaming.
    #expect(await loader.image(URL(string: "https://media.discordapp.net/b.png")!, box: box, scale: 2) == nil)
    #expect(server.requests.count == 2)
}

@Test func aPNGDeclaring20000By20000GivesNoImage() async throws {
    let server = try LocalServer([response(body: declaredPNG(width: 20000, height: 20000))])
    let loader = MediaImages(rewrite: server.rewrite)
    #expect(await loader.image(media, box: box, scale: 2) == nil)
}

@Test func imagesOverFiftyMegapixelsAreNeverDecoded() {
    // 8000 x 8000 = 64 MP decodes fine without the cap; 7000 x 7000 = 49 MP is under it.
    #expect(MediaImages.decode(png(width: 8000, height: 8000), maxSide: 200) == nil)
    let under = MediaImages.decode(png(width: 7000, height: 7000), maxSide: 200)
    #expect(under.map { Swift.max($0.width, $0.height) } == 200)
}

@Test func aURLFailingTheCheckIsNeverFetched() async throws {
    // The seam would carry any of these to the local server: the loader must refuse first.
    let server = try LocalServer(Array(repeating: response(body: png(width: 10, height: 10)), count: 3))
    let loader = MediaImages(rewrite: server.rewrite)
    for raw in ["https://evil.test/a.png", "http://media.discordapp.net/a.png", "https://media.discordapp.net:8443/a.png"] {
        #expect(await loader.image(URL(string: raw)!, box: box, scale: 2) == nil, "\(raw)")
    }
    try await Task.sleep(for: .milliseconds(100))
    #expect(server.requests.isEmpty)
}
