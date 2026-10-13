//! Gateway policy that does not need a socket: resume-URL allow-list, close-code classification,
//! reconnect backoff. Allow-list and close codes adapted from Serein
//! (refs/serein/crates/discord-gateway/src/lib.rs `validated_url`, `close_action`;
//! MIT OR Apache-2.0).
use crate::identify::GATEWAY_QUERY;
use serde_json::json;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// A `resume_gateway_url` is attacker-influenced input that we would send the token to, so it must
/// be a Discord gateway host. wss only, host `gateway.discord.gg` or one `gateway-<label>.discord.gg`
/// label, no port/userinfo/fragment/query, path `/`. Accepted URLs are normalized to the fixed
/// query; anything else returns `None` and the caller falls back to IDENTIFY on the initial URL.
pub(crate) fn validated_resume_url(value: &str) -> Option<String> {
    let mut url = url::Url::parse(value).ok()?;
    let host = url.host_str()?;
    let label_ok = host
        .strip_prefix("gateway-")
        .and_then(|h| h.strip_suffix(".discord.gg"))
        .is_some_and(|l| {
            !l.is_empty() && l.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
        });
    if url.scheme() != "wss"
        || !(host == "gateway.discord.gg" || label_ok)
        || url.port().is_some()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.path() != "/"
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return None;
    }
    url.set_query(Some(GATEWAY_QUERY));
    Some(url.to_string())
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum CloseAction {
    Resume,
    Identify,
    /// 4004: the token is bad. Terminal, reported as `AuthFailed`.
    AuthFailed,
    /// Other terminal codes. Reported as `Closed`.
    Stop,
}

pub(crate) fn close_action(code: u16) -> CloseAction {
    match code {
        4004 => CloseAction::AuthFailed,
        4010..=4014 | 1008 | 1009 => CloseAction::Stop,
        4007 | 4009 => CloseAction::Identify,
        _ => CloseAction::Resume,
    }
}

/// 0 failures (a healthy session just dropped) reconnects at once; then 0.5 s doubling to 30 s.
pub(crate) fn backoff(failures: u32) -> Duration {
    if failures == 0 {
        return Duration::ZERO;
    }
    let base = 500u64 << (failures - 1).min(6);
    Duration::from_millis(base.min(30_000) + jitter_ms(500))
}

/// Op 37 guild-subscription frame in Serein's shape (refs/serein/crates/discord-gateway/src/lib.rs
/// `subscription_packet`, MIT OR Apache-2.0). Without a channel, `typing: true` subscribes the guild
/// (Discord ignores channel ranges until it is) and `typing: false` leaves it; with a channel it
/// asks for rows 0-99 of that channel's member list.
fn subscription_frame(guild: u64, typing: bool, channel: Option<u64>) -> String {
    let channels = match channel {
        Some(channel) => json!({ channel.to_string(): [[0, 99]] }),
        None => json!({}),
    };
    json!({"op": 37, "d": {"subscriptions": {guild.to_string(): {
        "typing": typing, "threads": false, "activities": true, "member_updates": false,
        "members": [], "channels": channels, "thread_member_lists": []
    }}}})
    .to_string()
}

/// Frames that move one connection's member subscription from `sent` to `want` (guild, channel),
/// in Serein's order (lib.rs:1317-1395, 1516-1530): leave the old guild on a guild switch or a
/// clear; subscribe a guild not yet subscribed on this connection, then its channel ranges. Another
/// channel in the same guild sends the ranges only: re-sending the guild subscribe would replace
/// the open channel.
pub(crate) fn member_frames(sent: Option<(u64, u64)>, want: Option<(u64, u64)>) -> Vec<String> {
    let mut frames = Vec::new();
    if sent == want {
        return frames;
    }
    let old_guild = sent.map(|(g, _)| g);
    let new_guild = want.map(|(g, _)| g);
    if let Some(old) = old_guild.filter(|old| Some(*old) != new_guild) {
        frames.push(subscription_frame(old, false, None));
    }
    if let Some((guild, channel)) = want {
        if old_guild != Some(guild) {
            frames.push(subscription_frame(guild, true, None));
        }
        frames.push(subscription_frame(guild, true, Some(channel)));
    }
    frames
}

/// Cheap jitter from the clock; it only has to de-synchronize clients, not be unpredictable.
pub(crate) fn jitter_ms(max: u64) -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .subsec_nanos() as u64
        % max.max(1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resume_url_allow_list() {
        let q = "?v=9&encoding=json&compress=zlib-stream";
        assert_eq!(
            validated_resume_url("wss://gateway.discord.gg/").unwrap(),
            format!("wss://gateway.discord.gg/{q}")
        );
        assert_eq!(
            validated_resume_url("wss://gateway-x.discord.gg/").unwrap(),
            format!("wss://gateway-x.discord.gg/{q}")
        );
        assert_eq!(
            validated_resume_url("wss://gateway-us-east1-b.discord.gg").unwrap(),
            format!("wss://gateway-us-east1-b.discord.gg/{q}")
        );
        for bad in [
            "ws://gateway.discord.gg/",
            "wss://gateway.discord.gg.evil.test/",
            "wss://evil.test/",
            "wss://gateway.discord.gg@evil.test/",
            "wss://user:pw@gateway.discord.gg/",
            "wss://gateway.discord.gg:8443/",
            "wss://gateway.discord.gg/path",
            "wss://gateway.discord.gg/?v=10",
            "wss://gateway.discord.gg/#frag",
            "wss://gateway-.discord.gg/",
            "wss://gateway-a.b.discord.gg/",
            "wss://gateway-x.discord.gg.evil.test/",
            "wss://notgateway-x.discord.gg/",
            "https://gateway.discord.gg/",
            "gateway.discord.gg",
            "",
        ] {
            assert_eq!(validated_resume_url(bad), None, "{bad}");
        }
    }

    #[test]
    fn close_codes_classify() {
        assert_eq!(close_action(4004), CloseAction::AuthFailed);
        for c in [4010, 4011, 4012, 4013, 4014, 1008, 1009] {
            assert_eq!(close_action(c), CloseAction::Stop);
        }
        assert_eq!(close_action(4007), CloseAction::Identify);
        assert_eq!(close_action(4009), CloseAction::Identify);
        for c in [1000, 1001, 1006, 4000, 4003, 4008] {
            assert_eq!(close_action(c), CloseAction::Resume);
        }
    }

    #[test]
    fn member_frames_have_serein_shapes_and_order() {
        let parse = |f: &String| serde_json::from_str::<serde_json::Value>(f).unwrap();
        let sub = |g: &str, typing: bool, channels: serde_json::Value| {
            json!({"op": 37, "d": {"subscriptions": {g: {"typing": typing, "threads": false,
                "activities": true, "member_updates": false, "members": [], "channels": channels,
                "thread_member_lists": []}}}})
        };
        let frames: Vec<_> = member_frames(None, Some((10, 20)))
            .iter()
            .map(parse)
            .collect();
        assert_eq!(
            frames,
            [
                sub("10", true, json!({})),
                sub("10", true, json!({"20": [[0, 99]]}))
            ]
        );
        let frames: Vec<_> = member_frames(Some((10, 20)), Some((10, 21)))
            .iter()
            .map(parse)
            .collect();
        assert_eq!(frames, [sub("10", true, json!({"21": [[0, 99]]}))]);
        let frames: Vec<_> = member_frames(Some((10, 20)), Some((11, 30)))
            .iter()
            .map(parse)
            .collect();
        assert_eq!(
            frames,
            [
                sub("10", false, json!({})),
                sub("11", true, json!({})),
                sub("11", true, json!({"30": [[0, 99]]}))
            ]
        );
        let frames: Vec<_> = member_frames(Some((10, 20)), None)
            .iter()
            .map(parse)
            .collect();
        assert_eq!(frames, [sub("10", false, json!({}))]);
        assert!(member_frames(Some((10, 20)), Some((10, 20))).is_empty());
        assert!(member_frames(None, None).is_empty());
    }

    #[test]
    fn backoff_grows_and_caps() {
        assert_eq!(backoff(0), Duration::ZERO);
        assert!(
            backoff(1) >= Duration::from_millis(500) && backoff(1) < Duration::from_millis(1000)
        );
        assert!(
            backoff(20) >= Duration::from_secs(30) && backoff(20) < Duration::from_millis(30_500)
        );
    }
}
