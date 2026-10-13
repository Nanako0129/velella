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

#[tokio::test]
async fn disconnect_emits_closed_and_generations_increase() {
    let mut rig = Rig::start().await;
    let mut conn = rig.ready(30_000, "wss://gateway-resume.discord.gg/").await;
    let generation = rig.handle.generation();
    rig.handle.disconnect();
    assert_eq!(rig.event().await, SessionEvent::Closed { generation });
    assert!(matches!(
        timeout(WAIT, conn.ws.next()).await.unwrap(),
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
