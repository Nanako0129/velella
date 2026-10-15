# velella Selection and Copy Plan (revision 1.1)

Repo: /Users/nanako/side-project/velella, branch `m1`. `docs/plans/markdown.md` still applies: its F5 rendering rules, link confirmation, spoiler and bidi rules, and every K/T/P contract this plan moves to a new renderer. The ground truth is `docs/element-map.md` (captures in git-excluded `refs/`).

The user asked for message copy (2026-10-15: "現在都沒辦法複製對吧？"), had the message-area elements of official Discord mapped against Swiftcord v2 first (E0), and decided:
- the context menu takes official Discord's layout (icons, sections, danger items), made of glass, with Discord's row height ("做成玻璃的，厚度要一樣不只是矮矮的文字行");
- text selection moves to TextKit 2 now ("直接做 NSTextView 版");
- selection crosses messages as in the official client ("跨訊息（跟官方）");
- a selection copies hidden spoiler text, as the official client does (measured), without its stray newline;
- the text work comes first, then the menu;
- N0 builds both a configured NSTextView and a custom TextKit 2 view, and the user picks from the numbers ("N0 兩種都做，看數字決定");
- the copied header follows the official line breaks ("照官方換行").

Revision 1.1 folds in the plan review of rev 1 (REVISE, 7 blockers) and the security review of rev 1 (1 P1, 9 P2, 7 P3, 3 P4); both disposition tables are at the end.

## Envelope

| Field | Value |
|---|---|
| Outcome | Message text can be drag-selected within and across messages; ⌘C copies the selection as displayed text. Right-clicking a message opens a glass menu in Discord's layout with Copy (when there is a selection), Copy Text (the raw content), Copy Message Link and Copy Message ID, plus Copy Link and Open Link on a visible link. |
| How it is seen working | The user selects across two messages with a hidden spoiler, a mention and a timestamp, pastes into a text editor, and compares with the official client's paste of the same range (E0 rows). The user right-clicks a message and a link and compares the menu with `e0-menu-other.png` / `e0-menu-link.png`. |
| Non-goals | The hover action bar, reactions, reply/edit/delete/pin/forward and other menu actions (they join the menu when the features exist), the Shift hover bar, image-attachment menu items, "Search with Google", ⌘A over the timeline, the composer. |
| Traffic added | None by velella. Main may post one E0 sample in the dev thread under the standing authority (E1 below). |
| Order | **N0** spike (throwaway) → main's live checks **U1-U3** and **E1** → the user picks the text view (D6) → plan revision 2 with N1/N2/C1 contracts → reviews → **N1** → **N2** → **C1**, sequentially. |
| Owners | N0: pilotfish:executor (no production change, nothing merges). N1: pilotfish:security-executor (untrusted rendering). N2: pilotfish:executor. C1: pilotfish:security-executor (link opening, pasteboard, focus). |
| External actions by agents | None: no Discord connection, no git push, no publishing, no installs, no launching velella or other GUI apps, no real preferences or Keychain. Live checks are main's or the user's. |
| Checkpoints | One commit per slice after its verifier; main merges into m1 and pushes main (auto-commit workflow). |
| Rollback | Each slice is a merge commit; `git revert -m 1`. |
| Budget | At most 2 executor passes per slice, then main takes over or asks. |
| Stops | N0 shows that neither prototype can hold N0-1, N0-7 or N0-10, or that both are more than 2× slower than today on an N0-6 metric: main brings the options to the user before revision 2. |

## Decisions

| # | Decision | Source |
|---|---|---|
| D1 | Copy Text (menu) = the message's raw `content`, byte for byte; Shift changes nothing | Measured (E0) |
| D2 | ⌘C of a selection = displayed text: formatting removed, mentions as shown (`@` + the raw name), timestamps as shown, link labels as shown, hidden spoiler text included, no newline added after a spoiler. velella's bidi isolates are not in the copy because substituted runs emit their raw text (S-14); the author's own characters stay | Measured (E0) + user + S-14 |
| D3 | A selection that crosses into a message emits its header as two lines, `name` then ` — time` (the displayed header time), as the official client does without the badge line velella doesn't show; then the body. Messages are separated by one newline | User ("照官方換行") |
| D4 | Every copy path writes plain text only: one writer does `clearContents()` then `setString(_:forType: .string)` (`CopyFeedback.copy`); no RTF, RTFD, URL or attributed type | Security (S-6) |
| D5 | Menu row pitch 36 pt (measured at 1× in `e0-menu-other.png`); CSS: label row min-height 32, padding 8, item radius 2, separators 1 pt with 8 above and below; icons as SF Symbols sized from the capture; on a glass panel | User + E0 |
| D6 | The text view is either **A**, a configured `NSTextView(usingTextLayoutManager: true)`, or **B**, a custom `NSView` driving `NSTextLayoutManager` + `NSTextContentStorage` itself. N0 builds both; the user picks from N0's report | User |

## N0 — Spike (no merge)

**Goal:** answer every question below for both A and B, with numbers. **Owner:** pilotfish:executor in a worktree; nothing from it merges. **Files:** a throwaway `Sources/VelellaUI/Spike/` folder and throwaway `Tests/VelellaUITests/Spike*Tests.swift`; everything else is read-only.

**Harness.** Every measurement runs under `swift test` in an offscreen borderless `NSWindow` hosting an `NSHostingView` (the precedent is `MarkdownTests.swift:494-495`), never a launched app. Pixels are read with `cacheDisplay(in:to:)` on the hosting view. Mouse input is synthesized as `NSEvent`s sent to that window; where a tracking loop cannot be driven that way, the report says so and the case moves to main's live check U3.

**Release run.** `swift test -c release -Xswiftc -enable-testing --filter Spike`. If SwiftPM rejects it, the report records the error and runs every timing in debug instead, with the same N on every side.

**Corpus for N0-6** (built in the test from `MessageItem`; 200 rows in this repeating order):

| Kind | Rows |
|---|---|
| One-line plain text | 60 |
| Two to four lines with a user mention, a custom emoji (fixture image) and an `f` timestamp | 40 |
| Heading, subtext, quote, nested ordered and bullet list, trailing break | 30 |
| Inline hidden spoilers, plus one spoiler around a code block | 20 |
| Code block of 12 highlighted Python lines | 20 |
| Plain and masked links | 20 |
| Image attachment (fixture) | 10 |

| ID | Question | Measure | Pass |
|---|---|---|---|
| N0-1 | TextKit 2 holds | A: an observer of `NSTextView.willSwitchToNSLayoutManagerNotification` is installed before configuration and stays installed through every other N0 row; list which actions (configuration, layout, selection, `copy:`, `writeSelection`, the N0-10 accessibility calls, `sizeThatFits`, attachments, find, print) fire it. B: no `NSLayoutManager` exists | A: never fires during N0 rows, or the firing actions are listed with a way to block each |
| N0-2 | Body metrics | `sizeThatFits` at proposed widths 320 and 600 for T1's test paragraphs: 22 pt line pitch at 16 pt body, height against today's `ParagraphView` | Within 1 pt |
| N0-3 | Block layout | One message with a heading, subtext, a quote, a nested ordered and bullet list and a trailing break: (a) in one text view without `NSTextBlock`/`NSTextTable`, (b) as one text view per block inside today's SwiftUI block layout. Name the technique for quote bars, list markers and block margins | Height within 1 pt of today's `MarkdownView` for at least one of (a)/(b); N0-1 still holds |
| N0-4 | Code block | 12 wrapped lines at width 320: line-number positions aligned to wrapped first lines, the 18 pt line and 19 pt minimum row; and the technique for the >2000-line plain block that scrolls sideways | Positions and height within 1 pt of `CodeBlockView` |
| N0-5 | Sizing in the stack | The N0-2..4 rows hosted in the `LazyVStack` at column widths 600 and 900, then resized from 900 to 600 | Heights equal the standalone values; `WindowSizingTests` still pass; off-screen rows are not re-laid on a width change (count layouts) |
| N0-6 | Performance | First frame: from assigning the root view to the end of `layoutSubtreeIfNeeded()` + `displayIfNeeded()` on a 1200 × 800 window. Scroll: from the bottom, 20 programmatic scrolls of 400 pt on the enclosing `NSScrollView`, each followed by layout and display; time per step. Median of 7 runs per side: today's SwiftUI rows, A, B. Adversarial rows (S-12), each alone: 3200 `||a||`; 2300 ```` ```x``` ```` blocks; a base plus 16 KiB of combining marks; one 16 KiB line; 64 emoji; alternating RTL and LTR runs | Each metric ≤ 2× today, per side; adversarial times recorded with an absolute bound for N1 |
| N0-7 | Hidden spoilers are never drawn | Which glyph runs are hidden comes from `Segment.covered`. The fragment clips out each hidden run's segment rects before drawing, then fills the cover. Record A's default `linkTextAttributes` and `selectedTextAttributes` on macOS 26. Pixels inside every cover rect, in four states: unselected, selected with the window key, selected with it not key, pointer hovering | Uniform (max channel delta ≤ 2) in all four states; removing the clip makes the test fail |
| N0-8 | Inline images | A custom emoji as an `NSTextAttachment` whose `image` comes from `EmojiImages.loaded` (no file wrapper, contents or cell), sized to the 22 pt box, swapped in when loaded; a copy over it emits `:name:` | Fits the box; copy emits `:name:` |
| N0-9 | Hover tip | Character index under the pointer → the run's tip → a rect for `TooltipOverlay` | Rect within 1 pt of the run |
| N0-10 | Accessibility | On a row holding `||secret||` and `||```secret```||`: walk `accessibilityChildren()` recursively; on every element read value, label, title, `accessibilityString(for:)` and `accessibilityAttributedString(for:)` over the full range, `selectedText` after a selection spanning the spoiler, the number of characters, the visible range and link children. Also: does SwiftUI `.accessibilityLabel` on a representable replace or sit beside its NSView subtree, and is a representable under `.hidden()` in the tree | `secret` appears in no output; the report lists every attribute queried with its output |
| N0-11 | NSTextView defaults (A) | With `isEditable` false and the view first responder: enumerate `NSApp.mainMenu` validation; `validRequestor(forSendType:returnType:)` with and without a selection; `writingToolsBehavior` default; what `menu(for:)`, `quickLook(with:)`, `startSpeaking(_:)`, `takeFindStringFromSelection(_:)`, `performTextFinderAction(_:)` and `print(_:)` do; then the same with the overrides switched on | Report before and after; after: only Copy and Select All validate among text items, `validRequestor` is nil, Writing Tools is `.none`, the menu hook is ours |
| N0-12 | Pasteboard | Native `copy:` through `NSApp.sendAction(#selector(NSText.copy(_:)), to: nil, from: nil)` and `writeSelection(to:types: [.rtf, .rtfd, .URL])`, on a private `NSPasteboard(name:)`, before and after the overrides (A); B's copy path | After: `types == [.string]` exactly |
| N0-13 | Links without `.link` | Storage holds a velella-private attribute with the link or spoiler index and no `.link` and no `URL` value anywhere; a synthesized click resolves the index and calls `LinkRouter.handle`; a drag starting on a link opens nothing | Enumeration finds no `.link`/`URL`; recorder opener called once for a plain link, a confirmation for a masked one, a reveal and no open for a hidden spoiler |
| N0-14 | Hit-testing | Index and segment-frame containment for a point 40 pt past the end of a line ending in a link | The point is outside every segment frame of the link |
| N0-15 | Edits under selection and covers | Select across two rows, replace the second's content with `""` then with a shorter text, ⌘C; render `a ||secret||`, update the same view to `aaaaaaaa ||secret||`, rerun N0-7's pixel check | No trap, only the first row's part copied; the new cover is uniform |
| N0-16 | Selection across rows, with headers | A coordinator prototype over two rows, each with a header participant (the styled name: gradient, glow and design from `displayNameStyle`, at T1 metrics; plus the 12 pt time) and a body. Name the header technique (a text view, or a SwiftUI header taking part in the coordinator). Cases: a drag from row A's body into row B's body; a drag that autoscrolls past the column edge until A is recycled, then ends in B; after each, ⌘C. SwiftUI handlers in the same row still fire over or beside the text: the row's `.onHover` gutter time, a hidden spoiler cover's tap to reveal, the code block's hover Copy button, the header time tooltip. State how each drag was driven | ⌘C gives A's tail, `name`, ` — time`, B's head per D2/D3, also after recycling; each handler fires; anything not drivable is listed for U3 |
| N0-17 | Size | Lines of code of prototype A (with its overrides) and B | Recorded |

**Report:** for each row, A's and B's measured value and pass or fail, the technique chosen, and what TextKit 2 could not do. Main adds U1-U3 and E1 and gives the user the A/B comparison.

### Main's checks after N0

| ID | Check | By |
|---|---|---|
| U1 | On a spike build launched by main: force-click, three-finger tap and ⌃⌘D on a hidden word and on a link; VO-Shift-M on a text view; Edit › Speech with an empty selection; ⌘E then ⌘G in another app; the Services menu over a selection | Main, announced input takeover |
| U2 | The default button of `confirmationDialog` for a link, and whether a held Return from a menu item repeats into it | Main |
| U3 | Every N0-16 drag case the harness could not drive | Main |
| E1 | Official client: does ⌘C of a selection keep the author's U+001B and U+202E? (blobhaj posts a labelled sample in the dev thread) | Main |

## N1 — Message text on TextKit 2 (SECURITY slice), outline

Each message's text renders through the view chosen under D6, in the block layout N0-3 picks; a code block keeps its Vencord chrome with its code in its own text view. Every behaviour of K2-K4, T1, P1 and K6's view carries over: fonts and the 22 pt line box, mention and inline-code fills, link routing through `LinkRouter` (S-3), spoiler covers that never draw hidden glyphs (S-1, S-2, S-10) with click-to-reveal and hidden links inert, custom emoji through `EmojiImages`' allow-list (S-17), timestamp tips and relative refresh, bidi isolation, accessibility that never exposes hidden text (S-4), quote bars, list markers, the trailing-break rule. Selection within a message and ⌘C per D2 and D4, through AppKit's own copy paths (S-6). With A, the defaults in S-5 are off. Tests that drive SwiftUI `Text` today (`renderingATippedSegmentRecordsItsFrame`, `pillFillsAndSpoilerCoversDrawUnderTheRenderer`, `theDrawnParagraphSpeaksItsLabelNotHiddenText`, the MessageLayout geometry tests, the MarkdownBlock pixel tests, `copyShowsOnHoverAndIsAbsentWhileCovered`, the header height tests, the Polish line-height tests) move to the hosted-window pixel method N0 proves, with the same assertions. Contract fixed in revision 2.

## N2 — Selection across messages, outline

A `SelectionCoordinator` per message column keeps the selection as model positions (message id, part, UTF-16 offset), so rows can leave and re-enter the `LazyVStack`. Participants: each row's header (N0-16 technique) and its body parts. It tracks the drag itself, autoscrolls at the column's edges, paints each visible participant's sub-range, clears on a click elsewhere, builds ⌘C per D2-D4, drops a part whose message content changed or left the store, and clamps offsets (S-9). A code block part copies its displayed text or maps through the escape table (S-15). Contract fixed in revision 2.

## C1 — Discord-layout glass menu (SECURITY slice), outline

Right-click anywhere on a message row opens a glass panel at the cursor in Discord's layout (D5): Copy ⌘C (selection only), Copy Text, Copy Message Link, Copy Message ID; on a visible link under the pointer (S-13), Copy Link and Open Link. The menu captures the message id, raw content and `LinkTarget` when it opens and is dismissed by a body change, a channel switch or a scroll (S-8). Open Link calls that row's `LinkRouter.handle(MarkdownText.linkURL(n))`, never the opener. Copy Message Link and Copy Message ID appear only for sent messages whose ids pass `UInt64(_:)`, built from the snapshot's channel and guild (`@me` for a DM) (S-11). The menu is the key responder while open; Escape and dismissal restore the previous responder; no key reaches the composer (S-7); the dismissing click is consumed and menu actions ignore key repeats (S-16). With A, `menu(for:)` and `accessibilityPerformShowMenu` open this menu (S-5). Pasteboard per D4. Contract fixed in revision 2.

## Security dispositions (security review of rev 1, 93f2cb3)

| # | Finding | Disposition |
|---|---|---|
| S-1 (P1) | Today's hiding (SwiftUI-scoped colours, a cover drawn instead of glyphs) does not survive NSTextView: SwiftUI attributes drop, `linkTextAttributes` and `selectedTextAttributes` repaint, the translucent cover shows glyphs | FIX: hidden glyphs are clipped out, never drawn; hidden runs from `Segment.covered`; no foreground colour in link or selection attributes; N0-7 four-state pixel test |
| S-2 (P2) | A TextKit 1 fallback silently drops the fragment drawing | FIX: N0-1 notification observer through every N0 row; N1 tests keep it; hiding also holds without the fragment |
| S-3 (P2) | `.link` brings NSTextView's link defaults (workspace open, styling, URL drag, previews, AX links) | FIX: no `.link` in storage; a private index attribute; clicks call `LinkRouter.handle`; N0-13 |
| S-4 (P2) | Accessibility beyond the value exposes hidden text; `.hidden()` representables | FIX: N0-10 full walk; N1 contract per its result |
| S-5 (P2) | Writing Tools, `AXShowMenu`/`menu(for:)`, Look Up, Speech with no selection, the find pasteboard, the find indicator, print | FIX: N0-11 (A) and U1; editing-only features stay off through `isEditable == false` without per-feature switches |
| S-6 (P2) | Copy must go through AppKit's copy paths; default RTF/RTFD | FIX: D4; N0-12 responder-chain and `writeSelection` tests |
| S-7 (P2) | Return in the menu could reach the composer and send the draft | FIX: C1 focus contract; test with a focused composer holding a draft |
| S-8 (P2) | The link can change between opening the menu and choosing | FIX: C1 captures the target and dismisses on change; test |
| S-9 (P2) | Edits or deletes under a selection crash ⌘C | FIX: N2 drops changed parts and clamps; N0-15 |
| S-10 (P2) | Covers drawn at stale ranges after an edit | FIX: covers from current storage at draw time; storage replaced on change; N0-15 |
| S-11 (P3) | UI ids are Strings; echoes carry nonces | FIX: C1 id rule; test |
| S-12 (P3) | Adversarial cost | FIX: N0-6 adversarial rows with an absolute bound; covers from the fragment's own range |
| S-13 (P3) | Nearest-character hit-testing offers link items off the link | FIX: segment-frame containment; N0-14; C1 test |
| S-14 (P3) | Stripping isolates by scanning deletes the author's own | FIX: D2 (raw text of substituted runs); N1 test |
| S-15 (P3) | Code-block display offsets vs `code` | FIX: N2 rule; test |
| S-16 (P3) | The dismissing click and key repeats | FIX: C1; U2 |
| S-17 (P3) | Emoji attachments | FIX: N0-8 image-only attachments, `:name:` in copies |
| S-18 (P4) | Copies carry the author's invisible controls (paste-jacking) | DEFER to E1: measure the official client, then pin the answer in an N1 test |
| S-19 (P4) | A one-line header lets an RLO in the name reorder the time | Resolved by D3 (the user chose the official line breaks) |
| S-20 (P4) | A bare TextKit 2 view removes most NSTextView defaults by construction | Adopted as D6 option B; the user picks after N0 |
| T5 | Attributed-string importers | Confirmed sufficient by the review |
| T8 | Copied hidden spoiler text | Accepted by the user |

## Plan review dispositions (plan review of rev 1, 93f2cb3)

| # | Blocker | Disposition |
|---|---|---|
| 1 | S3 not falsifiable | FIX: N0-6 corpus, both metrics defined in the hosted window, median of 7 per side, the release command and its fallback, the 2× bar per metric |
| 2 | S2 covered only standalone paragraphs | FIX: N0-3 blocks (one view or one per block), N0-4 code block, N0-5 in-stack sizing and resize with `WindowSizingTests` |
| 3 | S8 missed autoscroll with recycling, SwiftUI handlers and how drags are driven | FIX: N0-16 cases and handlers; synthesized events, else U3 |
| 4 | No header participant | FIX: N0-16 header technique with the styled name; ⌘C includes D3's lines |
| 5 | S7 checked only the value | FIX: N0-10 full accessibility walk |
| 6 | Pixels of AppKit views in tests | FIX: harness reads pixels with `cacheDisplay(in:to:)`; N0-7 test fails without the clip |
| 7 | No security dispositions; Writing Tools missing | FIX: the table above; Writing Tools in N0-11 |

## Deferred items found during E0

| Item | Where |
|---|---|
| `ReplyBar` builds `Text("Replying to \(reply.authorName)")` with interpolation instead of `Text(verbatim:)` and without bidi isolation of the name | `Sources/VelellaUI/MessageArea.swift:350`; P3, fold into the next composer slice |
| Image-attachment menu items (Copy Image, Save Image) | Not captured (no attachment in the thread); with attachments' menu work |
