//! The open channel's permissions and the signed-in user's own state across FFI (menu plan F1).
//! Both are computed in core from current state on every call; nothing is cached here.
use crate::SessionHandle;
use velella_core as core;

/// The user's permissions in one channel. `Private` is a DM or group DM: every permission test is
/// false there, and features that work in DMs check for it themselves. `Unknown` means something
/// the answer needs has not arrived: the caller must not treat it as denied or allowed.
#[derive(Clone, Copy, Debug, PartialEq, Eq, uniffi::Enum)]
pub enum ChannelPermissions {
    /// The bits of the official permission table (`permission_table`).
    Bits {
        bits: u64,
    },
    Private,
    Unknown,
}

impl From<core::ChannelPermissions> for ChannelPermissions {
    fn from(p: core::ChannelPermissions) -> Self {
        match p {
            core::ChannelPermissions::Bits(bits) => Self::Bits { bits },
            core::ChannelPermissions::Private => Self::Private,
            core::ChannelPermissions::Unknown => Self::Unknown,
        }
    }
}

/// One permission of the official table; its mask is `1 << bit`.
#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct PermissionBit {
    pub name: String,
    pub bit: u8,
}

/// Every permission of the official table, the single list the app tests `bits` against.
#[uniffi::export]
pub fn permission_table() -> Vec<PermissionBit> {
    core::PERMISSION_TABLE
        .iter()
        .map(|(name, bit)| PermissionBit {
            name: (*name).to_owned(),
            bit: *bit,
        })
        .collect()
}

#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct GuildEmoji {
    pub id: u64,
    pub name: String,
    pub animated: bool,
    pub available: bool,
    /// Roles allowed to use it; empty means everyone.
    pub roles: Vec<u64>,
    pub managed: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct GuildEmojis {
    pub guild_id: u64,
    pub emojis: Vec<GuildEmoji>,
}

/// The user's own account state. `None` is Unknown (not yet received), never a default.
#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct SelfState {
    /// 0 none, 1 Nitro Classic, 2 Nitro, 3 Nitro Basic.
    pub premium_type: Option<u8>,
    pub developer_mode: Option<bool>,
    pub render_reactions: Option<bool>,
    /// Guilds whose emoji list is known, in guild order; a guild missing here is Unknown, a guild
    /// with an empty list has none.
    pub guild_emojis: Vec<GuildEmojis>,
}

impl SelfState {
    fn of(state: &core::State) -> Self {
        Self {
            premium_type: state.self_state.premium_type,
            developer_mode: state.developer_mode(),
            render_reactions: state.render_reactions(),
            guild_emojis: state
                .guild_emojis()
                .into_iter()
                .map(|(guild_id, emojis)| GuildEmojis {
                    guild_id,
                    emojis: emojis
                        .iter()
                        .map(|e| GuildEmoji {
                            id: e.id,
                            name: e.name.clone(),
                            animated: e.animated,
                            available: e.available,
                            roles: e.roles.clone(),
                            managed: e.managed,
                        })
                        .collect(),
                })
                .collect(),
        }
    }
}

#[uniffi::export]
impl SessionHandle {
    /// The user's permissions in a guild channel, thread or DM, from the current state.
    pub fn channel_permissions(&self, channel_id: u64) -> ChannelPermissions {
        core::channel_permissions(&self.0.snapshot(), channel_id).into()
    }

    pub fn self_state(&self) -> SelfState {
        SelfState::of(&self.0.snapshot())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use velella_core::{Channel, Guild, Overwrite, Role, State};

    const VIEW: u64 = 1 << 10;
    const SEND: u64 = 1 << 11;

    /// Guild 10 owned by someone else: @everyone may view and send, channel 30 denies sending to
    /// @everyone, the user (1) holds no roles.
    fn state() -> State {
        let channel = |id, kind, deny| Channel {
            id,
            guild_id: 10,
            kind,
            name: None,
            position: None,
            parent_id: None,
            permission_overwrites: Some(vec![Overwrite {
                id: 10,
                kind: 0,
                allow: 0,
                deny,
            }]),
            last_message_id: None,
            flags: 0,
        };
        let mut s = State::default();
        s.user_id = Some(1);
        s.guilds.push(Guild {
            id: 10,
            name: None,
            icon: None,
            owner_id: Some(99),
            unavailable: false,
            after_ready: false,
            roles: Some(vec![Role {
                id: 10,
                permissions: VIEW | SEND,
                position: 0,
                name: None,
                color: 0,
            }]),
            member_roles: Some(vec![]),
            channels: vec![channel(20, 0, 0), channel(30, 0, SEND)],
        });
        s.self_state
            .members
            .insert(10, velella_core::OwnMember::default());
        s.self_state.mfa_levels.insert(10, 0);
        s
    }

    #[test]
    fn permissions_cross_as_bits_private_and_unknown() {
        let s = state();
        let at = |id| ChannelPermissions::from(core::channel_permissions_at(&s, id, 0));
        assert_eq!(at(20), ChannelPermissions::Bits { bits: VIEW | SEND });
        assert_eq!(at(30), ChannelPermissions::Bits { bits: VIEW });
        assert_eq!(at(404), ChannelPermissions::Unknown);
        let mut s = s;
        s.private_channels.push(velella_core::PrivateChannel {
            id: 50,
            kind: 1,
            name: None,
            icon: None,
            recipient_ids: vec![],
            last_message_id: None,
            flags: 0,
        });
        assert_eq!(
            ChannelPermissions::from(core::channel_permissions_at(&s, 50, 0)),
            ChannelPermissions::Private
        );
        // The wall clock variant agrees for a state without timeouts.
        assert_eq!(
            ChannelPermissions::from(core::channel_permissions(&s, 20)),
            ChannelPermissions::Bits { bits: VIEW | SEND }
        );
    }

    #[test]
    fn the_table_crosses_with_names_and_bit_numbers() {
        let t = permission_table();
        assert_eq!(t.len(), 53);
        let bit = |n: &str| t.iter().find(|p| p.name == n).unwrap().bit;
        assert_eq!((bit("VIEW_CHANNEL"), bit("PIN_MESSAGES")), (10, 51));
        assert_eq!(1u64 << bit("SEND_MESSAGES"), SEND);
    }

    #[test]
    fn self_state_crosses_known_and_unknown_values() {
        let mut s = state();
        let unknown = SelfState::of(&s);
        assert_eq!(
            (
                unknown.premium_type,
                unknown.developer_mode,
                unknown.render_reactions
            ),
            (None, None, None)
        );
        assert!(unknown.guild_emojis.is_empty());
        s.self_state.premium_type = Some(2);
        s.self_state.emojis.insert(
            10,
            vec![velella_core::GuildEmoji {
                id: 5,
                name: "e".into(),
                animated: true,
                available: false,
                roles: vec![7],
                managed: true,
            }],
        );
        // A list for a guild that is not in state is not offered.
        s.self_state.emojis.insert(77, vec![]);
        let known = SelfState::of(&s);
        assert_eq!(known.premium_type, Some(2));
        assert_eq!(
            known.guild_emojis,
            vec![GuildEmojis {
                guild_id: 10,
                emojis: vec![GuildEmoji {
                    id: 5,
                    name: "e".into(),
                    animated: true,
                    available: false,
                    roles: vec![7],
                    managed: true
                }]
            }]
        );
    }
}
