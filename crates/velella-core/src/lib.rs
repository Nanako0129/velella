//! UI-agnostic core (session, gateway, state). Must stay free of uniffi.
//!
//! `Session::connect` spawns onto the current tokio runtime; velella-ffi calls it from its owned
//! runtime. Nothing here owns a runtime.
mod compression;
mod gateway;
mod identify;
mod permissions;
mod session;
mod state;
mod token;

#[cfg(test)]
mod fixture_tests;
#[cfg(test)]
mod session_tests;

pub use identify::USER_AGENT;
pub use session::{Session, SessionEvent, SessionHandle};
pub use state::{Channel, Guild, GuildFolder, Overwrite, PrivateChannel, Role, State, User};
pub use token::{Token, TokenError};
