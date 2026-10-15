# velella Wave 3 Plan (revision 1)

Repo: /Users/nanako/side-project/velella, branch `m1`. The user asked for faster progress through parallel work (2026-10-15: "開發進度太緩慢了，你能拆分工作，平行處理嗎？") and chose four tracks to run beside the selection spike N0: older history, reply sending with the typing indicator, reactions (read-only), and embeds. `docs/plans/m2.md` and `docs/plans/markdown.md` still apply (REST rules, F5 rendering, link confirmation, logging: none).

**Process (user, 2026-10-15):** short contracts. Tracks P, R and X have no risk trigger beyond what M2 already reviewed, so they start at once and each gets a fresh pilotfish:verifier after it is done. Track E renders untrusted embed content, so it gets a security review and a plan review in one round before it starts.

## Shared rules for all four tracks

| Rule | Detail |
|---|---|
| Worktrees | Each track runs in its own worktree from m1; nothing is pushed; main merges in the order P, R, X, E and resolves conflicts |
| Additive edits | New fields go at the end of `MessageRow` (core and FFI), `MessagesSnapshot`, `MessageItem` and their initialisers (new parameters with defaults, last). New logic goes in new files where it can |
| Not yours | `MarkdownView.swift`, `ParagraphView.swift`, `MarkdownText.swift`, `Tooltip.swift` (the selection work N1 rewrites them) except where a track's row says otherwise; `docs/`; memory |
| Live data | Tests use the mock HTTP server and mock gateway; nobody connects to Discord; nobody launches velella or any GUI app |
| Acceptance | `CARGO_NET_OFFLINE=true make build`, `swift test`, `cargo test`, `cargo clippy --all-targets -- -D warnings`, `cargo fmt --check` pass; then a fresh pilotfish:verifier with mutants; then main's live check |

## P — Older history

| Part | Contract |
|---|---|
| Core | `Rest::channel_messages(channel_id, before: Option<u64>)` adds `&before=<id>` (`u64` only). `MessageStore` gains `load_older(epoch)`: one GET at a time per open channel, `before` = the oldest buffered id, merged by id like the first page (ascending, deduplicated, deleted-during-fetch ids skipped). A page with fewer than 50 messages marks the start reached; then `load_older` does nothing. The cap becomes 1000; an older page that would exceed it is not requested (start treated as reached). A failure keeps the rows and allows a retry. The snapshot gains `older: OlderState { idle, loading, reachedStart, failed(text) }` |
| FFI/Swift | `MessageSubscription.load_older()`; `ShellState` carries the older state; `MessageArea` asks for an older page when its first row appears and the state is idle; a small spinner row at the top while loading; the visible row keeps its position on screen when older rows are prepended (anchor the scroll position on the previously first row) |
| Traffic | One extra `GET /channels/{id}/messages?limit=50&before={id}` each time the user reaches the top; never automatic beyond that |
| Tests | GET path with `before`; merge order and dedupe; deleted-during-fetch skipped; short page sets reached start; no second request while loading or after reaching the start; cap; failure then retry; an older result for a closed epoch is dropped. Swift: the top row triggers one request; position kept after a prepend (offscreen host) |

## R — Reply sending and the typing indicator

| Part | Contract |
|---|---|
| Reply | `send_message(channel_id, content, reply_to: Option<u64>)`; with a reply it posts `message_reference: {message_id, channel_id, fail_if_not_exists: false}` and leaves mentions at Discord's default (the replied user is pinged, as the official client does by default). The echo carries the reply preview. The composer's reply bar sends its target and clears after the submit. `ReplyBar` renders the name with `Text(verbatim:)` and `MarkdownText.isolated` (the E0 deferred item) |
| Typing | Core observes `TYPING_START` for the open channel: user id and arrival time; an entry expires 10 s after its last event or when that user's message arrives; the self user is ignored. The snapshot gains `typing: Vec<String>` (display names resolved as message authors are, member nick, then global name, then username) and the store bumps on expiry. Swift shows Discord's line under the composer: "A is typing…", "A and B are typing…", "A, B and C are typing…", "Several people are typing…" (4+), names verbatim and isolated, with three animated dots (Discord: 7 pt dots, 2 pt gap, 1.2 s pulse) |
| Not done | velella never sends typing events |
| Tests | Reply body JSON exact; no `message_reference` without a reply; echo preview; reply bar clears; typing add, refresh, expiry at 10 s (injected clock), removal on that user's message, self ignored, other channels ignored; the four label forms; verbatim and isolated names |

## X — Reactions (read-only)

| Part | Contract |
|---|---|
| Core | Parse a message's `reactions`: `count`, `me`, `emoji {id, name, animated}` (custom emoji URL from core's existing CDN rule; unicode emoji as its `name`). Live: `MESSAGE_REACTION_ADD`, `_REMOVE`, `_REMOVE_ALL`, `_REMOVE_EMOJI` for buffered messages of the open channel; `me` follows events whose `user_id` is the self user; a count reaching 0 removes the reaction; order is first-seen. At most 20 reactions per message are kept (Discord's limit); burst/super reactions count as normal ones |
| FFI/Swift | `MessageRow.reactions` → `MessageItem.reactions`. A new `ReactionsView` under the message (after attachments and embeds): pills with radius 8, padding 2×6, emoji 20 pt, count 14 pt semibold with 6 before it, 4 between pills; `me` pills use Discord's brand-tinted fill and border; custom emoji through `EmojiImages`, unicode emoji as verbatim text; read-only (no click) |
| Tests | Parse; each live event; `me` from self events only; zero removes; cap 20; events for other channels or unbuffered messages ignored. Swift: pill layout values; `me` styling; verbatim emoji name |

## E — Embeds and media (SECURITY slice), revision 1.1

Revision 1.1 folds in E's plan review (REVISE, 2 blockers) and security review (E-1 … E-17, no P0/P1); both tables follow the contract. F11 (the image host allow-list deferred from M2 to "the embeds milestone") is closed here for attachments too.

| Part | Contract |
|---|---|
| Media URLs (core) | `media_url(raw) -> Option<String>`: `url::Url::parse`; scheme `https`; `host_str()` equal by exact string to one of `media.discordapp.net`, `images-ext-1.discordapp.net`, `images-ext-2.discordapp.net`, `cdn.discordapp.com`; `port().is_none()`; no username or password; no C0/C1/Cf character in `raw`; returns `as_str()`. The host list is one documented constant that Swift mirrors. |
| What is parsed (core) | At most 10 embeds. Text: `title`, `description`, `url`, `color` (unsigned integer ≤ 0xFFFFFF, else none), `timestamp`, `author.name`/`author.url`/`author.proxy_icon_url`, `footer.text`/`footer.proxy_icon_url`, `fields` (≤ 25; `name`, `value`, `inline`), `provider.name`/`provider.url`, `thumbnail` and `image` as `proxy_url` + `width` + `height` + `description` (alt text), and `is_video` (type `video`/`gifv` or a present `video`). Never parsed or carried across FFI: `url`, `icon_url`, `thumbnail.url`, `image.url`, `video.url`, `video.proxy_url`. |
| Budgets (core) | Per-field caps (title, author name and field names 256, description 4096, field value 1024, footer 2048), cut by `char`; then Discord's 6000-character combined budget over title, description, field names and values, footer text and author name, in document order; text past it is dropped. A cut description or value containing `||` becomes one `Spoiler` (the markdown fallback rule). Descriptions and field values are parsed once in `parse`/`merge` with core's markdown limits and stored; `row()` only walks them; embed code blocks are never highlighted (no `pending`). Custom emoji in embeds count toward the row's 64-image cap with the body. |
| Images (core) | Thumbnail and image use `media_url(proxy_url)` with positive `width` and `height` ≤ 16384; anything else gives no image. Author and footer icons use `media_url(proxy_icon_url)` with no dimension check and a fixed display size (24 pt author, 20 pt footer). |
| Links (core) | `url`, `author.url` and `provider.url` become `LinkTarget`s through the same checked path as message links (http(s), host, no userinfo, the C0/C1/Cf rejection) plus a 2048-byte cap; anything else renders as plain text. |
| Spoilers (core) | An embed whose canonical `url` equals the target of a link inside a `Spoiler` of the body is covered as one unit: its links are inert, its images are not fetched until revealed, accessibility exposes only "Spoiler". An image attachment with flag `IS_SPOILER` (1 << 3) or a `SPOILER_` filename is covered the same way. |
| Updates (core) | In `merge`, a present `embeds` (including `[]`) replaces the stored ones and an absent one keeps them; a present `flags` with bit 2 (`SUPPRESS_EMBEDS`) hides them and clearing it shows them again; a `MESSAGE_CREATE` re-insert takes the new copy's embeds. |
| Timestamp (core) | Grammar `YYYY-MM-DDTHH:MM:SS(.d{1,9})?(Z|±HH:MM)`, checked arithmetic, years 1970-9999, otherwise none; i64 ms, clamped again in Swift before any `Int` conversion (K4 rule). |
| Image loading (Swift) | One `MediaImages` loader for embed images, icons and image attachments (attachments move off `AsyncImage`): an ephemeral `URLSession` with no cookie storage, no cookies, no credential storage and no added headers; in-memory cache only; a redirect is followed only if its target passes the Swift host check, else cancelled; the body is refused above 16 MiB (`expectedContentLength` and while streaming); the image is decoded with `CGImageSourceCreateThumbnailAtIndex` at most 2 × the display box's longest side after `CGImageSourceCopyPropertiesAtIndex` shows ≤ 50 MP; the request asks the proxy for the display size (`width`/`height` × backing scale; provisional, confirmed on a live payload); an `NSCache` with a byte cost limit; loads cancel when the view disappears. Before any fetch Swift re-checks: `https`, host in the mirrored set, `port == nil`, no user or password. A test-only seam lets tests point the session at 127.0.0.1. |
| Display (Swift) | `EmbedView` in Discord's card look: left accent bar from `color` (default bar otherwise), author row, title as a link, description and field values through separate `MarkdownView`s, inline fields in up to 3 columns, thumbnail top-right, image below, footer with time. Boxes fit (w, h) inside 400 × 300 for images and 80 × 80 for thumbnails, keeping the ratio, in `Double` (provisional maxima, checked against a capture); the same clamp applies to image attachments. A video or gifv shows its preview (`thumbnail` or `image`) and never plays; no AVKit or web view. Title, author name, field names, footer text, provider name and alt text use `Text(verbatim:)`; any of them sharing a line with other text goes through `MarkdownText.isolated`. Image accessibility labels are the alt text verbatim or "Image", never a URL. |
| Embed links (Swift) | `EmbedView` routes `url`, author and provider links through a `LinkRouter` built from a `MessageBody` holding only those targets with `masked: true`, so every embed link confirms; the card root installs `.environment(\.openURL)` returning `.handled`; no `Link`, no direct `NSWorkspace` call, no http(s) `.link` attribute in `EmbedView`. `MarkdownView.swift`, `ParagraphView.swift` and `MarkdownText.swift` stay unedited. |
| Traffic | Embed images, icons and image attachments load only from the four Discord media hosts, without cookies or credentials, when a row is on screen; no origin server sees the viewer |
| Not done | Video playback; the CFNetwork User-Agent difference from Electron (DEFER, exposure exists since M2); explicit-media filters (DEFER) |
| Tests | **Core:** the `media_url` table (accepted: the four hosts, an uppercase host, `:443`; rejected: `http://`, `:8443`, `user@`, trailing dot, `media.discordapp.net.evil.test`, `https://evil.test\@media.discordapp.net/`, a Cyrillic homograph, scheme-relative, `file:`, `data:`, empty, an allowed `url` with no `proxy_url`); a fixture whose `url`/`icon_url`/`video.*` point at `evil.test` leaves no `evil.test` in the FFI row and the icon equals the proxy URL; field caps; 10 × 25 full fields leave ≤ 6000 characters; a cut `"a"*4090 + "||secret||"` leaves no `secret` outside a `Spoiler`; a 4097-character CJK description does not panic; a parse counter shows N snapshots cause 0 embed parses; an embed code block never sets `pending`; non-http(s), userinfo, forbidden-character and `:8443`... links render as text; a spoilered-link embed is covered and fetches nothing, the same link unspoilered is shown; `IS_SPOILER` and `SPOILER_` attachments are covered; update with `[]`, without `embeds`, `flags: 4`, then `flags: 0`; timestamps (Discord style, `Z`, and garbage incl. a 1 MB string); colours (−1, 2^32, 1.5, "red" → default); emoji cap 60 + 10 → 64. **Swift:** the `fetchable` table mirroring core's; the loader against a local server (a 302 to a foreign host never contacts it; a `Set-Cookie` brings no `Cookie` next request; a 17 MiB body and a PNG declaring 20000 × 20000 give no image; the request carries `width`/`height`); 1 × 65535 and 65535 × 1 boxes stay inside the maxima; a title tap opens only after `accept()`; a stray `velella-link:99` is `.handled`; a masked link in a field value confirms; a covered card's accessibility label holds no title or description; an isolated footer before the time. **Mutants** (verifier): no port check, no Swift re-check, foreign redirects followed, no byte cap, no pixel cap, no 6000 budget, parsing in `row()`, no spoiler cover, `icon_url` or `video.url` reaching FFI. **Live (user):** the official client's treatment of a spoilered link's embed, markdown in titles and field names, the display maxima and the proxy's resize parameters, checked once against a capture |

### E plan review dispositions (31c7950)

| # | Blocker | Disposition |
|---|---|---|
| 1 | No security dispositions; provisional tests; Traffic unchecked | FIX: the table below; final Tests cell; Traffic claims map to the loader tests and the live check |
| 2 | Icons have no dimensions and different field names | FIX: Images row (icons from `proxy_icon_url`, no dimension check, fixed size); `icon_url` never parsed |

### E security dispositions (31c7950)

| # | Finding | Disposition |
|---|---|---|
| E-1 (P2) | Image loader unspecified; `AsyncImage` shares cookies, cache and redirects; decompression bombs | FIX: Image loading row |
| E-2 (P2) | Icon fields | FIX: Images and What is parsed rows |
| E-3 (P2) | Host match as a rule on both sides | FIX: Media URLs row and the Swift re-check |
| E-4 (P2) | No total budget; parse cost | FIX: Budgets row |
| E-5 (P2) | Embeds of spoilered links | FIX: Spoilers row; official behaviour confirmed by the user's live check |
| E-6 (P3) | Cuts can break a spoiler | FIX: Budgets row |
| E-7 (P3) | Embed links must reach `LinkRouter` without editing not-yours files | FIX: Embed links row |
| E-8 (P3) | F5 for embed plain strings | FIX: Display row; markdown in titles checked live |
| E-9 (P2/P3) | Attachments: F11 and `IS_SPOILER` | FIX: loader, host check, clamp and spoiler cover apply to image attachments |
| E-10 (P3) | Display clamp | FIX: Display row |
| E-11 (P3) | `video` / `gifv` | FIX: What is parsed and Display rows |
| E-12 (P3) | Updates and `SUPPRESS_EMBEDS` | FIX: Updates row |
| E-13 (P3) | Timestamp | FIX: Timestamp row |
| E-14 (P3) | Accessibility | FIX: Display and Spoilers rows |
| E-15 (P4) | Bidi on shared lines | FIX: Display row |
| E-16 (P4) | Colour | FIX: What is parsed row |
| E-17 (P4) | Emoji and mentions in embeds | FIX: Budgets row (emoji cap); mentions resolve by the AST walk |
| Privacy | Proxy-only matches the official client | ACCEPT; User-Agent difference DEFER |
