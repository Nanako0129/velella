// Adapted from Swiftcord (https://github.com/SwiftcordApp/Swiftcord, GPL-3.0):
// the server-list half of Swiftcord/Views/ContentView.swift, (c) Vincent Kwok.
// Changes: gateway cache, folder merging, onboarding sheets and the add-server button dropped;
// items come from `RailItem`; the AppKit VisualEffect background removed (Swiftcord v2's rail
// has no panel of its own); width and spacing follow v2 (refs/swiftcord-v2-teardown.md, kept out of git).

import SwiftUI

private typealias T = DesignTokens

struct ServerRail: View {
    let items: [RailItem]
    @Binding var selection: String?

    var body: some View {
        ScrollView(showsIndicators: false) {
            LazyVStack(spacing: T.Size.railRowSpacing) {
                ForEach(items) { item in
                    switch item {
                    case .dm(let dm):
                        ServerButton(
                            selected: selection == dm.id, name: dm.name, icon: dm.icon,
                            hasUnread: dm.hasUnread, mentionCount: dm.mentionCount
                        ) { selection = dm.id }
                        Divider().frame(width: T.Size.railDivider)
                    case .guild(let guild):
                        ServerButton(guild: guild, selected: selection == guild.id) { selection = guild.id }
                    case .folder(let folder):
                        ServerFolder(folder: folder, selectedID: $selection)
                    }
                }
            }
            .padding(.top, T.Size.railTopInset)
            .padding(.bottom, T.Spacing.md)
        }
        // Measured from the window top, not the toolbar: v2's first icon starts just under the
        // traffic lights, inside the toolbar's height.
        .ignoresSafeArea(.container, edges: .top)
        .frame(width: T.Size.railWidth)
        .frame(maxHeight: .infinity, alignment: .top)
    }
}

#Preview("Rail") {
    ServerRail(items: Fixtures.rail, selection: .constant("g-lorem"))
        .frame(height: 560)
        .padding()
}
