import Foundation
import VelellaFFI
import VelellaUI

/// Follows the shell's (guild, channel) target and fills `ShellState.memberSections` from the
/// engine's member-list snapshots. Dropping `sub` stops the deliveries on the Rust side.
@MainActor
final class MembersController {
    private weak var model: AppModel?
    private var sub: MembersSubscription?
    private var target: MemberTarget?
    private var generation: UInt64?

    init(model: AppModel) { self.model = model }

    /// Called from `AppModel.selectionChanged()`: when the target changed, empties the list and
    /// subscribes the new one (nil target leaves the gateway list). The demo has no session.
    func retarget(handle: SessionHandle?, generation: UInt64?) {
        guard let model, !model.isDemo, model.shell.memberTargetChanged(from: target) else { return }
        let next = model.shell.memberTarget
        target = next
        sub = nil
        self.generation = generation
        guard let handle, let generation else { return }
        sub = handle.subscribeMembers(
            guildId: next?.guild, channelId: next?.channel,
            listener: MemberSnapshots(controller: self, generation: generation)
        )
    }

    /// Session over: late deliveries of the old subscription no longer match.
    func teardown() {
        sub = nil
        target = nil
        generation = nil
    }

    fileprivate func apply(
        _ sections: [MemberListSection], guild: UInt64, channel: UInt64, generation gen: UInt64
    ) {
        // The generation must be the live subscription's, and `applyMembers` checks the selected
        // channel, the same two guards as the S4 channel list.
        guard gen == generation, let model else { return }
        model.shell.applyMembers(sections.map(Self.section), guild: guild, channel: channel)
    }

    private static func section(_ s: MemberListSection) -> MemberSection {
        MemberSection(id: s.id, title: s.title, count: Int(clamping: s.count), members: s.members.map(row))
    }

    private static func row(_ m: MemberListEntry) -> MemberRow {
        MemberRow(
            id: String(m.id), name: m.name,
            avatar: m.avatarUrl.flatMap { URL(string: $0) }.map(Avatar.url) ?? .initials(initials(of: m.name)),
            roleColor: m.color.map { RGB(rgb: $0) }, status: status(m.status), customStatus: m.customStatus
        )
    }

    private static func status(_ s: MemberListStatus) -> PresenceStatus {
        switch s {
        case .online: .online
        case .idle: .idle
        case .doNotDisturb: .doNotDisturb
        case .offline: .offline
        }
    }
}

/// Member-list callbacks arrive on `velella-rt` threads; each goes to the main queue in order,
/// tagged with the generation it was subscribed under and the (guild, channel) it was made for.
private final class MemberSnapshots: MembersListener {
    let controller: MembersController
    let generation: UInt64
    init(controller: MembersController, generation: UInt64) {
        self.controller = controller; self.generation = generation
    }

    func onMembers(guildId: UInt64, channelId: UInt64, sections: [MemberListSection]) {
        DispatchQueue.main.async { [controller, generation] in
            MainActor.assumeIsolated {
                controller.apply(sections, guild: guildId, channel: channelId, generation: generation)
            }
        }
    }
}
