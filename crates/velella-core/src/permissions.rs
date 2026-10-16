//! Permission bits and the signed-in user's permissions in a channel. Algorithm pattern adapted
//! from Serein's `effective` (refs/serein/crates/model/src/permissions.rs, MIT OR Apache-2.0):
//! owner/ADMINISTRATOR get everything; otherwise @everyone | member roles, then overwrites
//! @everyone -> roles -> member. Shape of the thread and timeout rules: the official store's own
//! computation (menu-requests.md §16, §17.5; evidence in `channel_permissions_at`). `None` or
//! `Unknown` means the data needed to decide is missing, never "denied".
use crate::state::{Channel, Guild, Overwrite, State};
use std::time::{SystemTime, UNIX_EPOCH};

/// Defines one `u64` mask per permission and the table of (name, bit number). Bits and names are
/// menu-requests.md §16 (module 818348, `1 << n` each); bit 47 is absent there.
macro_rules! permission_bits {
    ($($name:ident = $bit:literal,)*) => {
        $(pub const $name: u64 = 1 << $bit;)*
        /// Every §16 permission as (name, bit number).
        pub const PERMISSION_TABLE: &[(&str, u8)] = &[$((stringify!($name), $bit)),*];
        /// All of the table's bits: what the owner and ADMINISTRATOR hold.
        pub const ALL_PERMISSIONS: u64 = 0 $(| $name)*;
    };
}

permission_bits! {
    CREATE_INSTANT_INVITE = 0,
    KICK_MEMBERS = 1,
    BAN_MEMBERS = 2,
    ADMINISTRATOR = 3,
    MANAGE_CHANNELS = 4,
    MANAGE_GUILD = 5,
    ADD_REACTIONS = 6,
    VIEW_AUDIT_LOG = 7,
    PRIORITY_SPEAKER = 8,
    STREAM = 9,
    VIEW_CHANNEL = 10,
    SEND_MESSAGES = 11,
    SEND_TTS_MESSAGES = 12,
    MANAGE_MESSAGES = 13,
    EMBED_LINKS = 14,
    ATTACH_FILES = 15,
    READ_MESSAGE_HISTORY = 16,
    MENTION_EVERYONE = 17,
    USE_EXTERNAL_EMOJIS = 18,
    VIEW_GUILD_ANALYTICS = 19,
    CONNECT = 20,
    SPEAK = 21,
    MUTE_MEMBERS = 22,
    DEAFEN_MEMBERS = 23,
    MOVE_MEMBERS = 24,
    USE_VAD = 25,
    CHANGE_NICKNAME = 26,
    MANAGE_NICKNAMES = 27,
    MANAGE_ROLES = 28,
    MANAGE_WEBHOOKS = 29,
    MANAGE_GUILD_EXPRESSIONS = 30,
    USE_APPLICATION_COMMANDS = 31,
    REQUEST_TO_SPEAK = 32,
    MANAGE_EVENTS = 33,
    MANAGE_THREADS = 34,
    CREATE_PUBLIC_THREADS = 35,
    CREATE_PRIVATE_THREADS = 36,
    USE_EXTERNAL_STICKERS = 37,
    SEND_MESSAGES_IN_THREADS = 38,
    USE_EMBEDDED_ACTIVITIES = 39,
    MODERATE_MEMBERS = 40,
    VIEW_CREATOR_MONETIZATION_ANALYTICS = 41,
    USE_SOUNDBOARD = 42,
    CREATE_GUILD_EXPRESSIONS = 43,
    CREATE_EVENTS = 44,
    USE_EXTERNAL_SOUNDS = 45,
    SEND_VOICE_MESSAGES = 46,
    SET_VOICE_CHANNEL_STATUS = 48,
    SEND_POLLS = 49,
    USE_EXTERNAL_APPS = 50,
    PIN_MESSAGES = 51,
    BYPASS_SLOWMODE = 52,
    MANAGE_OFFICIAL_MESSAGES = 53,
}

/// What a timed-out member keeps (`v`, W:9605221; applied at W:9606150 when `communicationDisabledUntil`
/// is later than now, W:4994848, and the member is not an administrator).
const TIMEOUT_KEEPS: u64 = VIEW_CHANNEL | READ_MESSAGE_HISTORY;

/// The signed-in user's permissions in one channel.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChannelPermissions {
    Bits(u64),
    /// A DM or group DM: the official store answers every `can(...)` with false there (§17.5);
    /// features that work in DMs test `isPrivate()` themselves.
    Private,
    /// The channel, the guild, the member, a role or a timeout is missing or unreadable.
    Unknown,
}

impl ChannelPermissions {
    /// Whether every bit of `mask` is held; `None` when unknown.
    pub fn can(self, mask: u64) -> Option<bool> {
        match self {
            Self::Bits(b) => Some(b & mask == mask),
            Self::Private => Some(false),
            Self::Unknown => None,
        }
    }
}

/// The guild-wide part: owner, roles, ADMINISTRATOR, overwrites, then the timeout mask.
/// `timed_out` is read only when it can matter (not owner, not administrator).
fn guild_bits(
    user_id: u64,
    guild: &Guild,
    overwrites: Option<&[Overwrite]>,
    timed_out: Option<bool>,
) -> Option<u64> {
    if guild.owner_id == Some(user_id) {
        return Some(ALL_PERMISSIONS);
    }
    if guild.unavailable {
        return None;
    }
    let roles = guild.roles.as_ref()?;
    let member_roles = guild.member_roles.as_ref()?;
    let perms = |id: u64| roles.iter().find(|r| r.id == id).map(|r| r.permissions);
    // The @everyone role has the guild's id.
    let mut bits = perms(guild.id)?;
    for id in member_roles.iter().filter(|id| **id != guild.id) {
        bits |= perms(*id)?;
    }
    if bits & ADMINISTRATOR != 0 {
        return Some(ALL_PERMISSIONS);
    }
    // Without the owner identity a non-administrator may still be the owner.
    guild.owner_id?;
    let (mut role_allow, mut role_deny) = (0, 0);
    let (mut everyone, mut own) = (None, None);
    for o in overwrites? {
        match o.kind {
            0 if o.id == guild.id => everyone = Some(o),
            0 if member_roles.contains(&o.id) => {
                role_allow |= o.allow;
                role_deny |= o.deny;
            }
            1 if o.id == user_id => own = Some(o),
            _ => {}
        }
    }
    if let Some(o) = everyone {
        bits = (bits & !o.deny) | o.allow;
    }
    bits = (bits & !role_deny) | role_allow;
    if let Some(o) = own {
        bits = (bits & !o.deny) | o.allow;
    }
    if bits & ADMINISTRATOR == 0 && timed_out? {
        bits &= TIMEOUT_KEEPS;
    }
    Some(bits)
}

pub(crate) fn can_view_channel(user_id: u64, guild: &Guild, channel: &Channel) -> Option<bool> {
    // Obfuscated (I-5): no access, whoever asks and whatever the channel carries.
    if channel.is_obfuscated() {
        return Some(false);
    }
    // A timeout keeps VIEW_CHANNEL, so it never changes this answer: not consulted.
    guild_bits(
        user_id,
        guild,
        channel.permission_overwrites.as_deref(),
        Some(false),
    )
    .map(|bits| bits & VIEW_CHANNEL != 0)
}

fn guild_channel_bits(
    state: &State,
    user: u64,
    guild: &Guild,
    channel: &Channel,
    now_ms: i64,
) -> Option<u64> {
    if channel.is_obfuscated() {
        return Some(0);
    }
    guild_bits(
        user,
        guild,
        channel.permission_overwrites.as_deref(),
        state.self_state.timed_out(guild.id, now_ms),
    )
}

fn find_channel(guild: &Guild, id: u64) -> Option<&Channel> {
    guild.channels.iter().find(|c| c.id == id)
}

/// The user's permissions in `channel_id` (a guild channel, a thread or a DM) from current
/// state, evaluated now; see `channel_permissions_at`.
pub fn channel_permissions(state: &State, channel_id: u64) -> ChannelPermissions {
    let now_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| i64::try_from(d.as_millis()).unwrap_or(i64::MAX));
    channel_permissions_at(state, channel_id, now_ms)
}

/// `channel_permissions` at `now_ms` (Unix milliseconds), a pure function of `state`.
///
/// Follows the official store (W:9036875 `H`, W:9606751 `G`, W:9606189 `w`, W:9605796 `U`,
/// W:9607730 `x`): the owner and ADMINISTRATOR hold every bit; the timeout mask applies after the
/// overwrites to everyone else; a thread takes its parent's permissions with SEND_MESSAGES
/// replaced by SEND_MESSAGES_IN_THREADS (and removed in a locked thread without MANAGE_THREADS).
/// The store applies no further implicit rule: a missing VIEW_CHANNEL does not clear the other
/// bits, and a missing SEND_MESSAGES does not clear MENTION_EVERYONE, SEND_TTS_MESSAGES,
/// ATTACH_FILES or EMBED_LINKS (its callers combine them, e.g. `can(ATTACH_FILES) &&
/// can(SEND_MESSAGES)`). Not modelled, for lack of the data in state: the private and media
/// thread rules, the guild's elevated-MFA stripping, and the pending and quarantined member masks.
pub fn channel_permissions_at(state: &State, channel_id: u64, now_ms: i64) -> ChannelPermissions {
    use ChannelPermissions::{Bits, Private, Unknown};
    if state.private_channels.iter().any(|c| c.id == channel_id) {
        return Private;
    }
    let Some(user) = state.user_id else {
        return Unknown;
    };
    if let Some((guild, channel)) = state
        .guilds
        .iter()
        .find_map(|g| find_channel(g, channel_id).map(|c| (g, c)))
    {
        return guild_channel_bits(state, user, guild, channel, now_ms).map_or(Unknown, Bits);
    }
    let Some(thread) = state.threads.get(channel_id) else {
        return Unknown;
    };
    let Some((guild, parent)) = state
        .guild(thread.guild_id)
        .and_then(|g| find_channel(g, thread.parent_id).map(|c| (g, c)))
    else {
        return Unknown;
    };
    let Some(parent_bits) = guild_channel_bits(state, user, guild, parent, now_ms) else {
        return Unknown;
    };
    let sends = parent_bits & SEND_MESSAGES_IN_THREADS != 0;
    // `locked` matters only to a member who cannot manage threads.
    let locked = if sends && parent_bits & MANAGE_THREADS == 0 {
        thread.locked
    } else {
        Some(false)
    };
    match locked {
        None => Unknown,
        Some(false) if sends => Bits(parent_bits | SEND_MESSAGES),
        Some(_) => Bits(parent_bits & !SEND_MESSAGES),
    }
}
