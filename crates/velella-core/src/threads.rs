//! Threads shown under their parent channel: active (not archived), joined by the user, and under
//! a parent the user can see. Rules from Concord (refs/concord/src/discord/channel/state.rs
//! `thread_is_sidebar_active`, thread/state.rs `apply_thread_list_sync`, gateway/parser/
//! {ready,channels}.rs, GPL-3.0-only): threads in READY or GUILD_CREATE guilds count as joined;
//! THREAD_CREATE/UPDATE join only when they carry the user's own `member`; THREAD_MEMBER_UPDATE and
//! THREAD_MEMBERS_UPDATE join and leave; THREAD_LIST_SYNC keeps the listed parents' active set.
use crate::state::{get_id, get_str, id};
use serde_json::Value;

const THREAD_KINDS: [u64; 3] = [10, 11, 12];

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Thread {
    pub id: u64,
    pub guild_id: u64,
    pub parent_id: u64,
    pub name: Option<String>,
    pub archived: bool,
    pub last_message_id: Option<u64>,
    pub joined: bool,
    /// Whether `thread_metadata.archived` was present: `archived` is false otherwise, which
    /// then says nothing (F1).
    pub archived_known: bool,
    /// `thread_metadata.locked`; `None` when the metadata or the key was absent (F1).
    pub locked: Option<bool>,
}

#[derive(Clone, Debug, Default)]
pub struct Threads(Vec<Thread>);

/// A thread object; `joined` is whether it carried the user's own thread member.
fn parse(v: &Value, default_guild: Option<u64>) -> Option<Thread> {
    if !THREAD_KINDS.contains(&v.get("type")?.as_u64()?) {
        return None;
    }
    Some(Thread {
        id: get_id(v, "id")?,
        guild_id: get_id(v, "guild_id").or(default_guild)?,
        parent_id: get_id(v, "parent_id")?,
        name: get_str(v, "name"),
        archived: v
            .get("thread_metadata")
            .and_then(|m| m.get("archived"))
            .and_then(Value::as_bool)
            .unwrap_or(false),
        last_message_id: get_id(v, "last_message_id"),
        joined: v
            .get("member")
            .or_else(|| v.get("thread_member"))
            .is_some_and(Value::is_object),
        archived_known: metadata_bool(v, "archived").is_some(),
        locked: metadata_bool(v, "locked"),
    })
}

fn metadata_bool(v: &Value, key: &str) -> Option<bool> {
    v.get("thread_metadata")?.get(key)?.as_bool()
}

impl Threads {
    pub fn get(&self, thread: u64) -> Option<&Thread> {
        self.0.iter().find(|t| t.id == thread)
    }

    pub fn parent(&self, thread: u64) -> Option<u64> {
        self.get(thread).map(|t| t.parent_id)
    }

    /// Threads to draw under `parent`: active and joined, most recent activity first.
    pub fn shown_under(&self, parent: u64) -> Vec<&Thread> {
        let mut out: Vec<_> = self
            .0
            .iter()
            .filter(|t| t.parent_id == parent && !t.archived && t.joined)
            .collect();
        out.sort_by_key(|t| std::cmp::Reverse((t.last_message_id.unwrap_or(t.id), t.id)));
        out
    }

    pub(crate) fn last_message_slot(&mut self, thread: u64) -> Option<&mut Option<u64>> {
        self.0
            .iter_mut()
            .find(|t| t.id == thread)
            .map(|t| &mut t.last_message_id)
    }

    pub(crate) fn remove_guild(&mut self, guild: u64) {
        self.0.retain(|t| t.guild_id != guild);
    }

    /// Insert or update; an update that does not carry a thread member keeps the joined flag.
    fn upsert(&mut self, mut t: Thread) -> bool {
        match self.0.iter_mut().find(|x| x.id == t.id) {
            Some(slot) => {
                t.joined |= slot.joined;
                t.last_message_id = t.last_message_id.max(slot.last_message_id);
                let changed = *slot != t;
                *slot = t;
                changed
            }
            None => {
                self.0.push(t);
                true
            }
        }
    }

    /// The `threads` array of a READY, READY_SUPPLEMENTAL or GUILD_CREATE guild: the user's own
    /// active threads, so all of them count as joined. `replace` drops the guild's others first.
    pub(crate) fn guild_payload(&mut self, guild: u64, raw: &[Value], replace: bool) -> bool {
        let mut changed = false;
        if replace {
            let before = self.0.len();
            self.0.retain(|t| t.guild_id != guild);
            changed = self.0.len() != before;
        }
        for t in raw.iter().filter_map(|v| parse(v, Some(guild))) {
            changed |= self.upsert(Thread { joined: true, ..t });
        }
        changed
    }

    /// THREAD_CREATE / THREAD_UPDATE.
    pub(crate) fn upsert_event(&mut self, d: &Value) -> bool {
        parse(d, None).is_some_and(|t| self.upsert(t))
    }

    /// THREAD_DELETE.
    pub(crate) fn delete(&mut self, d: &Value) -> bool {
        let Some(thread) = get_id(d, "id") else {
            return false;
        };
        let before = self.0.len();
        self.0.retain(|t| t.id != thread);
        self.0.len() != before
    }

    /// THREAD_LIST_SYNC: the listed threads are the guild's active ones under `channel_ids` (all
    /// parents when absent); other active threads under those parents are gone. A present
    /// `members` array says which of the listed threads the user is in.
    pub(crate) fn list_sync(&mut self, d: &Value) -> bool {
        let (Some(guild), Some(listed)) = (
            get_id(d, "guild_id"),
            d.get("threads").and_then(Value::as_array),
        ) else {
            return false;
        };
        let parents: Option<Vec<u64>> = d
            .get("channel_ids")
            .and_then(Value::as_array)
            .map(|a| a.iter().filter_map(id).collect());
        let members: Option<Vec<u64>> = d.get("members").and_then(Value::as_array).map(|a| {
            a.iter()
                .filter_map(|m| get_id(m, "id").or_else(|| get_id(m, "thread_id")))
                .collect()
        });
        let threads: Vec<Thread> = listed
            .iter()
            .filter_map(|v| parse(v, Some(guild)))
            .collect();
        let mut changed = false;
        let before = self.0.len();
        self.0.retain(|t| {
            t.guild_id != guild
                || t.archived
                || !parents.as_ref().is_none_or(|p| p.contains(&t.parent_id))
                || threads.iter().any(|s| s.id == t.id)
        });
        changed |= self.0.len() != before;
        for mut t in threads {
            if let Some(members) = &members {
                // Authoritative for the listed threads.
                t.joined = members.contains(&t.id);
                if let Some(slot) = self.0.iter_mut().find(|x| x.id == t.id) {
                    // `upsert` compares against the slot, so record a membership change here,
                    // before the slot is reset for it.
                    changed |= slot.joined != t.joined;
                    slot.joined = false;
                }
            }
            changed |= self.upsert(t);
        }
        changed
    }

    fn set_joined(&mut self, thread: u64, joined: bool) -> bool {
        match self.0.iter_mut().find(|t| t.id == thread) {
            Some(t) if t.joined != joined => {
                t.joined = joined;
                true
            }
            _ => false,
        }
    }

    /// THREAD_MEMBER_UPDATE: the user's own thread member changed, which joins them.
    pub(crate) fn member_update(&mut self, d: &Value, me: Option<u64>) -> bool {
        let Some(thread) = get_id(d, "id").or_else(|| get_id(d, "thread_id")) else {
            return false;
        };
        if get_id(d, "user_id").is_some_and(|u| Some(u) != me) {
            return false;
        }
        self.set_joined(thread, true)
    }

    /// THREAD_MEMBERS_UPDATE: the user among the added members joins, among the removed leaves.
    pub(crate) fn members_update(&mut self, d: &Value, me: Option<u64>) -> bool {
        let (Some(thread), Some(me)) = (get_id(d, "id"), me) else {
            return false;
        };
        let added = d
            .get("added_members")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .any(|m| get_id(m, "user_id") == Some(me));
        let removed = d
            .get("removed_member_ids")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(id)
            .any(|u| u == me);
        if added {
            self.set_joined(thread, true)
        } else if removed {
            self.set_joined(thread, false)
        } else {
            false
        }
    }
}
