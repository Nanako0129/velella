//! Who is in each guild voice channel. Shapes from Concord (refs/concord/src/discord/gateway/
//! parser/{voice,ready}.rs, GPL-3.0-only): `voice_states` on READY_SUPPLEMENTAL (and READY or
//! GUILD_CREATE) guilds, then VOICE_STATE_UPDATE; a null `channel_id` is a leave.
use crate::state::{get_id, get_str, parse_user, State};
use serde_json::Value;

#[derive(Clone, Debug, PartialEq, Eq)]
struct Occupant {
    guild: u64,
    user: u64,
    channel: u64,
    /// Guild nickname from the voice state's `member`.
    nick: Option<String>,
}

/// In arrival order, which is the order the channel row lists them in.
#[derive(Clone, Debug, Default)]
pub struct VoiceStates(Vec<Occupant>);

impl VoiceStates {
    /// `(user id, nickname)` of everyone in `channel`.
    pub fn in_channel(&self, channel: u64) -> impl Iterator<Item = (u64, Option<&str>)> {
        self.0
            .iter()
            .filter(move |o| o.channel == channel)
            .map(|o| (o.user, o.nick.as_deref()))
    }

    pub(crate) fn clear_guild(&mut self, guild: u64) -> bool {
        let before = self.0.len();
        self.0.retain(|o| o.guild != guild);
        self.0.len() != before
    }

    fn set(&mut self, o: Occupant) -> bool {
        match self
            .0
            .iter()
            .position(|x| x.guild == o.guild && x.user == o.user)
        {
            Some(i) if self.0[i].channel == o.channel => {
                let changed = self.0[i] != o;
                self.0[i] = o;
                changed
            }
            Some(i) => {
                self.0.remove(i);
                self.0.push(o);
                true
            }
            None => {
                self.0.push(o);
                true
            }
        }
    }

    fn leave(&mut self, guild: u64, user: u64) -> bool {
        let before = self.0.len();
        self.0.retain(|x| !(x.guild == guild && x.user == user));
        self.0.len() != before
    }
}

impl State {
    /// One voice state; `guild` is given when the payload sits inside a guild object. DM calls
    /// (no guild) are not shown.
    pub(crate) fn voice_update(&mut self, v: &Value, guild: Option<u64>) -> bool {
        let Some(guild) = guild.or_else(|| get_id(v, "guild_id")) else {
            return false;
        };
        let member = v.get("member");
        let user = get_id(v, "user_id").or_else(|| {
            member
                .and_then(|m| m.get("user"))
                .and_then(|u| get_id(u, "id"))
        });
        let Some(user) = user else {
            return false;
        };
        let mut changed = false;
        // The directory resolves names; a voice state's member is often the only place a joiner is.
        if let Some(u) = member.and_then(|m| m.get("user")).and_then(parse_user) {
            if self.users.get(&u.id) != Some(&u) {
                self.users.insert(u.id, u);
                changed = true;
            }
        }
        match get_id(v, "channel_id") {
            Some(channel) => {
                let nick = member.and_then(|m| get_str(m, "nick"));
                changed |= self.voice.set(Occupant {
                    guild,
                    user,
                    channel,
                    nick,
                });
            }
            None => changed |= self.voice.leave(guild, user),
        }
        changed
    }
}
