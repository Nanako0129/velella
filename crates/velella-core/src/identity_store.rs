//! The local identity store (menu plan ID, Local identity store row; I-7, D16, D17): one JSON file
//! under velella's Application Support directory, written atomically (temporary file, then
//! rename). Every field has its own schema and is validated on load and on write; an invalid
//! field is dropped on its own and never fails a request.
//!
//! - `fetch`: the last accepted identity fetch (chunk hash, build number, capabilities pair,
//!   fetched_at). Values only: nothing fetched is kept as text.
//! - per Discord user id: `installation_id` (D16: that account's first non-empty READY
//!   `apex_experiments.installation`, never generated here), `qos_derived` (READY `qos_token`) and
//!   `heartbeat_session`. Nothing stored for one account is ever read for another.
use crate::identity_fetch::{BUILD_MAX, BUILD_MIN, SUPPORTED_PAIRS};
use crate::proto::{base64_decode, Message, Value as PValue};
use serde_json::{json, Map, Value};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

const FILE: &str = "identity.json";
/// `qos_derived` cap (contract: at most 4 KiB), for the base64 text and the decoded bytes.
const MAX_QOS: usize = 4096;
/// The official heartbeat session expires 30 minutes after its last use (I§2).
pub(crate) const HEARTBEAT_SESSION_TTL_MS: u64 = 30 * 60 * 1000;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct FetchCache {
    pub chunk_hash: String,
    pub build: u64,
    pub pair: (u64, u64),
    pub fetched_at_ms: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct HeartbeatSession {
    pub uuid: String,
    pub created_at_ms: u64,
    pub last_used_ms: u64,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct Account {
    installation_id: Option<String>,
    qos_derived: Option<String>,
    heartbeat_session: Option<HeartbeatSession>,
}

impl Account {
    fn is_empty(&self) -> bool {
        *self == Account::default()
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct Data {
    fetch: Option<FetchCache>,
    accounts: BTreeMap<u64, Account>,
}

/// The store of one launch. `None` path: in memory only (tests of other modules, a launch without
/// a directory); nothing is written anywhere.
pub(crate) struct IdentityStore {
    path: Option<PathBuf>,
    data: Mutex<Data>,
}

pub(crate) fn chunk_hash_ok(h: &str) -> bool {
    h.len() == 16 && h.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
}

pub(crate) fn build_ok(b: u64) -> bool {
    (BUILD_MIN..=BUILD_MAX).contains(&b)
}

pub(crate) fn installation_id_ok(id: &str) -> bool {
    (1..=128).contains(&id.len())
        && id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b".:_-".contains(&b))
}

/// Standard (or url-safe) base64 of `DerivedQosData {1 claims bytes, 2 signature bytes, 3 key_id
/// uint32}`: only those fields, with those wire types, at most 4 KiB.
pub(crate) fn qos_derived_bytes(b64: &str) -> Option<Vec<u8>> {
    if b64.is_empty() || b64.len() > MAX_QOS {
        return None;
    }
    let bytes = base64_decode(b64).ok()?;
    let message = Message::decode(&bytes, MAX_QOS).ok()?;
    message
        .fields
        .iter()
        .all(|f| match (f.number, &f.value) {
            (1 | 2, PValue::Len(_)) => true,
            (3, PValue::Varint(v)) => u32::try_from(*v).is_ok(),
            _ => false,
        })
        .then_some(bytes)
}

/// Lowercase `8-4-4-4-12` hex.
fn uuid_ok(u: &str) -> bool {
    let parts: Vec<&str> = u.split('-').collect();
    parts.iter().map(|p| p.len()).eq([8, 4, 4, 4, 12])
        && parts
            .iter()
            .all(|p| p.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f')))
}

fn session_ok(s: &HeartbeatSession, now_ms: u64) -> bool {
    uuid_ok(&s.uuid) && s.created_at_ms <= s.last_used_ms && s.last_used_ms <= now_ms
}

fn fetch_ok(f: &FetchCache, now_ms: u64) -> bool {
    chunk_hash_ok(&f.chunk_hash)
        && build_ok(f.build)
        && SUPPORTED_PAIRS.contains(&f.pair)
        && f.fetched_at_ms <= now_ms
}

fn parse_fetch(v: &Value, now_ms: u64) -> Option<FetchCache> {
    let pair = v.get("capabilities")?.as_array()?;
    let f = FetchCache {
        chunk_hash: v.get("chunk_hash")?.as_str()?.to_owned(),
        build: v.get("build_number")?.as_u64()?,
        pair: match pair.as_slice() {
            [a, b] => (a.as_u64()?, b.as_u64()?),
            _ => return None,
        },
        fetched_at_ms: v.get("fetched_at")?.as_u64()?,
    };
    fetch_ok(&f, now_ms).then_some(f)
}

fn parse_session(v: &Value, now_ms: u64) -> Option<HeartbeatSession> {
    let s = HeartbeatSession {
        uuid: v.get("uuid")?.as_str()?.to_owned(),
        created_at_ms: v.get("created_at")?.as_u64()?,
        last_used_ms: v.get("last_used")?.as_u64()?,
    };
    session_ok(&s, now_ms).then_some(s)
}

fn parse_account(v: &Value, now_ms: u64) -> Account {
    let s = |k: &str| v.get(k).and_then(Value::as_str);
    Account {
        installation_id: s("installation_id")
            .filter(|i| installation_id_ok(i))
            .map(str::to_owned),
        qos_derived: s("qos_derived")
            .filter(|q| qos_derived_bytes(q).is_some())
            .map(str::to_owned),
        heartbeat_session: v
            .get("heartbeat_session")
            .and_then(|h| parse_session(h, now_ms)),
    }
}

fn parse(text: &str, now_ms: u64) -> Data {
    let Ok(root) = serde_json::from_str::<Value>(text) else {
        return Data::default();
    };
    let accounts = root
        .get("accounts")
        .and_then(Value::as_object)
        .into_iter()
        .flatten()
        .filter_map(|(id, v)| {
            let id = id.parse::<u64>().ok().filter(|i| *i != 0)?;
            Some((id, parse_account(v, now_ms))).filter(|(_, a)| !a.is_empty())
        })
        .collect();
    Data {
        fetch: root.get("fetch").and_then(|f| parse_fetch(f, now_ms)),
        accounts,
    }
}

fn render(data: &Data) -> String {
    let mut accounts = Map::new();
    for (id, a) in &data.accounts {
        let mut o = Map::new();
        if let Some(i) = &a.installation_id {
            o.insert("installation_id".into(), json!(i));
        }
        if let Some(q) = &a.qos_derived {
            o.insert("qos_derived".into(), json!(q));
        }
        if let Some(h) = &a.heartbeat_session {
            o.insert(
                "heartbeat_session".into(),
                json!({"uuid": h.uuid, "created_at": h.created_at_ms, "last_used": h.last_used_ms}),
            );
        }
        accounts.insert(id.to_string(), Value::Object(o));
    }
    let mut root = Map::new();
    if let Some(f) = &data.fetch {
        root.insert(
            "fetch".into(),
            json!({"chunk_hash": f.chunk_hash, "build_number": f.build,
                "capabilities": [f.pair.0, f.pair.1], "fetched_at": f.fetched_at_ms}),
        );
    }
    root.insert("accounts".into(), Value::Object(accounts));
    Value::Object(root).to_string()
}

/// Temporary file in the same directory, then rename: a reader sees the old file or the new one.
/// Owner-only permissions; any failure leaves the old file and is only logged by name.
fn write_atomically(path: &Path, text: &str) -> std::io::Result<()> {
    use std::io::Write;
    use std::os::unix::fs::OpenOptionsExt;
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let tmp = path.with_extension(format!("tmp-{}", std::process::id()));
    // A leftover from a crashed write is removed; `create_new` never follows what sits there.
    let _ = std::fs::remove_file(&tmp);
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&tmp)?;
    file.write_all(text.as_bytes())?;
    file.sync_all()?;
    std::fs::rename(&tmp, path)
}

impl IdentityStore {
    pub(crate) fn memory() -> IdentityStore {
        IdentityStore {
            path: None,
            data: Mutex::default(),
        }
    }

    /// Loads `<dir>/identity.json`; a missing or unreadable file is an empty store.
    pub(crate) fn open(dir: &Path, now_ms: u64) -> IdentityStore {
        let path = dir.join(FILE);
        let data = std::fs::read_to_string(&path)
            .map(|t| parse(&t, now_ms))
            .unwrap_or_default();
        IdentityStore {
            path: Some(path),
            data: Mutex::new(data),
        }
    }

    fn update(&self, change: impl FnOnce(&mut Data)) {
        let mut data = self.data.lock().unwrap_or_else(|e| e.into_inner());
        let before = data.clone();
        change(&mut data);
        data.accounts.retain(|_, a| !a.is_empty());
        if *data == before {
            return;
        }
        if let Some(path) = &self.path {
            if write_atomically(path, &render(&data)).is_err() {
                tracing::warn!("identity store not written");
            }
        }
    }

    fn read<T>(&self, f: impl FnOnce(&Data) -> T) -> T {
        f(&self.data.lock().unwrap_or_else(|e| e.into_inner()))
    }

    fn account<T>(&self, user: u64, f: impl FnOnce(&Account) -> Option<T>) -> Option<T> {
        self.read(|d| d.accounts.get(&user).and_then(f))
    }

    /// The accepted fetch values, re-validated against `now_ms` (a future `fetched_at` is dropped,
    /// so a tampered time cannot extend the 24-hour skip).
    pub(crate) fn fetch_cache(&self, now_ms: u64) -> Option<FetchCache> {
        self.read(|d| d.fetch.clone())
            .filter(|f| fetch_ok(f, now_ms))
    }

    pub(crate) fn set_fetch_cache(&self, cache: FetchCache) {
        if fetch_ok(&cache, cache.fetched_at_ms) {
            self.update(|d| d.fetch = Some(cache));
        }
    }

    pub(crate) fn installation_id(&self, user: u64) -> Option<String> {
        self.account(user, |a| a.installation_id.clone())
    }

    /// Stores `id` for `user` only when valid and none is stored yet (the first non-empty value
    /// wins, D16). Returns the account's id afterwards.
    pub(crate) fn adopt_installation_id(&self, user: u64, id: &str) -> Option<String> {
        if installation_id_ok(id) {
            self.update(|d| {
                let a = d.accounts.entry(user).or_default();
                if a.installation_id.is_none() {
                    a.installation_id = Some(id.to_owned());
                }
            });
        }
        self.installation_id(user)
    }

    /// The decoded `DerivedQosData` bytes for `user`'s next normal IDENTIFY.
    pub(crate) fn qos_derived(&self, user: u64) -> Option<Vec<u8>> {
        self.account(user, |a| {
            a.qos_derived.as_deref().and_then(qos_derived_bytes)
        })
    }

    pub(crate) fn set_qos_derived(&self, user: u64, b64: &str) {
        if qos_derived_bytes(b64).is_some() {
            self.update(|d| d.accounts.entry(user).or_default().qos_derived = Some(b64.to_owned()));
        }
    }

    pub(crate) fn heartbeat_session(&self, user: u64, now_ms: u64) -> Option<HeartbeatSession> {
        self.account(user, |a| a.heartbeat_session.clone())
            .filter(|s| session_ok(s, now_ms))
    }

    pub(crate) fn set_heartbeat_session(&self, user: u64, session: Option<HeartbeatSession>) {
        self.update(|d| match session {
            Some(s) => d.accounts.entry(user).or_default().heartbeat_session = Some(s),
            None => {
                if let Some(a) = d.accounts.get_mut(&user) {
                    a.heartbeat_session = None;
                }
            }
        });
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::proto::base64_encode;

    const NOW: u64 = 1_800_000_000_000;
    const MAIN: u64 = 111;
    const ALT: u64 = 222;

    fn dir() -> PathBuf {
        let d = std::env::temp_dir().join(format!(
            "velella-store-{}-{}",
            std::process::id(),
            crate::identify::uuid_v4()
        ));
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    pub(crate) fn qos_b64() -> String {
        let mut m = Message::new();
        m.push_string(1, "claims");
        m.push_string(2, "sig");
        m.push_varint(3, 7);
        base64_encode(&m.encode())
    }

    fn session() -> HeartbeatSession {
        HeartbeatSession {
            uuid: "0b5f7c4e-1d2a-4c3b-8a9f-0123456789ab".into(),
            created_at_ms: NOW - 10,
            last_used_ms: NOW - 5,
        }
    }

    fn cache() -> FetchCache {
        FetchCache {
            chunk_hash: "f0cfe6eb7552ae67".into(),
            build: 640_102,
            pair: (1_767_421, 1_734_653),
            fetched_at_ms: NOW - 1,
        }
    }

    #[test]
    fn values_round_trip_through_the_file_and_stay_per_account() {
        let d = dir();
        let store = IdentityStore::open(&d, NOW);
        store.set_fetch_cache(cache());
        assert_eq!(
            store.adopt_installation_id(MAIN, "1757.abcDEF_-:x"),
            Some("1757.abcDEF_-:x".into())
        );
        // The first value wins.
        assert_eq!(
            store.adopt_installation_id(MAIN, "other"),
            Some("1757.abcDEF_-:x".into())
        );
        store.set_qos_derived(MAIN, &qos_b64());
        store.set_heartbeat_session(MAIN, Some(session()));
        let again = IdentityStore::open(&d, NOW);
        assert_eq!(again.fetch_cache(NOW), Some(cache()));
        assert_eq!(
            again.installation_id(MAIN).as_deref(),
            Some("1757.abcDEF_-:x")
        );
        assert!(again.qos_derived(MAIN).is_some());
        assert_eq!(again.heartbeat_session(MAIN, NOW), Some(session()));
        // Nothing of main's is ever read for alt.
        assert_eq!(again.installation_id(ALT), None);
        assert_eq!(again.qos_derived(ALT), None);
        assert_eq!(again.heartbeat_session(ALT, NOW), None);
        again.set_heartbeat_session(MAIN, None);
        assert_eq!(
            IdentityStore::open(&d, NOW).heartbeat_session(MAIN, NOW),
            None
        );
        // Written owner-only, and no temporary file is left.
        use std::os::unix::fs::PermissionsExt;
        let meta = std::fs::metadata(d.join(FILE)).unwrap();
        assert_eq!(meta.permissions().mode() & 0o777, 0o600);
        assert_eq!(std::fs::read_dir(&d).unwrap().count(), 1);
        std::fs::remove_dir_all(d).unwrap();
    }

    /// One tampered value per field: that field is dropped, the others survive.
    #[test]
    fn a_tampered_field_is_dropped_on_its_own() {
        let good_account = json!({"installation_id": "inst", "qos_derived": qos_b64(),
            "heartbeat_session": {"uuid": session().uuid, "created_at": NOW - 10, "last_used": NOW - 5}});
        let good_fetch = json!({"chunk_hash": "f0cfe6eb7552ae67", "build_number": 640102,
            "capabilities": [1767421, 1734653], "fetched_at": NOW - 1});
        let tampered: [(&str, &str, Value); 12] = [
            ("fetch", "chunk_hash", json!("F0CFE6EB7552AE67")),
            ("fetch", "chunk_hash", json!("../../etc/passwd")),
            ("fetch", "build_number", json!(640101)),
            ("fetch", "build_number", json!("640102")),
            ("fetch", "capabilities", json!([253, 1734653])),
            ("fetch", "fetched_at", json!(NOW + 1)),
            ("account", "installation_id", json!("has space")),
            ("account", "installation_id", json!("x".repeat(129))),
            ("account", "qos_derived", json!("not base64!")),
            (
                "account",
                "qos_derived",
                json!(base64_encode(&{
                    let mut m = Message::new();
                    m.push_varint(1, 5); // claims must be bytes
                    m.encode()
                })),
            ),
            (
                "account",
                "heartbeat_session",
                json!({"uuid": "not-a-uuid", "created_at": NOW, "last_used": NOW}),
            ),
            (
                "account",
                "heartbeat_session",
                json!({"uuid": session().uuid, "created_at": NOW, "last_used": NOW - 1}),
            ),
        ];
        for (section, field, bad) in tampered {
            let (mut fetch, mut account) = (good_fetch.clone(), good_account.clone());
            match section {
                "fetch" => fetch[field] = bad.clone(),
                _ => account[field] = bad.clone(),
            }
            let text = json!({"fetch": fetch, "accounts": {MAIN.to_string(): account}}).to_string();
            let d = dir();
            std::fs::write(d.join(FILE), text).unwrap();
            let s = IdentityStore::open(&d, NOW);
            let fetch_kept = s.fetch_cache(NOW).is_some();
            let kept = [
                s.installation_id(MAIN).is_some(),
                s.qos_derived(MAIN).is_some(),
                s.heartbeat_session(MAIN, NOW).is_some(),
            ];
            match (section, field) {
                ("fetch", _) => assert!(!fetch_kept && kept == [true; 3], "{field} {bad}"),
                (_, "installation_id") => assert!(fetch_kept && kept == [false, true, true]),
                (_, "qos_derived") => assert!(fetch_kept && kept == [true, false, true]),
                _ => assert!(fetch_kept && kept == [true, true, false]),
            }
            std::fs::remove_dir_all(d).unwrap();
        }
        // An unreadable file and an id key that is no snowflake are empty, never an error.
        let d = dir();
        std::fs::write(d.join(FILE), "{not json").unwrap();
        assert_eq!(IdentityStore::open(&d, NOW).fetch_cache(NOW), None);
        std::fs::write(
            d.join(FILE),
            json!({"accounts": {"main": good_account}}).to_string(),
        )
        .unwrap();
        assert_eq!(IdentityStore::open(&d, NOW).installation_id(MAIN), None);
        std::fs::remove_dir_all(d).unwrap();
    }

    #[test]
    fn invalid_values_are_never_written() {
        let s = IdentityStore::memory();
        assert_eq!(s.adopt_installation_id(MAIN, "bad\r\nX: 1"), None);
        s.set_qos_derived(MAIN, "%%%");
        assert_eq!(s.qos_derived(MAIN), None);
        s.set_fetch_cache(FetchCache {
            pair: (1, 2),
            ..cache()
        });
        assert_eq!(s.fetch_cache(NOW), None);
    }
}
