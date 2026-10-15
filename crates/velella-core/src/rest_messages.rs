//! Typed message endpoints on the session's `Rest` (plan M). Paths are built from `u64` ids only,
//! so nothing from a payload or the UI can shape the URL; `Rest` still validates it.
use crate::rest::{Rest, RestError};
use serde::Serialize;
use serde_json::Value;

/// The send body, exactly `{content, nonce, tts:false, flags:0}`.
#[derive(Serialize)]
struct CreateMessage<'a> {
    content: &'a str,
    nonce: &'a str,
    tts: bool,
    flags: u8,
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

    /// One plain-text message. POST is never retried (`Rest` rule).
    pub(crate) async fn create_message(
        &self,
        channel_id: u64,
        content: &str,
        nonce: &str,
    ) -> Result<Value, RestError> {
        let body = CreateMessage {
            content,
            nonce,
            tts: false,
            flags: 0,
        };
        self.post(&format!("/channels/{channel_id}/messages"), &body)
            .await
    }
}
