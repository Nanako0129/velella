//! Session tests through the public `Session::connect` on the in-memory transport. The mock
//! gateway speaks zlib-stream Binary frames like Discord; the transport routes a dial to the
//! fixed gateway URL to `Rig::initial` and any other (resume) URL to `Rig::resume`.
use crate::{
    identify::{self, GATEWAY_URL},
    rest::test_support::TEST_HTTP,
    transport::{
        memory::{Endpoints, FromClient, MemoryTransport, ServerEnd},
        Frame, Received,
    },
    Session, SessionEvent, SessionHandle, Token,
};
use flate2::write::ZlibEncoder;
use serde_json::{json, Value};
use std::{
    io::Write,
    sync::{Arc, Mutex, OnceLock},
    time::Duration,
};
use tokio::{sync::mpsc, time::timeout};

const MARKER: &str = "SYNTHETIC_TOKEN_MARKER_0123456789";
const WAIT: Duration = Duration::from_secs(15);

/// One gateway endpoint: the connections the session dials there, in order.
struct Listener(tokio::sync::Mutex<mpsc::UnboundedReceiver<ServerEnd>>);

impl Listener {
    async fn accept(&self) -> ServerEnd {
        timeout(WAIT, self.0.lock().await.recv())
            .await
            .expect("no connection")
            .expect("transport gone")
    }

    /// A connection is already waiting (no wait).
    fn has_pending(&self) -> bool {
        !self.0.try_lock().expect("listener busy").is_empty()
    }
}

/// One accepted client connection with its own zlib-stream compressor.
struct Conn {
    end: ServerEnd,
    enc: ZlibEncoder<Vec<u8>>,
    sent: usize,
}

impl Conn {
    async fn accept(listener: &Listener) -> Conn {
        Conn {
            end: listener.accept().await,
            enc: ZlibEncoder::new(Vec::new(), flate2::Compression::fast()),
            sent: 0,
        }
    }

    fn to_client(&self, received: Received) {
        if let Some(tx) = &self.end.to_client {
            let _ = tx.send(received);
        }
    }

    /// Sends one gateway packet as a Binary zlib-stream frame (sync-flushed, `00 00 FF FF` suffix).
    async fn send(&mut self, packet: Value) {
        self.enc.write_all(packet.to_string().as_bytes()).unwrap();
        self.enc.flush().unwrap();
        let bytes = self.enc.get_ref()[self.sent..].to_vec();
        self.sent = self.enc.get_ref().len();
        self.to_client(Received::Frame(Frame::Binary(bytes)));
    }

    async fn hello(&mut self, interval_ms: u64) {
        self.send(json!({"op": 10, "d": {"heartbeat_interval": interval_ms}}))
            .await;
    }

    /// Next thing the client did on this connection.
    async fn next(&mut self) -> Option<FromClient> {
        timeout(WAIT, self.end.from_client.recv())
            .await
            .expect("client did nothing")
    }

    /// Next client packet as raw text (clients send uncompressed Text).
    async fn recv_raw(&mut self) -> String {
        match self.next().await {
            Some(FromClient::Frame(Frame::Text(t))) => t.to_string(),
            other => panic!("expected a text frame, got {other:?}"),
        }
    }

    async fn recv(&mut self) -> Value {
        serde_json::from_str(&self.recv_raw().await).unwrap()
    }

    /// Next client packet that is not a heartbeat: the first beat is jittered and may go out at
    /// any moment, even right after HELLO.
    async fn recv_non_beat(&mut self) -> Value {
        loop {
            let packet = self.recv().await;
            if packet["op"] != 40 {
                return packet;
            }
        }
    }

    /// The close code the client sent, skipping heartbeats before it.
    async fn closed_with(&mut self) -> u16 {
        loop {
            match self.next().await {
                Some(FromClient::Close(code)) => return code,
                Some(FromClient::Frame(Frame::Text(t)))
                    if serde_json::from_str::<Value>(&t).is_ok_and(|v| v["op"] == 40) => {}
                other => panic!("expected a close, got {other:?}"),
            }
        }
    }

    /// True when nothing but heartbeats arrives for a while.
    async fn quiet(&mut self) -> bool {
        let deadline = tokio::time::Instant::now() + Duration::from_millis(400);
        loop {
            match tokio::time::timeout_at(deadline, self.end.from_client.recv()).await {
                Err(_) => return true,
                Ok(Some(FromClient::Frame(Frame::Text(t))))
                    if serde_json::from_str::<Value>(&t).is_ok_and(|v| v["op"] == 40) => {}
                Ok(_) => return false,
            }
        }
    }

    /// A close frame with `code`.
    async fn close(&mut self, code: u16) {
        self.to_client(Received::Closed(code));
    }

    /// The connection ends without a close frame (TCP drop).
    fn drop_abruptly(&mut self) {
        self.end.to_client = None;
    }
}

fn ready(seq: u64, resume_url: &str) -> Value {
    json!({"op": 0, "t": "READY", "s": seq, "d": {
        "user": {"id": "1", "username": "synthetic", "global_name": "Syn"},
        "session_id": "sess-1", "resume_gateway_url": resume_url,
        "guilds": [], "private_channels": [],
        "user_settings": {"guild_folders": []}
    }})
}

struct Rig {
    initial: Listener,
    resume: Listener,
    handle: SessionHandle,
    events: mpsc::UnboundedReceiver<SessionEvent>,
    identity: mpsc::UnboundedReceiver<crate::IdentityEvent>,
    /// Every session and identity event's name, in the order the callbacks ran.
    order: Arc<Mutex<Vec<String>>>,
}

impl Rig {
    /// A session on the in-memory transport: the gateway endpoints here, HTTP answered by the
    /// mock installed on this thread (if any).
    async fn start() -> Rig {
        Rig::start_with(crate::SessionOptions::default()).await
    }

    async fn start_with(mut options: crate::SessionOptions) -> Rig {
        Rig::start_token(MARKER, options_with_log(&mut options), options).await
    }

    async fn start_token(
        token: &str,
        (order, identity): (
            Arc<Mutex<Vec<String>>>,
            mpsc::UnboundedReceiver<crate::IdentityEvent>,
        ),
        options: crate::SessionOptions,
    ) -> Rig {
        capture();
        let (initial_tx, initial) = mpsc::unbounded_channel();
        let (resume_tx, resume) = mpsc::unbounded_channel();
        let transport = Arc::new(MemoryTransport {
            http: TEST_HTTP.with(|h| h.borrow().clone()),
            endpoints: Some(Endpoints {
                initial: initial_tx,
                resume: resume_tx,
            }),
        });
        let (tx, events) = mpsc::unbounded_channel();
        let log = order.clone();
        let handle = Session::connect_with(
            Token::new(token.into()).unwrap(),
            transport,
            options,
            move |e| {
                log.lock().unwrap().push(
                    format!("{e:?}")
                        .split([' ', '{'])
                        .next()
                        .unwrap()
                        .to_owned(),
                );
                let _ = tx.send(e);
            },
        );
        Rig {
            initial: Listener(tokio::sync::Mutex::new(initial)),
            resume: Listener(tokio::sync::Mutex::new(resume)),
            handle,
            events,
            identity,
            order,
        }
    }

    async fn event(&mut self) -> SessionEvent {
        timeout(WAIT, self.events.recv())
            .await
            .expect("no event")
            .expect("session ended")
    }

    /// Connects, completes HELLO/IDENTIFY, sends READY and waits for the `Ready` event.
    async fn ready(&mut self, interval_ms: u64, resume_url: &str) -> Conn {
        let mut conn = Conn::accept(&self.initial).await;
        conn.hello(interval_ms).await;
        assert_eq!(conn.recv().await["op"], 2);
        conn.send(ready(1, resume_url)).await;
        assert!(matches!(self.event().await, SessionEvent::Ready { .. }));
        conn
    }
}

/// Routes `options`' identity events into a channel and the shared order log.
fn options_with_log(
    options: &mut crate::SessionOptions,
) -> (
    Arc<Mutex<Vec<String>>>,
    mpsc::UnboundedReceiver<crate::IdentityEvent>,
) {
    let order = Arc::new(Mutex::new(Vec::new()));
    let (tx, rx) = mpsc::unbounded_channel();
    let log = order.clone();
    options.identity_events = Some(Box::new(move |e| {
        let name = match &e {
            crate::IdentityEvent::TokenRefreshed { .. } => "TokenRefreshed",
            crate::IdentityEvent::Notice { .. } => "Notice",
        };
        log.lock().unwrap().push(name.to_owned());
        let _ = tx.send(e);
    }));
    (order, rx)
}

async fn no_connection(listener: &Listener) -> bool {
    timeout(Duration::from_millis(400), listener.accept())
        .await
        .is_err()
}

#[tokio::test]
async fn hello_identify_ready_yields_ready_event() {
    let mut rig = Rig::start().await;
    let mut conn = Conn::accept(&rig.initial).await;
    conn.hello(30_000).await;
    let identify = conn.recv().await;
    assert_eq!(identify["op"], 2);
    assert_eq!(identify["d"]["token"], MARKER);
    assert_eq!(identify["d"]["capabilities"], 1_734_653);
    conn.send(ready(1, "wss://gateway-resume.discord.gg/"))
        .await;
    assert_eq!(
        rig.event().await,
        SessionEvent::Ready {
            generation: rig.handle.generation(),
            user_id: 1,
            username: "synthetic".into()
        }
    );
    let state = rig.handle.snapshot();
    assert_eq!(state.user_id, Some(1));
    assert_eq!(state.guild_folders.as_ref().map(Vec::len), Some(0));
    assert_eq!(*rig.handle.state().borrow(), 1);
}

/// The dial carries the fixed URL, the session's User-Agent (never the platform default), and the
/// inflation cap as the message-size limit; the resume dial the validated, normalized URL.
#[tokio::test]
async fn the_dial_carries_the_fixed_url_the_user_agent_and_the_cap() {
    let mut rig = Rig::start().await;
    let first = rig.ready(100, "wss://gateway-us-east1-b.discord.gg").await;
    let request = &first.end.request;
    assert_eq!(request.url(), GATEWAY_URL);
    assert_eq!(
        request.headers(),
        [
            ("User-Agent".to_owned(), identify::USER_AGENT.to_owned()),
            ("Accept-Language".to_owned(), "en-US,en;q=0.9".to_owned())
        ]
    );
    assert_eq!(request.max_message_size(), 64 << 20);
    // Missed ACK: the session resumes at the validated resume URL.
    let resumed = Conn::accept(&rig.resume).await;
    assert_eq!(
        resumed.end.request.url(),
        "wss://gateway-us-east1-b.discord.gg/?v=9&encoding=json&compress=zlib-stream"
    );
}

#[tokio::test]
async fn dispatches_bump_state_revision() {
    let mut rig = Rig::start().await;
    let mut conn = rig.ready(30_000, "wss://gateway-resume.discord.gg/").await;
    let mut revision = rig.handle.state();
    let before = *revision.borrow_and_update();
    conn.send(json!({"op": 0, "t": "GUILD_CREATE", "s": 2, "d": {"id": "10", "name": "g", "owner_id": "9", "roles": [], "channels": []}})).await;
    timeout(WAIT, revision.changed()).await.unwrap().unwrap();
    let after_guild = *revision.borrow_and_update();
    assert!(after_guild > before);
    assert_eq!(rig.handle.snapshot().post_ready_guild_ids(), vec![10]);
    conn.send(json!({"op": 0, "t": "CHANNEL_CREATE", "s": 3, "d": {"id": "20", "guild_id": "10", "type": 0, "name": "c"}})).await;
    timeout(WAIT, revision.changed()).await.unwrap().unwrap();
    assert!(*revision.borrow_and_update() > after_guild);
    assert_eq!(rig.handle.snapshot().guild(10).unwrap().channels[0].id, 20);
    // A snapshot taken earlier is immutable.
    let old = rig.handle.snapshot();
    conn.send(json!({"op": 0, "t": "GUILD_DELETE", "s": 4, "d": {"id": "10"}}))
        .await;
    timeout(WAIT, revision.changed()).await.unwrap().unwrap();
    assert!(old.guild(10).is_some());
    assert!(rig.handle.snapshot().guild(10).is_none());
}

#[tokio::test]
async fn missed_ack_resumes_at_the_resume_endpoint() {
    let mut rig = Rig::start().await;
    // 100 ms interval and the mock never ACKs: the second beat finds the first unanswered.
    let mut first = rig.ready(100, "wss://gateway-resume.discord.gg/").await;
    let mut conn = Conn::accept(&rig.resume).await;
    assert_eq!(
        rig.event().await,
        SessionEvent::Reconnecting {
            generation: rig.handle.generation()
        }
    );
    // T-4: the old connection was closed explicitly, with a code that keeps the session resumable.
    assert_eq!(first.closed_with().await, 4000);
    conn.hello(30_000).await;
    let resume = conn.recv().await;
    assert_eq!(resume["op"], 6);
    assert_eq!(resume["d"]["token"], MARKER);
    assert_eq!(resume["d"]["session_id"], "sess-1");
    assert_eq!(resume["d"]["seq"], 1);
    assert!(no_connection(&rig.initial).await, "must not IDENTIFY again");
}

/// One token slot for REST and the gateway: after `replace_token`, a RESUME of the running
/// session still sends the token that session identified with, and the next fresh IDENTIFY sends
/// the new one.
#[tokio::test]
async fn a_replaced_token_goes_to_the_next_identify_but_not_to_a_resume() {
    const FRESH: &str = "SYNTHETIC_FRESH_TOKEN_9876543210";
    let mut rig = Rig::start().await;
    let _first = rig.ready(100, "wss://gateway-resume.discord.gg/").await;
    rig.handle
        .rest()
        .replace_token(Arc::new(Token::new(FRESH.into()).unwrap()));
    // Missed ACK: RESUME on a new connection, with the session's own token.
    let mut conn = Conn::accept(&rig.resume).await;
    conn.hello(30_000).await;
    let resume = conn.recv_non_beat().await;
    assert_eq!(
        (resume["op"].as_u64(), resume["d"]["token"].as_str()),
        (Some(6), Some(MARKER))
    );
    // Op 9 clears the session: the fresh IDENTIFY reads the slot.
    conn.send(json!({"op": 9, "d": false})).await;
    let mut again = Conn::accept(&rig.initial).await;
    again.hello(30_000).await;
    let identify = again.recv_non_beat().await;
    assert_eq!(
        (identify["op"].as_u64(), identify["d"]["token"].as_str()),
        (Some(2), Some(FRESH))
    );
}

/// A connection that ends without a close frame (TCP drop) resumes; a 4004 close frame ends the
/// session. The two must never be confused (T-6).
#[tokio::test]
async fn an_abrupt_drop_resumes() {
    let mut rig = Rig::start().await;
    let mut first = rig.ready(30_000, "wss://gateway-resume.discord.gg/").await;
    first.drop_abruptly();
    let mut conn = Conn::accept(&rig.resume).await;
    assert_eq!(
        rig.event().await,
        SessionEvent::Reconnecting {
            generation: rig.handle.generation()
        }
    );
    conn.hello(30_000).await;
    assert_eq!(conn.recv().await["op"], 6);
}

#[tokio::test]
async fn resumed_dispatch_emits_resumed_and_no_second_ready() {
    let mut rig = Rig::start().await;
    let generation = rig.handle.generation();
    let _first = rig.ready(100, "wss://gateway-resume.discord.gg/").await;
    let mut conn = Conn::accept(&rig.resume).await;
    assert_eq!(rig.event().await, SessionEvent::Reconnecting { generation });
    conn.hello(30_000).await;
    assert_eq!(conn.recv().await["op"], 6);
    conn.send(json!({"op": 0, "t": "RESUMED", "s": 2, "d": {}}))
        .await;
    assert_eq!(rig.event().await, SessionEvent::Resumed { generation });
    // Nothing else follows: no second Ready, no further reconnect.
    assert!(
        timeout(Duration::from_millis(400), rig.events.recv())
            .await
            .is_err(),
        "unexpected event after Resumed"
    );
    assert!(!format!("{:?}", SessionEvent::Resumed { generation }).contains("SYNTHETIC"));
}

#[tokio::test]
async fn acked_heartbeats_keep_the_connection_and_carry_the_sequence() {
    let mut rig = Rig::start().await;
    let mut conn = rig.ready(100, "wss://gateway-resume.discord.gg/").await;
    let mut last = Value::Null;
    for _ in 0..5 {
        last = conn.recv().await;
        assert_eq!(last["op"], 40);
        conn.send(json!({"op": 11, "d": null})).await;
    }
    // (A first beat can race ahead of READY and carry null; by now the sequence is known.)
    assert_eq!(last["d"]["seq"], 1);
    // Checked right after the last ACK (before the mock goes silent): nothing reconnected.
    assert!(!rig.initial.has_pending());
    assert!(!rig.resume.has_pending());
    assert!(
        rig.events.try_recv().is_err(),
        "no Reconnecting while ACKs arrive"
    );
}

#[tokio::test]
async fn evil_resume_url_falls_back_to_identify_at_the_initial_endpoint() {
    let mut rig = Rig::start().await;
    let first = rig.ready(100, "wss://evil.test").await;
    // Missed ACK forces a reconnect; the rejected URL must leave no resume state.
    let mut conn = Conn::accept(&rig.initial).await;
    assert_eq!(
        rig.event().await,
        SessionEvent::Reconnecting {
            generation: rig.handle.generation()
        }
    );
    conn.hello(30_000).await;
    let identify = conn.recv().await;
    assert_eq!(identify["op"], 2);
    assert_eq!(identify["d"]["token"], MARKER);
    assert!(
        no_connection(&rig.resume).await,
        "evil URL must never be dialed"
    );
    drop(first);
}

#[tokio::test]
async fn invalid_session_reidentifies() {
    let mut rig = Rig::start().await;
    let mut first = rig.ready(30_000, "wss://gateway-resume.discord.gg/").await;
    first.send(json!({"op": 9, "d": true})).await;
    // Even `d: true` re-identifies; Discord asks for a 1-5 s pause first.
    let mut conn = Conn::accept(&rig.initial).await;
    assert_eq!(
        rig.event().await,
        SessionEvent::Reconnecting {
            generation: rig.handle.generation()
        }
    );
    conn.hello(30_000).await;
    assert_eq!(conn.recv().await["op"], 2);
    assert!(no_connection(&rig.resume).await);
}

#[tokio::test]
async fn close_4004_is_auth_failed_and_not_retried() {
    let mut rig = Rig::start().await;
    let mut conn = Conn::accept(&rig.initial).await;
    conn.hello(30_000).await;
    assert_eq!(conn.recv().await["op"], 2);
    conn.close(4004).await;
    assert_eq!(
        rig.event().await,
        SessionEvent::AuthFailed {
            generation: rig.handle.generation()
        }
    );
    // T-4: closed explicitly on this exit path too.
    assert_eq!(conn.closed_with().await, 1000);
    assert!(no_connection(&rig.initial).await, "4004 must not retry");
    assert!(no_connection(&rig.resume).await);
    // The task ended: its callback was dropped, so the channel is closed and nothing else came.
    assert_eq!(timeout(WAIT, rig.events.recv()).await.unwrap(), None);
}

/// The client may beat at any moment before it closes: its first beat is jittered, and the jitter
/// comes in whole seconds on macOS (`SystemTime` has microsecond resolution, so
/// `subsec_nanos() % 30_000` is a multiple of 1000), so about one run in 30 beat at once and the
/// heartbeat, not the Close frame, was the next frame here. The mock's op 1 below makes that beat
/// happen in (nearly) every run; heartbeats before the Close frame are skipped.
#[tokio::test]
async fn disconnect_emits_closed_and_generations_increase() {
    let mut rig = Rig::start().await;
    let mut conn = rig.ready(30_000, "wss://gateway-resume.discord.gg/").await;
    let generation = rig.handle.generation();
    conn.send(json!({"op": 1, "d": null})).await;
    // ACK the beat that op 1 provoked: an unacked beat would make a later jittered tick report
    // "heartbeat ACK missed" and Reconnect before the disconnect lands (seen under load).
    while conn.recv().await["op"] != 40 {}
    conn.send(json!({"op": 11})).await;
    tokio::time::sleep(Duration::from_millis(100)).await;
    rig.handle.disconnect();
    // A heartbeat tick can still race the disconnect under load (macOS jitter is whole seconds)
    // and report one Reconnecting first; the claim here is that Closed always ends the session.
    let ended = loop {
        match rig.event().await {
            SessionEvent::Reconnecting { generation: g } if g == generation => continue,
            other => break other,
        }
    };
    assert_eq!(ended, SessionEvent::Closed { generation });
    // T-4: a disconnect closes with 1000 (or 4000 if that racing reconnect won).
    assert!(matches!(conn.closed_with().await, 1000 | 4000));
    let second = Rig::start().await;
    assert!(second.handle.generation() > generation);
}

#[tokio::test]
async fn debug_output_never_contains_the_token() {
    let mut rig = Rig::start().await;
    let _conn = rig.ready(30_000, "wss://gateway-resume.discord.gg/").await;
    let token = Token::new(MARKER.into()).unwrap();
    let texts = [
        format!("{:?}", rig.handle),
        format!("{:#?}", rig.handle),
        format!("{token:?}"),
        format!(
            "{:?}",
            SessionEvent::Ready {
                generation: 1,
                user_id: 1,
                username: "u".into()
            }
        ),
        format!("{:?}", SessionEvent::AuthFailed { generation: 1 }),
        format!("{:?}", Token::new("short".into()).unwrap_err()),
    ];
    for text in texts {
        assert!(!text.contains("SYNTHETIC"), "{text}");
    }
}

#[derive(Clone, Default)]
struct Capture(Arc<Mutex<Vec<u8>>>);
impl Write for Capture {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.0.lock().unwrap().extend_from_slice(buf);
        Ok(buf.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}
impl<'a> tracing_subscriber::fmt::MakeWriter<'a> for Capture {
    type Writer = Capture;
    fn make_writer(&'a self) -> Capture {
        self.clone()
    }
}

/// Process-wide TRACE capture, installed by every `Rig::start` before any session code runs.
/// A scoped subscriber would race with tracing's callsite-interest cache when tests run in
/// parallel and silently drop events; the global one sees everything, and the test below selects
/// its own lines by generation.
fn capture() -> &'static Capture {
    static CAPTURE: OnceLock<Capture> = OnceLock::new();
    CAPTURE.get_or_init(|| {
        let capture = Capture::default();
        let subscriber = tracing_subscriber::fmt()
            .with_max_level(tracing::Level::TRACE)
            .with_writer(capture.clone())
            .with_ansi(false)
            .finish();
        tracing::subscriber::set_global_default(subscriber).expect("global subscriber");
        capture
    })
}

#[tokio::test]
async fn trace_logs_never_contain_the_token_but_identify_and_resume_frames_do() {
    let mut rig = Rig::start().await;
    let generation = rig.handle.generation();
    let mut first = Conn::accept(&rig.initial).await;
    first.hello(100).await;
    let identify = first.recv_raw().await;
    first
        .send(ready(1, "wss://gateway-resume.discord.gg/"))
        .await;
    assert!(matches!(rig.event().await, SessionEvent::Ready { .. }));
    // Missed ACK -> RESUME.
    let mut second = Conn::accept(&rig.resume).await;
    second.hello(30_000).await;
    let resume = second.recv_raw().await;

    // The frames really carry the token (the guard is not vacuous) ...
    assert!(identify.contains(MARKER) && identify.contains(r#""op":2"#));
    assert!(resume.contains(MARKER) && resume.contains(r#""op":6"#));
    // ... while the TRACE log, which did record this session's exchange, never does. Every test
    // shares the same marker token, so this also covers all other sessions in the process.
    let logs = String::from_utf8(capture().0.lock().unwrap().clone()).unwrap();
    let mine = format!("generation={generation} ");
    let has = |what: &str| logs.lines().any(|l| l.contains(&mine) && l.contains(what));
    assert!(
        has("send IDENTIFY") && has("send RESUME") && has("t=\"READY\""),
        "positive control failed: {logs}"
    );
    assert!(!logs.contains(MARKER), "token leaked into logs");
    assert!(!logs.contains("SYNTHETIC"));
}

// ---- Sidebar subscriptions (through the same mock gateway) ----

mod subscriptions {
    use super::*;
    use crate::sidebar_tests::{ch, guild, hidden, ready as world};
    use crate::{channel_categories, guild_entries, SidebarEntry, Subscription};

    const SETTLE: Duration = Duration::from_millis(300);

    /// Subscribes `compute`, forwarding every delivery to the returned channel.
    fn watch<T: PartialEq + Clone + Send + 'static>(
        rig: &Rig,
        compute: impl Fn(&crate::State) -> T + Send + 'static,
    ) -> (Subscription, mpsc::UnboundedReceiver<T>) {
        let (tx, rx) = mpsc::unbounded_channel();
        let sub = rig.handle.subscribe(compute, move |v| {
            let _ = tx.send(v);
        });
        (sub, rx)
    }

    async fn next<T>(rx: &mut mpsc::UnboundedReceiver<T>) -> T {
        timeout(WAIT, rx.recv())
            .await
            .expect("no delivery")
            .expect("subscription ended")
    }

    async fn silent<T>(rx: &mut mpsc::UnboundedReceiver<T>) -> bool {
        timeout(SETTLE, rx.recv()).await.is_err()
    }

    /// Accepts the connection, answers HELLO and IDENTIFY, and sends this READY.
    async fn send_ready(rig: &mut Rig, d: Value) -> Conn {
        let mut conn = Conn::accept(&rig.initial).await;
        conn.hello(30_000).await;
        assert_eq!(conn.recv().await["op"], 2);
        conn.send(json!({"op": 0, "t": "READY", "s": 1, "d": d}))
            .await;
        assert!(matches!(rig.event().await, SessionEvent::Ready { .. }));
        conn
    }

    fn dispatch(seq: u64, t: &str, d: Value) -> Value {
        json!({"op": 0, "t": t, "s": seq, "d": d})
    }

    fn in_guild(mut channel: Value, guild_id: u64) -> Value {
        channel["guild_id"] = json!(guild_id.to_string());
        channel
    }

    fn two_guilds() -> Value {
        world(
            vec![
                guild(
                    10,
                    vec![ch(20, 0, "general", 0, None), ch(21, 0, "extra", 1, None)],
                ),
                guild(11, vec![ch(30, 0, "other", 0, None)]),
            ],
            Value::Null,
            json!([]),
            vec![],
        )
    }

    fn names(cats: &[crate::SidebarCategory]) -> Vec<String> {
        cats.iter()
            .flat_map(|c| c.channels.iter().map(|c| c.name.clone()))
            .collect()
    }

    #[tokio::test]
    async fn subscribing_before_ready_gets_the_empty_snapshot_then_the_ready_one() {
        let mut rig = Rig::start().await;
        let (_sub, mut rx) = watch(&rig, guild_entries);
        assert_eq!(next(&mut rx).await, vec![]);
        let _conn = send_ready(&mut rig, two_guilds()).await;
        let after = next(&mut rx).await;
        assert_eq!(after.len(), 2);
        assert!(matches!(&after[0], SidebarEntry::Guild(g) if g.id == 10));
    }

    #[tokio::test]
    async fn subscribing_after_ready_gets_the_current_snapshot_immediately() {
        let mut rig = Rig::start().await;
        let _conn = send_ready(&mut rig, two_guilds()).await;
        let (_sub, mut rx) = watch(&rig, |s| channel_categories(s, 10));
        assert_eq!(names(&next(&mut rx).await), vec!["general", "extra"]);
        assert!(silent(&mut rx).await, "no repeat delivery");
    }

    #[tokio::test]
    async fn an_overwrite_that_hides_a_channel_emits_a_snapshot_without_it() {
        let mut rig = Rig::start().await;
        let mut conn = send_ready(&mut rig, two_guilds()).await;
        let (_sub, mut rx) = watch(&rig, |s| channel_categories(s, 10));
        assert_eq!(names(&next(&mut rx).await), vec!["general", "extra"]);
        let hide = in_guild(hidden(ch(21, 0, "extra", 1, None), 10), 10);
        conn.send(dispatch(2, "CHANNEL_UPDATE", hide)).await;
        assert_eq!(names(&next(&mut rx).await), vec!["general"]);
    }

    #[tokio::test]
    async fn a_channel_create_in_another_guild_does_not_fire_and_one_here_does() {
        let mut rig = Rig::start().await;
        let mut conn = send_ready(&mut rig, two_guilds()).await;
        let (_sub, mut rx) = watch(&rig, |s| channel_categories(s, 10));
        next(&mut rx).await;
        let mut revision = rig.handle.state();
        let before = *revision.borrow_and_update();
        let other = in_guild(ch(31, 0, "new-elsewhere", 5, None), 11);
        conn.send(dispatch(2, "CHANNEL_CREATE", other)).await;
        timeout(WAIT, revision.changed()).await.unwrap().unwrap();
        assert!(*revision.borrow() > before, "the revision did move");
        assert!(
            silent(&mut rx).await,
            "another guild's channel fired this subscription"
        );
        let mine = in_guild(ch(22, 0, "new-here", 5, None), 10);
        conn.send(dispatch(3, "CHANNEL_CREATE", mine)).await;
        assert_eq!(
            names(&next(&mut rx).await),
            vec!["general", "extra", "new-here"]
        );
    }

    #[tokio::test]
    async fn guild_delete_gives_an_empty_snapshot() {
        let mut rig = Rig::start().await;
        let mut conn = send_ready(&mut rig, two_guilds()).await;
        let (_sub, mut rx) = watch(&rig, |s| channel_categories(s, 10));
        assert!(!next(&mut rx).await.is_empty());
        conn.send(dispatch(2, "GUILD_DELETE", json!({"id": "10"})))
            .await;
        assert!(next(&mut rx).await.is_empty());
    }

    #[tokio::test]
    async fn a_dropped_subscription_stops_delivery() {
        let mut rig = Rig::start().await;
        let mut conn = send_ready(&mut rig, two_guilds()).await;
        let (sub, mut rx) = watch(&rig, |s| channel_categories(s, 10));
        next(&mut rx).await;
        drop(sub);
        let mut revision = rig.handle.state();
        revision.borrow_and_update();
        let mine = in_guild(ch(22, 0, "after-drop", 5, None), 10);
        conn.send(dispatch(2, "CHANNEL_CREATE", mine)).await;
        timeout(WAIT, revision.changed()).await.unwrap().unwrap();
        // Aborting the task drops its deliver closure, which closes the channel: nothing arrives.
        assert_eq!(timeout(WAIT, rx.recv()).await.unwrap(), None);
    }

    #[tokio::test]
    async fn a_subscription_ends_with_the_session() {
        let mut rig = Rig::start().await;
        let _conn = send_ready(&mut rig, two_guilds()).await;
        let (_sub, mut rx) = watch(&rig, guild_entries);
        next(&mut rx).await;
        rig.handle.disconnect();
        assert!(matches!(rig.event().await, SessionEvent::Closed { .. }));
        // The handle is still alive, yet the driver has returned and dropped its closure.
        assert_eq!(timeout(WAIT, rx.recv()).await.unwrap(), None);
    }
}

// ---- Member-list subscription (op 37), through the mock gateway ----

mod member_sub {
    use super::*;

    fn sub(guild: u64, typing: bool, channel: Option<u64>) -> Value {
        let channels = match channel {
            Some(c) => json!({ c.to_string(): [[0, 99]] }),
            None => json!({}),
        };
        json!({"op": 37, "d": {"subscriptions": {guild.to_string(): {"typing": typing,
            "threads": false, "activities": true, "member_updates": false, "members": [],
            "channels": channels, "thread_member_lists": []}}}})
    }

    /// Subscribe the guild, then ask for the channel's rows.
    async fn expect_subscribe(conn: &mut Conn, guild: u64, channel: u64) {
        assert_eq!(conn.recv_non_beat().await, sub(guild, true, None));
        assert_eq!(conn.recv_non_beat().await, sub(guild, true, Some(channel)));
    }

    #[tokio::test]
    async fn member_sub_set_before_ready_is_sent_once_after_ready() {
        let rig = Rig::start().await;
        rig.handle.set_member_subscription(Some((10, 20)));
        let mut conn = Conn::accept(&rig.initial).await;
        conn.hello(30_000).await;
        assert_eq!(conn.recv_non_beat().await["op"], 2);
        assert!(conn.quiet().await, "op 37 went out before READY");
        conn.send(ready(1, "wss://gateway-resume.discord.gg/"))
            .await;
        expect_subscribe(&mut conn, 10, 20).await;
        assert!(conn.quiet().await, "sent more than once");
    }

    #[tokio::test]
    async fn member_sub_is_resent_after_a_forced_reidentify() {
        let mut rig = Rig::start().await;
        let mut first = rig.ready(30_000, "wss://gateway-resume.discord.gg/").await;
        rig.handle.set_member_subscription(Some((10, 20)));
        expect_subscribe(&mut first, 10, 20).await;
        first.send(json!({"op": 9, "d": false})).await;
        let mut conn = Conn::accept(&rig.initial).await;
        conn.hello(30_000).await;
        assert_eq!(conn.recv_non_beat().await["op"], 2);
        conn.send(ready(1, "wss://gateway-resume.discord.gg/"))
            .await;
        expect_subscribe(&mut conn, 10, 20).await;
        assert!(conn.quiet().await);
    }

    #[tokio::test]
    async fn member_sub_is_resent_after_resumed() {
        let mut rig = Rig::start().await;
        rig.handle.set_member_subscription(Some((10, 20)));
        // 100 ms beats that the mock never ACKs: the session resumes at the resume endpoint.
        let _first = rig.ready(100, "wss://gateway-resume.discord.gg/").await;
        let mut conn = Conn::accept(&rig.resume).await;
        conn.hello(30_000).await;
        assert_eq!(conn.recv_non_beat().await["op"], 6);
        conn.send(json!({"op": 0, "t": "RESUMED", "s": 2, "d": {}}))
            .await;
        expect_subscribe(&mut conn, 10, 20).await;
        assert!(conn.quiet().await);
    }

    #[tokio::test]
    async fn member_sub_guild_switch_sends_a_leave_frame_for_the_old_guild() {
        let mut rig = Rig::start().await;
        let mut conn = rig.ready(30_000, "wss://gateway-resume.discord.gg/").await;
        rig.handle.set_member_subscription(Some((10, 20)));
        expect_subscribe(&mut conn, 10, 20).await;
        // Another channel in the same guild: ranges only.
        rig.handle.set_member_subscription(Some((10, 21)));
        assert_eq!(conn.recv_non_beat().await, sub(10, true, Some(21)));
        assert!(conn.quiet().await);
        // Another guild: leave the old one, then subscribe the new one.
        rig.handle.set_member_subscription(Some((11, 30)));
        assert_eq!(conn.recv_non_beat().await, sub(10, false, None));
        expect_subscribe(&mut conn, 11, 30).await;
        // Cleared (a DM or nothing selected): leave.
        rig.handle.set_member_subscription(None);
        assert_eq!(conn.recv_non_beat().await, sub(11, false, None));
        assert!(conn.quiet().await);
    }

    #[tokio::test]
    async fn two_quick_member_sub_sets_send_only_the_latest() {
        let mut rig = Rig::start().await;
        let mut conn = rig.ready(30_000, "wss://gateway-resume.discord.gg/").await;
        // No await in between: the loop wakes once and sees only the newest value.
        rig.handle.set_member_subscription(Some((10, 20)));
        rig.handle.set_member_subscription(Some((11, 30)));
        expect_subscribe(&mut conn, 11, 30).await;
        assert!(conn.quiet().await, "the first selection was sent too");
    }

    #[tokio::test]
    async fn member_sub_is_visible_to_a_dispatch_applied_right_after_the_call() {
        let mut rig = Rig::start().await;
        let mut conn = rig.ready(30_000, "wss://gateway-resume.discord.gg/").await;
        rig.handle.set_member_subscription(Some((10, 20)));
        // Before the session task has run at all, State already holds the selection.
        let set = rig
            .handle
            .snapshot()
            .member_sub
            .clone()
            .expect("not in State");
        assert_eq!((set.guild_id, set.channel_id), (10, 20));
        let mut revision = rig.handle.state();
        revision.borrow_and_update();
        conn.send(json!({"op": 0, "t": "GUILD_CREATE", "s": 2, "d": {"id": "10", "name": "g", "owner_id": "9", "roles": [], "channels": []}})).await;
        timeout(WAIT, revision.changed()).await.unwrap().unwrap();
        let applied = rig.handle.applied();
        let (name, seen) = applied.last().unwrap();
        assert_eq!(name, "GUILD_CREATE");
        assert_eq!(
            seen.as_ref(),
            Some(&set),
            "the dispatch did not see the new selection"
        );
    }

    #[tokio::test]
    async fn member_sub_survives_ready() {
        let mut rig = Rig::start().await;
        let mut conn = rig.ready(30_000, "wss://gateway-resume.discord.gg/").await;
        rig.handle.set_member_subscription(Some((10, 20)));
        let set = rig.handle.snapshot().member_sub.clone().unwrap();
        // A second READY rebuilds State (its guild list here) but keeps the selection.
        let mut again = ready(5, "wss://gateway-resume.discord.gg/");
        again["d"]["guilds"] = json!([{"id": "77", "name": "n", "channels": []}]);
        conn.send(again).await;
        assert!(matches!(rig.event().await, SessionEvent::Ready { .. }));
        let state = rig.handle.snapshot();
        assert!(state.guild(77).is_some(), "READY was not applied");
        assert_eq!(state.member_sub.as_ref(), Some(&set));
    }

    #[tokio::test]
    async fn setting_the_same_member_sub_again_sends_nothing_and_keeps_the_epoch() {
        let mut rig = Rig::start().await;
        let mut conn = rig.ready(30_000, "wss://gateway-resume.discord.gg/").await;
        rig.handle.set_member_subscription(Some((10, 20)));
        expect_subscribe(&mut conn, 10, 20).await;
        conn.send(json!({"op": 0, "t": "GUILD_MEMBER_LIST_UPDATE", "s": 2, "d": {
            "guild_id": "10", "id": "everyone", "member_count": 1, "online_count": 1,
            "groups": [{"id": "online", "count": 1}],
            "ops": [{"op": "SYNC", "range": [0, 99], "items": [{"group": {"id": "online", "count": 1}}]}]
        }}))
        .await;
        let before = rig.handle.snapshot();
        rig.handle.set_member_subscription(Some((10, 20)));
        let after = rig.handle.snapshot();
        assert_eq!(after.member_sub, before.member_sub, "the epoch moved");
        assert_eq!(after.revision, before.revision);
        assert!(
            conn.quiet().await,
            "a frame went out for the same selection"
        );
    }
}

// ---- Member list (GUILD_MEMBER_LIST_UPDATE), through the mock gateway ----

mod member_list {
    use super::*;
    use crate::member_list_tests::{header, member, sync, CHANNEL, GUILD};
    use crate::MemberListSection;

    fn watch(rig: &Rig, channel: u64) -> mpsc::UnboundedReceiver<Vec<MemberListSection>> {
        let (tx, rx) = mpsc::unbounded_channel();
        // Dropped with the session; the test ends first.
        std::mem::forget(rig.handle.subscribe_members(GUILD, channel, move |v| {
            let _ = tx.send(v);
        }));
        rx
    }

    async fn next(
        rx: &mut mpsc::UnboundedReceiver<Vec<MemberListSection>>,
    ) -> Vec<MemberListSection> {
        timeout(WAIT, rx.recv())
            .await
            .expect("no delivery")
            .expect("subscription ended")
    }

    /// The subscribe frame and the range frame for the selection.
    async fn two_frames(conn: &mut Conn) {
        assert_eq!(conn.recv_non_beat().await["op"], 37);
        assert_eq!(conn.recv_non_beat().await["op"], 37);
    }

    fn names(rows: &[MemberListSection]) -> Vec<&str> {
        rows.iter()
            .flat_map(|s| s.members.iter().map(|m| &*m.name))
            .collect()
    }

    #[tokio::test]
    async fn subscribe_members_then_a_sync_adopts_the_list() {
        let mut rig = Rig::start().await;
        let mut conn = rig.ready(30_000, "wss://gateway-resume.discord.gg/").await;
        let mut rx = watch(&rig, CHANNEL);
        assert!(
            next(&mut rx).await.is_empty(),
            "empty until a list is adopted"
        );
        two_frames(&mut conn).await;
        let items = vec![
            header("online", 1),
            member(1, "m1"),
            header("offline", 4),
            member(2, "m2"),
        ];
        conn.send(
            json!({"op": 0, "t": "GUILD_MEMBER_LIST_UPDATE", "s": 2, "d": sync("everyone", items)}),
        )
        .await;
        let rows = next(&mut rx).await;
        assert_eq!(names(&rows), ["m1", "m2"]);
        assert_eq!((rows[0].title.as_str(), rows[1].count), ("Online", 4));
    }

    #[tokio::test]
    async fn resumed_then_a_sync_with_the_same_id_leaves_the_list_filled() {
        let mut rig = Rig::start().await;
        let mut conn = rig.ready(30_000, "wss://gateway-resume.discord.gg/").await;
        let mut rx = watch(&rig, CHANNEL);
        next(&mut rx).await;
        two_frames(&mut conn).await;
        conn.send(
            json!({"op": 0, "t": "GUILD_MEMBER_LIST_UPDATE", "s": 2, "d":
            sync("everyone", vec![header("online", 1), member(1, "before")])}),
        )
        .await;
        assert_eq!(names(&next(&mut rx).await), ["before"]);
        // Opcode 7 makes the client reconnect and resume at the resume endpoint.
        conn.send(json!({"op": 7, "d": null})).await;
        let mut conn = Conn::accept(&rig.resume).await;
        conn.hello(30_000).await;
        assert_eq!(conn.recv_non_beat().await["op"], 6);
        conn.send(json!({"op": 0, "t": "RESUMED", "s": 3, "d": {}}))
            .await;
        two_frames(&mut conn).await;
        conn.send(
            json!({"op": 0, "t": "GUILD_MEMBER_LIST_UPDATE", "s": 4, "d":
            sync("everyone", vec![header("online", 1), member(2, "after")])}),
        )
        .await;
        assert_eq!(names(&next(&mut rx).await), ["after"]);
    }

    #[tokio::test]
    async fn selecting_another_channel_shows_no_rows_before_its_first_update() {
        let mut rig = Rig::start().await;
        let mut conn = rig.ready(30_000, "wss://gateway-resume.discord.gg/").await;
        let mut first = watch(&rig, CHANNEL);
        next(&mut first).await;
        two_frames(&mut conn).await;
        conn.send(
            json!({"op": 0, "t": "GUILD_MEMBER_LIST_UPDATE", "s": 2, "d":
            sync("everyone", vec![header("online", 1), member(1, "m1")])}),
        )
        .await;
        assert_eq!(names(&next(&mut first).await), ["m1"]);
        let mut second = watch(&rig, 21);
        assert!(
            next(&mut second).await.is_empty(),
            "the old channel's rows leaked"
        );
        // Only the channel-range frame goes out for another channel of the same guild.
        assert_eq!(conn.recv_non_beat().await["op"], 37);
        assert!(conn.quiet().await);
    }
}

// ---- Message-store observer slot ----

#[tokio::test]
async fn a_dispatch_reaches_the_message_store_observer() {
    let mut rig = Rig::start().await;
    let mut conn = rig.ready(30_000, "wss://gateway-resume.discord.gg/").await;
    let store = rig.handle.messages();
    assert!(
        Arc::ptr_eq(&store, &rig.handle.messages()),
        "one store per session"
    );
    // State ignores MESSAGE_CREATE (no revision bump); the observer still sees it.
    conn.send(json!({"op": 0, "t": "MESSAGE_CREATE", "s": 2, "d": {"id": "5", "channel_id": "20", "content": "hi"}}))
        .await;
    timeout(WAIT, async {
        while !store
            .seen
            .lock()
            .unwrap()
            .iter()
            .any(|n| n == "MESSAGE_CREATE")
        {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("the observer never saw the dispatch");
}

// ---- REST 401 through the session ----

mod rest_401 {
    use super::*;
    use crate::rest::{
        test_support::{response, MockHttp},
        RestError,
    };

    async fn unauthorized_mock() -> MockHttp {
        let mock = MockHttp::start(vec![response(401, "", "")]).await;
        mock.install();
        mock
    }

    async fn call(rig: &Rig) -> Result<Value, RestError> {
        rig.handle.rest().get("/users/@me").await
    }

    #[tokio::test]
    async fn a_rest_401_leaves_the_gateway_up_and_blocks_later_calls() {
        let mock = unauthorized_mock().await;
        let mut rig = Rig::start().await;
        let generation = rig.handle.generation();
        let mut conn = rig.ready(30_000, "wss://gateway-resume.discord.gg/").await;
        assert_eq!(call(&rig).await.unwrap_err(), RestError::Unauthorized);
        assert_eq!(
            rig.event().await,
            SessionEvent::RestUnauthorized { generation }
        );
        // The gateway carries on: a dispatch still lands, and no AuthFailed or reconnect follows.
        let mut revision = rig.handle.state();
        revision.borrow_and_update();
        conn.send(json!({"op": 0, "t": "GUILD_CREATE", "s": 2, "d": {"id": "10", "name": "g", "owner_id": "9", "roles": [], "channels": []}})).await;
        timeout(WAIT, revision.changed()).await.unwrap().unwrap();
        assert!(
            timeout(Duration::from_millis(400), rig.events.recv())
                .await
                .is_err(),
            "an event followed the REST 401"
        );
        assert_eq!(call(&rig).await.unwrap_err(), RestError::Unauthorized);
        assert_eq!(mock.count(), 1, "a call after the 401 reached the server");
    }

    #[tokio::test]
    async fn two_401s_from_two_rest_clones_give_one_event() {
        let mock = unauthorized_mock().await;
        let mut rig = Rig::start().await;
        let generation = rig.handle.generation();
        let _conn = rig.ready(30_000, "wss://gateway-resume.discord.gg/").await;
        let (a, b) = (rig.handle.rest(), rig.handle.rest());
        // Both pass the latch check before either answer arrives.
        let (ra, rb) = tokio::join!(a.get::<Value>("/users/@me"), b.get::<Value>("/users/@me"));
        assert_eq!(
            (ra.unwrap_err(), rb.unwrap_err()),
            (RestError::Unauthorized, RestError::Unauthorized)
        );
        assert_eq!(mock.count(), 2, "both requests reached the server");
        assert_eq!(
            rig.event().await,
            SessionEvent::RestUnauthorized { generation }
        );
        assert!(
            timeout(Duration::from_millis(400), rig.events.recv())
                .await
                .is_err(),
            "a second RestUnauthorized"
        );
    }

    #[tokio::test]
    async fn a_401_after_disconnect_emits_nothing() {
        let mock = unauthorized_mock().await;
        let mut rig = Rig::start().await;
        let generation = rig.handle.generation();
        let _conn = rig.ready(30_000, "wss://gateway-resume.discord.gg/").await;
        rig.handle.disconnect();
        assert_eq!(rig.event().await, SessionEvent::Closed { generation });
        assert_eq!(call(&rig).await.unwrap_err(), RestError::Unauthorized);
        assert_eq!(mock.count(), 1);
        // The task ended and dropped its callback: the channel closes with nothing after Closed.
        assert_eq!(timeout(WAIT, rig.events.recv()).await.unwrap(), None);
    }
}

// ---- Messages through the session (plan M) ----

mod messages_live {
    use super::*;
    use crate::{
        rest::test_support::{response, MockHttp},
        LoadState, MessageSubscription, MessagesSnapshot,
    };

    fn ok(body: Value) -> String {
        response(200, "Content-Type: application/json\r\n", &body.to_string())
    }

    fn dispatch(seq: u64, t: &str, d: Value) -> Value {
        json!({"op": 0, "t": t, "s": seq, "d": d})
    }

    /// Snapshots until one satisfies `done`.
    async fn until(
        rx: &mut mpsc::UnboundedReceiver<MessagesSnapshot>,
        done: impl Fn(&MessagesSnapshot) -> bool,
    ) -> MessagesSnapshot {
        loop {
            let s = timeout(WAIT, rx.recv())
                .await
                .expect("no snapshot")
                .expect("deliveries ended");
            if done(&s) {
                return s;
            }
        }
    }

    #[tokio::test]
    async fn history_live_events_and_a_send_flow_through_the_session() {
        let history = json!([
            {"id": "11", "channel_id": "20", "content": "second", "author": {"id": "9", "username": "u9", "global_name": "Nine"}},
            {"id": "10", "channel_id": "20", "content": "first <#100000000000000022>", "author": {"id": "9", "username": "u9"}},
        ]);
        let sent = json!({"id": "14", "channel_id": "20", "content": "out",
            "author": {"id": "1", "username": "synthetic", "global_name": "Syn"}});
        let mock = MockHttp::start(vec![ok(history), ok(sent)]).await;
        mock.install();
        let mut rig = Rig::start().await;
        let mut conn = rig.ready(30_000, "wss://gateway-resume.discord.gg/").await;
        conn.send(dispatch(2, "GUILD_CREATE", json!({
            "id": "10", "name": "g", "owner_id": "1",
            "roles": [{"id": "10", "permissions": "1024", "position": 0},
                      {"id": "100000000000000077", "permissions": "0", "position": 3, "name": "mods", "color": 0x112233}],
            "channels": [{"id": "20", "type": 0, "name": "general"},
                         {"id": "100000000000000022", "type": 0, "name": "lounge"}],
        })))
        .await;
        let mut revision = rig.handle.state();
        timeout(WAIT, revision.wait_for(|r| *r >= 2))
            .await
            .unwrap()
            .unwrap();

        let (tx, mut rx) = mpsc::unbounded_channel();
        let sub = rig.handle.subscribe_messages(20, move |s| {
            let _ = tx.send(s);
        });
        let s = until(&mut rx, |s| s.load == LoadState::Loaded).await;
        assert_eq!(
            s.rows
                .iter()
                .map(|r| (r.id, r.content.as_str()))
                .collect::<Vec<_>>(),
            [(10, "first <#100000000000000022>"), (11, "second")]
        );
        // Mentions resolve against the state into side data.
        let channel = crate::ResolvedMention {
            kind: crate::MentionKind::Channel,
            id: 100_000_000_000_000_022,
            name: Some("lounge".into()),
            color: 0,
        };
        assert_eq!(s.rows[0].mentions, [channel]);

        // Live: a create with a member (nick, role colour), one for another channel (ignored),
        // then an update and a delete.
        conn.send(dispatch(3, "MESSAGE_CREATE", json!({
            "id": "12", "channel_id": "20", "content": "hey <@&100000000000000077>",
            "author": {"id": "9", "username": "u9"}, "member": {"nick": "Niner", "roles": ["100000000000000077"]},
        })))
        .await;
        conn.send(dispatch(4, "MESSAGE_CREATE", json!({
            "id": "13", "channel_id": "21", "content": "elsewhere", "author": {"id": "9", "username": "u9"},
        })))
        .await;
        let s = until(&mut rx, |s| s.rows.len() == 3).await;
        let live = &s.rows[2];
        assert_eq!(
            (live.author_name.as_str(), live.author_color),
            ("Niner", Some(0x112233))
        );
        let role = crate::ResolvedMention {
            kind: crate::MentionKind::Role,
            id: 100_000_000_000_000_077,
            name: Some("mods".into()),
            color: 0x112233,
        };
        assert_eq!(live.mentions, std::slice::from_ref(&role));
        conn.send(dispatch(5, "MESSAGE_UPDATE", json!({"id": "11", "channel_id": "20", "content": "edited", "edited_timestamp": "2026-10-14T00:00:00+00:00"}))).await;
        conn.send(dispatch(
            6,
            "MESSAGE_DELETE",
            json!({"id": "10", "channel_id": "20"}),
        ))
        .await;
        let s = until(&mut rx, |s| s.rows.len() == 2 && s.rows[0].edited).await;
        assert_eq!(
            s.rows
                .iter()
                .map(|r| (r.id, r.content.as_str()))
                .collect::<Vec<_>>(),
            [(11, "edited"), (12, "hey <@&100000000000000077>")]
        );
        assert_eq!(s.rows[1].mentions, [role]);

        // Send: only to the open channel, never for blank text; the gateway echo reconciles.
        assert!(rig
            .handle
            .send_message(21, "wrong channel".into(), None)
            .is_none());
        assert!(rig.handle.send_message(20, " \n\t".into(), None).is_none());
        rig.handle
            .send_message(20, "out".into(), None)
            .unwrap()
            .await
            .unwrap();
        let posts: Vec<_> = mock
            .requests()
            .into_iter()
            .filter(|r| r.starts_with("POST"))
            .collect();
        assert_eq!(posts.len(), 1);
        assert!(
            posts[0].starts_with("POST /api/v9/channels/20/messages "),
            "{}",
            posts[0]
        );
        let body: Value = serde_json::from_str(posts[0].split("\r\n\r\n").nth(1).unwrap()).unwrap();
        let nonce = body["nonce"].as_str().unwrap().to_owned();
        // The POST answer replaces the echo (the echo itself is covered in messages_tests.rs), and
        // the gateway's copy of the same message (same nonce, self author) adds no second row.
        until(&mut rx, |s| {
            s.rows.last().is_some_and(|r| r.id == 14 && !r.pending)
        })
        .await;
        conn.send(dispatch(
            7,
            "MESSAGE_CREATE",
            json!({
                "id": "14", "channel_id": "20", "content": "out", "nonce": nonce,
                "author": {"id": "1", "username": "synthetic", "global_name": "Syn"},
                "member": {"nick": "Me", "roles": []},
            }),
        ))
        .await;
        let s = until(&mut rx, |s| {
            s.rows.last().is_some_and(|r| r.author_name == "Me")
        })
        .await;
        assert_eq!(
            s.rows.iter().map(|r| (r.id, r.pending)).collect::<Vec<_>>(),
            [(11, false), (12, false), (14, false)]
        );

        // Leaving ends the deliveries.
        drop(sub);
        conn.send(dispatch(8, "MESSAGE_CREATE", json!({
            "id": "15", "channel_id": "20", "content": "late", "author": {"id": "9", "username": "u9"},
        })))
        .await;
        assert!(
            matches!(
                timeout(Duration::from_millis(400), rx.recv()).await,
                Err(_) | Ok(None)
            ),
            "a snapshot after the subscription was dropped"
        );
    }

    async fn open_general(
        rig: &mut Rig,
    ) -> (
        Conn,
        mpsc::UnboundedReceiver<MessagesSnapshot>,
        MessageSubscription,
    ) {
        let mut conn = rig.ready(30_000, "wss://gateway-resume.discord.gg/").await;
        conn.send(dispatch(
            2,
            "GUILD_CREATE",
            json!({
                "id": "10", "name": "g", "owner_id": "1",
                "roles": [{"id": "10", "permissions": "1024", "position": 0}],
                "channels": [{"id": "20", "type": 0, "name": "general"}],
            }),
        ))
        .await;
        let mut revision = rig.handle.state();
        timeout(WAIT, revision.wait_for(|r| *r >= 2))
            .await
            .unwrap()
            .unwrap();
        let (tx, rx) = mpsc::unbounded_channel();
        let sub = rig.handle.subscribe_messages(20, move |s| {
            let _ = tx.send(s);
        });
        (conn, rx, sub)
    }

    #[tokio::test]
    async fn a_failed_send_through_the_session_posts_once_and_shows_not_sent() {
        let mock = MockHttp::start(vec![ok(json!([])), response(500, "", ""), ok(json!({}))]).await;
        mock.install();
        let mut rig = Rig::start().await;
        let (_conn, mut rx, _sub) = open_general(&mut rig).await;
        until(&mut rx, |s| s.load == LoadState::Loaded).await;
        rig.handle
            .send_message(20, "hi".into(), None)
            .unwrap()
            .await
            .unwrap();
        let s = until(&mut rx, |s| s.rows.last().is_some_and(|r| !r.pending)).await;
        assert_eq!(s.rows.last().unwrap().failure, Some("Not sent"));
        let posts = mock
            .requests()
            .into_iter()
            .filter(|r| r.starts_with("POST"))
            .count();
        assert_eq!(posts, 1, "the POST was retried");
    }

    /// `MessageSubscription::load_older` (the call the FFI makes) runs one older GET and the page
    /// arrives as a snapshot with the rows prepended.
    #[tokio::test]
    async fn load_older_through_the_subscription_prepends_one_page() {
        let first: Vec<Value> = (51..=100u64)
            .rev()
            .map(|id| json!({"id": id.to_string(), "channel_id": "20", "content": "new", "author": {"id": "9", "username": "u9"}}))
            .collect();
        let older = json!([{"id": "50", "channel_id": "20", "content": "old", "author": {"id": "9", "username": "u9"}}]);
        let mock = MockHttp::start(vec![ok(Value::Array(first)), ok(older)]).await;
        mock.install();
        let mut rig = Rig::start().await;
        let (_conn, mut rx, sub) = open_general(&mut rig).await;
        let s = until(&mut rx, |s| s.load == LoadState::Loaded).await;
        assert_eq!((s.rows.len(), s.older), (50, crate::OlderState::Idle));
        sub.load_older();
        let s = until(&mut rx, |s| s.older == crate::OlderState::ReachedStart).await;
        assert_eq!((s.rows.len(), s.rows[0].id), (51, 50));
        let requests = mock.requests();
        assert_eq!(requests.len(), 2);
        assert!(requests[1].starts_with("GET /api/v9/channels/20/messages?limit=50&before=51 "));
    }

    /// TYPING_START reaches the store through the session with the state `apply` left: the self
    /// user (1, from READY) is ignored, a named user shows, and that user's message ends it.
    #[tokio::test]
    async fn typing_start_flows_through_the_session_into_the_snapshot() {
        let mock = MockHttp::start(vec![ok(json!([]))]).await;
        mock.install();
        let mut rig = Rig::start().await;
        let (mut conn, mut rx, _sub) = open_general(&mut rig).await;
        until(&mut rx, |s| s.load == LoadState::Loaded).await;
        let typing = |seq, user: &str, channel: &str| {
            dispatch(
                seq,
                "TYPING_START",
                json!({
                    "channel_id": channel, "user_id": user, "timestamp": 1,
                    "member": {"user": {"id": user, "username": "u9"}, "nick": "Niner"},
                }),
            )
        };
        conn.send(typing(3, "1", "20")).await;
        conn.send(typing(4, "9", "21")).await;
        conn.send(typing(5, "9", "20")).await;
        let s = until(&mut rx, |s| !s.typing.is_empty()).await;
        assert_eq!(s.typing, ["Niner"]);
        conn.send(dispatch(6, "MESSAGE_CREATE", json!({
            "id": "30", "channel_id": "20", "content": "hi", "author": {"id": "9", "username": "u9"},
        })))
        .await;
        let s = until(&mut rx, |s| s.rows.len() == 1).await;
        assert!(s.typing.is_empty());
    }

    /// The store learns the self user from the session state: a reaction event from user 1 (the
    /// READY user) sets `me`, one from another user does not.
    #[tokio::test]
    async fn reaction_events_set_me_from_the_session_user() {
        let history = json!([{"id": "11", "channel_id": "20", "content": "x",
            "author": {"id": "9", "username": "u9"},
            "reactions": [{"count": 1, "me": false, "emoji": {"id": null, "name": "\u{1f44d}"}}]}]);
        let mock = MockHttp::start(vec![ok(history)]).await;
        mock.install();
        let mut rig = Rig::start().await;
        let (mut conn, mut rx, _sub) = open_general(&mut rig).await;
        let s = until(&mut rx, |s| s.load == LoadState::Loaded).await;
        assert_eq!(s.rows[0].reactions.len(), 1);
        let event = |seq, user: &str, name: &str| {
            dispatch(
                seq,
                name,
                json!({
                    "user_id": user, "channel_id": "20", "message_id": "11",
                    "emoji": {"id": null, "name": "\u{1f44d}"},
                }),
            )
        };
        conn.send(event(3, "9", "MESSAGE_REACTION_ADD")).await;
        let s = until(&mut rx, |s| s.rows[0].reactions[0].count == 2).await;
        assert!(!s.rows[0].reactions[0].me);
        conn.send(event(4, "1", "MESSAGE_REACTION_ADD")).await;
        let s = until(&mut rx, |s| s.rows[0].reactions[0].count == 3).await;
        assert!(s.rows[0].reactions[0].me);
    }

    #[tokio::test]
    async fn nothing_is_sent_before_ready_even_to_the_open_channel() {
        let mock = MockHttp::start(vec![ok(json!([]))]).await;
        mock.install();
        let rig = Rig::start().await;
        let (tx, mut rx) = mpsc::unbounded_channel();
        let _sub = rig.handle.subscribe_messages(20, move |s| {
            let _ = tx.send(s);
        });
        until(&mut rx, |s| s.load == LoadState::Loaded).await;
        assert!(rig.handle.send_message(20, "hi".into(), None).is_none());
        assert_eq!(mock.count(), 1, "only the history GET");
    }

    /// Selection plan C1 Reply: the menu's per-message preview call flattens one buffered message
    /// by core's spoiler-safe rules, so hidden spoiler text never reaches the reply bar; it answers
    /// only for its own, still open channel.
    #[tokio::test]
    async fn the_per_message_preview_never_returns_spoiler_text() {
        let author = json!({"id": "9", "username": "u9"});
        let history = json!([
            {"id": "12", "channel_id": "20", "content": "||```secret```||", "author": author},
            {"id": "11", "channel_id": "20", "content": "a ||secret|| b", "author": author},
        ]);
        let mock = MockHttp::start(vec![ok(history), ok(json!([]))]).await;
        mock.install();
        let rig = Rig::start().await;
        let (tx, mut rx) = mpsc::unbounded_channel();
        let sub = rig.handle.subscribe_messages(20, move |s| {
            let _ = tx.send(s);
        });
        until(&mut rx, |s| s.load == LoadState::Loaded).await;
        let placeholder = "\u{2592}\u{2592}\u{2592}";
        assert_eq!(
            sub.preview(11).as_deref(),
            Some(&*format!("a {placeholder} b"))
        );
        assert_eq!(sub.preview(12).as_deref(), Some(placeholder));
        for id in [11, 12] {
            assert!(!sub.preview(id).unwrap().contains("secret"));
        }
        assert_eq!(sub.preview(99), None, "not buffered");
        // Another channel opened: the first subscription answers nothing.
        let (tx2, mut rx2) = mpsc::unbounded_channel();
        let _other = rig.handle.subscribe_messages(21, move |s| {
            let _ = tx2.send(s);
        });
        until(&mut rx2, |s| s.load == LoadState::Loaded).await;
        assert_eq!(sub.preview(11), None);
    }

    /// D and M both read MESSAGE_CREATE: the state (unread, mentions) and the message buffer.
    #[tokio::test]
    async fn one_message_create_reaches_both_the_sidebar_state_and_the_buffer() {
        let mock = MockHttp::start(vec![ok(json!([]))]).await;
        mock.install();
        let mut rig = Rig::start().await;
        let (mut conn, mut rx, _sub) = open_general(&mut rig).await;
        until(&mut rx, |s| s.load == LoadState::Loaded).await;
        let mentions = |rig: &Rig| {
            crate::channel_categories(&rig.handle.snapshot(), 10)
                .iter()
                .flat_map(|c| &c.channels)
                .find(|c| c.id == 20)
                .map(|c| c.mention_count)
        };
        assert_eq!(mentions(&rig), Some(0));

        conn.send(dispatch(
            3,
            "MESSAGE_CREATE",
            json!({
                "id": "30", "channel_id": "20", "guild_id": "10", "content": "hi <@1>",
                "author": {"id": "9", "username": "u9"},
                "mentions": [{"id": "1", "username": "synthetic", "global_name": "Syn"}],
            }),
        ))
        .await;
        // The store observes a dispatch after `State::apply`, so once the row shows, the state
        // has applied the same event.
        let s = until(&mut rx, |s| !s.rows.is_empty()).await;
        assert_eq!(
            s.rows
                .iter()
                .map(|r| (r.id, r.content.as_str()))
                .collect::<Vec<_>>(),
            [(30, "hi <@1>")]
        );
        assert_eq!(mentions(&rig), Some(1));
    }
}

/// Slice ID-b: the connection's identity (menu plan ID: IDENTIFY, QoS, Presence, Token refresh,
/// Identity fetch, Local identity store, the ID-a leftovers).
mod identity {
    use super::*;
    use crate::{
        identify::{qos_token, Launch, LaunchConfig},
        identity_store::FetchCache,
        rest::test_support::{response, MockHttp},
        settings_proto::tests::{settings, status_field},
        transport::Method,
        IdentityEvent, IdentityNotice, SessionOptions,
    };
    use base64::Engine as _;

    const MAIN: u64 = 1;
    const ALT: u64 = 2;
    const RESUME_URL: &str = "wss://gateway-resume.discord.gg/";
    const HTML: &str = include_str!("../tests/fixtures/identity/app.html");
    const CHUNK: &str = include_str!("../tests/fixtures/identity/fast-connect.js");

    fn launch(fetch: bool) -> Arc<Launch> {
        Launch::new(LaunchConfig {
            safari_version: Some("27.0.1".into()),
            preferred_languages: vec!["zh-Hant-TW".into()],
            store_dir: None,
            fetch,
        })
    }

    fn opts(launch: &Arc<Launch>, expected_user: Option<u64>) -> SessionOptions {
        SessionOptions {
            launch: launch.clone(),
            expected_user,
            identity_events: None,
        }
    }

    fn ready_with(extra: Value) -> Value {
        let mut r = ready(1, RESUME_URL);
        for (k, v) in extra.as_object().unwrap() {
            r["d"][k] = v.clone();
        }
        r
    }

    fn sorted_keys(v: &Value) -> Vec<String> {
        v.as_object().unwrap().keys().cloned().collect()
    }

    async fn identify(conn: &mut Conn) -> Value {
        let p = conn.recv_non_beat().await;
        assert_eq!(p["op"], 2, "{p}");
        p
    }

    impl Rig {
        async fn ready_event(&mut self) {
            loop {
                match self.event().await {
                    SessionEvent::Reconnecting { .. } => continue,
                    SessionEvent::Ready { .. } => return,
                    other => panic!("expected Ready, got {other:?}"),
                }
            }
        }
    }

    fn decoded_super_properties(rig: &Rig) -> Value {
        let raw = base64::engine::general_purpose::STANDARD
            .decode(rig.handle.rest().identity().super_properties())
            .unwrap();
        serde_json::from_slice(&raw).unwrap()
    }

    fn now() -> u64 {
        crate::identify::now_ms()
    }

    /// The launch's first IDENTIFY is the fast-connect shape with its own launch id; the next
    /// (after a connection that died before READY) the normal shape with COLD_START; after a
    /// READY, NO_REASONS.
    #[tokio::test]
    async fn fast_then_normal_identify_shapes() {
        let l = launch(false);
        let mut rig = Rig::start_with(opts(&l, None)).await;
        let mut c1 = Conn::accept(&rig.initial).await;
        c1.hello(30_000).await;
        let fast = identify(&mut c1).await;
        assert_eq!(
            sorted_keys(&fast["d"]),
            ["capabilities", "client_state", "properties", "token"]
        );
        let props = &fast["d"]["properties"];
        assert_eq!(props["is_fast_connect"], true);
        assert!(props.get("launch_signature").is_none());
        assert_ne!(props["client_launch_id"], l.launch_id());
        assert_eq!(fast["d"]["capabilities"], 1_734_653);
        assert_eq!(fast["d"]["client_state"], json!({"guild_versions": {}}));
        c1.close(4009).await;
        let mut c2 = Conn::accept(&rig.initial).await;
        c2.hello(30_000).await;
        let normal = identify(&mut c2).await;
        assert_eq!(
            sorted_keys(&normal["d"]),
            [
                "capabilities",
                "client_state",
                "compress",
                "properties",
                "qos_token",
                "token"
            ]
        );
        let props = &normal["d"]["properties"];
        assert_eq!(props["is_fast_connect"], false);
        assert_eq!(props["gateway_connect_reasons"], "COLD_START");
        assert_eq!(props["client_app_state"], "focused");
        assert_eq!(props["launch_signature"], Value::Null);
        assert_eq!(props["client_launch_id"], l.launch_id());
        assert_eq!(props["system_locale"], "zh-TW");
        assert_eq!(normal["d"]["compress"], false);
        // No stored QoS for this (unknown) account: only `client_provided`.
        assert_eq!(normal["d"]["qos_token"], qos_token(false, None));
        // The gateway upgrade carries this Mac's Accept-Language, never NET's constant.
        assert_eq!(
            c2.end.request.headers()[1],
            (
                "Accept-Language".to_owned(),
                "zh-TW,zh-Hant;q=0.9".to_owned()
            )
        );
        c2.send(ready(1, RESUME_URL)).await;
        rig.ready_event().await;
        c2.send(json!({"op": 9, "d": false})).await;
        let mut c3 = Conn::accept(&rig.initial).await;
        c3.hello(30_000).await;
        let again = identify(&mut c3).await;
        assert_eq!(
            again["d"]["properties"]["gateway_connect_reasons"],
            "NO_REASONS"
        );
    }

    /// Op 40 with `qos` null until the first activity report, then the report; a gateway op 1 is
    /// answered with op 40. Op 1 never goes out.
    #[tokio::test]
    async fn heartbeats_are_op_40_and_never_op_1() {
        let mut rig = Rig::start().await;
        let mut conn = rig.ready(30_000, RESUME_URL).await;
        conn.send(json!({"op": 1, "d": null})).await;
        loop {
            let beat = conn.recv().await;
            assert_eq!(beat["op"], 40, "{beat}");
            if beat["d"]["seq"] == 1 {
                assert_eq!(beat, json!({"op": 40, "d": {"seq": 1, "qos": null}}));
                break;
            }
        }
        conn.send(json!({"op": 11})).await;
        rig.handle.set_app_focused(false);
        tokio::time::sleep(Duration::from_millis(100)).await;
        conn.send(json!({"op": 1, "d": null})).await;
        // A jittered tick may have queued one more report-less beat before the focus change.
        let beat = loop {
            let beat = conn.recv().await;
            assert_eq!(beat["op"], 40, "{beat}");
            if !beat["d"]["qos"].is_null() {
                break beat;
            }
        };
        assert_eq!(
            beat,
            json!({"op": 40, "d": {"seq": 1, "qos": {"active": false, "ver": 31, "reasons": []}}})
        );
        conn.send(json!({"op": 11})).await;
        // Unfocused: no heartbeat session, so no op 41 either.
        assert!(conn.quiet().await);
    }

    /// Active at READY: a new heartbeat session, announced with op 41 (launch id of REST) and one
    /// heartbeat, and carried in X-Super-Properties; stored for this account only.
    #[tokio::test]
    async fn op_41_announces_the_accounts_heartbeat_session() {
        let l = launch(false);
        let mut rig = Rig::start_with(opts(&l, None)).await;
        rig.handle.set_app_focused(true);
        let mut conn = rig.ready(30_000, RESUME_URL).await;
        let op41 = conn.recv_non_beat().await;
        assert_eq!(op41["op"], 41, "{op41}");
        assert_eq!(
            sorted_keys(&op41["d"]),
            ["client_launch_id", "initialization_timestamp", "session_id"]
        );
        assert_eq!(op41["d"]["client_launch_id"], l.launch_id());
        let sid = op41["d"]["session_id"].as_str().unwrap().to_owned();
        assert_eq!(sid.len(), 36);
        let beat = conn.recv().await;
        assert_eq!(beat["op"], 40);
        assert_eq!(
            beat["d"]["qos"],
            json!({"active": true, "ver": 31, "reasons": ["foregrounded"]})
        );
        let props = decoded_super_properties(&rig);
        assert_eq!(props["client_heartbeat_session_id"], sid.as_str());
        assert_eq!(props["client_app_state"], "focused");
        let stored = l.store.heartbeat_session(MAIN, now()).unwrap();
        assert_eq!(stored.uuid, sid);
        assert_eq!(op41["d"]["initialization_timestamp"], stored.created_at_ms);
        assert!(l.store.heartbeat_session(ALT, now()).is_none());

        // The saved account's token on the same launch keeps the live session (announced again);
        // a token from the login window (a token change) starts a new one.
        let mut kept = Rig::start_with(opts(&l, Some(MAIN))).await;
        kept.handle.set_app_focused(true);
        let mut conn = kept.ready(30_000, RESUME_URL).await;
        assert_eq!(conn.recv_non_beat().await["d"]["session_id"], sid.as_str());
        let mut fresh = Rig::start_with(opts(&l, None)).await;
        fresh.handle.set_app_focused(true);
        let mut conn = fresh.ready(30_000, RESUME_URL).await;
        assert_ne!(conn.recv_non_beat().await["d"]["session_id"], sid.as_str());
    }

    /// Presence: none while the settings are unknown; READY's settings give op 3 at once, and a
    /// settings change another; an unknown status sends nothing and leaves IDENTIFY without it.
    #[tokio::test]
    async fn presence_follows_the_settings_and_is_omitted_while_unknown() {
        let mut rig = Rig::start().await;
        let mut conn = Conn::accept(&rig.initial).await;
        conn.hello(30_000).await;
        identify(&mut conn).await;
        let mut loc = crate::proto::Message::new();
        loc.push_wrapper_string(1, "ja");
        let proto = settings(&[(11, status_field("dnd", None)), (12, loc)]);
        conn.send(ready_with(json!({"user_settings_proto": proto})))
            .await;
        rig.ready_event().await;
        assert_eq!(
            conn.recv_non_beat().await,
            json!({"op": 3, "d": {"status": "dnd", "since": 0, "activities": [], "afk": false}})
        );
        // The account's Discord locale becomes X-Discord-Locale.
        assert_eq!(&*rig.handle.rest().identity().locale(), "ja");
        let update = |s: &str, t: u64| {
            json!({"op": 0, "t": "USER_SETTINGS_PROTO_UPDATE", "s": 2, "d": {
                "settings": {"type": t, "proto": settings(&[(11, status_field(s, None))])},
                "partial": true}})
        };
        conn.send(update("idle", 1)).await;
        assert_eq!(conn.recv_non_beat().await["d"]["status"], "idle");
        // Type 2 belongs to frecency (slice A): nothing changes, nothing is sent.
        conn.send(update("online", 2)).await;
        assert!(conn.quiet().await);
        assert_eq!(
            rig.handle
                .snapshot()
                .user_settings
                .as_ref()
                .unwrap()
                .status
                .as_deref(),
            Some("idle")
        );
        // A status outside the closed set: unknown, so no op 3 and no IDENTIFY presence.
        conn.send(update("offline", 1)).await;
        assert!(conn.quiet().await);
        conn.send(json!({"op": 9, "d": false})).await;
        let mut again = Conn::accept(&rig.initial).await;
        again.hello(30_000).await;
        let normal = identify(&mut again).await;
        assert!(normal["d"].get("presence").is_none(), "{normal}");

        // A plain READY (no settings proto): no op 3 at all.
        let mut plain = Rig::start().await;
        let mut conn = plain.ready(30_000, RESUME_URL).await;
        assert!(conn.quiet().await);
    }

    /// Main's stored installation id and QoS never go out for alt; they do for main.
    #[tokio::test]
    async fn per_account_values_are_never_sent_for_another_account() {
        let l = launch(false);
        l.store.adopt_installation_id(MAIN, "main.inst");
        let qos = crate::identity_store::tests::qos_b64();
        l.store.set_qos_derived(MAIN, &qos);
        // Alt's saved token: the fast IDENTIFY and the normal one carry nothing of main's.
        let mut alt = Rig::start_with(opts(&l, Some(ALT))).await;
        let mut c = Conn::accept(&alt.initial).await;
        c.hello(30_000).await;
        let fast = identify(&mut c).await;
        assert!(fast["d"]["properties"].get("installation_id").is_none());
        c.close(4009).await;
        let mut c = Conn::accept(&alt.initial).await;
        c.hello(30_000).await;
        let normal = identify(&mut c).await;
        assert!(normal["d"]["properties"].get("installation_id").is_none());
        assert_eq!(normal["d"]["qos_token"], qos_token(false, None));
        // Alt's READY: its own installation id and QoS are adopted, main's stay.
        c.send(ready_with(json!({
            "user": {"id": "2", "username": "alt"},
            "apex_experiments": {"assignments": {}, "installation": "alt.inst"},
            "qos_token": qos,
        })))
        .await;
        alt.ready_event().await;
        tokio::time::sleep(Duration::from_millis(50)).await;
        assert_eq!(
            alt.handle.rest().identity().installation_id().as_deref(),
            Some("alt.inst")
        );
        assert_eq!(l.store.installation_id(ALT).as_deref(), Some("alt.inst"));
        assert_eq!(l.store.installation_id(MAIN).as_deref(), Some("main.inst"));
        // Main's saved token: its values go out (the launch's fast IDENTIFY is already used).
        let main = Rig::start_with(opts(&l, Some(MAIN))).await;
        let mut c = Conn::accept(&main.initial).await;
        c.hello(30_000).await;
        let normal = identify(&mut c).await;
        assert_eq!(normal["d"]["properties"]["installation_id"], "main.inst");
        let derived = crate::proto::base64_decode(&qos).unwrap();
        assert_eq!(normal["d"]["qos_token"], qos_token(false, Some(&derived)));
    }

    /// READY `auth_token`: validated, the slot replaced after READY, the app told after Ready with
    /// a one-shot token; the next IDENTIFY and every REST request use it (I-1, T-8).
    #[tokio::test]
    async fn a_refreshed_token_goes_to_the_slot_and_the_app_after_ready() {
        const FRESH: &str = "SYNTHETIC_REFRESHED_TOKEN_0123456789";
        let mut rig = Rig::start_with(opts(&launch(false), Some(MAIN))).await;
        let generation = rig.handle.generation();
        let mut conn = Conn::accept(&rig.initial).await;
        conn.hello(30_000).await;
        identify(&mut conn).await;
        conn.send(ready_with(json!({"auth_token": FRESH}))).await;
        rig.ready_event().await;
        let event = timeout(WAIT, rig.identity.recv()).await.unwrap().unwrap();
        assert!(!format!("{event:?}").contains("SYNTHETIC"), "{event:?}");
        let IdentityEvent::TokenRefreshed {
            generation: g,
            user_id,
            token,
        } = event
        else {
            panic!("expected TokenRefreshed");
        };
        assert_eq!((g, user_id), (generation, MAIN));
        assert_eq!(token.take().as_deref(), Some(FRESH));
        assert_eq!(token.take(), None, "one-shot");
        assert_eq!(*rig.order.lock().unwrap(), ["Ready", "TokenRefreshed"]);
        let url = url::Url::parse("https://discord.com/api/v9/users/@me").unwrap();
        let request = rig
            .handle
            .rest()
            .build_request(Method::Get, &url, None)
            .unwrap();
        assert_eq!(request.authorization().unwrap().reveal(), FRESH);
        // Refresh, then op 9: the next IDENTIFY carries the new token.
        conn.send(json!({"op": 9, "d": false})).await;
        let mut again = Conn::accept(&rig.initial).await;
        again.hello(30_000).await;
        assert_eq!(identify(&mut again).await["d"]["token"], FRESH);
    }

    #[tokio::test]
    async fn an_invalid_or_equal_auth_token_is_ignored() {
        for bad in ["short", "Bot SYNTHETIC_TOKEN_MARKER_0123456789", MARKER] {
            let mut rig = Rig::start_with(opts(&launch(false), Some(MAIN))).await;
            let mut conn = Conn::accept(&rig.initial).await;
            conn.hello(30_000).await;
            identify(&mut conn).await;
            conn.send(ready_with(json!({"auth_token": bad}))).await;
            rig.ready_event().await;
            assert!(conn.quiet().await);
            assert!(rig.identity.try_recv().is_err(), "{bad}");
            let url = url::Url::parse("https://discord.com/api/v9/users/@me").unwrap();
            let request = rig
                .handle
                .rest()
                .build_request(Method::Get, &url, None)
                .unwrap();
            assert_eq!(request.authorization().unwrap().reveal(), MARKER);
        }
    }

    /// The fetch at launch: the captured page and chunk accepted (only the two URLs, no
    /// Authorization, the values cached).
    #[tokio::test]
    async fn the_identity_fetch_feeds_the_first_identify() {
        // The chunk the page names was not captured: the newest captured chunk's literal is
        // substituted (see identity_fetch.rs tests).
        let chunk = CHUNK.replace("\"634304\"", "\"640102\"");
        let mock = MockHttp::start(vec![response(200, "", HTML), response(200, "", &chunk)]).await;
        mock.install();
        let l = launch(true);
        let mut rig = Rig::start_with(opts(&l, None)).await;
        let mut c = Conn::accept(&rig.initial).await;
        c.hello(30_000).await;
        let fast = identify(&mut c).await;
        assert_eq!(fast["d"]["properties"]["client_build_number"], 640_102);
        let requests = mock.requests();
        assert_eq!(requests.len(), 2);
        assert!(requests[0].starts_with("GET /app HTTP/1.1\r\nhost: discord.com\r\n"));
        assert!(requests[1].starts_with(
            "GET /assets/fast-connect.f0cfe6eb7552ae67.js HTTP/1.1\r\nhost: discord.com\r\n"
        ));
        for r in &requests {
            assert!(!r.to_ascii_lowercase().contains("authorization"), "{r}");
        }
        assert_eq!(
            l.store.fetch_cache(now()).map(|c| (c.chunk_hash, c.build)),
            Some(("f0cfe6eb7552ae67".to_owned(), 640_102))
        );
        assert!(rig.identity.try_recv().is_err(), "no notice on success");
    }

    /// A failure falls back to the cache with one notice per launch, and the launch fetches once.
    #[tokio::test]
    async fn a_failed_fetch_uses_the_cache_and_notices_once_per_launch() {
        let mock = MockHttp::start(vec![response(404, "", "")]).await;
        mock.install();
        let l = launch(true);
        l.store.set_fetch_cache(FetchCache {
            chunk_hash: "0123456789abcdef".into(),
            build: 650_000,
            pair: (1_767_421, 1_734_653),
            fetched_at_ms: now() - 25 * 3_600_000,
        });
        let mut rig = Rig::start_with(opts(&l, None)).await;
        let mut c = Conn::accept(&rig.initial).await;
        c.hello(30_000).await;
        assert_eq!(
            identify(&mut c).await["d"]["properties"]["client_build_number"],
            650_000
        );
        let notice = timeout(WAIT, rig.identity.recv()).await.unwrap().unwrap();
        assert!(matches!(
            notice,
            IdentityEvent::Notice {
                notice: IdentityNotice::IdentityFetchFailed,
                ..
            }
        ));
        // A second session of the launch: the same values, no second fetch, no second notice.
        let mut second = Rig::start_with(opts(&l, None)).await;
        let mut c = Conn::accept(&second.initial).await;
        c.hello(30_000).await;
        assert_eq!(
            identify(&mut c).await["d"]["properties"]["client_build_number"],
            650_000
        );
        assert_eq!(mock.count(), 1);
        assert!(second.identity.try_recv().is_err());
    }

    #[tokio::test]
    async fn an_unsupported_pair_notices_and_keeps_the_supported_value() {
        let chunk = CHUNK
            .replace("\"634304\"", "\"640102\"")
            .replace("t?1767421:1734653}", "t?1800189:1767421}");
        let mock = MockHttp::start(vec![response(200, "", HTML), response(200, "", &chunk)]).await;
        mock.install();
        let mut rig = Rig::start_with(opts(&launch(true), None)).await;
        let mut c = Conn::accept(&rig.initial).await;
        c.hello(30_000).await;
        assert_eq!(identify(&mut c).await["d"]["capabilities"], 1_734_653);
        let notice = timeout(WAIT, rig.identity.recv()).await.unwrap().unwrap();
        assert!(matches!(
            notice,
            IdentityEvent::Notice {
                notice: IdentityNotice::UnsupportedCapabilities,
                ..
            }
        ));
    }

    /// The first IDENTIFY waits for a hanging fetch at most its deadline, then goes out with the
    /// built-in values and the notice (T-14).
    #[tokio::test]
    async fn the_first_identify_waits_at_most_the_fetch_deadline() {
        let mock = MockHttp::start(vec![crate::rest::test_support::HANG.to_owned()]).await;
        mock.install();
        let started = tokio::time::Instant::now();
        let mut rig = Rig::start_with(opts(&launch(true), None)).await;
        let mut c = Conn::accept(&rig.initial).await;
        let waited = started.elapsed();
        assert!(
            waited >= Duration::from_millis(2_900) && waited < Duration::from_secs(6),
            "{waited:?}"
        );
        c.hello(30_000).await;
        assert_eq!(
            identify(&mut c).await["d"]["properties"]["client_build_number"],
            640_102
        );
        assert!(matches!(
            timeout(WAIT, rig.identity.recv()).await.unwrap().unwrap(),
            IdentityEvent::Notice {
                notice: IdentityNotice::IdentityFetchFailed,
                ..
            }
        ));
    }

    /// Leftover (a): an INITIAL_GUILD buffered on a connection that died before READY is dropped
    /// by the next IDENTIFY, never merged into a later READY.
    #[tokio::test]
    async fn an_initial_guild_from_a_dead_connection_is_never_merged() {
        let mut rig = Rig::start().await;
        let mut c1 = Conn::accept(&rig.initial).await;
        c1.hello(30_000).await;
        identify(&mut c1).await;
        c1.send(json!({"op": 0, "t": "INITIAL_GUILD", "s": 1, "d": {
            "id": "10", "name": "early", "owner_id": "9", "roles": [], "channels": []}}))
            .await;
        while !rig
            .handle
            .applied()
            .iter()
            .any(|(e, _)| e == "INITIAL_GUILD")
        {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        c1.close(4009).await;
        let mut c2 = Conn::accept(&rig.initial).await;
        c2.hello(30_000).await;
        identify(&mut c2).await;
        c2.send(ready(1, RESUME_URL)).await;
        rig.ready_event().await;
        assert!(rig.handle.snapshot().guild(10).is_none());
    }

    /// Leftover (b): the stored op-37 selection is re-checked when re-sent after a reconnect; a
    /// channel obfuscated meanwhile gets no subscription.
    #[tokio::test]
    async fn a_resent_member_subscription_skips_a_channel_obfuscated_meanwhile() {
        let mut rig = Rig::start().await;
        rig.handle.set_member_subscription(Some((10, 20)));
        let mut c1 = Conn::accept(&rig.initial).await;
        c1.hello(30_000).await;
        identify(&mut c1).await;
        c1.send(ready_with(
            json!({"guilds": [{"id": "10", "name": "g", "owner_id": "9",
            "roles": [], "channels": [{"id": "20", "type": 0, "name": "c"}]}]}),
        ))
        .await;
        rig.ready_event().await;
        assert_eq!(c1.recv_non_beat().await["op"], 37);
        assert_eq!(c1.recv_non_beat().await["op"], 37);
        assert!(!rig.handle.is_obfuscated(20));
        c1.send(json!({"op": 0, "t": "CHANNEL_UPDATE", "s": 2, "d": {
            "id": "20", "guild_id": "10", "type": 0, "name": "c", "flags": 131072}}))
            .await;
        while !rig.handle.is_obfuscated(20) {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        c1.drop_abruptly();
        let mut c2 = Conn::accept(&rig.resume).await;
        c2.hello(30_000).await;
        assert_eq!(c2.recv_non_beat().await["op"], 6);
        c2.send(json!({"op": 0, "t": "RESUMED", "s": 3, "d": {}}))
            .await;
        assert!(c2.quiet().await, "op 37 for an obfuscated channel");
    }
}
