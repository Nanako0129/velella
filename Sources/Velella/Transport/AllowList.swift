import Foundation

/// Where the transport may send anything (plan NET, T-1, T-10): (host, path rule) pairs fixed at
/// init. Core validates every URL first; this is the second, independent check, and the only one
/// for the identity fetch (slice ID). The shipped list is `shipped`; tests build their own for
/// 127.0.0.1 through the internal initializer. There is no URL rewrite hook.
struct AllowList: Sendable {
    enum Host: Sendable, Equatable {
        case exact(String)
        /// `gateway-<label>.discord.gg`, one label of letters, digits and hyphens.
        case gatewayLabel
    }

    enum Path: Sendable, Equatable {
        /// Path starts with this; a query is allowed.
        case prefix(String)
        /// Path is exactly this; a query only when `query`.
        case exact(String, query: Bool)
        /// `/assets/fast-connect.` + 16 of `[0-9a-f]` + `.js`, no query (D12's script).
        case fastConnect
    }

    struct Rule: Sendable, Equatable {
        let scheme: String
        let host: Host
        /// nil: the URL must name no port.
        let port: Int?
        let path: Path
    }

    let http: [Rule]
    let webSocket: [Rule]
    /// Only URLs starting with this may carry an Authorization; a request elsewhere that has one
    /// is refused.
    let authorizedPrefix: String

    static let shipped = AllowList(
        http: [
            Rule(scheme: "https", host: .exact("discord.com"), port: nil, path: .prefix("/api/v9/")),
            Rule(scheme: "https", host: .exact("discord.com"), port: nil, path: .exact("/app", query: false)),
            Rule(scheme: "https", host: .exact("discord.com"), port: nil, path: .fastConnect),
        ],
        webSocket: [
            Rule(scheme: "wss", host: .exact("gateway.discord.gg"), port: nil, path: .exact("/", query: true)),
            Rule(scheme: "wss", host: .gatewayLabel, port: nil, path: .exact("/", query: true)),
        ],
        authorizedPrefix: "https://discord.com/api/v9/"
    )

    func allowsHTTP(_ url: URL) -> Bool { http.contains { Self.matches($0, url) } }
    func allowsWebSocket(_ url: URL) -> Bool { webSocket.contains { Self.matches($0, url) } }
    func authorizes(_ url: String) -> Bool { url.hasPrefix(authorizedPrefix) }

    private static func matches(_ rule: Rule, _ url: URL) -> Bool {
        guard url.scheme == rule.scheme, url.user == nil, url.password == nil, url.fragment == nil,
              url.port == rule.port, let host = url.host(percentEncoded: true) else { return false }
        let hostOK = switch rule.host {
        case .exact(let name): host == name
        case .gatewayLabel: isGatewayLabel(host)
        }
        guard hostOK else { return false }
        let path = url.path(percentEncoded: true)
        // A dot segment (plain or percent-encoded) could be resolved into another path by the
        // platform or a server, after this check passed.
        guard !path.split(separator: "/", omittingEmptySubsequences: false).contains(where: isDotSegment) else {
            return false
        }
        let query = url.query(percentEncoded: true)
        return switch rule.path {
        case .prefix(let prefix): path.hasPrefix(prefix)
        case .exact(let exact, let allowsQuery): path == exact && (allowsQuery || query == nil)
        case .fastConnect: query == nil && isFastConnect(path)
        }
    }

    /// `.` or `..`, each dot possibly written `%2e` / `%2E`.
    private static func isDotSegment(_ segment: Substring) -> Bool {
        let plain = segment.lowercased().replacingOccurrences(of: "%2e", with: ".")
        return plain == "." || plain == ".."
    }

    private static func isGatewayLabel(_ host: String) -> Bool {
        guard host.hasPrefix("gateway-"), host.hasSuffix(".discord.gg") else { return false }
        let label = host.dropFirst("gateway-".count).dropLast(".discord.gg".count)
        return !label.isEmpty && label.utf8.allSatisfy { $0.isASCIIAlphanumeric || $0 == UInt8(ascii: "-") }
    }

    private static func isFastConnect(_ path: String) -> Bool {
        let prefix = "/assets/fast-connect.", suffix = ".js"
        guard path.hasPrefix(prefix), path.hasSuffix(suffix) else { return false }
        let hash = path.utf8.dropFirst(prefix.utf8.count).dropLast(suffix.utf8.count)
        return hash.count == 16 && hash.allSatisfy { (0x30...0x39).contains($0) || (0x61...0x66).contains($0) }
    }
}

private extension UInt8 {
    var isASCIIAlphanumeric: Bool {
        (0x30...0x39).contains(self) || (0x41...0x5A).contains(self) || (0x61...0x7A).contains(self)
    }
}
