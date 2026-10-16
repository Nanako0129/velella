//! What the signed-in user has of their own account besides identity (menu plan F1): `premium_type`,
//! the emojis of each guild, whether they are timed out in a guild (an input of `permissions.rs`),
//! and Developer Mode and the reactions setting, which come from the settings ID already parses
//! (`settings_proto.rs`). Every value is Unknown (`None`, absent) until a payload carried it.
//! Sources: READY, READY_SUPPLEMENTAL, GUILD_CREATE, GUILD_MEMBER_UPDATE, USER_UPDATE,
//! GUILD_EMOJIS_UPDATE; shapes from identify.md §1 (bits 4, 5, 10) and menu-requests.md §18.7.
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
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Timeout {
    /// The member object carried none (or null).
    Clear,
    /// Unix milliseconds.
    Until(i64),
    /// Present but not a timestamp: neither timed out nor clear is known.
    Unreadable,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SelfState {
    /// `None` until READY or USER_UPDATE carried it (0 none, 1 Classic, 2 Nitro, 3 Basic).
    pub premium_type: Option<u8>,
    /// Emoji lists by guild id; a guild missing here is Unknown, an empty list is known empty.
    pub emojis: HashMap<u64, Vec<GuildEmoji>>,
    /// Own member's timeout by guild id; a guild missing here is Unknown.
    pub member_timeouts: HashMap<u64, Timeout>,
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

fn timeout_of(member: &Value) -> Timeout {
    match member.get("communication_disabled_until") {
        None | Some(Value::Null) => Timeout::Clear,
        Some(v) => v
            .as_str()
            .and_then(timestamp_ms)
            .map_or(Timeout::Unreadable, Timeout::Until),
    }
}

/// The timeout of the first member in `members` that is `own`.
fn own_timeout<'a>(members: impl Iterator<Item = &'a Value>, own: u64) -> Option<Timeout> {
    members
        .into_iter()
        .find(|m| own_member_roles(m, own).is_some())
        .map(timeout_of)
}

impl SelfState {
    /// READY. `State::ready` starts from a default `SelfState`, so this only adds. Guild members
    /// come first and `merged_members` (index-aligned with `guilds`) overrides them, as the
    /// roles do in `State::ready_guild`.
    pub(crate) fn ready(&mut self, d: &Value, own: u64) {
        self.premium_type = d
            .get("user")
            .and_then(|u| u.get("premium_type"))
            .and_then(Value::as_u64)
            .and_then(|p| u8::try_from(p).ok());
        let merged = d.get("merged_members").and_then(Value::as_array);
        for (i, raw) in items(d.get("guilds")).enumerate() {
            let Some(gid) = get_id(raw, "id") else {
                continue;
            };
            self.guild_payload(raw, gid, own);
            let mine = merged
                .and_then(|m| m.get(i))
                .and_then(|m| own_timeout(items(Some(m)), own));
            if let Some(t) = mine {
                self.member_timeouts.insert(gid, t);
            }
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
                .and_then(|m| own_timeout(items(Some(m)), own));
            if let (Some(gid), Some(t)) = (get_id(raw, "id"), mine) {
                self.member_timeouts.insert(gid, t);
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

    /// The own member's timeout and a guild's emojis, from a READY or GUILD_CREATE guild object.
    /// A full guild replaces the emoji list; a partial one (`partial_updates.emojis` and
    /// `deleted_emoji_ids`, identify.md §1 bit 10) changes a list already held and leaves an
    /// unknown one unknown.
    fn guild_payload(&mut self, raw: &Value, gid: u64, own: u64) {
        let members = items(raw.get("members")).chain(raw.get("member"));
        if let Some(t) = own_timeout(members, own) {
            self.member_timeouts.insert(gid, t);
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
            self.member_timeouts.insert(gid, timeout_of(d));
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

    /// USER_UPDATE of the user themselves; a payload without `premium_type` keeps the value.
    pub(crate) fn user_update(&mut self, d: &Value, own: Option<u64>) -> bool {
        if d.get("id").and_then(id) != own || own.is_none() {
            return false;
        }
        let Some(p) = d
            .get("premium_type")
            .and_then(Value::as_u64)
            .and_then(|p| u8::try_from(p).ok())
        else {
            return false;
        };
        self.premium_type.replace(p) != Some(p)
    }

    /// Whether the member is timed out in `guild` at `now_ms`; `None` when not known.
    pub(crate) fn timed_out(&self, guild: u64, now_ms: i64) -> Option<bool> {
        match self.member_timeouts.get(&guild)? {
            Timeout::Clear => Some(false),
            Timeout::Until(t) => Some(*t > now_ms),
            Timeout::Unreadable => None,
        }
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
