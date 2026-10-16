//! Settings-proto type 2, `FrecencyUserSettings` (§13.7), on PB's bounded codec (`proto.rs`, the
//! one protobuf decoder in core, T-11), and its GET and PATCH (§13.8, encoding §17.2). Crate-
//! private: slice A (reaction frecency) and K (favourites) build on it.
//!
//! S-2: the PATCH takes a typed partial proto (fields 3, 5, 6, 13 only), never raw bytes, and
//! only type 2 can be written. Every PATCH is preceded by a GET of type 2 whose decode succeeded,
//! and the caller's merge runs on that fresh server copy. `required_data_version` is omitted. An
//! `out_of_date` answer or a 400 discards the change and reloads; nothing is retried. A decode
//! failure turns writes off for the session. Unknown fields are kept everywhere, including
//! inside `FrecencyItem`.
#![allow(dead_code)] // Callers: slices A and K.

use crate::{
    proto::{base64_decode, base64_encode, Field, Message, ProtoError, Value},
    rest::{ActionError, Call, Rest, Retry},
    transport::Method,
};
use serde::{Deserialize, Serialize};

/// Largest decoded proto accepted (the REST body cap is 4 MiB).
const MAX_PROTO: usize = 4 << 20;
/// The only settings type F2 reads or writes: `FRECENCY_AND_FAVORITES_SETTINGS` (§13.7).
/// Type 1 (`PRELOADED_USER_SETTINGS`) has no value here, so it can never be written.
const FRECENCY_TYPE: u8 = 2;

type Result<T> = std::result::Result<T, ProtoError>;

fn read_varint(buf: &[u8], pos: &mut usize) -> Result<u64> {
    let mut out = 0u64;
    for i in 0..10 {
        let b = *buf.get(*pos).ok_or(ProtoError::Truncated)?;
        *pos += 1;
        if i == 9 && b > 1 {
            return Err(ProtoError::BadVarint);
        }
        out |= u64::from(b & 0x7f) << (7 * i);
        if b & 0x80 == 0 {
            return Ok(out);
        }
    }
    Err(ProtoError::BadVarint)
}

fn write_varint(out: &mut Vec<u8>, mut v: u64) {
    while v >= 0x80 {
        out.push((v as u8 & 0x7f) | 0x80);
        v >>= 7;
    }
    out.push(v as u8);
}

/// `FrecencyItem {uint32 total_uses=1; repeated uint64 recent_uses=2; int32 frecency=3;
/// int32 score=4}`; any other field is kept in `unknown`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct FrecencyItem {
    pub(crate) total_uses: u32,
    pub(crate) recent_uses: Vec<u64>,
    pub(crate) frecency: i32,
    pub(crate) score: i32,
    pub(crate) unknown: Vec<Field>,
}

impl FrecencyItem {
    fn decode(m: &Message) -> Result<FrecencyItem> {
        let mut item = FrecencyItem::default();
        for f in &m.fields {
            match (f.number, &f.value) {
                (1, Value::Varint(v)) => item.total_uses = *v as u32,
                (2, Value::Varint(v)) => item.recent_uses.push(*v),
                (2, Value::Len(packed)) => {
                    let mut pos = 0;
                    while pos < packed.len() {
                        item.recent_uses.push(read_varint(packed, &mut pos)?);
                    }
                }
                (3, Value::Varint(v)) => item.frecency = *v as i32,
                (4, Value::Varint(v)) => item.score = *v as i32,
                (1..=4, _) => return Err(ProtoError::WrongType),
                _ => item.unknown.push(f.clone()),
            }
        }
        Ok(item)
    }

    /// proto3: zero values omitted, `recent_uses` packed.
    fn encode(&self) -> Message {
        let mut m = Message::new();
        if self.total_uses != 0 {
            m.push_varint(1, u64::from(self.total_uses));
        }
        if !self.recent_uses.is_empty() {
            let mut packed = Vec::new();
            for v in &self.recent_uses {
                write_varint(&mut packed, *v);
            }
            m.fields.push(Field {
                number: 2,
                value: Value::Len(packed),
            });
        }
        if self.frecency != 0 {
            m.push_varint(3, i64::from(self.frecency) as u64);
        }
        if self.score != 0 {
            m.push_varint(4, i64::from(self.score) as u64);
        }
        m.fields.extend(self.unknown.iter().cloned());
        m
    }
}

/// `EmojiFrecency {map<string, FrecencyItem> emojis = 1}`, entries in wire order.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct EmojiFrecency {
    pub(crate) emojis: Vec<(String, FrecencyItem)>,
    pub(crate) unknown: Vec<Field>,
}

impl EmojiFrecency {
    fn decode(m: &Message) -> Result<EmojiFrecency> {
        let mut out = EmojiFrecency::default();
        for f in &m.fields {
            if f.number != 1 {
                out.unknown.push(f.clone());
                continue;
            }
            let entry = m.nested(f)?;
            let key = entry.string(1)?.unwrap_or("").to_owned();
            let item = match entry.message(2)? {
                Some(item) => FrecencyItem::decode(&item)?,
                None => FrecencyItem::default(),
            };
            out.emojis.push((key, item));
        }
        Ok(out)
    }

    fn encode(&self) -> Message {
        let mut m = Message::new();
        for (key, item) in &self.emojis {
            let mut entry = Message::new();
            entry.push_string(1, key);
            entry.push_message(2, &item.encode());
            m.push_message(1, &entry);
        }
        m.fields.extend(self.unknown.iter().cloned());
        m
    }
}

/// `FavoriteEmojis {repeated string emojis = 1}` (at most 250, §13.7).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct FavoriteEmojis {
    pub(crate) emojis: Vec<String>,
    pub(crate) unknown: Vec<Field>,
}

impl FavoriteEmojis {
    fn decode(m: &Message) -> Result<FavoriteEmojis> {
        let mut out = FavoriteEmojis::default();
        for f in &m.fields {
            match (f.number, &f.value) {
                (1, Value::Len(b)) => out.emojis.push(
                    std::str::from_utf8(b)
                        .map_err(|_| ProtoError::Utf8)?
                        .to_owned(),
                ),
                (1, _) => return Err(ProtoError::WrongType),
                _ => out.unknown.push(f.clone()),
            }
        }
        Ok(out)
    }

    fn encode(&self) -> Message {
        let mut m = Message::new();
        for e in &self.emojis {
            m.push_string(1, e);
        }
        m.fields.extend(self.unknown.iter().cloned());
        m
    }
}

/// The fields F2 may write (3, 5, 6, 13); `None` is not written. Field 3 (`favorite_stickers`)
/// is kept as its decoded field tree: its inner layout is not in §13.7, so slice K types it.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct FrecencyPatch {
    pub(crate) favorite_stickers: Option<Message>,
    pub(crate) favorite_emojis: Option<FavoriteEmojis>,
    pub(crate) emoji_frecency: Option<EmojiFrecency>,
    pub(crate) emoji_reaction_frecency: Option<EmojiFrecency>,
}

impl FrecencyPatch {
    /// The partial proto: only the fields set, in field-number order.
    fn encode(&self) -> Vec<u8> {
        self.message().encode()
    }

    fn message(&self) -> Message {
        let mut m = Message::new();
        if let Some(s) = &self.favorite_stickers {
            m.push_message(3, s);
        }
        if let Some(f) = &self.favorite_emojis {
            m.push_message(5, &f.encode());
        }
        if let Some(f) = &self.emoji_frecency {
            m.push_message(6, &f.encode());
        }
        if let Some(f) = &self.emoji_reaction_frecency {
            m.push_message(13, &f.encode());
        }
        m
    }
}

/// A decoded type-2 proto: the writable fields typed, every other top-level field kept.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct FrecencySettings {
    pub(crate) fields: FrecencyPatch,
    /// Every other field (1, 2, 4, 7-12 and unknown ones), in wire order.
    pub(crate) other: Vec<Field>,
}

impl FrecencySettings {
    pub(crate) fn decode(bytes: &[u8]) -> Result<FrecencySettings> {
        let top = Message::decode(bytes, MAX_PROTO)?;
        let mut out = FrecencySettings::default();
        for f in &top.fields {
            let p = &mut out.fields;
            match f.number {
                3 => p.favorite_stickers = Some(top.nested(f)?),
                5 => p.favorite_emojis = Some(FavoriteEmojis::decode(&top.nested(f)?)?),
                6 => p.emoji_frecency = Some(EmojiFrecency::decode(&top.nested(f)?)?),
                13 => p.emoji_reaction_frecency = Some(EmojiFrecency::decode(&top.nested(f)?)?),
                _ => out.other.push(f.clone()),
            }
        }
        Ok(out)
    }

    /// Every field again, in field-number order (wire order within a number).
    pub(crate) fn encode(&self) -> Vec<u8> {
        let mut m = self.fields.message();
        m.fields.extend(self.other.iter().cloned());
        m.fields.sort_by_key(|f| f.number);
        m.encode()
    }

    /// Standard or url-safe base64, leniently (§17.2), then `decode`.
    pub(crate) fn from_base64(s: &str) -> Result<FrecencySettings> {
        FrecencySettings::decode(&base64_decode(s)?)
    }
}

/// What a PATCH did.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum PatchOutcome {
    /// Written; the server's proto from the answer.
    Applied(FrecencySettings),
    /// `out_of_date` or a 400: the change was discarded; the server's current proto.
    OutOfDate(FrecencySettings),
    /// The merge wrote nothing; the server's proto from the GET.
    Unchanged(FrecencySettings),
}

#[derive(Deserialize)]
struct SettingsBody {
    settings: Option<String>,
    #[serde(default)]
    out_of_date: bool,
}

#[derive(Serialize)]
struct PatchBody {
    settings: String,
}

fn path() -> String {
    format!("/users/@me/settings-proto/{FRECENCY_TYPE}")
}

impl Rest {
    /// Decodes a type-2 proto from the server; a failure turns writes off for the session.
    fn frecency_decode(
        &self,
        settings: &str,
    ) -> std::result::Result<FrecencySettings, ActionError> {
        FrecencySettings::from_base64(settings).map_err(|_| {
            self.disable_settings();
            ActionError::BadResponse
        })
    }

    /// `GET /users/@me/settings-proto/2` → `{settings}` (§13.8); never retried.
    pub(crate) async fn frecency_get(&self) -> std::result::Result<FrecencySettings, ActionError> {
        let body: SettingsBody = self
            .call(&Call::new(Method::Get, &path())?, Retry::None)
            .await?;
        let settings = body.settings.ok_or_else(|| {
            self.disable_settings();
            ActionError::BadResponse
        })?;
        self.frecency_decode(&settings)
    }

    /// GET type 2, let `merge` build the partial proto from that fresh copy (slice A's and K's
    /// merge rule), then `PATCH /users/@me/settings-proto/2` `{"settings": base64}`. Refused
    /// (nothing sent) once writes are off.
    pub(crate) async fn frecency_patch(
        &self,
        merge: impl FnOnce(&FrecencySettings) -> Option<FrecencyPatch>,
    ) -> std::result::Result<PatchOutcome, ActionError> {
        if self.settings_disabled() {
            return Err(ActionError::SettingsUnavailable);
        }
        let server = self.frecency_get().await?;
        let Some(patch) = merge(&server) else {
            return Ok(PatchOutcome::Unchanged(server));
        };
        let call = Call::new(Method::Patch, &path())?.json(&PatchBody {
            settings: base64_encode(&patch.encode()),
        })?;
        match self.call::<SettingsBody>(&call, Retry::None).await {
            Ok(SettingsBody {
                settings: Some(settings),
                out_of_date,
            }) => {
                let settings = self.frecency_decode(&settings)?;
                Ok(if out_of_date {
                    PatchOutcome::OutOfDate(settings)
                } else {
                    PatchOutcome::Applied(settings)
                })
            }
            Ok(SettingsBody {
                settings: None,
                out_of_date: true,
            }) => Ok(PatchOutcome::OutOfDate(self.frecency_get().await?)),
            Ok(_) => Err(ActionError::BadResponse),
            // INVALID_USER_SETTINGS_DATA: reload from the server (§13.9).
            Err(ActionError::Api { status: 400, .. }) => {
                Ok(PatchOutcome::OutOfDate(self.frecency_get().await?))
            }
            Err(e) => Err(e),
        }
    }
}
