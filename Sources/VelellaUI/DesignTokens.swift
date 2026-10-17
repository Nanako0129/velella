import AppKit
import SwiftUI

/// Every visual parameter of the shell lives here. Views reference these names and never
/// hold a literal size, radius, font or colour of their own, so visual review rounds edit
/// this one file. Colours are semantic (primary/secondary/system tints) so light and dark
/// appearance both follow the system instead of two hand-tuned palettes.
///
/// Contrast rule (user decision 2026-10-13, deviating from Swiftcord v2, whose tinted profile
/// popover fades secondary text until it is unreadable): text on any tinted or glass background
/// must reach WCAG AA, 4.5:1. Over system glass, use the semantic label colours (primary,
/// secondary), which the system keeps legible as the backdrop changes. Over a fixed tint that
/// velella chooses itself, pick the text colour from that tint's luminance with
/// `ContrastText.color(on:)`; never lower an opacity until the text fades.
enum DesignTokens {
    enum Spacing {
        /// Tightest gap: icon to label inside a row.
        static let xs: CGFloat = 4
        /// Gap between stacked rows and between an avatar and its text.
        static let sm: CGFloat = 8
        /// Inner padding of cards, footers and the composer.
        static let md: CGFloat = 12
        /// Outer gutter of columns.
        static let lg: CGFloat = 16
    }

    enum Radius {
        /// Rail icons and the folder tile: continuous corner 14 at 30 pt and wider (v2
        /// GuildRailIcon / GuildRailArtwork, refs/swiftcord-v2-teardown.md, kept out of git).
        static let railIcon: CGFloat = 14
        /// Open folder panel: continuous corner 18 (v2 GuildFolderBackground).
        static let folderPanel: CGFloat = 18
        /// Channel and DM row selection background: 8 (v2 SidebarRow background layer; the DM
        /// highlight in a-main-dark-dm-1x.png measures the same ~8).
        static let row: CGFloat = 8
        /// Cut-out around a rail mention badge, and around a private channel's lock (v2).
        static let railBadgeCutout: CGFloat = 10
        static let channelLockCutout: CGFloat = 2
        /// Member section cards: corner ~12 measured on Swiftcord v2's member list (user's capture
        /// 2026-10-14).
        static let memberCard: CGFloat = 12
        /// The user footer floats at the bottom of the channel column as a glass card; k-footer-card.png
        /// shows a corner of roughly 12-14pt, clearly squarer than a capsule.
        static let footer: CGFloat = 14
        /// Composer card: 16, continuous. Swiftcord v2's composer corner measures ~15-16 pt (user's
        /// capture, 2026-10-13), and v2 falls back to RoundedRectangle(cornerRadius: 16,
        /// .continuous) where ConcentricRectangle is unavailable (that this is the composer is
        /// inferred). The card is the container shape its "+" button and reply bar follow.
        static let composer: CGFloat = 16
        /// Smallest corner of the "+" button and the reply bar, which are ConcentricRectangles of
        /// the composer: 16 - 8 padding = 8, the ~8 pt v2's "+" measures.
        static let composerInnerMinimum: CGFloat = 8
        /// Rows of the "+" menu and their hover highlight.
        static let plusMenuRow: CGFloat = 12
        /// Server-tag pill next to a DM name.
        static let tagPill: CGFloat = 6
        /// Images under messages.
        static let messageImage: CGFloat = 8
    }

    enum Size {
        /// Rail column (v2 GuildsColumn, 82), and the 1 pt line between it and the channel list:
        /// v2's sidebar column spans 283-403 = 82 + 1 + 200...320 (refs/swiftcord-v2-teardown.md, kept out of git).
        static let railWidth: CGFloat = 82
        static let railColumnDivider: CGFloat = 1
        /// Server icon edge, and the larger tile a folder gets (v2 GuildRailRow: 46 or 54).
        static let railIcon: CGFloat = 46
        static let railFolderTile: CGFloat = 54
        /// Gap between rail rows; v2's rows sit 54 pt apart in a-main-dark-dm-1x.png (46 + 8). The
        /// rows live in SwiftUI there, so this one is measured from the screen, not the binary.
        static let railRowSpacing: CGFloat = 8
        /// Rail mention badge: 16 tall, inside the icon's bottom-trailing corner, with the icon cut
        /// away 2 pt around it (v2 GuildMentionBadge).
        static let railBadgeHeight: CGFloat = 16
        static let railBadgeCutout: CGFloat = 2
        /// Window top to the first rail icon. a-main-dark-dm-1x.png puts that icon's top 19 pt below
        /// the traffic lights' centre; ours sit at 25 pt, hence 44 (was 64 inside the safe area).
        static let railTopInset: CGFloat = 44
        /// Rail indicator (v2 GuildIndicator): 8 wide at x = -4, so half shows; hidden it slides
        /// to x = -6 and fades. Heights: selected 46, hover 23, unread 8.
        static let indicatorWidth: CGFloat = 8
        static let indicatorShownX: CGFloat = -4
        static let indicatorHiddenX: CGFloat = -6
        /// Sidebar width the toolbar title must leave for the traffic lights, the column button and
        /// the system sidebar toggle; the title truncates to the rest. The toolbar never shrinks
        /// an item, it drops whatever does not fit (Swiftcord legacy caps its title for the same
        /// reason, "Largest width before disappearing"). Measured 2026-10-13 (macOS 27.0.1): the
        /// title starts at 96 pt and may end 195 pt before the sidebar's trailing edge; a wider
        /// title dropped the column button at the minimum sidebar width.
        static let sidebarToolbarReserved: CGFloat = 200
        /// Open folder panel: as wide as the folder tile, so 4 pt either side of the 46 pt icons.
        static let folderPanelWidth: CGFloat = 54
        static let folderPanelBorder: CGFloat = 1
        static let indicatorSelectedHeight: CGFloat = 46
        static let indicatorHoverHeight: CGFloat = 23
        static let indicatorRestHeight: CGFloat = 8
        /// Collapsed folder preview: 2x2 thumbs of 22 at a 24 pitch, filling the 46 pt icon (v2).
        static let folderPreviewIcon: CGFloat = 22
        static let folderPreviewSpacing: CGFloat = 2
        /// Divider under the DM entry; narrower than the icons.
        static let railDivider: CGFloat = 32
        /// Channel row geometry in row coordinates (v2 SidebarRow): the selection background is
        /// inset 12 at both sides, the kind glyph box spans x 20-40, the name starts at 48 and ends
        /// 20 before the trailing edge (badge + 8 before it when a badge shows).
        static let channelSelectionInset: CGFloat = 12
        static let channelIconLeading: CGFloat = 20
        static let channelIcon: CGFloat = 20
        /// The glyph inside that box: v2's 20 pt UIImageView draws the symbol at about 16x17 pt
        /// (user's 2x capture); a resizable SwiftUI symbol has no image padding, so it is fit to 17.
        static let channelGlyph: CGFloat = 17
        static let channelTitleLeading: CGFloat = 48
        static let channelTrailing: CGFloat = 20
        static let channelBadgeGap: CGFloat = 8
        /// Private channel: 8 pt lock in the glyph box's top-trailing corner; the glyph is cut away
        /// 1 pt around it (v2 SidebarRow lock / iconMask).
        static let channelLock: CGFloat = 8
        static let channelLockKnockout: CGFloat = 1
        /// Thread tree line in the glyph box (v2: x 29 in the row, elbow radius 4, ends at x 40;
        /// 1.5 pt butt-capped stroke).
        static let threadSpineX: CGFloat = 9
        static let threadElbowRadius: CGFloat = 4
        static let threadLineWidth: CGFloat = 1.5
        /// Channel and category rows are 32 pt apart in c-channels-guild.png (SwiftUI-level in v2,
        /// so measured from the screen).
        static let channelRowHeight: CGFloat = 32
        /// Category header text starts at x 20 in the row (v2 SidebarRow header mode).
        static let categoryHeaderLeading: CGFloat = 20
        /// Channel mention badge: 18 tall, at least as wide, text + 8 (v2); unread is a 6 pt dot
        /// at the same trailing position.
        static let channelBadgeHeight: CGFloat = 18
        static let channelBadgePadding: CGFloat = 4
        static let channelUnreadDot: CGFloat = 6
        /// Voice occupants are secondary information, hence small; they start in the name column.
        static let occupantAvatar: CGFloat = 20
        /// DM rows: v2 SidebarRow's avatar mode puts a 32 pt avatar at x 18 and the name at 62.
        /// a-main-dark-dm-1x.png: rows (activity line or not) 40 pt apart, the selected highlight
        /// 38 pt tall; the "Direct Messages" header 32 pt from the row centres around it.
        static let dmAvatar: CGFloat = 32
        static let dmAvatarLeading: CGFloat = 18
        static let dmTitleLeading: CGFloat = 62
        static let dmRowHeight: CGFloat = 38
        static let dmRowGap: CGFloat = 2
        static let dmSectionHeaderHeight: CGFloat = 24
        /// DM top rows (Friends, ...) use a larger glyph than channel rows.
        static let dmTopIcon: CGFloat = 28
        static let messageAvatar: CGFloat = 40
        static let memberAvatar: CGFloat = 32
        /// Member rows (v2 DiscordUI.MemberView layout, refs/swiftcord-v2-teardown.md, kept out of
        /// git): 44 tall, the avatar 16 in from the leading edge, the text 12 after it (x 60) and
        /// 16 clear of the trailing edge. Separators start at the text, as in the v2 capture.
        static let memberRowHeight: CGFloat = 44
        static let memberRowInset: CGFloat = 16
        static let memberAvatarGap: CGFloat = 12
        /// Name and status line boxes: 20 and 15 tall, the status starting 1 above the name's
        /// bottom (v2 MemberView: name at y 5, status at y 24).
        static let memberNameLine: CGFloat = 20
        static let memberStatusLine: CGFloat = 15
        static let memberStatusOverlap: CGFloat = 1
        /// Presence on member avatars (v2 MemberPresenceBadge): the dot's centre sits at 45° on
        /// the avatar's rim, and the avatar is cut away 1.6 × the dot around it, so the gap shows
        /// the card behind the row instead of a ring in the window colour.
        static let memberPresenceCentre: CGFloat = (1 + 0.5.squareRoot()) / 2
        static let memberPresenceCutout: CGFloat = 1.6
        /// 12 above and below a member section header: 40 between cards in the v2 capture.
        static let memberSectionHeaderPadding: CGFloat = 12
        static let footerAvatar: CGFloat = 32
        /// Footer height: k-footer-card.png and a-main-dark-dm-1x.png show a card of about 44pt (legacy 52).
        static let footerHeight: CGFloat = 44
        /// Filled gear in the footer, ~16-17pt tall in k-footer-card.png (legacy 18).
        static let footerGear: CGFloat = 16
        /// Footer presence dot: smaller than the shared one, with a hairline instead of the 2pt
        /// window-colour ring (k-footer-card.png shows a plain green dot).
        static let footerStatusDot: CGFloat = 8
        static let footerStatusDotRingWidth: CGFloat = 1
        /// Light rim around the footer glass card (k-footer-card.png).
        static let footerRimWidth: CGFloat = 1
        /// Presence dot on avatars, its separating ring and how far it hangs off the corner.
        static let statusDot: CGFloat = 10
        static let statusDotRingWidth: CGFloat = 2
        static let statusDotOffset: CGFloat = 2
        /// Mention counter in DM top rows: never narrower than a circle, ringed in the window colour.
        static let mentionBadgeMin: CGFloat = 18
        static let mentionBadgeRingWidth: CGFloat = 2
        /// Composer: one comfortable line, with a square "+" inside it. The button measures ~34pt in
        /// g-composer-plus-button.png.
        static let composerHeight: CGFloat = 52
        static let composerPlus: CGFloat = 34
        /// The input row's height at one line: the card's `composerHeight` less its 8 pt padding on
        /// both sides. The "+" and the glyphs each take this height, so aligned to the last line
        /// of a multi-line input (D1) they sit exactly where they sat centred on the single line.
        static let composerRow: CGFloat = composerHeight - 2 * Spacing.sm
        /// Editor insets above and below the text: (`composerRow` - the 13 pt body's ~16 pt line) / 2,
        /// so a single line is centred in `composerRow` as the old text field was, and the
        /// placeholder aligns to the same top.
        static let composerInputInset: CGFloat = 10
        /// The editor's height limit (plan D2, Swiftcord v2): `composerMaxHeight`, or half the
        /// window's safe-area height when that is smaller; `composerNoWindowHeight` stands in for
        /// that half before the view has a window.
        static let composerMaxHeight: CGFloat = 320
        static let composerMaxWindowFraction: CGFloat = 0.5
        static let composerNoWindowHeight: CGFloat = 160
        /// One line of the character counter; the input reserves at least it (the upsell line adds its own)
        /// while the counter shows, as the official `.charCountShowing` does.
        static let composerCounterLine: CGFloat = 16
        /// Close button of the reply bar and the custom-emoji image in its preview.
        static let replyEmoji: CGFloat = 16
        /// "+" menu: wide Discord-style rows rather than a narrow native menu.
        /// Round 1 (2026-10-13): the user found 300/48/24 too big; scaled toward Discord's
        /// compact "+" menu while keeping the wide, icon-led rows.
        static let plusMenuWidth: CGFloat = 232
        static let plusMenuRowHeight: CGFloat = 36
        static let plusMenuIcon: CGFloat = 18
        /// Channel list column; the sidebar adds the rail and its divider on top. Min and max are
        /// v2's (200-320); v2 restores the user's last width as the ideal, ours is a default.
        static let channelColumnMin: CGFloat = 200
        static let channelColumnIdeal: CGFloat = 260
        static let channelColumnMax: CGFloat = 320
        /// Member list: fixed, as in v2 (not resizable there). 256 is v2's fixed channel-inspector
        /// width (refs/swiftcord-v2-teardown.md, kept out of git); that this case is the member
        /// list is inferred, pending a v2 screenshot.
        static let memberColumnWidth: CGFloat = 256
        /// Smallest message column that still reads as a chat.
        static let detailMinWidth: CGFloat = 480
        /// Widest sidebar + narrowest message column + member list: constant, so dragging the
        /// sidebar divider never needs a wider window (as in Swiftcord v2). It must stay above
        /// 2 × widest sidebar + member list (1062), the most SwiftUI's inspector sizing asks for
        /// (see ShellView).
        static let windowMinWidth: CGFloat =
            railWidth + railColumnDivider + channelColumnMax + detailMinWidth + memberColumnWidth
        static let windowMinHeight: CGFloat = 600
    }

    enum Typography {
        /// Bold toolbar titles: the sidebar-column title ("Direct Messages" / server name) and the
        /// content title (channel or DM name). a-main-dark-dm-1x.png: ~8px per character, about 15pt bold.
        static let toolbarTitle = Font.system(size: 15, weight: .bold)
        /// Initials on rail icons without artwork: 0.36 of the icon edge, at least 8 (v2
        /// GuildRailIcon), shrinkable for long names.
        static let railInitialsScale: CGFloat = 0.36
        static let railInitialsMinSize: CGFloat = 8
        static let railInitialsMinScale: CGFloat = 0.5
        /// Rail symbol icons such as the DM entry: 22 pt (v2 GuildRailIcon symbol configuration).
        static let railSymbol = Font.system(size: 22)
        /// Folder glyph when open (legacy 18pt).
        static let folderGlyph = Font.system(size: 18)
        /// Initials inside the 22 pt folder preview thumbs (0.36 of 22, as on rail icons).
        static let folderThumbInitials = Font.system(size: 8)
        /// Member-list section headers ("Admin — 3"): 13 regular, secondary, normal case, the size
        /// of v2's channel category headers (measured on the v2 member-list capture).
        static let memberSectionHeader = Font.system(size: 13)
        /// Trailing chevron after a category name.
        static let categoryChevron = Font.system(size: 8, weight: .bold)
        /// Channel names 14, category headers 13, both regular; unread changes the colour, not
        /// the weight (v2 SidebarRow).
        static let channelName = Font.system(size: 14)
        static let channelCategory = Font.system(size: 13)
        /// Channel and rail mention badges: 11 pt semibold (v2 SidebarRow badge; the rail badge's
        /// font was not observed and follows it).
        static let channelBadge = Font.system(size: 11, weight: .semibold)
        /// DM rows share the channel size; the section header above them is quieter.
        static let dmName = Font.system(size: 15)
        static let dmActivity = Font.caption
        static let dmTag = Font.caption2.weight(.bold)
        static let dmSectionHeader = Font.callout
        static let occupantName = Font.callout
        /// Username 16, weight 500 (`.username_c19a55`: font-size 1rem, --font-weight-medium).
        static let authorName = Font.system(size: Message.authorSize, weight: .medium)
        /// Header time 12, weight 500 (`.cozy_c19a55 .timestamp_c19a55`: .75rem, medium).
        static let timestamp = Font.system(size: Message.timestampSize, weight: .medium)
        /// Chrome text near the messages (load failure, file names, hover tips); the message body
        /// itself is `Message.bodySize` through `MarkdownText`.
        static let messageBody = Font.callout
        /// Time in a follow-up row's gutter: 11, medium (`.timestamp_c19a55.alt_c19a55`: .6875rem).
        static let gutterTime = Font.system(size: Message.gutterTimeSize, weight: .medium)
        /// Date divider label: 12, semibold (`.content__908e2`, module 5126c).
        static let dividerLabel = Font.system(size: Message.dividerLabelSize, weight: .semibold)
        /// Reply preview: 14 (`.repliedMessage_c19a55`: font-size .875rem).
        static let reply = Font.system(size: Message.replySize)
        /// "(edited)": 10 (`.edited_c19a55`: font-size .625rem).
        static let edited = Font.system(size: Message.editedSize)
        /// Member name 14 and status line 12, both regular, one line (v2 MemberView /
        /// MemberActivityView labels).
        static let memberName = Font.system(size: 14)
        static let memberStatus = Font.system(size: 12)
        /// Footer: k-footer-card.png shows a semibold ~13pt name over an ~11pt secondary username (legacy headline / 12pt).
        static let footerName = Font.system(size: 13, weight: .semibold)
        static let footerUsername = Font.system(size: 11)
        static let badge = Font.caption2.weight(.bold)
        static let composer = Font.body
        /// The same font for the AppKit editor (`.body` is 13 pt on macOS).
        static var composerAppKit: NSFont { .preferredFont(forTextStyle: .body) }
        /// Character counter and its upsell line.
        static let composerCounter = Font.system(size: 12)
        /// Composer glyphs and reply bar, from Swiftcord v2's rendered composer (user's capture,
        /// 2026-10-13; the SwiftUI code could not be located in the binary): sticker / emoji
        /// glyphs ~19 pt, "GIF" 21x9 pt bold, reply arrow 15x13, close cross 9x9 semibold,
        /// "Replying to" and the preview ~13 pt; all white rather than secondary.
        static let composerGlyph = Font.system(size: 18)
        /// Thin "+" of the composer button: ~14pt wide with a ~1pt stroke in g-composer-plus-button.png.
        static let composerPlusGlyph = Font.system(size: 18, weight: .light)
        /// "GIF" text button in the composer.
        static let composerGif = Font.system(size: 14, weight: .bold)
        static let replyAuthor = Font.system(size: 13, weight: .semibold)
        static let replyPreview = Font.system(size: 13)
        static let replyGlyph = Font.system(size: 15)
        static let replyClose = Font.system(size: 11, weight: .semibold)
        static let plusMenuRow = Font.body
        /// Initials on generated avatars, as a fraction of the avatar's edge.
        static let initialsScale: CGFloat = 0.4
    }

    /// The message area, from Discord's own CSS (docs/plans/markdown.md, T1; 1 CSS px = 1 pt,
    /// 1 rem = 16). Module c19a55 is the message, 75297 the markup, 5126c the group and divider
    /// spacing. gg sans is not shipped: the system font is used at the same sizes.
    enum Message {
        /// `--custom-message-spacing-vertical-container-cozy: .125rem`, top and bottom of every row.
        static let rowPaddingVertical: CGFloat = 2
        /// `.avatar_c19a55`: `inset-inline-start: var(--custom-message-margin-horizontal)`.
        static let avatarLeading: CGFloat = 16
        /// `.avatar_c19a55`: `margin-top: calc(4px - 2px)`, below the row's content top.
        static let avatarTop: CGFloat = 2
        /// Avatar to text: 72 = 16 + 40 + 16 (`--custom-message-margin-left-content-cozy`).
        static let contentInset: CGFloat = avatarLeading + Size.messageAvatar + 16
        /// `.wrapper_c19a55`: `padding-inline-end: var(--custom-message-margin-horizontal)`.
        static let trailingInset: CGFloat = 16
        /// `.cozy_c19a55 .header_c19a55`: `line-height: 1.375rem; min-height: 1.375rem`.
        static let headerLineHeight: CGFloat = 22
        /// `.cozy_c19a55 .headerText_c19a55`: `margin-inline-end: .25rem`, username to time.
        static let headerGap: CGFloat = 4
        /// `.username_c19a55`: font-size 1rem.
        static let authorSize: CGFloat = 16
        /// `.cozy_c19a55 .timestamp_c19a55`: font-size .75rem.
        static let timestampSize: CGFloat = 12
        /// `.timestamp_c19a55.alt_c19a55`: font-size .6875rem; 56 wide (`width: 56px`), right-aligned.
        static let gutterTimeSize: CGFloat = 11
        static let gutterWidth: CGFloat = 56
        /// `.markup__75297`: font-size 1rem; `--chat-markup-line-height: 1.375rem`.
        static let bodySize: CGFloat = 16
        static let bodyLineHeight: CGFloat = 22
        /// `.edited_c19a55`: font-size .625rem.
        static let editedSize: CGFloat = 10
        /// `.group-spacing-16 .groupStart`: `--custom-group-spacing-start: 1.0625rem`, above a group's first row.
        static let groupStart: CGFloat = 17
        /// PROVISIONAL: the gap past which the same author's next message starts a new group.
        /// Seven minutes is the threshold commonly documented for Discord; it was not read from
        /// Discord's code (only CSS was). To be checked live against the official client.
        static let groupWindow: TimeInterval = 7 * 60
        /// `--custom-message-reply-message-preview-line-height`: 18; `.repliedMessage_c19a55`: font-size .875rem.
        static let replyLineHeight: CGFloat = 18
        static let replySize: CGFloat = 14
        /// `.cozy_c19a55 .repliedMessage_c19a55`: `margin-bottom: var(--reply-spacing)` (4px). The
        /// plan's 10 pt reply indent (`--custom-message-reply-indent`) is applied by `.compact_c19a55`
        /// only; the cozy layout the app uses has none.
        static let replyGap: CGFloat = 4
        /// `.group-spacing-16 .divider.hasContent`: margin 24 above and 8 below; the line is 1 pt.
        static let dividerAbove: CGFloat = 24
        static let dividerBelow: CGFloat = 8
        static let dividerLineWidth: CGFloat = 1
        /// `.content__908e2`: font-size 12, line-height 13, padding 2 x 4.
        static let dividerLabelSize: CGFloat = 12
        static let dividerLabelLineHeight: CGFloat = 13
        static let dividerLabelPaddingV: CGFloat = 2
        static let dividerLabelPaddingH: CGFloat = 4
    }

    /// The message text view (docs/plans/selection.md, N1).
    enum TextView {
        /// Text views per `MarkdownView` instance; past it the rest of the instance's text renders in
        /// one more view (N1 Limits, F-4).
        static let viewLimit = 64
        /// A press that moves farther than this is a drag: it selects and activates nothing (N1 Links).
        /// AppKit's own drag hysteresis is about this size; not measured.
        static let dragThreshold: CGFloat = 3
        /// Inline code: `.markup__75297 code.inline` has a 1px border and radius 4 (T1). The border
        /// colour was not read from the CSS: the separator colour, provisional.
        static let inlineCodeBorderWidth: CGFloat = 1
        static let inlineCodeRadius: CGFloat = 4
        static let inlineCodeBorder = NSColor.separatorColor
    }

    enum Palette {
        /// Character counter: default text, and the official `text-feedback-critical` once the
        /// draft is over the limit.
        static let counterDefault = Color.primary
        static let counterCritical = Color.red
        /// Placeholder of the composer's editor (the system's placeholder colour, as a text field's).
        static let composerPlaceholder = Color(nsColor: .placeholderTextColor)
        /// Unread indicator and bold-name colour: the strongest label colour of the appearance.
        static let unread = Color.primary
        /// Read channel names use the secondary label colour, unread and selected ones `unread` (v2).
        static let readChannel = Color.secondary
        /// Channel kind glyphs are secondary gray in every state (c-channels-guild.png), never accent.
        static let channelIcon = Color.secondary
        /// Thread tree lines: secondary label colour at 0.65 (v2; 0.3 in another state, inferred muted).
        static let threadLine = Color.secondary.opacity(0.65)
        /// Filled gear of the footer: strongest label colour, white in the dark reference
        /// (k-footer-card.png) while staying legible on light glass.
        static let footerGear = Color.primary
        /// Light rim of the footer glass card (k-footer-card.png shows a ~1px lighter edge).
        static let footerRim = Color.primary.opacity(0.15)
        /// Muted channel names drop to half strength (v2).
        static let mutedOpacity: Double = 0.5
        /// Offline members dim so online ones stand out.
        static let offlineOpacity: Double = 0.5
        /// Channel-button fills: selected and hovered.
        static let channelSelectedFill = Color.gray.opacity(0.3)
        static let channelHoverFill = Color.gray.opacity(0.2)
        /// Rail tile behind initials and symbols when idle (an icon with artwork has no tile).
        static let railIconIdleFill = Color.gray.opacity(0.25)
        /// Backdrop of the mini thumbs in a collapsed folder.
        static let folderThumbFill = Color.gray.opacity(0.15)
        /// Open folder's glyph square, idle and hover (legacy values; b-rail-folder.png shows the
        /// lighter square behind the folder glyph).
        static let folderOpenFill = Color.gray.opacity(0.15)
        static let folderOpenFillHover = Color.gray.opacity(0.25)
        /// Open folder panel: the folder's colour (gray without one) at 0.1, a 1 pt separator-colour
        /// border (v2 GuildFolderBackground).
        static let folderPanelTint: Double = 0.1
        static let folderPanelBorder = Color(nsColor: .separatorColor)
        /// Line between the rail and the channel list; its colour was not observed in v2.
        static let railColumnDivider = Color(nsColor: .separatorColor)
        /// Collapsed folder tint strength over the folder's own colour: idle and hover.
        static let folderTintIdle: Double = 0.4
        static let folderTintHover: Double = 0.5
        /// Member section cards: faint fill so the rounded card shape shows on any backdrop (v2's
        /// card sits ~15 levels above its pane in the dark capture, about this wash).
        static let memberCardFill = Color.primary.opacity(0.06)
        /// Server-tag pill and reply bar fills: faint label-colour wash that tracks the appearance.
        static let tagPillFill = Color.primary.opacity(0.12)
        /// v2's reply bar sits ~19 levels above the composer card in the capture, about twice
        /// the lift 0.08 gave ours; it stays translucent, so content shows through it as in v2.
        static let replyBarFill = Color.primary.opacity(0.12)
        /// g-composer-plus-button.png: button fill (67) sits ~9 levels above the card (58), a ~5% white wash.
        static let plusButtonFill = Color.primary.opacity(0.05)
        static let plusMenuRowHover = Color.primary.opacity(0.1)
        static let mentionBadge = Color.red
        static let mentionBadgeText = Color.white
        static let online = Color.green
        static let idle = Color.yellow
        static let doNotDisturb = Color.red
        static let offline = Color.gray
        /// Ring that separates dots and badges from the artwork beneath them.
        static let statusDotRing = Color(nsColor: .windowBackgroundColor)
        /// Generated avatar backgrounds: hue from a hash of the name; initials colour comes from
        /// `ContrastText`, so both appearances meet the contrast rule.
        static let avatarSaturation: Double = 0.5
        static let avatarBrightness: Double = 0.62
        /// Window backdrop: the system material that gives the translucent window.
        static let windowMaterial = Material.ultraThinMaterial
        /// Message column: the text background colour laid over the translucent window, so the
        /// desktop tints it like the sidebars, only darker. A deliberate departure from Swiftcord
        /// v2, whose panes are near-opaque neutral greys: the user prefers the translucency as
        /// long as every region carries it alike (decision 2026-10-13; an opaque pane under a
        /// translucent sidebar read as inconsistent).
        static let messagePane = Color(nsColor: .textBackgroundColor).opacity(0.6)
        /// Glow radius of the display-name "glow" effect.
        static let nameGlowRadius: CGFloat = 4
    }

    /// The message context menu (selection plan C1, D5): Discord's layout, measured at 1x in
    /// refs/discord-ground-truth/captures/e0-menu-other.png (kept out of git) and its CSS module
    /// c1e9c4 (docs/element-map.md), drawn on glass instead of Discord's dark fill.
    enum Menu {
        /// The capture's menu is 222 px wide including its 1 px border; glass has no border.
        static let width: CGFloat = 220
        /// Panel edge to the first and last row, and to the rows' sides: the capture's last row
        /// ends 8 above the bottom border.
        static let panelPadding: CGFloat = 8
        /// `.item`: min-height 32 (D5). Rows sit 36 apart in the capture, so 2 above and below each.
        static let rowHeight: CGFloat = 32
        static let rowMargin: CGFloat = 2
        /// `.item`: padding 8; radius 2 (D5).
        static let itemPadding: CGFloat = 8
        static let itemRadius: CGFloat = 2
        /// `.separator`: 1 pt with 8 above and below (D5); a section break adds 17 to the pitch in
        /// the capture.
        static let separatorHeight: CGFloat = 1
        static let separatorGap: CGFloat = 8
        /// Icon box 18; the capture's glyphs are 16 px, starting 18 px in from the menu's inner edge
        /// (8 panel + 8 item + 2), and the label starts 10 px after the 18 pt box.
        static let iconBox: CGFloat = 18
        static let iconGap: CGFloat = 10
        static let icon = Font.system(size: 15)
        /// Labels: cap height 10 px in the capture, Discord's 14 px medium.
        static let label = Font.system(size: 14, weight: .medium)
        /// "⌘C" and the Open Link arrow at the trailing edge.
        static let trailing = Font.system(size: 13, weight: .medium)
        /// Corner of the glass panel; the capture's corner sits under its border, so the radius is
        /// not measured (provisional, a visual round may change it).
        static let panelRadius: CGFloat = 8
        /// `--background-mod-subtle` under the highlighted row: a faint label-colour wash, as the
        /// "+" menu's hover (`Palette.plusMenuRowHover`).
        static let rowHighlight = Color.primary.opacity(0.1)
        /// `--border-subtle`.
        static let separator = Color(nsColor: .separatorColor)
    }

    enum Motion {
        /// The composer's one spring (plan D6, Swiftcord v2: the same constants drive its reply bar
        /// and attachment row), keyed on whether a reply is present.
        static let composer = Animation.spring(response: 0.15, dampingFraction: 0.85, blendDuration: 0.25)
        /// Legacy spring for the rail indicator and icon morph.
        static let railSpring = Animation.interpolatingSpring(stiffness: 500, damping: 30)
    }

    /// Message skeleton while history loads (v2 MessageSkeletonView, refs/swiftcord-v2-teardown.md,
    /// kept out of git; bar heights, offsets, pitch and row gap checked on the v2 capture of
    /// 2026-10-14).
    enum Skeleton {
        /// Avatar circle 40 at x 16, two below the row top; bars start at 72 and stop 16 short
        /// of the trailing edge.
        static let inset: CGFloat = 16
        static let avatar: CGFloat = 40
        static let avatarTop: CGFloat = 2
        static let textLeading: CGFloat = 72
        /// Name bar 14 tall at +3; the timestamp bar 9 tall at +6, 8 after the name bar.
        static let nameTop: CGFloat = 3
        static let nameHeight: CGFloat = 14
        static let timeTop: CGFloat = 6
        static let timeHeight: CGFloat = 9
        static let timeGap: CGFloat = 8
        /// Body lines 12 tall every 17 from +23; a media block 3 under the last line, corner 8.
        static let firstLineTop: CGFloat = 23
        static let lineHeight: CGFloat = 12
        static let linePitch: CGFloat = 17
        static let mediaTop: CGFloat = 3
        static let mediaRadius: CGFloat = 8
        /// Rows are at least the avatar tall and 19 apart.
        static let rowGap: CGFloat = 19
        /// Secondary label colour at these alphas (v2 sets the alpha, it does not multiply it).
        static let avatarAlpha: CGFloat = 0.2
        static let barAlpha: CGFloat = 0.18
        static let timeAlpha: CGFloat = 0.12
        static let mediaAlpha: CGFloat = 0.14
    }
}

// MARK: Contrast

extension RGB {
    /// HSB to RGB, to know the colour a generated avatar background actually is.
    init(hue: Double, saturation: Double, brightness: Double) {
        let h = (hue - hue.rounded(.down)) * 6
        let c = brightness * saturation
        let x = c * (1 - abs(h.truncatingRemainder(dividingBy: 2) - 1))
        let m = brightness - c
        let (r, g, b): (Double, Double, Double) = switch Int(h) {
        case 0: (c, x, 0)
        case 1: (x, c, 0)
        case 2: (0, c, x)
        case 3: (0, x, c)
        case 4: (x, 0, c)
        default: (c, 0, x)
        }
        self.init(r + m, g + m, b + m)
    }

    /// WCAG 2.x relative luminance.
    var relativeLuminance: Double {
        func linear(_ v: Double) -> Double { v <= 0.03928 ? v / 12.92 : pow((v + 0.055) / 1.055, 2.4) }
        return 0.2126 * linear(red) + 0.7152 * linear(green) + 0.0722 * linear(blue)
    }

    /// WCAG contrast ratio, 1...21.
    func contrastRatio(with other: RGB) -> Double {
        let a = relativeLuminance, b = other.relativeLuminance
        return (max(a, b) + 0.05) / (min(a, b) + 0.05)
    }
}

enum ContrastText {
    static let white = RGB(1, 1, 1)
    static let black = RGB(0, 0, 0)

    /// Whichever of white or black contrasts more with `background`; the better of the two
    /// always reaches at least 4.58:1, so AA holds for any opaque background.
    static func rgb(on background: RGB) -> RGB {
        background.contrastRatio(with: white) >= background.contrastRatio(with: black) ? white : black
    }

    static func color(on background: RGB) -> Color { Color(rgb(on: background)) }
}
