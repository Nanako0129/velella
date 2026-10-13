import Testing
@testable import VelellaUI

private let dmRail = RailItem.dm(RailDM(id: "dm", name: "Direct Messages", icon: .symbol("bubble.left.fill")))
private let guildRail = RailItem.guild(RailGuild(id: "10", name: "Guild", icon: .initials("G")))
private let otherGuildRail = RailItem.guild(RailGuild(id: "11", name: "Other", icon: .initials("O")))
private let sections = [MemberSection(id: "s", title: "Online", count: 1, members: [
    MemberRow(id: "1", name: "Ada", avatar: .initials("A")),
])]

private func shell(rail: String? = "10", channel: String? = "20") -> ShellState {
    ShellState(rail: [dmRail, guildRail, otherGuildRail], selectedRailID: rail, selectedChannelID: channel, selectedDMID: "500")
}

@Test func memberTargetIsTheSelectedGuildChannelAndNilForDMsOrNothing() {
    #expect(shell().memberTarget == MemberTarget(guild: 10, channel: 20))
    #expect(shell(rail: "dm").memberTarget == nil) // channel 20 stays selected underneath
    #expect(shell(channel: nil).memberTarget == nil)
    #expect(ShellState().memberTarget == nil)
}

@Test func aSnapshotForAnotherChannelOrAfterTheSelectionWasClearedLeavesTheSectionsAlone() {
    var state = shell()
    state.applyMembers(sections, guild: 10, channel: 20)
    #expect(state.memberSections == sections)
    state.applyMembers([], guild: 10, channel: 21)
    state.applyMembers([], guild: 11, channel: 20)
    #expect(state.memberSections == sections)
    state.memberSections = []
    state.selectedChannelID = nil
    state.applyMembers(sections, guild: 10, channel: 20)
    #expect(state.memberSections.isEmpty)
    state.selectedChannelID = "20"
    state.selectedRailID = "dm"
    state.applyMembers(sections, guild: 10, channel: 20)
    #expect(state.memberSections.isEmpty)
}

@Test func changingTheTargetEmptiesTheSectionsAndKeepingItDoesNot() {
    var state = shell()
    let target = state.memberTarget
    state.memberSections = sections
    let unchanged = !state.memberTargetChanged(from: target)
    #expect(unchanged)
    #expect(state.memberSections == sections)
    let changes: [(inout ShellState) -> Void] = [
        { $0.selectedChannelID = "21" }, // another channel
        { $0.selectedRailID = "dm" }, // a DM
        { $0.selectedRailID = "11" }, // another guild
        { $0.selectedChannelID = nil }, // nothing
    ]
    for change in changes {
        var next = state
        change(&next)
        let changed = next.memberTargetChanged(from: target)
        #expect(changed)
        #expect(next.memberSections.isEmpty)
    }
}
