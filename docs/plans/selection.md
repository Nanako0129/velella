# velella Selection and Copy Plan (revision 1)

Repo: /Users/nanako/side-project/velella, branch `m1` (at 055948b). `docs/plans/markdown.md` still applies: its F5 rendering rules, link confirmation, spoiler and bidi rules, and every K/T/P contract this plan moves to a new renderer. The ground truth is `docs/element-map.md` (captures in git-excluded `refs/`).

The user asked for message copy (2026-10-15: "現在都沒辦法複製對吧？"), had the message-area elements of official Discord mapped against Swiftcord v2 first (E0), and decided:
- the context menu takes official Discord's layout (icons, sections, danger items), made of glass, with Discord's row height ("做成玻璃的，厚度要一樣不只是矮矮的文字行");
- text selection moves to NSTextView now ("直接做 NSTextView 版");
- selection crosses messages as in the official client ("跨訊息（跟官方）");
- a selection copies hidden spoiler text, as the official client does (measured), without its stray newline;
- the NSTextView work comes first, then the menu.

## Envelope

| Field | Value |
|---|---|
| Outcome | Message text can be drag-selected within and across messages; ⌘C copies the selection as displayed text. Right-clicking a message opens a glass menu in Discord's layout with Copy (when there is a selection), Copy Text (the raw content), Copy Message Link and Copy Message ID, plus Copy Link and Open Link on a link. |
| How it is seen working | The user selects across two messages with a hidden spoiler, a mention and a timestamp, pastes into a text editor, and compares with the official client's paste of the same range (E0 rows). The user right-clicks a message and a link and compares the menu with `e0-menu-other.png` / `e0-menu-link.png`. |
| Non-goals | The hover action bar, reactions, reply/edit/delete/pin/forward and other menu actions (they join the menu when the features exist), the Shift hover bar, image-attachment menu items, "Search with Google", ⌘A over the timeline, the composer. |
| Traffic added | None. |
| Order | **N0** spike (throwaway, measurements) → plan revision 2 with N1/N2/C1 contracts fixed by N0's results → reviews → **N1** → **N2** → **C1**, sequentially. |
| Owners | N0: pilotfish:executor. N1: pilotfish:security-executor (untrusted rendering). N2: pilotfish:executor. C1: pilotfish:security-executor (link opening, pasteboard). |
| External actions by agents | None: no Discord connection, no git push, no publishing, no installs, no launching velella or other GUI apps, no real preferences or Keychain. Live checks are main's or the user's. |
| Checkpoints | One commit per slice after its verifier; main merges into m1 and pushes main (auto-commit workflow). |
| Rollback | Each slice is a merge commit; `git revert -m 1`. |
| Budget | At most 2 executor passes per slice, then main takes over or asks. |
| Stops | N0 shows TextKit 2 cannot hold one of the N0 rows below, or the NSTextView rows are more than 2× slower than today's SwiftUI rows: main brings the options to the user before N1. |

## Decisions

| # | Decision | Source |
|---|---|---|
| D1 | Copy Text (menu) = the message's raw `content`, byte for byte; Shift changes nothing | Measured (E0) |
| D2 | ⌘C of a selection = displayed text: formatting removed, mentions as shown (`@name`), timestamps as shown, link labels as shown, hidden spoiler text included, no newline added after a spoiler, velella's own bidi isolates (FSI…PDI around substituted names) removed, the author's own characters kept | Measured (E0) + user |
| D3 | A selection that crosses into a message includes that message's header as one line `name — time` (the displayed header time), then its body; messages are separated by one newline. Discord splits name, badge and time onto separate lines (a DOM artefact, like the spoiler newline); velella keeps them on one line. | Default, user may override |
| D4 | The pasteboard gets plain text only (`.string`), never RTF or attributed data | Security (hidden-spoiler link URLs and attributes must not ride along) |
| D5 | Menu row height and metrics come from the E0 capture (36 pt pitch at 1× in `e0-menu-other.png`; CSS min-height 32, padding 8, separators 1 pt with 8 above and below), icons as SF Symbols at Discord's size, the danger colour for destructive items (none in C1), on a glass panel | User + E0 |

## N0 — Spike (no merge)

**Goal:** decide the N1/N2 techniques with numbers. **Owner:** pilotfish:executor in a worktree; nothing from it merges. **Files:** a throwaway `Sources/VelellaUI/Spike/` folder and a throwaway test file; everything else is read-only.

| Question | Measure | Pass |
|---|---|---|
| S1 TextKit 2 holds | A non-editable, selectable `NSTextView(usingTextLayoutManager: true)` configured as N1 needs (no link detection, no data detectors, no find bar, custom menu hook, plain-text pasteboard) still has `textLayoutManager != nil` after configuration, after layout and after a selection | Stays TextKit 2 |
| S2 Metrics | `sizeThatFits` at a proposed width gives T1's 22 pt line pitch at 16 pt body and the same wrapped height as today's `ParagraphView` within 1 pt, for the T1 test paragraphs | Within 1 pt |
| S3 Performance | 200 fixture messages in the existing `LazyVStack`: time to first frame and the cost per row created while scrolling, against today's SwiftUI rows in the same harness (release) | ≤ 2× today |
| S4 Spoiler cover | A rounded (radius 3) cover over a glyph range that hides the glyphs while the text stays selectable; how it is drawn (custom `NSTextLayoutFragment` or rendering attributes) | Drawn, selectable |
| S5 Inline images | A custom emoji as a text attachment sized to the line box, swapped in when loaded | Fits the 22 pt box |
| S6 Hover tip | Character index under the mouse → the run's tip → a rect for the existing `TooltipOverlay` | Rect correct |
| S7 Accessibility | The view's accessibility value can be set to `ParagraphView.spoken`'s text so hidden spoiler text never reaches VoiceOver | Hidden text absent |
| S8 Two-view selection | A prototype coordinator: a drag starting in view A and ending in view B selects A's tail and B's head; ⌘C returns their concatenation; with the rows inside the `LazyVStack`, a range survives one of them scrolling out and back | Works |
| S9 Default affordances | Which of NSTextView's defaults must be switched off: Services, Look Up / force-click, Translate, Share, Speech, drag of the selection, find, spelling | List with the switch for each |

**Report:** each row's measured value, the chosen technique, code size of the prototype, and anything TextKit 2 could not do.

## N1 — Message text on NSTextView (SECURITY slice), outline

Each message's text blocks (paragraphs, headings, subtext, quotes, lists) render in one TextKit 2 text view; a code block keeps its Vencord chrome with its code in its own text view. Every behaviour of K2-K4, T1, P1 and K6's view carries over: fonts and the 22 pt line box, mention and inline-code fills, link routing through `LinkRouter` and `LinkConfirmation` (no automatic opening, no data detectors), spoiler covers and click-to-reveal with hidden links inert, custom emoji through `EmojiImages`' allow-list, timestamp tips and relative refresh, bidi isolation, the spoken label, quote bars, list markers, the trailing-break rule. Selection within a message and ⌘C per D2 and D4. Tests that drive SwiftUI `Text` today (`renderingATippedSegmentRecordsItsFrame`, `pillFillsAndSpoilerCoversDrawUnderTheRenderer`, `theDrawnParagraphSpeaksItsLabelNotHiddenText`, the MessageLayout geometry tests, the MarkdownBlock pixel tests, `copyShowsOnHoverAndIsAbsentWhileCovered`, the header height tests, the Polish line-height tests) are rewritten against the new view with the same assertions. Contract fixed in revision 2 after N0.

## N2 — Selection across messages, outline

A `SelectionCoordinator` per message column keeps the selection as model positions (message id, text part, UTF-16 offset), so rows can leave and re-enter the `LazyVStack`. Registered text views: each row's header and its body parts. It tracks the drag itself, autoscrolls at the column's edges, paints each visible view's sub-range, clears on a click elsewhere, and builds ⌘C per D2-D4. Contract fixed in revision 2 after N0.

## C1 — Discord-layout glass menu (SECURITY slice), outline

Right-click anywhere on a message row opens a glass panel at the cursor in Discord's layout (D5): Copy ⌘C (selection only), Copy Text, Copy Message Link (`https://discord.com/channels/<guild id or @me>/<channel id>/<message id>`), Copy Message ID; on a visible link, Copy Link and Open Link (Open Link goes through `LinkConfirmation` like a click); nothing on a hidden spoiler's link. Keyboard navigation (arrows, Return, Escape), dismissal on an outside click, and a menu accessibility role. Pasteboard per D4. Contract fixed in revision 2 after N0.

## Threats for the security review

| # | Threat | Planned guard |
|---|---|---|
| T1 | NSTextView defaults act on untrusted text: link auto-detection, data detectors, Services, Look Up, Translate, Share, Speech, drag-out of the selection, find | S9 lists them; N1 switches each off, with a test per switch |
| T2 | Attributed pasteboard types carry hidden spoilers' link URLs and attributes | D4: plain text only; test reads every pasteboard type after ⌘C and Copy |
| T3 | Hidden spoiler text reaches VoiceOver through the text view's accessibility | S7; N1 test |
| T4 | Clicking a hidden spoiler's link, or a link's attribute, opens it | Links only through `LinkRouter`; hidden runs carry only the reveal link (K2 rule); test |
| T5 | Building attributed strings from payload text through HTML/RTF/markdown importers | Strings built from velella's AST only with `NSAttributedString(string:attributes:)`; no importer |
| T6 | Bidi controls from substituted names leak into copied text or reorder the menu | D2 strips velella's isolates only; menu labels are fixed strings; Copy Link copies the URL, never the label |
| T7 | Copy Message Link built from untrusted ids | Ids are core `u64`s formatted in Swift; no payload text in the URL |
| T8 | Copied hidden spoiler text | Accepted by the user (matches official Discord) |

## Deferred items found during E0

| Item | Where |
|---|---|
| `ReplyBar` builds `Text("Replying to \(reply.authorName)")` with interpolation instead of `Text(verbatim:)` and without bidi isolation of the name | `Sources/VelellaUI/MessageArea.swift:350`; P3, fold into the next composer slice |
| Image-attachment menu items (Copy Image, Save Image) | Not captured (no attachment in the thread); with attachments' menu work |
