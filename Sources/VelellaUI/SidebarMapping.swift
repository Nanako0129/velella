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
