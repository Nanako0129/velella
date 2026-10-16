# velella Selection and Copy Plan (revision 2.1)

Repo: /Users/nanako/side-project/velella, branch `m1`. `docs/plans/markdown.md` still applies: its F5 rendering rules, link confirmation, spoiler and bidi rules, and every K/T/P contract this plan moves to a new renderer. The ground truth is `docs/element-map.md` (captures in git-excluded `refs/`).

The user asked for message copy (2026-10-15: "現在都沒辦法複製對吧？"), had the message-area elements of official Discord mapped against Swiftcord v2 first (E0), and decided:
- the context menu takes official Discord's layout (icons, sections, danger items), made of glass, with Discord's row height ("做成玻璃的，厚度要一樣不只是矮矮的文字行");
- text selection moves to TextKit 2 now ("直接做 NSTextView 版");
- selection crosses messages as in the official client ("跨訊息（跟官方）");
- a selection copies hidden spoiler text, as the official client does (measured), without its stray newline;
- the text work comes first, then the menu;
- N0 builds both a configured NSTextView and a custom TextKit 2 view, and the user picks from the numbers ("N0 兩種都做，看數字決定");
- the copied header follows the official line breaks ("照官方換行").

Revision 1.1 folds in the plan review of rev 1 (REVISE, 7 blockers) and the security review of rev 1 (1 P1, 9 P2, 7 P3, 3 P4). Revision 1.2 folds in the plan review of rev 1.1 (REVISE, 10 blockers on N0's harness); at that review cap the user chose to fix and start N0 without another round (2026-10-15: "修正後直接做 N0"). All disposition tables are at the end.

## Envelope

| Field | Value |
|---|---|
| Outcome | Message text can be drag-selected within and across messages; ⌘C copies the selection as displayed text. Right-clicking a message opens a glass menu in Discord's layout with Copy (when there is a selection), Copy Text (the raw content), Copy Message Link and Copy Message ID, plus Copy Link and Open Link on a visible link. |
| How it is seen working | The user selects across two messages with a hidden spoiler, a mention and a timestamp, pastes into a text editor, and compares with the official client's paste of the same range (E0 rows). The user right-clicks a message and a link and compares the menu with `e0-menu-other.png` / `e0-menu-link.png`. |
| Non-goals | The hover action bar, reactions, edit/delete/pin/forward and other menu actions (they join the menu when the features exist), the Shift hover bar, image-attachment menu items, "Search with Google", ⌘A over the timeline, the composer. (Reply joined in revision 2: track R's sending exists.) |
| Traffic added | C1's Reply sends a message with `message_reference`, which pings the replied author (Discord's default, as the official client). Live acceptance sends one reply on an account the user picks. Main posted one E1 sample in the dev thread (done). |
| Order | N0 (done) → D6 = B (done) → revision 2.1 → **N1** and **C1** in parallel worktrees → **N2** (after N1) → **W1** (after N2 and C1). U1 runs on the N1 build before N1 merges; U2 on the C1 build before C1 merges; U3 is folded into N2's tests and live check. |
| Owners | N1: pilotfish:security-executor. C1: pilotfish:security-executor. N2: pilotfish:executor. W1: pilotfish:security-executor. Each slice gets a fresh pilotfish:verifier. File ownership: the table in Revision 2.1. |
| External actions by agents | None: no Discord connection, no git push, no publishing, no installs, no launching velella or other GUI apps, no real preferences or Keychain. Live checks are main's or the user's. |
| Checkpoints | One commit per slice after its verifier; main merges into m1 and pushes main (auto-commit workflow). |
| Rollback | Each slice is a merge commit; `git revert -m 1`. |
| Budget | At most 2 executor passes per slice, then main takes over or asks. |
| Stops | No prototype is eligible (N0's eligibility rule), or every eligible one is more than 2× slower than today on an N0-6 metric: main brings the options to the user before revision 2. |

## Decisions

| # | Decision | Source |
|---|---|---|
| D1 | Copy Text (menu) = the message's raw `content`, byte for byte; Shift changes nothing | Measured (E0) |
| D2 | ⌘C of a selection = displayed text: formatting removed, mentions as shown (`@` + the raw name), timestamps as shown, link labels as shown, hidden spoiler text included, no newline added after a spoiler. velella's bidi isolates are not in the copy because substituted runs emit their raw text (S-14); the author's own characters stay | Measured (E0) + user + S-14 |
| D3 | A selection that crosses into a message emits its header as two lines, `name` then ` — time` (the displayed header time), as the official client does without the badge line velella doesn't show; then the body. Messages are separated by one newline | User ("照官方換行") |
| D4 | Every copy path writes plain text only: one writer does `clearContents()` then `setString(_:forType: .string)` (`CopyFeedback.copy`); no RTF, RTFD, URL or attributed type | Security (S-6) |
| D5 | Menu row pitch 36 pt (measured at 1× in `e0-menu-other.png`); CSS: label row min-height 32, padding 8, item radius 2, separators 1 pt with 8 above and below; icons as SF Symbols sized from the capture; on a glass panel | User + E0 |
| D6 | The text view is either **A**, a configured `NSTextView(usingTextLayoutManager: true)`, or **B**, a custom `NSView` driving `NSTextLayoutManager` + `NSTextContentStorage` itself. N0 builds both; the user picks among the eligible ones (N0's eligibility rule) from N0's report. **Chosen: B** (user, 2026-10-16, after trying the spike B window: "先試 spike B，沒問題就用 B", then "驗收OK"). N0 (5e521f6 on `worktree-agent-a760539e18d998c61`): both eligible; B ran at 0.85-1.47× today's rows against A's ~2×, 164 lines against 198, no TextKit 1 fallback and none of NSTextView's defaults | User |

## N0 — Spike (no merge)

**Goal:** answer every question below for both A and B, with numbers. **Owner:** pilotfish:executor in a worktree; nothing from it merges. **Files** (all throwaway): a `Sources/VelellaUI/Spike/` folder; `Tests/VelellaUITests/Spike*Tests.swift`; a seam in `ParagraphView.swift` that inserts an image into `EmojiImages.loaded` for a URL (no fetch); a `--spike A|B` launch switch in `Sources/Velella/VelellaApp.swift` that opens a window over the N0-6 corpus with the N0-16 coordinator and no Discord connection. Everything else is read-only.

**Harness.**
- Every measurement runs under `swift test` in an offscreen `NSWindow` subclass hosting an `NSHostingView` (precedent: `MarkdownTests.swift:494-495`), never a launched app.
- **Key state:** the subclass's `canBecomeKey` and `isKeyWindow` are switchable; the test sets the text view first responder with `makeFirstResponder`; every row that depends on the state asserts `isKeyWindow` and the first responder, never assumes them.
- **Copy:** "⌘C" and native copy mean `copy:` sent to an explicit target (the text view or the coordinator), never `NSApp.sendAction(_:to: nil, …)`. Every copy writes to a pasteboard injected by the test (production default `.general`); `NSPasteboard.general.changeCount` is recorded before and after the whole suite and must not change.
- **No system side effects:** no test calls a default that speaks, prints, writes the find pasteboard or opens a panel (`startSpeaking`, `print`, `takeFindStringFromSelection`, `quickLook(with:)`, `performTextFinderAction`, `performFindPanelAction`). Those are observed only through validation results and override hit counters (N0-11); anything that needs a real call goes to U1.
- **No network:** corpus emoji use URLs on a non-Discord host, so `EmojiImages.fetchable` is false and no fetch starts; their images come from the seam, for today's renderer and for A and B alike.
- Pixels are read with `cacheDisplay(in:to:)` on the hosting view. Mouse input is synthesized as `NSEvent`s sent to that window; where a tracking loop cannot be driven that way, the report says so and the case moves to U3.

**Release run.** `swift test -c release -Xswiftc -enable-testing --filter Spike`. If SwiftPM rejects it, the report records the error and runs every timing in debug instead, with the same N on every side.

**Eligibility.** A prototype is eligible for D6 only if it passes N0-1, N0-7, N0-10, N0-12 and N0-13, and for A also N0-11. Each failed eligibility row goes to the user as a named revision-2 blocker. The stop fires when no prototype is eligible; the user picks only among eligible ones.

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
| N0-6 | Performance | First frame: from assigning the root view to the end of `layoutSubtreeIfNeeded()` + `displayIfNeeded()` on a 1200 × 800 window. Scroll: from the bottom, 20 programmatic scrolls of 400 pt on the enclosing `NSScrollView`, each followed by layout and display; a run's scroll value is the total of its 20 steps. One warm-up run per side is discarded; then 7 runs per side, sides alternating within one process (today, A, B, today, …); the report lists the 7 per-run values per side and compares medians. Sides: today's SwiftUI rows, A, B. Adversarial rows (S-12), each alone: 3200 `||a||`; 2300 ```` ```x``` ```` blocks; a base plus 16 KiB of combining marks; one 16 KiB line; 64 emoji; alternating RTL and LTR runs | Each metric ≤ 2× today, per side; adversarial times recorded with an absolute bound for N1 |
| N0-7 | Hidden spoilers are never drawn | Which glyph runs are hidden comes from `Segment.covered`. Layer 1: the fragment clips out each hidden run's segment rects before drawing, then fills the cover. Layer 2: the hidden runs' own attributes make their glyphs invisible, and A's `linkTextAttributes` and `selectedTextAttributes` carry no foreground (record A's macOS 26 defaults). Cases: an inline `||secret||`, and a hidden `||```secret```||` whose code text view sits under today's `SpoilerCover`. Sample each cover rect inset by `spoilerRadius` (3), in four states: unselected, selected with `isKeyWindow == true`, selected with `isKeyWindow == false`, pointer hovering; assert the key state in each and that selection pixels outside the cover differ between the key and not-key states. Mutations, each alone, for A and B: layer 1 only, layer 2 only (clip off, or for A a forced TextKit 1 fallback), neither; and for the block, its text view un-hidden | Uniform (max channel delta ≤ 2) in all four states with both layers; each single layer's result recorded (S-2's wording follows it: a prototype without a working second layer drops that claim); neither layer, or the un-hidden block, fails the test |
| N0-8 | Inline images | A custom emoji as an `NSTextAttachment` whose `image` comes from `EmojiImages.loaded` (no file wrapper, contents or cell), sized to the 22 pt box; the seam inserts the image after first layout and the view swaps it in; a copy over it emits `:name:`; no fetch starts | Fits the box; the swap is observed; copy emits `:name:` |
| N0-9 | Hover tip | Character index under the pointer → the run's tip → a rect for `TooltipOverlay` | Rect within 1 pt of the run |
| N0-10 | Accessibility | On a row holding `||secret||` and `||```secret```||`: walk `accessibilityChildren()` recursively; on every element read value, label, title, `accessibilityString(for:)` and `accessibilityAttributedString(for:)` over the full range, `selectedText` after a selection spanning the spoiler, the number of characters, the visible range and link children. Also: does SwiftUI `.accessibilityLabel` on a representable replace or sit beside its NSView subtree, and is a representable under `.hidden()` in the tree | `secret` appears in no output; the report lists every attribute queried with its output |
| N0-11 | NSTextView defaults (A) | With `isEditable` false and the view first responder (asserted), validate each of these selectors through `validateUserInterfaceItem(_:)` with an `NSMenuItem` built in the test: `copy:`, `selectAll:`, `cut:`, `paste:`, `delete:`, `print:`, `startSpeaking:`, `stopSpeaking:`, `takeFindStringFromSelection:`, `performFindPanelAction:`, `performTextFinderAction:`, `centerSelectionInVisibleArea:`, `showWritingTools:`, `orderFrontSharingServicePicker:`, `orderFrontSubstitutionsPanel:`, `checkSpelling:`. Also: `validRequestor(forSendType: .string, returnType: .string)` and `(.string, nil)` with and without a selection; `writingToolsBehavior`; `menu(for:)` and `quickLook(with:)` through override hit counters (the default is never run). Before and after the overrides | One row per selector with before and after; after: only `copy:` and `selectAll:` validate true, both `validRequestor` calls return nil, Writing Tools is `.none`, `menu(for:)` reaches our hook |
| N0-12 | Pasteboard | A before: `writeSelection(to: private, types: writablePasteboardTypes)` records the default types. A after and B: the overridden `copy:` sent to the view with the injected private pasteboard; then A's `writeSelection(to: private, types: [.rtf, .rtfd, .URL])` | After: `types == [.string]` exactly on the private board; `NSPasteboard.general.changeCount` unchanged |
| N0-13 | Links without `.link` | Storage holds a velella-private attribute with the link or spoiler index and no `.link` and no `URL` value anywhere; a synthesized click resolves the index and calls `LinkRouter.handle`; a drag starting on a link opens nothing | Enumeration finds no `.link`/`URL`; recorder opener called once for a plain link, a confirmation for a masked one, a reveal and no open for a hidden spoiler |
| N0-14 | Hit-testing | Index and segment-frame containment for a point 40 pt past the end of a line ending in a link | The point is outside every segment frame of the link |
| N0-15 | Edits under selection and covers | Select across two rows, replace the second's content with `""` then with a shorter text, ⌘C; render `a ||secret||`, update the same view to `aaaaaaaa ||secret||`, rerun N0-7's pixel check | No trap, only the first row's part copied; the new cover is uniform |
| N0-16 | Selection across rows, with headers | A coordinator prototype over two rows, each with a header participant (the styled name: gradient, glow and design from `displayNameStyle`, at T1 metrics; plus the 12 pt time) and a body. Name the header technique (a text view, or a SwiftUI header taking part in the coordinator). Cases: a drag from row A's body into row B's body; a drag that autoscrolls past the column edge until A is recycled, then ends in B; after each, ⌘C. SwiftUI handlers in the same row still fire over or beside the text: the row's `.onHover` gutter time, a hidden spoiler cover's tap to reveal, the code block's hover Copy button, the header time tooltip. State how each drag was driven | ⌘C gives A's tail, `name`, ` — time`, B's head per D2/D3, also after recycling; each handler fires; anything not drivable is listed for U3 |
| N0-17 | Size | Lines of code of prototype A (with its overrides) and B | Recorded |

**Report:** for each row, A's and B's measured value and pass or fail, the technique chosen, and what TextKit 2 could not do. Main adds U1-U3 and E1 and gives the user the A/B comparison.

### Main's checks after N0

| ID | Check | By |
|---|---|---|
| U1 | On the spike build (`--spike A` and `--spike B`; the N0 report gives the exact build and launch commands), launched by main: force-click, three-finger tap and ⌃⌘D on a hidden word and on a link; VO-Shift-M on a text view; Edit › Speech with an empty selection; ⌘E then ⌘G in another app; the Services menu over a selection | Main, announced input takeover |
| U2 | The default button of `confirmationDialog` for a link, and whether a held Return from a menu item repeats into it | Main |
| U3 | Every N0-16 drag case the harness could not drive, on the same spike build | Main |
| E1 | Official client: does ⌘C of a selection keep the author's U+001B and U+202E? **Done 2026-10-15:** keeps U+001B and U+202C; U+202E never arrives (stripped by the server) | Main + user |

## Revision 2.1 (after N0; the user chose B)

N0 (5e521f6, kept on branch `spike/n0-textkit2`) measured both prototypes; the user chose **B** and accepted the spike window (2026-10-16). Revision 2 turned the outlines into contracts; revision 2.1 folds in its plan review (REVISE, 8 blockers) and security review (6 P2, 11 P3, 5 P4; tables below). **Process (user, 2026-10-15):** one closing review of this revision, then start; at the cap the user decides.

The spike code is a reference, not a merge source: executors port what they use into production files and leave the spike branch alone.

### N0 results for B (from the N0 report)

| Row | B's result |
|---|---|
| N0-1 | No `NSLayoutManager`, no `NSTextView` (`layoutManagers = 0`) |
| N0-2/3 | Body heights within 0.5 pt of today at 320 and 600; block layout (b) exact (435/435, 386/386), (a) 1.25 pt off |
| N0-4 | Code block: height and line-number positions exact (437/437; 460/460 with header) |
| N0-5 | In-stack heights exact; a resize re-lays only live rows |
| N0-6 | First frame ×0.99, scroll ×0.85 of today (release, median of 7; worst run ×1.47); adversarial: 2300 code blocks 5.6 s against today's 2.6 s, all others ≤ today |
| N0-7 | Cover pixels uniform (delta 0) in all four states; layer 1 alone and layer 2 alone each held; neither failed (145); un-hidden block failed (134); key vs not-key selection colours differ |
| N0-8 | Image-only attachment 18 × 18 at y −3.375; swap observed; copy `:blob:` |
| N0-10 | 14 accessibility outputs, no hidden text; a SwiftUI `.accessibilityLabel` on a representable neither replaces nor hides the NSView subtree; a representable under `.hidden()` is absent from the tree |
| N0-12 | Types after copy: `public.utf8-plain-text` and AppKit's alias `NSStringPboardType`, nothing else |
| N0-13 | No `.link`/URL in storage; plain link opens once, masked confirms, hidden spoiler reveals only, a drag opens nothing |
| N0-16 | Cross-row selection with headers, recycled-origin autoscroll and ⌘C text correct; four SwiftUI handlers not drivable offscreen (→ N2 live) |

### Files and order

| File | N1 | C1 |
|---|---|---|
| `Sources/VelellaUI/Text/*` (new) | owns | — |
| `ParagraphView.swift`, `MarkdownView.swift`, `MarkdownText.swift`, `Tooltip.swift` | owns (`CopyFeedback.copy(_:to:)` keeps its name, signature and visibility) | read only |
| `Sources/VelellaUI/MessageMenu.swift` (new) | — | owns |
| `MessageArea.swift`, `ShellView.swift`, `ShellState.swift`, `Sources/Velella/AppModel.swift` | read only | owns |
| core `messages.rs`, FFI `messages.rs` | — | additive: the per-message preview call (C1 Items) |
| `DesignTokens.swift` | additive only, its own `enum TextView` | additive only, its own `enum Menu` |
| Tests | moved and new N1 tests | new C1 tests |

Main merges N1 and C1 in either order; N2 starts from m1 after N1; W1 starts after N2 and C1.

## N1 — Message text on B (SECURITY slice)

| Part | Contract |
|---|---|
| View | `MessageTextView`: an `NSView` driving its own `NSTextLayoutManager` + `NSTextContentStorage`; no `NSTextView`, no `NSLayoutManager`; hosted by an `NSViewRepresentable`. It adopts `NSAccessibilityStaticText` only, and none of `NSTextInputClient`, `NSServicesMenuRequestor`, `NSTextFinderClient`, `NSAccessibilityNavigableStaticText`; it overrides `validRequestor(forSendType:returnType:)` to return nil and `quickLook(with:)` to do nothing; `menu(for:)` returns nil (W1 adds the hook). |
| Block layout | N0-3 (b): today's SwiftUI block layout stays; each text block (paragraph, heading, subtext, list item text, inline part) is one `MessageTextView`; a code block keeps its Vencord chrome with its code in one `MessageTextView`. |
| Metrics | N0-2/3 technique (min = max line height, glyph lift, first baseline for `.firstTextBaseline`, heights rounded to the display scale). Pass: the existing MessageLayout, MarkdownBlock, Polish and CodeBlock geometry assertions, unchanged in value, against hosted-window renders. |
| Content | From velella's AST through `MarkdownText`'s segments with `NSAttributedString(string:attributes:)` only; fills and styles as today; the inline-code border (T1 F3) drawn. Storage is rebuilt on each relative-time tick without losing hover state or the selection. |
| Links | No `.link` and no URL value in storage; a private attribute holds the index. A link or spoiler activates only when `clickCount == 1`, mouse-down and mouse-up fall in the same index's segment frames, and the drag threshold is not crossed; it then calls the nearest `MarkdownView`'s router through its `openURL` environment. A covered run carries only its spoiler index. |
| Spoilers | Hidden runs from `Segment.covered`; layer 1 (fragment clip + cover) and layer 2 (clear glyphs) both kept; covers computed at draw time. The reveal set is stored with the blocks it was made for, and storage is built with an empty set unless those blocks equal the current ones (F-8). A hidden code block stays under `SpoilerCover` (`.hidden()`). A covered emoji gets no attachment and starts no fetch. |
| Emoji | `NSTextAttachment` subclass, no file wrapper, contents or cell; image from `EmojiImages.loaded`; 18 × 18 at y −3.375. |
| Hover tips | The view's tracking area; the run's tip and its rect from the font's ascent + descent; shown at once through `TooltipOverlay`. |
| Accessibility | Each text view's value is `ParagraphView.spoken`'s text; no range-based text API returns stored text; hidden text appears in no accessibility output. |
| First responder and copy | `MessageTextView` accepts first responder on mouse-down and forwards `copy:` to its coordinator. A `SelectionCoordinator` spans one message's text views. Copy writes through `CopyFeedback.copy` to an injected pasteboard; the types are exactly `public.utf8-plain-text` and `NSStringPboardType` (N0-12). Text per D2: hidden spoiler text included without a trailing newline; every run made by `MarkdownText.isolated` (mentions, channels, roles, emoji alt text, slash commands) copies its raw text, so no U+2066-2069 appears except the author's own; an emoji copies `:name:` and U+FFFC never reaches the pasteboard; code copies its raw scalars through the display-to-code offset map, and a boundary inside an escape snaps to the whole escape (S-15); the author's characters are unchanged (E1). Selection colour: the system's selected-text background, unemphasized when the window is not key. |
| Limits | At most 64 `MessageTextView`s per row, embeds included. Past the 64th, the rest of the row's text (in document order) renders in one more view that keeps every rule: covered runs with both layers, "Spoiler" in accessibility, the code escape table and the copy mapping. Absolute bounds, release: each adversarial row's first frame ≤ 1 s; the N0-6 corpus medians ≤ 1.5 × the baseline. |
| Baseline | Before replacing the renderer, N1 measures today's renderer once on its base commit with N0-6's protocol (corpus and adversarial rows) and records the medians and commit in its report; the standing test asserts absolute values only and references no pre-N1 code. |
| Tests | The N0 harness (offscreen window subclass, asserted key state, `cacheDisplay`, injected pasteboard, `NSPasteboard.general.changeCount` unchanged). The moved tests listed in revision 2 with the same assertions. Kept from N0: N0-7 four states with each single-layer mutation; N0-10's walk; N0-12; N0-13; N0-15. New: a view-tree check after a full render (no `NSTextView`, each view's content manager is its own storage); conformance checks for the protocols above and `validRequestor(.string, .string) == nil` with a selection; a responder test (synthesized mouse-down and drag in the key window, `window.firstResponder` is the text view, `copy:` sent to `window.firstResponder` writes the expected string); copy cases (slash command, image-less emoji, hidden emoji, mention: no stray isolates; ESC and U+202E in code copy raw; a selection ending mid-escape copies the whole character or none); a covered emoji makes no attachment and no fetch; reveal then edit to new spoiler content: the first render shows a uniform cover and "Spoiler"; resize 600 → 320 keeps a wrapped cover uniform; a double click on a plain link opens at most once; a drag starting on a hidden spoiler reveals nothing; the E1 ESC copy; limit cases: 2300 code blocks, code blocks interleaved with text, about 1000 list items, about 1000 headings, 10 embeds full of code blocks, each asserting the view count ≤ 65 and ≤ 1 s; 64 code blocks then `||```secret```||` and ```` ```‮x``` ````: uniform cover, no `secret` in accessibility, U+202E drawn escaped. |
| Live (main) | U1 on the N1 build before merge: force-click, three-finger tap and ⌃⌘D on a hidden word and a link; VO-Shift-M; Edit › Speech with an empty selection; ⌘E then ⌘G elsewhere; the Services menu over a selection. Results recorded here. |
| Mutants (verifier) | no clip, no clear glyphs, `.link` in storage, nearest-character hit, hidden text in the spoken value, RTF on the pasteboard, a stale cover after an edit, reveal set not keyed by blocks, `acceptsFirstResponder` false or no `copy:` forwarding, no view cap |

## N2 — Selection across messages (after N1)

| Part | Contract |
|---|---|
| Model | The coordinator moves to the message column; the selection is kept as model positions (message id, part, UTF-16 offset). A part is dropped when its displayed string changes (a relative-time refresh or a later name resolution counts); offsets clamp and snap to composed-character boundaries. |
| Parts | Each row's header and its body text blocks only; embeds and attachments contribute nothing. |
| Headers | The SwiftUI header stays; a transparent-glyph `MessageTextView` behind it, holding only the name and the time, takes part in hit-testing and paints the selection; it is not an accessibility element. ⌘C emits `name⏎ — time⏎` for a message entered from above (D3). |
| Drag | The coordinator tracks the drag, autoscrolls while the pointer stays past the column's edge, and survives the origin row being recycled; a click elsewhere clears. |
| Copy | D2-D4 across messages, one newline between messages, the N1 copy mapping and pasteboard rule. |
| Tests | N0-16's cases (body to body, recycled-origin autoscroll, ⌘C with headers); edit and delete under a selection; copy across an `R` timestamp after a refresh (no trap, no lone surrogate); the header participant absent from accessibility. |
| Live (main, U3) | The row's gutter time on hover, a hidden spoiler's tap to reveal, the code block's Copy button, the header time tooltip, all beside the text views. |

## C1 — Discord-layout glass menu (SECURITY slice, parallel with N1)

| Part | Contract |
|---|---|
| Panel | A borderless panel class overriding `canBecomeKey` to true, at the cursor, in Discord's layout on glass: 36 pt row pitch, label row min-height 32, padding 8, item radius 2, separators 1 pt with 8 above and below (D5). |
| Context | The presenter takes `MessageMenuContext { message id, delivery, author name, raw content, channel id, guild id or nil, link: (target: LinkTarget, router: LinkRouter)?, copySelection: (() -> Void)? }`, all captured at open. C1 itself supplies `link: nil` and `copySelection: nil`; W1 supplies them. |
| Items | Reply; Copy Text (shown only when `content` is non-empty); Copy Message Link; Copy Message ID; with a context link, Copy Link (writes the captured `LinkTarget.url`, never the label) and Open Link; with `copySelection`, Copy ⌘C. Order and sections follow `e0-menu-other.png`; items velella cannot do are absent. |
| Gating | Reply, Copy Message Link and Copy Message ID appear only when `delivery == .sent`; the link and ID items also need every id to pass `UInt64(_:)` (re-formatted with `String(_:)`); `@me` only when the conversation is a DM; in a guild a failing guild id hides them. |
| Ids source | `ShellState` gains the open conversation's channel id and guild id (nil for a DM), written by `AppModel.apply(messages:)` in the same assignment as `messages`, from `messagesChannel` and its guild, inside the existing open/generation guard. The menu reads them at open. |
| Reply | Builds the `ReplyPreview` only from the capture: `messageID` and `authorName` from the context, the preview line from core's spoiler-safe `preview()` through a new per-message FFI call (`MessageSubscription.preview(message_id) -> String?`, one flatten under the store lock at menu open); the preview is text only (emoji as `:name:`). `ReplyPreview` carries its channel id; `AppModel.send` drops a `replyTo` whose channel is not `messagesChannel`. |
| Dismissal | A body change, the message leaving the store, its delivery changing, a channel switch, a scroll, the app resigning active, the main window closing or losing key, sign-out. |
| Focus and keys | The panel is the key window while open. An item chosen with a key runs on keyDown, then the panel swallows that key, repeats included, until its keyUp, before restoring the previous responder or presenting a dialog; Escape and an outside click restore the responder; the dismissing click is consumed. |
| Pasteboard | `CopyFeedback.copy` to an injected pasteboard, the N1 type set. |
| Tests | Item sets for a sent row, a `.pending` row with a numeric id (no Reply, link or ID items), a DM (`@me`), an empty-content row (no Copy Text); id rejection; the link regex; Copy Text exact bytes; the ids source (rows for guild A, the selection switched without a new snapshot: the link still uses A, or the menu is dismissed); Reply on `a ||secret|| b` and `||```secret```||` sets `reply.messageID` to the row id and no segment or accessibility output contains `secret`; reply then channel switch gives `reply == nil`; a reply target from another channel is dropped by `send`; deleting the captured message dismisses; `canBecomeKey` is true on the production class; keys routed through the key window: Down, Return down, two repeats, keyUp with a focused composer holding a draft never call `onSend`. |
| Live (main, U2) | On the C1 build before merge: held Return on an item never reaches the composer; the confirmation dialog's default button and whether a repeat reaches it (with W1 later for Open Link). |
| Mutants (verifier) | Return or repeats reaching the composer, Reply target read at choose time, ids not checked, `delivery` not gated, `canBecomeKey` false, RTF on the pasteboard |

## W1 — Wiring the menu to the text views (SECURITY slice, after N2 and C1)

| Part | Contract |
|---|---|
| Trigger | A right-click on a `MessageTextView` (and its `menu(for:)`, and `accessibilityPerformShowMenu`) opens C1's presenter for the row under the pointer, never the selection's anchor. |
| Link | Link items only when the point lies inside a link segment's frames, never over a covered run. The context link is the hit's owning router (the nearest `MarkdownView`'s, so an embed's description or field uses its own numbering) and the captured `LinkTarget`; Open Link runs that router's checks on the captured target and does nothing unless `router.body.links[n] == captured`. |
| Selection | Copy ⌘C only with a non-empty selection, through the coordinator's copy. |
| Tests | A right-click 40 pt past the end of a line ending in a link gives no link items (S-13); a right-click on a hidden spoiler inside a masked link gives none; with a selection anchored in message Y, a right-click on X targets X; an embed description's plain link 0 in a message whose body link 0 differs opens the embed's URL under the embed body's rule; a body change between open and choose (dismissal bypassed) opens nothing; Copy Link on `[https://a.com](https://b.com)` writes `https://b.com/`. |
| Live (main) | U2's dialog part with Open Link on a masked link. |
| Mutants (verifier) | nearest-character hit, link items on a covered run, the row router instead of the owning router, the target re-read at choose time |

### Security dispositions (security review of revision 2, 515cf7c)

| # | Finding | Disposition |
|---|---|---|
| F-1 (P2) | Reply could put hidden spoiler text in the bar | FIX: C1 Reply (core `preview()` per message over FFI) |
| F-2 (P2) | Nested routers and choose-time resolution | FIX: W1 Link row |
| F-3 (P2) | Return reaching the composer or the dialog; borderless panel never key | FIX: C1 Panel, Focus and keys; U2 before merge |
| F-4 (P2) | Limit counted code blocks only; fallback lost the rules | FIX: N1 Limits (all text views, embeds included; the remainder keeps every rule) |
| F-5 (P2) | Wiring without contract, owner or tests | FIX: W1 slice |
| F-6 (P2) | S-5 had no evidence for B | FIX: N1 View (protocols, `validRequestor`, `quickLook`), conformance tests, U1 on the N1 build |
| F-7 (P3) | Envelope contradicted Reply | FIX: Envelope Non-goals and Traffic |
| F-8 (P3) | Reveal state survived an edit for one pass | FIX: N1 Spoilers |
| F-9 (P3) | Copy mapping scope (S-14, S-15, S-17) | FIX: N1 First responder and copy; S-15 moved to N1 |
| F-10 (P3) | Echo gating and the ids source | FIX: C1 Gating and Ids source |
| F-11 (P3) | Dismissal triggers and the reply target's lifetime | FIX: C1 Reply and Dismissal |
| F-12 (P3) | Click semantics | FIX: N1 Links |
| F-13 (P3) | Copy Link content; covered runs | FIX: C1 Items, N1 Links |
| F-14 (P3) | N0 results not recorded; N0-1 test dropped | FIX: N0 results table; N1 view-tree test |
| F-15 (P3) | N2 offsets, parts, header accessibility | FIX: N2 Model, Parts, Headers |
| F-16 (P3) | Reply bar emoji fetch | FIX: the preview is text only |
| F-17 (P3) | Missing N1 cases | FIX: N1 Tests |
| F-18 (P4) | Pasteboard type set loosened | FIX: pinned to N0-12's measured set |
| F-19 (P4) | Stale ReplyBar deferred row | FIX: closed |
| F-20 (P4) | Line breaks in display names forging header lines | DEFER: unknown whether Discord allows them |
| F-21 (P4) | Relative-time rebuild vs hover and selection | FIX: N1 Content |
| F-22 (P4) | Copy Text on empty content | FIX: hidden |

### Plan review dispositions (plan review of revision 2, 515cf7c)

| # | Blocker | Disposition |
|---|---|---|
| 1 | No rev 2 security dispositions; U1-U3 unrecorded | FIX: the table above; U1 on the N1 build, U2 on the C1 build, U3 in N2 |
| 2 | Order contradiction; no file ownership | FIX: Envelope Order and Owners; Files and order table |
| 3 | Wiring undefined | FIX: C1 Context; W1 slice with owner, merge point and tests |
| 4 | No Swift source for channel and guild ids | FIX: C1 Ids source |
| 5 | Reply preview source | FIX: C1 Reply |
| 6 | Relative performance bars after the swap | FIX: N1 Baseline; absolute standing bounds |
| 7 | The 64 limit ambiguous and aimed at code blocks | FIX: N1 Limits and the new adversarial rows |
| 8 | No proof ⌘C works through the responder chain | FIX: N1 First responder and copy; responder test and mutant |

## Security dispositions (security review of rev 1, 93f2cb3)

| # | Finding | Disposition |
|---|---|---|
| S-1 (P1) | Today's hiding (SwiftUI-scoped colours, a cover drawn instead of glyphs) does not survive NSTextView: SwiftUI attributes drop, `linkTextAttributes` and `selectedTextAttributes` repaint, the translucent cover shows glyphs | FIX: hidden glyphs are clipped out, never drawn; hidden runs from `Segment.covered`; no foreground colour in link or selection attributes; N0-7 four-state pixel test |
| S-2 (P2) | A TextKit 1 fallback silently drops the fragment drawing | FIX: N0-1 notification observer through every N0 row; N1 tests keep it; a second hiding layer (attributes) is tested alone in N0-7, and a prototype without a working second layer drops this claim |
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
| S-18 (P4) | Copies carry the author's invisible controls (paste-jacking) | Measured (E1, 2026-10-15): the official client's selection copy keeps U+001B and U+202C byte for byte; Discord's server strips U+202E from message content. velella matches: the author's characters are copied unchanged; an N1 test pins that ESC survives a copy |
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

## Plan review dispositions (plan review of rev 1.1, 768f7bc)

| # | Blocker | Disposition |
|---|---|---|
| 1 | No key or first-responder state; `sendAction` finds no target | FIX: harness key state (switchable subclass, asserted); copy sent to an explicit target |
| 2 | Native `copy:` writes the general pasteboard | FIX: injected pasteboard; A's default types via `writeSelection`; `NSPasteboard.general.changeCount` unchanged |
| 3 | Calling speech, print, find or Quick Look defaults changes real state | FIX: harness rule; validation and hit counters only; real calls in U1 |
| 4 | No main menu under `swift test`; undefined item list | FIX: N0-11 named selectors validated with test-built menu items |
| 5 | Emoji images only load from the CDN | FIX: throwaway `EmojiImages.loaded` seam, non-Discord URLs, no fetch, same images for all sides |
| 6 | S-2's second layer contradicted N0-7 and was untested | FIX: N0-7 mutates each layer alone and both; S-2 reworded |
| 7 | No hidden code block case; rounded corners in sampling | FIX: N0-7 block case and un-hidden mutation; samples inset by `spoilerRadius` |
| 8 | Scroll statistic | FIX: total of 20 steps per run, warm-up discarded, alternating sides, 7 per-run values reported |
| 9 | Security rows not required for D6 | FIX: eligibility rule (N0-1, N0-7, N0-10, N0-12, N0-13, plus N0-11 for A); stop and D6 reworded |
| 10 | No launchable spike build for U1 and U3 | FIX: throwaway `--spike A|B` switch in the worktree app, no Discord connection; the report gives the commands |

## Deferred items found during E0

| Item | Where |
|---|---|
| `ReplyBar` built `Text("Replying to \(reply.authorName)")` with interpolation | Closed: track R made it `Text(verbatim:)` with `isolated` (F-19) |
| Image-attachment menu items (Copy Image, Save Image) | Not captured (no attachment in the thread); with attachments' menu work |
