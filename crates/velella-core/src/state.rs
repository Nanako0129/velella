//! In-memory state built from READY and later dispatches. Wire shapes (capabilities 253) follow
//! Concord's parsers (refs/concord/src/discord/gateway/parser/{ready,guilds,channels,
//! user_settings}.rs and state.rs `ready_users`, GPL-3.0-only; see THIRD_PARTY_NOTICES.md).
//! Parsing is deliberately lenient: a malformed or unknown piece is skipped, never fatal.
use crate::permissions;
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
}

/// One legacy `user_settings.guild_folders` entry (`id` is null for single-guild containers).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GuildFolder {
    pub id: Option<u64>,
    pub name: Option<String>,
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
}

fn id(v: &Value) -> Option<u64> {
    v.as_str()
        .and_then(|s| s.parse().ok())
        .or_else(|| v.as_u64())
        .filter(|n| *n != 0)
}
fn get_id(d: &Value, key: &str) -> Option<u64> {
    d.get(key).and_then(id)
}
fn get_str(d: &Value, key: &str) -> Option<String> {
    d.get(key).and_then(Value::as_str).map(str::to_owned)
}
/// Permission bitfields are strings on the wire; numbers are accepted too.
fn bits(v: &Value) -> Option<u64> {
    v.as_str()
        .and_then(|s| s.parse().ok())
        .or_else(|| v.as_u64())
}
fn ids(v: Option<&Value>) -> Vec<u64> {
    v.and_then(Value::as_array)
        .map(|a| a.iter().filter_map(id).collect())
        .unwrap_or_default()
}
/// Guild fields sit on the guild object or, under some capability sets, in `properties`.
fn guild_field<'a>(d: &'a Value, key: &str) -> Option<&'a Value> {
    d.get(key)
        .or_else(|| d.get("properties").and_then(|p| p.get(key)))
}

fn parse_user(v: &Value) -> Option<User> {
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
    })
}

/// Own role ids from a member object (`user.id` or `user_id` identifies it).
fn own_member_roles(member: &Value, own: u64) -> Option<Vec<u64>> {
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
        unavailable: v.get("unavailable").and_then(Value::as_bool) == Some(true),
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
        },
        inline,
    ))
}

fn parse_folders(d: &Value) -> Option<Vec<GuildFolder>> {
    let folders = d.get("user_settings")?.get("guild_folders")?.as_array()?;
    Some(
        folders
            .iter()
            .filter_map(|f| {
                let guild_ids = ids(f.get("guild_ids"));
                (!guild_ids.is_empty()).then(|| GuildFolder {
                    id: get_id(f, "id"),
                    name: get_str(f, "name").filter(|n| !n.is_empty()),
                    guild_ids,
                })
            })
            .collect(),
    )
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

    /// Applies one dispatch; returns whether state changed (and bumps `revision`). Unknown events
    /// and unreadable payloads change nothing.
    pub(crate) fn apply(&mut self, event: &str, d: &Value) -> bool {
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
            _ => false,
        };
        if changed {
            self.revision += 1;
        }
        changed
    }

    fn ready(&mut self, d: &Value) -> bool {
        let Some(me) = d.get("user").and_then(parse_user) else {
            return false;
        };
        let revision = self.revision;
        let mut users = std::mem::take(&mut self.users);
        *self = State {
            revision,
            user_id: Some(me.id),
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
        for (i, raw) in d
            .get("guilds")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .enumerate()
        {
            let Some(mut guild) = parse_guild(raw, Some(me.id)) else {
                continue;
            };
            if self.guild(guild.id).is_some() {
                continue;
            }
            // `merged_members` is index-aligned with `guilds`.
            if let Some(roles) = merged
                .and_then(|m| m.get(i))
                .and_then(|m| self.merged_roles(m))
            {
                guild.member_roles = Some(roles);
            }
            self.guilds.push(guild);
        }
        for raw in d
            .get("private_channels")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            self.upsert_private(raw);
        }
        self.guild_folders = parse_folders(d);
        if self.guild_folders.is_none() {
            tracing::warn!("READY carried no user_settings.guild_folders; guild order falls back to READY order");
        }
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
        for raw in d
            .get("lazy_private_channels")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            changed |= self.upsert_private(raw);
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
            Some(slot) => *slot = channel,
            None => self.private_channels.push(channel),
        }
        true
    }

    fn guild_create(&mut self, d: &Value) -> bool {
        let Some(mut guild) = parse_guild(d, self.user_id) else {
            return false;
        };
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
        true
    }

    fn guild_update(&mut self, d: &Value) -> bool {
        let Some(guild) =
            get_id(d, "id").and_then(|id| self.guilds.iter_mut().find(|g| g.id == id))
        else {
            return false;
        };
        if let Some(v) = guild_field(d, "name") {
            guild.name = v.as_str().map(str::to_owned);
        }
        if let Some(v) = guild_field(d, "icon") {
            guild.icon = v.as_str().map(str::to_owned);
        }
        if let Some(v) = guild_field(d, "owner_id") {
            guild.owner_id = id(v);
        }
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
        match guild.channels.iter_mut().find(|c| c.id == channel.id) {
            Some(slot) => *slot = channel,
            None => guild.channels.push(channel),
        }
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
        match roles.iter_mut().find(|r| r.id == role.id) {
            Some(slot) => *slot = role,
            None => roles.push(role),
        }
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
