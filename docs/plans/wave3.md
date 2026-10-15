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

## E — Embeds (SECURITY slice; starts after its review)

| Part | Contract |
|---|---|
| Core | Parse `embeds` (at most 10 per message): `type`, `title` (≤ 256 chars), `description` (≤ 4096), `url`, `color`, `timestamp`, `author {name ≤ 256, url, icon}`, `footer {text ≤ 2048, icon}`, `fields` (≤ 25; `name` ≤ 256, `value` ≤ 1024, `inline`), `thumbnail`, `image`, `video` (thumbnail only), `provider {name, url}`. Longer text is cut at the limit. `description` and field values go through core's markdown parser with its existing limits; titles, names and footers stay plain. Images use only `proxy_url` on Discord's media hosts (`media.discordapp.net`, `images-ext-1.discordapp.net`, `images-ext-2.discordapp.net`, `cdn.discordapp.com`) over HTTPS, with validated positive dimensions; anything else shows no image. Links (`url`, author and provider urls) go through the same canonicalisation as message links. `MESSAGE_UPDATE` with `embeds` replaces them (link previews arrive this way) |
| Swift | A new `EmbedView` in Discord's card look (left accent bar from `color`, author row, title as a link, description and fields through `MarkdownView`, inline fields in up to 3 columns, thumbnail top-right, image below, footer with time); a video or gifv shows its thumbnail without playing; every link opens through `LinkRouter` with confirmation (an embed link's text never equals its URL) |
| Traffic | Embed images load from Discord's media hosts only, without credentials, when a row is on screen |
| Tests | To be fixed after the review: parse and caps; host allow-list (each host, HTTP, a foreign host, missing dimensions); markdown only in description and values; update replaces; link routing with confirmation; no autoplay |
