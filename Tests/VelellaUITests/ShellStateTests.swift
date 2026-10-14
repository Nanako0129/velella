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

@Test func eachConversationKeepsItsOwnDraft() {
    var state = ShellState(rail: [dmRail, guildRail], selectedRailID: "g", selectedChannelID: "X", selectedDMID: "D")
    state.draft = "for X"
    // Channel X → DM D: D's composer starts empty, X's text stays with X.
    state.selectedRailID = "dm"
    #expect(state.draft == "")
    state.draft = "for D"
    state.selectedRailID = "g"
    #expect(state.draft == "for X")
    // A sent (cleared) draft leaves no entry; nothing open writes nothing.
    state.draft = ""
    state.selectedChannelID = nil
    state.draft = "lost"
    #expect(state.drafts == ["D": "for D"])
}

@Test func messageLoadStartsIdleAndFixturesKeepIt() {
    #expect(ShellState().messageLoad == .idle)
    #expect(ShellState.demo.messageLoad == .idle)
}
