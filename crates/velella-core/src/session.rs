//! Public session API and the gateway task behind it. The connection state machine (HELLO,
//! IDENTIFY/RESUME, heartbeat with ACK timeout, op 7/9, close-code classification, backoff) is
//! modeled on Serein's gateway loop (refs/serein/crates/discord-gateway/src/lib.rs, MIT OR
//! Apache-2.0) but written for velella's much smaller surface.
//!
//! Logging policy: tracing records op codes and event names only, never payloads or the token.
use crate::{
    compression::{DecodeError, Decoder, MAX_GATEWAY_WIRE},
    gateway::{backoff, close_action, jitter_ms, member_frames, validated_resume_url, CloseAction},
    identify::{self, GATEWAY_URL},
    messages::MessageStore,
    rest::Rest,
    state::{MemberSub, State},
    token::Token,
};
use futures_util::{Sink, SinkExt, Stream, StreamExt};
use serde::Deserialize;
use serde_json::Value;
use std::{
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
use tokio_tungstenite::{
    connect_async_with_config,
    tungstenite::{protocol::WebSocketConfig, Error as WsError, Message},
};

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
    #[cfg(test)]
    applied: Mutex<Vec<(String, Option<MemberSub>)>>,
}

impl Shared {
    fn snapshot(&self) -> Arc<State> {
        self.state.lock().unwrap_or_else(|e| e.into_inner()).clone()
    }

    /// Copy-on-write: readers holding a snapshot are never mutated underneath.
    // ponytail: clones the whole State when a reader holds a snapshot; fine at sidebar sizes,
    // switch to per-guild Arcs if profiling says otherwise.
    fn apply(&self, event: &str, d: &Value) -> bool {
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
            changed
        };
        // Outside the state lock: the store keeps its own buffers. Every dispatch goes through,
        // including the ones `State` ignores (MESSAGE_*).
        if let Some(store) = self.messages.get() {
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
            .get_or_init(|| MessageStore::new(self.shared.rest.clone()))
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
    /// inside a runtime context. `events` runs on the session task, so it must not block.
    pub fn connect(
        token: Token,
        events: impl Fn(SessionEvent) + Send + Sync + 'static,
    ) -> SessionHandle {
        let generation = GENERATION.fetch_add(1, Ordering::SeqCst) + 1;
        let token = Arc::new(token);
        let (notice, rest_notice) = mpsc::unbounded_channel();
        let shared = Arc::new(Shared {
            state: Mutex::new(Arc::new(State::default())),
            revision: watch::channel(0).0,
            ended: watch::channel(false).0,
            rest: Rest::new(token.clone(), generation, notice),
            messages: OnceLock::new(),
            member_sub: watch::channel(None).0,
            member_epoch: AtomicU64::new(0),
            #[cfg(test)]
            applied: Mutex::default(),
        });
        let stop = Arc::new(Notify::new());
        let task = Task {
            token,
            generation,
            events: Box::new(events),
            member_rx: shared.member_sub.subscribe(),
            shared: shared.clone(),
            stop: stop.clone(),
            rest_notice,
            session_id: None,
            resume_url: None,
            seq: None,
            #[cfg(test)]
            test_endpoints: test_support::TEST_ENDPOINTS.with(|e| e.borrow().clone()),
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
    Close(Option<u16>),
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
    /// Shared with the session's `Rest`; zeroized when the last holder drops it.
    token: Arc<Token>,
    generation: u64,
    events: Box<dyn Fn(SessionEvent) + Send + Sync>,
    shared: Arc<Shared>,
    stop: Arc<Notify>,
    /// The REST 401 notice. Dropped with the task, so a 401 after the end reaches nobody.
    rest_notice: mpsc::UnboundedReceiver<()>,
    member_rx: watch::Receiver<Option<(u64, u64)>>,
    session_id: Option<String>,
    /// Already validated and normalized.
    resume_url: Option<String>,
    seq: Option<u64>,
    /// (initial endpoint, resume endpoint) replacing only the dial target.
    #[cfg(test)]
    test_endpoints: Option<(String, String)>,
}

const MIN_HEARTBEAT_MS: u64 = 50;
const MAX_HEARTBEAT_MS: u64 = 120_000;

impl Task {
    fn emit(&self, event: SessionEvent) {
        (self.events)(event);
    }

    fn clear_session(&mut self) {
        self.session_id = None;
        self.resume_url = None;
        self.seq = None;
    }

    /// Compiled out of shipped builds: tests replace the dial target after URL validation.
    fn dial_target(&self, url: &str) -> String {
        #[cfg(test)]
        if let Some((initial, resume)) = &self.test_endpoints {
            return if url == GATEWAY_URL {
                initial.clone()
            } else {
                resume.clone()
            };
        }
        url.to_owned()
    }

    async fn run(mut self) {
        let mut failures = 0u32;
        loop {
            let resuming =
                self.session_id.is_some() && self.resume_url.is_some() && self.seq.is_some();
            if !resuming {
                self.clear_session();
            }
            let url = self
                .resume_url
                .clone()
                .filter(|_| resuming)
                .unwrap_or_else(|| GATEWAY_URL.to_owned());
            let target = self.dial_target(&url);
            let config = WebSocketConfig::default()
                .max_message_size(Some(MAX_GATEWAY_WIRE))
                .max_frame_size(Some(MAX_GATEWAY_WIRE));
            let dial = timeout(
                Duration::from_secs(15),
                connect_async_with_config(target, Some(config), false),
            );
            let Some(dialed) = self.unless_stopped(dial).await else {
                return self.emit(SessionEvent::Closed {
                    generation: self.generation,
                });
            };
            let mut established = false;
            let end = match dialed {
                Ok(Ok((mut socket, _))) => {
                    self.connection(&mut socket, resuming, &mut established)
                        .await
                }
                _ => End::Retry,
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

    /// Sends the op-37 frames that bring this connection to the newest selection.
    async fn sync_members<S: Sink<Message, Error = WsError> + Unpin>(
        &mut self,
        socket: &mut S,
        sent: &mut Option<(u64, u64)>,
    ) -> bool {
        let want = *self.member_rx.borrow_and_update();
        for frame in member_frames(*sent, want) {
            tracing::trace!(
                generation = self.generation,
                op = 37,
                "gateway send member subscription"
            );
            if !send(socket, &frame).await {
                return false;
            }
        }
        *sent = want;
        true
    }

    async fn connection<S>(&mut self, socket: &mut S, resuming: bool, established: &mut bool) -> End
    where
        S: Stream<Item = Result<Message, WsError>> + Sink<Message, Error = WsError> + Unpin,
    {
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
        let handshake = match (resuming, &self.session_id, self.seq) {
            (true, Some(session_id), Some(seq)) => {
                tracing::trace!(generation = self.generation, op = 6, "gateway send RESUME");
                identify::encode(&identify::resume(&self.token, session_id, seq))
            }
            _ => {
                tracing::trace!(
                    generation = self.generation,
                    op = 2,
                    "gateway send IDENTIFY"
                );
                identify::encode(&identify::identify(&self.token))
            }
        };
        if !send(socket, &handshake).await {
            return End::Retry;
        }
        drop(handshake);

        let interval = Duration::from_millis(interval_ms);
        // The first beat is jittered so many clients do not beat in lockstep.
        let mut timer = interval_at(
            Instant::now() + Duration::from_millis(jitter_ms(interval_ms)),
            interval,
        );
        timer.set_missed_tick_behavior(MissedTickBehavior::Delay);
        let mut awaiting_ack = false;
        let ready_deadline = Instant::now() + Duration::from_secs(30);
        // What this connection has subscribed; a new connection starts with nothing, so READY or
        // RESUMED re-sends the current selection.
        let mut members_sent = None;
        loop {
            tokio::select! {
                biased;
                () = self.stop.notified() => {
                    let _ = timeout(Duration::from_secs(1), socket.close()).await;
                    return End::Disconnect;
                }
                _ = timer.tick() => {
                    if awaiting_ack {
                        tracing::debug!("heartbeat ACK missed");
                        return End::Retry;
                    }
                    awaiting_ack = true;
                    if !self.heartbeat(socket).await {
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
                frame = read_packet(socket, &mut decoder) => {
                    let packet = match frame {
                        Ok(p) => p,
                        Err(gone) => return self.gone(gone),
                    };
                    tracing::trace!(generation = self.generation, op = packet.op, t = packet.t.as_deref().unwrap_or(""), "gateway packet");
                    match packet.op {
                        0 => {
                            let was_established = *established;
                            if let Some(end) = self.dispatch(packet, established) {
                                return end;
                            }
                            if !was_established
                                && *established
                                && !self.sync_members(socket, &mut members_sent).await
                            {
                                return End::Retry;
                            }
                        }
                        1 => {
                            awaiting_ack = true;
                            if !self.heartbeat(socket).await {
                                return End::Retry;
                            }
                        }
                        7 => return End::Retry,
                        9 => {
                            self.clear_session();
                            return End::InvalidSession;
                        }
                        11 => awaiting_ack = false,
                        _ => {}
                    }
                }
            }
        }
    }

    async fn heartbeat<S: Sink<Message, Error = WsError> + Unpin>(&self, socket: &mut S) -> bool {
        tracing::trace!(
            generation = self.generation,
            op = 1,
            "gateway send heartbeat"
        );
        let beat = match self.seq {
            Some(seq) => format!(r#"{{"op":1,"d":{seq}}}"#),
            None => r#"{"op":1,"d":null}"#.to_owned(),
        };
        send(socket, &beat).await
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
            Gone::Lost | Gone::Close(None) => End::Retry,
            Gone::Close(Some(code)) => match close_action(code) {
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

async fn send<S: Sink<Message, Error = WsError> + Unpin>(socket: &mut S, text: &str) -> bool {
    matches!(
        timeout(Duration::from_secs(5), socket.send(Message::text(text))).await,
        Ok(Ok(()))
    )
}

/// Next JSON packet, inflating binary zlib-stream frames. Control frames are skipped.
async fn read_packet<S: Stream<Item = Result<Message, WsError>> + Unpin>(
    socket: &mut S,
    decoder: &mut Decoder,
) -> Result<Packet, Gone> {
    loop {
        let frame = match socket.next().await {
            Some(Ok(frame)) => frame,
            _ => return Err(Gone::Lost),
        };
        let frame = match decoder.frame(frame) {
            Ok(Some(frame)) => frame,
            Ok(None) => continue,
            Err(DecodeError::CompressedCap | DecodeError::DecompressedCap) => {
                return Err(Gone::Capacity)
            }
            Err(DecodeError::Protocol) => return Err(Gone::Lost),
        };
        match frame {
            Message::Text(text) => return serde_json::from_str(&text).map_err(|_| Gone::Lost),
            Message::Close(frame) => return Err(Gone::Close(frame.map(|f| u16::from(f.code)))),
            _ => {}
        }
    }
}

#[cfg(test)]
pub(crate) mod test_support {
    use std::cell::RefCell;
    thread_local! {
        /// Set by a test on its own (current-thread runtime) thread before `Session::connect`.
        pub(crate) static TEST_ENDPOINTS: RefCell<Option<(String, String)>> = const { RefCell::new(None) };
    }
}
