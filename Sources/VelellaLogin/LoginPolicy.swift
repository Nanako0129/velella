// Origin predicates adapted from Serein (https://github.com/ViceVerse-cz/Serein, MIT OR Apache-2.0):
// `discord_origin` / `login_navigation` in crates/platform/src/lib.rs and `hcaptcha_origin` in
// crates/platform/src/captcha.rs. Changes: Swift on Foundation `URL`; the main frame and subframes
// get separate rules; the native token-accept check works on a frame's security origin.

import Foundation

/// Pure decisions for the login webview. Hosts compare exactly (no case folding): WebKit hands
/// over canonical lowercase hosts, and anything else fails closed.
public enum LoginPolicy {
    /// https, host exactly `discord.com`, default port, no userinfo.
    public static func isDiscord(_ url: URL) -> Bool {
        url.scheme == "https" && url.host == "discord.com" && defaultHTTPSPort(url) && noUserInfo(url)
    }

    /// https, `hcaptcha.com` or a subdomain of it, port 443, no userinfo.
    public static func isHCaptcha(_ url: URL) -> Bool {
        guard url.scheme == "https", let host = url.host, defaultHTTPSPort(url), noUserInfo(url) else { return false }
        return host == "hcaptcha.com" || host.hasSuffix(".hcaptcha.com")
    }

    /// The main frame may only ever show discord.com.
    public static func allowsMainFrame(_ url: URL) -> Bool { isDiscord(url) }

    /// Subframes additionally host the hCaptcha challenge and blank/srcdoc frames.
    public static func allowsSubframe(_ url: URL) -> Bool {
        isDiscord(url) || isHCaptcha(url) || url.absoluteString == "about:blank" || url.absoluteString == "about:srcdoc"
    }

    /// A navigation action: `isMainFrame == nil` means no target frame (a new-window request),
    /// which is always cancelled, as is anything WebKit wants to download.
    public static func allowsNavigation(to url: URL?, isMainFrame: Bool?, shouldDownload: Bool) -> Bool {
        guard let url, let isMainFrame, !shouldDownload else { return false }
        return isMainFrame ? allowsMainFrame(url) : allowsSubframe(url)
    }

    /// The token arrives only from our own webview's main frame on https://discord.com (WebKit
    /// reports port 0 for the scheme default), only as a string, and only once.
    public static func acceptedToken(
        fromLoginWebView: Bool, isMainFrame: Bool, origin: FrameOrigin, body: Any, alreadyCaptured: Bool
    ) -> String? {
        guard fromLoginWebView, isMainFrame, !alreadyCaptured,
              origin.scheme == "https", origin.host == "discord.com", origin.port == 0 || origin.port == 443,
              let token = body as? String
        else { return nil }
        return token
    }

    private static func defaultHTTPSPort(_ url: URL) -> Bool { url.port == nil || url.port == 443 }
    private static func noUserInfo(_ url: URL) -> Bool { url.user == nil && url.password == nil }
}

/// `WKSecurityOrigin`'s three fields, as a value the predicate (and its tests) can take.
public struct FrameOrigin: Equatable, Sendable {
    public var scheme: String
    public var host: String
    public var port: Int
    public init(scheme: String, host: String, port: Int) {
        self.scheme = scheme; self.host = host; self.port = port
    }
}
