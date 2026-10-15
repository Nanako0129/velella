//! Typed message endpoints on the session's `Rest` (plan M). Paths are built from `u64` ids only,
//! so nothing from a payload or the UI can shape the URL; `Rest` still validates it.
use crate::rest::{Rest, RestError};
use serde::Serialize;
use serde_json::Value;

/// The send body: `{content, nonce, tts:false, flags:0}`, plus `message_reference` for a reply.
/// No `allowed_mentions`, so Discord's default applies (the replied user is pinged).
#[derive(Serialize)]
struct CreateMessage<'a> {
    content: &'a str,
    nonce: &'a str,
    tts: bool,
    flags: u8,
    #[serde(skip_serializing_if = "Option::is_none")]
    message_reference: Option<MessageReference>,
}

/// Ids as decimal strings, as Discord's snowflakes travel in JSON.
#[derive(Serialize)]
struct MessageReference {
    message_id: String,
    channel_id: String,
    fail_if_not_exists: bool,
}

impl Rest {
    /// The latest 50 messages of a channel or DM, newest first as Discord returns them.
    pub(crate) async fn channel_messages(&self, channel_id: u64) -> Result<Vec<Value>, RestError> {
        self.get(&format!("/channels/{channel_id}/messages?limit=50"))
            .await
    }

    /// One plain-text message, a reply to `reply_to` in the same channel if given. POST is never
    /// retried (`Rest` rule).
    pub(crate) async fn create_message(
        &self,
        channel_id: u64,
        content: &str,
        nonce: &str,
        reply_to: Option<u64>,
    ) -> Result<Value, RestError> {
        let body = CreateMessage {
            content,
            nonce,
            tts: false,
            flags: 0,
            message_reference: reply_to.map(|message_id| MessageReference {
                message_id: message_id.to_string(),
                channel_id: channel_id.to_string(),
                fail_if_not_exists: false,
            }),
        };
        self.post(&format!("/channels/{channel_id}/messages"), &body)
            .await
    }
}
