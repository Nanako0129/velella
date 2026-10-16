# velella Composer Growth, Long Text and Attachments Plan (revision 1.2)

Repo: /Users/nanako/side-project/velella, branch `sel-int` (merged into `m1` with the menu wave). Ground truth:

| Source | Path (git-excluded) | Cited as |
|---|---|---|
| Official composer CSS and JS | `refs/discord-ground-truth/composer.md` | OC |
| Swiftcord v2 composer teardown | `refs/swiftcord-v2-teardown.md`, section "Composer (growth and transitions), 2026-10-17" | SC |
| Swiftcord v2 live recording (main, 2026-10-17) | `scratchpad/composer-live/` (frames, `tops.csv`) | SL |
| Official attachment flow (discovery CU-D, running) | `refs/discord-ground-truth/attachments.md` (to be written) | OA |
| Swiftcord v2 attachment row (discovery CU-D, running) | `refs/swiftcord-v2-teardown.md`, section "Composer attachments, 2026-10-17" | SA |

## Purpose

The user compared velella's one-line composer with Swiftcord v2's growing one (2026-10-17). They asked how many lines Swiftcord grows to, asked for its transitions to be copied in full, and asked that anything Swiftcord lacks but the official client has be done too. Their decisions (AskUserQuestion, 2026-10-17) are recorded as D1-D4 below. The project rule applies: a complete client, with no part left out without an ASK (project `CLAUDE.md`).

## Envelope

| Field | Value |
|---|---|
| Outcome | The composer grows line by line to Swiftcord's limit and then scrolls. Its buttons follow the last line. Reply-bar and attachment-row transitions match Swiftcord. Long text follows the official limits: the counter, a blocked send with the official alert, Enter inside a code block, and a long paste turning into `message.txt`. Files can be attached and sent as the official client does: from the "+" menu, by drag and drop, by paste, and through a long paste. |
| How it is seen working | The user types and pastes multi-line text in velella next to Swiftcord at the same window height, comparing the growth, the limit, the button position and the reply bar's motion. The user crosses the counter threshold and the limit. The user attaches a file by each entry point and sends it to the dev thread, and the official client shows the same message. |
| Non-goals | Autocomplete (mentions, emoji, channels, commands) and its popup; edit mode (menu slice B, which reuses this editor); the send button (the official client hides it behind an accessibility setting; Swiftcord's condition was not found); voice messages; stickers and GIF pickers; slash-command options. |
| Traffic added | Slice CG adds none. Slice CU adds only what the official client sends for an attachment: the upload-slot request, the upload `PUT` to the storage host the slot names, and the message create with `attachments` (shapes in OA). |
| Order | **CG** starts once F1 is merged (done, d536ccd) and its long-text facts are in OC. **CU-D** (discovery) is running now. **CU** (core upload) after CU-D, its contract and a security review. **CUA** (attachment UI) after CG and CU. Menu slice B (edit mode, reply-bar mention toggle) waits for CG: it reuses CG's editor and edits `ReplyBar` after CG (menu.md B's prerequisites list CG). |
| Owners | CG: pilotfish:executor. CU: pilotfish:security-executor (account writes, a new external host). CUA: pilotfish:executor; any request it needs goes through CU. Each slice gets a fresh pilotfish:verifier, and CU gets a pilotfish:security-reviewer before it starts. Slice IDs are prefixed `C` + letter so none collides with menu.md (whose C is Pins). |
| External actions by agents | None: no network, no git push, no GUI apps, no real Keychain or preferences. Live checks are main's or the user's. Main announces input takeover and waits for the user's go. |
| Rollback | `git revert -m 1` per slice. CU's upload can be turned off with one line that hides the entry points. |
| Budget | At most 2 executor passes per slice; after that, main takes over or asks. |
| Stops | Any 401, captcha or unexpected 429 in a live upload; an upload host outside the pattern OA establishes; a request shape that cannot be established without sending a real message. In each case main stops the slice and asks. |

## Decisions

| # | Decision | Source |
|---|---|---|
| D1 | While multi-line, the "+" and the GIF, sticker and emoji glyphs align to the last line (`alignment: .bottom`), as in Swiftcord (SC §4). This differs from the official client, which keeps its buttons sticky on the first line (OC). | User, 2026-10-17 |
| D2 | Height limit: `min(320 pt, window safe-area height × 0.5)`, using 160 pt when the view has no window yet, as in Swiftcord (SC §1). Measured live: about 14.5 Swiftcord lines at a 768 pt window (SL). velella's 13 pt composer font shows a few more lines at the same height. | User, 2026-10-17 |
| D3 | The official long-text behaviour, which Swiftcord lacks, comes in this wave: the 2000 / 4000 limit, the remaining counter, Enter blocked over the limit with the official alert or upsell, and Enter inside an open code block inserting a newline (OC). | User, 2026-10-17 |
| D4 | The official long paste into `message.txt` comes in this wave, and so does attachment upload, which it needs: slices CU and CUA. | User, 2026-10-17 |
| D8 | When the composer grows or shrinks: a message list at its bottom stays at its bottom, the last message fully visible above the composer; a list scrolled up keeps its top visible row where it is. Swiftcord's anchoring rule could not be decoded (SC §3, obfuscated); this is main's decision, the same result a browser gives the official client's list (its scroll offset is kept while the list's height changes) plus staying at the bottom. | Main |
| D9 | Message length is counted in UTF-16 code units (`draft.utf16.count`), as the official `s.length` does (menu-requests.md §4.5), on the draft as sent. | Main, from §4.5 |
| D5 | Typing growth is not animated. Swiftcord has no animation on it (SC §3, SL: each line lands in one frame) and neither does the official client (OC). | SC, OC |
| D6 | The composer's motion uses Swiftcord's one spring, `.spring(response: 0.15, dampingFraction: 0.85, blendDuration: 0.25)`, keyed on the reply target's presence and on the attachment count. The reply bar's transition is `.blurReplace(.downUp)`, the attachment row's is `.move(edge: .bottom)` with opacity, and the placeholder's is `.identity`. Changing channel disables animations for that transaction (SC §3, §6). | SC |
| D7 | The non-Nitro over-limit case (OC L7, `MESSAGE_LENGTH_UPSELL`) opens a modal whose layout is not on disk (OC L8: its chunks return 404 today). Until it is captured, velella shows the official inline upsell copy (OC L6) natively with "Learn more" (opens `https://discord.com/nitro` in the browser, velella has no store) and "Okay". Capturing the real modal needs a non-Nitro account and the user's go; it is listed for the user in this wave's report. | Main, from OC L6-L8 |

## Slices

| ID | Outcome | Prerequisites | Owner |
|---|---|---|---|
| CG | Multi-line composer: growth, limit, keys, transitions, and the long-text rules (D1-D3, D5-D9) | F1 merged; OC long-text rows | executor |
| CU-D | Discovery: the official upload flow, limits, entry points, attachment row, edit modal and errors (OA); Swiftcord's attachment row (SA); screens of the official attachment UI, captured by main after the user's go | none | main, scout, RE agent |
| CU | Core upload: slot request, upload to the storage host, message create with attachments, per-tier and boost size limits, cancellation and errors, typed over FFI. Contract written from OA after CU-D | CU-D, security review | security-executor |
| CUA | Attachment UI: row and cards (Swiftcord's look and motion), "+" → Upload a File, drag and drop with its overlay, paste of files and images, long paste → `message.txt`, the edit modal (filename, description, spoiler), progress / cancel / failure / retry, the ATTACH_FILES gate (F1). Contract after CU's | CG, CU | executor |

## CG — Composer growth, keys, transitions and long text (contract)

| Part | Contract |
|---|---|
| Editor | Replace the `TextField` with a multi-line AppKit editor: an `NSViewRepresentable` around an `NSScrollView` with a TextKit 2 `NSTextView`, plain text only (`isRichText = false`, no automatic quote, dash or link substitution, spelling on), font `T.Typography.composer`, `lineFragmentPadding` 0. The draft binding (`ShellState.draft`) and `ComposerSubmit` stay the source of truth. The single-line look (card height, radius, "+" size, glyphs, padding) is today's, unchanged; only the input becomes multi-line. |
| Height | The content height is the maximum `maxY` over the layout fragments plus the vertical insets, rounded up and clamped between today's single-line height and D2's limit. It is recomputed on text change, width change and window resize. The height change is applied without animation (D5). Past the limit, the text view scrolls with the system overlay scroller and keeps the caret visible. |
| Layout | The input row's `HStack` uses `alignment: .bottom` (D1). The reply bar stays inside the card above the row, as today. |
| Message list | D8. Mechanism: the composer stays in the list's `.safeAreaInset(edge: .bottom)` (MessageArea.swift), so its height reaches the scroll view as the bottom inset; the list keeps `.defaultScrollAnchor(.bottom)` and adds `.defaultScrollAnchor(.bottom, for: .sizeChanges)` so a list at its bottom stays there, and `.scrollPosition(id: $topID, anchor: .top)` keeps the top row when scrolled up. If SwiftUI does not give both results with these modifiers (measured in the tests below), the executor stops and reports before trying another mechanism. No animation (D5). |
| Keys | Return sends through `ComposerSubmit` (never blank, never while the input method holds marked text). Shift-Return inserts a newline. Return inside an open code block (an odd number of fences before the caret) inserts a newline, per the official `hasOpenCodeBlock` (OC). Esc cancels the reply when one is set (SC §7). Option-Return sends, as in both clients. Up on an empty editor is left for menu slice B. |
| Transitions | D6: the reply bar inserts and removes with `.blurReplace(.downUp)` under the spring; the placeholder uses `.identity`; a `.transaction(value: channelID)` clears the animation when the channel changes. |
| Long text | Limit (OC L1): 4000 when `premium_type == 2` (Nitro; only TIER_2 carries INCREASED_MESSAGE_LENGTH), else 2000; Unknown `premium_type` (before READY) counts as 2000. Length in UTF-16 units (D9, OC L9). Counter (OC L2-L5): visible when remaining ≤ limit / 10, or for a 4000 user whose draft passed 2000; text "{count} characters remaining" (remaining > 0), "Message is at max length" (0), "Message is too long" (< 0), never a negative number; colour default, critical below 0; placed at the input's bottom trailing corner (official: absolute, bottom 10, trailing 14), the input reserving one extra line while it shows (`.charCountShowing`); a non-Nitro user also sees the inline upsell line (OC L6) under it. VoiceOver announces "Character limit reached" when the limit is hit (OC L2). Return over the limit (OC L7) sends nothing and shows, for a Nitro user, the alert: title "Your message is too long.", body "Please make your message of {currentLength} characters shorter. We've set the limit at {maxLength} characters to be courteous to others.", button "Okay"; otherwise D7. The draft is never truncated. |
| Accessibility | The editor is a text area for VoiceOver, labelled with the placeholder; the counter is announced when it appears. |
| Tests | Growth: one line, N lines, clamp at D2's limit, for both the window-height and the 320 branches, and shrinking back. Message list, hosted in a real window: at its bottom, grow to the limit, the last message's frame stays fully above the composer; scrolled up, grow, the top visible row's id and its y position are unchanged; shrink back, both hold. Length (D9): a draft of 1001 "👍" against the 2000 limit (2002 UTF-16 units) shows a negative counter and Return sends nothing; 1000 "👍" sends. Keys: Return sends, Shift-Return, the code-block newline (open and closed fence), marked text, Esc. Counter (OC L2-L5): hidden above the threshold, shown at limit/10 remaining for 2000 and 4000, shown past 2000 for a Nitro user; the three strings at remaining > 0, == 0 and < 0; critical colour below 0; the inline upsell only for non-Nitro. Return over the limit sends nothing and shows the OC L7 alert for `premium_type` 2 and the D7 sheet otherwise; Unknown `premium_type` uses 2000. The channel-switch transaction disables animation. Height changes are not animated (no transaction animation on growth). |
| Files | `Sources/VelellaUI/MessageArea.swift` (Composer; `ReplyBar`'s insertion transition and Esc only, menu slice B later adds the mention toggle), a new `Sources/VelellaUI/ComposerEditor.swift`, `ShellState.swift` (message-length limit field), `DesignTokens.swift` (D2 constants, the spring), `Sources/Velella/AppModel.swift` (feeds the limit from `self_state()`), tests in `Tests/VelellaUITests/`. |
| Live (user) | Type and paste multi-line text side by side with Swiftcord at the same window height; at the bottom of a channel, grow to the limit and see the last message stay visible; scroll up, grow, and see the view stay put; reply and cancel; cross 200 remaining and the limit. |

## CU and CUA

Their contracts are written from OA and SA once CU-D returns. They then go through a plan review and a security review before CU starts. The questions OA answers are listed in the CU-D brief: the request flow and shapes, the upload host, limits by tier and boost, entry points, the row and modal, states and errors, and eager versus on-send upload.
