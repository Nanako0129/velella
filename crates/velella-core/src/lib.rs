//! UI-agnostic core (session, gateway, state). Must stay free of uniffi.
//!
//! `Session::connect` spawns onto the current tokio runtime; velella-ffi calls it from its owned
//! runtime. Nothing here owns a runtime.
mod compression;
mod gateway;
mod guild_settings;
mod identify;
pub mod markdown;
mod member_list;
mod messages;
mod permissions;
mod presence;
mod read_state;
mod relationships;
mod rest;
mod rest_messages;
mod session;
mod sidebar;
mod state;
mod threads;
mod token;
mod voice;

#[cfg(test)]
mod fixture_tests;
#[cfg(test)]
mod member_list_tests;
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
    MessageSubscription, MessagesSnapshot, ResolvedMention,
};
pub use presence::PresenceStatus;
pub use session::{Session, SessionEvent, SessionHandle, Subscription};
pub use sidebar::{
    channel_categories, dm_list, guild_entries, SidebarCategory, SidebarChannel, SidebarDm,
    SidebarEntry, SidebarGuild, SidebarOccupant, SidebarThread,
};
pub use state::{
    Channel, Guild, GuildFolder, MemberSub, Overwrite, PrivateChannel, Role, State, User,
};
pub use token::{Token, TokenError};
