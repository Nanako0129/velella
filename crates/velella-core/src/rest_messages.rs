//! Typed message endpoints on the session's `Rest` (plan M, menu plan F2). Paths are built from
//! `u64` ids only, so nothing from a payload or the UI can shape the URL; `Rest` still validates
//! it.
//!
//! Sends, replies, forwards and forward comments are one `POST /channels/{c}/messages` whose
//! body follows §18.4 in key order: `mobile_network_type`, `content`, `nonce`, `tts`, then
//! `message_reference` and `allowed_mentions` when present, then `flags` (always). Each carries
//! `X-Context-Properties` (`chat_input`, or `forwarding`, §17.1) and is retried only when the
//! request provably never reached Discord (D11: connect failed, 522, 523), with the same bytes.
use crate::{
    rest::{ActionError, Call, Context, Rest, RestError, Retry},
    transport::Method,
};
use serde::Serialize;
use serde_json::Value;
use std::{
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

/// SUPPRESS_NOTIFICATIONS (`@silent`, §7.3, §18.4).
const SUPPRESS_NOTIFICATIONS: u64 = 1 << 12;
const DISCORD_EPOCH_MS: u64 = 1_420_070_400_000;

/// `{parse:["users","roles","everyone"], replied_user:false}`: a reply (or the edit of one) that
/// does not ping its author (§4.2, §18.4).
#[derive(Serialize)]
pub(crate) struct AllowedMentions {
    parse: [&'static str; 3],
    replied_user: bool,
}

impl AllowedMentions {
    pub(crate) fn reply_ping_off() -> AllowedMentions {
        AllowedMentions {
            parse: ["users", "roles", "everyone"],
            replied_user: false,
        }
    }
}

#[derive(Serialize)]
struct CreateMessage<'a> {
    mobile_network_type: &'static str,
    content: &'a str,
    nonce: &'a str,
    tts: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    message_reference: Option<MessageReference>,
    #[serde(skip_serializing_if = "Option::is_none")]
    allowed_mentions: Option<AllowedMentions>,
    flags: u64,
    /// After `flags` (§18.4 appends `attachments` last), only when there are attachments.
    #[serde(skip_serializing_if = "Option::is_none")]
    attachments: Option<Vec<AttachmentEntry<'a>>>,
}

/// One `attachments` entry of a message create, in OA A10's key order: `id` is the index as a
/// string, `description` whenever set (`""` included), `is_spoiler` only when true,
/// `original_content_type` only when known.
#[derive(Serialize)]
pub(crate) struct AttachmentEntry<'a> {
    pub(crate) id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) description: Option<&'a str>,
    pub(crate) filename: &'a str,
    pub(crate) uploaded_filename: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) is_spoiler: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) original_content_type: Option<&'a str>,
}

/// The upload-slot body `{"files":[…]}` (OA A1, A2): one file per request.
#[derive(Serialize)]
struct SlotRequest<'a> {
    files: [SlotFile<'a>; 1],
}

/// OA A2's order. `original_content_type` is always sent, `""` when unknown (CU-5).
#[derive(Serialize)]
struct SlotFile<'a> {
    filename: &'a str,
    file_size: u64,
    id: String,
    is_clip: bool,
    original_content_type: &'a str,
}

/// Ids as decimal strings, as Discord's snowflakes travel in JSON. A reply has no `type`; a
/// forward has `type: 1` (§7.1).
#[derive(Serialize)]
struct MessageReference {
    #[serde(skip_serializing_if = "Option::is_none")]
    guild_id: Option<String>,
    channel_id: String,
    message_id: String,
    #[serde(rename = "type", skip_serializing_if = "Option::is_none")]
    kind: Option<u8>,
}

/// The message a send replies to (same channel). `guild_id` is omitted in DMs.
pub(crate) struct Reply {
    pub(crate) guild_id: Option<u64>,
    pub(crate) message_id: u64,
    /// The reply's mention toggle: off sends `allowed_mentions` with `replied_user: false`.
    pub(crate) mention: bool,
}

/// A fresh nonce: a snowflake of the current millisecond, strictly increasing within the
/// process. Bit 21 is set so it never equals a nonce of the existing send's generator in
/// `messages.rs` (which counts up from bit 0 within a millisecond).
// ponytail: a second generator beside messages.rs's private one; slice B folds them into one
// when it moves the send caller here.
pub(crate) fn nonce() -> String {
    static LAST: AtomicU64 = AtomicU64::new(0);
    let now_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(DISCORD_EPOCH_MS, |d| {
            u64::try_from(d.as_millis()).unwrap_or(u64::MAX)
        });
    let candidate = (now_ms.saturating_sub(DISCORD_EPOCH_MS) << 22) | (1 << 21);
    let mut previous = LAST.load(Ordering::Relaxed);
    loop {
        let next = candidate.max(previous.saturating_add(1));
        match LAST.compare_exchange_weak(previous, next, Ordering::Relaxed, Ordering::Relaxed) {
            Ok(_) => return next.to_string(),
            Err(seen) => previous = seen,
        }
    }
}

impl Rest {
    /// 50 messages of a channel or DM, newest first as Discord returns them: the latest, or the
    /// ones just before message `before` (a `u64`, so the query cannot be shaped either).
    pub(crate) async fn channel_messages(
        &self,
        channel_id: u64,
        before: Option<u64>,
    ) -> Result<Vec<Value>, RestError> {
        let before = before.map_or_else(String::new, |id| format!("&before={id}"));
        self.get(&format!("/channels/{channel_id}/messages?limit=50{before}"))
            .await
    }

    /// The composer's send (`SessionHandle::send_message`): a reply pings its author (the
    /// composer has no mention toggle until slice B) and names `guild_id`, the channel's guild
    /// from state (`None` in DMs, so the key is omitted, §18.4).
    pub(crate) async fn create_message(
        &self,
        channel_id: u64,
        guild_id: Option<u64>,
        content: &str,
        nonce: &str,
        reply_to: Option<u64>,
    ) -> Result<Value, RestError> {
        let reply = reply_to.map(|message_id| Reply {
            guild_id,
            message_id,
            mention: true,
        });
        Ok(self.send_message(channel_id, content, nonce, reply).await?)
    }

    async fn post_message(
        &self,
        channel_id: u64,
        body: &CreateMessage<'_>,
        context: Context,
    ) -> Result<Value, ActionError> {
        let call = Call::new(Method::Post, &format!("/channels/{channel_id}/messages"))?
            .json(body)?
            .context(context);
        self.call(&call, Retry::Send).await
    }

    /// One chat-input message (§18.4), a reply when `reply` is given.
    pub(crate) async fn send_message(
        &self,
        channel_id: u64,
        content: &str,
        nonce: &str,
        reply: Option<Reply>,
    ) -> Result<Value, ActionError> {
        let allowed_mentions = reply
            .as_ref()
            .filter(|r| !r.mention)
            .map(|_| AllowedMentions::reply_ping_off());
        let body = CreateMessage {
            mobile_network_type: "unknown",
            content,
            nonce,
            tts: false,
            message_reference: reply.map(|r| MessageReference {
                guild_id: r.guild_id.map(|g| g.to_string()),
                channel_id: channel_id.to_string(),
                message_id: r.message_id.to_string(),
                kind: None,
            }),
            allowed_mentions,
            flags: 0,
            attachments: None,
        };
        self.post_message(channel_id, &body, Context::ChatInput)
            .await
    }

    /// A send with uploaded attachments (composer plan CU Message create): the chat-input body
    /// with `attachments` after `flags`, content possibly empty (OA A11), the same D11 retry rule.
    /// `instant` is Shift-drop's send (`instant_upload`, OA A35).
    pub(crate) async fn send_with_attachments(
        &self,
        channel_id: u64,
        content: &str,
        nonce: &str,
        reply: Option<Reply>,
        attachments: Vec<AttachmentEntry<'_>>,
        instant: bool,
    ) -> Result<Value, ActionError> {
        let body = CreateMessage {
            mobile_network_type: "unknown",
            content,
            nonce,
            tts: false,
            message_reference: reply.map(|r| MessageReference {
                guild_id: r.guild_id.map(|g| g.to_string()),
                channel_id: channel_id.to_string(),
                message_id: r.message_id.to_string(),
                kind: None,
            }),
            allowed_mentions: None,
            flags: 0,
            attachments: Some(attachments),
        };
        let context = if instant {
            Context::InstantUpload
        } else {
            Context::ChatInput
        };
        self.post_message(channel_id, &body, context).await
    }

    /// `POST /channels/{c}/attachments` for one file (OA A1-A3), retried per CU-10, with
    /// `X-Discord-Original-MD5` for a converted image (D16a). The parsed response is
    /// `attachments[0]`'s `upload_url` and `upload_filename`.
    pub(crate) async fn attachment_slot(
        &self,
        channel_id: u64,
        filename: &str,
        file_size: u64,
        id: u64,
        content_type: &str,
        original_md5: Option<String>,
    ) -> Result<crate::uploads::Slot, ActionError> {
        let body = SlotRequest {
            files: [SlotFile {
                filename,
                file_size,
                id: id.to_string(),
                is_clip: false,
                original_content_type: content_type,
            }],
        };
        let call = Call::new(Method::Post, &format!("/channels/{channel_id}/attachments"))?
            .json(&body)?
            .original_md5(original_md5);
        let value: Value = self.call(&call, Retry::Slot).await?;
        crate::uploads::Slot::parse(&value).ok_or(ActionError::BadResponse)
    }

    /// `DELETE /attachments/{uploaded_filename}` (OA A6) with the CU-8-encoded path segment
    /// (`uploads::delete_path`); never retried.
    pub(crate) async fn delete_upload(&self, path: &str) -> Result<(), ActionError> {
        let call = Call::new(Method::Delete, path)?;
        let _: Value = self.call(&call, Retry::None).await?;
        Ok(())
    }

    /// A forward (§7.1): `content:""` and `message_reference {guild_id (the origin's, omitted
    /// for a DM), channel_id, message_id, type:1}` to `destination_id`. Always the whole message
    /// (`forward_only` is never sent).
    pub(crate) async fn forward_message(
        &self,
        destination_id: u64,
        origin_guild_id: Option<u64>,
        origin_channel_id: u64,
        origin_message_id: u64,
        nonce: &str,
        silent: bool,
    ) -> Result<Value, ActionError> {
        let body = CreateMessage {
            mobile_network_type: "unknown",
            content: "",
            nonce,
            tts: false,
            message_reference: Some(MessageReference {
                guild_id: origin_guild_id.map(|g| g.to_string()),
                channel_id: origin_channel_id.to_string(),
                message_id: origin_message_id.to_string(),
                kind: Some(1),
            }),
            allowed_mentions: None,
            flags: if silent { SUPPRESS_NOTIFICATIONS } else { 0 },
            attachments: None,
        };
        self.post_message(destination_id, &body, Context::Forwarding)
            .await
    }

    /// A forward's comment (§7.3): a plain message to the same destination, `forwarding`.
    pub(crate) async fn forward_comment(
        &self,
        destination_id: u64,
        content: &str,
        nonce: &str,
        silent: bool,
    ) -> Result<Value, ActionError> {
        let body = CreateMessage {
            mobile_network_type: "unknown",
            content,
            nonce,
            tts: false,
            message_reference: None,
            allowed_mentions: None,
            flags: if silent { SUPPRESS_NOTIFICATIONS } else { 0 },
            attachments: None,
        };
        self.post_message(destination_id, &body, Context::Forwarding)
            .await
    }
}
