//! Everything that fingerprints this client on the gateway, in one file. Shapes follow Concord's
//! user-account IDENTIFY (refs/concord/src/discord/gateway.rs, GPL-3.0-only; see
//! THIRD_PARTY_NOTICES.md), with Safari-on-macOS properties instead of Concord's Chrome ones.
use crate::token::{self, Token};
use base64::Engine as _;
use serde::Serialize;
use zeroize::Zeroizing;

/// Initial gateway URL: v9 because the READY shapes we parse (Concord's) are v9's, zlib-stream
/// because the browser client uses the stateful transport mode to keep READY small on the wire.
/// Crate-private so nothing on the FFI surface can change it.
pub(crate) const GATEWAY_URL: &str =
    "wss://gateway.discord.gg/?v=9&encoding=json&compress=zlib-stream";
/// Query an accepted `resume_gateway_url` is normalized to (same as `GATEWAY_URL`'s).
pub(crate) const GATEWAY_QUERY: &str = "v=9&encoding=json&compress=zlib-stream";

/// Bits Discord checks before delivering user-account payload shapes (sum 253). Without them
/// Discord treats the session as a bot. Concord's list:
/// 0 LAZY_USER_NOTES, 2 VERSIONED_READ_STATES, 3 VERSIONED_USER_GUILD_SETTINGS,
/// 4 DEDUPE_USER_OBJECTS (READY `users` + `recipient_ids`), 5 PRIORITIZED_READY_PAYLOAD,
/// 6 MULTIPLE_GUILD_EXPERIMENT_POPULATIONS, 7 NON_CHANNEL_READ_STATES.
/// 253 sets only bits 0-7, so the USER_SETTINGS_PROTO capability is off. Concord parses legacy
/// `user_settings` JSON (with `guild_folders`) from READY under exactly this value
/// (refs/concord/src/discord/gateway/parser/ready.rs:198-202); that Discord still sends it is
/// unverified until the first live login (S3).
pub(crate) const CAPABILITIES: u64 = 253;

/// The one user-agent constant. The login webview (S3, via FFI) sends the same string so the
/// webview and the gateway `browser_user_agent` agree, following Serein's consistency stance
/// (refs/serein/crates/client-core/src/fingerprint.rs:1-7). Whether Discord acts on a mismatch
/// is unverified.
pub const USER_AGENT: &str = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/26.0 Safari/605.1.15";

const OS: &str = "Mac OS X";
const BROWSER: &str = "Safari";
const BROWSER_VERSION: &str = "26.0";
/// Safari freezes the macOS version in its UA at 10.15.7.
const OS_VERSION: &str = "10.15.7";
pub(crate) const LOCALE: &str = "en-US";
/// Safari's Accept-Language for an en-US system, matching `LOCALE`.
pub(crate) const ACCEPT_LANGUAGE: &str = "en-US,en;q=0.9";
/// Concord's fallback build number (refs/concord/src/discord/fingerprint.rs); the live number
/// would need a REST call, which M1 forbids. Whether Discord accepts a stale value is
/// unverified; the first live login (S3) is the check.
const CLIENT_BUILD_NUMBER: u64 = 580_004;

#[derive(Serialize, Debug)]
struct Properties {
    os: &'static str,
    browser: &'static str,
    device: &'static str,
    system_locale: &'static str,
    browser_user_agent: &'static str,
    browser_version: &'static str,
    os_version: &'static str,
    referrer: &'static str,
    referring_domain: &'static str,
    referrer_current: &'static str,
    referring_domain_current: &'static str,
    release_channel: &'static str,
    client_build_number: u64,
    client_event_source: Option<()>,
}

/// Fresh-launch values as Concord sends them with an empty cache.
#[derive(Serialize, Debug)]
struct ClientState {
    guild_versions: std::collections::BTreeMap<String, u64>,
    highest_last_message_id: &'static str,
    read_state_version: i64,
    user_guild_settings_version: i64,
    user_settings_version: i64,
    private_channels_version: &'static str,
    api_code_version: i64,
}

#[derive(Serialize, Debug)]
struct Presence {
    since: u64,
    activities: [(); 0],
    status: &'static str,
    afk: bool,
}

#[derive(Serialize, Debug)]
pub(crate) struct IdentifyData<'a> {
    #[serde(serialize_with = "token::serialize")]
    token: &'a Token,
    capabilities: u64,
    properties: Properties,
    presence: Presence,
    /// Browsers keep payload compression off; transport compression is the zlib-stream URL.
    compress: bool,
    client_state: ClientState,
}

#[derive(Serialize, Debug)]
pub(crate) struct ResumeData<'a> {
    #[serde(serialize_with = "token::serialize")]
    token: &'a Token,
    session_id: &'a str,
    seq: u64,
}

#[derive(Serialize, Debug)]
pub(crate) struct Payload<D> {
    op: u8,
    d: D,
}

/// The one source of the client properties: IDENTIFY sends them as `properties`, REST as the
/// base64 `X-Super-Properties` header, so the two can never disagree.
fn properties() -> Properties {
    Properties {
        os: OS,
        browser: BROWSER,
        device: "",
        system_locale: LOCALE,
        browser_user_agent: USER_AGENT,
        browser_version: BROWSER_VERSION,
        os_version: OS_VERSION,
        referrer: "",
        referring_domain: "",
        referrer_current: "",
        referring_domain_current: "",
        release_channel: "stable",
        client_build_number: CLIENT_BUILD_NUMBER,
        client_event_source: None,
    }
}

/// `X-Super-Properties`: standard base64 of the IDENTIFY properties JSON, as the web client sends.
pub(crate) fn super_properties() -> String {
    let json = serde_json::to_string(&properties()).expect("properties serialize");
    base64::engine::general_purpose::STANDARD.encode(json)
}

/// `X-Discord-Timezone`: the system's IANA zone, which a browser reports from the same setting.
/// macOS keeps it as the `/etc/localtime` symlink into `.../zoneinfo/<Zone>`; anything unreadable
/// or outside the IANA name alphabet falls back to "UTC".
pub(crate) fn timezone() -> String {
    std::fs::read_link("/etc/localtime")
        .ok()
        .and_then(|p| Some(p.to_str()?.split_once("zoneinfo/")?.1.to_owned()))
        .filter(|z| {
            !z.is_empty()
                && z.bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b"/_+-".contains(&b))
        })
        .unwrap_or_else(|| "UTC".to_owned())
}

pub(crate) fn identify(token: &Token) -> Payload<IdentifyData<'_>> {
    Payload {
        op: 2,
        d: IdentifyData {
            token,
            capabilities: CAPABILITIES,
            properties: properties(),
            presence: Presence {
                since: 0,
                activities: [],
                status: "unknown",
                afk: false,
            },
            compress: false,
            client_state: ClientState {
                guild_versions: Default::default(),
                highest_last_message_id: "0",
                read_state_version: 0,
                user_guild_settings_version: -1,
                user_settings_version: -1,
                private_channels_version: "0",
                api_code_version: 0,
            },
        },
    }
}

pub(crate) fn resume<'a>(
    token: &'a Token,
    session_id: &'a str,
    seq: u64,
) -> Payload<ResumeData<'a>> {
    Payload {
        op: 6,
        d: ResumeData {
            token,
            session_id,
            seq,
        },
    }
}

/// Serialized payload held in a zeroizing buffer; the token is in it, so it never gets logged.
pub(crate) fn encode(payload: &impl Serialize) -> Zeroizing<String> {
    Zeroizing::new(serde_json::to_string(payload).expect("payload serializes"))
}

#[cfg(test)]
mod tests {
    use super::*;

    const MARKER: &str = "SYNTHETIC_TOKEN_MARKER_0123456789";

    #[test]
    fn payloads_carry_the_token_but_debug_redacts_it() {
        let token = Token::new(MARKER.into()).unwrap();
        let identify = identify(&token);
        let resume = resume(&token, "sess", 7);
        for debug in [
            format!("{identify:?}"),
            format!("{identify:#?}"),
            format!("{resume:?}"),
        ] {
            assert!(!debug.contains("SYNTHETIC"), "{debug}");
            assert!(debug.contains("[REDACTED]"));
        }
        let wire: serde_json::Value = serde_json::from_str(&encode(&identify)).unwrap();
        assert_eq!(wire["op"], 2);
        assert_eq!(wire["d"]["token"], MARKER);
        assert_eq!(wire["d"]["capabilities"], 253);
        assert_eq!(wire["d"]["compress"], false);
        assert_eq!(wire["d"]["properties"]["browser_user_agent"], USER_AGENT);
        let wire: serde_json::Value = serde_json::from_str(&encode(&resume)).unwrap();
        assert_eq!(wire["op"], 6);
        assert_eq!(wire["d"]["token"], MARKER);
        assert_eq!(wire["d"]["session_id"], "sess");
        assert_eq!(wire["d"]["seq"], 7);
    }

    #[test]
    fn super_properties_decode_to_the_identify_properties() {
        let token = Token::new(MARKER.into()).unwrap();
        let wire: serde_json::Value = serde_json::from_str(&encode(&identify(&token))).unwrap();
        let raw = base64::engine::general_purpose::STANDARD
            .decode(super_properties())
            .unwrap();
        assert!(!String::from_utf8_lossy(&raw).contains("SYNTHETIC"));
        let decoded: serde_json::Value = serde_json::from_slice(&raw).unwrap();
        assert_eq!(decoded, wire["d"]["properties"]);
        let tz = timezone();
        assert!(!tz.is_empty() && reqwest::header::HeaderValue::from_str(&tz).is_ok());
    }

    #[test]
    fn capability_bits_sum_to_253() {
        let bits = [0, 2, 3, 4, 5, 6, 7];
        assert_eq!(bits.iter().map(|b| 1u64 << b).sum::<u64>(), CAPABILITIES);
    }
}
