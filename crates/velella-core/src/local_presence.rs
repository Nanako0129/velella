//! The presence the official web client reports for the signed-in user (`{status, since,
//! activities, afk}`, op 3 and the normal IDENTIFY), as a pure function of the parsed settings,
//! the clock and the time of the user's last input (menu plan ID, I-3, I-16). Rules and evidence:
//! refs/discord-ground-truth/identify.md §7 (SelfPresenceStore `getLocalPresence`, IdleStore,
//! LocalActivityStore, the custom-status activity mapping). Nothing here sends anything; when a
//! value cannot be established the result is `None` and the caller sends no presence.
#![allow(dead_code)] // consumer: slice ID-b (IDENTIFY presence and op 3)
use crate::settings_proto::{CustomStatus, UserSettings};
use serde::Serialize;

/// Web IdleStore: idle once no input arrived for more than 10 minutes (`A.sdF` = 6e5 ms).
pub const IDLE_AFTER_MS: u64 = 600_000;

/// The status the user chose, after the official idle rule. Closed: any other value is unknown.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum SelfStatus {
    Online,
    Idle,
    Dnd,
    Invisible,
}

/// Field order is the official object's (`getLocalPresence`), so serde writes the same JSON.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct LocalPresence {
    pub status: SelfStatus,
    /// Epoch ms the user went idle, 0 while active.
    pub since: u64,
    pub activities: Vec<CustomStatusActivity>,
    pub afk: bool,
}

/// The custom status as the official client turns it into an activity (module 685396 `d`).
/// Absent keys are omitted, `emoji` is `null` when there is none, as `JSON.stringify` writes it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct CustomStatusActivity {
    pub name: &'static str,
    #[serde(rename = "type")]
    pub kind: u8,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub state: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timestamps: Option<Timestamps>,
    pub emoji: Option<ActivityEmoji>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<String>,
    pub metadata: Metadata,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Timestamps {
    pub end: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Metadata {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
}

/// `{id, name, animated}`: `id` is the snowflake string of a custom emoji, `null` for unicode.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct ActivityEmoji {
    pub id: Option<String>,
    pub name: String,
    pub animated: bool,
}

/// What the account's emoji data says about a custom emoji id (`getUsableCustomEmojiById`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EmojiLookup {
    Usable {
        name: String,
        animated: bool,
    },
    /// Known to the account but not usable by it (the official client then tries the name).
    NotUsable,
    /// The data to decide is missing.
    Unknown,
}

/// Activity type CUSTOM_STATUS.
const CUSTOM_STATUS: u8 = 4;

/// `{status, since, activities, afk}` for `now_ms`, with the user's last input at
/// `last_input_ms` (mouse click, scroll, key press, window focus or channel switch, as the web
/// IdleStore counts them). `None` when the chosen status is outside the closed set or the custom
/// status emoji cannot be resolved.
pub fn local_presence(
    settings: &UserSettings,
    now_ms: u64,
    last_input_ms: u64,
    custom_emoji: impl Fn(u64) -> EmojiLookup,
) -> Option<LocalPresence> {
    // An absent status setting reads as UNKNOWN, which the store turns into online.
    let chosen = match settings.status.as_deref() {
        None => SelfStatus::Online,
        Some("online") => SelfStatus::Online,
        Some("idle") => SelfStatus::Idle,
        Some("dnd") => SelfStatus::Dnd,
        Some("invisible") => SelfStatus::Invisible,
        Some(_) => return None,
    };
    // A timed status that ran out is set back to online (custom-status manager).
    let expires = settings.status_expires_at_ms;
    let chosen = if chosen != SelfStatus::Online && expires != 0 && expires <= now_ms {
        SelfStatus::Online
    } else {
        chosen
    };
    let quiet_ms = now_ms.saturating_sub(last_input_ms);
    let since = if quiet_ms > IDLE_AFTER_MS {
        last_input_ms
    } else {
        0
    };
    let afk_after = u64::from(settings.afk_timeout_s)
        .saturating_mul(1000)
        .min(IDLE_AFTER_MS);
    let afk = settings.afk_timeout_s == 0 || quiet_ms > afk_after;
    let status = if chosen == SelfStatus::Online && since > 0 {
        SelfStatus::Idle
    } else {
        chosen
    };
    let activities = match &settings.custom_status {
        Some(c) if status != SelfStatus::Invisible && unexpired(c, now_ms) => {
            vec![activity(c, &custom_emoji)?]
        }
        _ => Vec::new(),
    };
    Some(LocalPresence {
        status,
        since,
        activities,
        afk,
    })
}

fn unexpired(c: &CustomStatus, now_ms: u64) -> bool {
    c.expires_at_ms == 0 || c.expires_at_ms > now_ms
}

fn activity(
    c: &CustomStatus,
    custom_emoji: &impl Fn(u64) -> EmojiLookup,
) -> Option<CustomStatusActivity> {
    let by_name = || {
        // ponytail: the official client normalises the name through its emoji table
        // (`getByName(convertSurrogateToName(name))`). A surrogate is passed through as stored;
        // a plain name (no table here yet) leaves the presence unknown rather than guessed.
        match c.emoji_name.as_str() {
            "" => Some(None),
            n if n.is_ascii() => None,
            n => Some(Some(ActivityEmoji {
                id: None,
                name: n.to_owned(),
                animated: false,
            })),
        }
    };
    let emoji = if c.emoji_id != 0 {
        match custom_emoji(c.emoji_id) {
            EmojiLookup::Usable { name, animated } => Some(ActivityEmoji {
                id: Some(c.emoji_id.to_string()),
                name,
                animated,
            }),
            EmojiLookup::NotUsable => by_name()?,
            EmojiLookup::Unknown => return None,
        }
    } else {
        by_name()?
    };
    Some(CustomStatusActivity {
        name: "Custom Status",
        kind: CUSTOM_STATUS,
        state: (!c.text.is_empty()).then(|| c.text.clone()),
        timestamps: (c.expires_at_ms > 0).then_some(Timestamps {
            end: c.expires_at_ms,
        }),
        emoji,
        details: c.label.clone(),
        metadata: Metadata {
            label: c.label.clone(),
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::proto::Message;
    use crate::settings_proto::tests::{settings, status_field};

    const NOW: u64 = 1_800_000_000_000;

    fn parsed(fields: &[(u32, Message)]) -> UserSettings {
        UserSettings::from_base64(&settings(fields)).unwrap()
    }

    fn custom(text: &str, emoji_id: u64, emoji_name: &str, expires: u64) -> Message {
        let mut c = Message::new();
        c.push_string(1, text);
        if emoji_id != 0 {
            c.push_fixed64_unpacked(2, &[emoji_id]);
        }
        c.push_string(3, emoji_name);
        if expires != 0 {
            c.push_fixed64_unpacked(4, &[expires]);
        }
        c
    }

    fn no_emoji(_: u64) -> EmojiLookup {
        EmojiLookup::Unknown
    }

    fn active(s: &UserSettings) -> Option<LocalPresence> {
        local_presence(s, NOW, NOW - 1_000, no_emoji)
    }

    #[test]
    fn chosen_status_is_closed_and_absent_means_online() {
        let st = |v: &str| active(&parsed(&[(11, status_field(v, None))])).map(|p| p.status);
        assert_eq!(st("online"), Some(SelfStatus::Online));
        assert_eq!(st("idle"), Some(SelfStatus::Idle));
        assert_eq!(st("dnd"), Some(SelfStatus::Dnd));
        assert_eq!(st("invisible"), Some(SelfStatus::Invisible));
        assert_eq!(st("offline"), None);
        assert_eq!(st("streaming"), None);
        assert_eq!(st("garbage"), None);
        let p = active(&parsed(&[])).unwrap();
        assert_eq!(
            p,
            LocalPresence {
                status: SelfStatus::Online,
                since: 0,
                activities: vec![],
                afk: false
            }
        );
    }

    #[test]
    fn ten_quiet_minutes_make_online_idle_with_since_and_afk() {
        let s = parsed(&[(11, status_field("online", None))]);
        let last = NOW - IDLE_AFTER_MS - 1;
        let p = local_presence(&s, NOW, last, no_emoji).unwrap();
        assert_eq!((p.status, p.since, p.afk), (SelfStatus::Idle, last, true));
        // Exactly ten minutes is not yet idle; the 60 s AFK default has passed.
        let p = local_presence(&s, NOW, NOW - IDLE_AFTER_MS, no_emoji).unwrap();
        assert_eq!((p.status, p.since, p.afk), (SelfStatus::Online, 0, true));
        // DND stays DND while idle, but `since` and `afk` still follow the input clock.
        let s = parsed(&[(11, status_field("dnd", None))]);
        let p = local_presence(&s, NOW, last, no_emoji).unwrap();
        assert_eq!((p.status, p.since, p.afk), (SelfStatus::Dnd, last, true));
    }

    #[test]
    fn afk_follows_the_afk_timeout_setting() {
        let mut voice = Message::new();
        voice.push_wrapper_varint(6, 300);
        let s = parsed(&[(5, voice)]);
        assert!(
            !local_presence(&s, NOW, NOW - 299_000, no_emoji)
                .unwrap()
                .afk
        );
        assert!(
            local_presence(&s, NOW, NOW - 300_001, no_emoji)
                .unwrap()
                .afk
        );
        let mut zero = Message::new();
        zero.push_wrapper_varint(6, 0);
        assert!(active(&parsed(&[(5, zero)])).unwrap().afk);
    }

    #[test]
    fn an_expired_timed_status_reads_as_online() {
        let mut st = status_field("dnd", None);
        st.push_fixed64_unpacked(4, &[NOW - 1]);
        assert_eq!(
            active(&parsed(&[(11, st)])).unwrap().status,
            SelfStatus::Online
        );
        let mut st = status_field("dnd", None);
        st.push_fixed64_unpacked(4, &[NOW + 60_000]);
        assert_eq!(
            active(&parsed(&[(11, st)])).unwrap().status,
            SelfStatus::Dnd
        );
    }

    #[test]
    fn custom_status_becomes_the_official_activity() {
        let mut c = custom("brb", 0, "\u{1F600}", NOW + 5_000);
        c.push_wrapper_string(6, "lbl");
        let p = active(&parsed(&[(11, status_field("online", Some(c)))])).unwrap();
        let json = serde_json::to_string(&p).unwrap();
        assert_eq!(
            json,
            format!(
                r#"{{"status":"online","since":0,"activities":[{{"name":"Custom Status","type":4,"state":"brb","timestamps":{{"end":{end}}},"emoji":{{"id":null,"name":"{e}","animated":false}},"details":"lbl","metadata":{{"label":"lbl"}}}}],"afk":false}}"#,
                end = NOW + 5_000,
                e = '\u{1F600}'
            )
        );
        // No text, no emoji, no expiry: the activity still goes out, with `emoji: null`.
        let p = active(&parsed(&[(
            11,
            status_field("online", Some(custom("", 0, "", 0))),
        )]))
        .unwrap();
        assert_eq!(
            serde_json::to_value(&p.activities).unwrap(),
            serde_json::json!([{"name": "Custom Status", "type": 4, "emoji": null, "metadata": {}}])
        );
    }

    #[test]
    fn expired_or_invisible_custom_status_sends_no_activity() {
        let expired = custom("old", 0, "", NOW);
        let p = active(&parsed(&[(11, status_field("online", Some(expired)))])).unwrap();
        assert!(p.activities.is_empty());
        let hidden = custom("hi", 0, "", 0);
        let p = active(&parsed(&[(11, status_field("invisible", Some(hidden)))])).unwrap();
        assert_eq!(p.status, SelfStatus::Invisible);
        assert!(p.activities.is_empty());
    }

    #[test]
    fn custom_emoji_needs_the_account_emoji_data() {
        let s = parsed(&[(11, status_field("online", Some(custom("x", 42, "blob", 0))))]);
        let usable = |_| EmojiLookup::Usable {
            name: "blob".into(),
            animated: true,
        };
        let p = local_presence(&s, NOW, NOW, usable).unwrap();
        assert_eq!(
            p.activities[0].emoji,
            Some(ActivityEmoji {
                id: Some("42".into()),
                name: "blob".into(),
                animated: true
            })
        );
        // Missing data, or a plain name the client would look up in its emoji table: unknown.
        assert_eq!(local_presence(&s, NOW, NOW, no_emoji), None);
        assert_eq!(
            local_presence(&s, NOW, NOW, |_| EmojiLookup::NotUsable),
            None
        );
        // Not usable with a surrogate name: the unicode emoji, as the official fallback does.
        let s = parsed(&[(
            11,
            status_field("online", Some(custom("x", 42, "\u{2764}", 0))),
        )]);
        let p = local_presence(&s, NOW, NOW, |_| EmojiLookup::NotUsable).unwrap();
        assert_eq!(p.activities[0].emoji.as_ref().unwrap().id, None);
    }
}
