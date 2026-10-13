//! Messages of the one open channel or DM (plan M). The buffer lives here, outside `State` (which
//! is copied on write); the session calls `observe` with every dispatch right after
//! `State::apply`, once `SessionHandle::messages()` has created the store.
//!
//! Rules, each with a named test in messages_tests.rs:
//! - Only the open channel is buffered. Opening fetches its latest 50 once; leaving cancels the
//!   fetch. History is merged into live events that arrived meanwhile (a live copy wins), sorted
//!   ascending by snowflake, deduplicated by id and capped at 200 (oldest dropped).
//! - MESSAGE_UPDATE merges only the fields it carries (Concord's partial update,
//!   refs/concord/src/discord/gateway/parser/messages.rs:943-1021; see THIRD_PARTY_NOTICES.md).
//! - Sends go only to the open channel; the local echo is reconciled on nonce and self author.
//! - Every failure is fixed text (`&'static str`): no REST error, body or header reaches a row.
//! - Mentions are resolved in one pass over the content; replacement names are never rescanned.
use crate::{
    rest::{Rest, RestError},
    session::{SessionHandle, Subscription},
    sidebar::avatar_url,
    state::{Role, State, User},
};
use serde_json::Value;
use std::{
    cmp::Reverse,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc, Mutex,
    },
    time::{SystemTime, UNIX_EPOCH},
};
use tokio::{sync::watch, task::AbortHandle, task::JoinHandle};

/// Rows kept per open channel.
const CAP: usize = 200;
const DISCORD_EPOCH_MS: u64 = 1_420_070_400_000;
const CAPTCHA_TEXT: &str = "Discord asked for a captcha; send this from the official app";

/// Load state of the open channel; failures carry fixed text only.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LoadState {
    Loading,
    Loaded,
    Failed(&'static str),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MessageImage {
    pub url: String,
    /// Both > 0 (checked when parsed).
    pub width: u32,
    pub height: u32,
}

/// The message a row replies to.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MessageReply {
    pub author_name: String,
    pub content: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MessageRow {
    /// The nonce's value for an unconfirmed local echo.
    pub id: u64,
    pub author_id: u64,
    pub author_name: String,
    pub author_avatar_url: String,
    /// Highest coloured role (0xRRGGBB); `None` without a member or a coloured role.
    pub author_color: Option<u32>,
    /// Mentions resolved; otherwise verbatim (no Markdown, no links).
    pub content: String,
    /// Creation time from the snowflake id, in Unix milliseconds.
    pub timestamp_ms: u64,
    pub edited: bool,
    pub images: Vec<MessageImage>,
    /// Filenames of the other attachments.
    pub files: Vec<String>,
    pub reply: Option<MessageReply>,
    /// A local echo not yet confirmed by Discord.
    pub pending: bool,
    /// A local echo whose send failed.
    pub failure: Option<&'static str>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MessagesSnapshot {
    pub channel_id: u64,
    pub load: LoadState,
    /// Ascending; unconfirmed and failed echoes last, in send order.
    pub rows: Vec<MessageRow>,
}

// ---- Parsing ----

fn id(v: &Value) -> Option<u64> {
    v.as_str()
        .and_then(|s| s.parse().ok())
        .or_else(|| v.as_u64())
        .filter(|n| *n != 0)
}

fn text(v: &Value, key: &str) -> Option<String> {
    v.get(key).and_then(Value::as_str).map(str::to_owned)
}

fn user(v: &Value) -> Option<User> {
    Some(User {
        id: v.get("id").and_then(id)?,
        username: text(v, "username").unwrap_or_default(),
        global_name: text(v, "global_name"),
        avatar: text(v, "avatar"),
        discriminator: text(v, "discriminator"),
    })
}

/// The first non-empty of `member.nick`, `global_name` and `username`.
fn display_name(user: &User, nick: Option<&str>) -> String {
    [nick, user.global_name.as_deref()]
        .into_iter()
        .flatten()
        .find(|n| !n.is_empty())
        .unwrap_or(&user.username)
        .to_owned()
}

#[derive(Clone, Debug)]
enum Attachment {
    Image(MessageImage),
    File(String),
}

/// An image needs an image (or absent) content type, a URL and both dimensions > 0; anything
/// else is listed by filename.
fn attachment(a: &Value) -> Option<Attachment> {
    let filename = text(a, "filename")?;
    let dim = |key| {
        a.get(key)
            .and_then(Value::as_u64)
            .and_then(|n| u32::try_from(n).ok())
            .filter(|n| *n > 0)
    };
    let image_type = a
        .get("content_type")
        .and_then(Value::as_str)
        .is_none_or(|t| t.starts_with("image/"));
    Some(
        match (image_type, text(a, "url"), dim("width"), dim("height")) {
            (true, Some(url), Some(width), Some(height)) => {
                Attachment::Image(MessageImage { url, width, height })
            }
            _ => Attachment::File(filename),
        },
    )
}

fn attachments(v: Option<&Value>) -> Vec<Attachment> {
    v.and_then(Value::as_array)
        .map(|a| a.iter().filter_map(attachment).collect())
        .unwrap_or_default()
}

/// (user id, display name) of each mentioned user, from the payload's own `mentions`.
fn mentions(v: Option<&Value>) -> Vec<(u64, String)> {
    v.and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .filter_map(|m| {
                    let u = user(m)?;
                    let nick = m.get("member").and_then(|mb| mb.get("nick")?.as_str());
                    Some((u.id, display_name(&u, nick)))
                })
                .collect()
        })
        .unwrap_or_default()
}

#[derive(Clone, Debug)]
struct Stored {
    id: u64,
    author: User,
    nick: Option<String>,
    roles: Vec<u64>,
    content: String,
    edited: bool,
    attachments: Vec<Attachment>,
    mentions: Vec<(u64, String)>,
    reply: Option<Box<Stored>>,
    nonce: Option<String>,
}

/// A message object from the gateway or REST. A referenced message is parsed one level deep.
fn parse(v: &Value, nested: bool) -> Option<Stored> {
    let member = v.get("member").filter(|m| m.is_object());
    Some(Stored {
        id: v.get("id").and_then(id)?,
        author: user(v.get("author")?)?,
        nick: member.and_then(|m| text(m, "nick")),
        roles: member
            .and_then(|m| m.get("roles")?.as_array())
            .map(|r| r.iter().filter_map(id).collect())
            .unwrap_or_default(),
        content: text(v, "content").unwrap_or_default(),
        edited: v.get("edited_timestamp").is_some_and(|t| !t.is_null()),
        attachments: attachments(v.get("attachments")),
        mentions: mentions(v.get("mentions")),
        reply: if nested {
            None
        } else {
            v.get("referenced_message")
                .and_then(|r| parse(r, true))
                .map(Box::new)
        },
        nonce: v.get("nonce").and_then(|n| match n {
            Value::String(s) => Some(s.clone()),
            Value::Number(n) => Some(n.to_string()),
            _ => None,
        }),
    })
}

impl Stored {
    /// MESSAGE_UPDATE: only the fields the payload carries; absent `attachments` = unchanged.
    fn merge(&mut self, d: &Value) {
        if let Some(content) = text(d, "content") {
            self.content = content;
        }
        if d.get("edited_timestamp").is_some_and(|t| !t.is_null()) {
            self.edited = true;
        }
        if d.get("attachments").is_some() {
            self.attachments = attachments(d.get("attachments"));
        }
        if d.get("mentions").is_some() {
            self.mentions = mentions(d.get("mentions"));
        }
    }

    fn name(&self) -> String {
        display_name(&self.author, self.nick.as_deref())
    }
}

// ---- Resolution against the state ----

/// What rows need from `State`: the open channel's guild roles and channel names (empty for DMs).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct Context {
    roles: Vec<Role>,
    channels: Vec<(u64, String)>,
}

fn context(state: &State, channel_id: u64) -> Context {
    state
        .guilds
        .iter()
        .find(|g| g.channels.iter().any(|c| c.id == channel_id))
        .map(|g| Context {
            roles: g.roles.clone().unwrap_or_default(),
            channels: g
                .channels
                .iter()
                .filter_map(|c| Some((c.id, c.name.clone()?)))
                .collect(),
        })
        .unwrap_or_default()
}

/// Colour of the highest-position role with a non-zero colour among `roles` (W0.1 data).
fn colour(roles: &[u64], ctx: &Context) -> Option<u32> {
    ctx.roles
        .iter()
        .filter(|r| r.color != 0 && roles.contains(&r.id))
        .max_by_key(|r| (r.position, Reverse(r.id)))
        .map(|r| r.color)
}

enum Mention {
    User(u64),
    Channel(u64),
    Role(u64),
}

/// `<@id>`, `<@!id>`, `<#id>` or `<@&id>` at the start of `s`, with its byte length.
fn mention_at(s: &str) -> Option<(Mention, usize)> {
    let end = s.find('>')?;
    let inner = &s[1..end];
    let (kind, digits): (fn(u64) -> Mention, &str) = if let Some(d) = inner.strip_prefix("@&") {
        (Mention::Role, d)
    } else if let Some(d) = inner.strip_prefix("@!").or_else(|| inner.strip_prefix('@')) {
        (Mention::User, d)
    } else {
        (Mention::Channel, inner.strip_prefix('#')?)
    };
    if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    Some((kind(digits.parse().ok()?), end + 1))
}

/// One left-to-right pass: each token is replaced once and the output is never rescanned, so a
/// name that itself contains `<@&id>` stays literal.
fn resolve(content: &str, mentioned: &[(u64, String)], ctx: &Context) -> String {
    let mut out = String::with_capacity(content.len());
    let mut rest = content;
    while let Some(start) = rest.find('<') {
        out.push_str(&rest[..start]);
        rest = &rest[start..];
        let Some((mention, len)) = mention_at(rest) else {
            out.push('<');
            rest = &rest[1..];
            continue;
        };
        match mention {
            Mention::User(id) => match mentioned.iter().find(|(m, _)| *m == id) {
                Some((_, name)) => {
                    out.push('@');
                    out.push_str(name);
                }
                None => out.push_str("@unknown-user"),
            },
            Mention::Channel(id) => match ctx.channels.iter().find(|(c, _)| *c == id) {
                Some((_, name)) => {
                    out.push('#');
                    out.push_str(name);
                }
                None => out.push_str("#unknown"),
            },
            Mention::Role(id) => match ctx.roles.iter().find(|r| r.id == id) {
                Some(Role {
                    name: Some(name), ..
                }) => {
                    out.push('@');
                    out.push_str(name);
                }
                _ => out.push_str("@deleted-role"),
            },
        }
        rest = &rest[len..];
    }
    out.push_str(rest);
    out
}

fn row(m: &Stored, ctx: &Context, pending: bool, failure: Option<&'static str>) -> MessageRow {
    let (images, files) =
        m.attachments
            .iter()
            .fold((Vec::new(), Vec::new()), |(mut images, mut files), a| {
                match a {
                    Attachment::Image(i) => images.push(i.clone()),
                    Attachment::File(f) => files.push(f.clone()),
                }
                (images, files)
            });
    MessageRow {
        id: m.id,
        author_id: m.author.id,
        author_name: m.name(),
        author_avatar_url: avatar_url(
            m.author.id,
            m.author.avatar.as_deref(),
            m.author.discriminator.as_deref(),
        ),
        author_color: colour(&m.roles, ctx),
        content: resolve(&m.content, &m.mentions, ctx),
        timestamp_ms: (m.id >> 22) + DISCORD_EPOCH_MS,
        edited: m.edited,
        images,
        files,
        reply: m.reply.as_ref().map(|r| MessageReply {
            author_name: r.name(),
            content: resolve(&r.content, &r.mentions, ctx),
        }),
        pending,
        failure,
    }
}

fn load_failure(e: RestError) -> &'static str {
    match e {
        RestError::Unauthorized => "Couldn't load messages: Discord did not accept this sign-in",
        RestError::Forbidden => "Couldn't load messages: no access to this channel",
        RestError::RateLimited => "Couldn't load messages: rate limited",
        _ => "Couldn't load messages",
    }
}

fn send_failure(e: RestError) -> &'static str {
    match e {
        RestError::Captcha => CAPTCHA_TEXT,
        RestError::Unauthorized => "Not sent: Discord did not accept this sign-in",
        RestError::Forbidden => "Not sent: no permission to send here",
        RestError::RateLimited => "Not sent: rate limited",
        _ => "Not sent",
    }
}

/// A fresh nonce per send: a snowflake for now, strictly increasing within the process (Concord's
/// `next_message_nonce`, refs/concord/src/discord/commands.rs:31-46).
fn next_nonce() -> u64 {
    static LAST: AtomicU64 = AtomicU64::new(0);
    let now_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(DISCORD_EPOCH_MS, |d| {
            u64::try_from(d.as_millis()).unwrap_or(u64::MAX)
        });
    let candidate = now_ms.saturating_sub(DISCORD_EPOCH_MS) << 22;
    let mut previous = LAST.load(Ordering::Relaxed);
    loop {
        let next = candidate.max(previous.saturating_add(1)).max(1);
        match LAST.compare_exchange_weak(previous, next, Ordering::Relaxed, Ordering::Relaxed) {
            Ok(_) => return next,
            Err(seen) => previous = seen,
        }
    }
}

// ---- Store ----

struct Echo {
    nonce: String,
    message: Stored,
    failure: Option<&'static str>,
}

struct Open {
    epoch: u64,
    channel_id: u64,
    load: LoadState,
    /// Ascending by id, no duplicates, at most `CAP`.
    messages: Vec<Stored>,
    echoes: Vec<Echo>,
    /// Deleted while the history fetch ran, so the fetch cannot bring them back.
    deleted: Vec<u64>,
    fetch: Option<AbortHandle>,
}

impl Open {
    /// Inserts in id order; `replace` decides whether an existing copy is overwritten.
    fn insert(&mut self, m: Stored, replace: bool) {
        match self.messages.binary_search_by_key(&m.id, |s| s.id) {
            Ok(i) if replace => self.messages[i] = m,
            Ok(_) => {}
            Err(i) => self.messages.insert(i, m),
        }
        let over = self.messages.len().saturating_sub(CAP);
        self.messages.drain(..over);
    }
}

#[derive(Default)]
struct Inner {
    epoch: u64,
    open: Option<Open>,
}

pub(crate) struct MessageStore {
    rest: Rest,
    inner: Mutex<Inner>,
    /// Bumped on every buffer change; message subscriptions wait on it.
    revision: watch::Sender<u64>,
    /// Dispatch names seen, so a test can show the observer is wired.
    #[cfg(test)]
    pub(crate) seen: std::sync::Mutex<Vec<String>>,
}

impl MessageStore {
    pub(crate) fn new(rest: Rest) -> MessageStore {
        MessageStore {
            rest,
            inner: Mutex::default(),
            revision: watch::channel(0).0,
            #[cfg(test)]
            seen: Default::default(),
        }
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Inner> {
        self.inner.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn bump(&self) {
        self.revision.send_modify(|r| *r += 1);
    }

    /// Makes `channel_id` the open channel (dropping the previous buffer and cancelling its fetch)
    /// and starts the one history GET. Returns the open's epoch. Needs a runtime context.
    pub(crate) fn open(self: &Arc<Self>, channel_id: u64) -> u64 {
        let mut inner = self.lock();
        if let Some(fetch) = inner.open.take().and_then(|o| o.fetch) {
            fetch.abort();
        }
        inner.epoch += 1;
        let epoch = inner.epoch;
        let store = self.clone();
        // The task cannot finish before `inner` is released: `history` takes the same lock.
        let fetch = tokio::spawn(async move {
            let result = store.rest.channel_messages(channel_id).await;
            store.history(epoch, result);
        });
        inner.open = Some(Open {
            epoch,
            channel_id,
            load: LoadState::Loading,
            messages: Vec::new(),
            echoes: Vec::new(),
            deleted: Vec::new(),
            fetch: Some(fetch.abort_handle()),
        });
        drop(inner);
        self.bump();
        epoch
    }

    /// Leaves the channel opened at `epoch` (a no-op once another open replaced it) and cancels
    /// its in-flight GET.
    pub(crate) fn close(&self, epoch: u64) {
        let mut inner = self.lock();
        if inner.open.as_ref().is_some_and(|o| o.epoch == epoch) {
            if let Some(fetch) = inner.open.take().and_then(|o| o.fetch) {
                fetch.abort();
            }
            drop(inner);
            self.bump();
        }
    }

    fn history(&self, epoch: u64, result: Result<Vec<Value>, RestError>) {
        let mut inner = self.lock();
        let Some(open) = inner.open.as_mut().filter(|o| o.epoch == epoch) else {
            return;
        };
        open.fetch = None;
        match result {
            Ok(list) => {
                for m in list.iter().filter_map(|v| parse(v, false)) {
                    if !open.deleted.contains(&m.id) {
                        open.insert(m, false);
                    }
                }
                open.deleted = Vec::new();
                open.load = LoadState::Loaded;
            }
            Err(e) => open.load = LoadState::Failed(load_failure(e)),
        }
        drop(inner);
        self.bump();
    }

    /// Runs on the session task for every dispatch; must not block.
    pub(crate) fn observe(&self, name: &str, d: &Value) {
        #[cfg(test)]
        self.seen.lock().unwrap().push(name.to_owned());
        if !matches!(name, "MESSAGE_CREATE" | "MESSAGE_UPDATE" | "MESSAGE_DELETE") {
            return;
        }
        let mut inner = self.lock();
        let Some(open) = inner
            .open
            .as_mut()
            .filter(|o| d.get("channel_id").and_then(id) == Some(o.channel_id))
        else {
            return;
        };
        match name {
            "MESSAGE_CREATE" => {
                let Some(m) = parse(d, false) else { return };
                // Our own echo: same nonce and sent by us (the echo's author is the self user).
                open.echoes.retain(|e| {
                    !(m.nonce.as_deref() == Some(e.nonce.as_str())
                        && m.author.id == e.message.author.id)
                });
                open.insert(m, true);
            }
            "MESSAGE_UPDATE" => {
                let Some(target) = d.get("id").and_then(id) else {
                    return;
                };
                let Some(m) = open.messages.iter_mut().find(|m| m.id == target) else {
                    // ponytail: an edit to a message not buffered yet (fetch still running) is
                    // dropped; the fetch then shows the pre-edit copy until the next open.
                    return;
                };
                m.merge(d);
            }
            _ => {
                let Some(target) = d.get("id").and_then(id) else {
                    return;
                };
                open.messages.retain(|m| m.id != target);
                if open.load == LoadState::Loading {
                    open.deleted.push(target);
                }
            }
        }
        drop(inner);
        self.bump();
    }

    /// Adds a local echo to `channel_id` if it is the open channel; returns (epoch, nonce).
    fn begin_send(&self, channel_id: u64, content: &str, me: User) -> Option<(u64, String)> {
        let mut inner = self.lock();
        let open = inner.open.as_mut().filter(|o| o.channel_id == channel_id)?;
        let nonce = next_nonce();
        open.echoes.push(Echo {
            nonce: nonce.to_string(),
            message: Stored {
                id: nonce,
                author: me,
                nick: None,
                roles: Vec::new(),
                content: content.to_owned(),
                edited: false,
                attachments: Vec::new(),
                mentions: Vec::new(),
                reply: None,
                nonce: None,
            },
            failure: None,
        });
        let epoch = open.epoch;
        drop(inner);
        self.bump();
        Some((epoch, nonce.to_string()))
    }

    fn finish_send(&self, epoch: u64, nonce: &str, result: Result<Value, RestError>) {
        let mut inner = self.lock();
        let Some(open) = inner.open.as_mut().filter(|o| o.epoch == epoch) else {
            return;
        };
        match result {
            Ok(v) => {
                open.echoes.retain(|e| e.nonce != nonce);
                if let Some(m) = parse(&v, false) {
                    open.insert(m, false); // the gateway copy, if it came first, carries `member`
                }
            }
            Err(e) => {
                if let Some(echo) = open.echoes.iter_mut().find(|e| e.nonce == nonce) {
                    echo.failure = Some(send_failure(e));
                }
            }
        }
        drop(inner);
        self.bump();
    }

    /// The buffer opened at `epoch`, or `None` once it was left or replaced.
    fn snapshot(&self, epoch: u64, ctx: &Context) -> Option<MessagesSnapshot> {
        let inner = self.lock();
        let open = inner.open.as_ref().filter(|o| o.epoch == epoch)?;
        let rows = open
            .messages
            .iter()
            .map(|m| row(m, ctx, false, None))
            .chain(
                open.echoes
                    .iter()
                    .map(|e| row(&e.message, ctx, e.failure.is_none(), e.failure)),
            )
            .collect();
        Some(MessagesSnapshot {
            channel_id: open.channel_id,
            load: open.load.clone(),
            rows,
        })
    }
}

/// A running `SessionHandle::subscribe_messages`. Dropping it leaves the channel: deliveries stop
/// and an in-flight history GET is cancelled.
pub struct MessageSubscription {
    store: Arc<MessageStore>,
    epoch: u64,
    task: JoinHandle<()>,
    _context: Subscription,
}

impl Drop for MessageSubscription {
    fn drop(&mut self) {
        self.task.abort();
        self.store.close(self.epoch);
    }
}

impl SessionHandle {
    /// Opens `channel_id` (a guild channel or DM) as the one open channel and delivers its
    /// snapshot now and after every change, until the subscription is dropped, another channel is
    /// opened, or the session ends. Must be called inside a runtime context; `deliver` runs on a
    /// runtime thread and must not block.
    pub fn subscribe_messages(
        &self,
        channel_id: u64,
        deliver: impl Fn(MessagesSnapshot) + Send + 'static,
    ) -> MessageSubscription {
        let store = self.messages();
        let epoch = store.open(channel_id);
        // Role and channel names follow the state; this subscription ends with the session,
        // which closes `names` and so ends the task below.
        let (names_tx, mut names) = watch::channel(Context::default());
        let state = self.subscribe(
            move |state| context(state, channel_id),
            move |c| {
                names_tx.send_replace(c);
            },
        );
        let mut revision = store.revision.subscribe();
        let buffer = store.clone();
        let task = tokio::spawn(async move {
            let mut last = None;
            loop {
                revision.borrow_and_update();
                let ctx = names.borrow_and_update().clone();
                let Some(snapshot) = buffer.snapshot(epoch, &ctx) else {
                    return;
                };
                if last.as_ref() != Some(&snapshot) {
                    last = Some(snapshot.clone());
                    deliver(snapshot);
                }
                tokio::select! {
                    changed = revision.changed() => if changed.is_err() { return },
                    changed = names.changed() => if changed.is_err() { return },
                }
            }
        });
        MessageSubscription {
            store,
            epoch,
            task,
            _context: state,
        }
    }

    /// Sends `content` to `channel_id` when that is the open channel, with a local echo and a
    /// fresh nonce; never retried. Returns `None` (nothing sent) for an empty or whitespace-only
    /// text, before READY, or when `channel_id` is not the open channel. The outcome shows in the
    /// snapshot: the echo is replaced by the real message, or marked failed with fixed text. Must
    /// be called inside a runtime context.
    pub fn send_message(&self, channel_id: u64, content: String) -> Option<JoinHandle<()>> {
        if content.trim().is_empty() {
            return None;
        }
        let state = self.snapshot();
        let me = state.users.get(&state.user_id?)?.clone();
        let store = self.messages();
        let (epoch, nonce) = store.begin_send(channel_id, &content, me)?;
        Some(tokio::spawn(async move {
            let result = store
                .rest
                .create_message(channel_id, &content, &nonce)
                .await;
            store.finish_send(epoch, &nonce, result);
        }))
    }
}

#[cfg(test)]
#[path = "messages_tests.rs"]
mod tests;
