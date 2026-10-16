import CryptoKit
import Foundation
import Testing
import VelellaFFI
@testable import VelellaTransport

// Plan NET Measurement: the transport's TLS ClientHello, captured by a listener on localhost that
// records it and aborts the handshake (no certificate is involved, nothing is trusted). Written
// to `measurement/clienthello-velella.txt` (git-ignored) for main to compare with Safari's.
// HTTP/2 SETTINGS, header order and the WebSocket upgrade headers need a finished handshake and
// are not measured here.

struct ClientHello {
    var recordVersion: UInt16 = 0
    var clientVersion: UInt16 = 0
    var ciphers: [UInt16] = []
    var extensions: [UInt16] = []
    var sni: String?
    var alpn: [String] = []
    var groups: [UInt16] = []
    var signatureAlgorithms: [UInt16] = []
    var supportedVersions: [UInt16] = []
    var keyShares: [UInt16] = []
    var pointFormats: [UInt8] = []
    var compressCertificate: [UInt16] = []

    static func isGrease(_ v: UInt16) -> Bool { v & 0x0F0F == 0x0A0A && v >> 8 == v & 0xFF }

    /// Parses one TLS record holding a ClientHello.
    init?(_ bytes: [UInt8]) {
        var i = 0
        func u8() -> UInt8? { guard i < bytes.count else { return nil }; defer { i += 1 }; return bytes[i] }
        func u16() -> UInt16? { guard let a = u8(), let b = u8() else { return nil }; return UInt16(a) << 8 | UInt16(b) }
        func skip(_ n: Int) -> Bool { i += n; return i <= bytes.count }
        guard u8() == 0x16, let rv = u16(), u16() != nil, u8() == 0x01, skip(3), let cv = u16(), skip(32),
              let sid = u8(), skip(Int(sid)), let cl = u16() else { return nil }
        recordVersion = rv
        clientVersion = cv
        for _ in 0..<(Int(cl) / 2) { guard let c = u16() else { return nil }; ciphers.append(c) }
        guard let comp = u8(), skip(Int(comp)), let total = u16() else { return nil }
        let end = i + Int(total)
        while i < end {
            guard let type = u16(), let len = u16() else { return nil }
            extensions.append(type)
            let data = Array(bytes[i..<min(i + Int(len), bytes.count)])
            guard skip(Int(len)) else { return nil }
            func list16(_ from: Int) -> [UInt16] {
                stride(from: from, to: data.count - 1, by: 2).map { UInt16(data[$0]) << 8 | UInt16(data[$0 + 1]) }
            }
            switch type {
            case 0x0000 where data.count > 5:
                sni = String(decoding: data[5...], as: UTF8.self)
            case 0x0010:
                var j = 2
                while j < data.count {
                    let n = Int(data[j]); alpn.append(String(decoding: data[(j + 1)..<min(j + 1 + n, data.count)], as: UTF8.self)); j += 1 + n
                }
            case 0x000A: groups = list16(2)
            case 0x000D: signatureAlgorithms = list16(2)
            case 0x002B: supportedVersions = list16(1)
            case 0x000B where !data.isEmpty: pointFormats = Array(data.dropFirst())
            case 0x001B where !data.isEmpty: compressCertificate = list16(1)
            case 0x0033:
                var j = 2
                while j + 4 <= data.count {
                    keyShares.append(UInt16(data[j]) << 8 | UInt16(data[j + 1]))
                    j += 4 + (Int(data[j + 2]) << 8 | Int(data[j + 3]))
                }
            default: break
            }
        }
    }

    static func hex(_ v: UInt16) -> String { String(format: "%04x", v) }
    static func sha12(_ s: String) -> String {
        SHA256.hash(data: Data(s.utf8)).map { String(format: "%02x", $0) }.joined().prefix(12).description
    }

    /// JA4 (FoxIO): `t` + version + SNI + counts + ALPN, then the hashed sorted ciphers, then the
    /// hashed sorted extensions (without SNI and ALPN) with the signature algorithms in order.
    var ja4: (ja4: String, raw: String) {
        let ciphers = ciphers.filter { !Self.isGrease($0) }
        let exts = extensions.filter { !Self.isGrease($0) }
        let top = (supportedVersions.filter { !Self.isGrease($0) }.max() ?? clientVersion)
        let version = switch top {
        case 0x0304: "13"
        case 0x0303: "12"
        case 0x0302: "11"
        case 0x0301: "10"
        default: "00"
        }
        let alpnPart = alpn.first.map { a in a.isEmpty ? "00" : "\(a.first!)\(a.last!)" } ?? "00"
        let a = "t\(version)\(sni == nil ? "i" : "d")\(String(format: "%02d", min(ciphers.count, 99)))"
            + "\(String(format: "%02d", min(exts.count, 99)))\(alpnPart)"
        let b = ciphers.map(Self.hex).sorted().joined(separator: ",")
        let sortedExts = exts.filter { $0 != 0x0000 && $0 != 0x0010 }.map(Self.hex).sorted().joined(separator: ",")
        let sigs = signatureAlgorithms.filter { !Self.isGrease($0) }.map(Self.hex).joined(separator: ",")
        let c = sigs.isEmpty ? sortedExts : "\(sortedExts)_\(sigs)"
        return ("\(a)_\(ciphers.isEmpty ? "000000000000" : Self.sha12(b))_\(Self.sha12(c))", "\(a)_\(b)_\(c)")
    }
}

func names(_ values: [UInt16], _ table: [UInt16: String]) -> String {
    values.map { v in
        ClientHello.isGrease(v) ? "GREASE" : "\(ClientHello.hex(v))" + (table[v].map { " \($0)" } ?? "")
    }.joined(separator: ", ")
}

let extensionNames: [UInt16: String] = [
    0x0000: "server_name", 0x0005: "status_request", 0x000A: "supported_groups", 0x000B: "ec_point_formats",
    0x000D: "signature_algorithms", 0x0010: "alpn", 0x0012: "signed_certificate_timestamp", 0x0015: "padding",
    0x0017: "extended_master_secret", 0x001B: "compress_certificate", 0x0023: "session_ticket",
    0x002B: "supported_versions", 0x002D: "psk_key_exchange_modes", 0x0033: "key_share", 0xFF01: "renegotiation_info",
    0x0029: "pre_shared_key", 0x0039: "quic_transport_parameters", 0x44CD: "application_settings",
    0xFE0D: "encrypted_client_hello",
]
let groupNames: [UInt16: String] = [
    0x001D: "x25519", 0x0017: "secp256r1", 0x0018: "secp384r1", 0x0019: "secp521r1", 0x11EC: "X25519MLKEM768",
    0x6399: "X25519Kyber768Draft00",
]
let cipherNames: [UInt16: String] = [
    0x1301: "TLS_AES_128_GCM_SHA256", 0x1302: "TLS_AES_256_GCM_SHA384", 0x1303: "TLS_CHACHA20_POLY1305_SHA256",
    0xC02C: "ECDHE_ECDSA_AES_256_GCM", 0xC02B: "ECDHE_ECDSA_AES_128_GCM", 0xCCA9: "ECDHE_ECDSA_CHACHA20",
    0xC030: "ECDHE_RSA_AES_256_GCM", 0xC02F: "ECDHE_RSA_AES_128_GCM", 0xCCA8: "ECDHE_RSA_CHACHA20",
    0xC00A: "ECDHE_ECDSA_AES_256_CBC_SHA", 0xC009: "ECDHE_ECDSA_AES_128_CBC_SHA", 0xC014: "ECDHE_RSA_AES_256_CBC_SHA",
    0xC013: "ECDHE_RSA_AES_128_CBC_SHA", 0x009D: "RSA_AES_256_GCM", 0x009C: "RSA_AES_128_GCM",
    0x0035: "RSA_AES_256_CBC_SHA", 0x002F: "RSA_AES_128_CBC_SHA", 0xC008: "ECDHE_ECDSA_3DES_EDE_CBC_SHA",
    0xC012: "ECDHE_RSA_3DES_EDE_CBC_SHA", 0x000A: "RSA_3DES_EDE_CBC_SHA", 0x5600: "TLS_FALLBACK_SCSV",
]

func describe(_ hello: ClientHello) -> String {
    let (ja4, raw) = hello.ja4
    return """
    JA4: \(ja4)
    JA4_r: \(raw)
    record version: \(ClientHello.hex(hello.recordVersion)); client_version: \(ClientHello.hex(hello.clientVersion))
    SNI: \(hello.sni.map { "present (\($0))" } ?? "absent")
    ALPN: \(hello.alpn.joined(separator: ", "))
    cipher suites (\(hello.ciphers.count), as sent): \(names(hello.ciphers, cipherNames))
    extensions (\(hello.extensions.count), as sent): \(names(hello.extensions, extensionNames))
    supported_groups: \(names(hello.groups, groupNames))
    key_share groups: \(names(hello.keyShares, groupNames))
    signature_algorithms: \(hello.signatureAlgorithms.map { ClientHello.isGrease($0) ? "GREASE" : ClientHello.hex($0) }.joined(separator: ", "))
    supported_versions: \(hello.supportedVersions.map { ClientHello.isGrease($0) ? "GREASE" : ClientHello.hex($0) }.joined(separator: ", "))
    ec_point_formats: \(hello.pointFormats.map { String($0) }.joined(separator: ", "))
    compress_certificate: \(hello.compressCertificate.map(ClientHello.hex).joined(separator: ", "))
    """
}

/// Records every ClientHello in order: after an aborted handshake URLSession tries once more
/// (with TLS_FALLBACK_SCSV 0x5600), so the first attempt is the one a real server sees. The JA4
/// depends on the OS, so it is printed and written, never asserted.
@Test func captureTheTransportClientHello() async throws {
    let captured = Box<[[UInt8]]>([])
    let server = try TestServer { peer in
        guard let head = peer.take(5) else { return }
        let length = Int(head[3]) << 8 | Int(head[4])
        guard let body = peer.take(length) else { return }
        captured.update { $0.append(Array(head) + Array(body)) }
        peer.drop() // abort: no ServerHello, nothing to trust
    }
    let list = AllowList(
        http: [AllowList.Rule(scheme: "https", host: .exact("localhost"), port: Int(server.port), path: .prefix("/api/v9/"))],
        webSocket: [], upload: [], authorizedPrefix: "https://localhost:\(server.port)/api/v9/"
    )
    let transport = URLSessionTransport(allowList: list)
    let outcome = await transport.perform(try request("https://localhost:\(server.port)/api/v9/users/@me"))
    // A handshake that never completed: no request byte was written.
    guard case .failure(.connectFailed) = outcome else { Issue.record("\(outcome)"); return }
    try await Task.sleep(for: .milliseconds(300))
    let hellos = captured.get.compactMap(ClientHello.init)
    guard let first = hellos.first, hellos.count == captured.get.count else { Issue.record("unparsed ClientHello"); return }
    #expect(first.sni == "localhost")

    let safari = (NSDictionary(contentsOfFile: "/Applications/Safari.app/Contents/Info.plist")?["CFBundleShortVersionString"] as? String) ?? "unknown"
    var report = """
    velella transport ClientHello (URLSessionTransport, ephemeral configuration)
    captured: \(ISO8601DateFormatter().string(from: Date())) on \(ProcessInfo.processInfo.operatingSystemVersionString), target https://localhost:<port>/
    attempts: \(hellos.count). Attempt 1 is the one that counts (what a server that answers sees); later ones are URLSession's retries after the aborted handshake.
    User-Agent velella sends: \(ua)
    This machine's Safari (Info.plist CFBundleShortVersionString): \(safari)
    Not measured (need a finished handshake): HTTP/2 SETTINGS and frame order, header order, WebSocket upgrade headers.

    """
    for (i, hello) in hellos.enumerated() {
        report += "--- attempt \(i + 1)\(i == 0 ? " (counts)" : " (retry)") ---\n\(describe(hello))\n\n"
    }
    let root = URL(fileURLWithPath: #filePath).deletingLastPathComponent().deletingLastPathComponent().deletingLastPathComponent()
    let dir = root.appending(path: "measurement")
    try FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
    try report.write(to: dir.appending(path: "clienthello-velella.txt"), atomically: true, encoding: .utf8)
    print(report)
}
