//! REST client for `https://discord.com/api/v9` (M2 W0.5). One `RestInner` per session holds the
//! token, the generation, the 401 latch, the global-429 deadline, the session's `Identity` and the
//! transport; every `Rest` clone shares it.
//!
//! Security rules, each with a named test in `rest_tests.rs`:
//! - The only destination is `https://discord.com/api/v9/...`: a constant base plus a typed path,
//!   validated before anything is sent; a typed path that the URL parser would normalise into
//!   another path is refused (S-1). Nothing can change the validated URL afterwards: the
//!   transport sends it byte for byte or refuses it.
//! - Requests go through the session's `Transport` (plan NET): on the app it is URLSession with
//!   the system's trust and proxy settings, like Safari (D15), no cookies, no cache and no
//!   redirects (the app's transport refuses them and its wire tests show it); core adds the
//!   deadline below, the body cap and the header checks at that boundary (T-10).
//! - The token goes only into the request's opaque `Authorization`, never into the header list,
//!   and no request is ever formatted. Errors are fixed text: no transport error, body or header
//!   is ever formatted.
//! - Bodies are read through a 4 MiB cap.
//! - 429: one retry for a GET whose JSON `retry_after` is at most 10 s; a global 429 blocks every
//!   call until its deadline; POST, 403 and captcha answers are never retried.
//! - 401 latches REST off for the session and sends one notice to the session task, which emits
//!   `SessionEvent::RestUnauthorized`. It never touches the gateway or the Keychain (plan Q3).
use crate::{
    identify::Identity,
    token::Token,
    transport::{Authorization, HttpRequest, HttpResponse, Method, Transport, TransportError},
};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use serde_json::Value;
use std::{
    fmt,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    time::Duration,
};
use tokio::{sync::mpsc, time::Instant};
use url::Url;

/// Paths are appended to this, e.g. `/channels/1/messages?limit=50`.
const BASE: &str = "https://discord.com/api/v9";
const MAX_BODY: usize = 4 << 20;
/// Longest `retry_after` (seconds) still worth one automatic retry.
const MAX_RETRY_AFTER: f64 = 10.0;
/// Whole-request deadline; the transport also applies it, and core drops (cancels) the request
/// when it passes.
const TOTAL_TIMEOUT: Duration = Duration::from_secs(30);

/// Fixed-text failures. None carries a transport error, a body or a header.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum RestError {
    /// 401, or REST already latched off by one this session.
    Unauthorized,
    RateLimited,
    /// 403, e.g. code 50001 (missing access). Does not latch.
    Forbidden,
    Captcha,
    /// Any other non-success status, a redirect included.
    Failed,
    /// Connect, TLS, timeout or transport failure.
    Network,
    TooLarge,
    /// A success body that is not the expected JSON.
    BadResponse,
    /// A URL that fails validation; nothing was sent.
    InvalidUrl,
}

impl fmt::Display for RestError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Unauthorized => "Discord did not accept this sign-in",
            Self::RateLimited => "rate limited",
            Self::Forbidden => "no access",
            Self::Captcha => "Discord asked for a captcha",
            Self::Failed => "request failed",
            Self::Network => "network error",
            Self::TooLarge => "response too large",
            Self::BadResponse => "unreadable response",
            Self::InvalidUrl => "invalid request",
        })
    }
}

impl std::error::Error for RestError {}

/// Cheap handle; every clone shares the session's latch, deadline, token and transport.
#[derive(Clone)]
pub(crate) struct Rest(Arc<RestInner>);

/// The session's one token, shared by `Rest` and the gateway task. `replace` swaps it for both at
/// once (slice ID, after validating a refreshed token, T-8); each request and each IDENTIFY reads
/// the current value when it is built.
pub(crate) struct TokenSlot(Mutex<Arc<Token>>);

impl TokenSlot {
    pub(crate) fn new(token: Arc<Token>) -> Arc<TokenSlot> {
        Arc::new(TokenSlot(Mutex::new(token)))
    }

    pub(crate) fn current(&self) -> Arc<Token> {
        self.0.lock().unwrap_or_else(|e| e.into_inner()).clone()
    }

    pub(crate) fn replace(&self, token: Arc<Token>) {
        *self.0.lock().unwrap_or_else(|e| e.into_inner()) = token;
    }
}

struct RestInner {
    token: Arc<TokenSlot>,
    generation: u64,
    transport: Arc<dyn Transport>,
    identity: Arc<Identity>,
    unauthorized: AtomicBool,
    global_until: Mutex<Option<Instant>>,
    /// To the session task; it emits `RestUnauthorized`. Sent at most once (the latch's swap).
    notice: mpsc::UnboundedSender<()>,
}

impl fmt::Debug for Rest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Rest")
            .field("generation", &self.0.generation)
            .field("unauthorized", &self.0.unauthorized.load(Ordering::SeqCst))
            .finish_non_exhaustive()
    }
}

/// https, host exactly `discord.com`, default port, no userinfo or fragment, path under
/// `/api/v9/` after dot-segment normalization. Anything else is never sent.
pub(crate) fn validated(url: &str) -> Option<Url> {
    let url = Url::parse(url).ok()?;
    (url.scheme() == "https"
        && url.host_str() == Some("discord.com")
        && url.port().is_none()
        && url.username().is_empty()
        && url.password().is_none()
        && url.fragment().is_none()
        && url.path().starts_with("/api/v9/"))
    .then_some(url)
}

/// `BASE` + `path`, validated, and only if parsing kept the typed path as written (S-1): a dot
/// segment or an encoded one (`%2e`) would otherwise reach another endpoint.
fn typed(path: &str) -> Result<String, RestError> {
    let url = format!("{BASE}{path}");
    let written = path.split(['?', '#']).next().unwrap_or_default();
    let parsed = validated(&url).ok_or(RestError::InvalidUrl)?;
    (parsed.path() == format!("/api/v9{written}"))
        .then_some(url)
        .ok_or(RestError::InvalidUrl)
}

#[derive(Deserialize)]
struct RateLimit {
    retry_after: f64,
    #[serde(default)]
    global: bool,
}

impl Rest {
    /// One per session (`Session::connect`); everything else clones it.
    pub(crate) fn new(
        token: Arc<TokenSlot>,
        generation: u64,
        notice: mpsc::UnboundedSender<()>,
        transport: Arc<dyn Transport>,
        identity: Arc<Identity>,
    ) -> Rest {
        Rest(Arc::new(RestInner {
            token,
            generation,
            transport,
            identity,
            unauthorized: AtomicBool::new(false),
            global_until: Mutex::new(None),
            notice,
        }))
    }

    /// A refreshed token for every later request and IDENTIFY of this session (the slot is shared
    /// with the gateway task). Slice ID calls it after validating the new token (T-8); nothing
    /// else does.
    #[allow(dead_code)]
    pub(crate) fn replace_token(&self, token: Arc<Token>) {
        self.0.token.replace(token);
    }

    /// The session's identity (its extras are set through it).
    #[allow(dead_code)] // Slice ID.
    pub(crate) fn identity(&self) -> &Identity {
        &self.0.identity
    }

    /// GET `BASE` + `path`; `path` starts with `/`.
    pub(crate) async fn get<T: DeserializeOwned>(&self, path: &str) -> Result<T, RestError> {
        self.request(Method::Get, &typed(path)?, None).await
    }

    /// POST a JSON body to `BASE` + `path`. Never retried automatically.
    pub(crate) async fn post<T: DeserializeOwned>(
        &self,
        path: &str,
        body: &impl Serialize,
    ) -> Result<T, RestError> {
        let url = typed(path)?;
        let body = serde_json::to_vec(body).map_err(|_| RestError::Failed)?;
        self.request(Method::Post, &url, Some(body)).await
    }

    /// The session's identity headers, the token in its own field, the deadline and the cap.
    pub(crate) fn build_request(
        &self,
        method: Method,
        url: &Url,
        body: Option<Vec<u8>>,
    ) -> Result<HttpRequest, RestError> {
        let identity = &self.0.identity;
        let mut headers: Vec<(String, String)> = vec![
            ("User-Agent".into(), identity.user_agent().into()),
            ("X-Super-Properties".into(), identity.super_properties()),
            ("X-Discord-Locale".into(), identity.locale().into()),
            ("X-Discord-Timezone".into(), identity.timezone().into()),
            ("Accept-Language".into(), identity.accept_language().into()),
        ];
        if let Some(id) = identity.installation_id() {
            headers.push(("X-Installation-ID".into(), id));
        }
        if body.is_some() {
            headers.push(("Content-Type".into(), "application/json".into()));
        }
        HttpRequest::new(
            method,
            url.as_str().to_owned(),
            headers,
            body,
            Some(Authorization::new(self.0.token.current())),
            TOTAL_TIMEOUT,
            MAX_BODY,
        )
        .map_err(|_| RestError::Failed)
    }

    /// One exchange under core's own deadline: when it passes, the request future is dropped,
    /// which cancels it in the transport. The body is re-checked against the cap here.
    async fn send(&self, request: HttpRequest) -> Result<HttpResponse, RestError> {
        let cap = request.body_cap();
        let deadline = request.deadline();
        match tokio::time::timeout(deadline, self.0.transport.http(request)).await {
            Err(_) => Err(RestError::Network),
            Ok(Err(TransportError::Rejected)) => Err(RestError::InvalidUrl),
            Ok(Err(_)) => Err(RestError::Network),
            Ok(Ok(mut response)) => {
                if response.body.as_ref().is_some_and(|b| b.len() > cap) {
                    response.body = None;
                }
                Ok(response)
            }
        }
    }

    fn latch(&self) {
        if !self.0.unauthorized.swap(true, Ordering::SeqCst) {
            // The task may be gone (session ended): then nobody is told, by design.
            let _ = self.0.notice.send(());
        }
    }

    fn blocked(&self) -> bool {
        self.0
            .global_until
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .is_some_and(|until| Instant::now() < until)
    }

    pub(crate) async fn request<T: DeserializeOwned>(
        &self,
        method: Method,
        url: &str,
        body: Option<Vec<u8>>,
    ) -> Result<T, RestError> {
        let url = validated(url).ok_or(RestError::InvalidUrl)?;
        let mut retried = false;
        loop {
            if self.0.unauthorized.load(Ordering::SeqCst) {
                return Err(RestError::Unauthorized);
            }
            if self.blocked() {
                return Err(RestError::RateLimited);
            }
            let request = self.build_request(method, &url, body.clone())?;
            let response = self.send(request).await?;
            let status = response.status;
            if (200..300).contains(&status) {
                let body = response.body.ok_or(RestError::TooLarge)?;
                let body: &[u8] = if body.is_empty() { b"null" } else { &body };
                return serde_json::from_slice(body).map_err(|_| RestError::BadResponse);
            }
            match status {
                401 => {
                    self.latch();
                    return Err(RestError::Unauthorized);
                }
                403 => return Err(RestError::Forbidden),
                429 => {
                    let limit = response
                        .body
                        .and_then(|b| serde_json::from_slice::<RateLimit>(&b).ok())
                        .and_then(|l| Some((Duration::try_from_secs_f64(l.retry_after).ok()?, l)));
                    let Some((wait, limit)) = limit else {
                        return Err(RestError::RateLimited);
                    };
                    if let Some(until) = Instant::now().checked_add(wait).filter(|_| limit.global) {
                        *self
                            .0
                            .global_until
                            .lock()
                            .unwrap_or_else(|e| e.into_inner()) = Some(until);
                    }
                    if retried || method != Method::Get || limit.retry_after > MAX_RETRY_AFTER {
                        return Err(RestError::RateLimited);
                    }
                    retried = true;
                    tokio::time::sleep(wait).await;
                }
                _ => {
                    let captcha = response
                        .body
                        .and_then(|b| serde_json::from_slice::<Value>(&b).ok())
                        .is_some_and(|v| v.get("captcha_key").is_some());
                    return Err(if captcha {
                        RestError::Captcha
                    } else {
                        RestError::Failed
                    });
                }
            }
        }
    }
}

#[cfg(test)]
pub(crate) mod test_support {
    use super::Rest;
    use crate::{
        identify::Identity,
        token::Token,
        transport::{
            memory::{Http, MemoryTransport},
            Transport,
        },
    };
    use std::{cell::RefCell, sync::Arc};
    use tokio::sync::mpsc;

    pub(crate) use crate::transport::memory::HANG;

    thread_local! {
        /// Set by a test on its own (current-thread runtime) thread before `Rest::test` or
        /// `Session::connect`'s test transport is built.
        pub(crate) static TEST_HTTP: RefCell<Option<Arc<Http>>> = const { RefCell::new(None) };
    }

    /// Scripted HTTP answers on the in-memory transport, in order (the last one repeats), with
    /// every request recorded as the HTTP/1.1 text a server would read.
    pub(crate) struct MockHttp(pub(crate) Arc<Http>);

    impl MockHttp {
        pub(crate) async fn start(responses: Vec<String>) -> MockHttp {
            MockHttp(Arc::new(Http::new(responses)))
        }

        /// Answers every `Rest` created afterwards on this thread.
        pub(crate) fn install(&self) {
            TEST_HTTP.with(|h| *h.borrow_mut() = Some(self.0.clone()));
        }

        pub(crate) fn requests(&self) -> Vec<String> {
            self.0.requests.lock().unwrap().clone()
        }

        pub(crate) fn count(&self) -> usize {
            self.0.requests.lock().unwrap().len()
        }

        /// Resolves once a `HANG` request has been dropped (cancelled).
        pub(crate) async fn cancelled(&self) {
            self.0.cancelled.notified().await
        }
    }

    /// The installed mock (none: every request fails to connect), without WebSocket endpoints.
    pub(crate) fn installed_transport() -> Arc<dyn Transport> {
        Arc::new(MemoryTransport {
            http: TEST_HTTP.with(|h| h.borrow().clone()),
            endpoints: None,
        })
    }

    impl Rest {
        /// A session-less `Rest` on the installed mock.
        pub(crate) fn test(
            token: Arc<Token>,
            generation: u64,
            notice: mpsc::UnboundedSender<()>,
        ) -> Rest {
            Rest::new(
                super::TokenSlot::new(token),
                generation,
                notice,
                installed_transport(),
                Arc::new(Identity::current()),
            )
        }
    }

    pub(crate) fn response(status: u16, headers: &str, body: &str) -> String {
        format!(
            "HTTP/1.1 {status} X\r\nContent-Length: {}\r\nConnection: close\r\n{headers}\r\n{body}",
            body.len()
        )
    }
}
