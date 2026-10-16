//! Settings-proto type 1 (`PreloadedUserSettings`): READY `user_settings_proto` and
//! `USER_SETTINGS_PROTO_UPDATE` (menu plan ID, T-11, I-4, I-14), decoded on PB's bounded codec.
//! Field numbers and defaults: refs/discord-ground-truth/identify.md §3 and §7. Only the fields
//! velella reads are typed; the whole message is kept so a partial update can replace top-level
//! fields whole, as the official `mergePartial` does (W:8159507).
use crate::proto::{base64_decode, Message, ProtoError};
use crate::state::GuildFolder;
use std::collections::HashSet;

/// Decoded-size cap (contract: 1 MiB).
pub(crate) const MAX_PROTO_BYTES: usize = 1 << 20;
/// Base64 of `MAX_PROTO_BYTES`, with room for padding; a longer string is refused undecoded.
const MAX_PROTO_BASE64: usize = MAX_PROTO_BYTES.div_ceil(3) * 4 + 4;

/// `StatusSettings.custom_status` (W:11011776).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CustomStatus {
    pub text: String,
    /// 0 = no custom emoji.
    pub emoji_id: u64,
    pub emoji_name: String,
    /// Epoch ms; 0 = never expires.
    pub expires_at_ms: u64,
    pub label: Option<String>,
}

/// The parsed view of one decoded `PreloadedUserSettings`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UserSettings {
    raw: Message,
    /// `guild_folders.folders` (14.1), empty folders dropped as the legacy parse does.
    pub guild_folders: Vec<GuildFolder>,
    /// `guild_folders.guild_positions` (14.2): the order used only while `guild_folders` is empty.
    pub guild_positions: Vec<u64>,
    /// `appearance.developer_mode` (13.2), absent = false.
    pub developer_mode: bool,
    /// `text_and_images.render_reactions` (6.13 BoolValue), absent = true.
    pub render_reactions: bool,
    /// `status.status` (11.1 StringValue) as sent; `None` when absent.
    pub status: Option<String>,
    /// `status.status_expires_at_ms` (11.4 fixed64), 0 = never.
    pub status_expires_at_ms: u64,
    /// `status.custom_status` (11.2).
    pub custom_status: Option<CustomStatus>,
    /// `voice_and_video.afk_timeout` (5.6 UInt32Value) in seconds, absent = 60.
    pub afk_timeout_s: u32,
}

/// Standard or url-safe base64, then one bounded protobuf level.
pub(crate) fn decode(b64: &str) -> Result<Message, ProtoError> {
    if b64.len() > MAX_PROTO_BASE64 {
        return Err(ProtoError::TooLarge);
    }
    Message::decode(&base64_decode(b64)?, MAX_PROTO_BYTES)
}

/// The last fixed64 of field `n` (a singular fixed64), 0 when absent.
fn fixed64(m: &Message, n: u32) -> Result<u64, ProtoError> {
    Ok(m.fixed64_repeated(n)?.last().copied().unwrap_or(0))
}

fn folder(m: &Message) -> Result<GuildFolder, ProtoError> {
    Ok(GuildFolder {
        guild_ids: m.fixed64_repeated(1)?,
        // Int64Value: a present wrapper is a folder even when its value is 0 (I§3). The varint is
        // the id's two's-complement bit pattern, kept as is.
        id: m.wrapper_varint(2)?,
        name: m.wrapper_string(3)?.filter(|n| !n.is_empty()),
        color: m.wrapper_varint(4)?.and_then(|c| u32::try_from(c).ok()),
    })
}

fn custom_status(m: &Message) -> Result<CustomStatus, ProtoError> {
    Ok(CustomStatus {
        text: m.string(1)?.unwrap_or_default().to_owned(),
        emoji_id: fixed64(m, 2)?,
        emoji_name: m.string(3)?.unwrap_or_default().to_owned(),
        expires_at_ms: fixed64(m, 4)?,
        label: m.wrapper_string(6)?,
    })
}

impl UserSettings {
    pub(crate) fn from_base64(b64: &str) -> Result<Self, ProtoError> {
        Self::from_message(decode(b64)?)
    }

    /// Any unreadable field velella reads fails the whole parse: the caller keeps what it had.
    pub(crate) fn from_message(raw: Message) -> Result<Self, ProtoError> {
        let (guild_folders, guild_positions) = match raw.message(14)? {
            Some(f) => {
                let mut folders = f
                    .messages(1)?
                    .iter()
                    .map(folder)
                    .collect::<Result<Vec<_>, _>>()?;
                folders.retain(|f| !f.guild_ids.is_empty());
                (folders, f.fixed64_repeated(2)?)
            }
            None => (Vec::new(), Vec::new()),
        };
        let developer_mode = match raw.message(13)? {
            Some(a) => a.bool(2)?.unwrap_or(false),
            None => false,
        };
        let render_reactions = match raw.message(6)? {
            Some(t) => t.wrapper_bool(13)?.unwrap_or(true),
            None => true,
        };
        let afk_timeout_s = match raw.message(5)? {
            Some(v) => v
                .wrapper_varint(6)?
                .map_or(60, |s| u32::try_from(s).unwrap_or(u32::MAX)),
            None => 60,
        };
        let (status, status_expires_at_ms, custom) = match raw.message(11)? {
            Some(s) => (
                s.wrapper_string(1)?,
                fixed64(&s, 4)?,
                s.message(2)?.as_ref().map(custom_status).transpose()?,
            ),
            None => (None, 0, None),
        };
        Ok(Self {
            raw,
            guild_folders,
            guild_positions,
            developer_mode,
            render_reactions,
            status,
            status_expires_at_ms,
            custom_status: custom,
            afk_timeout_s,
        })
    }

    /// `partial: true`: every top-level field present in `update` replaces the stored one whole;
    /// absent ones are kept (identify.md §3, `USER_SETTINGS_PROTO_UPDATE`).
    pub(crate) fn merged(&self, update: Message) -> Result<Self, ProtoError> {
        let replaced: HashSet<u32> = update.fields.iter().map(|f| f.number).collect();
        let mut raw = self.raw.clone();
        raw.fields.retain(|f| !replaced.contains(&f.number));
        raw.fields.extend(update.fields);
        Self::from_message(raw)
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::proto::base64_encode;

    pub(crate) fn wrapper_string(s: &str) -> Message {
        let mut w = Message::new();
        w.push_string(1, s);
        w
    }

    /// A folder message: `id` as an Int64Value wrapper when given.
    pub(crate) fn folder_msg(id: Option<u64>, name: Option<&str>, guilds: &[u64]) -> Message {
        let mut f = Message::new();
        f.push_fixed64_packed(1, guilds);
        if let Some(id) = id {
            f.push_wrapper_varint(2, id);
        }
        if let Some(n) = name {
            f.push_wrapper_string(3, n);
        }
        f
    }

    pub(crate) fn folders_field(folders: &[Message], positions: &[u64]) -> Message {
        let mut g = Message::new();
        for f in folders {
            g.push_message(1, f);
        }
        g.push_fixed64_packed(2, positions);
        g
    }

    pub(crate) fn status_field(status: &str, custom: Option<Message>) -> Message {
        let mut s = Message::new();
        s.push_message(1, &wrapper_string(status));
        if let Some(c) = custom {
            s.push_message(2, &c);
        }
        s
    }

    pub(crate) fn settings(fields: &[(u32, Message)]) -> String {
        let mut m = Message::new();
        for (n, f) in fields {
            m.push_message(*n, f);
        }
        base64_encode(&m.encode())
    }

    #[test]
    fn decodes_folders_positions_flags_and_status() {
        let mut appearance = Message::new();
        appearance.push_bool(2, true);
        let mut text = Message::new();
        text.push_wrapper_bool(13, false);
        let mut voice = Message::new();
        voice.push_wrapper_varint(6, 300);
        let mut custom = Message::new();
        custom.push_string(1, "away");
        custom.push_fixed64_unpacked(2, &[77]);
        custom.push_string(3, "blob");
        custom.push_fixed64_unpacked(4, &[5_000]);
        custom.push_wrapper_string(6, "lbl");
        let mut status = status_field("dnd", Some(custom));
        status.push_fixed64_unpacked(4, &[9_000]);
        let b64 = settings(&[
            (
                14,
                folders_field(
                    &[
                        folder_msg(Some(0), Some("zero"), &[10, 20]),
                        folder_msg(None, None, &[30]),
                        folder_msg(Some(5), Some(""), &[]),
                    ],
                    &[30, 10],
                ),
            ),
            (13, appearance),
            (6, text),
            (5, voice),
            (11, status),
        ]);
        let s = UserSettings::from_base64(&b64).unwrap();
        assert_eq!(
            s.guild_folders,
            vec![
                GuildFolder {
                    id: Some(0),
                    name: Some("zero".into()),
                    color: None,
                    guild_ids: vec![10, 20]
                },
                GuildFolder {
                    id: None,
                    name: None,
                    color: None,
                    guild_ids: vec![30]
                },
            ]
        );
        assert_eq!(s.guild_positions, vec![30, 10]);
        assert!(s.developer_mode && !s.render_reactions);
        assert_eq!(s.afk_timeout_s, 300);
        assert_eq!(
            (s.status.as_deref(), s.status_expires_at_ms),
            (Some("dnd"), 9_000)
        );
        assert_eq!(
            s.custom_status,
            Some(CustomStatus {
                text: "away".into(),
                emoji_id: 77,
                emoji_name: "blob".into(),
                expires_at_ms: 5_000,
                label: Some("lbl".into()),
            })
        );
    }

    #[test]
    fn an_empty_proto_gives_the_official_defaults() {
        let s = UserSettings::from_base64("").unwrap();
        assert!(s.guild_folders.is_empty() && s.guild_positions.is_empty());
        assert!(!s.developer_mode && s.render_reactions);
        assert_eq!(
            (s.status, s.custom_status, s.afk_timeout_s),
            (None, None, 60)
        );
    }

    #[test]
    fn broken_or_oversized_input_is_an_error() {
        assert!(UserSettings::from_base64("not base64!").is_err());
        // Field 14 is a varint, not a message.
        let mut m = Message::new();
        m.push_varint(14, 1);
        assert!(UserSettings::from_base64(&base64_encode(&m.encode())).is_err());
        // Truncated.
        assert!(UserSettings::from_base64(&base64_encode(&[0x72, 0x05, 0x01])).is_err());
        let big = "A".repeat(MAX_PROTO_BASE64 + 4);
        assert_eq!(decode(&big), Err(ProtoError::TooLarge));
    }

    #[test]
    fn partial_merge_replaces_present_top_level_fields_whole() {
        let base = UserSettings::from_base64(&settings(&[
            (
                14,
                folders_field(&[folder_msg(Some(1), Some("a"), &[10, 20])], &[]),
            ),
            (11, status_field("idle", None)),
        ]))
        .unwrap();
        let update = decode(&settings(&[(11, status_field("dnd", None))])).unwrap();
        let next = base.merged(update).unwrap();
        assert_eq!(next.status.as_deref(), Some("dnd"));
        assert_eq!(next.guild_folders, base.guild_folders);
    }
}
