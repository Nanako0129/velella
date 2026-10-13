import Foundation

/// Pure helpers for filling `ShellState` from engine snapshots (the mapping itself lives in the app,
/// which is the only target that sees FFI types).

/// First character of each whitespace-separated word, at most three, uppercased.
public func initials(of name: String) -> String {
    String(name.split(whereSeparator: \.isWhitespace).prefix(3).compactMap(\.first)).uppercased()
}

extension RGB {
    /// Discord's 0xRRGGBB integer (folder colors).
    public init(rgb: UInt32) {
        self.init(
            Double((rgb >> 16) & 0xFF) / 255, Double((rgb >> 8) & 0xFF) / 255, Double(rgb & 0xFF) / 255
        )
    }

    /// Folder color when the account set none: Discord's default blurple (0x5865F2).
    public static let defaultFolder = RGB(rgb: 0x5865F2)
}

extension RailDM {
    /// The Direct Messages rail entry before any DM is known.
    public static let directMessages = RailDM(id: "dm", name: "Direct Messages", icon: .symbol("bubble.left.fill"))
}

extension DMTopRow {
    /// The Friends row; its badge is the number of incoming friend requests.
    public static func friends(pending: Int) -> DMTopRow {
        DMTopRow(id: "dm-friends", title: "Friends", symbol: "person.2.fill", badgeCount: pending)
    }
}

extension ShellState {
    private var dmRailEntry: RailDM {
        for case .dm(let dm) in rail { return dm }
        return .directMessages
    }

    /// Replaces the servers on the rail. The Direct Messages entry stays first with its counts: the
    /// engine's guild snapshots know nothing about DMs, so they must not rebuild it.
    public mutating func applyGuildRail(_ guilds: [RailItem]) {
        rail = [.dm(dmRailEntry)] + guilds
    }

    /// Applies a DM snapshot: the list, the Friends row, and the rail's Direct Messages totals (the
    /// sum over the DMs; the engine reports a muted DM as read, so it adds nothing).
    public mutating func applyDMs(_ entries: [DMEntry], friendsPending: Int) {
        dms = entries
        dmTopRows = [.friends(pending: friendsPending)]
        var entry = dmRailEntry
        entry.mentionCount = entries.reduce(0) { $0 + $1.mentionCount }
        entry.hasUnread = entries.contains { $0.unread }
        let others = rail.filter { if case .dm = $0 { false } else { true } }
        rail = [.dm(entry)] + others
        // A DM that vanished (closed, left) cannot stay selected.
        if let selected = selectedDMID, !entries.contains(where: { $0.id == selected }) {
            selectedDMID = nil
        }
    }
}
