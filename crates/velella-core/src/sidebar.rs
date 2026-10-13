//! Sidebar views of the state: pure functions from a `State` snapshot to what the UI lists.
//! Ordering and filter rules: docs/plans/m1.md, S4 row (patterns from Serein's guild_folders.rs and
//! categories.rs and Concord's channel_tree.rs / presentation.rs, see THIRD_PARTY_NOTICES.md).
//! Badges, mutes, locks, occupants, threads and DM presence: docs/plans/m2.md, slice D (rules from
//! Concord's notification/state.rs, GPL-3.0-only).
use crate::guild_settings::now_ms;
use crate::permissions::VIEW_CHANNEL;
use crate::presence::PresenceStatus;
use crate::state::{Channel, Guild, PrivateChannel, State};
use std::{
    cmp::Reverse,
    collections::HashSet,
    sync::{Mutex, OnceLock},
};

const CDN: &str = "https://cdn.discordapp.com";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SidebarGuild {
    pub id: u64,
    pub name: String,
    pub icon_url: Option<String>,
    /// Some visible, unmuted channel is unread. Always false for a muted guild.
    pub unread: bool,
    /// Sum of the mentions of the visible, unmuted channels. Zero for a muted guild.
    pub mention_count: u32,
    pub muted: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SidebarEntry {
    Guild(SidebarGuild),
    Folder {
        id: Option<u64>,
        name: Option<String>,
        color: Option<u32>,
        guilds: Vec<SidebarGuild>,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SidebarChannel {
    pub id: u64,
    /// Discord channel type: 0, 2, 5, 13, 15 or 16.
    pub kind: u8,
    pub name: String,
    /// Unread and mention count are both zero while the channel is muted.
    pub unread: bool,
    pub mention_count: u32,
    pub muted: bool,
    /// @everyone is denied VIEW_CHANNEL here.
    pub private: bool,
    /// Voice and stage channels: who is in them.
    pub occupants: Vec<SidebarOccupant>,
    /// Joined, active threads of this channel.
    pub threads: Vec<SidebarThread>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SidebarOccupant {
    pub id: u64,
    pub name: String,
    pub icon_url: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SidebarThread {
    pub id: u64,
    pub name: String,
    pub unread: bool,
    pub mention_count: u32,
    pub muted: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SidebarCategory {
    /// `None` is the root block above every category.
    pub id: Option<u64>,
    pub name: Option<String>,
    pub channels: Vec<SidebarChannel>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SidebarDm {
    pub id: u64,
    pub name: String,
    pub icon_url: Option<String>,
    /// Zero/false while the DM is muted, so a sum over the list leaves muted DMs out.
    pub unread: bool,
    pub mention_count: u32,
    /// 1:1 DMs only; group DMs carry none.
    pub status: Option<PresenceStatus>,
}

/// What a channel, thread or DM shows: no unread or mentions while muted.
struct Badge {
    unread: bool,
    mentions: u32,
    muted: bool,
}

impl Badge {
    fn new(state: &State, id: u64, last: Option<u64>, muted: bool) -> Self {
        let (unread, mentions) = if muted {
            (false, 0)
        } else {
            state.read.status(id, last)
        };
        Badge {
            unread,
            mentions,
            muted,
        }
    }
}

/// Badge of a guild channel or thread: muted by the first override up its ancestry.
fn guild_badge(state: &State, guild: u64, id: u64, last: Option<u64>, now: i64) -> Badge {
    let muted = state
        .settings
        .channel_muted(guild, &state.mute_ancestry(guild, id), now);
    Badge::new(state, id, last, muted)
}

/// Totals of a guild over its visible channels and the threads shown under them. A muted or
/// unavailable guild shows none.
fn guild_totals(state: &State, g: &Guild, now: i64) -> (bool, u32, bool) {
    let muted = state.settings.guild_muted(g.id, now);
    if muted || g.unavailable {
        return (false, 0, muted);
    }
    let (mut unread, mut mentions) = (false, 0u32);
    for c in listed_channels(state, g) {
        let threads = state.threads.shown_under(c.id).into_iter();
        let parts = std::iter::once((c.id, c.last_message_id))
            .chain(threads.map(|t| (t.id, t.last_message_id)));
        for (id, last) in parts {
            let b = guild_badge(state, g.id, id, last, now);
            unread |= b.unread;
            mentions = mentions.saturating_add(b.mentions);
        }
    }
    (unread, mentions, false)
}

fn guild_icon_url(guild_id: u64, hash: &str) -> String {
    format!("{CDN}/icons/{guild_id}/{hash}.png?size=64")
}

fn sidebar_guild(state: &State, g: &Guild, now: i64) -> SidebarGuild {
    let (unread, mention_count, muted) = guild_totals(state, g, now);
    SidebarGuild {
        id: g.id,
        name: g
            .name
            .clone()
            .unwrap_or_else(|| "Unknown server".to_owned()),
        icon_url: g.icon.as_deref().map(|h| guild_icon_url(g.id, h)),
        unread,
        mention_count,
        muted,
    }
}

/// Rail entries: guilds in no folder first (post-READY ones most recent first, then READY order),
/// then the folders in order. A folder with a null id, or with one known guild, becomes plain
/// guilds; a guild shows once (its first place wins); unknown ids and empty folders vanish.
/// Without `guild_folders` every guild is ungrouped.
pub fn guild_entries(state: &State) -> Vec<SidebarEntry> {
    let now = now_ms();
    let folders = state.guild_folders.as_deref().unwrap_or_default();
    let foldered: HashSet<u64> = folders
        .iter()
        .flat_map(|f| f.guild_ids.iter().copied())
        .collect();
    let mut seen = HashSet::new();
    let mut out: Vec<SidebarEntry> = state
        .post_ready_guild_ids()
        .into_iter()
        .rev()
        .chain(state.ready_guild_ids())
        .filter(|id| !foldered.contains(id))
        .filter_map(|id| state.guild(id))
        .filter(|g| seen.insert(g.id))
        .map(|g| SidebarEntry::Guild(sidebar_guild(state, g, now)))
        .collect();
    for f in folders {
        let known: Vec<SidebarGuild> = f
            .guild_ids
            .iter()
            .filter_map(|id| state.guild(*id))
            .filter(|g| seen.insert(g.id))
            .map(|g| sidebar_guild(state, g, now))
            .collect();
        if f.id.is_some() && known.len() >= 2 {
            out.push(SidebarEntry::Folder {
                id: f.id,
                name: f.name.clone(),
                color: f.color,
                guilds: known,
            });
        } else {
            out.extend(known.into_iter().map(SidebarEntry::Guild));
        }
    }
    out
}

const LISTED_KINDS: [u8; 6] = [0, 2, 5, 13, 15, 16];
const CATEGORY: u8 = 4;

/// Channels of `guild` the user can see; a channel whose visibility is undecidable
/// (`can_view_channel == None`) is left out.
fn listed_channels<'a>(state: &State, guild: &'a Guild) -> Vec<&'a Channel> {
    let mut undecidable = false;
    let listed = guild
        .channels
        .iter()
        .filter(|c| LISTED_KINDS.contains(&c.kind))
        .filter(|c| match state.can_view_channel(c) {
            Some(visible) => visible,
            None => {
                undecidable = true;
                false
            }
        })
        .collect();
    if undecidable {
        warn_undecidable_once(guild.id);
    }
    listed
}

/// @everyone (the role with the guild's id) is denied VIEW_CHANNEL on the channel itself.
fn is_private(guild: &Guild, c: &Channel) -> bool {
    c.permission_overwrites.as_ref().is_some_and(|os| {
        os.iter()
            .any(|o| o.kind == 0 && o.id == guild.id && o.deny & VIEW_CHANNEL != 0)
    })
}

fn sidebar_channel(state: &State, guild: &Guild, c: &Channel, now: i64) -> SidebarChannel {
    let b = guild_badge(state, guild.id, c.id, c.last_message_id, now);
    SidebarChannel {
        id: c.id,
        kind: c.kind,
        name: c.name.clone().unwrap_or_default(),
        unread: b.unread,
        mention_count: b.mentions,
        muted: b.muted,
        private: is_private(guild, c),
        occupants: state
            .voice
            .in_channel(c.id)
            .map(|(user, nick)| {
                let u = state.users.get(&user);
                SidebarOccupant {
                    id: user,
                    name: nick
                        .filter(|n| !n.is_empty())
                        .or_else(|| u.map(display_name))
                        .unwrap_or("Unknown user")
                        .to_owned(),
                    icon_url: u.map(|u| {
                        avatar_url(u.id, u.avatar.as_deref(), u.discriminator.as_deref())
                    }),
                }
            })
            .collect(),
        threads: state
            .threads
            .shown_under(c.id)
            .into_iter()
            .map(|t| {
                let b = guild_badge(state, guild.id, t.id, t.last_message_id, now);
                SidebarThread {
                    id: t.id,
                    name: t.name.clone().unwrap_or_default(),
                    unread: b.unread,
                    mention_count: b.mentions,
                    muted: b.muted,
                }
            })
            .collect(),
    }
}

/// Channels of `guild_id` the signed-in user can see, grouped as the official client does: a root
/// block (channels whose parent is not a known category) first, then categories by (position, id).
/// Inside a block text-like channels precede voice and stage ones, then (position, id). A category
/// appears only with at least one listed child. A channel whose visibility is undecidable
/// (`can_view_channel == None`) is hidden. Unknown guild gives an empty list.
pub fn channel_categories(state: &State, guild_id: u64) -> Vec<SidebarCategory> {
    let Some(guild) = state.guild(guild_id) else {
        return Vec::new();
    };
    let now = now_ms();
    let listed = listed_channels(state, guild);
    let categories: Vec<_> = guild
        .channels
        .iter()
        .filter(|c| c.kind == CATEGORY)
        .collect();
    let block = |parent: Option<u64>| {
        let mut children: Vec<_> = listed
            .iter()
            .filter(|c| match parent {
                Some(p) => c.parent_id == Some(p),
                None => !c
                    .parent_id
                    .is_some_and(|p| categories.iter().any(|k| k.id == p)),
            })
            .collect();
        children.sort_by_key(|c| (matches!(c.kind, 2 | 13), c.position.unwrap_or(0), c.id));
        children
            .into_iter()
            .map(|c| sidebar_channel(state, guild, c, now))
            .collect::<Vec<_>>()
    };
    let mut out = Vec::new();
    let root = block(None);
    if !root.is_empty() {
        out.push(SidebarCategory {
            id: None,
            name: None,
            channels: root,
        });
    }
    let mut ordered = categories.clone();
    ordered.sort_by_key(|c| (c.position.unwrap_or(0), c.id));
    for cat in ordered {
        let channels = block(Some(cat.id));
        if !channels.is_empty() {
            out.push(SidebarCategory {
                id: Some(cat.id),
                name: Some(cat.name.clone().unwrap_or_default()),
                channels,
            });
        }
    }
    out
}

/// One warning per guild per process, not one per recompute.
fn warn_undecidable_once(guild_id: u64) {
    static WARNED: OnceLock<Mutex<HashSet<u64>>> = OnceLock::new();
    let mut warned = WARNED
        .get_or_init(Default::default)
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    if warned.insert(guild_id) {
        tracing::warn!(guild_id, "channels hidden: permissions undecidable");
    }
}

fn display_name(user: &crate::state::User) -> &str {
    user.global_name
        .as_deref()
        .filter(|n| !n.is_empty())
        .unwrap_or(&user.username)
}

fn dm_name(state: &State, dm: &PrivateChannel) -> String {
    let who = state.recipients(dm);
    if dm.kind == 3 {
        if let Some(name) = dm.name.as_deref().filter(|n| !n.is_empty()) {
            return name.to_owned();
        }
        let names: Vec<_> = who.iter().map(|u| display_name(u)).collect();
        return if names.is_empty() {
            "Group DM".to_owned()
        } else {
            names.join(", ")
        };
    }
    who.first()
        .map_or_else(|| "Unknown user".to_owned(), |u| display_name(u).to_owned())
}

/// Avatar of `user_id`, or Discord's default avatar when there is no hash: index
/// `(id >> 22) % 6` for the new username system (discriminator "0"), else `discriminator % 5`.
pub(crate) fn avatar_url(user_id: u64, hash: Option<&str>, discriminator: Option<&str>) -> String {
    match hash {
        Some(h) => format!("{CDN}/avatars/{user_id}/{h}.png?size=64"),
        None => {
            let legacy = discriminator
                .and_then(|d| d.parse::<u64>().ok())
                .filter(|d| *d != 0);
            let index = legacy.map_or((user_id >> 22) % 6, |d| d % 5);
            format!("{CDN}/embed/avatars/{index}.png")
        }
    }
}

fn dm_icon_url(state: &State, dm: &PrivateChannel) -> Option<String> {
    if dm.kind == 3 {
        return dm
            .icon
            .as_deref()
            .map(|h| format!("{CDN}/channel-icons/{}/{h}.png?size=64", dm.id));
    }
    let user = *state.recipients(dm).first()?;
    Some(avatar_url(
        user.id,
        user.avatar.as_deref(),
        user.discriminator.as_deref(),
    ))
}

/// DMs and group DMs, newest `last_message_id` first (none last), ties by channel id descending.
pub fn dm_list(state: &State) -> Vec<SidebarDm> {
    let now = now_ms();
    let mut dms: Vec<_> = state.private_channels.iter().collect();
    dms.sort_by_key(|c| Reverse((c.last_message_id, c.id)));
    dms.into_iter()
        .map(|c| {
            let b = Badge::new(
                state,
                c.id,
                c.last_message_id,
                state.settings.dm_muted(c.id, now),
            );
            SidebarDm {
                id: c.id,
                name: dm_name(state, c),
                icon_url: dm_icon_url(state, c),
                unread: b.unread,
                mention_count: b.mentions,
                // No presence record means offline: READY_SUPPLEMENTAL lists online people only.
                status: (c.kind == 1).then(|| {
                    c.recipient_ids
                        .first()
                        .and_then(|u| state.presence.get(*u))
                        .unwrap_or(PresenceStatus::Offline)
                }),
            }
        })
        .collect()
}
