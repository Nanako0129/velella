//! Attachment upload (composer plan rev 1.8, slice CU; OA = refs/discord-ground-truth/attachments.md).
//!
//! A send with attachments runs at send time (D10): the limits, then a pending echo row in the
//! message store (messages.rs) keyed by the send's nonce, then for each file in order the slot
//! request (`POST /channels/{c}/attachments`, rest_messages.rs), the upload to the host the slot
//! names, and finally one message create referencing them all. An upload failure creates no
//! message and turns the row failed; retry starts again from the slots; cancel follows D14a and
//! CU-9; discard removes the row.
//!
//! Rules, each with a named test in uploads_tests.rs:
//! - The upload host is exactly `UPLOAD_HOST` over https, with no user, password, port, fragment
//!   or dot segment, and a query (D13a, `upload_url`). Anything else sends nothing more: the file
//!   fails `HostRefused` and the host alone goes to the launch's one-time diagnostic line.
//! - The upload does not go through `Rest`: its requests are `UploadRequest`s, which have no
//!   Authorization field, and their header lists come from `headers` alone, Safari's closed lists
//!   in Safari's order (D15a, OA A40-A43). The app's transport refuses any other header name.
//! - Every PUT and probe is preceded by a CORS preflight, reused only within the response's
//!   `Access-Control-Max-Age` (5 s when absent, capped at 600 s) for the same URL and request-header
//!   names; a preflight that does not allow `https://discord.com`, PUT and the headers fails the
//!   file before any PUT (`Preflights`).
//! - The body streams from a file the app opened and verified (CU-6, `Transport::open_file`),
//!   never through core as bytes. Resume (CU-3): a probe PUT with `Content-Range: bytes */*`; a
//!   308 `Range` is accepted only as `bytes=0-N` with `N + 1 <= size` (`resume_at`); one counter of
//!   at most `MAX_ATTEMPTS` over probes and PUTs; deadlines `PUT_DEADLINE` and `PROBE_DEADLINE`.
//! - Limits are byte constants (P4), checked at add time for an account without premium and at
//!   send for every account (OA A36), each case its own `LimitError`.
//! - Core refuses a send with attachments unless the channel is a DM or F1 says ATTACH_FILES is
//!   held; Unknown refuses (CU-10).
//! - DELETE of an upload (cancel before the create was dispatched, CU-9) uses only this send's
//!   `upload_filename`s, encoded and checked by `delete_path` (CU-8).
//! - Nothing here formats a URL, query, path, filename, description, `upload_filename` or server
//!   text (CU-7): the types that carry them have hand-written `Debug`.
use crate::{
    experiments::{Gate, HEIC_CONVERSION, JXR_CONVERSION, NITRO_FILE_UPLOAD},
    identify::Identity,
    messages::MessageStore,
    permissions::{channel_permissions, ChannelPermissions, ATTACH_FILES},
    rest::{ActionError, ApiCode},
    rest_actions::guild_of,
    rest_messages::{AttachmentEntry, Reply},
    session::SessionHandle,
    state::State,
    transport::{
        FileIdentity, HttpResponse, Transport, TransportError, UploadBody, UploadMethod,
        UploadRequest,
    },
};
use serde_json::Value;
use std::{
    collections::HashMap,
    fmt,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc, Mutex,
    },
    time::Duration,
};
use tokio::time::Instant;
use url::Url;

/// The one host uploads go to (D13a).
pub const UPLOAD_HOST: &str = "discord-attachments-uploads-prd.storage.googleapis.com";
const UPLOAD_PREFIX: &str = "https://discord-attachments-uploads-prd.storage.googleapis.com/";
const ORIGIN: &str = "https://discord.com";
const REFERER: &str = "https://discord.com/";
pub const MAX_FILES: usize = 10;
/// The P4 byte constants (OA A14-A17).
pub const MIB_10: u64 = 10_485_760;
pub const MIB_20: u64 = 20_971_520;
pub const MIB_50: u64 = 52_428_800;
pub const MIB_100: u64 = 104_857_600;
pub const MB_500: u64 = 524_288_000;
pub const GIB_1: u64 = 1_073_741_824;
const PUT_DEADLINE: Duration = Duration::from_secs(3600);
const PROBE_DEADLINE: Duration = Duration::from_secs(30);
/// One counter over probes and PUTs (CU-3).
const MAX_ATTEMPTS: u32 = 13;
/// Upload responses are read only for their status and headers (CU Transport).
const RESPONSE_CAP: usize = 64 << 10;
/// A preflight without `Access-Control-Max-Age` is kept this long; WebKit's cap (D15a).
const PREFLIGHT_DEFAULT: Duration = Duration::from_secs(5);
const PREFLIGHT_MAX: Duration = Duration::from_secs(600);

// ---- Descriptors ----

/// One attached file as the app describes it: its verified identity (CU-6), the name the
/// message shows (Modify-edited; the slot uses the file's own name), alt text, spoiler, the
/// original's MIME type (`""` when unknown, CU-5) and, for a converted HEIC / JXR, the
/// original's MD5 (D16a). `Debug` shows the size and spoiler flag only (CU-7).
#[derive(Clone, PartialEq, Eq)]
pub struct AttachmentDescriptor {
    pub file: FileIdentity,
    pub filename: String,
    pub description: Option<String>,
    pub spoiler: bool,
    pub content_type: String,
    pub original_md5: Option<String>,
}

impl fmt::Debug for AttachmentDescriptor {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "AttachmentDescriptor([REDACTED], {} bytes, spoiler: {})",
            self.file.size, self.spoiler
        )
    }
}

impl AttachmentDescriptor {
    /// The slot's `filename`: the uploaded file's own name, the last path component (a synthesized
    /// file's directory is `velella-attachments/<UUID>/`, CU-1).
    fn slot_filename(&self) -> &str {
        self.file.path.rsplit('/').next().unwrap_or_default()
    }

    /// The MD5 header value, only when it is 32 lowercase hex digits (OA A49).
    fn md5(&self) -> Option<String> {
        self.original_md5
            .clone()
            .filter(|m| m.len() == 32 && m.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f')))
    }
}

/// `attachments[0]` of a slot response (OA A3). `Debug` shows nothing of either value (CU-7).
pub(crate) struct Slot {
    pub(crate) upload_url: String,
    pub(crate) upload_filename: String,
}

impl Slot {
    pub(crate) fn parse(v: &Value) -> Option<Slot> {
        let first = v.get("attachments")?.as_array()?.first()?;
        Some(Slot {
            upload_url: first.get("upload_url")?.as_str()?.to_owned(),
            upload_filename: first.get("upload_filename")?.as_str()?.to_owned(),
        })
    }
}

impl fmt::Debug for Slot {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Slot([REDACTED])")
    }
}

/// The slot `id`: a process-wide decimal counter from 1 (lodash `uniqueId`, P4).
fn next_slot_id() -> u64 {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    NEXT.fetch_add(1, Ordering::Relaxed) + 1
}

// ---- Failures ----

/// Why an attachment send failed; the row shows `text`, fixed text only.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UploadFailure {
    Unauthorized,
    Captcha,
    RateLimited,
    Network,
    /// 403, e.g. no permission in the channel.
    Forbidden,
    /// The server's 40005 (OA A18).
    TooLarge,
    /// The slot named a host other than `UPLOAD_HOST` (D13a); nothing was sent there.
    HostRefused,
    /// The file is gone or changed since it was attached (CU-6); nothing was sent.
    FileChanged,
    /// The upload host's preflight did not allow the upload (D15a); no PUT was sent.
    PreflightRefused,
    /// The upload host refused the PUT, a resume answer was refused, or the attempts ran out.
    UploadFailed,
    BadResponse,
    Failed,
}

impl UploadFailure {
    pub fn text(self) -> &'static str {
        match self {
            Self::Unauthorized => "Upload Failed: Discord did not accept this sign-in",
            Self::Captcha => "Upload Failed: Discord asked for a captcha",
            Self::RateLimited => "Upload Failed: rate limited",
            Self::Network => "Upload Failed: network error",
            Self::Forbidden => "Upload Failed: no permission to upload here",
            Self::TooLarge => "Upload Failed: the file is too large",
            Self::HostRefused => "Upload Failed: Discord named an unexpected upload host",
            Self::FileChanged => "Upload Failed: the file changed or is no longer available",
            Self::PreflightRefused => "Upload Failed: the upload host refused the upload",
            Self::UploadFailed => "Upload Failed",
            Self::BadResponse => "Upload Failed: unreadable response",
            Self::Failed => "Upload Failed",
        }
    }

    fn of(e: ActionError) -> UploadFailure {
        match e {
            ActionError::Unauthorized => Self::Unauthorized,
            ActionError::Captcha => Self::Captcha,
            ActionError::RateLimited => Self::RateLimited,
            ActionError::Network => Self::Network,
            ActionError::Api {
                code: ApiCode::EntityTooLarge,
                ..
            } => Self::TooLarge,
            ActionError::Api { status: 403, .. } => Self::Forbidden,
            ActionError::TooLarge | ActionError::BadResponse => Self::BadResponse,
            _ => Self::Failed,
        }
    }
}

// ---- Limits ----

/// One limit a set of attachments breaks (OA A13-A17, A37); nothing is added or sent.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LimitError {
    TooManyFiles { limit: usize },
    EmptyFile,
    FileTooLarge { limit: u64 },
    TotalTooLarge { limit: u64 },
}

/// When the check runs: adding to the composer, or sending (OA A36).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LimitCheck {
    Add,
    Send,
}

/// `max(20 MiB, max(guild boost limit, user limit))` (OA A14-A16). A guild whose features are
/// unknown counts as unboosted; an unknown `premium_type` as none; an unknown rollout as off.
pub fn file_size_limit(state: &State, channel_id: u64) -> u64 {
    let rollout = state.experiment_gate(NITRO_FILE_UPLOAD, None) == Gate::Enabled;
    let user = match state.self_state.premium_type {
        Some(1 | 3) => MIB_50,
        Some(2) if rollout => GIB_1,
        Some(2) => MB_500,
        _ => MIB_10,
    };
    let guild = guild_of(state, channel_id);
    let feature = |f: &str| state.guild_feature(guild, f) == Gate::Enabled;
    let boost = if feature("MAX_FILE_SIZE_100_MB") {
        MIB_100
    } else if feature("MAX_FILE_SIZE_50_MB") {
        MIB_50
    } else {
        MIB_10
    };
    MIB_20.max(boost.max(user))
}

/// 500 MB, or 1 GiB under the rollout (OA A17).
pub fn total_size_limit(state: &State) -> u64 {
    if state.experiment_gate(NITRO_FILE_UPLOAD, None) == Gate::Enabled {
        GIB_1
    } else {
        MB_500
    }
}

/// The official checks over `sizes` (each file's bytes): more than 10 files, an empty file, then
/// at send, or at add for an account without premium, a file over the per-file limit and the
/// total over the total limit.
pub fn check_limits(
    state: &State,
    channel_id: u64,
    sizes: &[u64],
    at: LimitCheck,
) -> Result<(), LimitError> {
    if sizes.len() > MAX_FILES {
        return Err(LimitError::TooManyFiles { limit: MAX_FILES });
    }
    if sizes.contains(&0) {
        return Err(LimitError::EmptyFile);
    }
    let premium = matches!(state.self_state.premium_type, Some(p) if p != 0);
    if at == LimitCheck::Add && premium {
        return Ok(());
    }
    let limit = file_size_limit(state, channel_id);
    if sizes.iter().any(|s| *s > limit) {
        return Err(LimitError::FileTooLarge { limit });
    }
    let limit = total_size_limit(state);
    if sizes.iter().fold(0u64, |t, s| t.saturating_add(*s)) > limit {
        return Err(LimitError::TotalTooLarge { limit });
    }
    Ok(())
}

// ---- HEIC / JXR ----

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConversionKind {
    Heic,
    Jxr,
}

/// The account's conversion config (D16a, OA A45). `Unknown` sends the original.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConversionConfig {
    Unknown,
    Disabled,
    Enabled {
        quality: u8,
        max_file_size_bytes: u64,
    },
}

pub fn conversion_config(state: &State, kind: ConversionKind) -> ConversionConfig {
    let name = match kind {
        ConversionKind::Heic => HEIC_CONVERSION,
        ConversionKind::Jxr => JXR_CONVERSION,
    };
    let Some(variation) = state.legacy_user_variation(name) else {
        return ConversionConfig::Unknown;
    };
    let enabled = |quality, max_file_size_bytes| ConversionConfig::Enabled {
        quality,
        max_file_size_bytes,
    };
    match (kind, variation) {
        (ConversionKind::Heic, Some(1)) => enabled(60, MIB_20),
        (ConversionKind::Heic, Some(2)) => enabled(80, MIB_20),
        (ConversionKind::Jxr, Some(1)) => enabled(85, MIB_50),
        _ => ConversionConfig::Disabled,
    }
}

// ---- URL and path checks ----

/// D13a: `raw` must be `https://UPLOAD_HOST/<path>?<query>` exactly as written (so no uppercase,
/// trailing dot, percent-encoded or full-width host, explicit port or userinfo slips through the
/// parser's normalization), with no fragment and nothing the parser would rewrite (dot segments).
/// `Err` carries the host for the notice, when one parses.
pub(crate) fn upload_url(raw: &str) -> Result<Url, Option<String>> {
    let parsed = Url::parse(raw);
    let host = || parsed.as_ref().ok()?.host_str().map(str::to_owned);
    let Ok(url) = &parsed else {
        return Err(None);
    };
    let ok = raw.starts_with(UPLOAD_PREFIX)
        && url.scheme() == "https"
        && url.host_str() == Some(UPLOAD_HOST)
        && url.port().is_none()
        && url.username().is_empty()
        && url.password().is_none()
        && url.fragment().is_none()
        && url.query().is_some()
        && url.as_str() == raw;
    if ok {
        Ok(url.clone())
    } else {
        Err(host())
    }
}

/// The WHATWG path percent-encode set (C0 controls, space, `"`, `#`, `<`, `>`, `?`, `` ` ``, `{`,
/// `}`, DEL and every non-ASCII byte).
fn path_encode(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    for b in raw.bytes() {
        if !(0x20..0x7f).contains(&b) || b" \"#<>?`{}".contains(&b) {
            out.push_str(&format!("%{b:02X}"));
        } else {
            out.push(b as char);
        }
    }
    out
}

/// CU-8: the API path of `DELETE /attachments/{upload_filename}`, or `None` (no request) when the
/// value holds `?`, `#`, `\`, a control character, `%` or a dot segment, or when the parsed path is
/// not exactly the prefix plus the encoded value.
pub(crate) fn delete_path(upload_filename: &str) -> Option<String> {
    let refused = upload_filename.is_empty()
        || upload_filename
            .chars()
            .any(|c| matches!(c, '?' | '#' | '\\' | '%') || c.is_control())
        || upload_filename
            .split('/')
            .any(|segment| segment == "." || segment == "..");
    if refused {
        return None;
    }
    let encoded = path_encode(upload_filename);
    let url = Url::parse(&format!("https://discord.com/api/v9/attachments/{encoded}")).ok()?;
    (url.path() == format!("/api/v9/attachments/{encoded}"))
        .then(|| format!("/attachments/{encoded}"))
}

/// CU-3: where to resume after a 308. `None` (the file fails) unless every `Range` value says
/// `bytes=0-N` with N a u64 and `N + 1 <= size`; no `Range` means start over at 0.
fn resume_at(response: &HttpResponse, size: u64) -> Option<u64> {
    let mut ranges = response
        .headers
        .iter()
        .filter(|(n, _)| n.eq_ignore_ascii_case("range"));
    let Some((_, value)) = ranges.next() else {
        return Some(0);
    };
    if ranges.next().is_some() {
        return None;
    }
    let n = value.trim().strip_prefix("bytes=0-")?;
    if n.is_empty() || n.len() > 20 || !n.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let next = n.parse::<u64>().ok()?.checked_add(1)?;
    (next <= size).then_some(next)
}

// ---- Headers (D15a) ----

/// One leg of the upload, for its closed header list.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Leg<'a> {
    /// The OPTIONS before a PUT; `range` when that PUT carries `Content-Range`.
    Preflight { range: bool },
    /// A PUT with `length` body bytes, `Content-Range` when resuming.
    Put { range: Option<&'a str>, length: u64 },
    /// The empty PUT asking where to resume.
    Probe,
}

impl Leg<'_> {
    /// `Access-Control-Request-Headers` of the preflight for this PUT: lowercase, sorted, joined
    /// without spaces.
    fn request_headers(range: bool) -> &'static str {
        if range {
            "content-range,content-type"
        } else {
            "content-type"
        }
    }
}

/// The one place upload headers are built (CU-2): Safari 27's lists in Safari's order (OA
/// A40-A42) with A43's values; `Host`, `Accept-Encoding` and `Connection` are the transport's.
pub(crate) fn headers(leg: Leg<'_>, identity: &Identity) -> Vec<(String, String)> {
    let ua = identity.user_agent();
    let language = identity.accept_language();
    let list: Vec<(&str, String)> = match leg {
        Leg::Preflight { range } => vec![
            ("Origin", ORIGIN.into()),
            ("Sec-Fetch-Site", "cross-site".into()),
            ("Access-Control-Request-Method", "PUT".into()),
            (
                "Access-Control-Request-Headers",
                Leg::request_headers(range).into(),
            ),
            ("Sec-Fetch-Mode", "cors".into()),
            ("User-Agent", ua.into()),
            ("Referer", REFERER.into()),
            ("Sec-Fetch-Dest", "empty".into()),
            ("Content-Length", "0".into()),
            ("Accept", "*/*".into()),
            ("Accept-Language", language.into()),
            ("Priority", "u=3, i".into()),
        ],
        Leg::Put { range, length } => {
            let mut list = vec![
                ("Accept", "*/*".into()),
                ("Content-Type", "application/octet-stream".into()),
                ("Sec-Fetch-Site", "cross-site".into()),
                ("Origin", ORIGIN.into()),
            ];
            if let Some(range) = range {
                list.push(("Content-Range", range.into()));
            }
            list.extend([
                ("Sec-Fetch-Mode", "cors".into()),
                ("User-Agent", ua.into()),
                ("Referer", REFERER.into()),
                ("Sec-Fetch-Dest", "empty".into()),
                ("Content-Length", length.to_string()),
                ("Accept-Language", language.into()),
                ("Priority", "u=3, i".into()),
            ]);
            list
        }
        Leg::Probe => vec![
            ("Accept", "*/*".into()),
            ("Content-Type", "application/octet-stream".into()),
            ("Origin", ORIGIN.into()),
            ("Content-Range", "bytes */*".into()),
            ("Sec-Fetch-Site", "cross-site".into()),
            ("Sec-Fetch-Mode", "cors".into()),
            ("User-Agent", ua.into()),
            ("Referer", REFERER.into()),
            ("Sec-Fetch-Dest", "empty".into()),
            ("Content-Length", "0".into()),
            ("Accept-Language", language.into()),
            ("Priority", "u=3, i".into()),
        ],
    };
    list.into_iter().map(|(n, v)| (n.to_owned(), v)).collect()
}

// ---- Preflight cache (D15a) ----

/// Successful preflights by (URL, request-header names), until their max-age passes. PUT is the
/// only method preflighted, so it is not part of the key.
#[derive(Default)]
pub(crate) struct Preflights(Mutex<HashMap<(String, &'static str), Instant>>);

impl Preflights {
    fn fresh(&self, url: &str, names: &'static str) -> bool {
        let map = self.0.lock().unwrap_or_else(|e| e.into_inner());
        map.get(&(url.to_owned(), names))
            .is_some_and(|until| Instant::now() < *until)
    }

    fn store(&self, url: &str, names: &'static str, max_age: Duration) {
        let now = Instant::now();
        let mut map = self.0.lock().unwrap_or_else(|e| e.into_inner());
        map.retain(|_, until| now < *until);
        map.insert((url.to_owned(), names), now + max_age);
    }
}

/// Every value of `name`, joined as a browser combines repeated headers.
fn combined(response: &HttpResponse, name: &str) -> Option<String> {
    let values: Vec<&str> = response
        .headers
        .iter()
        .filter(|(n, _)| n.eq_ignore_ascii_case(name))
        .map(|(_, v)| v.as_str())
        .collect();
    (!values.is_empty()).then(|| values.join(", "))
}

/// Whether a preflight answer allows the PUT as a browser would, and for how long it may be
/// reused (WebKit: an unreadable or absent max-age is 5 s, and 600 s at most).
fn preflight_allows(response: &HttpResponse, names: &str) -> Option<Duration> {
    if !(200..300).contains(&response.status) {
        return None;
    }
    let origin = combined(response, "access-control-allow-origin")?;
    if origin.trim() != ORIGIN && origin.trim() != "*" {
        return None;
    }
    let list = |name| {
        combined(response, name)
            .map(|v| {
                v.split(',')
                    .map(|t| t.trim().to_owned())
                    .filter(|t| !t.is_empty())
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default()
    };
    let methods = list("access-control-allow-methods");
    if !methods.iter().any(|m| m == "PUT" || m == "*") {
        return None;
    }
    let allowed = list("access-control-allow-headers");
    let wildcard = allowed.iter().any(|h| h == "*");
    if !wildcard
        && !names
            .split(',')
            .all(|n| allowed.iter().any(|h| h.eq_ignore_ascii_case(n)))
    {
        return None;
    }
    let age = combined(response, "access-control-max-age")
        .and_then(|v| v.trim().parse::<u64>().ok())
        .map_or(PREFLIGHT_DEFAULT, Duration::from_secs);
    Some(age.min(PREFLIGHT_MAX))
}

// ---- One file ----

/// Closes the app's file handle when the upload ends or is dropped (cancel).
struct OpenFile<'a> {
    transport: &'a dyn Transport,
    id: u64,
}

impl Drop for OpenFile<'_> {
    fn drop(&mut self) {
        self.transport.close_file(self.id);
    }
}

/// What one file's upload needs.
pub(crate) struct Uploader<'a> {
    pub(crate) transport: &'a dyn Transport,
    pub(crate) identity: &'a Identity,
    pub(crate) preflights: &'a Preflights,
}

enum Step {
    Done,
    /// Retry the leg (or probe) after a backoff.
    Again,
}

/// Waits after a failed attempt: 1 s, 2 s, 4 s … at most 32 s.
fn backoff(attempt: u32) -> Duration {
    Duration::from_secs(1 << attempt.saturating_sub(1).min(5))
}

impl Uploader<'_> {
    /// One exchange under core's own deadline; the body is re-checked against the cap.
    async fn exchange(
        &self,
        request: UploadRequest,
        progress: crate::transport::UploadProgress,
    ) -> Result<HttpResponse, TransportError> {
        let deadline = request.deadline();
        match tokio::time::timeout(deadline, self.transport.upload(request, progress)).await {
            Err(_) => Err(TransportError::Timeout),
            Ok(Err(e)) => Err(e),
            Ok(Ok(mut response)) => {
                if response
                    .body
                    .as_ref()
                    .is_some_and(|b| b.len() > RESPONSE_CAP)
                {
                    response.body = None;
                }
                Ok(response)
            }
        }
    }

    fn request(
        &self,
        method: UploadMethod,
        url: &Url,
        leg: Leg<'_>,
        body: Option<UploadBody>,
        deadline: Duration,
    ) -> Result<UploadRequest, UploadFailure> {
        UploadRequest::new(
            method,
            url.as_str().to_owned(),
            headers(leg, self.identity),
            body,
            deadline,
            RESPONSE_CAP,
        )
        .map_err(|_| UploadFailure::Failed)
    }

    /// The preflight for a PUT (`range`: it carries Content-Range), unless a fresh one is cached.
    /// `Ok(false)`: a transport failure, to be retried like the PUT it guards.
    async fn preflight(&self, url: &Url, range: bool) -> Result<bool, UploadFailure> {
        let names = Leg::request_headers(range);
        if self.preflights.fresh(url.as_str(), names) {
            return Ok(true);
        }
        let request = self.request(
            UploadMethod::Options,
            url,
            Leg::Preflight { range },
            None,
            PROBE_DEADLINE,
        )?;
        match self.exchange(request, Box::new(|_| {})).await {
            Ok(response) => {
                let age =
                    preflight_allows(&response, names).ok_or(UploadFailure::PreflightRefused)?;
                self.preflights.store(url.as_str(), names, age);
                Ok(true)
            }
            Err(TransportError::Rejected) => Err(UploadFailure::Failed),
            Err(_) => Ok(false),
        }
    }

    /// Uploads `file` to `url` (D15a, CU-3), reporting the bytes the host has (clamped to the
    /// size) to `progress`.
    pub(crate) async fn put(
        &self,
        url: &Url,
        file: &FileIdentity,
        progress: Arc<dyn Fn(u64) + Send + Sync>,
    ) -> Result<(), UploadFailure> {
        let id = self
            .transport
            .open_file(file)
            .ok_or(UploadFailure::FileChanged)?;
        let _open = OpenFile {
            transport: self.transport,
            id,
        };
        let size = file.size;
        // `None`: ask the host where to resume first.
        let mut start = Some(0u64);
        let mut attempts = 0u32;
        let mut wait = false;
        loop {
            if wait {
                tokio::time::sleep(backoff(attempts)).await;
            }
            attempts += 1;
            if attempts > MAX_ATTEMPTS {
                return Err(UploadFailure::UploadFailed);
            }
            match start {
                None => match self.probe(url, size).await? {
                    // A probe that found the resume point goes straight on to the PUT.
                    Ok(Some(at)) => {
                        start = Some(at);
                        wait = false;
                    }
                    Ok(None) => {
                        progress(size);
                        return Ok(());
                    }
                    Err(()) => wait = true,
                },
                Some(at) => match self.send(url, id, at, size, progress.clone()).await? {
                    Step::Done => return Ok(()),
                    Step::Again => {
                        start = None;
                        wait = true;
                    }
                },
            }
        }
    }

    /// The probe: `Ok(Ok(Some(start)))` to resume there, `Ok(Ok(None))` when the host already has
    /// everything, `Ok(Err(()))` to probe again after a backoff.
    async fn probe(&self, url: &Url, size: u64) -> Result<Result<Option<u64>, ()>, UploadFailure> {
        if !self.preflight(url, true).await? {
            return Ok(Err(()));
        }
        let request = self.request(UploadMethod::Put, url, Leg::Probe, None, PROBE_DEADLINE)?;
        match self.exchange(request, Box::new(|_| {})).await {
            Ok(r) if (200..300).contains(&r.status) => Ok(Ok(None)),
            Ok(r) if r.status == 308 => resume_at(&r, size)
                .map(|at| Ok(Some(at)))
                .ok_or(UploadFailure::UploadFailed),
            Ok(r) if retryable(r.status) => Ok(Err(())),
            Ok(_) => Err(UploadFailure::UploadFailed),
            Err(TransportError::Rejected | TransportError::Cancelled) => Err(UploadFailure::Failed),
            Err(_) => Ok(Err(())),
        }
    }

    /// The PUT of bytes `start..size`.
    async fn send(
        &self,
        url: &Url,
        id: u64,
        start: u64,
        size: u64,
        progress: Arc<dyn Fn(u64) + Send + Sync>,
    ) -> Result<Step, UploadFailure> {
        let range = (start > 0).then(|| {
            if start == size {
                format!("bytes */{size}")
            } else {
                format!("bytes {start}-{}/{size}", size - 1)
            }
        });
        if !self.preflight(url, range.is_some()).await? {
            return Ok(Step::Again);
        }
        let length = size - start;
        let request = self.request(
            UploadMethod::Put,
            url,
            Leg::Put {
                range: range.as_deref(),
                length,
            },
            (length > 0).then_some(UploadBody {
                file: id,
                start,
                length,
            }),
            PUT_DEADLINE,
        )?;
        let report = progress.clone();
        let on_sent = Box::new(move |sent: u64| report(start.saturating_add(sent).min(size)));
        match self.exchange(request, on_sent).await {
            Ok(r) if (200..300).contains(&r.status) => {
                progress(size);
                Ok(Step::Done)
            }
            Ok(r) if r.status == 308 || retryable(r.status) => Ok(Step::Again),
            Ok(_) => Err(UploadFailure::UploadFailed),
            Err(TransportError::Rejected | TransportError::Cancelled) => Err(UploadFailure::Failed),
            Err(_) => Ok(Step::Again),
        }
    }
}

/// Statuses after which the upload host is asked where to resume (GCS's retryable set).
fn retryable(status: u16) -> bool {
    status >= 500 || status == 408 || status == 429
}

// ---- The send ----

/// Everything a send needs to run, and to run again on retry.
#[derive(Clone, Debug)]
pub(crate) struct SendJob {
    pub(crate) channel_id: u64,
    /// The channel's guild (`None` in a DM), for a reply's `message_reference`.
    pub(crate) guild_id: Option<u64>,
    pub(crate) content: String,
    pub(crate) reply_to: Option<u64>,
    /// Shift-drop's send (OA A35).
    pub(crate) instant: bool,
    pub(crate) files: Vec<AttachmentDescriptor>,
}

/// Slot, host check and upload of file `index`; its `upload_filename` once uploaded.
async fn upload_one(
    store: &Arc<MessageStore>,
    uploader: &Uploader<'_>,
    (nonce, generation, index): (u64, u64, usize),
    job: &SendJob,
) -> Result<String, UploadFailure> {
    let file = &job.files[index];
    let rest = &store.rest;
    let slot = rest
        .attachment_slot(
            job.channel_id,
            file.slot_filename(),
            file.file.size,
            next_slot_id(),
            &file.content_type,
            file.md5(),
        )
        .await
        .map_err(UploadFailure::of)?;
    let launch = rest.identity().launch();
    let url = match upload_url(&slot.upload_url) {
        Ok(url) => {
            launch.diagnose_once("upload-host", || format!("upload host {UPLOAD_HOST}"));
            url
        }
        Err(host) => {
            // The host alone, once per launch (CU Upload host, CU-7).
            launch.diagnose_once("upload-host-refused", || {
                format!(
                    "upload host refused: {}",
                    host.as_deref().unwrap_or("(none)")
                )
            });
            return Err(UploadFailure::HostRefused);
        }
    };
    let progress: Arc<dyn Fn(u64) + Send + Sync> = {
        let store = store.clone();
        Arc::new(move |bytes| store.upload_progress(nonce, generation, index, bytes))
    };
    uploader.put(&url, &file.file, progress).await?;
    store.upload_completed(nonce, generation, index, &slot.upload_filename);
    Ok(slot.upload_filename)
}

/// One run of send `nonce` (attempt `generation`): every file in order, then the message create
/// unless the send was cancelled first (CU-9). Its outcome lands in the store's row.
pub(crate) async fn run(store: Arc<MessageStore>, nonce: u64, generation: u64, job: SendJob) {
    let rest = store.rest.clone();
    let uploader = Uploader {
        transport: rest.transport().as_ref(),
        identity: rest.identity(),
        preflights: &store.preflights,
    };
    let mut names = Vec::with_capacity(job.files.len());
    for index in 0..job.files.len() {
        match upload_one(&store, &uploader, (nonce, generation, index), &job).await {
            Ok(name) => names.push(name),
            Err(failure) => return store.upload_failed(nonce, generation, failure),
        }
    }
    if !store.dispatch_create(nonce, generation) {
        return;
    }
    let entries = job
        .files
        .iter()
        .zip(&names)
        .enumerate()
        .map(|(i, (f, name))| AttachmentEntry {
            id: i.to_string(),
            description: f.description.as_deref(),
            filename: &f.filename,
            uploaded_filename: name,
            is_spoiler: f.spoiler.then_some(true),
            original_content_type: (!f.content_type.is_empty()).then_some(f.content_type.as_str()),
        })
        .collect();
    let reply = job.reply_to.map(|message_id| Reply {
        guild_id: job.guild_id,
        message_id,
        mention: true,
    });
    let result = rest
        .send_with_attachments(
            job.channel_id,
            &job.content,
            &nonce.to_string(),
            reply,
            entries,
            job.instant,
        )
        .await;
    store.finish_upload(
        nonce,
        generation,
        job.channel_id,
        result.map_err(UploadFailure::of),
    );
}

/// Why a send with attachments was not started; nothing was sent or queued.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SendRefusal {
    NoFiles,
    Limits(LimitError),
    /// A guild channel where ATTACH_FILES is denied or unknown (CU-10).
    NotAllowed,
    /// Before READY.
    NotReady,
    /// `channel_id` is not the open channel.
    NotOpen,
}

/// The checks, then the pending row and its task. Needs a runtime context.
pub(crate) fn start_send(
    state: &State,
    store: &Arc<MessageStore>,
    job: SendJob,
) -> Result<u64, SendRefusal> {
    if job.files.is_empty() {
        return Err(SendRefusal::NoFiles);
    }
    let me = state
        .user_id
        .and_then(|id| state.users.get(&id))
        .cloned()
        .ok_or(SendRefusal::NotReady)?;
    let allowed = match channel_permissions(state, job.channel_id) {
        ChannelPermissions::Private => true,
        p => p.can(ATTACH_FILES) == Some(true),
    };
    if !allowed {
        return Err(SendRefusal::NotAllowed);
    }
    let sizes: Vec<u64> = job.files.iter().map(|f| f.file.size).collect();
    check_limits(state, job.channel_id, &sizes, LimitCheck::Send).map_err(SendRefusal::Limits)?;
    store.begin_upload(job, me).ok_or(SendRefusal::NotOpen)
}

impl SessionHandle {
    /// The attachment limits over the files' sizes (see `check_limits`); the UI asks on add, and
    /// `send_with_attachments` asks again.
    pub fn attachment_limits(
        &self,
        channel_id: u64,
        sizes: &[u64],
        at: LimitCheck,
    ) -> Result<(), LimitError> {
        check_limits(&self.snapshot(), channel_id, sizes, at)
    }

    /// Sends `files` with `content` (possibly empty) to the open channel `channel_id`: the
    /// permission and limits are checked, then the send's echo row appears with its files and
    /// progress, and the uploads and message create run (composer plan CU Flow). Returns the
    /// nonce the row and the cancel, retry and discard calls go by. `instant` is Shift-drop's
    /// send. Must be called inside a runtime context.
    pub fn send_with_attachments(
        &self,
        channel_id: u64,
        content: String,
        reply_to: Option<u64>,
        files: Vec<AttachmentDescriptor>,
        instant: bool,
    ) -> Result<u64, SendRefusal> {
        let state = self.snapshot();
        let job = SendJob {
            channel_id,
            guild_id: guild_of(&state, channel_id),
            content,
            reply_to,
            instant,
            files,
        };
        start_send(&state, &self.messages(), job)
    }

    /// Cancels a pending send whose message create has not been dispatched (D14a, CU-9): the
    /// running upload stops, its completed uploads are deleted, and the row turns cancelled,
    /// keeping its text, reply target and files for the composer. False when there is nothing
    /// to cancel any more. Must be called inside a runtime context.
    pub fn cancel_upload(&self, nonce: u64) -> bool {
        self.messages().cancel_upload(nonce)
    }

    /// Runs a failed send again from the slot requests. Must be called inside a runtime context.
    pub fn retry_upload(&self, nonce: u64) -> bool {
        self.messages().retry_upload(nonce)
    }

    /// Removes a failed or cancelled send's row.
    pub fn discard_upload(&self, nonce: u64) -> bool {
        self.messages().discard_upload(nonce)
    }

    /// The account's HEIC or JXR conversion config (D16a).
    pub fn upload_conversion(&self, kind: ConversionKind) -> ConversionConfig {
        conversion_config(&self.snapshot(), kind)
    }
}

#[cfg(test)]
#[path = "uploads_tests.rs"]
mod tests;
