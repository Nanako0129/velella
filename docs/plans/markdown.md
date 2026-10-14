# velella Markdown and Time Plan (revision 3.1)

Repo: /Users/nanako/side-project/velella, branch `m1` (M2 integrated; the working tree also holds four uncommitted changes: per-conversation drafts, the v2 member list, the Makefile signing step and the message skeleton). `docs/plans/m1.md` and `docs/plans/m2.md` still apply: the refs and licence policy, the checkpoint convention, the logging rule (no logger; the parser and renderers never log content, URLs or names), the FFI mirror pattern and the M2 security dispositions except F5, restated below.

The user asked for full Discord markdown because Swiftcord's is incomplete (a thread member reported it on 2026-10-14), chose the approach ("照你建議", 2026-10-14), asked that times follow Discord's localisation exactly ("時間格式要照discord完全的本地化相容"), and showed v2's timestamp hover (a tooltip with the full date, e.g. "Wednesday, April 21, 2021 at 5:20 AM").

Revision 2 folds in the plan review of rev 1 (REVISE, 9 blockers, all FIX) and the security review of rev 1 (P1-P3 FIX, P4 noted), and adds a differential oracle so the parser's behaviour is checked mechanically, not by judgment. Revision 3 folds in the plan review of rev 2 (REVISE, 5 blockers, all FIX; see the disposition table at the end); per the review cap it got one closing review (REVISE, 2 blockers on the K2→K4 interface). Revision 3.1 applies those two fixes and the review's notes; the review cap pauses here for the user's decision.

## Envelope

| Field | Value |
|---|---|
| Outcome | Message content renders as the official Discord client renders it: bold, italic, underline, strike, spoilers, inline code, fenced code blocks, headings 1-3, subtext, quotes (`>` and `>>>`), bullet and numbered lists, masked links, plain and `<…>` links, escapes, user/role/channel mentions as pills with names and colours, @everyone/@here, custom emoji as images, unicode emoji, slash-command mentions, guild navigation tokens and `<t:…>` timestamps in all nine styles with a full-date hover. Message header times use the Discord account's locale and Discord's formats. |
| How it is seen working | The user compares velella with the official client on the three samples posted in the dev thread (`1559813791384870953` time styles, `1559813797764669537` markdown, `1559816987192852531` edge cases) and on ordinary channel history. |
| Non-goals | Syntax highlighting inside code blocks (later; SimpleAST `CodeRules`, Apache-2.0). Embeds, reactions, stickers, other gap-report items. Composer markdown. Mention profiles, channel-mention navigation. |
| Traffic added | Custom emoji images: `https://cdn.discordapp.com/emojis/{id}.{webp|gif}?size=48`, built in core from the parsed id, loaded by the views without credentials when a row is on screen, at most 64 distinct ids per row. Links open in the default browser only when the user clicks them. Nothing else. |
| Order | **K0** (ground truth; user + main) → checkpoint commit → **K1** (parser) → **K2** (FFI, resolution, inline rendering, links) → **K3** (blocks) and **K4** (times) in parallel worktrees. |
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
