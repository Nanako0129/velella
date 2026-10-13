//! Incoming friend requests (relationship type 3) for the Friends badge. Shapes from Concord
//! (refs/concord/src/discord/gateway/parser/relationships.rs, GPL-3.0-only).
use crate::state::{get_id, State};
use serde_json::Value;
use std::collections::HashSet;

const INCOMING_REQUEST: u64 = 3;

#[derive(Clone, Debug, Default)]
pub struct Relationships {
    incoming: HashSet<u64>,
}

/// The other user's id: on the entry itself, or under `user` on older shards.
fn user_id(v: &Value) -> Option<u64> {
    get_id(v, "id").or_else(|| v.get("user").and_then(|u| get_id(u, "id")))
}

impl Relationships {
    pub fn pending(&self) -> u32 {
        u32::try_from(self.incoming.len()).unwrap_or(u32::MAX)
    }

    /// READY `relationships`.
    pub(crate) fn sync(&mut self, d: &Value) {
        for r in d
            .get("relationships")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            if r.get("type").and_then(Value::as_u64) == Some(INCOMING_REQUEST) {
                self.incoming.extend(user_id(r));
            }
        }
    }

    /// RELATIONSHIP_ADD: any other type (the request was accepted, or turned into a block)
    /// clears a pending request from that user.
    pub(crate) fn add(&mut self, d: &Value) -> bool {
        let Some(user) = user_id(d) else {
            return false;
        };
        if d.get("type").and_then(Value::as_u64) == Some(INCOMING_REQUEST) {
            self.incoming.insert(user)
        } else {
            self.incoming.remove(&user)
        }
    }

    pub(crate) fn remove(&mut self, d: &Value) -> bool {
        user_id(d).is_some_and(|user| self.incoming.remove(&user))
    }
}

impl State {
    /// Incoming friend requests, for the Friends row badge.
    pub fn friends_pending(&self) -> u32 {
        self.relationships.pending()
    }
}
