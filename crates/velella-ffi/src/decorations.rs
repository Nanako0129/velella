//! FFI records for the sidebar's per-channel and per-DM extras (slice D). The core types are
//! UI-agnostic; these mirror them field for field for the generated Swift bindings.

#[derive(uniffi::Record)]
pub struct SidebarOccupant {
    pub id: u64,
    pub name: String,
    pub icon_url: Option<String>,
}

#[derive(uniffi::Record)]
pub struct SidebarThread {
    pub id: u64,
    pub name: String,
    pub unread: bool,
    pub mention_count: u32,
    pub muted: bool,
}

#[derive(uniffi::Enum)]
pub enum SidebarStatus {
    Online,
    Idle,
    DoNotDisturb,
    Offline,
}

impl From<velella_core::SidebarOccupant> for SidebarOccupant {
    fn from(o: velella_core::SidebarOccupant) -> Self {
        Self {
            id: o.id,
            name: o.name,
            icon_url: o.icon_url,
        }
    }
}

impl From<velella_core::SidebarThread> for SidebarThread {
    fn from(t: velella_core::SidebarThread) -> Self {
        Self {
            id: t.id,
            name: t.name,
            unread: t.unread,
            mention_count: t.mention_count,
            muted: t.muted,
        }
    }
}

impl From<velella_core::PresenceStatus> for SidebarStatus {
    fn from(s: velella_core::PresenceStatus) -> Self {
        use velella_core::PresenceStatus as P;
        match s {
            P::Online => Self::Online,
            P::Idle => Self::Idle,
            P::DoNotDisturb => Self::DoNotDisturb,
            P::Offline => Self::Offline,
        }
    }
}
