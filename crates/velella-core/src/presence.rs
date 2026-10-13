//! Presence for 1:1 DM recipients. Shapes from Concord (refs/concord/src/discord/gateway/parser/
//! {ready,presence,shared}.rs, GPL-3.0-only): READY_SUPPLEMENTAL `merged_presences` (nested guild
//! and friends arrays) and PRESENCE_UPDATE.
use crate::state::{get_id, State};
use serde_json::Value;
use std::collections::HashMap;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PresenceStatus {
    Online,
    Idle,
    DoNotDisturb,
    Offline,
}

#[derive(Clone, Debug, Default)]
pub struct Presence(HashMap<u64, PresenceStatus>);

impl Presence {
    pub fn get(&self, user: u64) -> Option<PresenceStatus> {
        self.0.get(&user).copied()
    }
}

fn status(v: &Value) -> Option<PresenceStatus> {
    match v.get("status")?.as_str()? {
        "online" => Some(PresenceStatus::Online),
        "idle" => Some(PresenceStatus::Idle),
        "dnd" => Some(PresenceStatus::DoNotDisturb),
        "offline" | "invisible" => Some(PresenceStatus::Offline),
        _ => None,
    }
}

fn user_of(v: &Value) -> Option<u64> {
    v.get("user")
        .and_then(|u| get_id(u, "id"))
        .or_else(|| get_id(v, "user_id"))
}

impl State {
    fn is_dm_recipient(&self, user: u64) -> bool {
        self.private_channels
            .iter()
            .any(|c| c.kind == 1 && c.recipient_ids.contains(&user))
    }

    fn set_presence(&mut self, user: u64, status: PresenceStatus) -> bool {
        self.presence.0.insert(user, status) != Some(status)
    }

    /// PRESENCE_UPDATE. Guild members arrive in the thousands, so only a user already tracked or
    /// a DM recipient is kept.
    pub(crate) fn presence_update(&mut self, d: &Value) -> bool {
        let (Some(user), Some(status)) = (user_of(d), status(d)) else {
            return false;
        };
        (self.presence.0.contains_key(&user) || self.is_dm_recipient(user))
            && self.set_presence(user, status)
    }

    /// READY_SUPPLEMENTAL `merged_presences`. Everything in the `friends` array is kept; the guild
    /// arrays hold every member of each guild, so only DM recipients are taken from them.
    pub(crate) fn merged_presences(&mut self, d: &Value) -> bool {
        let Some(merged) = d.get("merged_presences") else {
            return false;
        };
        self.collect_presences(merged, false)
    }

    fn collect_presences(&mut self, v: &Value, friends: bool) -> bool {
        if let (Some(user), Some(status)) = (user_of(v), status(v)) {
            return (friends || self.is_dm_recipient(user)) && self.set_presence(user, status);
        }
        let mut changed = false;
        match v {
            Value::Array(items) => {
                for item in items {
                    changed |= self.collect_presences(item, friends);
                }
            }
            Value::Object(map) => {
                for (key, item) in map {
                    changed |= self.collect_presences(item, friends || key == "friends");
                }
            }
            _ => {}
        }
        changed
    }
}
