//! The message-menu REST actions (menu plan F2): reactions, edit, delete, pins, acks, mark
//! unread, saved messages, and the thread unarchive and join some of them need first. Shapes per
//! `menu-requests.md` §1-§9 and §17.4, encoded per §18.1-§18.3; retries per the F2 retry table.
//!
//! - Paths are composed from `u64` ids and typed parts only; an emoji segment is validated and
//!   percent-encoded here (§18.2, S-1), and `Rest` re-checks every composed URL.
//! - Every input is a closed type (S-13). Thread preconditions come from `ThreadState`, computed
//!   in core (`thread_state`), never from booleans the UI passes (S-7).
//! - Each write that may be clicked twice holds a single-flight key for its whole run (S-16).
use crate::{
    rest::{ActionError, Call, Flight, Rest, Retry},
    rest_messages::AllowedMentions,
    session::SessionHandle,
    state::{get_id, get_str, parse_user, State, User},
    transport::Method,
};
use serde::Serialize;
use serde_json::Value;
use std::sync::Arc;

/// An emoji as a reaction path names it: a custom emoji `name:id`, or a unicode emoji's own
/// characters (a skin-tone variant is its full sequence, §18.2).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Emoji {
    Custom { name: String, id: u64 },
    Unicode(String),
}

/// Percent-encodes every byte outside RFC 3986's `pchar` (so `%`, `/`, `?`, `#`, `\` and all
/// non-ASCII are always encoded, as UTF-8); `:` and `@` stay literal, as the browser leaves
/// them (§18.2).
fn encode_segment(s: &str) -> String {
    let mut out = String::with_capacity(s.len() * 3);
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || b"-._~!$&'()*+,;=:@".contains(&b) {
            out.push(char::from(b));
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

impl Emoji {
    /// The path segment, or `InvalidInput`. A custom name is `[A-Za-z0-9_]{1,32}`. A unicode
    /// emoji must contain a non-ASCII character, no control character, and no ASCII other than
    /// the keycap bases `#`, `*` and digits: so `.`, `..`, `%2e%2e`, `a/b`, `%` and empty are
    /// refused before any encoding.
    pub(crate) fn segment(&self) -> Result<String, ActionError> {
        let ok = match self {
            Emoji::Custom { name, .. } => {
                (1..=32).contains(&name.len())
                    && name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
            }
            Emoji::Unicode(s) => {
                s.len() <= 128
                    && !s.is_ascii()
                    && s.chars().all(|c| {
                        !c.is_control() && (!c.is_ascii() || matches!(c, '#' | '*' | '0'..='9'))
                    })
            }
        };
        if !ok {
            return Err(ActionError::InvalidInput);
        }
        Ok(match self {
            Emoji::Custom { name, id } => format!("{name}:{id}"),
            Emoji::Unicode(s) => encode_segment(s),
        })
    }
}

/// `type` of a reaction (§1.3): NORMAL 0, BURST 1 (super reaction). VOTE belongs to polls.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReactionType {
    Normal,
    Burst,
}

impl ReactionType {
    fn number(self) -> u8 {
        match self {
            Self::Normal => 0,
            Self::Burst => 1,
        }
    }
}

/// `location` of a reaction add or remove (§1.4).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReactionLocation {
    Message,
    ForumToolbar,
    MobileMediaViewer,
    MessageHoverBar,
    MessageInlineButton,
    MessageContextMenu,
    MessageReactionPicker,
    MessageShortcut,
    DoubleTap,
    InAppNotification,
}

impl ReactionLocation {
    fn as_str(self) -> &'static str {
        match self {
            Self::Message => "Message",
            Self::ForumToolbar => "Forum Toolbar",
            Self::MobileMediaViewer => "Mobile Media Viewer",
            Self::MessageHoverBar => "Message Hover Bar",
            Self::MessageInlineButton => "Message Inline Button",
            Self::MessageContextMenu => "Message Context Menu",
            Self::MessageReactionPicker => "Message Reaction Picker",
            Self::MessageShortcut => "Message Shortcut",
            Self::DoubleTap => "Double Tap",
            Self::InAppNotification => "In App Notification",
        }
    }
}

/// Saved-message `source` (§9.2).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SaveSource {
    ReactionButton,
    ContextMenu,
    LongPressActionSheet,
    MessageToolbar,
    ForLaterList,
    NotificationsInbox,
}

/// The `flags` of an automatic ack (§8.7): 1 for guild channels, 2 for threads.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AckFlags {
    Guild,
    Thread,
}

/// A fact core knows, knows to be false, or does not know.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Tri {
    #[default]
    Unknown,
    Yes,
    No,
}

/// What core knows about the target channel as a thread (S-7). Every field Unknown by default:
/// with Unknown, F2 never unarchives or joins; the action is sent as is and a 50083 comes back
/// as its typed error.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ThreadState {
    pub archived: Tri,
    pub locked: Tri,
    pub joined: Tri,
    pub manage_threads: Tri,
}

/// The thread state of `channel_id` from current state. All Unknown until slice F1 fills it
/// from `threads.rs` and MANAGE_THREADS (phase 3).
pub fn thread_state(_state: &State, _channel_id: u64) -> ThreadState {
    ThreadState::default()
}

/// The guild a channel or thread belongs to, from current state; `None` for DMs and unknown
/// channels.
fn guild_of(state: &State, channel_id: u64) -> Option<u64> {
    state
        .guilds
        .iter()
        .find(|g| g.channels.iter().any(|c| c.id == channel_id))
        .map(|g| g.id)
        .or_else(|| state.threads.get(channel_id).map(|t| t.guild_id))
}

/// `encodeURIComponent` (§18.1): everything but `A-Z a-z 0-9 - _ . ! ~ * ' ( )` as UTF-8
/// `%XX`; a space is `%20`.
fn encode_component(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || b"-_.!~*'()".contains(&b) {
            out.push(char::from(b));
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

/// A query in insertion order with absent values dropped (§18.1); "" when empty.
fn query(pairs: &[(&str, Option<String>)]) -> String {
    let joined: Vec<String> = pairs
        .iter()
        .filter_map(|(k, v)| Some(format!("{k}={}", encode_component(v.as_deref()?))))
        .collect();
    if joined.is_empty() {
        String::new()
    } else {
        format!("?{}", joined.join("&"))
    }
}

/// An ISO-8601 instant as Discord returns it (`pinned_at`): digits, letters, `:.+-` only.
fn iso_cursor(s: &str) -> bool {
    (1..=64).contains(&s.len())
        && s.bytes()
            .all(|b| b.is_ascii_alphanumeric() || b":.+-".contains(&b))
}

/// `Date.prototype.toISOString` of `ms` (UTC, milliseconds, `Z`), years 0000-9999 only.
fn iso_from_ms(ms: i64) -> Option<String> {
    if !(0..=253_402_300_799_999).contains(&ms) {
        return None;
    }
    let (days, rem) = (ms.div_euclid(86_400_000), ms.rem_euclid(86_400_000));
    // Howard Hinnant's civil_from_days.
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    Some(format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}.{:03}Z",
        rem / 3_600_000,
        rem / 60_000 % 60,
        rem / 1000 % 60,
        rem % 1000
    ))
}

/// One pinned message of a pins page (§6.6).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PinnedMessage {
    pub message_id: u64,
    /// The paging cursor: pass the last item's value as `before` for the next page.
    pub pinned_at: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PinsPage {
    pub items: Vec<PinnedMessage>,
    pub has_more: bool,
}

/// A saved-message record's `save_data` (§9.1, §9.7).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SavedMessage {
    pub channel_id: u64,
    pub message_id: u64,
    pub saved_at: Option<String>,
    /// Absent for a plain bookmark.
    pub due_at: Option<String>,
}

fn saved_message(record: &Value) -> Option<SavedMessage> {
    let data = record.get("save_data")?;
    Some(SavedMessage {
        channel_id: get_id(data, "channel_id")?,
        message_id: get_id(data, "message_id")?,
        saved_at: get_str(data, "saved_at"),
        due_at: get_str(data, "due_at"),
    })
}

#[derive(Serialize)]
struct Unarchive {
    archived: bool,
}

#[derive(Serialize)]
struct Edit<'a> {
    content: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    allowed_mentions: Option<AllowedMentions>,
}

#[derive(Serialize)]
struct ManualAck {
    manual: bool,
    mention_count: u32,
}

/// `token` is sent as `null` when there is none (first ack); `last_viewed` and `flags` are
/// omitted when absent (§8.7, §18.3).
#[derive(Serialize)]
struct Ack<'a> {
    token: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    last_viewed: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    flags: Option<u8>,
}

#[derive(Serialize)]
struct Save {
    #[serde(skip_serializing_if = "Option::is_none")]
    due_at: Option<String>,
    source: SaveSource,
}

impl Rest {
    /// `PATCH /channels/{id}` `{"archived":false}` (§17.4); never retried.
    async fn unarchive_thread(&self, channel_id: u64) -> Result<(), ActionError> {
        let call = Call::new(Method::Patch, &format!("/channels/{channel_id}"))?
            .json(&Unarchive { archived: false })?;
        self.call::<serde::de::IgnoredAny>(&call, Retry::None)
            .await
            .map(drop)
    }

    /// `unarchiveThreadIfNecessary` (§17.4): only when archived is known true and MANAGE_THREADS
    /// is known or locked is known false. Its failure stops the caller's request.
    async fn unarchive_if_needed(
        &self,
        channel_id: u64,
        thread: &ThreadState,
    ) -> Result<(), ActionError> {
        if thread.archived == Tri::Yes
            && (thread.manage_threads == Tri::Yes || thread.locked == Tri::No)
        {
            self.unarchive_thread(channel_id).await?;
        }
        Ok(())
    }

    /// `POST /channels/{id}/thread-members/@me?location=Mark%20Unread` (§17.4); never retried.
    async fn join_thread(&self, channel_id: u64) -> Result<(), ActionError> {
        let path = format!(
            "/channels/{channel_id}/thread-members/@me{}",
            query(&[("location", Some("Mark Unread".into()))])
        );
        self.call::<serde::de::IgnoredAny>(&Call::new(Method::Post, &path)?, Retry::None)
            .await
            .map(drop)
    }

    async fn unit(&self, call: Call, retry: Retry) -> Result<(), ActionError> {
        self.call::<serde::de::IgnoredAny>(&call, retry)
            .await
            .map(drop)
    }

    /// `PUT …/reactions/{emoji}/@me?location&type` (§1.1); no body.
    pub(crate) async fn add_reaction(
        &self,
        channel_id: u64,
        message_id: u64,
        emoji: &Emoji,
        kind: ReactionType,
        location: ReactionLocation,
        thread: &ThreadState,
    ) -> Result<(), ActionError> {
        let segment = emoji.segment()?;
        let path = format!(
            "/channels/{channel_id}/messages/{message_id}/reactions/{segment}/@me{}",
            query(&[
                ("location", Some(location.as_str().into())),
                ("type", Some(kind.number().to_string())),
            ])
        );
        let call = Call::new(Method::Put, &path)?;
        let _flight = self.flight(Flight::Reaction(message_id, segment, kind.number()))?;
        self.unarchive_if_needed(channel_id, thread).await?;
        self.unit(call, Retry::Reaction).await
    }

    /// `DELETE …/reactions/{emoji}/{type}/{user}?location&burst` (§1.5, §3.3); `user` `None` is
    /// `@me`. Own removal retries as a reaction add does; another user's is never retried.
    #[allow(clippy::too_many_arguments)]
    pub(crate) async fn remove_reaction(
        &self,
        channel_id: u64,
        message_id: u64,
        emoji: &Emoji,
        kind: ReactionType,
        user: Option<u64>,
        location: ReactionLocation,
        thread: &ThreadState,
    ) -> Result<(), ActionError> {
        let segment = emoji.segment()?;
        let who = user.map_or_else(|| "@me".to_owned(), |u| u.to_string());
        let path = format!(
            "/channels/{channel_id}/messages/{message_id}/reactions/{segment}/{}/{who}{}",
            kind.number(),
            query(&[
                ("location", Some(location.as_str().into())),
                ("burst", Some((kind == ReactionType::Burst).to_string())),
            ])
        );
        let call = Call::new(Method::Delete, &path)?;
        let retry = if user.is_none() {
            Retry::Reaction
        } else {
            Retry::None
        };
        let _flight = self.flight(Flight::Reaction(message_id, segment, kind.number()))?;
        self.unarchive_if_needed(channel_id, thread).await?;
        self.unit(call, retry).await
    }

    /// `GET …/reactions/{emoji}?limit&after&type` (§2.1): one page of reactors, `limit` 1-100.
    pub(crate) async fn list_reactors(
        &self,
        channel_id: u64,
        message_id: u64,
        emoji: &Emoji,
        kind: ReactionType,
        limit: u8,
        after: Option<u64>,
    ) -> Result<Vec<User>, ActionError> {
        if !(1..=100).contains(&limit) {
            return Err(ActionError::InvalidInput);
        }
        let path = format!(
            "/channels/{channel_id}/messages/{message_id}/reactions/{}{}",
            emoji.segment()?,
            query(&[
                ("limit", Some(limit.to_string())),
                ("after", after.map(|a| a.to_string())),
                ("type", Some(kind.number().to_string())),
            ])
        );
        let users: Vec<Value> = self
            .call(&Call::new(Method::Get, &path)?, Retry::None)
            .await?;
        users
            .iter()
            .map(parse_user)
            .collect::<Option<_>>()
            .ok_or(ActionError::BadResponse)
    }

    /// `DELETE …/reactions/{emoji}` (§3.2); never retried.
    pub(crate) async fn remove_emoji_reactions(
        &self,
        channel_id: u64,
        message_id: u64,
        emoji: &Emoji,
        thread: &ThreadState,
    ) -> Result<(), ActionError> {
        let segment = emoji.segment()?;
        let path = format!("/channels/{channel_id}/messages/{message_id}/reactions/{segment}");
        let call = Call::new(Method::Delete, &path)?;
        let _flight = self.flight(Flight::Reaction(message_id, segment, u8::MAX))?;
        self.unarchive_if_needed(channel_id, thread).await?;
        self.unit(call, Retry::None).await
    }

    /// `DELETE …/reactions` (§3.1); retried as a reaction add.
    pub(crate) async fn remove_all_reactions(
        &self,
        channel_id: u64,
        message_id: u64,
        thread: &ThreadState,
    ) -> Result<(), ActionError> {
        let path = format!("/channels/{channel_id}/messages/{message_id}/reactions");
        let call = Call::new(Method::Delete, &path)?;
        let _flight = self.flight(Flight::Reaction(message_id, String::new(), u8::MAX))?;
        self.unarchive_if_needed(channel_id, thread).await?;
        self.unit(call, Retry::Reaction).await
    }

    /// `PATCH /channels/{c}/messages/{m}` `{content, allowed_mentions?}` (§4.1-§4.2).
    /// `keep_reply_ping_off`: the message is a reply whose author is not in its mentions.
    pub(crate) async fn edit_message(
        &self,
        channel_id: u64,
        message_id: u64,
        content: &str,
        keep_reply_ping_off: bool,
    ) -> Result<(), ActionError> {
        let call = Call::new(
            Method::Patch,
            &format!("/channels/{channel_id}/messages/{message_id}"),
        )?
        .json(&Edit {
            content,
            allowed_mentions: keep_reply_ping_off.then(AllowedMentions::reply_ping_off),
        })?;
        let _flight = self.flight(Flight::Edit(message_id))?;
        self.unit(call, Retry::Edit).await
    }

    /// `DELETE /channels/{c}/messages/{m}` (§5.1); never retried.
    pub(crate) async fn delete_message(
        &self,
        channel_id: u64,
        message_id: u64,
        thread: &ThreadState,
    ) -> Result<(), ActionError> {
        let call = Call::new(
            Method::Delete,
            &format!("/channels/{channel_id}/messages/{message_id}"),
        )?;
        let _flight = self.flight(Flight::Delete(message_id))?;
        self.unarchive_if_needed(channel_id, thread).await?;
        self.unit(call, Retry::None).await
    }

    /// `PUT /channels/{c}/messages/pins/{m}` (§6.1); never retried.
    pub(crate) async fn pin_message(
        &self,
        channel_id: u64,
        message_id: u64,
        thread: &ThreadState,
    ) -> Result<(), ActionError> {
        let call = Call::new(
            Method::Put,
            &format!("/channels/{channel_id}/messages/pins/{message_id}"),
        )?;
        let _flight = self.flight(Flight::Pin(message_id))?;
        self.unarchive_if_needed(channel_id, thread).await?;
        self.unit(call, Retry::None).await
    }

    /// `DELETE /channels/{c}/messages/pins/{m}` (§6.1); never retried.
    pub(crate) async fn unpin_message(
        &self,
        channel_id: u64,
        message_id: u64,
    ) -> Result<(), ActionError> {
        let call = Call::new(
            Method::Delete,
            &format!("/channels/{channel_id}/messages/pins/{message_id}"),
        )?;
        let _flight = self.flight(Flight::Pin(message_id))?;
        self.unit(call, Retry::None).await
    }

    /// `GET /channels/{c}/messages/pins?limit=25&before=…` (§6.6), `retries:2`.
    pub(crate) async fn list_pins(
        &self,
        channel_id: u64,
        before: Option<&str>,
    ) -> Result<PinsPage, ActionError> {
        if before.is_some_and(|b| !iso_cursor(b)) {
            return Err(ActionError::InvalidInput);
        }
        let path = format!(
            "/channels/{channel_id}/messages/pins{}",
            query(&[
                ("limit", Some("25".into())),
                ("before", before.map(str::to_owned)),
            ])
        );
        let page: Value = self
            .call(&Call::new(Method::Get, &path)?, Retry::Statuses(2))
            .await?;
        let items = page
            .get("items")
            .and_then(Value::as_array)
            .ok_or(ActionError::BadResponse)?
            .iter()
            .map(|item| {
                Some(PinnedMessage {
                    message_id: get_id(item.get("message")?, "id")?,
                    pinned_at: get_str(item, "pinned_at")?,
                })
            })
            .collect::<Option<_>>()
            .ok_or(ActionError::BadResponse)?;
        Ok(PinsPage {
            items,
            has_more: page
                .get("has_more")
                .and_then(Value::as_bool)
                .unwrap_or(false),
        })
    }

    /// `POST /channels/{c}/pins/ack` (§6.8); no body, never retried.
    pub(crate) async fn ack_pins(&self, channel_id: u64) -> Result<(), ActionError> {
        let call = Call::new(Method::Post, &format!("/channels/{channel_id}/pins/ack"))?;
        self.unit(call, Retry::None).await
    }

    /// The automatic ack (§8.7): `POST /channels/{c}/messages/{m}/ack`
    /// `{token, last_viewed?, flags?}`. Returns the response's `token` for the caller's next ack;
    /// F2 keeps none.
    pub(crate) async fn ack_message(
        &self,
        channel_id: u64,
        message_id: u64,
        token: Option<&str>,
        last_viewed: Option<u32>,
        flags: Option<AckFlags>,
    ) -> Result<Option<String>, ActionError> {
        let call = Call::new(
            Method::Post,
            &format!("/channels/{channel_id}/messages/{message_id}/ack"),
        )?
        .json(&Ack {
            token,
            last_viewed,
            flags: flags.map(|f| match f {
                AckFlags::Guild => 1,
                AckFlags::Thread => 2,
            }),
        })?;
        let body: Value = self.call(&call, Retry::None).await?;
        Ok(get_str(&body, "token"))
    }

    /// Mark Unread (§8.1, §17.4): `POST /channels/{c}/messages/{ack}/ack`
    /// `{manual:true, mention_count}`. `ack_message_id` is the message before the chosen one and
    /// `mention_count` is counted by the caller (§8.2-§8.3). An archived thread is unarchived
    /// first (no MANAGE_THREADS or locked check; its failure stops the ack); a thread not joined
    /// is joined (its failure does not).
    pub(crate) async fn mark_unread(
        &self,
        channel_id: u64,
        ack_message_id: u64,
        mention_count: u32,
        thread: &ThreadState,
    ) -> Result<(), ActionError> {
        let call = Call::new(
            Method::Post,
            &format!("/channels/{channel_id}/messages/{ack_message_id}/ack"),
        )?
        .json(&ManualAck {
            manual: true,
            mention_count,
        })?;
        if thread.archived == Tri::Yes {
            self.unarchive_thread(channel_id).await?;
        }
        if thread.joined == Tri::No {
            // §17.4: joinThread's error is caught and not rethrown.
            let _ = self.join_thread(channel_id).await;
        }
        self.unit(call, Retry::None).await
    }

    /// `PUT /users/@me/saved-messages/{c}/{m}` `{due_at?, source}` (§9.1); `due_at_ms` is UTC
    /// milliseconds, sent as `toISOString` would.
    pub(crate) async fn save_message(
        &self,
        channel_id: u64,
        message_id: u64,
        due_at_ms: Option<i64>,
        source: SaveSource,
    ) -> Result<SavedMessage, ActionError> {
        let due_at = due_at_ms
            .map(|ms| iso_from_ms(ms).ok_or(ActionError::InvalidInput))
            .transpose()?;
        let call = Call::new(
            Method::Put,
            &format!("/users/@me/saved-messages/{channel_id}/{message_id}"),
        )?
        .json(&Save { due_at, source })?
        .keep_message();
        let record: Value = self.call(&call, Retry::None).await?;
        saved_message(&record).ok_or(ActionError::BadResponse)
    }

    /// `DELETE /users/@me/saved-messages/{c}/{m}` (§9.1).
    pub(crate) async fn unsave_message(
        &self,
        channel_id: u64,
        message_id: u64,
    ) -> Result<(), ActionError> {
        let call = Call::new(
            Method::Delete,
            &format!("/users/@me/saved-messages/{channel_id}/{message_id}"),
        )?
        .keep_message();
        self.unit(call, Retry::None).await
    }

    /// `GET /users/@me/saved-messages` → `{results}` (§9.1).
    pub(crate) async fn list_saved_messages(&self) -> Result<Vec<SavedMessage>, ActionError> {
        let call = Call::new(Method::Get, "/users/@me/saved-messages")?;
        let body: Value = self.call(&call, Retry::None).await?;
        body.get("results")
            .and_then(Value::as_array)
            .ok_or(ActionError::BadResponse)?
            .iter()
            .map(saved_message)
            .collect::<Option<_>>()
            .ok_or(ActionError::BadResponse)
    }
}

/// The session's actions with the state they were taken at: thread preconditions and guild ids
/// come from that snapshot, never from the caller.
#[derive(Clone)]
pub struct Actions {
    rest: Rest,
    state: Arc<State>,
}

impl SessionHandle {
    /// The session's REST actions, with the current state snapshot.
    pub fn actions(&self) -> Actions {
        Actions {
            rest: self.rest(),
            state: self.snapshot(),
        }
    }
}

impl Actions {
    fn thread(&self, channel_id: u64) -> ThreadState {
        thread_state(&self.state, channel_id)
    }

    pub async fn add_reaction(
        &self,
        channel_id: u64,
        message_id: u64,
        emoji: Emoji,
        kind: ReactionType,
        location: ReactionLocation,
    ) -> Result<(), ActionError> {
        let thread = self.thread(channel_id);
        self.rest
            .add_reaction(channel_id, message_id, &emoji, kind, location, &thread)
            .await
    }

    /// `user` `None` removes the self user's reaction.
    pub async fn remove_reaction(
        &self,
        channel_id: u64,
        message_id: u64,
        emoji: Emoji,
        kind: ReactionType,
        user: Option<u64>,
        location: ReactionLocation,
    ) -> Result<(), ActionError> {
        let thread = self.thread(channel_id);
        self.rest
            .remove_reaction(
                channel_id, message_id, &emoji, kind, user, location, &thread,
            )
            .await
    }

    pub async fn list_reactors(
        &self,
        channel_id: u64,
        message_id: u64,
        emoji: Emoji,
        kind: ReactionType,
        limit: u8,
        after: Option<u64>,
    ) -> Result<Vec<User>, ActionError> {
        self.rest
            .list_reactors(channel_id, message_id, &emoji, kind, limit, after)
            .await
    }

    pub async fn remove_emoji_reactions(
        &self,
        channel_id: u64,
        message_id: u64,
        emoji: Emoji,
    ) -> Result<(), ActionError> {
        let thread = self.thread(channel_id);
        self.rest
            .remove_emoji_reactions(channel_id, message_id, &emoji, &thread)
            .await
    }

    pub async fn remove_all_reactions(
        &self,
        channel_id: u64,
        message_id: u64,
    ) -> Result<(), ActionError> {
        let thread = self.thread(channel_id);
        self.rest
            .remove_all_reactions(channel_id, message_id, &thread)
            .await
    }

    pub async fn edit_message(
        &self,
        channel_id: u64,
        message_id: u64,
        content: String,
        keep_reply_ping_off: bool,
    ) -> Result<(), ActionError> {
        self.rest
            .edit_message(channel_id, message_id, &content, keep_reply_ping_off)
            .await
    }

    pub async fn delete_message(
        &self,
        channel_id: u64,
        message_id: u64,
    ) -> Result<(), ActionError> {
        let thread = self.thread(channel_id);
        self.rest
            .delete_message(channel_id, message_id, &thread)
            .await
    }

    pub async fn pin_message(&self, channel_id: u64, message_id: u64) -> Result<(), ActionError> {
        let thread = self.thread(channel_id);
        self.rest.pin_message(channel_id, message_id, &thread).await
    }

    pub async fn unpin_message(&self, channel_id: u64, message_id: u64) -> Result<(), ActionError> {
        self.rest.unpin_message(channel_id, message_id).await
    }

    pub async fn list_pins(
        &self,
        channel_id: u64,
        before: Option<String>,
    ) -> Result<PinsPage, ActionError> {
        self.rest.list_pins(channel_id, before.as_deref()).await
    }

    pub async fn ack_pins(&self, channel_id: u64) -> Result<(), ActionError> {
        self.rest.ack_pins(channel_id).await
    }

    pub async fn ack_message(
        &self,
        channel_id: u64,
        message_id: u64,
        token: Option<String>,
        last_viewed: Option<u32>,
        flags: Option<AckFlags>,
    ) -> Result<Option<String>, ActionError> {
        self.rest
            .ack_message(channel_id, message_id, token.as_deref(), last_viewed, flags)
            .await
    }

    pub async fn mark_unread(
        &self,
        channel_id: u64,
        ack_message_id: u64,
        mention_count: u32,
    ) -> Result<(), ActionError> {
        let thread = self.thread(channel_id);
        self.rest
            .mark_unread(channel_id, ack_message_id, mention_count, &thread)
            .await
    }

    pub async fn save_message(
        &self,
        channel_id: u64,
        message_id: u64,
        due_at_ms: Option<i64>,
        source: SaveSource,
    ) -> Result<SavedMessage, ActionError> {
        self.rest
            .save_message(channel_id, message_id, due_at_ms, source)
            .await
    }

    pub async fn unsave_message(
        &self,
        channel_id: u64,
        message_id: u64,
    ) -> Result<(), ActionError> {
        self.rest.unsave_message(channel_id, message_id).await
    }

    pub async fn list_saved_messages(&self) -> Result<Vec<SavedMessage>, ActionError> {
        self.rest.list_saved_messages().await
    }

    /// Sends `content` to `channel_id` (§18.4), a reply to `reply_to` of the same channel when
    /// given, with the reply's mention on or off. No local echo (the existing send keeps that
    /// until slice B moves its caller here). Returns the created message's id.
    pub async fn send_message(
        &self,
        channel_id: u64,
        content: String,
        reply_to: Option<u64>,
        mention: bool,
    ) -> Result<u64, ActionError> {
        if content.trim().is_empty() {
            return Err(ActionError::InvalidInput);
        }
        let reply = reply_to.map(|message_id| crate::rest_messages::Reply {
            guild_id: guild_of(&self.state, channel_id),
            message_id,
            mention,
        });
        let nonce = crate::rest_messages::nonce();
        let sent = self
            .rest
            .send_message(channel_id, &content, &nonce, reply)
            .await?;
        get_id(&sent, "id").ok_or(ActionError::BadResponse)
    }

    /// Forwards message `message_id` of `channel_id` to `destination_id` (§7.1); the origin
    /// guild comes from current state. `silent` sets SUPPRESS_NOTIFICATIONS.
    pub async fn forward_message(
        &self,
        destination_id: u64,
        channel_id: u64,
        message_id: u64,
        silent: bool,
    ) -> Result<(), ActionError> {
        let guild_id = guild_of(&self.state, channel_id);
        let nonce = crate::rest_messages::nonce();
        self.rest
            .forward_message(
                destination_id,
                guild_id,
                channel_id,
                message_id,
                &nonce,
                silent,
            )
            .await
            .map(drop)
    }

    /// The forward's optional comment (§7.3): a second message to `destination_id`.
    pub async fn forward_comment(
        &self,
        destination_id: u64,
        content: String,
        silent: bool,
    ) -> Result<(), ActionError> {
        if content.trim().is_empty() {
            return Err(ActionError::InvalidInput);
        }
        let nonce = crate::rest_messages::nonce();
        self.rest
            .forward_comment(destination_id, &content, &nonce, silent)
            .await
            .map(drop)
    }

    /// Sends a verification code to the phone of MFA challenge `id` (§18.6).
    pub async fn mfa_send_sms(&self, id: u64) -> Result<(), ActionError> {
        self.rest.mfa_send_sms(id).await
    }

    /// Answers MFA challenge `id` and re-sends the request that asked for it (§18.6).
    pub async fn mfa_submit(
        &self,
        id: u64,
        kind: crate::rest::MfaKind,
        data: String,
    ) -> Result<(), ActionError> {
        self.rest.mfa_submit(id, kind, &data).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn iso_matches_to_iso_string() {
        assert_eq!(iso_from_ms(0).unwrap(), "1970-01-01T00:00:00.000Z");
        // 2026-10-17T09:30:05.123Z
        assert_eq!(
            iso_from_ms(1_792_229_405_123).unwrap(),
            "2026-10-17T09:30:05.123Z"
        );
        assert_eq!(
            iso_from_ms(951_782_400_000).unwrap(),
            "2000-02-29T00:00:00.000Z"
        );
        assert_eq!(
            iso_from_ms(253_402_300_799_999).unwrap(),
            "9999-12-31T23:59:59.999Z"
        );
        assert!(iso_from_ms(-1).is_none());
        assert!(iso_from_ms(253_402_300_800_000).is_none());
    }

    #[test]
    fn query_encodes_like_encode_uri_component_in_insertion_order() {
        assert_eq!(
            query(&[
                ("location", Some("Message Context Menu".into())),
                ("burst", Some("false".into())),
                ("type", Some("0".into())),
                ("after", None),
                ("limit", Some("100".into())),
            ]),
            "?location=Message%20Context%20Menu&burst=false&type=0&limit=100"
        );
        assert_eq!(query(&[("a", None)]), "");
        assert_eq!(
            encode_component("2026-10-17T09:30:05.123+00:00"),
            "2026-10-17T09%3A30%3A05.123%2B00%3A00"
        );
    }
}
