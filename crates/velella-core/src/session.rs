//! Public session API and the gateway task behind it. The connection state machine (HELLO,
//! IDENTIFY/RESUME, heartbeat with ACK timeout, op 7/9, close-code classification, backoff) is
//! modeled on Serein's gateway loop (refs/serein/crates/discord-gateway/src/lib.rs, MIT OR
//! Apache-2.0) but written for velella's much smaller surface.
//!
//! Logging policy: tracing records op codes and event names only, never payloads or the token.
use crate::{
    compression::{DecodeError, Decoder, MAX_GATEWAY_WIRE},
    gateway::{backoff, close_action, jitter_ms, validated_resume_url, CloseAction},
    identify::{self, GATEWAY_URL},
    state::State,
    token::Token,
};
use futures_util::{Sink, SinkExt, Stream, StreamExt};
use serde::Deserialize;
use serde_json::Value;
use std::{
    fmt,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc, Mutex,
    },
    time::Duration,
};
use tokio::{
    sync::{watch, Notify},
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
    Reconnecting {
        generation: u64,
    },
    AuthFailed {
        generation: u64,
    },
    Closed {
        generation: u64,
    },
}

/// Process-wide, monotonic per connect.
static GENERATION: AtomicU64 = AtomicU64::new(0);

struct Shared {
    state: Mutex<Arc<State>>,
    revision: watch::Sender<u64>,
}

impl Shared {
    fn snapshot(&self) -> Arc<State> {
        self.state.lock().unwrap_or_else(|e| e.into_inner()).clone()
    }

    /// Copy-on-write: readers holding a snapshot are never mutated underneath.
    // ponytail: clones the whole State when a reader holds a snapshot; fine at sidebar sizes,
    // switch to per-guild Arcs if profiling says otherwise.
    fn apply(&self, event: &str, d: &Value) -> bool {
        let mut guard = self.state.lock().unwrap_or_else(|e| e.into_inner());
        let changed = Arc::make_mut(&mut guard).apply(event, d);
        if changed {
            self.revision.send_replace(guard.revision);
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
        let shared = Arc::new(Shared {
            state: Mutex::new(Arc::new(State::default())),
            revision: watch::channel(0).0,
        });
        let stop = Arc::new(Notify::new());
        let task = Task {
            token,
            generation,
            events: Box::new(events),
            shared: shared.clone(),
            stop: stop.clone(),
            session_id: None,
            resume_url: None,
            seq: None,
            #[cfg(test)]
            test_endpoints: test_support::TEST_ENDPOINTS.with(|e| e.borrow().clone()),
        };
        tokio::spawn(task.run());
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
    token: Token,
    generation: u64,
    events: Box<dyn Fn(SessionEvent) + Send + Sync>,
    shared: Arc<Shared>,
    stop: Arc<Notify>,
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
            let dialed = tokio::select! {
                biased;
                () = self.stop.notified() => return self.emit(SessionEvent::Closed { generation: self.generation }),
                r = timeout(Duration::from_secs(15), connect_async_with_config(target, Some(config), false)) => r,
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
            tokio::select! {
                biased;
                () = self.stop.notified() => return self.emit(SessionEvent::Closed { generation: self.generation }),
                () = sleep(delay) => {}
            }
        }
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
                frame = read_packet(socket, &mut decoder) => {
                    let packet = match frame {
                        Ok(p) => p,
                        Err(gone) => return self.gone(gone),
                    };
                    tracing::trace!(generation = self.generation, op = packet.op, t = packet.t.as_deref().unwrap_or(""), "gateway packet");
                    match packet.op {
                        0 => {
                            if let Some(end) = self.dispatch(packet, established) {
                                return end;
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
            "RESUMED" => *established = true,
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
