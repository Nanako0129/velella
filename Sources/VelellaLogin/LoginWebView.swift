import AppKit
import SwiftUI
import WebKit

public enum LoginWebViewFactory {
    /// A throwaway webview for discord.com/login: nothing it stores outlives it, the capture hook
    /// runs in the main frame only, no inspector, no link previews.
    @MainActor
    public static func make(userAgent: String, messageHandler: WKScriptMessageHandler) -> WKWebView {
        let configuration = WKWebViewConfiguration()
        // Before init: WKWebView copies its configuration, so a store set afterwards never applies.
        configuration.websiteDataStore = .nonPersistent()
        let content = WKUserContentController()
        content.addUserScript(WKUserScript(
            source: LoginScript.source, injectionTime: .atDocumentStart, forMainFrameOnly: true, in: .page
        ))
        content.add(messageHandler, contentWorld: .page, name: LoginScript.handlerName)
        configuration.userContentController = content
        let webView = WKWebView(frame: .zero, configuration: configuration)
        webView.customUserAgent = userAgent
        webView.allowsLinkPreview = false
        webView.isInspectable = false
        return webView
    }
}

/// Owns one login webview: its navigation and UI rules, the native half of the capture check, and
/// its teardown. Holds no token: a captured token goes straight to `onToken`.
@MainActor
public final class LoginController: NSObject, WKNavigationDelegate, WKUIDelegate {
    public static let loginURL = URL(string: "https://discord.com/login")!

    private let userAgent: String
    private let onToken: @MainActor (String) -> Void
    private(set) var webView: WKWebView?
    private(set) var handler: ScriptMessageProxy?
    private var captured = false
    private var tornDown = false

    public init(userAgent: String, onToken: @escaping @MainActor (String) -> Void) {
        self.userAgent = userAgent
        self.onToken = onToken
    }

    /// Builds the webview; does not load anything.
    public func makeWebView() -> WKWebView {
        let handler = ScriptMessageProxy()
        handler.controller = self
        let webView = LoginWebViewFactory.make(userAgent: userAgent, messageHandler: handler)
        webView.navigationDelegate = self
        webView.uiDelegate = self
        self.webView = webView
        self.handler = handler
        return webView
    }

    public func load() {
        webView?.load(URLRequest(url: Self.loginURL))
    }

    /// Runs once, on capture or when the window closes, whichever comes first.
    public func teardown() {
        guard !tornDown else { return }
        tornDown = true
        guard let webView else { return }
        webView.stopLoading()
        let content = webView.configuration.userContentController
        content.removeAllScriptMessageHandlers()
        content.removeAllUserScripts()
        webView.navigationDelegate = nil
        webView.uiDelegate = nil
        webView.removeFromSuperview()
        self.webView = nil
        handler = nil
    }

    fileprivate func receive(_ message: WKScriptMessage) {
        guard let webView, let token = LoginPolicy.acceptedToken(
            fromLoginWebView: message.webView === webView,
            isMainFrame: message.frameInfo.isMainFrame,
            origin: FrameOrigin(message.frameInfo.securityOrigin),
            body: message.body,
            alreadyCaptured: captured
        ) else { return }
        captured = true
        teardown()
        onToken(token)
    }

    // MARK: WKNavigationDelegate

    public func webView(
        _ webView: WKWebView, decidePolicyFor navigationAction: WKNavigationAction,
        decisionHandler: @escaping @MainActor @Sendable (WKNavigationActionPolicy) -> Void
    ) {
        let url = navigationAction.request.url
        if LoginPolicy.allowsNavigation(
            to: url, isMainFrame: navigationAction.targetFrame?.isMainFrame,
            shouldDownload: navigationAction.shouldPerformDownload
        ) {
            decisionHandler(.allow)
        } else {
            // Scheme and host only: paths and queries can carry codes and state.
            let line = "velella: login navigation blocked \(url?.scheme ?? "-") \(url?.host ?? "-")\n"
            FileHandle.standardError.write(Data(line.utf8))
            decisionHandler(.cancel)
        }
    }

    public func webView(
        _ webView: WKWebView, decidePolicyFor navigationResponse: WKNavigationResponse,
        decisionHandler: @escaping @MainActor @Sendable (WKNavigationResponsePolicy) -> Void
    ) {
        decisionHandler(navigationResponse.canShowMIMEType ? .allow : .cancel)
    }

    // MARK: WKUIDelegate

    public func webView(
        _ webView: WKWebView, createWebViewWith configuration: WKWebViewConfiguration,
        for navigationAction: WKNavigationAction, windowFeatures: WKWindowFeatures
    ) -> WKWebView? {
        nil
    }

    public func webView(
        _ webView: WKWebView, runOpenPanelWith parameters: WKOpenPanelParameters,
        initiatedByFrame frame: WKFrameInfo,
        completionHandler: @escaping @MainActor @Sendable ([URL]?) -> Void
    ) {
        completionHandler(nil)
    }

    public func webView(
        _ webView: WKWebView, requestMediaCapturePermissionFor origin: WKSecurityOrigin,
        initiatedByFrame frame: WKFrameInfo, type: WKMediaCaptureType,
        decisionHandler: @escaping @MainActor @Sendable (WKPermissionDecision) -> Void
    ) {
        decisionHandler(.deny)
    }
}

/// Forwards script messages without retaining the controller (the user content controller retains
/// its handlers).
@MainActor
final class ScriptMessageProxy: NSObject, WKScriptMessageHandler {
    weak var controller: LoginController?

    func userContentController(_ userContentController: WKUserContentController, didReceive message: WKScriptMessage) {
        controller?.receive(message)
    }
}

extension FrameOrigin {
    @MainActor init(_ origin: WKSecurityOrigin) {
        self.init(scheme: origin.protocol, host: origin.host, port: origin.port)
    }
}

/// The login page as a SwiftUI view. Its coordinator is the controller, so each window gets a
/// fresh webview, and closing the window tears it down.
public struct LoginWebView: NSViewRepresentable {
    private let userAgent: String
    private let onToken: @MainActor (String) -> Void

    public init(userAgent: String, onToken: @escaping @MainActor (String) -> Void) {
        self.userAgent = userAgent
        self.onToken = onToken
    }

    public func makeCoordinator() -> LoginController {
        LoginController(userAgent: userAgent, onToken: onToken)
    }

    public func makeNSView(context: Context) -> WKWebView {
        let webView = context.coordinator.makeWebView()
        context.coordinator.load()
        return webView
    }

    public func updateNSView(_ nsView: WKWebView, context: Context) {}

    public static func dismantleNSView(_ nsView: WKWebView, coordinator: LoginController) {
        coordinator.teardown()
    }
}
