//! VIEW_CHANNEL decision. Algorithm pattern adapted from Serein's `effective`
//! (refs/serein/crates/model/src/permissions.rs, MIT OR Apache-2.0): owner/ADMINISTRATOR see all;
//! otherwise @everyone | member roles, then overwrites @everyone -> roles -> member. `None` means
//! the data needed to decide is missing, never "denied".
use crate::state::{Channel, Guild};

const ADMINISTRATOR: u64 = 1 << 3;
const VIEW_CHANNEL: u64 = 1 << 10;

pub(crate) fn can_view_channel(user_id: u64, guild: &Guild, channel: &Channel) -> Option<bool> {
    if guild.owner_id == Some(user_id) {
        return Some(true);
    }
    if guild.unavailable {
        return None;
    }
    let roles = guild.roles.as_ref()?;
    let member_roles = guild.member_roles.as_ref()?;
    let perms = |id: u64| roles.iter().find(|r| r.id == id).map(|r| r.permissions);
    // The @everyone role has the guild's id.
    let mut bits = perms(guild.id)?;
    for id in member_roles.iter().filter(|id| **id != guild.id) {
        bits |= perms(*id)?;
    }
    if bits & ADMINISTRATOR != 0 {
        return Some(true);
    }
    // Without the owner identity a non-administrator may still be the owner.
    guild.owner_id?;
    let overwrites = channel.permission_overwrites.as_ref()?;
    let (mut role_allow, mut role_deny) = (0, 0);
    let (mut everyone, mut own) = (None, None);
    for o in overwrites {
        match o.kind {
            0 if o.id == guild.id => everyone = Some(o),
            0 if member_roles.contains(&o.id) => {
                role_allow |= o.allow;
                role_deny |= o.deny;
            }
            1 if o.id == user_id => own = Some(o),
            _ => {}
        }
    }
    if let Some(o) = everyone {
        bits = (bits & !o.deny) | o.allow;
    }
    bits = (bits & !role_deny) | role_allow;
    if let Some(o) = own {
        bits = (bits & !o.deny) | o.allow;
    }
    Some(bits & VIEW_CHANNEL != 0)
}
