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
    Arc::new(MessageStore::new(Rest::new(token(), 7, tx)))
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

#[test]
fn mentions_resolve_in_one_pass_and_a_nickname_holding_a_role_token_stays_literal() {
    let ctx = Context {
        roles: vec![Role {
            id: 5,
            permissions: 0,
            position: 1,
            name: Some("mods".into()),
            color: 0,
        }],
        channels: vec![(7, "general".into())],
    };
    let mentioned = vec![(1, "<@&5>".to_owned()), (2, "Two".to_owned())];
    let content = "hi <@1> <@!2> <#7> <@&5> <@3> <#8> <@&6> <@x> <@> <<@2> <a>";
    assert_eq!(
        resolve(content, &mentioned, &ctx),
        "hi @<@&5> @Two #general @mods @unknown-user #unknown @deleted-role <@x> <@> <@Two <a>"
    );
    // The same through a parsed message: the nickname comes from the payload's `mentions`.
    let mut m = msg(10, 5, "<@1> and <@&5>");
    m["mentions"] = json!([{"id": "1", "username": "u1", "member": {"nick": "<@&5>"}}]);
    let r = row(&parse(&m, false).unwrap(), &ctx, false, None);
    assert_eq!(r.content, "@<@&5> and @mods");
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
        "id": "8", "content": "question <@2>",
        "author": {"id": "2", "username": "u2", "global_name": "Asker"},
        "mentions": [{"id": "2", "username": "u2", "global_name": "Asker"}],
    });
    let r = row(&parse(&m, false).unwrap(), &Context::default(), false, None);
    assert_eq!(
        r.reply,
        Some(MessageReply {
            author_name: "Asker".into(),
            content: "question @Asker".into()
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
    let store = Arc::new(MessageStore::new(Rest::new(token(), 7, tx)));
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
