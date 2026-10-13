//! REST client for `https://discord.com/api/v9` (M2 W0.5). One `RestInner` per session holds the
//! token, the generation, the 401 latch, the global-429 deadline and the HTTP client; every
//! `Rest` clone shares it.
//!
//! Security rules, each with a named test in `rest_tests.rs`:
//! - The only destination is `https://discord.com/api/v9/...`: a constant base plus a typed path,
//!   validated before anything is sent. Tests rewrite that validated URL to a local mock
//!   (`#[cfg(test)]` only, like the gateway's dial seam); nothing else can change it.
//! - The stack matches the gateway's: rustls with the ring provider and webpki roots only, no
//!   proxy (environment or system), no redirects, `https_only`, connect and total timeouts.
//! - The token goes only into a per-request `Authorization` header marked sensitive, so the
//!   request's Debug output shows `Sensitive`. Errors are fixed text: no reqwest error, body or
//!   header is ever formatted.
//! - Bodies are read through a 4 MiB cap.
//! - 429: one retry for a GET whose JSON `retry_after` is at most 10 s; a global 429 blocks every
//!   call until its deadline; POST, 403 and captcha answers are never retried.
//! - 401 latches REST off for the session and sends one notice to the session task, which emits
//!   `SessionEvent::RestUnauthorized`. It never touches the gateway or the Keychain (plan Q3).
use crate::{identify, token::Token};
use reqwest::{
    header::{self, HeaderMap, HeaderValue},
    redirect, Client, Method, Request, Response, StatusCode, Url,
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

/// Paths are appended to this, e.g. `/channels/1/messages?limit=50`.
const BASE: &str = "https://discord.com/api/v9";
const MAX_BODY: usize = 4 << 20;
/// Longest `retry_after` (seconds) still worth one automatic retry.
const MAX_RETRY_AFTER: f64 = 10.0;
const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
const TOTAL_TIMEOUT: Duration = Duration::from_secs(30);

/// Fixed-text failures. None carries a reqwest error, a body or a header.
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

/// Cheap handle; every clone shares the session's latch, deadline and client.
#[derive(Clone)]
pub(crate) struct Rest(Arc<RestInner>);

struct RestInner {
    token: Arc<Token>,
    generation: u64,
    /// `None` only if the TLS stack could not be built; every call then fails with `Network`.
    client: Option<Client>,
    unauthorized: AtomicBool,
    global_until: Mutex<Option<Instant>>,
    /// To the session task; it emits `RestUnauthorized`. Sent at most once (the latch's swap).
    notice: mpsc::UnboundedSender<()>,
    #[cfg(test)]
    test_base: Option<String>,
}

impl fmt::Debug for Rest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Rest")
            .field("generation", &self.0.generation)
            .field("unauthorized", &self.0.unauthorized.load(Ordering::SeqCst))
            .finish_non_exhaustive()
    }
}

/// The one builder. Only the `#[cfg(test)]` rewrite to a plain-http mock passes `false`.
fn build_client(https_only: bool) -> Option<Client> {
    let mut headers = HeaderMap::new();
    headers.insert(
        header::USER_AGENT,
        HeaderValue::from_static(identify::USER_AGENT),
    );
    headers.insert(
        "x-super-properties",
        HeaderValue::from_str(&identify::super_properties()).ok()?,
    );
    headers.insert(
        "x-discord-locale",
        HeaderValue::from_static(identify::LOCALE),
    );
    headers.insert(
        "x-discord-timezone",
        HeaderValue::from_str(&identify::timezone()).ok()?,
    );
    headers.insert(
        header::ACCEPT_LANGUAGE,
        HeaderValue::from_static(identify::ACCEPT_LANGUAGE),
    );
    Client::builder()
        .default_headers(headers)
        .no_proxy()
        .redirect(redirect::Policy::none())
        .https_only(https_only)
        .connect_timeout(CONNECT_TIMEOUT)
        .timeout(TOTAL_TIMEOUT)
        .build()
        .ok()
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

async fn execute(client: &Client, request: Request) -> Result<Response, RestError> {
    client
        .execute(request)
        .await
        .map_err(|_| RestError::Network)
}

/// Reads at most `MAX_BODY` bytes; one byte more is `TooLarge`.
async fn read_capped(mut response: Response) -> Result<Vec<u8>, RestError> {
    let mut body = Vec::new();
    while let Some(chunk) = response.chunk().await.map_err(|_| RestError::Network)? {
        if body.len() + chunk.len() > MAX_BODY {
            return Err(RestError::TooLarge);
        }
        body.extend_from_slice(&chunk);
    }
    Ok(body)
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
        token: Arc<Token>,
        generation: u64,
        notice: mpsc::UnboundedSender<()>,
    ) -> Rest {
        // The shipped build has no test base, so `https_only` is true there; tests without a mock
        // (TLS, https-only) run this same expression with `None`.
        #[cfg(test)]
        let test_base = test_support::TEST_REST_BASE.with(|b| b.borrow().clone());
        #[cfg(not(test))]
        let test_base: Option<String> = None;
        Rest(Arc::new(RestInner {
            token,
            generation,
            client: build_client(test_base.is_none()),
            unauthorized: AtomicBool::new(false),
            global_until: Mutex::new(None),
            notice,
            #[cfg(test)]
            test_base,
        }))
    }

    /// GET `BASE` + `path`; `path` starts with `/`.
    #[allow(dead_code)] // Seam for wave-1 M (rest_messages.rs); used by the tests until then.
    pub(crate) async fn get<T: DeserializeOwned>(&self, path: &str) -> Result<T, RestError> {
        self.request(Method::GET, &format!("{BASE}{path}"), None)
            .await
    }

    /// POST a JSON body to `BASE` + `path`. Never retried automatically.
    #[allow(dead_code)] // Seam for wave-1 M (rest_messages.rs); used by the tests until then.
    pub(crate) async fn post<T: DeserializeOwned>(
        &self,
        path: &str,
        body: &impl Serialize,
    ) -> Result<T, RestError> {
        let body = serde_json::to_vec(body).map_err(|_| RestError::Failed)?;
        self.request(Method::POST, &format!("{BASE}{path}"), Some(body))
            .await
    }

    /// Compiled out of shipped builds: tests point the validated URL at a local mock.
    fn target(&self, url: Url) -> Url {
        #[cfg(test)]
        if let Some(base) = &self.0.test_base {
            if let Ok(mock) = Url::parse(&url.as_str().replacen("https://discord.com", base, 1)) {
                return mock;
            }
        }
        url
    }

    pub(crate) fn build_request(
        &self,
        client: &Client,
        method: Method,
        url: Url,
        body: Option<Vec<u8>>,
    ) -> Result<Request, RestError> {
        let mut auth =
            HeaderValue::from_str(self.0.token.expose()).map_err(|_| RestError::Failed)?;
        auth.set_sensitive(true);
        let mut builder = client
            .request(method, url)
            .header(header::AUTHORIZATION, auth);
        if let Some(body) = body {
            builder = builder
                .header(header::CONTENT_TYPE, "application/json")
                .body(body);
        }
        builder.build().map_err(|_| RestError::InvalidUrl)
    }

    /// The shipped client and error mapping without URL validation, for the TLS and https-only
    /// tests, which must aim at a local listener.
    #[cfg(test)]
    pub(crate) async fn send_unvalidated(
        &self,
        method: Method,
        url: &str,
    ) -> Result<Response, RestError> {
        let client = self.0.client.as_ref().ok_or(RestError::Network)?;
        let url = Url::parse(url).map_err(|_| RestError::InvalidUrl)?;
        execute(client, self.build_request(client, method, url, None)?).await
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
        let url = self.target(validated(url).ok_or(RestError::InvalidUrl)?);
        let client = self.0.client.as_ref().ok_or(RestError::Network)?;
        let mut retried = false;
        loop {
            if self.0.unauthorized.load(Ordering::SeqCst) {
                return Err(RestError::Unauthorized);
            }
            if self.blocked() {
                return Err(RestError::RateLimited);
            }
            let request = self.build_request(client, method.clone(), url.clone(), body.clone())?;
            let response = execute(client, request).await?;
            let status = response.status();
            if status.is_success() {
                let body = read_capped(response).await?;
                let body: &[u8] = if body.is_empty() { b"null" } else { &body };
                return serde_json::from_slice(body).map_err(|_| RestError::BadResponse);
            }
            match status {
                StatusCode::UNAUTHORIZED => {
                    self.latch();
                    return Err(RestError::Unauthorized);
                }
                StatusCode::FORBIDDEN => return Err(RestError::Forbidden),
                StatusCode::TOO_MANY_REQUESTS => {
                    let limit = read_capped(response)
                        .await
                        .ok()
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
                    if retried || method != Method::GET || limit.retry_after > MAX_RETRY_AFTER {
                        return Err(RestError::RateLimited);
                    }
                    retried = true;
                    tokio::time::sleep(wait).await;
                }
                _ => {
                    let captcha = read_capped(response)
                        .await
                        .ok()
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
    use std::{
        cell::RefCell,
        sync::{Arc, Mutex},
    };
    use tokio::{
        io::{AsyncReadExt, AsyncWriteExt},
        net::TcpListener,
    };

    thread_local! {
        /// Set by a test on its own (current-thread runtime) thread before `Rest::new`.
        pub(crate) static TEST_REST_BASE: RefCell<Option<String>> = const { RefCell::new(None) };
    }

    /// Minimal HTTP/1.1 server: one request per connection (`Connection: close`), answered with
    /// the next scripted response (the last one repeats). Records every raw request.
    pub(crate) struct MockHttp {
        pub(crate) base: String,
        requests: Arc<Mutex<Vec<String>>>,
    }

    impl MockHttp {
        pub(crate) async fn start(responses: Vec<String>) -> MockHttp {
            let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
            let base = format!("http://{}", listener.local_addr().unwrap());
            let requests = Arc::new(Mutex::new(Vec::new()));
            let seen = requests.clone();
            tokio::spawn(async move {
                let mut i = 0;
                while let Ok((mut stream, _)) = listener.accept().await {
                    let Some(request) = read_request(&mut stream).await else {
                        continue;
                    };
                    seen.lock().unwrap().push(request);
                    let reply = &responses[i.min(responses.len() - 1)];
                    i += 1;
                    let _ = stream.write_all(reply.as_bytes()).await;
                    let _ = stream.shutdown().await;
                }
            });
            MockHttp { base, requests }
        }

        /// Points every `Rest` created afterwards on this thread at this mock.
        pub(crate) fn install(&self) {
            TEST_REST_BASE.with(|b| *b.borrow_mut() = Some(self.base.clone()));
        }

        pub(crate) fn requests(&self) -> Vec<String> {
            self.requests.lock().unwrap().clone()
        }

        pub(crate) fn count(&self) -> usize {
            self.requests.lock().unwrap().len()
        }
    }

    async fn read_request(stream: &mut tokio::net::TcpStream) -> Option<String> {
        let mut buf = Vec::new();
        let mut chunk = [0u8; 4096];
        loop {
            let n = stream.read(&mut chunk).await.ok()?;
            if n == 0 {
                return None;
            }
            buf.extend_from_slice(&chunk[..n]);
            let text = String::from_utf8_lossy(&buf).to_string();
            if let Some(end) = text.find("\r\n\r\n") {
                let length = text[..end]
                    .lines()
                    .find_map(|l| {
                        let (k, v) = l.split_once(':')?;
                        k.eq_ignore_ascii_case("content-length")
                            .then(|| v.trim().parse::<usize>().ok())?
                    })
                    .unwrap_or(0);
                if buf.len() >= end + 4 + length {
                    return Some(text);
                }
            }
        }
    }

    pub(crate) fn response(status: u16, headers: &str, body: &str) -> String {
        format!(
            "HTTP/1.1 {status} X\r\nContent-Length: {}\r\nConnection: close\r\n{headers}\r\n{body}",
            body.len()
        )
    }
}
