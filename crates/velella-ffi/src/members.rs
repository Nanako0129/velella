//! Member-list records and `SessionHandle::subscribe_members`.
use crate::{runtime, SessionHandle};
use std::sync::Arc;

/// Member-list callbacks run on a `velella-rt` thread and must not block.
#[uniffi::export(with_foreign)]
pub trait MembersListener: Send + Sync {
    /// Every delivery names the selection it was made for, so a late one from a dropped
    /// subscription can be told apart. Empty until a list is adopted for that selection.
    fn on_members(&self, guild_id: u64, channel_id: u64, sections: Vec<MemberListSection>);
}

#[derive(uniffi::Enum)]
pub enum MemberListStatus {
    Online,
    Idle,
    DoNotDisturb,
    Offline,
}

#[derive(uniffi::Record)]
pub struct MemberListEntry {
    pub id: u64,
    pub name: String,
    pub avatar_url: Option<String>,
    /// RGB integer (0xRRGGBB).
    pub color: Option<u32>,
    pub status: MemberListStatus,
    pub custom_status: Option<String>,
}

#[derive(uniffi::Record)]
pub struct MemberListSection {
    pub id: String,
    pub title: String,
    pub count: u64,
    pub members: Vec<MemberListEntry>,
}

impl From<velella_core::MemberListSection> for MemberListSection {
    fn from(s: velella_core::MemberListSection) -> Self {
        Self {
            id: s.id,
            title: s.title,
            count: s.count,
            members: s
                .members
                .into_iter()
                .map(|m| MemberListEntry {
                    id: m.id,
                    name: m.name,
                    avatar_url: m.avatar_url,
                    color: m.color,
                    status: match m.status {
                        velella_core::MemberListStatus::Online => MemberListStatus::Online,
                        velella_core::MemberListStatus::Idle => MemberListStatus::Idle,
                        velella_core::MemberListStatus::DoNotDisturb => {
                            MemberListStatus::DoNotDisturb
                        }
                        velella_core::MemberListStatus::Offline => MemberListStatus::Offline,
                    },
                    custom_status: m.custom_status,
                })
                .collect(),
        }
    }
}

/// Holds the delivery of a member list (nothing when the selection was cleared); dropping it
/// stops the deliveries.
#[derive(uniffi::Object)]
pub struct MembersSubscription(#[allow(dead_code)] Option<velella_core::Subscription>);

#[uniffi::export]
impl SessionHandle {
    /// Selects the member list of a guild channel and delivers its snapshots. Either id `None`
    /// (a DM, or nothing selected) leaves the list: the gateway subscription is dropped and
    /// nothing is delivered, so the caller shows an empty list.
    pub fn subscribe_members(
        &self,
        guild_id: Option<u64>,
        channel_id: Option<u64>,
        listener: Arc<dyn MembersListener>,
    ) -> Arc<MembersSubscription> {
        let _context = runtime().enter();
        let (Some(guild_id), Some(channel_id)) = (guild_id, channel_id) else {
            self.0.set_member_subscription(None);
            return Arc::new(MembersSubscription(None));
        };
        Arc::new(MembersSubscription(Some(self.0.subscribe_members(
            guild_id,
            channel_id,
            move |sections| {
                listener.on_members(
                    guild_id,
                    channel_id,
                    sections.into_iter().map(Into::into).collect(),
                )
            },
        ))))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn core_sections_convert_field_for_field() {
        let section = MemberListSection::from(velella_core::MemberListSection {
            id: "group-online".into(),
            title: "Online".into(),
            count: 7,
            members: vec![velella_core::MemberListEntry {
                id: 5,
                name: "n".into(),
                avatar_url: Some("u".into()),
                color: Some(0xff0000),
                status: velella_core::MemberListStatus::DoNotDisturb,
                custom_status: Some("busy".into()),
            }],
        });
        let m = &section.members[0];
        assert_eq!((section.id.as_str(), section.count), ("group-online", 7));
        assert_eq!(
            (m.id, m.color, m.custom_status.as_deref()),
            (5, Some(0xff0000), Some("busy"))
        );
        assert!(matches!(m.status, MemberListStatus::DoNotDisturb));
    }
}
