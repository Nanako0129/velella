//! Unread and mention state. Rules ported from Concord (refs/concord/src/discord/
//! {notification,read}/state.rs and state.rs `apply_message_create`, GPL-3.0-only; see
//! THIRD_PARTY_NOTICES.md): a channel with a `last_message_id` above its acked id is unread, a
//! channel with no read-state entry is unread, a stale ack changes nothing, and the user's own
//! message marks its channel read locally. velella never sends an ACK (plan Q1).
use crate::guild_settings::now_ms;
use crate::state::{get_id, State};
use serde_json::Value;
use std::collections::HashMap;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ChannelRead {
    pub acked: Option<u64>,
    pub mentions: u32,
}

#[derive(Clone, Debug, Default)]
pub struct ReadStates(HashMap<u64, ChannelRead>);

/// RECIPIENT_REMOVE: a system message that never notifies.
const MESSAGE_RECIPIENT_REMOVE: u64 = 2;

fn count(v: Option<&Value>) -> Option<u32> {
    v.and_then(Value::as_u64)
        .map(|n| u32::try_from(n).unwrap_or(u32::MAX))
}

impl ReadStates {
    pub fn acked(&self, channel: u64) -> Option<u64> {
        self.0.get(&channel).and_then(|r| r.acked)
    }

    /// `(unread, mention_count)` of a channel whose newest message is `last`. No message: read.
    /// No entry, or an acked id below `last`: unread. A mention always makes it unread.
    pub fn status(&self, channel: u64, last: Option<u64>) -> (bool, u32) {
        let Some(last) = last else {
            return (false, 0);
        };
        let read = self.0.get(&channel);
        let mentions = read.map_or(0, |r| r.mentions);
        let acked = read.and_then(|r| r.acked);
        (mentions > 0 || acked.is_none_or(|a| a < last), mentions)
    }

    /// READY `read_state`: `{entries: [...]}` or a bare array; channel entries only (type 0).
    pub(crate) fn sync(&mut self, d: &Value) {
        let Some(node) = d.get("read_state") else {
            return;
        };
        let entries = node.get("entries").unwrap_or(node);
        for e in entries.as_array().into_iter().flatten() {
            if e.get("read_state_type")
                .and_then(Value::as_u64)
                .unwrap_or(0)
                != 0
            {
                continue;
            }
            let Some(channel) = get_id(e, "id") else {
                continue;
            };
            self.0.insert(
                channel,
                ChannelRead {
                    acked: get_id(e, "last_message_id").or_else(|| get_id(e, "last_acked_id")),
                    mentions: count(e.get("mention_count")).unwrap_or(0),
                },
            );
        }
    }

    /// MESSAGE_ACK from the server: an older id than the acked one is stale and ignored; otherwise
    /// the id moves (or stays) and `mention_count`, when sent, replaces the count.
    fn ack(&mut self, channel: u64, message: u64, mentions: Option<u32>) -> bool {
        if self.acked(channel).is_some_and(|a| a > message) {
            return false;
        }
        let entry = self.0.entry(channel).or_default();
        let before = entry.clone();
        entry.acked = Some(message);
        if let Some(m) = mentions {
            entry.mentions = m;
        }
        *entry != before
    }

    /// The user's own message: read up to it, mentions cleared, locally only.
    fn mark_read(&mut self, channel: u64, message: u64) -> bool {
        if self.acked(channel).is_some_and(|a| a >= message) {
            return false;
        }
        let entry = self.0.entry(channel).or_default();
        entry.acked = Some(message);
        entry.mentions = 0;
        true
    }

    fn add_mention(&mut self, channel: u64) {
        let entry = self.0.entry(channel).or_default();
        entry.mentions = entry.mentions.saturating_add(1);
    }
}

impl State {
    pub(crate) fn message_ack(&mut self, d: &Value) -> bool {
        let (Some(channel), Some(message)) = (get_id(d, "channel_id"), get_id(d, "message_id"))
        else {
            return false;
        };
        self.read
            .ack(channel, message, count(d.get("mention_count")))
    }

    /// CHANNEL_UNREAD_UPDATE: newer `last_message_id`s for channels of a guild the client has not
    /// subscribed to.
    pub(crate) fn channel_unread_update(&mut self, d: &Value) -> bool {
        let Some(guild) = get_id(d, "guild_id") else {
            return false;
        };
        let mut changed = false;
        for u in d
            .get("channel_unread_updates")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            if let (Some(channel), Some(last)) = (get_id(u, "id"), get_id(u, "last_message_id")) {
                changed |= self.raise_last_message(channel, Some(guild), last);
            }
        }
        changed
    }

    /// Raises the newest-message id of a guild channel, thread or private channel; never lowers it.
    fn raise_last_message(&mut self, channel: u64, guild: Option<u64>, message: u64) -> bool {
        let slot = match guild {
            Some(g) => self
                .guilds
                .iter_mut()
                .find(|x| x.id == g)
                .and_then(|g| g.channels.iter_mut().find(|c| c.id == channel))
                .map(|c| &mut c.last_message_id)
                .or_else(|| self.threads.last_message_slot(channel)),
            None => self
                .private_channels
                .iter_mut()
                .find(|c| c.id == channel)
                .map(|c| &mut c.last_message_id),
        };
        match slot {
            Some(last) if last.is_none_or(|l| l < message) => {
                *last = Some(message);
                true
            }
            _ => false,
        }
    }

    pub(crate) fn message_create(&mut self, d: &Value) -> bool {
        let (Some(channel), Some(message)) = (get_id(d, "channel_id"), get_id(d, "id")) else {
            return false;
        };
        let guild = get_id(d, "guild_id");
        let mut changed = self.raise_last_message(channel, guild, message);
        let author = d.get("author").and_then(|a| get_id(a, "id"));
        if author.is_some() && author == self.user_id {
            return changed | self.read.mark_read(channel, message);
        }
        if self.read.acked(channel).is_some_and(|a| a >= message) {
            return changed;
        }
        if self.counts_as_mention(channel, guild, d) {
            self.read.add_mention(channel);
            changed = true;
        }
        changed
    }

    /// Whether another user's message adds to the channel's mention count. In a DM or group DM
    /// every message does, unless the DM is muted, which leaves direct mentions. In a guild only a
    /// mention of the user, of one of their roles, or of @everyone does, as far as the guild's
    /// `suppress_*` settings allow.
    fn counts_as_mention(&self, channel: u64, guild: Option<u64>, d: &Value) -> bool {
        let Some(me) = self.user_id else {
            return false;
        };
        let direct = d
            .get("mentions")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .any(|u| get_id(u, "id") == Some(me));
        let Some(guild) = guild else {
            if d.get("type").and_then(Value::as_u64) == Some(MESSAGE_RECIPIENT_REMOVE) {
                return false;
            }
            return direct || !self.settings.dm_muted(channel, now_ms());
        };
        if direct {
            return true;
        }
        let (no_everyone, no_roles) = self
            .settings
            .guild(guild)
            .map_or((false, false), |s| (s.suppress_everyone, s.suppress_roles));
        if !no_everyone && d.get("mention_everyone").and_then(Value::as_bool) == Some(true) {
            return true;
        }
        let mine = self.guild(guild).and_then(|g| g.member_roles.as_deref());
        !no_roles
            && mine.is_some_and(|mine| {
                d.get("mention_roles")
                    .and_then(Value::as_array)
                    .into_iter()
                    .flatten()
                    .filter_map(crate::state::id)
                    .any(|r| mine.contains(&r))
            })
    }
}
