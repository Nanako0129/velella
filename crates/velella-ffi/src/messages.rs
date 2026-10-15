//! Messages across FFI (plan M): open one channel's messages and send to it. Only ids, text and
//! fixed-text states cross; the token stays in core.
use crate::markdown::{blocks, MdBlock, MdEmoji, MdMention};
use crate::{runtime, SessionHandle};
use std::sync::Arc;

/// Message callbacks run on a `velella-rt` thread and must not block.
#[uniffi::export(with_foreign)]
pub trait MessagesListener: Send + Sync {
    /// Every delivery names the channel it was made for.
    fn on_messages(&self, channel_id: u64, snapshot: MessagesSnapshot);
}

#[derive(uniffi::Enum)]
pub enum MessageLoad {
    Loading,
    Loaded,
    /// Fixed text.
    Failed {
        text: String,
    },
}

/// Older-history state of the open channel.
#[derive(uniffi::Enum)]
pub enum MessageOlder {
    Idle,
    Loading,
    ReachedStart,
    /// Fixed text.
    Failed {
        text: String,
    },
}

#[derive(uniffi::Record)]
pub struct MessageImage {
    pub url: String,
    pub width: u32,
    pub height: u32,
}

#[derive(uniffi::Record)]
pub struct MessageReply {
    pub author_name: String,
    pub content: String,
}

/// One emoji's reaction. A custom emoji has `url` (built in core from its id); a unicode emoji is
/// its `name` and has none.
#[derive(uniffi::Record)]
pub struct MessageReaction {
    pub name: String,
    pub url: Option<String>,
    pub count: u32,
    pub me: bool,
}

#[derive(uniffi::Record)]
pub struct MessageRow {
    pub id: u64,
    pub author_id: u64,
    pub author_name: String,
    pub author_avatar_url: String,
    pub author_color: Option<u32>,
    /// Raw content; render `body`, never this.
    pub content: String,
    pub body: Vec<MdBlock>,
    /// Each distinct mention in `body`, resolved.
    pub mentions: Vec<MdMention>,
    /// Image URLs of at most 64 distinct custom emoji in `body`; others render as `:name:`.
    pub emoji: Vec<MdEmoji>,
    pub timestamp_ms: u64,
    pub edited: bool,
    pub images: Vec<MessageImage>,
    pub files: Vec<String>,
    pub reply: Option<MessageReply>,
    pub pending: bool,
    /// Fixed text.
    pub failure: Option<String>,
    /// First-seen order, at most 20.
    pub reactions: Vec<MessageReaction>,
}

#[derive(uniffi::Record)]
pub struct MessagesSnapshot {
    pub load: MessageLoad,
    pub rows: Vec<MessageRow>,
    pub older: MessageOlder,
    /// Display names of the users typing now, never the self user; text, not markup.
    pub typing: Vec<String>,
}

/// Dropping this leaves the channel: deliveries stop and an in-flight history fetch is cancelled.
#[derive(uniffi::Object)]
pub struct MessageSubscription(velella_core::MessageSubscription);

#[uniffi::export]
impl MessageSubscription {
    /// Asks for the 50 messages before the oldest buffered one. Core ignores it unless the first
    /// page is in and no older page is loading or the start reached; the result arrives as a
    /// snapshot.
    pub fn load_older(&self) {
        let _context = runtime().enter();
        self.0.load_older();
    }
}

#[uniffi::export]
impl SessionHandle {
    /// Opens `channel_id` (guild channel or DM) and delivers its messages now and on every
    /// change. Opening another channel ends the previous subscription's deliveries.
    pub fn subscribe_messages(
        &self,
        channel_id: u64,
        listener: Arc<dyn MessagesListener>,
    ) -> Arc<MessageSubscription> {
        let _context = runtime().enter();
        Arc::new(MessageSubscription(
            self.0.subscribe_messages(channel_id, move |snapshot| {
                listener.on_messages(snapshot.channel_id, snapshot.into())
            }),
        ))
    }

    /// Sends `content` to `channel_id` only if that is the open channel (the one the live
    /// `subscribe_messages` opened); false when nothing was sent (blank text, not open, before
    /// READY). The result shows in the snapshot as an echo that is confirmed or fails. With
    /// `reply_to` the message replies to that message of the same channel.
    #[uniffi::method(default(reply_to = None))]
    pub fn send_message(&self, channel_id: u64, content: String, reply_to: Option<u64>) -> bool {
        let _context = runtime().enter();
        self.0.send_message(channel_id, content, reply_to).is_some()
    }
}

impl From<velella_core::MessagesSnapshot> for MessagesSnapshot {
    fn from(s: velella_core::MessagesSnapshot) -> Self {
        Self {
            load: match s.load {
                velella_core::LoadState::Loading => MessageLoad::Loading,
                velella_core::LoadState::Loaded => MessageLoad::Loaded,
                velella_core::LoadState::Failed(text) => MessageLoad::Failed {
                    text: text.to_owned(),
                },
            },
            rows: s.rows.into_iter().map(Into::into).collect(),
            older: match s.older {
                velella_core::OlderState::Idle => MessageOlder::Idle,
                velella_core::OlderState::Loading => MessageOlder::Loading,
                velella_core::OlderState::ReachedStart => MessageOlder::ReachedStart,
                velella_core::OlderState::Failed(text) => MessageOlder::Failed {
                    text: text.to_owned(),
                },
            },
            typing: s.typing,
        }
    }
}

impl From<velella_core::MessageRow> for MessageRow {
    fn from(r: velella_core::MessageRow) -> Self {
        Self {
            id: r.id,
            author_id: r.author_id,
            author_name: r.author_name,
            author_avatar_url: r.author_avatar_url,
            author_color: r.author_color,
            content: r.content,
            body: blocks(r.body),
            mentions: r.mentions.into_iter().map(Into::into).collect(),
            emoji: r.emoji.into_iter().map(Into::into).collect(),
            timestamp_ms: r.timestamp_ms,
            edited: r.edited,
            images: r
                .images
                .into_iter()
                .map(|i| MessageImage {
                    url: i.url,
                    width: i.width,
                    height: i.height,
                })
                .collect(),
            files: r.files,
            reply: r.reply.map(|p| MessageReply {
                author_name: p.author_name,
                content: p.content,
            }),
            pending: r.pending,
            failure: r.failure.map(str::to_owned),
            reactions: r
                .reactions
                .into_iter()
                .map(|x| MessageReaction {
                    name: x.name,
                    url: x.url,
                    count: x.count,
                    me: x.me,
                })
                .collect(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn core_snapshot_converts_field_for_field() {
        let row = velella_core::MessageRow {
            id: 1,
            author_id: 2,
            author_name: "n".into(),
            author_avatar_url: "https://cdn.discordapp.com/embed/avatars/0.png".into(),
            author_color: Some(0x112233),
            content: "[a](file:///etc)".into(),
            body: velella_core::markdown::parse("[a](file:///etc)"),
            mentions: vec![velella_core::ResolvedMention {
                kind: velella_core::MentionKind::Role,
                id: 6,
                name: Some("mods".into()),
                color: 0x445566,
            }],
            emoji: vec![velella_core::EmojiImage {
                id: 7,
                animated: true,
                url: "https://cdn.discordapp.com/emojis/7.gif?size=48".into(),
            }],
            timestamp_ms: 3,
            edited: true,
            images: vec![velella_core::MessageImage {
                url: "u".into(),
                width: 4,
                height: 5,
            }],
            files: vec!["f.txt".into()],
            reply: Some(velella_core::MessageReply {
                author_name: "r".into(),
                content: "c".into(),
            }),
            pending: false,
            failure: Some("Not sent"),
            reactions: vec![velella_core::Reaction {
                id: Some(8),
                name: "party".into(),
                animated: false,
                url: Some("https://cdn.discordapp.com/emojis/8.webp?size=48".into()),
                count: 3,
                me: true,
            }],
        };
        let s = MessagesSnapshot::from(velella_core::MessagesSnapshot {
            channel_id: 9,
            load: velella_core::LoadState::Failed("Couldn't load messages"),
            rows: vec![row],
            older: velella_core::OlderState::Failed("Couldn't load older messages"),
            typing: vec!["Ann".into(), "Bob".into()],
        });
        assert!(
            matches!(&s.load, MessageLoad::Failed { text } if text == "Couldn't load messages")
        );
        assert!(
            matches!(&s.older, MessageOlder::Failed { text } if text == "Couldn't load older messages")
        );
        assert_eq!(s.typing, ["Ann", "Bob"]);
        // Every older state maps to its own case (only Failed is in the snapshot above).
        let older = |o| {
            MessagesSnapshot::from(velella_core::MessagesSnapshot {
                channel_id: 9,
                load: velella_core::LoadState::Loaded,
                rows: vec![],
                older: o,
                typing: vec![],
            })
            .older
        };
        assert!(matches!(
            older(velella_core::OlderState::Idle),
            MessageOlder::Idle
        ));
        assert!(matches!(
            older(velella_core::OlderState::Loading),
            MessageOlder::Loading
        ));
        assert!(matches!(
            older(velella_core::OlderState::ReachedStart),
            MessageOlder::ReachedStart
        ));
        let r = &s.rows[0];
        assert_eq!(
            (
                r.id,
                r.author_id,
                &*r.author_name,
                r.author_color,
                &*r.content
            ),
            (1, 2, "n", Some(0x112233), "[a](file:///etc)")
        );
        // A rejected link target stays literal text in the body (K1 D4).
        assert_eq!(
            r.body,
            [MdBlock::Paragraph {
                content: vec![crate::markdown::MdInline::Text {
                    text: "[a](file:///etc)".into()
                }]
            }]
        );
        assert_eq!(
            r.mentions,
            [MdMention {
                kind: crate::markdown::MdMentionKind::Role,
                id: 6,
                name: Some("mods".into()),
                color: 0x445566
            }]
        );
        assert_eq!(
            r.emoji,
            [MdEmoji {
                id: 7,
                animated: true,
                url: "https://cdn.discordapp.com/emojis/7.gif?size=48".into()
            }]
        );
        assert_eq!((r.timestamp_ms, r.edited, r.pending), (3, true, false));
        assert_eq!(
            (&*r.images[0].url, r.images[0].width, r.images[0].height),
            ("u", 4, 5)
        );
        assert_eq!(r.files, ["f.txt"]);
        let reply = r.reply.as_ref().unwrap();
        assert_eq!((&*reply.author_name, &*reply.content), ("r", "c"));
        assert_eq!(r.failure.as_deref(), Some("Not sent"));
        let reaction = &r.reactions[0];
        assert_eq!(
            (
                &*reaction.name,
                reaction.url.as_deref(),
                reaction.count,
                reaction.me
            ),
            (
                "party",
                Some("https://cdn.discordapp.com/emojis/8.webp?size=48"),
                3,
                true
            )
        );
    }
}
