//! Experiment gates (menu plan slice X, D7): a closed query by gate, answered by core from the
//! account's READY and later dispatches. `Unknown` means an input is missing or unreadable; the UI
//! shows the gated item only for `Enabled` (S-6).
use crate::SessionHandle;
use velella_core::{Gate, MenuGate};

/// The gates of experiments.md's table. `guild_id` of a query is the selected guild (nil in a DM).
#[derive(Clone, Copy, Debug, PartialEq, Eq, uniffi::Enum)]
pub enum ExperimentGate {
    /// Staff user flag; the caller also needs non-empty message content.
    Translate,
    /// Experiment and guild feature; the caller also needs a conversation for the message.
    ViewConversation,
    /// The guild's `moderator_reporting`: enabled with a report channel.
    ReportToModerators,
    /// Experiment and VERIFIED; the caller also needs MANAGE_OFFICIAL_MESSAGES.
    OfficialMessage,
    TidaWebform,
    /// Using it needs Nitro tier 2 (the caller's).
    BookmarksV1,
    BookmarksV2,
    /// The hover bar's `message_utilities`: v1 or v2.
    BookmarksVisible,
    ChannelObfuscation,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, uniffi::Enum)]
pub enum GateState {
    Enabled,
    Disabled,
    Unknown,
}

/// The v2 bookmark limits of a non-Nitro account (tier 2 gets 200 and 50).
#[derive(Clone, Copy, Debug, PartialEq, Eq, uniffi::Record)]
pub struct BookmarkLimits {
    pub bookmarks: u32,
    pub reminders: u32,
}

impl From<ExperimentGate> for MenuGate {
    fn from(g: ExperimentGate) -> Self {
        match g {
            ExperimentGate::Translate => Self::Translate,
            ExperimentGate::ViewConversation => Self::ViewConversation,
            ExperimentGate::ReportToModerators => Self::ReportToModerators,
            ExperimentGate::OfficialMessage => Self::OfficialMessage,
            ExperimentGate::TidaWebform => Self::TidaWebform,
            ExperimentGate::BookmarksV1 => Self::BookmarksV1,
            ExperimentGate::BookmarksV2 => Self::BookmarksV2,
            ExperimentGate::BookmarksVisible => Self::BookmarksVisible,
            ExperimentGate::ChannelObfuscation => Self::ChannelObfuscation,
        }
    }
}

impl From<Gate> for GateState {
    fn from(g: Gate) -> Self {
        match g {
            Gate::Enabled => Self::Enabled,
            Gate::Disabled => Self::Disabled,
            Gate::Unknown => Self::Unknown,
        }
    }
}

#[uniffi::export]
impl SessionHandle {
    /// Whether `gate` is on for the account (and `guild_id`, the selected guild).
    pub fn experiment_gate(&self, gate: ExperimentGate, guild_id: Option<u64>) -> GateState {
        self.0.snapshot().menu_gate(gate.into(), guild_id).into()
    }

    /// The v2 bookmark limits, when v2 is on for the account.
    pub fn bookmark_v2_limits(&self) -> Option<BookmarkLimits> {
        self.0
            .snapshot()
            .bookmark_v2_limits()
            .map(|l| BookmarkLimits {
                bookmarks: l.bookmarks,
                reminders: l.reminders,
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_gate_maps_to_its_core_gate_and_every_answer_to_its_state() {
        let pairs = [
            (ExperimentGate::Translate, MenuGate::Translate),
            (ExperimentGate::ViewConversation, MenuGate::ViewConversation),
            (
                ExperimentGate::ReportToModerators,
                MenuGate::ReportToModerators,
            ),
            (ExperimentGate::OfficialMessage, MenuGate::OfficialMessage),
            (ExperimentGate::TidaWebform, MenuGate::TidaWebform),
            (ExperimentGate::BookmarksV1, MenuGate::BookmarksV1),
            (ExperimentGate::BookmarksV2, MenuGate::BookmarksV2),
            (ExperimentGate::BookmarksVisible, MenuGate::BookmarksVisible),
            (
                ExperimentGate::ChannelObfuscation,
                MenuGate::ChannelObfuscation,
            ),
        ];
        for (ffi, core) in pairs {
            assert_eq!(MenuGate::from(ffi), core);
        }
        assert_eq!(GateState::from(Gate::Enabled), GateState::Enabled);
        assert_eq!(GateState::from(Gate::Disabled), GateState::Disabled);
        assert_eq!(GateState::from(Gate::Unknown), GateState::Unknown);
    }
}
