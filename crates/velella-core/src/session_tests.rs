//! Mock-server tests through the public `Session::connect`. The mock speaks zlib-stream Binary
//! frames like Discord; the only test seam is the dial target (see `Task::dial_target`).
use crate::{session::test_support::TEST_ENDPOINTS, Session, SessionEvent, SessionHandle, Token};
use flate2::write::ZlibEncoder;
use futures_util::{SinkExt, StreamExt};
use serde_json::{json, Value};
use std::{
    io::Write,
    sync::{Arc, Mutex, OnceLock},
    time::Duration,
};
use tokio::{
    net::{TcpListener, TcpStream},
    sync::mpsc,
    time::timeout,
};
use tokio_tungstenite::{
    accept_async,
    tungstenite::{protocol::CloseFrame, Message},
    WebSocketStream,
};

const MARKER: &str = "SYNTHETIC_TOKEN_MARKER_0123456789";
const WAIT: Duration = Duration::from_secs(15);

/// One accepted client connection with its own zlib-stream compressor.
struct Conn {
    ws: WebSocketStream<TcpStream>,
    enc: ZlibEncoder<Vec<u8>>,
    sent: usize,
}

impl Conn {
    async fn accept(listener: &TcpListener) -> Conn {
        let (stream, _) = timeout(WAIT, listener.accept())
            .await
            .expect("no connection")
            .unwrap();
        Conn {
            ws: accept_async(stream).await.unwrap(),
            enc: ZlibEncoder::new(Vec::new(), flate2::Compression::fast()),
            sent: 0,
        }
    }

    /// Sends one gateway packet as a Binary zlib-stream frame (sync-flushed, `00 00 FF FF` suffix).
    async fn send(&mut self, packet: Value) {
        self.enc.write_all(packet.to_string().as_bytes()).unwrap();
        self.enc.flush().unwrap();
        let bytes = self.enc.get_ref()[self.sent..].to_vec();
        self.sent = self.enc.get_ref().len();
        self.ws.send(Message::Binary(bytes.into())).await.unwrap();
    }

    async fn hello(&mut self, interval_ms: u64) {
        self.send(json!({"op": 10, "d": {"heartbeat_interval": interval_ms}}))
            .await;
    }

    /// Next client packet as raw text (clients send uncompressed Text).
    async fn recv_raw(&mut self) -> String {
        match timeout(WAIT, self.ws.next())
            .await
            .expect("client sent nothing")
        {
            Some(Ok(Message::Text(t))) => t.to_string(),
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
            if packet["op"] != 1 {
                return packet;
            }
        }
    }

    /// True when nothing but heartbeats arrives for a while.
    async fn quiet(&mut self) -> bool {
        let deadline = tokio::time::Instant::now() + Duration::from_millis(400);
        loop {
            match tokio::time::timeout_at(deadline, self.ws.next()).await {
                Err(_) => return true,
                Ok(Some(Ok(Message::Text(t))))
                    if serde_json::from_str::<Value>(&t).is_ok_and(|v| v["op"] == 1) => {}
                Ok(_) => return false,
            }
        }
    }

    async fn close(&mut self, code: u16) {
        let frame = CloseFrame {
            code: code.into(),
            reason: "".into(),
        };
        self.ws.send(Message::Close(Some(frame))).await.unwrap();
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
    initial: TcpListener,
    resume: TcpListener,
    handle: SessionHandle,
    events: mpsc::UnboundedReceiver<SessionEvent>,
}

impl Rig {
    async fn start() -> Rig {
        capture();
        let initial = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let resume = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = |l: &TcpListener| format!("ws://{}/", l.local_addr().unwrap());
        TEST_ENDPOINTS.with(|e| *e.borrow_mut() = Some((url(&initial), url(&resume))));
        let (tx, events) = mpsc::unbounded_channel();
        let handle = Session::connect(Token::new(MARKER.into()).unwrap(), move |e| {
            let _ = tx.send(e);
        });
        Rig {
            initial,
            resume,
            handle,
            events,
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

async fn no_connection(listener: &TcpListener) -> bool {
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
    assert_eq!(identify["d"]["capabilities"], 253);
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
    let _first = rig.ready(100, "wss://gateway-resume.discord.gg/").await;
    let mut conn = Conn::accept(&rig.resume).await;
    assert_eq!(
        rig.event().await,
        SessionEvent::Reconnecting {
            generation: rig.handle.generation()
        }
    );
    conn.hello(30_000).await;
    let resume = conn.recv().await;
    assert_eq!(resume["op"], 6);
    assert_eq!(resume["d"]["token"], MARKER);
    assert_eq!(resume["d"]["session_id"], "sess-1");
    assert_eq!(resume["d"]["seq"], 1);
    assert!(no_connection(&rig.initial).await, "must not IDENTIFY again");
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
        assert_eq!(last["op"], 1);
        conn.send(json!({"op": 11, "d": null})).await;
    }
    // (A first beat can race ahead of READY and carry null; by now the sequence is known.)
    assert_eq!(last["d"], 1);
    // Checked right after the last ACK (before the mock goes silent): nothing reconnected.
    assert!(timeout(Duration::from_millis(5), rig.initial.accept())
        .await
        .is_err());
    assert!(timeout(Duration::from_millis(5), rig.resume.accept())
        .await
        .is_err());
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
    while conn.recv().await["op"] != 1 {}
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
    let closing = loop {
        match timeout(WAIT, conn.ws.next()).await.unwrap() {
            Some(Ok(Message::Text(t))) if serde_json::from_str::<Value>(&t).unwrap()["op"] == 1 => {
            }
            other => break other,
        }
    };
    assert!(matches!(
        closing,
        Some(Ok(Message::Close(_))) | Some(Err(_)) | None
    ));
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

/// A `wss://` dial must reach the TLS handshake. Without an explicit rustls crypto provider the
/// stack panics before any I/O (what the Cargo.toml `rustls` entry exists to prevent). A plain TCP
/// peer cannot complete a handshake, so a clean error is the expected outcome.
#[tokio::test]
async fn wss_dial_builds_the_tls_stack_and_fails_cleanly_against_a_non_tls_peer() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("wss://127.0.0.1:{}/", listener.local_addr().unwrap().port());
    tokio::spawn(async move {
        let _ = listener.accept().await;
    });
    let dialed = timeout(
        Duration::from_secs(5),
        tokio_tungstenite::connect_async(url),
    )
    .await;
    assert!(matches!(dialed, Ok(Err(_))), "expected a handshake error");
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
