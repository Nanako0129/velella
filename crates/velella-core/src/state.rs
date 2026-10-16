//! In-memory state built from READY and later dispatches. Wire shapes (capabilities 253) follow
//! Concord's parsers (refs/concord/src/discord/gateway/parser/{ready,guilds,channels,
//! user_settings}.rs and state.rs `ready_users`, GPL-3.0-only; see THIRD_PARTY_NOTICES.md).
//! Parsing is deliberately lenient: a malformed or unknown piece is skipped, never fatal.
use crate::guild_settings::NotifySettings;
use crate::permissions;
use crate::presence::Presence;
use crate::read_state::ReadStates;
use crate::relationships::Relationships;
use crate::self_state::SelfState;
use crate::settings_proto::UserSettings;
use crate::threads::Threads;
use crate::voice::VoiceStates;
use serde_json::Value;
use std::collections::HashMap;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct User {
    pub id: u64,
    pub username: String,
    pub global_name: Option<String>,
    pub avatar: Option<String>,
    pub discriminator: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Role {
    pub id: u64,
    pub permissions: u64,
    pub position: i64,
    pub name: Option<String>,
    /// RGB integer (0xRRGGBB); 0 = no colour, as Discord sends it.
    pub color: u32,
}

/// The member-list selection set through `SessionHandle::set_member_subscription`. `epoch` grows
/// with every change, so a reader can tell a new selection from the same one set again.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MemberSub {
    pub epoch: u64,
    pub guild_id: u64,
    pub channel_id: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Overwrite {
    pub id: u64,
    /// 0 = role, 1 = member.
    pub kind: u8,
    pub allow: u64,
    pub deny: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Channel {
    pub id: u64,
    pub guild_id: u64,
    /// Discord channel type number (0 text, 2 voice, 4 category, ...).
    pub kind: u8,
    pub name: Option<String>,
    pub position: Option<i64>,
    pub parent_id: Option<u64>,
    /// `None` when the payload carried none: permissions are then undecidable.
    pub permission_overwrites: Option<Vec<Overwrite>>,
    pub last_message_id: Option<u64>,
    /// Channel flags; `OBFUSCATED` hides the channel whatever else it carries.
    pub flags: u64,
}

/// Channel flag OBFUSCATED (1 << 17, identify.md §5): the server withheld the channel's data.
pub(crate) const OBFUSCATED: u64 = 1 << 17;

impl Channel {
    pub fn is_obfuscated(&self) -> bool {
        self.flags & OBFUSCATED != 0
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Guild {
    pub id: u64,
    pub name: Option<String>,
    pub icon: Option<String>,
    pub owner_id: Option<u64>,
    /// Stub from READY (`unavailable: true`); a later GUILD_CREATE fills it in place.
    pub unavailable: bool,
    /// Created after READY (GUILD_CREATE for an id READY did not list).
    pub after_ready: bool,
    /// `None` when roles were never received: permissions are then undecidable.
    pub roles: Option<Vec<Role>>,
    /// The signed-in user's own role ids; `None` when unknown.
    pub member_roles: Option<Vec<u64>>,
    pub channels: Vec<Channel>,
}

/// DM (type 1) or group DM (type 3). Recipients are resolved through `State::users` on read.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PrivateChannel {
    pub id: u64,
    pub kind: u8,
    pub name: Option<String>,
    pub icon: Option<String>,
    pub recipient_ids: Vec<u64>,
    pub last_message_id: Option<u64>,
    /// Channel flags, as on `Channel`.
    pub flags: u64,
}

impl PrivateChannel {
    pub fn is_obfuscated(&self) -> bool {
        self.flags & OBFUSCATED != 0
    }
}

/// One legacy `user_settings.guild_folders` entry (`id` is null for single-guild containers).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GuildFolder {
    pub id: Option<u64>,
    pub name: Option<String>,
    /// RGB integer (0xRRGGBB); `None` when null or absent.
    pub color: Option<u32>,
    pub guild_ids: Vec<u64>,
}

#[derive(Clone, Debug, Default)]
pub struct State {
    /// Bumped on every mutation; mirrored by `SessionHandle::state()`.
    pub revision: u64,
    pub user_id: Option<u64>,
    /// Lasting user directory: READY `users`, the self user, inline private-channel recipients.
    pub users: HashMap<u64, User>,
    /// READY order first, then guilds created after READY in arrival order.
    pub guilds: Vec<Guild>,
    pub private_channels: Vec<PrivateChannel>,
    /// `None` when READY carried no `user_settings.guild_folders`.
    pub guild_folders: Option<Vec<GuildFolder>>,
    /// Written by `SessionHandle::set_member_subscription` under the state lock; kept across READY.
    pub member_sub: Option<MemberSub>,
    /// Slice D: unread and mention state, mutes, voice occupants, DM presence, pending friend
    /// requests and joined threads. All rebuilt by READY.
    pub read: ReadStates,
    pub settings: NotifySettings,
    pub voice: VoiceStates,
    pub presence: Presence,
    pub relationships: Relationships,
    pub threads: Threads,
    /// The adopted member list of `member_sub` (see `member_list.rs`); rebuilt by READY.
    pub(crate) member_list: crate::member_list::MemberList,
    /// READY `user_settings_proto`, then USER_SETTINGS_PROTO_UPDATE type 1. `None` (Unknown) when
    /// READY carried none or it could not be decoded: folders fall back to legacy, and status,
    /// Developer Mode and presence stay unknown.
    pub user_settings: Option<UserSettings>,
    /// `guild_positions` from the settings: the rail order while there are no folders.
    pub guild_positions: Vec<u64>,
    /// An INITIAL_GUILD received before READY, merged by READY when READY lacks it (I-13).
    pub(crate) initial_guild: Option<Value>,
    /// Raw `apex_experiments` objects for slice X, in arrival order: READY's, then each
    /// STATE_UPDATE's (each overwrites the units it lists). Rebuilt by READY.
    pub apex_experiments: Vec<Value>,
    /// Slice F1: premium type, guild emojis and the own member's timeouts. Rebuilt by READY.
    pub self_state: SelfState,
}

/// STATE_UPDATEs kept after READY's entry; older ones are dropped first.
const MAX_APEX_UPDATES: usize = 32;

pub(crate) fn id(v: &Value) -> Option<u64> {
    v.as_str()
        .and_then(|s| s.parse().ok())
        .or_else(|| v.as_u64())
        .filter(|n| *n != 0)
}
pub(crate) fn get_id(d: &Value, key: &str) -> Option<u64> {
    d.get(key).and_then(id)
}
pub(crate) fn get_str(d: &Value, key: &str) -> Option<String> {
    d.get(key).and_then(Value::as_str).map(str::to_owned)
}
/// Permission bitfields are strings on the wire; numbers are accepted too.
fn bits(v: &Value) -> Option<u64> {
    v.as_str()
        .and_then(|s| s.parse().ok())
        .or_else(|| v.as_u64())
}
pub(crate) fn ids(v: Option<&Value>) -> Vec<u64> {
    v.and_then(Value::as_array)
        .map(|a| a.iter().filter_map(id).collect())
        .unwrap_or_default()
}
/// Guild fields sit on the guild object or, under some capability sets, in `properties`.
fn guild_field<'a>(d: &'a Value, key: &str) -> Option<&'a Value> {
    d.get(key)
        .or_else(|| d.get("properties").and_then(|p| p.get(key)))
}

pub(crate) fn parse_user(v: &Value) -> Option<User> {
    Some(User {
        id: get_id(v, "id")?,
        username: get_str(v, "username").unwrap_or_default(),
        global_name: get_str(v, "global_name"),
        avatar: get_str(v, "avatar"),
        discriminator: get_str(v, "discriminator"),
    })
}

fn parse_role(v: &Value) -> Option<Role> {
    Some(Role {
        id: get_id(v, "id")?,
        permissions: v.get("permissions").and_then(bits).unwrap_or(0),
        position: v.get("position").and_then(Value::as_i64).unwrap_or(0),
        name: get_str(v, "name"),
        color: v
            .get("color")
            .and_then(Value::as_u64)
            .and_then(|c| u32::try_from(c).ok())
            .unwrap_or(0),
    })
}

fn parse_overwrite(v: &Value) -> Option<Overwrite> {
    let kind = match v.get("type")? {
        t if t.as_u64() == Some(0) || t.as_str() == Some("0") || t.as_str() == Some("role") => 0,
        t if t.as_u64() == Some(1) || t.as_str() == Some("1") || t.as_str() == Some("member") => 1,
        _ => return None,
    };
    Some(Overwrite {
        id: get_id(v, "id")?,
        kind,
        allow: v.get("allow").and_then(bits).unwrap_or(0),
        deny: v.get("deny").and_then(bits).unwrap_or(0),
    })
}

fn parse_channel(v: &Value, default_guild: u64) -> Option<Channel> {
    Some(Channel {
        id: get_id(v, "id")?,
        guild_id: get_id(v, "guild_id").unwrap_or(default_guild),
        kind: u8::try_from(v.get("type")?.as_u64()?).ok()?,
        name: get_str(v, "name"),
        position: v.get("position").and_then(Value::as_i64),
        parent_id: get_id(v, "parent_id"),
        // One unreadable overwrite makes the whole set undecidable rather than silently partial.
        permission_overwrites: v
            .get("permission_overwrites")
            .and_then(Value::as_array)
            .and_then(|a| a.iter().map(parse_overwrite).collect()),
        last_message_id: get_id(v, "last_message_id"),
        flags: flags(v),
    })
}

fn flags(v: &Value) -> u64 {
    v.get("flags").and_then(Value::as_u64).unwrap_or(0)
}

/// JavaScript truthiness, for flags the official client tests with `||`.
fn truthy(v: Option<&Value>) -> bool {
    match v {
        None | Some(Value::Null) => false,
        Some(Value::Bool(b)) => *b,
        Some(Value::Number(n)) => n.as_f64() != Some(0.0),
        Some(Value::String(s)) => !s.is_empty(),
        Some(_) => true,
    }
}

/// Own role ids from a member object (`user.id` or `user_id` identifies it).
pub(crate) fn own_member_roles(member: &Value, own: u64) -> Option<Vec<u64>> {
    let who = member
        .get("user")
        .and_then(|u| get_id(u, "id"))
        .or_else(|| get_id(member, "user_id"))?;
    (who == own && member.get("roles").is_some_and(Value::is_array))
        .then(|| ids(member.get("roles")))
}

fn parse_guild(v: &Value, own: Option<u64>) -> Option<Guild> {
    let gid = get_id(v, "id")?;
    let own_roles = own.and_then(|own| {
        let members = v
            .get("members")
            .and_then(Value::as_array)
            .into_iter()
            .flatten();
        members
            .chain(v.get("member"))
            .find_map(|m| own_member_roles(m, own))
    });
    Some(Guild {
        id: gid,
        name: guild_field(v, "name")
            .and_then(Value::as_str)
            .map(str::to_owned),
        icon: guild_field(v, "icon")
            .and_then(Value::as_str)
            .map(str::to_owned),
        owner_id: guild_field(v, "owner_id").and_then(id),
        unavailable: v.get("unavailable").and_then(Value::as_bool) == Some(true)
            || v.get("data_mode").and_then(Value::as_str) == Some("unavailable"),
        after_ready: false,
        roles: v
            .get("roles")
            .and_then(Value::as_array)
            .map(|a| a.iter().filter_map(parse_role).collect()),
        member_roles: own_roles,
        channels: v
            .get("channels")
            .and_then(Value::as_array)
            .map(|a| a.iter().filter_map(|c| parse_channel(c, gid)).collect())
            .unwrap_or_default(),
    })
}

fn parse_private(v: &Value) -> Option<(PrivateChannel, Vec<User>)> {
    let kind = u8::try_from(v.get("type")?.as_u64()?)
        .ok()
        .filter(|k| matches!(k, 1 | 3))?;
    // READY and `lazy_private_channels` carry only `recipient_ids`; CHANNEL_CREATE/UPDATE carry
    // full `recipients` objects, which also refresh the user directory.
    let inline: Vec<User> = v
        .get("recipients")
        .and_then(Value::as_array)
        .map(|a| a.iter().filter_map(parse_user).collect())
        .unwrap_or_default();
    let recipient_ids = if inline.is_empty() {
        ids(v.get("recipient_ids"))
    } else {
        inline.iter().map(|u| u.id).collect()
    };
    Some((
        PrivateChannel {
            id: get_id(v, "id")?,
            kind,
            name: get_str(v, "name"),
            icon: get_str(v, "icon"),
            recipient_ids,
            last_message_id: get_id(v, "last_message_id"),
            flags: flags(v),
        },
        inline,
    ))
}

/// A legacy folder id: a number (0 included, which is still a folder) or a numeric string.
fn folder_id(v: &Value) -> Option<u64> {
    v.as_u64()
        .or_else(|| v.as_i64().map(|i| i as u64))
        .or_else(|| v.as_str()?.parse().ok())
}

/// Legacy `user_settings.guild_folders` (capabilities without bit 9).
fn parse_folders(d: &Value) -> Option<Vec<GuildFolder>> {
    let folders = d.get("user_settings")?.get("guild_folders")?.as_array()?;
    Some(
        folders
            .iter()
            .filter_map(|f| {
                let guild_ids = ids(f.get("guild_ids"));
                (!guild_ids.is_empty()).then(|| GuildFolder {
                    id: f.get("id").and_then(folder_id),
                    name: get_str(f, "name").filter(|n| !n.is_empty()),
                    color: f
                        .get("color")
                        .and_then(Value::as_u64)
                        .and_then(|c| u32::try_from(c).ok()),
                    guild_ids,
                })
            })
            .collect(),
    )
}

/// A guild known only by id: unavailable, so nothing in it is decidable or navigable.
fn stub(id: u64) -> Guild {
    Guild {
        id,
        name: None,
        icon: None,
        owner_id: None,
        unavailable: true,
        after_ready: false,
        roles: None,
        member_roles: None,
        channels: Vec::new(),
    }
}

fn is_partial(v: &Value) -> bool {
    v.get("data_mode").and_then(Value::as_str) == Some("partial")
}

/// Name, icon and owner, wherever the payload carries them (top level or `properties`).
fn merge_props(guild: &mut Guild, d: &Value) {
    if let Some(v) = guild_field(d, "name") {
        guild.name = v.as_str().map(str::to_owned);
    }
    if let Some(v) = guild_field(d, "icon") {
        guild.icon = v.as_str().map(str::to_owned);
    }
    if let Some(v) = guild_field(d, "owner_id") {
        guild.owner_id = id(v);
    }
}

/// Insert or replace; a payload without `last_message_id` keeps the newest one known.
fn upsert_channel(guild: &mut Guild, channel: Channel) {
    match guild.channels.iter_mut().find(|c| c.id == channel.id) {
        Some(slot) => {
            let last = channel.last_message_id.max(slot.last_message_id);
            *slot = Channel {
                last_message_id: last,
                ..channel
            };
        }
        None => guild.channels.push(channel),
    }
}

fn upsert_role(roles: &mut Vec<Role>, role: Role) {
    match roles.iter_mut().find(|r| r.id == role.id) {
        Some(slot) => *slot = role,
        None => roles.push(role),
    }
}

fn items(v: Option<&Value>) -> impl Iterator<Item = &Value> {
    v.and_then(Value::as_array).into_iter().flatten()
}

/// CLIENT_STATE_V2 `data_mode: "partial"` against the guild already held (identify.md §1 bit 10,
/// W:4679860): `properties` (may be null) and `partial_updates` writes and deletes. Whether the
/// guild is available is left as it was.
fn apply_partial(guild: &mut Guild, v: &Value, own: Option<u64>) {
    merge_props(guild, v);
    let updates = v.get("partial_updates");
    let gid = guild.id;
    for c in items(updates.and_then(|u| u.get("channels"))) {
        if let Some(channel) = parse_channel(c, gid) {
            upsert_channel(guild, channel);
        }
    }
    let gone = ids(updates.and_then(|u| u.get("deleted_channel_ids")));
    guild.channels.retain(|c| !gone.contains(&c.id));
    // `channel_updates`: `{id, last_message_id, last_pin_timestamp}` per channel (W:9100953).
    for u in items(v.get("channel_updates")) {
        let (Some(cid), Some(last)) = (get_id(u, "id"), get_id(u, "last_message_id")) else {
            continue;
        };
        if let Some(c) = guild.channels.iter_mut().find(|c| c.id == cid) {
            c.last_message_id = c.last_message_id.max(Some(last));
        }
    }
    // Writes onto an unknown role set would make a partial set look complete.
    if let Some(roles) = guild.roles.as_mut() {
        for r in items(updates.and_then(|u| u.get("roles"))) {
            if let Some(role) = parse_role(r) {
                upsert_role(roles, role);
            }
        }
        let gone = ids(updates.and_then(|u| u.get("deleted_role_ids")));
        roles.retain(|r| !gone.contains(&r.id));
    }
    if let Some(mine) = own.and_then(|own| {
        items(v.get("members"))
            .chain(v.get("member"))
            .find_map(|m| own_member_roles(m, own))
    }) {
        guild.member_roles = Some(mine);
    }
}

/// CHANNEL_SYNC: only the keys present replace the held values; `flags` always comes from the
/// payload, absent meaning none (I-15).
fn merge_present(slot: &mut Channel, c: &Value) {
    if let Some(v) = c.get("name") {
        slot.name = v.as_str().map(str::to_owned);
    }
    if let Some(kind) = c
        .get("type")
        .and_then(Value::as_u64)
        .and_then(|k| u8::try_from(k).ok())
    {
        slot.kind = kind;
    }
    if let Some(v) = c.get("position") {
        slot.position = v.as_i64();
    }
    if let Some(v) = c.get("parent_id") {
        slot.parent_id = id(v);
    }
    if let Some(v) = c.get("permission_overwrites") {
        slot.permission_overwrites = v
            .as_array()
            .and_then(|a| a.iter().map(parse_overwrite).collect());
    }
    slot.last_message_id = get_id(c, "last_message_id").max(slot.last_message_id);
    slot.flags = flags(c);
}

impl State {
    pub fn guild(&self, id: u64) -> Option<&Guild> {
        self.guilds.iter().find(|g| g.id == id)
    }

    /// Guild ids in the order READY listed them.
    pub fn ready_guild_ids(&self) -> Vec<u64> {
        self.guilds
            .iter()
            .filter(|g| !g.after_ready)
            .map(|g| g.id)
            .collect()
    }

    /// Guild ids created after READY, in arrival order.
    pub fn post_ready_guild_ids(&self) -> Vec<u64> {
        self.guilds
            .iter()
            .filter(|g| g.after_ready)
            .map(|g| g.id)
            .collect()
    }

    /// Recipients of a private channel that the user directory can resolve.
    pub fn recipients(&self, channel: &PrivateChannel) -> Vec<&User> {
        channel
            .recipient_ids
            .iter()
            .filter_map(|id| self.users.get(id))
            .collect()
    }

    /// Whether the signed-in user can view `channel`; `None` when owner, roles, member or
    /// overwrite data is missing.
    pub fn can_view_channel(&self, channel: &Channel) -> Option<bool> {
        permissions::can_view_channel(self.user_id?, self.guild(channel.guild_id)?, channel)
    }

    /// Whether a guild channel or private channel with this id is obfuscated (I-5): such a
    /// channel is no target for member lists, acks or menu actions.
    pub fn is_obfuscated(&self, channel: u64) -> bool {
        self.guilds
            .iter()
            .flat_map(|g| &g.channels)
            .find(|c| c.id == channel)
            .map(Channel::is_obfuscated)
            .or_else(|| {
                self.private_channels
                    .iter()
                    .find(|c| c.id == channel)
                    .map(PrivateChannel::is_obfuscated)
            })
            .unwrap_or(false)
    }

    /// Applies one dispatch; returns whether state changed (and bumps `revision`). Unknown events
    /// and unreadable payloads change nothing.
    #[cfg(test)]
    pub(crate) fn apply(&mut self, event: &str, d: &Value) -> bool {
        self.apply_known(event, d).0
    }

    /// `apply`, and whether `event` has a handler here at all (the live check logs the names
    /// nothing handles).
    pub(crate) fn apply_known(&mut self, event: &str, d: &Value) -> (bool, bool) {
        let mut known = true;
        let changed = match event {
            "READY" => self.ready(d),
            "READY_SUPPLEMENTAL" => self.ready_supplemental(d),
            "GUILD_CREATE" => self.guild_create(d),
            "GUILD_UPDATE" => self.guild_update(d),
            "GUILD_DELETE" => self.guild_delete(d),
            "CHANNEL_CREATE" | "CHANNEL_UPDATE" => self.channel_upsert(d),
            "CHANNEL_DELETE" => self.channel_delete(d),
            "GUILD_ROLE_CREATE" | "GUILD_ROLE_UPDATE" => self.role_upsert(d),
            "GUILD_ROLE_DELETE" => self.role_delete(d),
            "GUILD_MEMBER_UPDATE" => self.self_member_update(d),
            "MESSAGE_CREATE" => self.message_create(d),
            "MESSAGE_ACK" => self.message_ack(d),
            "CHANNEL_UNREAD_UPDATE" => self.channel_unread_update(d),
            "USER_GUILD_SETTINGS_UPDATE" => self.settings.update(d),
            "VOICE_STATE_UPDATE" => self.voice_update(d, None),
            "PRESENCE_UPDATE" => self.presence_update(d),
            "RELATIONSHIP_ADD" => self.relationships.add(d),
            "RELATIONSHIP_REMOVE" => self.relationships.remove(d),
            "THREAD_CREATE" | "THREAD_UPDATE" => self.threads.upsert_event(d),
            "THREAD_DELETE" => self.threads.delete(d),
            "THREAD_LIST_SYNC" => self.threads.list_sync(d),
            "THREAD_MEMBER_UPDATE" => self.threads.member_update(d, self.user_id),
            "THREAD_MEMBERS_UPDATE" => self.threads.members_update(d, self.user_id),
            "GUILD_MEMBER_LIST_UPDATE" => self.member_list_update(d),
            "USER_SETTINGS_PROTO_UPDATE" => self.settings_proto_update(d),
            "INITIAL_GUILD" => {
                // Sent before READY; READY merges it when its own list lacks it.
                self.initial_guild = Some(d.clone());
                false
            }
            "CHANNEL_SYNC" => self.channel_sync(d),
            "STATE_UPDATE" => self.state_update(d),
            "VOICE_STATE_UPDATE_BATCH" => self.voice_batch(d),
            "PASSIVE_UPDATE_V2" => self.passive_update_v2(d),
            "GUILD_EMOJIS_UPDATE" => self.self_state.emojis_update(d),
            "USER_UPDATE" => self.self_state.user_update(d, self.user_id),
            _ => {
                known = false;
                false
            }
        };
        if changed {
            self.revision += 1;
        }
        (changed, known)
    }

    fn ready(&mut self, d: &Value) -> bool {
        let Some(me) = d.get("user").and_then(parse_user) else {
            return false;
        };
        let revision = self.revision;
        let mut users = std::mem::take(&mut self.users);
        // A partial guild (CLIENT_STATE_V2) applies to the guild held before this READY.
        let mut held = std::mem::take(&mut self.guilds);
        let initial = self.initial_guild.take();
        // The selection outlives the connection: L re-arms on the SYNC after the re-send.
        let member_sub = self.member_sub.take();
        *self = State {
            revision,
            user_id: Some(me.id),
            member_sub,
            ..Default::default()
        };
        // DEDUPE_USER_OBJECTS: every referenced user once, here.
        users.extend(
            d.get("users")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(parse_user)
                .map(|u| (u.id, u)),
        );
        users.insert(me.id, me.clone());
        self.users = users;
        let merged = d.get("merged_members").and_then(Value::as_array);
        for (i, raw) in items(d.get("guilds")).enumerate() {
            // `merged_members` is index-aligned with `guilds`.
            self.ready_guild(raw, merged.and_then(|m| m.get(i)), &mut held, me.id);
        }
        // After READY's own guilds, so READY's copy wins when it lists the same guild.
        if let Some(raw) = initial {
            self.ready_guild(&raw, None, &mut held, me.id);
        }
        for raw in items(d.get("private_channels")) {
            self.upsert_private(raw);
        }
        self.read.sync(d);
        self.settings.sync(d);
        self.relationships.sync(d);
        // Bit 9. A missing or broken proto leaves READY applied with the settings Unknown (I-14).
        self.user_settings = d
            .get("user_settings_proto")
            .and_then(Value::as_str)
            .and_then(|b64| match UserSettings::from_base64(b64) {
                Ok(s) => Some(s),
                Err(e) => {
                    tracing::warn!(error = %e, "READY user_settings_proto unreadable; settings unknown");
                    None
                }
            });
        match &self.user_settings {
            Some(s) => {
                self.guild_folders = Some(s.guild_folders.clone());
                self.guild_positions = s.guild_positions.clone();
            }
            // Legacy `user_settings` is the fallback while the proto is unknown.
            None => {
                self.guild_folders = parse_folders(d);
                self.guild_positions = ids(d
                    .get("user_settings")
                    .and_then(|s| s.get("guild_positions")));
            }
        }
        if self.guild_folders.is_none() {
            tracing::warn!("READY carried no guild folders; guild order falls back to READY order");
        }
        if let Some(a) = d.get("apex_experiments").filter(|a| a.is_object()) {
            self.apex_experiments.push(a.clone());
        }
        self.self_state.ready(d, me.id);
        true
    }

    /// One READY guild (or the buffered INITIAL_GUILD). A partial guild applies to the guild
    /// held before READY, else it becomes an unavailable stub (I-13).
    fn ready_guild(&mut self, raw: &Value, merged: Option<&Value>, held: &mut Vec<Guild>, me: u64) {
        let Some(gid) = get_id(raw, "id") else {
            return;
        };
        if self.guild(gid).is_some() {
            return;
        }
        let mut guild = if is_partial(raw) {
            match held.iter().position(|g| g.id == gid) {
                Some(at) => {
                    let mut g = held.swap_remove(at);
                    g.after_ready = false;
                    apply_partial(&mut g, raw, Some(me));
                    g
                }
                None => stub(gid),
            }
        } else {
            match parse_guild(raw, Some(me)) {
                Some(g) => g,
                None => return,
            }
        };
        if let Some(roles) = merged.and_then(|m| self.merged_roles(m)) {
            guild.member_roles = Some(roles);
        }
        self.guilds.push(guild);
        self.guild_extras(raw, gid, false);
    }

    /// USER_SETTINGS_PROTO_UPDATE: type 1 updates the settings (`partial` replaces the top-level
    /// fields it carries); type 2 belongs to the frecency owner (F2) and other types are ignored.
    /// An unreadable update keeps the previous values (I-4).
    fn settings_proto_update(&mut self, d: &Value) -> bool {
        let Some(s) = d.get("settings") else {
            return false;
        };
        match s.get("type").and_then(Value::as_u64) {
            Some(1) => {}
            Some(2) => return self.frecency_settings_update(d),
            _ => return false,
        }
        let Some(b64) = s.get("proto").and_then(Value::as_str) else {
            return false;
        };
        let partial = d.get("partial").and_then(Value::as_bool).unwrap_or(false);
        let next = crate::settings_proto::decode(b64).and_then(|m| {
            match (&self.user_settings, partial) {
                (_, false) => UserSettings::from_message(m).map(Some),
                (Some(held), true) => held.merged(m).map(Some),
                // A partial update on unknown settings would read every absent field as default.
                (None, true) => Ok(None),
            }
        });
        let next = match next {
            Ok(Some(n)) => n,
            Ok(None) => return false,
            Err(e) => {
                tracing::warn!(error = %e, "USER_SETTINGS_PROTO_UPDATE unreadable; settings kept");
                return false;
            }
        };
        if self.user_settings.as_ref() == Some(&next) {
            return false;
        }
        self.guild_folders = Some(next.guild_folders.clone());
        self.guild_positions = next.guild_positions.clone();
        self.user_settings = Some(next);
        true
    }

    /// Hook for USER_SETTINGS_PROTO_UPDATE type 2 (`FrecencyUserSettings`): consumed by slice A
    /// (frecency read and write, D3), which replaces this body. Until then the update is ignored
    /// and leaves folders, status and every type-1 value unchanged.
    fn frecency_settings_update(&mut self, _d: &Value) -> bool {
        false
    }

    /// A new IDENTIFY starts a new gateway session: an INITIAL_GUILD buffered on a connection that
    /// died before its READY belongs to no READY and is dropped, never merged later.
    pub(crate) fn identify_started(&mut self) {
        self.initial_guild = None;
    }

    /// CHANNEL_SYNC: skipped when `integrity_check` is set; otherwise each channel merges into
    /// the held one (or is added), as the official CHANNEL_UPDATES batcher does (I-15).
    fn channel_sync(&mut self, d: &Value) -> bool {
        if truthy(d.get("integrity_check")) {
            return false;
        }
        let Some(guild) =
            get_id(d, "guild_id").and_then(|g| self.guilds.iter_mut().find(|x| x.id == g))
        else {
            return false;
        };
        let mut changed = false;
        for c in items(d.get("channels")) {
            let Some(cid) = get_id(c, "id") else {
                continue;
            };
            match guild.channels.iter_mut().find(|x| x.id == cid) {
                Some(slot) => {
                    let before = slot.clone();
                    merge_present(slot, c);
                    changed |= *slot != before;
                }
                None => {
                    if let Some(channel) = parse_channel(c, guild.id) {
                        guild.channels.push(channel);
                        changed = true;
                    }
                }
            }
        }
        changed
    }

    /// STATE_UPDATE `{apex_experiments}`: kept raw for slice X.
    fn state_update(&mut self, d: &Value) -> bool {
        let Some(a) = d.get("apex_experiments").filter(|a| a.is_object()) else {
            return false;
        };
        if self.apex_experiments.len() > MAX_APEX_UPDATES {
            self.apex_experiments.remove(1);
        }
        self.apex_experiments.push(a.clone());
        true
    }

    fn merged_roles(&self, members: &Value) -> Option<Vec<u64>> {
        let own = self.user_id?;
        members
            .as_array()?
            .iter()
            .find_map(|m| own_member_roles(m, own))
    }

    fn ready_supplemental(&mut self, d: &Value) -> bool {
        let mut changed = false;
        let merged = d.get("merged_members").and_then(Value::as_array);
        for (i, raw) in d
            .get("guilds")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .enumerate()
        {
            let roles = merged
                .and_then(|m| m.get(i))
                .and_then(|m| self.merged_roles(m));
            let (Some(gid), Some(roles)) = (get_id(raw, "id"), roles) else {
                continue;
            };
            if let Some(g) = self.guilds.iter_mut().find(|g| g.id == gid) {
                g.member_roles = Some(roles);
                changed = true;
            }
        }
        self.self_state.ready_supplemental(d, self.user_id);
        // Voice occupants (and threads, when present) per guild; the roles loop above skips a guild
        // without a usable merged member, so this walks the guilds again on their own.
        for raw in d
            .get("guilds")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            if let Some(gid) = get_id(raw, "id") {
                changed |= self.guild_extras(raw, gid, false);
            }
        }
        for raw in d
            .get("lazy_private_channels")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            changed |= self.upsert_private(raw);
        }
        // After the private channels: presences are kept only for people the sidebar can show.
        changed |= self.merged_presences(d);
        changed
    }

    /// Voice states and threads carried by a guild object (READY, READY_SUPPLEMENTAL,
    /// GUILD_CREATE). `replace` makes the payload the guild's whole truth (GUILD_CREATE).
    fn guild_extras(&mut self, raw: &Value, gid: u64, replace: bool) -> bool {
        let mut changed = false;
        if replace {
            changed |= self.voice.clear_guild(gid);
        }
        for v in raw
            .get("voice_states")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            changed |= self.voice_update(v, Some(gid));
        }
        if let Some(threads) = raw.get("threads").and_then(Value::as_array) {
            changed |= self.threads.guild_payload(gid, threads, replace);
        }
        changed
    }

    fn upsert_private(&mut self, raw: &Value) -> bool {
        let Some((channel, inline)) = parse_private(raw) else {
            return false;
        };
        for user in inline {
            self.users.insert(user.id, user);
        }
        match self
            .private_channels
            .iter_mut()
            .find(|c| c.id == channel.id)
        {
            Some(slot) => {
                // A payload without `last_message_id` must not forget what MESSAGE_CREATE raised.
                let last = channel.last_message_id.max(slot.last_message_id);
                *slot = PrivateChannel {
                    last_message_id: last,
                    ..channel
                };
            }
            None => self.private_channels.push(channel),
        }
        true
    }

    fn guild_create(&mut self, d: &Value) -> bool {
        self.self_state.guild_create(d, self.user_id);
        if is_partial(d) {
            return self.partial_guild_create(d);
        }
        let Some(mut guild) = parse_guild(d, self.user_id) else {
            return false;
        };
        let gid = guild.id;
        match self.guilds.iter_mut().find(|g| g.id == guild.id) {
            Some(slot) => {
                // Same position and origin as before; keep a known own-member if this lacks one.
                guild.after_ready = slot.after_ready;
                guild.member_roles = guild.member_roles.or_else(|| slot.member_roles.take());
                *slot = guild;
            }
            None => {
                guild.after_ready = true;
                self.guilds.push(guild);
            }
        }
        self.guild_extras(d, gid, true);
        true
    }

    /// A partial GUILD_CREATE: applied to the held guild, else an unavailable stub (I-13).
    fn partial_guild_create(&mut self, d: &Value) -> bool {
        let Some(gid) = get_id(d, "id") else {
            return false;
        };
        let own = self.user_id;
        match self.guilds.iter_mut().find(|g| g.id == gid) {
            Some(slot) => apply_partial(slot, d, own),
            None => self.guilds.push(Guild {
                after_ready: true,
                ..stub(gid)
            }),
        }
        self.guild_extras(d, gid, false);
        true
    }

    fn guild_update(&mut self, d: &Value) -> bool {
        let Some(guild) =
            get_id(d, "id").and_then(|id| self.guilds.iter_mut().find(|g| g.id == id))
        else {
            return false;
        };
        merge_props(guild, d);
        if let Some(roles) = d.get("roles").and_then(Value::as_array) {
            guild.roles = Some(roles.iter().filter_map(parse_role).collect());
        }
        true
    }

    fn guild_delete(&mut self, d: &Value) -> bool {
        let Some(gid) = get_id(d, "id") else {
            return false;
        };
        if d.get("unavailable").and_then(Value::as_bool) == Some(true) {
            // Outage, not removal: keep the guild and its place, mark it undecidable.
            return self
                .guilds
                .iter_mut()
                .find(|g| g.id == gid)
                .map(|g| g.unavailable = true)
                .is_some();
        }
        self.voice.clear_guild(gid);
        self.threads.remove_guild(gid);
        let before = self.guilds.len();
        self.guilds.retain(|g| g.id != gid);
        self.guilds.len() != before
    }

    fn channel_upsert(&mut self, d: &Value) -> bool {
        let Some(gid) = get_id(d, "guild_id") else {
            return self.upsert_private(d);
        };
        let (Some(channel), Some(guild)) = (
            parse_channel(d, gid),
            self.guilds.iter_mut().find(|g| g.id == gid),
        ) else {
            return false;
        };
        upsert_channel(guild, channel);
        true
    }

    fn channel_delete(&mut self, d: &Value) -> bool {
        let Some(cid) = get_id(d, "id") else {
            return false;
        };
        match get_id(d, "guild_id") {
            Some(gid) => {
                let Some(guild) = self.guilds.iter_mut().find(|g| g.id == gid) else {
                    return false;
                };
                let before = guild.channels.len();
                guild.channels.retain(|c| c.id != cid);
                guild.channels.len() != before
            }
            None => {
                let before = self.private_channels.len();
                self.private_channels.retain(|c| c.id != cid);
                self.private_channels.len() != before
            }
        }
    }

    /// Roles stay `None` (undecidable) if the guild never had a role list.
    fn guild_roles(&mut self, d: &Value) -> Option<&mut Vec<Role>> {
        let gid = get_id(d, "guild_id")?;
        self.guilds.iter_mut().find(|g| g.id == gid)?.roles.as_mut()
    }

    fn role_upsert(&mut self, d: &Value) -> bool {
        let Some(role) = d.get("role").and_then(parse_role) else {
            return false;
        };
        let Some(roles) = self.guild_roles(d) else {
            return false;
        };
        upsert_role(roles, role);
        true
    }

    fn role_delete(&mut self, d: &Value) -> bool {
        let Some(rid) = get_id(d, "role_id") else {
            return false;
        };
        let Some(roles) = self.guild_roles(d) else {
            return false;
        };
        let before = roles.len();
        roles.retain(|r| r.id != rid);
        roles.len() != before
    }

    fn self_member_update(&mut self, d: &Value) -> bool {
        self.self_state.member_update(d, self.user_id);
        let (Some(own), Some(gid)) = (self.user_id, get_id(d, "guild_id")) else {
            return false;
        };
        let Some(roles) = own_member_roles(d, own) else {
            return false;
        };
        match self.guilds.iter_mut().find(|g| g.id == gid) {
            Some(g) => {
                g.member_roles = Some(roles);
                true
            }
            None => false,
        }
    }
}
