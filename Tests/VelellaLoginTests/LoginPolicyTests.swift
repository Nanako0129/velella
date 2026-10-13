import Foundation
import Testing
@testable import VelellaLogin

/// Serein's adversarial origins (refs/serein/crates/platform/src/lib.rs, test
/// `handoff_accepts_only_our_discord_origin`).
private let sereinAdversarial = [
    "http://discord.com",
    "https://discord.com.evil.test",
    "https://evil.test/discord.com",
    "https://user@discord.com",
    "https://discord.com:444",
]

private func url(_ string: String) -> URL { URL(string: string)! }

@Test func mainFrameAllowsOnlyDiscord() {
    #expect(LoginPolicy.allowsMainFrame(url("https://discord.com/login")))
    #expect(LoginPolicy.allowsMainFrame(url("https://discord.com:443/channels/@me")))
    for bad in sereinAdversarial + [
        "https://user:pw@discord.com/", "https://DISCORD.com/", "discord://login", "file:///etc/passwd",
        "about:blank", "javascript:alert(1)", "https://ptb.discord.com/",
    ] {
        #expect(!LoginPolicy.allowsMainFrame(url(bad)), "\(bad)")
    }
}

@Test func hCaptchaAsMainFrameIsRejected() {
    #expect(!LoginPolicy.allowsMainFrame(url("https://newassets.hcaptcha.com/captcha/")))
    #expect(!LoginPolicy.allowsMainFrame(url("https://hcaptcha.com/")))
}

@Test func subframeAllowsDiscordHCaptchaAndBlankOnly() {
    for good in [
        "https://discord.com/", "https://newassets.hcaptcha.com/captcha/", "https://hcaptcha.com/",
        "https://hcaptcha.com:443/x", "about:blank", "about:srcdoc",
    ] {
        #expect(LoginPolicy.allowsSubframe(url(good)), "\(good)")
    }
    for bad in sereinAdversarial + [
        "http://hcaptcha.com/", "https://hcaptcha.com.evil.test/", "https://evilhcaptcha.com/",
        "https://user@newassets.hcaptcha.com/", "https://newassets.hcaptcha.com:8443/",
        "about:config", "data:text/html,x", "discord://x",
    ] {
        #expect(!LoginPolicy.allowsSubframe(url(bad)), "\(bad)")
    }
}

@Test func navigationWithoutTargetFrameOrAsDownloadIsCancelled() {
    let login = url("https://discord.com/login")
    #expect(LoginPolicy.allowsNavigation(to: login, isMainFrame: true, shouldDownload: false))
    #expect(!LoginPolicy.allowsNavigation(to: login, isMainFrame: nil, shouldDownload: false))
    #expect(!LoginPolicy.allowsNavigation(to: login, isMainFrame: true, shouldDownload: true))
    #expect(!LoginPolicy.allowsNavigation(to: nil, isMainFrame: true, shouldDownload: false))
    #expect(!LoginPolicy.allowsNavigation(to: url("https://hcaptcha.com/"), isMainFrame: true, shouldDownload: false))
    #expect(LoginPolicy.allowsNavigation(to: url("https://hcaptcha.com/"), isMainFrame: false, shouldDownload: false))
}

private let discordOrigin = FrameOrigin(scheme: "https", host: "discord.com", port: 0)
private let token = "SYNTHETIC_TOKEN_MARKER_0123456789"

private func accept(
    origin: FrameOrigin = discordOrigin, ours: Bool = true, mainFrame: Bool = true, body: Any = token, captured: Bool = false
) -> String? {
    LoginPolicy.acceptedToken(fromLoginWebView: ours, isMainFrame: mainFrame, origin: origin, body: body, alreadyCaptured: captured)
}

@Test func tokenAcceptedOnlyFromOurDiscordMainFrame() {
    #expect(accept() == token)
    #expect(accept(origin: FrameOrigin(scheme: "https", host: "discord.com", port: 443)) == token)
    // Serein's list as WebKit security origins (an origin carries no userinfo or path; the
    // navigation predicates keep `https://user@discord.com` from ever loading).
    for adversarial in sereinAdversarial where !adversarial.contains("@") {
        let parts = URLComponents(string: adversarial)!
        let origin = FrameOrigin(scheme: parts.scheme!, host: parts.host!, port: parts.port ?? 0)
        #expect(accept(origin: origin) == nil, "\(adversarial)")
    }
    #expect(accept(origin: FrameOrigin(scheme: "https", host: "newassets.hcaptcha.com", port: 0)) == nil)
    // Host must equal discord.com exactly: no subdomain, no look-alike suffix.
    for host in ["ptb.discord.com", "canary.discord.com", "evildiscord.com"] {
        #expect(accept(origin: FrameOrigin(scheme: "https", host: host, port: 0)) == nil, "\(host)")
    }
    // Port must be the https default: reported as 0, or 443 explicitly.
    for port in [80, 444, 8443] {
        #expect(accept(origin: FrameOrigin(scheme: "https", host: "discord.com", port: port)) == nil, "\(port)")
    }
    #expect(accept(ours: false) == nil)
    #expect(accept(body: 42) == nil)
    #expect(accept(body: ["token": token]) == nil)
    #expect(accept(captured: true) == nil)
}

@Test func tokenFromDiscordSubframeIsRejected() {
    #expect(accept(mainFrame: false) == nil)
}
