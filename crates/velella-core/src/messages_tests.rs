//! Message-store tests against W0's mock HTTP server (`rest::test_support`). Live wiring through
//! the session and the mock gateway is in session_tests.rs (`messages_live`).
use super::*;
use crate::{
    identify,
    rest::test_support::{response, MockHttp, TEST_REST_BASE},
    Token,
};
use base64::Engine as _;
use serde_json::json;
use std::time::Duration;
use tokio::{io::AsyncReadExt, net::TcpListener, sync::mpsc, time::timeout};

const MARKER: &str = "SYNTHETIC_TOKEN_MARKER_0123456789";
const WAIT: Duration = Duration::from_secs(10);

fn token() -> Arc<Token> {
    Arc::new(Token::new(MARKER.into()).unwrap())
}

/// A store whose `Rest` talks to `mock`.
fn store_for(mock: &MockHttp) -> Arc<MessageStore> {
    mock.install();
    let (tx, _rx) = mpsc::unbounded_channel();
    MessageStore::with_wake(Rest::new(token(), 7, tx), None)
}

async fn serve(responses: &[String]) -> (MockHttp, Arc<MessageStore>) {
    let mock = MockHttp::start(responses.to_vec()).await;
    let store = store_for(&mock);
    (mock, store)
}

fn ok(body: &Value) -> String {
    response(200, "Content-Type: application/json\r\n", &body.to_string())
}

fn msg(id: u64, channel: u64, content: &str) -> Value {
    json!({
        "id": id.to_string(), "channel_id": channel.to_string(), "content": content,
        "author": {"id": "9", "username": "user9", "global_name": "Nine"},
    })
}

fn ids(s: &MessagesSnapshot) -> Vec<u64> {
    s.rows.iter().map(|r| r.id).collect()
}

fn now(store: &MessageStore, epoch: u64) -> MessagesSnapshot {
    store.snapshot(epoch, &Context::default()).unwrap()
}

/// The snapshot once the history fetch has finished.
async fn settled(store: &MessageStore, epoch: u64) -> MessagesSnapshot {
    let mut revision = store.revision.subscribe();
    timeout(WAIT, async {
        loop {
            let s = now(store, epoch);
            if s.load != LoadState::Loading {
                return s;
            }
            revision.changed().await.unwrap();
        }
    })
    .await
    .expect("history never settled")
}

fn me() -> User {
    User {
        id: 1,
        username: "me".into(),
        global_name: None,
        avatar: None,
        discriminator: None,
    }
}

fn request_headers(raw: &str) -> Vec<(String, String)> {
    raw.split("\r\n\r\n")
        .next()
        .unwrap()
        .lines()
        .skip(1)
        .filter_map(|l| {
            let (k, v) = l.split_once(':')?;
            Some((k.trim().to_ascii_lowercase(), v.trim().to_owned()))
        })
        .collect()
}

#[tokio::test]
async fn history_get_uses_the_typed_path_and_the_token_only_in_authorization() {
    let (mock, store) = serve(&[ok(&json!([]))]).await;
    let epoch = store.open(123);
    assert_eq!(settled(&store, epoch).await.load, LoadState::Loaded);
    let raw = &mock.requests()[0];
    assert!(
        raw.starts_with("GET /api/v9/channels/123/messages?limit=50 HTTP/1.1\r\n"),
        "{raw}"
    );
    assert_eq!(raw.matches(MARKER).count(), 1, "{raw}");
    let headers = request_headers(raw);
    let header = |name: &str| {
        headers
            .iter()
            .find(|(k, _)| k == name)
            .map(|(_, v)| v.clone())
    };
    assert_eq!(header("authorization").as_deref(), Some(MARKER));
    let props = base64::engine::general_purpose::STANDARD
        .decode(header("x-super-properties").unwrap())
        .unwrap();
    let props: Value = serde_json::from_slice(&props).unwrap();
    let token = token();
    let identify: Value =
        serde_json::from_str(&identify::encode(&identify::identify(&token))).unwrap();
    assert_eq!(props, identify["d"]["properties"]);
}

#[tokio::test]
async fn newest_first_history_becomes_ascending_rows_without_duplicates() {
    let page = json!([
        msg(30, 5, "c"),
        msg(20, 5, "b"),
        msg(20, 5, "b"),
        msg(10, 5, "a")
    ]);
    let (_mock, store) = serve(&[ok(&page)]).await;
    let epoch = store.open(5);
    let s = settled(&store, epoch).await;
    assert_eq!(ids(&s), [10, 20, 30]);
    assert_eq!(s.rows[0].content, "a");
    assert_eq!(s.channel_id, 5);
}

#[tokio::test]
async fn a_message_create_before_the_get_resolves_appears_once() {
    let page = json!([msg(20, 5, "from history"), msg(10, 5, "a")]);
    let (mock, store) = serve(&[ok(&page)]).await;
    let epoch = store.open(5);
    // The fetch task has not run yet (current-thread runtime, no await since `open`).
    store.observe("MESSAGE_CREATE", &msg(20, 5, "live"));
    store.observe("MESSAGE_CREATE", &msg(30, 5, "newer"));
    let early = now(&store, epoch);
    assert_eq!(
        (early.load.clone(), ids(&early)),
        (LoadState::Loading, vec![20, 30])
    );
    let s = settled(&store, epoch).await;
    assert_eq!(ids(&s), [10, 20, 30], "history merged into the live rows");
    assert_eq!(
        s.rows[1].content, "live",
        "the history copy replaced the live one"
    );
    assert_eq!(mock.count(), 1);
}

#[tokio::test]
async fn a_delete_during_the_fetch_is_not_undone_by_the_history() {
    let page = json!([msg(20, 5, "b"), msg(10, 5, "a")]);
    let (_mock, store) = serve(&[ok(&page)]).await;
    let epoch = store.open(5);
    store.observe("MESSAGE_DELETE", &json!({"id": "20", "channel_id": "5"}));
    assert_eq!(ids(&settled(&store, epoch).await), [10]);
}

#[tokio::test]
async fn the_201st_message_evicts_the_oldest() {
    let (_mock, store) = serve(&[ok(&json!([]))]).await;
    let epoch = store.open(5);
    settled(&store, epoch).await;
    for id in 1..=200 {
        store.observe("MESSAGE_CREATE", &msg(id, 5, "x"));
    }
    assert_eq!(now(&store, epoch).rows.len(), 200);
    store.observe("MESSAGE_CREATE", &msg(201, 5, "x"));
    let s = now(&store, epoch);
    assert_eq!(s.rows.len(), 200);
    assert_eq!((s.rows[0].id, s.rows[199].id), (2, 201));
}

#[tokio::test]
async fn an_update_without_content_author_or_attachments_keeps_them() {
    let (_mock, store) = serve(&[ok(&json!([]))]).await;
    let epoch = store.open(5);
    settled(&store, epoch).await;
    let mut created = msg(10, 5, "hello");
    created["member"] = json!({"nick": "Nick", "roles": []});
    created["attachments"] = json!([
        {"id": "1", "filename": "a.png", "content_type": "image/png", "url": "https://cdn.discordapp.com/a.png", "width": 4, "height": 3},
        {"id": "2", "filename": "notes.txt", "content_type": "text/plain", "url": "https://cdn.discordapp.com/notes.txt"},
    ]);
    store.observe("MESSAGE_CREATE", &created);
    let before = now(&store, epoch).rows[0].clone();
    assert!(!before.edited);

    store.observe(
        "MESSAGE_UPDATE",
        &json!({"id": "10", "channel_id": "5", "edited_timestamp": "2026-10-14T00:00:00+00:00"}),
    );
    let after = now(&store, epoch).rows[0].clone();
    assert_eq!(
        (
            &after.content,
            &after.author_name,
            &after.images,
            &after.files
        ),
        (
            &before.content,
            &before.author_name,
            &before.images,
            &before.files
        )
    );
    assert_eq!(after.author_name, "Nick");
    assert_eq!(after.files, ["notes.txt"]);
    assert!(after.edited, "edited_timestamp did not mark the row edited");

    store.observe(
        "MESSAGE_UPDATE",
        &json!({"id": "10", "channel_id": "5", "content": "changed", "attachments": []}),
    );
    let last = now(&store, epoch).rows[0].clone();
    assert_eq!(last.content, "changed");
    assert!(last.images.is_empty() && last.files.is_empty());
    assert_eq!(last.author_name, "Nick");
}

#[tokio::test]
async fn message_delete_removes_the_row() {
    let page = json!([msg(20, 5, "b"), msg(10, 5, "a")]);
    let (_mock, store) = serve(&[ok(&page)]).await;
    let epoch = store.open(5);
    settled(&store, epoch).await;
    store.observe("MESSAGE_DELETE", &json!({"id": "10", "channel_id": "5"}));
    assert_eq!(ids(&now(&store, epoch)), [20]);
}

#[tokio::test]
async fn only_the_open_channel_is_buffered() {
    let (_mock, store) = serve(&[ok(&json!([]))]).await;
    let epoch = store.open(5);
    settled(&store, epoch).await;
    store.observe("MESSAGE_CREATE", &msg(10, 6, "elsewhere"));
    store.observe("MESSAGE_CREATE", &msg(11, 5, "here"));
    assert_eq!(ids(&now(&store, epoch)), [11]);
    // Opening another channel starts from an empty buffer and ends the old one.
    let next = store.open(6);
    assert!(store.snapshot(epoch, &Context::default()).is_none());
    assert!(now(&store, next).rows.is_empty());
}

/// A snowflake-length id (the parser takes 17-20 digits) ending in `n`.
const fn sf(n: u64) -> u64 {
    100_000_000_000_000_000 + n
}

/// `s` with each `{n}` replaced by `sf(n)`, so mention tokens stay readable.
fn snow(s: &str) -> String {
    let mut out = String::new();
    let mut rest = s;
    while let Some(open) = rest.find('{') {
        let close = open + rest[open..].find('}').unwrap();
        out.push_str(&rest[..open]);
        out.push_str(&sf(rest[open + 1..close].parse().unwrap()).to_string());
        rest = &rest[close + 1..];
    }
    out + rest
}

/// Role {5} "mods" (coloured) and channel {7} "general".
fn guild_ctx() -> Context {
    Context {
        roles: vec![Role {
            id: sf(5),
            permissions: 0,
            position: 1,
            name: Some("mods".into()),
            color: 0xAA0000,
        }],
        channels: vec![(sf(7), "general".into())],
    }
}

fn mention(kind: MentionKind, n: u64, name: Option<&str>, color: u32) -> ResolvedMention {
    ResolvedMention {
        kind,
        id: sf(n),
        name: name.map(str::to_owned),
        color,
    }
}

/// Resolution walks the stored AST (never the text), once per distinct mention, and a name is
/// side data: a nickname holding `**x** <@&{5}>` is neither parsed nor expanded (M2's rule).
#[test]
fn mentions_resolve_from_the_body_and_a_nickname_holding_markup_stays_literal() {
    let nick = snow("**x** <@&{5}>");
    let mut m = msg(
        10,
        5,
        &snow(
            "hi <@{1}> <@!{2}> <#{7}> <@&{5}> <@{3}> <#{8}> <@&{6}> <@{1}> ||<@{2}>|| **<#{7}>** \
             <@x> `<@{4}>` <@1>",
        ),
    );
    m["mentions"] = json!([
        {"id": sf(1).to_string(), "username": "u1", "member": {"nick": nick}},
        {"id": sf(2).to_string(), "username": "u2", "global_name": "Two"},
    ]);
    let stored = parse(&m, false).unwrap();
    let r = row(&stored, &guild_ctx(), false, None);
    use MentionKind::*;
    assert_eq!(
        r.mentions,
        [
            mention(User, 1, Some(&nick), 0),
            mention(User, 2, Some("Two"), 0),
            mention(Channel, 7, Some("general"), 0),
            mention(Role, 5, Some("mods"), 0xAA0000),
            mention(User, 3, None, 0),
            mention(Channel, 8, None, 0),
            mention(Role, 6, None, 0),
        ]
    );
    // The row carries the stored parse and the raw content.
    assert_eq!(r.body, stored.body);
    assert_eq!(r.content, m["content"].as_str().unwrap());
}

/// The replied message as a row's reply preview, with users {2} ("Two") and {9} mentioned.
fn preview_of(content: &str, nick9: &str) -> String {
    let mut m = msg(20, 5, "answer");
    m["referenced_message"] = json!({
        "id": "8", "content": snow(content),
        "author": {"id": "2", "username": "u2"},
        "mentions": [
            {"id": sf(2).to_string(), "username": "u2", "global_name": "Two"},
            {"id": sf(9).to_string(), "username": "u9", "member": {"nick": nick9}},
        ],
    });
    row(&parse(&m, false).unwrap(), &guild_ctx(), false, None)
        .reply
        .unwrap()
        .content
}

#[test]
fn a_reply_preview_never_shows_spoiler_text() {
    assert_eq!(
        preview_of("a ||secret|| b", "n"),
        "a \u{2592}\u{2592}\u{2592} b"
    );
    assert_eq!(
        preview_of("||```secret```||", "n"),
        "\u{2592}\u{2592}\u{2592}"
    );
    for content in [
        "||**secret** <@{2}> [x](https://secret.example)||",
        "x ||> secret||",
        "||# secret||",
        "- ||```secret```||",
        "> a ||b\n> secret||",
    ] {
        let p = preview_of(content, "n");
        assert!(
            !p.contains("secret") && p.contains('\u{2592}'),
            "{content:?} → {p:?}"
        );
    }
}

#[test]
fn a_reply_preview_flattens_the_body_to_one_line() {
    let fsi = |s: &str| format!("\u{2068}{s}\u{2069}");
    let nick = snow("**x** <@&{5}>");
    assert_eq!(
        preview_of(
            "**b** `co  de`\nline <:cat:{123}> <@{2}> <#{7}> <@&{5}> <@{9}> <@{3}> <#{8}> <@&{6}> \
             @everyone @here </cmd sub:{5}> <{10}:guide> <t:1:R> [lab](https://x.y) https://a.b",
            &nick,
        ),
        format!(
            "b co de line :cat: {} {} {} {} {} {} {} @everyone @here {} Server Guide <t:1:R> lab \
             https://a.b",
            fsi("@Two"),
            fsi("#general"),
            fsi("@mods"),
            fsi(&format!("@{nick}")),
            fsi("@unknown-user"),
            fsi("#unknown"),
            fsi("@deleted-role"),
            fsi("/cmd sub"),
        )
    );
    assert_eq!(preview_of("# h\n```\nx\n\ny\n```\n> q", "n"), "h x y q");
    // A bidi override in a name stays inside its isolate.
    assert_eq!(
        preview_of("<@{9}> xyz", "\u{202E}abc"),
        "\u{2068}@\u{202E}abc\u{2069} xyz"
    );
    // At most 200 characters (not bytes).
    let long = "字".repeat(300);
    assert_eq!(preview_of(&long, "n").chars().count(), 200);
}

#[test]
fn emoji_urls_come_from_the_parsed_id_on_the_cdn() {
    assert_eq!(
        emoji_url(123, false),
        "https://cdn.discordapp.com/emojis/123.webp?size=48"
    );
    assert_eq!(
        emoji_url(u64::MAX, true),
        "https://cdn.discordapp.com/emojis/18446744073709551615.gif?size=48"
    );
}

/// Per row: distinct (id, animated) pairs in document order, spoilers included, at most 64.
#[test]
fn a_row_lists_at_most_64_distinct_emoji_images() {
    let mut content = snow("<:aa:{7}> <a:aa:{7}> <:bb:{7}> ||<:cc:{8}>||");
    for n in 100..200 {
        content.push_str(&snow(&format!(" <:ee:{{{n}}}>")));
    }
    let r = row(
        &parse(&msg(10, 5, &content), false).unwrap(),
        &Context::default(),
        false,
        None,
    );
    assert_eq!(r.emoji.len(), 64);
    let image = |n, animated| EmojiImage {
        id: sf(n),
        animated,
        url: emoji_url(sf(n), animated),
    };
    assert_eq!(
        r.emoji[..4],
        [
            image(7, false),
            image(7, true),
            image(8, false),
            image(100, false)
        ]
    );
    assert_eq!(r.emoji[63], image(160, false));
}

#[test]
fn display_name_is_member_nick_then_global_name_then_username() {
    let ctx = Context::default();
    let name = |v: Value| row(&parse(&v, false).unwrap(), &ctx, false, None).author_name;
    let author = json!({"id": "9", "username": "user9", "global_name": "Nine"});
    let base = json!({"id": "10", "channel_id": "5", "author": author});
    let with = |extra: Value| {
        let mut v = base.clone();
        v.as_object_mut()
            .unwrap()
            .extend(extra.as_object().unwrap().clone());
        v
    };
    assert_eq!(name(with(json!({"member": {"nick": "Nick"}}))), "Nick");
    assert_eq!(name(with(json!({"member": {"nick": null}}))), "Nine");
    assert_eq!(name(with(json!({"member": {"nick": ""}}))), "Nine");
    // A history row from GET carries no member: the global name shows.
    assert_eq!(name(base.clone()), "Nine");
    assert_eq!(
        name(json!({"id": "10", "author": {"id": "9", "username": "user9"}})),
        "user9"
    );
    // Mentioned users follow the same rule.
    let mentions = mentions(Some(&json!([
        {"id": "1", "username": "u1", "global_name": "G1", "member": {"nick": "N1"}},
        {"id": "2", "username": "u2", "global_name": "G2"},
        {"id": "3", "username": "u3"},
    ])));
    assert_eq!(
        mentions,
        [
            (1, "N1".to_owned()),
            (2, "G2".to_owned()),
            (3, "u3".to_owned())
        ]
    );
}

#[test]
fn author_colour_is_the_highest_coloured_role() {
    let role = |id, position, color| Role {
        id,
        permissions: 0,
        position,
        name: None,
        color,
    };
    let ctx = Context {
        roles: vec![role(1, 2, 0), role(2, 1, 0xAA0000), role(3, 0, 0x00BB00)],
        channels: vec![],
    };
    let colour_of = |v: Value| row(&parse(&v, false).unwrap(), &ctx, false, None).author_color;
    let mut m = msg(10, 5, "x");
    m["member"] = json!({"roles": ["1", "2", "3"]});
    assert_eq!(colour_of(m.clone()), Some(0xAA0000));
    m["member"] = json!({"roles": ["1"]});
    assert_eq!(colour_of(m), None);
    assert_eq!(colour_of(msg(10, 5, "x")), None);
}

#[test]
fn image_attachments_need_positive_dimensions_and_others_are_listed_by_filename() {
    let mut m = msg(10, 5, "x");
    let url = "https://cdn.discordapp.com/attachments/1/2/f";
    m["attachments"] = json!([
        {"filename": "ok.png", "content_type": "image/png", "url": url, "width": 640, "height": 480},
        {"filename": "zero.png", "content_type": "image/png", "url": url, "width": 0, "height": 480},
        {"filename": "nodims.png", "content_type": "image/png", "url": url},
        {"filename": "clip.mp4", "content_type": "video/mp4", "url": url, "width": 640, "height": 480},
        {"filename": "huge.png", "content_type": "image/png", "url": url, "width": 5_000_000_000u64, "height": 1},
    ]);
    let r = row(&parse(&m, false).unwrap(), &Context::default(), false, None);
    assert_eq!(
        r.images,
        [MessageImage {
            url: url.into(),
            width: 640,
            height: 480
        }]
    );
    assert_eq!(r.files, ["zero.png", "nodims.png", "clip.mp4", "huge.png"]);
}

#[test]
fn a_reply_shows_the_referenced_author_and_text() {
    // Discord's documented example snowflake: created 2016-04-30 11:18:25.796 UTC.
    let mut m = msg(175_928_847_299_117_063, 5, "answer");
    m["referenced_message"] = json!({
        "id": "8", "content": snow("question <@{2}>"),
        "author": {"id": sf(2).to_string(), "username": "u2", "global_name": "Asker"},
        "mentions": [{"id": sf(2).to_string(), "username": "u2", "global_name": "Asker"}],
    });
    let r = row(&parse(&m, false).unwrap(), &Context::default(), false, None);
    assert_eq!(
        r.reply,
        Some(MessageReply {
            author_name: "Asker".into(),
            content: "question \u{2068}@Asker\u{2069}".into()
        })
    );
    assert_eq!(r.timestamp_ms, 1_462_015_105_796);
}

#[tokio::test]
async fn a_history_403_fails_with_fixed_text_and_no_body_text() {
    let body = r#"{"message": "SECRET_BODY_TEXT", "code": 50001}"#;
    let (_mock, store) = serve(&[response(403, "Content-Type: application/json\r\n", body)]).await;
    let epoch = store.open(5);
    let s = settled(&store, epoch).await;
    assert_eq!(
        s.load,
        LoadState::Failed("Couldn't load messages: no access to this channel")
    );
    assert!(!format!("{s:?}").contains("SECRET"));
}

#[tokio::test]
async fn a_latched_401_shows_as_failed_and_sends_nothing_more() {
    let (mock, store) = serve(&[response(401, "", "")]).await;
    let epoch = store.open(5);
    let failed = LoadState::Failed("Couldn't load messages: Discord did not accept this sign-in");
    assert_eq!(settled(&store, epoch).await.load, failed);
    let epoch = store.open(6);
    assert_eq!(settled(&store, epoch).await.load, failed);
    assert_eq!(mock.count(), 1, "a latched client sent a request");
}

#[tokio::test]
async fn leaving_the_channel_cancels_the_in_flight_get() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    TEST_REST_BASE.with(|b| *b.borrow_mut() = Some(base));
    let (tx, _rx) = mpsc::unbounded_channel();
    let store = MessageStore::with_wake(Rest::new(token(), 7, tx), None);
    let epoch = store.open(5);
    // The server takes the request and never answers.
    let (mut stream, _) = timeout(WAIT, listener.accept()).await.unwrap().unwrap();
    let mut buf = [0u8; 4096];
    assert!(stream.read(&mut buf).await.unwrap() > 0);
    store.close(epoch);
    // The aborted request drops its connection: the server reads end-of-stream.
    let n = timeout(WAIT, stream.read(&mut buf))
        .await
        .expect("the GET was not cancelled");
    assert_eq!(n.unwrap(), 0);
    assert!(store.snapshot(epoch, &Context::default()).is_none());
}

#[tokio::test]
async fn the_echo_is_reconciled_only_on_its_nonce_and_the_self_author() {
    let (_mock, store) = serve(&[ok(&json!([]))]).await;
    let epoch = store.open(5);
    settled(&store, epoch).await;
    let (_, nonce) = store.begin_send(5, "hi", me()).unwrap();
    let echo = now(&store, epoch);
    assert_eq!(echo.rows.len(), 1);
    assert!(echo.rows[0].pending && echo.rows[0].content == "hi");

    // Same nonce from someone else: not our echo.
    let mut other = msg(50, 5, "hi");
    other["nonce"] = json!(nonce);
    store.observe("MESSAGE_CREATE", &other);
    let s = now(&store, epoch);
    assert_eq!(
        s.rows.iter().map(|r| r.pending).collect::<Vec<_>>(),
        [false, true]
    );

    let mut mine = msg(51, 5, "hi");
    mine["author"] = json!({"id": "1", "username": "me"});
    mine["nonce"] = json!(nonce);
    store.observe("MESSAGE_CREATE", &mine);
    let s = now(&store, epoch);
    assert_eq!(ids(&s), [50, 51]);
    assert!(s.rows.iter().all(|r| !r.pending));
    // A late POST answer for the same message adds nothing.
    store.finish_send(epoch, &nonce, Ok(mine));
    assert_eq!(ids(&now(&store, epoch)), [50, 51]);
}

fn bold(s: &str) -> Vec<markdown::Block> {
    use markdown::{Block, Inline};
    vec![Block::Paragraph(vec![Inline::Bold(vec![Inline::Text(
        s.into(),
    )])])]
}

#[tokio::test]
async fn a_local_echo_carries_its_parsed_body() {
    let (_mock, store) = serve(&[ok(&json!([]))]).await;
    let epoch = store.open(5);
    settled(&store, epoch).await;
    store.begin_send(5, "**b**", me()).unwrap();
    let inner = store.lock();
    assert_eq!(
        inner.open.as_ref().unwrap().echoes[0].message.body,
        bold("b")
    );
}

/// The body is parsed when a message arrives and again only when an edit changes its content.
#[tokio::test]
async fn created_and_edited_messages_carry_their_parsed_body() {
    let (_mock, store) = serve(&[ok(&json!([]))]).await;
    let epoch = store.open(5);
    settled(&store, epoch).await;
    let body = |store: &MessageStore| store.lock().open.as_ref().unwrap().messages[0].body.clone();
    store.observe("MESSAGE_CREATE", &msg(50, 5, "**a**"));
    assert_eq!(body(&store), bold("a"));
    store.observe(
        "MESSAGE_UPDATE",
        &json!({"id": "50", "channel_id": "5", "content": "**c**"}),
    );
    assert_eq!(body(&store), bold("c"));
    store.observe(
        "MESSAGE_UPDATE",
        &json!({"id": "50", "channel_id": "5", "edited_timestamp": "x"}),
    );
    assert_eq!(
        body(&store),
        bold("c"),
        "an edit without content keeps the body"
    );
}

#[tokio::test]
async fn sends_post_the_exact_body_with_a_fresh_nonce_each() {
    let reply = |id: u64, nonce: &str| {
        let mut m = msg(id, 5, "hi");
        m["author"] = json!({"id": "1", "username": "me"});
        m["nonce"] = json!(nonce);
        m
    };
    let (mock, store) = serve(&[ok(&json!([]))]).await;
    let epoch = store.open(5);
    settled(&store, epoch).await;
    let mut nonces = Vec::new();
    for id in [60, 61] {
        let (_, nonce) = store.begin_send(5, "hi", me()).unwrap();
        // The mock answers in order: the GET, then each POST.
        let result = store
            .rest
            .create_message(5, "hi", &nonce)
            .await
            .map(|_| reply(id, &nonce));
        store.finish_send(epoch, &nonce, result);
        nonces.push(nonce);
    }
    assert_ne!(nonces[0], nonces[1]);
    let posts: Vec<_> = mock.requests().into_iter().skip(1).collect();
    assert_eq!(posts.len(), 2);
    for (raw, nonce) in posts.iter().zip(&nonces) {
        assert!(
            raw.starts_with("POST /api/v9/channels/5/messages HTTP/1.1\r\n"),
            "{raw}"
        );
        let body: Value = serde_json::from_str(raw.split("\r\n\r\n").nth(1).unwrap()).unwrap();
        assert_eq!(
            body,
            json!({"content": "hi", "nonce": nonce, "tts": false, "flags": 0})
        );
    }
    let s = now(&store, epoch);
    assert_eq!(ids(&s), [60, 61]);
    assert!(s.rows.iter().all(|r| !r.pending && r.failure.is_none()));
}

#[tokio::test]
async fn a_post_500_sends_one_request_and_marks_the_echo_failed() {
    let (mock, store) = serve(&[ok(&json!([])), response(500, "", ""), ok(&json!({}))]).await;
    let epoch = store.open(5);
    settled(&store, epoch).await;
    let (_, nonce) = store.begin_send(5, "hi", me()).unwrap();
    let result = store.rest.create_message(5, "hi", &nonce).await;
    store.finish_send(epoch, &nonce, result);
    assert_eq!(mock.count(), 2, "the POST was retried");
    let s = now(&store, epoch);
    assert_eq!(s.rows.len(), 1);
    assert_eq!(
        (s.rows[0].pending, s.rows[0].failure),
        (false, Some("Not sent"))
    );
}

#[tokio::test]
async fn a_captcha_answer_fails_with_the_official_app_text() {
    let captcha = r#"{"captcha_key": ["captcha-required"], "captcha_sitekey": "x"}"#;
    let (mock, store) = serve(&[
        ok(&json!([])),
        response(400, "Content-Type: application/json\r\n", captcha),
    ])
    .await;
    let epoch = store.open(5);
    settled(&store, epoch).await;
    let (_, nonce) = store.begin_send(5, "hi", me()).unwrap();
    let result = store.rest.create_message(5, "hi", &nonce).await;
    store.finish_send(epoch, &nonce, result);
    assert_eq!(mock.count(), 2);
    assert_eq!(
        now(&store, epoch).rows[0].failure,
        Some("Discord asked for a captcha; send this from the official app")
    );
}

#[tokio::test]
async fn a_send_to_a_channel_that_is_not_open_adds_nothing() {
    let (_mock, store) = serve(&[ok(&json!([]))]).await;
    assert!(store.begin_send(5, "hi", me()).is_none(), "nothing open");
    let epoch = store.open(5);
    assert!(store.begin_send(6, "hi", me()).is_none());
    assert!(now(&store, epoch).rows.is_empty());
}

/// A fetch or send that finishes in the moment its channel is left (already past the abort, or
/// waiting on the store lock) must not write into the channel opened next.
#[tokio::test]
async fn a_late_result_from_a_left_channel_is_dropped() {
    let (_mock, store) = serve(&[ok(&json!([]))]).await;
    let old = store.open(5);
    let new = store.open(6);
    store.history(old, Ok(vec![msg(10, 5, "old channel")]));
    store.finish_send(old, "1", Ok(msg(11, 5, "old channel")));
    let s = now(&store, new);
    assert_eq!(
        (s.channel_id, s.load, s.rows.len()),
        (6, LoadState::Loading, 0)
    );
}

/// K2 verification F1: a name holding its own PDI or isolate initiators can neither close the
/// FSI early nor leave it open, so the author's text after it keeps its direction.
#[test]
fn a_name_cannot_close_or_hold_open_its_isolate_in_a_preview() {
    for (nick, want) in [
        ("\u{2069}\u{202E}abc", "\u{2068}@\u{202E}abc\u{2069} xyz"),
        ("\u{2067}abc", "\u{2068}@\u{2067}abc\u{2069}\u{2069} xyz"),
        (
            "\u{2066}\u{2068}abc",
            "\u{2068}@\u{2066}\u{2068}abc\u{2069}\u{2069}\u{2069} xyz",
        ),
        (
            "a\u{2066}b\u{2069}c",
            "\u{2068}@a\u{2066}b\u{2069}c\u{2069} xyz",
        ),
    ] {
        assert_eq!(preview_of("<@{9}> xyz", nick), want, "nick {nick:?}");
    }
}

// ---- Highlight worker (K6 rev 3.1) ----

/// `content` with one fenced block per (fence, code).
fn fenced(blocks: &[(&str, &str)]) -> String {
    blocks
        .iter()
        .map(|(f, c)| format!("```{f}\n{c}\n```"))
        .collect::<Vec<_>>()
        .join("\n")
}

/// Every code block's highlight in `body`, in walk order.
fn highlights(body: &[markdown::Block]) -> Vec<Option<Arc<Highlighted>>> {
    let mut body = body.to_vec();
    let mut out = Vec::new();
    each_code_block(&mut body, &mut |_, _, h| out.push(h.clone()));
    out
}

/// No code block in the open channel's snapshot has a highlight.
fn all_plain(store: &MessageStore, epoch: u64) -> bool {
    now(store, epoch)
        .rows
        .iter()
        .all(|r| highlights(&r.body).iter().all(Option::is_none))
}

/// `compute_job` with no budget.
fn computed(job: &Job) -> Vec<Option<Arc<Highlighted>>> {
    let t = std::time::Instant::now();
    highlight::compute_job_with(job, &mut || t)
}

fn py_job(id: u64, code: &str) -> Job {
    Job {
        id,
        blocks: vec![(Some("py".into()), code.into())],
    }
}

/// A store (no worker) with channel 5 open and its history `page` loaded.
async fn opened(page: Value) -> (MockHttp, Arc<MessageStore>, u64) {
    let (mock, store) = serve(&[ok(&page)]).await;
    let epoch = store.open(5);
    settled(&store, epoch).await;
    (mock, store, epoch)
}

#[tokio::test]
async fn each_pending_site_queues_the_message_and_computes_nothing() {
    let py = fenced(&[("py", "x = 1")]);
    // MESSAGE_CREATE.
    let (_m, store, epoch) = opened(json!([])).await;
    store.observe("MESSAGE_CREATE", &msg(50, 5, &py));
    assert!(all_plain(&store, epoch));
    assert_eq!(store.next_job().map(|j| j.id), Some(50));
    // `history` Ok.
    let (_m, store, epoch) = opened(json!([msg(60, 5, &py)])).await;
    assert!(all_plain(&store, epoch));
    assert_eq!(store.next_job().map(|j| j.id), Some(60));
    // `finish_send` Ok.
    let (_m, store, epoch) = opened(json!([])).await;
    let (_, nonce) = store.begin_send(5, &py, me()).unwrap();
    let mut sent = msg(70, 5, &py);
    sent["author"] = json!({"id": "1", "username": "me"});
    sent["nonce"] = json!(nonce);
    store.finish_send(epoch, &nonce, Ok(sent));
    assert!(all_plain(&store, epoch));
    assert_eq!(store.next_job().map(|j| j.id), Some(70));
    // `merge` with changed content.
    let (_m, store, epoch) = opened(json!([msg(80, 5, "plain")])).await;
    assert_eq!(store.next_job(), None);
    store.observe(
        "MESSAGE_UPDATE",
        &json!({"id": "80", "channel_id": "5", "content": py}),
    );
    assert!(all_plain(&store, epoch));
    assert_eq!(store.next_job().map(|j| j.id), Some(80));
}

#[tokio::test]
async fn nothing_is_pending_without_a_mapped_fence_in_the_message_itself() {
    let py = fenced(&[("py", "x = 1")]);
    let (_m, store, epoch) = opened(json!([msg(10, 5, &py)])).await;
    assert_eq!(store.next_job().map(|j| j.id), Some(10));
    // `merge` with equal content.
    store.observe(
        "MESSAGE_UPDATE",
        &json!({"id": "10", "channel_id": "5", "content": py, "edited_timestamp": "x"}),
    );
    assert_eq!(store.next_job(), None);
    // An echo with a `py` block. The worker never reads echoes, even one flagged by hand.
    store.begin_send(5, &py, me()).unwrap();
    assert_eq!(store.next_job(), None);
    let echo_id = {
        let mut inner = store.lock();
        let echo = &mut inner.open.as_mut().unwrap().echoes[0].message;
        echo.pending = true;
        echo.id
    };
    assert_eq!(store.next_job(), None, "next_job searched the echoes");
    let job = py_job(echo_id, "x = 1");
    store.apply(&job, computed(&job));
    let echo_body = store.lock().open.as_ref().unwrap().echoes[0]
        .message
        .body
        .clone();
    assert_eq!(highlights(&echo_body), [None], "apply searched the echoes");
    // A reply whose only `py` block is in its referenced message.
    let mut reply = msg(20, 5, "a reply");
    reply["referenced_message"] = msg(15, 5, &py);
    store.observe("MESSAGE_CREATE", &reply);
    assert_eq!(store.next_job(), None);
    // Only an unmapped fence.
    store.observe(
        "MESSAGE_CREATE",
        &msg(30, 5, &fenced(&[("swift", "let x = 1")])),
    );
    assert_eq!(store.next_job(), None);
    assert!(all_plain(&store, epoch));
}

#[tokio::test]
async fn the_newest_pending_message_goes_first() {
    let py = fenced(&[("py", "x = 1")]);
    let (_m, store, _) = opened(json!([msg(20, 5, &py), msg(10, 5, &py)])).await;
    assert_eq!(store.next_job().map(|j| j.id), Some(20));
    assert_eq!(store.next_job().map(|j| j.id), Some(10));
    assert_eq!(store.next_job(), None);
}

#[tokio::test]
async fn history_inserts_a_pending_message_with_its_blocks() {
    let content = fenced(&[("py", "x = 1"), ("swift", "let y = 2")]);
    let (_m, store, _) = opened(json!([msg(10, 5, &content)])).await;
    let job = store.next_job().unwrap();
    assert_eq!(job.id, 10);
    // Every block, mapped or not, in walk order.
    assert_eq!(
        job.blocks,
        [
            (Some("py".to_owned()), "x = 1".to_owned()),
            (Some("swift".to_owned()), "let y = 2".to_owned())
        ]
    );
}

#[tokio::test]
async fn a_result_for_a_channel_left_is_dropped_and_still_right_after_reopening() {
    let a = json!([msg(10, 5, &fenced(&[("py", "x = 1")]))]);
    let b = json!([msg(20, 6, &fenced(&[("rb", "y = 2")]))]);
    let (_mock, store) = serve(&[ok(&a), ok(&b), ok(&a)]).await;
    let ea = store.open(5);
    settled(&store, ea).await;
    let job = store.next_job().unwrap();
    assert_eq!(job.id, 10);
    let results = computed(&job);
    let eb = store.open(6);
    settled(&store, eb).await;
    store.apply(&job, results.clone());
    assert!(all_plain(&store, eb));
    assert_eq!(store.next_job().map(|j| j.id), Some(20));
    assert_eq!(store.next_job(), None);
    // Back to A with the same message: A's old result is still that block's own, so it applies.
    store.close(eb);
    let ea = store.open(5);
    settled(&store, ea).await;
    store.apply(&job, results);
    let h = highlights(&now(&store, ea).rows[0].body);
    assert_eq!(h.len(), 1);
    assert_eq!(h[0].as_ref().map(|h| h.text()).as_deref(), Some("x = 1"));
}

#[tokio::test]
async fn an_edit_during_compute_drops_the_result_and_queues_the_new_code() {
    // New code, and a fence-only edit (`py` → `rb`, same code).
    for edited in [fenced(&[("py", "x = 2")]), fenced(&[("rb", "x = 1")])] {
        let (_m, store, epoch) = opened(json!([msg(10, 5, &fenced(&[("py", "x = 1")]))])).await;
        let job = store.next_job().unwrap();
        let results = computed(&job);
        assert!(results[0].is_some());
        store.observe(
            "MESSAGE_UPDATE",
            &json!({"id": "10", "channel_id": "5", "content": edited}),
        );
        store.apply(&job, results);
        assert!(all_plain(&store, epoch), "{edited}");
        assert_eq!(store.next_job().map(|j| j.id), Some(10));
    }
}

#[tokio::test]
async fn a_result_for_a_deleted_or_evicted_message_is_dropped() {
    let py = fenced(&[("py", "x = 1")]);
    let (_m, store, epoch) = opened(json!([msg(10, 5, &py)])).await;
    let job = store.next_job().unwrap();
    let results = computed(&job);
    store.observe("MESSAGE_DELETE", &json!({"id": "10", "channel_id": "5"}));
    store.apply(&job, results);
    assert!(now(&store, epoch).rows.is_empty());
    // Pushed out past `CAP`.
    let (_m, store, epoch) = opened(json!([msg(10, 5, &py)])).await;
    let job = store.next_job().unwrap();
    let results = computed(&job);
    for id in 11..=(10 + CAP as u64) {
        store.observe("MESSAGE_CREATE", &msg(id, 5, "x"));
    }
    assert!(!ids(&now(&store, epoch)).contains(&10));
    store.apply(&job, results);
    assert!(all_plain(&store, epoch));
}

#[tokio::test]
async fn a_result_that_does_not_spell_the_code_is_not_applied() {
    let (_m, store, epoch) = opened(json!([msg(10, 5, &fenced(&[("py", "x = 1")]))])).await;
    let job = store.next_job().unwrap();
    let other = computed(&py_job(10, "y = 2"));
    assert!(other[0].is_some());
    store.apply(&job, other);
    assert!(all_plain(&store, epoch));
    store.apply(&job, computed(&job));
    assert!(!all_plain(&store, epoch));
}

#[tokio::test]
async fn a_replacing_copy_keeps_colours_for_equal_content_only() {
    let py = fenced(&[("py", "x = 1")]);
    let (_m, store, epoch) = opened(json!([msg(10, 5, &py)])).await;
    let job = store.next_job().unwrap();
    store.apply(&job, computed(&job));
    let first = |store: &MessageStore| highlights(&now(store, epoch).rows[0].body)[0].clone();
    let before = first(&store).unwrap();
    store.observe("MESSAGE_CREATE", &msg(10, 5, &py));
    assert!(Arc::ptr_eq(&before, &first(&store).unwrap()));
    assert_eq!(store.next_job(), None);
    store.observe("MESSAGE_CREATE", &msg(10, 5, &fenced(&[("py", "x = 2")])));
    assert_eq!(first(&store), None);
    assert_eq!(store.next_job().map(|j| j.id), Some(10));
}

/// Plan K6-1 through `apply`: a hidden-spoiler block before a visible one; each block's spans
/// join to its own code. The FFI half (each node converts with its own spans) is in velella-ffi.
#[tokio::test]
async fn a_hidden_spoiler_block_and_a_visible_one_each_get_their_own_spans() {
    let content = "||```py\nsecret = 1\n```|| then\n```py\nshown = 2\n```";
    let (_m, store, epoch) = opened(json!([msg(10, 5, content)])).await;
    let job = store.next_job().unwrap();
    store.apply(&job, computed(&job));
    let mut body = now(&store, epoch).rows[0].body.clone();
    let mut seen = Vec::new();
    each_code_block(&mut body, &mut |_, code, h| {
        seen.push((code.to_owned(), h.as_ref().map(|h| h.text())))
    });
    assert_eq!(
        seen,
        [
            ("secret = 1".to_owned(), Some("secret = 1".to_owned())),
            ("shown = 2".to_owned(), Some("shown = 2".to_owned()))
        ]
    );
}

#[tokio::test]
async fn snapshots_share_the_highlight_instead_of_copying_it() {
    let (_m, store, epoch) = opened(json!([msg(10, 5, &fenced(&[("py", "x = 1")]))])).await;
    let job = store.next_job().unwrap();
    store.apply(&job, computed(&job));
    let a = highlights(&now(&store, epoch).rows[0].body)[0]
        .clone()
        .unwrap();
    let b = highlights(&now(&store, epoch).rows[0].body)[0]
        .clone()
        .unwrap();
    assert!(Arc::ptr_eq(&a, &b));
}

/// A store with the given wake sender and channel 5 open.
async fn opened_with_wake(wake: SyncSender<()>) -> (MockHttp, Arc<MessageStore>, u64) {
    let mock = MockHttp::start(vec![ok(&json!([]))]).await;
    mock.install();
    let (tx, _rx) = mpsc::unbounded_channel();
    let store = MessageStore::with_wake(Rest::new(token(), 7, tx), Some(wake));
    let epoch = store.open(5);
    settled(&store, epoch).await;
    (mock, store, epoch)
}

#[tokio::test]
async fn an_undrained_wake_never_blocks_and_pending_stays_within_cap() {
    let (wake, _never_read) = std::sync::mpsc::sync_channel(1);
    let (_m, store, _) = opened_with_wake(wake).await;
    let py = fenced(&[("py", "x = 1")]);
    for id in 1..=10_000u64 {
        store.observe("MESSAGE_CREATE", &msg(id, 5, &py));
    }
    let inner = store.lock();
    let open = inner.open.as_ref().unwrap();
    assert_eq!(open.messages.len(), CAP);
    assert!(open.messages.iter().filter(|m| m.pending).count() <= CAP);
}

#[tokio::test]
async fn a_dead_worker_never_blocks_or_panics_the_store() {
    let (wake, receiver) = std::sync::mpsc::sync_channel(1);
    drop(receiver);
    let (_m, store, epoch) = opened_with_wake(wake).await;
    for id in 1..=3 {
        store.observe("MESSAGE_CREATE", &msg(id, 5, &fenced(&[("py", "x = 1")])));
    }
    assert_eq!(ids(&now(&store, epoch)), [1, 2, 3]);
}

/// A liveness limit, not a timing bound: the worker first warms every grammar, which takes
/// seconds in a debug build.
const LIVENESS: Duration = Duration::from_secs(60);

/// Polls `done` every 10 ms for up to `limit`.
fn within(limit: Duration, mut done: impl FnMut() -> bool) -> bool {
    let deadline = std::time::Instant::now() + limit;
    while !done() {
        if std::time::Instant::now() > deadline {
            return false;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    true
}

#[tokio::test]
async fn the_worker_colours_a_message_and_ends_with_its_store() {
    let mock = MockHttp::start(vec![ok(&json!([]))]).await;
    mock.install();
    let (tx, _rx) = mpsc::unbounded_channel();
    let started = std::time::Instant::now();
    let (store, handle) = MessageStore::start(Rest::new(token(), 7, tx), highlight::compute_job);
    let handle = handle.expect("the worker thread started");
    let epoch = store.open(5);
    settled(&store, epoch).await;
    store.observe("MESSAGE_CREATE", &msg(10, 5, &fenced(&[("py", "x = 1")])));
    assert!(
        within(LIVENESS, || !all_plain(&store, epoch)),
        "no colours within {LIVENESS:?}"
    );
    println!(
        "end to end: colours {:.3} s after the store was created",
        started.elapsed().as_secs_f64()
    );
    let h = highlights(&now(&store, epoch).rows[0].body);
    assert_eq!(h[0].as_ref().map(|h| h.text()).as_deref(), Some("x = 1"));
    drop(store);
    assert!(
        within(Duration::from_secs(5), || handle.is_finished()),
        "the worker outlived its store"
    );
}

#[tokio::test]
async fn the_worker_holds_no_store_while_it_computes() {
    let mock = MockHttp::start(vec![ok(&json!([]))]).await;
    mock.install();
    let (tx, _rx) = mpsc::unbounded_channel();
    let (started_tx, started) = std::sync::mpsc::channel();
    let (release, release_rx) = std::sync::mpsc::channel::<()>();
    let compute = move |job: &Job| {
        started_tx.send(job.id).unwrap();
        let _ = release_rx.recv();
        vec![None; job.blocks.len()]
    };
    let (store, handle) = MessageStore::start(Rest::new(token(), 7, tx), compute);
    let handle = handle.expect("the worker thread started");
    let epoch = store.open(5);
    settled(&store, epoch).await;
    store.observe("MESSAGE_CREATE", &msg(10, 5, &fenced(&[("py", "x = 1")])));
    assert_eq!(started.recv_timeout(LIVENESS), Ok(10));
    let weak = Arc::downgrade(&store);
    drop(store);
    assert_eq!(
        weak.strong_count(),
        0,
        "the worker holds the store while it computes"
    );
    release.send(()).unwrap();
    assert!(
        within(Duration::from_secs(5), || handle.is_finished()),
        "the worker outlived its store"
    );
}

// ---- Release measurements (K6 rev 3.1); run each alone, so the statics start cold ----

/// One 1024-byte line of `(` in a `cs` block, 15 times: 15 × 1035 = 15,525 bytes, under the
/// parser's 16,384-byte limit.
fn cold_payload() -> String {
    let content = format!("```cs\n{}\n```\n", "(".repeat(1024)).repeat(15);
    assert_eq!(content.len(), 15_525);
    content
}

#[tokio::test]
#[ignore = "timing, release only: cargo test --release -p velella-core -- --ignored --exact messages::tests::release_cold_observe --test-threads=1 --nocapture"]
async fn release_cold_observe() {
    if cfg!(debug_assertions) {
        panic!("run in release: the numbers are for the shipped build");
    }
    let content = cold_payload();
    // Precondition, on a twin store without a worker (the real one may take a job at once): each
    // stored body has 15 `cs` code blocks, and the message is pending.
    let (_m, twin, _) = opened(json!([])).await;
    for id in 1..=10u64 {
        twin.observe("MESSAGE_CREATE", &msg(id, 5, &content));
    }
    {
        let mut inner = twin.lock();
        for m in &mut inner.open.as_mut().unwrap().messages {
            let mut fences = Vec::new();
            each_code_block(&mut m.body, &mut |f, _, _| {
                fences.push(f.map(str::to_owned))
            });
            assert_eq!(fences, vec![Some("cs".to_owned()); 15]);
            assert!(m.pending);
        }
    }
    // The real store: `new()` starts the worker, which warms the statics while the session path
    // runs. The worst single `observe`.
    let mock = MockHttp::start(vec![ok(&json!([]))]).await;
    mock.install();
    let (tx, _rx) = mpsc::unbounded_channel();
    let store = MessageStore::new(Rest::new(token(), 7, tx));
    let epoch = store.open(5);
    settled(&store, epoch).await;
    let mut worst = Duration::ZERO;
    for id in 1..=10u64 {
        let m = msg(id, 5, &content);
        let start = std::time::Instant::now();
        store.observe("MESSAGE_CREATE", &m);
        worst = worst.max(start.elapsed());
    }
    let stored = now(&store, epoch);
    assert_eq!(stored.rows.len(), 10);
    assert!(stored.rows.iter().all(|r| highlights(&r.body).len() == 15));
    println!(
        "cold observe: worst of 10 = {:.3} ms",
        worst.as_secs_f64() * 1e3
    );
    assert!(worst <= Duration::from_millis(5), "{worst:?}");
}

#[tokio::test]
#[ignore = "timing, release only: cargo test --release -p velella-core -- --ignored --exact messages::tests::release_contended_observe --test-threads=1 --nocapture"]
async fn release_contended_observe() {
    if cfg!(debug_assertions) {
        panic!("run in release: the numbers are for the shipped build");
    }
    // 200 messages, each one block of 64 lines × 64 characters with one span per character
    // (4096 synthetic spans), applied through `apply`.
    let code = vec!["x".repeat(64); 64].join("\n");
    let spans = Arc::new(Highlighted {
        lines: code
            .split('\n')
            .map(|l| {
                l.chars()
                    .map(|c| highlight::Span {
                        text: c.to_string(),
                        rgb: 0x9CDCFE,
                        bold: false,
                        italic: false,
                        underline: false,
                    })
                    .collect()
            })
            .collect(),
    });
    let (_m, store, epoch) = opened(json!([])).await;
    let content = fenced(&[("py", &code)]);
    for id in 1001..=1200u64 {
        store.observe("MESSAGE_CREATE", &msg(id, 5, &content));
    }
    while let Some(job) = store.next_job() {
        store.apply(&job, vec![Some(spans.clone())]);
    }
    let rows = now(&store, epoch).rows;
    assert_eq!(rows.len(), CAP);
    assert!(rows
        .iter()
        .all(|r| highlights(&r.body)[0].as_ref().is_some_and(|h| h
            .lines
            .iter()
            .map(Vec::len)
            .sum::<usize>()
            == 4096)));
    // A thread looping `snapshot()`; each call's time bounds its lock hold.
    let stop = Arc::new(AtomicU64::new(0));
    let looping = {
        let (store, stop) = (store.clone(), stop.clone());
        std::thread::spawn(move || {
            let mut longest = Duration::ZERO;
            let mut calls = 0u32;
            while stop.load(Ordering::Relaxed) == 0 {
                let start = std::time::Instant::now();
                let s = store.snapshot(epoch, &Context::default());
                longest = longest.max(start.elapsed());
                drop(s);
                calls += 1;
            }
            (longest, calls)
        })
    };
    std::thread::sleep(Duration::from_millis(50));
    // 100 CREATEs of older messages: each is parsed, inserted and evicted again, so the 200
    // highlighted messages stay.
    let mut worst = Duration::ZERO;
    for id in 1..=100u64 {
        let m = msg(id, 5, &fenced(&[("py", "x = 1")]));
        let start = std::time::Instant::now();
        store.observe("MESSAGE_CREATE", &m);
        worst = worst.max(start.elapsed());
    }
    stop.store(1, Ordering::Relaxed);
    let (longest, calls) = looping.join().unwrap();
    println!(
        "contended observe: worst of 100 = {:.3} ms; snapshot (lock hold bound): longest of {calls} = {:.3} ms",
        worst.as_secs_f64() * 1e3,
        longest.as_secs_f64() * 1e3
    );
    assert!(worst <= Duration::from_millis(5), "{worst:?}");
    assert!(longest <= Duration::from_millis(5), "{longest:?}");
}
