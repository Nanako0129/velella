//! The one transport seam (plan NET). All of core's HTTP (rest.rs) and the gateway WebSocket
//! (session.rs) go through `Transport`. The app implements it in Swift on URLSession, behind
//! velella-ffi's foreign trait of the same shape; tests implement it in memory (`memory`).
//!
//! What stays in core, above this trait: URL validation, header composition and the header checks
//! below (T-10), the 401 latch, the 429 blocks, body caps (re-checked here even though the app's
//! transport streams them), deadlines, retries, the gateway URL check, zlib-stream inflation,
//! heartbeats, resume, and an explicit close of every WebSocket (T-4).
//!
//! No logging (T-2, T-16): the token travels only in `Authorization`, never in the header list,
//! and every type here that can carry it or a payload has a hand-written redacting `Debug`.
//! Residual, stated once: copies made across the FFI (RustBuffer, Swift `String`) are not
//! zeroized.
use crate::token::Token;
use std::{fmt, future::Future, pin::Pin, sync::Arc, time::Duration};
use zeroize::Zeroizing;

pub type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Method {
    Get,
    Post,
}

impl Method {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Get => "GET",
            Self::Post => "POST",
        }
    }
}

/// The value of one request's `Authorization` header. Opaque: only the transport reads it, once,
/// when it builds the platform request (T-2).
pub struct Authorization(Arc<Token>);

impl Authorization {
    pub fn new(token: Arc<Token>) -> Authorization {
        Authorization(token)
    }

    pub fn reveal(&self) -> &str {
        self.0.expose()
    }
}

impl fmt::Debug for Authorization {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Authorization([REDACTED])")
    }
}

/// A header list that failed the boundary check; nothing was sent. Fixed text.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct InvalidHeader;

/// RFC 9110 token characters.
fn valid_name(name: &str) -> bool {
    !name.is_empty()
        && name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"!#$%&'*+-.^_`|~".contains(&b))
}

/// Visible ASCII and space: no CR, LF or other control byte can reach the wire.
pub(crate) fn valid_value(value: &str) -> bool {
    value.bytes().all(|b| (0x20..=0x7e).contains(&b))
}

/// Names are tokens, values visible ASCII, and `Authorization` never rides in the list (it has
/// its own field).
fn checked(headers: Vec<(String, String)>) -> Result<Vec<(String, String)>, InvalidHeader> {
    headers
        .iter()
        .all(|(n, v)| valid_name(n) && valid_value(v) && !n.eq_ignore_ascii_case("authorization"))
        .then_some(headers)
        .ok_or(InvalidHeader)
}

pub struct HttpRequest {
    method: Method,
    url: String,
    headers: Vec<(String, String)>,
    body: Option<Vec<u8>>,
    authorization: Option<Authorization>,
    deadline: Duration,
    body_cap: usize,
}

impl HttpRequest {
    /// The transport sends `url` exactly as given (the app's transport refuses it if the platform
    /// would change a byte) and stops reading the body at `body_cap` + 1 bytes.
    pub fn new(
        method: Method,
        url: String,
        headers: Vec<(String, String)>,
        body: Option<Vec<u8>>,
        authorization: Option<Authorization>,
        deadline: Duration,
        body_cap: usize,
    ) -> Result<HttpRequest, InvalidHeader> {
        Ok(HttpRequest {
            method,
            url,
            headers: checked(headers)?,
            body,
            authorization,
            deadline,
            body_cap,
        })
    }

    pub fn method(&self) -> Method {
        self.method
    }
    pub fn url(&self) -> &str {
        &self.url
    }
    pub fn headers(&self) -> &[(String, String)] {
        &self.headers
    }
    pub fn body(&self) -> Option<&[u8]> {
        self.body.as_deref()
    }
    pub fn authorization(&self) -> Option<&Authorization> {
        self.authorization.as_ref()
    }
    pub fn deadline(&self) -> Duration {
        self.deadline
    }
    pub fn body_cap(&self) -> usize {
        self.body_cap
    }
}

/// Method, header names and sizes only: never the URL, a header value, the body or the token.
impl fmt::Debug for HttpRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let names: Vec<&str> = self.headers.iter().map(|(n, _)| n.as_str()).collect();
        f.debug_struct("HttpRequest")
            .field("method", &self.method)
            .field("headers", &names)
            .field("body_len", &self.body.as_ref().map(Vec::len))
            .field("authorization", &self.authorization)
            .field("deadline", &self.deadline)
            .field("body_cap", &self.body_cap)
            .finish_non_exhaustive()
    }
}

/// Any status, a 3xx included (the transport never follows a redirect). `body` is `None` when it
/// went over the request's cap; the transport stopped reading there.
pub struct HttpResponse {
    pub status: u16,
    pub headers: Vec<(String, String)>,
    pub body: Option<Vec<u8>>,
}

impl HttpResponse {
    /// Case-insensitive; `None` when absent or present more than once (ambiguous).
    pub fn header(&self, name: &str) -> Option<&str> {
        let mut found = self
            .headers
            .iter()
            .filter(|(n, _)| n.eq_ignore_ascii_case(name));
        let first = found.next()?;
        found.next().is_none().then_some(first.1.as_str())
    }
}

impl fmt::Debug for HttpResponse {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("HttpResponse")
            .field("status", &self.status)
            .field("headers", &self.headers.len())
            .field("body_len", &self.body.as_ref().map(Vec::len))
            .finish()
    }
}

/// The closed set of transport failures (T-3). Only `ConnectFailed` means no request byte can
/// have been written; nothing else is ever a reason to resend a write.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TransportError {
    ConnectFailed,
    Timeout,
    Cancelled,
    /// The transport refused the request before sending it (URL changed by the platform, host
    /// off its allow-list).
    Rejected,
    /// A WebSocket upgrade answered with this status instead of 101 (a 3xx is not followed).
    UpgradeStatus(u16),
    Other,
}

pub struct WebSocketRequest {
    url: String,
    headers: Vec<(String, String)>,
    max_message_size: usize,
    deadline: Duration,
}

impl WebSocketRequest {
    pub fn new(
        url: String,
        headers: Vec<(String, String)>,
        max_message_size: usize,
        deadline: Duration,
    ) -> Result<WebSocketRequest, InvalidHeader> {
        Ok(WebSocketRequest {
            url,
            headers: checked(headers)?,
            max_message_size,
            deadline,
        })
    }

    pub fn url(&self) -> &str {
        &self.url
    }
    pub fn headers(&self) -> &[(String, String)] {
        &self.headers
    }
    pub fn max_message_size(&self) -> usize {
        self.max_message_size
    }
    pub fn deadline(&self) -> Duration {
        self.deadline
    }
}

impl fmt::Debug for WebSocketRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("WebSocketRequest")
            .field("max_message_size", &self.max_message_size)
            .finish_non_exhaustive()
    }
}

/// One WebSocket message. IDENTIFY and RESUME text carries the token, so text is zeroized on drop
/// and never formatted.
pub enum Frame {
    Text(Zeroizing<String>),
    Binary(Vec<u8>),
}

impl Frame {
    pub fn text(text: String) -> Frame {
        Frame::Text(Zeroizing::new(text))
    }
}

impl fmt::Debug for Frame {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Text(t) => write!(f, "Text({} bytes)", t.len()),
            Self::Binary(b) => write!(f, "Binary({} bytes)", b.len()),
        }
    }
}

/// What one `receive` gives.
#[derive(Debug)]
pub enum Received {
    Frame(Frame),
    /// A close frame arrived with this code.
    Closed(u16),
    /// The connection ended without a close frame (or failed).
    Lost,
}

pub trait Transport: Send + Sync {
    fn http(&self, request: HttpRequest) -> BoxFuture<'_, Result<HttpResponse, TransportError>>;
    fn websocket(
        &self,
        request: WebSocketRequest,
    ) -> BoxFuture<'_, Result<Box<dyn WebSocket>, TransportError>>;
}

/// Dropping an unfinished future cancels that operation.
pub trait WebSocket: Send + Sync {
    fn send(&self, frame: Frame) -> BoxFuture<'_, Result<(), TransportError>>;
    /// One message (or the end) per call.
    fn receive(&self) -> BoxFuture<'_, Received>;
    /// Sends a close frame with `code` and tears the connection down; never waits.
    fn close(&self, code: u16);
}

/// The in-memory transport for core's tests: scripted HTTP answers with recorded requests, and
/// WebSocket endpoints a test accepts connections on.
#[cfg(test)]
pub(crate) mod memory {
    use super::*;
    use std::sync::Mutex;
    use tokio::sync::{mpsc, Mutex as AsyncMutex, Notify};

    /// Scripted reply that never answers; the future waits until dropped (cancelled).
    pub(crate) const HANG: &str = "HANG";

    #[derive(Default)]
    pub(crate) struct Http {
        responses: Vec<String>,
        next: Mutex<usize>,
        /// Each request as an HTTP/1.1 text, the way a server would read it.
        pub(crate) requests: Mutex<Vec<String>>,
        pub(crate) cancelled: Notify,
    }

    impl Http {
        pub(crate) fn new(responses: Vec<String>) -> Http {
            Http {
                responses,
                ..Default::default()
            }
        }

        fn record(&self, request: &HttpRequest) {
            let url = url::Url::parse(request.url()).expect("core sent a parseable URL");
            let target = match url.query() {
                Some(q) => format!("{}?{q}", url.path()),
                None => url.path().to_owned(),
            };
            let mut raw = format!(
                "{} {target} HTTP/1.1\r\nhost: {}\r\n",
                request.method().as_str(),
                url.host_str().unwrap_or("")
            );
            for (name, value) in request.headers() {
                raw += &format!("{name}: {value}\r\n");
            }
            if let Some(auth) = request.authorization() {
                raw += &format!("authorization: {}\r\n", auth.reveal());
            }
            raw += "\r\n";
            raw += &String::from_utf8_lossy(request.body().unwrap_or_default());
            self.requests.lock().unwrap().push(raw);
        }

        async fn answer(&self, request: HttpRequest) -> Result<HttpResponse, TransportError> {
            self.record(&request);
            // A round trip is never instant: let other requests start meanwhile.
            tokio::task::yield_now().await;
            let reply = {
                let mut next = self.next.lock().unwrap();
                let Some(last) = self.responses.len().checked_sub(1) else {
                    return Err(TransportError::ConnectFailed);
                };
                let reply = self.responses[(*next).min(last)].clone();
                *next += 1;
                reply
            };
            if reply == HANG {
                struct OnDrop<'a>(&'a Notify);
                impl Drop for OnDrop<'_> {
                    fn drop(&mut self) {
                        self.0.notify_one();
                    }
                }
                let _guard = OnDrop(&self.cancelled);
                std::future::pending::<()>().await;
            }
            if let Some(error) = reply.strip_prefix("ERROR ") {
                return Err(match error {
                    "connect" => TransportError::ConnectFailed,
                    "timeout" => TransportError::Timeout,
                    _ => TransportError::Other,
                });
            }
            let (head, body) = reply
                .split_once("\r\n\r\n")
                .expect("scripted response head");
            let mut lines = head.lines();
            let status = lines
                .next()
                .and_then(|l| l.split(' ').nth(1))
                .and_then(|s| s.parse().ok())
                .expect("scripted status line");
            let headers = lines
                .filter_map(|l| {
                    let (k, v) = l.split_once(':')?;
                    Some((k.trim().to_owned(), v.trim().to_owned()))
                })
                .collect();
            // Like the app's transport: the body stops at cap + 1 bytes and is dropped.
            let body = (body.len() <= request.body_cap()).then(|| body.as_bytes().to_vec());
            Ok(HttpResponse {
                status,
                headers,
                body,
            })
        }
    }

    /// What the server end of an in-memory WebSocket sees from the client.
    #[derive(Debug)]
    pub(crate) enum FromClient {
        Frame(Frame),
        Close(u16),
    }

    /// The server end of one accepted connection. Dropping it is an abrupt drop: the client's
    /// next `receive` gives `Lost`.
    pub(crate) struct ServerEnd {
        /// The upgrade request as core built it.
        pub(crate) request: WebSocketRequest,
        pub(crate) to_client: Option<mpsc::UnboundedSender<Received>>,
        pub(crate) from_client: mpsc::UnboundedReceiver<FromClient>,
    }

    struct Socket {
        incoming: AsyncMutex<mpsc::UnboundedReceiver<Received>>,
        outgoing: mpsc::UnboundedSender<FromClient>,
    }

    impl WebSocket for Socket {
        fn send(&self, frame: Frame) -> BoxFuture<'_, Result<(), TransportError>> {
            let sent = self.outgoing.send(FromClient::Frame(frame));
            Box::pin(async move { sent.map_err(|_| TransportError::Other) })
        }

        fn receive(&self) -> BoxFuture<'_, Received> {
            Box::pin(async move {
                self.incoming
                    .lock()
                    .await
                    .recv()
                    .await
                    .unwrap_or(Received::Lost)
            })
        }

        fn close(&self, code: u16) {
            let _ = self.outgoing.send(FromClient::Close(code));
        }
    }

    /// (initial gateway URL, everything else): the two places a session dials.
    pub(crate) struct Endpoints {
        pub(crate) initial: mpsc::UnboundedSender<ServerEnd>,
        pub(crate) resume: mpsc::UnboundedSender<ServerEnd>,
    }

    pub(crate) struct MemoryTransport {
        pub(crate) http: Option<Arc<Http>>,
        pub(crate) endpoints: Option<Endpoints>,
    }

    impl Transport for MemoryTransport {
        fn http(
            &self,
            request: HttpRequest,
        ) -> BoxFuture<'_, Result<HttpResponse, TransportError>> {
            Box::pin(async move {
                match &self.http {
                    Some(http) => http.answer(request).await,
                    None => Err(TransportError::ConnectFailed),
                }
            })
        }

        fn websocket(
            &self,
            request: WebSocketRequest,
        ) -> BoxFuture<'_, Result<Box<dyn WebSocket>, TransportError>> {
            Box::pin(async move {
                let endpoints = self
                    .endpoints
                    .as_ref()
                    .ok_or(TransportError::ConnectFailed)?;
                let queue = if request.url() == crate::identify::GATEWAY_URL {
                    &endpoints.initial
                } else {
                    &endpoints.resume
                };
                let (to_client, incoming) = mpsc::unbounded_channel();
                let (outgoing, from_client) = mpsc::unbounded_channel();
                queue
                    .send(ServerEnd {
                        request,
                        to_client: Some(to_client),
                        from_client,
                    })
                    .map_err(|_| TransportError::ConnectFailed)?;
                Ok(Box::new(Socket {
                    incoming: AsyncMutex::new(incoming),
                    outgoing,
                }) as Box<dyn WebSocket>)
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MARKER: &str = "SYNTHETIC_TOKEN_MARKER_0123456789";

    fn auth() -> Authorization {
        Authorization::new(Arc::new(Token::new(MARKER.into()).unwrap()))
    }

    #[test]
    fn header_names_and_values_are_checked_at_the_boundary() {
        let ok = |n: &str, v: &str| {
            HttpRequest::new(
                Method::Get,
                "https://discord.com/api/v9/x".into(),
                vec![(n.into(), v.into())],
                None,
                None,
                Duration::from_secs(1),
                1,
            )
            .is_ok()
        };
        assert!(ok("User-Agent", "Mozilla/5.0 (Macintosh) Safari/605.1.15"));
        assert!(ok("X-Super-Properties", "eyJvcyI6Ik1hYyJ9"));
        for (n, v) in [
            ("X-Evil", "a\r\nAuthorization: x"),
            ("X-Evil", "a\nb"),
            ("X-Evil", "caf\u{e9}"),
            ("X-Evil", "tab\there"),
            ("Bad Name", "v"),
            ("Bad:Name", "v"),
            ("", "v"),
            ("Authorization", MARKER),
            ("authorization", MARKER),
        ] {
            assert!(!ok(n, v), "{n:?}: {v:?}");
        }
        let ws = WebSocketRequest::new(
            "wss://gateway.discord.gg/".into(),
            vec![("X".into(), "a\r\nb".into())],
            1,
            Duration::from_secs(1),
        );
        assert!(ws.is_err());
    }

    #[test]
    fn response_header_lookup_is_case_insensitive_and_refuses_duplicates() {
        let response = HttpResponse {
            status: 429,
            headers: vec![
                ("Retry-After".into(), "5".into()),
                ("set-cookie".into(), "a=1".into()),
                ("Set-Cookie".into(), "b=2".into()),
            ],
            body: None,
        };
        assert_eq!(response.header("retry-after"), Some("5"));
        assert_eq!(response.header("RETRY-AFTER"), Some("5"));
        assert_eq!(response.header("set-cookie"), None);
        assert_eq!(response.header("x-missing"), None);
    }

    /// Plan NET No logging: a marker token is absent from every `{:?}` of what carries it.
    #[test]
    fn debug_of_requests_frames_and_responses_never_shows_the_token() {
        let request = HttpRequest::new(
            Method::Post,
            format!("https://discord.com/api/v9/x?{MARKER}"),
            vec![("X-Context".into(), MARKER.into())],
            Some(MARKER.as_bytes().to_vec()),
            Some(auth()),
            Duration::from_secs(1),
            1,
        )
        .unwrap();
        // The guard is not vacuous: the request does carry the token.
        assert_eq!(request.authorization().unwrap().reveal(), MARKER);
        let frame = Frame::Text(Zeroizing::new(format!(
            r#"{{"op":2,"d":{{"token":"{MARKER}"}}}}"#
        )));
        let response = HttpResponse {
            status: 200,
            headers: vec![("X".into(), MARKER.into())],
            body: Some(MARKER.as_bytes().to_vec()),
        };
        let ws = WebSocketRequest::new(
            format!("wss://gateway.discord.gg/?{MARKER}"),
            vec![],
            1,
            Duration::from_secs(1),
        )
        .unwrap();
        for text in [
            format!("{request:?}"),
            format!("{request:#?}"),
            format!("{frame:?}"),
            format!("{:?}", Received::Frame(Frame::Binary(MARKER.into()))),
            format!("{response:?}"),
            format!("{ws:?}"),
            format!("{:?}", auth()),
        ] {
            assert!(!text.contains("SYNTHETIC"), "{text}");
        }
    }
}
