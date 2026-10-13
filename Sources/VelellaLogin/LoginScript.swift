// The capture hook is adapted from Serein (https://github.com/ViceVerse-cz/Serein, MIT OR Apache-2.0):
// crates/platform/src/login-handoff.js. Changes: it posts to a WebKit script message handler,
// whose reference it takes at document start, instead of wry's `window.ipc`; Serein's capability
// prefix is dropped (the native side checks the sending webview, frame and origin instead).

enum LoginScript {
    /// Name of the script message handler the hook posts to.
    static let handlerName = "velellaLogin"

    /// Injected at document start, main frame only, in the page world. Observes the page's own
    /// Authorization header on same-origin discord.com API calls once the user has signed in.
    /// Discord runs the whole sign-in flow itself. One-shot; never writes to the console.
    static let source = #"""
    (() => {
      if (window !== window.top || location.origin !== "https://discord.com") return;
      const handler = window.webkit && window.webkit.messageHandlers && window.webkit.messageHandlers.velellaLogin;
      if (!handler) return;
      const post = handler.postMessage.bind(handler);
      let delivered = false;
      const opened = Date.now();
      const allowed = value => {
        try {
          const url = new URL(value, location.href);
          return url.origin === "https://discord.com" && /^\/api\/v\d+\//.test(url.pathname);
        } catch { return false; }
      };
      const deliver = value => {
        if (delivered || Date.now() - opened > 600000 || typeof value !== "string" || value.length < 16 || value.length > 2048 || /\s/.test(value)) return;
        delivered = true;
        post(value);
      };
      const destinations = new WeakMap();
      const open = XMLHttpRequest.prototype.open;
      const setHeader = XMLHttpRequest.prototype.setRequestHeader;
      XMLHttpRequest.prototype.open = function(method, url, ...rest) {
        destinations.set(this, allowed(url));
        return open.call(this, method, url, ...rest);
      };
      XMLHttpRequest.prototype.setRequestHeader = function(name, value) {
        if (destinations.get(this) && String(name).toLowerCase() === "authorization") deliver(value);
        return setHeader.call(this, name, value);
      };
      const originalFetch = window.fetch;
      window.fetch = function(input, init) {
        if (allowed(input instanceof Request ? input.url : input)) {
          const headers = new Headers(init?.headers ?? (input instanceof Request ? input.headers : undefined));
          deliver(headers.get("authorization"));
        }
        return originalFetch.apply(this, arguments);
      };
    })();
    """#
}
