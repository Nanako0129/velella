import AppKit
import Testing
import WebKit
@testable import VelellaLogin

// Nothing here loads a page: the webviews are built and torn down offline.

@MainActor @Test func factoryUsesNonPersistentStoreAndMainFrameOnlyScripts() {
    let controller = LoginController(userAgent: "TestUA/1.0") { _ in }
    let webView = controller.makeWebView()
    #expect(webView.configuration.websiteDataStore.isPersistent == false)
    let scripts = webView.configuration.userContentController.userScripts
    #expect(!scripts.isEmpty)
    #expect(scripts.allSatisfy { $0.isForMainFrameOnly && $0.injectionTime == .atDocumentStart })
    #expect(webView.customUserAgent == "TestUA/1.0")
    #expect(webView.allowsLinkPreview == false)
    #expect(webView.isInspectable == false)
    #expect(webView.navigationDelegate === controller)
    #expect(webView.uiDelegate === controller)
    controller.teardown()
}

@MainActor @Test func teardownReleasesHandlerAndWebView() {
    let controller = LoginController(userAgent: "TestUA/1.0") { _ in }
    weak var webView: WKWebView?
    weak var handler: ScriptMessageProxy?
    autoreleasepool {
        let host = NSView(frame: NSRect(x: 0, y: 0, width: 100, height: 100))
        let made = controller.makeWebView()
        host.addSubview(made)
        webView = made
        handler = controller.handler
    }
    #expect(webView != nil && handler != nil)
    controller.teardown()
    controller.teardown() // one-shot: a second run is a no-op
    RunLoop.main.run(until: Date().addingTimeInterval(0.05))
    #expect(handler == nil)
    #expect(webView == nil)
}
