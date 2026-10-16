//! Everything that identifies this client to Discord, in one file (menu plan ID, D12-D14, D16,
//! D17; I§1-§2 of refs/discord-ground-truth/identify.md): the per-launch values (`Launch`), the
//! per-session `Identity` snapshot REST and the gateway share, and the gateway frames that carry
//! it (IDENTIFY in its fast-connect and normal shapes, RESUME, op 40 heartbeats, op 41, op 3).
//!
//! Claimed to match the official web client only where identify.md shows it; unverified and
//! listed as such (T-13): `launch_signature` null rather than a string (O6), the key order after
//! `client_launch_id` (O7), the two launch ids (O9), when `gateway_connect_reasons` drops
//! `COLD_START`, the trigger of op 41, Safari's Accept-Language for a tag without a region.
use crate::identity_fetch::{
    self, newest_supported, Fetched, BUILTIN_BUILD, BUILTIN_PAIR, SUPPORTED_PAIRS,
};
use crate::identity_store::{FetchCache, IdentityStore};
use crate::local_presence::LocalPresence;
use crate::proto::{base64_encode, Field, Message, Value as PValue};
use crate::token::{self, Token};
use crate::transport::Transport;
use serde::Serialize;
use std::path::PathBuf;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex, OnceLock,
};
use tokio::sync::watch;
use zeroize::Zeroizing;

/// Initial gateway URL: v9 because the READY shapes we parse are v9's, zlib-stream because the
/// browser client uses the stateful transport mode. Crate-private so nothing on the FFI surface
/// can change it.
pub(crate) const GATEWAY_URL: &str =
    "wss://gateway.discord.gg/?v=9&encoding=json&compress=zlib-stream";
/// Query an accepted `resume_gateway_url` is normalized to (same as `GATEWAY_URL`'s).
pub(crate) const GATEWAY_QUERY: &str = "v=9&encoding=json&compress=zlib-stream";

/// The fallback user agent: used when this Mac's Safari version is unknown or unreadable. The
/// login webview, REST and the gateway all use `Launch::user_agent`, so they agree.
pub const USER_AGENT: &str = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/26.0 Safari/605.1.15";
const BROWSER_VERSION: &str = "26.0";
/// Safari freezes the macOS version in its UA at 10.15.7.
const OS_VERSION: &str = "10.15.7";
/// Safari's Accept-Language for an en-US system; the fallback when no system language is usable.
const FALLBACK_LOCALE: &str = "en-US";
const FALLBACK_ACCEPT_LANGUAGE: &str = "en-US,en;q=0.9";
/// The `ver` literal of the official QoS heartbeat (W:2681437); a drift note lives in identify.md.
const QOS_VER: u32 = 31;
/// A cache entry younger than this skips the identity fetch (D17).
const FETCH_SKIP_MS: u64 = 24 * 60 * 60 * 1000;

/// Discord's interface locales. `X-Discord-Locale` is always one of these.
const DISCORD_LOCALES: [&str; 32] = [
    "id", "da", "de", "en-GB", "en-US", "es-ES", "es-419", "fr", "hr", "it", "lt", "hu", "nl",
    "no", "pl", "pt-BR", "ro", "fi", "sv-SE", "vi", "tr", "cs", "el", "bg", "ru", "uk", "hi", "th",
    "zh-CN", "ja", "zh-TW", "ko",
];

pub(crate) fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_millis() as u64)
}

/// A random UUID v4 (lowercase), from the system's random device.
pub(crate) fn uuid_v4() -> String {
    use std::io::Read;
    let mut b = [0u8; 16];
    if std::fs::File::open("/dev/urandom")
        .and_then(|mut f| f.read_exact(&mut b))
        .is_err()
    {
        // ponytail: SipHash keys come from the OS once per process; only reached when
        // /dev/urandom is unreadable, which macOS never is.
        use std::hash::{BuildHasher, Hasher};
        for chunk in b.chunks_mut(8) {
            let mut h = std::collections::hash_map::RandomState::new().build_hasher();
            h.write_u64(now_ms());
            chunk.copy_from_slice(&h.finish().to_le_bytes());
        }
    }
    b[6] = (b[6] & 0x0f) | 0x40;
    b[8] = (b[8] & 0x3f) | 0x80;
    let h: String = b.iter().map(|x| format!("{x:02x}")).collect();
    format!(
        "{}-{}-{}-{}-{}",
        &h[0..8],
        &h[8..12],
        &h[12..16],
        &h[16..20],
        &h[20..32]
    )
}

/// `^\d{2}\.\d{1,2}(\.\d{1,2})?$` (I-18): anything else read from disk is not used.
fn safari_version_ok(v: &str) -> bool {
    let parts: Vec<&str> = v.split('.').collect();
    let digits = |p: &str, len: std::ops::RangeInclusive<usize>| {
        len.contains(&p.len()) && p.bytes().all(|b| b.is_ascii_digit())
    };
    matches!(parts.len(), 2 | 3)
        && digits(parts[0], 2..=2)
        && parts[1..].iter().all(|p| digits(p, 1..=2))
}

/// One BCP 47 tag split into language, script and region (`zh-Hant-TW`, `en_US`, `fr`).
struct Tag {
    lang: String,
    script: Option<String>,
    region: Option<String>,
}

fn parse_tag(tag: &str) -> Option<Tag> {
    if tag.is_empty() || tag.len() > 35 {
        return None;
    }
    let mut parts = tag.split(['-', '_']);
    let lang = parts.next()?;
    if !(2..=3).contains(&lang.len()) || !lang.bytes().all(|b| b.is_ascii_alphabetic()) {
        return None;
    }
    let mut t = Tag {
        lang: lang.to_ascii_lowercase(),
        script: None,
        region: None,
    };
    for p in parts {
        if !p.bytes().all(|b| b.is_ascii_alphanumeric()) {
            return None;
        }
        if p.len() == 4 && t.script.is_none() && t.region.is_none() {
            t.script = Some(format!(
                "{}{}",
                p[..1].to_ascii_uppercase(),
                p[1..].to_ascii_lowercase()
            ));
        } else if t.region.is_none()
            && ((p.len() == 2 && p.bytes().all(|b| b.is_ascii_alphabetic()))
                || (p.len() == 3 && p.bytes().all(|b| b.is_ascii_digit())))
        {
            t.region = Some(p.to_ascii_uppercase());
        }
    }
    Some(t)
}

/// Safari's `navigator.languages[0]` and Accept-Language from the system's first usable
/// preferred language: `lang-REGION` first, then `lang-Script` (or `lang`) at q=0.9, which gives
/// the captured `zh-TW,zh-Hant;q=0.9` for `zh-Hant-TW` and `en-US,en;q=0.9` for `en-US`. A tag
/// without a region sends the one tag (unverified against Safari).
pub(crate) fn safari_languages(preferred: &[String]) -> (String, String) {
    let Some(t) = preferred.iter().find_map(|p| parse_tag(p)) else {
        return (FALLBACK_LOCALE.into(), FALLBACK_ACCEPT_LANGUAGE.into());
    };
    let primary = match &t.region {
        Some(r) => format!("{}-{r}", t.lang),
        None => t.lang.clone(),
    };
    let secondary = match &t.script {
        Some(s) => format!("{}-{s}", t.lang),
        None => t.lang.clone(),
    };
    let accept = if secondary == primary {
        primary.clone()
    } else {
        format!("{primary},{secondary};q=0.9")
    };
    (primary, accept)
}

/// The system's first usable language as one of Discord's locales (unverified mapping: the
/// official client's own browser-language mapping is not recorded in identify.md). Chinese by
/// script or region, English and Spanish by region, Portuguese as pt-BR, Swedish as sv-SE,
/// Norwegian as no; any other language Discord lacks is en-US.
pub(crate) fn discord_locale_for(preferred: &[String]) -> String {
    let Some(t) = preferred.iter().find_map(|p| parse_tag(p)) else {
        return FALLBACK_LOCALE.into();
    };
    let region = t.region.as_deref();
    let mapped = match t.lang.as_str() {
        "zh" if t.script.as_deref() == Some("Hant")
            || matches!(region, Some("TW" | "HK" | "MO")) =>
        {
            "zh-TW".to_owned()
        }
        "zh" => "zh-CN".into(),
        "en" if region == Some("GB") => "en-GB".into(),
        "en" => "en-US".into(),
        "es" if region == Some("ES") => "es-ES".into(),
        "es" => "es-419".into(),
        "pt" => "pt-BR".into(),
        "sv" => "sv-SE".into(),
        "nb" | "nn" | "no" => "no".into(),
        other => other.to_owned(),
    };
    if DISCORD_LOCALES.contains(&mapped.as_str()) {
        mapped
    } else {
        FALLBACK_LOCALE.into()
    }
}

/// An account's Discord locale (PreloadedUserSettings 12.1) when it is one Discord has.
pub(crate) fn account_locale(value: &str) -> Option<String> {
    DISCORD_LOCALES.contains(&value).then(|| value.to_owned())
}

/// What the app passes once at launch.
#[derive(Clone, Debug, Default)]
pub struct LaunchConfig {
    /// `CFBundleShortVersionString` of /Applications/Safari.app, as read; validated here.
    pub safari_version: Option<String>,
    /// `Locale.preferredLanguages`, in order.
    pub preferred_languages: Vec<String>,
    /// velella's Application Support directory; `None` keeps the store in memory only.
    pub store_dir: Option<PathBuf>,
    /// Off in tests that must not fetch; on in the app.
    pub fetch: bool,
}

/// The identity values settled by the fetch (or the cache, or the built-ins) for every session of
/// this launch that waited for them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Resolved {
    pub build: u64,
    pub pair: (u64, u64),
    pub fetch_failed: bool,
    pub unsupported: bool,
}

/// Per-launch identity: one per process in the app (velella-ffi keeps it), a fresh one per test.
pub struct Launch {
    user_agent: String,
    browser_version: String,
    system_locale: String,
    accept_language: String,
    discord_locale: String,
    /// `client_launch_id` of REST and the normal IDENTIFY (W's module 71931).
    launch_id: String,
    /// The fast-connect script computes its own (FC:458, O9).
    fast_launch_id: String,
    pub(crate) store: IdentityStore,
    fetch_enabled: bool,
    fetch: Mutex<Option<watch::Receiver<Option<Resolved>>>>,
    /// The first IDENTIFY of the launch is the fast-connect one.
    fast_connect_pending: AtomicBool,
    /// `gateway_connect_reasons` is `COLD_START` until the launch's first READY.
    cold_start: AtomicBool,
    notified_fetch: AtomicBool,
    notified_unsupported: AtomicBool,
    /// The account of the launch's last READY (an account switch clears the heartbeat session).
    pub(crate) last_account: Mutex<Option<u64>>,
    /// Live-check lines already written this launch (each once).
    diagnosed: Mutex<std::collections::HashSet<String>>,
    #[cfg(test)]
    pub(crate) diagnostics: Mutex<Vec<String>>,
}

impl Launch {
    pub fn new(config: LaunchConfig) -> Arc<Launch> {
        let version = config
            .safari_version
            .filter(|v| safari_version_ok(v))
            .unwrap_or_else(|| BROWSER_VERSION.to_owned());
        let (system_locale, accept_language) = safari_languages(&config.preferred_languages);
        Arc::new(Launch {
            user_agent: format!(
                "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/{version} Safari/605.1.15"
            ),
            browser_version: version,
            system_locale,
            accept_language,
            discord_locale: discord_locale_for(&config.preferred_languages),
            launch_id: uuid_v4(),
            fast_launch_id: uuid_v4(),
            store: match &config.store_dir {
                Some(dir) => IdentityStore::open(dir, now_ms()),
                None => IdentityStore::memory(),
            },
            fetch_enabled: config.fetch,
            fetch: Mutex::new(None),
            fast_connect_pending: AtomicBool::new(true),
            cold_start: AtomicBool::new(true),
            notified_fetch: AtomicBool::new(false),
            notified_unsupported: AtomicBool::new(false),
            last_account: Mutex::new(None),
            diagnosed: Mutex::default(),
            #[cfg(test)]
            diagnostics: Mutex::default(),
        })
    }

    /// No fetch, no store, no languages: the built-in values (tests, `Session::connect`).
    pub fn offline() -> Arc<Launch> {
        Launch::new(LaunchConfig::default())
    }

    pub fn user_agent(&self) -> &str {
        &self.user_agent
    }

    pub(crate) fn launch_id(&self) -> &str {
        &self.launch_id
    }

    /// Sign-out: the account's heartbeat session is cleared (I§2: cleared on token change).
    pub fn forget_heartbeat_session(&self, user_id: u64) {
        self.store.set_heartbeat_session(user_id, None);
    }

    /// True exactly once per launch: this IDENTIFY is the fast-connect one.
    pub(crate) fn take_fast_connect(&self) -> bool {
        self.fast_connect_pending.swap(false, Ordering::SeqCst)
    }

    pub(crate) fn connect_reasons(&self) -> &'static str {
        if self.cold_start.load(Ordering::SeqCst) {
            "COLD_START"
        } else {
            "NO_REASONS"
        }
    }

    /// Writes `velella: <line>` to stderr the first time this launch sees `key` (menu plan ID,
    /// Live row). The caller builds the line from names only, never values; the shipped build
    /// installs no logger, so this is the one place these names reach the terminal.
    pub(crate) fn diagnose_once(&self, key: &str, line: impl FnOnce() -> String) {
        if !self
            .diagnosed
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .insert(key.to_owned())
        {
            return;
        }
        let line = line();
        eprintln!("velella: {line}");
        #[cfg(test)]
        self.diagnostics
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .push(line);
    }

    pub(crate) fn ready_seen(&self) {
        self.cold_start.store(false, Ordering::SeqCst);
    }

    /// True the first time per launch for each kind: the notice goes out once.
    pub(crate) fn notice_once(&self, unsupported: bool) -> bool {
        let flag = if unsupported {
            &self.notified_unsupported
        } else {
            &self.notified_fetch
        };
        !flag.swap(true, Ordering::SeqCst)
    }

    /// The launch's resolved values: the first caller starts the fetch on `transport`; every
    /// caller waits for the same result, which is bounded by `FETCH_DEADLINE` (a cache younger
    /// than 24 hours or a launch without fetching resolves at once).
    pub(crate) async fn resolve(self: &Arc<Self>, transport: Arc<dyn Transport>) -> Resolved {
        let now = now_ms();
        let cached = self.store.fetch_cache(now);
        let builtin = |c: Option<&FetchCache>| match c {
            Some(c) => (c.build, c.pair),
            None => (BUILTIN_BUILD, BUILTIN_PAIR),
        };
        if !self.fetch_enabled
            || cached
                .as_ref()
                .is_some_and(|c| now.saturating_sub(c.fetched_at_ms) < FETCH_SKIP_MS)
        {
            let (build, pair) = builtin(cached.as_ref());
            return Resolved {
                build,
                pair,
                fetch_failed: false,
                unsupported: false,
            };
        }
        let mut rx = {
            let mut slot = self.fetch.lock().unwrap_or_else(|e| e.into_inner());
            match &*slot {
                Some(rx) => rx.clone(),
                None => {
                    let (tx, rx) = watch::channel(None);
                    *slot = Some(rx.clone());
                    let launch = self.clone();
                    let cached = cached.clone();
                    tokio::spawn(async move {
                        let (ua, al) = (launch.user_agent.clone(), launch.accept_language.clone());
                        let out =
                            identity_fetch::fetch(&*transport, &ua, &al, cached.as_ref(), now)
                                .await;
                        let resolved = match out {
                            Fetched::Accepted {
                                build,
                                pair,
                                unsupported,
                                cache,
                            } => {
                                if let Some(cache) = cache {
                                    launch.store.set_fetch_cache(cache);
                                }
                                Resolved {
                                    build,
                                    pair,
                                    fetch_failed: false,
                                    unsupported,
                                }
                            }
                            Fetched::Failed => {
                                tracing::warn!("identity fetch failed; cached or built-in values");
                                let (build, pair) = builtin(cached.as_ref());
                                Resolved {
                                    build,
                                    pair,
                                    fetch_failed: true,
                                    unsupported: false,
                                }
                            }
                        };
                        tx.send_replace(Some(resolved));
                    });
                    rx
                }
            }
        };
        let fallback = || {
            let (build, pair) = builtin(cached.as_ref());
            Resolved {
                build,
                pair,
                fetch_failed: true,
                unsupported: false,
            }
        };
        let out = rx.wait_for(Option::is_some).await.map(|v| *v);
        out.ok().flatten().unwrap_or_else(fallback)
    }
}

/// The base super-properties, keys #1-#16 of I§2 in their order (the closing brace and the later
/// keys are appended as text, so serde never reorders them).
#[derive(Serialize)]
struct Properties<'a> {
    os: &'static str,
    browser: &'static str,
    device: &'static str,
    system_locale: &'a str,
    has_client_mods: bool,
    browser_user_agent: &'a str,
    browser_version: &'a str,
    os_version: &'static str,
    referrer: &'static str,
    referring_domain: &'static str,
    referrer_current: &'static str,
    referring_domain_current: &'static str,
    release_channel: &'static str,
    client_build_number: u64,
    client_event_source: Option<()>,
    client_launch_id: &'a str,
}

/// The JSON of `p` without its closing brace, for appending more keys in a fixed order.
fn open_object(p: &Properties) -> String {
    let mut json = serde_json::to_string(p).expect("properties serialize");
    json.pop();
    json
}

fn push_key(json: &mut String, key: &str, value: &impl Serialize) {
    json.push_str(&format!(
        ",\"{key}\":{}",
        serde_json::to_string(value).expect("value serializes")
    ));
}

/// The immutable part of a session's identity, fixed once per session (I-6, T-7).
struct Base {
    capabilities: u64,
    /// `{…#1-#16, "launch_signature":null` (no closing brace).
    props: String,
    /// The fast-connect IDENTIFY's: its own launch id, no `launch_signature`.
    fast_props: String,
}

/// `client_app_state` in X-Super-Properties and the normal IDENTIFY.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum AppState {
    Focused,
    Unfocused,
}

impl AppState {
    fn as_str(self) -> &'static str {
        match self {
            Self::Focused => "focused",
            Self::Unfocused => "unfocused",
        }
    }
}

/// The only per-session values that change after start, read by REST at send time. No other
/// key can be added at runtime.
#[derive(Default)]
struct Extras {
    client_app_state: Option<AppState>,
    client_heartbeat_session_id: Option<String>,
    /// Sent as `X-Installation-ID` once the account's value is known.
    installation_id: Option<String>,
    /// The account's Discord locale (settings 12.1), once known.
    account_locale: Option<String>,
}

/// An extra string that may reach a header: 1-128 visible ASCII bytes.
fn extra_ok(value: &str) -> bool {
    (1..=128).contains(&value.len()) && value.bytes().all(|b| b.is_ascii_graphic())
}

/// The identity one session presents: REST takes its headers from it and the gateway task its
/// IDENTIFY, so the two cannot disagree within a session. The base is fixed once per session —
/// by the session task with the launch's resolved values before its first IDENTIFY, or with the
/// built-in values by whatever reads it first — and never changes afterwards; only the closed set
/// of `Extras` changes during a session.
pub(crate) struct Identity {
    launch: Arc<Launch>,
    base: OnceLock<Base>,
    timezone: String,
    extras: Mutex<Extras>,
}

impl Identity {
    /// A session's identity, its base still open.
    pub(crate) fn new(launch: Arc<Launch>) -> Identity {
        Identity {
            launch,
            base: OnceLock::new(),
            timezone: timezone(),
            extras: Mutex::default(),
        }
    }

    /// The built-in identity of an offline launch shared by the whole process (REST tests).
    #[cfg(test)]
    pub(crate) fn current() -> Identity {
        static OFFLINE: OnceLock<Arc<Launch>> = OnceLock::new();
        Identity::new(OFFLINE.get_or_init(Launch::offline).clone())
    }

    /// Fixes the base with `build` and `capabilities`; false when it was already fixed.
    pub(crate) fn fix(&self, build: u64, capabilities: u64) -> bool {
        let mut fixed = false;
        self.base.get_or_init(|| {
            fixed = true;
            self.make_base(build, capabilities)
        });
        fixed
    }

    fn make_base(&self, build: u64, capabilities: u64) -> Base {
        let l = &*self.launch;
        fn properties<'a>(l: &'a Launch, launch_id: &'a str, build: u64) -> Properties<'a> {
            Properties {
                os: "Mac OS X",
                browser: "Safari",
                device: "",
                system_locale: &l.system_locale,
                has_client_mods: false,
                browser_user_agent: &l.user_agent,
                browser_version: &l.browser_version,
                os_version: OS_VERSION,
                referrer: "",
                referring_domain: "",
                referrer_current: "",
                referring_domain_current: "",
                release_channel: "stable",
                client_build_number: build,
                client_event_source: None,
                client_launch_id: launch_id,
            }
        }
        let mut full = open_object(&properties(l, &l.launch_id, build));
        push_key(&mut full, "launch_signature", &None::<()>);
        Base {
            capabilities,
            props: full,
            fast_props: open_object(&properties(l, &l.fast_launch_id, build)),
        }
    }

    fn base(&self) -> &Base {
        self.base
            .get_or_init(|| self.make_base(BUILTIN_BUILD, normal_capabilities(BUILTIN_PAIR)))
    }

    pub(crate) fn launch(&self) -> &Arc<Launch> {
        &self.launch
    }

    fn extras(&self) -> std::sync::MutexGuard<'_, Extras> {
        self.extras.lock().unwrap_or_else(|e| e.into_inner())
    }

    pub(crate) fn set_client_app_state(&self, state: AppState) {
        self.extras().client_app_state = Some(state);
    }

    /// False (and nothing stored) unless 1-128 visible ASCII bytes.
    pub(crate) fn set_client_heartbeat_session_id(&self, id: String) -> bool {
        let ok = extra_ok(&id);
        if ok {
            self.extras().client_heartbeat_session_id = Some(id);
        }
        ok
    }

    /// An expired or cleared heartbeat session: the key leaves X-Super-Properties.
    pub(crate) fn clear_client_heartbeat_session_id(&self) {
        self.extras().client_heartbeat_session_id = None;
    }

    /// False (and nothing stored) unless 1-128 visible ASCII bytes.
    pub(crate) fn set_installation_id(&self, id: String) -> bool {
        let ok = extra_ok(&id);
        if ok {
            self.extras().installation_id = Some(id);
        }
        ok
    }

    pub(crate) fn installation_id(&self) -> Option<String> {
        self.extras().installation_id.clone()
    }

    /// The account's Discord locale for `X-Discord-Locale`; ignored unless Discord has it.
    pub(crate) fn set_account_locale(&self, locale: Option<&str>) {
        self.extras().account_locale = locale.and_then(account_locale);
    }

    pub(crate) fn user_agent(&self) -> &str {
        &self.launch.user_agent
    }

    /// The extras in X-Super-Properties order: `client_app_state`, then
    /// `client_heartbeat_session_id`, each once set (O7: the official order is unverified).
    fn push_extras(&self, json: &mut String, extras: &Extras) {
        if let Some(state) = extras.client_app_state {
            push_key(json, "client_app_state", &state.as_str());
        }
        if let Some(id) = &extras.client_heartbeat_session_id {
            push_key(json, "client_heartbeat_session_id", id);
        }
    }

    /// `X-Super-Properties`: standard padded base64 of the base properties (with
    /// `launch_signature`), then the extras once set.
    pub(crate) fn super_properties(&self) -> String {
        let mut json = self.base().props.clone();
        self.push_extras(&mut json, &self.extras());
        json.push('}');
        base64_encode(json.as_bytes())
    }

    /// The account's locale once known, else the system's first language mapped to Discord's.
    /// Owned because the account's value can change; a `Box<str>` so REST's `.into()` into its
    /// header `String` stays a real conversion.
    pub(crate) fn locale(&self) -> Box<str> {
        self.extras()
            .account_locale
            .clone()
            .unwrap_or_else(|| self.launch.discord_locale.clone())
            .into()
    }

    pub(crate) fn accept_language(&self) -> &str {
        &self.launch.accept_language
    }

    pub(crate) fn timezone(&self) -> &str {
        &self.timezone
    }
}

/// The normal capabilities value, which every IDENTIFY sends unless the obfuscation experiment is
/// Enabled (D14, `obfuscated_capabilities`).
pub(crate) fn normal_capabilities(pair: (u64, u64)) -> u64 {
    if SUPPORTED_PAIRS.contains(&pair) {
        pair.1
    } else {
        newest_supported().1
    }
}

/// The obfuscation variant: what a normal IDENTIFY sends while slice X reports
/// `2026-02-private-channel-hiding` Enabled for the account (D14); Disabled and Unknown send
/// `normal_capabilities`. Never used by the fast-connect IDENTIFY.
pub(crate) fn obfuscated_capabilities(pair: (u64, u64)) -> u64 {
    if SUPPORTED_PAIRS.contains(&pair) {
        pair.0
    } else {
        newest_supported().0
    }
}

/// `X-Discord-Timezone`: the system's IANA zone, which a browser reports from the same setting.
/// macOS keeps it as the `/etc/localtime` symlink into `.../zoneinfo/<Zone>`; anything unreadable
/// or outside the IANA name alphabet falls back to "UTC".
pub(crate) fn timezone() -> String {
    std::fs::read_link("/etc/localtime")
        .ok()
        .and_then(|p| Some(p.to_str()?.split_once("zoneinfo/")?.1.to_owned()))
        .filter(|z| {
            !z.is_empty()
                && z.bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b"/_+-".contains(&b))
        })
        .unwrap_or_else(|| "UTC".to_owned())
}

/// Which IDENTIFY to build.
pub(crate) enum Shape<'a> {
    /// The launch's first: `{token, capabilities, properties:{…base without launch_signature,
    /// is_fast_connect:true, installation_id?}, client_state}` (FC:29375).
    Fast { installation_id: Option<&'a str> },
    /// Every later one (W:4789647).
    Normal {
        installation_id: Option<&'a str>,
        reasons: &'a str,
        presence: Option<&'a LocalPresence>,
        qos_token: Option<&'a str>,
    },
}

/// The IDENTIFY frame, byte for byte. Held in a zeroizing buffer and never formatted: the token
/// is in it.
pub(crate) fn identify(token: &Token, identity: &Identity, shape: Shape) -> Zeroizing<String> {
    identify_with(token, identity, shape, None)
}

/// `identify`, with the normal shape's `capabilities` replaced by `capabilities` when given (D14).
/// The fast-connect shape always sends the session's fixed value.
pub(crate) fn identify_with(
    token: &Token,
    identity: &Identity,
    shape: Shape,
    capabilities: Option<u64>,
) -> Zeroizing<String> {
    let base = identity.base();
    let mut props = match shape {
        Shape::Fast { .. } => base.fast_props.clone(),
        Shape::Normal { .. } => base.props.clone(),
    };
    let installation = match &shape {
        Shape::Fast { installation_id } => {
            push_key(&mut props, "is_fast_connect", &true);
            *installation_id
        }
        Shape::Normal {
            installation_id,
            reasons,
            ..
        } => {
            let extras = identity.extras();
            identity.push_extras(&mut props, &extras);
            if extras.client_app_state.is_none() {
                push_key(&mut props, "client_app_state", &AppState::Focused.as_str());
            }
            drop(extras);
            push_key(&mut props, "is_fast_connect", &false);
            push_key(&mut props, "gateway_connect_reasons", reasons);
            *installation_id
        }
    };
    if let Some(id) = installation.filter(|i| !i.is_empty()) {
        push_key(&mut props, "installation_id", &id);
    }
    props.push('}');
    let mut frame = Zeroizing::new(String::with_capacity(props.len() + 512));
    frame.push_str("{\"op\":2,\"d\":{\"token\":");
    frame.push_str(&serde_json::to_string(token.expose()).expect("token serializes"));
    let capabilities = match shape {
        Shape::Normal { .. } => capabilities.unwrap_or(base.capabilities),
        Shape::Fast { .. } => base.capabilities,
    };
    push_key(&mut frame, "capabilities", &capabilities);
    frame.push_str(",\"properties\":");
    frame.push_str(&props);
    if let Shape::Normal {
        presence,
        qos_token,
        ..
    } = shape
    {
        if let Some(p) = presence {
            push_key(&mut frame, "presence", p);
        }
        push_key(&mut frame, "compress", &false);
        frame.push_str(",\"client_state\":{\"guild_versions\":{}}");
        if let Some(q) = qos_token {
            push_key(&mut frame, "qos_token", &q);
        }
    } else {
        frame.push_str(",\"client_state\":{\"guild_versions\":{}}");
    }
    frame.push_str("}}");
    frame
}

/// `qos_token`: standard base64 of `QosToken {1 client_provided {1 is_active}, 2 derived}` with
/// the account's stored `DerivedQosData` bytes (absent on a first login).
pub(crate) fn qos_token(active: bool, derived: Option<&[u8]>) -> String {
    let mut provided = Message::new();
    if active {
        provided.push_bool(1, true);
    }
    let mut token = Message::new();
    token.push_message(1, &provided);
    if let Some(d) = derived {
        token.fields.push(Field {
            number: 2,
            value: PValue::Len(d.to_vec()),
        });
    }
    base64_encode(&token.encode())
}

/// Op 40 `QOS_HEARTBEAT {seq, qos}`: `qos` null until the first activity report, then
/// `{active, ver:31, reasons}` (W:2681437, W:4791135). Op 1 is never sent.
pub(crate) fn heartbeat(seq: Option<u64>, focused: Option<bool>) -> String {
    #[derive(Serialize)]
    struct Qos {
        active: bool,
        ver: u32,
        reasons: Vec<&'static str>,
    }
    #[derive(Serialize)]
    struct Beat {
        seq: Option<u64>,
        qos: Option<Qos>,
    }
    let qos = focused.map(|f| {
        let reasons: Vec<&'static str> = if f { vec!["foregrounded"] } else { vec![] };
        Qos {
            active: !reasons.is_empty(),
            ver: QOS_VER,
            reasons,
        }
    });
    serde_json::to_string(&Payload {
        op: 40,
        d: Beat { seq, qos },
    })
    .expect("heartbeat serializes")
}

/// Op 41 `UPDATE_TIME_SPENT_SESSION_ID` (W:4777466).
pub(crate) fn time_spent_session(created_at_ms: u64, session_id: &str, launch_id: &str) -> String {
    #[derive(Serialize)]
    struct D<'a> {
        initialization_timestamp: u64,
        session_id: &'a str,
        client_launch_id: &'a str,
    }
    serde_json::to_string(&Payload {
        op: 41,
        d: D {
            initialization_timestamp: created_at_ms,
            session_id,
            client_launch_id: launch_id,
        },
    })
    .expect("op 41 serializes")
}

/// Op 3 with the local presence `{status, since, activities, afk}`.
pub(crate) fn presence_update(presence: &LocalPresence) -> String {
    serde_json::to_string(&Payload { op: 3, d: presence }).expect("presence serializes")
}

#[derive(Serialize, Debug)]
pub(crate) struct ResumeData<'a> {
    #[serde(serialize_with = "token::serialize")]
    token: &'a Token,
    session_id: &'a str,
    seq: u64,
}

#[derive(Serialize, Debug)]
pub(crate) struct Payload<D> {
    op: u8,
    d: D,
}

pub(crate) fn resume<'a>(
    token: &'a Token,
    session_id: &'a str,
    seq: u64,
) -> Payload<ResumeData<'a>> {
    Payload {
        op: 6,
        d: ResumeData {
            token,
            session_id,
            seq,
        },
    }
}

/// Serialized payload held in a zeroizing buffer; the token is in it, so it never gets logged.
pub(crate) fn encode(payload: &impl Serialize) -> Zeroizing<String> {
    Zeroizing::new(serde_json::to_string(payload).expect("payload serializes"))
}

/// `X-Super-Properties` of the process-wide offline identity (tests).
#[cfg(test)]
pub(crate) fn super_properties() -> String {
    Identity::current().super_properties()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::proto::base64_decode;
    use crate::transport::memory::{Http, MemoryTransport};

    const MARKER: &str = "SYNTHETIC_TOKEN_MARKER_0123456789";

    fn launch(languages: &[&str], safari: Option<&str>) -> Arc<Launch> {
        Launch::new(LaunchConfig {
            safari_version: safari.map(str::to_owned),
            preferred_languages: languages.iter().map(|s| s.to_string()).collect(),
            store_dir: None,
            fetch: false,
        })
    }

    fn keys(json: &str) -> Vec<String> {
        // Top-level keys of a flat JSON object, in wire order.
        let v: serde_json::Value = serde_json::from_str(json).unwrap();
        let mut keys: Vec<(usize, String)> = v
            .as_object()
            .unwrap()
            .keys()
            .map(|k| (json.find(&format!("\"{k}\":")).unwrap(), k.clone()))
            .collect();
        keys.sort();
        keys.into_iter().map(|(_, k)| k).collect()
    }

    const BASE_KEYS: [&str; 17] = [
        "os",
        "browser",
        "device",
        "system_locale",
        "has_client_mods",
        "browser_user_agent",
        "browser_version",
        "os_version",
        "referrer",
        "referring_domain",
        "referrer_current",
        "referring_domain_current",
        "release_channel",
        "client_build_number",
        "client_event_source",
        "client_launch_id",
        "launch_signature",
    ];

    #[test]
    fn super_properties_follow_the_official_key_order_and_this_mac() {
        let l = launch(&["zh-Hant-TW", "en-TW"], Some("27.0.1"));
        let identity = Identity::new(l.clone());
        assert!(identity.fix(640_102, 1_734_653));
        assert!(!identity.fix(1, 2), "the base is fixed once");
        let json = String::from_utf8(base64_decode(&identity.super_properties()).unwrap()).unwrap();
        assert_eq!(keys(&json), BASE_KEYS);
        let v: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(v["system_locale"], "zh-TW");
        assert_eq!(v["has_client_mods"], false);
        assert_eq!(v["browser_version"], "27.0.1");
        assert_eq!(
            v["browser_user_agent"],
            "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/27.0.1 Safari/605.1.15"
        );
        assert_eq!(v["client_build_number"], 640_102);
        assert_eq!(v["client_event_source"], serde_json::Value::Null);
        assert_eq!(v["launch_signature"], serde_json::Value::Null);
        assert_eq!(v["client_launch_id"], l.launch_id());
        assert_eq!(identity.user_agent(), l.user_agent());
        assert_eq!(identity.accept_language(), "zh-TW,zh-Hant;q=0.9");
        assert_eq!(&*identity.locale(), "zh-TW");
        identity.set_account_locale(Some("ja"));
        assert_eq!(&*identity.locale(), "ja");
        identity.set_account_locale(Some("klingon"));
        assert_eq!(&*identity.locale(), "zh-TW");
        // Extras are appended after the base keys.
        identity.set_client_app_state(AppState::Unfocused);
        assert!(identity.set_client_heartbeat_session_id("hb".into()));
        let json = String::from_utf8(base64_decode(&identity.super_properties()).unwrap()).unwrap();
        let mut with = BASE_KEYS.to_vec();
        with.extend(["client_app_state", "client_heartbeat_session_id"]);
        assert_eq!(keys(&json), with);
        identity.clear_client_heartbeat_session_id();
        assert!(
            !String::from_utf8(base64_decode(&identity.super_properties()).unwrap())
                .unwrap()
                .contains("client_heartbeat_session_id")
        );
    }

    #[test]
    fn an_unusable_safari_version_keeps_the_constants() {
        for bad in ["27", "100.0", "27.0.1.2", "27.x", "", "27.0.123", " 27.0"] {
            let l = launch(&[], Some(bad));
            assert_eq!(l.user_agent(), USER_AGENT, "{bad:?}");
        }
        assert!(launch(&[], Some("18.6"))
            .user_agent()
            .contains("Version/18.6 "));
    }

    #[test]
    fn languages_are_formatted_as_safari_sends_them() {
        let al =
            |l: &[&str]| safari_languages(&l.iter().map(|s| s.to_string()).collect::<Vec<_>>());
        assert_eq!(
            al(&["zh-Hant-TW"]),
            ("zh-TW".into(), "zh-TW,zh-Hant;q=0.9".into())
        );
        assert_eq!(al(&["en-US"]), ("en-US".into(), "en-US,en;q=0.9".into()));
        assert_eq!(al(&["ja_JP"]), ("ja-JP".into(), "ja-JP,ja;q=0.9".into()));
        assert_eq!(al(&["fr"]), ("fr".into(), "fr".into()));
        assert_eq!(al(&["es-419"]), ("es-419".into(), "es-419,es;q=0.9".into()));
        assert_eq!(al(&["bad\r\nX", "de-DE"]).0, "de-DE");
        assert_eq!(al(&[]), ("en-US".into(), "en-US,en;q=0.9".into()));
        let dl = |l: &str| discord_locale_for(&[l.to_owned()]);
        assert_eq!(dl("zh-Hant-TW"), "zh-TW");
        assert_eq!(dl("zh-Hans-CN"), "zh-CN");
        assert_eq!(dl("zh-HK"), "zh-TW");
        assert_eq!(dl("en-GB"), "en-GB");
        assert_eq!(dl("en-AU"), "en-US");
        assert_eq!(dl("es-MX"), "es-419");
        assert_eq!(dl("pt-PT"), "pt-BR");
        assert_eq!(dl("nb-NO"), "no");
        assert_eq!(dl("ja-JP"), "ja");
        assert_eq!(dl("ca-ES"), "en-US");
        for v in [al(&["zh-Hant-TW"]).1, dl("zh-Hant-TW")] {
            assert!(crate::transport::valid_value(&v));
        }
    }

    #[test]
    fn fast_and_normal_identify_are_byte_for_byte_the_official_shapes() {
        let l = launch(&["zh-Hant-TW"], Some("27.0.1"));
        let identity = Identity::new(l.clone());
        identity.fix(640_102, 1_734_653);
        let token = Token::new(MARKER.into()).unwrap();
        let base16 = |launch_id: &str| {
            format!(
                r#""os":"Mac OS X","browser":"Safari","device":"","system_locale":"zh-TW","has_client_mods":false,"browser_user_agent":"{ua}","browser_version":"27.0.1","os_version":"10.15.7","referrer":"","referring_domain":"","referrer_current":"","referring_domain_current":"","release_channel":"stable","client_build_number":640102,"client_event_source":null,"client_launch_id":"{launch_id}""#,
                ua = l.user_agent()
            )
        };
        let fast = identify(
            &token,
            &identity,
            Shape::Fast {
                installation_id: Some("inst.1"),
            },
        );
        assert_eq!(
            *fast,
            format!(
                r#"{{"op":2,"d":{{"token":"{MARKER}","capabilities":1734653,"properties":{{{},"is_fast_connect":true,"installation_id":"inst.1"}},"client_state":{{"guild_versions":{{}}}}}}}}"#,
                base16(&l.fast_launch_id)
            )
        );
        assert!(!fast.contains("launch_signature"));
        assert_ne!(l.fast_launch_id, l.launch_id);
        let fast = identify(
            &token,
            &identity,
            Shape::Fast {
                installation_id: None,
            },
        );
        assert!(!fast.contains("installation_id"));

        let presence = LocalPresence {
            status: crate::local_presence::SelfStatus::Online,
            since: 0,
            activities: vec![],
            afk: false,
        };
        let normal = identify(
            &token,
            &identity,
            Shape::Normal {
                installation_id: Some("inst.1"),
                reasons: "COLD_START",
                presence: Some(&presence),
                qos_token: Some("CgA="),
            },
        );
        assert_eq!(
            *normal,
            format!(
                r#"{{"op":2,"d":{{"token":"{MARKER}","capabilities":1734653,"properties":{{{},"launch_signature":null,"client_app_state":"focused","is_fast_connect":false,"gateway_connect_reasons":"COLD_START","installation_id":"inst.1"}},"presence":{{"status":"online","since":0,"activities":[],"afk":false}},"compress":false,"client_state":{{"guild_versions":{{}}}},"qos_token":"CgA="}}}}"#,
                base16(&l.launch_id)
            )
        );
        // Presence and qos omitted when unknown; app state and the heartbeat session as set.
        identity.set_client_app_state(AppState::Unfocused);
        identity.set_client_heartbeat_session_id("0b5f7c4e-1d2a-4c3b-8a9f-0123456789ab".into());
        let normal = identify(
            &token,
            &identity,
            Shape::Normal {
                installation_id: None,
                reasons: "NO_REASONS",
                presence: None,
                qos_token: None,
            },
        );
        assert_eq!(
            *normal,
            format!(
                r#"{{"op":2,"d":{{"token":"{MARKER}","capabilities":1734653,"properties":{{{},"launch_signature":null,"client_app_state":"unfocused","client_heartbeat_session_id":"0b5f7c4e-1d2a-4c3b-8a9f-0123456789ab","is_fast_connect":false,"gateway_connect_reasons":"NO_REASONS"}},"compress":false,"client_state":{{"guild_versions":{{}}}}}}}}"#,
                base16(&l.launch_id)
            )
        );
    }

    /// D14: the override replaces only the normal shape's capabilities; fast-connect keeps the
    /// session's value, and the obfuscation variant is the pair's first member.
    #[test]
    fn a_capabilities_override_reaches_the_normal_identify_only() {
        let identity = Identity::new(launch(&[], None));
        identity.fix(640_102, 1_734_653);
        let token = Token::new(MARKER.into()).unwrap();
        let normal = |capabilities| {
            let shape = Shape::Normal {
                installation_id: None,
                reasons: "NO_REASONS",
                presence: None,
                qos_token: None,
            };
            identify_with(&token, &identity, shape, capabilities)
        };
        assert!(normal(None).contains(r#""capabilities":1734653,"#));
        assert!(normal(Some(1_767_421)).contains(r#""capabilities":1767421,"#));
        let fast = identify_with(
            &token,
            &identity,
            Shape::Fast {
                installation_id: None,
            },
            Some(1_767_421),
        );
        assert!(fast.contains(r#""capabilities":1734653,"#));
        assert_eq!(obfuscated_capabilities(BUILTIN_PAIR), 1_767_421);
        assert_eq!(obfuscated_capabilities((1, 2)), 1_767_421);
        assert_eq!(normal_capabilities((1, 2)), 1_734_653);
    }

    #[test]
    fn resume_carries_the_token_but_debug_redacts_it() {
        let token = Token::new(MARKER.into()).unwrap();
        let resume = resume(&token, "sess", 7);
        for debug in [format!("{resume:?}"), format!("{resume:#?}")] {
            assert!(!debug.contains("SYNTHETIC"), "{debug}");
        }
        let wire: serde_json::Value = serde_json::from_str(&encode(&resume)).unwrap();
        assert_eq!(
            wire,
            serde_json::json!({"op": 6, "d": {"token": MARKER, "session_id": "sess", "seq": 7}})
        );
    }

    #[test]
    fn qos_token_round_trips_through_the_codec() {
        let derived = base64_decode(&crate::identity_store::tests::qos_b64()).unwrap();
        for (active, derived) in [(true, Some(derived.as_slice())), (false, None)] {
            let b64 = qos_token(active, derived);
            let m = Message::decode(&base64_decode(&b64).unwrap(), 1 << 12).unwrap();
            let provided = m.message(1).unwrap().unwrap();
            assert_eq!(provided.bool(1).unwrap(), active.then_some(true));
            match derived {
                Some(d) => {
                    assert_eq!(m.get(2).unwrap().value, PValue::Len(d.to_vec()));
                    let inner = m.message(2).unwrap().unwrap();
                    assert_eq!(inner.string(1).unwrap(), Some("claims"));
                    assert_eq!(inner.varint(3).unwrap(), Some(7));
                }
                None => assert!(m.get(2).is_none()),
            }
        }
        assert_eq!(qos_token(false, None), "CgA=");
    }

    #[test]
    fn heartbeat_op41_and_op3_frames() {
        assert_eq!(
            heartbeat(None, None),
            r#"{"op":40,"d":{"seq":null,"qos":null}}"#
        );
        assert_eq!(
            heartbeat(Some(5), Some(true)),
            r#"{"op":40,"d":{"seq":5,"qos":{"active":true,"ver":31,"reasons":["foregrounded"]}}}"#
        );
        assert_eq!(
            heartbeat(Some(5), Some(false)),
            r#"{"op":40,"d":{"seq":5,"qos":{"active":false,"ver":31,"reasons":[]}}}"#
        );
        assert_eq!(
            time_spent_session(9, "u", "l"),
            r#"{"op":41,"d":{"initialization_timestamp":9,"session_id":"u","client_launch_id":"l"}}"#
        );
        let p = LocalPresence {
            status: crate::local_presence::SelfStatus::Dnd,
            since: 0,
            activities: vec![],
            afk: true,
        };
        assert_eq!(
            presence_update(&p),
            r#"{"op":3,"d":{"status":"dnd","since":0,"activities":[],"afk":true}}"#
        );
    }

    #[test]
    fn uuids_are_v4() {
        let u = uuid_v4();
        assert_eq!(u.len(), 36);
        assert_eq!(&u[14..15], "4");
        assert!(matches!(&u[19..20], "8" | "9" | "a" | "b"));
        assert_ne!(u, uuid_v4());
    }

    /// A launch resolves once: a younger-than-24-hours cache skips the network entirely.
    #[tokio::test]
    async fn a_fresh_cache_skips_the_fetch() {
        let l = launch(&[], None);
        let l = Arc::new(Launch {
            fetch_enabled: true,
            ..Arc::try_unwrap(l).ok().unwrap()
        });
        l.store.set_fetch_cache(FetchCache {
            chunk_hash: "f0cfe6eb7552ae67".into(),
            build: 650_000,
            pair: BUILTIN_PAIR,
            fetched_at_ms: now_ms() - 1_000,
        });
        let http = Arc::new(Http::new(vec![]));
        let transport = Arc::new(MemoryTransport {
            http: Some(http.clone()),
            endpoints: None,
        });
        let r = l.resolve(transport).await;
        assert_eq!((r.build, r.fetch_failed), (650_000, false));
        assert!(http.requests.lock().unwrap().is_empty());
    }
}
