import Testing
@testable import VelellaUI

private let dmRail = RailItem.dm(RailDM(id: "dm", name: "Direct Messages", icon: .symbol("bubble.left.fill")))
private let guildRail = RailItem.guild(RailGuild(id: "g", name: "Guild", icon: .initials("G")))

@Test func openTargetIsTheChannelOutsideDMModeAndTheDMInside() {
    var state = ShellState(rail: [dmRail, guildRail], selectedRailID: "g", selectedChannelID: "X", selectedDMID: "D")
    #expect(state.openTarget == "X")
    // Channel X stays selected underneath, but the DM rail entry opens D.
    state.selectedRailID = "dm"
    #expect(state.openTarget == "D")
    state.selectedDMID = nil
    #expect(state.openTarget == nil)
    state.selectedRailID = "g"
    #expect(state.openTarget == "X")
    state.selectedChannelID = nil
    #expect(state.openTarget == nil)
}

@Test func messageLoadStartsIdleAndFixturesKeepIt() {
    #expect(ShellState().messageLoad == .idle)
    #expect(ShellState.demo.messageLoad == .idle)
}
