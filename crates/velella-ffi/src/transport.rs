//! Core's transport seam (velella_core::transport, plan NET) as a UniFFI foreign trait. The app
//! implements `Transport` in Swift on URLSession (Sources/Velella/Transport); `ForeignTransport`
//! adapts it back to core's trait.
//!
//! Every method of the foreign traits is synchronous and returns at once: Swift starts the work
//! and reports the outcome to a completion object exactly once (later calls are ignored). This
//! keeps the FFI free of foreign async methods. Dropping a core future cancels the Swift work
//! (`TransportTask::cancel`), so core's deadlines and aborts tear the connection down (T-4).
//!
//! No logging (T-2, T-16): requests, WebSocket requests and outgoing frames are objects, so Swift
//! sees only methods; their `Debug` and `Display` (Swift's `description` and `debugDescription`)
//! are redacted. The Authorization value is handed out once, to the code that builds the
//! platform request.
use std::fmt;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex,
};
use std::time::Duration;
use tokio::sync::{oneshot, Mutex as AsyncMutex};
use velella_core::transport as core;
use velella_core::Token;

/// Starts HTTP exchanges and WebSocket connections. Implemented in Swift.
#[uniffi::export(with_foreign)]
pub trait Transport: Send + Sync {
    /// Starts `request`; the outcome goes to `completion`. A 3xx is an outcome, never followed.
    fn http(
        &self,
        request: Arc<HttpRequest>,
        completion: Arc<HttpCompletion>,
    ) -> Arc<dyn TransportTask>;
    /// Starts the upgrade; the connection or the failure goes to `completion`.
    fn websocket(
        &self,
        request: Arc<WebSocketRequest>,
        completion: Arc<WebSocketCompletion>,
    ) -> Arc<dyn TransportTask>;
    /// Opens an attached file once for its upload (composer plan CU-6): without following a
    /// link, non-blocking, then fstat; only a regular file whose (device, inode, size, mtime)
    /// equal `file`'s is kept, under the returned id, until `close_file`. nil: gone or changed.
    fn open_file(&self, file: Arc<UploadFile>) -> Option<u64>;
    fn close_file(&self, id: u64);
    /// Starts one upload exchange (preflight or PUT) to the upload-only allow-list, the body
    /// streamed from the opened file; the outcome goes to `completion`, the bytes sent of the
    /// body to `progress`. A 3xx is an outcome, never followed.
    fn upload(
        &self,
        request: Arc<UploadRequest>,
        progress: Arc<UploadProgress>,
        completion: Arc<HttpCompletion>,
    ) -> Arc<dyn TransportTask>;
}

/// One started operation.
#[uniffi::export(with_foreign)]
pub trait TransportTask: Send + Sync {
    /// Core no longer wants the outcome (deadline passed, caller gone): tear it down.
    fn cancel(&self);
}

/// One open WebSocket. Implemented in Swift.
#[uniffi::export(with_foreign)]
pub trait WebSocketConnection: Send + Sync {
    fn send(&self, frame: Arc<WebSocketFrame>, completion: Arc<SendCompletion>);
    /// Reads exactly one message (or the end) into `completion`. Core never has two outstanding.
    fn receive(&self, completion: Arc<ReceiveCompletion>);
    /// Sends a close frame with `code` and tears the connection down.
    fn close(&self, code: u16);
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, uniffi::Enum)]
pub enum TransportErrorKind {
    /// No request byte can have been written (DNS, refused, TLS handshake, offline).
    ConnectFailed,
    Timeout,
    Cancelled,
    /// Refused before sending (URL changed by the platform, host off the allow-list).
    Rejected,
    Other,
}

impl From<TransportErrorKind> for core::TransportError {
    fn from(kind: TransportErrorKind) -> Self {
        match kind {
            TransportErrorKind::ConnectFailed => Self::ConnectFailed,
            TransportErrorKind::Timeout => Self::Timeout,
            TransportErrorKind::Cancelled => Self::Cancelled,
            TransportErrorKind::Rejected => Self::Rejected,
            TransportErrorKind::Other => Self::Other,
        }
    }
}

#[derive(Clone, uniffi::Record)]
pub struct HttpHeader {
    pub name: String,
    pub value: String,
}

#[derive(Clone, Copy, uniffi::Enum)]
pub enum HttpMethod {
    Get,
    Post,
    Put,
    Patch,
    Delete,
}

/// Fixed text; never carries the rejected input.
#[derive(Debug, uniffi::Error)]
pub enum TransportRequestError {
    InvalidHeader,
    InvalidToken,
}

impl fmt::Display for TransportRequestError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::InvalidHeader => "invalid header",
            Self::InvalidToken => "invalid token",
        })
    }
}

impl std::error::Error for TransportRequestError {}

fn pairs(headers: Vec<HttpHeader>) -> Vec<(String, String)> {
    headers.into_iter().map(|h| (h.name, h.value)).collect()
}

fn records(headers: &[(String, String)]) -> Vec<HttpHeader> {
    headers
        .iter()
        .map(|(name, value)| HttpHeader {
            name: name.clone(),
            value: value.clone(),
        })
        .collect()
}

fn millis(d: Duration) -> u64 {
    u64::try_from(d.as_millis()).unwrap_or(u64::MAX)
}

/// One HTTP request as core built it.
#[derive(uniffi::Object)]
#[uniffi::export(Debug, Display)]
pub struct HttpRequest {
    inner: core::HttpRequest,
    authorization_taken: AtomicBool,
}

impl fmt::Debug for HttpRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.inner.fmt(f)
    }
}

impl fmt::Display for HttpRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "HttpRequest({} [REDACTED])",
            self.inner.method().as_str()
        )
    }
}

#[uniffi::export]
impl HttpRequest {
    /// For the transport's own tests: a request with the same boundary checks core applies (header
    /// names and values, the token's rules). The shipped transport still refuses any URL off its
    /// allow-list.
    #[uniffi::constructor]
    pub fn new(
        method: HttpMethod,
        url: String,
        headers: Vec<HttpHeader>,
        body: Option<Vec<u8>>,
        authorization: Option<String>,
        deadline_ms: u64,
        body_cap: u64,
    ) -> Result<Arc<Self>, TransportRequestError> {
        let authorization = authorization
            .map(|t| Token::new(t).map(|t| core::Authorization::new(Arc::new(t))))
            .transpose()
            .map_err(|_| TransportRequestError::InvalidToken)?;
        let method = match method {
            HttpMethod::Get => core::Method::Get,
            HttpMethod::Post => core::Method::Post,
            HttpMethod::Put => core::Method::Put,
            HttpMethod::Patch => core::Method::Patch,
            HttpMethod::Delete => core::Method::Delete,
        };
        let inner = core::HttpRequest::new(
            method,
            url,
            pairs(headers),
            body,
            authorization,
            Duration::from_millis(deadline_ms),
            usize::try_from(body_cap).unwrap_or(usize::MAX),
        )
        .map_err(|_| TransportRequestError::InvalidHeader)?;
        Ok(Arc::new(Self::from(inner)))
    }

    pub fn method(&self) -> String {
        self.inner.method().as_str().to_owned()
    }

    /// Exactly the string to send; the transport refuses the request if the platform would
    /// change a byte of it.
    pub fn url(&self) -> String {
        self.inner.url().to_owned()
    }

    /// Never contains Authorization (see `take_authorization`).
    pub fn headers(&self) -> Vec<HttpHeader> {
        records(self.inner.headers())
    }

    pub fn body(&self) -> Option<Vec<u8>> {
        self.inner.body().map(<[u8]>::to_vec)
    }

    pub fn deadline_ms(&self) -> u64 {
        millis(self.inner.deadline())
    }

    pub fn body_cap(&self) -> u64 {
        u64::try_from(self.inner.body_cap()).unwrap_or(u64::MAX)
    }

    /// The Authorization header value, the first time only (T-2); `None` afterwards or when the
    /// request has none.
    pub fn take_authorization(&self) -> Option<String> {
        if self.authorization_taken.swap(true, Ordering::SeqCst) {
            return None;
        }
        self.inner.authorization().map(|a| a.reveal().to_owned())
    }
}

impl From<core::HttpRequest> for HttpRequest {
    fn from(inner: core::HttpRequest) -> Self {
        Self {
            inner,
            authorization_taken: AtomicBool::new(false),
        }
    }
}

/// An attached file's identity (composer plan CU-6): the resolved path and (device, inode, size,
/// mtime) recorded when it was added. An object, so nothing Swift prints shows the path (CU-7).
#[derive(uniffi::Object)]
#[uniffi::export(Debug, Display)]
pub struct UploadFile(pub(crate) core::FileIdentity);

impl fmt::Debug for UploadFile {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl fmt::Display for UploadFile {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("UploadFile([REDACTED])")
    }
}

#[uniffi::export]
impl UploadFile {
    /// What the app's attach-time check recorded (`mtime_ns`: nanoseconds since the epoch).
    #[uniffi::constructor]
    pub fn new(path: String, device: u64, inode: u64, size: u64, mtime_ns: i64) -> Arc<Self> {
        Arc::new(Self(core::FileIdentity {
            path,
            device,
            inode,
            size,
            mtime_ns,
        }))
    }

    pub fn path(&self) -> String {
        self.0.path.clone()
    }
    pub fn device(&self) -> u64 {
        self.0.device
    }
    pub fn inode(&self) -> u64 {
        self.0.inode
    }
    pub fn size(&self) -> u64 {
        self.0.size
    }
    pub fn mtime_ns(&self) -> i64 {
        self.0.mtime_ns
    }
}

#[derive(Clone, Copy, uniffi::Enum)]
pub enum UploadMethod {
    Options,
    Put,
}

/// One upload exchange as core built it: no Authorization exists on it, and its header list is
/// core's closed list (the transport checks the names again).
#[derive(uniffi::Object)]
#[uniffi::export(Debug, Display)]
pub struct UploadRequest(core::UploadRequest);

impl fmt::Debug for UploadRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl fmt::Display for UploadRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "UploadRequest({} [REDACTED])", self.0.method().as_str())
    }
}

#[uniffi::export]
impl UploadRequest {
    /// For the transport's own tests, with core's header checks (no Authorization, no CR/LF).
    #[uniffi::constructor]
    #[allow(clippy::too_many_arguments)] // One per field of core's request; a test seam only.
    pub fn new(
        method: UploadMethod,
        url: String,
        headers: Vec<HttpHeader>,
        file: Option<u64>,
        start: u64,
        length: u64,
        deadline_ms: u64,
        body_cap: u64,
    ) -> Result<Arc<Self>, TransportRequestError> {
        let method = match method {
            UploadMethod::Options => core::UploadMethod::Options,
            UploadMethod::Put => core::UploadMethod::Put,
        };
        core::UploadRequest::new(
            method,
            url,
            pairs(headers),
            file.map(|file| core::UploadBody {
                file,
                start,
                length,
            }),
            Duration::from_millis(deadline_ms),
            usize::try_from(body_cap).unwrap_or(usize::MAX),
        )
        .map(|r| Arc::new(Self(r)))
        .map_err(|_| TransportRequestError::InvalidHeader)
    }

    /// `OPTIONS` or `PUT`.
    pub fn method(&self) -> String {
        self.0.method().as_str().to_owned()
    }

    /// Exactly the string to send.
    pub fn url(&self) -> String {
        self.0.url().to_owned()
    }

    pub fn headers(&self) -> Vec<HttpHeader> {
        records(self.0.headers())
    }

    /// The opened file the body streams from; nil for an empty body.
    pub fn file(&self) -> Option<u64> {
        self.0.body().map(|b| b.file)
    }

    pub fn body_start(&self) -> u64 {
        self.0.body().map_or(0, |b| b.start)
    }

    pub fn body_length(&self) -> u64 {
        self.0.body().map_or(0, |b| b.length)
    }

    pub fn deadline_ms(&self) -> u64 {
        millis(self.0.deadline())
    }

    pub fn body_cap(&self) -> u64 {
        u64::try_from(self.0.body_cap()).unwrap_or(u64::MAX)
    }
}

/// Where the transport reports the bytes of the current body sent so far.
#[derive(uniffi::Object)]
pub struct UploadProgress {
    report: Option<core::UploadProgress>,
    /// What a test-made progress received.
    seen: Mutex<Vec<u64>>,
}

#[uniffi::export]
impl UploadProgress {
    /// For the transport's own tests: records every report.
    #[uniffi::constructor]
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            report: None,
            seen: Mutex::new(Vec::new()),
        })
    }

    pub fn sent(&self, bytes: u64) {
        match &self.report {
            Some(report) => report(bytes),
            None => self
                .seen
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .push(bytes),
        }
    }

    /// What a test-made progress received, in order.
    pub fn reports(&self) -> Vec<u64> {
        self.seen.lock().unwrap_or_else(|e| e.into_inner()).clone()
    }
}

/// One WebSocket upgrade as core built it.
#[derive(uniffi::Object)]
#[uniffi::export(Debug, Display)]
pub struct WebSocketRequest(core::WebSocketRequest);

impl fmt::Debug for WebSocketRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl fmt::Display for WebSocketRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("WebSocketRequest([REDACTED])")
    }
}

#[uniffi::export]
impl WebSocketRequest {
    /// For the transport's own tests, with core's header checks.
    #[uniffi::constructor]
    pub fn new(
        url: String,
        headers: Vec<HttpHeader>,
        max_message_size: u64,
        deadline_ms: u64,
    ) -> Result<Arc<Self>, TransportRequestError> {
        core::WebSocketRequest::new(
            url,
            pairs(headers),
            usize::try_from(max_message_size).unwrap_or(usize::MAX),
            Duration::from_millis(deadline_ms),
        )
        .map(|r| Arc::new(Self(r)))
        .map_err(|_| TransportRequestError::InvalidHeader)
    }

    pub fn url(&self) -> String {
        self.0.url().to_owned()
    }

    pub fn headers(&self) -> Vec<HttpHeader> {
        records(self.0.headers())
    }

    /// Core's inflation cap; the transport's message-size limit.
    pub fn max_message_size(&self) -> u64 {
        u64::try_from(self.0.max_message_size()).unwrap_or(u64::MAX)
    }

    pub fn deadline_ms(&self) -> u64 {
        millis(self.0.deadline())
    }
}

/// One outgoing message. IDENTIFY and RESUME text carries the token.
#[derive(uniffi::Object)]
#[uniffi::export(Debug, Display)]
pub struct WebSocketFrame(core::Frame);

impl fmt::Debug for WebSocketFrame {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl fmt::Display for WebSocketFrame {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("WebSocketFrame([REDACTED])")
    }
}

#[uniffi::export]
impl WebSocketFrame {
    /// For the transport's own tests.
    #[uniffi::constructor]
    pub fn new_text(text: String) -> Arc<Self> {
        Arc::new(Self(core::Frame::text(text)))
    }

    /// For the transport's own tests.
    #[uniffi::constructor]
    pub fn new_binary(data: Vec<u8>) -> Arc<Self> {
        Arc::new(Self(core::Frame::Binary(data)))
    }

    /// The text, for a text frame.
    pub fn text(&self) -> Option<String> {
        match &self.0 {
            core::Frame::Text(t) => Some(t.to_string()),
            core::Frame::Binary(_) => None,
        }
    }

    /// The bytes, for a binary frame.
    pub fn binary(&self) -> Option<Vec<u8>> {
        match &self.0 {
            core::Frame::Binary(b) => Some(b.clone()),
            core::Frame::Text(_) => None,
        }
    }
}

/// A one-shot slot: the first outcome wins, later ones are dropped.
struct Slot<T>(Mutex<Option<oneshot::Sender<T>>>);

impl<T> Slot<T> {
    fn new() -> (Self, oneshot::Receiver<T>) {
        let (tx, rx) = oneshot::channel();
        (Self(Mutex::new(Some(tx))), rx)
    }

    /// `Err(value)` when nobody waits any more (or it already completed).
    fn complete(&self, value: T) -> Result<(), T> {
        match self.0.lock().unwrap_or_else(|e| e.into_inner()).take() {
            Some(tx) => tx.send(value),
            None => Err(value),
        }
    }
}

#[derive(uniffi::Object)]
pub struct HttpCompletion(Slot<Result<core::HttpResponse, core::TransportError>>);

#[uniffi::export]
impl HttpCompletion {
    /// Any status, a 3xx included. `body_over_cap`: the body passed the request's cap and was
    /// not read further (`body` is then ignored).
    pub fn succeed(
        &self,
        status: u16,
        headers: Vec<HttpHeader>,
        body: Vec<u8>,
        body_over_cap: bool,
    ) {
        let _ = self.0.complete(Ok(core::HttpResponse {
            status,
            headers: pairs(headers),
            body: (!body_over_cap).then_some(body),
        }));
    }

    pub fn fail(&self, kind: TransportErrorKind) {
        let _ = self.0.complete(Err(kind.into()));
    }
}

#[derive(uniffi::Object)]
pub struct WebSocketCompletion(Slot<Result<Box<dyn core::WebSocket>, core::TransportError>>);

#[uniffi::export]
impl WebSocketCompletion {
    pub fn opened(&self, connection: Arc<dyn WebSocketConnection>) {
        let socket = Box::new(ForeignSocket {
            conn: connection.clone(),
            pending: AsyncMutex::new(None),
        });
        if self.0.complete(Ok(socket)).is_err() {
            // The dial was abandoned (stop or deadline): nobody will ever close this one.
            connection.close(1000);
        }
    }

    pub fn failed(&self, kind: TransportErrorKind) {
        let _ = self.0.complete(Err(kind.into()));
    }

    /// The upgrade was answered with `status` instead of 101; a 3xx is not followed.
    pub fn rejected(&self, status: u16) {
        let _ = self
            .0
            .complete(Err(core::TransportError::UpgradeStatus(status)));
    }
}

#[derive(uniffi::Object)]
pub struct SendCompletion(Slot<Result<(), core::TransportError>>);

#[uniffi::export]
impl SendCompletion {
    pub fn sent(&self) {
        let _ = self.0.complete(Ok(()));
    }

    pub fn failed(&self, kind: TransportErrorKind) {
        let _ = self.0.complete(Err(kind.into()));
    }
}

#[derive(uniffi::Object)]
pub struct ReceiveCompletion(Slot<core::Received>);

#[uniffi::export]
impl ReceiveCompletion {
    pub fn text(&self, text: String) {
        let _ = self
            .0
            .complete(core::Received::Frame(core::Frame::text(text)));
    }

    pub fn binary(&self, data: Vec<u8>) {
        let _ = self
            .0
            .complete(core::Received::Frame(core::Frame::Binary(data)));
    }

    /// A close frame with this code arrived.
    pub fn closed(&self, code: u16) {
        let _ = self.0.complete(core::Received::Closed(code));
    }

    /// The connection ended without a close frame (or failed).
    pub fn lost(&self) {
        let _ = self.0.complete(core::Received::Lost);
    }
}

/// Cancels the Swift task when the core future is dropped before its outcome arrived.
struct CancelOnDrop(Option<Arc<dyn TransportTask>>);

impl CancelOnDrop {
    fn done(mut self) {
        self.0 = None;
    }
}

impl Drop for CancelOnDrop {
    fn drop(&mut self) {
        if let Some(task) = self.0.take() {
            task.cancel();
        }
    }
}

/// Core's `Transport` on the app's foreign one.
pub(crate) struct ForeignTransport(pub(crate) Arc<dyn Transport>);

impl core::Transport for ForeignTransport {
    fn http(
        &self,
        request: core::HttpRequest,
    ) -> core::BoxFuture<'_, Result<core::HttpResponse, core::TransportError>> {
        let (slot, rx) = Slot::new();
        let task = self
            .0
            .http(Arc::new(request.into()), Arc::new(HttpCompletion(slot)));
        let guard = CancelOnDrop(Some(task));
        Box::pin(async move {
            // A completion dropped unanswered (a Swift bug) is a transport failure.
            let outcome = rx.await.unwrap_or(Err(core::TransportError::Other));
            guard.done();
            outcome
        })
    }

    fn websocket(
        &self,
        request: core::WebSocketRequest,
    ) -> core::BoxFuture<'_, Result<Box<dyn core::WebSocket>, core::TransportError>> {
        let (slot, rx) = Slot::new();
        let task = self.0.websocket(
            Arc::new(WebSocketRequest(request)),
            Arc::new(WebSocketCompletion(slot)),
        );
        let guard = CancelOnDrop(Some(task));
        Box::pin(async move {
            let outcome = rx.await.unwrap_or(Err(core::TransportError::Other));
            guard.done();
            outcome
        })
    }

    fn open_file(&self, file: &core::FileIdentity) -> Option<u64> {
        self.0.open_file(Arc::new(UploadFile(file.clone())))
    }

    fn close_file(&self, id: u64) {
        self.0.close_file(id);
    }

    fn upload(
        &self,
        request: core::UploadRequest,
        progress: core::UploadProgress,
    ) -> core::BoxFuture<'_, Result<core::HttpResponse, core::TransportError>> {
        let (slot, rx) = Slot::new();
        let progress = Arc::new(UploadProgress {
            report: Some(progress),
            seen: Mutex::new(Vec::new()),
        });
        let task = self.0.upload(
            Arc::new(UploadRequest(request)),
            progress,
            Arc::new(HttpCompletion(slot)),
        );
        let guard = CancelOnDrop(Some(task));
        Box::pin(async move {
            let outcome = rx.await.unwrap_or(Err(core::TransportError::Other));
            guard.done();
            outcome
        })
    }
}

struct ForeignSocket {
    conn: Arc<dyn WebSocketConnection>,
    /// A `receive` asked of Swift whose core future was dropped; the next call awaits it, so no
    /// message is ever lost to a cancelled read (core's select loop drops reads all the time).
    pending: AsyncMutex<Option<oneshot::Receiver<core::Received>>>,
}

impl core::WebSocket for ForeignSocket {
    fn send(&self, frame: core::Frame) -> core::BoxFuture<'_, Result<(), core::TransportError>> {
        let (slot, rx) = Slot::new();
        self.conn.send(
            Arc::new(WebSocketFrame(frame)),
            Arc::new(SendCompletion(slot)),
        );
        Box::pin(async move { rx.await.unwrap_or(Err(core::TransportError::Other)) })
    }

    fn receive(&self) -> core::BoxFuture<'_, core::Received> {
        Box::pin(async move {
            // Held across the await: if this future is dropped, the receiver stays in the slot.
            let mut pending = self.pending.lock().await;
            let rx = pending.get_or_insert_with(|| {
                let (slot, rx) = Slot::new();
                self.conn.receive(Arc::new(ReceiveCompletion(slot)));
                rx
            });
            let received = rx.await.unwrap_or(core::Received::Lost);
            *pending = None;
            received
        })
    }

    fn close(&self, code: u16) {
        self.conn.close(code);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicUsize;
    use velella_core::transport::Transport as _;

    const MARKER: &str = "SYNTHETIC_TOKEN_MARKER_0123456789";

    /// A Rust stand-in for the Swift transport: keeps every completion for the test to answer
    /// and counts cancels, receives and closes.
    #[derive(Default)]
    struct Fake {
        http: Mutex<Vec<Arc<HttpCompletion>>>,
        ws: Mutex<Vec<Arc<WebSocketCompletion>>>,
        uploads: Mutex<Vec<Arc<HttpCompletion>>>,
        cancels: Arc<AtomicUsize>,
    }

    struct Task(Arc<AtomicUsize>);
    impl TransportTask for Task {
        fn cancel(&self) {
            self.0.fetch_add(1, Ordering::SeqCst);
        }
    }

    impl Transport for Fake {
        fn http(&self, _: Arc<HttpRequest>, c: Arc<HttpCompletion>) -> Arc<dyn TransportTask> {
            self.http.lock().unwrap().push(c);
            Arc::new(Task(self.cancels.clone()))
        }
        fn websocket(
            &self,
            _: Arc<WebSocketRequest>,
            c: Arc<WebSocketCompletion>,
        ) -> Arc<dyn TransportTask> {
            self.ws.lock().unwrap().push(c);
            Arc::new(Task(self.cancels.clone()))
        }
        fn open_file(&self, file: Arc<UploadFile>) -> Option<u64> {
            (file.size() == 3).then_some(7)
        }
        fn close_file(&self, _: u64) {}
        fn upload(
            &self,
            request: Arc<UploadRequest>,
            progress: Arc<UploadProgress>,
            c: Arc<HttpCompletion>,
        ) -> Arc<dyn TransportTask> {
            progress.sent(request.body_length());
            self.uploads.lock().unwrap().push(c);
            Arc::new(Task(self.cancels.clone()))
        }
    }

    #[derive(Default)]
    struct Conn {
        receives: Mutex<Vec<Arc<ReceiveCompletion>>>,
        closes: Mutex<Vec<u16>>,
    }
    impl WebSocketConnection for Conn {
        fn send(&self, _: Arc<WebSocketFrame>, c: Arc<SendCompletion>) {
            c.sent();
        }
        fn receive(&self, c: Arc<ReceiveCompletion>) {
            self.receives.lock().unwrap().push(c);
        }
        fn close(&self, code: u16) {
            self.closes.lock().unwrap().push(code);
        }
    }

    fn request() -> core::HttpRequest {
        core::HttpRequest::new(
            core::Method::Get,
            "https://discord.com/api/v9/users/@me".into(),
            vec![],
            None,
            None,
            Duration::from_secs(1),
            8,
        )
        .unwrap()
    }

    fn rt() -> tokio::runtime::Runtime {
        tokio::runtime::Builder::new_current_thread()
            .enable_time()
            .build()
            .unwrap()
    }

    /// T-4: core dropping the future (its deadline, an abort) cancels the Swift task; a finished
    /// one is not cancelled.
    #[test]
    fn dropping_an_unfinished_request_cancels_the_foreign_task() {
        let fake = Arc::new(Fake::default());
        let transport = ForeignTransport(fake.clone());
        rt().block_on(async {
            let timed_out =
                tokio::time::timeout(Duration::from_millis(20), transport.http(request())).await;
            assert!(timed_out.is_err());
            assert_eq!(fake.cancels.load(Ordering::SeqCst), 1);

            let pending = transport.http(request());
            fake.http.lock().unwrap()[1].succeed(
                302,
                vec![HttpHeader {
                    name: "Location".into(),
                    value: "https://evil.test/".into(),
                }],
                vec![],
                false,
            );
            let response = pending.await.unwrap();
            assert_eq!(
                (response.status, response.header("location")),
                (302, Some("https://evil.test/"))
            );
            assert_eq!(
                fake.cancels.load(Ordering::SeqCst),
                1,
                "a finished task was cancelled"
            );
        });
    }

    #[test]
    fn completion_outcomes_map_and_the_first_one_wins() {
        let fake = Arc::new(Fake::default());
        let transport = ForeignTransport(fake.clone());
        rt().block_on(async {
            let pending = transport.http(request());
            let c = fake.http.lock().unwrap()[0].clone();
            c.succeed(200, vec![], b"0123456789".to_vec(), true);
            c.fail(TransportErrorKind::ConnectFailed);
            let response = pending.await.unwrap();
            assert_eq!((response.status, response.body), (200, None));

            for (kind, want) in [
                (
                    TransportErrorKind::ConnectFailed,
                    core::TransportError::ConnectFailed,
                ),
                (TransportErrorKind::Timeout, core::TransportError::Timeout),
                (
                    TransportErrorKind::Cancelled,
                    core::TransportError::Cancelled,
                ),
                (TransportErrorKind::Rejected, core::TransportError::Rejected),
                (TransportErrorKind::Other, core::TransportError::Other),
            ] {
                let pending = transport.http(request());
                fake.http.lock().unwrap().last().unwrap().fail(kind);
                assert_eq!(pending.await.unwrap_err(), want);
            }
            let dial = transport.websocket(
                core::WebSocketRequest::new(
                    "wss://gateway.discord.gg/".into(),
                    vec![],
                    1,
                    Duration::from_secs(1),
                )
                .unwrap(),
            );
            fake.ws.lock().unwrap()[0].rejected(302);
            assert_eq!(
                dial.await.err(),
                Some(core::TransportError::UpgradeStatus(302))
            );
        });
    }

    /// A read core drops mid-wait (its select loop does this on every heartbeat) is not lost: the
    /// next `receive` gets that message and Swift was asked only once.
    #[test]
    fn a_dropped_receive_keeps_its_message_for_the_next_one() {
        let conn = Arc::new(Conn::default());
        let socket = ForeignSocket {
            conn: conn.clone(),
            pending: AsyncMutex::new(None),
        };
        rt().block_on(async {
            use velella_core::transport::WebSocket as _;
            let dropped = tokio::time::timeout(Duration::from_millis(20), socket.receive()).await;
            assert!(dropped.is_err());
            conn.receives.lock().unwrap()[0].binary(vec![1, 2, 3]);
            match socket.receive().await {
                core::Received::Frame(core::Frame::Binary(b)) => assert_eq!(b, [1, 2, 3]),
                other => panic!("{other:?}"),
            }
            assert_eq!(conn.receives.lock().unwrap().len(), 1);
        });
    }

    /// A connection that opens after core gave up on the dial is closed, not leaked.
    #[test]
    fn an_upgrade_finishing_after_the_dial_was_dropped_is_closed() {
        let fake = Arc::new(Fake::default());
        let transport = ForeignTransport(fake.clone());
        let dial = transport.websocket(
            core::WebSocketRequest::new(
                "wss://gateway.discord.gg/".into(),
                vec![],
                1,
                Duration::from_secs(1),
            )
            .unwrap(),
        );
        drop(dial);
        assert_eq!(fake.cancels.load(Ordering::SeqCst), 1);
        let conn = Arc::new(Conn::default());
        fake.ws.lock().unwrap()[0].opened(conn.clone());
        assert_eq!(*conn.closes.lock().unwrap(), [1000]);
    }

    /// Plan NET No logging: what Swift can print of a request or frame never shows the token, and
    /// the Authorization value comes out once.
    #[test]
    fn requests_and_frames_are_redacted_and_authorization_is_read_once() {
        let request = HttpRequest::new(
            HttpMethod::Post,
            format!("https://discord.com/api/v9/x?{MARKER}"),
            vec![HttpHeader {
                name: "X-Context".into(),
                value: MARKER.into(),
            }],
            Some(MARKER.as_bytes().to_vec()),
            Some(MARKER.into()),
            1000,
            8,
        )
        .unwrap();
        let frame = WebSocketFrame::new_text(format!(r#"{{"token":"{MARKER}"}}"#));
        let ws = WebSocketRequest::new(format!("wss://gateway.discord.gg/?{MARKER}"), vec![], 1, 1)
            .unwrap();
        for text in [
            format!("{request:?}"),
            format!("{request}"),
            format!("{frame:?}"),
            format!("{frame}"),
            format!("{ws:?}"),
            format!("{ws}"),
        ] {
            assert!(!text.contains("SYNTHETIC"), "{text}");
        }
        assert!(request
            .headers()
            .iter()
            .all(|h| !h.name.eq_ignore_ascii_case("authorization")));
        assert_eq!(request.take_authorization().as_deref(), Some(MARKER));
        assert_eq!(request.take_authorization(), None);
        assert!(frame.text().unwrap().contains(MARKER));
    }

    /// The upload operation through the foreign trait: the file opens by identity, progress
    /// reaches core's callback, the outcome comes back, and a dropped future cancels the task.
    #[test]
    fn uploads_cross_the_foreign_trait() {
        let fake = Arc::new(Fake::default());
        let transport = ForeignTransport(fake.clone());
        let identity = |size| core::FileIdentity {
            path: format!("/p/{MARKER}"),
            device: 1,
            inode: 2,
            size,
            mtime_ns: 3,
        };
        assert_eq!(
            core::Transport::open_file(&transport, &identity(3)),
            Some(7)
        );
        assert_eq!(core::Transport::open_file(&transport, &identity(4)), None);
        let request = || {
            core::UploadRequest::new(
                core::UploadMethod::Put,
                format!("https://u.test/{MARKER}?q"),
                vec![("Origin".into(), "https://discord.com".into())],
                Some(core::UploadBody {
                    file: 7,
                    start: 1,
                    length: 2,
                }),
                Duration::from_secs(1),
                8,
            )
            .unwrap()
        };
        let seen = Arc::new(Mutex::new(Vec::new()));
        rt().block_on(async {
            let sink = seen.clone();
            let pending = core::Transport::upload(
                &transport,
                request(),
                Box::new(move |n| sink.lock().unwrap().push(n)),
            );
            fake.uploads.lock().unwrap()[0].succeed(200, vec![], vec![], false);
            assert_eq!(pending.await.unwrap().status, 200);
            assert_eq!(*seen.lock().unwrap(), [2]);
            let timed_out = tokio::time::timeout(
                Duration::from_millis(20),
                core::Transport::upload(&transport, request(), Box::new(|_| {})),
            )
            .await;
            assert!(timed_out.is_err());
            assert_eq!(fake.cancels.load(Ordering::SeqCst), 1);
        });
        let ffi = UploadRequest(request());
        for text in [format!("{ffi:?}"), format!("{ffi}")] {
            assert!(!text.contains("SYNTHETIC"), "{text}");
        }
    }

    #[test]
    fn the_test_constructors_keep_core_boundary_checks() {
        let bad_header = HttpRequest::new(
            HttpMethod::Get,
            "https://discord.com/api/v9/x".into(),
            vec![HttpHeader {
                name: "X".into(),
                value: "a\r\nAuthorization: b".into(),
            }],
            None,
            None,
            1000,
            8,
        );
        assert!(matches!(
            bad_header,
            Err(TransportRequestError::InvalidHeader)
        ));
        let smuggled = HttpRequest::new(
            HttpMethod::Get,
            "https://discord.com/api/v9/x".into(),
            vec![HttpHeader {
                name: "Authorization".into(),
                value: MARKER.into(),
            }],
            None,
            None,
            1000,
            8,
        );
        assert!(matches!(
            smuggled,
            Err(TransportRequestError::InvalidHeader)
        ));
        let bad_token = HttpRequest::new(
            HttpMethod::Get,
            "https://discord.com/api/v9/x".into(),
            vec![],
            None,
            Some("SYNTHETIC with space".into()),
            1000,
            8,
        );
        let Err(err) = bad_token else {
            panic!("accepted")
        };
        assert!(!format!("{err} {err:?}").contains("SYNTHETIC"));
    }
}
