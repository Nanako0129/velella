//! Slice CU (composer plan rev 1.8): the request shapes, the host check, the closed header lists,
//! the preflight cache, resume, files, limits, the pending row's flow, retry, permission,
//! redaction and the HEIC / JXR config, against an in-memory transport that plays both Discord's
//! API and the upload host. Nothing here reaches a network.
use super::*;
use crate::{
    messages::{MessageRow, MessageStore, UploadState},
    rest::{Rest, TokenSlot},
    token::Token,
    transport::{BoxFuture, HttpRequest, WebSocket, WebSocketRequest},
};
use serde_json::json;
use std::collections::VecDeque;
use std::sync::atomic::AtomicBool;
use tokio::sync::{mpsc, Notify};

const ME: u64 = 1;
const GUILD: u64 = 100;
const CHANNEL: u64 = 300;
const DM: u64 = 500;
const TOKEN: &str = "SYNTHETIC_TOKEN_MARKER_0123456789";
const MD5: &str = "0123456789abcdef0123456789abcdef";

// ---- The in-memory API and upload host ----

/// One request as the server end saw it, headers in the order core composed them.
#[derive(Clone, Debug)]
struct Rec {
    method: String,
    url: String,
    headers: Vec<(String, String)>,
    body: Vec<u8>,
    authorized: bool,
}

impl Rec {
    fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(n, _)| n.eq_ignore_ascii_case(name))
            .map(|(_, v)| v.as_str())
    }

    fn names(&self) -> Vec<&str> {
        self.headers.iter().map(|(n, _)| n.as_str()).collect()
    }

    fn target(&self) -> String {
        let url = Url::parse(&self.url).unwrap();
        match url.query() {
            Some(q) => format!("{} {}?{q}", self.method, url.path()),
            None => format!("{} {}", self.method, url.path()),
        }
    }

    fn json(&self) -> Value {
        serde_json::from_slice(&self.body).unwrap()
    }

    /// An upload leg as the tests name it.
    fn leg(&self) -> String {
        match (self.method.as_str(), self.header("content-range")) {
            ("OPTIONS", _) => format!(
                "OPTIONS {}",
                self.header("access-control-request-headers").unwrap()
            ),
            (_, Some("bytes */*")) => "PROBE".into(),
            (_, Some(_)) => "PUT+range".into(),
            _ => "PUT".into(),
        }
    }
}

enum Put {
    Ok,
    /// The host keeps the first n bytes, then the connection drops.
    Drop(usize),
    Status(u16),
    /// Never answers; the future waits until dropped.
    Hang,
}

struct Host {
    /// Bytes each upload URL holds.
    received: HashMap<String, Vec<u8>>,
    puts: VecDeque<Put>,
    /// `Some(None)`: a 308 without Range; `Some(Some(v))`: Range `v`; `None`: what the host holds.
    probe: Option<Option<String>>,
    probe_status: Option<u16>,
    preflight_status: u16,
    preflight: Vec<(String, String)>,
}

fn allowing(max_age: Option<&str>) -> Vec<(String, String)> {
    let mut h = vec![
        (
            "Access-Control-Allow-Origin".into(),
            "https://discord.com".into(),
        ),
        ("Access-Control-Allow-Methods".into(), "PUT".into()),
        (
            "Access-Control-Allow-Headers".into(),
            "Content-Type, Content-Range".into(),
        ),
    ];
    if let Some(age) = max_age {
        h.push(("Access-Control-Max-Age".into(), age.into()));
    }
    h
}

type Responder = Box<dyn FnMut(&Rec, usize) -> Option<Result<HttpResponse, TransportError>> + Send>;

struct Fake {
    log: Mutex<Vec<Rec>>,
    /// Overrides for API answers (`None`: the default); called with the request and how many
    /// slot requests came before.
    api: Mutex<Option<Responder>>,
    slots: AtomicU64,
    /// `upload_filename` the slot answers with, `{n}` replaced by the slot's number.
    upload_filename: Mutex<String>,
    upload_url: Mutex<Option<String>>,
    hang_create: AtomicBool,
    host: Mutex<Host>,
    files: Mutex<HashMap<String, (FileIdentity, Vec<u8>)>>,
    open: Mutex<HashMap<u64, String>>,
    next_file: AtomicU64,
    dropped: Notify,
}

fn ok(body: Value) -> HttpResponse {
    HttpResponse {
        status: 200,
        headers: vec![],
        body: Some(body.to_string().into_bytes()),
    }
}

fn status(code: u16) -> HttpResponse {
    HttpResponse {
        status: code,
        headers: vec![],
        body: Some(b"{}".to_vec()),
    }
}

/// Resolves only when dropped, then tells `dropped`.
async fn hang(dropped: &Notify) {
    struct OnDrop<'a>(&'a Notify);
    impl Drop for OnDrop<'_> {
        fn drop(&mut self) {
            self.0.notify_one();
        }
    }
    let _guard = OnDrop(dropped);
    std::future::pending::<()>().await;
}

impl Fake {
    fn new() -> Arc<Fake> {
        Arc::new(Fake {
            log: Mutex::default(),
            api: Mutex::new(None),
            slots: AtomicU64::new(0),
            upload_filename: Mutex::new("4f1e9c2a-uuid/{n}/f.bin".into()),
            upload_url: Mutex::new(None),
            hang_create: AtomicBool::new(false),
            host: Mutex::new(Host {
                received: HashMap::new(),
                puts: VecDeque::new(),
                probe: None,
                probe_status: None,
                preflight_status: 200,
                preflight: allowing(None),
            }),
            files: Mutex::default(),
            open: Mutex::default(),
            next_file: AtomicU64::new(0),
            dropped: Notify::new(),
        })
    }

    fn log(&self) -> Vec<Rec> {
        self.log.lock().unwrap().clone()
    }

    fn api_log(&self) -> Vec<Rec> {
        self.log()
            .into_iter()
            .filter(|r| r.url.starts_with("https://discord.com/") && !r.url.contains("limit=50"))
            .collect()
    }

    fn upload_log(&self) -> Vec<Rec> {
        self.log()
            .into_iter()
            .filter(|r| r.url.starts_with(UPLOAD_PREFIX))
            .collect()
    }

    fn legs(&self) -> Vec<String> {
        self.upload_log().iter().map(Rec::leg).collect()
    }

    fn host(&self) -> std::sync::MutexGuard<'_, Host> {
        self.host.lock().unwrap()
    }

    /// Registers a file the app "attached".
    fn file(&self, path: &str, bytes: Vec<u8>) -> FileIdentity {
        let identity = FileIdentity {
            path: path.into(),
            device: 1,
            inode: 1000 + self.files.lock().unwrap().len() as u64,
            size: bytes.len() as u64,
            mtime_ns: 1_700_000_000_000_000_000,
        };
        self.files
            .lock()
            .unwrap()
            .insert(path.into(), (identity.clone(), bytes));
        identity
    }

    fn slot_url(n: u64) -> String {
        format!("{UPLOAD_PREFIX}upload/storage/v1/b/b/o?uploadType=resumable&upload_id=U{n}")
    }

    fn default_api(&self, rec: &Rec) -> HttpResponse {
        let path = Url::parse(&rec.url).unwrap().path().to_owned();
        match (rec.method.as_str(), path.as_str()) {
            ("GET", _) => ok(json!([])),
            ("POST", p) if p.ends_with("/attachments") => {
                let n = self.slots.fetch_add(1, Ordering::SeqCst) + 1;
                let url = self
                    .upload_url
                    .lock()
                    .unwrap()
                    .clone()
                    .unwrap_or_else(|| Fake::slot_url(n));
                let name = self
                    .upload_filename
                    .lock()
                    .unwrap()
                    .replace("{n}", &n.to_string());
                ok(
                    json!({"attachments": [{"id": "0", "upload_url": url, "upload_filename": name}]}),
                )
            }
            ("POST", p) if p.ends_with("/messages") => {
                let body = rec.json();
                ok(json!({
                    "id": "900000000000000001", "channel_id": CHANNEL.to_string(),
                    "author": {"id": ME.to_string(), "username": "me"},
                    "content": body["content"], "nonce": body["nonce"], "attachments": []
                }))
            }
            ("DELETE", _) => HttpResponse {
                status: 204,
                headers: vec![],
                body: Some(vec![]),
            },
            _ => status(404),
        }
    }

    /// A PUT or probe; also how many of its body bytes the host kept.
    fn put(&self, rec: &Rec) -> (Result<HttpResponse, TransportError>, usize) {
        let mut host = self.host();
        let range = rec.header("content-range").map(str::to_owned);
        if range.as_deref() == Some("bytes */*") {
            if let Some(code) = host.probe_status {
                return (Ok(status(code)), 0);
            }
            let held = host.received.get(&rec.url).map_or(0, Vec::len);
            let range = match host.probe.clone() {
                Some(r) => r,
                None => (held > 0).then(|| format!("bytes=0-{}", held - 1)),
            };
            let answer = HttpResponse {
                status: 308,
                headers: range.map(|r| vec![("Range".into(), r)]).unwrap_or_default(),
                body: Some(vec![]),
            };
            return (Ok(answer), 0);
        }
        let action = host.puts.pop_front().unwrap_or(Put::Ok);
        let stored = host.received.entry(rec.url.clone()).or_default();
        // `bytes {start}-…`: the body goes at `start`; no range: from 0; `bytes */n`: finishing.
        let start = match range.as_deref() {
            None => 0,
            Some(r) => r
                .strip_prefix("bytes ")
                .and_then(|r| r.split('-').next())
                .and_then(|s| s.parse::<usize>().ok())
                .unwrap_or(stored.len()),
        };
        stored.truncate(start);
        match action {
            Put::Ok => {
                stored.extend_from_slice(&rec.body);
                (Ok(ok(json!({}))), rec.body.len())
            }
            Put::Drop(n) => {
                let n = n.min(rec.body.len());
                stored.extend_from_slice(&rec.body[..n]);
                (Err(TransportError::Other), n)
            }
            Put::Status(code) => (Ok(status(code)), 0),
            Put::Hang => unreachable!("handled in `upload`"),
        }
    }
}

impl Transport for Fake {
    fn http(&self, request: HttpRequest) -> BoxFuture<'_, Result<HttpResponse, TransportError>> {
        let rec = Rec {
            method: request.method().as_str().into(),
            url: request.url().into(),
            headers: request.headers().to_vec(),
            body: request.body().unwrap_or_default().to_vec(),
            authorized: request.authorization().is_some(),
        };
        self.log.lock().unwrap().push(rec.clone());
        Box::pin(async move {
            tokio::task::yield_now().await;
            if rec.method == "POST"
                && rec.url.ends_with("/messages")
                && self.hang_create.load(Ordering::SeqCst)
            {
                hang(&self.dropped).await;
            }
            let slots = self.slots.load(Ordering::SeqCst) as usize;
            let custom = self
                .api
                .lock()
                .unwrap()
                .as_mut()
                .and_then(|f| f(&rec, slots));
            if let Some(answer) = custom {
                if rec.url.ends_with("/attachments") {
                    self.slots.fetch_add(1, Ordering::SeqCst);
                }
                return answer;
            }
            Ok(self.default_api(&rec))
        })
    }

    fn websocket(
        &self,
        _: WebSocketRequest,
    ) -> BoxFuture<'_, Result<Box<dyn WebSocket>, TransportError>> {
        Box::pin(async { Err(TransportError::ConnectFailed) })
    }

    fn open_file(&self, file: &FileIdentity) -> Option<u64> {
        let files = self.files.lock().unwrap();
        let (identity, _) = files.get(&file.path)?;
        (identity == file).then(|| {
            let id = self.next_file.fetch_add(1, Ordering::SeqCst) + 1;
            self.open.lock().unwrap().insert(id, file.path.clone());
            id
        })
    }

    fn close_file(&self, id: u64) {
        self.open.lock().unwrap().remove(&id);
    }

    fn upload(
        &self,
        request: UploadRequest,
        progress: crate::transport::UploadProgress,
    ) -> BoxFuture<'_, Result<HttpResponse, TransportError>> {
        let body = request.body().map_or_else(Vec::new, |b| {
            let path = self.open.lock().unwrap().get(&b.file).cloned().unwrap();
            let files = self.files.lock().unwrap();
            let bytes = &files[&path].1;
            bytes[b.start as usize..(b.start + b.length) as usize].to_vec()
        });
        let rec = Rec {
            method: request.method().as_str().into(),
            url: request.url().into(),
            headers: request.headers().to_vec(),
            body,
            authorized: false,
        };
        self.log.lock().unwrap().push(rec.clone());
        Box::pin(async move {
            tokio::task::yield_now().await;
            if rec.method == "OPTIONS" {
                let host = self.host();
                return Ok(HttpResponse {
                    status: host.preflight_status,
                    headers: host.preflight.clone(),
                    body: Some(vec![]),
                });
            }
            let hangs = rec.header("content-range") != Some("bytes */*")
                && matches!(self.host().puts.front(), Some(Put::Hang));
            if hangs {
                self.host().puts.pop_front();
                hang(&self.dropped).await;
            }
            let (answer, sent) = self.put(&rec);
            progress(sent as u64);
            answer
        })
    }
}

// ---- The world ----

fn role(id: u64, permissions: u64) -> Value {
    json!({"id": id.to_string(), "permissions": permissions.to_string(), "position": 1})
}

/// A READY: the user (`premium_type`), guild 100 whose @everyone role holds `everyone` (and the
/// guild `features`), channel 300, DM 500, and the legacy `experiments` list when given.
fn ready(
    premium: Option<u8>,
    everyone: u64,
    features: &[&str],
    experiments: Option<Value>,
) -> State {
    let mut user = json!({"id": ME.to_string(), "username": "me"});
    if let Some(p) = premium {
        user["premium_type"] = json!(p);
    }
    let mut d = json!({
        "user": user,
        "guilds": [{
            "id": GUILD.to_string(), "name": "g", "owner_id": "999", "features": features,
            "mfa_level": 0,
            "roles": [role(GUILD, everyone)],
            "channels": [{"id": CHANNEL.to_string(), "type": 0, "permission_overwrites": []}],
        }],
        "merged_members": [[{"user_id": ME.to_string(), "roles": []}]],
        "private_channels": [{"id": DM.to_string(), "type": 1, "recipient_ids": []}],
    });
    if let Some(e) = experiments {
        d["experiments"] = e;
    }
    let mut s = State::default();
    assert!(s.apply("READY", &d));
    s
}

const SEND: u64 = crate::permissions::SEND_MESSAGES | crate::permissions::VIEW_CHANNEL;

fn member_state() -> State {
    ready(Some(0), SEND | ATTACH_FILES, &[], Some(json!([])))
}

/// A legacy user tuple (experiments.md 6c's shape).
fn legacy(hash: u64, bucket: i64, aa: i64) -> Value {
    json!([hash, 2, bucket, 1, 0, 1234, aa, 0, null, null, null])
}

struct World {
    fake: Arc<Fake>,
    store: Arc<MessageStore>,
    state: State,
}

async fn world_in(state: State, channel: u64) -> World {
    let fake = Fake::new();
    let (tx, rx) = mpsc::unbounded_channel();
    std::mem::forget(rx);
    let rest = Rest::new(
        TokenSlot::new(Arc::new(Token::new(TOKEN.into()).unwrap())),
        1,
        tx,
        fake.clone(),
        Arc::new(Identity::current()),
    );
    let store = MessageStore::with_wake(rest, None);
    store.set_me(Some(ME));
    store.open(channel);
    tokio::task::yield_now().await;
    World { fake, store, state }
}

async fn world() -> World {
    world_in(member_state(), CHANNEL).await
}

fn bytes(n: usize) -> Vec<u8> {
    (0..n).map(|i| (i % 251) as u8).collect()
}

fn descriptor(file: FileIdentity) -> AttachmentDescriptor {
    AttachmentDescriptor {
        filename: file.path.rsplit('/').next().unwrap().into(),
        file,
        description: None,
        spoiler: false,
        content_type: "image/png".into(),
        original_md5: None,
    }
}

impl World {
    fn job(&self, content: &str, files: Vec<AttachmentDescriptor>) -> SendJob {
        SendJob {
            channel_id: CHANNEL,
            guild_id: Some(GUILD),
            content: content.into(),
            reply_to: None,
            instant: false,
            files,
        }
    }

    fn send(&self, job: SendJob) -> Result<u64, SendRefusal> {
        start_send(&self.state, &self.store, job)
    }

    /// One registered file of `n` bytes at `path`.
    fn attach(&self, path: &str, n: usize) -> AttachmentDescriptor {
        descriptor(self.fake.file(path, bytes(n)))
    }

    fn upload_row(&self) -> Option<MessageRow> {
        self.store.rows().into_iter().find(|r| r.upload.is_some())
    }

    fn state_of(&self) -> Option<UploadState> {
        self.upload_row().map(|r| r.upload.unwrap().state)
    }

    /// Lets the tasks run (time is paused, so sleeps and deadlines pass at once) until `done`.
    async fn until(&self, what: &str, done: impl Fn(&World) -> bool) {
        // Each step is 100 ms of paused time; the tasks' own timers fire in between.
        for _ in 0..200_000 {
            if done(self) {
                return;
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
        panic!("never: {what}; legs {:?}", self.fake.legs());
    }

    /// Until the send's row is gone (sent) or failed.
    async fn settle(&self) {
        self.until("the send settles", |w| {
            !matches!(w.state_of(), Some(UploadState::Sending))
        })
        .await;
    }

    fn creates(&self) -> Vec<Rec> {
        self.fake
            .api_log()
            .into_iter()
            .filter(|r| r.method == "POST" && r.url.ends_with("/messages"))
            .collect()
    }

    fn slots(&self) -> Vec<Rec> {
        self.fake
            .api_log()
            .into_iter()
            .filter(|r| r.url.ends_with("/attachments"))
            .collect()
    }
}

// ---- Request shapes (OA A1-A3, A9-A10) ----

#[tokio::test(start_paused = true)]
async fn slot_bodies_match_the_web_client() {
    let w = world().await;
    let plain = w.attach("/Users/x/Pictures/cat 1.png", 10);
    let mut unknown = w.attach("/Users/x/Downloads/data.qqq", 11);
    unknown.content_type = String::new();
    let mut paste = w.attach("/tmp/velella-attachments/A/message.txt", 12);
    paste.content_type = "text/plain".into();
    let mut heic = w.attach("/tmp/velella-attachments/B/IMG_1.jpg", 13);
    heic.content_type = "image/heic".into();
    heic.original_md5 = Some(MD5.into());
    w.send(w.job("hi", vec![plain, unknown, paste, heic]))
        .unwrap();
    w.settle().await;
    let slots = w.slots();
    assert_eq!(slots.len(), 4);
    let ids: Vec<u64> = slots
        .iter()
        .map(|r| {
            r.json()["files"][0]["id"]
                .as_str()
                .unwrap()
                .parse()
                .unwrap()
        })
        .collect();
    // One process-wide counter: increasing (other tests in the process may take numbers between).
    assert!(ids.windows(2).all(|p| p[0] < p[1]), "{ids:?}");
    let expected = [
        ("cat 1.png", 10, "image/png"),
        ("data.qqq", 11, ""),
        ("message.txt", 12, "text/plain"),
        ("IMG_1.jpg", 13, "image/heic"),
    ];
    for ((rec, id), (name, size, mime)) in slots.iter().zip(&ids).zip(expected) {
        assert_eq!(
            rec.target(),
            format!("POST /api/v9/channels/{CHANNEL}/attachments")
        );
        assert_eq!(
            String::from_utf8(rec.body.clone()).unwrap(),
            format!(
                r#"{{"files":[{{"filename":"{name}","file_size":{size},"id":"{id}","is_clip":false,"original_content_type":"{mime}"}}]}}"#
            )
        );
        assert!(rec.authorized);
    }
    // The MD5 header rides on the converted file's slot request only (OA A39, A49).
    let md5: Vec<Option<&str>> = slots
        .iter()
        .map(|r| r.header("X-Discord-Original-MD5"))
        .collect();
    assert_eq!(md5, [None, None, None, Some(MD5)]);
    assert_eq!(slots[3].names().last(), Some(&"X-Discord-Original-MD5"));
    for rec in w.fake.upload_log().iter().chain(&w.creates()) {
        assert!(
            rec.header("X-Discord-Original-MD5").is_none(),
            "{}",
            rec.leg()
        );
    }
}

#[tokio::test(start_paused = true)]
async fn an_invalid_md5_is_dropped() {
    let w = world().await;
    for bad in [
        "0123456789ABCDEF0123456789ABCDEF",
        "abc",
        "0123456789abcdef0123456789abcdeg",
    ] {
        let mut d = w.attach(&format!("/p/{bad}.jpg"), 5);
        d.original_md5 = Some(bad.into());
        w.send(w.job("", vec![d])).unwrap();
        w.settle().await;
    }
    assert_eq!(w.slots().len(), 3);
    assert!(w
        .slots()
        .iter()
        .all(|r| r.header("X-Discord-Original-MD5").is_none()));
}

#[tokio::test(start_paused = true)]
async fn message_bodies_match_the_web_client() {
    let w = world().await;
    // One attachment.
    w.send(w.job("one", vec![w.attach("/p/a.png", 3)])).unwrap();
    w.settle().await;
    // Three: a spoiler, a description, an emptied description and an edited filename.
    let mut b = w.attach("/p/b.png", 3);
    b.spoiler = true;
    let mut c = w.attach("/p/c.txt", 3);
    c.description = Some("a \"cat\"".into());
    c.content_type = String::new();
    let mut d = w.attach("/p/original name.png", 3);
    d.description = Some(String::new());
    d.filename = "edited.png".into();
    w.send(w.job("three", vec![b, c, d])).unwrap();
    w.settle().await;
    // Attachments only, and Shift-drop's send.
    w.send(w.job("", vec![w.attach("/p/e.png", 3)])).unwrap();
    w.settle().await;
    let mut instant = w.job("", vec![w.attach("/p/f.png", 3)]);
    instant.instant = true;
    w.send(instant).unwrap();
    w.settle().await;

    let creates = w.creates();
    assert_eq!(creates.len(), 4);
    let body = |i: usize| String::from_utf8(creates[i].body.clone()).unwrap();
    let nonce = |i: usize| creates[i].json()["nonce"].as_str().unwrap().to_owned();
    let name = |n: u64| format!("4f1e9c2a-uuid/{n}/f.bin");
    assert_eq!(
        body(0),
        format!(
            r#"{{"mobile_network_type":"unknown","content":"one","nonce":"{}","tts":false,"flags":0,"attachments":[{{"id":"0","filename":"a.png","uploaded_filename":"{}","original_content_type":"image/png"}}]}}"#,
            nonce(0),
            name(1)
        )
    );
    assert_eq!(
        body(1),
        format!(
            r#"{{"mobile_network_type":"unknown","content":"three","nonce":"{}","tts":false,"flags":0,"attachments":[{{"id":"0","filename":"b.png","uploaded_filename":"{}","is_spoiler":true,"original_content_type":"image/png"}},{{"id":"1","description":"a \"cat\"","filename":"c.txt","uploaded_filename":"{}"}},{{"id":"2","description":"","filename":"edited.png","uploaded_filename":"{}","original_content_type":"image/png"}}]}}"#,
            nonce(1),
            name(2),
            name(3),
            name(4)
        )
    );
    // The slot names the original file; the message names the edited one.
    assert_eq!(
        w.slots()[3].json()["files"][0]["filename"],
        "original name.png"
    );
    assert!(body(2).contains(r#""content":"","#));
    let chat = "eyJsb2NhdGlvbiI6ImNoYXRfaW5wdXQifQ==";
    let instant = "eyJsb2NhdGlvbiI6Imluc3RhbnRfdXBsb2FkIn0=";
    assert_eq!(
        base64::Engine::decode(&base64::engine::general_purpose::STANDARD, instant).unwrap(),
        br#"{"location":"instant_upload"}"#
    );
    for (i, context) in [chat, chat, chat, instant].into_iter().enumerate() {
        assert_eq!(
            creates[i].header("X-Context-Properties"),
            Some(context),
            "{i}"
        );
        assert_eq!(
            creates[i].target(),
            format!("POST /api/v9/channels/{CHANNEL}/messages")
        );
    }
    assert!(body(3).contains(r#""content":"","#));
}

// ---- Host check (D13a) ----

#[test]
fn only_the_exact_upload_host_is_accepted() {
    let good =
        format!("https://{UPLOAD_HOST}/upload/storage/v1/b/x/o?uploadType=resumable&upload_id=A");
    assert!(upload_url(&good).is_ok());
    let refused = [
        "https://evil.test/upload?upload_id=A".to_owned(),
        format!("http://{UPLOAD_HOST}/u?upload_id=A"),
        format!("https://{UPLOAD_HOST}:8443/u?upload_id=A"),
        format!("https://user@{UPLOAD_HOST}/u?upload_id=A"),
        format!("https://{UPLOAD_HOST}/a/../u?upload_id=A"),
        format!("https://{UPLOAD_HOST}/a/%2e%2e/u?upload_id=A"),
        format!("https://{UPLOAD_HOST}.evil.test/u?upload_id=A"),
        format!("https://evil{UPLOAD_HOST}/u?upload_id=A"),
        format!("https://{}/u?upload_id=A", UPLOAD_HOST.to_uppercase()),
        format!("https://{UPLOAD_HOST}./u?upload_id=A"),
        "https://%64iscord-attachments-uploads-prd.storage.googleapis.com/u?upload_id=A".to_owned(),
        format!(
            "https://{}/u?upload_id=A",
            UPLOAD_HOST.replace('.', "\u{FF0E}")
        ),
        format!(
            "https://{}/u?upload_id=A",
            UPLOAD_HOST.replace('.', "\u{3002}")
        ),
        format!("https://{UPLOAD_HOST}:443/u?upload_id=A"),
        format!("https://{UPLOAD_HOST}@evil.test/u?upload_id=A"),
        format!("https://evil.test\\@{UPLOAD_HOST}/u?upload_id=A"),
        "https://xn--discord-attachments-uploads-prd-3m5d.storage.googleapis.com/u?a".to_owned(),
        format!("https://{UPLOAD_HOST}/u?upload_id=A#frag"),
        format!("https://{UPLOAD_HOST}/u"),
        format!("wss://{UPLOAD_HOST}/u?upload_id=A"),
        "not a url".to_owned(),
    ];
    for url in refused {
        assert!(upload_url(&url).is_err(), "{url}");
    }
    // The notice gets the host alone.
    assert_eq!(
        upload_url("https://evil.test/a/b?secret=1")
            .unwrap_err()
            .as_deref(),
        Some("evil.test")
    );
}

#[tokio::test(start_paused = true)]
async fn a_refused_host_gets_no_request_and_fails_the_send() {
    let w = world().await;
    *w.fake.upload_url.lock().unwrap() =
        Some(format!("https://{UPLOAD_HOST}.evil.test/u?upload_id=A"));
    w.send(w.job("x", vec![w.attach("/p/a.png", 4)])).unwrap();
    w.settle().await;
    assert_eq!(
        w.state_of(),
        Some(UploadState::Failed(UploadFailure::HostRefused))
    );
    assert!(w.fake.log().iter().all(|r| !r.url.contains("evil")));
    assert!(w.fake.upload_log().is_empty());
    assert!(w.creates().is_empty());
}

// ---- Headers (D15a, CU-2) ----

const PREFLIGHT: [&str; 12] = [
    "Origin",
    "Sec-Fetch-Site",
    "Access-Control-Request-Method",
    "Access-Control-Request-Headers",
    "Sec-Fetch-Mode",
    "User-Agent",
    "Referer",
    "Sec-Fetch-Dest",
    "Content-Length",
    "Accept",
    "Accept-Language",
    "Priority",
];
const PUT: [&str; 11] = [
    "Accept",
    "Content-Type",
    "Sec-Fetch-Site",
    "Origin",
    "Sec-Fetch-Mode",
    "User-Agent",
    "Referer",
    "Sec-Fetch-Dest",
    "Content-Length",
    "Accept-Language",
    "Priority",
];
const PROBE: [&str; 12] = [
    "Accept",
    "Content-Type",
    "Origin",
    "Content-Range",
    "Sec-Fetch-Site",
    "Sec-Fetch-Mode",
    "User-Agent",
    "Referer",
    "Sec-Fetch-Dest",
    "Content-Length",
    "Accept-Language",
    "Priority",
];

/// Every preflight, PUT and probe carries exactly Safari's list in Safari's order with A43's
/// values; none carries a token, cookie, super properties or X-Discord-* header.
#[tokio::test(start_paused = true)]
async fn upload_requests_carry_exactly_safaris_lists() {
    let w = world().await;
    w.fake.host().puts.push_back(Put::Drop(600));
    w.fake.host().preflight = allowing(Some("0"));
    w.send(w.job("x", vec![w.attach("/p/a.bin", 1000)]))
        .unwrap();
    w.settle().await;
    assert_eq!(
        w.fake.legs(),
        [
            "OPTIONS content-type",
            "PUT",
            "OPTIONS content-range,content-type",
            "PROBE",
            "OPTIONS content-range,content-type",
            "PUT+range"
        ]
    );
    let identity = Identity::current();
    let mut put_range: Vec<&str> = PUT.to_vec();
    put_range.insert(4, "Content-Range");
    for rec in w.fake.upload_log() {
        let want: &[&str] = match rec.leg().as_str() {
            "PUT" => &PUT,
            "PUT+range" => &put_range,
            "PROBE" => &PROBE,
            _ => &PREFLIGHT,
        };
        assert_eq!(rec.names(), want, "{}", rec.leg());
        assert!(!rec.authorized);
        for (name, value) in [
            ("Origin", "https://discord.com"),
            ("Referer", "https://discord.com/"),
            ("Sec-Fetch-Site", "cross-site"),
            ("Sec-Fetch-Mode", "cors"),
            ("Sec-Fetch-Dest", "empty"),
            ("Accept", "*/*"),
            ("Priority", "u=3, i"),
        ] {
            assert_eq!(rec.header(name), Some(value), "{}: {name}", rec.leg());
        }
        assert_eq!(rec.header("User-Agent"), Some(identity.user_agent()));
        assert_eq!(
            rec.header("Accept-Language"),
            Some(identity.accept_language())
        );
        if rec.method == "PUT" {
            assert_eq!(rec.header("Content-Type"), Some("application/octet-stream"));
            assert_eq!(
                rec.header("Content-Length"),
                Some(rec.body.len().to_string().as_str())
            );
        } else {
            assert_eq!(rec.header("Access-Control-Request-Method"), Some("PUT"));
            assert_eq!(rec.header("Content-Length"), Some("0"));
        }
    }
    let put = w.fake.upload_log().pop().unwrap();
    assert_eq!(put.header("Content-Range"), Some("bytes 600-999/1000"));
}

#[test]
fn an_upload_request_cannot_carry_authorization() {
    for name in ["Authorization", "authorization"] {
        let built = UploadRequest::new(
            UploadMethod::Put,
            format!("{UPLOAD_PREFIX}u?upload_id=A"),
            vec![(name.into(), TOKEN.into())],
            None,
            PUT_DEADLINE,
            RESPONSE_CAP,
        );
        assert!(built.is_err(), "{name}");
    }
}

// ---- Preflight cache (D15a) ----

/// `Access-Control-Max-Age` absent: reused for 5 s. The waits after attempts 1, 3 and 5 are 1, 4
/// and 16 s.
#[tokio::test(start_paused = true)]
async fn a_preflight_without_max_age_is_reused_for_five_seconds() {
    let w = world().await;
    for _ in 0..3 {
        w.fake.host().puts.push_back(Put::Drop(100));
    }
    w.send(w.job("x", vec![w.attach("/p/a.bin", 1000)]))
        .unwrap();
    w.settle().await;
    assert_eq!(
        w.fake.legs(),
        [
            "OPTIONS content-type",
            "PUT",
            // after 1 s: new names, new preflight
            "OPTIONS content-range,content-type",
            "PROBE",
            "PUT+range",
            // after 4 s: still within 5 s
            "PROBE",
            "PUT+range",
            // after 16 s: 20 s old, sent again
            "OPTIONS content-range,content-type",
            "PROBE",
            "PUT+range",
        ]
    );
    assert_eq!(w.upload_row(), None, "sent");
}

/// A max-age over 600 s is kept 600 s (WebKit's cap): after a PUT that hangs to its one-hour
/// deadline, the probe's names need a new preflight.
#[tokio::test(start_paused = true)]
async fn a_preflight_max_age_is_capped_at_600_seconds() {
    let w = world().await;
    w.fake.host().preflight = allowing(Some("100000"));
    w.fake.host().puts.extend([Put::Drop(100), Put::Hang]);
    w.send(w.job("x", vec![w.attach("/p/a.bin", 1000)]))
        .unwrap();
    w.settle().await;
    assert_eq!(
        w.fake.legs(),
        [
            "OPTIONS content-type",
            "PUT",
            "OPTIONS content-range,content-type",
            "PROBE",
            // hangs for an hour
            "PUT+range",
            "OPTIONS content-range,content-type",
            "PROBE",
            "PUT+range",
        ]
    );
    assert_eq!(w.upload_row(), None);
}

#[tokio::test(start_paused = true)]
async fn a_preflight_that_does_not_allow_the_put_fails_the_file_with_no_put() {
    let cases: [(u16, Vec<(String, String)>); 6] = [
        (200, {
            let mut h = allowing(None);
            h[0].1 = "https://evil.test".into();
            h
        }),
        (200, {
            let mut h = allowing(None);
            h[1].1 = "GET, POST".into();
            h
        }),
        (200, {
            let mut h = allowing(None);
            h[2].1 = "X-Other".into();
            h
        }),
        (200, allowing(None)[1..].to_vec()),
        (403, allowing(None)),
        (200, {
            let mut h = allowing(None);
            h.push(("Access-Control-Allow-Origin".into(), "*".into()));
            h
        }),
    ];
    for (code, headers) in cases {
        let w = world().await;
        w.fake.host().preflight_status = code;
        w.fake.host().preflight = headers.clone();
        w.send(w.job("x", vec![w.attach("/p/a.bin", 10)])).unwrap();
        w.settle().await;
        assert_eq!(w.fake.legs(), ["OPTIONS content-type"], "{headers:?}");
        assert_eq!(
            w.state_of(),
            Some(UploadState::Failed(UploadFailure::PreflightRefused))
        );
        assert!(w.creates().is_empty());
    }
    // `*` everywhere is allowed (no credentials).
    let w = world().await;
    w.fake.host().preflight = vec![
        ("Access-Control-Allow-Origin".into(), "*".into()),
        ("Access-Control-Allow-Methods".into(), "*".into()),
        ("Access-Control-Allow-Headers".into(), "*".into()),
    ];
    w.send(w.job("x", vec![w.attach("/p/a.bin", 10)])).unwrap();
    w.settle().await;
    assert_eq!(w.upload_row(), None);
}

// ---- Upload and resume (CU-3) ----

#[tokio::test(start_paused = true)]
async fn a_resume_after_a_dropped_connection_sends_exactly_the_rest() {
    let w = world().await;
    w.fake.host().puts.push_back(Put::Drop(600));
    let file = bytes(1000);
    w.fake.file("/p/a.bin", file.clone());
    let d = descriptor(w.fake.files.lock().unwrap()["/p/a.bin"].0.clone());
    w.send(w.job("x", vec![d])).unwrap();
    w.settle().await;
    let puts: Vec<Rec> = w
        .fake
        .upload_log()
        .into_iter()
        .filter(|r| r.method == "PUT")
        .collect();
    assert_eq!(puts.len(), 3);
    assert_eq!(puts[0].body, file);
    assert!(puts[1].body.is_empty());
    assert_eq!(puts[2].body, file[600..]);
    assert_eq!(puts[2].header("Content-Length"), Some("400"));
    let url = puts[0].url.clone();
    assert_eq!(
        w.fake.host().received[&url],
        file,
        "reassembled on the host"
    );
    assert_eq!(w.creates().len(), 1);
    assert!(
        w.fake.open.lock().unwrap().is_empty(),
        "the file was closed"
    );
}

#[tokio::test(start_paused = true)]
async fn a_hostile_resume_range_gets_no_request_after_the_probe() {
    for range in [
        Some("bytes=0-1005"),
        Some("bytes=7-9"),
        Some("garbage"),
        Some("bytes=0-12345678901234567890"),
        Some("bytes=0-"),
        Some("bytes=0-1000"),
    ] {
        let w = world().await;
        w.fake.host().puts.push_back(Put::Drop(500));
        w.fake.host().probe = Some(range.map(str::to_owned));
        w.send(w.job("x", vec![w.attach("/p/a.bin", 1000)]))
            .unwrap();
        w.settle().await;
        assert_eq!(
            w.fake.legs().last().map(String::as_str),
            Some("PROBE"),
            "{range:?}"
        );
        assert_eq!(
            w.state_of(),
            Some(UploadState::Failed(UploadFailure::UploadFailed)),
            "{range:?}"
        );
        assert!(w.creates().is_empty());
    }
    // At the bound: N + 1 == size resumes with an empty finishing PUT.
    let w = world().await;
    w.fake.host().puts.push_back(Put::Drop(1000));
    w.send(w.job("x", vec![w.attach("/p/a.bin", 1000)]))
        .unwrap();
    w.settle().await;
    let last = w.fake.upload_log().pop().unwrap();
    assert_eq!(
        (last.header("Content-Range"), last.body.len()),
        (Some("bytes */1000"), 0)
    );
    assert_eq!(w.upload_row(), None);
}

/// CU-3's bound itself: `bytes=0-N` with `N + 1 <= size`, one header, nothing else.
#[test]
fn a_resume_range_is_accepted_only_within_the_size() {
    let at = |ranges: &[&str]| {
        let response = HttpResponse {
            status: 308,
            headers: ranges
                .iter()
                .map(|r| ("Range".into(), (*r).into()))
                .collect(),
            body: None,
        };
        resume_at(&response, 1000)
    };
    assert_eq!(at(&[]), Some(0));
    assert_eq!(at(&["bytes=0-0"]), Some(1));
    assert_eq!(at(&["bytes=0-599"]), Some(600));
    assert_eq!(at(&["bytes=0-999"]), Some(1000));
    for hostile in [
        "bytes=0-1000",
        "bytes=0-1005",
        "bytes=7-9",
        "garbage",
        "bytes=0-",
        "bytes=0-12345678901234567890",
        "bytes=0-18446744073709551615",
        "bytes=0--1",
        "bytes=0-+5",
    ] {
        assert_eq!(at(&[hostile]), None, "{hostile}");
    }
    assert_eq!(at(&["bytes=0-1", "bytes=0-2"]), None, "two Range headers");
}

#[tokio::test(start_paused = true)]
async fn the_upload_stops_after_13_attempts() {
    let w = world().await;
    for _ in 0..20 {
        w.fake.host().puts.push_back(Put::Status(503));
    }
    w.fake.host().probe_status = Some(503);
    w.send(w.job("x", vec![w.attach("/p/a.bin", 100)])).unwrap();
    w.settle().await;
    let attempts = w
        .fake
        .upload_log()
        .iter()
        .filter(|r| r.method == "PUT")
        .count();
    assert_eq!(attempts, 13);
    assert_eq!(
        w.state_of(),
        Some(UploadState::Failed(UploadFailure::UploadFailed))
    );
}

/// CU-6: a file whose identity changed since it was attached (a symlink swapped in) is refused by
/// the transport: no preflight, no PUT.
#[tokio::test(start_paused = true)]
async fn a_changed_file_fails_with_no_upload_request() {
    let w = world().await;
    let mut d = w.attach("/p/a.bin", 100);
    d.file.inode += 1;
    w.send(w.job("x", vec![d])).unwrap();
    w.settle().await;
    assert!(w.fake.upload_log().is_empty());
    assert_eq!(
        w.state_of(),
        Some(UploadState::Failed(UploadFailure::FileChanged))
    );
}

#[tokio::test(start_paused = true)]
async fn progress_is_clamped_and_bumps_at_most_every_50_ms() {
    let w = world().await;
    w.fake.host().puts.push_back(Put::Hang);
    let nonce = w
        .send(w.job("x", vec![w.attach("/p/a.bin", 1000)]))
        .unwrap();
    w.until("the PUT is running", |w| w.fake.legs().len() == 2)
        .await;
    let before = w.store.revision();
    for i in 1..=20 {
        w.store.upload_progress(nonce, 1, 0, i * 10);
    }
    assert_eq!(w.store.revision(), before + 1, "one bump within 50 ms");
    tokio::time::advance(Duration::from_millis(50)).await;
    w.store.upload_progress(nonce, 1, 0, 300);
    assert_eq!(w.store.revision(), before + 2);
    w.store.upload_progress(nonce, 1, 0, 5000);
    assert_eq!(
        w.store.revision(),
        before + 3,
        "reaching the size always bumps"
    );
    let row = w.upload_row().unwrap();
    assert_eq!(row.upload.unwrap().files[0].sent, 1000, "clamped");
    // Another run's report is ignored.
    w.store.upload_progress(nonce, 2, 0, 1);
    assert_eq!(w.upload_row().unwrap().upload.unwrap().files[0].sent, 1000);
}

// ---- Limits (P4, OA A13-A17, A36) ----

#[test]
fn limits_follow_tier_boost_and_rollout_in_bytes() {
    let rollout = |bucket| Some(json!([legacy(981_354_709, bucket, 0)]));
    let none = ready(Some(0), SEND, &[], Some(json!([])));
    let classic = ready(Some(1), SEND, &[], Some(json!([])));
    let basic = ready(Some(3), SEND, &[], Some(json!([])));
    let nitro = ready(Some(2), SEND, &[], Some(json!([])));
    let nitro_rollout = ready(Some(2), SEND, &[], rollout(1));
    let nitro_unknown = ready(Some(2), SEND, &[], None);
    let boost50 = ready(Some(0), SEND, &["MAX_FILE_SIZE_50_MB"], Some(json!([])));
    let boost100 = ready(
        Some(0),
        SEND,
        &["MAX_FILE_SIZE_50_MB", "MAX_FILE_SIZE_100_MB"],
        Some(json!([])),
    );
    assert_eq!(
        (MIB_10, MIB_20, MIB_50, MIB_100, MB_500, GIB_1),
        (
            10_485_760,
            20_971_520,
            52_428_800,
            104_857_600,
            524_288_000,
            1_073_741_824
        )
    );
    for (state, channel, limit) in [
        (&none, CHANNEL, MIB_20),
        (&none, DM, MIB_20),
        (&classic, CHANNEL, MIB_50),
        (&basic, CHANNEL, MIB_50),
        (&nitro, CHANNEL, MB_500),
        (&nitro_rollout, CHANNEL, GIB_1),
        (&nitro_unknown, CHANNEL, MB_500),
        (&boost50, CHANNEL, MIB_50),
        (&boost100, CHANNEL, MIB_100),
        (&boost100, DM, MIB_20),
    ] {
        assert_eq!(file_size_limit(state, channel), limit);
        let send = |n| check_limits(state, channel, &[n], LimitCheck::Send);
        assert_eq!(send(limit), Ok(()));
        assert_eq!(send(limit + 1), Err(LimitError::FileTooLarge { limit }));
    }
    // The rollout lets a Nitro file over 500 MB through.
    assert_eq!(
        check_limits(&nitro_rollout, CHANNEL, &[MB_500 + 1], LimitCheck::Send),
        Ok(())
    );
    // Files, empty, total.
    let ten = [1u64; 10];
    assert_eq!(check_limits(&none, CHANNEL, &ten, LimitCheck::Send), Ok(()));
    assert_eq!(
        check_limits(&none, CHANNEL, &[1; 11], LimitCheck::Add),
        Err(LimitError::TooManyFiles { limit: 10 })
    );
    assert_eq!(
        check_limits(&nitro, CHANNEL, &[1, 0], LimitCheck::Add),
        Err(LimitError::EmptyFile)
    );
    let half = 300 * 1024 * 1024;
    assert_eq!(
        check_limits(&nitro, CHANNEL, &[half, half], LimitCheck::Send),
        Err(LimitError::TotalTooLarge { limit: MB_500 })
    );
    assert_eq!(
        check_limits(&nitro_rollout, CHANNEL, &[half, half], LimitCheck::Send),
        Ok(())
    );
    // At add time only an account without premium is checked for size (OA A36).
    assert_eq!(
        check_limits(&nitro, CHANNEL, &[2 * GIB_1], LimitCheck::Add),
        Ok(())
    );
    assert_eq!(
        check_limits(&nitro, CHANNEL, &[2 * GIB_1], LimitCheck::Send),
        Err(LimitError::FileTooLarge { limit: MB_500 })
    );
    assert_eq!(
        check_limits(&none, CHANNEL, &[MIB_20 + 1], LimitCheck::Add),
        Err(LimitError::FileTooLarge { limit: MIB_20 })
    );
}

#[tokio::test(start_paused = true)]
async fn a_send_over_a_limit_creates_no_row_and_sends_nothing() {
    let w = world().await;
    let big = descriptor(FileIdentity {
        path: "/p/big.bin".into(),
        device: 1,
        inode: 2,
        size: MIB_20 + 1,
        mtime_ns: 0,
    });
    assert_eq!(
        w.send(w.job("keep", vec![big])),
        Err(SendRefusal::Limits(LimitError::FileTooLarge {
            limit: MIB_20
        }))
    );
    let eleven = (0..11).map(|i| w.attach(&format!("/p/{i}"), 1)).collect();
    assert_eq!(
        w.send(w.job("keep", eleven)),
        Err(SendRefusal::Limits(LimitError::TooManyFiles { limit: 10 }))
    );
    assert_eq!(w.send(w.job("keep", vec![])), Err(SendRefusal::NoFiles));
    tokio::time::sleep(Duration::from_millis(10)).await;
    assert!(w.store.rows().is_empty());
    assert!(w.fake.api_log().is_empty());
}

// ---- Permission (CU-10) ----

#[tokio::test(start_paused = true)]
async fn attach_files_denied_or_unknown_refuses_and_a_dm_sends() {
    let denied = world_in(ready(Some(0), SEND, &[], Some(json!([]))), CHANNEL).await;
    let unknown = world_in(
        {
            let mut s = member_state();
            s.guilds[0].roles = None;
            s
        },
        CHANNEL,
    )
    .await;
    for w in [&denied, &unknown] {
        assert_eq!(
            w.send(w.job("x", vec![w.attach("/p/a.png", 3)])),
            Err(SendRefusal::NotAllowed)
        );
        tokio::time::sleep(Duration::from_millis(10)).await;
        assert!(w.fake.api_log().is_empty());
        assert!(w.store.rows().is_empty());
    }
    // A DM has no ATTACH_FILES bit and sends.
    let dm = world_in(ready(Some(0), 0, &[], Some(json!([]))), DM).await;
    let mut job = dm.job("x", vec![dm.attach("/p/a.png", 3)]);
    job.channel_id = DM;
    job.guild_id = None;
    dm.send(job).unwrap();
    dm.settle().await;
    assert_eq!(dm.slots().len(), 1);
    assert_eq!(
        dm.slots()[0].target(),
        format!("POST /api/v9/channels/{DM}/attachments")
    );
}

// ---- The pending row (CU Flow, D14a, CU-9) ----

#[tokio::test(start_paused = true)]
async fn a_failed_upload_creates_no_message_and_retry_starts_from_the_slots() {
    let w = world().await;
    w.fake.host().puts.push_back(Put::Status(403));
    let mut a = w.attach("/p/a.png", 30);
    a.description = Some("alt".into());
    let nonce = w.send(w.job("text", vec![a.clone()])).unwrap();
    // Sending: the row is the echo with its files.
    let row = w.upload_row().unwrap();
    assert_eq!((row.id, row.pending, &*row.content), (nonce, true, "text"));
    w.settle().await;
    let row = w.upload_row().unwrap();
    assert!(!row.pending);
    assert_eq!(row.failure, Some(UploadFailure::UploadFailed.text()));
    let upload = row.upload.unwrap();
    assert_eq!(
        upload.state,
        UploadState::Failed(UploadFailure::UploadFailed)
    );
    assert_eq!(upload.files[0].descriptor, a);
    assert!(
        w.creates().is_empty(),
        "an upload failure creates no message"
    );
    // Retry: a new slot request, then the message.
    assert!(w.store.retry_upload(nonce));
    w.settle().await;
    assert_eq!(w.slots().len(), 2);
    assert_eq!(w.creates().len(), 1);
    assert_eq!(w.creates()[0].json()["nonce"], nonce.to_string());
    let rows = w.store.rows();
    assert_eq!(rows.len(), 1);
    assert_eq!(
        (rows[0].id, rows[0].upload.is_none()),
        (900000000000000001, true)
    );
    assert!(!w.store.retry_upload(nonce), "nothing left to retry");
}

#[tokio::test(start_paused = true)]
async fn discard_removes_a_failed_row() {
    let w = world().await;
    w.fake.host().puts.push_back(Put::Status(400));
    let nonce = w.send(w.job("x", vec![w.attach("/p/a.png", 3)])).unwrap();
    w.settle().await;
    assert!(w.store.discard_upload(nonce));
    assert!(w.store.rows().is_empty());
    assert!(!w.store.discard_upload(nonce));
}

/// D14a, CU-9: a cancel before the create stops the upload, deletes the completed uploads by
/// their CU-8 paths and keeps the text, reply target and files in the cancelled row.
#[tokio::test(start_paused = true)]
async fn cancel_before_the_create_deletes_the_uploads_and_keeps_the_draft() {
    let w = world().await;
    *w.fake.upload_filename.lock().unwrap() = "uuid{n}/截圖 1.png".into();
    w.fake.host().puts.extend([Put::Ok, Put::Hang]);
    let files = vec![w.attach("/p/a.png", 10), w.attach("/p/b.png", 10)];
    let mut job = w.job("draft", files.clone());
    job.reply_to = Some(42);
    let nonce = w.send(job).unwrap();
    w.until("the second PUT runs", |w| w.fake.legs().len() == 4)
        .await;
    let dropped = w.fake.dropped.notified();
    assert!(w.store.cancel_upload(nonce));
    dropped.await;
    w.until("the DELETE", |w| {
        w.fake.api_log().iter().any(|r| r.method == "DELETE")
    })
    .await;
    let deletes: Vec<String> = w
        .fake
        .api_log()
        .iter()
        .filter(|r| r.method == "DELETE")
        .map(Rec::target)
        .collect();
    assert_eq!(
        deletes,
        ["DELETE /api/v9/attachments/uuid1/%E6%88%AA%E5%9C%96%201.png"]
    );
    let row = w.upload_row().unwrap();
    let upload = row.upload.unwrap();
    assert_eq!(upload.state, UploadState::Cancelled);
    assert_eq!(
        (&*row.content, upload.reply_to, row.pending),
        ("draft", Some(42), false)
    );
    assert_eq!(
        upload
            .files
            .iter()
            .map(|f| f.descriptor.clone())
            .collect::<Vec<_>>(),
        files
    );
    tokio::time::sleep(Duration::from_secs(5)).await;
    assert!(w.creates().is_empty());
    assert!(
        w.fake.open.lock().unwrap().is_empty(),
        "the file was closed"
    );
    assert!(!w.store.cancel_upload(nonce));
    assert!(w.store.discard_upload(nonce));
}

#[tokio::test(start_paused = true)]
async fn cancel_after_the_create_was_dispatched_sends_no_delete() {
    let w = world().await;
    w.fake.hang_create.store(true, Ordering::SeqCst);
    let nonce = w.send(w.job("x", vec![w.attach("/p/a.png", 10)])).unwrap();
    w.until("the create is dispatched", |w| !w.creates().is_empty())
        .await;
    assert!(!w.store.cancel_upload(nonce));
    tokio::time::sleep(Duration::from_secs(5)).await;
    assert!(w.fake.api_log().iter().all(|r| r.method != "DELETE"));
    assert_eq!(w.state_of(), Some(UploadState::Sending));
}

#[tokio::test(start_paused = true)]
async fn a_hostile_upload_filename_is_never_deleted() {
    let w = world().await;
    *w.fake.upload_filename.lock().unwrap() = "../users/@me".into();
    w.fake.host().puts.extend([Put::Ok, Put::Hang]);
    let nonce = w
        .send(w.job(
            "x",
            vec![w.attach("/p/a.png", 10), w.attach("/p/b.png", 10)],
        ))
        .unwrap();
    w.until("the second PUT runs", |w| w.fake.legs().len() == 4)
        .await;
    assert!(w.store.cancel_upload(nonce));
    tokio::time::sleep(Duration::from_secs(5)).await;
    assert!(w.fake.api_log().iter().all(|r| r.method != "DELETE"));
}

#[test]
fn delete_paths_are_encoded_and_checked() {
    for bad in [
        "../users/@me",
        "%2e%2e/x",
        "a\\..\\b",
        "a?b",
        "a#b",
        "a/./b",
        "a\u{7}b",
        "",
        "a%20b",
    ] {
        assert_eq!(delete_path(bad), None, "{bad:?}");
    }
    assert_eq!(
        delete_path("uuid/截圖 1.png").as_deref(),
        Some("/attachments/uuid/%E6%88%AA%E5%9C%96%201.png")
    );
    assert_eq!(
        delete_path("0b5a/a\"b<c>`{}.txt").as_deref(),
        Some("/attachments/0b5a/a%22b%3Cc%3E%60%7B%7D.txt")
    );
}

#[tokio::test(start_paused = true)]
async fn the_send_survives_a_channel_switch() {
    let w = world().await;
    w.fake.host().puts.push_back(Put::Status(400));
    w.send(w.job("x", vec![w.attach("/p/a.png", 3)])).unwrap();
    w.settle().await;
    let epoch = w.store.open(DM);
    tokio::time::sleep(Duration::from_millis(5)).await;
    assert!(w.store.rows().is_empty(), "not shown in another channel");
    w.store.close(epoch);
    w.store.open(CHANNEL);
    tokio::time::sleep(Duration::from_millis(5)).await;
    assert!(matches!(w.state_of(), Some(UploadState::Failed(_))));
}

// ---- Slot retry (CU-10) ----

#[tokio::test(start_paused = true)]
async fn the_slot_request_retries_transport_errors_5xx_and_429_at_most_12_times() {
    let w = world().await;
    *w.fake.api.lock().unwrap() = Some(Box::new(|rec: &Rec, n: usize| {
        if !rec.url.ends_with("/attachments") {
            return None;
        }
        Some(match n % 4 {
            0 => Ok(status(500)),
            1 => Err(TransportError::Timeout),
            2 => Ok(HttpResponse {
                status: 429,
                headers: vec![("Retry-After".into(), "2".into())],
                body: Some(br#"{"retry_after":2}"#.to_vec()),
            }),
            _ => Err(TransportError::ConnectFailed),
        })
    }));
    w.send(w.job("x", vec![w.attach("/p/a.png", 3)])).unwrap();
    w.settle().await;
    let slots = w.slots();
    assert_eq!(slots.len(), 13, "one try and 12 retries");
    assert_eq!(slots[1].header("X-Failed-Requests"), Some("1"));
    assert!(w.fake.upload_log().is_empty());
    for (code, want) in [
        (400, UploadFailure::Failed),
        (403, UploadFailure::Forbidden),
        (404, UploadFailure::Failed),
    ] {
        let w = world().await;
        *w.fake.api.lock().unwrap() = Some(Box::new(move |rec: &Rec, _| {
            rec.url.ends_with("/attachments").then(|| Ok(status(code)))
        }));
        w.send(w.job("x", vec![w.attach("/p/a.png", 3)])).unwrap();
        w.settle().await;
        assert_eq!(w.slots().len(), 1, "{code}");
        assert_eq!(w.state_of(), Some(UploadState::Failed(want)));
    }
    // 40005 is too large (OA A18).
    let w = world().await;
    *w.fake.api.lock().unwrap() = Some(Box::new(|rec: &Rec, _| {
        rec.url.ends_with("/attachments").then(|| {
            Ok(HttpResponse {
                status: 400,
                headers: vec![],
                body: Some(br#"{"code":40005,"message":"Request entity too large"}"#.to_vec()),
            })
        })
    }));
    w.send(w.job("x", vec![w.attach("/p/a.png", 3)])).unwrap();
    w.settle().await;
    assert_eq!(
        w.state_of(),
        Some(UploadState::Failed(UploadFailure::TooLarge))
    );
}

// ---- Two synthesized files (CU-1) ----

#[tokio::test(start_paused = true)]
async fn two_files_with_one_name_upload_their_own_bytes() {
    let w = world().await;
    let a = w.fake.file(
        "/tmp/velella-attachments/AAAA/message.txt",
        b"first".to_vec(),
    );
    let b = w.fake.file(
        "/tmp/velella-attachments/BBBB/message.txt",
        b"second".to_vec(),
    );
    w.send(w.job("", vec![descriptor(a)])).unwrap();
    w.send(w.job("", vec![descriptor(b)])).unwrap();
    w.until("both sent", |w| w.creates().len() == 2).await;
    let mut bodies: Vec<Vec<u8>> = w
        .fake
        .upload_log()
        .into_iter()
        .filter(|r| r.method == "PUT")
        .map(|r| r.body)
        .collect();
    bodies.sort();
    assert_eq!(bodies, [b"first".to_vec(), b"second".to_vec()]);
}

// ---- Redaction (CU-7) ----

#[test]
fn nothing_that_carries_a_path_name_url_or_alt_text_formats_it() {
    let m = "SYNTHETIC_MARKER";
    let d = AttachmentDescriptor {
        file: FileIdentity {
            path: format!("/Users/{m}/{m}.png"),
            device: 1,
            inode: 2,
            size: 3,
            mtime_ns: 4,
        },
        filename: format!("{m}.png"),
        description: Some(m.into()),
        spoiler: true,
        content_type: "image/png".into(),
        original_md5: Some(MD5.into()),
    };
    let slot = Slot::parse(&json!({"attachments": [{
        "upload_url": format!("{UPLOAD_PREFIX}{m}?upload_id={m}"),
        "upload_filename": format!("{m}/{m}.png"),
    }]}))
    .unwrap();
    let request = UploadRequest::new(
        UploadMethod::Put,
        format!("{UPLOAD_PREFIX}{m}?upload_id={m}"),
        vec![("Origin".into(), ORIGIN.into())],
        None,
        PUT_DEADLINE,
        RESPONSE_CAP,
    )
    .unwrap();
    let job = SendJob {
        channel_id: 1,
        guild_id: None,
        content: String::new(),
        reply_to: None,
        instant: false,
        files: vec![d.clone()],
    };
    let pending = crate::messages::PendingUpload {
        files: vec![crate::messages::PendingFile {
            descriptor: d.clone(),
            sent: 1,
        }],
        reply_to: None,
        instant: false,
        state: UploadState::Failed(UploadFailure::HostRefused),
    };
    for text in [
        format!("{d:?}"),
        format!("{d:#?}"),
        format!("{slot:?}"),
        format!("{request:?}"),
        format!("{job:?}"),
        format!("{pending:?}"),
        format!("{:?}", d.file),
        UploadFailure::HostRefused.text().to_owned(),
    ] {
        assert!(!text.contains("SYNTHETIC"), "{text}");
    }
    // Not vacuous.
    assert!(d.file.path.contains(m) && slot.upload_url.contains(m));
}

// ---- HEIC / JXR (D16a) ----

#[test]
fn the_conversion_config_follows_the_variation() {
    const HEIC: u64 = 2_078_450_398;
    const JXR: u64 = 3_850_701_141;
    let state = |list: Value| ready(Some(0), SEND, &[], Some(list));
    use ConversionConfig::*;
    use ConversionKind::*;
    let cases = [
        (
            json!([legacy(HEIC, 1, 0)]),
            Heic,
            Enabled {
                quality: 60,
                max_file_size_bytes: MIB_20,
            },
        ),
        (
            json!([legacy(HEIC, 2, 0)]),
            Heic,
            Enabled {
                quality: 80,
                max_file_size_bytes: MIB_20,
            },
        ),
        (json!([legacy(HEIC, 0, 0)]), Heic, Disabled),
        (json!([legacy(HEIC, 3, 0)]), Heic, Disabled),
        (json!([legacy(HEIC, 1, 1)]), Heic, Disabled),
        (json!([legacy(HEIC, -1, 0)]), Heic, Disabled),
        (json!([]), Heic, Disabled),
        (
            json!([legacy(JXR, 1, 0)]),
            Jxr,
            Enabled {
                quality: 85,
                max_file_size_bytes: MIB_50,
            },
        ),
        (json!([legacy(JXR, 2, 0)]), Jxr, Disabled),
        (json!([legacy(HEIC, 1, 0)]), Jxr, Disabled),
        (json!([[HEIC, 2, "x"]]), Heic, Unknown),
    ];
    for (list, kind, want) in cases {
        assert_eq!(
            conversion_config(&state(list.clone()), kind),
            want,
            "{list} {kind:?}"
        );
    }
    let unknown = ready(Some(0), SEND, &[], None);
    assert_eq!(conversion_config(&unknown, Heic), Unknown);
    assert_eq!(conversion_config(&unknown, Jxr), Unknown);
}
