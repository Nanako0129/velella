//! The message-menu REST actions across FFI (menu plan F2). Inputs are closed types (S-13): ids
//! as `u64`, enums for location, reaction type, saved-message source and ack flags, `due_at` as
//! integer milliseconds. Thread preconditions and guild ids are computed in core from current
//! state, never passed from Swift. Each call returns success or a typed `ActionError`; the token,
//! MFA tickets and MFA tokens stay in core.
use crate::{runtime, SessionHandle};
use std::fmt;
use std::future::Future;
use velella_core as core;

#[derive(Clone, uniffi::Enum)]
pub enum ReactionEmoji {
    /// `name` is `[A-Za-z0-9_]{1,32}`; anything else is refused in core.
    Custom { name: String, id: u64 },
    /// The emoji's own characters (a skin-tone variant is its full sequence).
    Unicode { value: String },
}

#[derive(Clone, Copy, uniffi::Enum)]
pub enum ReactionKind {
    Normal,
    /// A super reaction.
    Burst,
}

/// Where a reaction was added or removed (§1.4).
#[derive(Clone, Copy, uniffi::Enum)]
pub enum ReactionLocation {
    Message,
    ForumToolbar,
    MobileMediaViewer,
    MessageHoverBar,
    MessageInlineButton,
    MessageContextMenu,
    MessageReactionPicker,
    MessageShortcut,
    DoubleTap,
    InAppNotification,
}

#[derive(Clone, Copy, uniffi::Enum)]
pub enum SaveSource {
    ReactionButton,
    ContextMenu,
    LongPressActionSheet,
    MessageToolbar,
    ForLaterList,
    NotificationsInbox,
}

#[derive(Clone, Copy, uniffi::Enum)]
pub enum AckFlags {
    Guild,
    Thread,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, uniffi::Enum)]
pub enum MfaKind {
    Webauthn,
    Totp,
    Sms,
    Backup,
    Password,
}

#[derive(Clone, uniffi::Record)]
pub struct MfaMethod {
    pub kind: MfaKind,
    /// WebAuthn's challenge.
    pub challenge: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, uniffi::Enum)]
pub enum ApiErrorCode {
    UnknownMessage,
    TooManyPins,
    TooManyReactions,
    TooManySavedMessages,
    InvalidAccess,
    InvalidPinChannel,
    SystemMessage,
    InvalidFormBody,
    ArchivedThread,
    MfaInvalidCode,
    ReactionBlocked,
    Other,
}

/// Typed failures. The Rust `Debug` and `Display` below are fixed text (no server message, no
/// challenge). They do not reach Swift: UniFFI's generated Swift `errorDescription` is
/// `String(reflecting: self)`, which includes the associated values (`message`, the WebAuthn
/// `challenge`), so Swift callers must never log an `ActionError`'s description.
#[derive(uniffi::Error)]
pub enum ActionError {
    Unauthorized,
    /// Answer with `mfa_submit(challenge_id, …)`; the original request is then re-sent.
    MfaRequired {
        challenge_id: u64,
        methods: Vec<MfaMethod>,
    },
    RateLimited,
    Captcha,
    /// `message`: the server's, at most 512 characters, saved-message actions only; display it,
    /// never log it.
    Api {
        status: u16,
        code: ApiErrorCode,
        message: Option<String>,
    },
    Network,
    TooLarge,
    BadResponse,
    InvalidUrl,
    InvalidInput,
    Busy,
    SettingsUnavailable,
}

impl fmt::Debug for ActionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Api { status, code, .. } => write!(f, "Api({status}, {code:?})"),
            Self::MfaRequired { challenge_id, .. } => write!(f, "MfaRequired({challenge_id})"),
            other => fmt::Display::fmt(other, f),
        }
    }
}

impl fmt::Display for ActionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Unauthorized => "Discord did not accept this sign-in",
            Self::MfaRequired { .. } => "Discord asked for multi-factor authentication",
            Self::RateLimited => "rate limited",
            Self::Captcha => "Discord asked for a captcha",
            Self::Api { .. } => "request failed",
            Self::Network => "network error",
            Self::TooLarge => "response too large",
            Self::BadResponse => "unreadable response",
            Self::InvalidUrl => "invalid request",
            Self::InvalidInput => "invalid input",
            Self::Busy => "already in progress",
            Self::SettingsUnavailable => "settings unavailable",
        })
    }
}

impl std::error::Error for ActionError {}

impl From<core::ActionError> for ActionError {
    fn from(e: core::ActionError) -> Self {
        use core::ActionError as C;
        match e {
            C::Unauthorized => Self::Unauthorized,
            C::MfaRequired(c) => Self::MfaRequired {
                challenge_id: c.id,
                methods: c
                    .methods
                    .into_iter()
                    .map(|m| MfaMethod {
                        kind: match m.kind {
                            core::MfaKind::Webauthn => MfaKind::Webauthn,
                            core::MfaKind::Totp => MfaKind::Totp,
                            core::MfaKind::Sms => MfaKind::Sms,
                            core::MfaKind::Backup => MfaKind::Backup,
                            core::MfaKind::Password => MfaKind::Password,
                        },
                        challenge: m.challenge,
                    })
                    .collect(),
            },
            C::RateLimited => Self::RateLimited,
            C::Captcha => Self::Captcha,
            C::Api {
                status,
                code,
                message,
            } => Self::Api {
                status,
                code: match code {
                    core::ApiCode::UnknownMessage => ApiErrorCode::UnknownMessage,
                    core::ApiCode::TooManyPins => ApiErrorCode::TooManyPins,
                    core::ApiCode::TooManyReactions => ApiErrorCode::TooManyReactions,
                    core::ApiCode::TooManySavedMessages => ApiErrorCode::TooManySavedMessages,
                    core::ApiCode::InvalidAccess => ApiErrorCode::InvalidAccess,
                    core::ApiCode::InvalidPinChannel => ApiErrorCode::InvalidPinChannel,
                    core::ApiCode::SystemMessage => ApiErrorCode::SystemMessage,
                    core::ApiCode::InvalidFormBody => ApiErrorCode::InvalidFormBody,
                    core::ApiCode::ArchivedThread => ApiErrorCode::ArchivedThread,
                    core::ApiCode::MfaInvalidCode => ApiErrorCode::MfaInvalidCode,
                    core::ApiCode::ReactionBlocked => ApiErrorCode::ReactionBlocked,
                    // 40005 is read only by the upload slot (composer plan CU), not by F2's actions.
                    core::ApiCode::EntityTooLarge | core::ApiCode::Other => ApiErrorCode::Other,
                },
                message,
            },
            C::Network => Self::Network,
            C::TooLarge => Self::TooLarge,
            C::BadResponse => Self::BadResponse,
            C::InvalidUrl => Self::InvalidUrl,
            C::InvalidInput => Self::InvalidInput,
            C::Busy => Self::Busy,
            C::SettingsUnavailable => Self::SettingsUnavailable,
        }
    }
}

#[derive(uniffi::Record)]
pub struct Reactor {
    pub id: u64,
    pub username: String,
    pub global_name: Option<String>,
    /// Avatar hash.
    pub avatar: Option<String>,
}

#[derive(uniffi::Record)]
pub struct PinnedMessage {
    pub message_id: u64,
    /// Pass the last item's value as `before` for the next page.
    pub pinned_at: String,
}

#[derive(uniffi::Record)]
pub struct PinsPage {
    pub items: Vec<PinnedMessage>,
    pub has_more: bool,
}

#[derive(uniffi::Record)]
pub struct SavedMessage {
    pub channel_id: u64,
    pub message_id: u64,
    pub saved_at: Option<String>,
    pub due_at: Option<String>,
}

impl From<core::SavedMessage> for SavedMessage {
    fn from(s: core::SavedMessage) -> Self {
        Self {
            channel_id: s.channel_id,
            message_id: s.message_id,
            saved_at: s.saved_at,
            due_at: s.due_at,
        }
    }
}

fn emoji(e: ReactionEmoji) -> core::Emoji {
    match e {
        ReactionEmoji::Custom { name, id } => core::Emoji::Custom { name, id },
        ReactionEmoji::Unicode { value } => core::Emoji::Unicode(value),
    }
}

fn kind(k: ReactionKind) -> core::ReactionType {
    match k {
        ReactionKind::Normal => core::ReactionType::Normal,
        ReactionKind::Burst => core::ReactionType::Burst,
    }
}

fn location(l: ReactionLocation) -> core::ReactionLocation {
    use core::ReactionLocation as C;
    match l {
        ReactionLocation::Message => C::Message,
        ReactionLocation::ForumToolbar => C::ForumToolbar,
        ReactionLocation::MobileMediaViewer => C::MobileMediaViewer,
        ReactionLocation::MessageHoverBar => C::MessageHoverBar,
        ReactionLocation::MessageInlineButton => C::MessageInlineButton,
        ReactionLocation::MessageContextMenu => C::MessageContextMenu,
        ReactionLocation::MessageReactionPicker => C::MessageReactionPicker,
        ReactionLocation::MessageShortcut => C::MessageShortcut,
        ReactionLocation::DoubleTap => C::DoubleTap,
        ReactionLocation::InAppNotification => C::InAppNotification,
    }
}

/// Runs an action on the owned runtime (the body never runs on the caller's thread).
async fn run<T: Send + 'static>(
    action: impl Future<Output = Result<T, core::ActionError>> + Send + 'static,
) -> Result<T, ActionError> {
    runtime()
        .spawn(action)
        .await
        .map_err(|_| ActionError::Network)?
        .map_err(Into::into)
}

#[uniffi::export]
impl SessionHandle {
    pub async fn add_reaction(
        &self,
        channel_id: u64,
        message_id: u64,
        emoji: ReactionEmoji,
        kind: ReactionKind,
        location: ReactionLocation,
    ) -> Result<(), ActionError> {
        let a = self.0.actions();
        let (e, k, l) = (
            self::emoji(emoji),
            self::kind(kind),
            self::location(location),
        );
        run(async move { a.add_reaction(channel_id, message_id, e, k, l).await }).await
    }

    /// `user_id` nil removes the signed-in user's own reaction.
    pub async fn remove_reaction(
        &self,
        channel_id: u64,
        message_id: u64,
        emoji: ReactionEmoji,
        kind: ReactionKind,
        user_id: Option<u64>,
        location: ReactionLocation,
    ) -> Result<(), ActionError> {
        let a = self.0.actions();
        let (e, k, l) = (
            self::emoji(emoji),
            self::kind(kind),
            self::location(location),
        );
        run(async move {
            a.remove_reaction(channel_id, message_id, e, k, user_id, l)
                .await
        })
        .await
    }

    /// One page of reactors (`limit` 1-100, `after` the last reactor's id).
    pub async fn list_reactors(
        &self,
        channel_id: u64,
        message_id: u64,
        emoji: ReactionEmoji,
        kind: ReactionKind,
        limit: u8,
        after: Option<u64>,
    ) -> Result<Vec<Reactor>, ActionError> {
        let a = self.0.actions();
        let (e, k) = (self::emoji(emoji), self::kind(kind));
        let users = run(async move {
            a.list_reactors(channel_id, message_id, e, k, limit, after)
                .await
        })
        .await?;
        Ok(users
            .into_iter()
            .map(|u| Reactor {
                id: u.id,
                username: u.username,
                global_name: u.global_name,
                avatar: u.avatar,
            })
            .collect())
    }

    pub async fn remove_emoji_reactions(
        &self,
        channel_id: u64,
        message_id: u64,
        emoji: ReactionEmoji,
    ) -> Result<(), ActionError> {
        let a = self.0.actions();
        let e = self::emoji(emoji);
        run(async move { a.remove_emoji_reactions(channel_id, message_id, e).await }).await
    }

    pub async fn remove_all_reactions(
        &self,
        channel_id: u64,
        message_id: u64,
    ) -> Result<(), ActionError> {
        let a = self.0.actions();
        run(async move { a.remove_all_reactions(channel_id, message_id).await }).await
    }

    /// `keep_reply_ping_off`: the message is a reply whose author is not in its mentions.
    pub async fn edit_message(
        &self,
        channel_id: u64,
        message_id: u64,
        content: String,
        keep_reply_ping_off: bool,
    ) -> Result<(), ActionError> {
        let a = self.0.actions();
        run(async move {
            a.edit_message(channel_id, message_id, content, keep_reply_ping_off)
                .await
        })
        .await
    }

    pub async fn delete_message(
        &self,
        channel_id: u64,
        message_id: u64,
    ) -> Result<(), ActionError> {
        let a = self.0.actions();
        run(async move { a.delete_message(channel_id, message_id).await }).await
    }

    pub async fn pin_message(&self, channel_id: u64, message_id: u64) -> Result<(), ActionError> {
        let a = self.0.actions();
        run(async move { a.pin_message(channel_id, message_id).await }).await
    }

    pub async fn unpin_message(&self, channel_id: u64, message_id: u64) -> Result<(), ActionError> {
        let a = self.0.actions();
        run(async move { a.unpin_message(channel_id, message_id).await }).await
    }

    /// 25 pins; `before` is the last item's `pinned_at` for the next page.
    pub async fn list_pins(
        &self,
        channel_id: u64,
        before: Option<String>,
    ) -> Result<PinsPage, ActionError> {
        let a = self.0.actions();
        let page = run(async move { a.list_pins(channel_id, before).await }).await?;
        Ok(PinsPage {
            items: page
                .items
                .into_iter()
                .map(|p| PinnedMessage {
                    message_id: p.message_id,
                    pinned_at: p.pinned_at,
                })
                .collect(),
            has_more: page.has_more,
        })
    }

    pub async fn ack_pins(&self, channel_id: u64) -> Result<(), ActionError> {
        let a = self.0.actions();
        run(async move { a.ack_pins(channel_id).await }).await
    }

    /// The automatic read ack; returns the token to pass to the next one (core keeps none).
    pub async fn ack_message(
        &self,
        channel_id: u64,
        message_id: u64,
        token: Option<String>,
        last_viewed: Option<u32>,
        flags: Option<AckFlags>,
    ) -> Result<Option<String>, ActionError> {
        let a = self.0.actions();
        let flags = flags.map(|f| match f {
            AckFlags::Guild => core::AckFlags::Guild,
            AckFlags::Thread => core::AckFlags::Thread,
        });
        run(async move {
            a.ack_message(channel_id, message_id, token, last_viewed, flags)
                .await
        })
        .await
    }

    /// Mark Unread: acks `ack_message_id` (the message before the chosen one) manually.
    pub async fn mark_unread(
        &self,
        channel_id: u64,
        ack_message_id: u64,
        mention_count: u32,
    ) -> Result<(), ActionError> {
        let a = self.0.actions();
        run(async move {
            a.mark_unread(channel_id, ack_message_id, mention_count)
                .await
        })
        .await
    }

    /// Bookmark (`due_at_ms` nil) or reminder (UTC milliseconds).
    pub async fn save_message(
        &self,
        channel_id: u64,
        message_id: u64,
        due_at_ms: Option<i64>,
        source: SaveSource,
    ) -> Result<SavedMessage, ActionError> {
        let a = self.0.actions();
        let source = match source {
            SaveSource::ReactionButton => core::SaveSource::ReactionButton,
            SaveSource::ContextMenu => core::SaveSource::ContextMenu,
            SaveSource::LongPressActionSheet => core::SaveSource::LongPressActionSheet,
            SaveSource::MessageToolbar => core::SaveSource::MessageToolbar,
            SaveSource::ForLaterList => core::SaveSource::ForLaterList,
            SaveSource::NotificationsInbox => core::SaveSource::NotificationsInbox,
        };
        run(async move {
            a.save_message(channel_id, message_id, due_at_ms, source)
                .await
        })
        .await
        .map(Into::into)
    }

    pub async fn unsave_message(
        &self,
        channel_id: u64,
        message_id: u64,
    ) -> Result<(), ActionError> {
        let a = self.0.actions();
        run(async move { a.unsave_message(channel_id, message_id).await }).await
    }

    pub async fn list_saved_messages(&self) -> Result<Vec<SavedMessage>, ActionError> {
        let a = self.0.actions();
        let all = run(async move { a.list_saved_messages().await }).await?;
        Ok(all.into_iter().map(Into::into).collect())
    }

    /// Sends `content` to `channel_id`, a reply to `reply_to` (same channel) with its mention on
    /// or off. No local echo: slice B moves the composer here and adds it. Returns the new
    /// message's id.
    pub async fn send_message_with_mention(
        &self,
        channel_id: u64,
        content: String,
        reply_to: Option<u64>,
        mention: bool,
    ) -> Result<u64, ActionError> {
        let a = self.0.actions();
        run(async move { a.send_message(channel_id, content, reply_to, mention).await }).await
    }

    /// Forwards message `message_id` of `channel_id` to `destination_id`.
    pub async fn forward_message(
        &self,
        destination_id: u64,
        channel_id: u64,
        message_id: u64,
        silent: bool,
    ) -> Result<(), ActionError> {
        let a = self.0.actions();
        run(async move {
            a.forward_message(destination_id, channel_id, message_id, silent)
                .await
        })
        .await
    }

    /// The forward's optional comment, sent after the forward succeeded.
    pub async fn forward_comment(
        &self,
        destination_id: u64,
        content: String,
        silent: bool,
    ) -> Result<(), ActionError> {
        let a = self.0.actions();
        run(async move { a.forward_comment(destination_id, content, silent).await }).await
    }

    pub async fn mfa_send_sms(&self, challenge_id: u64) -> Result<(), ActionError> {
        let a = self.0.actions();
        run(async move { a.mfa_send_sms(challenge_id).await }).await
    }

    /// Answers an MFA challenge; on success the request that asked for it has been sent again.
    /// `MfaInvalidCode` keeps the challenge for another try.
    pub async fn mfa_submit(
        &self,
        challenge_id: u64,
        kind: MfaKind,
        data: String,
    ) -> Result<(), ActionError> {
        let a = self.0.actions();
        let kind = match kind {
            MfaKind::Webauthn => core::MfaKind::Webauthn,
            MfaKind::Totp => core::MfaKind::Totp,
            MfaKind::Sms => core::MfaKind::Sms,
            MfaKind::Backup => core::MfaKind::Backup,
            MfaKind::Password => core::MfaKind::Password,
        };
        run(async move { a.mfa_submit(challenge_id, kind, data).await }).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn errors_convert_and_never_show_the_server_message() {
        let e = ActionError::from(core::ActionError::Api {
            status: 400,
            code: core::ApiCode::TooManySavedMessages,
            message: Some("SERVER TEXT".into()),
        });
        assert!(matches!(
            &e,
            ActionError::Api { status: 400, code: ApiErrorCode::TooManySavedMessages, message: Some(m) } if m == "SERVER TEXT"
        ));
        assert!(!format!("{e} {e:?}").contains("SERVER"));
        let e = ActionError::from(core::ActionError::MfaRequired(core::MfaChallenge {
            id: 3,
            methods: vec![core::MfaMethod {
                kind: core::MfaKind::Webauthn,
                challenge: Some("CHALLENGE".into()),
            }],
        }));
        assert!(
            matches!(&e, ActionError::MfaRequired { challenge_id: 3, methods } if methods[0].kind == MfaKind::Webauthn)
        );
        assert!(!format!("{e} {e:?}").contains("CHALLENGE"));
        assert_eq!(
            ActionError::from(core::ActionError::Busy).to_string(),
            "already in progress"
        );
    }
}
