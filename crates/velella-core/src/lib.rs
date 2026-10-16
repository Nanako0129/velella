//! UI-agnostic core (session, gateway, state). Must stay free of uniffi.
//!
//! `Session::connect` spawns onto the current tokio runtime; velella-ffi calls it from its owned
//! runtime. Nothing here owns a runtime.
mod compression;
pub mod embeds;
mod gateway;
mod guild_settings;
pub mod highlight;
mod identify;
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
mod rest_messages;
mod session;
mod settings_proto;
mod sidebar;
mod state;
mod threads;
mod token;
mod typing;
mod voice;

#[cfg(test)]
mod fixture_tests;
#[cfg(test)]
mod member_list_tests;
#[cfg(test)]
mod proto_tests;
#[cfg(test)]
mod ready_shapes_tests;
#[cfg(test)]
mod rest_tests;
#[cfg(test)]
mod session_tests;
#[cfg(test)]
mod sidebar_state_tests;
#[cfg(test)]
mod sidebar_tests;

pub use identify::USER_AGENT;
pub use member_list::{member_sections, MemberListEntry, MemberListSection, MemberListStatus};
pub use messages::{
    EmojiImage, LoadState, MentionKind, MessageImage, MessageReply, MessageRow,
    MessageSubscription, MessagesSnapshot, OlderState, ResolvedMention,
};
pub use presence::PresenceStatus;
pub use reactions::Reaction;
pub use session::{Session, SessionEvent, SessionHandle, Subscription};
pub use sidebar::{
    channel_categories, dm_list, guild_entries, SidebarCategory, SidebarChannel, SidebarDm,
    SidebarEntry, SidebarGuild, SidebarOccupant, SidebarThread,
};
pub use state::{
    Channel, Guild, GuildFolder, MemberSub, Overwrite, PrivateChannel, Role, State, User,
};
pub use token::{Token, TokenError};
