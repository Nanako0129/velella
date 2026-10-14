# velella Markdown and Time Plan (revision 3.1)

Repo: /Users/nanako/side-project/velella, branch `m1` (M2 integrated; the working tree also holds four uncommitted changes: per-conversation drafts, the v2 member list, the Makefile signing step and the message skeleton). `docs/plans/m1.md` and `docs/plans/m2.md` still apply: the refs and licence policy, the checkpoint convention, the logging rule (no logger; the parser and renderers never log content, URLs or names), the FFI mirror pattern and the M2 security dispositions except F5, restated below.

The user asked for full Discord markdown because Swiftcord's is incomplete (a thread member reported it on 2026-10-14), chose the approach ("照你建議", 2026-10-14), asked that times follow Discord's localisation exactly ("時間格式要照discord完全的本地化相容"), and showed v2's timestamp hover (a tooltip with the full date, e.g. "Wednesday, April 21, 2021 at 5:20 AM").

Revision 2 folds in the plan review of rev 1 (REVISE, 9 blockers, all FIX) and the security review of rev 1 (P1-P3 FIX, P4 noted), and adds a differential oracle so the parser's behaviour is checked mechanically, not by judgment. Revision 3 folds in the plan review of rev 2 (REVISE, 5 blockers, all FIX; see the disposition table at the end); per the review cap it got one closing review (REVISE, 2 blockers on the K2→K4 interface). Revision 3.1 applies those two fixes and the review's notes; the review cap pauses here for the user's decision.

## Envelope

| Field | Value |
|---|---|
| Outcome | Message content renders as the official Discord client renders it: bold, italic, underline, strike, spoilers, inline code, fenced code blocks, headings 1-3, subtext, quotes (`>` and `>>>`), bullet and numbered lists, masked links, plain and `<…>` links, escapes, user/role/channel mentions as pills with names and colours, @everyone/@here, custom emoji as images, unicode emoji, slash-command mentions, guild navigation tokens and `<t:…>` timestamps in all nine styles with a full-date hover. Message header times use the Discord account's locale and Discord's formats. |
| How it is seen working | The user compares velella with the official client on the three samples posted in the dev thread (`1559813791384870953` time styles, `1559813797764669537` markdown, `1559816987192852531` edge cases) and on ordinary channel history. |
| Non-goals | (Syntax highlighting moved into K6 on 2026-10-14.) Embeds, reactions, stickers, other gap-report items. Composer markdown. Mention profiles, channel-mention navigation. |
| Traffic added | Custom emoji images: `https://cdn.discordapp.com/emojis/{id}.{webp|gif}?size=48`, built in core from the parsed id, loaded by the views without credentials when a row is on screen, at most 64 distinct ids per row. Links open in the default browser only when the user clicks them. Nothing else. |
| Order | **K0** (ground truth; user + main) → checkpoint commit → **K1** (parser) → **K2** (FFI, resolution, inline rendering, links) → **K3** (blocks) and **K4** (times) in parallel worktrees. Then **K5** (K0 adjustments) → **K6** (code blocks), sequentially. |
| Ownership | Exactly one slice owns each file (table below). No slice consumes the output of a slice running in parallel with it. |
| External actions by agents | None. No Discord connection, no git, no publishing, no installs. Main posts K0 samples (done) under the standing thread authority. |
| Checkpoints | One commit of the current uncommitted work before K1 (asks the user). One commit per slice after its verifier (asks each time). No push. |
| Rollback | Each slice is one commit; `git revert` with the user's go. |
| Budget | At most 2 executor passes per slice, then main takes over or escalates. |
| Stops | A K0 capture contradicting the oracle on a case the plan treats as settled (main re-plans that rule); any need for live Discord before the user's check. |

## Decisions

| # | Question | Answer |
|---|---|---|
| Q1 | Parser location and engine | Rust core; a simple-markdown-style ordered rule engine run in inline mode over the whole message, as Discord's parser is (`parse(input, {inline: true})`, discord-markdown-parser `src/index.ts`). Not CommonMark. |
| Q2 | Locale | The Discord account's locale from READY legacy `user_settings.locale` (capabilities 253 keep the legacy JSON) and USER_SETTINGS_UPDATE; fallback `en-US`. Formatting uses `Locale(identifier:)` without macOS overrides (K0 confirms that the official client ignores the macOS 24-hour setting). |
| Q3 | Ground truth | The official client's rendering of the three samples, captured by the user (K0). |
| Q4 | Spoilers crossing other styles (`**a ||b** c||`) | Follow the oracle (strict nesting) unless the K0 capture of sample 3 shows crossing; then K1 implements crossing as Serein does (`markdown.rs:3818`). |
| Q5 | Plain links | Open directly unless the K0 click test shows the official client warns; masked links, `xn--` hosts and links whose visible text differs from the canonical URL always confirm. |

## Behaviour sources (pinned clones under refs/, git-excluded; adaptations go in THIRD_PARTY_NOTICES.md)

| Source | Licence | SHA | Used for |
|---|---|---|---|
| `refs/simple-markdown` (ariabuckles) | MIT | `7fb8bb5943ee4e561fec17c2e271a327f4e86d64` | Engine (rule sort by order, then quality-bearing first, then name; quality picks the best match among same-order rules), default rules |
| `refs/discord-markdown-parser` (ItzDerock) | Apache-2.0 | `45d511fc882934b382852953f2c07a96d7635ae5` | Discord rule set and its tests; **the oracle** |
| `refs/SimpleAST` (discord) | Apache-2.0 | `048671be1b59e127d1951fab9b04e01bd6a127a1` | Header tests |
| `refs/serein` | MIT OR Apache-2.0 | (existing clone) | Selected behaviour cases, limits, spoiler safety |
| `refs/concord` | GPL-3.0 | (existing clone) | Timestamp style letters, `_` word-boundary rule |

## The oracle

**Main owns the runner and the fixture; both are read-only to every executor.** Main generates the fixture before K1 branches and commits it with the checkpoint, recording the generation command, the line count and the SHA-256 in `scripts/markdown-oracle/README.md`. K1 owns only the Rust test that reads it; K1's verifier re-runs the runner and checks the checksum.

| Part | Contract |
|---|---|
| Runner | `scripts/markdown-oracle/run.mjs` runs a copy of discord-markdown-parser `src/` under Node 24 type stripping (imports rewritten to `.ts`, type-only imports marked `type`), always in `extended` mode (masked links on, as Discord now allows for everyone), with one documented patch: `TimestampRegex` gains styles `s` and `S` (Discord docs). The copy lives in the scratchpad; the runner documents how to rebuild it from the pinned SHA. Inputs go through simple-markdown's `preprocess` (`\r\n` → `\n`, tab → 4 spaces), and velella's parser applies the same normalisation first. |
| Fixture | `crates/velella-core/tests/fixtures/markdown_oracle.jsonl`: one line per input with the oracle's AST normalised (adjacent text merged; `twemoji` and `emoticon` nodes become text; `br` stays; link targets canonicalised in the runner with Node's WHATWG `new URL(t).href`, which matches the `url` crate (also WHATWG) that K1's reading test applies to velella's side; labels compared literally). |
| Corpus (closed) | (a) every input of discord-markdown-parser `tests/parse.test.ts` (32 tests; lines 4-663); (b) SimpleAST `MarkdownRulesTest` `headers`, `headersConsumeNewlines`, `headersInvalidPrefix`, `headerParseBlockPunctuations` inputs (setext `headerAlt*` excluded: not Discord; `listItems*` excluded: lists are D1); (c) Serein inputs from `only_spaced_markers_start_discord_quotes` (`markdown.rs:2692`, 12 cases), `inline_spoilers_preserve_crossing_styles_and_ignore_nonliteral_delimiters` (:3818), `fenced_blocks_record_language_and_discord_fence_shapes` (:4982), `mass_mentions_render_as_pills_only_for_exact_plain_tokens` (:5407); (d) every line of the three K0 samples and each whole sample as one input (source texts in `refs/discord-ground-truth/samples/`); (e) 3000 strings generated with seed 20261014 from the alphabet `* _ ~ | \` \\ > # - [ ] ( ) < @ t : 1 a 字 space newline https://x.y` of lengths 1-24. |
| Use | K1's differential test runs velella's flat stream (before top-level block grouping, below) over the corpus and requires equality with the fixture, except inputs in deviation classes D1-D7, which the test filters by a predicate stated in code over the **input text or the oracle's output, never velella's output** (so a velella bug cannot filter itself out), and covers with named tests instead. The comparison is of the pre-grouping tree at every level, including inside quotes and list items. |

| Deviation | velella behaviour (named tests) |
|---|---|
| D1 Lists | Parsed (grammar below); the oracle has no list rule in inline mode |
| D2 Ids | Ids must parse as nonzero `u64`; longer or zero ids stay text |
| D3 Timestamps | Kept only for `-?\d{1,17}` with \|unix\| ≤ 8_640_000_000_000 (the JS Date range the official client is bound by); otherwise text |
| D4 Links | link, autolink and url accept only `http`/`https` targets that pass `url::Url::parse` with a host, no userinfo and no C0/C1 or Unicode Cf characters. A rejected candidate is not a link: the engine moves on to the later rules at the same position, so it ends up as literal text (and any inner markup the later rules find). Named tests: `[a](file:///etc)`, `[a](javascript:x)` and `[a](t:1)` → only `Text`; `<ftp://x/y>` → `Text` |
| D5 Limits | Fallback rules below |
| D6 K0 overrides | Any case where a K0 capture shows the official client differs from the oracle; each is listed in the K1 test table with its capture |
| D7 Link labels | Labels never contain links: autolink, url and link do not match inside a label (the oracle parses labels with the plain state, `src/simple-markdown/index.ts:1140`). Test: `[https://a.b](https://c.d)` → `Link{label: [Text "https://a.b"]}` |

## K0 — Ground truth (user + main)

| Part | Contract |
|---|---|
| Samples | The three samples are posted; their source texts are stored in `refs/discord-ground-truth/samples/{times,markdown,edge}.txt` (git-excluded). |
| Captures | The user captures in the official client: the three samples; header times of a message from today, yesterday and an older day; the timestamp hover; and what happens when clicking the plain link and the masked link in sample 3. Main stores them in `refs/discord-ground-truth/` and transcribes the expected text per line (no personal data) into K1's D6 table and K4's tables. |
| Settles | `d`/`s` year digits and 12/24-hour form in the user's locale; `R` wording; header formats; hover text; Q4 crossing; Q5 link warnings; `+`/`1)`/blank-line list behaviour; ten-digit list numbers; out-of-range timestamps; `***x***`; `####`. |

## K1 — Parser (SECURITY slice: untrusted input)

**Owner:** pilotfish:security-executor, branch from the checkpoint commit.

**Files:** new `crates/velella-core/src/markdown.rs`, `markdown_tests.rs` (including the differential test that reads main's fixture); in `messages.rs`, only the `Stored.body` field and its assignment at all three constructions, `parse()`, `Stored::merge()` and the local echo in `begin_send` (the AST is computed once when content arrives or changes, never in `row()`/`snapshot()`). Test: a local echo of `**b**` carries a `Bold` body.

### AST

| Type | Definition |
|---|---|
| `Block` | `Paragraph(Vec<Inline>)`, `Heading{level: u8 /*1-3*/, content: Vec<Inline>}`, `Subtext(Vec<Inline>)`, `Quote(Vec<Block>)` (never contains `Quote`), `List{ordered: bool, start: u32, items: Vec<Vec<Block>>}`, `CodeBlock{language: Option<String>, code: String}` |
| `Inline` | `Text(String)`, `Bold`/`Italic`/`Underline`/`Strike`/`Spoiler(Vec<Inline>)`, `Code(String)`, `Link{url: String /*canonical*/, host: String /*ASCII*/, label: Vec<Inline> /*no Link inside*/, masked: bool}`, `UserMention(u64)`, `RoleMention(u64)`, `ChannelMention(u64)`, `Everyone`, `Here`, `CustomEmoji{id: u64, name: String, animated: bool}`, `Timestamp{unix: i64, style: TimestampStyle}`, `SlashCommand{name: String, id: u64}`, `GuildNavigation{guild: u64, target: NavTarget}`, `LineBreak`, `Block(Box<Block>)` |
| Blocks inside inline containers | As in the oracle, codeBlock matches at any position, and nested parses inherit the outer state's previous capture (it is set only after the outer rule's parse returns, `src/simple-markdown/index.ts:321,344`), so heading, subtext and blockQuote also match at the start of a container's content when the container itself starts a line. Such a block stays inside its container as `Inline::Block`; it is never hoisted out, so spoiler content stays hidden as one unit. Block grouping (below) applies at every block-list level, the top level, `Quote` content and list items; inside inline containers, `Inline::Block` keeps its position. Labels of links never hold blocks (block rules do not match inside a label). Named tests: `\|\|\`\`\`x\`\`\`\|\|` → `[Paragraph[Spoiler[Block(CodeBlock{None,"x"})]]]`; `**# h**` → `[Paragraph[Bold[Block(Heading{1,[Text h]})]]]`; `\|\|> q\|\|` → `[Paragraph[Spoiler[Block(Quote[Paragraph[Text q]])]]]`; `- \|\|\`\`\`x\`\`\`\|\|` → `List{items:[[Paragraph[Spoiler[Block(CodeBlock{None,"x"})]]]]}`. |
| `TimestampStyle` | 9 variants `ShortTime t, MediumTime T, ShortDate d, LongDate D, LongDateShortTime f (default), FullDateShortTime F, ShortDateShortTime s, ShortDateMediumTime S, Relative R` (uniffi has no `char`) |
| `NavTarget` | `Customize`, `Browse`, `Guide`, `LinkedRoles(Option<u64>)` |

### Ordered rules (inline mode throughout)

Sort key as simple-markdown: order, then quality-bearing rules first, then name. "Line start" = no previous capture, or the previous capture ends in `\n` (blockQuote and list also accept `\n` followed by spaces). Sources are discord-markdown-parser files unless noted.

| # | Order | Rule | Produces | Line start | Match (behaviour of) | State |
|---|---|---|---|---|---|---|
| 1 | −0.5 | subtext | `Subtext` | yes | `rules/subtext.ts`, `SubtextRegex` `^-# +([^\n]+?)(\n\|$)` | consumes trailing `\n` |
| 2 | 0 | heading | `Heading` | yes | `rules/heading.ts`, `^(#{1,3}) +([^\n]+?)(\n\|$)` | consumes trailing `\n` |
| 3 | 0 | spoiler | `Spoiler` | no | `^\|\|([\s\S]+?)\|\|` | |
| 4 | 4 | codeBlock | `CodeBlock` | no | `CodeBlockRegex` `^\`\`\`(([a-z0-9_+\-.#]+?)\n+)?\n*([^]+?)\n*\`\`\`` (i); language kept only if `[A-Za-z0-9_+#.-]{1,32}` | does not consume the `\n` after |
| 5 | 6 | blockQuote | `Quote` | yes, and not in a quote | `rules/blockQuote.ts`, `BlockQuoteRegex`; `>>> ` takes the rest of the message, `> ` takes consecutive `> ` lines; markers stripped, content re-parsed with `inQuote` | `inQuote` |
| 6 | 7 | list (velella, D1) | `List` | yes, not in heading/subtext | grammar below | `inList` |
| 7 | 12 | escape | `Text` | no | simple-markdown `^\\([^0-9A-Za-z\s])` | |
| 8 | 14 | autolink | `Link{masked:false}` | no | simple-markdown autolink `<scheme:/…>`, http/https only (D4) | |
| 9 | 16 | url | `Link{masked:false}` | no | simple-markdown url | |
| 10 | 17 | link | `Link{masked:true}` | no | simple-markdown link `[label](url)`; label parsed with `inLink` (no links inside) | `inLink` |
| 11 | 21 q | em, strong, underline | `Italic`, `Bold`, `Underline` | no | simple-markdown em (`_` only at word boundaries; `*` followed by non-space, closed by `*` after non-space and not before `*`), strong `\*\*…\*\*(?!\*)`, u `__…__(?!_)`; quality = capture length, em +0.2; `rules/em.ts`: an em inside em yields its content only | `inEmphasis` |
| 12 | 21 | channel, emoji, everyone, guildNavigation, here, role, slashCommand, timestamp, user (name order) | mention and token variants | no | `utils/regex.ts` lines 7-135 with D2/D3; `<…>` tokens scan at most 512 bytes (a 3-word slash command of 32 non-ASCII letters per word plus its id fits) | |
| 13 | 22 | strikethrough | `Strike` | no | `^~~([\s\S]+?)~~(?!_)` | |
| 14 | 23 | inlineCode | `Code` | no | simple-markdown `^(\`+)([\s\S]*?[^\`])\1(?!\`)` | |
| 15 | 24 | br | `LineBreak` | no | `^\n` | |
| 16 | 25 | emoticon | `Text` | no | `¯\_(ツ)_/¯` literal | |
| 17 | 25 | text | `Text` | no | `TextRegex`, implemented linearly (the oracle's `\w+:\S` lookahead is quadratic on long words; velella tracks the current word run instead) | adjacent text merged |

Paired precedence tests: `||**a||**` → `Spoiler[Text "**a"]`, `Text "**"`; `**[x](https://a.b)**` → `Bold[Link]`; `> # h` → `Quote[Heading]`; `* a` at line start → list, `*a*` → italic; `<t:1:R>` vs autolink; `-# x` vs `- x`.

### Block grouping (worked example)

At each block-list level (top level, `Quote` content, list items) the engine yields a flat stream of inline nodes and block nodes. Maximal runs of inline nodes become `Paragraph`. A `LineBreak` directly before or after a block node, or next to an `Inline::Block` where K2 splits a paragraph, is dropped (K0 may revise the latter) (heading, subtext and quote already consume their own trailing newline; a code block's following newline arrives as a `LineBreak` and is dropped). `a\n# h\nb` → stream `Text a, LineBreak, Heading[h], Text b` → `[Paragraph[Text a], Heading{1,[Text h]}, Paragraph[Text b]]`. Tests also fix `> > x` → `Quote[Paragraph[Text "> x"]]` and `<:n:notdigits>` → `Paragraph[Text "<:n:notdigits>"]`.

### List grammar (D1; K0 may narrow it)

| Clause | Rule | Test |
|---|---|---|
| Markers | `-`, `*`, `+`, or 1-9 digits followed by `.` | `- a`, `* a`, `+ a`, `3. a`; `1234567890. a` stays text |
| Space | at least one space after the marker | `-a` stays text |
| Position | line start, after up to 11 leading spaces | `x - a` stays text |
| Nesting | a marker indented more than the current item's marker starts a nested list; depth counts toward the global depth limit | `- a\n  - b` → item a contains a nested list with b |
| Ordered start | the first item's number | `3. x\n4. y` → `start 3` |
| Continuation | a following line indented at least to the item's content column joins the item; any other line ends the list | `- a\n  b` → one item `a\nb` |
| End | a blank line or a non-item line at a shallower indent ends the list | `- a\n\n- b` → two lists |
| Content | items hold inline rules and nested lists only | `- # h` → item text `# h` |

### Tokens and numbers (security P1-3, P3-2)

Ids are the oracle's digit counts and must also parse as a nonzero `u64`. Emoji names `[A-Za-z0-9_]{2,32}`. Slash-command names: the oracle's character class (`[-_'\p{L}\p{N}\p{sc=Deva}\p{sc=Thai}]`), 1-3 words of 1-32 characters; the oracle's nameless form `</:id>` stays text in velella (D2). Timestamps per D3; any unit conversion uses `checked_mul`. List numbers at most 9 digits, so `start + items` fits `u32` and Swift computes markers in `Int`.

### Limits and fallback (security P1-1, P3-3; review blockers 6-8)

| Limit | Value | On exceed |
|---|---|---|
| Input | 16384 bytes (4000 characters × 4) | fallback |
| Nesting depth | 16, checked before each recursive call | fallback |
| Nodes | 8192 | fallback |
| Fallback | the whole content as `Paragraph(Text)`; **if the content contains `||`, the whole content becomes one `Spoiler`**, so a limit can never reveal spoiler text (Serein `markdown.rs:3925`) | |

**Linear time.** The step bound is ≤ 64 × n matcher steps per nesting level. Two mechanisms provide it. First, each delimiter kind keeps a precomputed next-candidate table, built escape-aware once per parse level: `||`, `~~`, `**`, `__`, `*`, `_`, backtick runs keyed by run length, `](`, `)`, `>` and newline. Second, a per-kind failure memo: once no closer exists at or after position p, later searches from q ≥ p fail at once. The parser counts steps behind `cfg(test)`. A test feeds each pattern to the uncapped inner parser at 1 MiB and asserts the step bound. The patterns are runs of `*`, `_`, `**`, `__`, `~~`, `||`, alternating `*_~|`, backtick runs of increasing length, `[`, `[a](`, `<`, `<t:`, `<@`, `\`, nested `>` lines, nested list indentation, `# ` lines, an unclosed fence and one long run of word characters. A mutant that removes the memo fails the test. Further named tests:
- A 4000-character CJK message (12000 bytes) containing `**b**` yields `Bold`, not the fallback.
- An over-deep and an over-long message, each containing `||secret||`, leave no `Text` outside a `Spoiler` containing `secret`.

### Acceptance

- The differential corpus matches the fixture, apart from D1-D7. The fixture is main's and is not edited by K1.
- Named tests pass for every rule-table row, every pairing, the grouping examples, every list clause, tokens, limits and linear time.
- `cargo clippy --all-targets -D warnings` is clean.
- A fresh pilotfish:verifier runs with mutants: removing each rule's guard, the memo, the depth pre-check or the spoiler-safe fallback must each fail a test. There is no UI change.

## K2 — FFI, resolution, inline rendering, links (SECURITY slice)

**Owner:** pilotfish:security-executor. **Files:** `crates/velella-ffi/src/markdown.rs` (new; mirror types with `From`, the repo pattern of `crates/velella-ffi/src/messages.rs`), the FFI `MessageRow.body`, the core `MessageRow.body` and reply-preview logic in `messages.rs` `row()` (replacing `resolve`), Swift `Sources/VelellaUI/MarkdownText.swift` (new; inline mapping), `ParagraphView.swift` (new; the paragraph host, below) and `MarkdownView.swift` (new, minimal: every block as plain paragraphs, which K3 styles), and MessageArea's body line.

| Part | Contract |
|---|---|
| Resolution | `row()` resolves the stored AST's mention ids against `Context` into `ResolvedMention{kind, id, name: Option<String>, color: u32}` side data; it never re-parses. A nick of `**x** <@&1>` renders literally inside its pill (carries M2's "not expanded twice" test). |
| Reply preview | Core flattens the replied message's AST to one line of at most 200 characters: spoilers (with everything inside them, `Inline::Block` included) become a fixed placeholder (K0 shows Discord's), code and newlines become single spaces, emoji `:name:`, mentions resolved names. Tests: `a \|\|secret\|\| b` and `\|\|\`\`\`secret\`\`\`\|\|` → previews contain no `secret`. |
| Paragraph host | `ParagraphView(build: (RenderContext) -> [Segment])` is the one paragraph host, created by K2 and fixed before K3 and K4 branch. `RenderContext{now: Date, locale: Locale}`; `Segment` is a piece of `AttributedString` plus an optional `hoverTip: String`. Inside a `TimelineView` (periodic at the smallest `refreshEvery` the segments request, otherwise static), it calls `build` with `context.date`, turns each segment into `Text(segment.text)`, applies `.customAttribute(HoverTipAttribute(tip))` to tipped segments (a SwiftUI `TextRenderer` can read only `TextAttribute`s attached this way, never `AttributedString` keys), and joins them by `Text` interpolation (`Text + Text` is deprecated on macOS 26). The renderer records each tipped run's frame in a frame store; `.onContinuousHover` looks the pointer up and shows the tip after 0.5 s. An `Inline::Block` splits the paragraph into a vertical stack (text before, the block through `MarkdownView`'s block view, text after), each part keeping its containers' attributes; a block under a spoiler is covered by that spoiler's reveal unit. **Locale route (K2):** `ShellState.locale: String` (default `en-US`) and an environment key set in ShellView from it; MarkdownView and MessageArea read the key into `RenderContext`. K2 passes a fixed `en-US` formatter for timestamps and a stub tip on its temporary `f` pill; K4 replaces both through the timestamp case only. Tests: an offscreen `ImageRenderer` render of a ParagraphView with one stub-tipped segment leaves a non-empty frame for it in the store; `build` receives two different dates on two timeline ticks; the split of `a \|\|\`\`\`x\`\`\`\|\| b` gives three parts with the block covered. **Manual:** hovering the stub-tipped pill inside a paragraph shows the stub tip. |
| Emoji URLs | One core function from the parsed `u64` (`{}` formatting, never the source substring), host = the existing `CDN` constant (`sidebar.rs:16`), `.gif` if animated else `.webp`, `?size=48`. Per row, ids are deduplicated and at most 64 render as images; the rest render as `:name:`. |
| Inline mapping | Each paragraph becomes one `AttributedString`, built only from `String`s plus attribute runs:<br>• bold = semibold; italic; underline; strike.<br>• code = monospaced at 0.875 × body on `secondarySystemFill` × 0.45.<br>• mentions and @everyone/@here = text on systemBlue × 0.22.<br>• custom emoji = inline images at line height.<br>• slash commands as pills.<br>• timestamps as a pill showing the `f` style through a temporary fixed `en-US` formatter, which K4 replaces.<br>Every substituted untrusted string (names, slash-command names, emoji alt text) is wrapped in U+2068…U+2069. |
| Links and spoilers | Every `.link` attribute holds an in-app URL, `velella-link:<n>` or `velella-spoiler:<n>`, indexing the row's AST. `MarkdownView` installs `.environment(\.openURL, …)`, which always returns `.handled`. For `velella-spoiler:` it reveals that spoiler. For `velella-link:` it rebuilds `URL(string: canonical)`, re-checks that the scheme is http/https and `host()` equals the core host, applies Q5 (confirm when required), and only then calls the injected opener (`NSWorkspace.open` in the app). While a spoiler is hidden, its whole range carries only its reveal link; inner links, mentions and emoji are inert. The confirmation shows the ASCII host on its own line, then the canonical URL, both `Text(verbatim:)`, with Cf characters escaped and the middle truncated. |
| F5 restated | Message-body text is built only from velella's AST, never with `AttributedString(markdown:)`, `LocalizedStringKey(payload)` or HTML. Author names, file names, failure texts and reply previews stay `Text(verbatim:)`. `[a](file:///etc)` stays full literal text, so the existing Swift and FFI tests keep their expectation. |
| Acceptance | **Cargo:** FFI conversion of every AST variant; resolution; preview flattening; emoji URL construction (an id with a non-digit source is impossible by type).<br>**Swift (behaviour, not source scans):**<br>• one mapping test per inline kind.<br>• `Text("**b** [a](https://x)")` maps to literal characters with no link.<br>• `javascript:`, `file:`, userinfo and backslash-host inputs render as text.<br>• hidden `\|\|https://evil.example\|\|` carries only the reveal link.<br>• the injected opener is never called for `velella-spoiler:`, a host mismatch or a declined confirmation.<br>• a nick `"\u{202E}abc"` before `" xyz"` gives `\u{2068}@\u{202E}abc\u{2069} xyz`.<br>• `4294967295. a`, `<t:9223372036854775807>`, `<t:-1>` and `<@99999999999999999999999>` map without trapping.<br>**Verifier:** with mutants.<br>**Manual (P4):** right-clicking a link and a spoiler reaches nothing except through the handler. **Live:** sample 2 matches inline-wise. |

## K3 — Block rendering

**Owner:** pilotfish:executor (worktree). **Files:** `Sources/VelellaUI/MarkdownView.swift` block views only.

| Part | Contract |
|---|---|
| Blocks inside containers | The same block views, with the container's style applied to the block's text runs (bold, italic, underline, strike) and not to code blocks; under a spoiler, the view is covered until its reveal unit opens (ParagraphView's mechanism). |
| Views | Headings 24/20/16 semibold (v2). Subtext 11 secondary. Quotes: left bar and indent. Lists: bullets or numbers (computed in `Int`) with nested indent. Code blocks: SF Mono 0.875 × body on `secondarySystemFill` × 0.45, a 0.5 pt `separatorColor` × 0.35 border, radius 4 (v2), horizontally scrollable. Values without a v2 source (quote bar, list indent) come from the K0 captures, as named constants citing their source. Row view models are `Equatable`, so unchanged rows are skipped. |
| Acceptance | View-model tests for block mapping; previews. **Live:** sample 2 matches block-wise. |

## K4 — Time formatting

**Owner:** pilotfish:executor (worktree). **Files:** core locale parsing (READY `user_settings.locale`, USER_SETTINGS_UPDATE) with its FFI field, the one line in AppModel that copies it into `ShellState.locale`, `Sources/VelellaUI/DiscordTime.swift` (new), the timestamp case in `MarkdownText.swift` (the segment's text from `RenderContext`, its `hoverTip`, and `refreshEvery` for `R`), and the header time in MessageArea. It edits no other file; K3 never touches these.

| Part | Contract |
|---|---|
| Styles | `DiscordTime.format(unix, style, locale)` uses ICU `DateFormatter` date and time styles:<br>• `t` short time, `T` medium time<br>• `d` short date, `D` long date<br>• `f` long date with short time, `F` full date with short time<br>• `s` short date with short time, `S` short date with medium time<br>• `R` relative, updating on screen<br>Any style whose K0 output differs from ICU for the user's locale gets an explicit pattern, recorded with its capture. Intervals are clamped before any `Int` conversion. |
| Hover | Timestamp segments carry a `hoverTip` with the `F`-style full date, and `R` segments request `refreshEvery` (ParagraphView, K2, hosts both; K4 edits no view file other than MessageArea's header line). Header times show the same tip with `.help`. v2 capture: "Wednesday, April 21, 2021 at 5:20 AM"; K0 confirms the official client's text. Manual check: hovering a timestamp pill inside a paragraph, not only the header time, shows the F-style text. |
| Headers | Message header times use Discord's forms for today, yesterday and older days, per K0. |
| Acceptance | Swift tests:<br>• each style for `en-US`, `en-GB` (the Discord docs' examples) and the user's locale, with the K0 strings as expected values<br>• hover text<br>• headers for today, yesterday and older<br>• `R` past and future<br>• out-of-range values never reach the formatter (K1 D3)<br>**Live:** sample 1 matches the official client line by line. |

## File ownership

| File | Owner |
|---|---|
| `scripts/markdown-oracle/`, `crates/velella-core/tests/fixtures/markdown_oracle.jsonl` | main (read-only to executors) |
| `crates/velella-core/src/markdown.rs`, `markdown_tests.rs` | K1 |
| `crates/velella-core/src/messages.rs`: `Stored.body` and its assignment in `parse()`, `Stored::merge()` and `begin_send` | K1 |
| `crates/velella-core/src/messages.rs`: `row()`, `MessageRow.body`, resolution, preview | K2 |
| `crates/velella-ffi/src/markdown.rs`, FFI `MessageRow.body` | K2 |
| `Sources/VelellaUI/MarkdownText.swift` (except the timestamp case), `ParagraphView.swift`, MessageArea body line | K2 |
| `Sources/VelellaUI/MarkdownView.swift` | K2 creates; K3 owns the block views |
| Core locale parsing + FFI locale field, the AppModel line copying it into `ShellState.locale`, `DiscordTime.swift`, timestamp case in `MarkdownText.swift`, MessageArea header time | K4 |
| `ShellState.locale`, the environment key and its ShellView line, MessageArea's `RenderContext` property | K2 |
| `crates/velella-core/src/messages_tests.rs` | K1 adds the echo test; K2 then rewrites the `resolve` tests (sequential, no conflict) |
| `THIRD_PARTY_NOTICES.md` | each slice adds its own rows |

## Security dispositions (security review of rev 1)

| # | Finding | Disposition |
|---|---|---|
| P1-1 | Parser re-run per snapshot under the store lock; quadratic matchers undetected | FIX: AST stored once (K1 files); linear mechanism, step counter and 1 MiB test; 512-byte `<…>` lookahead; depth checked before recursion |
| P1-2 | Link inside a hidden spoiler clickable unseen | FIX: hidden spoiler ranges carry only the reveal (K2) |
| P1-3 | Integer overflow traps from one message | FIX: id, list-number and timestamp grammars (K1); `checked_mul`; Swift `Int` and clamping; trap tests (K2, K4) |
| P2-1 | Rust/Swift URL parsing mismatch; private schemes reaching the system | FIX: core `url::Url` validation, canonical URL + ASCII host; in-app link schemes; handler re-check; injected opener (K2) |
| P2-2 | Masked-link deception; plain-link policy unverified | FIX: no links in labels; confirmation content; Q5 with the K0 click test |
| P2-3 | Bidi overrides in names spoofing the line | FIX: FSI/PDI isolation (K2); author content untouched (REJECT stripping, as Discord does) |
| P2-4 | N3 claimed a nonexistent allow-list | FIX: emoji URLs built in core from the id with the fixed CDN host and size; F11 stays DEFERRED for attachments |
| P2-5 | Reply preview reveals spoilers | FIX: placeholder in flattening (K2) |
| P3-1 | F5 superseded without restating | FIX: F5 restated in K2 |
| P3-2 | Open-ended token grammars; `char` cannot cross UniFFI | FIX: grammars (K1); `TimestampStyle`, `NavTarget` enums; quotes never nest |
| P3-3 | 8 KiB cap drops Nitro CJK messages | FIX: 16384 bytes; node cap 8192 |
| P3-4 | Emoji fetch volume | FIX: per-row dedupe, at most 64 images |
| P4 | Text-selection context menu; local-network links; render cost; logging | Manual check (K2); REJECT (browser's concern, as in the official client); `Equatable` rows (K3); logging rule restated above |

## Verification

Every slice gets a fresh pilotfish:verifier with mutants. K1 and K2 security claims are re-verified on the integrated m1. The user runs the live comparison after integration.

## Plan review dispositions (plan review of rev 2)

| # | Blocker | Disposition |
|---|---|---|
| 1 | Block constructs inside inline containers (`\|\|\`\`\`x\`\`\`\|\|`, `**# h**`, `\|\|> q\|\|`) had no AST shape | FIX: `Inline::Block`, prevCapture inheritance stated, four named tests, preview and rendering contracts (K1, K2, K3) |
| 2 | Rejected links (schemes, labels) and target canonicalisation missing from the deviations | FIX: D4 rewritten (http/https only; rejected candidates fall through), D7 (no links in labels), fixture canonicalises targets; named tests |
| 3 | Fixture written by the slice it judges | FIX: main owns the runner and fixture, with command, line count and SHA-256 recorded; K1 owns only the reading test |
| 4 | `begin_send` builds `Stored` too | FIX: added to K1 files and ownership, with an echo test |
| 5 | Hover and `R` refresh had no host | FIX: K2 builds `ParagraphView` (TextRenderer frame store + hover, TimelineView refresh); K4 only sets attributes; K3 and K4 share no file |
| notes | Slash-command length, quadratic text lookahead, whole samples, preprocess, nameless slash command, long language | FIX: 512-byte token scan; linear text rule and a long-word step pattern; whole samples in corpus (d); `\r\n`/tab preprocess; nameless form stays text (D2); language over 32 → `None` |

## Plan review dispositions (closing review of rev 3)

| # | Blocker | Disposition |
|---|---|---|
| 1 | A `TextRenderer` cannot see `AttributedString` keys, so the hover store stays empty and K2's lookup test cannot notice | FIX: segments become `Text(...).customAttribute(HoverTipAttribute)` joined by interpolation; K2 tests an offscreen render filling the store and hovers a stub tip manually |
| 2 | `R` refresh redraws a finished string; no locale route; K4 would edit K2/K3 files | FIX: `ParagraphView(build: (RenderContext) -> [Segment])` evaluated per timeline tick; `RenderContext{now, locale}`; K2 owns `ShellState.locale`, the environment key and MessageArea's property; K4 owns only its listed lines |
| notes | Grouping inside quotes/lists; predicate on velella output; canonicaliser at fixture time; LineBreak beside `Inline::Block`; `messages_tests.rs` ownership | FIX: grouping at every block-list level, compared pre-grouping at every level; predicate over input or oracle output only; Node `URL.href` (WHATWG, as the `url` crate); dropped by default, K0 may revise; ownership row added. `THIRD_PARTY_NOTICES.md` merges trivially between K3 and K4 |

## K1 rulings (main, on the executor's report)

The executor settled ten points the plan left open, each pinned by tests; main rules on them as follows.

| # | Point | Ruling |
|---|---|---|
| 1 | Line start: previous capture text ends in `\n` (plan) vs the oracle's last-regex-group check | Keep the plan's rule; no fixture input distinguishes them; pinned by `line_start_follows_the_previous_capture_text_not_its_last_group` (`> a\n# h` heads, `` `\n`# h `` does not). K0 may revise |
| 2 | List details (mixed bullets stay one list, ordered/unordered switch ends it, content column after marker spaces, shallower marker ends, deeper marker nests, spaces-only line is blank, `- ` alone is one empty item, code blocks allowed in items, item content starts a line so `- - a` nests, lists match at the start of a container that starts a line) | Accept as defaults; pinned in `list_cases_the_grammar_table_leaves_open`; K0 sample 3 may narrow |
| 3 | "Next to an `Inline::Block`" | Immediate sibling in the same inline list: accept |
| 4 | Inputs with blocks inside link labels | Filtered under D7: accept |
| 5 | Preprocess | All of simple-markdown's (lone `\r`, form feed included): accept |
| 6 | DEL in link targets | Not a C0/C1/Cf rejection; the url crate percent-encodes or rejects it: accept |
| 7 | Limit and fallback measure the raw content | Accept |
| 8 | Depth 16 allowed, the 17th nested parse aborts; step bound per (deepest level + 1) | Accept |
| 9 | Slash-command name class as a table generated from Node 24 (ICU 78.3, Unicode 17) | Accept |
| 10 | Emphasis uses deterministic walks, not memos (pairs depend on the walk's entry point) | Accept: the step test bounds it (worst 3.5 steps/byte/level, bound 64) |

## K1 verification

A fresh pilotfish:verifier found two P2 findings: `cargo fmt` drift in main's ruling test, and no test failing when the 8192-node limit was removed (the `wide` case tripped the input cap instead). Both are fixed, and a narrow re-verification is CONFIRMED: the node-limit and `>=` mutants now fail by assertion. Everything else was CONFIRMED with 38 mutants; two equivalent mutants survive (a scheme prefix and a host check that `url::Url` already enforces).

Advisory A1 (P3, deferred): the step counter does not tick inside a few helper scans (the list `line_end` scan, the 512-byte token window, the code-block language count, `take_while` loops). The step-bound test therefore covers ticked work only, and a list mutant that went quadratic passed it while its own named tests caught it. The shipped parser measured linear: 52 adversarial 256 KiB patterns took 0.58 s in total in release, and no fuzz input among 400k exceeded the bound. Add ticks to those helpers when the counter is next touched.

## K2 verification

A fresh pilotfish:verifier (26 mutants: 21 killed by assertion, 4 survivors noted below, 1 build error not counted) confirmed C1-C3 and C5-C7 and refuted C4 on one P2.

| # | Finding | Disposition |
|---|---|---|
| F1 (P2) | A name containing U+2069 closed velella's FSI early, so an RLO after it reversed the author's text; reply previews share the wrapper | FIX (main): Swift `MarkdownText.isolated` and core `isolate` drop a PDI the name has no isolate of its own to close and close every isolate the name leaves open; tests `aNameCannotCloseOrHoldOpenItsIsolate` and `a_name_cannot_close_or_hold_open_its_isolate_in_a_preview` (four nicks each), both killed by the keep-unmatched-PDI mutant |
| A3 (P3) | Hidden spoiler text likely readable by VoiceOver | FIX (main): ParagraphView's accessibility label replaces each covered run by "Spoiler" (`ParagraphView.spoken`, test `hiddenSpoilerTextNeverReachesTheAccessibilityLabel`) |
| A1 (P3) | No test fails when the view ignores the tick date (S11), `SpoilerCover(hidden: nil)` (S14), the handler returns `.systemAction` (S12), `row()` re-parses (R6, result identical), or the emoji host check goes | Emoji host check: FIX (`EmojiImages.fetchable`, test `onlyTheEmojiCDNOverHTTPSIsFetched`). S11, S12, S14, R6: DEFER (code correct by reading and by the verifier's render probe; add tests when the views are next touched) |
| A2 (P3) | IPv6-literal links do nothing (`[::1]` vs `::1` host forms) | DEFER: fails closed |
| A4 (P4) | Emoji deduplicated by (id, animated), not id | ACCEPT: cap still holds |
| A5 (P4) | Emoji fetches follow redirects; image cache never evicted | DEFER (ponytail-marked) |
| Review points | Trailing-"/" equality for Q5; FSI/PDI in previews | Trailing "/": safe (verifier). Previews: isolated, now with the F1 fix; a 200-char cut may leave an FSI open at the line end, which is harmless |

A narrow re-verification is CONFIRMED. The F1 fix holds under CoreText layout for the four test nicks and ten adversarial nicks the verifier chose; the old wrapper fails on several of those. All 6 fix mutants fail by assertion. It raised two new advisories:
- A-1 (P3, deferred): a bidi paragraph separator in a name still ends every isolate. U+001C-U+001E pass through both wrappers; `\n`, `\r`, U+0085 and U+2029 pass through the Swift body only, since the core preview turns them into spaces. An RLO after one of these reverses the text that follows. This predates the fix. It rises to P2 if Discord accepts such characters in nicks, role names or channel names. The fix is to turn paragraph separators in substituted names into spaces in both wrappers.
- A-2 (P4, deferred): no test fails if the `.accessibilityLabel` line is removed.

## K0 findings (official client, zh-TW, captures in refs/discord-ground-truth/captures/, git-excluded)

| Item | Official client | velella now | Action |
|---|---|---|---|
| Times t…S | Flexible day periods (`清晨5:20`), `2021/4/21`, `2021年4月21日 清晨5:20`, `2021年4月21日 星期三 清晨5:20`, `2021/4/21 清晨5:20(:30)` | ICU zh-TW gives the same shapes (`晚上9:20`, `2021/4/20`, `2021年4月20日 星期二 晚上9:20`) | Keep ICU; K5 adds tests at 05:20 Asia/Taipei asserting the capture's strings (separator of s/S: ICU uses U+2009, the capture is indistinguishable at its size) |
| R | **English even in zh-TW**: `5 years ago`, `6 hours ago`, `4 hours ago` | ICU in the account locale (`2小時前`) | K5: R always formats in `en-US`; singular/"now" wording stays ICU's until a capture shows Discord's |
| Header, today | **Time only** (`下午2:23`) | "Today at …" / "今天 …" | K5: today → short time only. Yesterday and older: awaiting a capture |
| `+ item` | Literal text | List (D1) | K5: `+` is not a list marker |
| `1234567890. x` | Ordered item shown as `1000000000.` | Literal (≤ 9 digits rule) | K5: any digit count is an ordered marker; `start` = min(value, 1_000_000_000), parsed saturating |
| `1. a` right after `- b` | Continues the bullet list as bullets | Ends the list (K1 ruling 2) | K5: a marker of the other kind continues the current list; ordered-ness comes from the first item |
| Plain links (`url`, autolink) | Show the canonical URL (`https://discord.com/`) | Show the typed text | K5: non-masked links display `Link.url` |
| Spoiler crossing bold (sample 3 lines 1-2) | Inconclusive (no cover visible, delimiters partly consumed) | Oracle's strict nesting | Keep; revisit with a clearer capture |
| Code block | Language header, line numbers, highlighting | K3: plain block (v2) | K6, styled after Vencord's ShikiCodeblocks (user, 2026-10-14) |
| Still to capture | Timestamp hover text; plain and masked link click warnings; yesterday and older headers | | K0b |

## K5 — K0 adjustments (SECURITY slice: parser grammar and link display), revision 2

**Owner:** pilotfish:security-executor, after K3 (merged). **Files:**
- core `markdown.rs`: `list_marker`, the list trigger set, the mixed-kind rule, step ticks in `list_marker` and `line_end`;
- `markdown_tests.rs`: list tests, the `d1_lists` predicate, step patterns;
- core `state.rs` with the smallest FFI and AppModel addition, for live locale updates;
- `MarkdownText.swift`: the plain-link label in `Numbering`;
- `MarkdownView.swift`: only the Q5 clause deletion and the stale marker comment at ~180;
- `DiscordTime.swift`: R in en-US, today header;
- tests.

| Part | Contract |
|---|---|
| Lists | `+` leaves both the list trigger set and `list_marker`. Ordered markers take any number of digits: `number = digits.fold(0u32, \|n, d\| n.saturating_mul(10).saturating_add(d)).min(1_000_000_000)`, with no shortcut on digit count and leading zeros counted, as JS `parseInt` does. A marker of the other kind continues the current list; the list's kind is its first item's. `list_marker` and `line_end` tick the step counter, which closes K1 advisory A1 for lists. |
| Differential predicate | `d1_lists` matches a line-start marker of `-`/`*` + space, or **one or more** digits + `. `. It reads the input text only. The edge sample's `1234567890. 十位數編號` line is counted under D1, and the run still reports 0 mismatches. Reverting to `1..=9` makes the run fail on that line. |
| Plain links | In `Numbering.inline`'s `.link` case, a non-masked link's label becomes `[.text(url)]` once, used by both `visibleText` and the returned node. The `visibleText + "/" == url` clause in `needsConfirmation` is deleted (now dead). Core labels are unchanged, so the differential fixture stays literal. |
| R | Always `en-US`, whatever the account locale (K0). |
| Header | Today → short time only (K0). Yesterday and older unchanged until K0b. |
| Locale updates | USER_SETTINGS_UPDATE reaches `ShellState.locale` without a reconnect, by the smallest addition (a session event or subscription). If a live session shows Discord sends only USER_SETTINGS_PROTO_UPDATE under capabilities 253, record it and keep READY as the source; add no protobuf decoder. |
| Test gaps closed | K4 A1 (READY clears an old locale); K4 A2 (R refresh from `context.now`, with now within 60 s); K2 A-2 (accessibility label attached: removing it fails a test); K2 S12 (the handler returns `.handled`). |
| Named tests | `1234567890. a` → start 1_000_000_000; `00000000000001. a` → 1; 16 KiB of digits + `. a` → 1_000_000_000 with no trap; `+ a` → text; `- b\n1. a` → one unordered list of 2 items; `1. a\n- b` → one ordered list; `https://a.b` shows `https://a.b/`; `https://a.b/ñ` shows `https://a.b/%C3%B1`; an IDN host shows `xn--…` and confirms; a masked link still confirms. Step patterns: a 1 MiB digit run + `. a`, and alternating `- a` / `1. a` lines. |
| Acceptance | All suites, clippy and fmt pass; differential 0 mismatches; a fresh verifier with mutants (no saturation, `+` back, kind switch ending the list, `visibleText` not using the url, predicate back to 1-9). **Live:** sample 3's `+`, ten-digit and mixed lists, and sample 2's links match the capture. |

### K5 security dispositions (security review of K5/K6, commit f05eec6)

| # | Finding | Disposition |
|---|---|---|
| P3-K5-1 | Any-length numbers: the fold must saturate, `+` must leave the trigger set, A1 ticks | FIX (Lists row, tests, step patterns) |
| P3-K5-2 | Plain links showing `Link.url`: no Q5 exploit (WHATWG output is ASCII, and IDN still confirms); trap if only `visibleText` or only the rendering changes | FIX (Plain links row); ASCII look-alikes REJECT, as in the official client |
| P4 | Live locale adds no new input; R in en-US has no security effect; stale marker comment | ACCEPT; comment updated in K5 |

## K6 — Vencord-style code blocks with syntect highlighting, revision 2

The user chose Vencord's ShikiCodeblocks look as the default and `syntect` as the highlighter (2026-10-14); the envelope's non-goal "syntax highlighting" is lifted for K6. **Order:** after K5 merges. **Owner:** pilotfish:security-executor. **Files:**
- `Cargo.toml` and `Cargo.lock` (velella-core only);
- `crates/velella-core/src/highlight.rs` and `highlight_tests.rs` (new);
- `crates/velella-core/assets/vscode-dark-plus/` (main vendors it; read-only to the executor);
- core `markdown.rs`: `Block::CodeBlock` gains `highlight: Option<Highlighted>` (the parser sets `None`), plus the `CodeBlock` literals in `markdown_tests.rs`;
- `messages.rs`: the three body assignments fill each `CodeBlock`'s `highlight` in place; `history()` parses before the lock;
- FFI `markdown.rs`: `MdBlock::CodeBlock` gains `highlight`, converted with the node;
- `MarkdownText.swift`: the `MarkdownBlock.codeBlock` case and its matches only;
- `Fixtures.swift`: `codeBlock` literals;
- `AppModel.swift`: the mapping of the new field;
- `MarkdownView.swift`: the code-block view only;
- `THIRD_PARTY_NOTICES.md`.

### What Vencord does (`refs/vencord/src/plugins/shikiCodeblocks.desktop`, GPL-3.0)

| Part | Value |
|---|---|
| Container | `background-base-lower` fill, radius 4; the highlighted root uses the theme's background |
| Header | Shown only when a language resolved: its display name, bold, padding 0 5, 6 below, with a colourless devicon 8 before it (`Header.tsx`, `shiki.css`) |
| Code | 0.875 rem, line height 1.125 rem, padding 0.5 em; wraps (`pre-wrap`, `overflow-wrap: anywhere`) |
| Line numbers | Table with 19 px rows; the number cell has padding 5 left and 8 right in the theme's plain colour; the code cell has 8 left |
| Copy | Appears on hover at the bottom right; "Copy" turns to "Copied!" for a 1 s cooldown |

### Language table (syntect 5.3.0 `load_defaults_newlines()` names, checked 2026-10-14)

| Fence aliases (case-insensitive) | syntect syntax | Header name |
|---|---|---|
| `py`, `python`, `py3` | Python | Python |
| `js`, `javascript`, `jsx`, `mjs`, `cjs` | JavaScript | JavaScript |
| `ts`, `typescript`, `tsx` | JavaScript (no TypeScript grammar is bundled; closest match) | TypeScript |
| `rs`, `rust` | Rust | Rust |
| `sh`, `bash`, `shell`, `zsh`, `console` | Bourne Again Shell (bash) | Shell |
| `c`, `h` | C | C |
| `cpp`, `c++`, `cc`, `hpp`, `cxx` | C++ | C++ |
| `cs`, `csharp`, `c#` | C# | C# |
| `go`, `golang` | Go | Go |
| `java` | Java | Java |
| `json`, `jsonc` | JSON | JSON |
| `html`, `htm`, `xhtml` | HTML | HTML |
| `css` | CSS | CSS |
| `sql` | SQL | SQL |
| `yaml`, `yml` | YAML | YAML |
| `xml`, `svg` | XML | XML |
| `rb`, `ruby` | Ruby | Ruby |
| `php` | PHP | PHP |
| `lua` | Lua | Lua |
| `diff`, `patch` | Diff | Diff |
| `md`, `markdown` | Markdown | Markdown |
| `hs`, `haskell` | Haskell | Haskell |
| `scala` | Scala | Scala |
| `pl`, `perl` | Perl | Perl |
| `r` | R | R |
| `makefile`, `make`, `mk` | Makefile | Makefile |
| `tex`, `latex` | LaTeX | LaTeX |
| `clj`, `clojure` | Clojure | Clojure |
| `erl`, `erlang` | Erlang | Erlang |
| `ml`, `ocaml` | OCaml | OCaml |
| `m`, `objc`, `objective-c` | Objective-C | Objective-C |
| `groovy`, `gradle` | Groovy | Groovy |
| `bat`, `cmd`, `batch` | Batch File | Batch |
| `d` | D | D |
| `pas`, `pascal` | Pascal | Pascal |
| `tcl` | Tcl | Tcl |
| `lisp`, `el`, `scm` | Lisp | Lisp |
| anything else (`swift`, `kotlin`, `toml`, `dockerfile`, …) or none | none | no header; plain text in the theme's plain colour |

Header names come only from this table, never from the fence text (shown with `Text(verbatim:)`). A test asserts that every mapped syntax name exists in `load_defaults_newlines()`.

### Contract

| Part | Contract |
|---|---|
| Dependency | `syntect = { version = "=5.3.0", default-features = false, features = ["parsing", "default-syntaxes", "regex-fancy"] }` in velella-core only, with `Cargo.lock` committed. Only `SyntaxSet::load_defaults_newlines()` is called: no `from_dump_file`, `from_binary`, `load_from_folder`, `ThemeSet` loading or `find_syntax_by_first_line`. Only the fence selects the grammar. Recorded checks: `cargo tree -p velella-core -e features -i syntect`; `cargo tree -i onig` and `-i onig_sys` empty; `cargo tree -i bincode` shows 1.3.3 via syntect only. THIRD_PARTY_NOTICES rows: syntect; fancy-regex with bit-set/bit-vec; bincode, noting RUSTSEC-2025-0141 (unmaintained; only the bundled dump goes through it); walkdir; fnv; regex-syntax; the sublimehq/Packages grammars; Vencord (`shikiCodeblocks`, GPL-3.0); VS Code (MIT). |
| Theme | Main vendors `dark_plus.json` and `dark_vs.json` from microsoft/vscode at `1b92929b5683438c738ded7708c5418e9ff476ec` (MIT), converted to strict JSON with comments and trailing commas removed and nothing else changed. Both SHA-256 values go in the assets README and THIRD_PARTY_NOTICES. Core embeds them with `include_str!` and resolves `include` only to the other embedded file. A `scope` may be a string or an array; a selector syntect cannot parse skips that entry only. Background and default foreground come from `colors.editor.background` / `editor.foreground`. The theme is held in a `OnceLock<Option<Theme>>`. Test: the shipped assets build a theme with 0 skipped entries and at least 60 rules, with `#569CD6` on `keyword`. |
| Computing | `highlight(code, language) -> Option<Highlighted>`, where `Highlighted{name, lines: Vec<Vec<Span{text, rgb, bold, italic, underline}>>}` and adjacent same-style spans are merged. **Carrier:** the highlight lives on its own node, as `Block::CodeBlock{language, code, highlight}`. The parser sets `highlight` to `None`. Every `Stored.body` assignment (`parse`, `merge`, `begin_send`) then walks the body mutably, including blocks inside `Inline::Block`, and fills each `CodeBlock`'s `highlight` from that block's own `code`. `row()` clones the body unchanged, and the FFI `From` conversion carries `highlight` with the node. No highlight ever crosses as a positional `Vec`. A nested `referenced_message` is never highlighted. The differential test compares `CodeBlock` without `highlight`, since the parser always leaves it `None`. A core property test, over the corpus and a fuzz set, checks that the spans' concatenated text equals the block's `code`. |
| Limits (untrusted input) | A block with a line longer than 1024 bytes, more than 2000 lines, or more than 4096 spans after merging gets `None`. One wall-clock budget of 25 ms per message, shared by its blocks, is checked before each line; once it is spent, every remaining block is `None`. The clock starts after the syntax set and theme exist; `highlight::warm()` builds them on a background thread when the message store is created, so the first block is not starved. fancy-regex keeps its default limit of 1,000,000 backtrack steps per search: syntect 5.3.0 has no setter and maps a limit error to no match. That bounds one search, not one line, so the line cap is the real bound. `history()` parses and highlights the fetched list **before** taking the store lock; `observe()` stays under the lock, bounded by the per-message budget. Measured acceptance, in release: on the adversarial corpus (1024-byte runs of `(`, `"\"`, `/*`, `/[`, `a=`, `<a `, `${`, backticks, `\\`, and one long word), through every aliased syntax and through `md`, the worst capped line takes ≤ 10 ms. If it doesn't, lower the cap and record the measured value. A message of 20 maximum-cost blocks finishes under 40 ms in total. |
| Panics | `catch_unwind(AssertUnwindSafe(...))` wraps only the computation, which writes to locals and returns its value. No velella state is mutated inside it, and a panic discards the whole result (`None`). The workspace never sets `panic = "abort"`. The default panic hook's stderr line is accepted: there is no logger, and stderr is not persisted. Tests: every syntax in `load_defaults_newlines()` highlights a 3-line sample without panicking; an injected panic gives `None`. |
| Swift view | `MarkdownView`'s code-block view takes the Vencord look:<br>• the header (name from the table, and the SF Symbol `chevron.left.forwardslash.chevron.right`, provisional, standing in for devicons, which are a web font);<br>• a line-number column, only for blocks of ≤ 2000 lines (longer ones keep K3's plain block);<br>• wrapped code at 0.875 × body with an 18 pt line height, on the theme background, radius 4;<br>• for display only, C0 except `\n` and `\t`, DEL, C1 and the bidi controls U+061C, U+200E, U+200F, U+202A-202E and U+2066-2069 drawn as `\u{XXXX}`, as `LinkConfirmation.escaped` does, with ZWJ untouched.<br>Spans are used only when their concatenated text equals the block's `code`; otherwise the block renders plain. Spoiler cover, container rules and `Equatable` stay as K3 left them. |
| Copy | A hover button at the bottom right writes the AST's `code` exactly: `clearContents()` then `setString(_:forType: .string)`. It never reads the pasteboard and shows "Copied!" for 1 s. There is no Copy control while the block is covered. |
| Tests | **Core:**<br>• the alias table, every row, including `ts` → JavaScript with the name TypeScript, and `swift` → `None`;<br>• the theme build;<br>• Dark+ colours for a fixed Python snippet;<br>• the concatenation property;<br>• limits: 1025-byte line, 2001 lines, 4097 spans, two slow blocks sharing one budget, injected panic;<br>• the all-syntaxes sweep;<br>• highlights computed at the three assignments and not in `row()`;<br>• the release timing corpus (`#[ignore]`d in debug, run by the verifier in release).<br>**Swift:**<br>• header only with a language;<br>• line numbers 1…n;<br>• wrapping;<br>• escapes shown;<br>• Copy writes the exact code, cools down, and is absent while covered;<br>• a hidden-spoiler code block before a visible one: the visible one shows only its own text.<br>**Verifier mutants:** no line cap, per-block instead of per-message budget, no `catch_unwind`, a Swift index without the equality check, Copy while covered. **Live:** sample 2's Python block against Vencord's look. |

### K6 security dispositions (security review of K5/K6, commit f05eec6)

| # | Finding | Disposition |
|---|---|---|
| P1-K6-1 | Highlight cost unbounded, under the store lock | Rev 2's fix (line cap, 25 ms in-lock budget) failed its release bounds. **Now FIX by isolation (rev 3.1):** no syntect on the session task or under the store lock; one worker thread per store computes, and the session path only flags messages and applies finished results (see "K6 revision 3.1") |
| P2-K6-2 | The JSONC theme would fail and silently disable highlighting | FIX: strict JSON vendored with SHA-256; string-or-array scopes; colours from `colors`; `OnceLock`; build test |
| P2-K6-3 | An index side table could attach a hidden block's spans to a visible one | FIX: highlights travel on their own `CodeBlock` node; the concatenation property in core; the equality check in Swift; spoiler-order test |
| P3-K6-4 | Panic handling conditions | FIX: Panics row |
| P3-K6-5 | Output size and view count | FIX: 4096-span cap; line-number layout only up to 2000 lines |
| P3-K6-6 | Copy and invisible control characters | FIX: exact code copied, escapes for display, no Copy while covered |
| P3-K6-7 | Supply chain | FIX: Dependency row (calls allowed, checks, notices, lockfile) |
| P4 | Header name source; first-use cost; test timing | FIX: names from the table; `warm()` (rev 3.1: runs first on the worker thread, never at store creation); release timing tests |
| P4 | Grammar recursion, deep nesting | REJECT: trusted bundled grammars; syntect caps the stack at 100 |

### Plan review dispositions (plan review of K5/K6 at f05eec6)

| # | Blocker | Disposition |
|---|---|---|
| K5-1, K6-1 | No security dispositions | FIX: the two tables above |
| K5-2 | `d1_lists` would miss the ten-digit line | FIX: Differential predicate row |
| K6-2 | Outside the envelope; no Files line or ownership | FIX: non-goal lifted, order and Files line added, ownership rows below |
| K6-3 | No alias table | FIX: language table, with every target checked against syntect's names |
| K6-4 | Limits do not bound the work | FIX: Limits row |
| K6-5 | Side-table index alignment | FIX: highlights on the node; equality checks |
| K6-1 (closing review, 7cc37be) | No type could carry a highlight onto the node within K6's files | FIX: `highlight` field on core `Block::CodeBlock`, filled in place; `markdown.rs`, `markdown_tests.rs` literals, `MarkdownText.swift` case, `Fixtures.swift` added to K6's Files and ownership; the user chose to start after this fix without another round (2026-10-14). Test: a hidden-spoiler code block before a visible one, through the FFI conversion, leaves the visible `MdBlock::CodeBlock`'s spans joined to exactly its own `code` |

### File ownership (K5, K6)

| File | K5 | K6 |
|---|---|---|
| `markdown.rs`, `markdown_tests.rs` | list grammar, predicate, ticks | `CodeBlock.highlight` field and literals (after K5 merges) |
| `state.rs`, FFI locale route, AppModel locale line | live locale | — |
| `MarkdownText.swift` | plain-link label | `codeBlock` case and matches (after K5 and T1 merge) |
| `Fixtures.swift` | — | `codeBlock` literals |
| `MarkdownView.swift` | Q5 clause, marker comment | code-block view |
| `DiscordTime.swift` | R, today header | — |
| `Cargo.toml`/`Cargo.lock`, `highlight.rs`, `highlight_tests.rs`, `assets/` (main vendors) | — | yes |
| `messages.rs` | — | rev 2: in-place fill at the three assignments; **replaced in rev 3.1** by its ownership table |
| FFI `markdown.rs`, AppModel mapping | — | `highlight` on `CodeBlock` |
| `THIRD_PARTY_NOTICES.md` | — | rows above |

## T1 — Discord message typography (user, 2026-10-14: "拆官方 Discord")

The user chose the official Discord client as the reference for the message area's type, sizes and spacing. The server rail, channel list and member list keep their approved v2 styling. Values were read from Discord's CSS in the local Discord cache. The user chose that source; only `.css` entries were read, and the `.css` filter was applied to cache keys before any body was read. Module `c19a55` is messages; `75297` is markup; `5126c` is the group and divider spacing. rem = 16 px, and 1 CSS px is one point on macOS. gg sans is proprietary and is not shipped, so velella uses the system font (SF Pro) at the same sizes. **Owner:** pilotfish:executor (UI only, no risk trigger), in parallel with K5 under the additive-edit rule; main resolves merge conflicts.

**Files:**
- `MessageArea.swift`: the row layout, header, grouping, date dividers and reply line;
- `DesignTokens.swift`: message tokens;
- `MarkdownText.swift`: base sizes only;
- `MarkdownView.swift`: the `MarkdownBlockStyle` constants for non-code blocks only;
- `ShellState.swift` `MessageItem`: author id;
- AppModel's message mapping: author id;
- tests.

### Discord values

| Element | Discord value (selector) | velella now |
|---|---|---|
| Message row | padding 2 top/bottom (`--custom-message-spacing-vertical-container-cozy: 0.125rem`); content inset 72 = avatar 40 + 2 × 16 (`--custom-message-margin-left-content-cozy`) | flat 16 gap; inset 68 |
| Avatar | 40 × 40 circle at x 16, 2 below the row's content top (`.avatar_c19a55`: `margin-top: calc(4px - 2px)`) | 40 at x 16 |
| Header line | line height 22, min-height 22 (`.cozy .header`) | — |
| Username | 16, weight 500 (medium), line height 22, then 4 before the timestamp (`.username`, `.headerText` margin-end .25rem) | 11 semibold, gap 8 |
| Timestamp | 12, weight 500, muted colour, baseline-aligned (`.cozy .timestamp`) | 10 |
| Body (markup) | 16, weight 400, line height 22 (`.markup__75297`, `--chat-markup-line-height: 1.375rem`) | 12 |
| Group start | +17 above (`.group-spacing-16 .groupStart`: `--custom-group-spacing-start: 1.0625rem`); Discord's default group spacing is 16 | none |
| Follow-up rows | no header or avatar; on hover a time in the 56 pt gutter, right-aligned, 11 pt, weight 500, muted, line height 22 (`.cozy .timestamp.alt`) | none |
| Date divider | margin 24 above and 8 below (`.group-spacing-16 .divider.hasContent`); a 1 pt line; a centred label 12, weight 600, line height 13, padding 2 × 4, radius 8, on the chat background (`.content__908e2`) | none |
| Reply | preview line height 18 (`--custom-message-reply-message-preview-line-height`), 4 below (`--reply-spacing`), indent 10 (`--custom-message-reply-indent`) | 10 pt line |
| Emoji | inline 1.375 em; jumbo (emoji-only message) 48 (`--custom-emoji-size-jumbo-emoji: 3rem`) | — |
| Headings | h1 24, h2 20, h3 16, **weight 700**, line height 1.375 em, margin 16 above / 8 below (8 / 4 when first) | 24/20/16 semibold, provisional spacing |
| Subtext (`-#`) | **14**, subtle colour, line height 19.25 | 11 |
| Quote | bar 4 wide, radius 4; text 12 from the bar and 8 at the end; 4 above and below; subtle colour | bar 4, gap 8 |
| Lists | indent 16, 4 above; each item 4 below; nested bullets disc → circle; ordered indent grows with the marker's characters (.4 em + n × .6 em) | indent 8, item gap 2 |
| Inline code | 85 % of the body, radius 4, padding 0 × .2 em, 1 pt border | 0.875 ×, no border |

### Contract

| Part | Contract |
|---|---|
| Tokens | Every value above is a named constant in `DesignTokens` (or `MarkdownBlockStyle`), with a comment citing its Discord selector. They replace the K3 values marked "awaiting K0" and the v2-derived message sizes. |
| Grouping | A row starts a new group when the previous row is from another author, is more than 7 minutes older, or sits across a date divider. **7 minutes** is Discord's commonly documented threshold; it was not read from Discord's code, since only CSS was read. It is a named provisional constant, to be checked live by the user against the official client. `MessageItem` carries `authorId`, mapped from the FFI row that already has it. |
| Date dividers | Between rows on different local calendar days. The label is the long date in the account locale (`DiscordTime`, `D` style), pending a K0b capture of the official label. |
| Rendering | The body's base size is 16 with a 22 pt line box (SwiftUI line spacing = 22 minus the font's line height, measured, not hard-coded). The header is 16 medium plus the 12 medium time. Avatars and headers appear only on group starts. Follow-up rows show the hover time in the gutter. |
| Tests | Grouping: same author within and over 7 minutes, author change, day change. Divider placement across days. Token values. An offscreen render: a group start's text begins at x 72, the avatar sits at x 16, and the body line pitch is 22. |
| Acceptance | `make build`, `swift test` and `cargo test` pass. A fresh verifier with mutants (threshold, divider placement, inset, line pitch). **Live:** the user compares a busy channel with the official client side by side. |

## T1 verification and deviations

A fresh verifier CONFIRMED T1 (every value against the CSS; 13 mutants killed). Recorded here as the repo's record:

| Item | Status |
|---|---|
| Inline code border, radius 4 and 0.2 em padding | Not done: runs carry only a fill; needs a `SegmentRenderer` attribute (K2's file). Size 85 % done |
| Emoji: inline 1.375 em (22 at 16) and jumbo 48 for emoji-only messages | Not done: inline custom emoji are ~19; unicode emoji are text |
| Blockquote `max-width: 90%` | Not done |
| Bold inside a heading | Lighter than the heading (runs are fixed at semibold, headings are 700): F3, P3 |
| Text position inside the 22 pt line box | ~1 pt higher than CSS centring (SwiftUI's exact line height) |
| Gap after a date divider | velella 8; Discord likely 8 + 17 = 25, since the presets' divider margin and group spacing always sum to 25. F1, P3, check live |
| Header and group-start render tests | Four mutants survive: group-start spacing, username weight, timestamp size, header gap. F2, P3 |
| Group-start min-height 44 (velella 46), divider trailing margin 14 (velella 16) | P4 |
| 7-minute group window, divider label text | Provisional, check live |

These are deferred to a T1 follow-up after K6, ordered by the user's live comparison.

## P1 — Polish from the first live look (user, 2026-10-14)

**Owner:** pilotfish:executor (UI and wiring; no risk trigger), in parallel with K6 under the additive-edit rule. **Files:**
- `ParagraphView.swift`: the tooltip;
- `MarkdownText.swift`: the trailing break, render side only;
- `MessageArea.swift`: the header time tooltip only;
- `ShellState.swift`/`ShellView.swift`: the locale default;
- `Sources/Velella/AppModel.swift`: remove the locale subscription;
- `crates/velella-ffi/src/lib.rs`: remove `subscribe_locale`/`LocaleListener`;
- `crates/velella-core`: remove the now-unused account-locale tracking (`State.locale`, `valid_locale`, `locale_update`, and their tests);
- tests.

| Part | Contract |
|---|---|
| Tooltip | The timestamp tooltip follows Swiftcord v2's `DiscordUITooltips` (`TooltipPresenter`: `sourceRect` + `spacing`; `TooltipSurface`), read from the binary with REA: position relative to the hovered run's frame (centred above it, v2's spacing), the surface (material/glass, corner radius, padding, font, colours) and the show and hide timing. Values go into named constants citing the v2 addresses; facts go into refs/swiftcord-v2-teardown.md (git-excluded, never decompiled code). Message header times use the same tooltip instead of `.help`. |
| Locale | Times use the macOS preferred language, `Locale.preferredLanguages.first`, through `Locale(identifier:)` without the user's overrides. The user's capture showed velella reading `en-GB` from READY's legacy `user_settings.locale` while the official client showed zh-TW, so the legacy field is stale. The Discord account-locale route (K4 and K5) is deleted, not left unused. Q2 is superseded. |
| Trailing break | A paragraph's final `LineBreak` is not rendered: HTML shows nothing for a lone trailing `<br>` in a block, and the official client renders `> 單行引用` on one line (user's capture). Two trailing breaks render one empty line. Only the rendering changes; the core AST and the oracle comparison are untouched. |
| Tests | Tooltip placement: centred above a run's frame at v2's spacing, clamped inside the view. The locale default follows the preferred language (injectable). A quote of one line has the height of one line, two trailing breaks give one empty line, and a single trailing break in a top-level paragraph is dropped. |
| Acceptance | `make build`, `swift test`, `cargo test`, clippy and fmt pass; a fresh verifier. **Live:** hover a timestamp pill and a header time, and compare with the v2 capture; `> 單行引用` is one line; times show zh-TW formats. **Live accepted by the user (2026-10-15: "P1驗收OK").** |

## K6 revision 3.1 — highlighting off the session task (user, 2026-10-14: "移到背景 worker")

The K6 implementation (uncommitted, worktree `agent-a0bc5870b5bbfffa8`, on 2d08b4e) failed its release timing acceptance. Measured on a loaded machine:
- the worst 1024-byte line took 320-341 ms (C#), against ≤ 10 ms;
- JavaScript took 151 ms and Scala 92 ms;
- 20 maximum-cost blocks took 959 ms, against < 40 ms.

Lowering the line cap does not fix it. Markdown's `\\` runs cost 12-25 ms at every length, and Makefile costs 20-90 ms from 64 bytes up. The cause is fancy-regex's 1,000,000-step backtrack limit per search, which syntect gives no way to change; one line runs many searches. So P1-K6-1 cannot be closed while highlighting runs in `observe()` under the store lock. The user chose to move highlighting to a background worker.

Revision 3 put jobs on a bounded push queue. Its plan review (7 blockers) and security review (P2-R3-1 … P4-R3-8) both found gaps, so revision 3.1 replaces it with a pull model: the store is the queue. Messages carry a `pending` flag, and one worker thread per store takes the newest pending message, computes off the lock, and applies the result if the message is still the same. Rev 3.1 supersedes rev 2's **Computing** row (with its carrier text) and **Limits** row, and the rev 2 test, mutant and ownership items mapped below. Rev 2's language table, Dependency, Theme, Panics, Swift view and Copy rows stand, except where a row below changes them.

### Contract

| Part | Contract |
|---|---|
| Session path | On the session task and under the store lock, the only highlighting code reached is the pure table lookup over the `LANGUAGES` const (the existing private `highlight::language`, and a new public `highlight::header(language: Option<&str>) -> Option<&'static str>` for the FFI). It touches no `OnceLock` static and no syntect. This covers `observe`, `history`, `parse`, `merge`, `begin_send`, `finish_send`, `snapshot` and `row`. `warm`, `compute` and the statics are reached only from the worker thread and from tests. Rev 2's `std::thread::spawn(highlight::warm)` in `MessageStore::new` is deleted, and `fill`/`fill_with` no longer exist as session-path entry points. |
| Carrier | `Block::CodeBlock.highlight: Option<Arc<Highlighted>>`; the parser sets `None`. `row()` clones the `Arc`, never the spans. The FFI conversion copies the spans after `snapshot()` returned, off the lock. `Highlighted.name` is removed: the header comes from the table (Header row). |
| Pending flag | `Stored` gains `pending: bool`, set to true exactly when a body is assigned to a message that goes into `open.messages` and at least one of its code blocks' fences maps in the language table: `parse(v, false)` (reaching the store through `observe` MESSAGE_CREATE, `history`, and `finish_send` Ok), and `merge` when `content` changed. A nested `referenced_message` (`parse(v, true)`) and an echo (`begin_send`) are never pending. The worker never reads `open.echoes`. `Open::insert` with `replace` keeps the existing copy's `body` and `pending` when the new copy's `content` is equal, so a re-delivered CREATE, or the gateway copy after `finish_send`, does not drop colours already shown (the body is a function of `content` alone). |
| Worker | `MessageStore::new(rest) -> Arc<MessageStore>`. Session.rs's `get_or_init` drops its `Arc::new`; that one line is K6's. `new` creates `sync_channel::<()>(1)`, keeps the sender as `wake: Option<SyncSender<()>>`, and starts one thread with `std::thread::Builder::new().name("velella-highlight").spawn(...)`. A spawn error leaves `wake: None`: highlighting is off for that store, and nothing panics. The thread holds only a `Weak<MessageStore>` and the receiver, and runs `run(weak, receiver, compute_job)`:<br>1. `warm()`; if it returns `None`, the thread ends.<br>2. `recv()`; an error (the store and its sender were dropped) ends the thread.<br>3. Loop: upgrade the `Weak` (failure ends the thread); `next_job()`, and on `None` go back to 2; drop the `Arc`; compute; upgrade again (failure ends the thread); `apply`.<br>`bump()` calls `wake()`, which is `try_send(())` ignoring `Full` (a wake is already pending) and `Disconnected` (the worker is gone). Every change, including the worker's own `apply`, therefore leads to a recheck for pending messages, and no enqueue site can be missed. |
| `next_job` | `fn next_job(&self) -> Option<Job>`, with `Job { id: u64, blocks: Vec<(Option<String>, String)> }` holding each code block's (fence, code) in walk order, mapped or not. Under the lock: take the newest message in the open channel with `pending` set, clear its flag, and copy its blocks. |
| Walk order | Checking for a mapped fence, collecting a job's blocks, and applying results all use one visitor over `&mut [Block]` (rev 2's `each_code_block`, which descends into quotes, list items and `Inline::Block`). So the order is the same by construction. |
| Computing and limits | `compute_job(&Job) -> Vec<Option<Arc<Highlighted>>>`, one entry per block, on the worker. Per block, as rev 2: a line over 1024 bytes, more than 2000 lines, or an unmapped fence gives `None`; `catch_unwind` wraps each block. Per job (one message): one 250 ms budget, checked before each line, and one allowance of 4096 spans. A block that would take the message past 4096 spans gets `None`, and so does every later block in that message. Once the budget is spent, every remaining block is `None`. A single line can overrun the budget, since fancy-regex cannot be interrupted; that costs only the worker's time. |
| `apply` | `fn apply(&self, job: &Job, results: Vec<Option<Arc<Highlighted>>>)`. Under the lock, with no syntect:<br>• message `job.id` must be in the open channel's `open.messages` (echoes are not searched);<br>• its code blocks must equal `job.blocks` in count and in each (fence, code), byte for byte.<br>Otherwise nothing is applied. An edited message is already pending again and gets its own job. Each block then takes its result only when the result's spans join to exactly that block's code; otherwise it stays `None`. `apply` bumps once if anything changed. There is no content fingerprint and no hash, so no crate is added. There is no epoch check either: id plus exact (fence, code) equality plus the join check already make any applied result that block's own highlight, so a result landing after the channel was closed and reopened is still correct (closing review, blocker 4). |
| Header | FFI `MdBlock::CodeBlock` gains `header: Option<String>`, set in the `From` conversion by `highlight::header(language)`, the same pure lookup. Swift's `MarkdownBlock.codeBlock` carries it. `CodeBlockModel` shows the header from that field whether or not a highlight is present, so a block's layout does not change when its colours arrive. `MdHighlighted.name` and `CodeHighlight.name` are removed. |
| Dead worker | Not restarted. A panic outside the per-block `catch_unwind` (in `warm`, `next_job` or `apply`) ends the thread. The store's mutex poison is already recovered (`lock()`), a partly applied body is still valid (each block was checked on its own), and `wake()` ignores `Disconnected`. That session's code blocks then stay plain. A deterministic failure would repeat on every restart; the end-to-end test catches a worker that never colours anything. |
| Thread QoS | DEFER: std's default. Utility QoS through `pthread_set_qos_class_self_np` waits for a flood test showing UI contention. |

### Tests

**Seam.** `MessageStore::with_wake(rest, wake: Option<SyncSender<()>>) -> Arc<MessageStore>` builds a store with no thread; `new` is a channel, `with_wake`, and the spawn. `run` takes the compute function as a parameter. Deterministic tests call `next_job`, `compute_job` and `apply` themselves.

| Test (core, deterministic) | Checks |
|---|---|
| Pending sites | One test per site: MESSAGE_CREATE, `history` Ok, `finish_send` Ok, and `merge` with changed content each make `next_job` return that id. `merge` with equal content, an echo with a `py` block, a reply whose only `py` block is in its `referenced_message`, and a body whose only fence is unmapped (`swift`) make it return `None`. After each, every `CodeBlock.highlight` in the snapshot is `None`: nothing was computed on the path. |
| Newest first | With two pending messages, `next_job` returns the newer. |
| Channel switch | Take a job in channel A; open B; `apply(A's job)` changes nothing; `next_job` returns only B's messages. Then close B and reopen A with the same message: `apply(A's old job)` colours it: the result is still that block's own. |
| History insert | `history` inserts a message with a mapped block, and `next_job` returns it. The ordering itself is structural, not tested: `pending` is assigned only inside `parse` and `merge`, before the `Stored` reaches `insert`, so no thread can see the message without its flag. |
| Edited during compute | Take a job; a MESSAGE_UPDATE changes the code; `apply` changes nothing; `next_job` returns the edited message. The same holds for a fence-only edit (`py` → `rb`, same code). |
| Evicted | Take a job; delete the message (and, separately, push it out past `CAP`); `apply` changes nothing and does not panic. |
| Join mismatch | `apply` with a result whose spans do not join to the block's code leaves `None`. |
| Replaced copy | A highlighted message, then a CREATE with equal content: still highlighted, with the same `Arc`. With different content: `None`, and pending. |
| Spoiler order | A hidden-spoiler block before a visible one: after `apply`, through the FFI conversion, each `MdBlock::CodeBlock`'s spans join to its own code (rev 2's test, run through `apply`). |
| Shared snapshot | Two snapshots with no change between them hold `Arc::ptr_eq` highlights. |
| Per-job caps | Two blocks of about 3000 spans each: the first gets colours and the second `None`. Two slow blocks share the 250 ms budget (injected clock, as rev 2). |
| Undrained wake | `with_wake(Some(tx))` with the receiver never read: 10,000 CREATEs, each with a mapped block, all return; the store holds at most `CAP` (200) messages, so at most 200 pending; memory is bounded by `CAP` alone. |
| Dead worker | `with_wake` whose receiver was dropped: `observe` neither blocks nor panics. |
| End to end | `new()` with the real thread: a CREATE with a `py` block gets colours in a snapshot within 5 s (polled). Dropping every `Arc` ends the thread within 5 s (`JoinHandle::is_finished`, from a test-only variant of `new` that returns the handle). With a compute function that blocks until the test drops the store, `Weak::strong_count` is 0 while compute runs, and the thread ends without applying. |

| Test (Swift) | Checks |
|---|---|
| Header without colours | A `py` block with `highlight: nil` shows "Python"; a `swift` block shows no header. Adding the highlight does not change the block's height. |
| Kept from rev 2 | line numbers 1…n; wrapping; escapes shown; Copy writes the exact code, cools down, and is absent while covered; the hidden-spoiler order test. |

Release tests are `#[ignore]`d. The verifier runs each one alone, so the statics start cold:

```bash
cargo test --release -p velella-core -- --ignored --exact <name> --test-threads=1
```

| Release test | Bound |
|---|---|
| Cold observe | Fresh process. `new()` starts the warm-up; then 10 MESSAGE_CREATEs, each holding 15 code blocks with the fence `cs` and one line of 1024 `(` characters (15,525 bytes, under the parser's `MAX_INPUT` of 16,384). **Precondition, asserted before the timing is trusted:** each stored body has 15 `CodeBlock`s with fence `cs`, and the message is pending. The worst single `observe` call (measured with `Instant`) is ≤ 5 ms. |
| Contended observe | A store of 200 messages, each holding one block with 4096 spans (applied through `apply`, synthetic spans), and a thread looping `snapshot()`. Over 100 `observe` calls, the worst is ≤ 5 ms. The longest `snapshot()` lock hold is recorded, with a bound of ≤ 5 ms. |
| Corpus | Rev 2's adversarial corpus plus backref grammars: heredoc `<<EOF` in bash, Ruby, Perl and PHP; Lua `[==[`; C# raw strings; Markdown fences. Each syntax's worst line is recorded in this plan; there is no bound. The `MAX_LINE_BYTES` comment is corrected: the cap limits a line's length, not its cost. |

### Verifier mutants (rev 3.1)

| Mutant | Expected killer |
|---|---|
| syntect (`compute`/`warm`) called from `parse`, `merge`, `observe`, `history` or `finish_send` | Cold observe; pending-site snapshot `None` checks |
| `pending` not set at one site (each site separately) | Pending sites |
| `history` inserting with `pending: false` | History insert |
| `next_job`/`apply` also searching `open.echoes` | Pending sites (echo case) |
| a message pending because its `referenced_message` has a mapped fence | Pending sites (a nested-reply case where only the referenced message has a `py` block) |
| `apply` comparing code without the fence | Edited during compute (fence-only edit) |
| `apply` without the join check | Join mismatch |
| an `Arc<MessageStore>` held across compute | End to end (`strong_count`) |
| `MAX_SPANS` counted per block again | Per-job caps |
| `row()` deep-cloning `highlight` | Shared snapshot |
| `wake()` with a blocking `send` | Undrained wake (the test hangs, so the harness times out) |
| `next_job` oldest first | Newest first |
| `insert` replace dropping the body for equal content | Replaced copy |
| the header taken from the highlight again | Header without colours |

### Rev 2 items

| Rev 2 item | Disposition |
|---|---|
| Tests: alias table; theme build; Dark+ Python colours; concatenation property; 1025-byte line; 2001 lines; injected panic; all-syntaxes sweep | keep |
| Test: 4097 spans | replaced by per-job caps (per message) |
| Test: two slow blocks sharing one budget (25 ms) | replaced by per-job caps (250 ms) |
| Test: "highlights computed at the three assignments and not in `row()`" (`messages_tests.rs:695` in the worktree) | replaced by pending sites and shared snapshot |
| Test: release timing corpus with a ≤ 10 ms per-line bound | replaced by the release Corpus test (recorded, no bound) |
| Swift: header only with a language | replaced by header without colours |
| Swift: line numbers, wrapping, escapes, Copy, spoiler order | keep |
| Mutants: no line cap; per-block instead of per-message budget; no `catch_unwind`; Swift index without the equality check; Copy while covered | keep |
| `std::thread::spawn(highlight::warm)` at `MessageStore::new` (`messages.rs:681`) | drop (the worker warms) |
| `history()` highlighting before the lock | drop (nothing to highlight on the path) |
| Ownership: `messages.rs` "in-place fill at the three assignments" | replaced by the ownership table below |

### Files and order

| File | K6 rev 3.1 owns |
|---|---|
| `Cargo.toml`, `Cargo.lock` | syntect in velella-core only (rev 2); nothing added in rev 3.1 |
| core `lib.rs` | `mod highlight` |
| core `highlight.rs`, `highlight_tests.rs` | table lookups, `compute_job`, per-job caps, comment fix, corpus |
| core `markdown.rs`, `markdown_tests.rs` | `CodeBlock.highlight: Option<Arc<Highlighted>>` and its literals |
| core `messages.rs`, `messages_tests.rs` | `pending`, `insert` body keep, `wake`, `with_wake`/`new`, `run`, `next_job`, `apply`, tests |
| core `session.rs` | the one `get_or_init` line |
| assets `vscode-dark-plus/` | read-only (main vendored them) |
| FFI `markdown.rs` | `header`, `highlight` conversion, spoiler-order test |
| `MarkdownText.swift` | the `codeBlock` case (with `header`) and its matches |
| `Fixtures.swift` | `codeBlock` literals |
| `AppModel.swift` | the `codeBlock` mapping |
| `MarkdownView.swift` | the code-block view |
| Swift tests (`CodeBlockTests.swift`, `MarkdownBlockTests.swift`, `MarkdownTests.swift`) | code-block tests |
| `THIRD_PARTY_NOTICES.md` | rev 2's rows |

`state.rs`, FFI `lib.rs`, `session_tests.rs` and `fixture_tests.rs` are not K6's. P1 changed them, and P1 is merged (06ca89d).

**Order.** P1 merged first. The K6 executor:
1. commits its rev 2 work-in-progress on `worktree-agent-a0bc5870b5bbfffa8`;
2. merges `m1` (06ca89d) into that branch, resolving the two overlapping files by keeping both sides:
   - `AppModel.swift`: P1's removal of the locale subscription and K6's `codeBlock` mapping;
   - `MarkdownText.swift`: P1's trailing break and K6's `codeBlock` case;
3. makes the rev 3.1 changes on top.

Main reads the merge commit's conflict hunks at integration.

### Acceptance

`make build`, `swift test`, `cargo test`, clippy and fmt pass. `cargo tree -p velella-core` shows no crate beyond rev 2's record. A fresh pilotfish:verifier runs the release tests and the mutants above. **Live:** sample 2's Python block against Vencord's look; a message with code blocks shows its header at once and gains colours without moving.

### K6 rev 3 security dispositions (security review of rev 3, 2026-10-14)

| # | Finding | Disposition |
|---|---|---|
| P2-R3-1 | The bounded push queue loses colours: a fast job reaches `apply` before `history`'s insert; a channel switch fills the queue with stale jobs; a replaced copy loses its colours with no retry | FIX: pull model (`pending`, `next_job`, wake channel of 1); `insert` keeps the body for equal content; tests: channel switch, history race, replaced copy |
| P2-R3-2 | Highlight data cloned under the store lock on every snapshot; span cap per block | FIX: `Option<Arc<Highlighted>>`, FFI conversion off the lock, 4096 spans per message; contended-observe and lock-hold release measurements |
| P3-R3-3 | A per-process worker holding `Arc<MessageStore>` keeps a signed-out session's token and message text alive; cross-session results | FIX: one worker per store holding only `Weak`, no `Arc` across compute, ends when the store drops; end-to-end drop test |
| P3-R3-4 | One line's worker cost is not bounded (backref patterns are not cached); the cap comment overstates it | FIX: backref grammars in the release corpus, worst line recorded per syntax, comment corrected. REJECT a per-sender cap and process isolation (cosmetic only) |
| P3-R3-5 | `warm()`'s `OnceLock`s would block any session-path caller for the whole warm-up; `finish_send` was missing from the list | FIX: Session path row (only the pure table lookup; `finish_send` listed); the cold-observe release test runs in a fresh process during warm-up |
| P4-R3-6 | Fingerprint choice | FIX: no hash; exact (fence, code) equality in `apply`, one visitor for the walk order |
| P4-R3-7 | Panics outside `catch_unwind` kill the worker | FIX: Dead worker row (no restart), dead-worker test |
| P4-R3-8 | The Rendering row promised a header that the Swift view only shows with a highlight | FIX: Header row (option a, from the table through the FFI, no syntect) |
| P4 | Timing side channel, logging | ACCEPT: highlighting makes no network request and logs nothing |
| P4 (main) | The FFI conversion copies up to 200 × 4096 spans per snapshot, off the lock, on a tokio thread | DEFER: unmeasured; measure if a live flood shows lag |

### Plan review dispositions (plan review of rev 3)

| # | Blocker | Disposition |
|---|---|---|
| 1 | No rev 3 security review; P1-K6-1's row described rev 2 | FIX: the table above; P1-K6-1's row now cites isolation |
| 2 | Routing, enqueue sites, echoes, apply method, `Disconnected` | FIX: one worker per store holding `Weak`; enqueue is the `pending` flag at the listed sites, and every `bump` wakes; echoes neither highlighted nor searched; `next_job`/`apply` signatures; `Disconnected` ignored and no restart (the security review rejected restart: a deterministic failure repeats), with the end-to-end and dead-worker tests |
| 3 | Fingerprint left open; a SHA-256 crate | FIX: no fingerprint; (fence, code) equality; no crate |
| 4 | Header promised for `None` | FIX: option (a), `header` through the FFI from the table |
| 5 | Rev 2 items unmapped; no rev 3 mutants | FIX: Rev 2 items table; Verifier mutants table |
| 6 | Timing and flood checks not falsifiable | FIX: exact payload (20 × one 1024-byte `cs` line of `(`), run command, worst-of-N measurement; the `with_wake` seam; the 64/9,936 overflow count no longer applies (there is no job queue), replaced by the undrained-wake test (10,000 CREATEs return, at most `CAP` pending) |
| 7 | No Files line or ownership; merge order with P1 | FIX: Files and order (P1 first, the executor merges m1 and resolves the two files) |

### Closing review dispositions (closing review of rev 3.1, 3373286)

All four blockers were test falsifiability; the design stands. The user chose to fix them and start without another review round (2026-10-14: "修正後直接開工").

| # | Blocker | Disposition |
|---|---|---|
| 1 | Cold observe's 20 × 1035-byte payload exceeds `MAX_INPUT` (16,384), so it parses as plain text and the bound proves nothing | FIX: 15 blocks (15,525 bytes) plus a precondition assertion (15 `cs` `CodeBlock`s, pending) |
| 2 | "`pending` set after the insert is visible" cannot be killed by a sequential test | FIX: the ordering is stated as structural (assigned only in `parse`/`merge`, before `insert`); the mutant is replaced by "`history` inserting with `pending: false`" |
| 3 | "`pending` set for an echo or a nested reply" is an equivalent mutant | FIX: replaced by "`next_job`/`apply` also search echoes" and "pending because the referenced message has a mapped fence", each with a killing Pending sites case |
| 4 | The Channel switch test does not kill "`apply` without the epoch check" | FIX: the epoch check is deleted as a redundant guard (id + exact (fence, code) + join already decide correctness); its mutant is gone; the Channel switch test adds the reopen case showing the late result is applied and correct |


## K6 rev 3.1 verification (merged 5dac025, 2026-10-15)

A fresh verifier at 3a826b5 checked every contract row against the code. It traced the call graph for session-path isolation instead of relying on grep. 24 of 25 mutants were killed. The survivor (F1, P2) was `warm()` called from `parse`: the cold-observe test warmed the statics with untimed twin-store observes before timing. d33f27a moved the twin store after the timed loop. A second fresh verifier CONFIRMED the fix: the mutant failed 5 of 5 runs at 536-620 ms against the 5 ms bound, and the restored code passed at 0.06-0.37 ms. Integrated m1 at 5dac025 passes core 248 (3 ignored release tests), FFI 13, UI 124 and login 17.

| Release measurement (loaded machine, each test run alone) | Value |
|---|---|
| Cold observe, worst of 10 (15 `cs` blocks of 1024 `(`) | 0.03-0.37 ms (bound 5 ms) |
| Contended observe, worst of 100 | 0.072-0.198 ms (bound 5 ms) |
| `snapshot()` lock hold, worst (upper bound: whole call) | 0.18-0.62 ms (bound 5 ms) |
| `warm()` | about 0.5-0.6 s release; about 6 s debug (the end-to-end test's liveness limit is 60 s for that reason) |

The corpus records the worst line per grammar after warm-up, from the verifier's run. It covers rev 2's adversarial lines plus heredoc, `[==[`, raw-string and fence openers. There is no bound; this only delays colours on the worker.

| Grammar | ms |
|---|---|
| C# | 161 (executor: 200-270) |
| JavaScript | 86 (executor: 134-141) |
| Scala | 46 |
| Makefile | 28 |
| Ruby (heredoc) | 27 |
| Rust | 25 |
| Bash (heredoc) | 25 |
| YAML | 15 |
| PHP (heredoc) | 15 |
| Markdown | 9 |
| D | 9 |
| C++ | 8 |
| Perl | 7 |
| every other grammar | < 6 |

**Interpretations accepted by the verifier:**

| Choice | Detail |
|---|---|
| Constructor | `new` uses `Arc::new_cyclic`, so a failed spawn really leaves `wake: None`. `with_wake` is test-only. `start(rest, compute)` returns the handle. |
| Spoiler-order test | Split into a core half, through `apply`, and an FFI half, through the conversion. |
| Contended observe | Uses ids that `CAP` evicts at once. |
| Budget start | The budget starts at job start, after `warm`. |
| Join failure | A failed join in `apply` leaves the block as it was. |
| End-to-end test | Polls for liveness with a 60 s limit; the drop check stays at 5 s. |

The executor dropped the per-grammar warm-up once, which would have let a heavy grammar's first block go plain for good under load. Main rejected that, and 3a826b5 restored it. **Live (user):** sample 2's Python block against Vencord's look; a message with code blocks shows its header at once and gains colours without moving.
