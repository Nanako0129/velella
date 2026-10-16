//! REST client for `https://discord.com/api/v9` (M2 W0.5, menu plan F2). One `RestInner` per
//! session holds the token, the generation, the 401 latch, the global and per-route 429 blocks,
//! the single-flight set, the pending MFA challenges, the session's `Identity` and the transport;
//! every `Rest` clone shares it.
//!
//! Security rules, each with a named test in `rest_tests.rs` or `rest_actions_tests.rs`:
//! - The only destination is `https://discord.com/api/v9/...`: a constant base plus a typed path,
//!   validated before anything is sent; a typed path that the URL parser would normalise into
//!   another path (or re-encode in any byte) is refused (S-1). Nothing can change the validated
//!   URL afterwards: the transport sends it byte for byte or refuses it.
//! - Requests go through the session's `Transport` (plan NET): on the app it is URLSession with
//!   the system's trust and proxy settings, like Safari (D15), no cookies, no cache and no
//!   redirects (the app's transport refuses them and its wire tests show it); core adds the
//!   deadline below, the body cap and the header checks at that boundary (T-10).
//! - The token goes only into the request's opaque `Authorization`, never into the header list,
//!   and no request is ever formatted. Errors are fixed text: no transport error, body or header
//!   is ever formatted. An MFA token lives only for the one re-send it authorizes.
//! - Bodies are read through a 4 MiB cap, error bodies included; of an error body only `code`
//!   (and the 429, captcha and MFA fields) is read (S-12).
//! - Every request names its retry policy (`Retry`, no default, S-5). Each loop iteration
//!   re-checks the 401 latch, the global 429 block and the per-route block. A 429 arms the global
//!   block when its body says `global`, whatever the wait source, and always records a per-route
//!   block (S-4, §17.3).
//! - 401 latches REST off for the session and sends one notice to the session task, which emits
//!   `SessionEvent::RestUnauthorized`; a 401 asking for MFA (§18.6) does not latch. It never
//!   touches the gateway or the Keychain (plan Q3).
use crate::{
    identify::Identity,
    token::Token,
    transport::{Authorization, HttpRequest, HttpResponse, Method, Transport, TransportError},
};
use base64::Engine as _;
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use serde_json::Value;
use std::{
    collections::{HashMap, HashSet, VecDeque},
    fmt,
    hash::{BuildHasher, Hasher},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    time::Duration,
};
use tokio::{sync::mpsc, time::Instant};
use url::Url;
use zeroize::Zeroizing;

/// Paths are appended to this, e.g. `/channels/1/messages?limit=50`.
const BASE: &str = "https://discord.com/api/v9";
const MAX_BODY: usize = 4 << 20;
/// Longest wait still worth one automatic retry (`MAX_RETRY_AFTER`, §1.9).
const MAX_RETRY_AFTER: Duration = Duration::from_secs(10);
/// Whole-request deadline; the transport also applies it, and core drops (cancels) the request
/// when it passes.
const TOTAL_TIMEOUT: Duration = Duration::from_secs(30);
/// `retries:N` retries only these statuses (and transport errors), §17.3.
const RETRY_STATUSES: [u16; 8] = [502, 504, 507, 522, 523, 524, 598, 599];
/// A 429 with no readable wait blocks for this long (§17.3).
const DEFAULT_WAIT: Duration = Duration::from_secs(5);
/// Per-route backoff: from 1 s, doubling, capped at 60 s (§17.3 entry Backoff(1000, 60000)).
const ROUTE_BACKOFF_MIN: Duration = Duration::from_secs(1);
const ROUTE_BACKOFF_MAX: Duration = Duration::from_secs(60);
/// A server `message` shown by a slice (I, §9.6) is cut to this many characters.
const MESSAGE_CAP: usize = 512;
/// Unanswered MFA challenges kept per session; the oldest goes first.
const MFA_PENDING: usize = 4;
/// MFA-required (§18.6) and invalid-code error codes.
const MFA_REQUIRED: u64 = 60003;

/// Fixed-text failures of the pre-F2 calls (message history, the legacy send). None carries a
/// transport error, a body or a header.
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

/// The error codes F2's callers act on (Errors row); everything else is `Other`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ApiCode {
    /// 10008
    UnknownMessage,
    /// 30003
    TooManyPins,
    /// 30010
    TooManyReactions,
    /// 30074
    TooManySavedMessages,
    /// 50001
    InvalidAccess,
    /// 50019
    InvalidPinChannel,
    /// 50021
    SystemMessage,
    /// 50035
    InvalidFormBody,
    /// 50083
    ArchivedThread,
    /// 60008, on an MFA re-send or `/mfa/finish` (§18.6)
    MfaInvalidCode,
    /// 90001
    ReactionBlocked,
    Other,
}

impl ApiCode {
    fn from_code(code: Option<u64>) -> ApiCode {
        match code {
            Some(10008) => Self::UnknownMessage,
            Some(30003) => Self::TooManyPins,
            Some(30010) => Self::TooManyReactions,
            Some(30074) => Self::TooManySavedMessages,
            Some(50001) => Self::InvalidAccess,
            Some(50019) => Self::InvalidPinChannel,
            Some(50021) => Self::SystemMessage,
            Some(50035) => Self::InvalidFormBody,
            Some(50083) => Self::ArchivedThread,
            Some(60008) => Self::MfaInvalidCode,
            Some(90001) => Self::ReactionBlocked,
            _ => Self::Other,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MfaKind {
    Webauthn,
    Totp,
    Sms,
    Backup,
    Password,
}

impl MfaKind {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Webauthn => "webauthn",
            Self::Totp => "totp",
            Self::Sms => "sms",
            Self::Backup => "backup",
            Self::Password => "password",
        }
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct MfaMethod {
    pub kind: MfaKind,
    /// WebAuthn's challenge (read only for `webauthn`, §18.6).
    pub challenge: Option<String>,
}

/// An MFA prompt for a request core keeps (with its ticket) until it is answered: the caller
/// answers by `id`; the ticket and the token never leave core.
#[derive(Clone, PartialEq, Eq)]
pub struct MfaChallenge {
    pub id: u64,
    pub methods: Vec<MfaMethod>,
}

/// Typed failures of F2's actions. `Debug` and `Display` never show a server message, a ticket
/// or a challenge.
#[derive(Clone, PartialEq, Eq)]
pub enum ActionError {
    /// 401, or REST already latched off by one this session.
    Unauthorized,
    /// 401 with code 60003 and an `mfa` object; does not latch.
    MfaRequired(MfaChallenge),
    /// A 429, or a global or per-route block still running (nothing was sent then).
    RateLimited,
    Captcha,
    /// Any other non-success status (a redirect included), with the code F2 knows. `message` is
    /// the server's, cut to 512 characters, kept only for the saved-message actions (§9.3, §9.6):
    /// for display, never for a log.
    Api {
        status: u16,
        code: ApiCode,
        message: Option<String>,
    },
    /// Connect, TLS, timeout or transport failure.
    Network,
    TooLarge,
    /// A success body that is not the expected JSON.
    BadResponse,
    /// A URL that fails validation; nothing was sent.
    InvalidUrl,
    /// An input core refuses (an emoji, a limit, a date); nothing was sent.
    InvalidInput,
    /// The same write is already in flight (S-16); nothing was sent.
    Busy,
    /// Settings-proto writes are off for this session (a decode failed), S-2.
    SettingsUnavailable,
}

impl fmt::Debug for ActionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Api { status, code, .. } => write!(f, "Api({status}, {code:?})"),
            Self::MfaRequired(c) => write!(f, "MfaRequired({})", c.id),
            other => f.write_str(match other {
                Self::Unauthorized => "Unauthorized",
                Self::RateLimited => "RateLimited",
                Self::Captcha => "Captcha",
                Self::Network => "Network",
                Self::TooLarge => "TooLarge",
                Self::BadResponse => "BadResponse",
                Self::InvalidUrl => "InvalidUrl",
                Self::InvalidInput => "InvalidInput",
                Self::Busy => "Busy",
                _ => "SettingsUnavailable",
            }),
        }
    }
}

impl fmt::Display for ActionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Unauthorized => "Discord did not accept this sign-in",
            Self::MfaRequired(_) => "Discord asked for multi-factor authentication",
            Self::RateLimited => "rate limited",
            Self::Captcha => "Discord asked for a captcha",
            Self::Api { .. } => "request failed",
            Self::Network => "network error",
            Self::TooLarge => "response too large",
            Self::BadResponse => "unreadable response",
            Self::InvalidUrl => "invalid request",
            Self::InvalidInput => "invalid input",
            Self::Busy => "already in progress",
            Self::SettingsUnavailable => "settings unavailable",
        })
    }
}

impl std::error::Error for ActionError {}

impl From<ActionError> for RestError {
    fn from(e: ActionError) -> RestError {
        match e {
            ActionError::Unauthorized => RestError::Unauthorized,
            ActionError::RateLimited => RestError::RateLimited,
            ActionError::Captcha => RestError::Captcha,
            ActionError::Api { status: 403, .. } => RestError::Forbidden,
            ActionError::Network => RestError::Network,
            ActionError::TooLarge => RestError::TooLarge,
            ActionError::BadResponse => RestError::BadResponse,
            ActionError::InvalidUrl => RestError::InvalidUrl,
            _ => RestError::Failed,
        }
    }
}

/// The retry policy of one request (F2 retry table). Required: no call has a default.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Retry {
    /// Never retried.
    None,
    /// Message history (pre-F2): one retry after a readable 429 of at most 10 s.
    History,
    /// §1.9: a readable 429 within 10 s → wait and retry once; any other non-2xx except 401,
    /// captcha, 403 30010 / 90001 and 3xx, or a transport error → one immediate retry.
    Reaction,
    /// `retries:N` (§17.3): the retry statuses and transport errors, backoff, X-Failed-Requests.
    Statuses(u8),
    /// `retries:1`, plus one re-send after a 429 within 10 s (§4.3).
    Edit,
    /// D11: only a connection that failed to establish, or 522 / 523; three times, same bytes.
    Send,
}

/// `X-Context-Properties` locations F2 sends (§17.1).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Context {
    ChatInput,
    Forwarding,
}

impl Context {
    /// Standard padded base64 of `{"location":"…"}`.
    fn header(self) -> String {
        let location = match self {
            Self::ChatInput => "chat_input",
            Self::Forwarding => "forwarding",
        };
        base64::engine::general_purpose::STANDARD.encode(format!(r#"{{"location":"{location}"}}"#))
    }
}

/// One request as composed: method, the validated URL, the JSON body bytes and its closed set of
/// extra headers. A retry or an MFA re-send sends exactly these bytes again.
#[derive(Clone)]
pub(crate) struct Call {
    method: Method,
    url: Url,
    body: Option<Vec<u8>>,
    context: Option<Context>,
    keep_message: bool,
}

impl Call {
    /// `path` (with its query) after `BASE`, checked by `typed` (S-1).
    pub(crate) fn new(method: Method, path: &str) -> Result<Call, ActionError> {
        Ok(Call {
            method,
            url: typed(path).map_err(|_| ActionError::InvalidUrl)?,
            body: None,
            context: None,
            keep_message: false,
        })
    }

    /// The body as serde writes it: struct field order, `None` fields skipped where marked.
    pub(crate) fn json(mut self, body: &impl Serialize) -> Result<Call, ActionError> {
        self.body = Some(serde_json::to_vec(body).map_err(|_| ActionError::InvalidInput)?);
        Ok(self)
    }

    pub(crate) fn context(mut self, context: Context) -> Call {
        self.context = Some(context);
        self
    }

    /// Keeps the server's `message` of an error (capped), for the saved-message actions.
    pub(crate) fn keep_message(mut self) -> Call {
        self.keep_message = true;
        self
    }
}

/// Cheap handle; every clone shares the session's latch, deadlines, token and transport.
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

/// A per-route block: requests to the path wait until `until`; `backoff` grows on each 429.
struct Route {
    until: Instant,
    backoff: Duration,
}

/// Writes that may not overlap (S-16): per (message, emoji, reaction type) for reactions, per
/// message and kind for edit, delete and pin / unpin.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(crate) enum Flight {
    Reaction(u64, String, u8),
    Edit(u64),
    Delete(u64),
    Pin(u64),
}

/// Held while a write is in flight; dropping it frees the key.
pub(crate) struct FlightGuard(Rest, Flight);

impl Drop for FlightGuard {
    fn drop(&mut self) {
        lock(&self.0 .0.flights).remove(&self.1);
    }
}

struct PendingMfa {
    id: u64,
    ticket: Zeroizing<String>,
    call: Call,
}

#[derive(Default)]
struct MfaState {
    next_id: u64,
    pending: VecDeque<PendingMfa>,
}

struct RestInner {
    token: Arc<TokenSlot>,
    generation: u64,
    transport: Arc<dyn Transport>,
    identity: Arc<Identity>,
    unauthorized: AtomicBool,
    global_until: Mutex<Option<Instant>>,
    routes: Mutex<HashMap<String, Route>>,
    flights: Mutex<HashSet<Flight>>,
    mfa: Mutex<MfaState>,
    /// Settings-proto writes are off for the session once a type-2 decode failed (S-2).
    settings_disabled: AtomicBool,
    /// To the session task; it emits `RestUnauthorized`. Sent at most once (the latch's swap).
    notice: mpsc::UnboundedSender<()>,
}

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
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
/// segment or an encoded one (`%2e`) would otherwise reach another endpoint. The whole URL must
/// also serialize back to the composed string, so no byte is re-encoded on the way.
fn typed(path: &str) -> Result<Url, RestError> {
    let url = format!("{BASE}{path}");
    let written = path.split(['?', '#']).next().unwrap_or_default();
    let parsed = validated(&url).ok_or(RestError::InvalidUrl)?;
    (parsed.path() == format!("/api/v9{written}") && parsed.as_str() == url)
        .then_some(parsed)
        .ok_or(RestError::InvalidUrl)
}

/// `retries` backoff (§17.3): from 500 ms, each failure `current += 2·current·random`, capped
/// at 5 s.
struct Backoff(Duration);

impl Backoff {
    fn new() -> Backoff {
        Backoff(Duration::from_millis(500))
    }

    fn fail(&mut self) -> Duration {
        let random = (std::collections::hash_map::RandomState::new()
            .build_hasher()
            .finish()
            >> 11) as f64
            / (1u64 << 53) as f64;
        self.0 = (self.0 + self.0.mul_f64(2.0 * random)).min(Duration::from_secs(5));
        self.0
    }
}

/// Why one attempt did not succeed.
enum Fail {
    /// No retry under any policy (401, MFA, captcha, 3xx, a refused URL).
    Final(ActionError),
    /// 429: `readable` is the server's wait when one was readable; `until` is the per-route
    /// deadline just recorded.
    Limited {
        readable: Option<Duration>,
        until: Instant,
    },
    Status(u16, ActionError),
    Transport(TransportError),
}

impl Fail {
    fn error(self) -> ActionError {
        match self {
            Self::Final(e) | Self::Status(_, e) => e,
            Self::Limited { .. } => ActionError::RateLimited,
            Self::Transport(TransportError::Rejected) => ActionError::InvalidUrl,
            Self::Transport(_) => ActionError::Network,
        }
    }
}

/// `Retry-After` as `parseInt` reads it (whole seconds, > 0), else the body's `retry_after`
/// (> 0); `None` when neither is readable.
fn retry_after(response: &HttpResponse, body: Option<&Value>) -> Option<Duration> {
    if let Some(header) = response.header("retry-after") {
        let digits: String = header
            .trim_start()
            .chars()
            .take_while(char::is_ascii_digit)
            .collect();
        if let Some(secs) = digits.parse::<u64>().ok().filter(|s| *s > 0) {
            return Some(Duration::from_secs(secs));
        }
    }
    let secs = body?.get("retry_after")?.as_f64()?;
    (secs > 0.0)
        .then(|| Duration::try_from_secs_f64(secs).ok())
        .flatten()
}

fn later(wait: Duration) -> Instant {
    let now = Instant::now();
    now.checked_add(wait)
        .unwrap_or_else(|| now + Duration::from_secs(86_400))
}

fn mfa_methods(mfa: &Value) -> Option<(String, Vec<MfaMethod>)> {
    let ticket = mfa.get("ticket")?.as_str()?.to_owned();
    let methods = mfa
        .get("methods")
        .and_then(Value::as_array)
        .map(|all| {
            all.iter()
                .filter_map(|m| {
                    let kind = match m.get("type")?.as_str()? {
                        "webauthn" => MfaKind::Webauthn,
                        "totp" => MfaKind::Totp,
                        "sms" => MfaKind::Sms,
                        "backup" => MfaKind::Backup,
                        "password" => MfaKind::Password,
                        _ => return None,
                    };
                    let challenge = (kind == MfaKind::Webauthn)
                        .then(|| m.get("challenge")?.as_str().map(str::to_owned))
                        .flatten();
                    Some(MfaMethod { kind, challenge })
                })
                .collect()
        })
        .unwrap_or_default();
    Some((ticket, methods))
}

/// A success body as `T`; an empty body reads as JSON `null`.
pub(crate) fn parse<T: DeserializeOwned>(body: &[u8]) -> Result<T, ActionError> {
    let body: &[u8] = if body.is_empty() { b"null" } else { body };
    serde_json::from_slice(body).map_err(|_| ActionError::BadResponse)
}

#[derive(Serialize)]
struct MfaFinish<'a> {
    ticket: &'a str,
    mfa_type: &'static str,
    data: &'a str,
}

#[derive(Serialize)]
struct MfaTicket<'a> {
    ticket: &'a str,
}

#[derive(Deserialize)]
struct MfaToken {
    token: String,
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
            routes: Mutex::new(HashMap::new()),
            flights: Mutex::new(HashSet::new()),
            mfa: Mutex::new(MfaState::default()),
            settings_disabled: AtomicBool::new(false),
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

    /// GET `BASE` + `path`; `path` starts with `/`. Message history only: one retry after a
    /// readable 429 of at most 10 s.
    pub(crate) async fn get<T: DeserializeOwned>(&self, path: &str) -> Result<T, RestError> {
        let url = typed(path)?;
        self.request(Method::Get, url.as_str(), None).await
    }

    /// POST a JSON body to `BASE` + `path`. Never retried automatically.
    #[allow(dead_code)] // rest_tests; sends now go through `rest_messages`.
    pub(crate) async fn post<T: DeserializeOwned>(
        &self,
        path: &str,
        body: &impl Serialize,
    ) -> Result<T, RestError> {
        let url = typed(path)?;
        let body = serde_json::to_vec(body).map_err(|_| RestError::Failed)?;
        self.request(Method::Post, url.as_str(), Some(body)).await
    }

    /// The pre-F2 entry: a validated URL, GET with the history policy, anything else never
    /// retried, errors as `RestError`.
    pub(crate) async fn request<T: DeserializeOwned>(
        &self,
        method: Method,
        url: &str,
        body: Option<Vec<u8>>,
    ) -> Result<T, RestError> {
        let url = validated(url).ok_or(RestError::InvalidUrl)?;
        let call = Call {
            method,
            url,
            body,
            context: None,
            keep_message: false,
        };
        let retry = if method == Method::Get {
            Retry::History
        } else {
            Retry::None
        };
        let body = self.run(&call, retry, None).await?;
        Ok(parse(&body)?)
    }

    /// One F2 request under its policy, the success body parsed as `T`.
    pub(crate) async fn call<T: DeserializeOwned>(
        &self,
        call: &Call,
        retry: Retry,
    ) -> Result<T, ActionError> {
        parse(&self.run(call, retry, None).await?)
    }

    /// The session's identity headers, then `Content-Type` when there is a body.
    fn headers(&self, has_body: bool) -> Vec<(String, String)> {
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
        if has_body {
            headers.push(("Content-Type".into(), "application/json".into()));
        }
        headers
    }

    /// The session's identity headers, the token in its own field, the deadline and the cap.
    #[allow(dead_code)] // rest_tests: what a plain request carries.
    pub(crate) fn build_request(
        &self,
        method: Method,
        url: &Url,
        body: Option<Vec<u8>>,
    ) -> Result<HttpRequest, RestError> {
        HttpRequest::new(
            method,
            url.as_str().to_owned(),
            self.headers(body.is_some()),
            body,
            Some(Authorization::new(self.0.token.current())),
            TOTAL_TIMEOUT,
            MAX_BODY,
        )
        .map_err(|_| RestError::Failed)
    }

    /// `build_request` plus the call's closed set of extra headers (§17.1, §18.5, §18.6).
    fn build(
        &self,
        call: &Call,
        failed_requests: u32,
        mfa: Option<&str>,
    ) -> Result<HttpRequest, ActionError> {
        let mut headers = self.headers(call.body.is_some());
        if let Some(context) = call.context {
            headers.push(("X-Context-Properties".to_owned(), context.header()));
        }
        if failed_requests > 0 {
            headers.push(("X-Failed-Requests".to_owned(), failed_requests.to_string()));
        }
        if let Some(token) = mfa {
            headers.push(("X-Discord-MFA-Authorization".to_owned(), token.to_owned()));
        }
        HttpRequest::new(
            call.method,
            call.url.as_str().to_owned(),
            headers,
            call.body.clone(),
            Some(Authorization::new(self.0.token.current())),
            TOTAL_TIMEOUT,
            MAX_BODY,
        )
        // An MFA token that is not a valid header value is refused here, never sent.
        .map_err(|_| ActionError::InvalidInput)
    }

    /// One exchange under core's own deadline: when it passes, the request future is dropped,
    /// which cancels it in the transport. The body is re-checked against the cap here.
    async fn send(&self, request: HttpRequest) -> Result<HttpResponse, TransportError> {
        let cap = request.body_cap();
        let deadline = request.deadline();
        match tokio::time::timeout(deadline, self.0.transport.http(request)).await {
            Err(_) => Err(TransportError::Timeout),
            Ok(Err(e)) => Err(e),
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

    /// The 401 latch, the global block and the path's block, before every attempt.
    fn precheck(&self, path: &str) -> Result<(), ActionError> {
        if self.0.unauthorized.load(Ordering::SeqCst) {
            return Err(ActionError::Unauthorized);
        }
        let now = Instant::now();
        if lock(&self.0.global_until).is_some_and(|until| now < until)
            || lock(&self.0.routes)
                .get(path)
                .is_some_and(|r| now < r.until)
        {
            return Err(ActionError::RateLimited);
        }
        Ok(())
    }

    /// Records a 429: the global block when the body says so, and the path's block of
    /// max(wait, backoff). Returns the path's deadline.
    fn rate_limited(&self, path: &str, wait: Duration, global: bool) -> Instant {
        if global {
            *lock(&self.0.global_until) = Some(later(wait));
        }
        let now = Instant::now();
        let mut routes = lock(&self.0.routes);
        // A route quiet for a minute starts over (and the map stays small).
        routes.retain(|_, r| now < r.until + ROUTE_BACKOFF_MAX);
        let backoff = routes.get(path).map_or(ROUTE_BACKOFF_MIN, |r| {
            (r.backoff * 2).min(ROUTE_BACKOFF_MAX)
        });
        let until = later(wait.max(backoff));
        routes.insert(path.to_owned(), Route { until, backoff });
        until
    }

    /// A non-2xx answer: blocks, latch and MFA recorded here.
    fn classify(&self, call: &Call, response: HttpResponse) -> Fail {
        let status = response.status;
        let body: Option<Value> = response
            .body
            .as_deref()
            .and_then(|b| serde_json::from_slice(b).ok());
        let code = body.as_ref().and_then(|b| b.get("code")?.as_u64());
        match status {
            401 => {
                let mfa = body
                    .as_ref()
                    .filter(|_| code == Some(MFA_REQUIRED))
                    .and_then(|b| b.get("mfa"))
                    .filter(|m| m.is_object())
                    .and_then(mfa_methods);
                match mfa {
                    Some((ticket, methods)) => Fail::Final(ActionError::MfaRequired(
                        self.stash_mfa(ticket, methods, call.clone()),
                    )),
                    None => {
                        self.latch();
                        Fail::Final(ActionError::Unauthorized)
                    }
                }
            }
            429 => {
                let readable = retry_after(&response, body.as_ref());
                let global = body
                    .as_ref()
                    .and_then(|b| b.get("global")?.as_bool())
                    .unwrap_or(false);
                let until =
                    self.rate_limited(call.url.path(), readable.unwrap_or(DEFAULT_WAIT), global);
                Fail::Limited { readable, until }
            }
            _ if body
                .as_ref()
                .is_some_and(|b| b.get("captcha_key").is_some()) =>
            {
                Fail::Final(ActionError::Captcha)
            }
            _ => {
                let message = body
                    .as_ref()
                    .filter(|_| call.keep_message)
                    .and_then(|b| b.get("message")?.as_str())
                    .map(|m| m.chars().take(MESSAGE_CAP).collect());
                let error = ActionError::Api {
                    status,
                    code: ApiCode::from_code(code),
                    message,
                };
                if (300..400).contains(&status) {
                    Fail::Final(error)
                } else {
                    Fail::Status(status, error)
                }
            }
        }
    }

    /// The request loop: one attempt, then the policy decides. Returns the 2xx body.
    async fn run(
        &self,
        call: &Call,
        retry: Retry,
        mfa: Option<&str>,
    ) -> Result<Vec<u8>, ActionError> {
        let path = call.url.path().to_owned();
        let mut backoff = Backoff::new();
        let mut failed_requests = 0u32;
        let mut statuses_left = match retry {
            Retry::Statuses(n) => u32::from(n),
            Retry::Edit => 1,
            Retry::Send => 3,
            _ => 0,
        };
        let mut once_used = false;
        loop {
            self.precheck(&path)?;
            let request = self.build(call, failed_requests, mfa)?;
            let fail = match self.send(request).await {
                Ok(response) if (200..300).contains(&response.status) => {
                    lock(&self.0.routes).remove(&path);
                    return response.body.ok_or(ActionError::TooLarge);
                }
                Ok(response) => self.classify(call, response),
                Err(e) => Fail::Transport(e),
            };
            if matches!(
                fail,
                Fail::Final(_)
                    | Fail::Transport(TransportError::Rejected | TransportError::Cancelled)
            ) {
                return Err(fail.error());
            }
            // Within the 10 s cap, the time until the path's block ends.
            let wait_within_cap = |until: Instant| {
                let wait = until.saturating_duration_since(Instant::now());
                (wait <= MAX_RETRY_AFTER).then_some(until)
            };
            match retry {
                Retry::None => return Err(fail.error()),
                Retry::History => match fail {
                    Fail::Limited {
                        readable: Some(_),
                        until,
                    } if !once_used => {
                        let Some(until) = wait_within_cap(until) else {
                            return Err(ActionError::RateLimited);
                        };
                        once_used = true;
                        tokio::time::sleep_until(until).await;
                    }
                    fail => return Err(fail.error()),
                },
                Retry::Reaction => {
                    if once_used {
                        return Err(fail.error());
                    }
                    once_used = true;
                    match fail {
                        Fail::Limited {
                            readable: Some(_),
                            until,
                        } => {
                            let Some(until) = wait_within_cap(until) else {
                                return Err(ActionError::RateLimited);
                            };
                            tokio::time::sleep_until(until).await;
                        }
                        Fail::Limited { readable: None, .. } => {
                            return Err(ActionError::RateLimited)
                        }
                        Fail::Status(
                            403,
                            e @ ActionError::Api {
                                code: ApiCode::TooManyReactions | ApiCode::ReactionBlocked,
                                ..
                            },
                        ) => return Err(e),
                        // Any other status or transport error: one immediate retry.
                        _ => {}
                    }
                }
                Retry::Statuses(_) | Retry::Edit | Retry::Send => {
                    let retryable = match (&fail, retry) {
                        (Fail::Transport(TransportError::ConnectFailed), _) => true,
                        (Fail::Status(s, _), Retry::Send) => matches!(s, 522 | 523),
                        (Fail::Transport(_), Retry::Send) => false,
                        (Fail::Status(s, _), _) => RETRY_STATUSES.contains(s),
                        (Fail::Transport(_), _) => true,
                        _ => false,
                    };
                    if retryable && statuses_left > 0 {
                        statuses_left -= 1;
                        failed_requests += 1;
                        tokio::time::sleep(backoff.fail()).await;
                        continue;
                    }
                    match fail {
                        // §4.3: the queue re-sends an edit once after a 429, as a new request
                        // (so without X-Failed-Requests).
                        Fail::Limited { until, .. } if retry == Retry::Edit && !once_used => {
                            let Some(until) = wait_within_cap(until) else {
                                return Err(ActionError::RateLimited);
                            };
                            once_used = true;
                            failed_requests = 0;
                            tokio::time::sleep_until(until).await;
                        }
                        fail => return Err(fail.error()),
                    }
                }
            }
        }
    }

    /// Claims `key` for one write; `Busy` (nothing sent) while another holds it (S-16).
    pub(crate) fn flight(&self, key: Flight) -> Result<FlightGuard, ActionError> {
        lock(&self.0.flights)
            .insert(key.clone())
            .then(|| FlightGuard(self.clone(), key))
            .ok_or(ActionError::Busy)
    }

    pub(crate) fn settings_disabled(&self) -> bool {
        self.0.settings_disabled.load(Ordering::SeqCst)
    }

    pub(crate) fn disable_settings(&self) {
        self.0.settings_disabled.store(true, Ordering::SeqCst);
    }

    fn stash_mfa(&self, ticket: String, methods: Vec<MfaMethod>, call: Call) -> MfaChallenge {
        let mut mfa = lock(&self.0.mfa);
        mfa.next_id += 1;
        let id = mfa.next_id;
        if mfa.pending.len() >= MFA_PENDING {
            mfa.pending.pop_front();
        }
        mfa.pending.push_back(PendingMfa {
            id,
            ticket: Zeroizing::new(ticket),
            call,
        });
        MfaChallenge { id, methods }
    }

    fn pending_mfa(&self, id: u64) -> Result<(Zeroizing<String>, Call), ActionError> {
        lock(&self.0.mfa)
            .pending
            .iter()
            .find(|p| p.id == id)
            .map(|p| (p.ticket.clone(), p.call.clone()))
            .ok_or(ActionError::InvalidInput)
    }

    fn forget_mfa(&self, id: u64) {
        lock(&self.0.mfa).pending.retain(|p| p.id != id);
    }

    /// `POST /auth/mfa/sms/send {ticket}` for challenge `id` (§18.6). Never retried.
    pub(crate) async fn mfa_send_sms(&self, id: u64) -> Result<(), ActionError> {
        let (ticket, _) = self.pending_mfa(id)?;
        let call =
            Call::new(Method::Post, "/auth/mfa/sms/send")?.json(&MfaTicket { ticket: &ticket })?;
        self.run(&call, Retry::None, None).await.map(drop)
    }

    /// Answers challenge `id`: `POST /mfa/finish {ticket, mfa_type, data}` (`retries:2`, §18.6)
    /// gives a token, and the original request is sent again unchanged with
    /// `X-Discord-MFA-Authorization`. The token lives only for that re-send. A 60008 keeps the
    /// challenge for another try; a 60003 comes back as a new challenge; anything else ends it.
    pub(crate) async fn mfa_submit(
        &self,
        id: u64,
        kind: MfaKind,
        data: &str,
    ) -> Result<(), ActionError> {
        let (ticket, original) = self.pending_mfa(id)?;
        let finish = Call::new(Method::Post, "/mfa/finish")?.json(&MfaFinish {
            ticket: &ticket,
            mfa_type: kind.as_str(),
            data,
        })?;
        let token: MfaToken = self.call(&finish, Retry::Statuses(2)).await?;
        let token = Zeroizing::new(token.token);
        let result = self.run(&original, Retry::None, Some(&token)).await;
        match &result {
            Err(ActionError::Api {
                code: ApiCode::MfaInvalidCode,
                ..
            }) => {}
            _ => self.forget_mfa(id),
        }
        result.map(drop)
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
