//! What the signed-in user has of their own account besides identity (menu plan F1): `premium_type`,
//! the emojis of each guild, the own member's state in each guild and the account's MFA state
//! (inputs of `permissions.rs`),
//! and Developer Mode and the reactions setting, which come from the settings ID already parses
//! (`settings_proto.rs`). Every value is Unknown (`None`, absent) until a payload carried it.
//! Sources: READY, READY_SUPPLEMENTAL, GUILD_CREATE, GUILD_UPDATE (`mfa_level`), GUILD_MEMBER_UPDATE,
//! USER_UPDATE, GUILD_EMOJIS_UPDATE; shapes from identify.md §1 (bits 4, 5, 10) and menu-requests.md §18.7.
use crate::embeds::timestamp_ms;
use crate::state::{get_id, get_str, id, ids, own_member_roles, State};
use serde_json::Value;
use std::collections::HashMap;

/// One guild emoji (`available` is true unless the server says false).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GuildEmoji {
    pub id: u64,
    pub name: String,
    pub animated: bool,
    pub available: bool,
    /// Roles allowed to use it; empty = everyone.
    pub roles: Vec<u64>,
    pub managed: bool,
}

/// The signed-in member's `communication_disabled_until` in one guild.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Timeout {
    /// The member object carried none (or null).
    #[default]
    Clear,
    /// Unix milliseconds.
    Until(i64),
    /// Present but not a timestamp: neither timed out nor clear is known.
    Unreadable,
}

/// Member flags AUTOMOD_QUARANTINED_USERNAME_OR_GUILD_NICKNAME (128), AUTOMOD_QUARANTINED_BIO
/// (256) and AUTOMOD_QUARANTINED_SERVER_TAG (1024): the enum at W:5002345 (module 340837), read
/// by the quarantine test at module 229527.
const QUARANTINE_FLAGS: u64 = 128 | 256 | 1024;

/// The signed-in user's own member object in one guild, as far as permissions read it. Absent
/// `pending` and `flags` read as false and 0, as the official member parse does (W:4698817).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct OwnMember {
    pub timeout: Timeout,
    /// Membership screening not passed.
    pub pending: bool,
    pub flags: u64,
}

impl OwnMember {
    fn of(member: &Value) -> Self {
        Self {
            timeout: timeout_of(member),
            pending: member.get("pending").and_then(Value::as_bool) == Some(true),
            flags: member.get("flags").and_then(Value::as_u64).unwrap_or(0),
        }
    }

    pub(crate) fn quarantined(&self) -> bool {
        self.flags & QUARANTINE_FLAGS != 0
    }

    /// Whether the member is timed out at `now_ms`; `None` when the timeout is unreadable.
    pub(crate) fn timed_out(&self, now_ms: i64) -> Option<bool> {
        match self.timeout {
            Timeout::Clear => Some(false),
            Timeout::Until(t) => Some(t > now_ms),
            Timeout::Unreadable => None,
        }
    }
}

/// `Guild.mfa_level` ELEVATED (W:241317): the guild requires MFA for moderator permissions.
const MFA_ELEVATED: u8 = 1;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SelfState {
    /// `None` until READY or USER_UPDATE carried it (0 none, 1 Classic, 2 Nitro, 3 Basic).
    pub premium_type: Option<u8>,
    /// The user's `mfa_enabled`; `None` until READY or USER_UPDATE carried it.
    pub mfa_enabled: Option<bool>,
    /// Emoji lists by guild id; a guild missing here is Unknown, an empty list is known empty.
    pub emojis: HashMap<u64, Vec<GuildEmoji>>,
    /// Own member by guild id; a guild missing here is Unknown.
    pub members: HashMap<u64, OwnMember>,
    /// Guild `mfa_level` by guild id; a guild missing here is Unknown. Set by READY (and the
    /// buffered INITIAL_GUILD), GUILD_CREATE and GUILD_UPDATE.
    pub mfa_levels: HashMap<u64, u8>,
}

fn parse_emoji(v: &Value) -> Option<GuildEmoji> {
    let flag = |k: &str| v.get(k).and_then(Value::as_bool);
    Some(GuildEmoji {
        id: get_id(v, "id")?,
        name: get_str(v, "name")?,
        animated: flag("animated") == Some(true),
        available: flag("available") != Some(false),
        roles: ids(v.get("roles")),
        managed: flag("managed") == Some(true),
    })
}

fn items(v: Option<&Value>) -> impl Iterator<Item = &Value> {
    v.and_then(Value::as_array).into_iter().flatten()
}

/// A guild object's `mfa_level`: on the guild or, with CLIENT_STATE_V2, in `properties`.
fn mfa_level(raw: &Value) -> Option<u8> {
    raw.get("mfa_level")
        .or_else(|| raw.get("properties")?.get("mfa_level"))
        .and_then(Value::as_u64)
        .and_then(|l| u8::try_from(l).ok())
}

fn timeout_of(member: &Value) -> Timeout {
    match member.get("communication_disabled_until") {
        None | Some(Value::Null) => Timeout::Clear,
        Some(v) => v
            .as_str()
            .and_then(timestamp_ms)
            .map_or(Timeout::Unreadable, Timeout::Until),
    }
}

/// The first member in `members` that is `own`.
fn own_member<'a>(members: impl Iterator<Item = &'a Value>, own: u64) -> Option<OwnMember> {
    members
        .into_iter()
        .find(|m| own_member_roles(m, own).is_some())
        .map(OwnMember::of)
}

impl SelfState {
    /// READY's own part: the user. Its guilds come through `ready_guild`.
    pub(crate) fn ready(&mut self, d: &Value) {
        let user = d.get("user");
        self.premium_type = user
            .and_then(|u| u.get("premium_type"))
            .and_then(Value::as_u64)
            .and_then(|p| u8::try_from(p).ok());
        self.mfa_enabled = user
            .and_then(|u| u.get("mfa_enabled"))
            .and_then(Value::as_bool);
    }

    /// One READY guild or the buffered INITIAL_GUILD, as `State::ready_guild` adds it: its own
    /// member objects first, then `merged` (READY's `merged_members[i]`, none for the initial
    /// guild) overriding them, as the roles do.
    pub(crate) fn ready_guild(&mut self, raw: &Value, merged: Option<&Value>, own: u64) {
        let Some(gid) = get_id(raw, "id") else {
            return;
        };
        self.guild_payload(raw, gid, own);
        if let Some(m) = merged.and_then(|m| own_member(items(Some(m)), own)) {
            self.members.insert(gid, m);
        }
    }

    /// READY_SUPPLEMENTAL: `merged_members[i]` for `guilds[i]`.
    pub(crate) fn ready_supplemental(&mut self, d: &Value, own: Option<u64>) {
        let Some(own) = own else {
            return;
        };
        let merged = d.get("merged_members").and_then(Value::as_array);
        for (i, raw) in items(d.get("guilds")).enumerate() {
            let mine = merged
                .and_then(|m| m.get(i))
                .and_then(|m| own_member(items(Some(m)), own));
            if let (Some(gid), Some(m)) = (get_id(raw, "id"), mine) {
                self.members.insert(gid, m);
            }
        }
    }

    /// GUILD_CREATE, full or partial (`data_mode`).
    pub(crate) fn guild_create(&mut self, d: &Value, own: Option<u64>) {
        let (Some(gid), Some(own)) = (get_id(d, "id"), own) else {
            return;
        };
        self.guild_payload(d, gid, own);
    }

    /// GUILD_UPDATE: a guild that turns its moderators' MFA requirement on or off.
    pub(crate) fn guild_update(&mut self, d: &Value) {
        if let (Some(gid), Some(level)) = (get_id(d, "id"), mfa_level(d)) {
            self.mfa_levels.insert(gid, level);
        }
    }

    /// The own member's timeout and a guild's emojis, from a READY or GUILD_CREATE guild object.
    /// A full guild replaces the emoji list; a partial one (`partial_updates.emojis` and
    /// `deleted_emoji_ids`, identify.md §1 bit 10) changes a list already held and leaves an
    /// unknown one unknown.
    fn guild_payload(&mut self, raw: &Value, gid: u64, own: u64) {
        let members = items(raw.get("members")).chain(raw.get("member"));
        if let Some(m) = own_member(members, own) {
            self.members.insert(gid, m);
        }
        if let Some(level) = mfa_level(raw) {
            self.mfa_levels.insert(gid, level);
        }
        if raw.get("data_mode").and_then(Value::as_str) == Some("partial") {
            let updates = raw.get("partial_updates");
            if let Some(held) = self.emojis.get_mut(&gid) {
                for e in items(updates.and_then(|u| u.get("emojis"))).filter_map(parse_emoji) {
                    match held.iter_mut().find(|h| h.id == e.id) {
                        Some(slot) => *slot = e,
                        None => held.push(e),
                    }
                }
                let gone = ids(updates.and_then(|u| u.get("deleted_emoji_ids")));
                held.retain(|e| !gone.contains(&e.id));
            }
        } else if let Some(list) = raw.get("emojis").and_then(Value::as_array) {
            self.emojis
                .insert(gid, list.iter().filter_map(parse_emoji).collect());
        }
    }

    /// GUILD_MEMBER_UPDATE of the user themselves.
    pub(crate) fn member_update(&mut self, d: &Value, own: Option<u64>) {
        let (Some(gid), Some(own)) = (get_id(d, "guild_id"), own) else {
            return;
        };
        if own_member_roles(d, own).is_some() {
            self.members.insert(gid, OwnMember::of(d));
        }
    }

    /// GUILD_EMOJIS_UPDATE: `emojis` is the guild's whole list.
    pub(crate) fn emojis_update(&mut self, d: &Value) -> bool {
        let (Some(gid), Some(list)) = (
            get_id(d, "guild_id"),
            d.get("emojis").and_then(Value::as_array),
        ) else {
            return false;
        };
        let next: Vec<GuildEmoji> = list.iter().filter_map(parse_emoji).collect();
        self.emojis.insert(gid, next.clone()) != Some(next)
    }

    /// USER_UPDATE of the user themselves; a payload without a field keeps the held value.
    pub(crate) fn user_update(&mut self, d: &Value, own: Option<u64>) -> bool {
        if d.get("id").and_then(id) != own || own.is_none() {
            return false;
        }
        let premium = d
            .get("premium_type")
            .and_then(Value::as_u64)
            .and_then(|p| u8::try_from(p).ok());
        let mfa = d.get("mfa_enabled").and_then(Value::as_bool);
        let before = (self.premium_type, self.mfa_enabled);
        self.premium_type = premium.or(self.premium_type);
        self.mfa_enabled = mfa.or(self.mfa_enabled);
        before != (self.premium_type, self.mfa_enabled)
    }

    /// Whether the guild's MFA requirement strips moderator permissions from the user (the guild
    /// is ELEVATED and the user has no MFA); `None` when that cannot be told.
    pub(crate) fn strips_moderator(&self, guild: u64) -> Option<bool> {
        if self.mfa_enabled == Some(true) {
            return Some(false);
        }
        if *self.mfa_levels.get(&guild)? != MFA_ELEVATED {
            return Some(false);
        }
        self.mfa_enabled.map(|enabled| !enabled)
    }
}

impl State {
    /// `appearance.developer_mode`; `None` while the settings are unknown (default off once known).
    pub fn developer_mode(&self) -> Option<bool> {
        self.user_settings.as_ref().map(|s| s.developer_mode)
    }

    /// `text_and_images.render_reactions`; `None` while the settings are unknown (default on).
    pub fn render_reactions(&self) -> Option<bool> {
        self.user_settings.as_ref().map(|s| s.render_reactions)
    }

    /// Emojis of every present guild whose list is known, in guild order.
    pub fn guild_emojis(&self) -> Vec<(u64, &[GuildEmoji])> {
        self.guilds
            .iter()
            .filter_map(|g| Some((g.id, self.self_state.emojis.get(&g.id)?.as_slice())))
            .collect()
    }
}
