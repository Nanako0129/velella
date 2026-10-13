//! The member list of the selected guild channel, built from GUILD_MEMBER_LIST_UPDATE.
//!
//! Ported rules: list identity and retirement from Serein (`crates/discord-gateway/src/lib.rs`
//! `adopt_list` and the retired lists, MIT OR Apache-2.0); row bookkeeping (header items occupy
//! indices, group counts refreshed from the top-level `groups`, implicit "Members" group, group
//! names and role colours) from Concord (`src/discord/member/list.rs`,
//! `gateway/parser/members.rs`, `tui/state/member_grouping.rs`, GPL-3.0-only); the list-id
//! computation (`member_list_id`, `murmur3`) from Serein's `crates/model/src/permissions.rs`. See
//! THIRD_PARTY_NOTICES.md.
use crate::{
    session::{SessionHandle, Subscription},
    state::{get_id, get_str, MemberSub, Overwrite, State},
};
use serde_json::Value;
use std::collections::BTreeMap;

const CDN: &str = "https://cdn.discordapp.com";
/// Rows the op-37 range frame asks for (`[[0, 99]]`); a SYNC must overlap them to be adopted.
const SUBSCRIBED_ROWS: std::ops::RangeInclusive<u32> = 0..=99;
/// Retired ids remembered; older ones are forgotten (Serein keeps a short list too).
const RETIRED_CAP: usize = 8;
const VIEW_CHANNEL: u64 = 1 << 10;
/// Serein's bound: more overwrites than this have no computed list id.
const MAX_OVERWRITES: usize = 1000;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MemberListStatus {
    Online,
    Idle,
    DoNotDisturb,
    Offline,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MemberListEntry {
    pub id: u64,
    pub name: String,
    pub avatar_url: Option<String>,
    /// RGB integer of the highest-position role that has a colour.
    pub color: Option<u32>,
    pub status: MemberListStatus,
    pub custom_status: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MemberListSection {
    pub id: String,
    pub title: String,
    pub count: u64,
    pub members: Vec<MemberListEntry>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Member {
    id: u64,
    name: String,
    avatar_url: Option<String>,
    role_ids: Vec<u64>,
    status: MemberListStatus,
    custom_status: Option<String>,
}

/// One list index. `Unknown` keeps the index occupied for an item that could not be read, so the
/// indices of the rows after it stay right.
#[derive(Clone, Debug, PartialEq, Eq)]
enum Entry {
    Group { id: String, count: u64 },
    Member(Member),
    Unknown,
}

/// Rows of the one active list. Valid only while `epoch` equals `State::member_sub`'s epoch.
#[derive(Clone, Debug, Default)]
pub(crate) struct MemberList {
    epoch: u64,
    guild_id: u64,
    adopted: Option<String>,
    /// The id computed for the selected channel when this epoch began; `None` when it could not
    /// be computed. With one, only a SYNC carrying it is adopted.
    expected: Option<String>,
    /// (guild, list id) of earlier epochs' adopted lists.
    retired: Vec<(u64, String)>,
    entries: BTreeMap<u32, Entry>,
}

impl MemberList {
    /// A new selection epoch starts. `keep`: the new channel has the adopted list's identity, so
    /// the rows stay and nothing is retired. Otherwise the adopted id is retired and the rows
    /// are emptied.
    fn arm(&mut self, epoch: u64, keep: bool, expected: Option<String>) {
        if self.epoch == epoch {
            return;
        }
        if !keep {
            if let Some(id) = self.adopted.take() {
                self.retired
                    .retain(|(g, i)| *g != self.guild_id || *i != id);
                if self.retired.len() == RETIRED_CAP {
                    self.retired.remove(0);
                }
                self.retired.push((self.guild_id, id));
            }
            self.entries.clear();
        }
        self.expected = expected;
        self.epoch = epoch;
    }

    /// Whether the adopted list is also the list of the channel `sub` selects.
    fn shared_with(&self, state: &State, sub: &MemberSub) -> bool {
        self.adopted.is_some()
            && self.guild_id == sub.guild_id
            && state.channel_list_id(sub.guild_id, sub.channel_id) == self.adopted
    }

    fn apply_op(&mut self, op: &Value, groups: &[Value]) {
        match op.get("op").and_then(Value::as_str) {
            Some("SYNC") => {
                let (Some((start, end)), Some(items)) =
                    (range(op), op.get("items").and_then(Value::as_array))
                else {
                    return;
                };
                self.invalidate(start, end);
                for (index, item) in (start..=end).zip(items) {
                    let entry = parse_item(item, groups, self.guild_id);
                    self.entries.insert(index, entry);
                }
            }
            Some("INSERT") => {
                let (Some(index), Some(item)) = (index(op), op.get("item")) else {
                    return;
                };
                let tail = self.entries.split_off(&index);
                let shifted = tail
                    .into_iter()
                    .filter_map(|(i, e)| Some((i.checked_add(1)?, e)));
                self.entries.extend(shifted);
                let entry = parse_item(item, groups, self.guild_id);
                self.entries.insert(index, entry);
            }
            Some("UPDATE") => {
                let (Some(index), Some(item)) = (index(op), op.get("item")) else {
                    return;
                };
                let entry = parse_item(item, groups, self.guild_id);
                self.entries.insert(index, entry);
            }
            Some("DELETE") => {
                let Some(index) = index(op) else { return };
                self.entries.remove(&index);
                let tail = self.entries.split_off(&index);
                self.entries
                    .extend(tail.into_iter().map(|(i, e)| (i - 1, e)));
            }
            // Only its own range: rows outside it stay.
            Some("INVALIDATE") => {
                if let Some((start, end)) = range(op) {
                    self.invalidate(start, end);
                }
            }
            _ => {}
        }
    }

    fn invalidate(&mut self, start: u32, end: u32) {
        self.entries.retain(|i, _| *i < start || *i > end);
    }

    /// The top-level `groups` array carries the current counts on every update.
    fn refresh_counts(&mut self, groups: &[Value]) {
        for g in groups {
            let (Some(id), Some(new)) = (
                g.get("id").and_then(Value::as_str),
                g.get("count").and_then(Value::as_u64),
            ) else {
                continue;
            };
            for entry in self.entries.values_mut() {
                if let Entry::Group { id: gid, count } = entry {
                    if gid == id {
                        *count = new;
                    }
                }
            }
        }
    }
}

fn index(op: &Value) -> Option<u32> {
    u32::try_from(op.get("index")?.as_u64()?).ok()
}

fn range(op: &Value) -> Option<(u32, u32)> {
    let r = op.get("range")?.as_array()?;
    let start = u32::try_from(r.first()?.as_u64()?).ok()?;
    let end = u32::try_from(r.get(1)?.as_u64()?).ok()?;
    (start <= end).then_some((start, end))
}

/// A SYNC with rows that overlaps the subscribed range.
fn covers_subscription(op: &Value) -> bool {
    op.get("op").and_then(Value::as_str) == Some("SYNC")
        && range(op)
            .is_some_and(|(s, e)| s <= *SUBSCRIBED_ROWS.end() && e >= *SUBSCRIBED_ROWS.start())
        && op
            .get("items")
            .and_then(Value::as_array)
            .is_some_and(|i| !i.is_empty())
}

/// Hashes are interpolated into CDN paths, so anything but a plain token is dropped.
fn clean_hash(v: Option<&Value>) -> Option<&str> {
    v.and_then(Value::as_str)
        .filter(|h| !h.is_empty() && h.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_'))
}

fn parse_item(item: &Value, groups: &[Value], guild_id: u64) -> Entry {
    if let Some(group) = item.get("group") {
        let Some(id) = group.get("id").and_then(Value::as_str) else {
            return Entry::Unknown;
        };
        let count = group.get("count").and_then(Value::as_u64).or_else(|| {
            groups
                .iter()
                .find(|g| g.get("id").and_then(Value::as_str) == Some(id))
                .and_then(|g| g.get("count")?.as_u64())
        });
        return count.map_or(Entry::Unknown, |count| Entry::Group {
            id: id.to_owned(),
            count,
        });
    }
    let member = item.get("member").unwrap_or(item);
    let Some(user) = member.get("user") else {
        return Entry::Unknown;
    };
    let Some(id) = get_id(user, "id") else {
        return Entry::Unknown;
    };
    let name = [
        get_str(member, "nick"),
        get_str(user, "global_name"),
        get_str(user, "username"),
    ]
    .into_iter()
    .flatten()
    .find(|n| !n.is_empty())
    .unwrap_or_else(|| "Unknown".to_owned());
    let avatar_url = match (
        clean_hash(member.get("avatar")),
        clean_hash(user.get("avatar")),
    ) {
        (Some(h), _) => Some(format!(
            "{CDN}/guilds/{guild_id}/users/{id}/avatars/{h}.png?size=64"
        )),
        (None, Some(h)) => Some(format!("{CDN}/avatars/{id}/{h}.png?size=64")),
        _ => None,
    };
    let presence = item.get("presence").or_else(|| member.get("presence"));
    let status = match presence
        .and_then(|p| p.get("status"))
        .and_then(Value::as_str)
    {
        Some("online") => MemberListStatus::Online,
        Some("idle") => MemberListStatus::Idle,
        Some("dnd") => MemberListStatus::DoNotDisturb,
        _ => MemberListStatus::Offline,
    };
    // Activity type 4 is the custom status; its text is `state`.
    let custom_status = presence
        .and_then(|p| p.get("activities")?.as_array())
        .and_then(|a| {
            a.iter()
                .find(|a| a.get("type").and_then(Value::as_u64) == Some(4))
        })
        .and_then(|a| get_str(a, "state"))
        .filter(|s| !s.is_empty());
    Entry::Member(Member {
        id,
        name,
        avatar_url,
        role_ids: member
            .get("roles")
            .and_then(Value::as_array)
            .map(|a| a.iter().filter_map(|r| r.as_str()?.parse().ok()).collect())
            .unwrap_or_default(),
        status,
        custom_status,
    })
}

impl State {
    /// GUILD_MEMBER_LIST_UPDATE. Reads `member_sub`: with no selection, or for another guild,
    /// nothing is kept. See the plan's L section for the adoption rules.
    pub(crate) fn member_list_update(&mut self, d: &Value) -> bool {
        let Some(sub) = self.member_sub.clone() else {
            return false;
        };
        let (Some(guild_id), Some(list_id), Some(ops)) = (
            get_id(d, "guild_id"),
            d.get("id")
                .and_then(Value::as_str)
                .filter(|i| !i.is_empty() && i.len() <= 64),
            d.get("ops").and_then(Value::as_array),
        ) else {
            return false;
        };
        let groups = d
            .get("groups")
            .and_then(Value::as_array)
            .map_or(&[][..], Vec::as_slice);
        let (keep, expected) = if self.member_list.epoch == sub.epoch {
            (false, None)
        } else {
            let expected = self.channel_list_id(sub.guild_id, sub.channel_id);
            (self.member_list.shared_with(self, &sub), expected)
        };
        let list = &mut self.member_list;
        list.arm(sub.epoch, keep, expected);
        if guild_id != sub.guild_id {
            return false;
        }
        if list.adopted.as_deref() != Some(list_id) {
            // Only a covering SYNC may adopt a list or replace the adopted one. The computed id
            // is always accepted, which also replaces a fallback adoption. Another id is a
            // fallback (Serein's `adopt_list`): taken while nothing is adopted (or when no id
            // could be computed), and never a list an earlier selection left.
            let retired = list
                .retired
                .iter()
                .any(|(g, i)| *g == guild_id && i == list_id);
            let allowed = list.expected.as_deref() == Some(list_id)
                || ((list.adopted.is_none() || list.expected.is_none()) && !retired);
            if !allowed || !ops.iter().any(covers_subscription) {
                return false;
            }
            list.adopted = Some(list_id.to_owned());
            list.guild_id = guild_id;
            list.entries.clear();
        }
        for op in ops {
            list.apply_op(op, groups);
        }
        list.refresh_counts(groups);
        true
    }
}

/// The member list for (`guild_id`, `channel_id`): empty unless that is the current selection and
/// a list was adopted for it in this epoch, or carried over from a channel with the same list id.
pub fn member_sections(state: &State, guild_id: u64, channel_id: u64) -> Vec<MemberListSection> {
    let list = &state.member_list;
    let selected = state.member_sub.as_ref().is_some_and(|s| {
        s.guild_id == guild_id
            && s.channel_id == channel_id
            && (s.epoch == list.epoch || list.shared_with(state, s))
    });
    if !selected || list.adopted.is_none() {
        return Vec::new();
    }
    let roles = state
        .guild(guild_id)
        .and_then(|g| g.roles.as_deref())
        .unwrap_or_default();
    let role_name = |id: &str| {
        let role = roles.iter().find(|r| Some(r.id) == id.parse().ok())?;
        role.name.clone()
    };
    let mut sections: Vec<MemberListSection> = Vec::new();
    let mut open = false;
    let mut implicit = false;
    let mut previous: Option<u32> = None;
    for (&at, entry) in &list.entries {
        // A hole means the group the next rows belong to is not loaded.
        if previous.is_some_and(|p| p.checked_add(1) != Some(at)) {
            open = false;
        }
        previous = Some(at);
        match entry {
            Entry::Group { id, count } => {
                let title = match id.as_str() {
                    "online" => "Online".to_owned(),
                    "offline" => "Offline".to_owned(),
                    other => role_name(other).unwrap_or_else(|| "Members".to_owned()),
                };
                sections.push(MemberListSection {
                    id: format!("group-{id}"),
                    title,
                    count: *count,
                    members: Vec::new(),
                });
                open = true;
                implicit = false;
            }
            Entry::Member(m) => {
                if !open {
                    sections.push(MemberListSection {
                        id: format!("implicit-{at}"),
                        title: "Members".to_owned(),
                        count: 0,
                        members: Vec::new(),
                    });
                    open = true;
                    implicit = true;
                }
                let section = sections.last_mut().expect("a section is open");
                if implicit {
                    section.count += 1;
                }
                let color = m
                    .role_ids
                    .iter()
                    .filter_map(|id| roles.iter().find(|r| r.id == *id))
                    .filter(|r| r.color != 0)
                    .max_by_key(|r| r.position)
                    .map(|r| r.color);
                section.members.push(MemberListEntry {
                    id: m.id,
                    name: m.name.clone(),
                    avatar_url: m.avatar_url.clone(),
                    color,
                    status: m.status,
                    custom_status: m.custom_status.clone(),
                });
            }
            Entry::Unknown => {}
        }
    }
    sections
}

/// Discord's unofficial list identity for a channel, observed by discord.py-self and ported from
/// Serein (`member_list_id`). It selects a server list; it never grants permissions.
/// `everyone` is the @everyone role's permissions.
fn member_list_id(everyone: u64, overwrites: &[Overwrite]) -> Option<String> {
    if overwrites.len() > MAX_OVERWRITES {
        return None;
    }
    if everyone & VIEW_CHANNEL != 0 && !overwrites.iter().any(|o| o.deny & VIEW_CHANNEL != 0) {
        return Some("everyone".into());
    }
    let mut entries: Vec<String> = overwrites
        .iter()
        .filter_map(|o| {
            if o.allow & VIEW_CHANNEL != 0 {
                Some(format!("allow:{}", o.id))
            } else if o.deny & VIEW_CHANNEL != 0 {
                Some(format!("deny:{}", o.id))
            } else {
                None
            }
        })
        .collect();
    entries.sort();
    Some(murmur3(entries.join(",").as_bytes()).to_string())
}

fn murmur3(bytes: &[u8]) -> u32 {
    let mix = |n: u32| {
        n.wrapping_mul(0xcc9e2d51)
            .rotate_left(15)
            .wrapping_mul(0x1b873593)
    };
    let mut hash = 0u32;
    let (chunks, remainder) = bytes.as_chunks::<4>();
    for part in chunks {
        hash ^= mix(u32::from_le_bytes(*part));
        hash = hash
            .rotate_left(13)
            .wrapping_mul(5)
            .wrapping_add(0xe6546b64);
    }
    let tail = remainder
        .iter()
        .enumerate()
        .fold(0u32, |n, (i, b)| n | (u32::from(*b) << (i * 8)));
    if !remainder.is_empty() {
        hash ^= mix(tail);
    }
    hash ^= bytes.len() as u32;
    hash ^= hash >> 16;
    hash = hash.wrapping_mul(0x85ebca6b);
    hash ^= hash >> 13;
    hash = hash.wrapping_mul(0xc2b2ae35);
    hash ^ (hash >> 16)
}

impl State {
    /// The list id of a channel from the state's roles and overwrites; `None` when any of them is
    /// unknown or the channel has too many overwrites.
    pub(crate) fn channel_list_id(&self, guild_id: u64, channel_id: u64) -> Option<String> {
        let guild = self.guild(guild_id)?;
        let everyone = guild.roles.as_ref()?.iter().find(|r| r.id == guild_id)?;
        let channel = guild.channels.iter().find(|c| c.id == channel_id)?;
        member_list_id(
            everyone.permissions,
            channel.permission_overwrites.as_deref()?,
        )
    }
}

impl SessionHandle {
    /// Selects the member list of (`guild_id`, `channel_id`) and delivers its snapshots (empty
    /// until a list is adopted for this selection). To leave the list (a DM, or nothing
    /// selected), call `set_member_subscription(None)`. Needs a runtime context, like `subscribe`.
    pub fn subscribe_members(
        &self,
        guild_id: u64,
        channel_id: u64,
        deliver: impl Fn(Vec<MemberListSection>) + Send + 'static,
    ) -> Subscription {
        self.set_member_subscription(Some((guild_id, channel_id)));
        self.subscribe(move |s| member_sections(s, guild_id, channel_id), deliver)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn overwrite(id: u64, kind: u8, allow: u64, deny: u64) -> Overwrite {
        Overwrite {
            id,
            kind,
            allow,
            deny,
        }
    }

    #[test]
    fn murmur3_matches_the_reference_vectors() {
        assert_eq!(murmur3(b""), 0);
        assert_eq!(murmur3(b"foo"), 0xf6a5c420);
        assert_eq!(murmur3(b"hello"), 0x248bfa47);
    }

    #[test]
    fn list_id_follows_serein_and_is_bounded() {
        assert_eq!(
            member_list_id(VIEW_CHANNEL, &[]).as_deref(),
            Some("everyone")
        );
        assert_eq!(member_list_id(0, &[]).as_deref(), Some("0"));
        let deny = overwrite(5, 0, 0, VIEW_CHANNEL);
        let allow = overwrite(9, 1, VIEW_CHANNEL | (1 << 40), VIEW_CHANNEL);
        assert_eq!(
            member_list_id(VIEW_CHANNEL, &[deny.clone(), allow.clone()]),
            Some(murmur3(b"allow:9,deny:5").to_string())
        );
        assert_eq!(
            member_list_id(0, &[allow.clone(), deny.clone()]),
            member_list_id(0, &[deny.clone(), allow])
        );
        assert_eq!(
            member_list_id(VIEW_CHANNEL, &vec![deny; MAX_OVERWRITES + 1]),
            None
        );
    }
}
