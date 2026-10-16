//! Public session API and the gateway task behind it. The connection state machine (HELLO,
//! IDENTIFY/RESUME, heartbeat with ACK timeout, op 7/9, close-code classification, backoff) is
//! modeled on Serein's gateway loop (refs/serein/crates/discord-gateway/src/lib.rs, MIT OR
//! Apache-2.0) but written for velella's much smaller surface.
//!
//! The socket is the session's `Transport` WebSocket (plan NET); every protection above it (URL
//! check, inflation caps, heartbeats, resume, close codes) stays here, and the task closes each
//! connection explicitly on every exit path (T-4).
//!
//! Logging policy: tracing records op codes and event names only, never payloads or the token.
use crate::{
    compression::{DecodeError, Decoder, MAX_GATEWAY_WIRE},
    gateway::{backoff, close_action, jitter_ms, member_frames, validated_resume_url, CloseAction},
    identify::{self, normal_capabilities, now_ms, AppState, Identity, Launch, Shape, GATEWAY_URL},
    identity_store::{HeartbeatSession, HEARTBEAT_SESSION_TTL_MS},
    local_presence::{local_presence, EmojiLookup, LocalPresence},
    messages::MessageStore,
    rest::{Rest, TokenSlot},
    state::{MemberSub, State},
    token::Token,
    transport::{Frame, Received, Transport, WebSocket, WebSocketRequest},
};
use serde::Deserialize;
use serde_json::Value;
use std::{
    collections::VecDeque,
    fmt,
    future::Future,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc, Mutex, OnceLock,
    },
    time::Duration,
};
use tokio::{
    sync::{mpsc, watch, Notify},
    time::{interval_at, sleep, timeout, Instant, MissedTickBehavior},
};
use zeroize::Zeroizing;

/// Close codes core sends. 1000 ends the session for good (Discord invalidates it); any other
/// code keeps it resumable, so every reconnect closes with 4000.
const CLOSE_FINAL: u16 = 1000;
const CLOSE_RECONNECT: u16 = 4000;
const DIAL_TIMEOUT: Duration = Duration::from_secs(15);

/// Lifecycle notifications, each tagged with the connect generation that produced it.
/// `AuthFailed` (token rejected, no retry) and `Closed` (`disconnect()` or a terminal close
/// code) end the session; no further events follow either.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SessionEvent {
    Ready {
        generation: u64,
        user_id: u64,
        username: String,
    },
    /// The gateway answered a RESUME with the RESUMED dispatch; state carried over, no new READY.
    Resumed {
        generation: u64,
    },
    Reconnecting {
        generation: u64,
    },
    AuthFailed {
        generation: u64,
    },
    Closed {
        generation: u64,
    },
    /// A REST call got 401: REST is latched off for this session while the gateway carries on.
    /// At most once per session, and never after `AuthFailed` or `Closed`. Takes no Keychain
    /// action (plan Q3): a revoked token also fails IDENTIFY with 4004, which is `AuthFailed`.
    RestUnauthorized {
        generation: u64,
    },
}

/// What a session tells the app about its identity, on a callback separate from `SessionEvent`
/// (menu plan ID, Token refresh and Notices rows).
#[derive(Debug)]
pub enum IdentityEvent {
    /// READY carried a valid `auth_token` different from the current one: the session's token
    /// slot already holds it (REST and the next IDENTIFY use it). Always after the generation's
    /// `SessionEvent::Ready`. The app writes it to the saved sign-in only for this generation and
    /// this account.
    TokenRefreshed {
        generation: u64,
        user_id: u64,
        token: RefreshedToken,
    },
    Notice {
        generation: u64,
        notice: IdentityNotice,
    },
}

/// At most once per launch each.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IdentityNotice {
    /// The official client sends a capabilities value velella does not support yet (D12).
    UnsupportedCapabilities,
    /// The identity fetch failed; cached or built-in values are in use (T-14).
    IdentityFetchFailed,
}

/// A refreshed token handed to the app once: `take` gives it the first time and `None` after.
/// Never formatted (redacting `Debug`); zeroized when dropped untaken.
pub struct RefreshedToken(Mutex<Option<Zeroizing<String>>>);

impl RefreshedToken {
    fn new(token: &Token) -> RefreshedToken {
        RefreshedToken(Mutex::new(Some(Zeroizing::new(token.expose().to_owned()))))
    }

    pub fn take(&self) -> Option<String> {
        self.0
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .take()
            .map(|mut z| std::mem::take(&mut *z))
    }
}

impl fmt::Debug for RefreshedToken {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("RefreshedToken([REDACTED])")
    }
}

/// How a session is identified beyond its token.
pub struct SessionOptions {
    /// The launch's identity values (one per process in the app).
    pub launch: Arc<Launch>,
    /// The account the token was saved for (a token read from the Keychain); `None` for a token
    /// captured by the login window. Per-account values (installation id, QoS, heartbeat session)
    /// are sent before READY only for this account, and never for any other.
    pub expected_user: Option<u64>,
    pub identity_events: Option<Box<dyn Fn(IdentityEvent) + Send + Sync>>,
}

impl Default for SessionOptions {
    fn default() -> Self {
        SessionOptions {
            launch: Launch::offline(),
            expected_user: None,
            identity_events: None,
        }
    }
}

/// What the app reports about the user (menu plan ID, Presence and QoS rows): whether velella is
/// frontmost (`None` until the first report) and the last input the web IdleStore would count.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Activity {
    focused: Option<bool>,
    last_input_ms: u64,
}

/// Op 3 limit: at most 5 per rolling 20 s (W:4675107).
const PRESENCE_BURST: usize = 5;
const PRESENCE_WINDOW: Duration = Duration::from_secs(20);
/// The web IdleStore polls every 30 s; presence (idle, custom-status expiry) is re-checked then.
const PRESENCE_POLL: Duration = Duration::from_secs(30);

/// Process-wide, monotonic per connect.
static GENERATION: AtomicU64 = AtomicU64::new(0);

struct Shared {
    state: Mutex<Arc<State>>,
    revision: watch::Sender<u64>,
    /// Set once the session task has returned (Closed or AuthFailed): no further mutations.
    ended: watch::Sender<bool>,
    /// The session's one REST client (shared latch and global-429 deadline).
    rest: Rest,
    /// Created by `SessionHandle::messages()`; sees every dispatch after `State::apply`.
    messages: OnceLock<Arc<MessageStore>>,
    /// Latest member-list selection (guild, channel) for the connection loop. A watch, not a
    /// queue: the loop only ever acts on the newest value.
    member_sub: watch::Sender<Option<(u64, u64)>>,
    /// Last `MemberSub::epoch` handed out; only changed under the state lock.
    member_epoch: AtomicU64,
    activity: watch::Sender<Activity>,
    #[cfg(test)]
    applied: Mutex<Vec<(String, Option<MemberSub>)>>,
}

impl Shared {
    fn snapshot(&self) -> Arc<State> {
        self.state.lock().unwrap_or_else(|e| e.into_inner()).clone()
    }

    /// A new IDENTIFY: what belonged to the previous gateway session's handshake is dropped.
    fn identify_started(&self) {
        let mut guard = self.state.lock().unwrap_or_else(|e| e.into_inner());
        if guard.initial_guild.is_some() {
            Arc::make_mut(&mut guard).identify_started();
        }
    }

    /// Copy-on-write: readers holding a snapshot are never mutated underneath.
    // ponytail: clones the whole State when a reader holds a snapshot; fine at sidebar sizes,
    // switch to per-guild Arcs if profiling says otherwise.
    fn apply(&self, event: &str, d: &Value) -> bool {
        let user_id;
        let changed = {
            let mut guard = self.state.lock().unwrap_or_else(|e| e.into_inner());
            let changed = Arc::make_mut(&mut guard).apply(event, d);
            if changed {
                self.revision.send_replace(guard.revision);
            }
            #[cfg(test)]
            self.applied
                .lock()
                .unwrap()
                .push((event.to_owned(), guard.member_sub.clone()));
            user_id = guard.user_id;
            changed
        };
        // Outside the state lock: the store keeps its own buffers. Every dispatch goes through,
        // including the ones `State` ignores (MESSAGE_*).
        if let Some(store) = self.messages.get() {
            store.set_me(user_id);
            store.observe(event, d);
            // Typing needs names and the self id from the state `apply` just left.
            if event == "TYPING_START" {
                store.typing_start(d, &self.snapshot());
            }
        }
        changed
    }
}

pub struct SessionHandle {
    generation: u64,
    shared: Arc<Shared>,
    stop: Arc<Notify>,
}

impl SessionHandle {
    /// Stops the session; the task closes the socket and emits `Closed`. Also runs on Drop.
    pub fn disconnect(&self) {
        // notify_one stores a permit, so a stop issued before the task polls is not lost.
        self.stop.notify_one();
    }

    pub fn generation(&self) -> u64 {
        self.generation
    }

    /// Revision counter, bumped on every state mutation.
    pub fn state(&self) -> watch::Receiver<u64> {
        self.shared.revision.subscribe()
    }

    /// Immutable snapshot of the current state.
    pub fn snapshot(&self) -> Arc<State> {
        self.shared.snapshot()
    }

    /// Selects the member list to subscribe to (op 37), or `None` to leave it. The selection and a
    /// new epoch land in `State::member_sub` before this returns, so every dispatch applied
    /// afterwards sees it; only then is the connection loop woken, and it sends the frames for the
    /// newest selection once the connection is established. Setting the current value again does
    /// nothing: no epoch, no wake, no frame.
    pub fn set_member_subscription(&self, selection: Option<(u64, u64)>) {
        let shared = &self.shared;
        let mut guard = shared.state.lock().unwrap_or_else(|e| e.into_inner());
        if guard
            .member_sub
            .as_ref()
            .map(|m| (m.guild_id, m.channel_id))
            == selection
        {
            return;
        }
        let epoch = shared.member_epoch.fetch_add(1, Ordering::SeqCst) + 1;
        let state = Arc::make_mut(&mut guard);
        state.member_sub = selection.map(|(guild_id, channel_id)| MemberSub {
            epoch,
            guild_id,
            channel_id,
        });
        state.revision += 1;
        shared.revision.send_replace(state.revision);
        // Still under the lock, so two racing calls leave the watch and `State` agreeing.
        shared.member_sub.send_replace(selection);
    }

    /// Whether velella is the frontmost app (`client_app_state`, the QoS heartbeat, the heartbeat
    /// session). Focus changes also count as input for the idle rule, as WINDOW_FOCUS and
    /// APP_STATE_UPDATE do in the web IdleStore.
    pub fn set_app_focused(&self, focused: bool) {
        self.shared
            .rest
            .identity()
            .set_client_app_state(if focused {
                AppState::Focused
            } else {
                AppState::Unfocused
            });
        self.shared.activity.send_replace(Activity {
            focused: Some(focused),
            last_input_ms: now_ms(),
        });
    }

    /// The user clicked, scrolled, typed or switched channel (the web IdleStore's inputs).
    pub fn note_user_activity(&self) {
        self.shared
            .activity
            .send_modify(|a| a.last_input_ms = now_ms());
    }

    /// Whether this channel or DM is obfuscated: not navigable, no member list, no actions (I-5).
    pub fn is_obfuscated(&self, channel_id: u64) -> bool {
        self.shared.snapshot().is_obfuscated(channel_id)
    }

    /// The session's REST client: a clone sharing its 401 latch and global-429 deadline.
    #[allow(dead_code)] // Seam for wave-1 M; used by the tests until then.
    pub(crate) fn rest(&self) -> Rest {
        self.shared.rest.clone()
    }

    /// The session's message store, created (with the session's `Rest`) on first use. From then
    /// on it observes every dispatch.
    #[allow(dead_code)] // Seam for wave-1 M; used by the tests until then.
    pub(crate) fn messages(&self) -> Arc<MessageStore> {
        self.shared
            .messages
            .get_or_init(|| {
                let store = MessageStore::new(self.shared.rest.clone());
                store.set_me(self.shared.snapshot().user_id);
                store
            })
            .clone()
    }

    /// (dispatch name, `State::member_sub` as that dispatch's `State::apply` left it).
    #[cfg(test)]
    pub(crate) fn applied(&self) -> Vec<(String, Option<MemberSub>)> {
        self.shared.applied.lock().unwrap().clone()
    }

    /// Runs `compute` on the current state now and after every revision change, calling `deliver`
    /// with each result that differs from the last one delivered (the first always goes out, so
    /// an empty pre-READY state is delivered too). Stops when the returned `Subscription` is
    /// dropped or the session ends. Must be called inside a runtime context; `compute` and
    /// `deliver` run on a runtime thread and must not block.
    pub fn subscribe<T: PartialEq + Clone + Send + 'static>(
        &self,
        compute: impl Fn(&State) -> T + Send + 'static,
        deliver: impl Fn(T) + Send + 'static,
    ) -> Subscription {
        let shared = self.shared.clone();
        Subscription(tokio::spawn(async move {
            let mut revision = shared.revision.subscribe();
            let mut ended = shared.ended.subscribe();
            let mut last: Option<T> = None;
            loop {
                // Mark the revision seen before reading, so a change in between is not lost.
                revision.borrow_and_update();
                let value = compute(&shared.snapshot());
                if last.as_ref() != Some(&value) {
                    last = Some(value.clone());
                    deliver(value);
                }
                tokio::select! {
                    changed = revision.changed() => if changed.is_err() { return },
                    _ = ended.wait_for(|e| *e) => return,
                }
            }
        }))
    }
}

/// A running `SessionHandle::subscribe`; dropping it stops the delivery.
pub struct Subscription(tokio::task::JoinHandle<()>);

impl Drop for Subscription {
    fn drop(&mut self) {
        self.0.abort();
    }
}

impl Drop for SessionHandle {
    fn drop(&mut self) {
        self.disconnect();
    }
}

impl fmt::Debug for SessionHandle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SessionHandle")
            .field("generation", &self.generation)
            .finish_non_exhaustive()
    }
}

pub struct Session;

impl Session {
    /// Starts a gateway session on the current tokio runtime and returns immediately; must be called
    /// inside a runtime context. `events` runs on the session task, so it must not block. All of
    /// the session's traffic (gateway and REST) goes through `transport`.
    pub fn connect(
        token: Token,
        transport: Arc<dyn Transport>,
        events: impl Fn(SessionEvent) + Send + Sync + 'static,
    ) -> SessionHandle {
        Session::connect_with(token, transport, SessionOptions::default(), events)
    }

    /// `connect` with the launch's identity, the saved account and the identity callback.
    pub fn connect_with(
        token: Token,
        transport: Arc<dyn Transport>,
        options: SessionOptions,
        events: impl Fn(SessionEvent) + Send + Sync + 'static,
    ) -> SessionHandle {
        let generation = GENERATION.fetch_add(1, Ordering::SeqCst) + 1;
        // One token slot for REST and the gateway task.
        let token = TokenSlot::new(Arc::new(token));
        // One identity per session: REST headers and IDENTIFY read the same snapshot.
        let identity = Arc::new(Identity::new(options.launch.clone()));
        let (notice, rest_notice) = mpsc::unbounded_channel();
        let shared = Arc::new(Shared {
            state: Mutex::new(Arc::new(State::default())),
            revision: watch::channel(0).0,
            ended: watch::channel(false).0,
            rest: Rest::new(
                token.clone(),
                generation,
                notice,
                transport.clone(),
                identity.clone(),
            ),
            messages: OnceLock::new(),
            member_sub: watch::channel(None).0,
            member_epoch: AtomicU64::new(0),
            activity: watch::channel(Activity {
                focused: None,
                last_input_ms: now_ms(),
            })
            .0,
            #[cfg(test)]
            applied: Mutex::default(),
        });
        let stop = Arc::new(Notify::new());
        let task = Task {
            token,
            transport,
            identity,
            generation,
            events: Box::new(events),
            member_rx: shared.member_sub.subscribe(),
            activity_rx: shared.activity.subscribe(),
            shared: shared.clone(),
            stop: stop.clone(),
            rest_notice,
            session_token: None,
            session_id: None,
            resume_url: None,
            seq: None,
            identity_events: options.identity_events,
            account: options.expected_user,
            fresh_token: options.expected_user.is_none(),
            identity_fixed: false,
            heartbeat_session: None,
            awaiting_ack: false,
            ready_extras: None,
            presence_sent: None,
            presence_slots: VecDeque::new(),
            presence_retry: None,
        };
        let ended = shared.clone();
        tokio::spawn(async move {
            task.run().await;
            ended.ended.send_replace(true);
        });
        SessionHandle {
            generation,
            shared,
            stop,
        }
    }
}

#[derive(Deserialize)]
struct Packet {
    op: u64,
    #[serde(default)]
    s: Option<u64>,
    #[serde(default)]
    t: Option<String>,
    #[serde(default)]
    d: Value,
}

enum Gone {
    Close(u16),
    Lost,
    Capacity,
}

enum End {
    /// Reconnect (resuming if session state survived).
    Retry,
    /// Op 9: session state was cleared; Discord wants a 1-5 s pause before IDENTIFY.
    InvalidSession,
    Auth,
    Stop,
    Disconnect,
}

struct Task {
    /// The slot shared with the session's `Rest`: every IDENTIFY reads its current token.
    token: Arc<TokenSlot>,
    transport: Arc<dyn Transport>,
    identity: Arc<Identity>,
    generation: u64,
    events: Box<dyn Fn(SessionEvent) + Send + Sync>,
    shared: Arc<Shared>,
    stop: Arc<Notify>,
    /// The REST 401 notice. Dropped with the task, so a 401 after the end reaches nobody.
    rest_notice: mpsc::UnboundedReceiver<()>,
    member_rx: watch::Receiver<Option<(u64, u64)>>,
    /// The token the current gateway session identified with; a RESUME of that session sends
    /// this one, even after the slot was replaced. Zeroized when the last holder drops it.
    session_token: Option<Arc<Token>>,
    session_id: Option<String>,
    /// Already validated and normalized.
    resume_url: Option<String>,
    seq: Option<u64>,
    identity_events: Option<Box<dyn Fn(IdentityEvent) + Send + Sync>>,
    activity_rx: watch::Receiver<Activity>,
    /// The token's account: the saved one until this session's READY says.
    account: Option<u64>,
    /// The token came from the login window (a token change): the account's heartbeat session
    /// is cleared at the first READY.
    fresh_token: bool,
    /// The identity base was fixed with the launch's resolved values (first IDENTIFY only).
    identity_fixed: bool,
    heartbeat_session: Option<HeartbeatSession>,
    awaiting_ack: bool,
    /// READY's identity fields, read before `State` consumes the payload.
    ready_extras: Option<ReadyExtras>,
    /// Last op 3 of this connection (none yet: READY always sends one).
    presence_sent: Option<LocalPresence>,
    presence_slots: VecDeque<Instant>,
    /// A rate-limited op 3 waits until this slot frees (`true`: forced, RESUMED).
    presence_retry: Option<(Instant, bool)>,
}

/// READY's fields for the identity (I§1, experiments.md §5): read raw, validated where used.
struct ReadyExtras {
    auth_token: Option<String>,
    installation: Option<String>,
    qos_token: Option<String>,
}

impl ReadyExtras {
    fn of(d: &Value) -> ReadyExtras {
        let s = |v: Option<&Value>| v.and_then(Value::as_str).map(str::to_owned);
        ReadyExtras {
            auth_token: s(d.get("auth_token")),
            installation: s(d
                .get("apex_experiments")
                .and_then(|a| a.get("installation"))),
            qos_token: s(d.get("qos_token")),
        }
    }
}

const MIN_HEARTBEAT_MS: u64 = 50;
const MAX_HEARTBEAT_MS: u64 = 120_000;

impl Task {
    fn emit(&self, event: SessionEvent) {
        (self.events)(event);
    }

    fn clear_session(&mut self) {
        self.session_token = None;
        self.session_id = None;
        self.resume_url = None;
        self.seq = None;
    }

    /// The upgrade request: the URL (fixed or validated), the session's User-Agent and
    /// Accept-Language (never the platform's defaults, which URLSession would otherwise add, the
    /// latter from the system locale), and the inflation cap as the message-size limit.
    fn dial_request(&self, url: String) -> Option<WebSocketRequest> {
        let headers = vec![
            (
                "User-Agent".to_owned(),
                self.identity.user_agent().to_owned(),
            ),
            (
                "Accept-Language".to_owned(),
                self.identity.accept_language().to_owned(),
            ),
        ];
        WebSocketRequest::new(url, headers, MAX_GATEWAY_WIRE, DIAL_TIMEOUT).ok()
    }

    async fn run(mut self) {
        let mut failures = 0u32;
        loop {
            let resuming = self.session_token.is_some()
                && self.session_id.is_some()
                && self.resume_url.is_some()
                && self.seq.is_some();
            if !resuming {
                self.clear_session();
            }
            // The session's first IDENTIFY waits for the launch's identity values (at most the
            // fetch deadline); reconnects and resumes reuse the session's fixed snapshot (T-14).
            if !resuming && !self.identity_fixed {
                self.identity_fixed = true;
                let launch = self.identity.launch().clone();
                let Some(resolved) = self
                    .unless_stopped(launch.resolve(self.transport.clone()))
                    .await
                else {
                    return self.emit(SessionEvent::Closed {
                        generation: self.generation,
                    });
                };
                self.identity
                    .fix(resolved.build, normal_capabilities(resolved.pair));
                if resolved.fetch_failed && launch.notice_once(false) {
                    self.notice(IdentityNotice::IdentityFetchFailed);
                }
                if resolved.unsupported && launch.notice_once(true) {
                    self.notice(IdentityNotice::UnsupportedCapabilities);
                }
            }
            let url = self
                .resume_url
                .clone()
                .filter(|_| resuming)
                .unwrap_or_else(|| GATEWAY_URL.to_owned());
            let transport = self.transport.clone();
            let dialed = match self.dial_request(url) {
                Some(request) => {
                    let dial = timeout(DIAL_TIMEOUT, transport.websocket(request));
                    let Some(dialed) = self.unless_stopped(dial).await else {
                        return self.emit(SessionEvent::Closed {
                            generation: self.generation,
                        });
                    };
                    dialed.ok().and_then(Result::ok)
                }
                None => None,
            };
            let mut established = false;
            let end = match dialed {
                Some(socket) => {
                    let end = self.connection(&*socket, resuming, &mut established).await;
                    // Every exit closes explicitly; a reconnect keeps the session resumable.
                    socket.close(match end {
                        End::Retry | End::InvalidSession => CLOSE_RECONNECT,
                        End::Auth | End::Stop | End::Disconnect => CLOSE_FINAL,
                    });
                    end
                }
                None => End::Retry,
            };
            match end {
                End::Disconnect => {
                    return self.emit(SessionEvent::Closed {
                        generation: self.generation,
                    })
                }
                End::Stop => {
                    return self.emit(SessionEvent::Closed {
                        generation: self.generation,
                    })
                }
                End::Auth => {
                    return self.emit(SessionEvent::AuthFailed {
                        generation: self.generation,
                    })
                }
                End::Retry | End::InvalidSession => {}
            }
            failures = if established { 0 } else { failures + 1 };
            // A dead resume host must not strand an otherwise reachable account.
            if resuming && failures >= 3 {
                self.clear_session();
            }
            self.emit(SessionEvent::Reconnecting {
                generation: self.generation,
            });
            let delay = match end {
                End::InvalidSession => Duration::from_millis(1000 + jitter_ms(4000)),
                _ => backoff(failures),
            };
            if self.unless_stopped(sleep(delay)).await.is_none() {
                return self.emit(SessionEvent::Closed {
                    generation: self.generation,
                });
            }
        }
    }

    /// Drives `fut` to completion unless `disconnect()` fires first (`None`). A REST 401 notice
    /// arriving meanwhile is emitted, so it never waits for the next connection.
    async fn unless_stopped<F: Future>(&mut self, fut: F) -> Option<F::Output> {
        tokio::pin!(fut);
        loop {
            tokio::select! {
                biased;
                () = self.stop.notified() => return None,
                Some(()) = self.rest_notice.recv() => self.rest_unauthorized(),
                out = &mut fut => return Some(out),
            }
        }
    }

    /// Only reached through the one notice `Rest`'s latch sends per session.
    fn rest_unauthorized(&self) {
        self.emit(SessionEvent::RestUnauthorized {
            generation: self.generation,
        });
    }

    fn identity_event(&self, event: IdentityEvent) {
        if let Some(events) = &self.identity_events {
            events(event);
        }
    }

    fn notice(&self, notice: IdentityNotice) {
        self.identity_event(IdentityEvent::Notice {
            generation: self.generation,
            notice,
        });
    }

    /// The fast-connect IDENTIFY for the launch's first, the normal one after (I-9). Per-account
    /// values only for the token's known account.
    fn identify_frame(&self, token: &Token) -> Zeroizing<String> {
        let launch = self.identity.launch();
        let store = &launch.store;
        let installation = self.account.and_then(|u| store.installation_id(u));
        if launch.take_fast_connect() {
            return identify::identify(
                token,
                &self.identity,
                Shape::Fast {
                    installation_id: installation.as_deref(),
                },
            );
        }
        let presence = self.presence_now();
        let focused = self.activity_rx.borrow().focused == Some(true);
        let derived = self.account.and_then(|u| store.qos_derived(u));
        let qos = identify::qos_token(focused, derived.as_deref());
        identify::identify(
            token,
            &self.identity,
            Shape::Normal {
                installation_id: installation.as_deref(),
                reasons: launch.connect_reasons(),
                presence: presence.as_ref(),
                qos_token: Some(&qos),
            },
        )
    }

    /// `{status, since, activities, afk}` from the current settings; `None` (nothing sent) while
    /// the settings or any part of it is unknown (I-3).
    fn presence_now(&self) -> Option<LocalPresence> {
        let state = self.shared.snapshot();
        let settings = state.user_settings.as_ref()?;
        let last_input = self.activity_rx.borrow().last_input_ms;
        // ponytail: guild emoji data is F1's; until it lands a custom-emoji custom status leaves
        // the presence unknown (unsent) rather than guessed.
        local_presence(settings, now_ms(), last_input, |_| EmojiLookup::Unknown)
    }

    /// Op 3 when the local presence differs from the last one sent on this connection (or when
    /// `force`d after RESUMED), at most 5 per rolling 20 s; a limited one waits for its slot.
    async fn sync_presence(&mut self, socket: &dyn WebSocket, force: bool) -> bool {
        let Some(presence) = self.presence_now() else {
            return true;
        };
        if !force && self.presence_sent.as_ref() == Some(&presence) {
            return true;
        }
        let now = Instant::now();
        while self
            .presence_slots
            .front()
            .is_some_and(|t| now.duration_since(*t) >= PRESENCE_WINDOW)
        {
            self.presence_slots.pop_front();
        }
        if self.presence_slots.len() >= PRESENCE_BURST {
            let force = force || self.presence_retry.is_some_and(|r| r.1);
            self.presence_retry = Some((self.presence_slots[0] + PRESENCE_WINDOW, force));
            return true;
        }
        self.presence_slots.push_back(now);
        tracing::trace!(
            generation = self.generation,
            op = 3,
            "gateway send presence"
        );
        let frame = identify::presence_update(&presence);
        self.presence_sent = Some(presence);
        send(socket, Zeroizing::new(frame)).await
    }

    /// The account's heartbeat session (I§2): kept while used, a new one when velella is active
    /// and none is live (30 minutes after its last use it is gone). A new session, or `announce`
    /// with one live (READY), sends op 41 and one heartbeat.
    async fn sync_heartbeat_session(&mut self, socket: &dyn WebSocket, announce: bool) -> bool {
        let Some(user) = self.account else {
            return true;
        };
        let now = now_ms();
        let active = self.activity_rx.borrow().focused == Some(true);
        let live = self
            .heartbeat_session
            .take()
            .filter(|s| now < s.last_used_ms.saturating_add(HEARTBEAT_SESSION_TTL_MS));
        let (next, created) = match (live, active) {
            (Some(mut s), true) => {
                s.last_used_ms = now;
                (Some(s), false)
            }
            (live, false) => (live, false),
            (None, true) => (
                Some(HeartbeatSession {
                    uuid: identify::uuid_v4(),
                    created_at_ms: now,
                    last_used_ms: now,
                }),
                true,
            ),
        };
        let store = &self.identity.launch().store;
        store.set_heartbeat_session(user, next.clone());
        match &next {
            Some(s) => {
                self.identity
                    .set_client_heartbeat_session_id(s.uuid.clone());
            }
            None => self.identity.clear_client_heartbeat_session_id(),
        }
        self.heartbeat_session = next;
        let Some(s) = self
            .heartbeat_session
            .as_ref()
            .filter(|_| created || announce)
        else {
            return true;
        };
        tracing::trace!(
            generation = self.generation,
            op = 41,
            "gateway send session id"
        );
        let frame = identify::time_spent_session(
            s.created_at_ms,
            &s.uuid,
            self.identity.launch().launch_id(),
        );
        if !send(socket, Zeroizing::new(frame)).await {
            return false;
        }
        self.awaiting_ack = true;
        self.heartbeat(socket).await
    }

    /// READY's identity work, after `SessionEvent::Ready`: the account's stored values, the token
    /// refresh, the heartbeat session and the first op 3.
    async fn after_ready(&mut self, socket: &dyn WebSocket) -> bool {
        let extras = self.ready_extras.take();
        let state = self.shared.snapshot();
        let Some(user) = state.user_id else {
            return true;
        };
        let launch = self.identity.launch().clone();
        launch.ready_seen();
        let store = &launch.store;
        // Another account than the launch's last READY, or a token from the login window: the
        // token changed, so the account's heartbeat session starts over.
        let previous = launch
            .last_account
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .replace(user);
        if self.fresh_token || previous.is_some_and(|p| p != user) {
            store.set_heartbeat_session(user, None);
        }
        self.fresh_token = false;
        self.account = Some(user);
        let (auth_token, installation, qos) = match extras {
            Some(e) => (e.auth_token, e.installation, e.qos_token),
            None => (None, None, None),
        };
        if let Some(i) = installation.as_deref().filter(|i| !i.is_empty()) {
            store.adopt_installation_id(user, i);
        }
        if let Some(id) = store.installation_id(user) {
            self.identity.set_installation_id(id);
        }
        if let Some(q) = qos.as_deref() {
            store.set_qos_derived(user, q);
        }
        self.identity.set_account_locale(
            state
                .user_settings
                .as_ref()
                .and_then(|s| s.locale.as_deref()),
        );
        self.refresh_token(user, auth_token);
        self.heartbeat_session = store.heartbeat_session(user, now_ms());
        self.sync_heartbeat_session(socket, true).await && self.sync_presence(socket, true).await
    }

    /// READY `auth_token` (T-8, I-2): validated with `Token::new`; invalid or equal to the current
    /// token → ignored. Otherwise the slot (REST and the next IDENTIFY) takes it — this socket's
    /// RESUME keeps the token it identified with — and the app is told after `Ready`.
    fn refresh_token(&mut self, user: u64, raw: Option<String>) {
        let Some(raw) = raw else {
            return;
        };
        let current = self.token.current();
        let fresh = match Token::new(raw) {
            Ok(t) if t.expose() != current.expose() => Arc::new(t),
            _ => {
                tracing::debug!("READY auth_token ignored");
                return;
            }
        };
        self.shared.rest.replace_token(fresh.clone());
        self.identity
            .launch()
            .store
            .set_heartbeat_session(user, None);
        self.identity.clear_client_heartbeat_session_id();
        self.identity_event(IdentityEvent::TokenRefreshed {
            generation: self.generation,
            user_id: user,
            token: RefreshedToken::new(&fresh),
        });
    }

    /// Sends the op-37 frames that bring this connection to the newest selection. The selection is
    /// re-checked against the current state each time, so a stored selection re-sent after a
    /// reconnect never subscribes a channel that became obfuscated meanwhile (I-5).
    async fn sync_members(
        &mut self,
        socket: &dyn WebSocket,
        sent: &mut Option<(u64, u64)>,
    ) -> bool {
        let selected = *self.member_rx.borrow_and_update();
        let state = self.shared.snapshot();
        let want = selected.and_then(|(g, c)| state.member_list_target(g, c));
        for frame in member_frames(*sent, want) {
            tracing::trace!(
                generation = self.generation,
                op = 37,
                "gateway send member subscription"
            );
            if !send(socket, Zeroizing::new(frame)).await {
                return false;
            }
        }
        *sent = want;
        true
    }

    /// Runs one connection until it ends; the caller closes the socket.
    async fn connection(
        &mut self,
        socket: &dyn WebSocket,
        resuming: bool,
        established: &mut bool,
    ) -> End {
        let mut decoder = Decoder::default();
        let hello = tokio::select! {
            biased;
            () = self.stop.notified() => return End::Disconnect,
            r = timeout(Duration::from_secs(10), read_packet(socket, &mut decoder)) => r,
        };
        let hello = match hello {
            Ok(Ok(p)) if p.op == 10 => p,
            Ok(Err(gone)) => return self.gone(gone),
            _ => return End::Retry,
        };
        let interval_ms = hello
            .d
            .get("heartbeat_interval")
            .and_then(Value::as_u64)
            .unwrap_or(0);
        if !(MIN_HEARTBEAT_MS..=MAX_HEARTBEAT_MS).contains(&interval_ms) {
            return End::Retry;
        }
        let handshake = match (resuming, &self.session_token, &self.session_id, self.seq) {
            (true, Some(token), Some(session_id), Some(seq)) => {
                tracing::trace!(generation = self.generation, op = 6, "gateway send RESUME");
                identify::encode(&identify::resume(token, session_id, seq))
            }
            _ => {
                tracing::trace!(
                    generation = self.generation,
                    op = 2,
                    "gateway send IDENTIFY"
                );
                self.shared.identify_started();
                // The slot's token now; a RESUME of the session this starts reuses it.
                let token = self.token.current();
                let handshake = self.identify_frame(&token);
                self.session_token = Some(token);
                handshake
            }
        };
        if !send(socket, handshake).await {
            return End::Retry;
        }

        let interval = Duration::from_millis(interval_ms);
        // The first beat is jittered so many clients do not beat in lockstep.
        let mut timer = interval_at(
            Instant::now() + Duration::from_millis(jitter_ms(interval_ms)),
            interval,
        );
        timer.set_missed_tick_behavior(MissedTickBehavior::Delay);
        self.awaiting_ack = false;
        self.presence_sent = None;
        self.presence_retry = None;
        let mut poll = interval_at(Instant::now() + PRESENCE_POLL, PRESENCE_POLL);
        poll.set_missed_tick_behavior(MissedTickBehavior::Delay);
        let ready_deadline = Instant::now() + Duration::from_secs(30);
        // What this connection has subscribed; a new connection starts with nothing, so READY or
        // RESUMED re-sends the current selection.
        let mut members_sent = None;
        loop {
            let retry_at = self.presence_retry.map_or_else(Instant::now, |r| r.0);
            tokio::select! {
                biased;
                () = self.stop.notified() => return End::Disconnect,
                _ = timer.tick() => {
                    if self.awaiting_ack {
                        tracing::debug!("heartbeat ACK missed");
                        return End::Retry;
                    }
                    self.awaiting_ack = true;
                    if !self.heartbeat(socket).await {
                        return End::Retry;
                    }
                    if *established && !self.sync_heartbeat_session(socket, false).await {
                        return End::Retry;
                    }
                }
                () = tokio::time::sleep_until(ready_deadline), if !*established => return End::Retry,
                Some(()) = self.rest_notice.recv() => self.rest_unauthorized(),
                Ok(()) = self.member_rx.changed() => {
                    if *established && !self.sync_members(socket, &mut members_sent).await {
                        return End::Retry;
                    }
                }
                Ok(()) = self.activity_rx.changed() => {
                    // A session is created as soon as velella becomes active; kept ones are
                    // refreshed on heartbeats.
                    if *established
                        && !((self.heartbeat_session.is_some()
                            || self.sync_heartbeat_session(socket, false).await)
                            && self.sync_presence(socket, false).await)
                    {
                        return End::Retry;
                    }
                }
                _ = poll.tick() => {
                    if *established && !self.sync_presence(socket, false).await {
                        return End::Retry;
                    }
                }
                () = tokio::time::sleep_until(retry_at), if self.presence_retry.is_some() => {
                    let force = self.presence_retry.take().is_some_and(|r| r.1);
                    if !self.sync_presence(socket, force).await {
                        return End::Retry;
                    }
                }
                frame = read_packet(socket, &mut decoder) => {
                    let packet = match frame {
                        Ok(p) => p,
                        Err(gone) => return self.gone(gone),
                    };
                    tracing::trace!(generation = self.generation, op = packet.op, t = packet.t.as_deref().unwrap_or(""), "gateway packet");
                    match packet.op {
                        0 => {
                            let was_established = *established;
                            let name = packet.t.clone().unwrap_or_default();
                            if name == "READY" {
                                self.ready_extras = Some(ReadyExtras::of(&packet.d));
                            }
                            if let Some(end) = self.dispatch(packet, established) {
                                return end;
                            }
                            if !was_established && *established {
                                let identity_ok = if name == "READY" {
                                    self.after_ready(socket).await
                                } else {
                                    // RESUMED: the official client re-sends its presence.
                                    self.sync_presence(socket, true).await
                                };
                                if !identity_ok || !self.sync_members(socket, &mut members_sent).await {
                                    return End::Retry;
                                }
                            }
                            if name == "USER_SETTINGS_PROTO_UPDATE" && *established {
                                let state = self.shared.snapshot();
                                self.identity.set_account_locale(
                                    state.user_settings.as_ref().and_then(|s| s.locale.as_deref()),
                                );
                                if !self.sync_presence(socket, false).await {
                                    return End::Retry;
                                }
                            }
                        }
                        // The gateway asks for a beat: answered with op 40 like every beat.
                        1 => {
                            self.awaiting_ack = true;
                            if !self.heartbeat(socket).await {
                                return End::Retry;
                            }
                        }
                        7 => return End::Retry,
                        9 => {
                            self.clear_session();
                            return End::InvalidSession;
                        }
                        11 => self.awaiting_ack = false,
                        _ => {}
                    }
                }
            }
        }
    }

    /// Op 40 `QOS_HEARTBEAT`; op 1 is never sent (I§1).
    async fn heartbeat(&self, socket: &dyn WebSocket) -> bool {
        tracing::trace!(
            generation = self.generation,
            op = 40,
            "gateway send heartbeat"
        );
        let focused = self.activity_rx.borrow().focused;
        let beat = identify::heartbeat(self.seq, focused);
        send(socket, Zeroizing::new(beat)).await
    }

    /// Handles an op 0 packet; `Some` ends the connection.
    fn dispatch(&mut self, packet: Packet, established: &mut bool) -> Option<End> {
        if packet.s.is_some() {
            self.seq = packet.s;
        }
        let name = packet.t.as_deref().unwrap_or("");
        match name {
            "READY" => {
                if !self.shared.apply("READY", &packet.d) {
                    self.clear_session();
                    return Some(End::Retry);
                }
                self.session_id = packet
                    .d
                    .get("session_id")
                    .and_then(Value::as_str)
                    .map(str::to_owned);
                self.resume_url = packet
                    .d
                    .get("resume_gateway_url")
                    .and_then(Value::as_str)
                    .and_then(validated_resume_url);
                if self.resume_url.is_none() {
                    // Rejected (or absent) resume URL: never resume, IDENTIFY on the initial URL.
                    tracing::warn!("READY resume_gateway_url rejected; will IDENTIFY on reconnect");
                    self.session_id = None;
                }
                *established = true;
                let state = self.shared.snapshot();
                let user_id = state.user_id?;
                let username = state
                    .users
                    .get(&user_id)
                    .map(|u| u.username.clone())
                    .unwrap_or_default();
                self.emit(SessionEvent::Ready {
                    generation: self.generation,
                    user_id,
                    username,
                });
            }
            "RESUMED" => {
                *established = true;
                self.emit(SessionEvent::Resumed {
                    generation: self.generation,
                });
            }
            _ => {
                self.shared.apply(name, &packet.d);
            }
        }
        None
    }

    fn gone(&mut self, gone: Gone) -> End {
        match gone {
            Gone::Capacity => End::Stop,
            Gone::Lost => End::Retry,
            Gone::Close(code) => match close_action(code) {
                CloseAction::AuthFailed => End::Auth,
                CloseAction::Stop => End::Stop,
                CloseAction::Identify => {
                    self.clear_session();
                    End::Retry
                }
                CloseAction::Resume => End::Retry,
            },
        }
    }
}

async fn send(socket: &dyn WebSocket, text: Zeroizing<String>) -> bool {
    matches!(
        timeout(Duration::from_secs(5), socket.send(Frame::Text(text))).await,
        Ok(Ok(()))
    )
}

/// Next JSON packet, inflating binary zlib-stream frames. Cancel-safe: the transport keeps a
/// message whose `receive` was dropped for the next call, and the decoder keeps partial input.
async fn read_packet(socket: &dyn WebSocket, decoder: &mut Decoder) -> Result<Packet, Gone> {
    loop {
        let frame = match socket.receive().await {
            Received::Frame(frame) => frame,
            Received::Closed(code) => return Err(Gone::Close(code)),
            Received::Lost => return Err(Gone::Lost),
        };
        match decoder.frame(frame) {
            Ok(Some(text)) => return serde_json::from_str(&text).map_err(|_| Gone::Lost),
            Ok(None) => continue,
            Err(DecodeError::CompressedCap | DecodeError::DecompressedCap) => {
                return Err(Gone::Capacity)
            }
            Err(DecodeError::Protocol) => return Err(Gone::Lost),
        }
    }
}
