// Adapted from Swiftcord (https://github.com/SwiftcordApp/Swiftcord, GPL-3.0):
// Swiftcord/Views/Server/ServerFolder.swift, (c) Teddy Gaillard.
// Changes: DiscordKit GuildFolder/PreloadedGuild replaced by `RailFolder`/`RailGuild`; the
// luminanceToAlpha inverse-mask backdrop replaced by Swiftcord v2's GuildFolderBackground (a
// 54 pt panel, folder colour at 0.1, 1 pt separator border, continuous corner 18); folder tile and
// preview grid sized as v2's GuildRailRow / GuildRailArtwork (refs/swiftcord-v2-teardown.md, kept out of git);
// UserDefaults persistence and the hover popover dropped (open state is local, name is a
// `.help`); CachedAsyncImage/SwiftyGif thumbs replaced by `AvatarGlyph`.

import SwiftUI

private typealias T = DesignTokens

/// A folder in the rail: a tile showing a 2x2 preview that expands to its servers.
struct ServerFolder: View {
    let folder: RailFolder
    @Binding var selectedID: String?
    @State private var hovered = false
    @State private var open = false

    var body: some View {
        VStack(spacing: T.Size.railRowSpacing) {
            Button(folder.name ?? "Folder") {
                withAnimation(.interactiveSpring()) { open.toggle() }
            }
            .buttonStyle(ServerFolderButtonStyle(
                open: open, color: Color(folder.color),
                guilds: Array(folder.children.prefix(4)), hovered: hovered
            ))
            .onHover { hovered = $0 }
            .help(folder.name ?? "Folder")
            .frame(width: T.Size.railWidth)

            if open {
                ForEach(folder.children) { guild in
                    ServerButton(
                        selected: selectedID == guild.id, name: guild.name, icon: guild.icon,
                        hasUnread: guild.hasUnread, mentionCount: guild.mentionCount
                    ) { selectedID = guild.id }
                    .transition(.move(edge: .top).combined(with: .opacity))
                }
            }
        }
        // The last server sits as far inside the panel as the 46 pt icons sit inside its 54 pt width.
        .padding(.bottom, open ? (T.Size.folderPanelWidth - T.Size.railIcon) / 2 : 0)
        .background {
            let shape = RoundedRectangle(cornerRadius: T.Radius.folderPanel, style: .continuous)
            shape
                .fill(Color(folder.color).opacity(T.Palette.folderPanelTint))
                .overlay(shape.strokeBorder(T.Palette.folderPanelBorder, lineWidth: T.Size.folderPanelBorder))
                .frame(width: T.Size.folderPanelWidth)
                .opacity(open ? 1 : 0)
        }
    }
}

struct ServerFolderButtonStyle: ButtonStyle {
    let open: Bool
    let color: Color
    let guilds: [RailGuild]
    let hovered: Bool

    func makeBody(configuration: Configuration) -> some View {
        ZStack {
            if open {
                Image(systemName: "folder.fill")
                    .font(T.Typography.folderGlyph)
                    .foregroundStyle(color)
                    .transition(.move(edge: .top).combined(with: .opacity))
            } else {
                LazyVGrid(
                    columns: Array(
                        repeating: GridItem(.fixed(T.Size.folderPreviewIcon), spacing: T.Size.folderPreviewSpacing),
                        count: 2
                    ),
                    spacing: T.Size.folderPreviewSpacing
                ) {
                    ForEach(guilds) { MiniServerThumb(guild: $0) }
                }
                .foregroundStyle(hovered ? Color.white : Color.primary)
                .transition(.move(edge: .bottom).combined(with: .opacity))
                .frame(width: T.Size.railIcon, height: T.Size.railIcon, alignment: .topLeading)
            }
        }
        .frame(width: T.Size.railFolderTile, height: T.Size.railFolderTile)
        .background(
            open
            ? (hovered ? T.Palette.folderOpenFillHover : T.Palette.folderOpenFill)
            : color.opacity(hovered ? T.Palette.folderTintHover : T.Palette.folderTintIdle)
        )
        .clipShape(RoundedRectangle(cornerRadius: T.Radius.railIcon, style: .continuous))
        .offset(y: configuration.isPressed ? 1 : 0)
        .animation(.none, value: configuration.isPressed)
        .animation(T.Motion.railSpring, value: hovered)
    }
}

struct MiniServerThumb: View {
    let guild: RailGuild

    var body: some View {
        AvatarGlyph(avatar: guild.icon, initialsFont: T.Typography.folderThumbInitials)
            .frame(width: T.Size.folderPreviewIcon, height: T.Size.folderPreviewIcon)
            .background(T.Palette.folderThumbFill)
            .clipShape(Circle())
    }
}

#Preview("Folder") {
    ServerFolder(
        folder: RailFolder(id: "f", name: "Sample Folder", color: RGB(0.7, 0.45, 0.9), children: [
            RailGuild(id: "a", name: "Foo Bar", icon: .initials("FB")),
            RailGuild(id: "b", name: "Test Guild", icon: .initials("TG"), mentionCount: 3),
        ]),
        selectedID: .constant("a")
    )
    .padding()
}
