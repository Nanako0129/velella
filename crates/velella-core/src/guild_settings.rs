//! Mute settings from READY `user_guild_settings` and USER_GUILD_SETTINGS_UPDATE. Rules ported from
//! Concord (refs/concord/src/discord/notification/state.rs and gateway/parser/{guilds,shared}.rs,
//! GPL-3.0-only; see THIRD_PARTY_NOTICES.md): a guild is muted by its own flag; a channel by the
//! first override up its ancestry (channel, then parent category); DMs and group DMs by the
//! private scope (`guild_id` null or "@me") and its overrides keyed by DM channel id; a mute with an
//! `end_time` in the past is over.
use crate::state::{get_id, State};
use serde_json::Value;
use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};

/// `muted` plus the optional `mute_config.end_time` (epoch ms; `None` = until unmuted).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct Mute {
    muted: bool,
    until_ms: Option<i64>,
}

impl Mute {
    fn parse(v: &Value) -> Self {
        Mute {
            muted: v.get("muted").and_then(Value::as_bool).unwrap_or(false),
            // An end time that cannot be read mutes for good (Concord).
            until_ms: v
                .get("mute_config")
                .and_then(|c| c.get("end_time"))
                .and_then(Value::as_str)
                .filter(|s| !s.is_empty())
                .and_then(parse_rfc3339_ms),
        }
    }

    fn active(&self, now: i64) -> bool {
        self.muted && self.until_ms.is_none_or(|t| t > now)
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Scope {
    mute: Mute,
    pub suppress_everyone: bool,
    pub suppress_roles: bool,
    overrides: HashMap<u64, Mute>,
}

#[derive(Clone, Debug, Default)]
pub struct NotifySettings {
    guilds: HashMap<u64, Scope>,
    private: Scope,
}

pub(crate) fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| i64::try_from(d.as_millis()).unwrap_or(i64::MAX))
}

// ponytail: expiry is read at compute time, so a mute that ends while nothing else changes shows
// on the next state change; add a timer if that is ever noticed.
impl NotifySettings {
    pub fn guild(&self, guild: u64) -> Option<&Scope> {
        self.guilds.get(&guild)
    }

    pub fn guild_muted(&self, guild: u64, now: i64) -> bool {
        self.guild(guild).is_some_and(|s| s.mute.active(now))
    }

    /// The first override found walking `ancestry` (the channel, then its parents) decides.
    pub fn channel_muted(&self, guild: u64, ancestry: &[u64], now: i64) -> bool {
        self.guild(guild).is_some_and(|s| {
            ancestry
                .iter()
                .find_map(|id| s.overrides.get(id))
                .is_some_and(|m| m.active(now))
        })
    }

    /// A DM is muted by the private scope as a whole or by its own override.
    pub fn dm_muted(&self, channel: u64, now: i64) -> bool {
        self.private.mute.active(now)
            || self
                .private
                .overrides
                .get(&channel)
                .is_some_and(|m| m.active(now))
    }

    /// READY: `{entries: [...]}` or a bare array.
    pub(crate) fn sync(&mut self, d: &Value) {
        let Some(node) = d.get("user_guild_settings") else {
            return;
        };
        for e in node
            .get("entries")
            .unwrap_or(node)
            .as_array()
            .into_iter()
            .flatten()
        {
            self.upsert(e);
        }
    }

    /// USER_GUILD_SETTINGS_UPDATE: the entry replaces the scope's settings whole.
    pub(crate) fn update(&mut self, d: &Value) -> bool {
        self.upsert(d)
    }

    fn upsert(&mut self, v: &Value) -> bool {
        let scope = Scope {
            mute: Mute::parse(v),
            suppress_everyone: v
                .get("suppress_everyone")
                .and_then(Value::as_bool)
                .unwrap_or(false),
            suppress_roles: v
                .get("suppress_roles")
                .and_then(Value::as_bool)
                .unwrap_or(false),
            overrides: parse_overrides(v.get("channel_overrides")),
        };
        match v.get("guild_id") {
            None | Some(Value::Null) => replace(&mut self.private, scope),
            Some(g) if g.as_str() == Some("@me") => replace(&mut self.private, scope),
            Some(g) => match crate::state::id(g) {
                Some(g) => replace(self.guilds.entry(g).or_default(), scope),
                None => false,
            },
        }
    }
}

fn replace(slot: &mut Scope, scope: Scope) -> bool {
    let changed = *slot != scope;
    *slot = scope;
    changed
}

/// `channel_overrides` is an array of entries with `channel_id`, or an object keyed by channel id.
fn parse_overrides(v: Option<&Value>) -> HashMap<u64, Mute> {
    match v {
        Some(Value::Array(a)) => a
            .iter()
            .filter_map(|o| Some((get_id(o, "channel_id")?, Mute::parse(o))))
            .collect(),
        Some(Value::Object(m)) => m
            .iter()
            .filter_map(|(k, o)| Some((k.parse().ok()?, Mute::parse(o))))
            .collect(),
        _ => HashMap::new(),
    }
}

/// `2026-10-14T13:00:00.000000+00:00` (or `Z`) to epoch milliseconds.
fn parse_rfc3339_ms(s: &str) -> Option<i64> {
    let b = s.as_bytes();
    let num = |from: usize, to: usize| s.get(from..to)?.parse::<i64>().ok();
    let (y, mo, d) = (num(0, 4)?, num(5, 7)?, num(8, 10)?);
    let (h, mi, sec) = (num(11, 13)?, num(14, 16)?, num(17, 19)?);
    let mut i = 19;
    let mut frac_ms = 0;
    if b.get(i) == Some(&b'.') {
        let digits = b[i + 1..].iter().take_while(|c| c.is_ascii_digit()).count();
        frac_ms = format!("{:0<3}", &s[i + 1..i + 1 + digits.min(3)]).parse().ok()?;
        i += 1 + digits;
    }
    let offset_s = match b.get(i)? {
        b'Z' | b'z' => 0,
        sign @ (b'+' | b'-') => {
            let minutes = num(i + 1, i + 3)? * 60 + num(i + 4, i + 6)?;
            if *sign == b'+' {
                minutes * 60
            } else {
                -minutes * 60
            }
        }
        _ => return None,
    };
    // Days since 1970-01-01 (Howard Hinnant's civil-date algorithm).
    let y = if mo <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let doy = (153 * (if mo > 2 { mo - 3 } else { mo + 9 }) + 2) / 5 + d - 1;
    let days = era * 146_097 + yoe * 365 + yoe / 4 - yoe / 100 + doy - 719_468;
    Some(((days * 86_400 + h * 3600 + mi * 60 + sec - offset_s) * 1000) + frac_ms)
}

impl State {
    /// Channel ids to check for an override: the channel, then its parent category, or for a
    /// thread its parent channel and that one's category.
    pub(crate) fn mute_ancestry(&self, guild: u64, channel: u64) -> Vec<u64> {
        let mut out = vec![channel];
        let channels = self.guild(guild).map(|g| g.channels.as_slice());
        while out.len() < 4 {
            let cur = out[out.len() - 1];
            let parent = channels
                .and_then(|cs| cs.iter().find(|c| c.id == cur))
                .and_then(|c| c.parent_id)
                .or_else(|| self.threads.parent(cur));
            match parent {
                Some(p) if !out.contains(&p) => out.push(p),
                _ => break,
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::parse_rfc3339_ms;

    #[test]
    fn rfc3339_to_epoch_ms() {
        assert_eq!(parse_rfc3339_ms("1970-01-01T00:00:00Z"), Some(0));
        // 2000-03-01 is day 11017 since the epoch (a leap-year boundary).
        let utc = parse_rfc3339_ms("2000-03-01T00:00:00.000000+00:00");
        assert_eq!(utc, Some(951_868_800_000));
        assert_eq!(parse_rfc3339_ms("2000-03-01T01:00:00.5+01:00"), Some(951_868_800_500));
        assert_eq!(parse_rfc3339_ms("2000-02-29T23:00:00-01:00"), Some(951_868_800_000));
        assert_eq!(parse_rfc3339_ms("not a time"), None);
    }
}
