> 這份文件是 Discord 官方桌面客戶端的功能全貌，2026-10-16 由網路調查整理而成（未登入任何帳號、未呼叫任何 Discord API）。「來源」欄是官方說明中心、開發者文件或更新日誌的網址；標示 `(unconfirmed)` 的項目只有第三方來源（多半是 docs.discord.food、新聞網站或長期存在的客戶端行為），實作前要先對照實際客戶端。velella 對照這份清單的完成狀態，記錄在 [docs/parity.md](parity.md)，那份是從這裡衍生出來的狀態追蹤表。以下為調查結果原文（英文），未經修改。

# Discord desktop client: feature inventory (October 2026)

A parity checklist for velella: the user-facing features of the official Discord desktop client as of 2026-10-16, area by area. It was compiled from web research only. No account was logged into and no Discord API was called.

## How to read this inventory

| Convention | Meaning |
|---|---|
| `(unconfirmed)` | No official Discord source (help center, developer docs or discord.com blog) was found for the item. It rests on third-party docs (mostly docs.discord.food, the community user-API reference), news sites or long-standing client behavior. Verify these against the live client before building to them. |
| `(2025)` / `(2026)` | Launch or change year, where an official source dates it. |
| Help-center URLs | Cited as `https://support.discord.com/hc/en-us/articles/{id}`. The bodies were read through the Zendesk API, because the HTML pages return 403 to automated fetchers. |
| Developer-docs URLs | The developer docs moved from discord.com/developers/docs to `https://docs.discord.com/developers/...` and are cited at the new location. |
| Multiple sources | Separated by ` ; `. Where official sources contradict each other, the row gives both values. |

## Points that matter most for a native macOS client

| Point | Detail | Source URL |
|---|---|---|
| Voice E2EE (DAVE) is mandatory | Clients without DAVE support can't join voice, video or Go Live; the voice gateway closes with code 4017. The help article stating this was published 2026-03-02 and gives no separate enforcement date. Minimum client versions are listed in the linked article. See Voice & video. | https://support.discord.com/hc/en-us/articles/38749827197591 ; https://support.discord.com/hc/en-us/articles/38025123604631 |
| Game overlay and Clips are Windows-only | Overlay, the overlay soundboard and Clip saving have no macOS equivalent in the official client. | https://support.discord.com/hc/en-us/articles/217659737 ; https://support.discord.com/hc/en-us/articles/16861982215703 |
| Voice messages are record-on-mobile only | Desktop only plays voice messages back; recording is mobile-only. | https://support.discord.com/hc/en-us/articles/13091096725527 |
| Settings were reorganized in 2025–2026 | Older help articles still use pre-rename labels ("Privacy & Safety", "Content & Social", "Text & Images", "Activity"). The current names are "Messaging Permissions", "Games & Apps", and Appearance/Accessibility/Developer consolidation. See User Settings. | https://discord.com/blog/discord-update-august-11-2026-changelog |
| macOS hotkeys come from the official chart image | Article 225878307 publishes its macOS bindings only as an image. They were transcribed from that image; see Keyboard shortcuts. | https://support.discord.com/hc/en-us/articles/225878307 |

## Index

1902 feature rows, 260 of them marked unconfirmed.

| Area | Rows | Unconfirmed markers |
|---|---|---|
| [Account & login](#account--login) | 45 | 1 |
| [Server rail (guild sidebar)](#server-rail-guild-sidebar) | 82 | 7 |
| [Channel list](#channel-list) | 143 | 11 |
| [DMs and friends](#dms-and-friends) | 115 | 13 |
| [Message display](#message-display) | 193 | 97 |
| [Message actions](#message-actions) | 53 | 16 |
| [Composer](#composer) | 44 | 25 |
| [Reactions](#reactions) | 15 | 4 |
| [Search, Inbox & pins](#search-inbox--pins) | 77 | 9 |
| [Member list & profiles](#member-list--profiles) | 120 | 11 |
| [Presence & status](#presence--status) | 39 | 5 |
| [Notifications](#notifications) | 59 | 14 |
| [Voice & video](#voice--video) | 169 | 8 |
| [Threads and forums](#threads-and-forums) | 61 | 3 |
| [User Settings](#user-settings) | 158 | 7 |
| [Server settings & moderation](#server-settings--moderation) | 208 | 5 |
| [Keyboard shortcuts](#keyboard-shortcuts) | 94 | 17 |
| [App-level features](#app-level-features) | 74 | 5 |
| [Nitro, monetization & extras](#nitro-monetization--extras) | 75 | 1 |
| [Appendix: 2024–2026 feature timeline (official changelogs and patch notes)](#appendix-20242026-feature-timeline-official-changelogs-and-patch-notes) | 78 | 1 |

## Account & login

### Sign-up and login

| Feature | Detail (states, dialogs, conditions such as permission/Nitro/DM-only) | Source URL |
|---|---|---|
| Sign-up (registration) | Desktop/browser fields: email, username, password, date of birth; CAPTCHA if prompted. Mobile can start with phone number or email. Can start from an invite link. | https://support.discord.com/hc/en-us/articles/31676852332439 |
| Email + password login | discord.com/login or desktop app; enter verified email (or phone) then password. | https://support.discord.com/hc/en-us/articles/360057027354 |
| Phone number + password login | Same login field accepts phone number; phone must be verified to the account first. | https://support.discord.com/hc/en-us/articles/360057027354 |
| "Forgot your password?" reset | On login page, enter account email then "Forgot your password?"; reset link by email. MFA code required if MFA enabled. Phone-based reset via SMS code exists for phone-registered accounts. | https://support.discord.com/hc/en-us/articles/218410947 ; https://support.discord.com/hc/en-us/articles/4413460214807 |
| Password rules | Minimum 8 characters; additional regional requirements may apply. | https://support.discord.com/hc/en-us/articles/218410947 |
| QR code login | Desktop/browser login screen shows a QR code; scan from logged-in mobile app (Settings > "Scan QR Code") or any camera app that deep-links to Discord; must Approve or Cancel on phone. Code expires after 2 minutes (refresh). Phishing warning. | https://support.discord.com/hc/en-us/articles/360039213771 |
| Passkey (passwordless) login | If the password manager holds a Discord passkey it is offered automatically; otherwise "Or, sign in with passkey" on login screen; OS challenge (Face ID etc.) then logged in. (2024) | https://support.discord.com/hc/en-us/articles/25966860846231 ; https://discord.com/blog/discord-patch-notes-september-8-2026 |
| Security key as second factor | After username + password, extra challenge "Authenticate with security key". Security keys generally cannot do one-step login. | https://support.discord.com/hc/en-us/articles/25966860846231 |
| MFA login challenge | After password, prompt for 6-digit authenticator code; "Verify with something else" lets user pick backup code (8-digit, one-time) or SMS code; "Confirm". Backup-code option disappears when all codes are used. | https://support.discord.com/hc/en-us/articles/115001221072 ; https://support.discord.com/hc/en-us/articles/26304482627095 |
| CAPTCHA challenges | CAPTCHA can appear at sign-up, phone verification, and email change (Aug 2026 fix: email change failing after CAPTCHA). | https://support.discord.com/hc/en-us/articles/31676852332439 ; https://support.discord.com/hc/en-us/articles/6181726888215 ; https://discord.com/blog/discord-patch-notes-august-4-2026 |
| New login location verification | Login from an unrecognized IP/location can require clicking a "New login location detected" verification email before login completes (unconfirmed). | https://support.discord.com/hc/en-us/community/posts/4415712921751--new-IP-detected-email-confirmation-Germany |
| Purchase authorization email | Some purchases prompt in-app to authorize; email "Authorize Payments for Discord Client" with "Verify Purchase" button, then retry purchase. | https://support.discord.com/hc/en-us/articles/9074940276503 |
| Provisional accounts | Use Discord voice/text chat inside Discord-integrated games without creating a full Discord account first (2025; article "under construction"). | https://support.discord.com/hc/en-us/articles/29667419799063 |

### Multi-factor authentication (MFA)

| Feature | Detail (states, dialogs, conditions such as permission/Nitro/DM-only) | Source URL |
|---|---|---|
| MFA settings location | Desktop: User Settings > My Account > "Password and Authentication"; since June 2026 Multi-Factor Authentication is a nested page within the Account category. (2026) | https://support.discord.com/hc/en-us/articles/26304482627095 ; https://discord.com/blog/discord-patch-notes-june-4-2026 |
| Register a Security Key / Passkey | "Register a Security Key" → reauthenticate (password or existing MFA) → "Let's go" → OS passkey prompt → name the key → backup codes shown. Up to 16 keys/passkeys per account. Recommended ("phishing resistant") method. | https://support.discord.com/hc/en-us/articles/25966860846231 ; https://support.discord.com/hc/en-us/articles/219576828 |
| Remove a passkey / security key | Delete old passkey from Account settings (path described only in troubleshooting). | https://support.discord.com/hc/en-us/articles/25966860846231 |
| Enable Authenticator App (TOTP) | "Enable Authenticator App" → scan QR code (or copy secret key) into Google Authenticator/Authy/password manager → enter 6-digit code under "Login with your code" → "Activate" → save backup codes. | https://support.discord.com/hc/en-us/articles/26304482627095 |
| Remove Authenticator App | My Account > "Remove Authenticator App" (requires code). Support cannot remove MFA. | https://support.discord.com/hc/en-us/articles/115001221072 |
| SMS authentication (backup) | Only after authenticator app is set up: add phone → verify with SMS code → "Enable SMS Authentication" → approve with MFA code. Login screen then offers SMS code link. Not recommended. | https://support.discord.com/hc/en-us/articles/219576828 ; https://discord.com/blog/discord-patch-notes-august-4-2025 |
| Backup codes | "View Backup Codes" (password required) shows codes, which are used, and download option (discord_backup_codes.txt). One-time 8-digit codes. Generating codes right after first enabling 2FA fixed Oct 2026. | https://support.discord.com/hc/en-us/articles/219576828 ; https://support.discord.com/hc/en-us/articles/115001221072 ; https://discord.com/blog/discord-patch-notes-october-6-2026 |
| MFA required for sensitive actions | MFA code required to change/reset password, delete account, and similar account management. | https://support.discord.com/hc/en-us/articles/218410947 ; https://support.discord.com/hc/en-us/articles/212500837 |
| MFA lockout recovery | Use backup code or SMS; Discord cannot remove MFA or issue new codes without them. | https://support.discord.com/hc/en-us/articles/115001221072 |
| Show-password button on MFA password screen | iOS password MFA screen got reveal button (Oct 2026). | https://discord.com/blog/discord-patch-notes-october-6-2026 |

### Verification, credentials and identity

| Feature | Detail (states, dialogs, conditions such as permission/Nitro/DM-only) | Source URL |
|---|---|---|
| Email verification | Account may need verified email (required for data requests, new username, server verification levels, Go Live notifications). Verification email resend via linked help flow. | https://support.discord.com/hc/en-us/articles/6181726888215 ; https://support.discord.com/hc/en-us/articles/12620128861463 |
| Phone verification | "Start Verification" → enter mobile number (no VoIP/burner/landline; not linked to another account) → SMS code. Can be forced after suspicious activity or by server verification level; 24–48 h wait for recently used numbers. | https://support.discord.com/hc/en-us/articles/6181726888215 |
| Add / change / remove phone number | My Account > phone "Remove" (password); only one number per account; waiting period before reuse. | https://support.discord.com/hc/en-us/articles/4413460214807 |
| Change email | My Account > "Edit" next to email → verification email to new address. Rate limit "You are changing your e-mail too fast" (wait 24 h). | https://support.discord.com/hc/en-us/articles/4423385681175 ; https://support.discord.com/hc/en-us/articles/32557375603479 |
| Email-change undo / account recovery | Original address gets email with "Start Account Recovery" (discord.com/wasntme/ link, valid 48 h): revert email, log out all devices, set new password, remove phone/MFA. | https://support.discord.com/hc/en-us/articles/27299669812375 |
| Change password | My Account > "Change Password" (current + new; MFA if enabled). Changing password logs out all devices. | https://support.discord.com/hc/en-us/articles/218410947 ; https://support.discord.com/hc/en-us/articles/360039213771 |
| Change username | My Account > Edit next to Username; 2–32 chars, a–z 0–9 _ . (no consecutive periods), lowercase, unique; max 2 changes per week (error states 7-day wait); requires verified email/phone. | https://support.discord.com/hc/en-us/articles/12620128861463 ; https://discord.com/blog/discord-patch-notes-march-6-2026 |
| Display name | 1–32 chars, any characters/emoji; change any time; overridden by server/friend nicknames. | https://support.discord.com/hc/en-us/articles/12620128861463 |
| Age group / Account Status page | User Settings > Account Status shows age group; "Confirm Age" and "Retry". Methods: App Store / Google Play age range, credit card (k-ID), Google Wallet passport, on-device video selfie, ID scan, AgeKey (reusable age passkey). Result arrives as official DM. (2025–2026) | https://support.discord.com/hc/en-us/articles/30326565624343 |
| Teen-by-default protections | Teen/unconfirmed: sensitive filters fixed (blur/block), Message Requests forced on, DM spam filter "filter all", friend request alerts on, Stage speaking blocked, age-restricted spaces blocked; teens default Profile Privacy and Activity Sharing to friends & small servers. (2026) | https://support.discord.com/hc/en-us/articles/42178056096663 |

### Sessions, account switching and account lifecycle

| Feature | Detail (states, dialogs, conditions such as permission/Nitro/DM-only) | Source URL |
|---|---|---|
| Account switcher | Click avatar (lower-left) > "Switch Accounts"; up to 5 accounts logged in on desktop; other users can't see which accounts are linked. | https://support.discord.com/hc/en-us/articles/4412732047127 |
| Account switch side-effects | Some local prefs (e.g., font scaling) reset on switch; unsent drafts are discarded; each account keeps its own privacy settings. | https://support.discord.com/hc/en-us/articles/4412732047127 |
| Log out | User Settings > red "Log Out" button. Voice messages stop playing on logout (Feb 2026). | https://support.discord.com/hc/en-us/articles/209572128 ; https://discord.com/blog/discord-patch-notes-february-4-2026 |
| Devices page | Nested page under Account (June 2026); lists logged-in sessions with option to log out individual devices or all other devices (session-list behavior unconfirmed). | https://discord.com/blog/discord-patch-notes-june-4-2026 ; https://www.xda-developers.com/discord-log-out-sessions-other-devices/ |
| Log out all devices | Officially documented route: change password (logs out all devices); also in email-change recovery flow. | https://support.discord.com/hc/en-us/articles/360039213771 ; https://support.discord.com/hc/en-us/articles/27299669812375 |
| Disable account | My Account > Account Removal > "Disable Account"; must transfer owned servers; subscriptions keep billing; re-enable by logging in and choosing "Restore Account". | https://support.discord.com/hc/en-us/articles/360004066391 |
| Delete account | My Account > "Delete Account" (bottom); transfer/delete owned servers first; password + MFA; 15-day pending-deletion period (log in to cancel); then anonymized, Nitro/Boosts cancelled. | https://support.discord.com/hc/en-us/articles/212500837 |
| Inactive account deletion | Accounts unused for 2+ years may be scheduled for deletion (email/phone notice). | https://support.discord.com/hc/en-us/articles/5106714856215 ; https://support.discord.com/hc/en-us/articles/212500837 |
| Account Standing | User Settings > Account > Account Standing (nested page since June 2026). Levels: All good, Limited, Very limited, At risk, Permanent suspension. | https://support.discord.com/hc/en-us/articles/18210965981847 ; https://discord.com/blog/discord-patch-notes-june-4-2026 |
| Violations & appeals | Violations delivered as system DM with summary card (policy, content, action); Active vs Expired Violations (most expire after 90 days); per-violation review webform; suspended users can still log in to view standing. | https://support.discord.com/hc/en-us/articles/18210965981847 |
| Warnings | Separate from violations; don't affect standing; sent when user was in a rule-breaking server or interacted with violating content. | https://support.discord.com/hc/en-us/articles/18210965981847 |
| Hacked-account guidance | Review Authorized Apps (Deauthorize), reset password, enable MFA; dis.gd/hackedaccount. | https://support.discord.com/hc/en-us/articles/24160905919511 |

## Server rail (guild sidebar)

### Rail chrome and navigation

| Feature | Detail (states, dialogs, conditions such as permission/Nitro/DM-only) | Source URL |
|---|---|---|
| Home / Direct Messages button | Top of the rail; opens the DM list and Friends. Ctrl+1 from a server returns to the Friends list instead of the first DM (2026 behavior change). | https://discord.com/blog/discord-patch-notes-june-4-2026 |
| Favorites "server" icon | Nitro experiment (2026). Icon top-left above the server list on desktop, browser and mobile. Hidden unless enabled via the pop-up or User Settings > Chat > "Show Favorites Server". Non-Nitro users can preview it in settings. Saved favorites persist if the feature is turned off or Nitro lapses. | https://support.discord.com/hc/en-us/articles/38810584460439 |
| Favorites view | Text, voice and forum channels, DMs and group DMs (not threads). User-created categories; drag to reorder or categorize; uncategorized items on top; up to 200 items. Editing on desktop/browser only; private. | https://support.discord.com/hc/en-us/articles/38810584460439 |
| Server icon | Squircle icon per joined server; animated server icon needs Boost Level 1. Stacked icons (folders) also use the squircle shape (2026 fix). | https://support.discord.com/hc/en-us/articles/360028038352 ; https://discord.com/blog/discord-patch-notes-september-8-2026 |
| Server hover tooltip | Shows the server name plus a type badge (Community, Discoverable Community, Partnered, Verified, or "Verified & Partnered"). The badge tooltip shows the server type, whether anyone can join, and the Boost level. Hidden tooltip icons are not tabbable (2026). | https://support.discord.com/hc/en-us/articles/14078261239831 ; https://discord.com/blog/discord-patch-notes-june-4-2026 |
| Unread indicator on server icon | A server with unread messages shows an unread indicator. Muting suppresses it unless you are mentioned. The left-side white "pill" shape, and its growth on hover/selection, is not described in official text (unconfirmed). | https://support.discord.com/hc/en-us/articles/209791877 |
| Mention badge (red counter) | Red counter for @username, @role, @everyone and @here mentions. It still shows when the server is muted. | https://support.discord.com/hc/en-us/articles/209791877 |
| Muted server state | No unread indicator or notifications except mentions; mobile push is disabled while muted. | https://support.discord.com/hc/en-us/articles/209791877 ; https://support.discord.com/hc/en-us/articles/215253258 |
| Reorder servers by drag | Drag icons to reorder; folders are positioned the same way. No extra background behind icons while reordering (2026 fix). | https://support.discord.com/hc/en-us/articles/360030853132 ; https://discord.com/blog/discord-patch-notes-february-4-2026 |
| Keyboard reorder (accessible drag and drop) | Select a server, Ctrl/Cmd+D to lift, Up/Down arrows to move, Space or Enter to drop, Esc to cancel. A green dot or line shows the drop target. Desktop and browser only. Keyboard reordering keeps focus in place (2026 fix). | https://support.discord.com/hc/en-us/articles/4408877527703 ; https://discord.com/blog/discord-patch-notes-july-7-2026 |
| Navigate servers hotkey | Windows Ctrl+Alt+Up/Down; Mac Cmd+Option+Up/Down. | https://support.discord.com/hc/en-us/articles/225977308 ; https://support.discord.com/hc/en-us/articles/31232432266647 |
| Mark Server Read hotkey | Shift+Esc. Marking a server read also clears unread state on new, unopened forum posts (2026 fix). | https://support.discord.com/hc/en-us/articles/225977308 ; https://discord.com/blog/discord-patch-notes-september-8-2026 |
| Rail scrolling | The rail scrolls; the bottom Discover icon is fully reachable (2026 fix). | https://discord.com/blog/discord-patch-notes-september-8-2026 |
| Voice-connected server popover | When in voice, the guild sidebar shows popovers tied to the voice session. The exact content (speaker icon, participant avatars) is unconfirmed. | https://discord.com/blog/discord-patch-notes-july-7-2026 |
| Explore Public Servers (Student Hubs) | Button at the bottom of the left panel for finding your school's Student Hub. Needs a school email; has a waitlist; rolling out slowly. | https://support.discord.com/hc/en-us/articles/4406046651927 |
| Download Apps button | Download entry at the bottom of the rail, shown in browser and desktop (unconfirmed). | (unconfirmed, no official source found) |
| Nitro / Shop entries | Nitro and Shop are not rail items; they sit in the Home/DM sidebar (unconfirmed placement). The Shop has tabs, including a Game Servers tab. | https://support.discord.com/hc/en-us/articles/35370817986839 |

### Server folders

| Feature | Detail (states, dialogs, conditions such as permission/Nitro/DM-only) | Source URL |
|---|---|---|
| Create folder by drag | Click, hold and drag a server over another server for a few seconds, then drop. Mobile: long-press, hover over a server, drop. | https://support.discord.com/hc/en-us/articles/360030853132 |
| Create folder via accessibility action | Screen-reader users can create a folder from the sidebar without dragging (2026). | https://discord.com/blog/discord-patch-notes-june-4-2026 |
| Add / remove servers | Drag servers into an open or closed folder; drag out to remove. A folder with no servers left is deleted automatically. | https://support.discord.com/hc/en-us/articles/360030853132 |
| Folder capacity | Up to 100 servers per folder. | https://support.discord.com/hc/en-us/articles/360030853132 |
| No folder merging | A folder cannot be dragged onto another folder. | https://support.discord.com/hc/en-us/articles/360030853132 |
| Expand / collapse folder | Click to open or close. Screen readers announce the expanded state (2026). A closed folder shows its servers' icons in a stacked/mini grid (visual details unconfirmed). | https://discord.com/blog/discord-patch-notes-february-4-2026 ; https://discord.com/blog/discord-patch-notes-september-8-2026 |
| Folder notification badge | A folder shows a badge when a server inside has notifications. | https://support.discord.com/hc/en-us/articles/360030853132 |
| Folder context menu | Right-click a folder: "Folder Settings" and "Mark Server Folder as Read". Mobile: long-press an open folder. | https://support.discord.com/hc/en-us/articles/360030853132 |
| Folder Settings dialog | Rename the folder and change its color. Stored data: id, name (defaults to the comma-joined guild names), color int, guild_ids (unconfirmed). | https://support.discord.com/hc/en-us/articles/360030853132 ; https://docs.discord.food/resources/user-settings |
| Move folder (mobile) | Long-press a closed folder to reposition it. | https://support.discord.com/hc/en-us/articles/360030853132 |

### Add a Server, join, templates, Discover

| Feature | Detail (states, dialogs, conditions such as permission/Nitro/DM-only) | Source URL |
|---|---|---|
| "Add a Server" (+) button | Bottom of the server list. Opens the creation pop-up. Hotkey Ctrl/Cmd+Shift+N ("Create or Join a Server"). | https://support.discord.com/hc/en-us/articles/204849977 ; https://support.discord.com/hc/en-us/articles/225977308 |
| Create My Own | Blank server, then an optional guided setup ("For a club or community" vs "For me and my friends", skippable), then name and icon (min 128x128, crop in multiples of 128), then Create. | https://support.discord.com/hc/en-us/articles/204849977 ; https://support.discord.com/hc/en-us/articles/33023827550359 |
| Start from a template | The creation pop-up offers templates. A template link opens a preview where you can rename the server and upload an icon, then Create; channels, roles and permissions are copied. Expired template links show an error (2026 fix). | https://support.discord.com/hc/en-us/articles/360041033511 ; https://discord.com/blog/discord-patch-notes-september-8-2026 |
| Join via invite code | Invite-code field at the bottom of the creation pop-up. | https://support.discord.com/hc/en-us/articles/204849977 |
| Join by pasting an invite in the Quick Switcher | Pasting an invite link in the Quick Switcher joins and opens the server (Desktop, 2026). | https://discord.com/blog/discord-patch-notes-may-4-2026 |
| Server limit dialog | 100 servers (Base/Basic) or 200 (Nitro). Joining at the cap shows a server-limit error, including from event links and invites. | https://support.discord.com/hc/en-us/articles/33694251638295 ; https://discord.com/blog/discord-patch-notes-august-4-2026 ; https://discord.com/blog/discord-patch-notes-may-4-2026 |
| Invites-disabled toast | Joining from Discovery shows a toast when the server has paused invites (2026). | https://discord.com/blog/discord-patch-notes-june-4-2026 |
| Guest invites | Accepting a guest invite lands you in the channel as a guest, even in servers with onboarding or Apply to Join (2026). Guests are limited to the invited voice channel (IS_GUEST member flag). | https://discord.com/blog/discord-patch-notes-june-4-2026 ; https://docs.discord.com/developers/resources/guild |
| Discover icon / Discover page | Icon at the bottom of the server list opens the Discover page, whose sidebar has Servers, Apps and Quests pages (keyboard reachable, 2026). Experimental for a percentage of users. | https://support.discord.com/hc/en-us/articles/25323248535319 ; https://discord.com/blog/discord-patch-notes-october-6-2026 |
| Discover > Servers | Discoverable Community servers in a category bar (gaming, music, entertainment…), sorted by member-count popularity. Search is desktop-only. The "All" count matches "Results Found" (2026 fix). | https://support.discord.com/hc/en-us/articles/25323248535319 ; https://discord.com/blog/discord-patch-notes-september-8-2026 |
| Discover > Quests | Quests from the last 30 days; Accept Quest; Claimed Quests list (desktop). | https://support.discord.com/hc/en-us/articles/25323248535319 |
| Discover > Apps | App directory browsing inside Discover (2026; content not documented in the help center). | https://discord.com/blog/discord-patch-notes-october-6-2026 |
| Legacy Server Discovery (compass) | The old experiment (compass icon; categories Gaming, Music, Education, Science & Tech, Entertainment, Student Hubs; search with 5 recent emoji per result) has ended and was replaced by the Discover tab. | https://support.discord.com/hc/en-us/articles/360023968311 |
| Server preview ("lurk") | Selecting a Discover result previews public channels without joining. The server appears temporarily in the list; Back removes it; a Join button sits at the top. Settings can't be used while previewing (2026 fix). | https://support.discord.com/hc/en-us/articles/360023968311 ; https://support.discord.com/hc/en-us/articles/25323248535319 ; https://discord.com/blog/discord-patch-notes-february-4-2026 |
| Server Game Shop (Discovery) | A game server's Game Shop lets you wishlist in-game items that friends can gift (2026, e.g. Marvel Rivals). | https://discord.com/blog/discord-update-march-24-2026-changelog |

### Server context menu (right-click icon) and server-name dropdown

| Feature | Detail (states, dialogs, conditions such as permission/Nitro/DM-only) | Source URL |
|---|---|---|
| Mark As Read | Marks the whole server read (same as Shift+Esc). | https://support.discord.com/hc/en-us/articles/225977308 ; https://discord.com/blog/discord-patch-notes-september-8-2026 |
| Invite People | Invite modal: send to recently messaged DMs or friends, searchable by username or display name (2026), and copy the link. Link settings: Expire After, Max Uses ("No limit"), Grant temporary membership, "Generate a New Link". Default 7-day link. Needs Create Invite. | https://support.discord.com/hc/en-us/articles/208866998 ; https://discord.com/blog/discord-patch-notes-february-4-2026 |
| Mute Server (submenu) | For 15 minutes, 1 hour, 3 hours, 8 hours, 24 hours, or Until I turn it back on. | https://support.discord.com/hc/en-us/articles/218892547 ; https://support.discord.com/hc/en-us/articles/209791877 |
| Notification Settings | Opens the server notification dialog (see Channel list > Notifications). | https://support.discord.com/hc/en-us/articles/215253258 |
| Hide Muted Channels | Checkbox toggle in the server-name dropdown; hides muted channels from the list. | https://support.discord.com/hc/en-us/articles/213599277 |
| Show All Channels | Toggle between opt-in (onboarding-customized) channels and all channels (user guild flags OPT_IN_CHANNELS_ON/OFF) (unconfirmed label). | https://docs.discord.food/resources/user-settings |
| Privacy Settings | Per server: DMs from server members, message requests, Activity Status, all under the server dropdown. Also in User Settings > Content & Social > Social permissions. | https://support.discord.com/hc/en-us/articles/22045487931799 ; https://support.discord.com/hc/en-us/articles/217916488 |
| Edit Per-server Profile | Server avatar and banner (Nitro), About Me, profile theme, avatar decoration and profile effect, server nickname, per-server Nameplate (2025). | https://support.discord.com/hc/en-us/articles/4409388345495 ; https://discord.com/blog/discord-update-november-6-2025-changelog |
| Server Settings | Opens the settings modal. Also reachable from the dropdown next to the server name. | https://support.discord.com/hc/en-us/articles/213595197 |
| Server Profile shortcut | The dropdown arrow next to the server name goes straight to Server Settings > Server Profile (admins). | https://support.discord.com/hc/en-us/articles/30715364399511 |
| Create Channel | Opens the create-channel modal (type picker, name, Private toggle). Manage Channels. | https://support.discord.com/hc/en-us/articles/19583625604887 |
| Create Category | Right-click the server icon or use the server-name chevron. Manage Channels. | https://support.discord.com/hc/en-us/articles/115001580171 |
| Create Event | Opens the Scheduled Event creator. Create Events. | https://support.discord.com/hc/en-us/articles/4409494125719 ; https://support.discord.com/hc/en-us/articles/1500005513722 |
| App Directory | Server Settings > App Directory (desktop/browser). Add App needs Manage Server. | https://support.discord.com/hc/en-us/articles/21334461140375 |
| Security Actions | Pause Invites and Pause DMs with durations. Needs Administrator, Kick, Timeout, Ban or Manage Server. | https://support.discord.com/hc/en-us/articles/17439993574167 |
| Server Boost | Boost This Server, plus the Server Boosts tab for admins. | https://support.discord.com/hc/en-us/articles/360028038352 ; https://discord.com/blog/discord-patch-notes-may-4-2026 |
| Server Insights | Shown in the server settings dropdown for holders of View Server Insights. | https://support.discord.com/hc/en-us/articles/360032807371 |
| Linked Roles | Member opt-in to Linked Roles from the server's Linked Roles tab. | https://support.discord.com/hc/en-us/articles/10388356626711 |
| Leave Server | Confirmation dialog; screen readers announce it (2026). An owner must transfer ownership first (unconfirmed). | https://discord.com/blog/discord-patch-notes-february-4-2026 ; https://discord.com/blog/discord-patch-notes-september-8-2026 |
| Copy Server ID / "Copy Server Info" submenu | Needs Developer Mode (User Settings > Advanced). In 2026, copying moved into a "Copy Server Info" submenu. | https://support.discord.com/hc/en-us/articles/206346498 ; https://discord.com/blog/discord-patch-notes-august-4-2026 ; https://discord.com/blog/discord-patch-notes-september-8-2026 |
| Context menu repositioning | The menu repositions to stay visible near the bottom of the list (2026 fixes). | https://discord.com/blog/discord-patch-notes-september-8-2026 |

### Server header, banner and boost display (top of channel list)

| Feature | Detail (states, dialogs, conditions such as permission/Nitro/DM-only) | Source URL |
|---|---|---|
| Server name header + dropdown chevron | Click the name or chevron for the server menu. | https://support.discord.com/hc/en-us/articles/115001580171 |
| Server banner | Static banner at Boost Level 2, animated at Level 3, shown at the top of the channel list. | https://support.discord.com/hc/en-us/articles/360028038352 |
| Server type badge in header | Community, Discoverable, Partnered or Verified badge with a tooltip. | https://support.discord.com/hc/en-us/articles/14078261239831 |
| Boost badge in header | Clicking it opens the boost page (2026 fix: no longer also opens the server menu). | https://discord.com/blog/discord-patch-notes-july-7-2026 |
| Boost progress bar / boost goal | Server-level toggle (premium_progress_bar_enabled) shows a boost progress bar or goal at the top of the channel list. | https://docs.discord.com/developers/resources/guild ; https://discord.com/blog/discord-patch-notes-october-6-2026 |
| Custom invite background (invite splash) | Boost Level 1 perk. | https://support.discord.com/hc/en-us/articles/360028038352 |
| Server Boosts tab | Top of the channel list or upper-left, for admins/owners with Manage Server. Shows available boosts, activity, perk enabling and boost reallocation. | https://support.discord.com/hc/en-us/articles/360028038352 |
| Live Stage topic | An open Stage's topic shows below the server title or banner. | https://support.discord.com/hc/en-us/articles/1500005513722 |

### Server tags (guild tags)

| Feature | Detail (states, dialogs, conditions such as permission/Nitro/DM-only) | Source URL |
|---|---|---|
| Tag display next to names | 4-character tag plus badge icon shown next to a member's name in chat, member lists and DM lists (2025). One tag at a time. | https://support.discord.com/hc/en-us/articles/31444248479639 ; https://discord.com/blog/discord-update-june-30-2025-changelog |
| Click tag → Server Profile | Opens the server's profile card with info, Join Server, or Apply (if the server uses Member Applications). | https://support.discord.com/hc/en-us/articles/31444248479639 |
| Adopt / remove a tag | User Settings > Profiles (or Edit User Profile) > Server Tag > Select; saves automatically. Remove with the X. Mobile editing added Sept 2026. | https://support.discord.com/hc/en-us/articles/31444248479639 ; https://discord.com/blog/discord-update-september-25-2026 |
| Server Tag Adoption screen | Opened from invites or tag CTAs (2026). | https://discord.com/blog/discord-patch-notes-july-7-2026 |
| Tag in profile previews | Desktop profile previews show the selected tag (2026). | https://discord.com/blog/discord-patch-notes-february-4-2026 |
| Non-clickable tags in report modals | Tags inside Report and Flag to Mods modals are inert (2026). | https://discord.com/blog/discord-patch-notes-october-6-2026 |
| Tag quarantined by AutoMod | Guild member flag AUTOMOD_QUARANTINED_GUILD_TAG hides a tag that breaks AutoMod rules. | https://docs.discord.com/developers/resources/guild |
| Unlock requirement | Owners or admins enable the Server Tags perk with 3 boosts, separate from Boost levels. The tag is removed from every profile if the boost count drops below the requirement. | https://support.discord.com/hc/en-us/articles/31444248479639 |
| Tag color and usage rules | Color set by the admin, the same for all members; admins can't restrict who displays the tag; tags aren't unique across servers. | https://support.discord.com/hc/en-us/articles/31444248479639 |
| Tag adoption coachmark | "Unlock a server tag with boosts" coachmark in the channel list (2026). | https://discord.com/blog/discord-patch-notes-september-8-2026 |
| Legacy Guilds experiment | The Guilds experiment (guild tags, applications, up to 200 members) has closed; Server Tags replaced it. | https://support.discord.com/hc/en-us/articles/23187611406999 |

## Channel list

### Structure, categories and ordering

| Feature | Detail (states, dialogs, conditions such as permission/Nitro/DM-only) | Source URL |
|---|---|---|
| Categories | Group channels; created from the server menu, then dragged anywhere. Up to 50 channels per category, 50 categories, 500 channels total. | https://support.discord.com/hc/en-us/articles/115001580171 ; https://support.discord.com/hc/en-us/articles/33694251638295 |
| Collapse / expand category | A collapsed category shows only channels with unreads; notifications continue. Collapsed state is stored per user (channel override `collapsed`). | https://support.discord.com/hc/en-us/articles/115001580171 ; https://docs.discord.food/resources/user-settings |
| Collapse All Categories | Category context-menu item that collapses every category (unconfirmed). | (unconfirmed, no official source found) |
| Mute category | Suppresses notifications; unread indicators still show. Mute plus collapse hides channels unless you or your role is mentioned. | https://support.discord.com/hc/en-us/articles/115001580171 |
| Category permission sync | Dragging a channel into a category prompts a permission sync. Channels are Synced or Not synced; "Sync Now" in channel permissions re-syncs. | https://support.discord.com/hc/en-us/articles/115001580171 |
| Edit Category | Right-click > Edit Category (Overview, Permissions). | https://support.discord.com/hc/en-us/articles/115001580171 |
| Hidden vs locked channels | Removing "View Channels" hides channels from members without access instead of showing them locked. | https://support.discord.com/hc/en-us/articles/115001580171 |
| Drag reorder channels | Manage Channels. Lists auto-scroll while dragging (2026). Only one parent change per move. | https://discord.com/blog/discord-patch-notes-september-8-2026 ; https://docs.discord.com/developers/resources/guild |
| Keyboard reorder channels | Ctrl/Cmd+D, arrows, Space/Enter, Esc. | https://support.discord.com/hc/en-us/articles/4408877527703 |
| "Uncategorized" target when moving a channel | Option in the category list when moving a channel (iOS fix 2026). | https://discord.com/blog/discord-patch-notes-june-4-2026 |
| Resizable channel list | Drag the edge to widen the sidebar (2025). The resize bar is not tab-focusable (2026). | https://discord.com/blog/discord-update-march-25-2025-changelog ; https://discord.com/blog/discord-patch-notes-july-7-2026 |
| Pin channels to top | Personal pin that moves channels to the top of your own channel list (June 2026); available in all servers, not only Community (July 2026). | https://discord.com/blog/discord-update-june-25-2026-changelog ; https://discord.com/blog/discord-patch-notes-july-7-2026 |
| "NEW" pill on new channels | Red NEW pill on newly created channels. Enabling Community no longer marks all channels NEW (2026 fixes). | https://discord.com/blog/discord-patch-notes-february-4-2026 ; https://discord.com/blog/discord-patch-notes-august-4-2026 |
| Navigate channels hotkeys | Alt+Up/Down (all), Alt+Shift+Up/Down (unread), Ctrl+Shift+Alt+Up/Down (unread with mentions); Esc = Mark Channel as Read. | https://support.discord.com/hc/en-us/articles/225977308 |
| Unread jump pill | A pill or bar jumps to unread channels off-screen (iOS fix 2026; desktop "Unread mentions" bar unconfirmed). | https://discord.com/blog/discord-patch-notes-july-7-2026 |

### Top-of-list entries

| Feature | Detail (states, dialogs, conditions such as permission/Nitro/DM-only) | Source URL |
|---|---|---|
| Events | Events entry with a count/badge. In Community servers it is always visible to Manage Events holders, and to others only when an event is upcoming. Event previews render Markdown (2026). | https://support.discord.com/hc/en-us/articles/4409494125719 ; https://discord.com/blog/discord-patch-notes-february-4-2026 ; https://discord.com/blog/discord-patch-notes-august-4-2026 |
| Server Guide | Shown when Server Guide is enabled (Community); welcome sign, to-dos, resource pages. | https://support.discord.com/hc/en-us/articles/13497665141655 |
| Channels & Roles | Onboarding questions not asked pre-join; members change answers here. Pending-application count shows next to the Members tab. | https://support.discord.com/hc/en-us/articles/11074987197975 |
| Browse Channels | Lists every channel you can view, so you can add or remove channels when onboarding has customized your list. Left beta on mobile in 2026. | https://support.discord.com/hc/en-us/articles/11074987197975 ; https://discord.com/blog/discord-patch-notes-february-4-2026 |
| Members | Shown in Community servers, or elsewhere via "Show Members in Channel List". Pending-application indicator. | https://support.discord.com/hc/en-us/articles/15946797617431 ; https://support.discord.com/hc/en-us/articles/29729107418519 |
| Server Boosts | Admin-only boost tab. | https://support.discord.com/hc/en-us/articles/360028038352 |
| Server Shop | Appears when Server Products are enabled (US, desktop/web experiment): Downloadables and Premium Roles. | https://support.discord.com/hc/en-us/articles/17633135983383 |
| Game Servers | Tab at the top of the server after the first game server is set up (experiment, 2025). | https://support.discord.com/hc/en-us/articles/35370817986839 |
| Active Now / Server Home (retired) | The Server Home beta (Highlights feed, Active Now, featured messages) was phased out on March 22, 2023. | https://support.discord.com/hc/en-us/articles/6156116949911 |

### Channel types and icons

| Feature | Detail (states, dialogs, conditions such as permission/Nitro/DM-only) | Source URL |
|---|---|---|
| Text channel (#) | GUILD_TEXT (0). To chat: View Channel, Send Messages, Read Message History. | https://support.discord.com/hc/en-us/articles/4412085582359 ; https://docs.discord.com/developers/resources/channel |
| Voice channel (speaker icon) | GUILD_VOICE (2). A lock icon means no Connect permission. | https://support.discord.com/hc/en-us/articles/19583625604887 |
| Text chat in voice | "Open Chat" bubble beside the voice channel name. When closed, up to 3 message previews (toggle "Show New Message Previews"). Unread badge: gray count, red for mentions. | https://support.discord.com/hc/en-us/articles/4412085582359 |
| Stage channel | GUILD_STAGE_VOICE (13); Community only. Open Stage is pinned to the top of the list with its topic. | https://support.discord.com/hc/en-us/articles/1500005513722 |
| Announcement channel (megaphone) | GUILD_ANNOUNCEMENT (5); Community only. Follow button; Publish button (10 per hour); an existing text channel can be converted. | https://support.discord.com/hc/en-us/articles/360032008192 ; https://support.discord.com/hc/en-us/articles/360028384531 |
| Forum channel | GUILD_FORUM (15); thread-only; Community only. | https://support.discord.com/hc/en-us/articles/6208479917079 |
| Media channel | GUILD_MEDIA (16); beta for servers with Server Subscriptions; gallery-like posts. | https://creator-support.discord.com/hc/en-us/articles/14346342766743 ; https://docs.discord.com/developers/resources/channel |
| Directory channel (Student Hubs) | GUILD_DIRECTORY (14): hub channel listing member servers. | https://docs.discord.com/developers/resources/channel ; https://support.discord.com/hc/en-us/articles/4406046651927 |
| Store channel (removed) | No longer in the channel-type table (legacy type 6) (unconfirmed removal details). | https://docs.discord.com/developers/resources/channel |
| Rules channel | Designated in Community Overview; required to enable Community. Special rules icon (unconfirmed visual). | https://support.discord.com/hc/en-us/articles/360047132851 |
| Community Updates channel | Moderators-only channel receiving Discord admin updates. | https://support.discord.com/hc/en-us/articles/360047132851 |
| Safety notifications channel | Receives safety updates (safety_alerts_channel_id). | https://support.discord.com/hc/en-us/articles/360047132851 ; https://docs.discord.com/developers/resources/guild |
| Private channel (lock badge) | Edit Channel > Permissions > Private Channel removes View Channel from @everyone. The lock is announced to screen readers (2026). | https://support.discord.com/hc/en-us/articles/214836687 ; https://discord.com/blog/discord-patch-notes-march-6-2026 |
| Age-restricted channel gate | Toggle in Edit Channel > Overview > Content Visibility (desktop/browser). Opening shows an adult-content notice; age assurance applies in some regions. | https://support.discord.com/hc/en-us/articles/115000084051 ; https://support.discord.com/hc/en-us/articles/40248967926167 |
| Spoiler channel | (2026) Content Visibility > Spoiler Channel toggle for text, voice, forum and announcement channels. A notice shows the name and topic; your opt-in is remembered. Not allowed with NSFW. Media channels show a spoiler icon. | https://support.discord.com/hc/en-us/articles/40248967926167 ; https://discord.com/blog/discord-update-august-11-2026-changelog ; https://discord.com/blog/discord-patch-notes-august-4-2026 |
| LFG channel | (2026 experiment) Forum-style live game invites; only in the official ARC Raiders server. | https://support.discord.com/hc/en-us/articles/41832532728215 |
| Premium (subscription-gated) channels | Channels gated to a Server Subscription tier role. | https://creator-support.discord.com/hc/en-us/articles/10423011974551 |
| Resource pages | Read-only channels converted into Server Guide resource pages; removed from the channel list. | https://support.discord.com/hc/en-us/articles/13497665141655 |
| Read-only / thread-only chat guard | Channels where you can't send show a chat guard or redirect. | https://discord.com/blog/discord-patch-notes-february-4-2026 |
| Channel name emoji | Channel names accept Unicode emoji only; no "Add Emoji" button (2026). | https://discord.com/blog/discord-patch-notes-april-6-2026 |
| Obfuscated channels | (Nov 16, 2026, API) Channels the user can't view are obfuscated or omitted. The client should not show hidden metadata. | https://docs.discord.com/developers/resources/channel |

### Channel states and badges

| Feature | Detail (states, dialogs, conditions such as permission/Nitro/DM-only) | Source URL |
|---|---|---|
| Unread state | Unread channels are highlighted; muted channels show no unread unless you are mentioned. Exact styling (white/bold vs grey) is unconfirmed. | https://support.discord.com/hc/en-us/articles/209791877 |
| Mention badge | Red count on channels with unread mentions. Screen readers announce badge updates (2026). | https://support.discord.com/hc/en-us/articles/4412085582359 ; https://discord.com/blog/discord-patch-notes-june-4-2026 |
| Unread behavior setting | Per channel or server: mark unread on all messages vs only mentions (flags UNREADS_ALL_MESSAGES / UNREADS_ONLY_MENTIONS); forum "new posts" unread toggle (unconfirmed UI). | https://docs.discord.food/resources/user-settings |
| Selected channel | The active channel is highlighted. | https://support.discord.com/hc/en-us/articles/4412085582359 |
| Hover actions | Hovering shows Create Invite and Edit Channel (cog). User-limit badge and hover icons swap on hover for voice, stage and text. | https://support.discord.com/hc/en-us/articles/208866998 ; https://discord.com/blog/discord-patch-notes-august-4-2026 |
| Slowmode indication | Members see that Slowmode is on and for how long. | https://support.discord.com/hc/en-us/articles/360016150952 |
| Activity icon | Activity indicator in the channel list (accessible label, 2026). | https://discord.com/blog/discord-patch-notes-april-6-2026 |

### Voice channel sidebar

| Feature | Detail (states, dialogs, conditions such as permission/Nitro/DM-only) | Source URL |
|---|---|---|
| Connected-user list under voice channel | Avatars and names nested under the channel. A collapsed voice channel shows user icons; clicking one opens the profile. | https://discord.com/blog/discord-patch-notes-july-7-2026 ; https://discord.com/blog/discord-patch-notes-april-6-2026 |
| Per-user voice state icons | Muted, deafened, server-muted, LIVE (Go Live), video and activity indicators. Muted status is announced to screen readers; exact icon set unconfirmed. | https://discord.com/blog/discord-patch-notes-april-6-2026 |
| User limit badge | Shows count / limit on voice and stage channels. | https://discord.com/blog/discord-patch-notes-august-4-2026 |
| Voice channel status | "Set a channel status" (text plus optional emoji, up to 500 chars) under the channel name. Needs Set Voice Channel Status; Manage Channels if not connected. | https://support.discord.com/hc/en-us/articles/19583625604887 ; https://docs.discord.com/developers/resources/channel |
| Active-call timer | Desktop shows how long a voice channel has been active (2025). | https://discord.com/blog/discord-update-november-6-2025-changelog |
| "Chat" and "Invite to Voice" buttons | On active voice channels. Invite to Voice suggests server members and close friends (2026). Voice invites show who is in the channel, user count and animated avatars (2026). | https://discord.com/blog/discord-update-march-24-2026-changelog ; https://discord.com/blog/discord-patch-notes-april-6-2026 ; https://discord.com/blog/discord-update-june-25-2026-changelog ; https://discord.com/blog/discord-patch-notes-june-4-2026 |
| Drag members between voice channels | Moderators with Move Members drag users, including guests. Targets include channels the member can't see (2026). | https://discord.com/blog/discord-patch-notes-july-7-2026 ; https://discord.com/blog/discord-patch-notes-may-4-2026 |
| AFK channel | Idle users are moved to the server's Inactive (AFK) channel; their status is preserved (2026). | https://discord.com/blog/discord-patch-notes-march-6-2026 ; https://discord.com/blog/discord-patch-notes-september-8-2026 |
| Living Room | Experimental voice feature in a small number of servers. | https://support.discord.com/hc/en-us/articles/19583625604887 |
| Stage: end stage from list | Moderators can end a stage from the channel context menu (Android parity with desktop, 2026). | https://discord.com/blog/discord-patch-notes-july-7-2026 |

### Announcement and Stage specifics

| Feature | Detail (states, dialogs, conditions such as permission/Nitro/DM-only) | Source URL |
|---|---|---|
| Follow button / dialog | In announcement channels: choose a destination server and channel (only those where you have Manage Webhooks; forum channels aren't valid targets). | https://support.discord.com/hc/en-us/articles/360028384531 ; https://discord.com/blog/discord-patch-notes-september-8-2026 |
| Publish button | Appears on messages in announcement channels. Own messages need Send Messages; any message needs Manage Messages. Hidden without permission (2026). 10 publishes per hour. | https://support.discord.com/hc/en-us/articles/360032008192 ; https://discord.com/blog/discord-patch-notes-august-4-2026 |
| Crossposted message rendering | Posted by a webhook named after the source server; role and @everyone mentions stripped; edits propagate; deletions show "[Original Message Deleted]" (CROSSPOSTED / IS_CROSSPOST / SOURCE_MESSAGE_DELETED flags). | https://support.discord.com/hc/en-us/articles/360032008192 ; https://docs.discord.com/developers/resources/message |
| "X has added Y to this channel" | CHANNEL_FOLLOW_ADD system message. | https://docs.discord.com/developers/resources/message |
| Start Stage dialog | Topic required; "Start the Stage" notifies the server; "Create Event"; "Continue without starting". Needs Manage Channel, Manage Permissions, Move Members, Mention @everyone and Manage Events. | https://support.discord.com/hc/en-us/articles/1500005513722 |
| Stage layout | Speakers on top, audience below, waiting music (mute via music-note button), raise hand, "Exit Quietly" (silent join and leave), text chat. Picture-in-Picture plus text-in-voice (2026). | https://support.discord.com/hc/en-us/articles/1500005513722 ; https://discord.com/blog/discord-patch-notes-october-6-2026 |
| Stage capacity | 10,000 audio-only. Video: 50 / 150 (L2) / 300 (L3), plus 30 per extra boost; 5 cameras and 1 Go Live. | https://support.discord.com/hc/en-us/articles/1500005513722 ; https://support.discord.com/hc/en-us/articles/33694251638295 |
| Stage system messages | STAGE_START, STAGE_END, STAGE_SPEAKER, STAGE_TOPIC. | https://docs.discord.com/developers/resources/message |

### Scheduled Events

| Feature | Detail (states, dialogs, conditions such as permission/Nitro/DM-only) | Source URL |
|---|---|---|
| Create Event: location type | Stage Channel (Create Events plus Stage Moderator), Voice Channel (99-participant cap), or Somewhere Else (text, link or address; location 1–100 chars; end time required). | https://support.discord.com/hc/en-us/articles/4409494125719 ; https://docs.discord.com/developers/resources/guild-scheduled-event |
| Event details form | Topic/name 1–100 chars, description 1–1000, start date/time, end date/time, cover image, privacy (guild-only). Description field wraps (2026). | https://docs.discord.com/developers/resources/guild-scheduled-event ; https://discord.com/blog/discord-patch-notes-september-8-2026 |
| Recurring events | Daily (weekday sets: Mon–Fri, Tue–Sat, Sun–Thu, Fri–Sat, Sat–Sun, Sun–Mon), Weekly (one day; interval 1 or 2 for every other week), Monthly (nth weekday), Yearly (month + day). Series events are readable individually (2026). | https://docs.discord.com/developers/resources/guild-scheduled-event ; https://discord.com/blog/discord-patch-notes-april-6-2026 |
| Preview before publishing | Preview step before the event is created. | https://support.discord.com/hc/en-us/articles/4409494125719 |
| Interested (RSVP) | Mark/unmark Interested; "# people are interested" button. Interested users are notified on start. Hidden on in-progress events (2026). | https://support.discord.com/hc/en-us/articles/4409494125719 ; https://discord.com/blog/discord-patch-notes-february-4-2026 ; https://discord.com/blog/discord-patch-notes-june-4-2026 |
| Event Details modal | Banner (cover), tab control (info / interested users) and location. Channel mentions in the location render. | https://discord.com/blog/discord-patch-notes-september-8-2026 ; https://discord.com/blog/discord-patch-notes-june-4-2026 |
| Start / Edit / Cancel event | Ellipsis menu on the event. Manage Events covers all events; Create Events only your own. | https://support.discord.com/hc/en-us/articles/4409494125719 |
| Event states | Scheduled → Active → Completed, or Scheduled → Canceled. Voice/stage events auto-complete after the channel empties; external events start and end at their times; unstarted events auto-cancel after a few hours. | https://docs.discord.com/developers/resources/guild-scheduled-event |
| "Live Now" event card | Shows Join, or "Go to Channel" if you are already in the channel (2026). | https://discord.com/blog/discord-patch-notes-august-4-2026 |
| Share event | Event server invite link (embed with RSVP; needs Create Invite; falls back to the vanity URL) or direct event link. Private-channel events use direct links. | https://support.discord.com/hc/en-us/articles/4409494125719 ; https://discord.com/blog/discord-patch-notes-june-4-2026 |
| Event visibility | Events in restricted channels are visible only to members with access. | https://support.discord.com/hc/en-us/articles/4409494125719 |
| Event limit | 100 scheduled or active events per server. | https://docs.discord.com/developers/resources/guild-scheduled-event |
| Mute New Events | Server notification option. | https://support.discord.com/hc/en-us/articles/215253258 |

### Onboarding, Server Guide, Welcome Screen

| Feature | Detail (states, dialogs, conditions such as permission/Nitro/DM-only) | Source URL |
|---|---|---|
| Pre-join onboarding questions | Shown during join. Options assign roles and/or channels. Single or multiple select, Required, emoji and description per option; multiple-choice or dropdown. | https://support.discord.com/hc/en-us/articles/11074987197975 ; https://docs.discord.com/developers/resources/guild |
| Post-join questions | Questions with "Ask before member joins" off appear in Channels & Roles. | https://support.discord.com/hc/en-us/articles/11074987197975 |
| Default channels | At least 7, with 5 or more writable by @everyone. Customized channel list ("opt-in channels"). | https://support.discord.com/hc/en-us/articles/11074987197975 |
| Rejoin re-onboarding | Members who leave and rejoin go through onboarding again. | https://support.discord.com/hc/en-us/articles/11074987197975 |
| Server Guide: Welcome Sign | Admin message at the top of the guide. | https://support.discord.com/hc/en-us/articles/13497665141655 |
| Server Guide: New Member To-Dos | 3–5 tasks linking to channels, with a member progress bar. | https://support.discord.com/hc/en-us/articles/13497665141655 ; https://discord.com/blog/discord-patch-notes-april-6-2026 |
| Server Guide: Resource Pages | Converted channels shown as pages: no message bar or avatars, opens at the top. | https://support.discord.com/hc/en-us/articles/13497665141655 |
| Welcome Screen (legacy) | Up to 5 recommended channels with descriptions and emoji. Not shown when Server Guide is enabled. | https://support.discord.com/hc/en-us/articles/360043913591 ; https://docs.discord.com/developers/resources/guild |
| Rules Screening bar | Pending members see a bottom bar, read the rules and agree ("Agree and Finish"); they can't talk, react or DM until they do. | https://support.discord.com/hc/en-us/articles/1500000466882 ; https://discord.com/blog/discord-patch-notes-may-4-2026 |
| Apply to Join pending state | While an application is pending, the applicant can't view server content. | https://support.discord.com/hc/en-us/articles/29729107418519 |

### Channel context menu (right-click channel)

| Feature | Detail (states, dialogs, conditions such as permission/Nitro/DM-only) | Source URL |
|---|---|---|
| Mark As Read | Same as Esc ("Mark Channel as Read"). | https://support.discord.com/hc/en-us/articles/225977308 |
| Invite People / Create Invite | Per-channel invite link. Needs Create Invite. | https://support.discord.com/hc/en-us/articles/208866998 |
| Copy Link | Copies the channel URL (unconfirmed label). | (unconfirmed, no official source found) |
| Mute Channel | Bell icon or menu; duration submenu like Mute Server (channel duration list unconfirmed). | https://support.discord.com/hc/en-us/articles/209791877 ; https://support.discord.com/hc/en-us/articles/218892547 |
| Notification Settings | All Messages / Only @mentions / Nothing, and "Use Category/Server Default" when inherited (2026). | https://support.discord.com/hc/en-us/articles/215253258 ; https://discord.com/blog/discord-patch-notes-july-7-2026 |
| Edit Channel | Settings modal (Overview, Permissions, Invites, Integrations). Manage Channels. | https://support.discord.com/hc/en-us/articles/10543994968087 |
| Duplicate Channel | Copies the channel and its permissions under a new name. | https://support.discord.com/hc/en-us/articles/214836687 |
| Create Text/Voice Channel | Creates a channel in that category. | https://support.discord.com/hc/en-us/articles/19583625604887 |
| Delete Channel | Confirmation; the reason is recorded in the Audit Log (2026). | https://discord.com/blog/discord-patch-notes-march-6-2026 |
| Copy Channel ID | Needs Developer Mode. | https://support.discord.com/hc/en-us/articles/206346498 |
| Add to Favorites / Remove from Favorites | Nitro Favorites experiment; can pick a category. Threads can't be favorited. Up to 200 items. | https://support.discord.com/hc/en-us/articles/38810584460439 |
| Pin channel | Pins the channel to the top of your list (2026). | https://discord.com/blog/discord-update-june-25-2026-changelog |
| Set a Channel Status | Voice channels; Title Case label (2026). | https://discord.com/blog/discord-patch-notes-february-4-2026 |
| Open Chat | Voice channels: opens text-in-voice. | https://support.discord.com/hc/en-us/articles/4412085582359 |
| Hide Names (voice) | Hides participant names in the sidebar voice list (unconfirmed). | (unconfirmed, no official source found) |

### Edit Channel settings

| Feature | Detail (states, dialogs, conditions such as permission/Nitro/DM-only) | Source URL |
|---|---|---|
| Create Channel modal | Pick a type (Text, Voice, Forum, Announcement, Stage, Media where available); name; Private Channel toggle, then add members or roles (with a Skip option). | https://support.discord.com/hc/en-us/articles/19583625604887 ; https://support.discord.com/hc/en-us/articles/214836687 ; https://discord.com/blog/discord-patch-notes-march-6-2026 |
| Overview: name and topic | Name with Unicode emoji only; topic up to 1024 chars (4096 for forum/media). | https://docs.discord.com/developers/resources/channel ; https://discord.com/blog/discord-patch-notes-april-6-2026 |
| Overview: Slowmode | Off, 5/10/15/30 seconds, 1/2/5/10/15/30 minutes, 1/2/6 hours. Advanced input disabled without permission. Mobile uses a dial. | https://support.discord.com/hc/en-us/articles/360016150952 ; https://discord.com/blog/discord-patch-notes-june-4-2026 |
| Overview: Content Visibility | Age-Restricted Channel toggle; Spoiler Channel toggle (2026; mutually exclusive). | https://support.discord.com/hc/en-us/articles/115000084051 ; https://support.discord.com/hc/en-us/articles/40248967926167 |
| Overview: Announcement Channel toggle | Converts a text channel to an announcement channel (Community). | https://support.discord.com/hc/en-us/articles/360032008192 |
| Overview: Hide After Inactivity (default) | Default auto-archive for new threads: 1h, 24h, 3d or 1w. | https://docs.discord.com/developers/resources/channel |
| Overview: Enable Channel Summaries | Per-channel toggle (Summaries experiment). | https://support.discord.com/hc/en-us/articles/12926016807575 |
| Voice: bitrate | Up to 96 / 128 / 256 / 384 kbps by Boost level. | https://support.discord.com/hc/en-us/articles/33694251638295 ; https://support.discord.com/hc/en-us/articles/11635925354775 |
| Voice: video quality | Auto or Full (720p). | https://docs.discord.com/developers/resources/channel |
| Voice: user limit | Cap on concurrent users, shown as a badge (limit range unconfirmed). | https://discord.com/blog/discord-patch-notes-august-4-2026 |
| Voice: region override | Per-channel RTC region (automatic by default) (unconfirmed UI label). | https://docs.discord.com/developers/resources/channel |
| Forum/media: guidelines, tags, default reaction, sort, layout, post slowmode, default thread slowmode, require tags | See Threads and forums. | https://docs.discord.com/developers/resources/channel |
| Media: hide media download options | Media channels only. | https://docs.discord.com/developers/resources/channel |
| Permissions tab | Add members or roles; Deny / Pass-through / Allow per permission; Sync Now; Private toggle; remove overwrite. | https://support.discord.com/hc/en-us/articles/10543994968087 ; https://support.discord.com/hc/en-us/articles/115001580171 |
| Invites tab | Channel invites (unconfirmed list UI). | https://support.discord.com/hc/en-us/articles/208866998 |
| Integrations tab | Channel webhooks, followed channels. | https://support.discord.com/hc/en-us/articles/360045093012 |
| Delete Channel | From settings or the context menu. | https://discord.com/blog/discord-patch-notes-march-6-2026 |

### Notifications (server, category, channel)

| Feature | Detail (states, dialogs, conditions such as permission/Nitro/DM-only) | Source URL |
|---|---|---|
| Server notification level | All Messages / Only @mentions / Nothing. The server-wide default is set by admins. | https://support.discord.com/hc/en-us/articles/215253258 ; https://docs.discord.com/developers/resources/guild |
| Suppress @everyone and @here | Toggle. | https://support.discord.com/hc/en-us/articles/215253258 |
| Suppress All Role @mentions | Toggle. | https://support.discord.com/hc/en-us/articles/215253258 |
| Suppress Highlights | Controls highlight emails. | https://support.discord.com/hc/en-us/articles/215253258 |
| Mute New Events | Toggle. | https://support.discord.com/hc/en-us/articles/215253258 |
| Mobile Push Notifications | Toggle. All-message push is disabled on servers over 2,500 members. | https://support.discord.com/hc/en-us/articles/215253258 ; https://support.discord.com/hc/en-us/articles/33694251638295 |
| Notification Overrides | Per channel or category level and mute. | https://support.discord.com/hc/en-us/articles/215253258 |

### Channel header

| Feature | Detail (states, dialogs, conditions such as permission/Nitro/DM-only) | Source URL |
|---|---|---|
| Channel name and topic | Topic shown in the header and truncated with an ellipsis; click for a details menu. Links inside the topic are focusable. | https://discord.com/blog/discord-patch-notes-june-4-2026 ; https://discord.com/blog/discord-patch-notes-august-4-2026 |
| Threads button | Opens the Threads browser for the channel. | https://support.discord.com/hc/en-us/articles/4403205878423 |
| Notification bell | Mute or notification settings for the channel. | https://support.discord.com/hc/en-us/articles/209791877 |
| Pinned Messages popout | Ctrl/Cmd+P. Jump and remove; remove needs Pin Messages (2026). Pin limit raised from 50 to 250 (2025). | https://support.discord.com/hc/en-us/articles/31232432266647 ; https://discord.com/blog/discord-patch-notes-june-4-2026 ; https://discord.com/blog/discord-update-september-25-2025-changelog |
| Member list toggle | Ctrl/Cmd+U. | https://support.discord.com/hc/en-us/articles/31232432266647 |
| Search | Ctrl/Cmd+F for the current channel, Ctrl/Cmd+Shift+F for all channels. Results include threads. | https://support.discord.com/hc/en-us/articles/31232432266647 ; https://support.discord.com/hc/en-us/articles/4403205878423 |
| Inbox | Ctrl/Cmd+I. Unreads and Mentions tabs, mention filters, Mark Inbox As Read, Jump. | https://support.discord.com/hc/en-us/articles/360045027712 |
| Help | Ctrl/Cmd+Shift+H. | https://support.discord.com/hc/en-us/articles/31232432266647 |
| Conversation Summaries | Experiment: header icon toggles the topic subheader and summaries panel; admins can disable it per channel or server-wide. | https://support.discord.com/hc/en-us/articles/12926016807575 |
| Back / Forward navigation | Top-left buttons and mouse buttons (2026). | https://discord.com/blog/discord-update-march-24-2026-changelog |
| Members list Recent Activity cards | Beta: activity cards at the top of the member list; "Hide Activity Cards". | https://support.discord.com/hc/en-us/articles/22045487931799 |

## DMs and friends

### Home view and DM list

| Feature | Detail (states, dialogs, conditions such as permission/Nitro/DM-only) | Source URL |
|---|---|---|
| Home / Direct Messages view | Home button on top of the server rail opens the DM sidebar (top entries + DM list) and the Friends page in the main pane. | https://support.discord.com/hc/en-us/articles/217674288 |
| Friends entry | Top of DM sidebar; opens Friends page (tabs Online / All / Pending / Blocked / Add Friend). Ctrl+1 from a server goes to Friends rather than the first DM (fix, 2026). | https://discord.com/blog/discord-patch-notes-june-4-2026 |
| Nitro entry | "Nitro tab under Direct Messages" on desktop; shows Nitro Home incl. evolving tenure badge progress. | https://support.discord.com/hc/en-us/articles/29136565881623 |
| Shop entry | "Shop tab" in the Direct Messages list (beside server list) for decorations, effects, nameplates, frames; Wishlist button on item cards. | https://support.discord.com/hc/en-us/articles/40775065582615 |
| Message Requests entry | Folder/tab in Home that appears only when ≥1 request is pending; shows notification badge. | https://support.discord.com/hc/en-us/articles/7924992471191 |
| Family Center tab in DM sidebar | Appears when "Enable Family Center Tab" is on (User Settings); has a close button in the DM sidebar (keyboard fix 2026). | https://discord.com/blog/discord-patch-notes-august-4-2026 |
| DM list rows | One row per open DM / group DM with avatar, name (friend nickname > display name), presence dot, custom status/activity line; group DMs show member count or icon. Sidebar holds max 100 DMs. | https://support.discord.com/hc/en-us/articles/33694251638295 |
| Presence dot on DM row | Green online, yellow idle, red DND, purple streaming; offline/invisible shows no dot and greyed avatar/name. | https://support.discord.com/hc/en-us/articles/211374998 |
| Unread DM badge | DMs and group DMs display a red badge with the unread count. | https://support.discord.com/hc/en-us/articles/360045027712 |
| Nameplate on DM row | User's nameplate shows behind the DM / group chat row on hover or selection (2025). | https://support.discord.com/hc/en-us/articles/30408457944215 |
| Server tag on DM row | Adopted server tag shows next to the name in DM lists (2025). | https://support.discord.com/hc/en-us/articles/31444248479639 |
| Display name style on DM row | DM list shows only the custom font; color/effect visible on hover (2025). | https://support.discord.com/hc/en-us/articles/33833879643927 |
| Ignored user DM dimmed | DM conversations with an ignored user appear dimmed with an Ignore icon. | https://support.discord.com/hc/en-us/articles/28084948873623 |
| Close DM ("x" on row) | Hover "x" closes (hides) the DM; reopening it restores history. (unconfirmed) | https://docs.discord.food/resources/channel |
| Pinned DMs / group DMs | Pin a DM or group DM so it stays at the top of the DM list (2026). | https://discord.com/blog/discord-update-june-25-2026-changelog |
| Favorite DM | "Add to Favorite" on a DM/GDM (right-click) adds it to the Favorites server at the top of the rail; categories, up to 200 items; Nitro experiment, desktop/browser edit only (2026). | https://support.discord.com/hc/en-us/articles/38810584460439 |
| "Show Favorites Server" toggle | User Settings > Chat; controls visibility of Favorites icon (2026). | https://support.discord.com/hc/en-us/articles/38810584460439 |
| Create DM / New Group DM button | "+" / New Group DM icon opens Create DM modal with friend picker and search (Create DM modal fix 2026). | https://discord.com/blog/discord-patch-notes-august-4-2026 |
| Friendly wave prompt | After adding someone new, Discord suggests sending a wave in the DM list (desktop, 2026); Wave button pill in DM list. | https://discord.com/blog/discord-update-june-25-2026-changelog |
| Unread DM avatars in server rail | Unread DMs appear as avatar bubbles with unread count above the server list. (unconfirmed — no official doc found) | — (no source found) |
| DM preview timestamps | DM rows show time since last message (mobile fix 2025). | https://discord.com/blog/discord-patch-notes-july-7-2025 |
| Compact mode DM list | DM list buttons render in Compact mode (fix 2026). | https://discord.com/blog/discord-patch-notes-july-7-2026 |

### DM row / DM header context menu and actions

| Feature | Detail (states, dialogs, conditions such as permission/Nitro/DM-only) | Source URL |
|---|---|---|
| Profile | Right-click DM row > Profile opens full profile modal. | https://support.discord.com/hc/en-us/articles/360036118732 |
| Add Friend Nickname | Right-click user avatar/row > "Add Friend Nickname"; private, only visible to you; overrides display name everywhere. | https://support.discord.com/hc/en-us/articles/12620128861463 |
| Add Note | Private per-user note (max 500 chars, 1,500 notes per account); editable from profile; Edit Note discard-changes modal (2026). Context-menu placement (unconfirmed). | https://support.discord.com/hc/en-us/articles/33694251638295 |
| Call / Video Call | Start voice (phone icon) or video call from DM header; mobile asks confirmation before ringing (2026). Context-menu "Call" item (unconfirmed). | https://support.discord.com/hc/en-us/articles/223657667 |
| Invite to Server | Opens invite-to-server modal listing servers with Invite buttons; searches usernames and display names (2026). | https://discord.com/blog/discord-patch-notes-february-4-2026 |
| Add Friend | Right-click a user (member list or message) > Add Friend; sends request with no confirmation pop-up. | https://support.discord.com/hc/en-us/articles/218344397 |
| Remove Friend | Removes friendship (confirmation dialog). (unconfirmed) | https://docs.discord.food/resources/relationships |
| Ignore / Unignore | Right-click avatar > Ignore + confirm; Unignore from Friends list right-click, profile ellipsis, or settings list. | https://support.discord.com/hc/en-us/articles/28084948873623 |
| Block / Unblock | Right-click user in Friends list or DM > Block + confirm. | https://support.discord.com/hc/en-us/articles/217916488 |
| Mute DM / Mute @user | Right-click group > Mute Notifications stops desktop and push notifications. Durations (15m/1h/8h/24h/until on) and 1:1 label "Mute @user" (unconfirmed). | https://support.discord.com/hc/en-us/articles/223657667 |
| Mark As Read | Mark a DM read from the row menu. (unconfirmed) Esc marks the current channel read. | https://support.discord.com/hc/en-us/articles/31232432266647 |
| Favorite | "Add to Favorite" / "Remove from Favorites" (Nitro experiment, 2026). | https://support.discord.com/hc/en-us/articles/38810584460439 |
| Pin / Unpin DM | Pin conversation to top of DM list (2026). | https://discord.com/blog/discord-update-june-25-2026-changelog |
| Apps submenu | Right-click user or message > Apps lists user/server-installed app commands. | https://support.discord.com/hc/en-us/articles/21334461140375 |
| Copy User ID | Requires Developer Mode (User Settings > Advanced); right-click user > Copy User ID. | https://support.discord.com/hc/en-us/articles/206346498 |
| Close DM / Leave Group | 1:1: Close DM (unconfirmed label); group: Leave Group with confirmation. | https://support.discord.com/hc/en-us/articles/223657667 |
| Accept Friend Request button in DM | DM with pending incoming request shows "Accept Friend Request"; after ignoring, button becomes "Add Friend" (2026); "Send Message" button after accepting (2026). | https://discord.com/blog/discord-patch-notes-april-6-2026 |
| DM header buttons | Voice call, video call, pinned messages, add friends to DM (creates group), user profile panel / member list toggle, search, Inbox. | https://support.discord.com/hc/en-us/articles/223657667 |
| DM profile side panel | Viewing a DM shows the other user's profile panel (incl. Wishlist section, mutuals); profile preview in DMs (2024). | https://support.discord.com/hc/en-us/articles/36288192746903 |
| "AKA" tooltip in DM | DM header shows an "AKA" tooltip listing other names (nicknames) for the user. | https://discord.com/blog/discord-patch-notes-september-3-2025 |
| DM call system messages | "started a call" system messages in DMs; jump to them from search. Dedicated call-history list: not found. | https://discord.com/blog/discord-patch-notes-september-8-2026 |
| DM calls rejoin after restart | Client rejoins a DM/GDM call after restart or update (2026). | https://discord.com/blog/discord-patch-notes-august-4-2026 |
| Activities in DM text chat | Launch Activities in DM / GDM text chats without a call (2024). | https://discord.com/blog/discord-update-july-26-2024-changelog |
| Message forwarding to DMs | Forward button on message hover; forward to DMs/channels; supports @silent (2024, 2026). | https://discord.com/blog/discord-update-november-18-2024-changelog |

### Friends page

| Feature | Detail (states, dialogs, conditions such as permission/Nitro/DM-only) | Source URL |
|---|---|---|
| Online tab | Friends who are online or idle. | https://support.discord.com/hc/en-us/articles/217674288 |
| All tab | Every accepted friend. | https://support.discord.com/hc/en-us/articles/217674288 |
| Pending tab | Incoming and outgoing requests; accept (check) / decline (x); cancel outgoing with X. Tab stays visible when only spam/blocked incoming requests remain (fix 2026). | https://support.discord.com/hc/en-us/articles/217674288 |
| View spam requests | Sub-section of Pending that holds requests flagged as spam. | https://support.discord.com/hc/en-us/articles/217674288 |
| Blocked tab | Lists blocked users. | https://support.discord.com/hc/en-us/articles/217674288 |
| Add Friend tab | Enter exact username > "Send Friend Request"; errors "Friend Request Failed" / "Hm, didn't work. Double check that the username is correct."; discriminator hint when typing "#". | https://support.discord.com/hc/en-us/articles/218344397 |
| Friend Suggestions | Suggestions list shown on Add Friend page (desktop; can push Add Friend button off-screen, fix 2025); remove suggestion. | https://discord.com/blog/discord-patch-notes-september-3-2025 |
| Active Now column | Right-side panel on Friends tab showing friends' current activities/voice; picks up custom theme gradient (fix 2026). | https://discord.com/blog/discord-patch-notes-august-4-2026 |
| Friend row actions | Hover row: Message button, More (ellipsis) menu; keyboard focus fixes (2026). | https://discord.com/blog/discord-patch-notes-june-4-2026 |
| New Group DM from Friends | New Group DM icon top-right of Friends list. | https://support.discord.com/hc/en-us/articles/223657667 |
| Friend limits | 1,000 accepted friends; 1,500 pending incoming / 1,000 outgoing; oldest incoming auto-pruned. | https://support.discord.com/hc/en-us/articles/218344397 |
| Unfamiliar-user warning | Alert before accepting a request from a user with no mutual friends and no shared server under 200 members; cannot be disabled. | https://support.discord.com/hc/en-us/articles/218344397 |
| Friend request notes | Attach a note to a friend request explaining why you're adding them (2026); up to 120 chars (unconfirmed limit). | https://discord.com/blog/discord-update-august-11-2026-changelog |
| Friend-online notifications | Choose how often you're notified when friends come online, or turn off; configurable as sender or receiver (2026). | https://discord.com/blog/discord-update-september-25-2026 |
| Friendship anniversary | Friendship anniversary prompt with GIF picker (2026; details unconfirmed). | https://discord.com/blog/discord-patch-notes-september-8-2026 |
| Game Friends | Add someone met in a game as a Game Friend (per-game) vs Discord Friend (2025; article under construction). | https://support.discord.com/hc/en-us/articles/29216909413143 |
| Find Your Friends (contact sync) | Mobile-only phone-contact matching; "Allow contacts to add me" toggle; desktop availability unconfirmed. | https://support.discord.com/hc/en-us/articles/360061878534 |
| Add friend by QR code | Mobile-only (Android confirmation fix 2026). | https://discord.com/blog/discord-patch-notes-april-6-2026 |
| Nitro friend trials | Nitro members can send 2-week friend passes/trials to up to 3 eligible friends; "Send Nitro Trials" / "Remind Friends" modals (2024–2026). | https://discord.com/blog/discord-update-june-20-2024-changelog |
| Riot friends-list linking | Discord friends list visible and invitable inside League of Legends / VALORANT (2026, in-game). | https://discord.com/blog/discord-update-june-25-2026-changelog |

### Friend requests and DM privacy settings

| Feature | Detail (states, dialogs, conditions such as permission/Nitro/DM-only) | Source URL |
|---|---|---|
| Friend Requests settings | Checkboxes Everyone / Friends of Friends / Server Members; deselect all blocks all requests. Under Content & Social, renamed "Messaging Permissions" (2026). | https://support.discord.com/hc/en-us/articles/217916488 |
| Direct messages toggle (server members) | Social permissions: allow DMs from non-friend server members; prompt to apply to all existing servers; per-server override via server Privacy Settings. | https://support.discord.com/hc/en-us/articles/217916488 |
| Message Requests toggle | User Settings > Messaging Permissions; adults only; per-server toggle in server dropdown > Privacy Settings. | https://support.discord.com/hc/en-us/articles/7924992471191 |
| DM spam filter | Default "filter all" applies spam filtering to every incoming DM; changeable; Spam folder can't be disabled. | https://support.discord.com/hc/en-us/articles/7924992471191 |
| Settings rename | "Content & Social" is now "Messaging Permissions"; "Activity" is now "Games & Apps" (User Settings redesign, 2026). | https://discord.com/blog/discord-update-august-11-2026-changelog |

### Friend nicknames and notes

| Feature | Detail (states, dialogs, conditions such as permission/Nitro/DM-only) | Source URL |
|---|---|---|
| Friend nickname display priority | Friend nickname > server nickname/display name in DMs; used in mention suggestions and group-DM search. 1–32 chars (unconfirmed). | https://support.discord.com/hc/en-us/articles/12620128861463 |
| Group DM search by nicknames | Group DMs can be found by included users' nicknames. | https://discord.com/blog/discord-patch-notes-september-3-2025 |
| Notes on users | Note field in profile popout/full profile; private; 500 chars; 1,500 notes. | https://support.discord.com/hc/en-us/articles/33694251638295 |

### Group DMs

| Feature | Detail (states, dialogs, conditions such as permission/Nitro/DM-only) | Source URL |
|---|---|---|
| Create group DM | From Friends list New Group DM icon; pick friends; must be friends; max 10 members incl. you. | https://support.discord.com/hc/en-us/articles/223657667 |
| Convert 1:1 to group | "Add Friends to DM" icon in a 1:1 creates a new group; original DM unchanged. | https://support.discord.com/hc/en-us/articles/223657667 |
| Add members | Any member can add friends via Add Member icon; capped at 10; invite button hidden when full (2026). | https://discord.com/blog/discord-patch-notes-september-8-2026 |
| Rename group | Click group name; visible to all; name max 100 chars (2026). | https://discord.com/blog/discord-patch-notes-may-4-2026 |
| Group icon | Upload custom icon or remove to revert; default shows grouped member avatars. | https://support.discord.com/hc/en-us/articles/223657667 |
| Group invite links | Any member on desktop creates via Add Friends icon; expire after 48 h; stop working when group has 10; blocked invitees can't use. | https://support.discord.com/hc/en-us/articles/360004851252 |
| Manage/revoke group invites | Owner right-clicks group icon > "Instant Invite"; X to revoke. | https://support.discord.com/hc/en-us/articles/360004851252 |
| Accept group invite at server cap | Accepting a GDM invite at server limit no longer shows "join more servers" upsell (fix 2026). | https://discord.com/blog/discord-patch-notes-september-8-2026 |
| Remove From Group (kick) | Only the creator/owner; right-click member in member list > Remove From Group. | https://support.discord.com/hc/en-us/articles/223657667 |
| Leave Group | Right-click group in DM list > Leave Group + confirm; rejoin only if re-added. | https://support.discord.com/hc/en-us/articles/223657667 |
| Leave silently | Option to leave without posting the leave system message. (unconfirmed) | https://docs.discord.food/resources/channel |
| Transfer group ownership | Owner can transfer ownership to another member. (unconfirmed) | https://docs.discord.food/resources/channel |
| Owner crown in group member list | Crown next to group owner. (unconfirmed) | — (no source found) |
| Clean Up Inactive Group DMs | Bulk-leave all group DMs without recent messages via a modal (2026). | https://discord.com/blog/discord-update-september-25-2026 |
| Group voice/video calls | Any member starts; call persists while ≥1 member remains; camera toggle. | https://support.discord.com/hc/en-us/articles/223657667 |
| Group member list | Toggle via member list icon; view profiles / DM members. | https://support.discord.com/hc/en-us/articles/223657667 |
| Group pins | Pin icon in header; any participant can pin. | https://support.discord.com/hc/en-us/articles/221421867 |
| Group mute | Right-click > Mute Notifications. | https://support.discord.com/hc/en-us/articles/223657667 |
| Blocked user in group alert | One-time alert when joining/returning to a group with a blocked user; leave or stay; leaving doesn't notify. | https://support.discord.com/hc/en-us/articles/217916488 |
| Group DM from overlay | Create group DM from game overlay (fix 2026). | https://discord.com/blog/discord-patch-notes-august-4-2026 |

### Message requests and spam

| Feature | Detail (states, dialogs, conditions such as permission/Nitro/DM-only) | Source URL |
|---|---|---|
| Message Requests inbox | Non-friend DMs routed here unless sender shares mutual friend or small server; on by default; forced on for teens/unconfirmed. | https://support.discord.com/hc/en-us/articles/7924992471191 |
| Accept / Ignore request | View profile (shared servers), accept, or ignore; ignored requests can't be retrieved. | https://support.discord.com/hc/en-us/articles/7924992471191 |
| Reply auto-accepts | Replying to a message request accepts it (fix 2026). | https://discord.com/blog/discord-patch-notes-june-4-2026 |
| Request preview restrictions | Text preview only; images/files hidden; links plain unclickable text. | https://support.discord.com/hc/en-us/articles/7924992471191 |
| Spam tab | Inside Message Requests; view profile, accept, report spam, preview and ignore. | https://support.discord.com/hc/en-us/articles/7924992471191 |
| Teen first-DM safety alert | Shown once per sender on first DM to self-reported teens: More Tips, Block, Mute, Dismiss, Report, thumbs feedback; not shown for requests/spam folders. | https://support.discord.com/hc/en-us/articles/18210977897239 |

### Block and Ignore

| Feature | Detail (states, dialogs, conditions such as permission/Nitro/DM-only) | Source URL |
|---|---|---|
| Block effects | No DMs either way; friendship removed; they can't send requests; their messages hidden for you; profile hidden unless "View Profile". | https://support.discord.com/hc/en-us/articles/28084948873623 |
| Blocked-message collapse | Messages from blocked users collapse into a "blocked messages" bar that can be expanded; endless-loop fix with many blocked messages (2026). (Bar wording unconfirmed) | https://discord.com/blog/discord-patch-notes-september-8-2026 |
| Blocked profile warning | Blocked/ignored user's profile is hidden behind a warning; View Profile reveals without notifying. | https://support.discord.com/hc/en-us/articles/217916488 |
| Block from member list | Select member > ellipsis > Block + confirm. | https://support.discord.com/hc/en-us/articles/217916488 |
| Game Invite prompt suppressed | "Game Invite" prompt not rendered for blocked users. | https://discord.com/blog/discord-patch-notes-july-7-2025 |
| Ignore effects | Hides their new messages (Show button reveals), no notifications, dimmed DMs, profile warning, warning before joining call with them, friendship kept, requests go to inbox silently; they can't tell (2024–2025). | https://support.discord.com/hc/en-us/articles/28084948873623 |
| Ignore after report | Report Message flow offers Ignore in "More you can do". | https://support.discord.com/hc/en-us/articles/28084948873623 |
| Blocked & ignored list | User Settings > Content & Social (now Messaging Permissions) > "Accounts you've blocked & ignored"; Unignore/Unblock per row; deleted ignored users removed (fix 2026). | https://support.discord.com/hc/en-us/articles/28084948873623 |

### Gifting, system DMs, Family Center

| Feature | Detail (states, dialogs, conditions such as permission/Nitro/DM-only) | Source URL |
|---|---|---|
| Gift button in chat bar | Gift icon right of the text box; choose Nitro/Shop item; DM-only full customization: card design, note, soundboard effect, emoji confetti. | https://support.discord.com/hc/en-us/articles/360020877112 |
| Gift from Wishlist | "Send a Gift" window shows friend's public Wishlist; gifted items auto-checked off (2025). | https://support.discord.com/hc/en-us/articles/36288192746903 |
| Gift Inventory | User Settings > Gift Inventory: find, revoke, regenerate unclaimed gift links. | https://support.discord.com/hc/en-us/articles/360020877112 |
| Discord Official Messages | DM from "Discord Updates" with OFFICIAL badge; can't reply (banner replaces composer); urgent ones trigger popup. | https://support.discord.com/hc/en-us/articles/360036118732 |
| APP label in system DM | App responses in system DM labeled "APP" not "OFFICIAL" (fix 2026). | https://discord.com/blog/discord-patch-notes-august-4-2026 |
| Clyde | Clyde AI chatbot removed (date unconfirmed); Clyde system bot still posts local error messages (e.g., nickname denied). | https://support.discord.com/hc/en-us/articles/219070107 |
| Family Center | Opt-in teen–parent link; QR connect on mobile; parent dashboard shows new friends, servers joined, users messaged/called (no content); weekly email. | https://support.discord.com/hc/en-us/articles/14155039712407 |

## Message display

### Message types and system messages

Every message `type` the client renders. Types 0–46 that appear in the official docs are confirmed. Types not in the official table come from the community user-API docs and are marked (unconfirmed). Deletable means a user with permission can delete it.

| Feature | Detail (states, dialogs, conditions such as permission/Nitro/DM-only) | Source URL |
|---|---|---|
| DEFAULT (0) | Normal user/bot message; renders content, embeds, attachments, stickers, components, poll. Deletable. | https://docs.discord.com/developers/resources/message |
| RECIPIENT_ADD (1) | "{author} added {user} to the group / thread." Group DM or thread only. Not deletable. | https://docs.discord.com/developers/resources/message |
| RECIPIENT_REMOVE (2) | "{author} removed {user}" / "{user} left the group". Group DM or thread only. Not deletable. | https://docs.discord.com/developers/resources/message |
| CALL (3) | DM/group DM call record: "{author} started a call" with a Join button while live; "that lasted N minutes" once ended; "You missed a call from {author}" variant. Carries `participants` and `ended_timestamp`. Not deletable. | https://docs.discord.com/developers/resources/message |
| CHANNEL_NAME_CHANGE (4) | "{author} changed the channel name: {name}" for group DMs and threads. Not deletable. | https://docs.discord.com/developers/resources/message |
| CHANNEL_ICON_CHANGE (5) | "{author} changed the channel icon." Group DM only. Not deletable. | https://docs.discord.com/developers/resources/message |
| CHANNEL_PINNED_MESSAGE (6) | "{author} pinned a message to this channel. See all pinned messages." Links to the pinned message and opens the pins list. The system message itself can't be pinned. Deletable. | https://support.discord.com/hc/en-us/articles/221421867 |
| USER_JOIN (7) | Welcome message. One of 13 templates chosen by `timestamp_ms % 13` ("{author} joined the party.", "A wild {author} appeared.", "Welcome, {author}. We hope you brought pizza.", and so on). Shows a "Wave to say hi!" sticker button (unconfirmed). Deletable. | https://docs.discord.food/resources/message (unconfirmed for template text) |
| GUILD_BOOST (8) | "{author} just boosted the server!" (or "boosted the server N times"). Deletable. | https://docs.discord.com/developers/resources/message |
| GUILD_BOOST_TIER_1/2/3 (9–11) | "{author} just boosted the server! {server} has achieved Level N!" Deletable. | https://docs.discord.com/developers/resources/message |
| CHANNEL_FOLLOW_ADD (12) | "{author} has added {source channel} to this channel. Its most important updates will show up here." Deletable. | https://docs.discord.com/developers/resources/message |
| GUILD_DISCOVERY_DISQUALIFIED / REQUALIFIED / GRACE_PERIOD_INITIAL_WARNING / GRACE_PERIOD_FINAL_WARNING (14–17) | Discovery eligibility notices in the system channel. Deletable. | https://docs.discord.com/developers/resources/message |
| THREAD_CREATED (18) | "{author} started a thread: {name}. See all threads." Clicking opens the thread. Deletable. | https://docs.discord.com/developers/resources/message |
| REPLY (19) | Normal message with a reply preview strip above it (see Replies below). Deletable. | https://docs.discord.com/developers/resources/message |
| CHAT_INPUT_COMMAND (20) | Bot response to a slash command, shown with an "{user} used /{command}" header (unconfirmed wording). Deletable. | https://docs.discord.com/developers/resources/message |
| THREAD_STARTER_MESSAGE (21) | First item in a thread: copy of the parent message, with fallback text if the parent is gone ("Sorry, we couldn't load the first message in this thread" (unconfirmed wording)). Not deletable. | https://docs.discord.com/developers/resources/message |
| GUILD_INVITE_REMINDER (22) | "Invite your friends" prompt for new servers, with an Invite button. Deletable. | https://docs.discord.com/developers/resources/message |
| CONTEXT_MENU_COMMAND (23) | Bot response to a user/message context-menu command, shown with an "{user} used {command}" header (unconfirmed wording). Deletable. | https://docs.discord.com/developers/resources/message |
| AUTO_MODERATION_ACTION (24) | AutoMod alert card in the alert channel. Rendered from `embeds[0]` (type `auto_moderation_message`): blocked content with the keyword highlighted, rule name, and actions. Only members with Manage Messages can delete it. | https://docs.discord.com/developers/resources/message |
| ROLE_SUBSCRIPTION_PURCHASE (25) | "{author} joined {tier} and has been a subscriber for N months". Deletable. | https://docs.discord.com/developers/resources/message |
| INTERACTION_PREMIUM_UPSELL (26) | App premium upsell message. Deletable. | https://docs.discord.com/developers/resources/message |
| STAGE_START / STAGE_END / STAGE_SPEAKER / STAGE_TOPIC (27, 28, 29, 31) | Stage lifecycle notices in the stage's text chat. Deletable. | https://docs.discord.com/developers/resources/message |
| STAGE_RAISE_HAND (30) | "{author} requested to speak". Deletable. (unconfirmed) | https://docs.discord.food/resources/message |
| GUILD_APPLICATION_PREMIUM_SUBSCRIPTION (32) | "{author} upgraded {app} to premium for this server". Deletable. | https://docs.discord.com/developers/resources/message |
| PREMIUM_REFERRAL (35) | Nitro referral (friend pass) gift message. (unconfirmed) | https://docs.discord.food/resources/message |
| GUILD_INCIDENT_ALERT_MODE_ENABLED / DISABLED (36, 37) | Security Actions notice: "{author} paused invites/DMs until {time}" / lifted. Deletable. | https://docs.discord.com/developers/resources/message |
| GUILD_INCIDENT_REPORT_RAID / REPORT_FALSE_ALARM (38, 39) | Raid reported / false alarm notices. Deletable. | https://docs.discord.com/developers/resources/message |
| GUILD_DEADCHAT_REVIVE_PROMPT (40) | Prompt after an hour of channel inactivity. (unconfirmed) | https://docs.discord.food/resources/message |
| CUSTOM_GIFT (41) | Gift card for a Shop or Nitro gift with a custom emoji or sound, rendered from `embeds[0].url` and `gift_info`. (unconfirmed) | https://docs.discord.food/resources/message |
| GUILD_GAMING_STATS_PROMPT (42) | Gaming stats prompt. (unconfirmed) | https://docs.discord.food/resources/message |
| PURCHASE_NOTIFICATION (44) | "{author} has purchased {product}!" for server products. Deletable. | https://docs.discord.com/developers/resources/message |
| POLL_RESULT (46) | "{author}'s poll {question} has closed", showing the winning answer, vote counts and a View Poll link. Rendered from a `poll_result` embed. Deletable. | https://docs.discord.com/developers/resources/message |
| CHANGELOG (47) | Changelog post from Discord Updates. (unconfirmed) | https://docs.discord.food/resources/message |
| NITRO_NOTIFICATION (48) | Nitro promotion card. (unconfirmed) | https://docs.discord.food/resources/message |
| CHANNEL_LINKED_TO_LOBBY (49) | Voice channel linked to a game lobby. (unconfirmed) | https://docs.discord.food/resources/message |
| GIFTING_PROMPT (50) | Local-only, ephemeral Nitro gifting prompt on a friendship anniversary. (unconfirmed) | https://docs.discord.food/resources/message |
| IN_GAME_MESSAGE_NUX (51) | Local-only notice about in-game messaging. (unconfirmed) | https://docs.discord.food/resources/message |
| GUILD_JOIN_REQUEST_ACCEPT / REJECT / WITHDRAWN_NOTIFICATION (52–54) | Results of a join application (servers that require applications). (unconfirmed) | https://docs.discord.food/resources/message |
| HD_STREAMING_UPGRADED (55) | "{author} activated HD streaming" (HD Splash Potion). (unconfirmed) | https://docs.discord.food/resources/message |
| REPORT_TO_MOD_DELETED_MESSAGE / TIMEOUT_USER / KICK_USER / BAN_USER / CLOSED_REPORT (58–62) | Outcome notices for member reports to moderators. (unconfirmed) | https://docs.discord.food/resources/message |
| PREMIUM_GROUP_INVITE (64) | Invite to join a group Nitro plan; special embed. (unconfirmed) | https://docs.discord.food/resources/message |
| VOICE_SESSION (65) | Voice hangout started in a small server. (unconfirmed) | https://docs.discord.food/resources/message |
| GUILD_BOOST_UPSELL (66) | Local-only "be the first to boost" prompt. (unconfirmed) | https://docs.discord.food/resources/message |
| FRIEND_REQUEST_ACCEPTED (67) | Friend request accepted notice in a DM. (unconfirmed) | https://docs.discord.food/resources/message |
| MEDIA_MENTION_MESSAGE (68) | Semantics undocumented. (unconfirmed) | https://docs.discord.food/resources/message |
| Removed types | 13 GUILD_STREAM, 33/34 private-channel integration added/removed, 43 POLL, 45 VOICE_HANGOUT_INVITE, 56/57 chat wallpaper set/remove, 63 EMOJI_ADDED. No longer sent, but they can still appear in history. (unconfirmed) | https://docs.discord.food/resources/message |

### Message chrome and author line

| Feature | Detail (states, dialogs, conditions such as permission/Nitro/DM-only) | Source URL |
|---|---|---|
| Cozy display | Default layout: avatar on the left and a header line with display name and timestamp. Consecutive messages from the same author are grouped without repeating the header; the grouping time window is (unconfirmed). | https://support.discord.com/hc/en-us/articles/217047657 |
| Compact display | User Settings > Appearance > Message Display > Compact. Hides avatars, puts timestamp and name inline, keeps role colors. Wumboji are not shown in compact mode. "Show avatars in compact mode" toggle (unconfirmed). | https://support.discord.com/hc/en-us/articles/217047657 |
| Space between message groups | Appearance slider (0–24px) (unconfirmed). Covered by the User Settings section. | https://support.discord.com/hc/en-us/articles/207260127 |
| UI density | Three density options, new in 2025. | https://discord.com/blog/discord-update-march-25-2025-changelog |
| Display name vs username vs nickname | Shows server nickname if set, otherwise global display name, otherwise username. | https://support.discord.com/hc/en-us/articles/12620128861463 |
| Role color display option | Accessibility: "Show role colors in names" / "Show role colors next to names" (dot) / "Don't show role colors". | https://support.discord.com/hc/en-us/articles/1500010454681 |
| Role color on name | Name colored by highest colored role. Enhanced role styles (glowing gradient/holographic, Boost-unlocked, 2025) render on names. | https://discord.com/blog/discord-update-june-30-2025-changelog |
| Display Name Styles | Nitro fonts, effects and colors (Solid, Gradient, Neon, Toon, Pop, Gummy, Prism). In server messages only the font shows; color and effect are replaced by role color. In DMs only the font shows until hover. Accessibility toggle turns them off globally. (2025) | https://support.discord.com/hc/en-us/articles/33833879643927 |
| Role icon | Role icon next to the name (Boost Level 2 server perk). | https://support.discord.com/hc/en-us/articles/4409571023639 |
| Server Tag | Server tag badge next to the name. Click opens a tag popover with server info and Join or Apply. (2025) | https://discord.com/blog/discord-update-june-30-2025-changelog |
| APP tag | Bot, app and webhook authors get an "APP" tag (formerly "BOT"), with a checkmark for verified apps (unconfirmed for checkmark). | https://discord.com/blog/discord-patch-notes-september-3-2025 |
| OFFICIAL / SYSTEM tag | Discord system messages (Discord Updates, Safety) carry an Official/System badge. | https://support.discord.com/hc/en-us/articles/360036118732 |
| OP tag | "OP" (Original Poster) tag on the author's messages in forum posts and threads. | https://discord.com/blog/discord-patch-notes-september-3-2025 |
| Server / crosspost label | Messages published from a followed announcement channel show the source server's name and icon as the author, with a SERVER-style tag (unconfirmed). "[Original Message Deleted]" when the source is deleted. | https://support.discord.com/hc/en-us/articles/360032008192 |
| Avatar decoration on avatar | Avatar decorations render around chat avatars (unconfirmed for chat); per-server decorations are a Nitro perk. | https://discord.com/blog/discord-update-april-3-2024-changelog |
| Timestamp | Relative ("Today at 14:03", "Yesterday at", date) in the header; a hover tooltip gives the full date and time. Compact mode shows the time at the left. (unconfirmed for formats) | https://docs.discord.com/developers/reference |
| Timeout icon | Moderators who can time out members see a timeout icon after the timestamp on a timed-out member's messages. Hidden from everyone else. | https://support.discord.com/hc/en-us/articles/4413305239191 |
| (edited) marker | "(edited)" after edited content, with a tooltip showing the edit time (unconfirmed for tooltip). No edit history is exposed. | https://docs.discord.com/developers/resources/message |
| Mention highlight | Messages that mention you (user, @everyone/@here, your role) get a highlighted background (yellow/gold bar) (unconfirmed for color). | https://support.discord.com/hc/en-us/articles/360037842511 |
| Click name or avatar | Opens the profile popout. Right-click opens the user context menu. Shift-click inserts a mention (unconfirmed). | https://support.discord.com/hc/en-us/articles/206346498 |
| Sending state | Pending messages render dimmed until acknowledged (unconfirmed). Deleting a message that is still sending cancels it (iOS fix, 2026). | https://discord.com/blog/discord-patch-notes-august-4-2026 |
| Failed state | Failed sends show in red with "Retry" and "Delete" options (unconfirmed). | (unconfirmed) |
| Suspended author hidden | Messages from suspended users can be hidden (flag HIDDEN_SUSPENDED_USER). Suspended accounts' names and avatars are stripped from reply previews. | https://discord.com/blog/discord-patch-notes-october-6-2026 |
| Spam-flagged messages | Messages flagged as spam are collapsed; you can jump to them from search. (2026) | https://discord.com/blog/discord-patch-notes-february-4-2026 |
| Emoji Confetti badge | "CONFETTI" badge on messages with Emoji Confetti applied. Hover triggers a burst that only you see; hovering the badge shows who applied it. Experimental, desktop, paid packs. | https://support.discord.com/hc/en-us/articles/29133681590679 |
| In-Channel Conversation Summaries | Experimental. Topic subheader with back/forward arrows and a Summaries panel (topic, snippet, participants) that jumps to the start of a conversation. Toggled by a header icon; admins enable it per channel or server. | https://support.discord.com/hc/en-us/articles/12926016807575 |

### Reply, forward, thread and crosspost rendering

| Feature | Detail (states, dialogs, conditions such as permission/Nitro/DM-only) | Source URL |
|---|---|---|
| Reply preview strip | Above a reply: a curved connector, the replied author's small avatar and name (role color), and a one-line content preview. Click jumps to the original. "@" before the name when the reply pinged (unconfirmed). Role mention colors and gradients are kept in the preview (2026). | https://support.discord.com/hc/en-us/articles/360057382374 |
| Reply to deleted message | Preview reads "Original message was deleted" (unconfirmed). | (unconfirmed) |
| Reply to blocked or ignored user | Preview reads "Blocked message" / hidden (unconfirmed). | https://support.discord.com/hc/en-us/articles/28084948873623 |
| Reply preview with attachment only | "Click to see attachment" with an image icon (unconfirmed). | (unconfirmed) |
| Forwarded message | "Forwarded" label with an arrow icon, then the snapshot content (text, embeds, attachments, stickers, components) without the original author. A source footer (server/channel and timestamp) is shown only to people with access to the source or if the server is public. Snapshot does not update on edit. Optional note from the forwarder appears as a normal message. | https://support.discord.com/hc/en-us/articles/24640649961367 |
| Forwarded media context menu | Right-clicking forwarded video offers media actions: "Stats for Nerds", "Copy Media Link", "Open Media Link". (2026) | https://discord.com/blog/discord-patch-notes-october-6-2026 |
| Thread starter preview | A message that started a thread shows a thread card beneath it: thread name, message count, the latest message preview, and "See Thread". | https://support.discord.com/hc/en-us/articles/4403205878423 |
| Crossposted (published) message | In following servers: identical rendering. Edits propagate. Deletion leaves "[Original Message Deleted]". @everyone/@here are not passed through. | https://support.discord.com/hc/en-us/articles/360032008192 |
| Ephemeral message | Visible only to you. Shows "Only you can see this • Dismiss message" (wording unconfirmed; "Dismiss this message" confirmed for Clyde/system). Disappears on reload. | https://docs.discord.com/developers/interactions/receiving-and-responding |
| Interaction header | Bot responses to commands show "{user} used /{command}" (or "{user} used {App}") with the invoker's avatar above the response (unconfirmed wording; data from `interaction_metadata`). | https://docs.discord.com/developers/resources/message |
| Deferred "thinking" state | "{Bot} is thinking..." loading message (flag LOADING) until the app edits it. 15-minute token lifetime; the app has 3 seconds to respond initially. "The application did not respond" on failure (unconfirmed wording). | https://docs.discord.com/developers/interactions/receiving-and-responding |

### Markdown and inline formatting

| Feature | Detail (states, dialogs, conditions such as permission/Nitro/DM-only) | Source URL |
|---|---|---|
| Italic | `*text*` or `_text_` | https://support.discord.com/hc/en-us/articles/210298617 |
| Bold | `**text**`; bold italic `***text***` | https://support.discord.com/hc/en-us/articles/210298617 |
| Underline | `__text__`, combinable with bold/italic | https://support.discord.com/hc/en-us/articles/210298617 |
| Strikethrough | `~~text~~` | https://support.discord.com/hc/en-us/articles/210298617 |
| Spoiler | `\|\|text\|\|`. Click to reveal; leaving the channel re-hides it. Not hidden inside code blocks. Link spoilers work, invite embeds can't be spoilered. Display setting: On click / On servers I moderate / Always. | https://support.discord.com/hc/en-us/articles/360022320632 |
| Headers | `# `, `## `, `### ` at line start, followed by a space | https://support.discord.com/hc/en-us/articles/210298617 |
| Subtext | `-# text` at line start (small grey text). Links inside subtext render as hyperlinks (2026). | https://support.discord.com/hc/en-us/articles/210298617 |
| Masked links | `[text](url)`. A "Leaving Discord" confirmation with a "trust this domain" option appears on click (unconfirmed wording). | https://support.discord.com/hc/en-us/articles/210298617 |
| Lists | `- ` or `* ` bullets, `1.` numbered, two-space indent for nesting. Reply previews keep the start number of ordered lists. | https://support.discord.com/hc/en-us/articles/210298617 |
| Inline code | `` `code` ``. Inline code inside links uses a distinct color (2026). | https://support.discord.com/hc/en-us/articles/210298617 |
| Code blocks | Triple backticks with an optional language tag for syntax highlighting (highlight.js languages) (unconfirmed for language list); ` ```ansi ` color escapes (unconfirmed). | https://support.discord.com/hc/en-us/articles/210298617 |
| Block quotes | `> ` single line; `>>> ` quotes the rest of the message | https://support.discord.com/hc/en-us/articles/210298617 |
| Escaping | Backslash escapes markdown and mentions; escaped mentions no longer ping (2026 desktop fix). | https://discord.com/blog/discord-patch-notes-october-6-2026 |
| Autolinks | Bare URLs become links. `<url>` suppresses the embed. | https://support.discord.com/hc/en-us/articles/206342858 |
| Email links | `<user@example.com>` becomes a clickable mailto link. (2025) | https://discord.com/blog/discord-update-june-30-2025-changelog |
| Jumbo emoji (Wumboji) | Emoji-only messages render large, up to 27 emoji. Not in compact mode. | https://support.discord.com/hc/en-us/articles/360041139231 |
| Emoticon conversion | `:)` and similar convert to emoji (setting `convert_emoticons`) (unconfirmed). | https://docs.discord.food/resources/user-settings-proto |
| `/me` rendering | `/me text` sends italic text (unconfirmed). | (unconfirmed) |
| Unsupported markdown | No tables, no images via markdown, no horizontal rules (unconfirmed). | (unconfirmed) |

### Mentions and rich tokens

| Feature | Detail (states, dialogs, conditions such as permission/Nitro/DM-only) | Source URL |
|---|---|---|
| User mention `<@id>` | Blurple pill "@name". Click opens the profile popout. An unknown user shows "@unknown-user" with Copy User ID. | https://docs.discord.com/developers/reference |
| Role mention `<@&id>` | Pill tinted with the role color (gradient for enhanced styles). Click lists up to 100 members with the role (2026). | https://discord.com/blog/discord-update-march-24-2026-changelog |
| @everyone / @here | Rendered as mentions; pings only if the author has Mention Everyone. | https://docs.discord.com/developers/resources/message |
| Channel mention `<#id>` | Pill with channel-type icon and name. Click navigates. Channels with emoji in their names render as pills (2026). No-access channels render as "#No Access" (unconfirmed wording). | https://docs.discord.com/developers/reference |
| Slash command mention `</name:id>` | Clickable. Click puts the command in the composer. Subcommand and subcommand-group forms are supported. | https://docs.discord.com/developers/reference |
| Game mention `<@$id>` / @game | Game profile pill; click opens the Game Profile page. Desktop composer `@game` search. (2026) | https://discord.com/blog/discord-update-august-11-2026-changelog |
| Custom emoji `<:name:id>` / `<a:name:id>` | Inline image; animated if `a:`. Hover shows `:name:`. Click opens the emoji popout (source server, Join/Nitro upsell) (unconfirmed for popout). Without Nitro, external emoji appear as text in the composer. | https://docs.discord.com/developers/reference |
| Unicode emoji | Rendered as Twemoji on desktop. | https://docs.discord.com/developers/reference |
| Timestamps `<t:unix:style>` | Styles t, T, d, D, f (default), F, s, S, R (relative, live-updating). Shown in the viewer's timezone; hover shows the full date. Desktop `@time` composer command inserts one. (2026) | https://docs.discord.com/developers/reference |
| Guild navigation `<id:customize>` etc. | `<id:customize>` (Channels & Roles), `<id:browse>` (Browse Channels), `<id:guide>` (Server Guide), `<id:linked-roles>`, `<id:linked-roles:roleId>` (opens connection modal). | https://docs.discord.com/developers/reference |
| Message links | `https://discord.com/channels/{guild}/{channel}/{message}` (or `@me` for DMs). Click jumps in-app; renders as a link with channel context (unconfirmed pill rendering). | https://support.discord.com/hc/en-us/articles/206346498 |
| Self-mention highlight | Mentioning yourself does not add the highlight in the composer (known issue). | https://support.discord.com/hc/en-us/articles/360037842511 |

### Embeds

| Feature | Detail (states, dialogs, conditions such as permission/Nitro/DM-only) | Source URL |
|---|---|---|
| Rich embed (`rich`) | Left color bar, author (icon, name, url), title (url), description (markdown), up to 25 fields (inline/non-inline grid), image, thumbnail, footer (icon, text), timestamp. Limits: title 256, description 4096, field name 256 / value 1024, footer 2048, author 256, 6000 total; up to 10 embeds per message. Embeds sharing a URL are deduplicated. | https://docs.discord.com/developers/resources/message |
| Multi-image embed gallery | Several embeds with the same `url` and different images merge into one image grid (unconfirmed). | (unconfirmed) |
| Image embed (`image`) | A direct image link renders the image inline (proxied). | https://docs.discord.com/developers/resources/message |
| Video embed (`video`) | Inline player. YouTube, Twitch etc. play in place; vertical YouTube Shorts get a taller frame (2026). | https://discord.com/blog/discord-patch-notes-august-4-2026 |
| GIFV (`gifv`) | GIF provider links (GIPHY, Tenor/Klipy) render as looping muted video; play/pause follows the GIF autoplay setting. | https://docs.discord.com/developers/resources/message |
| Article / link (`article`, `link`) | Open Graph cards (site name, title, description, image) built by Discordbot from title, description and image. | https://support.discord.com/hc/en-us/articles/42500550752919 |
| Spotify embed | Inline Spotify player, including `open.spotify.com/s/` short links (2026). | https://discord.com/blog/discord-patch-notes-august-4-2026 |
| TikTok embed playback | TikTok videos play inline on desktop. (2024) | https://discord.com/blog/discord-update-november-18-2024-changelog |
| Tumblr post embeds | (2025) | https://discord.com/blog/discord-update-september-25-2025-changelog |
| AV1 video | AV1 supported in attachments and embeds (2025). HDR AV1 thumbnails are tonemapped (2026). | https://discord.com/blog/discord-update-september-25-2025-changelog |
| Poll result embed (`poll_result`) | Closed-poll card (question, winner, votes). | https://docs.discord.com/developers/resources/message |
| AutoMod embeds | `auto_moderation_message` (alert) and `auto_moderation_notification` (incident). (unconfirmed for the second) | https://docs.discord.food/resources/message |
| Safety embeds | `safety_policy_notice`, `safety_system_notification`, `age_verification_system_notification` in system DMs. (unconfirmed) | https://docs.discord.food/resources/message |
| Gift embed (`gift`) | Nitro/Shop gift card with Accept/Claim button. States: unclaimed, claimed, expired (unconfirmed). | https://docs.discord.food/resources/message |
| Post preview (`post_preview`) | Preview of a linked paywalled media-channel post. (unconfirmed) | https://docs.discord.food/resources/message |
| Invite embed | discord.gg links render a server card (icon, name, online/member counts, Join/Joined button, expired/invalid state) (unconfirmed). Can't be hidden with spoilers. | https://support.discord.com/hc/en-us/articles/360022320632 |
| Event invite embed | Invite links with `?event=` render a scheduled-event card with an Interested button (unconfirmed). | (unconfirmed) |
| Activity / game invite embed | Rich Presence invites (Join / Ask to Join / Spectate; "Listen Along" for Spotify) from message `activity` types JOIN, SPECTATE, LISTEN, JOIN_REQUEST, STREAM_REQUEST. "Join" vs "Ask to Join" depends on activity privacy (2026). | https://docs.discord.com/developers/resources/message |
| Request to Stream embed | Ask a friend to stream their game (STREAM_REQUEST), with a cooldown on repeated clicks (2026). | https://discord.com/blog/discord-patch-notes-july-7-2026 |
| Suppressed embeds | Flag SUPPRESS_EMBEDS. The author or a Manage Messages member removes embeds with the X on the embed ("Remove Embed"/"Remove all embeds" confirmation) (unconfirmed for UI). | https://docs.discord.com/developers/resources/message |
| Embed display settings | Text & Images: "Show embeds and preview website links" / auto-embed when posted as links and when uploaded directly. | https://support.discord.com/hc/en-us/articles/206342858 |
| Embed Links permission | Without it, links don't embed (unconfirmed for UI). | https://docs.discord.com/developers/topics/permissions |

### Attachments and media

| Feature | Detail (states, dialogs, conditions such as permission/Nitro/DM-only) | Source URL |
|---|---|---|
| Image attachments | Inline preview, sized to fit. Multiple images/videos/GIFs form a mosaic grid. Click opens the media viewer. | https://support.discord.com/hc/en-us/articles/25444343291031 |
| Alt text (ALT badge) | When "With image descriptions" is enabled (Text & Images), an "ALT" label appears; select it to read the description (up to 1024 chars). | https://support.discord.com/hc/en-us/articles/25444343291031 |
| Media viewer (lightbox) | Full-window viewer with carousel between images, zoom, "Open in Browser", Copy Image (WebP and AVIF supported, 2026), Save Image (unconfirmed for full list). Tall portrait images fit. | https://discord.com/blog/discord-patch-notes-august-4-2026 |
| Video attachments | Inline player (play/pause, seek, volume, fullscreen, download). MP4/MOV with H.264, HEVC or AV1. Portrait video can go fullscreen. Maximizing into the viewer stops the inline copy. Right-click "Stats for Nerds" shows codec and bitrate (2026). | https://discord.com/blog/discord-patch-notes-february-4-2026 |
| Audio attachments | Inline audio player with filename, play/pause, seek, volume, download (unconfirmed for controls). | https://support.discord.com/hc/en-us/articles/25444343291031 |
| Voice message player | Play button, waveform (from `waveform`, at most 256 samples), duration (`duration_secs`), flag IS_VOICE_MESSAGE. Playback on all platforms; recording is mobile-only. Up to 20 minutes. Playback speed toggle (unconfirmed). | https://support.discord.com/hc/en-us/articles/13091096725527 |
| Text file preview | Plain-text uploads show an inline preview; Expand shows up to 100 lines; a popup shows up to 50 KB. | https://support.discord.com/hc/en-us/articles/1500005466681 |
| Generic file card | Non-previewable files (PDF, DOC…) show an icon, name, size and download link. | https://support.discord.com/hc/en-us/articles/25444343291031 |
| Spoiler attachments | IS_SPOILER flag (or `SPOILER_` filename prefix (unconfirmed)). Blurred with a "SPOILER" overlay; click to reveal. | https://support.discord.com/hc/en-us/articles/360022320632 |
| Sensitive media blur | Mature/graphic image and video detection. Per-context Show/Blur/Block (DMs from friends, DMs from others, servers). Blurred cover reads "May contain sensitive content", with an eye to reveal, a "Why we hide some media" dialog, and "Mark as Not Sensitive" via right-click. Teens can't unblur. | https://support.discord.com/hc/en-us/articles/18210995019671 |
| Clips | Clip attachments (IS_CLIP) with title, clip participants and creation time. | https://docs.discord.com/developers/resources/message |
| Animated attachments | IS_ANIMATED flag; GIFs autoplay per setting and pause when the window is unfocused if configured. | https://discord.com/blog/discord-patch-notes-august-4-2026 |
| Thumbhash placeholder | Low-res placeholder before media loads (`placeholder`). (unconfirmed) | https://docs.discord.food/resources/message |
| Attachment display setting | Text & Images: display images, videos and lolcats "When uploaded directly to Discord". Images over 20MB uploaded directly are shared as downloads. | https://support.discord.com/hc/en-us/articles/25444343291031 |
| Expiring CDN links | Attachment URLs are signed and expire; the client refreshes them (unconfirmed). | (unconfirmed) |

### Stickers

| Feature | Detail (states, dialogs, conditions such as permission/Nitro/DM-only) | Source URL |
|---|---|---|
| Sticker rendering | PNG, APNG, Lottie and GIF. Max 320x320, animated stickers up to 5 s. Standard (Discord packs) and server stickers. | https://docs.discord.com/developers/resources/sticker |
| Sticker animation setting | Accessibility: "Always animate" / "Animate on interaction" / "Never animate". | https://support.discord.com/hc/en-us/articles/1500010454681 |
| Sticker popout | Clicking a sticker shows its name and source (pack or server) with a Join or Nitro upsell (unconfirmed). | (unconfirmed) |
| Sticker alt text | Server stickers can carry a description for screen readers. | https://support.discord.com/hc/en-us/articles/4403089981975 |

### Components (bots and apps)

| Feature | Detail (states, dialogs, conditions such as permission/Nitro/DM-only) | Source URL |
|---|---|---|
| Action Row (1) | Up to 5 buttons or 1 select. | https://docs.discord.com/developers/components/reference |
| Button (2) | Styles Primary (blurple), Secondary (grey), Success (green), Danger (red), Link (opens URL, external icon), Premium (SKU purchase). Label up to 80 chars, optional emoji, disabled state. Click shows loading while the app responds. | https://docs.discord.com/developers/components/reference |
| String Select (3) | Dropdown with up to 25 options (label, description, emoji, default), placeholder, min/max values, disabled. | https://docs.discord.com/developers/components/reference |
| User / Role / Mentionable / Channel Select (5–8) | Auto-populated selects with search; default values; channel-type filter. | https://docs.discord.com/developers/components/reference |
| Section (9) (V2) | 1–3 Text Displays plus a Button or Thumbnail accessory. | https://docs.discord.com/developers/components/reference |
| Text Display (10) (V2) | Markdown text block with mentions. | https://docs.discord.com/developers/components/reference |
| Thumbnail (11) (V2) | Image accessory with alt text and spoiler. | https://docs.discord.com/developers/components/reference |
| Media Gallery (12) (V2) | 1–10 media items (images/videos) with descriptions and spoilers, laid out as a mosaic. | https://docs.discord.com/developers/components/reference |
| File (13) (V2) | Attached file card (`attachment://`) with spoiler. | https://docs.discord.com/developers/components/reference |
| Separator (14) (V2) | Divider line on/off, spacing small or large. | https://docs.discord.com/developers/components/reference |
| Container (17) (V2) | Card with an accent color bar holding rows, text, sections, galleries, separators and files; can be spoilered. | https://docs.discord.com/developers/components/reference |
| IS_COMPONENTS_V2 messages | Flag 1<<15. Content and embeds are replaced by components. Up to 40 components per message. | https://docs.discord.com/developers/resources/message |
| Modals | Popup dialog (title up to 45 chars, 1–5 components) opened by a command or component. Children: Label (18) wrapping Text Input (short/paragraph), selects, File Upload (19), Radio Group (21), Checkbox Group (22), Checkbox (23). Submit and Cancel buttons. | https://docs.discord.com/developers/components/reference |
| Autocomplete choices | Slash-command option suggestions from the app (up to 25). | https://docs.discord.com/developers/interactions/receiving-and-responding |
| Launch Activity response | Command or button can open the app's Activity. | https://docs.discord.com/developers/interactions/receiving-and-responding |
| Premium required (deprecated) | Upgrade button response. | https://docs.discord.com/developers/interactions/receiving-and-responding |
| Checkpoint card | `with_checkpoint` adds a Discord Checkpoint (year-in-review) card component. (2025) (unconfirmed) | https://docs.discord.food/resources/message |

### Polls (rendering)

| Feature | Detail (states, dialogs, conditions such as permission/Nitro/DM-only) | Source URL |
|---|---|---|
| Poll card | Question (up to 300 chars), up to 10 answers (55 chars each, optional emoji), "Select one/multiple answers" hint, time remaining, total votes. | https://docs.discord.com/developers/resources/poll |
| Voting | Select answers, then Vote. A check mark marks your choice. "Remove Vote" lets you change it while open. No anonymous voting. | https://support.discord.com/hc/en-us/articles/22163184112407 |
| Results display | Percent bars and counts after voting or after close; "Show results" without voting (unconfirmed). Counts are approximate until finalized. | https://docs.discord.com/developers/resources/poll |
| Voters list | Per-answer list of who voted (API "Get Answer Voters"); UI dialog (unconfirmed). | https://docs.discord.food/resources/message |
| End poll early | The creator can end the poll early ("End Poll" in the message menu (unconfirmed label)). | https://support.discord.com/hc/en-us/articles/22163184112407 |
| Poll closed | Final tally, plus a POLL_RESULT system message. | https://docs.discord.com/developers/resources/message |
| Poll restrictions | Polls can't be edited or forwarded; deleting the message deletes the poll; polls can be pinned. | https://support.discord.com/hc/en-us/articles/22163184112407 |

### Message list structure

| Feature | Detail (states, dialogs, conditions such as permission/Nitro/DM-only) | Source URL |
|---|---|---|
| Channel start header | "Welcome to #channel! This is the start of the #channel channel." with an Edit Channel button for managers (unconfirmed wording). | (unconfirmed) |
| DM start header | Large avatar, name, username, "This is the beginning of your direct message history with @user", mutual servers, and Add Friend / Remove Friend / Block / Report Spam buttons (unconfirmed for button set). Profile previews in DMs (2024). | https://discord.com/blog/discord-update-june-20-2024-changelog |
| Date dividers | Horizontal rule with the date between days (unconfirmed). | (unconfirmed) |
| Unread "NEW" divider | Red line labelled "NEW" before the first unread message (unconfirmed). | (unconfirmed) |
| New-messages bar | Top banner "N new messages since {time}" with "Mark As Read" (Esc) and click-to-jump to the oldest unread (Shift+PageUp) (unconfirmed for banner text). | https://support.discord.com/hc/en-us/articles/225977308 |
| Jump to Present | Bar "You're viewing older messages" with "Jump To Present" when scrolled away from the bottom (unconfirmed). | (unconfirmed) |
| Jump-to-message highlight | Jumping from search, reply, pins or a link scrolls to and briefly highlights the target (unconfirmed). | https://support.discord.com/hc/en-us/articles/221421867 |
| Blocked user messages | Collapsed to "N Blocked Message(s) — Show message" (unconfirmed wording). Blocked users' profiles are hidden by default with a View Profile option. | https://support.discord.com/hc/en-us/articles/217916488 |
| Ignored user messages | Hidden behind "Show". DMs from ignored users are darkened with an Ignore icon. (2025) | https://support.discord.com/hc/en-us/articles/28084948873623 |
| Typing indicator | Bottom-of-chat "{name} is typing…", "{a} and {b} are typing…", "Several people are typing…" (unconfirmed for multi-person text). Custom Typing Indicator (emoji, animation, word) for Nitro, mobile-only experiment, 2026. | https://support.discord.com/hc/en-us/articles/42962943077271 |
| Slowmode notice | Slowmode is shown on the channel, with a countdown before you can post again (unconfirmed for countdown). | https://support.discord.com/hc/en-us/articles/360016150952 |
| Text-in-voice previews | With voice chat closed, new messages pop up as previews in the voice view (up to 3, truncated). "Show New Message Previews" toggle. Red/grey unread counts. | https://support.discord.com/hc/en-us/articles/4412085582359 |
| Lost access | Losing access to a channel you're viewing removes you from it (2026). | https://discord.com/blog/discord-patch-notes-october-6-2026 |
| Load older/newer history | Infinite scroll in both directions, with skeleton placeholders while loading (unconfirmed). | (unconfirmed) |

## Message actions

### Hover action bar

| Feature | Detail (states, dialogs, conditions such as permission/Nitro/DM-only) | Source URL |
|---|---|---|
| Suggested reactions | Your top 3 emoji appear as one-click reactions (2024). | https://discord.com/blog/discord-update-november-18-2024-changelog |
| Add Reaction | Smiley icon opens the emoji picker. Shift-click in the picker adds several. | https://support.discord.com/hc/en-us/articles/12102061808663 |
| Add Super Reaction | Super Reaction icon or picker toggle (Nitro) (unconfirmed for a hover icon). | https://support.discord.com/hc/en-us/articles/12102061808663 |
| Reply | Arrow icon starts a reply. | https://support.discord.com/hc/en-us/articles/360057382374 |
| Forward | Forward icon opens the Forward To dialog (2024). | https://support.discord.com/hc/en-us/articles/24640649961367 |
| Create Thread | Thread icon (needs Create Public Threads, server text channels). | https://support.discord.com/hc/en-us/articles/4403205878423 |
| Edit | Pencil icon on your own messages. | https://support.discord.com/hc/en-us/articles/207618817 |
| Add Emoji Confetti | Confetti icon (experiment, desktop). | https://support.discord.com/hc/en-us/articles/29133681590679 |
| Apps on images | App Launcher button at the top-right of hovered images for image-editing commands (rolling out). | https://support.discord.com/hc/en-us/articles/21334461140375 |
| More (…) | Opens the same menu as right-click. | https://support.discord.com/hc/en-us/articles/221421867 |
| Shift-hover bar | Holding Shift exposes instant actions without confirmation: Delete (trash), Pin, Copy Message Link, Mark Unread, Copy ID with Developer Mode (unconfirmed for exact set). | (unconfirmed) |
| Shift-click to skip confirmation | Shift + delete/unpin skips the confirm dialog. Confirmed for unpin; delete (unconfirmed). | https://support.discord.com/hc/en-us/articles/221421867 |
| Alt/Option-click to mark unread | Alt+click (Windows) / Option+click (macOS) on a message marks it unread (unconfirmed). Alt+Enter on a focused message is confirmed. | https://support.discord.com/hc/en-us/articles/1500000056121 |

### Right-click (context) menu

Order follows the desktop client as best documented; items appear only when their conditions hold.

| Feature | Detail (states, dialogs, conditions such as permission/Nitro/DM-only) | Source URL |
|---|---|---|
| Quick React row | Most-used emoji at the top of the menu. | https://support.discord.com/hc/en-us/articles/12102061808663 |
| Add Reaction › | Submenu or full picker. Needs Add Reactions unless the emoji is already on the message, plus Read Message History. | https://docs.discord.com/developers/resources/message |
| View Reactions | Dialog: a tab per emoji (normal and super) listing reactors. Moderators (Manage Messages) can remove any user's reaction with the X (desktop/web). Your own can be removed too. | https://support.discord.com/hc/en-us/articles/12102061808663 |
| Edit Message | Own messages only. Inline edit mode (see Composer). | https://support.discord.com/hc/en-us/articles/207618817 |
| Reply | Starts a reply. | https://support.discord.com/hc/en-us/articles/360057382374 |
| Forward | Forward To dialog: multi-select destinations (max 5), search, optional note, Send. Not for monetized-channel messages, or from age-restricted to unrestricted channels. Can't publish a forward in announcement channels. Needs read access to the content. @silent supported (2026). | https://support.discord.com/hc/en-us/articles/24640649961367 |
| Create Thread | Opens the thread-creation pane (name, private toggle where allowed) (unconfirmed for pane). | https://support.discord.com/hc/en-us/articles/4403205878423 |
| Copy Text | Copies message content (unconfirmed label; Ctrl/Cmd+C on a focused message is confirmed). | https://support.discord.com/hc/en-us/articles/1500000056121 |
| Apps › | Message context-menu commands from server-installed and user-installed apps. | https://support.discord.com/hc/en-us/articles/21334461140375 |
| Mark Unread | Sets the read marker before this message (Alt+Enter). | https://support.discord.com/hc/en-us/articles/1500000056121 |
| Pin Message / Unpin Message | Confirmation dialog "Oh yeah. Pin it". Needs Pin Messages permission in servers (separate from Manage Messages since 2025); anyone in DMs/group DMs. 250-pin cap. | https://support.discord.com/hc/en-us/articles/221421867 |
| Bookmark Message | Nitro, experimental. Saves to Inbox > Bookmarks (200 limit) and shows a confirmation badge. | https://support.discord.com/hc/en-us/articles/26442819646999 |
| Create Reminder / Remind Me › | Preset delays or Custom date and time; reminder shows "Due in…" in Inbox > Bookmarks; edit, complete or remove. Nitro experiment. | https://support.discord.com/hc/en-us/articles/26442819646999 |
| Copy Message Link | Copies `https://discord.com/channels/...`. | https://support.discord.com/hc/en-us/articles/206346498 |
| Speak Message | Reads the message aloud via the OS TTS engine (unconfirmed; not in the help center). | (unconfirmed) |
| Publish | Announcement channels: pushes to following servers. Own messages need Send Messages, others' need Manage Messages. 10 per hour per channel. | https://support.discord.com/hc/en-us/articles/360032008192 |
| Remove Embed(s) / Suppress Embeds | Author or Manage Messages (unconfirmed for label). | https://docs.discord.com/developers/resources/message |
| Remove Reactions › / Remove All Reactions | Manage Messages: remove all of one emoji, or all reactions, after confirmation (unconfirmed for labels). | https://docs.discord.com/developers/resources/message |
| End Poll | Poll creator ends a poll early (unconfirmed for label). | https://support.discord.com/hc/en-us/articles/22163184112407 |
| Mark as Not Sensitive | On blurred media (adults only). | https://support.discord.com/hc/en-us/articles/18210995019671 |
| Media items | On images: Copy Image, Save Image, Copy Link, Open Link (unconfirmed for labels). On videos: Stats for Nerds, Copy Media Link, Open Media Link (2026). | https://discord.com/blog/discord-patch-notes-october-6-2026 |
| Text selection items | Copy, and "Search with {Google/Bing/DuckDuckGo/custom}" on selected text (setting `search_provider`) (unconfirmed). Spell-check suggestions when right-clicking in inputs. | https://docs.discord.food/resources/user-settings-proto |
| Report Message | In-app report flow: choose category and reason, then "More you can do" (Block / Ignore). Voice messages are reportable too. | https://support.discord.com/hc/en-us/articles/28084948873623 |
| Delete Message | Own messages, or anyone's with Manage Messages. Confirmation dialog previews the message; Shift skips it (unconfirmed). Some system types are not deletable (see types table). | https://docs.discord.com/developers/resources/message |
| Copy Message ID | Developer Mode only. | https://support.discord.com/hc/en-us/articles/206346498 |
| Shift+F10 / context-menu key | Opens the context menu from the keyboard, including in the composer (2026). | https://discord.com/blog/discord-patch-notes-july-7-2026 |

### Message keyboard shortcuts (focused message)

| Feature | Detail (states, dialogs, conditions such as permission/Nitro/DM-only) | Source URL |
|---|---|---|
| Enter message list | Tab focuses the list; arrow keys move between messages; Tab enters a message's content; Escape returns to the composer. A blue focus ring is shown. | https://support.discord.com/hc/en-us/articles/1500000056121 |
| Q | Quote message | https://support.discord.com/hc/en-us/articles/1500000056121 |
| + | Add reaction | https://support.discord.com/hc/en-us/articles/1500000056121 |
| R | Reply | https://support.discord.com/hc/en-us/articles/1500000056121 |
| E | Edit (own) | https://support.discord.com/hc/en-us/articles/1500000056121 |
| Backspace | Delete | https://support.discord.com/hc/en-us/articles/1500000056121 |
| P | Pin | https://support.discord.com/hc/en-us/articles/1500000056121 |
| Cmd+C (macOS) / Ctrl+C | Copy message content | https://support.discord.com/hc/en-us/articles/1500000056121 |
| Alt/Option+Enter | Mark unread | https://support.discord.com/hc/en-us/articles/1500000056121 |
| F | Forward the focused message (Discord blog). Other single-letter keys such as T for thread are (unconfirmed). | https://discord.com/blog/how-to-use-keyboard-shortcuts-on-discord-create-custom-keybinds |
| F6 / Shift+F6 | Jump between app sections (server list, channel list, messages, member list, search results). | https://support.discord.com/hc/en-us/articles/1500000056121 |
| Page Up / Page Down | Scroll chat | https://support.discord.com/hc/en-us/articles/225977308 |
| Shift+Page Up | Jump to oldest unread message | https://support.discord.com/hc/en-us/articles/225977308 |
| Esc / Shift+Esc | Mark channel / server as read | https://support.discord.com/hc/en-us/articles/225977308 |

## Composer

| Feature | Detail (states, dialogs, conditions such as permission/Nitro/DM-only) | Source URL |
|---|---|---|
| Rich chat input | Live markdown preview (spoiler background, code-block detection), and mentions, emoji and autocompleted items as rich pills. Backspace after a mention deletes the whole pill. Multi-line bold etc. not previewed. Legacy input toggle existed for testing. | https://support.discord.com/hc/en-us/articles/360037842511 |
| Placeholder | "Message #channel" / "Message @user" (unconfirmed wording). | (unconfirmed) |
| Enter to send / Shift+Enter newline | Enter sends. Shift+Enter inserts a newline; this is confirmed only indirectly, by the chat-box article's mention of Shift+Enter multi-line scrolling. | https://support.discord.com/hc/en-us/articles/4412085582359 ; https://support.discord.com/hc/en-us/articles/360037842511 |
| Character limit | 2000 characters (4000 with Nitro). A counter appears near the limit. Over-limit pasted text is offered as a `message.txt` file upload. | https://support.discord.com/hc/en-us/articles/360034632292 |
| Formatting shortcuts | Cmd+B bold, Cmd+I italic, Cmd+U underline; select text to get a floating toolbar (bold, italic, strike, code, spoiler/eye, quote) (toolbar unconfirmed; eye icon on selection confirmed). | https://support.discord.com/hc/en-us/articles/31232432266647 |
| Spoiler on selection | Highlight text and click the eye icon to mark it as a spoiler. | https://support.discord.com/hc/en-us/articles/360022320632 |
| @ mention autocomplete | Members, roles, @everyone/@here, `@time` (2026), `@game` (2026). Triggers after special characters such as "(@" (2026). Tab/Enter to select, arrow keys to move. | https://discord.com/blog/discord-patch-notes-october-6-2026 |
| # channel autocomplete | Channels (and threads) of the current server (unconfirmed for threads). | https://support.discord.com/hc/en-us/articles/360037842511 |
| : emoji autocomplete | `:name` suggests unicode and custom emoji, ranked by usage. Skin-tone-aware. | https://discord.com/blog/discord-patch-notes-october-6-2026 |
| Sticker suggestions | Suggestions while typing; Up arrow to enter, Left/Right to choose, Enter to send, Esc to hide until the field is cleared. Text & Images > Sticker Suggestions toggle. Stickers in autocomplete setting. | https://support.discord.com/hc/en-us/articles/4403135977495 |
| / command picker | Built-in and app commands with an app-icon rail, frecency, option chips, required/optional options, autocomplete choices, attachment options; NSFW/age-restricted commands gated. | https://support.discord.com/hc/en-us/articles/21334461140375 |
| Built-in commands | `/tts`, `/spoiler`, `/me`, `/shrug`, `/tableflip`, `/unflip`, `/nick`, `/gif` (GIF picker strip, mouse-wheel scrollable), `/thread` (unconfirmed for list beyond /tts, /spoiler, /gif). | https://support.discord.com/hc/en-us/articles/212517297 |
| @silent prefix | Prefix `@silent` to send without push or desktop notifications (flag SUPPRESS_NOTIFICATIONS). Works on forwards (2026). | https://discord.com/blog/discord-patch-notes-july-7-2026 |
| "+" upload menu | Upload a File, Create Thread, Create Poll, Use Apps/Activities (unconfirmed for full list). Polls are created through "+" > Poll. | https://support.discord.com/hc/en-us/articles/22163184112407 |
| File upload | "+" button, drag-and-drop onto the window, paste from clipboard, Cmd+Shift+U. Max 10 files per message. 20MB free (raised from 10MB in Aug 2026), 50MB Nitro Basic, 1GB Nitro; server boost levels raise the limit (unconfirmed for boost values). 0-byte files rejected immediately ("File cannot be empty"). | https://support.discord.com/hc/en-us/articles/25444343291031 |
| Attachment tray | Pending-attachment tiles with Modify (pencil: rename, alt text/description), Mark as Spoiler (eye), Remove (trash). Spoiler tag shown on the tile. | https://support.discord.com/hc/en-us/articles/25444343291031 |
| Explicit-media upload block | Uploads flagged as possibly NSFW are blocked when user/server settings require it (error 20009). | https://support.discord.com/hc/en-us/articles/25444343291031 |
| Emoji picker button | Categories, search, server sections (lock icon and purple tint for Nitro-only emoji), Shift-click for several, Add Emoji (Emoji Studio editor: crop, resize, rotate, choose server; 2025), favorites and frequently used (unconfirmed via docs.discord.food frecency), skin-tone selector (unconfirmed). | https://support.discord.com/hc/en-us/articles/360041139231 |
| GIF picker | Categories, search (provider moved from Tenor to Klipy/GIPHY in 2026 (unconfirmed)), Favorites folder (star toggle on hover). Shift-click inserts the GIF URL without sending; right-click copies the link (2026). | https://support.discord.com/hc/en-us/articles/360021235192 |
| Sticker picker | Standard packs and server stickers; Nitro unlocks cross-server use; favorites and recents (unconfirmed). Cmd+S opens it (Discord blog). | https://support.discord.com/hc/en-us/articles/360034632292 ; https://discord.com/blog/how-to-use-keyboard-shortcuts-on-discord-create-custom-keybinds |
| Gift button | Send a Nitro or Shop gift from the composer. | https://support.discord.com/hc/en-us/articles/360034632292 |
| App Launcher button | Right side of the composer: Recents, Collections, search, app commands, Activities. (2024) | https://support.discord.com/hc/en-us/articles/21334461140375 |
| Emoji Confetti button | Lower-right of the text box; banner "Select an emoji to apply to your message". Experiment. | https://support.discord.com/hc/en-us/articles/29133681590679 |
| Poll creator | Question, up to 10 answers with optional emoji, duration (1h, 4h, 8h, 24h, 3d, 1w), allow multiple answers, Post. Needs Create Polls permission. Available in text channels, DMs, group DMs, announcement channels and voice text chat. AutoMod applies. | https://support.discord.com/hc/en-us/articles/22163184112407 |
| Voice message recording | Mobile only (hold mic, swipe to lock, trash to cancel). Desktop can only play voice messages. | https://support.discord.com/hc/en-us/articles/13091096725527 |
| Reply mode | Bar above the input: "Replying to {name}", "@ ON/OFF" mention toggle (default on), X or Esc to cancel. | https://support.discord.com/hc/en-us/articles/360057382374 |
| Edit mode | Inline editor in place of the message. Enter saves, Esc cancels ("escape to cancel • enter to save" hint (unconfirmed wording)). Saving empty content prompts to delete (unconfirmed). Home key no longer scrolls chat (2026). | https://support.discord.com/hc/en-us/articles/207618817 |
| Up arrow edit | Edit your last message: Up arrow in an empty composer (Discord blog). The help-center guide instead lists Shift+Up (Windows) / Option+Up (macOS). | https://discord.com/blog/how-to-use-keyboard-shortcuts-on-discord-create-custom-keybinds ; https://support.discord.com/hc/en-us/articles/31232432266647 |
| Typing indicator emission | Sends a typing event while you type (about every 10 s, unconfirmed); others see "X is typing…". | https://support.discord.com/hc/en-us/articles/42962943077271 |
| Drafts | Unsent text is kept per channel when switching (unconfirmed). Thread starter drafts persist (2026 fix). | https://discord.com/blog/discord-patch-notes-july-7-2026 |
| Slowmode state | Cooldown countdown after sending; durations 5s–6h. Bypass Slowmode permission exempts users (Manage Messages/Channel/Threads stopped bypassing after Feb 23 2026). | https://support.discord.com/hc/en-us/articles/360016150952 |
| Timed-out state | Composer disabled with a timeout notice; can't react except on existing reactions, can't join voice. | https://support.discord.com/hc/en-us/articles/4413305239191 |
| No-permission state | "You do not have permission to send messages in this channel." (unconfirmed wording) | (unconfirmed) |
| Locked/archived thread state | Archived threads unarchive on send if allowed; locked threads block sending (error 160005). Typing blocked in read-only threads. | https://discord.com/blog/discord-patch-notes-october-6-2026 |
| DM blocked state | Can't message a user who blocked you or whose privacy settings forbid it ("Your message could not be delivered…" Clyde notice) (unconfirmed wording). | https://support.discord.com/hc/en-us/articles/360060145013 |
| Server preview (lurker) state | Previewing a server shows "Join Server" in place of the composer (unconfirmed). | (unconfirmed) |
| Verification-level gate | Composer locked until verification requirements are met (verified email/phone, account age, member duration) (unconfirmed for UI). | (unconfirmed) |
| Membership screening / rules | Composer replaced by "Complete" until rules are accepted (unconfirmed). | (unconfirmed) |
| Spell check | Underlines misspellings; right-click suggestions, also in plain inputs (2026). | https://discord.com/blog/discord-patch-notes-july-7-2026 |
| Home/End keys | Move to start/end of the visual line in wrapped text (2026). | https://discord.com/blog/discord-patch-notes-july-7-2026 |
| Ctrl/Cmd+Enter in forum composer | Posts a forum post (unconfirmed as a shortcut; referenced in a 2026 fix). | https://discord.com/blog/discord-patch-notes-october-6-2026 |
| Overlay composer | In-game overlay chat composer, with a notice when typing isn't available (2026). | https://discord.com/blog/discord-patch-notes-august-4-2026 |
| Scheduled messages | Not a Discord feature as of Oct 2026 (no official source; third-party guides say it isn't native). | (unconfirmed) |
| Message translation | No native translation found as of Oct 2026. | (unconfirmed) |

## Reactions

| Feature | Detail (states, dialogs, conditions such as permission/Nitro/DM-only) | Source URL |
|---|---|---|
| Reaction pills | Emoji plus count below the message. Highlighted (blurple border) when you reacted. Click toggles your reaction. Hover tooltip "X, Y and N others reacted with :emoji:" (unconfirmed wording). | https://support.discord.com/hc/en-us/articles/12102061808663 |
| Add-reaction button after pills | Smiley button next to the last pill. | https://support.discord.com/hc/en-us/articles/12102061808663 |
| Limit | Max 20 unique reactions per message (error 30010). | https://docs.discord.com/developers/topics/opcodes-and-status-codes |
| Permissions | Add Reactions is needed only for a new emoji; Read Message History is always needed. Timed-out members can only join existing reactions. | https://docs.discord.com/developers/resources/message |
| Super Reactions (burst) | Animated reactions sent by toggling the Super Reaction icon in the picker (blurple glow). Nitro, Nitro Basic and Classic get unlimited; everyone sees them. Separate burst pill with burst colors and count (`count_details.burst`, `burst_colors`, `me_burst`). | https://support.discord.com/hc/en-us/articles/12102061808663 |
| Super Reaction animation toggle | Accessibility > "Play animated emoji" (also Reduced Motion). | https://support.discord.com/hc/en-us/articles/12102061808663 |
| Multi-react | Shift-click several emoji in the picker. | https://support.discord.com/hc/en-us/articles/12102061808663 |
| Quick React / suggested | Most-used emoji in the context menu and hover bar (top 3, 2024). | https://discord.com/blog/discord-update-november-18-2024-changelog |
| Reaction frecency | Separate usage ranking for reactions (`emoji_reaction_frecency`) (unconfirmed). | https://docs.discord.food/resources/user-settings-proto |
| View Reactions dialog | Tabs per emoji including super reactions; list of users; X to remove your own; moderators can remove others' (desktop/web, Manage Messages). Keyboard focus fixed (2026). | https://support.discord.com/hc/en-us/articles/12102061808663 |
| Remove all / remove emoji | Manage Messages can clear all reactions or all of one emoji (unconfirmed for menu labels). | https://docs.discord.com/developers/resources/message |
| Reaction notifications | Experimental: Settings > Notifications > Reaction Notifications (All Messages / Only Direct Messages / Never). Follows mute settings. | https://support.discord.com/hc/en-us/articles/12102061808663 |
| Show reactions setting | Text & Images: show emoji reactions on messages (`render_reactions`) (unconfirmed for label). | https://docs.discord.food/resources/user-settings-proto |
| Forum "React to post" | Forum post cards show a React button (visible without clicking, 2026). Default reaction per forum. | https://discord.com/blog/discord-patch-notes-july-7-2026 |
| Default reaction / double-tap | Mobile double-tap reacts with your most-used (configurable) emoji. Mobile only. | https://support.discord.com/hc/en-us/articles/12102061808663 |

## Search, Inbox & pins

### Message search

| Feature | Detail (states, dialogs, conditions such as permission/Nitro/DM-only) | Source URL |
|---|---|---|
| Search bar | Upper-right of the channel/DM header; scope is the current server or the current DM / group DM. Bar scales with window; long channel topic no longer pushes it offscreen (2026). | https://support.discord.com/hc/en-us/articles/115000468588 |
| Ctrl/Cmd+F | Focus search for the current channel; pre-fills `in:` with the current text channel (2026); does nothing while a modal is open (2026). | https://discord.com/blog/discord-patch-notes-march-6-2026 |
| Ctrl/Cmd+Shift+F | "Search across all channels" shortcut. | https://support.discord.com/hc/en-us/articles/31232432266647 |
| Filter popout (autocomplete) | Typing in the bar shows filter suggestions (From user, In channel, Mentions user, Has type, dates); suggestions no longer hidden behind tabs (2025). | https://discord.com/blog/discord-patch-notes-july-7-2025 |
| `from:` | Messages from a specific user (user picker autocompletes members). | https://support.discord.com/hc/en-us/articles/115000468588 |
| `in:` | Messages in a specific channel; on mobile can target all DMs (2025). | https://discord.com/blog/discord-patch-notes-september-3-2025 |
| `mentions:` | Messages that mention a specific user; also role mentions (role part unconfirmed). | https://support.discord.com/hc/en-us/articles/115000468588 |
| `has:` link / embed / poll / file | Content-type filter values named officially. `has: embed` excludes messages whose embeds were removed (2026). Invalid value isn't treated as a filter (2026). | https://support.discord.com/hc/en-us/articles/115000468588 |
| `has:` forward | Filters forwarded messages; menu label changed from "Snapshot" to "Forward" (2026). | https://discord.com/blog/discord-patch-notes-may-4-2026 |
| `has:` image / video / sound / sticker | Other has types. (unconfirmed) | https://docs.discord.food/resources/message |
| Negation with "-" | Exclude a filter value, e.g. `has:-image` (fixed 2026); author types can also be negated (author negation unconfirmed). | https://discord.com/blog/discord-patch-notes-april-6-2026 |
| `authorType:` | user / bot / webhook (2025); desktop "More filters" and mobile "By author type". | https://discord.com/blog/discord-patch-notes-september-3-2025 |
| `before:` / `after:` / date "on" | Date filters with date picker; "After" date off-by-one fix (2026). | https://discord.com/blog/discord-patch-notes-july-7-2026 |
| `during:` | Messages during a given day/month/year. (unconfirmed) | https://docs.discord.food/resources/message |
| `pinned:` | Pinned true/false. (unconfirmed) | https://docs.discord.food/resources/message |
| Advanced filters | Reply-to user/message, link hostname, attachment filename/extension, embed type, embed provider, include NSFW. (unconfirmed — API params only; UI exposure unknown) | https://docs.discord.food/resources/message |
| Combine filters | Desktop "More filters": combine criteria, date ranges, author type. | https://support.discord.com/hc/en-us/articles/115000468588 |
| Sort menu | Sort icon in results panel: Newest / Oldest / Most Relevant (desktop/browser). | https://support.discord.com/hc/en-us/articles/115000468588 |
| Results side panel | Results open in a side panel with message previews, channel headers, and a page selector (bounded 1..page count, fix 2026). | https://discord.com/blog/discord-patch-notes-august-4-2026 |
| Results limit | 5,000 results or 200 pages before the "broken magnifying glass" state. | https://support.discord.com/hc/en-us/articles/33694251638295 |
| Result tabs | Messages, Media, Files, Links tabs (confirmed on iOS; desktop tab set unconfirmed). Pins tab exists in API (unconfirmed). | https://discord.com/blog/discord-patch-notes-august-4-2026 |
| Spoiler handling in results | Media from spoiler channels stays spoilered in search (fix 2026). | https://discord.com/blog/discord-patch-notes-september-8-2026 |
| System messages in results | System messages (e.g., Stage start, server join) render in results (2026). | https://discord.com/blog/discord-patch-notes-may-4-2026 |
| Jump to result | Click a result to jump to the message in its channel. | https://support.discord.com/hc/en-us/articles/115000468588 |
| Jump to spam-flagged results | Messages flagged as spam can be jumped to from search (2026). | https://discord.com/blog/discord-patch-notes-february-4-2026 |
| Keyboard navigation of results | Arrow keys + Enter on results (fix 2026). | https://discord.com/blog/discord-patch-notes-march-6-2026 |
| Clear (X) button | Clears query and closes results popout (fix 2026). | https://discord.com/blog/discord-patch-notes-september-8-2026 |
| Search within DM / group DM | Search bar in a DM searches that conversation. | https://support.discord.com/hc/en-us/articles/115000468588 |
| Search across all DMs | Available on mobile; desktop only as a limited experiment. | https://support.discord.com/hc/en-us/articles/115000468588 |
| Forum post search | Separate search box in forum channels with Clear link (2026). | https://discord.com/blog/discord-patch-notes-august-4-2026 |
| Gradient role colors in results | Respect "Show role colors next to names" (2026). | https://discord.com/blog/discord-patch-notes-april-6-2026 |
| Indexing | Search covers from first message; large servers may index slowly at first. | https://support.discord.com/hc/en-us/articles/115000468588 |
| Search history / recent searches | Recent queries listed in popout. (unconfirmed — no source found) | — (no source found) |
| Quick Switcher | Ctrl/Cmd+K to jump to channels, DMs, servers; shows online status (2025); more results without type filter (2025); handles per-server nicknames (2025). | https://discord.com/blog/discord-patch-notes-october-7-2025 |
| Quick Switcher prefixes | `@` users, `#` channels, `!` servers, `*` categories. (unconfirmed) | — (no source found) |
| Mention suggestions | Experimental desktop-only: typing a name without "@" suggests users incl. friend nicknames; TAB inserts; toggle in User Settings > Chat. | https://support.discord.com/hc/en-us/articles/35692242798743 |

### Inbox

| Feature | Detail (states, dialogs, conditions such as permission/Nitro/DM-only) | Source URL |
|---|---|---|
| Inbox button | Inbox icon in the desktop title bar (moved there 2025); desktop and browser only. | https://discord.com/blog/discord-update-march-25-2025-changelog |
| Ctrl/Cmd+I | Open Inbox. | https://support.discord.com/hc/en-us/articles/360045027712 |
| Tabs | For You, Unreads, Mentions, Bookmarks; tab buttons handle long server/GDM names (2026). | https://discord.com/blog/discord-patch-notes-february-4-2026 |
| For You tab | Personalized feed with server icons; AutoMod alerts with Jump button appear in Inbox (2026). Exact content types (unconfirmed). | https://discord.com/blog/discord-patch-notes-april-6-2026 |
| Unreads tab | Unread messages grouped by channel from unmuted channels plus DMs/group DMs (red badge with count); @here appears here not in Mentions. | https://support.discord.com/hc/en-us/articles/360045027712 |
| "View all unreads" link | Shown for channels with more unread messages than displayed (2026). | https://discord.com/blog/discord-patch-notes-july-7-2026 |
| "Notifications Disabled" divider | Divider in Unreads separating channels with notifications off (2026). | https://discord.com/blog/discord-patch-notes-june-4-2026 |
| Mark single item read | Check mark icon on a message/channel group. | https://support.discord.com/hc/en-us/articles/360045027712 |
| Mark Inbox As Read | Button upper-right clears all unreads. | https://support.discord.com/hc/en-us/articles/360045027712 |
| Ctrl/Cmd+Shift+E | Mark top-most Inbox item as read. | https://support.discord.com/hc/en-us/articles/360045027712 |
| Interact in Unreads | React (new or existing), play media/videos, YouTube embeds; fullscreen hidden for attachments (2026). | https://support.discord.com/hc/en-us/articles/360045027712 |
| Mute from Inbox | Bell icon: mute channel or server; must unmute from the server. | https://support.discord.com/hc/en-us/articles/360045027712 |
| Enter to navigate | Enter on a channel in Unreads navigates to it (2026). | https://discord.com/blog/discord-patch-notes-may-4-2026 |
| Mentions tab | Messages mentioning you via @username, @role, @everyone. | https://support.discord.com/hc/en-us/articles/360045027712 |
| Mentions filters | Filter button: Include @everyone mentions, Include @role mentions, Include all servers (last only when viewing a server). | https://support.discord.com/hc/en-us/articles/360045027712 |
| Remove mention | "X" next to a mention; mentions auto-clear after 7 days. | https://support.discord.com/hc/en-us/articles/360045027712 |
| Jump button | Hover > Jump to go to the message (reply there). | https://support.discord.com/hc/en-us/articles/360045027712 |
| Profile cards from Inbox | Clicking a mention inside Inbox/pins opens the profile card (2026). | https://discord.com/blog/discord-patch-notes-august-4-2026 |
| Bookmarks tab | Lists bookmarks and reminders (Nitro experiment). | https://support.discord.com/hc/en-us/articles/26442819646999 |
| Legacy mentions popout shortcut | Ctrl+Alt+@ / Cmd+Opt+@ toggles mentions popout. | https://support.discord.com/hc/en-us/articles/31232432266647 |

### Pins

| Feature | Detail (states, dialogs, conditions such as permission/Nitro/DM-only) | Source URL |
|---|---|---|
| Pins button / popout | Pins icon top-right of channel header; Ctrl/Cmd+P toggles; listed newest-pinned first. | https://support.discord.com/hc/en-us/articles/221421867 |
| Pin Message | Hover > More > Pin Message, confirm "Oh yeah. Pin it."; posts a system message to the channel; author not notified. | https://support.discord.com/hc/en-us/articles/221421867 |
| Pin Messages permission | Separate permission for pin/unpin (2025); unpin button no longer requires Manage Messages (fix 2026). | https://discord.com/blog/discord-patch-notes-september-3-2025 |
| Pin limit 250 | Per channel/DM; raised from 50 in 2025; must unpin at cap. | https://support.discord.com/hc/en-us/articles/221421867 |
| Pins in DMs / group DMs | Any participant can pin; no permission. | https://support.discord.com/hc/en-us/articles/221421867 |
| Jump | "Jump" next to a pinned message scrolls to original (list no longer auto-scrolls, fix 2026). | https://support.discord.com/hc/en-us/articles/221421867 |
| Unpin from popout | X icon with confirmation; Shift-click skips confirmation. | https://support.discord.com/hc/en-us/articles/221421867 |
| Unpin from message | Right-click message > Unpin Message. | https://support.discord.com/hc/en-us/articles/221421867 |
| Pin system message | "pinned a message" system message (itself not pinnable); old pinned system messages can be unpinned (2026). | https://discord.com/blog/discord-patch-notes-may-4-2026 |
| Thread pins | Pins open from a thread's Split View drop-down (2026). | https://discord.com/blog/discord-patch-notes-may-4-2026 |
| Screen reader pin announce | Announces when a message is pinned (2026). | https://discord.com/blog/discord-patch-notes-may-4-2026 |

### Bookmarks and Reminders

| Feature | Detail (states, dialogs, conditions such as permission/Nitro/DM-only) | Source URL |
|---|---|---|
| Availability | Experimental, limited to some Nitro members; desktop, browser, mobile; still present 2026. | https://support.discord.com/hc/en-us/articles/26442819646999 |
| Bookmark Message | Right-click > Bookmark Message > Bookmark; bookmark button on message hover bar (2026); confirmation badge. Works in channels, DMs, group chats. | https://support.discord.com/hc/en-us/articles/26442819646999 |
| Bookmark limit | 200 saved messages. | https://support.discord.com/hc/en-us/articles/26442819646999 |
| View bookmarks | Inbox > Bookmarks tab. | https://support.discord.com/hc/en-us/articles/26442819646999 |
| Unsave / convert to reminder | Options in the bookmarked message's upper-right corner. | https://support.discord.com/hc/en-us/articles/26442819646999 |
| Inaccessible source channel | Bookmarks from channels you lost access to show placeholder (2026). | https://discord.com/blog/discord-patch-notes-september-8-2026 |
| Create Reminder | Hover message in Inbox > Create Reminder icon > "Remind Me" presets or Custom date/time ("Set Custom Reminder" modal with time picker, 2026). | https://discord.com/blog/discord-patch-notes-june-4-2026 |
| Reminder list | In Bookmarks tab with "Due in" label above message. | https://support.discord.com/hc/en-us/articles/26442819646999 |
| Edit / complete / remove reminder | Right-click reminder in Inbox > Bookmarks. | https://support.discord.com/hc/en-us/articles/26442819646999 |
| Reminder due notification | Push/desktop alert behavior when due. (unconfirmed — not documented) | — (no source found) |

## Member list & profiles

### Member list

| Feature | Detail (states, dialogs, conditions such as permission/Nitro/DM-only) | Source URL |
|---|---|---|
| Member list toggle | Header icon or Ctrl/Cmd+U. | https://support.discord.com/hc/en-us/articles/31232432266647 |
| Hoisted role groups | Roles with "display separately" (hoist) get their own section. | https://docs.discord.com/developers/topics/permissions |
| Online / Offline grouping | Non-hoisted online members under Online; offline under Offline. Offline section hidden at 1,000+ members, reappears under 800. | https://support.discord.com/hc/en-us/articles/33694251638295 |
| Group header counts | Each section header shows "ROLE — N". (unconfirmed) | — (no source found) |
| Role color on names | Highest colored role tints the name; "Show role colors next to names" setting. | https://discord.com/blog/discord-patch-notes-april-6-2026 |
| Gradient / holographic roles | Enhanced Role Styles: Solid / Gradient (2 colors) / Holographic; 3-boost perk; animated, Reduced Motion stops it (2025). | https://support.discord.com/hc/en-us/articles/31444213087255 |
| Role icons | Shown next to role names in member list and highest icon next to names in chat (Cozy only); Level 2 boost. | https://support.discord.com/hc/en-us/articles/4409571023639 |
| Owner crown | Crown icon next to server owner in member list. (unconfirmed) | — (no source found) |
| APP / bot tag | "APP" tag next to bot/app names; OP tag in forum posts; order with role icon. | https://discord.com/blog/discord-patch-notes-august-4-2026 |
| Verified bot checkmark | VERIFIED_BOT user flag exists; rendering as a checkmark inside the APP tag (unconfirmed). | https://docs.discord.com/developers/resources/user |
| OFFICIAL badge | Discord Updates account shows OFFICIAL badge in member list. | https://support.discord.com/hc/en-us/articles/360036118732 |
| Server tag next to name | 4-char tag + badge next to names in chat, member list, DM list (2025). | https://support.discord.com/hc/en-us/articles/31444248479639 |
| Nameplates in member list | Background nameplate behind row in server, channel and thread member lists; only while user is active (2025). | https://support.discord.com/hc/en-us/articles/30408457944215 |
| Display name styles in member list | Full style in DM/group member lists; in servers only font shows, role color replaces color/effect (2025). | https://support.discord.com/hc/en-us/articles/33833879643927 |
| Presence dots | Online/idle/DND/streaming dots; offline greyed. | https://support.discord.com/hc/en-us/articles/211374998 |
| Activity / custom status line | Custom status overrides activity under the name; ellipsis truncation (2026). | https://support.discord.com/hc/en-us/articles/360035407531 |
| New status types | "Can't Wait For" and "Obsessed With" status messages shown in member lists (2025). | https://discord.com/blog/discord-patch-notes-september-3-2025 |
| Activity cards (recent activity) | Beta, desktop: live and recent games/music; collapse to 3 or expand to 50; reply or emoji sends DM; "Hide Activity Cards" (2024). | https://support.discord.com/hc/en-us/articles/22045487931799 |
| Member list recent activity | Server member lists on desktop show recent activity (2024). | https://discord.com/blog/discord-update-august-28-2024-changelog |
| Thread member list | Threads have their own member list (spacing fix 2026). | https://discord.com/blog/discord-patch-notes-june-4-2026 |
| Group DM member list | Toggleable; profile / DM members; owner kick. | https://support.discord.com/hc/en-us/articles/223657667 |
| Voice guest members | Voice guests in member list; Kick Member available (2025). | https://discord.com/blog/discord-patch-notes-november-4-2025 |
| Large server loading | Faster member list loading for large servers (2025). | https://discord.com/blog/discord-patch-notes-july-7-2025 |
| Kicked/banned removal | Kicked or banned members removed from list promptly (fix 2026). | https://discord.com/blog/discord-patch-notes-september-8-2026 |
| Role mention member list | Clicking a role @mention shows the members with that role (2026). | https://discord.com/blog/discord-patch-notes-february-4-2026 |
| Linked Role tag | Tag next to name in Linked-Roles-gated channels; click shows requirements. | https://support.discord.com/hc/en-us/articles/8063233404823 |
| Timeout indicator | Mods see icon after timestamp of timed-out user's messages. | https://support.discord.com/hc/en-us/articles/4413305239191 |
| Members page (server) | Desktop-only management table: join date, account age, join method, roles, signals (Unusual DM Activity, Timed Out, Quarantined), sort/filter, add roles, prune, bulk ban. | https://support.discord.com/hc/en-us/articles/15946797617431 |
| Mod View | "Open Mod View" from member row: management tools, account info, messages; experimental. | https://support.discord.com/hc/en-us/articles/15946797617431 |

### Profile popout (mini profile)

| Feature | Detail (states, dialogs, conditions such as permission/Nitro/DM-only) | Source URL |
|---|---|---|
| Open popout | Click name/avatar in chat or member list; click name shows server profile (2026). | https://discord.com/blog/discord-patch-notes-may-4-2026 |
| Banner | Image banner (Nitro, ≥680×240, GIF allowed) or banner color. | https://support.discord.com/hc/en-us/articles/4403147417623 |
| Avatar + decoration | Avatar with avatar decoration (per-server allowed). | https://support.discord.com/hc/en-us/articles/13410113109911 |
| Profile effect | Animated effect plays when profile viewed; Reduced Motion disables. | https://support.discord.com/hc/en-us/articles/17828465914263 |
| Profile frame | Decorative border around popout/full profile; Shop item, Nitro early access (2026). | https://support.discord.com/hc/en-us/articles/40775065582615 |
| Profile theme colors | Primary + accent gradient (Nitro); "Sync profile themes" accessibility setting. | https://support.discord.com/hc/en-us/articles/4409388345495 |
| Display name, username, pronouns | Display name with style; username; pronouns. | https://support.discord.com/hc/en-us/articles/4403147417623 |
| Server tag on profile | Clicking tag opens server's Server Profile with Join Server or Apply (2025). | https://support.discord.com/hc/en-us/articles/31444248479639 |
| Badges row | Hover tooltips (e.g., Nitro since date, boosting duration); click opens badge directory (2026). | https://support.discord.com/hc/en-us/articles/360035962891 |
| Custom status bubble | Thought bubble above avatar (2024); Send Reaction / Reply to Status (experimental) DMs the user. | https://support.discord.com/hc/en-us/articles/360035407531 |
| About Me / bio | 190→300 chars (2026), markdown, links, emoji (custom emoji Nitro); WYSIWYG editor desktop (2026). | https://discord.com/blog/discord-update-september-25-2026 |
| Member since / account age | Discord join date and server join date. (unconfirmed layout) Account age stays visible when profile private. | https://support.discord.com/hc/en-us/articles/38859942749463 |
| Roles list | Server roles with icons; add/remove role (Manage Roles); hidden for non-members (2026). | https://discord.com/blog/discord-patch-notes-april-6-2026 |
| Note field | Private note on user. | https://support.discord.com/hc/en-us/articles/33694251638295 |
| Activity cards stack | Game, music, voice shown as card stack (2025); Join Voice; Ask to Join for activities; Game Profile from game. | https://discord.com/blog/discord-update-september-25-2025-changelog |
| Game Profile | Click game name/icon: genre, screenshots, platforms, reviews, popularity, official server (desktop). | https://discord.com/blog/discord-update-june-25-2026-changelog |
| Connected accounts | Shortened list of connections; "Display on profile" / "display details" toggles; verified checkmarks. | https://support.discord.com/hc/en-us/articles/8063233404823 |
| Message box / quick actions | Send message or friend request from profile (2024). | https://discord.com/blog/discord-update-july-26-2024-changelog |
| Mutual servers / friends | Mutual friends and servers shown, also on private profiles. | https://support.discord.com/hc/en-us/articles/38859942749463 |
| View Full Profile | Opens full profile modal. | https://support.discord.com/hc/en-us/articles/4403147417623 |
| View main profile | Overflow menu shows main profile for per-server private profiles (2026). | https://discord.com/blog/discord-patch-notes-august-4-2026 |
| Copy Avatar Image Link | Copies per-server avatar link on server profile (2026). | https://discord.com/blog/discord-patch-notes-may-4-2026 |
| Voice user popout | Hover popout for users in voice channels. | https://discord.com/blog/discord-patch-notes-august-4-2026 |
| Private profile | Hides custom status, pronouns, badges, connections, bio, widgets, wishlist; options Friends & All Servers / Friends & Small Servers (≤200) / Friends Only (2026). | https://support.discord.com/hc/en-us/articles/38859942749463 |

### Full profile modal

| Feature | Detail (states, dialogs, conditions such as permission/Nitro/DM-only) | Source URL |
|---|---|---|
| Activity tab | Real-time activity; recent games list (2024); clear game history. | https://support.discord.com/hc/en-us/articles/35344672307607 |
| Mutual Servers tab | Shared servers with nicknames. | https://discord.com/blog/discord-patch-notes-may-4-2026 |
| Mutual Friends tab | Shared friends. | https://support.discord.com/hc/en-us/articles/217916488 |
| Wishlist tab | Shop items; public/private toggle; Add item; gift from it (2025). | https://support.discord.com/hc/en-us/articles/36288192746903 |
| Profile Board widgets | Favorite Game, Currently Playing (5), Want to Play (20), Games I've Played (20), Game Stats; drag to reorder; desktop/browser only (2025). | https://support.discord.com/hc/en-us/articles/35344672307607 |
| Desktop profile redesign | Refreshed design with activity cards (2025). | https://discord.com/blog/discord-update-september-25-2025-changelog |
| Badge Directory | Modal listing badges; arrow-key navigation (2026). | https://discord.com/blog/discord-patch-notes-august-4-2026 |

### Profile customization

| Feature | Detail (states, dialogs, conditions such as permission/Nitro/DM-only) | Source URL |
|---|---|---|
| Profiles settings | User Settings > Profiles: User Profile / Per-server Profiles tabs with live preview. | https://support.discord.com/hc/en-us/articles/4403147417623 |
| Avatar | PNG/GIF (GIF Nitro), max 8 MB, crop; 6 recent avatars (2025). | https://discord.com/blog/discord-update-june-30-2025-changelog |
| Display name | 1–32 chars; username 2–32 lowercase, changeable twice per week. | https://support.discord.com/hc/en-us/articles/12620128861463 |
| Display Name Styles | Nitro: 12 fonts, 7 effects (Solid, Gradient, Neon, Toon, Pop, Gummy, Prism), colors, Surprise Me; accessibility toggle hides (2025). | https://support.discord.com/hc/en-us/articles/33833879643927 |
| Avatar decorations | Shop/Quest items; Change Decoration; DISXCORE Nitro-only. | https://support.discord.com/hc/en-us/articles/13410113109911 |
| Nameplates | Shop item; per-server (2025). | https://discord.com/blog/discord-update-november-6-2025-changelog |
| Server tag selection | Profiles > Server Tag > Select; X removes; one at a time (2025). | https://support.discord.com/hc/en-us/articles/31444248479639 |
| Legacy Username badge toggle | Turn "Originally known as" badge on/off. | https://support.discord.com/hc/en-us/articles/12620128861463 |
| Per-server profiles | Nitro: avatar, banner, bio, theme, decoration, effect, nameplate, name style, pronouns. | https://support.discord.com/hc/en-us/articles/4409388345495 |
| Server nickname | Free; Change Nickname permission; "Edit per-server profile" or /nick; Manage Nicknames for others. | https://support.discord.com/hc/en-us/articles/219070107 |
| Banner | Nitro: PNG/JPG/GIF <10 MB, ≥680×240, crop; non-Nitro banner color (accent color). | https://support.discord.com/hc/en-us/articles/4403147417623 |
| Profile theme | Nitro: Primary + Accent colors with preview; also per-server. | https://support.discord.com/hc/en-us/articles/4403147417623 |
| About Me editor | Bio up to 300 chars (was 190, 2026); markdown; WYSIWYG formatting toolbar on desktop (2026); custom emoji with Nitro. | https://discord.com/blog/discord-update-august-11-2026-changelog |
| Pronouns | Free-text pronouns field; per-server pronouns field placeholder shows main pronouns (2026). | https://discord.com/blog/discord-patch-notes-may-4-2026 |
| Profile effects | Profiles > Change Effect; Shop items; DISXCORE Nitro-only; per-server. | https://support.discord.com/hc/en-us/articles/17828465914263 |
| Profile frames | Profiles > Profile Effect & Frame > Change Frames; Shop; Nitro early access (2026). | https://support.discord.com/hc/en-us/articles/40775065582615 |
| Profile widgets editing | Profiles > Profile Widgets > Add Widgets; game picker; Remove Widget confirm; Game Stats account linking (2025). | https://support.discord.com/hc/en-us/articles/35344672307607 |
| Recent games / clear history | Your profile lists recently played games; clear a game's play history (2024). | https://discord.com/blog/discord-update-november-18-2024-changelog |
| Profile privacy setting | Data & Privacy > Profile Privacy: Friends & All Servers / Friends & Small Servers / Friends Only; teen default small servers (2026). | https://support.discord.com/hc/en-us/articles/38859942749463 |
| Activity privacy | "Share Activity with Others" global, per-server Activity Status toggle, per-game detection toggle. | https://support.discord.com/hc/en-us/articles/22045487931799 |
| Connections display toggles | User Settings > Connections: "Display on profile", "display details on profile". | https://support.discord.com/hc/en-us/articles/8063233404823 |
| Reduced Motion | Accessibility > Reduced Motion stops profile effects, nameplate and role-style animation. | https://support.discord.com/hc/en-us/articles/30408457944215 |
| Sync profile themes | Accessibility: profile card background follows client theme. | https://support.discord.com/hc/en-us/articles/4409388345495 |
| Display Name Styles toggle | Accessibility > Display Name Styles off hides styles for you. | https://support.discord.com/hc/en-us/articles/33833879643927 |
| Server Profile (server card) | Server info card (name, icon, banner, traits, games, description) opened from server tags; Private Profile toggle (2025). | https://support.discord.com/hc/en-us/articles/30715364399511 |

### User context menu

| Feature | Detail (states, dialogs, conditions such as permission/Nitro/DM-only) | Source URL |
|---|---|---|
| Profile | Opens full profile. | https://support.discord.com/hc/en-us/articles/360036118732 |
| Mention | Inserts @mention of the user in the composer. (unconfirmed) | — (no source found) |
| Message | Opens/creates DM with the user. (unconfirmed) | — (no source found) |
| Call | Starts DM voice call. (unconfirmed) | — (no source found) |
| Add Note | Edit private note. (unconfirmed placement) | https://support.discord.com/hc/en-us/articles/33694251638295 |
| Add Friend Nickname | Friends only. | https://support.discord.com/hc/en-us/articles/12620128861463 |
| Invite to Server | Server picker submenu/modal. | https://discord.com/blog/discord-patch-notes-february-4-2026 |
| Add / Remove Friend | Add Friend sends request without confirmation. | https://support.discord.com/hc/en-us/articles/218344397 |
| Ignore / Block | Confirm dialogs. | https://support.discord.com/hc/en-us/articles/28084948873623 |
| Mute | Locally mutes the user's voice. (unconfirmed) | — (no source found) |
| Edit per-server profile / Change Nickname | Self or others with permission. | https://support.discord.com/hc/en-us/articles/219070107 |
| Roles submenu | Toggle roles (Manage Roles). | https://support.discord.com/hc/en-us/articles/15946797617431 |
| Apps submenu | User-context app commands. | https://support.discord.com/hc/en-us/articles/21334461140375 |
| Timeout / Remove Timeout | Durations 60s, 5m, 10m, 1h, 1d, 1w + reason; Time Out Members permission. | https://support.discord.com/hc/en-us/articles/4413305239191 |
| Kick / Ban | Kick Members / Ban Members; reason dialog. (labels unconfirmed) | https://support.discord.com/hc/en-us/articles/15946797617431 |
| Open Mod View | Admin experiment. | https://support.discord.com/hc/en-us/articles/15946797617431 |
| Copy User ID | Developer Mode. | https://support.discord.com/hc/en-us/articles/206346498 |
| Report | Report user/message flow. (unconfirmed for user menu) | https://support.discord.com/hc/en-us/articles/28084948873623 |

### Badges

| Feature | Detail (states, dialogs, conditions such as permission/Nitro/DM-only) | Source URL |
|---|---|---|
| Nitro (evolving tenure) | Hover shows subscription date; evolves by milestone (2025). | https://support.discord.com/hc/en-us/articles/29136565881623 |
| Server Booster | Evolves with streak; hover shows duration. | https://support.discord.com/hc/en-us/articles/360035962891 |
| Discord Quests | Quest completion after 2024-04-24; non-removable. | https://support.discord.com/hc/en-us/articles/22225719947543 |
| Orbs | Bought from Orbs shop; non-removable (2025). | https://support.discord.com/hc/en-us/articles/360035962891 |
| Legacy Username ("Originally known as") | Hover shows old name#discrim; toggleable. | https://support.discord.com/hc/en-us/articles/12620128861463 |
| Bug Hunter / Golden Bug Hunter | Discord Testers. | https://support.discord.com/hc/en-us/articles/360035962891 |
| Discord Staff | Employees. | https://support.discord.com/hc/en-us/articles/360035962891 |
| HypeSquad Events | Legacy. | https://support.discord.com/hc/en-us/articles/360035962891 |
| HypeSquad Houses | Bravery / Brilliance / Balance; legacy. | https://docs.discord.com/developers/resources/user |
| Moderator Programs Alumni | Legacy. | https://support.discord.com/hc/en-us/articles/360035962891 |
| Early Supporter | Nitro before 2018-10-10. | https://support.discord.com/hc/en-us/articles/360035962891 |
| Partnered Server Owner | Legacy. | https://support.discord.com/hc/en-us/articles/360035962891 |
| Early Verified Bot Developer | Legacy flag. | https://docs.discord.com/developers/resources/user |
| Active Developer | (unconfirmed — not in Badges 101) | https://docs.discord.com/developers/resources/user |
| Last Meadow Online | Earned 2026-04-01–07 (2026). | https://support.discord.com/hc/en-us/articles/360035962891 |
| Gifting badge | Experiment; levels with gifts. | https://support.discord.com/hc/en-us/articles/360035962891 |
| Account Age / Streaming / Game Time / Game Variety | Experimental. | https://support.discord.com/hc/en-us/articles/360035962891 |

## Presence & status

### Online status

| Feature | Detail (states, dialogs, conditions such as permission/Nitro/DM-only) | Source URL |
|---|---|---|
| Status picker | Click own avatar (lower-left) to open status menu ("Set Your Status"); Quick Switcher shows users' status (Oct 2025). | https://support.discord.com/hc/en-us/articles/227779547 ; https://discord.com/blog/discord-patch-notes-october-7-2025 |
| Online | Green dot. | https://support.discord.com/hc/en-us/articles/211374998 |
| Idle (manual) | Yellow moon; manual Idle persists until changed. | https://support.discord.com/hc/en-us/articles/227779547 |
| Do Not Disturb | Red dot; suppresses all desktop notifications. | https://support.discord.com/hc/en-us/articles/227779547 |
| Invisible | Appear offline while keeping full access; shown gray/offline to others; never received as a status by others. | https://support.discord.com/hc/en-us/articles/227779547 ; https://docs.discord.com/developers/events/gateway-events |
| Offline display | No dot; name/avatar grayed. | https://support.discord.com/hc/en-us/articles/211374998 |
| Streaming indicator | Purple status when streaming (Twitch/YouTube URL activity). | https://support.discord.com/hc/en-us/articles/211374998 ; https://docs.discord.com/developers/events/gateway-events |
| Status durations | Idle/DND/Invisible can be set to expire; settings proto stores status_expires_at_ms. Exact UI duration options not found in any source (commonly recalled as 15 min / 1 h / 8 h / 24 h / 3 days / forever; unverified) (unconfirmed). | https://docs.discord.food/resources/user-settings-proto |
| Status sync across clients | Status is a synced user setting across devices (unconfirmed). | https://docs.discord.food/resources/user-settings-proto |
| Auto-idle | Status switches to Idle automatically after inactivity (exact timeout not officially stated; commonly ~5–10 min, unconfirmed). AFK detection works on Linux Wayland (Apr 2026). | https://support.discord.com/hc/en-us/articles/227779547 ; https://discord.com/blog/discord-patch-notes-april-6-2026 |
| AFK channel interaction | Being auto-moved to AFK channel no longer flips user to Online (Mar 2026). | https://discord.com/blog/discord-patch-notes-march-6-2026 |
| "Clear Status When I Look Away" | Setting referenced in Nov 2025 notes (status reset bug). (2025) | https://discord.com/blog/discord-patch-notes-november-4-2025 |
| Platform indicators | Presence carries per-platform client_status: desktop, mobile, web, vr (embedded for Xbox/PlayStation in unofficial docs); UI shows a mobile-phone icon for mobile-only presence (icon unconfirmed). | https://docs.discord.com/developers/events/gateway-events ; https://docs.discord.food/resources/presence |

### Custom status

| Feature | Detail (states, dialogs, conditions such as permission/Nitro/DM-only) | Source URL |
|---|---|---|
| Set custom status | Avatar menu > "Add Status" bubble; text + emoji; "Save". Edit (pencil) / clear (trash). | https://support.discord.com/hc/en-us/articles/360035407531 |
| Custom status text length | Max 128 characters (unconfirmed); screen readers announce when limit reached. | https://docs.discord.food/resources/presence ; https://discord.com/blog/discord-patch-notes-september-8-2026 |
| Custom status emoji | Unicode emoji free; custom server emoji require Nitro or Nitro Basic. | https://support.discord.com/hc/en-us/articles/360035407531 |
| "Clear After" | Default one day; options include 24 hours (official); full list Today / 4 hours / 1 hour / 30 minutes / Don't clear (unconfirmed). Mobile "Status Duration". | https://support.discord.com/hc/en-us/articles/360035407531 ; https://discord.com/blog/discord-patch-notes-july-7-2026 ; https://www.computerhope.com/issues/ch002264.htm |
| Status pick with custom status | "Status" dropdown in the same dialog sets Online/Idle/DND/Invisible together. | https://support.discord.com/hc/en-us/articles/360035407531 |
| Custom status prompts/labels | Prompt-style statuses such as "Can't Wait For" and "Obsessed With" (2025); proto labels question/think/love/excited/recommend (labels list unconfirmed). | https://discord.com/blog/discord-patch-notes-september-3-2025 ; https://docs.discord.food/resources/presence |
| Status reactions & replies | React (emoji) or reply to another user's custom status from their profile; response appears in DM (experiment). | https://support.discord.com/hc/en-us/articles/360035407531 ; https://discord.com/blog/discord-patch-notes-june-4-2026 |
| Custom status visibility | Replaces activity line in member list and DM/friends list; hidden from non-friends when Profile Privacy is restricted. | https://support.discord.com/hc/en-us/articles/360035407531 ; https://support.discord.com/hc/en-us/articles/38859942749463 ; https://docs.discord.com/developers/events/gateway-events |
| Hang status | Voice-only status (chilling, gaming, focusing, brb, watching, custom) shown while in a voice channel (unconfirmed). | https://docs.discord.food/resources/presence |
| Voice channel status | Set a channel status text while in a voice channel (permission-gated). | https://support.discord.com/hc/en-us/articles/19583625604887 |

### Activity status / Rich Presence

| Feature | Detail (states, dialogs, conditions such as permission/Nitro/DM-only) | Source URL |
|---|---|---|
| Activity types | Playing {name}, Streaming {details}, Listening to {name}, Watching {name}, Custom {emoji} {state}, Competing in {name}. | https://docs.discord.com/developers/events/gateway-events |
| Status display type | Member-list status text can show the activity Name, State, or Details. | https://docs.discord.com/developers/events/gateway-events |
| Rich Presence details/state | details, state with optional details_url/state_url links. | https://docs.discord.com/developers/events/gateway-events |
| Rich Presence timestamps | start/end; Listening/Watching with both show a progress bar; "elapsed" display. | https://docs.discord.com/developers/events/gateway-events ; https://docs.discord.com/developers/rich-presence/overview |
| Rich Presence assets | large/small image with hover text and optional large_url/small_url; external image URLs (GIF/animated WebP/AVIF) via media proxy. | https://docs.discord.com/developers/events/gateway-events |
| Rich Presence party | Party id and size [current, max]. | https://docs.discord.com/developers/events/gateway-events |
| Rich Presence buttons | Up to 2 link buttons (label 1–32 chars). | https://docs.discord.com/developers/events/gateway-events |
| Join / Spectate / Ask to Join | Secrets and flags (JOIN, SPECTATE, JOIN_REQUEST, SYNC, PLAY, party privacy); Join button on profile; "Join" vs "Ask to Join" for embedded activities. | https://docs.discord.com/developers/events/gateway-events ; https://docs.discord.com/developers/rich-presence/overview ; https://discord.com/blog/discord-patch-notes-july-7-2026 |
| Game detection | Detectable PC games auto-detected; console games (Xbox/PlayStation) shown when connected. | https://support.discord.com/hc/en-us/articles/7931156448919 ; https://discord.com/blog/discord-patch-notes-august-4-2026 |
| Spotify listening status | "Display Spotify as your status"; "Play on Spotify", "Listen Along" (Premium required), invite to listen via +; pauses after 30 s of mic transmission. | https://support.discord.com/hc/en-us/articles/360000167212 |
| Crunchyroll watching status | Shared when Crunchyroll is connected. | https://support.discord.com/hc/en-us/articles/7931156448919 |
| Activity cards stack | Multiple activities (game, music, voice) stack as cards on profile (2025). | https://discord.com/blog/discord-update-september-25-2025-changelog |
| Game Profile pages | Click game name in someone's activity: screenshots, platforms, reviews, popularity, official server (2026). | https://discord.com/blog/discord-update-june-25-2026-changelog |
| Activity history | Profile Activity tab shows last 30 days; "Clear Game History". | https://support.discord.com/hc/en-us/articles/7931156448919 |
| Game activity update rate | Clients may update game status 5 times per 20 s. | https://docs.discord.com/developers/events/gateway-events |
| Activity privacy | See Games & Apps section (Share my activity, per-server, join permissions). | https://support.discord.com/hc/en-us/articles/7931156448919 |

## Notifications

### Desktop notification settings (User Settings > Notifications)

| Feature | Detail (states, dialogs, conditions such as permission/Nitro/DM-only) | Source URL |
|---|---|---|
| Notifications page | Redesigned Mar 2026 (layout, organization, phrasing). (2026) | https://discord.com/blog/discord-update-march-24-2026-changelog |
| Enable Desktop Notifications | OS notification toggle; DM prompt "Turn on notifications" (unconfirmed label). | https://www.itgeared.com/how-to-turn-on-discord-notification-sound/ ; https://discord.com/blog/discord-patch-notes-december-8-2025 |
| Enable Unread Message Badge | Dock/taskbar unread badge (unconfirmed label). | https://www.itgeared.com/how-to-turn-on-discord-notification-sound/ |
| Enable Taskbar Flashing | Windows taskbar flash on ping (unconfirmed). | https://www.itgeared.com/how-to-turn-on-discord-notification-sound/ |
| Push Notification Inactive Timeout | Mobile push only after desktop idle for N minutes (unconfirmed). | https://support.discord.com/hc/en-us/community/posts/360051256154-Push-Notification-Inactive-Timeout-no-longer-working |
| In-app notifications | In-app toast notifications toggle. | https://support.discord.com/hc/en-us/articles/218892547 |
| Reaction Notifications | All Messages / Only Direct Messages / Never (experiment); show reactor avatar (2026). | https://support.discord.com/hc/en-us/articles/12102061808663 ; https://discord.com/blog/discord-patch-notes-march-6-2026 |
| Game Activity Notifications | Experiment (desktop). | https://support.discord.com/hc/en-us/articles/7931156448919 |
| Community Activity Alerts | Raid/DM-spam alerts for moderators. | https://support.discord.com/hc/en-us/articles/17439993574167 |
| Voice Activity Notifications | Toggle (restored May 2026). | https://discord.com/blog/discord-patch-notes-may-4-2026 ; https://support.discord.com/hc/en-us/articles/218892547 |
| Friend online notifications | "Friends Online Status Notifications"; frequency adjustable or off, as sender and receiver (Sep 2026). (2026) | https://support.discord.com/hc/en-us/articles/218892547 ; https://discord.com/blog/discord-update-september-25-2026 |
| Friend Gaming Activity / Profile Updates | Toggles. | https://support.discord.com/hc/en-us/articles/218892547 |
| Highlight Notifications | Toggle. | https://support.discord.com/hc/en-us/articles/218892547 |
| Go Live notifications to friends | Streamer checkbox "Notify my friends in this server" (servers under 50 members, verified); receiver toggle "Get notifications when your friends Go Live"; 15 min / 6 h cooldowns. | https://support.discord.com/hc/en-us/articles/4416120444311 |
| Text-to-Speech notifications | For all channels / For current selected channel / Never. | https://support.discord.com/hc/en-us/articles/212517297 |
| Email Notifications | Communication, Social, Announcements & Updates, Tips, Recommendations; "Unsubscribe from all marketing emails". | https://support.discord.com/hc/en-us/articles/4408820335767 |
| Other notification types | Friend anniversary, game update, server trending, DM reply nudges, upcoming events, screen downtime (unconfirmed). | https://docs.discord.food/resources/user-settings-proto |

### Sounds

| Feature | Detail (states, dialogs, conditions such as permission/Nitro/DM-only) | Source URL |
|---|---|---|
| Disable All Notification Sounds | Master switch (unconfirmed). | https://www.itgeared.com/how-to-turn-on-discord-notification-sound/ |
| Message / Deafen / Undeafen / Mute / Unmute | Sound toggles (unconfirmed). | https://www.itgeared.com/how-to-turn-on-discord-notification-sound/ |
| User Join / User Leave | Toggles (unconfirmed). | https://www.itgeared.com/how-to-turn-on-discord-notification-sound/ |
| Stream Started / Stopped; Viewer Join / Leave | Toggles (unconfirmed). | https://www.itgeared.com/how-to-turn-on-discord-notification-sound/ ; https://itslinuxfoss.com/how-to-turn-off-sound-when-i-join-discord/ |
| Invited to Speak | Stage sound (unconfirmed). | https://www.itgeared.com/how-to-turn-on-discord-notification-sound/ |
| PTT Activate / PTT Deactivate | Push-to-talk beeps. | https://support.discord.com/hc/en-us/articles/209813978 |
| Incoming Ring | Disabling also disables event soundpack. | https://support.discord.com/hc/en-us/articles/28692828968215 |
| Camera on/off sounds | (2025) | https://discord.com/blog/discord-patch-notes-september-3-2025 ; https://discord.com/blog/discord-update-september-25-2025-changelog |
| Event Sound Pack | Dropdown: e.g., Winter (Dec 19 2025–Jan 5 2026), Halloween toggle (Oct 7–Nov 2 2026), "Discord Default"; desktop/browser only. | https://support.discord.com/hc/en-us/articles/28692828968215 ; https://support.discord.com/hc/en-us/articles/27304636159895 |
| Custom per-server notification sounds | Proto custom_notification_sound_config (unconfirmed). | https://docs.discord.food/resources/user-settings-proto |
| Entrance Sounds | Nitro; Voice & Video > Soundboard. | https://support.discord.com/hc/en-us/articles/12612888127767 |

### Server, category, channel and DM notification settings

| Feature | Detail (states, dialogs, conditions such as permission/Nitro/DM-only) | Source URL |
|---|---|---|
| Server Notification Settings | Right-click server > Notification Settings. | https://support.discord.com/hc/en-us/articles/215253258 |
| Mute server | "Mute [Server]" with duration. | https://support.discord.com/hc/en-us/articles/215253258 ; https://support.discord.com/hc/en-us/articles/209791877 |
| Mute durations | For 15 minutes / 1 hour / 3 hours / 8 hours / 24 hours / Until I turn it back on. | https://support.discord.com/hc/en-us/articles/218892547 |
| Server notification level | All Messages / Only @mentions / Nothing. | https://support.discord.com/hc/en-us/articles/218892547 ; https://support.discord.com/hc/en-us/articles/5304469213079 |
| Suppress @everyone and @here | Toggle. | https://support.discord.com/hc/en-us/articles/215253258 ; https://support.discord.com/hc/en-us/articles/215253148 |
| Suppress All Role @mentions | Toggle. | https://support.discord.com/hc/en-us/articles/218892547 ; https://support.discord.com/hc/en-us/articles/360039176412 |
| Suppress Highlights | Toggle. | https://support.discord.com/hc/en-us/articles/215253258 ; https://support.discord.com/hc/en-us/articles/5304469213079 |
| Mute New Events | Toggle. | https://support.discord.com/hc/en-us/articles/215253258 |
| Mobile Push Notifications | Per-server toggle. | https://support.discord.com/hc/en-us/articles/215253258 |
| Notification Overrides | Per channel/category: All / Mentions / Nothing / Mute; "Use Category/Server Default" (2026). | https://support.discord.com/hc/en-us/articles/215253258 ; https://discord.com/blog/discord-patch-notes-july-7-2026 |
| Mute channel via bell | Bell icon in header. | https://support.discord.com/hc/en-us/articles/209791877 |
| Mentions break through mute | Direct, role, @everyone/@here mentions still notify and show red badges in muted channels/servers. | https://support.discord.com/hc/en-us/articles/209791877 |
| Hide Muted Channels | Server dropdown option. | https://support.discord.com/hc/en-us/articles/213599277 |
| Thread notifications | All messages / Mentions only / Nothing; join/leave thread. | https://support.discord.com/hc/en-us/articles/4403205878423 |
| Forum post following | Follow post for notifications. | https://support.discord.com/hc/en-us/articles/6208479917079 |
| Highlights | Popular messages & friend conversations digest; personalization toggle. | https://support.discord.com/hc/en-us/articles/5304469213079 |
| Raid alert push | Per-guild raid alert push/nag toggles (unconfirmed). | https://docs.discord.food/resources/user-settings-proto |

### Mentions, replies, silent messages, inbox and badges

| Feature | Detail (states, dialogs, conditions such as permission/Nitro/DM-only) | Source URL |
|---|---|---|
| Reply pings | Replies ping author by default; @ ON/OFF toggle in reply bar. | https://support.discord.com/hc/en-us/articles/360057382374 |
| @silent messages | "@silent" prefix sends without notifications; works with forwarding (Jul 2026); screen-reader announcement. | https://discord.com/blog/discord-patch-notes-september-8-2026 ; https://discord.com/blog/discord-patch-notes-july-7-2026 |
| Escaped @everyone/@here | Escaped mentions don't notify (Mar 2026). | https://discord.com/blog/discord-patch-notes-march-6-2026 |
| Inbox: Mentions tab | Filters: Include @everyone, Include @role, Include all servers; remove with X; auto-clears after 7 days. | https://support.discord.com/hc/en-us/articles/360045027712 |
| Inbox: Unreads tab | Unread messages from unmuted channels; "Mark Inbox As Read"; Ctrl/Cmd+I; Ctrl/Cmd+Shift+E. In title bar since 2025. | https://support.discord.com/hc/en-us/articles/360045027712 ; https://discord.com/blog/discord-update-march-25-2025-changelog |
| Inbox: other tabs | For You, Bookmarks, Scheduled, Game Invites (unconfirmed). | https://docs.discord.food/resources/user-settings-proto |
| Bookmarks & reminders | Inbox > Bookmarks tab; "Remind Me" presets or Custom date/time, "Due in" label; badge on bookmark/reminder; 200-message limit; experiment for some Nitro members. | https://support.discord.com/hc/en-us/articles/26442819646999 |
| Unread / mention badges | Red mention count on server icons, channels and DMs (also in muted channels for mentions); screen readers announce badge updates (Jun 2026). Dock/taskbar badge see "Enable Unread Message Badge". | https://support.discord.com/hc/en-us/articles/360045027712 ; https://discord.com/blog/discord-patch-notes-june-4-2026 |
| Friend request notifications | Pending tab badge; email. | https://support.discord.com/hc/en-us/articles/217674288 ; https://support.discord.com/hc/en-us/articles/4408820335767 |
| Message request notifications | Badge when pending. | https://support.discord.com/hc/en-us/articles/7924992471191 |
| DND behavior | Silences desktop notifications. | https://support.discord.com/hc/en-us/articles/227779547 |
| Focus / quiet mode | Proto quiet_mode, focus_mode_expires_at_ms (unconfirmed). | https://docs.discord.food/resources/user-settings-proto |
| Quest notifications | Hide via "Hide This"; personalized via Data & Privacy. | https://support.discord.com/hc/en-us/articles/22225719947543 |
| Mobile settings | Separate mobile notification settings. | https://support.discord.com/hc/en-us/articles/218892547 |

## Voice & video

### Joining voice and the voice connection

| Feature | Detail (states, dialogs, conditions such as permission/Nitro/DM-only) | Source URL |
|---|---|---|
| Join a voice channel | Click a voice channel (speaker icon) to join. A lock icon means you lack permission (Connect). Once joined, the call shows in a small corner window and you can talk before sharing video. | https://support.discord.com/hc/en-us/articles/19583625604887 |
| Voice channel permissions | Connect (join), Speak (use mic), Video (camera and Go Live). All three are channel/role permissions. | https://support.discord.com/hc/en-us/articles/360045138471 |
| Video cap and "channel full" | Up to 25 people in a server voice channel while any camera is on. Users past 25 get a "channel full" error. The cap lifts when all cameras stop. | https://support.discord.com/hc/en-us/articles/360041721052 |
| Voice connection states | Connecting, RTC Connecting, ICE Checking, No Route (local network, firewall or VPN problems), Waiting for Voice Server (Discord-side; renamed from "Awaiting endpoint" in Sep 2026). Shown in the voice panel. | https://support.discord.com/hc/en-us/articles/115001310031 ; https://discord.com/blog/discord-patch-notes-september-8-2026 |
| Voice "Connecting" deadlock reduction | Desktop "Connecting" hangs cut by about 30% (2026). | https://discord.com/blog/discord-patch-notes-april-6-2026 |
| Outdated-client voice rejection | Clients without DAVE get voice gateway close code 4017 (help article published 2026-03-02; no separate enforcement date stated) and cannot join voice, video or Go Live. Minimum versions: macOS desktop 0.0.320, Windows 1.0.9164, Linux 0.0.69, Safari 15.4, Chrome/Edge 86, Firefox 142. (2026) | https://support.discord.com/hc/en-us/articles/38749827197591 ; https://support.discord.com/hc/en-us/articles/38025123604631 |
| Voice/Video Details panel (voice status panel, bottom left) | Opens connection details. It has a green lock labeled "End-to-end encrypted" and a Privacy tab with the Voice Privacy Code. | https://support.discord.com/hc/en-us/articles/25968222946071 |
| Connection info / RTC stats | Ping graph with average and last ping plus outbound packet loss rate in the voice connection popout. A Debug menu adds Transport and Outbound charts. (unconfirmed) | https://support.discord.com/hc/en-us/community/posts/7036365014167 |
| Voice status panel buttons | Bottom-left panel while connected: Video (camera), screen share / Go Live, App Launcher (Activities), Soundboard, and a Noise Suppression (Krisp) icon. Each has an arrow for advanced options. | https://support.discord.com/hc/en-us/articles/360040816151 ; https://support.discord.com/hc/en-us/articles/360041721052 ; https://support.discord.com/hc/en-us/articles/12612888127767 ; https://support.discord.com/hc/en-us/articles/360040843952 |
| Disconnect from voice | Leave the call from the voice panel. A configurable "Disconnect from Voice Channel" keybind also exists. | https://discord.com/blog/how-to-use-keyboard-shortcuts-on-discord-create-custom-keybinds |
| Switch to Voice Channel keybind | Configurable keybind that joins a chosen voice channel. | https://discord.com/blog/how-to-use-keyboard-shortcuts-on-discord-create-custom-keybinds |
| Voice region | Automatic by default: set by the first person to join an empty channel, then fixed until it empties. Per-channel "Region Override" needs Manage Channels. Cloning a channel or making a template copies the region. | https://support.discord.com/hc/en-us/articles/1500007645701 |
| DM call region switch | In DM calls the region follows the caller. On desktop, click the region name at the top right of the call screen to change it. Not available on mobile. | https://support.discord.com/hc/en-us/articles/1500007645701 |
| Channel bitrate | Set in Edit Channel > Overview > Bitrate. Caps: Level 1 (2 boosts) 128 kbps, Level 2 (7) 256 kbps, Level 3 (14) 384 kbps. | https://support.discord.com/hc/en-us/articles/11635925354775 |
| Voice channel status | While connected, choose "Set a channel status" under the channel name: text plus optional emoji. Needs the Voice Channel Status permission. Also in the right-click menu. (2023) | https://support.discord.com/hc/en-us/articles/19583625604887 ; https://discord.com/blog/discord-patch-notes-february-4-2026 |
| Voice channel timer | Shows how long a server voice channel has been active. (2025) | https://discord.com/blog/discord-update-november-6-2025-changelog |
| Collapsed voice channels and facepile | Voice channels in the sidebar show participant avatars (a facepile) and collapse. Clicking an avatar opens the profile popout. | https://discord.com/blog/discord-patch-notes-july-7-2026 |
| Voice user popover | Popover on a voice participant: rich-presence game title (click for game details), plus Join / "Ask to Join" for embedded Activities. | https://discord.com/blog/discord-patch-notes-july-7-2026 |
| Text Chat in Voice | "Open Chat" bubble beside the voice channel name or at the top right of the call. Previews of new messages appear at the top right (at most 3; turn off with "Show New Message Previews"). Red badge counts mentions, gray badge counts unreads. Needs Send Messages and Read Message History. | https://support.discord.com/hc/en-us/articles/4412085582359 |
| Invite to Voice | While in voice, suggests server members and close friends to invite. (2026) | https://discord.com/blog/discord-update-march-24-2026-changelog |
| Voice invite embeds | Voice invites in chat show who is already in the call (facepile popover) and have a Join button. (2026) | https://discord.com/blog/discord-update-june-25-2026-changelog ; https://discord.com/blog/discord-patch-notes-may-4-2026 |
| Request to Stream | Ask a voice participant to stream their game. Sends an embed that detects the game, with a cooldown against spam. (2026) | https://discord.com/blog/discord-patch-notes-july-7-2026 ; https://discord.com/blog/discord-patch-notes-october-6-2026 |
| Stream spectators in channel list | Viewers of a stream show under the voice channel, including in channels where Soundboard is disabled. | https://discord.com/blog/discord-patch-notes-may-4-2026 |
| Move / drag members | Members with Move Members can drag users (including guests) between voice channels. | https://discord.com/blog/discord-patch-notes-july-7-2026 |
| Server Mute / Server Deafen / Disconnect / Move (moderator) | Moderator right-click actions on a voice member. They also work on members who blocked the moderator. | https://discord.com/blog/discord-patch-notes-october-6-2026 |
| Priority Speaker | Needs the Priority Speaker permission (server-wide or per channel) and a "Push to Talk (Priority)" keybind. Push to Talk only. Holding it ducks everyone else; a visual cue marks the priority speaker. Desktop only. | https://support.discord.com/hc/en-us/articles/360011876531 |
| Living Room | Experimental voice feature for a very limited set of servers (popped-out call, avatar decoration picker). | https://support.discord.com/hc/en-us/articles/19583625604887 ; https://discord.com/blog/discord-patch-notes-october-6-2026 |
| Hang Status | Experimental auto status set on joining voice (default "Chilling"-style text), visible to people who can access the channel. (unconfirmed) | https://support.discord.com/hc/en-us/community/posts/20132038290199 |

### User panel (bottom-left)

| Feature | Detail (states, dialogs, conditions such as permission/Nitro/DM-only) | Source URL |
|---|---|---|
| Mute button | Toggles mic transmit. The mute indicator sits right of your avatar. A muted mic gets a stronger red glow (2025). | https://support.discord.com/hc/en-us/articles/360045138471 ; https://discord.com/blog/player-release-q12025 |
| Deafen button | Toggles audio playback. Deafening also mutes the mic. | https://support.discord.com/hc/en-us/articles/217083547 |
| User Settings cog | Opens User Settings from the cog beside your avatar. | https://support.discord.com/hc/en-us/articles/360045138471 |
| Input Options menu (arrow beside mic) | Switch input/output device, toggle the Push to Talk checkbox, open Voice Settings. | https://discord.com/blog/discord-patch-notes-may-4-2026 ; https://support.discord.com/hc/en-us/articles/360041721052 |
| Audio-device icons | Icons tell device types apart (headset or desktop speakers) instead of relying on generic default names. (2026) | https://discord.com/blog/discord-update-september-25-2026 |

### Call UI (voice/video call view)

| Feature | Detail (states, dialogs, conditions such as permission/Nitro/DM-only) | Source URL |
|---|---|---|
| Grid View | Default for all calls. Shows every video and stream tile. | https://support.discord.com/hc/en-us/articles/360041721052 |
| Focus View | One tile large, others small along the bottom. Hide Members / Show Members toggles the strip. | https://support.discord.com/hc/en-us/articles/360041721052 |
| Pop Out View | Moves the call into a separate movable window, with Stay On Top / Remove from Top. | https://support.discord.com/hc/en-us/articles/360041721052 |
| Full Screen View | Maximizes the call or video. Text in Voice also works in the fullscreen call view. | https://support.discord.com/hc/en-us/articles/360041721052 ; https://discord.com/blog/discord-patch-notes-july-7-2026 |
| Pop out individual tiles | Pop a single user or stream tile into its own window from the tile's right-click menu. (2026) | https://discord.com/blog/discord-patch-notes-april-6-2026 |
| Show Non-Video Participants | In the three-dot menu at the top right: uncheck it to hide voice-only tiles. Desktop only. | https://support.discord.com/hc/en-us/articles/360045784891 |
| Participant tile options | Right-click a tile or use its three-dot menu: stop watching a stream, mute the user, set stream volume, pop out, View Verification Code. | https://support.discord.com/hc/en-us/articles/360045784891 ; https://support.discord.com/hc/en-us/articles/25968222946071 |
| Consolidated call control bar | Voice and video controls grouped into a single bar during calls. Camera button stays green while on. (2025) | https://discord.com/blog/discord-update-march-25-2025-changelog |
| Speaking indicator | Green highlight or ring on the tile/avatar of whoever is speaking (also in collapsed voice channels). | https://discord.com/blog/discord-patch-notes-march-6-2026 ; https://discord.com/blog/discord-patch-notes-april-6-2026 |
| Mute / deafen icons on tiles | Tiles show a mute icon. Non-video participant tiles show a phone icon (2026). | https://discord.com/blog/discord-patch-notes-march-6-2026 |
| Server mute / deafen indicators | Distinct icons for server-muted or server-deafened members. (unconfirmed) | https://discord.com/blog/discord-patch-notes-october-6-2026 |
| Call controls auto-hide | Controls hide when idle, except while a control has keyboard focus. | https://discord.com/blog/discord-patch-notes-july-7-2026 |
| Screenshare zoom and pan | Zoom and pan a screenshare or game stream with the scroll wheel or trackpad. (2026) | https://discord.com/blog/discord-patch-notes-february-4-2026 ; https://discord.com/blog/discord-update-march-24-2026-changelog |
| Picture-in-Picture | Navigating away from a call keeps a floating PiP video window. It can be resized freely (2024). | https://discord.com/blog/discord-update-august-28-2024-changelog |
| Display stays awake | The display does not sleep while you watch video or a stream in a call. (2026) | https://discord.com/blog/discord-patch-notes-september-8-2026 |
| Per-user volume | Right-click a user > volume slider. The 0–200% range is unconfirmed. | https://support.discord.com/hc/en-us/articles/360045138471 |
| Local mute of a user | Right-click a participant > Mute. Affects only you. | https://support.discord.com/hc/en-us/articles/360045784891 |
| Custom username styles in voice list | Nitro display-name fonts and colors show in the voice channel user list. (2026) | https://discord.com/blog/discord-patch-notes-september-8-2026 |

### Video (camera)

| Feature | Detail (states, dialogs, conditions such as permission/Nitro/DM-only) | Source URL |
|---|---|---|
| Turn on / off camera | Video button in the voice panel, or "Turn on Camera" / "Turn off Camera" in the call. Needs the Video permission in servers. | https://support.discord.com/hc/en-us/articles/360041721052 |
| Start Video Call / Start Voice Call (DM) | Buttons in the DM / group DM header. A voice call can add camera later. | https://support.discord.com/hc/en-us/articles/360041721052 |
| Camera selection | Camera dropdown in Voice & Video settings and in the arrow menu beside the Video button. | https://support.discord.com/hc/en-us/articles/360041721052 |
| Test Video | Previews the camera in settings. | https://support.discord.com/hc/en-us/articles/360041721052 |
| Always preview video / Preview Camera | When on, clicking Video opens a preview (pick a background) before "Turn On Camera". When off, the camera starts with the last background. | https://support.discord.com/hc/en-us/articles/4413490191127 |
| Camera-on sound | A sound plays when you turn on your camera. (2025) | https://discord.com/blog/discord-update-september-25-2025-changelog |
| Background Blur | Free. Desktop only. | https://support.discord.com/hc/en-us/articles/4413490191127 |
| Preset video backgrounds | Four Discord presets, free, cannot be removed. | https://support.discord.com/hc/en-us/articles/4413490191127 |
| Custom video backgrounds | Nitro upload of JPG/PNG/GIF/MP4, 10 MB images, max 10 per account, crop and center. Remove via right-click > Remove Background. (The intro also says Nitro Basic.) | https://support.discord.com/hc/en-us/articles/4413490191127 |
| Change Video Background mid-call | Arrow next to the Video button > Change Video Background / Preview Camera. Works in servers and DMs. | https://support.discord.com/hc/en-us/articles/4413490191127 |
| Background replacement on Linux | Background replacement added for Linux desktop. (2026) | https://discord.com/blog/discord-patch-notes-march-6-2026 |
| Video filters | No official source found describing camera filters beyond blur and backgrounds. (unconfirmed) | https://support.discord.com/hc/en-us/articles/4413490191127 |
| Video codecs | Uses OS codecs. AVC (H.264) is needed for camera and Go Live. AV1 is used when every participant supports it. Windows Store codec extensions. | https://support.discord.com/hc/en-us/articles/37976724130711 |
| Video unavailable by region | Video features unavailable in Brazil. (2026) | https://support.discord.com/hc/en-us/articles/42704051358359 |

### Go Live / screen share

| Feature | Detail (states, dialogs, conditions such as permission/Nitro/DM-only) | Source URL |
|---|---|---|
| Start screen share | Screen icon in the voice status panel (or "Share Your Screen" in a DM call), then the picker: Applications or Screens, then "Go Live". Needs the Video permission. | https://support.discord.com/hc/en-us/articles/360040816151 |
| macOS share flow | Choose entire screen / window / app / "Or share a device", set quality, then the macOS system picker ("Make Selection"). Needs macOS Screen Recording permission. | https://support.discord.com/hc/en-us/articles/26459694693783 ; https://support.discord.com/hc/en-us/articles/33030151293079 |
| Stream a game | Game must be detected (Add it! in Game Activity / Registered Games). Stream button with voice channel, quality and framerate choice. | https://support.discord.com/hc/en-us/articles/360040816151 |
| Streamer PiP preview | Floating preview of your own stream with settings / end controls. Pauses when the game is focused. | https://support.discord.com/hc/en-us/articles/360040816151 |
| Stream quality | Resolution and frame rate. Free: up to 720p30. Nitro: up to 4K60. Browser: fixed. Change mid-stream via the gear > Stream Quality (server) or the down arrow (DM). | https://support.discord.com/hc/en-us/articles/360040816151 |
| Change Window / Stop Streaming | Swap the shared window or end the stream from the call menu. | https://support.discord.com/hc/en-us/articles/360045784891 |
| Stream audio | Supported on the Windows and macOS desktop apps, Chrome, and mobile. Not on Linux or other browsers. Error 1001 if system sound-sharing permission is not granted. | https://support.discord.com/hc/en-us/articles/360040816151 ; https://support.discord.com/hc/en-us/articles/30952914470807 |
| Concurrent streams | Up to 50 people sharing video or screen at once in one voice chat. Video and Go Live can run together. | https://support.discord.com/hc/en-us/articles/360040816151 |
| Stream Previews setting | Global toggle in Voice & Video, plus per-stream "Hide stream preview". Previews are not E2EE. | https://support.discord.com/hc/en-us/articles/360040816151 ; https://support.discord.com/hc/en-us/articles/25968222946071 |
| LIVE badge | Badge next to a streamer in the voice channel list. | https://support.discord.com/hc/en-us/articles/360040816151 |
| Watch Stream | Double-click the streamer, or click and choose Watch Stream. You can be in voice without watching. 50 viewers plus the broadcaster. | https://support.discord.com/hc/en-us/articles/360040816151 |
| Multistream | Eye icon > "Start watching multiple streams". Desktop only. | https://support.discord.com/hc/en-us/articles/360045784891 |
| Stream volume / mute | Hover the stream for the volume icon. Right-click > Mute / Stream Volume slider. | https://support.discord.com/hc/en-us/articles/360040816151 |
| Viewer Pop Out | Open a stream in a separate resizable window with Stay On Top. | https://support.discord.com/hc/en-us/articles/360040816151 |
| Spectators list | Viewers shown with an eye icon (overlay and stream spectators menu). | https://support.discord.com/hc/en-us/articles/360040816151 ; https://discord.com/blog/discord-patch-notes-october-6-2026 |
| Stream Privacy Code | Right-click the stream for its E2EE privacy code. | https://support.discord.com/hc/en-us/articles/25968222946071 |
| Invite to Stream / stream invite embed | Invite to Stream button. Stream invites in chat render with a Join button. (2026) | https://discord.com/blog/discord-patch-notes-april-6-2026 ; https://discord.com/blog/discord-patch-notes-march-6-2026 |
| Faster Go Live start | Single-window streams start faster, and stream preview loads faster in the start flow. (2026) | https://discord.com/blog/discord-update-march-24-2026-changelog ; https://discord.com/blog/discord-patch-notes-february-4-2026 |
| Streaming error codes | 2001 start failure, 2002–2006 low fps / packet loss / poor connection, 2007 reconnecting, 2008/2009 decoder/encoder, 2010 max viewers, 2011–2015 timeouts. | https://support.discord.com/hc/en-us/articles/30952914470807 |

### DM and group calls

| Feature | Detail (states, dialogs, conditions such as permission/Nitro/DM-only) | Source URL |
|---|---|---|
| Start call | Phone (voice) or camera (video) icon at the top right of a DM / group DM. | https://support.discord.com/hc/en-us/articles/223657667 |
| Group call size | Group DMs hold up to 10 members including you. The call stays open while at least one person remains. | https://support.discord.com/hc/en-us/articles/223657667 |
| Incoming Call modal | Desktop incoming-call modal with Answer (Cmd+Return) and Decline (Esc). | https://discord.com/blog/discord-patch-notes-february-4-2026 ; https://support.discord.com/hc/en-us/articles/225878307 |
| Ringing and call sounds | Incoming and outgoing ring sounds can be toggled in User Settings > Notifications. | https://support.discord.com/hc/en-us/articles/223657667 |
| Ring confirmation | Mobile asks before ringing everyone in a DM / group DM. Mobile only. (2026) | https://discord.com/blog/discord-update-june-25-2026-changelog |
| Ring individual members | Ring a group DM member who hasn't joined. (unconfirmed) | https://support.discord.com/hc/en-us/articles/223657667 |

### Stage channels

| Feature | Detail (states, dialogs, conditions such as permission/Nitro/DM-only) | Source URL |
|---|---|---|
| Stage channels | Community servers only. Not E2EE. | https://support.discord.com/hc/en-us/articles/1500005513722 ; https://support.discord.com/hc/en-us/articles/25968222946071 |
| Roles | Stage moderator, speaker, audience. Audience joins muted, silently. | https://support.discord.com/hc/en-us/articles/1500005513722 |
| Start the Stage | Needs a topic, notifies the server. Alternatives: Create Event, or Continue without starting (no topic or notification, for testing audio). | https://support.discord.com/hc/en-us/articles/1500005513722 |
| Stage topic | Shown at the top of the Stage and in the channel list while live. | https://support.discord.com/hc/en-us/articles/1500005513722 |
| Request to Speak | Raised-hand button at the bottom left of the action bar. | https://support.discord.com/hc/en-us/articles/1500005513722 |
| Invite to speak | Moderators invite audience members, with or without a raised hand. The invitee must accept. | https://support.discord.com/hc/en-us/articles/1500005513722 |
| Show Requests | Moderator list of raised hands to invite or dismiss. | https://support.discord.com/hc/en-us/articles/1500005513722 |
| Allow requests to speak toggle | @everyone toggle that disables hand raising. | https://support.discord.com/hc/en-us/articles/1500005513722 |
| Move to Audience | Moderator demotes a speaker, or a speaker moves themselves. | https://support.discord.com/hc/en-us/articles/1500005513722 |
| Disconnect from Stage | Moderator removes a member via right-click. | https://support.discord.com/hc/en-us/articles/1500005513722 |
| Exit Quietly | Audience leaves via the bottom-right button without notifying others. | https://support.discord.com/hc/en-us/articles/1500005513722 |
| Waiting music | Plays for waiting members. Each user can mute it with the music-note button. | https://support.discord.com/hc/en-us/articles/1500005513722 |
| Video / Go Live on Stage | Speakers can use camera or Go Live: one stream and up to 5 cameras. Video audience 50 / 150 (Boost T2) / 300 (T3), up to 10,000 via extra boosts. Audio-only audience up to 10,000. | https://support.discord.com/hc/en-us/articles/1500005513722 |
| Stage text chat | Text in Voice inside Stage channels. Works alongside PiP (2026). | https://support.discord.com/hc/en-us/articles/1500005513722 ; https://discord.com/blog/discord-patch-notes-october-6-2026 |
| End Stage | Ends when the last moderator leaves. Android moderators can end it by long-pressing the channel; an explicit desktop End Stage control is unconfirmed. | https://support.discord.com/hc/en-us/articles/1500005513722 ; https://discord.com/blog/discord-patch-notes-july-7-2026 |
| Stage pop-out | Stage can be popped out, with a "Start Stage Channel" button in the pop-out. | https://discord.com/blog/discord-patch-notes-july-7-2026 |
| Stage permissions | Moderators need Manage Channels, Mute Members and Move Members on the channel. Starting needs Mention @everyone and related permissions. | https://support.discord.com/hc/en-us/articles/1500005513722 |

### Soundboard

| Feature | Detail (states, dialogs, conditions such as permission/Nitro/DM-only) | Source URL |
|---|---|---|
| Open Soundboard | Confetti / Soundboard icon in the call bar or the voice panel at the bottom left. Shortcut Cmd+Shift+B while in a call. | https://support.discord.com/hc/en-us/articles/12612888127767 ; https://discord.com/blog/how-to-use-keyboard-shortcuts-on-discord-create-custom-keybinds |
| Play sound | Hover > Play: everyone in the channel hears it. Deafened users don't. | https://support.discord.com/hc/en-us/articles/12612888127767 |
| Preview sound | Speaker icon plays it only for you. | https://support.discord.com/hc/en-us/articles/12612888127767 |
| Favorite sound | Star icon. | https://support.discord.com/hc/en-us/articles/12612888127767 |
| Default sounds | Discord built-in sounds available to everyone. (unconfirmed) | https://support.discord.com/hc/en-us/articles/12612888127767 |
| Server sounds and slots | 8 slots per server; 24 / 36 / 48 at Boost T1 / T2 / T3. "Add Sound" upload (desktop/web only; OGG allowed). Manage in Server Settings > Soundboard. | https://support.discord.com/hc/en-us/articles/12612888127767 ; https://discord.com/blog/discord-patch-notes-october-6-2026 |
| Soundboard permissions | Use Soundboard (per role or channel), Create Expressions, Manage Expressions. | https://support.discord.com/hc/en-us/articles/12612888127767 |
| Cross-server sounds | Using another server's sounds requires Nitro. | https://support.discord.com/hc/en-us/articles/12612888127767 |
| Entrance sounds | Nitro: User Settings > Voice & Video > Soundboard > Sound Effects. One global sound or one per server. Plays when joining muted but not deafened, and not on auto-reconnect (2026). | https://support.discord.com/hc/en-us/articles/12612888127767 ; https://discord.com/blog/discord-patch-notes-february-4-2026 |
| Soundboard Volume | Slider in Voice & Video, synced across devices. 0% silences all sounds. | https://support.discord.com/hc/en-us/articles/12612888127767 |
| Soundboard in overlay | Windows overlay: Ctrl+` opens Soundboard. | https://support.discord.com/hc/en-us/articles/12612888127767 |
| Soundboard data preload | Sounds load when you join voice. (2026) | https://discord.com/blog/discord-patch-notes-may-4-2026 |

### Activities

| Feature | Detail (states, dialogs, conditions such as permission/Nitro/DM-only) | Source URL |
|---|---|---|
| App Launcher | Button right of the chat bar and at the bottom of a call. Recents, Collections, search, tags (In-App Purchases, Ad-Supported, Partner, Promoted). (2024) | https://support.discord.com/hc/en-us/articles/21334461140375 ; https://discord.com/blog/discord-update-september-26-2024-changelog |
| Launch an Activity in voice | In any voice call (server, DM, group DM). First use requires app authorization. | https://support.discord.com/hc/en-us/articles/21334461140375 |
| Join an Activity | Others in the same voice channel join. Each authorizes on first use. Voice popover shows Join or "Ask to Join". | https://support.discord.com/hc/en-us/articles/21334461140375 ; https://discord.com/blog/discord-patch-notes-july-7-2026 |
| Activities in text channels / DMs | Start an Activity from "+" in a server text channel without joining voice (2024), and in DM / group DM text chats (2024). | https://discord.com/blog/discord-update-may-13-2024-changelog ; https://discord.com/blog/discord-update-july-26-2024-changelog |
| Pop-out Activities | Activities such as Watch Together can float in their own window while you browse. (2025) | https://discord.com/blog/discord-update-september-25-2025-changelog |
| Watch Together host controls | Recent searches; admins can revoke host control. (2024) | https://discord.com/blog/discord-update-april-3-2024-changelog |
| Rocket button in voice | Suggests Activities in voice. (2024) | https://discord.com/blog/discord-update-june-20-2024-changelog |

### Voice & Video settings

| Feature | Detail (states, dialogs, conditions such as permission/Nitro/DM-only) | Source URL |
|---|---|---|
| Input Device | Mic dropdown. | https://support.discord.com/hc/en-us/articles/360045138471 |
| Output Device | Speaker/headset dropdown. | https://support.discord.com/hc/en-us/articles/360045138471 |
| Input Volume / Output Volume | Sliders. | https://support.discord.com/hc/en-us/articles/360045138471 |
| Mic Test ("Let's Check") | Loops your voice to the output. Auto-mutes and deafens you in an active call. "Stop Testing". Error shown if no input is detected. | https://support.discord.com/hc/en-us/articles/360020641332 |
| Input Mode | Voice Activity or Push to Talk. | https://support.discord.com/hc/en-us/articles/211376518 |
| Automatically determine input sensitivity | Checkbox. When off, a manual slider with a live meter (stay in yellow). Voice Activity has about 200 ms inherent delay. | https://support.discord.com/hc/en-us/articles/211376518 |
| Push to Talk shortcut | Edit Keybind / Record Keybind. In browsers it works only while focused. | https://support.discord.com/hc/en-us/articles/211376518 |
| Push To Talk Release Delay | Slider for how long transmit continues after release. | https://support.discord.com/hc/en-us/articles/211376518 |
| PTT Activate / Deactivate sounds | Toggles in User Settings > Notifications. | https://support.discord.com/hc/en-us/articles/209813978 |
| Noise Suppression | Krisp (on-device ML, free), Standard, or None. Toggle in settings or from the in-call noise suppression icon. Krisp turns itself off under high CPU with a notice. Krisp on Mac needs Apple silicon and macOS 12+. (Standard/None options unconfirmed) | https://support.discord.com/hc/en-us/articles/360040843952 ; https://www.techradar.com/computing/windows/how-to-improve-your-discord-audio-quality |
| Echo Cancellation | Toggle. System-wide echo cancellation of all PC audio on Windows (2026). | https://discord.com/blog/discord-update-september-25-2026 ; https://www.howtogeek.com/663414/how-to-configure-your-microphone-and-headset-in-discord/ |
| Automatic Gain Control | Toggle (desktop label per third party; mobile official). (unconfirmed for desktop) | https://www.howtogeek.com/663414/how-to-configure-your-microphone-and-headset-in-discord/ ; https://support.discord.com/hc/en-us/articles/360045138471 |
| Noise Reduction / Advanced Voice Activity | Mobile has Noise Reduction. Advanced Voice Activity is confirmed on Android; desktop availability is unconfirmed. | https://support.discord.com/hc/en-us/articles/360045138471 ; https://discord.com/blog/discord-patch-notes-july-7-2026 |
| Quality of Service High Packet Priority | "Enable Quality of Service High Packet Priority" toggle. | https://support.discord.com/hc/en-us/articles/360045138471 |
| Attenuation | Slider (0% disables) that lowers other apps' volume. Desktop only. Sub-toggles "When I speak" / "When others speak" are unconfirmed. | https://support.discord.com/hc/en-us/articles/206342888 ; https://www.howtogeek.com/663414/how-to-configure-your-microphone-and-headset-in-discord/ |
| Audio Subsystem | Windows only: Standard / Legacy. | https://support.discord.com/hc/en-us/articles/360045138471 |
| Camera / Test Video / Always preview video / backgrounds | In the Video section. | https://support.discord.com/hc/en-us/articles/4413490191127 |
| Hardware Acceleration | In the Video section. Disable it if streams lag. A separate app-wide toggle lives under Advanced. | https://support.discord.com/hc/en-us/articles/360045138471 ; https://support.discord.com/hc/en-us/articles/213491697 |
| OpenH264 Video Codec | "OpenH264 Video Codec provided by Cisco Systems, Inc." toggle. | https://support.discord.com/hc/en-us/articles/4978019693463 |
| Stream Previews | Toggle (see Go Live). | https://support.discord.com/hc/en-us/articles/360040816151 |
| Soundboard section | Soundboard Volume, Entrance Sounds. | https://support.discord.com/hc/en-us/articles/12612888127767 |
| Voice Activity notification | Setting restored (2026). The exact label is unconfirmed. | https://discord.com/blog/discord-patch-notes-may-4-2026 |
| Debugging | Debug log for support tickets, "Reset Voice and Video Settings". | https://support.discord.com/hc/en-us/articles/360045138471 |
| Discord System Helper | Experimental Windows component that lets keybinds work in elevated games. Install prompt in Overlay / Keybinds / Voice & Video / Windows Settings. Windows only. (2025) | https://support.discord.com/hc/en-us/articles/34853435033367 |
| Settings page redesign | Voice & Video, Notifications, Clips and Streamer Mode pages restyled. (2026) | https://discord.com/blog/discord-update-march-24-2026-changelog |

### End-to-end encryption (DAVE)

| Feature | Detail (states, dialogs, conditions such as permission/Nitro/DM-only) | Source URL |
|---|---|---|
| E2EE A/V coverage | DMs, group DMs, voice channels and Go Live. On by default, cannot be disabled. Required (per a help article published 2026-03-02). Not Stage channels or stream previews. (2024–2026) | https://support.discord.com/hc/en-us/articles/25968222946071 ; https://discord.com/blog/discord-update-september-26-2024-changelog |
| Encrypted indicator | Green lock "End-to-end encrypted" in Voice/Video Details. | https://support.discord.com/hc/en-us/articles/25968222946071 |
| Voice Privacy Code | Privacy tab: groups of five digits. Changes when anyone joins or leaves; compare out-of-band. | https://support.discord.com/hc/en-us/articles/25968222946071 |
| Stream Privacy Code | From the stream's right-click menu. | https://support.discord.com/hc/en-us/articles/25968222946071 |
| View Verification Code | Right-click a user (member list or tile). Per-device code that resets each call by default. | https://support.discord.com/hc/en-us/articles/25968222946071 |
| Mark as Verified | After comparing codes: a small icon appears by the name, with a warning if a verified code later changes. | https://support.discord.com/hc/en-us/articles/25968222946071 |
| Persistent verification codes | User Settings > Privacy & Safety > Encryption > "Enable persistent verification codes" (both parties). Keys are per device. | https://support.discord.com/hc/en-us/articles/25968222946071 |
| Manage verified devices | Remove verification per device or for all of a user's devices, under Privacy & Safety > Encryption. | https://support.discord.com/hc/en-us/articles/25968222946071 |

### Clips

| Feature | Detail (states, dialogs, conditions such as permission/Nitro/DM-only) | Source URL |
|---|---|---|
| Clips availability | Windows only (not macOS, mobile or console). Started as Nitro early access (2023). Minimum GPU: GTX 980/1600, RTX, or RX 500. | https://support.discord.com/hc/en-us/articles/16861982215703 ; https://discord.com/blog/discord-update-october-19-2023-changelog |
| Enable Clips | User Settings > Clips toggle. Default on for some high-end GPUs. | https://support.discord.com/hc/en-us/articles/16861982215703 |
| Clip keybind | Default Alt+C (Windows). | https://support.discord.com/hc/en-us/articles/16861982215703 |
| Clip length | 30 s, 60 s, or 2 min. | https://support.discord.com/hc/en-us/articles/16861982215703 |
| Hardware encoding / storage location | Toggle for compression, and a folder picker. Clips are stored locally only. | https://support.discord.com/hc/en-us/articles/16861982215703 |
| Save Clip button | Next to the streamed game's title. Overlay pop-up and sound confirm the clip. | https://support.discord.com/hc/en-us/articles/16861982215703 |
| Share / edit / delete clips | "+" > Share a Clip: trim, toggle Game Audio / Voice Channel Audio, rename, Share. Deleting removes the file from disk. | https://support.discord.com/hc/en-us/articles/16861982215703 |
| Clip notice to channel | Voice channel members are notified when someone with Clips enabled streams. (unconfirmed) | https://pcgamer.com/discord-clips-feature-privacy |

### Voice messages and overlay voice widget

| Feature | Detail (states, dialogs, conditions such as permission/Nitro/DM-only) | Source URL |
|---|---|---|
| Voice message playback | Desktop can play voice messages (Play button) but cannot record or send them; that is mobile only. Up to 20 min. Right-click > Report. | https://support.discord.com/hc/en-us/articles/13091096725527 |
| Overlay voice widget | Windows-only overlay. Display Users (Always / Only While Speaking), Display Names (Always / Only While Speaking / Never), Avatar Size (Large / Small). | https://support.discord.com/hc/en-us/articles/217659737 |

## Threads and forums

### Threads

| Feature | Detail (states, dialogs, conditions such as permission/Nitro/DM-only) | Source URL |
|---|---|---|
| Create thread from message | Message "…" > Create Thread; optional title; first message. Needs Create Public Threads. | https://support.discord.com/hc/en-us/articles/4403205878423 |
| Create thread from Threads menu / chat-bar + | Start from the Threads menu at the top or the attachment menu in the chat bar. | https://support.discord.com/hc/en-us/articles/4403205878423 |
| Create thread from a reply | Supported; the first message is not double-sent (2026 fix). | https://discord.com/blog/discord-patch-notes-october-6-2026 |
| Private thread toggle | "Private Thread" toggle. Needs Create Private Threads; the help article also says Boost Level 2 is required. Visible to invitees and Manage Threads holders. | https://support.discord.com/hc/en-us/articles/4403205878423 ; https://docs.discord.com/developers/resources/channel |
| "Anyone can invite" (private threads) | Unchecking it restricts who can add members (invitable). | https://support.discord.com/hc/en-us/articles/4404809613847 ; https://docs.discord.com/developers/resources/channel |
| Announcement threads | Threads in announcement channels (ANNOUNCEMENT_THREAD). | https://docs.discord.com/developers/resources/channel |
| Edit Thread: name | Rename (creator or Manage Threads). Invalid or zero-width names show an error (2026). | https://support.discord.com/hc/en-us/articles/4404809613847 ; https://discord.com/blog/discord-patch-notes-april-6-2026 |
| Hide After Inactivity | 1 hour, 24 hours, 3 days (default) or 1 week. | https://support.discord.com/hc/en-us/articles/4403205878423 ; https://support.discord.com/hc/en-us/articles/33694251638295 |
| Close Thread / Open Thread | Closing archives without deleting; typing reopens it. | https://support.discord.com/hc/en-us/articles/4403205878423 |
| Lock thread | Moderators can lock while archiving; only Manage Threads can reopen. | https://support.discord.com/hc/en-us/articles/4404809613847 |
| Delete thread | Manage Threads; confirmation (Enter confirms, 2026). | https://support.discord.com/hc/en-us/articles/4404809613847 ; https://discord.com/blog/discord-patch-notes-may-4-2026 |
| Join Thread / Leave Thread | Typing joins; ellipsis menu to join or leave. | https://support.discord.com/hc/en-us/articles/4403205878423 |
| Thread notification settings | All Messages / Only @mentions / Nothing. | https://support.discord.com/hc/en-us/articles/4403205878423 |
| Mute thread | Mute button on threads (Android fix 2026). Muted threads are skipped by the "new" jump. | https://discord.com/blog/discord-patch-notes-april-6-2026 ; https://discord.com/blog/discord-patch-notes-october-6-2026 |
| Threads in sidebar | Joined active threads nest under the parent channel; closed threads are removed; hover the channel > "See All". | https://support.discord.com/hc/en-us/articles/4403205878423 |
| Threads browser | Lists active and older threads with preview text. | https://support.discord.com/hc/en-us/articles/4403205878423 ; https://discord.com/blog/discord-patch-notes-august-4-2026 |
| Active Threads view | Server-wide active threads view; respects spoiler channels (2026). | https://discord.com/blog/discord-patch-notes-august-4-2026 ; https://docs.discord.com/developers/resources/guild |
| Thread member list | Members popout for a thread. Moderators remove users; the creator can remove users from private threads. Cap 1,000 members. | https://discord.com/blog/discord-patch-notes-june-4-2026 ; https://support.discord.com/hc/en-us/articles/4404809613847 ; https://support.discord.com/hc/en-us/articles/33694251638295 |
| Thread @mentions | A user mention adds the user. A role mention adds role members who can view (role under 100 members, up to 10 roles). A failure flag shows when some roles couldn't be added. | https://support.discord.com/hc/en-us/articles/4403205878423 ; https://docs.discord.com/developers/resources/message |
| Thread slowmode | Per thread, 5 seconds to 6 hours; not inherited from the parent; Manage Threads only. | https://support.discord.com/hc/en-us/articles/4403205878423 ; https://support.discord.com/hc/en-us/articles/4404809613847 |
| "X started a thread" system message | THREAD_CREATED message in the parent channel; THREAD_STARTER_MESSAGE at the top of the thread. | https://docs.discord.com/developers/resources/message |
| Split view | Threads open in a side panel with a drop-down (Pins and other actions). | https://discord.com/blog/discord-patch-notes-may-4-2026 ; https://discord.com/blog/discord-patch-notes-july-7-2026 |
| Search in threads | Channel search includes thread results, and archived threads can be searched. | https://support.discord.com/hc/en-us/articles/4403205878423 ; https://discord.com/blog/discord-patch-notes-july-7-2026 |
| Polls in threads | Supported. | https://discord.com/blog/discord-patch-notes-february-4-2026 |
| Locked thread read-only | Typing is blocked; Forward and Bookmark remain available. | https://discord.com/blog/discord-patch-notes-october-6-2026 ; https://discord.com/blog/discord-patch-notes-february-4-2026 |

### Forums

| Feature | Detail (states, dialogs, conditions such as permission/Nitro/DM-only) | Source URL |
|---|---|---|
| Create forum channel | + on a category > Forum; "Set recommended permissions". Community only. | https://support.discord.com/hc/en-us/articles/6208479917079 |
| New post composer | Create Post: title (line breaks stripped), message, tags, media/attachments. Needs Send Messages. Starter message required. | https://support.discord.com/hc/en-us/articles/6208479917079 ; https://discord.com/blog/discord-patch-notes-october-6-2026 ; https://docs.discord.com/developers/resources/channel |
| Tags | Up to 20 per channel; name 0–20 chars; optional emoji; up to 5 per post. | https://docs.discord.com/developers/resources/channel |
| Moderated tags | Only Manage Threads can apply or remove them. | https://docs.discord.com/developers/resources/channel |
| Require tags | REQUIRE_TAG flag: a tag is needed to post; posting without one is blocked. | https://docs.discord.com/developers/resources/channel ; https://discord.com/blog/discord-patch-notes-october-6-2026 |
| Edit Tags on a post | Right-click post > Edit Tags. | https://support.discord.com/hc/en-us/articles/6208479917079 ; https://discord.com/blog/discord-patch-notes-september-8-2026 |
| Tag filter | Filter by All or several tags. | https://support.discord.com/hc/en-us/articles/6208479917079 ; https://discord.com/blog/discord-patch-notes-august-4-2026 |
| Sort order | Recent Activity or Creation Date (admin default; user can change). | https://docs.discord.com/developers/resources/channel |
| Layout | List View or Gallery View (admin default). | https://support.discord.com/hc/en-us/articles/6208479917079 ; https://docs.discord.com/developers/resources/channel |
| Post Guidelines | Shown at the top; up to 4096 chars. | https://support.discord.com/hc/en-us/articles/6208479917079 ; https://docs.discord.com/developers/resources/channel |
| Default reaction | Default emoji on every post with a visible "React to post" button. | https://docs.discord.com/developers/resources/channel ; https://discord.com/blog/discord-patch-notes-july-7-2026 |
| Hide after inactivity / Older Posts | Inactive posts move to "Older Posts" after 1h, 24h, 3d (default) or 1w. | https://support.discord.com/hc/en-us/articles/6208479917079 |
| Pinned post | One pinned post at a time. | https://support.discord.com/hc/en-us/articles/6208479917079 ; https://docs.discord.com/developers/resources/channel |
| Close Post / lock | Right-click > Close Post; locks so only moderators can reopen. | https://support.discord.com/hc/en-us/articles/6208479917079 |
| Post thumbnail | Reply with media and choose "Add to Post" (shows a video thumbnail, 2026). | https://support.discord.com/hc/en-us/articles/6208479917079 ; https://discord.com/blog/discord-patch-notes-october-6-2026 |
| Post search | Search titles; Clear link. | https://support.discord.com/hc/en-us/articles/6208479917079 ; https://discord.com/blog/discord-patch-notes-august-4-2026 |
| Post slowmode / message slowmode | Channel slowmode limits post creation; default thread slowmode applies to new posts. | https://support.discord.com/hc/en-us/articles/4404809613847 ; https://docs.discord.com/developers/resources/channel |
| Follow post | Followers get mention notifications for the post. | https://support.discord.com/hc/en-us/articles/6208479917079 |
| Post card | Title, preview, image, author role. Spoilers are hidden in previews. | https://discord.com/blog/discord-patch-notes-february-4-2026 |
| Post templates | No official source documents forum post templates (unconfirmed). | (unconfirmed, no official source found) |
| AutoMod in forums | A keyword match blocks post creation. | https://support.discord.com/hc/en-us/articles/4421269296535 |
| "New" badge on posts | New, unopened posts show a "New" badge in list and gallery views. Marking the server read clears it. | https://discord.com/blog/discord-patch-notes-august-4-2026 ; https://discord.com/blog/discord-patch-notes-september-8-2026 |
| New-post unread toggle | Per-forum option for whether new posts mark the channel unread (NEW_FORUM_THREADS_ON/OFF) (unconfirmed UI). | https://docs.discord.food/resources/user-settings |
| Adding media to an archived post | Unarchives the post first (2026). | https://discord.com/blog/discord-patch-notes-july-7-2026 |
| Forum post side view | Opening a post shows it in a side view next to the post list; the tag row and filter adapt on resize. | https://discord.com/blog/discord-patch-notes-august-4-2026 |

### LFG channel posts (2026 experiment)

| Feature | Detail (states, dialogs, conditions such as permission/Nitro/DM-only) | Source URL |
|---|---|---|
| LFG Create Post | Description, tags, add or remove voice chat, optional "Link Account" for a live invite embed, then Create. Only for members with the game installed. | https://support.discord.com/hc/en-us/articles/41832532728215 |
| Live session card | Shows open squad spots and a request to join. Needs a linked account, the game running at post time, and activity sharing on (globally, per server, and on the gaming platform). | https://support.discord.com/hc/en-us/articles/41832532728215 |
| Join an LFG post | Open the post > Join: either a request accepted in-game, or a direct join. Each post has a chat thread with voice. | https://support.discord.com/hc/en-us/articles/41832532728215 |

### Thread and post states (summary)

| Feature | Detail (states, dialogs, conditions such as permission/Nitro/DM-only) | Source URL |
|---|---|---|
| Active | Listed in the sidebar (if joined) and the Threads browser; accepts messages. | https://support.discord.com/hc/en-us/articles/4403205878423 |
| Archived (closed) | Hidden from the sidebar; listed under older threads; reopened by typing (needs Send Messages if unlocked). | https://docs.discord.com/developers/resources/channel |
| Locked | Only Manage Threads can unarchive; read-only for others. | https://docs.discord.com/developers/resources/channel ; https://support.discord.com/hc/en-us/articles/4404809613847 |
| Pinned (forum/media) | PINNED flag; one per channel; shown at the top of the post list. | https://docs.discord.com/developers/resources/channel ; https://support.discord.com/hc/en-us/articles/6208479917079 |

### Media channels

| Feature | Detail (states, dialogs, conditions such as permission/Nitro/DM-only) | Source URL |
|---|---|---|
| Create media channel | + > Media; beta for subscription servers. | https://creator-support.discord.com/hc/en-us/articles/14346342766743 |
| Media posts | Gallery-like posts; thumbnail limited to one item. | https://creator-support.discord.com/hc/en-us/articles/14346342766743 |
| Subscription gating | Tier role grants access; non-subscribers see a blurred image. | https://creator-support.discord.com/hc/en-us/articles/14346342766743 |
| Hide media download options | HIDE_MEDIA_DOWNLOAD_OPTIONS flag. | https://docs.discord.com/developers/resources/channel |

## User Settings

### Settings navigation & 2025–2026 reorganization

| Feature | Detail (states, dialogs, conditions such as permission/Nitro/DM-only) | Source URL |
|---|---|---|
| Open User Settings | Cogwheel next to username/avatar (bottom-left). While in a call, the settings button opens straight to Voice & Video (Aug 2025). | https://support.discord.com/hc/en-us/articles/209572128 ; https://discord.com/blog/discord-patch-notes-august-4-2025 |
| Settings search bar | Search field remains in the refreshed settings. | https://discord.com/blog/discord-update-november-6-2025-changelog |
| Desktop settings redesign (stage 1) | Settings refresh announced Nov 2025 (reorganization + visual changes, rolled out over months); shipped Dec 2025 with an "Edit Profiles" button at the top of settings. (2025) | https://discord.com/blog/discord-update-november-6-2025-changelog ; https://discord.com/blog/discord-patch-notes-december-8-2025 |
| "More" group at end of list | Changelog / What's New moved to "More" at the end of the Settings list (Settings > More > What's New). (2025) | https://discord.com/blog/discord-update-november-6-2025-changelog ; https://discord.com/blog/discord-update-september-25-2026 |
| Notifications / Voice & Video / Clips / Streamer Mode pages redesigned | Layout, styling, organization and phrasing updates (Mar 2026). (2026) | https://discord.com/blog/discord-update-march-24-2026-changelog |
| App settings consolidation | Appearance, Accessibility, Chat, Streamer Mode and Advanced consolidated under three pages: Appearance, Accessibility, Developer (May 2026). (2026) | https://discord.com/blog/discord-patch-notes-may-4-2026 |
| Account category with nested pages | Devices, Family Center, Account Standing, Multi-Factor Authentication are nested pages within Account (June 2026). (2026) | https://discord.com/blog/discord-patch-notes-june-4-2026 |
| Category renames | "Activity" → "Games & Apps"; "Content & Social" → "Messaging Permissions"; Authorized Apps/Connections consolidated; OS-specific pages and keybinds consolidated; Data & Privacy and Activity Privacy copy rewritten (Aug 2026). (2026) | https://discord.com/blog/discord-update-august-11-2026-changelog ; https://discord.com/blog/discord-patch-notes-august-4-2026 |
| Language & Time from Quick Switcher | "Language & Time" settings reachable via Quick Switcher (Feb 2026). (2026) | https://discord.com/blog/discord-patch-notes-february-4-2026 |

### My Account / Account

| Feature | Detail (states, dialogs, conditions such as permission/Nitro/DM-only) | Source URL |
|---|---|---|
| Account overview fields | Username (Edit), Display name, Email (Edit/reveal), Phone (Add/Remove). | https://support.discord.com/hc/en-us/articles/12620128861463 ; https://support.discord.com/hc/en-us/articles/4423385681175 ; https://support.discord.com/hc/en-us/articles/4413460214807 |
| Password and Authentication | Change Password; Register a Security Key; Enable/Remove Authenticator App; Enable SMS Authentication; View Backup Codes. | https://support.discord.com/hc/en-us/articles/26304482627095 ; https://support.discord.com/hc/en-us/articles/219576828 |
| Account Status (age group) | Shows age group, "Confirm Age"/"Retry". | https://support.discord.com/hc/en-us/articles/30326565624343 |
| Account Standing | See Account & login. | https://support.discord.com/hc/en-us/articles/18210965981847 |
| Account Removal | "Disable Account" and "Delete Account". | https://support.discord.com/hc/en-us/articles/360004066391 ; https://support.discord.com/hc/en-us/articles/212500837 |

### Profiles (main profile)

| Feature | Detail (states, dialogs, conditions such as permission/Nitro/DM-only) | Source URL |
|---|---|---|
| Profiles page tabs | User Settings > Profiles with User Profile and Per-server Profiles tabs; "Edit Profiles" button at top of settings. | https://support.discord.com/hc/en-us/articles/12620128861463 ; https://discord.com/blog/discord-patch-notes-december-8-2025 |
| Avatar | "Change Avatar": PNG/GIF (animated GIF needs Nitro); crop tool (keyboard-adjustable, Aug 2026). | https://support.discord.com/hc/en-us/articles/4403147417623 ; https://discord.com/blog/discord-patch-notes-august-4-2026 |
| Recent Avatars | Last six avatars saved for reuse (desktop). (2025) | https://discord.com/blog/discord-update-june-30-2025-changelog |
| Banner | "Change Banner": PNG/JPG/GIF under 10 MB, min 680×240; Nitro required to upload. | https://support.discord.com/hc/en-us/articles/4403147417623 |
| About Me / bio | Markdown, links, emoji (custom emoji with Nitro); limit raised from 190 to 300 chars (Sep 2026); desktop WYSIWYG formatting toolbar (Aug 2026). (2026) | https://discord.com/blog/discord-update-september-25-2026 ; https://discord.com/blog/discord-update-august-11-2026-changelog |
| Pronouns | Free-text pronouns field. | https://support.discord.com/hc/en-us/articles/4403147417623 |
| Profile Theme | Primary and Accent colors (Nitro); also drives Nitro call tile color (2026). | https://support.discord.com/hc/en-us/articles/4403147417623 ; https://support.discord.com/hc/en-us/articles/42383370736023 |
| Display Name Style | Nitro: 12 fonts (gg sans, Tempo, Sakura, Jellybean, Modern, Medieval, 8Bit, Vampyre, Monkey Bars, Mainframe, Headbang, Journal), 7 effects (Solid, Gradient, Neon, Toon, Pop, Gummy, Prism), colors; "Surprise Me"; light/dark preview; non-Nitro can preview only. (2025–2026) | https://support.discord.com/hc/en-us/articles/33833879643927 ; https://discord.com/blog/discord-update-september-25-2026 |
| Avatar Decoration | "Change Decoration": apply owned decorations (Shop, Quests, Orbs). | https://support.discord.com/hc/en-us/articles/13410113109911 |
| Profile Effect | "Change Effect": animated effect when profile is viewed. | https://support.discord.com/hc/en-us/articles/17828465914263 |
| Nameplate | "Change Nameplate": decorative background behind name in member/DM lists; only shown while online. (2025) | https://support.discord.com/hc/en-us/articles/30408457944215 |
| Profile Frame | "Change Frames" under "Profile Effect & Frame"; border around full profile (Nitro Early Access). (2026) | https://support.discord.com/hc/en-us/articles/40775065582615 |
| Server Tag | Profiles > Server Tag > "Select" a tag from a server that enabled tags; X to remove; one tag at a time. (2025) | https://support.discord.com/hc/en-us/articles/31444248479639 |
| Profile Widgets | "Add Widgets": Favorite Game, Currently Playing (5), Want to Play (20), Games I've Played (20), Game Stats (linked game account); desktop/browser only. (2025) | https://support.discord.com/hc/en-us/articles/35344672307607 |
| Legacy Username Badge toggle | Profiles > User Profile > "Legacy Username Badge" toggle hides old name#discriminator badge. | https://support.discord.com/hc/en-us/articles/12620128861463 |
| Profile preview / unsaved-changes guard | Live preview; clicking outside editor warns about unsaved changes (May 2026). | https://discord.com/blog/discord-patch-notes-may-4-2026 |
| Recent activity on profile | Activity tab shows last 30 days; "Clear Game History". | https://support.discord.com/hc/en-us/articles/7931156448919 |
| Wishlist tab | Profile tab listing wished Shop items; "Public Wishlist" toggle. (2025) | https://support.discord.com/hc/en-us/articles/36288192746903 |

### Per-server profiles

| Feature | Detail (states, dialogs, conditions such as permission/Nitro/DM-only) | Source URL |
|---|---|---|
| Per-server Profiles tab | User Settings > Profiles > Per-server Profiles; choose server in selector; also right-click own avatar > "Edit Per-server Profile". Nitro perk. | https://support.discord.com/hc/en-us/articles/4409388345495 |
| Server nickname | Free for all users. | https://support.discord.com/hc/en-us/articles/4409388345495 |
| Server avatar / banner / About Me / theme | Per-server avatar (GIF with Nitro), banner, bio, profile theme; per-server bio falls back to main bio. | https://support.discord.com/hc/en-us/articles/4409388345495 ; https://discord.com/blog/discord-patch-notes-june-4-2026 |
| Per-server pronouns | Clearable per server (May 2026 fix). | https://discord.com/blog/discord-patch-notes-may-4-2026 |
| Per-server decoration / effect | Different Avatar Decoration and Profile Effect per server. | https://support.discord.com/hc/en-us/articles/13410113109911 ; https://support.discord.com/hc/en-us/articles/17828465914263 |
| Per-server nameplate | Nameplates settable per server (Oct/Nov 2025). (2025) | https://discord.com/blog/discord-update-november-6-2025-changelog ; https://discord.com/blog/discord-patch-notes-october-7-2025 |
| Per-server display name style / frame | Display name style editable per server; frames can be shown globally or per server. | https://support.discord.com/hc/en-us/articles/33833879643927 ; https://support.discord.com/hc/en-us/articles/40775065582615 |

### Messaging Permissions (formerly Content & Social / Privacy & Safety)

| Feature | Detail (states, dialogs, conditions such as permission/Nitro/DM-only) | Source URL |
|---|---|---|
| Page name | "Content & Social" renamed "Messaging Permissions" (Aug 2026); older docs call it Privacy & Safety. (2026) | https://discord.com/blog/discord-update-august-11-2026-changelog |
| Sensitive content filters | Two filters: "Mature Sexual Media Filter" and "Graphic Media Filter"; each set per context: "Direct messages from friends", "Direct messages from others", "Messages in server channels"; values Show / Blur / Block (servers: Show/Blur). Adult defaults Show/Block/Show; teen Blur/Block/Blur (locked). | https://support.discord.com/hc/en-us/articles/18210995019671 |
| DM spam filter | "Filter direct messages from non-friends" / "Filter all direct messages" / "Do not filter direct messages"; spam goes to Spam tab. | https://support.discord.com/hc/en-us/articles/115000068672 |
| Message Requests | Global toggle; DMs from non-friends go to Message Requests inbox (unless mutual friend/small server); adults can turn off; teens cannot. Per-server toggle in server Privacy Settings. | https://support.discord.com/hc/en-us/articles/7924992471191 |
| Direct Message (DM) Permissions | "Social permissions > Direct messages": allow DMs from server members; prompt to apply to all existing servers; per-server override. | https://support.discord.com/hc/en-us/articles/217916488 ; https://support.discord.com/hc/en-us/articles/7924992471191 |
| Friend Requests | Who can add you: Everyone / Friends of Friends / Server Members (all off = nobody). | https://support.discord.com/hc/en-us/articles/217916488 |
| Friend request notes visibility | Friend requests can carry a note (Aug 2026); proto has hide_friend_request_notes setting (setting unconfirmed). (2026) | https://discord.com/blog/discord-update-august-11-2026-changelog ; https://docs.discord.food/resources/user-settings-proto |
| Friend request alerts | Warning before accepting unfamiliar requests; cannot be turned off. | https://support.discord.com/hc/en-us/articles/42178056096663 |
| Blocked & ignored accounts list | "Accounts you've blocked & ignored" with Unignore/unblock. Ignore (2025) hides messages silently. | https://support.discord.com/hc/en-us/articles/28084948873623 ; https://discord.com/blog/discord-update-march-25-2025-changelog |
| Age-restricted commands in DMs | "Allow access to age-restricted commands from apps in DMs" (off by default; adult confirmation). | https://support.discord.com/hc/en-us/articles/42178056096663 ; https://support.discord.com/hc/en-us/articles/10123937946007 |
| Age-restricted servers on iOS | "Allow access to age-restricted servers on iOS" (off; iOS-only extra step). | https://support.discord.com/hc/en-us/articles/42178056096663 |
| Language (keyword) filter | "Enable language filter": local redaction of slurs/sexual/severe profanity with asterisks (experiment, default off). | https://support.discord.com/hc/en-us/articles/24269903094167 |
| Safety alerts (teens) | First-DM safety alert with More Tips/Block/Mute/Report/Dismiss; teens can't opt out. | https://support.discord.com/hc/en-us/articles/18210977897239 |
| Encryption / Verified Devices | Privacy & Safety > Encryption tab: Verified Devices list (remove per device or per user), "Enable persistent verification codes" (E2EE for calls mandatory since Mar 2 2026). | https://support.discord.com/hc/en-us/articles/25968222946071 |
| Safety settings preset | Preset Balanced/Strict/Relaxed/Custom exists in settings proto (unconfirmed). | https://docs.discord.food/resources/user-settings-proto |

### Data & Privacy

| Feature | Detail (states, dialogs, conditions such as permission/Nitro/DM-only) | Source URL |
|---|---|---|
| "Use Data to Improve Discord" | Toggle; account-wide. | https://support.discord.com/hc/en-us/articles/21864805694999 |
| "Use data to personalize my Discord experience" | Toggle; affects recommendations/promotions (incl. personalized Highlights, Quest promotions). | https://support.discord.com/hc/en-us/articles/21865322754327 ; https://support.discord.com/hc/en-us/articles/22225719947543 |
| Sponsored content personalization | "Use data based on my Discord activity to personalize sponsored content" and "Use data from third party sources about me to personalize sponsored content" (under "How Discord uses your data"). | https://support.discord.com/hc/en-us/articles/22225542459415 |
| Manage Sponsored Content | Data & Privacy > Sponsored Content > Manage Sponsored Content: pick ad topics to hide. (2026) | https://support.discord.com/hc/en-us/articles/43335916914071 |
| Profile Privacy | "Friends & All Servers" / "Friends & Small Servers Only" (≤200 members) / "Friends Only"; hides custom status, pronouns, badges, connections, bio, widgets, wishlists; prompts to match Activity Sharing. (2026) | https://support.discord.com/hc/en-us/articles/38859942749463 ; https://discord.com/blog/discord-update-august-11-2026-changelog |
| Request your data | "Request Data" → choose categories (desktop) or all (mobile); up to 30 days; emailed link; verified email needed. | https://support.discord.com/hc/en-us/articles/360004027692 |
| Screen-reader detection / other proto toggles | allow_accessibility_detection, show recent games on profile, quests 3P data opt-out exist in proto (unconfirmed as UI toggles). | https://docs.discord.food/resources/user-settings-proto |

### Family Center

| Feature | Detail (states, dialogs, conditions such as permission/Nitro/DM-only) | Source URL |
|---|---|---|
| Family Center tab | Account > Family Center (nested page, 2026); teen side "My Family" with "Show Code" (QR) or "Copy Link"; up to 3 guardians; parent up to 8 teens. Tab can be closed/disabled. | https://support.discord.com/hc/en-us/articles/14155060633623 ; https://support.discord.com/hc/en-us/articles/14155043715735 ; https://discord.com/blog/discord-patch-notes-august-4-2026 |
| Activity dashboard | 7-day: new friends, users messaged/called, call minutes, top users/servers, servers joined, purchases, gifts; weekly email. | https://support.discord.com/hc/en-us/articles/14155043715735 |
| Guardian-managed settings | Sensitive filters (blur/block), friend requests, server DMs, Message Requests, Data & Privacy toggles; teen can't change while connected. (2025) | https://support.discord.com/hc/en-us/articles/14155060633623 ; https://discord.com/blog/discord-update-november-6-2025-changelog |
| Spending cap | Monthly limit (can be $0); subscriptions excluded. | https://support.discord.com/hc/en-us/articles/14155043715735 |
| Screen Time Limits | Scheduled downtime; teen notified 15 min before. | https://support.discord.com/hc/en-us/articles/14155043715735 |
| Notify guardian of report | Optional when reporting. | https://support.discord.com/hc/en-us/articles/14155060633623 |

### Authorized Apps & Connections

| Feature | Detail (states, dialogs, conditions such as permission/Nitro/DM-only) | Source URL |
|---|---|---|
| Authorized Apps | List of OAuth-authorized and user-installed apps with "Deauthorize"; consolidated with Connections (Aug 2026). | https://support.discord.com/hc/en-us/articles/24160905919511 ; https://support.discord.com/hc/en-us/articles/21334461140375 ; https://discord.com/blog/discord-update-august-11-2026-changelog |
| Connections tab | Add account tiles; per-connection "Display on profile", "Display details on profile", "Refresh"; Spotify adds "Display Spotify as your status"; activity toggle per connection. | https://support.discord.com/hc/en-us/articles/8063233404823 ; https://support.discord.com/hc/en-us/articles/360000167212 ; https://support.discord.com/hc/en-us/articles/7931156448919 |
| Connection types | Amazon Music, Bungie.net, Bluesky, Crunchyroll, Domain, eBay, Epic Games, Facebook, GitHub, Mastodon, PayPal, PlayStation Network, Reddit, Roblox, Spotify, Steam, TikTok, Twitch, X (Twitter), Xbox, YouTube (Instagram, Skype no longer addable). Riot Games (League/VALORANT) linking (2026) and Battle.net linking also referenced. | https://docs.discord.com/developers/resources/user ; https://discord.com/blog/discord-update-june-25-2026-changelog ; https://support.discord.com/hc/en-us/articles/30593690165783 |
| Linked roles | Connection-gated server roles; Linked Role Tag next to name; opt-in desktop only. | https://support.discord.com/hc/en-us/articles/8063233404823 |
| App connections (third-party) | Apps can provide connection metadata (e.g., Bloxlink, Jefit). | https://support.discord.com/hc/en-us/articles/8063233404823 |

### Clips

| Feature | Detail (states, dialogs, conditions such as permission/Nitro/DM-only) | Source URL |
|---|---|---|
| Enable Clips | User Settings > Clips; Windows-only (Early Access), GPU requirements. | https://support.discord.com/hc/en-us/articles/16861982215703 |
| Clipping Keybind | Default Alt+C. | https://support.discord.com/hc/en-us/articles/16861982215703 |
| Clip Length | 30 s / 60 s / 2 min. | https://support.discord.com/hc/en-us/articles/16861982215703 |
| Hardware Encoding / Storage Location | Optional encoding; choose save folder. | https://support.discord.com/hc/en-us/articles/16861982215703 |
| Allow voice recording in clips | Proto allow_voice_recording (unconfirmed as UI label). | https://docs.discord.food/resources/user-settings-proto |

### Nitro / Server Boost / Subscriptions / Gift Inventory / Billing

| Feature | Detail (states, dialogs, conditions such as permission/Nitro/DM-only) | Source URL |
|---|---|---|
| Nitro page | User Settings > Nitro ("Your Nitro Home"): plans, perks, Share Nitro, gifting, monthly Orbs countdown, tenure badge. | https://support.discord.com/hc/en-us/articles/22567595858327 ; https://support.discord.com/hc/en-us/articles/37097755077015 ; https://discord.com/blog/discord-patch-notes-july-7-2026 |
| Server Boost tab | Lists boosts; "..." > Transfer Boost (7-day cooldown). | https://support.discord.com/hc/en-us/articles/360028038352 |
| Subscriptions | Active subscriptions (Nitro, Boosts, server subs, premium apps), renewal, "Pay for it With", switch plan/cancel/Re-Subscribe, gift credit, Nitro Credits section, boosts pending cancellation. | https://support.discord.com/hc/en-us/articles/360017693772 ; https://support.discord.com/hc/en-us/articles/26362805097623 ; https://discord.com/blog/discord-patch-notes-july-7-2026 |
| Gift Inventory | Unclaimed gifts (revoke/regenerate link, resend), redeem 19-character codes. | https://support.discord.com/hc/en-us/articles/360020877112 ; https://support.discord.com/hc/en-us/articles/360020564851 |
| Billing | Payment methods ("Add Payment Method", Edit, default, delete), Transaction History with spinner/empty state, Purchase Details, invoice/receipt PDF, billing address. Google Pay/Apple Pay, Venmo exist. | https://support.discord.com/hc/en-us/articles/360017693772 ; https://support.discord.com/hc/en-us/articles/31937335282967 ; https://discord.com/blog/discord-patch-notes-june-4-2026 |

### Appearance

| Feature | Detail (states, dialogs, conditions such as permission/Nitro/DM-only) | Source URL |
|---|---|---|
| Base themes | Light, Ash, Dark, Onyx (desktop since Mar 2025; all four on mobile Jul 2026, Onyx formerly "Midnight"). (2025) | https://support.discord.com/hc/en-us/articles/207260127 ; https://discord.com/blog/discord-update-march-25-2025-changelog ; https://support.discord.com/hc/en-us/articles/42383370736023 |
| Sync with computer / device | Desktop "Sync with computer"; mobile "Same as Device Theme" with chosen Light and Dark themes (2026). | https://support.discord.com/hc/en-us/articles/207260127 ; https://support.discord.com/hc/en-us/articles/42383370736023 |
| Sync across clients | "Sync Themes Across My Devices" / "Sync across clients" applies theme to mobile/browser. | https://support.discord.com/hc/en-us/articles/207260127 ; https://discord.com/blog/discord-patch-notes-may-4-2026 |
| Dark Sidebar | Optional with Light theme. | https://support.discord.com/hc/en-us/articles/207260127 |
| Nitro color themes | Preset gradient themes; non-Nitro can "Preview Theme". | https://support.discord.com/hc/en-us/articles/207260127 |
| Custom themes (Nitro) | Dark/Light palette, up to 5 colors, "Gradient Direction", "Color Intensity", "Apply"/"Surprise Me"/"Reset", "Share" theme to users/channels. (2025) | https://support.discord.com/hc/en-us/articles/207260127 ; https://discord.com/blog/discord-update-september-25-2025-changelog |
| Server theme preference | Choose server theme vs own theme (experiment, 3 boosts). | https://support.discord.com/hc/en-us/articles/207260127 |
| UI Density | Compact / Default / Spacious. (2025) | https://support.discord.com/hc/en-us/articles/207260127 |
| Message Spacing (display) | Default (cozy) or Compact (removes avatars). | https://support.discord.com/hc/en-us/articles/207260127 |
| Chat Font Scaling | Slider; text-only scaling. | https://support.discord.com/hc/en-us/articles/207260127 |
| Zoom Level | Slider; scales all UI ("Reset to Default"). | https://support.discord.com/hc/en-us/articles/207260127 ; https://discord.com/blog/discord-patch-notes-october-7-2025 |
| Message Group Spacing | Slider. | https://support.discord.com/hc/en-us/articles/207260127 |
| App Icon | Nitro/Nitro Basic custom app icon (desktop changes in-app icon only). | https://support.discord.com/hc/en-us/articles/17503907209239 |
| Clock format | 12/24-hour timestamp hour cycle (unconfirmed; proto timestamp_hour_cycle). | https://docs.discord.food/resources/user-settings-proto |
| Show Members in Channel List | Switch referenced in Feb 2026 notes. | https://discord.com/blog/discord-patch-notes-february-4-2026 |

### Accessibility

| Feature | Detail (states, dialogs, conditions such as permission/Nitro/DM-only) | Source URL |
|---|---|---|
| Saturation slider | Color richness. | https://support.discord.com/hc/en-us/articles/1500010454681 |
| High Contrast Mode | Desktop, any theme. | https://support.discord.com/hc/en-us/articles/1500010454681 |
| Sync contrast settings | Windows High Contrast only. | https://support.discord.com/hc/en-us/articles/1500010454681 |
| Chat background contrast | Setting respected (Jul 2026). | https://discord.com/blog/discord-patch-notes-july-7-2026 |
| Role Colors | "Show role colors in names" / "Show role colors next to names" / "Don't show role colors". | https://support.discord.com/hc/en-us/articles/1500010454681 |
| Always Underline Links | Link decoration toggle. | https://discord.com/blog/discord-patch-notes-october-6-2026 |
| Sync profile themes | Use client theme for profile cards. | https://support.discord.com/hc/en-us/articles/1500010454681 |
| Display Name Styles toggle | Hides name styles for yourself. | https://support.discord.com/hc/en-us/articles/33833879643927 |
| Reduced Motion | "Sync with computer" (default) or "Enable Reduced Motion"; stops animated emoji, GIF avatars, effects. | https://support.discord.com/hc/en-us/articles/360040613412 |
| GIF autoplay | "Automatically playing GIFs when Discord is focused" (applies to avatars too). | https://support.discord.com/hc/en-us/articles/1500010454681 ; https://discord.com/blog/discord-patch-notes-may-4-2026 |
| Play animated emoji | Toggle. | https://support.discord.com/hc/en-us/articles/1500010454681 |
| Sticker animation | Always animate / Animate on interaction / Never animate. | https://support.discord.com/hc/en-us/articles/1500010454681 |
| Text-to-Speech rate | Slider with Preview; "Allow playback and usage of /tts command". | https://support.discord.com/hc/en-us/articles/1500010454681 ; https://support.discord.com/hc/en-us/articles/208613137 |
| Keyboard Mode | Tab starts keyboard mode with focus ring; F6 section jumping; message shortcuts (q, +, r, e, p, Alt+Enter). | https://support.discord.com/hc/en-us/articles/1500000056121 |
| Screen Readers section | Section within Chat settings (Apr 2026). | https://discord.com/blog/discord-patch-notes-april-6-2026 |

### Voice & Video

| Feature | Detail (states, dialogs, conditions such as permission/Nitro/DM-only) | Source URL |
|---|---|---|
| Voice & Video page | Exists (covered by another researcher); includes Soundboard volume and Entrance Sounds. | https://discord.com/blog/discord-update-march-24-2026-changelog ; https://support.discord.com/hc/en-us/articles/12612888127767 |

### Chat (Text & Images)

| Feature | Detail (states, dialogs, conditions such as permission/Nitro/DM-only) | Source URL |
|---|---|---|
| Embeds and link previews | "When posted as links to chat", "When upload directly to Discord" (media display); <url> suppresses embed. | https://support.discord.com/hc/en-us/articles/206342858 |
| Show spoiler content | On click / On servers I moderate / Always. | https://support.discord.com/hc/en-us/articles/360022320632 |
| Mention suggestions | "Show suggestions for user mentions as you type" (also right-click > Suggestions). | https://support.discord.com/hc/en-us/articles/35692242798743 |
| Show Favorites Server | Toggle. | https://support.discord.com/hc/en-us/articles/38810584460439 |
| Default reaction emoji (Tap to React) | Chat > preset emoji for double-tap; "Sync Across Clients". (2026) | https://discord.com/blog/discord-update-june-25-2026-changelog ; https://discord.com/blog/discord-patch-notes-june-4-2026 |
| Super Reactions toggle | Toggle (May 2026). | https://discord.com/blog/discord-patch-notes-may-4-2026 |
| Spellcheck | Right-click chat box > Spellcheck. | https://support.discord.com/hc/en-us/articles/360000877191 |
| Other proto toggles | Show image descriptions, show emoji reactions, convert emoticons, expression/sticker suggestions, show command suggestions, legacy chat input, search provider (unconfirmed). | https://docs.discord.food/resources/user-settings-proto |

### Notifications (settings page)

| Feature | Detail (states, dialogs, conditions such as permission/Nitro/DM-only) | Source URL |
|---|---|---|
| Notifications page | Desktop toggles, sounds, TTS, email; redesigned Mar 2026. Full option list in Notifications area below. | https://discord.com/blog/discord-update-march-24-2026-changelog |

### Keybinds

| Feature | Detail (states, dialogs, conditions such as permission/Nitro/DM-only) | Source URL |
|---|---|---|
| Add a Keybind | Actions: Push to Talk, Push To Mute, Toggle Mute, Toggle Deafen, Toggle VAD, Toggle Streamer Mode, Toggle Overlay, Toggle Overlay Lock; desktop app only. | https://support.discord.com/hc/en-us/articles/217083547 |
| Remove a keybind | Per-row remove control (removal not described in official article; unconfirmed). | https://support.discord.com/hc/en-us/articles/217083547 |
| Built-in shortcuts list | Read-only reference in Keybinds; Ctrl/Cmd+/ opens shortcut list, Ctrl/Cmd+, opens User Settings, Ctrl/Cmd+Alt+@ mentions popout, Ctrl/Cmd+I inbox; desktop/browser only. Keybind descriptions translated (Nov 2025). | https://support.discord.com/hc/en-us/articles/31232432266647 ; https://support.discord.com/hc/en-us/articles/360045027712 ; https://discord.com/blog/discord-patch-notes-november-4-2025 |
| OS-specific keybinds consolidation | OS-specific keybinds merged with related pages (Aug 2026). (2026) | https://discord.com/blog/discord-update-august-11-2026-changelog |

### Language & Time

| Feature | Detail (states, dialogs, conditions such as permission/Nitro/DM-only) | Source URL |
|---|---|---|
| Language | Select UI language. | https://support.discord.com/hc/en-us/articles/216406447 |
| @time command | Timezone-adaptive timestamps (Feb 2026). | https://discord.com/blog/discord-patch-notes-february-4-2026 |

### Windows / macOS settings

| Feature | Detail (states, dialogs, conditions such as permission/Nitro/DM-only) | Source URL |
|---|---|---|
| Windows Settings page | Desktop OS-specific page ("User Settings > Windows Settings"); OS-specific pages consolidated in Aug 2026 redesign. macOS equivalent page name not documented. | https://support.discord.com/hc/en-us/articles/211339918 ; https://discord.com/blog/discord-update-august-11-2026-changelog |
| Open Discord (on startup) | "Open Discord" toggle launches at login. | https://support.discord.com/hc/en-us/articles/211339918 |
| Start Minimized | Launch minimized to tray when Open Discord is on (unconfirmed). | https://support.discord.com/hc/en-us/community/posts/19505920990231-incorrect-minimized-startup-behavior |
| Game library shortcuts | Desktop/Start-menu shortcut toggles for games exist in settings proto (unconfirmed). | https://docs.discord.food/resources/user-settings-proto |

### Streamer Mode

| Feature | Detail (states, dialogs, conditions such as permission/Nitro/DM-only) | Source URL |
|---|---|---|
| Enable Streamer Mode | Master toggle; banner above app with "x" (hide banner) and "Disable". Page redesigned Mar 2026 and folded into the consolidated app settings May 2026. | https://support.discord.com/hc/en-us/articles/218485407 ; https://discord.com/blog/discord-update-march-24-2026-changelog ; https://discord.com/blog/discord-patch-notes-may-4-2026 |
| Automatically Enable/Disable | Turns on while OBS or XSplit is running/live. | https://support.discord.com/hc/en-us/articles/218485407 |
| Hide Personal Information | Hides email, connections (incl. on profiles, 2026 fix), discriminator/username (display name stays). | https://support.discord.com/hc/en-us/articles/218485407 ; https://discord.com/blog/discord-patch-notes-june-4-2026 |
| Hide Instant Invite Links | Masks invite codes, e.g., in Server Settings > Invites. | https://support.discord.com/hc/en-us/articles/218485407 |
| Disable Sounds | Stops channel join and notification sounds. | https://support.discord.com/hc/en-us/articles/218485407 |
| Disable Notifications | Blocks desktop notifications. | https://support.discord.com/hc/en-us/articles/218485407 |
| Toggle Streamer Mode keybind | Assignable keybind action. | https://support.discord.com/hc/en-us/articles/217083547 |

### Advanced / Developer

| Feature | Detail (states, dialogs, conditions such as permission/Nitro/DM-only) | Source URL |
|---|---|---|
| Developer page | "Advanced" folded into a "Developer" page (May 2026). (2026) | https://discord.com/blog/discord-patch-notes-may-4-2026 |
| Developer Mode | Adds "Copy User ID", "Copy Server ID", "Copy Channel ID", "Copy Message Link"/"Copy Message ID" to context menus. | https://support.discord.com/hc/en-us/articles/206346498 |
| Hardware Acceleration | Toggle (Advanced; also under Voice & Video > Video in some docs); with tooltip. | https://support.discord.com/hc/en-us/articles/213491697 ; https://support.discord.com/hc/en-us/articles/33030151293079 ; https://discord.com/blog/discord-patch-notes-march-6-2026 |
| Client Info | Client build info sheet in settings (iOS fix Aug 2026). | https://discord.com/blog/discord-patch-notes-august-4-2026 |

### Games & Apps (formerly Activity) and Activity Privacy

| Feature | Detail (states, dialogs, conditions such as permission/Nitro/DM-only) | Source URL |
|---|---|---|
| Category name | "Activity" renamed "Games & Apps" (Aug 2026). (2026) | https://discord.com/blog/discord-update-august-11-2026-changelog |
| "Share my activity" | Global switch (Activity Privacy > Where I Share Activity). | https://support.discord.com/hc/en-us/articles/7931156448919 |
| "Automatically share my activity with" | Friends & All Servers / Friends & Small Servers Only (≤200 members) / Friends Only; teens default to small servers. | https://support.discord.com/hc/en-us/articles/7931156448919 |
| "My Servers" per-server overrides | Per-server activity toggles; also server dropdown > Privacy Settings > Activity Status. | https://support.discord.com/hc/en-us/articles/7931156448919 |
| Join permissions | "Allow friends to join my game" and "Allow voice channel participants to join my game" (non-Community servers). | https://support.discord.com/hc/en-us/articles/7931156448919 |
| Registered Games | List detected games; enable/disable detection per game ("Toggle Detection"); add game manually (manually added early-access Steam titles now show, Jul 2026). | https://support.discord.com/hc/en-us/articles/7931156448919 ; https://discord.com/blog/discord-patch-notes-december-8-2025 ; https://discord.com/blog/discord-patch-notes-july-7-2026 |
| Per-app activity sharing | Disable activity sharing for a specific app (proto disable_application_activity_sharing; unconfirmed as UI). | https://docs.discord.food/resources/user-settings-proto |

### Game Overlay

| Feature | Detail (states, dialogs, conditions such as permission/Nitro/DM-only) | Source URL |
|---|---|---|
| Enable Overlay / Enable Legacy Overlay | Windows 10/11 only; new widget overlay (2025) plus legacy overlay toggle. | https://support.discord.com/hc/en-us/articles/217659737 ; https://discord.com/blog/discord-patch-notes-june-4-2026 ; https://discord.com/blog/discord-update-march-25-2025-changelog |
| Overlay keybind | Default Shift+`; Toggle Overlay Lock keybind. | https://support.discord.com/hc/en-us/articles/217659737 ; https://support.discord.com/hc/en-us/articles/217083547 |
| Overlay widgets | Movable widgets: Notifications, Voice, Video, Activity, Streams; Soundboard (Ctrl+`). | https://support.discord.com/hc/en-us/articles/217659737 ; https://discord.com/blog/discord-update-march-25-2025-changelog |
| Overlay text notifications | "Messages" toggle shows DM/group/server messages in game. | https://support.discord.com/hc/en-us/articles/217659737 |
| Avatar size / show users / show names | Large/Small; Always/Only While Speaking; Always/Only While Speaking/Never. | https://support.discord.com/hc/en-us/articles/217659737 |

### More: What's New, Merch, HypeSquad, Log Out

| Feature | Detail (states, dialogs, conditions such as permission/Nitro/DM-only) | Source URL |
|---|---|---|
| What's New | Settings > More > What's New (in-app changelog). | https://discord.com/blog/discord-update-september-25-2026 ; https://discord.com/blog/discord-update-november-6-2025-changelog |
| HypeSquad | HypeSquad House quiz/tab is gone; house badges are legacy and can't be removed. | https://support.discord.com/hc/en-us/articles/360035962891 ; https://support.discord.com/hc/en-us/articles/360007553672 |
| Log Out | Red "Log Out" at end of settings list. | https://support.discord.com/hc/en-us/articles/209572128 |

## Server settings & moderation

### Settings shell, Overview and identity

| Feature | Detail (states, dialogs, conditions such as permission/Nitro/DM-only) | Source URL |
|---|---|---|
| Server Settings modal | Opened from the server-name dropdown or right-click. Sections are grouped (server identity, expressions, people/User Management, apps, moderation/Safety Setup, community, monetization). Tabs show only when you hold the permission. Mobile taps no longer stack duplicate pages (2026). | https://support.discord.com/hc/en-us/articles/213595197 ; https://discord.com/blog/discord-patch-notes-october-6-2026 |
| Unsaved-changes bar | "Save Changes" banner appears after edits and clears when you revert. Leaving AutoMod settings with unsaved changes asks for confirmation (2026). | https://discord.com/blog/discord-patch-notes-june-4-2026 ; https://discord.com/blog/discord-patch-notes-august-4-2026 |
| Overview: name and icon | Change the server name and icon. Manage Server. | https://support.discord.com/hc/en-us/articles/204849977 |
| Overview: System Messages Channel | Pick a channel; selectors show channel plus category (2026). | https://support.discord.com/hc/en-us/articles/213530048 ; https://discord.com/blog/discord-patch-notes-september-8-2026 |
| System message toggles | Toggles to suppress join messages, the sticker-reply buttons on join messages, boost messages, setup tips, and role-subscription purchase/renewal messages and their sticker replies. UI labels and the random welcome text are unconfirmed. | https://docs.discord.com/developers/resources/guild ; https://support.discord.com/hc/en-us/articles/360028038352 |
| Overview: Inactive (AFK) Channel and timeout | AFK channel and idle timeout selector. | https://discord.com/blog/discord-patch-notes-september-8-2026 |
| Overview: default notification settings | All Messages or Only @mentions as the members' default. | https://docs.discord.com/developers/resources/guild |
| Overview: Show Boost progress bar | Toggle (premium_progress_bar_enabled). | https://docs.discord.com/developers/resources/guild |
| Overview: server banner / invite background | Banner upload at Level 2 (animated at 3); invite splash at Level 1. | https://support.discord.com/hc/en-us/articles/360028038352 |
| Overview: Conversation Summaries toggle | Server-wide hide for the Summaries experiment (Manage Channel). | https://support.discord.com/hc/en-us/articles/12926016807575 |
| Server Profile tab | (2025) Name; icon; banner color tile (custom banner for discoverable servers); Traits (with emoji); Description; Games (search, suggested); Private Profile toggle, on by default; Discovery, widget or announcement channels make the profile public. Desktop/browser editing. | https://support.discord.com/hc/en-us/articles/30715364399511 |
| Server Tag tab | (2025) Tag name up to 4 alphanumeric/special chars (no spaces or emoji), badge picker, color picker, rate-limited changes; changing the tag makes members re-apply it. Needs the 3-boost perk. | https://support.discord.com/hc/en-us/articles/31444248479639 |
| Server Tag Badge Packs | Creepy Crawlies (2 boosts), Pet (3), Plant (3), Flex (5); Server Boosts tab > View Packs (Pet and Flex added 2025). | https://support.discord.com/hc/en-us/articles/31444248479639 ; https://discord.com/blog/discord-update-september-25-2025-changelog |
| Server Theme | Custom color theme for the server (3 boosts; experiment). | https://support.discord.com/hc/en-us/articles/360028038352 |
| Widget | Enable the server widget, pick the invite channel; widget.json and widget.png styles shield, banner1–4. Enabling it makes the Server Profile public. | https://docs.discord.com/developers/resources/guild ; https://support.discord.com/hc/en-us/articles/30715364399511 |
| Server Template tab | Name and description > Generate Template > Copy link; Sync Template (a "changes" notice appears by the server name); Delete Template. Excludes Community-only channel types. Fails above 1,000 overwrites. Manage Server; desktop/browser. | https://support.discord.com/hc/en-us/articles/360041033511 |
| Custom Invite Link (vanity URL) | Boost Level 3: up to 25 chars of lowercase letters, numbers and dashes. Needs a text channel @everyone can view. Remove option. Lost 30 days after losing Level 3. | https://support.discord.com/hc/en-us/articles/115001542132 ; https://support.discord.com/hc/en-us/articles/360028038352 |
| Server Web Page (beta) | Community > Server Web Pages: page content, social links (Twitch and others), Publish Page, View Page, Disable Web Page. For discoverable servers; a pilot adds announcement posts. | https://support.discord.com/hc/en-us/articles/4673515000983 ; https://discord.com/blog/discord-patch-notes-september-8-2026 |
| Delete Server | Owner only. Server Settings > Delete Server; type the server name, or enter a 2FA code when 2FA is on. Blocked if the server requires 2FA and the owner lacks it. Permanent. | https://support.discord.com/hc/en-us/articles/213595197 |

### Roles

| Feature | Detail (states, dialogs, conditions such as permission/Nitro/DM-only) | Source URL |
|---|---|---|
| Roles list | Ordered top-down; @everyone at the bottom acts as the template for new roles. Drag to reorder, with auto-scroll on long lists (2026). Large member counts are abbreviated (e.g. 148K). Mobile sorting is supported. | https://support.discord.com/hc/en-us/articles/214836687 ; https://discord.com/blog/discord-patch-notes-september-8-2026 ; https://discord.com/blog/discord-patch-notes-march-6-2026 |
| Create Role | "Create Role" button; the new role sits just above @everyone. Empty or whitespace names are rejected (2026). Max 250 roles; name up to 100 chars. | https://support.discord.com/hc/en-us/articles/214836687 ; https://discord.com/blog/discord-patch-notes-june-4-2026 ; https://support.discord.com/hc/en-us/articles/33694251638295 |
| Role hierarchy rules | Manage Roles holders can edit only roles below their highest role and grant only permissions they hold. They can't kick, ban or rename members with equal or higher roles. | https://support.discord.com/hc/en-us/articles/214836687 |
| Display tab: role name | Rename; the member list updates live (2026). | https://discord.com/blog/discord-patch-notes-february-4-2026 |
| Display tab: role color | Preset swatches or a custom picker. The highest colored role sets the name color. Reset clears the color (mobile fix 2026). | https://support.discord.com/hc/en-us/articles/214836687 ; https://discord.com/blog/discord-patch-notes-september-8-2026 |
| Display tab: role style (Solid / Gradient / Holographic) | (2025) Enhanced Role Styles perk (3 boosts): gradient uses 2 colors or presets; holographic is a preset. Animated; respects Reduced Motion; falls back to the primary color if boosts lapse. Data: role `colors` {primary, secondary, tertiary}. Desktop editing only. | https://support.discord.com/hc/en-us/articles/31444213087255 ; https://docs.discord.com/developers/resources/guild |
| Display tab: role icon | Custom image or Unicode emoji (Boost Level 2, ROLE_ICONS). Shown next to names in chat; tapping it shows the role name. | https://support.discord.com/hc/en-us/articles/360028038352 ; https://docs.discord.com/developers/resources/guild ; https://discord.com/blog/discord-patch-notes-august-4-2026 |
| Display tab: "Display role members separately" | Hoists the role as its own group in the member list. | https://support.discord.com/hc/en-us/articles/214836687 ; https://docs.discord.com/developers/resources/guild |
| Display tab: "Allow anyone to @mention this role" | Mentionable toggle; role mention suppression applies otherwise. | https://support.discord.com/hc/en-us/articles/360039176412 |
| Permissions tab | Toggles grouped as General Server, Membership, Text Channel, Voice Channel, Apps, Stage, Events and Advanced (Administrator). Search and "Clear permissions" options (unconfirmed for those two). | https://support.discord.com/hc/en-us/articles/4403089981975 ; https://support.discord.com/hc/en-us/articles/360016150952 ; https://discord.com/blog/discord-patch-notes-may-4-2026 |
| Links tab (Linked Roles) | Connection requirements (account age and other checks; multiple requirements combine with AND). App-provided connections. Admin/Manage Roles; not on mobile. | https://support.discord.com/hc/en-us/articles/10388356626711 |
| Manage Members tab | Add members to a role, and "Remove Member" per row. | https://discord.com/blog/discord-patch-notes-april-6-2026 |
| Role ellipsis menu | Includes View Server As Role; Delete Role (unconfirmed label). | https://support.discord.com/hc/en-us/articles/360055709773 |
| View Server As Role | Owner/Administrator only. Top control bar with Select Roles (multi-select, search), Disable, and Back to Roles Settings. Previews visible channels and settings; message history follows read permission (2026). | https://support.discord.com/hc/en-us/articles/360055709773 ; https://discord.com/blog/discord-patch-notes-october-6-2026 |
| Role @mention member list | Clicking a role mention lists up to 100 members with that role (desktop 2026, 100% stable Feb 2026). | https://discord.com/blog/discord-update-march-24-2026-changelog ; https://discord.com/blog/discord-patch-notes-february-4-2026 |
| Role member counts | Per-role member counts in role settings. | https://docs.discord.com/developers/resources/guild ; https://discord.com/blog/discord-patch-notes-september-8-2026 |
| Linked Role tag | Members with a Linked Role show a tag in chat; hover lists the passed checks. | https://support.discord.com/hc/en-us/articles/10388356626711 |
| Server Booster role | Automatic, can't be deleted or assigned manually; default pink gradient style. | https://support.discord.com/hc/en-us/articles/360028038352 |
| Managed roles (bots, integrations, subscriptions) | Can't be assigned manually; can be used in Onboarding (2026). Non-removable roles show no Remove button. | https://discord.com/blog/discord-patch-notes-may-4-2026 ; https://discord.com/blog/discord-patch-notes-march-6-2026 |
| Edit roles of members who blocked you | Admins with Manage Roles can view and edit them via the Members tab (2026). | https://discord.com/blog/discord-patch-notes-may-4-2026 |
| Channel permission overwrites | In Edit Channel > Permissions: add members or roles; per-permission Deny (X) / Pass-through (/) / Allow (check); Private Channel toggle; sync with category; Advanced view. | https://support.discord.com/hc/en-us/articles/10543994968087 ; https://support.discord.com/hc/en-us/articles/214836687 ; https://discord.com/blog/discord-patch-notes-september-8-2026 |

### Expressions

| Feature | Detail (states, dialogs, conditions such as permission/Nitro/DM-only) | Source URL |
|---|---|---|
| Emoji tab | "Upload Emoji": JPEG, PNG, GIF (animated) or WEBP, under 256 KB; name 2+ chars of letters, digits and underscores. Lists static and animated emoji separately, each with its uploader. Edit and Delete need permission. | https://support.discord.com/hc/en-us/articles/360036479811 ; https://discord.com/blog/discord-patch-notes-may-4-2026 |
| Emoji slots | 50 / 100 / 150 / 250 for Base / Level 1 / 2 / 3. | https://support.discord.com/hc/en-us/articles/33694251638295 |
| Emoji Studio | (2025) Crop and resize on upload, Flip, choose the target server. Opened from "Add Emoji" in the Emoji Picker. Desktop/browser. | https://discord.com/blog/discord-update-november-6-2025-changelog ; https://discord.com/blog/discord-patch-notes-may-4-2026 |
| Stickers tab | "Upload Sticker" one at a time; PNG or APNG; name; required related emoji; optional description (alt text). Edit (pencil; the image can't be replaced) and delete (X). Desktop only. | https://support.discord.com/hc/en-us/articles/4403089981975 |
| Sticker slots | 5 / 15 / 30 / 60. Over-limit stickers become unusable after a level drop. | https://support.discord.com/hc/en-us/articles/4403089981975 |
| Soundboard tab | View, edit and remove sounds (X). Upload via "Add Sound" in the soundboard panel (desktop/browser). Slots 8 / 24 / 36 / 48. | https://support.discord.com/hc/en-us/articles/12612888127767 |
| Expression permissions | Create Expressions covers adding and editing your own; Manage Expressions covers editing or deleting anyone's. | https://docs.discord.com/developers/topics/permissions ; https://support.discord.com/hc/en-us/articles/4403089981975 |

### Integrations and apps

| Feature | Detail (states, dialogs, conditions such as permission/Nitro/DM-only) | Source URL |
|---|---|---|
| Integrations page | Single page for webhooks, followed channels, Twitch/YouTube and bots/apps. Up to 50 integrations. | https://support.discord.com/hc/en-us/articles/360045093012 |
| Webhooks | List shows who added each webhook, when and to which channel. New Webhook (list scrolls to it, 2026); edit name, avatar and channel; Copy Webhook URL (with confirmation); delete. Manage Webhooks. Also under Edit Channel > Integrations. | https://support.discord.com/hc/en-us/articles/360045093012 ; https://discord.com/blog/discord-patch-notes-july-7-2026 ; https://discord.com/blog/discord-patch-notes-march-6-2026 |
| Channels Followed | One webhook per follow: edit name, avatar and destination channel; delete to unfollow. Up to 10 followed announcement channels per channel. | https://support.discord.com/hc/en-us/articles/360028384531 ; https://support.discord.com/hc/en-us/articles/33694251638295 |
| Twitch / YouTube integrations | Subscriber-role sync integrations, showing who added them. | https://support.discord.com/hc/en-us/articles/360045093012 |
| Bots and Apps | List with added-by and added-when. Open an app to see the permissions it requested and its webhooks. Remove an app by kicking its bot. | https://support.discord.com/hc/en-us/articles/360045093012 ; https://support.discord.com/hc/en-us/articles/21334461140375 |
| App command permissions | Per-app and per-command overrides by role, channel and member, with lockout protection (details unconfirmed beyond the article title). | https://support.discord.com/hc/en-us/articles/10952702911639 |
| App Directory | Server Settings > App Directory; Add App > choose server or user install > Authorize. Needs Manage Server. | https://support.discord.com/hc/en-us/articles/21334461140375 |
| Age-restricted commands | Apps' NSFW commands are gated to age-restricted contexts. | https://support.discord.com/hc/en-us/articles/10123937946007 |

### Safety Setup

| Feature | Detail (states, dialogs, conditions such as permission/Nitro/DM-only) | Source URL |
|---|---|---|
| Safety Setup tab | Groups Raid Protection and CAPTCHA, DM and Spam Protection, AutoMod, and Permissions/2FA. The Bans tab now sits under "Safety Setup" (2026). | https://support.discord.com/hc/en-us/articles/1500000466882 ; https://support.discord.com/hc/en-us/articles/17439993574167 ; https://discord.com/blog/discord-patch-notes-october-6-2026 |
| Verification Level | None; Low (verified email); Medium (account older than 5 minutes); High (server member for over 10 minutes); Highest (verified phone). Members with roles are exempt. | https://docs.discord.com/developers/resources/guild ; https://support.discord.com/hc/en-us/articles/10989121220631 |
| Manually verify member | Bypasses verification and rules gates (BYPASSES_VERIFICATION member flag). | https://support.discord.com/hc/en-us/articles/1500000466882 ; https://docs.discord.com/developers/resources/guild |
| Explicit image filter | Don't scan / scan members without roles / scan all members. High is the default in Community; required to enable Community. | https://docs.discord.com/developers/resources/guild ; https://support.discord.com/hc/en-us/articles/10989121220631 |
| Require 2FA for moderation | MFA level Elevated; owner-only setting. Gated permissions are marked * in the permissions list. | https://docs.discord.com/developers/resources/guild ; https://docs.discord.com/developers/topics/permissions |
| Raid Protection | ML join-raid detection; alerts to a chosen channel (safety alerts channel); RAID_ALERTS_DISABLED toggle. | https://support.discord.com/hc/en-us/articles/10989121220631 ; https://docs.discord.com/developers/resources/guild |
| CAPTCHA for suspicious joins | New joiners must pass CAPTCHA for an hour after a suspected raid; can be turned off. | https://support.discord.com/hc/en-us/articles/10989121220631 ; https://support.discord.com/hc/en-us/articles/11074987197975 |
| DM spam / raid detection | Incident data records dm_spam_detected_at and raid_detected_at. | https://docs.discord.com/developers/resources/guild |
| Activity Alerts | Top-of-app banner when online, push when offline, plus an AutoMod post in the alert channel. Banner opens Security Actions or "Review the Alert" (Members page). Mark as Resolved with a note. Personal toggle: User Settings > Notifications > Community Activity Alerts. | https://support.discord.com/hc/en-us/articles/17439993574167 |
| Alert-mode system messages | GUILD_INCIDENT_ALERT_MODE_ENABLED/DISABLED, GUILD_INCIDENT_REPORT_RAID and GUILD_INCIDENT_REPORT_FALSE_ALARM messages in the safety channel. | https://docs.discord.com/developers/resources/message |
| Security Actions dialog | Pause Invites and Pause DMs (between non-friend members), each with a duration (up to 24h ahead), Save. A banner shows the active actions and their end times. Open it from the server dropdown, the Members tab or an alert. | https://support.discord.com/hc/en-us/articles/17439993574167 ; https://docs.discord.com/developers/resources/guild |
| Pause Invites (Invites tab) | Indefinite pause and "Enable Invites". Invite links show a paused notice. Also blocks Discovery joins. Manage Server. | https://support.discord.com/hc/en-us/articles/8458903738647 |
| Rules Screening | Safety Setup > DM and Spam Protection: "Members must accept rules before they can talk or DM"; up to 16 rules (examples or custom). Community only. | https://support.discord.com/hc/en-us/articles/1500000466882 |
| Sensitive content filters (user side) | Mature sexual and graphic media filters (show, blur or block per context). In servers adults can only show or blur; blocking follows the server's settings. | https://support.discord.com/hc/en-us/articles/18210995019671 |
| Age-restricted server | Guild NSFW level: Default, Explicit, Safe, Age Restricted. | https://docs.discord.com/developers/resources/guild ; https://support.discord.com/hc/en-us/articles/1500005389362 |

### AutoMod

| Feature | Detail (states, dialogs, conditions such as permission/Nitro/DM-only) | Source URL |
|---|---|---|
| AutoMod tab | Server Settings > AutoMod. Manage Server or Administrator; editing on desktop only. | https://support.discord.com/hc/en-us/articles/4421269296535 |
| Commonly Flagged Words rule | Preset lists: Insults and Slurs, Sexual Content, Severe Profanity (English). Allow-list exceptions up to 1,000. One rule. | https://support.discord.com/hc/en-us/articles/4421269296535 ; https://docs.discord.com/developers/resources/auto-moderation |
| Custom Keyword rules | Up to 6 rules, each with up to 1,000 keywords of 60 chars. `*` wildcards for prefix, suffix or anywhere; multi-word phrases; case-insensitive. Allow list up to 100. | https://docs.discord.com/developers/resources/auto-moderation ; https://support.discord.com/hc/en-us/articles/4421269296535 |
| Regex patterns | In Custom Keyword rules: up to 10 patterns of 75 chars (API allows 260). Rust syntax; case-insensitive and Unicode by default; no backreferences. Validated when pasted. Errors: invalid syntax, compile size exceeded, too long. | https://support.discord.com/hc/en-us/articles/10069840290711 ; https://docs.discord.com/developers/resources/auto-moderation |
| Block Spam Content rule | ML spam and invite-spam filter. Actions: block or alert only. | https://support.discord.com/hc/en-us/articles/4421269296535 |
| Block Mention Spam rule | Limit of 1–50 unique user/role mentions per message, plus mention-raid protection toggle. Actions: block, alert, timeout. Invalid values show an inline error. | https://docs.discord.com/developers/resources/auto-moderation ; https://discord.com/blog/discord-patch-notes-october-6-2026 |
| Block Words in Member Profile Names | Safety Setup > Members. Checks nickname, else username (MEMBER_PROFILE trigger). Offenders are quarantined until they rename (AUTOMOD_QUARANTINED_USERNAME); blocked words show an error when renaming. | https://support.discord.com/hc/en-us/articles/4421269296535 ; https://docs.discord.com/developers/resources/guild |
| Action: Block message | Optional custom response up to 150 chars, shown only to the author. | https://docs.discord.com/developers/resources/auto-moderation ; https://support.discord.com/hc/en-us/articles/4421269296535 |
| Action: Send alert | Logs to an alert channel (who, what, where). | https://docs.discord.com/developers/resources/auto-moderation |
| Action: Timeout | Keyword and mention-spam rules only; up to 4 weeks; needs Time Out Members. | https://docs.discord.com/developers/resources/auto-moderation |
| Action: Block member interaction | Blocks text, voice and other interactions (quarantine). | https://docs.discord.com/developers/resources/auto-moderation |
| Exemptions | Up to 20 roles and 50 channels per rule. Admin and Manage Server holders are always exempt. Exempting a channel also exempts its threads and voice text. | https://docs.discord.com/developers/resources/auto-moderation ; https://support.discord.com/hc/en-us/articles/4421269296535 |
| Scope | Text channels, threads, forum posts and voice text chat. User-installed app messages are flagged against the invoking user. | https://support.discord.com/hc/en-us/articles/4421269296535 |
| "Flag Word" shortcut | Right-click a word in a message to add it to a rule. | https://support.discord.com/hc/en-us/articles/4421269296535 |
| AutoMod alert message rendering | AUTO_MODERATION_ACTION message from "AutoMod" with an Official badge; the blocked keyword is highlighted only when standalone; the content is hidden for blocked users. "Delete Message" in the right-click menu only with permission. Inbox Jump. | https://docs.discord.com/developers/resources/message ; https://discord.com/blog/discord-patch-notes-september-8-2026 ; https://discord.com/blog/discord-patch-notes-june-4-2026 |
| Alert quick actions | Action buttons on alerts (e.g. timeout, kick or ban the member, delete the message) (unconfirmed). | (unconfirmed, no official source found) |
| Report Issues / "Did we get it wrong?" | Feedback to Discord on false positives from alerts and blocked-message notices. | https://support.discord.com/hc/en-us/articles/4421269296535 ; https://discord.com/blog/discord-patch-notes-october-6-2026 ; https://discord.com/blog/discord-patch-notes-july-7-2026 |
| Username quarantine system message | Rendered in channel for quarantined users. | https://discord.com/blog/discord-patch-notes-september-8-2026 |

### Audit Log

| Feature | Detail (states, dialogs, conditions such as permission/Nitro/DM-only) | Source URL |
|---|---|---|
| Audit Log tab | Needs View Audit Log. Filter by user and by action type; entries expand to show changed fields and the reason; 45-day retention; entries can't be edited. | https://docs.discord.com/developers/resources/audit-log ; https://support.discord.com/hc/en-us/articles/33694251638295 |
| Entries: server | Server update. | https://docs.discord.com/developers/resources/audit-log |
| Entries: channels | Channel create, update, delete (with reason since 2026; correct type for forum, media and announcement channels); permission overwrite create, update, delete. | https://docs.discord.com/developers/resources/audit-log ; https://discord.com/blog/discord-patch-notes-march-6-2026 ; https://discord.com/blog/discord-patch-notes-september-8-2026 |
| Entries: members | Kick, prune, ban add/remove, member update (nickname, timeout), role update, voice move, voice disconnect, bot add. | https://docs.discord.com/developers/resources/audit-log |
| Entries: roles | Role create, update (including color/style), delete. | https://docs.discord.com/developers/resources/audit-log |
| Entries: invites / webhooks / integrations | Invite create, update, delete; webhook create, update, delete; integration create, update, delete. | https://docs.discord.com/developers/resources/audit-log |
| Entries: expressions | Emoji, sticker and soundboard sound create, update, delete. | https://docs.discord.com/developers/resources/audit-log |
| Entries: messages | Message delete, bulk delete, pin, unpin. | https://docs.discord.com/developers/resources/audit-log |
| Entries: stage / events / threads | Stage instance, scheduled event and thread create, update, delete. Forum post tag changes logged (2026); forum/media post slowmode and NSFW changes logged (2026). | https://docs.discord.com/developers/resources/audit-log ; https://discord.com/blog/discord-patch-notes-may-4-2026 ; https://discord.com/blog/discord-patch-notes-june-4-2026 |
| Entries: apps | Application command permission update. | https://docs.discord.com/developers/resources/audit-log |
| Entries: AutoMod | Rule create, update, delete; block message; flag to channel; timeout ("communication disabled"); quarantine user. | https://docs.discord.com/developers/resources/audit-log |
| Entries: monetization | Creator monetization request created, terms accepted. | https://docs.discord.com/developers/resources/audit-log |
| Entries: onboarding / Server Guide | Onboarding prompt create, update, delete; onboarding create, update; home settings (Server Guide) create, update. | https://docs.discord.com/developers/resources/audit-log |
| Entries: voice channel status | Voice channel status set and cleared. | https://docs.discord.com/developers/resources/audit-log |

### Bans

| Feature | Detail (states, dialogs, conditions such as permission/Nitro/DM-only) | Source URL |
|---|---|---|
| Bans tab | Lists banned users with their ban reason; search; select a user to Revoke Ban (unban). Lives under Safety Setup (2026 label). Needs Ban Members. Search and revoke flow unconfirmed beyond the tab's existence. | https://discord.com/blog/discord-patch-notes-october-6-2026 ; https://discord.com/blog/discord-patch-notes-august-4-2026 ; https://docs.discord.com/developers/topics/permissions |
| Bulk ban (Members page) | Select checkboxes > Ban Members. Needs Manage Server plus Ban Members, and a 2FA code or password each time. API: up to 200 users, delete messages 0–7 days. | https://support.discord.com/hc/en-us/articles/15946797617431 ; https://docs.discord.com/developers/resources/guild |
| Banned user rejoin block | Banned users can't rejoin or reapply via applications; invite pages show "Banned". | https://support.discord.com/hc/en-us/articles/29729107418519 ; https://support.discord.com/hc/en-us/articles/360001556852 |

### Members page

| Feature | Detail (states, dialogs, conditions such as permission/Nitro/DM-only) | Source URL |
|---|---|---|
| Members page access | Members item in the channel list (Community servers or "Show Members in Channel List") or Server Settings > User Management > Members. Desktop/browser. Needs owner or Administrator, Manage Server, Manage Roles, Manage Nicknames, Ban, Timeout or Kick. | https://support.discord.com/hc/en-us/articles/15946797617431 |
| Member table fields | Name, Member Since, Joined Discord (account age), Join Method (invite code needs Manage Server), Roles (+ to add), messages sent, rules agreement, safety signals, verification status. Booster and app icons shown. | https://support.discord.com/hc/en-us/articles/15946797617431 ; https://discord.com/blog/discord-patch-notes-february-4-2026 |
| Search | By username or server display name. | https://support.discord.com/hc/en-us/articles/15946797617431 |
| Sort | Member Since (newest or oldest), Joined Discord (newest or oldest). | https://support.discord.com/hc/en-us/articles/15946797617431 |
| Filters | Member Since timeframe; Joined Discord timeframe; Join Method (specific invite, or other: Discovery, Vanity URL, Bot invite, Student Hub); Signals. | https://support.discord.com/hc/en-us/articles/15946797617431 ; https://discord.com/blog/discord-patch-notes-april-6-2026 |
| Safety signals | Unusual DM Activity, Unusual Account Activity, Timed Out, Quarantined. | https://support.discord.com/hc/en-us/articles/15946797617431 |
| Bulk role add | Checkbox selection plus Roles [+]. | https://support.discord.com/hc/en-us/articles/15946797617431 |
| Row actions | More Options or right-click a row for profile and management tools; "view main profile" for members with per-server profiles. | https://support.discord.com/hc/en-us/articles/15946797617431 ; https://discord.com/blog/discord-patch-notes-august-4-2026 |
| Prune | Prune button near search: members inactive 7 or 30 days, optional "Also Include Members With These Roles", count preview, confirmation. Needs Kick Members (Administrator if PRUNE_REQUIRES_ADMIN). | https://support.discord.com/hc/en-us/articles/213507137 ; https://docs.discord.com/developers/resources/guild |
| Member Applications queue | Pending/approved/rejected lists on the Members page; pending count next to the Members tab. Each application shows the reason, Discord join date and submission time. Actions: Approve, Reject (applicant notified; may reapply unless banned), Interview (DM). Kick Members holders can review. | https://support.discord.com/hc/en-us/articles/29729107418519 ; https://discord.com/blog/discord-patch-notes-june-4-2026 |
| Live removal | Kicked or banned members leave the list immediately (2026 fix). | https://discord.com/blog/discord-patch-notes-september-8-2026 |
| Fast large member lists | Member lists load faster in very large servers (2025). Offline members are hidden at 1,000+ members. | https://discord.com/blog/discord-update-september-25-2025-changelog ; https://support.discord.com/hc/en-us/articles/33694251638295 |

### Community settings

| Feature | Detail (states, dialogs, conditions such as permission/Nitro/DM-only) | Source URL |
|---|---|---|
| Enable Community flow | Needs Administrator. Requirements: verified-email verification level, explicit media filter for all members, a Rules channel and a Community Updates channel. Unlocks Welcome Screen, Announcement, Stage and Forum channels, Insights, Discovery, Onboarding, Server Guide, Rules Screening and Community Updates. | https://support.discord.com/hc/en-us/articles/360047132851 ; https://docs.discord.com/developers/resources/guild |
| Community Overview | Rules channel; Community Updates (moderators-only) channel; Safety notifications channel; primary language; "Join Discord's Admins Server" prompt; Disable Community. | https://support.discord.com/hc/en-us/articles/360047132851 ; https://discord.com/blog/discord-patch-notes-may-4-2026 |
| Onboarding setup | Server Settings > Onboarding (Manage Server plus Manage Roles). Default Channels (at least 7, 5+ writable); Questions with options mapping roles/channels, "Ask before member joins", "Allow multiple answers", "Make Required", Preview; channel coverage check; Review and Enable. Desktop only. Managed roles allowed (2026). | https://support.discord.com/hc/en-us/articles/11074987197975 ; https://discord.com/blog/discord-patch-notes-may-4-2026 |
| Server Guide setup | Under Onboarding once Default Channels and Questions are done: Welcome Sign, 3–5 New Member To-Dos, Resource Pages. | https://support.discord.com/hc/en-us/articles/13497665141655 |
| Welcome Screen setup | Set Up Welcome Screen: up to 5 channels with descriptions, Preview, Enable. Becomes uneditable once enabled. | https://support.discord.com/hc/en-us/articles/360043913591 |
| Server Insights | Community with over 500 members (alpha); View Server Insights. Sections: Growth and Activation, Engagement, Audience, Announcement Channels, Welcome Screen. Daily, weekly or monthly; custom ranges; CSV export. | https://support.discord.com/hc/en-us/articles/360032807371 |
| Discovery settings | Enable Discovery (Administrator, requirements). The article says Discovery is limited to servers with over 1,000 members, or Partnered/Verified servers. Description, category and tag fields are unconfirmed. Discovery qualification and disqualification notices arrive as system messages. | https://support.discord.com/hc/en-us/articles/360030843331 ; https://docs.discord.com/developers/resources/message ; https://discord.com/blog/discord-patch-notes-october-6-2026 |
| Student Hub settings | Edit settings for Student Hub servers. | https://discord.com/blog/discord-patch-notes-july-7-2026 |

### Monetization and boosts

| Feature | Detail (states, dialogs, conditions such as permission/Nitro/DM-only) | Source URL |
|---|---|---|
| Server Subscriptions tab | Shown after monetization is enabled via the Developer Portal (owner 18+, US, 2FA, Community, MFA for moderation). Basic info (exclusive channels or whole server paid); 1–3 tiers with flair and preview; premium emoji (up to 25); free trials (1 day or 1 week; limits 10/25/50/100/unlimited). Payment tab is owner-only. | https://creator-support.discord.com/hc/en-us/articles/10423011974551 ; https://discord.com/blog/discord-patch-notes-july-7-2026 |
| Server Shop (Server Products) | Downloadables and Premium Roles (one-time). US desktop/web experiment. | https://support.discord.com/hc/en-us/articles/17633135983383 |
| Creator monetization analytics | View Creator Monetization Analytics permission. | https://docs.discord.com/developers/topics/permissions |
| Boost Level 1 (2 boosts) | 100 emoji, 15 stickers, 24 sounds, 128 kbps, 720p60 Go Live, invite background, animated icon. | https://support.discord.com/hc/en-us/articles/360028038352 |
| Boost Level 2 (7 boosts) | 150 emoji, 30 stickers, 36 sounds, 256 kbps, 1080p60, 150 stage video seats, 50 MB uploads, role icons, banner. | https://support.discord.com/hc/en-us/articles/360028038352 |
| Boost Level 3 (14 boosts) | 250 emoji, 60 stickers, 48 sounds, 384 kbps, 300 stage video seats, 100 MB uploads, vanity URL, animated banner. | https://support.discord.com/hc/en-us/articles/360028038352 |
| Additional boost perks | Server Tags (3), Enhanced Role Styles (3), Server Themes (3, experiment), Game Server Hosting (3+), 250 MB uploads (5, experiment). Use boosts separate from levels; reallocatable in the Server Boosts tab. | https://support.discord.com/hc/en-us/articles/360028038352 |
| Level grace period | Perks persist for 72 hours after losing a level. | https://support.discord.com/hc/en-us/articles/360028038352 |
| Boost system messages | GUILD_BOOST and TIER_1/2/3 messages in the system channel. | https://docs.discord.com/developers/resources/message |
| Game Servers | (2025 experiment) Game Panel (Shockbyte); up to 10 per server; choose game, name, location and plan (Starter/Pro/Ultimate); 16 games. Disable from the Server Boosts tab. | https://support.discord.com/hc/en-us/articles/35370817986839 |

### Invites and access

| Feature | Detail (states, dialogs, conditions such as permission/Nitro/DM-only) | Source URL |
|---|---|---|
| Invites tab | Lists invite links (inviter, code, uses, expiry); X to revoke (codes can't be recreated); Pause Invites. Up to 999 unique codes. | https://support.discord.com/hc/en-us/articles/208866998 ; https://support.discord.com/hc/en-us/articles/33694251638295 |
| Edit Channel > Invites | Per-channel invite list (unconfirmed detail). | https://support.discord.com/hc/en-us/articles/10543994968087 |
| Access tab: "How can people join your server?" | Invite Only (default), Apply to Join (applications) or Discoverable. Optionally show application links on the Server Profile. | https://support.discord.com/hc/en-us/articles/29729107418519 |
| Application questions | Custom screening questions for Apply to Join. Kick Members holders can create invites that skip applications. | https://support.discord.com/hc/en-us/articles/29729107418519 |
| Invite expiry options | Includes a 30-minute link and Never (Community servers' links never expire). | https://support.discord.com/hc/en-us/articles/208866998 ; https://support.discord.com/hc/en-us/articles/14078261239831 |

### Member moderation actions

| Feature | Detail (states, dialogs, conditions such as permission/Nitro/DM-only) | Source URL |
|---|---|---|
| Kick | Member context menu > Kick, with a confirmation dialog and optional reason (audit-logged). Needs Kick Members; target must rank below you. Dialog fields unconfirmed. | https://docs.discord.com/developers/topics/permissions ; https://support.discord.com/hc/en-us/articles/214836687 ; https://docs.discord.com/developers/resources/audit-log |
| Ban | Member context menu > Ban: delete message history (0 up to 7 days) and pick a reason from preset options or custom. Needs Ban Members (2FA if the server requires it). Exact history choices (e.g. previous hour/6h/12h/24h/3d/7d) are unconfirmed. | https://docs.discord.com/developers/resources/guild ; https://discord.com/blog/discord-patch-notes-july-7-2026 |
| Timeout | Right-click name > Timeout: 60 seconds, 5 min, 10 min, 1 hour, 1 day, 1 week, plus reason. API maximum 28 days. Moderators see a timeout icon after the user's message timestamps; tapping it shows a toast (Android). | https://support.discord.com/hc/en-us/articles/4413305239191 ; https://docs.discord.com/developers/resources/guild ; https://discord.com/blog/discord-patch-notes-september-8-2026 |
| Timed-out member experience | Can't send messages, add new reactions or join voice; can still read. Sees a notice. Can rejoin the server (iOS fix 2026). | https://support.discord.com/hc/en-us/articles/4413305239191 ; https://discord.com/blog/discord-patch-notes-october-6-2026 |
| Remove Timeout | Right-click > Remove Timeout > confirm. | https://support.discord.com/hc/en-us/articles/4413305239191 |
| Change Nickname | Edit another member's server nickname (Manage Nicknames); your own needs Change Nickname. | https://docs.discord.com/developers/topics/permissions ; https://support.discord.com/hc/en-us/articles/219070107 |
| Roles submenu / mini-profile role edit | Add or remove roles from the member context menu or the "+" in their profile; removal updates immediately. | https://discord.com/blog/discord-patch-notes-july-7-2026 ; https://support.discord.com/hc/en-us/articles/15946797617431 |
| Voice: Server Mute / Server Deafen | Mute Members / Deafen Members; works even on members who blocked you (2026). | https://discord.com/blog/discord-patch-notes-october-6-2026 ; https://docs.discord.com/developers/topics/permissions |
| Voice: Move To / Disconnect | Move Members: move to another voice channel (including ones the member can't see) or disconnect. | https://discord.com/blog/discord-patch-notes-october-6-2026 ; https://discord.com/blog/discord-patch-notes-may-4-2026 |
| Stage moderation | Invite to Speak, Move to Audience, Disconnect from Stage, Show Requests (raise hand), toggle "Allow requests to speak", end stage; Request to Stream cooldown (2026). Stage moderators need Manage Channels, Mute Members and Move Members. | https://support.discord.com/hc/en-us/articles/1500005513722 ; https://discord.com/blog/discord-patch-notes-july-7-2026 |
| Transfer Ownership | Members list > right-click > Transfer Ownership; acknowledgment toggle, MFA code if enabled; irreversible via Support. | https://support.discord.com/hc/en-us/articles/216273938 |
| Mod View | Opened from a Members-page row ("Open Mod View") or a member (experiment, limited servers). Shows account info, Server Activity (messages, with a hidden-message banner for blocked users), Highest role, Mod Permissions and a Moderator Action Log. Message button; ESC closes; management tools. | https://support.discord.com/hc/en-us/articles/15946797617431 ; https://discord.com/blog/discord-patch-notes-may-4-2026 ; https://discord.com/blog/discord-patch-notes-september-8-2026 ; https://discord.com/blog/discord-patch-notes-june-4-2026 |
| Report / Flag to Mods | Members can flag messages to server moderators through a "Flag to Mods" modal, separate from Report to Discord (2026 references; flow details unconfirmed). | https://discord.com/blog/discord-patch-notes-october-6-2026 |
| Report Raid | Raid reports and false-alarm reports post as system messages. The UI entry point (Security Actions / alert banner) is unconfirmed. | https://docs.discord.com/developers/resources/message |
| Spam-flagged messages | Mods can jump to spam-flagged messages from search (2026). | https://discord.com/blog/discord-patch-notes-february-4-2026 |
| Copy User ID | Developer Mode, from the member context menu. | https://support.discord.com/hc/en-us/articles/206346498 |

### Permissions (full flag list and what each gates in the UI)

Flags marked * require the moderator to have 2FA when the server requires 2FA for moderation. T/V/S = channel types where the flag can be overwritten (Text, Voice, Stage). Timed-out members temporarily lose most permissions.

| Feature | Detail (states, dialogs, conditions such as permission/Nitro/DM-only) | Source URL |
|---|---|---|
| CREATE_INSTANT_INVITE (0x1) "Create Invite" | Invite People, Create Invite on channels, event invite links (T/V/S). | https://docs.discord.com/developers/topics/permissions ; https://support.discord.com/hc/en-us/articles/10543994968087 |
| KICK_MEMBERS* (0x2) "Kick Members" | Kick and Prune; reviewing member applications; invites that skip applications; Members page access; Security Actions. | https://docs.discord.com/developers/topics/permissions ; https://support.discord.com/hc/en-us/articles/29729107418519 |
| BAN_MEMBERS* (0x4) | Ban, Bans tab, bulk ban (with Manage Server). | https://docs.discord.com/developers/topics/permissions |
| ADMINISTRATOR* (0x8) | All permissions; bypasses channel overwrites; View Server As Role. | https://docs.discord.com/developers/topics/permissions ; https://support.discord.com/hc/en-us/articles/360055709773 |
| MANAGE_CHANNELS* (0x10) | Create, edit, delete and reorder channels and categories; Edit Channel cog; stage moderator requirement (T/V/S). | https://docs.discord.com/developers/topics/permissions |
| MANAGE_GUILD* (0x20) "Manage Server" | Overview, Server Profile, Invites (codes), Pause Invites, AutoMod, Integrations, App Directory adds, templates, Welcome Screen, Security Actions. | https://docs.discord.com/developers/topics/permissions |
| ADD_REACTIONS (0x40) | Add new reactions (existing ones can still be clicked) (T/V/S). | https://docs.discord.com/developers/topics/permissions ; https://support.discord.com/hc/en-us/articles/10543994968087 |
| VIEW_AUDIT_LOG (0x80) | Audit Log tab. | https://docs.discord.com/developers/topics/permissions |
| PRIORITY_SPEAKER (0x100) | Priority speaker in voice (V). | https://docs.discord.com/developers/topics/permissions |
| STREAM (0x200) "Video" | Go Live and camera in voice and stage (V/S). | https://docs.discord.com/developers/topics/permissions ; https://support.discord.com/hc/en-us/articles/1500005513722 |
| VIEW_CHANNEL (0x400) "View Channel(s)" | Channel visible in the list (hidden otherwise; lock icon for voice when only Connect is missing) (T/V/S). | https://docs.discord.com/developers/topics/permissions ; https://support.discord.com/hc/en-us/articles/115001580171 |
| SEND_MESSAGES (0x800) "Send Messages and Create Posts" | Composer enabled; forum post creation; publish own announcements (T/V/S). | https://docs.discord.com/developers/topics/permissions ; https://support.discord.com/hc/en-us/articles/360032008192 |
| SEND_TTS_MESSAGES (0x1000) | /tts messages (T/V/S). | https://docs.discord.com/developers/topics/permissions |
| MANAGE_MESSAGES* (0x2000) | Delete others' messages; publish anyone's announcements; delete AutoMod alerts (T/V/S). | https://docs.discord.com/developers/topics/permissions ; https://support.discord.com/hc/en-us/articles/360032008192 |
| EMBED_LINKS (0x4000) | Link embeds render (T/V/S). | https://docs.discord.com/developers/topics/permissions |
| ATTACH_FILES (0x8000) | Upload button and paste uploads (T/V/S). | https://docs.discord.com/developers/topics/permissions |
| READ_MESSAGE_HISTORY (0x10000) | See past messages; also limits View As Role history (T/V/S). | https://docs.discord.com/developers/topics/permissions ; https://discord.com/blog/discord-patch-notes-october-6-2026 |
| MENTION_EVERYONE (0x20000) | @everyone, @here and all-roles mentions (T/V/S). | https://docs.discord.com/developers/topics/permissions ; https://support.discord.com/hc/en-us/articles/10543994968087 |
| USE_EXTERNAL_EMOJIS (0x40000) | Other servers' emoji (Nitro). Without it, custom typing-indicator emoji show as "…" (T/V/S). | https://docs.discord.com/developers/topics/permissions ; https://support.discord.com/hc/en-us/articles/42962943077271 |
| VIEW_GUILD_INSIGHTS (0x80000) | Server Insights. | https://docs.discord.com/developers/topics/permissions ; https://support.discord.com/hc/en-us/articles/360032807371 |
| CONNECT (0x100000) | Join voice and stage (V/S). | https://docs.discord.com/developers/topics/permissions |
| SPEAK (0x200000) | Transmit audio (V). | https://docs.discord.com/developers/topics/permissions |
| MUTE_MEMBERS (0x400000) | Server Mute; stage moderation (V/S). | https://docs.discord.com/developers/topics/permissions |
| DEAFEN_MEMBERS (0x800000) | Server Deafen (V). | https://docs.discord.com/developers/topics/permissions |
| MOVE_MEMBERS (0x1000000) | Drag or Move To and Disconnect (V/S). | https://docs.discord.com/developers/topics/permissions |
| USE_VAD (0x2000000) | Voice Activity; without it, push-to-talk only (V). | https://docs.discord.com/developers/topics/permissions |
| CHANGE_NICKNAME (0x4000000) | Change your own server nickname. | https://docs.discord.com/developers/topics/permissions |
| MANAGE_NICKNAMES (0x8000000) | Change others' nicknames; Members page access. | https://docs.discord.com/developers/topics/permissions |
| MANAGE_ROLES* (0x10000000) "Manage Roles" / "Manage Permissions" at channel level | Roles tab, role assignment, channel overwrites, Linked Roles, Onboarding (with Manage Server) (T/V/S). | https://docs.discord.com/developers/topics/permissions ; https://support.discord.com/hc/en-us/articles/10543994968087 |
| MANAGE_WEBHOOKS* (0x20000000) | Webhooks and Channel Following (T/V/S). | https://docs.discord.com/developers/topics/permissions ; https://support.discord.com/hc/en-us/articles/360028384531 |
| MANAGE_GUILD_EXPRESSIONS* (0x40000000) "Manage Expressions" | Edit or delete any emoji, sticker or sound. | https://docs.discord.com/developers/topics/permissions |
| USE_APPLICATION_COMMANDS (0x80000000) | Slash and context-menu commands (T/V/S). | https://docs.discord.com/developers/topics/permissions |
| REQUEST_TO_SPEAK (0x100000000) | Raise hand in a stage (S). | https://docs.discord.com/developers/topics/permissions |
| MANAGE_EVENTS (0x200000000) | Edit, cancel or start anyone's events (V/S). | https://docs.discord.com/developers/topics/permissions ; https://support.discord.com/hc/en-us/articles/4409494125719 |
| MANAGE_THREADS* (0x400000000) "Manage Threads and Posts" | Rename, delete, lock or unarchive threads; thread slowmode; view all private threads; moderated tags; pin forum post (T). | https://docs.discord.com/developers/topics/permissions ; https://support.discord.com/hc/en-us/articles/4404809613847 |
| CREATE_PUBLIC_THREADS (0x800000000) | Create Thread (T). | https://docs.discord.com/developers/topics/permissions |
| CREATE_PRIVATE_THREADS (0x1000000000) | Private Thread toggle (T). | https://docs.discord.com/developers/topics/permissions |
| USE_EXTERNAL_STICKERS (0x2000000000) | Other servers' stickers (T/V/S). | https://docs.discord.com/developers/topics/permissions |
| SEND_MESSAGES_IN_THREADS (0x4000000000) | Thread and post composer (T). | https://docs.discord.com/developers/topics/permissions |
| USE_EMBEDDED_ACTIVITIES (0x8000000000) "Use Activities" | Launch Activities (T/V). | https://docs.discord.com/developers/topics/permissions |
| MODERATE_MEMBERS** (0x10000000000) "Time out members" | Timeout and Remove Timeout; AutoMod timeout action; Members page; Security Actions. | https://docs.discord.com/developers/topics/permissions ; https://support.discord.com/hc/en-us/articles/4413305239191 |
| VIEW_CREATOR_MONETIZATION_ANALYTICS* (0x20000000000) | Role subscription insights. | https://docs.discord.com/developers/topics/permissions |
| USE_SOUNDBOARD (0x40000000000) | Soundboard in voice (V). | https://docs.discord.com/developers/topics/permissions ; https://support.discord.com/hc/en-us/articles/12612888127767 |
| CREATE_GUILD_EXPRESSIONS (0x80000000000) "Create Expressions" | Upload emoji, stickers and sounds; edit or delete your own. | https://docs.discord.com/developers/topics/permissions |
| CREATE_EVENTS (0x100000000000) | Create Event; edit or delete your own (V/S). | https://docs.discord.com/developers/topics/permissions |
| USE_EXTERNAL_SOUNDS (0x200000000000) | Other servers' sounds (V). | https://docs.discord.com/developers/topics/permissions |
| SEND_VOICE_MESSAGES (0x400000000000) | Voice-message recording (T/V/S). | https://docs.discord.com/developers/topics/permissions |
| SET_VOICE_CHANNEL_STATUS (0x1000000000000) | "Set a channel status" (V). | https://docs.discord.com/developers/topics/permissions ; https://support.discord.com/hc/en-us/articles/19583625604887 |
| SEND_POLLS (0x2000000000000) "Create Polls" | Poll creation (T/V/S). | https://docs.discord.com/developers/topics/permissions |
| USE_EXTERNAL_APPS (0x4000000000000) | User-installed apps' responses are public; ephemeral otherwise (T/V/S). | https://docs.discord.com/developers/topics/permissions |
| PIN_MESSAGES (0x8000000000000) | Pin and unpin; remove from the Pinned popout (2026 check) (T). | https://docs.discord.com/developers/topics/permissions ; https://discord.com/blog/discord-patch-notes-june-4-2026 |
| BYPASS_SLOWMODE (0x10000000000000) | (2026) Ignore slowmode. Manage Messages, Manage Channel and Manage Threads stopped bypassing after Feb 23, 2026 (T/V/S). | https://docs.discord.com/developers/topics/permissions ; https://support.discord.com/hc/en-us/articles/360016150952 ; https://discord.com/blog/discord-patch-notes-february-4-2026 |

## Keyboard shortcuts

macOS bindings come from the official chart image (Fulle_Keyboard_image.png, attachment 211072848) on the macOS hotkeys article. Where official sources disagree, both values are given.

### Navigation

| Action | macOS | Windows | Source URL |
|---|---|---|---|
| Navigate between servers | Cmd+Opt+Up / Cmd+Opt+Down | Ctrl+Alt+Up / Ctrl+Alt+Down (the Commands guide says Ctrl+Alt+Left/Right) | https://support.discord.com/hc/en-us/articles/225878307 ; https://support.discord.com/hc/en-us/articles/225977308 |
| Navigate between channels | Opt+Up / Opt+Down | Alt+Up / Alt+Down | https://support.discord.com/hc/en-us/articles/225878307 ; https://support.discord.com/hc/en-us/articles/225977308 |
| Navigate between unread channels | Opt+Shift+Up / Down | Alt+Shift+Up / Down | https://support.discord.com/hc/en-us/articles/225878307 ; https://support.discord.com/hc/en-us/articles/225977308 |
| Navigate between unread channels with mentions | Cmd+Shift+Opt+Up / Down | Ctrl+Shift+Alt+Up / Down | https://support.discord.com/hc/en-us/articles/225878307 ; https://support.discord.com/hc/en-us/articles/225977308 |
| Return to previous text channel | Ctrl+B | Ctrl+B (also Alt+Right) | https://support.discord.com/hc/en-us/articles/225878307 ; https://support.discord.com/hc/en-us/articles/225977308 |
| Return to active audio channel | Cmd+Opt+A | Ctrl+Alt+A (also Alt+Left) | https://support.discord.com/hc/en-us/articles/225878307 ; https://support.discord.com/hc/en-us/articles/225977308 |
| Jump back to your call | Cmd+Shift+Opt+V | Ctrl+Shift+Alt+V | https://discord.com/blog/how-to-use-keyboard-shortcuts-on-discord-create-custom-keybinds |
| Navigate back | Alt+Left (as listed by the official guide) | Alt+Left | https://support.discord.com/hc/en-us/articles/31232432266647 |
| Navigate back / forward (mac menu style) | Cmd+[ / Cmd+] (unconfirmed) | Alt+Left / Alt+Right | https://support.discord.com/hc/en-us/articles/31232432266647 |
| Mouse side buttons back / forward | Mouse button 4 / 5 | Mouse button 4 / 5 | https://discord.com/blog/discord-update-march-24-2026-changelog |
| Quick Switcher / find or start a DM | Cmd+K | Ctrl+K | https://support.discord.com/hc/en-us/articles/115000070311 |
| Close Quick Switcher | Cmd+T or Esc (Cmd+T as a toggle is inferred from a 2026 fix note) | Ctrl+T or Esc | https://discord.com/blog/discord-patch-notes-march-6-2026 |
| Create or join a server | Cmd+Shift+N | Ctrl+Shift+N | https://support.discord.com/hc/en-us/articles/225878307 |
| Create a private group (group DM) | Cmd+Shift+T | Ctrl+Shift+T | https://support.discord.com/hc/en-us/articles/225878307 |
| Open User Settings | Cmd+, | Ctrl+, | https://support.discord.com/hc/en-us/articles/31232432266647 |
| Toggle hotkeys list (keyboard shortcuts modal) | Cmd+/ | Ctrl+/ | https://support.discord.com/hc/en-us/articles/225878307 |
| Get help | Cmd+Shift+H | Ctrl+Shift+H | https://support.discord.com/hc/en-us/articles/225878307 |
| Move to next / previous section | F6 / Shift+F6 | F6 / Shift+F6 | https://support.discord.com/hc/en-us/articles/1500000056121 |
| Enter keyboard mode / move between elements | Tab / Shift+Tab | Tab / Shift+Tab | https://support.discord.com/hc/en-us/articles/1500000056121 |
| Activate focused element | Return / Space | Enter / Space | https://support.discord.com/hc/en-us/articles/31232432266647 |
| Move through lists, messages, menus | Arrow keys | Arrow keys | https://support.discord.com/hc/en-us/articles/1500000056121 |
| Lift item for reordering (servers, channels) | Cmd+D, then arrows, Space/Return drop, Esc cancel | Ctrl+D | https://support.discord.com/hc/en-us/articles/4408877527703 |

### Reading and marking

| Action | macOS | Windows | Source URL |
|---|---|---|---|
| Mark channel as read | Esc | Esc | https://support.discord.com/hc/en-us/articles/225878307 |
| Mark server read | Shift+Esc | Shift+Esc | https://support.discord.com/hc/en-us/articles/225878307 |
| Scroll chat up / down | PgUp / PgDown (Fn+Up/Down) | Page Up / Page Down | https://support.discord.com/hc/en-us/articles/225878307 |
| Jump to oldest unread message | Shift+PgUp | Shift+Page Up | https://support.discord.com/hc/en-us/articles/225878307 |
| Browse channel history | Cmd+Up / Cmd+Down | Ctrl+Up / Ctrl+Down | https://support.discord.com/hc/en-us/articles/31232432266647 |
| Toggle Inbox | Cmd+I | Ctrl+I | https://support.discord.com/hc/en-us/articles/360045027712 |
| Toggle mentions popout (legacy chart) | Cmd+Opt+@ | Ctrl+Alt+@ (article text shows Ctrl+I) | https://support.discord.com/hc/en-us/articles/225878307 ; https://support.discord.com/hc/en-us/articles/31232432266647 |
| Mark top inbox item read | Cmd+Shift+E | Ctrl+Shift+E | https://support.discord.com/hc/en-us/articles/360045027712 |
| Toggle pins popout | Cmd+P | Ctrl+P | https://support.discord.com/hc/en-us/articles/225878307 |
| Toggle channel member list | Cmd+U | Ctrl+U | https://support.discord.com/hc/en-us/articles/225878307 |
| Search current channel | Cmd+F (prefills the current channel) | Ctrl+F | https://support.discord.com/hc/en-us/articles/31232432266647 ; https://discord.com/blog/discord-patch-notes-march-6-2026 |
| Search all channels | Cmd+Shift+F | Ctrl+Shift+F | https://support.discord.com/hc/en-us/articles/31232432266647 |
| Delete text left of cursor in search | Cmd+Delete | Ctrl+Backspace (unconfirmed) | https://discord.com/blog/discord-patch-notes-may-4-2026 |

### Composer

| Action | macOS | Windows | Source URL |
|---|---|---|---|
| Focus text area | Tab | Tab | https://support.discord.com/hc/en-us/articles/225878307 |
| Edit last message (empty composer) | Up arrow (the Commands guide says Opt+Up) | Up arrow (the Commands guide says Shift+Up) | https://discord.com/blog/how-to-use-keyboard-shortcuts-on-discord-create-custom-keybinds ; https://support.discord.com/hc/en-us/articles/31232432266647 |
| Cancel message / reply / edit | Esc | Esc | https://support.discord.com/hc/en-us/articles/31232432266647 |
| New line without sending | Shift+Return (unconfirmed) | Shift+Enter (unconfirmed) | https://support.discord.com/hc/en-us/articles/217083547 |
| Autocomplete mention / emoji / command | Tab or Return (unconfirmed) | Tab or Enter (unconfirmed) | — |
| Bold | Cmd+B | Ctrl+B | https://support.discord.com/hc/en-us/articles/31232432266647 |
| Italic | Cmd+I | Ctrl+I | https://support.discord.com/hc/en-us/articles/31232432266647 |
| Underline | Cmd+U | Ctrl+U | https://support.discord.com/hc/en-us/articles/31232432266647 |
| Inline code | not listed | Ctrl+Shift+M (as listed; conflicts with Toggle Mute) | https://support.discord.com/hc/en-us/articles/31232432266647 |
| Toggle emoji picker | Cmd+E | Ctrl+E | https://support.discord.com/hc/en-us/articles/225878307 |
| Open GIF picker | Cmd+G | Ctrl+G | https://support.discord.com/hc/en-us/articles/31232432266647 |
| Open sticker picker | Cmd+S | Ctrl+S | https://discord.com/blog/how-to-use-keyboard-shortcuts-on-discord-create-custom-keybinds |
| Upload a file | Cmd+Shift+U | Ctrl+Shift+U | https://support.discord.com/hc/en-us/articles/225878307 |
| Insert GIF URL without sending | Shift+click GIF | Shift+click GIF | https://discord.com/blog/discord-patch-notes-october-6-2026 |
| Open composer context menu | Shift+F10 | Shift+F10 | https://discord.com/blog/discord-patch-notes-july-7-2026 |
| Move by visual line | Home / End | Home / End | https://discord.com/blog/discord-patch-notes-july-7-2026 |
| Submit forum post | Cmd+Return (unconfirmed) | Ctrl+Enter | https://discord.com/blog/discord-patch-notes-october-6-2026 |
| Confirm message deletion dialog | Return | Enter | https://discord.com/blog/discord-patch-notes-february-4-2026 |

### Focused message (Tab into messages, arrow keys to select)

| Action | macOS | Windows | Source URL |
|---|---|---|---|
| Edit message | E (or Up) | E (or Up) | https://support.discord.com/hc/en-us/articles/1500000056121 ; https://discord.com/blog/how-to-use-keyboard-shortcuts-on-discord-create-custom-keybinds |
| Reply | R | R | https://support.discord.com/hc/en-us/articles/1500000056121 |
| Quote | Q | Q | https://support.discord.com/hc/en-us/articles/1500000056121 |
| Add reaction | + | + | https://support.discord.com/hc/en-us/articles/1500000056121 |
| Pin (needs permission) | P | P | https://support.discord.com/hc/en-us/articles/1500000056121 |
| Forward | F | F | https://discord.com/blog/how-to-use-keyboard-shortcuts-on-discord-create-custom-keybinds |
| Delete message | Backspace | Backspace | https://support.discord.com/hc/en-us/articles/1500000056121 |
| Copy message text | Cmd+C | Ctrl+C | https://support.discord.com/hc/en-us/articles/1500000056121 |
| Mark unread | Opt+Return (Alt+Enter) | Alt+Enter | https://support.discord.com/hc/en-us/articles/1500000056121 |
| Return to composer | Esc | Esc | https://support.discord.com/hc/en-us/articles/1500000056121 |
| Delete without confirmation | Shift+click delete (unconfirmed) | Shift+click delete (unconfirmed) | https://discord.com/blog/discord-patch-notes-september-8-2026 |
| Mark unread with mouse | Opt+click message (unconfirmed) | Alt+click message (unconfirmed) | — |

### Calls and voice

| Action | macOS | Windows | Source URL |
|---|---|---|---|
| Answer incoming call | Cmd+Return | Ctrl+Enter | https://support.discord.com/hc/en-us/articles/225878307 ; https://support.discord.com/hc/en-us/articles/225977308 |
| Decline incoming call | Esc | Esc | https://support.discord.com/hc/en-us/articles/225878307 |
| Start call in DM or group | Ctrl+` (chart image) | Ctrl+[ (article); the blog says Ctrl+' | https://support.discord.com/hc/en-us/articles/225878307 ; https://support.discord.com/hc/en-us/articles/225977308 ; https://discord.com/blog/how-to-use-keyboard-shortcuts-on-discord-create-custom-keybinds |
| Toggle mute | Cmd+Shift+M | Ctrl+Shift+M | https://support.discord.com/hc/en-us/articles/225878307 |
| Toggle deafen | Cmd+Shift+D | Ctrl+Shift+D | https://support.discord.com/hc/en-us/articles/225878307 |
| Open Soundboard (in call) | Cmd+Shift+B | Ctrl+Shift+B | https://discord.com/blog/how-to-use-keyboard-shortcuts-on-discord-create-custom-keybinds |
| Soundboard in game overlay | n/a (no overlay on macOS) | Ctrl+` | https://support.discord.com/hc/en-us/articles/12612888127767 |
| Toggle overlay | n/a | Shift+` (default) | https://support.discord.com/hc/en-us/articles/217659737 |
| Save clip | n/a (Clips is Windows only) | Alt+C (default) | https://support.discord.com/hc/en-us/articles/16861982215703 |

### App and misc

| Action | macOS | Windows | Source URL |
|---|---|---|---|
| Zoom in / out | Cmd+= / Cmd+- | Ctrl+= / Ctrl+- | https://support.discord.com/hc/en-us/articles/207260127 |
| Reset zoom | Cmd+0 (unconfirmed) | Ctrl+0 (unconfirmed) | https://support.discord.com/hc/en-us/articles/207260127 |
| Reload client | Cmd+R (unconfirmed) | Ctrl+R (unconfirmed) | https://tutorialtactic.com/blog/discord-shortcuts/ |
| Vibing Wumpus 2.0 | Cmd+Opt+Shift+W | Ctrl+Alt+Shift+W | https://discord.com/blog/discord-patch-notes-february-4-2026 |
| Undo in Quick Switcher | Cmd+Z | Ctrl+Z | https://discord.com/blog/discord-patch-notes-july-7-2026 |

### Configurable keybind actions (User Settings > Keybinds, desktop only; global when Discord is unfocused)

| Action | macOS | Windows | Source URL |
|---|---|---|---|
| Push to Talk (Normal) | unbound by default | unbound by default | https://support.discord.com/hc/en-us/articles/217083547 |
| Push to Talk (Priority) | unbound | unbound | https://support.discord.com/hc/en-us/articles/360011876531 |
| Push to Mute | unbound | unbound | https://support.discord.com/hc/en-us/articles/217083547 |
| Toggle Mute | unbound (built-in Cmd+Shift+M) | unbound (built-in Ctrl+Shift+M) | https://support.discord.com/hc/en-us/articles/217083547 |
| Toggle Deafen | unbound (built-in Cmd+Shift+D) | unbound (built-in Ctrl+Shift+D) | https://support.discord.com/hc/en-us/articles/217083547 |
| Toggle Voice Activity Mode (VAD) | unbound | unbound | https://support.discord.com/hc/en-us/articles/217083547 |
| Toggle Streamer Mode | unbound | unbound | https://support.discord.com/hc/en-us/articles/217083547 |
| Toggle Overlay | n/a | Shift+` | https://support.discord.com/hc/en-us/articles/217083547 |
| Toggle Overlay Lock | n/a | unbound | https://support.discord.com/hc/en-us/articles/217083547 |
| Navigate Back / Navigate Forward | unbound | unbound | https://discord.com/blog/how-to-use-keyboard-shortcuts-on-discord-create-custom-keybinds |
| Switch to Voice Channel | unbound | unbound | https://discord.com/blog/how-to-use-keyboard-shortcuts-on-discord-create-custom-keybinds |
| Disconnect from Voice Channel | unbound | unbound | https://discord.com/blog/how-to-use-keyboard-shortcuts-on-discord-create-custom-keybinds |
| Save Clip | n/a | Alt+C | https://support.discord.com/hc/en-us/articles/16861982215703 |
| Toggle Go Live / Soundboard / Camera keybind | unbound (unconfirmed) | unbound (unconfirmed) | https://tutorialtactic.com/blog/discord-shortcuts/ |
| Keybind behavior notes | Desktop only. Multi-key sequences allowed. Keybinds pause while editing. F10–F24 keys save correctly (2026). On Windows, System Helper enables elevated games. | same | https://support.discord.com/hc/en-us/articles/217083547 ; https://discord.com/blog/discord-patch-notes-february-4-2026 ; https://support.discord.com/hc/en-us/articles/34853435033367 |

## App-level features

### Quick Switcher, links and invites

| Feature | Detail (states, dialogs, conditions such as permission/Nitro/DM-only) | Source URL |
|---|---|---|
| Quick Switcher | Cmd+K. Jump to any server, channel or DM. | https://support.discord.com/hc/en-us/articles/115000070311 |
| Quick Switcher prefixes | `*` servers, `@` DMs / users, `#` text channels, `!` voice channels. | https://support.discord.com/hc/en-us/articles/115000070311 |
| Quick Switcher ranking upgrade | More likely to surface the intended channel or DM. (2025) | https://discord.com/blog/discord-update-june-30-2025-changelog |
| Quick Switcher settings entries | Settings pages such as Language & Time can be opened from the Quick Switcher. (2026) | https://discord.com/blog/discord-patch-notes-february-4-2026 |
| Quick Switcher invite paste | Pasting an invite link joins and opens the server. Cmd+K also works while the search bar is focused. (2026) | https://discord.com/blog/discord-patch-notes-may-4-2026 |
| Message links | Copy Message Link (right-click a message). Format `https://discord.com/channels/{guild}/{channel}/{message}`, with `@me` for DMs (format unconfirmed). Linking to a very large message scrolls to its start. | https://support.discord.com/hc/en-us/articles/206346498 ; https://discord.js.org/docs/packages/formatters/main/messageLink:Function |
| Browser links open in desktop app | Clicking a Discord link in a browser opens it in the app without losing back-navigation (2026). | https://discord.com/blog/discord-patch-notes-april-6-2026 |
| discord:// URL scheme | Custom scheme the desktop app registers for deep links. (unconfirmed) | https://support.discord.com/hc/en-us/community/posts/360057105571 |
| In-message navigation mentions | `<id:customize>`, `<id:browse>`, `<id:guide>`, `<id:linked-roles>`, plus `<#channel>`, `<@user>`, `<@&role>`, `<@$game>`, `</cmd:id>`, `<t:ts:style>` render as clickable elements. | https://docs.discord.com/developers/reference |
| Create Invite | Channel hover > Create Invite. Send to recent DMs or copy the link. Edit Invite Link: expiry (default 7 days), max uses, "Grant temporary membership", "Generate a New Link". | https://support.discord.com/hc/en-us/articles/208866998 |
| Invite link formats | `discord.gg/<code>` and `discord.com/invite/<code>`; custom vanity links. (formats unconfirmed) | https://support.discord.com/hc/en-us/articles/115001542132 |
| Invalid invite states | Expired, revoked or invalid invite errors when opening a link. | https://support.discord.com/hc/en-us/articles/360001556852 |
| Group DM invites | Invite links to group DMs. Full group DMs hide the invite button (2026). | https://support.discord.com/hc/en-us/articles/360004851252 ; https://discord.com/blog/discord-patch-notes-september-8-2026 |

### Overlay and Streamer Mode

| Feature | Detail (states, dialogs, conditions such as permission/Nitro/DM-only) | Source URL |
|---|---|---|
| Game Overlay | Windows 10/11 only; does not work on macOS or Linux. User Settings > Overlay > Enable Overlay. Default toggle Shift+`. | https://support.discord.com/hc/en-us/articles/217659737 |
| New overlay (widgets) | Rebuilt without injecting into the game, so it supports more games. Movable / resizable widgets, an action bar (voice/video controls, one-click Go Live, join voice), watch streams, Soundboard, notifications. (2025) | https://discord.com/blog/player-release-q12025 ; https://discord.com/blog/discord-update-march-25-2025-changelog |
| Overlay text chat / notifications | "Messages" toggle. Messages from muted channels and during Streamer Mode are hidden. Typing is blocked in compatibility-mode games, with a notice (2026). | https://support.discord.com/hc/en-us/articles/217659737 ; https://discord.com/blog/discord-patch-notes-september-8-2026 |
| Pin widgets | Pin icon shows Notifications / Voice / Video / Activity / Streams widgets in-game. | https://support.discord.com/hc/en-us/articles/217659737 |
| Overlay call handling | Answer with the keybind, or hover > Join Call / Decline. | https://support.discord.com/hc/en-us/articles/217659737 |
| Streamer Mode | Toggle; auto enable/disable when OBS or XSplit is detected; keybind; top banner with Disable / x. | https://support.discord.com/hc/en-us/articles/218485407 |
| Streamer Mode: Hide Personal Information | Hides emails, connections and tags (display name still shown). | https://support.discord.com/hc/en-us/articles/218485407 |
| Streamer Mode: Hide Instant Invite Links | Hides invite codes, for example in Server Settings > Invites. | https://support.discord.com/hc/en-us/articles/218485407 |
| Streamer Mode: Disable Sounds | Mutes join and notification sounds. | https://support.discord.com/hc/en-us/articles/218485407 |
| Streamer Mode: Disable Notifications | Blocks desktop notifications. | https://support.discord.com/hc/en-us/articles/218485407 |
| Streamer Mode settings location | Streamer Mode page merged into the consolidated Appearance / Accessibility / Developer pages. (2026) | https://discord.com/blog/discord-patch-notes-may-4-2026 |

### Appearance, themes and accessibility

| Feature | Detail (states, dialogs, conditions such as permission/Nitro/DM-only) | Source URL |
|---|---|---|
| Base themes | Desktop: Light (optional Dark Sidebar), Ash, Dark, Onyx, Sync with computer. Ash is the old dark theme and Onyx is OLED black. (2025) | https://support.discord.com/hc/en-us/articles/207260127 ; https://discord.com/blog/player-release-q12025 |
| Mobile-only themes | Midnight on mobile. Mobile Dark returned (2026). | https://support.discord.com/hc/en-us/articles/207260127 ; https://discord.com/blog/discord-update-september-25-2026 |
| Sync across clients | "Sync across clients" / "Sync Themes Across My Devices". | https://support.discord.com/hc/en-us/articles/207260127 ; https://discord.com/blog/discord-patch-notes-may-4-2026 |
| Nitro color themes | Preset gradient themes. Non-Nitro users can try them via "Preview Theme". | https://support.discord.com/hc/en-us/articles/207260127 |
| Nitro custom theme | Dark or light palette, up to 5 gradient colors, Gradient Direction, Color Intensity, Surprise Me, Reset, share themes. (2025) | https://support.discord.com/hc/en-us/articles/207260127 ; https://discord.com/blog/discord-update-september-25-2025-changelog |
| Server Themes | Experimental: servers with 3 boosts set a theme; members opt in. | https://support.discord.com/hc/en-us/articles/207260127 |
| UI Density | Compact / Default / Spacious. (2025) | https://support.discord.com/hc/en-us/articles/207260127 |
| Message display | Default (cozy) / Compact. | https://support.discord.com/hc/en-us/articles/207260127 |
| Zoom level | Slider. Cmd + / Cmd − adjust zoom. | https://support.discord.com/hc/en-us/articles/207260127 |
| Chat Font Scaling | Chat text size only. | https://support.discord.com/hc/en-us/articles/207260127 |
| Message Group Spacing | Slider. | https://support.discord.com/hc/en-us/articles/207260127 |
| Accessibility: Saturation | Slider. | https://support.discord.com/hc/en-us/articles/1500010454681 |
| Accessibility: High Contrast Mode | Desktop. (Sync Contrast Settings is Windows only.) Chat backgrounds respect contrast (2026). | https://support.discord.com/hc/en-us/articles/1500010454681 ; https://discord.com/blog/discord-patch-notes-july-7-2026 |
| Accessibility: Role Colors | In names / next to names / off. | https://support.discord.com/hc/en-us/articles/1500010454681 |
| Accessibility: Reduced Motion | Toggle. | https://support.discord.com/hc/en-us/articles/360040613412 |
| Accessibility: GIF / emoji / sticker animation | Autoplay GIFs when focused, play animated emoji, stickers (always / on interaction / never). | https://support.discord.com/hc/en-us/articles/1500010454681 |
| Accessibility: Sync Profile Themes | Profile cards follow the client theme. | https://support.discord.com/hc/en-us/articles/1500010454681 |
| Accessibility: Underline links | "Underline all links" option. | https://discord.com/blog/discord-patch-notes-september-8-2026 |
| Text-to-Speech | `/tts` messages. Notifications > Text-to-Speech: For all channels / For current selected channel / Never. Accessibility toggle for playing your own /tts. TTS rate slider with Preview. Uses OS voices. | https://support.discord.com/hc/en-us/articles/212517297 ; https://support.discord.com/hc/en-us/articles/1500010454681 |
| Spellcheck | Uses the OS spellchecker (macOS, Windows 10+). Right-click a text box to toggle, "Add to Dictionary", spelling suggestions (2026). | https://support.discord.com/hc/en-us/articles/360000877191 ; https://discord.com/blog/discord-patch-notes-july-7-2026 |
| Custom app icon | Nitro / Nitro Basic. Appearance > App Icon. On desktop it changes only the in-app icon, not the Dock or taskbar. Preview App Icon. (2023) | https://support.discord.com/hc/en-us/articles/17503907209239 |

### Window, desktop shell and refresh

| Feature | Detail (states, dialogs, conditions such as permission/Nitro/DM-only) | Source URL |
|---|---|---|
| 2025 desktop refresh | New colors and illustrations, resizable channel list, Inbox moved into the title bar, consolidated call bar, new themes and density. (2025) | https://discord.com/blog/discord-update-march-25-2025-changelog ; https://discord.com/blog/player-release-q12025 |
| Back / Forward buttons | Top-left of the app; mouse side buttons work too. (2026) | https://discord.com/blog/discord-update-march-24-2026-changelog |
| Settings redesign | Ongoing since late 2025. Settings search kept. "Activity" renamed "Games & Apps", "Content & Social" renamed "Messaging Permissions". Appearance, Accessibility, Chat, Streamer Mode and Advanced merged into Appearance / Accessibility / Developer. (2025–2026) | https://discord.com/blog/discord-update-november-6-2025-changelog ; https://discord.com/blog/discord-update-august-11-2026-changelog ; https://discord.com/blog/discord-patch-notes-may-4-2026 |
| What's New / Changelog | Settings > More > Changelog (desktop); Settings > What's New (mobile). (2025) | https://discord.com/blog/discord-update-november-6-2025-changelog |
| Updater indicator | "Downloading Update" icon in the title bar. Linux gained the self-updater (2026). | https://discord.com/blog/discord-patch-notes-april-6-2026 ; https://discord.com/blog/discord-patch-notes-may-4-2026 |
| Pop-out windows | Voice call, individual tiles, streams, Stage and Activities can be popped out, with Stay On Top. | https://support.discord.com/hc/en-us/articles/360041721052 ; https://discord.com/blog/discord-patch-notes-april-6-2026 |
| Auto-launch at startup | Windows auto-launch opens quietly without stealing game focus (2026). macOS option unconfirmed. | https://discord.com/blog/discord-patch-notes-july-7-2026 |
| Minimize to Tray / Start Minimized | User Settings > Windows Settings > Minimize to Tray (X closes to tray). Start Minimized also exists. Windows only. (unconfirmed) | https://support.discord.com/hc/en-us/community/posts/360058097712 |
| Help | Cmd+Shift+H "Get Help". Support form at dis.gd/contact. | https://support.discord.com/hc/en-us/articles/225878307 ; https://support.discord.com/hc/en-us/articles/360045138471 |
| Download apps | Desktop download page; web client at discord.com/login. | https://support.discord.com/hc/en-us/articles/360033931551 |
| Multiple processes / GPU process | Electron multi-process; Electron 42 in 2026. | https://support.discord.com/hc/en-us/articles/225733808 ; https://discord.com/blog/discord-update-august-11-2026-changelog |

### Notices, states, input and misc

| Feature | Detail (states, dialogs, conditions such as permission/Nitro/DM-only) | Source URL |
|---|---|---|
| Streamer Mode banner | Bar above the app with Disable and x. | https://support.discord.com/hc/en-us/articles/218485407 |
| Voice connection state notices | See Voice: RTC Connecting / No Route / Waiting for Voice Server. | https://support.discord.com/hc/en-us/articles/115001310031 |
| Krisp auto-off notice | Shown when high CPU disables Krisp. | https://support.discord.com/hc/en-us/articles/360040843952 |
| Upload failure handling | Specific error messages; retries transient errors; refreshes stale upload URLs. Empty-file error. (2026) | https://discord.com/blog/discord-patch-notes-may-4-2026 ; https://discord.com/blog/discord-patch-notes-july-7-2026 |
| Rate-limit / verification / connection-lost banners | Not found in official sources. (unconfirmed; no source found) | — |
| Drag and drop files | Drop files onto the window to upload, with an upload preview (comment, alt text via Modify Attachment). Free limit raised from 10 MB to 20 MB (2026). | https://support.discord.com/hc/en-us/articles/211866427 ; https://discord.com/blog/discord-patch-notes-september-8-2026 |
| Accessible drag and drop | Reorder servers and channels: Cmd+D lift, arrow keys move, Space/Enter drop, Esc cancel. | https://support.discord.com/hc/en-us/articles/4408877527703 |
| Context menus | Right-click on messages, users, channels and servers (Copy Server Info submenu). Shift+F10 in the composer. Cut/Copy/Paste in the chat input. Media viewer Copy Image (WebP/AVIF). | https://discord.com/blog/discord-patch-notes-september-8-2026 ; https://discord.com/blog/discord-patch-notes-july-7-2026 ; https://discord.com/blog/discord-patch-notes-may-4-2026 |
| Stats for Nerds | Right-click the video player > Stats for Nerds (codec, bitrate, container). (2026) | https://discord.com/blog/discord-patch-notes-february-4-2026 |
| Active Now sidebar | Friends page right column with friends' current voice, games and streams. (Details unconfirmed.) | https://discord.com/blog/discord-patch-notes-september-8-2026 |
| Game library | Removed from the desktop app. (unconfirmed; no source found) | — |
| Game detection / Registered Games | Add it! for unrecognized games; manually added games show as activity. | https://support.discord.com/hc/en-us/articles/360040816151 ; https://discord.com/blog/discord-patch-notes-july-7-2026 |
| Developer Mode | Toggle under Advanced (now the Developer page). Adds Copy User ID / Server ID / Channel ID and Copy Message Link. | https://support.discord.com/hc/en-us/articles/206346498 ; https://discord.com/blog/discord-patch-notes-may-4-2026 |
| Developer Mode extras | Copy Role ID, Application Test Mode. (unconfirmed; not in the cited article) | https://support.discord.com/hc/en-us/articles/206346498 |
| Mark as read | Esc (channel), Shift+Esc (server); server right-click Mark As Read also clears forum posts; Inbox "Mark Inbox As Read"; Cmd+Shift+E marks the top inbox item read. | https://support.discord.com/hc/en-us/articles/225878307 ; https://support.discord.com/hc/en-us/articles/360045027712 ; https://discord.com/blog/discord-patch-notes-september-8-2026 |
| Mark unread | Focused message: Alt+Enter. Option-click on a message is unconfirmed. | https://support.discord.com/hc/en-us/articles/1500000056121 |
| Inbox | Cmd+I. Tabs: Mentions (filters: @everyone, @role, all servers), Unreads (react and play media inline, "View all unreads"). Jump to message. | https://support.discord.com/hc/en-us/articles/360045027712 ; https://discord.com/blog/discord-patch-notes-july-7-2026 |
| Vibing Wumpus 2.0 | Easter egg: Cmd+Option+Shift+W. (2026) | https://discord.com/blog/discord-patch-notes-february-4-2026 |

## Nitro, monetization & extras

### Nitro and Nitro Basic

| Feature | Detail (states, dialogs, conditions such as permission/Nitro/DM-only) | Source URL |
|---|---|---|
| Plans | Nitro, Nitro Basic (monthly/yearly); Nitro Classic credits only. | https://support.discord.com/hc/en-us/articles/115000435108 |
| Upload limits | Free 20 MB (since Aug 2026), Basic 50 MB, Nitro 1 GB. (2026) | https://support.discord.com/hc/en-us/articles/25444343291031 |
| Custom/animated emoji everywhere | Nitro and Basic. | https://support.discord.com/hc/en-us/articles/115000435108 |
| Custom stickers everywhere | Nitro and Basic. | https://support.discord.com/hc/en-us/articles/115000435108 |
| Super Reactions | Unlimited for Nitro/Basic/Classic. | https://support.discord.com/hc/en-us/articles/12102061808663 |
| HD streaming | Nitro. | https://support.discord.com/hc/en-us/articles/115000435108 |
| 2 Server Boosts + 30% off | Nitro only. | https://support.discord.com/hc/en-us/articles/360028038352 |
| Profile customization | Animated avatar, banner, profile theme; per-server profiles. | https://support.discord.com/hc/en-us/articles/115000435108 |
| Nitro badge | Tenure. | https://support.discord.com/hc/en-us/articles/360035962891 |
| Custom video backgrounds | Nitro and Basic. | https://support.discord.com/hc/en-us/articles/115000435108 |
| 200 servers | Nitro. | https://support.discord.com/hc/en-us/articles/115000435108 |
| 4,000-char messages | Nitro. | https://support.discord.com/hc/en-us/articles/115000435108 |
| Color & custom themes | Nitro. | https://support.discord.com/hc/en-us/articles/207260127 |
| Soundboard anywhere / entrance sounds | Nitro. | https://support.discord.com/hc/en-us/articles/12612888127767 |
| App icon | Nitro and Basic. | https://support.discord.com/hc/en-us/articles/17503907209239 |
| Display name styles | Nitro (2025). | https://support.discord.com/hc/en-us/articles/33833879643927 |
| Shop member pricing | Nitro. | https://support.discord.com/hc/en-us/articles/17162747936663 |
| Extended Quest rewards | Nitro (2025). | https://support.discord.com/hc/en-us/articles/29790581779735 |
| Monthly Orbs drop | 250 Orbs/month (Dec 2025). | https://support.discord.com/hc/en-us/articles/37097755077015 |
| 1.2x Orbs multiplier | From May 8 2026. | https://support.discord.com/hc/en-us/articles/30593690165783 |
| Nitro call tile colors | Match profile color (2026). | https://support.discord.com/hc/en-us/articles/42383370736023 |
| Share Nitro | Up to 3 two-week trials. | https://support.discord.com/hc/en-us/articles/22567595858327 |
| Nitro Squad | Experiment: up to 6 accounts (AU/FR desktop). (2025) | https://support.discord.com/hc/en-us/articles/36320645875479 |
| Nitro Credits | Orbs-exclusive Nitro time. | https://support.discord.com/hc/en-us/articles/26362805097623 |

### Server Boosting

| Feature | Detail (states, dialogs, conditions such as permission/Nitro/DM-only) | Source URL |
|---|---|---|
| Level 1 (2 boosts) | 100 emoji, 15 stickers, 24 sounds, 128 kbps, 720p60, invite background, animated icon. | https://support.discord.com/hc/en-us/articles/360028038352 |
| Level 2 (7 boosts) | 150 emoji, 30 stickers, 36 sounds, 256 kbps, 1080p60, 50 MB uploads, role icons, banner. | https://support.discord.com/hc/en-us/articles/360028038352 |
| Level 3 (14 boosts) | 250 emoji, 60 stickers, 48 sounds, 384 kbps, 100 MB uploads, custom invite link, animated banner. | https://support.discord.com/hc/en-us/articles/360028038352 |
| Additional perks | Server Tags (3), Enhanced Role Styles (3), Server Themes (3, exp.), 250 MB uploads (5, exp.), Game Server Hosting (3+). | https://support.discord.com/hc/en-us/articles/360028038352 |
| Booster badge / role / transfer | Evolving badge, Server Booster role, 7-day transfer cooldown. | https://support.discord.com/hc/en-us/articles/360028038352 |

### Shop and collectibles

| Feature | Detail (states, dialogs, conditions such as permission/Nitro/DM-only) | Source URL |
|---|---|---|
| Shop entry points | Desktop: Shop tab next to DM list / right of server list; from Edit Profile cosmetic pickers; mobile: profile > Shop icon. Mobile Shop got gifting, nameplates, bundles (Sep–Nov 2025). | https://support.discord.com/hc/en-us/articles/17162747936663 ; https://discord.com/blog/discord-patch-notes-september-3-2025 ; https://discord.com/blog/discord-update-november-6-2025-changelog |
| Shop search, sort and filters | Search and filtering (Oct 2025); Browse tab with Sort By / Show Filters (desktop experiment); "Clear Filters". (2025) | https://discord.com/blog/discord-patch-notes-october-7-2025 ; https://support.discord.com/hc/en-us/articles/17162747936663 ; https://discord.com/blog/discord-patch-notes-june-4-2026 |
| Avatar Decorations | Animated ring around avatar; bought or earned (Quests, Orbs); kept permanently; DISXCORE items need active Nitro; color variants. | https://support.discord.com/hc/en-us/articles/13410113109911 ; https://discord.com/blog/discord-patch-notes-march-6-2026 |
| Profile Effects | Animated overlay on profile view; DISXCORE effects Nitro-only; respects Reduced Motion. | https://support.discord.com/hc/en-us/articles/17828465914263 |
| Nameplates | Background design behind name in member/DM lists; buy on desktop/browser (mobile apply only originally; mobile purchase since Sep 2025). (2025) | https://support.discord.com/hc/en-us/articles/30408457944215 ; https://discord.com/blog/discord-patch-notes-september-3-2025 |
| Profile Frames | Border around full profile; Nitro Early Access; Orbs purchase rolling out. (2026) | https://support.discord.com/hc/en-us/articles/40775065582615 ; https://discord.com/blog/discord-update-august-11-2026-changelog |
| Bundles | Discounted sets; buying one item voids the bundle discount. | https://support.discord.com/hc/en-us/articles/17162747936663 |
| Nitro member pricing / member-only items | Nitro-only discounts and items; buying within 5 days of Nitro start voids Nitro refund. | https://support.discord.com/hc/en-us/articles/17162747936663 ; https://support.discord.com/hc/en-us/articles/30408457944215 |
| Wishlist | Add Shop items (not owned, not Orb-exclusive, not bundles) to profile Wishlist tab; "Public Wishlist" toggle; friends gift from it; mobile wishlists (Jun 2026). (2025–2026) | https://support.discord.com/hc/en-us/articles/36288192746903 ; https://discord.com/blog/discord-update-june-25-2026-changelog |
| Game Shops | Buy/gift in-game items (Marvel Rivals, Rust, Bloons TD 6) via Shop > Game Shops or a game's official server "Game Shop" tab; account linking; desktop/browser. (2026) | https://support.discord.com/hc/en-us/articles/40102783004311 ; https://discord.com/blog/discord-update-march-24-2026-changelog |
| Forward a Shop item | Share item link into chats. | https://discord.com/blog/discord-patch-notes-august-4-2026 |
| Localized pricing | Local currency where available, else USD. | https://support.discord.com/hc/en-us/articles/17162747936663 ; https://support.discord.com/hc/en-us/articles/4407269525911 |
| Refund rules | Collectibles non-refundable (refund policy). | https://support.discord.com/hc/en-us/articles/17162747936663 ; https://support.discord.com/hc/en-us/articles/360012668071 |
| Profile badges | Nitro, Server Booster (evolving), Quests, Orbs, Legacy Username, Bug Hunter, legacy HypeSquad/Early Supporter, experimental Gifting/Account Age badges. | https://support.discord.com/hc/en-us/articles/360035962891 |

### Orbs

| Feature | Detail (states, dialogs, conditions such as permission/Nitro/DM-only) | Source URL |
|---|---|---|
| Orbs currency | Virtual currency; can't be bought, gifted, transferred or cashed out; doesn't expire. (2025) | https://support.discord.com/hc/en-us/articles/30593690165783 |
| Earning Orbs | Orb-reward Quests; Nitro monthly drop (250); Orb Challenges (desktop experiment); promos; Riot/Battle.net linking offers; Bloons TD 6 purchase offer. | https://support.discord.com/hc/en-us/articles/30593690165783 ; https://support.discord.com/hc/en-us/articles/37097755077015 ; https://support.discord.com/hc/en-us/articles/40102783004311 |
| Orbs balance display | Top-right of Quest Home and Shop (desktop); mobile avatar > Orbs Balance. | https://support.discord.com/hc/en-us/articles/30593690165783 |
| Orbs Exclusives tab | Nitro credits, Orbs badge, exclusive decoration/effect; desktop/browser only. | https://support.discord.com/hc/en-us/articles/30593690165783 ; https://support.discord.com/hc/en-us/articles/17162747936663 |
| Dual price display | Orb-eligible items show money and Orbs price; Orbs first when affordable; not for gifts, Collabs, subscriptions. | https://support.discord.com/hc/en-us/articles/17162747936663 |

### Quests

| Feature | Detail (states, dialogs, conditions such as permission/Nitro/DM-only) | Source URL |
|---|---|---|
| Quest Home | Discover > Quests (desktop; keyboard reachable Oct 2026); mobile Settings > Quests. | https://support.discord.com/hc/en-us/articles/22225719947543 ; https://discord.com/blog/discord-patch-notes-october-6-2026 |
| Quest bar | Hover/expand in DM list; "Accept Quest" / "Start Video Quest" / "Claim Reward". | https://support.discord.com/hc/en-us/articles/22225719947543 |
| Play Quests | Play X minutes on desktop, stream to a friend in voice, or link account and complete in-game action (up to 36 h). Not on Linux. | https://support.discord.com/hc/en-us/articles/22225719947543 |
| Video Quests | Watch trailer in desktop/browser; pauses when window unfocused. | https://support.discord.com/hc/en-us/articles/22225719947543 |
| Cloud Play Quests | Experimental cloud-streamed game quests. | https://support.discord.com/hc/en-us/articles/35372187686295 ; https://support.discord.com/hc/en-us/articles/22225719947543 |
| Quest rewards | Avatar decorations (2-month unless Nitro), Orbs, partner in-game codes; Quests badge. | https://support.discord.com/hc/en-us/articles/22225719947543 ; https://support.discord.com/hc/en-us/articles/29790581779735 |
| Teen quest cap | Ages 13–17: 3 Quests per day. | https://support.discord.com/hc/en-us/articles/22225719947543 |
| Quest promo controls | "Hide This" per promotion; personalized promos via Data & Privacy. | https://support.discord.com/hc/en-us/articles/22225719947543 |

### Gifting and codes

| Feature | Detail (states, dialogs, conditions such as permission/Nitro/DM-only) | Source URL |
|---|---|---|
| Gift Nitro | Gift icon right of chat box (desktop) or Nitro page; Nitro/Nitro Basic, monthly/yearly; Basic not giftable from chat on desktop. | https://support.discord.com/hc/en-us/articles/360020877112 |
| Gift customization | DM gifts: card design, personal note, soundboard effect, emoji confetti. | https://support.discord.com/hc/en-us/articles/360020877112 |
| Gift links and claiming | Gifts in group chats/channels create a claimable link; embed "Open gift" → "Accept gift"; outside Discord "I accept" on correct account; claimed state shows grayed "Claimed"; claimed subscription appears in Subscriptions. | https://support.discord.com/hc/en-us/articles/360020877112 ; https://support.discord.com/hc/en-us/articles/1500001829622 |
| Gift Shop items | Gift icon next to Buy; recipient can't already own item; duplicates return in 48 h. | https://support.discord.com/hc/en-us/articles/17162747936663 |
| Gift from Wishlist | DM chat-bar gift icon opens "Send a Gift". | https://support.discord.com/hc/en-us/articles/36288192746903 |
| Bulk gifting | Desktop experiment. | https://support.discord.com/hc/en-us/articles/360020877112 |
| Gift Inventory | Unclaimed gifts; revoke/regenerate links. | https://support.discord.com/hc/en-us/articles/360020877112 |
| Redeem Codes | 19-character codes in Gift Inventory. | https://support.discord.com/hc/en-us/articles/360020564851 |
| Seasonal gifting promos | e.g., Nitro Halloween Gifting 2026 free decoration. (2026) | https://support.discord.com/hc/en-us/articles/43599516624407 |

### Apps, App Directory and premium app subscriptions

| Feature | Detail (states, dialogs, conditions such as permission/Nitro/DM-only) | Source URL |
|---|---|---|
| User-installable apps | "Add App" to account; usable in all servers, DMs, GDMs; visible only to installer; non-ephemeral replies limited where user can't send messages. | https://support.discord.com/hc/en-us/articles/21334461140375 ; https://docs.discord.com/developers/resources/application |
| Server-installed apps | Require Manage Server; appear as members. | https://support.discord.com/hc/en-us/articles/21334461140375 |
| App Launcher | Button by chat bar: Recents, Collections, search; voice-call launcher for Activities. | https://support.discord.com/hc/en-us/articles/21334461140375 |
| App Directory | Desktop/browser; Discover sidebar "Apps" page and Server Settings > App Directory. | https://support.discord.com/hc/en-us/articles/21334461140375 ; https://discord.com/blog/discord-patch-notes-october-6-2026 |
| Slash & context-menu commands | "/" picker; right-click > Apps. | https://support.discord.com/hc/en-us/articles/21334461140375 |
| Activities | Launch in chat or voice; PiP/pop-out window (2025). | https://support.discord.com/hc/en-us/articles/21334461140375 ; https://discord.com/blog/discord-update-september-25-2025-changelog |
| App labels | "In-App Purchases", "Ad-Supported", "Partner", "Promoted". | https://support.discord.com/hc/en-us/articles/21334461140375 |
| Premium App subscriptions | User or guild subscriptions; durable and consumable one-time purchases; manage in Subscriptions ("Manage", "Cancel my Membership", "Switch Plan", "Re-Subscribe"). | https://support.discord.com/hc/en-us/articles/26729692307351 ; https://docs.discord.com/developers/monetization/overview |
| App DMs | DM an app; per-app mobile push for app DMs (unconfirmed). | https://discord.com/blog/discord-patch-notes-june-4-2026 ; https://docs.discord.food/resources/user-settings-proto |
| Image-editing apps | App Launcher on an image (rolling out). | https://support.discord.com/hc/en-us/articles/21334461140375 |

## Appendix: 2024–2026 feature timeline (official changelogs and patch notes)

A cross-check list of user-facing additions announced in Discord's own changelog and patch-notes posts. Use it to spot parity items the area tables may underweight. Platform is given only where the post states one.

| Feature | Detail (states, dialogs, conditions such as permission/Nitro/DM-only) | Source URL |
|---|---|---|
| Polls + Create Polls permission (2024-05) | Polls created via "+" in chat. New server permission. | https://discord.com/blog/discord-update-may-13-2024-changelog |
| Activities in text channels (2024-05) | Start Activities without joining voice (desktop "+" button). | https://discord.com/blog/discord-update-may-13-2024-changelog |
| Mutual servers/friends in profile popout (2024-05) | Desktop. | https://discord.com/blog/discord-update-may-13-2024-changelog |
| Recommended Activities (2024-06) | Rocketship button in voice suggests Activities. | https://discord.com/blog/discord-update-june-20-2024-changelog |
| Nitro friend passes (2024-06) | Give up to 3 two-week passes. | https://discord.com/blog/discord-update-june-20-2024-changelog |
| Shop bundles (2024-06) | Decoration + Effect bundles. | https://discord.com/blog/discord-update-june-20-2024-changelog |
| Profile previews in DMs (2024-06) | DM view shows a profile preview. | https://discord.com/blog/discord-update-june-20-2024-changelog |
| Color profile preservation (2024-06) | Uploaded images keep color profile data. | https://discord.com/blog/discord-update-june-20-2024-changelog |
| Activities in DMs/GDMs text chat (2024-07) | | https://discord.com/blog/discord-update-july-26-2024-changelog |
| User-installable apps (2024-07) | Apps added to your account, usable everywhere. | https://discord.com/blog/discord-update-july-26-2024-changelog |
| Desktop profile redesign (2024-07) | Quick actions; status shown as a thought bubble above the avatar. | https://discord.com/blog/discord-update-july-26-2024-changelog |
| Member list activity (2024-08) | Member list shows recent games and music. Desktop. | https://discord.com/blog/discord-update-august-28-2024-changelog |
| Resizable picture-in-picture call window (2024-08) | Desktop. | https://discord.com/blog/discord-update-august-28-2024-changelog |
| App Launcher (2024-09) | Find and use apps in any chat or call. | https://discord.com/blog/discord-update-september-26-2024-changelog |
| In-chat image editing with apps (2024-09) | App Launcher button on hovered images. | https://discord.com/blog/discord-update-september-26-2024-changelog |
| E2EE audio/video (DAVE) (2024-09) | DMs, GDMs, voice channels, Go Live; verify encryption in a call. | https://discord.com/blog/discord-update-september-26-2024-changelog |
| Passkey sign-in (2024-09) | Biometric sign-in. | https://discord.com/blog/discord-update-september-26-2024-changelog |
| Message forwarding (2024-11) | Hover > Forward. | https://discord.com/blog/discord-update-november-18-2024-changelog |
| Recently played games on profiles (2024-11) | Can clear a game's history. Desktop. | https://discord.com/blog/discord-update-november-18-2024-changelog |
| TikTok embed playback (2024-11) | Desktop. | https://discord.com/blog/discord-update-november-18-2024-changelog |
| Suggested emoji reactions (2024-11) | Top 3 emoji suggested. | https://discord.com/blog/discord-update-november-18-2024-changelog |
| Overlay widgets (2025-03) | Movable widgets; stream watching, one-click gameplay share, soundboard, notifications in the overlay. Desktop. | https://discord.com/blog/discord-update-march-25-2025-changelog |
| New base themes (2025-03) | Light, Ash, Dark, Onyx. Desktop. | https://discord.com/blog/discord-update-march-25-2025-changelog |
| UI density (2025-03) | Three spacing options. Desktop. | https://discord.com/blog/discord-update-march-25-2025-changelog |
| Inbox moved to title bar (2025-03) | Desktop. | https://discord.com/blog/discord-update-march-25-2025-changelog |
| Centralized call controls (2025-03) | Single bar; green camera button while on. Desktop. | https://discord.com/blog/discord-update-march-25-2025-changelog |
| Resizable channel list (2025-03) | Desktop. | https://discord.com/blog/discord-update-march-25-2025-changelog |
| Ignore users (2025-03) | Hide someone's messages without notifying them. | https://discord.com/blog/discord-update-march-25-2025-changelog |
| Quick Switcher upgrade (2025-04) | Better ranking of channels/DMs. | https://discord.com/blog/discord-update-june-30-2025-changelog |
| Email markdown (2025-05) | `<email>` becomes a mailto link. | https://discord.com/blog/discord-update-june-30-2025-changelog |
| Server Tags (2025-06) | Tag next to your name; click to view, join or apply. | https://discord.com/blog/discord-update-june-30-2025-changelog |
| Avatar history (2025-06) | Last six avatars saved. Desktop. | https://discord.com/blog/discord-update-june-30-2025-changelog |
| Enhanced Role Styles (2025-06) | Glowing gradient role colors (Boost perk). | https://discord.com/blog/discord-update-june-30-2025-changelog |
| Nameplates (2025-06) | Shop cosmetic shown behind names in member/DM lists. | https://discord.com/blog/discord-update-june-30-2025-changelog |
| AV1 video (2025-07) | Attachments and embeds. | https://discord.com/blog/discord-update-september-25-2025-changelog |
| Pins 50→250 + Pin Messages permission (2025-08/09) | | https://discord.com/blog/discord-patch-notes-september-3-2025 |
| Search `authorType:` filter, markdown term highlight, partial-word forum search (2025-09) | | https://discord.com/blog/discord-patch-notes-september-3-2025 |
| Tumblr embeds (2025-08) | | https://discord.com/blog/discord-update-september-25-2025-changelog |
| Refreshed user profiles + stacked activity cards (2025-09) | Desktop profiles show interests and recent games. | https://discord.com/blog/discord-update-september-25-2025-changelog |
| Pop-out Activities (2025-09) | Floating Activity window. Desktop. | https://discord.com/blog/discord-update-september-25-2025-changelog |
| Camera-on sound (2025-09) | | https://discord.com/blog/discord-update-september-25-2025-changelog |
| Custom Themes (2025-09) | Nitro: up to 5 colors with gradients; syncs to mobile. | https://discord.com/blog/discord-update-september-25-2025-changelog |
| Emoji editor + Add Emoji from picker (2025-11) | Crop and resize on upload. | https://discord.com/blog/discord-update-november-6-2025-changelog |
| Per-server Nameplates (2025-11) | | https://discord.com/blog/discord-update-november-6-2025-changelog |
| Voice channel timer (2025-11) | Shows how long a voice channel has been active. Desktop. | https://discord.com/blog/discord-update-november-6-2025-changelog |
| Changelog moved to Settings > More / What's New (2025-11) | Desktop settings redesign begins. | https://discord.com/blog/discord-update-november-6-2025-changelog |
| Discord Checkpoint (2025-12) | Year-in-review recap with avatar decoration reward. (unconfirmed, third-party reports) | https://ts2.tech/en/discord-checkpoint-2025-how-to-see-your-discord-wrapped-recap-global-stats-and-avatar-rewards/ |
| `@time` command (2026-02) | Desktop timestamp helper. | https://discord.com/blog/discord-patch-notes-february-4-2026 |
| Stream zoom/pan (2026-02/03) | Scroll wheel or trackpad. Desktop. | https://discord.com/blog/discord-update-march-24-2026-changelog |
| Stats for Nerds on video (2026-02) | Right-click the video player. | https://discord.com/blog/discord-patch-notes-february-4-2026 |
| Role mention member list (2026-02/03) | Clicking a role mention lists up to 100 members. | https://discord.com/blog/discord-update-march-24-2026-changelog |
| Bypass Slowmode permission (2026-02) | Replaces legacy bypass via Manage permissions after Feb 23 2026. | https://discord.com/blog/discord-patch-notes-february-4-2026 |
| Back/Forward navigation buttons (2026-03) | Top-left; mouse side buttons. Desktop. | https://discord.com/blog/discord-update-march-24-2026-changelog |
| Invite to Voice (2026-03) | Suggests members and close friends. | https://discord.com/blog/discord-update-march-24-2026-changelog |
| Notifications / Voice & Video / Clips / Streamer Mode settings redesign (2026-03) | Desktop. | https://discord.com/blog/discord-update-march-24-2026-changelog |
| Pinned DMs + pinned channels (2026-06) | Pin DMs/GDMs to the top of the DM list; pin channels to the top of the channel list (all servers since July 2026). | https://discord.com/blog/discord-update-june-25-2026-changelog |
| Voice invite previews (2026-06) | Show who is in the channel. | https://discord.com/blog/discord-update-june-25-2026-changelog |
| Game Profile pages (2026-06) | Screenshots, platforms, reviews, popularity, official server. | https://discord.com/blog/discord-update-june-25-2026-changelog |
| Wave prompt after adding a friend (2026-06) | Desktop DM list. | https://discord.com/blog/discord-update-june-25-2026-changelog |
| Forwarding supports @silent (2026-07) | | https://discord.com/blog/discord-patch-notes-july-7-2026 |
| "Use Category/Server Default" notification option (2026-07) | iOS/Android. | https://discord.com/blog/discord-patch-notes-july-7-2026 |
| WYSIWYG bio editor (2026-08) | Desktop. | https://discord.com/blog/discord-update-august-11-2026-changelog |
| @game mention (2026-08) | Desktop composer. | https://discord.com/blog/discord-update-august-11-2026-changelog |
| Private profile (2026-08) | Everyone / smaller servers / friends only. | https://discord.com/blog/discord-update-august-11-2026-changelog |
| Friend request notes (2026-08) | | https://discord.com/blog/discord-update-august-11-2026-changelog |
| Spoiler channels (2026-08) | Members confirm before viewing a channel. | https://discord.com/blog/discord-update-august-11-2026-changelog |
| Frames (2026-08) | Decorative profile border (Shop). | https://discord.com/blog/discord-update-august-11-2026-changelog |
| Settings renames (2026-08) | "Activity" becomes "Games & Apps"; "Content & Social" becomes "Messaging Permissions"; Authorized Apps/Connections and OS pages/keybinds consolidated. | https://discord.com/blog/discord-update-august-11-2026-changelog |
| Free upload limit 10MB→20MB (2026-08) | All platforms. | https://discord.com/blog/discord-patch-notes-august-4-2026 |
| System-wide echo cancellation (2026-09) | Windows. | https://discord.com/blog/discord-update-september-25-2026 |
| New Nitro display-name fonts and Gummy/Prism effects (2026-09) | | https://discord.com/blog/discord-update-september-25-2026 |
| Bulk-leave inactive group DMs (2026-09) | | https://discord.com/blog/discord-update-september-25-2026 |
| Audio-device icons (2026-09) | | https://discord.com/blog/discord-update-september-25-2026 |
| Bio limit 190→300 chars (2026-09) | | https://discord.com/blog/discord-update-september-25-2026 |
| Friend-online notification controls (2026-09) | Frequency or off. | https://discord.com/blog/discord-update-september-25-2026 |
| DM privacy override indicator (2026-10) | "All Servers" control shows per-server overrides. Desktop. | https://discord.com/blog/discord-patch-notes-october-6-2026 |
| Stage PiP + Text-in-Voice together (2026-10) | | https://discord.com/blog/discord-patch-notes-october-6-2026 |
| GIF picker Shift-click inserts URL; right-click copies link (2026-10) | | https://discord.com/blog/discord-patch-notes-october-6-2026 |
