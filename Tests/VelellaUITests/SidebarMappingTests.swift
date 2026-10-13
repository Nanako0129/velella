import Testing
@testable import VelellaUI

@Test func initialsTakeTheFirstLetterOfUpToThreeWords() {
    #expect(initials(of: "Lorem Ipsum Club") == "LIC")
    #expect(initials(of: "lorem ipsum club dolor") == "LIC")
    #expect(initials(of: "  spaced   out\tname ") == "SON")
    #expect(initials(of: "Solo") == "S")
    #expect(initials(of: "Unknown server") == "US")
    #expect(initials(of: "") == "")
}

@Test func folderColorIntegersBecomeRGB() {
    #expect(RGB(rgb: 0xFF0000) == RGB(1, 0, 0))
    #expect(RGB(rgb: 0x000080) == RGB(0, 0, 128.0 / 255))
    #expect(RGB.defaultFolder == RGB(rgb: 0x5865F2))
}

private func guild(_ id: String, unread: Bool = false, mentions: Int = 0, muted: Bool = false) -> RailGuild {
    RailGuild(id: id, name: id, icon: .initials("G"), hasUnread: unread, mentionCount: mentions, isMuted: muted)
}

private func dmEntry(_ id: String, unread: Bool = false, mentions: Int = 0) -> DMEntry {
    DMEntry(id: id, name: id, avatar: .initials("D"), unread: unread, mentionCount: mentions)
}

private func dmRail(_ state: ShellState) -> RailDM? {
    for case .dm(let dm) in state.rail { return dm }
    return nil
}

@Test func closedFolderAggregatesItsChildren() {
    let folder = RailFolder(id: "f", color: .defaultFolder, children: [
        guild("a", unread: true, mentions: 2), guild("b", unread: true, mentions: 3), guild("c"),
    ])
    #expect(folder.mentionCount == 5)
    #expect(folder.hasUnread)
    let quiet = RailFolder(id: "q", color: .defaultFolder, children: [guild("a"), guild("b")])
    #expect(quiet.mentionCount == 0)
    #expect(!quiet.hasUnread)
    // Unread without a mention still marks the folder.
    #expect(RailFolder(id: "u", color: .defaultFolder, children: [guild("a", unread: true)]).hasUnread)
}

@Test func friendsRowShowsPendingRequests() {
    var state = ShellState()
    state.applyDMs([], friendsPending: 4)
    #expect(state.dmTopRows.map(\.title) == ["Friends"])
    #expect(state.dmTopRows.first?.badgeCount == 4)
    state.applyDMs([], friendsPending: 0)
    #expect(state.dmTopRows.first?.badgeCount == 0)
}

@Test func railDirectMessagesTotalSurvivesAGuildUpdate() {
    var state = ShellState()
    state.applyGuildRail([.guild(guild("g1"))])
    // The engine reports the muted DM with no unread and no mentions, so it adds nothing.
    state.applyDMs([dmEntry("d1", unread: true, mentions: 2), dmEntry("d2-muted"), dmEntry("d3", unread: true, mentions: 1)], friendsPending: 0)
    #expect(dmRail(state)?.mentionCount == 3)
    #expect(dmRail(state)?.hasUnread == true)
    // A guild snapshot arrives: the Direct Messages entry keeps its counts and its place.
    state.applyGuildRail([.guild(guild("g1")), .guild(guild("g2", mentions: 7))])
    #expect(dmRail(state)?.mentionCount == 3)
    #expect(state.rail.map(\.id) == ["dm", "g1", "g2"])
    // And the DM snapshot after it leaves the guilds alone.
    state.applyDMs([dmEntry("d1")], friendsPending: 0)
    #expect(dmRail(state)?.mentionCount == 0)
    #expect(dmRail(state)?.hasUnread == false)
    #expect(state.rail.map(\.id) == ["dm", "g1", "g2"])
}

@Test func aVanishedDMLosesItsSelection() {
    var state = ShellState(selectedDMID: "gone")
    state.applyDMs([dmEntry("d1")], friendsPending: 0)
    #expect(state.selectedDMID == nil)
}

/// ServerRail (top-level servers) and ServerFolder (folder children) both build their buttons with
/// `ServerButton(guild:selected:onSelect:)`, so this is where a muted server becomes a muted button.
/// The two call sites themselves cannot be observed from a unit test.
@Test func mutedServersReachServerButtonAsMuted() {
    let top = RailItem.guild(guild("top", muted: true))
    let folder = RailFolder(id: "f", color: .defaultFolder, children: [guild("kid", muted: true), guild("loud")])
    guard case .guild(let topGuild) = top else { return }
    #expect(ServerButton(guild: topGuild, selected: false) {}.isMuted)
    #expect(ServerButton(guild: folder.children[0], selected: false) {}.isMuted)
    #expect(!ServerButton(guild: folder.children[1], selected: false) {}.isMuted)
    let busy = ServerButton(guild: guild("x", unread: true, mentions: 4), selected: true) {}
    #expect(busy.hasUnread && busy.mentionCount == 4 && busy.selected)
}
