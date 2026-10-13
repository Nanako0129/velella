//! UI-agnostic core (session, gateway, state). Must stay free of uniffi.
//!
//! `Session::connect` spawns onto the current tokio runtime; velella-ffi calls it from its owned
//! runtime. Nothing here owns a runtime.
mod compression;
mod gateway;
mod identify;
mod messages;
mod permissions;
mod rest;
mod rest_messages;
mod session;
mod sidebar;
mod state;
mod token;

#[cfg(test)]
mod fixture_tests;
#[cfg(test)]
mod rest_tests;
#[cfg(test)]
mod session_tests;
#[cfg(test)]
mod sidebar_tests;

pub use identify::USER_AGENT;
pub use messages::{
    LoadState, MessageImage, MessageReply, MessageRow, MessageSubscription, MessagesSnapshot,
};
pub use session::{Session, SessionEvent, SessionHandle, Subscription};
pub use sidebar::{
    channel_categories, dm_list, guild_entries, SidebarCategory, SidebarChannel, SidebarDm,
    SidebarEntry, SidebarGuild,
};
pub use state::{
    Channel, Guild, GuildFolder, MemberSub, Overwrite, PrivateChannel, Role, State, User,
};
pub use token::{Token, TokenError};
