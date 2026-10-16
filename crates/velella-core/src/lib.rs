//! UI-agnostic core (session, gateway, state). Must stay free of uniffi.
//!
//! `Session::connect` spawns onto the current tokio runtime; velella-ffi calls it from its owned
//! runtime. Nothing here owns a runtime.
mod compression;
pub mod embeds;
mod experiments;
mod frecency_proto;
mod gateway;
mod guild_settings;
pub mod highlight;
mod identify;
mod identity_fetch;
mod identity_store;
mod local_presence;
pub mod markdown;
mod media;
mod member_list;
mod messages;
mod permissions;
mod presence;
mod proto;
mod reactions;
mod read_state;
mod relationships;
mod rest;
mod rest_actions;
mod rest_messages;
mod self_state;
mod session;
mod settings_proto;
mod sidebar;
mod state;
mod threads;
mod token;
pub mod transport;
mod typing;
mod voice;

#[cfg(test)]
mod experiments_tests;
#[cfg(test)]
mod fixture_tests;
#[cfg(test)]
mod member_list_tests;
#[cfg(test)]
mod permissions_tests;
#[cfg(test)]
mod proto_tests;
#[cfg(test)]
mod ready_shapes_tests;
#[cfg(test)]
mod rest_actions_tests;
#[cfg(test)]
mod rest_tests;
#[cfg(test)]
mod session_tests;
#[cfg(test)]
mod sidebar_state_tests;
#[cfg(test)]
mod sidebar_tests;

pub use experiments::{BookmarkLimits, Gate, MenuGate};
pub use identify::{Launch, LaunchConfig, USER_AGENT};
pub use member_list::{member_sections, MemberListEntry, MemberListSection, MemberListStatus};
pub use messages::{
    EmojiImage, LoadState, MentionKind, MessageImage, MessageReply, MessageRow,
    MessageSubscription, MessagesSnapshot, OlderState, ResolvedMention,
};
pub use permissions::{
    channel_permissions, channel_permissions_at, ChannelPermissions, ALL_PERMISSIONS,
    PERMISSION_TABLE,
};
pub use presence::PresenceStatus;
pub use reactions::Reaction;
pub use rest::{ActionError, ApiCode, MfaChallenge, MfaKind, MfaMethod};
pub use rest_actions::{
    thread_state, AckFlags, Actions, Emoji, PinnedMessage, PinsPage, ReactionLocation,
    ReactionType, SaveSource, SavedMessage, ThreadState, Tri,
};
pub use self_state::{GuildEmoji, OwnMember, SelfState, Timeout};
pub use session::{
    IdentityEvent, IdentityNotice, RefreshedToken, Session, SessionEvent, SessionHandle,
    SessionOptions, Subscription,
};
pub use sidebar::{
    channel_categories, dm_list, guild_entries, SidebarCategory, SidebarChannel, SidebarDm,
    SidebarEntry, SidebarGuild, SidebarOccupant, SidebarThread,
};
pub use state::{
    Channel, Guild, GuildFolder, MemberSub, Overwrite, PrivateChannel, Role, State, User,
};
pub use token::{Token, TokenError};
