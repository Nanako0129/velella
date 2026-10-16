//! Messages of the one open channel or DM (plan M). The buffer lives here, outside `State` (which
//! is copied on write); the session calls `observe` with every dispatch right after
//! `State::apply`, once `SessionHandle::messages()` has created the store.
//!
//! Rules, each with a named test in messages_tests.rs:
//! - Only the open channel is buffered. Opening fetches its latest 50 once; leaving cancels the
//!   fetch. History is merged into live events that arrived meanwhile (a live copy wins), sorted
//!   ascending by snowflake, deduplicated by id and capped at `CAP` (oldest dropped).
//! - Older history (wave 3 P): `load_older` fetches the 50 messages before the oldest buffered
//!   one, one GET at a time, only when the UI asks (it reaches the top). A page of fewer than 50,
//!   or a buffer too full for another page, marks the start reached; a failure keeps the rows and
//!   allows a retry.
//! - MESSAGE_UPDATE merges only the fields it carries (Concord's partial update,
//!   refs/concord/src/discord/gateway/parser/messages.rs:943-1021; see THIRD_PARTY_NOTICES.md).
//! - Sends go only to the open channel; the local echo is reconciled on nonce and self author.
//! - Every failure is fixed text (`&'static str`): no REST error, body or header reaches a row.
//! - The body is parsed once, when content arrives or changes (K1); `row()` never re-parses.
//!   Mentions are resolved by walking that AST into side data, so a resolved name is never parsed.
//! - Code blocks are highlighted off the session task (K6 rev 3.1). A message whose body has a
//!   fence in the language table is marked `pending` where the body is assigned (`parse` of a
//!   top-level message, `merge` with new content); never an echo or a nested reply. One worker
//!   thread per store, holding only a `Weak`, takes the newest pending message (`next_job`),
//!   computes off the lock, and `apply` sets each block's highlight only while the message still has
//!   exactly those blocks. Every `bump` wakes it. On the session path the only highlighting code is
//!   the pure table lookup `highlight::header`.
//! - Reactions (wave 3, X) are parsed with the message and edited in place by the four
//!   MESSAGE_REACTION_* dispatches (see reactions.rs); an event for a message not buffered is
//!   dropped, and `me` follows events whose `user_id` is the self user (`set_me`).
//! - A reply preview is flattened from the replied message's AST: a spoiler and everything in it
//!   become one placeholder, so a preview never shows spoiler text.
//! - Embeds (wave 3 E, embeds.rs) are parsed with the message (descriptions and field values
//!   parsed once there); a present `embeds` in an update replaces them, an absent one keeps them,
//!   and `flags` bit 2 (SUPPRESS_EMBEDS) hides them from rows. An embed whose link the body hides
//!   in a spoiler is marked `spoiler` in the row. Image attachments come only from `media_url`
//!   (the proxy URL first), and IS_SPOILER or a `SPOILER_` filename marks them `spoiler`.
use crate::{
    embeds::{self, Embed},
    highlight::{self, Highlighted, Job},
    markdown::{self, Block, Inline, NavTarget, TimestampStyle},
    media::media_url,
    reactions::{self, Reaction},
    rest::{Rest, RestError},
    session::{SessionHandle, Subscription},
    sidebar::{avatar_url, CDN},
    state::{Role, State, User},
    typing::{self, Typing},
};
use serde_json::Value;
use std::{
    cmp::Reverse,
    collections::HashSet,
    sync::{
        atomic::{AtomicU64, Ordering},
        mpsc::{sync_channel, Receiver, SyncSender},
        Arc, Mutex, Weak,
    },
    time::{Instant, SystemTime, UNIX_EPOCH},
};
use tokio::{sync::watch, task::AbortHandle, task::JoinHandle};

/// Rows kept per open channel.
const CAP: usize = 1000;
/// Messages per history GET (`limit=50`); a shorter page is the start of the channel.
const PAGE: usize = 50;
const DISCORD_EPOCH_MS: u64 = 1_420_070_400_000;
const CAPTCHA_TEXT: &str = "Discord asked for a captcha; send this from the official app";
/// What a spoiler shows as in a reply preview. Provisional: K0 has not captured Discord's yet.
const SPOILER_PLACEHOLDER: &str = "\u{2592}\u{2592}\u{2592}";
/// Reply previews are cut to this many characters.
const PREVIEW_CHARS: usize = 200;
/// Distinct custom-emoji images per row (plan P3-4); further ones render as `:name:`.
const MAX_EMOJI_IMAGES: usize = 64;
/// Message flag SUPPRESS_EMBEDS.
const SUPPRESS_EMBEDS: u64 = 1 << 2;
/// Attachment flag IS_SPOILER.
const IS_SPOILER: u64 = 1 << 3;

/// Load state of the open channel; failures carry fixed text only.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LoadState {
    Loading,
    Loaded,
    Failed(&'static str),
}

/// Older-history state of the open channel; failures carry fixed text only.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum OlderState {
    Idle,
    Loading,
    ReachedStart,
    Failed(&'static str),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MessageImage {
    /// Passed `media_url`.
    pub url: String,
    /// Both > 0 (checked when parsed).
    pub width: u32,
    pub height: u32,
    /// IS_SPOILER or a `SPOILER_` filename: covered until revealed, not fetched before.
    pub spoiler: bool,
}

/// The message a row replies to.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MessageReply {
    pub author_name: String,
    /// One line of at most `PREVIEW_CHARS` characters flattened from the replied message's body.
    pub content: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum MentionKind {
    User,
    Role,
    Channel,
}

/// A mention in a row's body, resolved against the payload's `mentions` (users) or the guild
/// (roles, channels). `name` is `None` for an unknown user or channel or a deleted role; `color`
/// is the role's colour (0 for none, and always for users and channels).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResolvedMention {
    pub kind: MentionKind,
    pub id: u64,
    pub name: Option<String>,
    pub color: u32,
}

/// A custom emoji's image; `url` comes from `emoji_url`, never from the message text.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EmojiImage {
    pub id: u64,
    pub animated: bool,
    pub url: String,
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
    /// The raw content, unresolved; `body` is its parse.
    pub content: String,
    pub body: Vec<Block>,
    /// Every distinct user, role and channel mention in `body`, in document order.
    pub mentions: Vec<ResolvedMention>,
    /// Image URLs of the first `MAX_EMOJI_IMAGES` distinct custom emoji in `body`; an emoji not
    /// listed renders as `:name:`.
    pub emoji: Vec<EmojiImage>,
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
    /// First-seen order, at most 20.
    pub reactions: Vec<Reaction>,
    /// At most 10; none while SUPPRESS_EMBEDS is set.
    pub embeds: Vec<Embed>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MessagesSnapshot {
    pub channel_id: u64,
    pub load: LoadState,
    /// Ascending; unconfirmed and failed echoes last, in send order.
    pub rows: Vec<MessageRow>,
    pub older: OlderState,
    /// Display names of the users typing in this channel now (never the self user), in the order
    /// they started. Resolved as message authors are: member nick, global name, username.
    pub typing: Vec<String>,
}

// ---- Parsing ----

pub(crate) fn id(v: &Value) -> Option<u64> {
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

/// An image needs an image (or absent) content type, a media URL (`proxy_url`, else `url`, through
/// `media_url`) and both dimensions > 0; anything else is listed by filename.
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
    let url = ["proxy_url", "url"]
        .into_iter()
        .find_map(|k| a.get(k)?.as_str().and_then(media_url));
    let spoiler = a
        .get("flags")
        .and_then(Value::as_u64)
        .is_some_and(|f| f & IS_SPOILER != 0)
        || filename.starts_with("SPOILER_");
    Some(match (image_type, url, dim("width"), dim("height")) {
        (true, Some(url), Some(width), Some(height)) => Attachment::Image(MessageImage {
            url,
            width,
            height,
            spoiler,
        }),
        _ => Attachment::File(filename),
    })
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

/// Calls `f(fence, code, highlight)` for every code block of `blocks`, in document order,
/// descending into quotes, list items and blocks inside inline containers. The one walk behind the
/// pending check, `next_job` and `apply`, so their orders agree by construction. Depth is bounded by
/// the parser's nesting limit.
fn each_code_block(
    blocks: &mut [Block],
    f: &mut impl FnMut(Option<&str>, &str, &mut Option<Arc<Highlighted>>),
) {
    for b in blocks {
        match b {
            Block::Paragraph(c) | Block::Subtext(c) | Block::Heading { content: c, .. } => {
                each_code_block_in(c, f)
            }
            Block::Quote(inner) => each_code_block(inner, f),
            Block::List { items, .. } => items.iter_mut().for_each(|i| each_code_block(i, f)),
            Block::CodeBlock {
                language,
                code,
                highlight,
            } => f(language.as_deref(), code, highlight),
        }
    }
}

fn each_code_block_in(
    inlines: &mut [Inline],
    f: &mut impl FnMut(Option<&str>, &str, &mut Option<Arc<Highlighted>>),
) {
    for i in inlines {
        match i {
            Inline::Bold(c)
            | Inline::Italic(c)
            | Inline::Underline(c)
            | Inline::Strike(c)
            | Inline::Spoiler(c)
            | Inline::Link { label: c, .. } => each_code_block_in(c, f),
            Inline::Block(b) => each_code_block(std::slice::from_mut(&mut **b), f),
            _ => {}
        }
    }
}

/// Whether any code block's fence is in the language table (the pure lookup; no syntect).
fn has_mapped_fence(body: &mut [Block]) -> bool {
    let mut mapped = false;
    each_code_block(body, &mut |fence, _, _| {
        mapped |= highlight::header(fence).is_some()
    });
    mapped
}

#[derive(Clone, Debug)]
struct Stored {
    id: u64,
    author: User,
    nick: Option<String>,
    roles: Vec<u64>,
    content: String,
    /// `content` parsed once, when it arrives or changes (never per snapshot).
    body: Vec<markdown::Block>,
    /// A highlight job is due: set with the body when a fence maps, cleared by `next_job`.
    pending: bool,
    edited: bool,
    attachments: Vec<Attachment>,
    mentions: Vec<(u64, String)>,
    reply: Option<Box<Stored>>,
    nonce: Option<String>,
    reactions: Vec<Reaction>,
    /// MESSAGE_POLL_VOTE_ADD_MANY totals per answer: (answer id, votes, includes the self user).
    /// Stored for the polls wave, which renders them; nothing reads them yet.
    #[allow(dead_code)]
    poll_votes: Vec<(u64, u32, bool)>,
    /// Parsed once with the message (descriptions and values included); never in `row()`.
    embeds: Vec<Embed>,
    /// `flags` bit 2: the embeds are kept but not shown.
    suppress_embeds: bool,
}

/// Whether `flags` has SUPPRESS_EMBEDS; `None` when the payload carries no integer `flags`.
fn suppress_embeds(v: &Value) -> Option<bool> {
    v.get("flags")
        .and_then(Value::as_u64)
        .map(|f| f & SUPPRESS_EMBEDS != 0)
}

/// A message object from the gateway or REST. A referenced message is parsed one level deep.
fn parse(v: &Value, nested: bool) -> Option<Stored> {
    let member = v.get("member").filter(|m| m.is_object());
    let content = text(v, "content").unwrap_or_default();
    let mut body = markdown::parse(&content);
    // A nested reply only feeds the one-line preview: never highlighted.
    let pending = !nested && has_mapped_fence(&mut body);
    Some(Stored {
        id: v.get("id").and_then(id)?,
        author: user(v.get("author")?)?,
        nick: member.and_then(|m| text(m, "nick")),
        roles: member
            .and_then(|m| m.get("roles")?.as_array())
            .map(|r| r.iter().filter_map(id).collect())
            .unwrap_or_default(),
        body,
        pending,
        content,
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
        reactions: reactions::parse(v.get("reactions")),
        poll_votes: Vec::new(),
        // A nested reply only feeds the one-line preview: its embeds are never parsed.
        embeds: if nested {
            Vec::new()
        } else {
            embeds::parse(v.get("embeds"))
        },
        suppress_embeds: suppress_embeds(v).unwrap_or(false),
    })
}

/// Most poll answers Discord allows; votes for more distinct answers are not kept.
const MAX_POLL_ANSWERS: usize = 10;

/// MESSAGE_POLL_VOTE_ADD_MANY `votes: [{answer_id, users: [...]}]` (identify.md §1, W:4704644).
fn add_poll_votes(votes: &mut Vec<(u64, u32, bool)>, d: &Value, me: Option<u64>) -> bool {
    let mut changed = false;
    for v in d
        .get("votes")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        let Some(answer) = v.get("answer_id").and_then(Value::as_u64) else {
            continue;
        };
        let users: Vec<Option<u64>> = v
            .get("users")
            .and_then(Value::as_array)
            .map(|u| u.iter().map(id).collect())
            .unwrap_or_default();
        if users.is_empty() {
            continue;
        }
        let at = match votes.iter().position(|x| x.0 == answer) {
            Some(i) => i,
            None if votes.len() < MAX_POLL_ANSWERS => {
                votes.push((answer, 0, false));
                votes.len() - 1
            }
            None => continue,
        };
        let slot = &mut votes[at];
        slot.1 = slot
            .1
            .saturating_add(u32::try_from(users.len()).unwrap_or(u32::MAX));
        slot.2 |= me.is_some() && users.contains(&me);
        changed = true;
    }
    changed
}

impl Stored {
    /// MESSAGE_UPDATE: only the fields the payload carries; absent `attachments` = unchanged.
    fn merge(&mut self, d: &Value) {
        if let Some(content) = text(d, "content") {
            if content != self.content {
                self.body = markdown::parse(&content);
                self.pending = has_mapped_fence(&mut self.body);
            }
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
        if d.get("embeds").is_some() {
            self.embeds = embeds::parse(d.get("embeds"));
        }
        if let Some(suppress) = suppress_embeds(d) {
            self.suppress_embeds = suppress;
        }
    }

    /// The embeds rows show: none while suppressed.
    fn shown_embeds(&self) -> &[Embed] {
        if self.suppress_embeds {
            &[]
        } else {
            &self.embeds
        }
    }

    /// The body, then each shown embed's description and field values: every stored AST whose
    /// mentions and emoji a row resolves.
    fn markdown(&self) -> impl Iterator<Item = &[Block]> {
        std::iter::once(self.body.as_slice()).chain(self.shown_embeds().iter().flat_map(|e| {
            std::iter::once(e.description.as_slice())
                .chain(e.fields.iter().map(|f| f.value.as_slice()))
        }))
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

/// A custom emoji's CDN image, built from the parsed id (never from the message text).
pub(crate) fn emoji_url(id: u64, animated: bool) -> String {
    let ext = if animated { "gif" } else { "webp" };
    format!("{CDN}/emojis/{id}.{ext}?size=48")
}

/// Calls `f` on every inline of `blocks` in document order, descending into containers, link
/// labels and blocks inside inlines. Depth is bounded by the parser's nesting limit.
fn each_inline(blocks: &[Block], f: &mut impl FnMut(&Inline)) {
    for b in blocks {
        match b {
            Block::Paragraph(c) | Block::Subtext(c) | Block::Heading { content: c, .. } => {
                each_in(c, f)
            }
            Block::Quote(inner) => each_inline(inner, f),
            Block::List { items, .. } => items.iter().for_each(|i| each_inline(i, f)),
            Block::CodeBlock { .. } => {}
        }
    }
}

fn each_in(inlines: &[Inline], f: &mut impl FnMut(&Inline)) {
    for i in inlines {
        f(i);
        match i {
            Inline::Bold(c)
            | Inline::Italic(c)
            | Inline::Underline(c)
            | Inline::Strike(c)
            | Inline::Spoiler(c)
            | Inline::Link { label: c, .. } => each_in(c, f),
            Inline::Block(b) => each_inline(std::slice::from_ref(b), f),
            _ => {}
        }
    }
}

fn mention_of(i: &Inline) -> Option<(MentionKind, u64)> {
    match *i {
        Inline::UserMention(id) => Some((MentionKind::User, id)),
        Inline::RoleMention(id) => Some((MentionKind::Role, id)),
        Inline::ChannelMention(id) => Some((MentionKind::Channel, id)),
        _ => None,
    }
}

/// One mention's name, `None` when unknown (or a role without a name).
fn mention_name(
    kind: MentionKind,
    id: u64,
    mentioned: &[(u64, String)],
    ctx: &Context,
) -> Option<String> {
    match kind {
        MentionKind::User => mentioned.iter().find(|(m, _)| *m == id).map(|(_, n)| n),
        MentionKind::Channel => ctx.channels.iter().find(|(c, _)| *c == id).map(|(_, n)| n),
        MentionKind::Role => ctx.roles.iter().find(|r| r.id == id)?.name.as_ref(),
    }
    .cloned()
}

/// Side data for every distinct mention in `m.body`; names are copied, never parsed.
fn resolve_mentions(m: &Stored, ctx: &Context) -> Vec<ResolvedMention> {
    let mut seen = HashSet::new();
    let mut out = Vec::new();
    let mut each = |i: &Inline| {
        let Some((kind, id)) = mention_of(i).filter(|k| seen.insert(*k)) else {
            return;
        };
        let color = match kind {
            MentionKind::Role => ctx.roles.iter().find(|r| r.id == id).map_or(0, |r| r.color),
            _ => 0,
        };
        out.push(ResolvedMention {
            kind,
            id,
            name: mention_name(kind, id, &m.mentions, ctx),
            color,
        });
    };
    m.markdown()
        .for_each(|blocks| each_inline(blocks, &mut each));
    out
}

/// The first `MAX_EMOJI_IMAGES` distinct custom emoji of `m`'s body and then its shown embeds,
/// with their CDN URLs.
fn emoji_images(m: &Stored) -> Vec<EmojiImage> {
    let mut seen = HashSet::new();
    let mut out = Vec::new();
    let mut each = |i: &Inline| {
        if let Inline::CustomEmoji { id, animated, .. } = *i {
            if out.len() < MAX_EMOJI_IMAGES && seen.insert((id, animated)) {
                out.push(EmojiImage {
                    id,
                    animated,
                    url: emoji_url(id, animated),
                });
            }
        }
    };
    m.markdown()
        .for_each(|blocks| each_inline(blocks, &mut each));
    out
}

/// Canonical URLs of the links inside spoilers of `body` (an embed of one is covered).
fn spoilered_links(body: &[Block]) -> HashSet<String> {
    let mut out = HashSet::new();
    each_inline(body, &mut |i| {
        if let Inline::Spoiler(c) = i {
            each_in(c, &mut |j| {
                if let Inline::Link { url, .. } = j {
                    out.insert(url.clone());
                }
            });
        }
    });
    out
}

/// Wraps a substituted name in FSI…PDI, so bidi controls in it cannot reorder the line around it.
/// Inside, a PDI with no isolate of the name's own to close is dropped (it would close the FSI
/// early), and every isolate the name leaves open is closed, so the final PDI always closes the
/// FSI (K2 verification F1). Embeddings and overrides end at that PDI (UAX #9).
fn isolate(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 6);
    out.push('\u{2068}');
    let mut open = 0usize;
    for c in s.chars() {
        match c {
            '\u{2066}'..='\u{2068}' => open += 1,
            '\u{2069}' if open == 0 => continue,
            '\u{2069}' => open -= 1,
            _ => {}
        }
        out.push(c);
    }
    out.extend(std::iter::repeat_n('\u{2069}', open + 1));
    out
}

/// What a guild-navigation token shows as (Discord's labels).
fn nav_label(target: NavTarget) -> &'static str {
    match target {
        NavTarget::Customize => "Channels & Roles",
        NavTarget::Browse => "Browse Channels",
        NavTarget::Guide => "Server Guide",
        NavTarget::LinkedRoles(_) => "Linked Roles",
    }
}

fn style_letter(style: TimestampStyle) -> char {
    match style {
        TimestampStyle::ShortTime => 't',
        TimestampStyle::MediumTime => 'T',
        TimestampStyle::ShortDate => 'd',
        TimestampStyle::LongDate => 'D',
        TimestampStyle::LongDateShortTime => 'f',
        TimestampStyle::FullDateShortTime => 'F',
        TimestampStyle::ShortDateShortTime => 's',
        TimestampStyle::ShortDateMediumTime => 'S',
        TimestampStyle::Relative => 'R',
    }
}

/// Reply preview: one line of at most `PREVIEW_CHARS` characters. A spoiler becomes the
/// placeholder together with everything inside it; code keeps its text; line breaks, blocks and
/// whitespace runs become single spaces; emoji `:name:`; mentions their resolved names, isolated.
fn preview(m: &Stored, ctx: &Context) -> String {
    let mut out = String::new();
    flatten_blocks(&m.body, m, ctx, &mut out);
    let line = out.split_whitespace().collect::<Vec<_>>().join(" ");
    line.chars().take(PREVIEW_CHARS).collect()
}

fn flatten_blocks(blocks: &[Block], m: &Stored, ctx: &Context, out: &mut String) {
    for b in blocks {
        match b {
            Block::Paragraph(c) | Block::Subtext(c) | Block::Heading { content: c, .. } => {
                flatten(c, m, ctx, out)
            }
            Block::Quote(inner) => flatten_blocks(inner, m, ctx, out),
            Block::List { items, .. } => items.iter().for_each(|i| flatten_blocks(i, m, ctx, out)),
            Block::CodeBlock { code, .. } => out.push_str(code),
        }
        out.push(' ');
    }
}

fn flatten(inlines: &[Inline], m: &Stored, ctx: &Context, out: &mut String) {
    for i in inlines {
        if let Some((kind, id)) = mention_of(i) {
            let (sigil, unknown) = match kind {
                MentionKind::User => ('@', "unknown-user"),
                MentionKind::Role => ('@', "deleted-role"),
                MentionKind::Channel => ('#', "unknown"),
            };
            let name = mention_name(kind, id, &m.mentions, ctx);
            out.push_str(&isolate(&format!(
                "{sigil}{}",
                name.as_deref().unwrap_or(unknown)
            )));
            continue;
        }
        match i {
            Inline::Text(t) | Inline::Code(t) => out.push_str(t),
            Inline::Bold(c)
            | Inline::Italic(c)
            | Inline::Underline(c)
            | Inline::Strike(c)
            | Inline::Link { label: c, .. } => flatten(c, m, ctx, out),
            Inline::Spoiler(_) => out.push_str(SPOILER_PLACEHOLDER),
            Inline::Everyone => out.push_str("@everyone"),
            Inline::Here => out.push_str("@here"),
            Inline::CustomEmoji { name, .. } => out.push_str(&format!(":{name}:")),
            // ponytail: the token itself; a localised date needs K4's formatter, which core lacks.
            Inline::Timestamp { unix, style } => {
                out.push_str(&format!("<t:{unix}:{}>", style_letter(*style)))
            }
            Inline::SlashCommand { name, .. } => out.push_str(&isolate(&format!("/{name}"))),
            Inline::GuildNavigation { target, .. } => out.push_str(nav_label(*target)),
            Inline::LineBreak => out.push(' '),
            Inline::Block(b) => {
                out.push(' ');
                flatten_blocks(std::slice::from_ref(b), m, ctx, out);
            }
            Inline::UserMention(_) | Inline::RoleMention(_) | Inline::ChannelMention(_) => {}
        }
    }
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
        content: m.content.clone(),
        body: m.body.clone(),
        mentions: resolve_mentions(m, ctx),
        emoji: emoji_images(m),
        timestamp_ms: (m.id >> 22) + DISCORD_EPOCH_MS,
        edited: m.edited,
        images,
        files,
        reply: m.reply.as_ref().map(|r| MessageReply {
            author_name: r.name(),
            content: preview(r, ctx),
        }),
        pending,
        failure,
        reactions: m.reactions.clone(),
        embeds: {
            let hidden = spoilered_links(&m.body);
            m.shown_embeds()
                .iter()
                .map(|e| Embed {
                    spoiler: e.url.as_ref().is_some_and(|u| hidden.contains(&u.url)),
                    ..e.clone()
                })
                .collect()
        },
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

fn older_failure(e: RestError) -> &'static str {
    match e {
        RestError::Unauthorized => {
            "Couldn't load older messages: Discord did not accept this sign-in"
        }
        RestError::Forbidden => "Couldn't load older messages: no access to this channel",
        RestError::RateLimited => "Couldn't load older messages: rate limited",
        _ => "Couldn't load older messages",
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
    older: OlderState,
    /// The one in-flight older-page GET, while `older` is `Loading`.
    older_fetch: Option<AbortHandle>,
    typing: Typing,
}

impl Open {
    /// Cancels every in-flight GET of this open channel (it is being left or replaced).
    fn cancel(self) {
        self.fetch
            .into_iter()
            .chain(self.older_fetch)
            .for_each(|f| f.abort());
    }

    /// Inserts in id order; `replace` decides whether an existing copy is overwritten.
    /// A replacing copy with equal `content` keeps the existing body and `pending`: the body is a
    /// function of `content` alone, so colours already applied stay.
    fn insert(&mut self, mut m: Stored, replace: bool) {
        match self.messages.binary_search_by_key(&m.id, |s| s.id) {
            Ok(i) if replace => {
                let old = &mut self.messages[i];
                if old.content == m.content {
                    m.body = std::mem::take(&mut old.body);
                    m.pending = old.pending;
                }
                // A MESSAGE_CREATE never carries reactions; the buffered copy has the events.
                m.reactions = std::mem::take(&mut old.reactions);
                m.poll_votes = std::mem::take(&mut old.poll_votes);
                *old = m;
            }
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
    /// Wakes this store's highlight worker; `None` without one (a failed spawn, or tests).
    wake: Option<SyncSender<()>>,
    /// The time typing entries are judged against (a field so tests own the clock).
    clock: Box<dyn Fn() -> Instant + Send + Sync>,
    /// The self user id (0 until known), from the session state; decides reactions' `me`.
    me: AtomicU64,
    /// Dispatch names seen, so a test can show the observer is wired.
    #[cfg(test)]
    pub(crate) seen: std::sync::Mutex<Vec<String>>,
}

impl MessageStore {
    /// A store with its highlight worker thread.
    pub(crate) fn new(rest: Rest) -> Arc<MessageStore> {
        Self::start(rest, highlight::compute_job).0
    }

    /// `new` with the compute function as a parameter, also returning the worker's handle (`None`
    /// when the spawn failed: highlighting is then off for this store, and nothing panics).
    fn start(
        rest: Rest,
        compute: impl FnMut(&Job) -> Vec<Option<Arc<Highlighted>>> + Send + 'static,
    ) -> (Arc<MessageStore>, Option<std::thread::JoinHandle<()>>) {
        let (tx, rx) = sync_channel(1);
        let mut handle = None;
        let store = Arc::new_cyclic(|weak| {
            let weak = weak.clone();
            handle = std::thread::Builder::new()
                .name("velella-highlight".into())
                .spawn(move || run(weak, rx, compute))
                .ok();
            Self::build(rest, handle.is_some().then_some(tx))
        });
        (store, handle)
    }

    /// A store without a thread; `wake` is what `bump` signals (the test seam).
    #[cfg(test)]
    pub(crate) fn with_wake(rest: Rest, wake: Option<SyncSender<()>>) -> Arc<MessageStore> {
        Arc::new(Self::build(rest, wake))
    }

    fn build(rest: Rest, wake: Option<SyncSender<()>>) -> MessageStore {
        MessageStore {
            rest,
            inner: Mutex::default(),
            revision: watch::channel(0).0,
            wake,
            clock: Box::new(Instant::now),
            me: AtomicU64::new(0),
            #[cfg(test)]
            seen: Default::default(),
        }
    }

    /// The self user's id, as the session state has it (`None` before READY).
    pub(crate) fn set_me(&self, user_id: Option<u64>) {
        self.me.store(user_id.unwrap_or(0), Ordering::Relaxed);
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Inner> {
        self.inner.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn bump(&self) {
        self.revision.send_modify(|r| *r += 1);
        self.wake();
    }

    /// Never blocks: a full channel means a wake is already pending, a disconnected one that the
    /// worker is gone (it is not restarted).
    fn wake(&self) {
        if let Some(wake) = &self.wake {
            let _ = wake.try_send(());
        }
    }

    /// The newest pending message of the open channel, its flag cleared, as a job: every code
    /// block's (fence, code) in walk order. Echoes are never looked at.
    fn next_job(&self) -> Option<Job> {
        let mut inner = self.lock();
        let m = inner
            .open
            .as_mut()?
            .messages
            .iter_mut()
            .rev()
            .find(|m| m.pending)?;
        m.pending = false;
        let mut blocks = Vec::new();
        each_code_block(&mut m.body, &mut |fence, code, _| {
            blocks.push((fence.map(str::to_owned), code.to_owned()))
        });
        Some(Job { id: m.id, blocks })
    }

    /// Applies a job's results if message `job.id` is still in the open channel with exactly the
    /// job's code blocks, (fence, code) byte for byte; then each block takes only a result whose
    /// spans spell its own code. No syntect here. Bumps once if anything changed.
    fn apply(&self, job: &Job, results: Vec<Option<Arc<Highlighted>>>) {
        let mut inner = self.lock();
        let Some(m) = inner
            .open
            .as_mut()
            .and_then(|o| o.messages.iter_mut().find(|m| m.id == job.id))
        else {
            return;
        };
        let mut seen = 0;
        let mut same = results.len() == job.blocks.len();
        each_code_block(&mut m.body, &mut |fence, code, _| {
            same &= job
                .blocks
                .get(seen)
                .is_some_and(|(f, c)| f.as_deref() == fence && c == code);
            seen += 1;
        });
        if !same || seen != job.blocks.len() {
            return;
        }
        let mut results = results.into_iter();
        let mut changed = false;
        each_code_block(&mut m.body, &mut |_, code, highlight| {
            if let Some(h) = results.next().flatten().filter(|h| h.text() == code) {
                *highlight = Some(h);
                changed = true;
            }
        });
        drop(inner);
        if changed {
            self.bump();
        }
    }

    /// Makes `channel_id` the open channel (dropping the previous buffer and cancelling its fetch)
    /// and starts the one history GET. Returns the open's epoch. Needs a runtime context.
    pub(crate) fn open(self: &Arc<Self>, channel_id: u64) -> u64 {
        let mut inner = self.lock();
        if let Some(open) = inner.open.take() {
            open.cancel();
        }
        inner.epoch += 1;
        let epoch = inner.epoch;
        let store = self.clone();
        // The task cannot finish before `inner` is released: `history` takes the same lock.
        let fetch = tokio::spawn(async move {
            let result = store.rest.channel_messages(channel_id, None).await;
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
            older: OlderState::Idle,
            older_fetch: None,
            typing: Typing::default(),
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
            if let Some(open) = inner.open.take() {
                open.cancel();
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
                if list.len() < PAGE {
                    open.older = OlderState::ReachedStart;
                }
            }
            Err(e) => open.load = LoadState::Failed(load_failure(e)),
        }
        drop(inner);
        self.bump();
    }

    /// Asks for the page before the oldest buffered message of the channel opened at `epoch`.
    /// Does nothing unless the first page is in and no older GET is running or the start reached
    /// (a failed one may be retried). A buffer without room for another page counts as the start:
    /// the page is not requested. Needs a runtime context.
    pub(crate) fn load_older(self: &Arc<Self>, epoch: u64) {
        let mut inner = self.lock();
        let Some(open) = inner.open.as_mut().filter(|o| o.epoch == epoch) else {
            return;
        };
        if open.load != LoadState::Loaded
            || !matches!(open.older, OlderState::Idle | OlderState::Failed(_))
        {
            return;
        }
        let before = open.messages.first().map(|m| m.id);
        match before.filter(|_| open.messages.len() + PAGE <= CAP) {
            None => open.older = OlderState::ReachedStart,
            Some(before) => {
                open.older = OlderState::Loading;
                let (store, channel_id) = (self.clone(), open.channel_id);
                // The task cannot finish before `inner` is released: `older_page` takes the lock.
                let fetch = tokio::spawn(async move {
                    let result = store.rest.channel_messages(channel_id, Some(before)).await;
                    store.older_page(epoch, result);
                });
                open.older_fetch = Some(fetch.abort_handle());
            }
        }
        drop(inner);
        self.bump();
    }

    /// An older page, merged like the first one; dropped when its channel was left or replaced.
    fn older_page(&self, epoch: u64, result: Result<Vec<Value>, RestError>) {
        let mut inner = self.lock();
        let Some(open) = inner.open.as_mut().filter(|o| o.epoch == epoch) else {
            return;
        };
        open.older_fetch = None;
        open.older = match result {
            Ok(list) => {
                for m in list.iter().filter_map(|v| parse(v, false)) {
                    if !open.deleted.contains(&m.id) {
                        open.insert(m, false);
                    }
                }
                open.deleted = Vec::new();
                if list.len() < PAGE {
                    OlderState::ReachedStart
                } else {
                    OlderState::Idle
                }
            }
            Err(e) => OlderState::Failed(older_failure(e)),
        };
        drop(inner);
        self.bump();
    }

    /// The dispatches `observe` acts on.
    pub(crate) fn observes(name: &str) -> bool {
        matches!(
            name,
            "MESSAGE_CREATE"
                | "MESSAGE_UPDATE"
                | "MESSAGE_DELETE"
                | "MESSAGE_REACTION_ADD"
                | "MESSAGE_REACTION_REMOVE"
                | "MESSAGE_REACTION_REMOVE_ALL"
                | "MESSAGE_REACTION_REMOVE_EMOJI"
                | "MESSAGE_REACTION_ADD_MANY"
                | "MESSAGE_POLL_VOTE_ADD_MANY"
        )
    }

    /// Runs on the session task for every dispatch; must not block.
    pub(crate) fn observe(&self, name: &str, d: &Value) {
        #[cfg(test)]
        self.seen.lock().unwrap().push(name.to_owned());
        if !Self::observes(name) {
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
                open.typing.remove(m.author.id);
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
            "MESSAGE_DELETE" => {
                let Some(target) = d.get("id").and_then(id) else {
                    return;
                };
                open.messages.retain(|m| m.id != target);
                if open.load == LoadState::Loading || open.older == OlderState::Loading {
                    open.deleted.push(target);
                }
            }
            _ => {
                let Some(m) = d
                    .get("message_id")
                    .and_then(id)
                    .and_then(|target| open.messages.iter_mut().find(|m| m.id == target))
                else {
                    return;
                };
                let me = Some(self.me.load(Ordering::Relaxed)).filter(|n| *n != 0);
                let changed = if name == "MESSAGE_POLL_VOTE_ADD_MANY" {
                    add_poll_votes(&mut m.poll_votes, d, me)
                } else {
                    reactions::apply(&mut m.reactions, name, d, me)
                };
                if !changed {
                    return;
                }
            }
        }
        drop(inner);
        self.bump();
    }

    /// Adds a local echo to `channel_id` if it is the open channel; returns (epoch, nonce). A
    /// reply's echo carries the preview of `reply_to` when that message is buffered.
    fn begin_send(
        &self,
        channel_id: u64,
        content: &str,
        me: User,
        reply_to: Option<u64>,
    ) -> Option<(u64, String)> {
        let mut inner = self.lock();
        let open = inner.open.as_mut().filter(|o| o.channel_id == channel_id)?;
        let reply = reply_to
            .and_then(|r| open.messages.iter().find(|m| m.id == r))
            .map(|m| {
                Box::new(Stored {
                    reply: None,
                    pending: false,
                    ..m.clone()
                })
            });
        let nonce = next_nonce();
        open.echoes.push(Echo {
            nonce: nonce.to_string(),
            message: Stored {
                id: nonce,
                author: me,
                nick: None,
                roles: Vec::new(),
                content: content.to_owned(),
                body: markdown::parse(content),
                pending: false, // an echo is never highlighted
                edited: false,
                attachments: Vec::new(),
                mentions: Vec::new(),
                reply,
                nonce: None,
                reactions: Vec::new(),
                poll_votes: Vec::new(),
                embeds: Vec::new(),
                suppress_embeds: false,
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
            older: open.older.clone(),
            typing: open.typing.names((self.clock)()),
        })
    }

    /// The reply preview (`preview`'s rules: spoilers as the placeholder, text only) of buffered
    /// message `id` of the channel opened at `epoch`: one flatten under the store lock. `None` once
    /// that buffer was left or replaced, or when `id` is not buffered (echoes are not looked up:
    /// the menu offers Reply on sent rows only).
    fn preview(&self, epoch: u64, id: u64, ctx: &Context) -> Option<String> {
        let inner = self.lock();
        let open = inner.open.as_ref().filter(|o| o.epoch == epoch)?;
        let i = open.messages.binary_search_by_key(&id, |s| s.id).ok()?;
        Some(preview(&open.messages[i], ctx))
    }

    /// TYPING_START (`d`), called by the session after `State::apply` with the state it left.
    /// Only the open channel counts and the self user never does; a user the payload and the state
    /// both fail to name is skipped. The entry's expiry bumps the store (`expire_typing`). Needs a
    /// runtime context.
    pub(crate) fn typing_start(self: &Arc<Self>, d: &Value, state: &State) {
        let (Some(channel_id), Some(user_id)) = (
            d.get("channel_id").and_then(id),
            d.get("user_id").and_then(id),
        ) else {
            return;
        };
        if state.user_id == Some(user_id) {
            return;
        }
        let member = d.get("member").filter(|m| m.is_object());
        let Some(user) = member
            .and_then(|m| user(m.get("user")?))
            .or_else(|| state.users.get(&user_id).cloned())
        else {
            return;
        };
        let name = display_name(&user, member.and_then(|m| text(m, "nick")).as_deref());
        let mut inner = self.lock();
        let Some(open) = inner.open.as_mut().filter(|o| o.channel_id == channel_id) else {
            return;
        };
        open.typing.start(user_id, name, (self.clock)());
        let epoch = open.epoch;
        drop(inner);
        self.bump();
        let store = self.clone();
        tokio::spawn(async move {
            tokio::time::sleep(typing::TTL).await;
            store.expire_typing(epoch);
        });
    }

    /// Drops lapsed typing entries of the channel opened at `epoch`; bumps if any went.
    fn expire_typing(&self, epoch: u64) {
        let mut inner = self.lock();
        let Some(open) = inner.open.as_mut().filter(|o| o.epoch == epoch) else {
            return;
        };
        let changed = open.typing.prune((self.clock)());
        drop(inner);
        if changed {
            self.bump();
        }
    }
}

/// The highlight worker of one store (plan K6 rev 3.1, Worker row). It holds only a `Weak` and the
/// wake receiver, never an `Arc` across a computation, so it ends once the store is dropped.
fn run(
    store: Weak<MessageStore>,
    wake: Receiver<()>,
    mut compute: impl FnMut(&Job) -> Vec<Option<Arc<Highlighted>>>,
) {
    if highlight::warm().is_none() {
        return;
    }
    while wake.recv().is_ok() {
        loop {
            let Some(strong) = store.upgrade() else {
                return;
            };
            let Some(job) = strong.next_job() else {
                break;
            };
            drop(strong);
            let results = compute(&job);
            let Some(strong) = store.upgrade() else {
                return;
            };
            strong.apply(&job, results);
        }
    }
}

/// A running `SessionHandle::subscribe_messages`. Dropping it leaves the channel: deliveries stop
/// and an in-flight history GET is cancelled.
pub struct MessageSubscription {
    store: Arc<MessageStore>,
    epoch: u64,
    task: JoinHandle<()>,
    /// The role and channel names the snapshots resolve mentions with.
    names: watch::Receiver<Context>,
    _context: Subscription,
}

impl MessageSubscription {
    /// Asks for the page of messages before the oldest buffered one (see
    /// `MessageStore::load_older`); the result arrives as a snapshot. Needs a runtime context.
    pub fn load_older(&self) {
        self.store.load_older(self.epoch);
    }

    /// The one-line reply preview of buffered message `message_id` of this subscription's
    /// channel, by the rules of a row's reply preview (a spoiler and everything in it become the
    /// placeholder; emoji `:name:`; text only). `None` when the message is not buffered or this
    /// subscription's channel is no longer the open one.
    pub fn preview(&self, message_id: u64) -> Option<String> {
        let ctx = self.names.borrow().clone();
        self.store.preview(self.epoch, message_id, &ctx)
    }
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
        let preview_names = names.clone();
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
            names: preview_names,
            _context: state,
        }
    }

    /// Sends `content` to `channel_id` when that is the open channel, with a local echo and a
    /// fresh nonce; never retried. Returns `None` (nothing sent) for an empty or whitespace-only
    /// text, before READY, or when `channel_id` is not the open channel. The outcome shows in the
    /// snapshot: the echo is replaced by the real message, or marked failed with fixed text. With
    /// `reply_to` the message is a reply to that message of the same channel (the echo carries its
    /// preview when buffered). Must be called inside a runtime context.
    pub fn send_message(
        &self,
        channel_id: u64,
        content: String,
        reply_to: Option<u64>,
    ) -> Option<JoinHandle<()>> {
        if content.trim().is_empty() {
            return None;
        }
        let state = self.snapshot();
        let me = state.users.get(&state.user_id?)?.clone();
        let store = self.messages();
        let (epoch, nonce) = store.begin_send(channel_id, &content, me, reply_to)?;
        Some(tokio::spawn(async move {
            let result = store
                .rest
                .create_message(channel_id, &content, &nonce, reply_to)
                .await;
            store.finish_send(epoch, &nonce, result);
        }))
    }
}

#[cfg(test)]
#[path = "messages_tests.rs"]
mod tests;
