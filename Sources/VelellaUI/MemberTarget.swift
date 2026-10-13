import Foundation

/// The guild channel whose member list is showing.
public struct MemberTarget: Hashable, Sendable {
    public var guild: UInt64
    public var channel: UInt64
    public init(guild: UInt64, channel: UInt64) { self.guild = guild; self.channel = channel }
}

extension ShellState {
    /// The (guild, channel) the member list follows: the selected channel of the selected guild.
    /// nil in DM mode or with no channel selected (a DM has no gateway member list).
    public var memberTarget: MemberTarget? {
        guard !isDMMode, let guild = selectedRailID.flatMap({ UInt64($0) }),
              let channel = selectedChannelID.flatMap({ UInt64($0) }) else { return nil }
        return MemberTarget(guild: guild, channel: channel)
    }

    /// Empties the member list when the target is no longer `old`; true when it changed.
    public mutating func memberTargetChanged(from old: MemberTarget?) -> Bool {
        guard memberTarget != old else { return false }
        memberSections = []
        return true
    }

    /// Applies a member snapshot made for (`guild`, `channel`) only while that is still the
    /// selected target; otherwise `memberSections` is left unchanged.
    public mutating func applyMembers(_ sections: [MemberSection], guild: UInt64, channel: UInt64) {
        guard memberTarget == MemberTarget(guild: guild, channel: channel) else { return }
        memberSections = sections
    }
}
