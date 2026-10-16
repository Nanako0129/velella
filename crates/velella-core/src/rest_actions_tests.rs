//! Slice F2 on the in-memory transport: each action's method, target, extra headers and body
//! byte for byte against `menu-requests.md`; emoji paths; the S-1 check; one test per retry-table
//! row; the per-route block; single flight; thread preconditions; MFA; settings-proto. No test
//! reaches the network: `MemoryTransport` is the only transport here (a test without a mock
//! gets a connect failure, see `without_a_mock_nothing_is_sent_anywhere`).
use crate::{
    frecency_proto::{
        EmojiFrecency, FavoriteEmojis, FrecencyItem, FrecencyPatch, FrecencySettings, PatchOutcome,
    },
    proto::{base64_encode, Field, Message, ProtoError, Value as PValue},
    rest::{
        test_support::{response, MockHttp, HANG},
        ActionError, ApiCode, Call, MfaKind, Rest, Retry,
    },
    rest_actions::{
        thread_state, AckFlags, Emoji, ReactionLocation, ReactionType, SaveSource, ThreadState, Tri,
    },
    rest_messages::Reply,
    state::State,
    transport::Method,
    Token,
};
use std::{sync::Arc, time::Duration};
use tokio::{sync::mpsc, time::Instant};

const MARKER: &str = "SYNTHETIC_TOKEN_MARKER_0123456789";
const CHAT_INPUT: &str = "eyJsb2NhdGlvbiI6ImNoYXRfaW5wdXQifQ==";
const FORWARDING: &str = "eyJsb2NhdGlvbiI6ImZvcndhcmRpbmcifQ==";
const UNKNOWN: ThreadState = ThreadState {
    archived: Tri::Unknown,
    locked: Tri::Unknown,
    joined: Tri::Unknown,
    manage_threads: Tri::Unknown,
};

async fn serve(responses: &[String]) -> (MockHttp, Rest, mpsc::UnboundedReceiver<()>) {
    let mock = MockHttp::start(responses.to_vec()).await;
    mock.install();
    let (tx, rx) = mpsc::unbounded_channel();
    let rest = Rest::test(Arc::new(Token::new(MARKER.into()).unwrap()), 7, tx);
    (mock, rest, rx)
}

fn ok(body: &str) -> String {
    response(200, "", body)
}

fn empty() -> String {
    response(204, "", "")
}

fn json_status(status: u16, body: &str) -> String {
    response(status, "Content-Type: application/json\r\n", body)
}

/// One recorded request: request line, lower-cased headers, body.
struct Raw {
    line: String,
    headers: Vec<(String, String)>,
    body: String,
}

impl Raw {
    fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(k, _)| k == name)
            .map(|(_, v)| v.as_str())
    }
}

fn raw(text: &str) -> Raw {
    let (head, body) = text.split_once("\r\n\r\n").unwrap();
    let mut lines = head.lines();
    let line = lines.next().unwrap().to_owned();
    let headers = lines
        .filter_map(|l| {
            let (k, v) = l.split_once(':')?;
            Some((k.trim().to_ascii_lowercase(), v.trim().to_owned()))
        })
        .collect();
    Raw {
        line,
        headers,
        body: body.to_owned(),
    }
}

fn requests(mock: &MockHttp) -> Vec<Raw> {
    mock.requests().iter().map(|r| raw(r)).collect()
}

/// Method and target, the body byte for byte, `Content-Type` exactly when there is a body, the
/// context header as given, and no retry or MFA header.
fn check(r: &Raw, line: &str, body: &str, context: Option<&str>) {
    assert_eq!(r.line, format!("{line} HTTP/1.1"));
    assert_eq!(r.body, body, "{line}");
    assert_eq!(
        r.header("content-type"),
        (!body.is_empty()).then_some("application/json"),
        "{line}"
    );
    assert_eq!(r.header("x-context-properties"), context, "{line}");
    assert_eq!(r.header("x-failed-requests"), None, "{line}");
    assert_eq!(r.header("x-discord-mfa-authorization"), None, "{line}");
    assert_eq!(r.header("authorization"), Some(MARKER));
    assert_eq!(r.header("host"), Some("discord.com"));
}

fn thumbs() -> Emoji {
    Emoji::Unicode("\u{1F44D}".into())
}

fn blobhaj() -> Emoji {
    Emoji::Custom {
        name: "blobhaj".into(),
        id: 123,
    }
}

const R: &str = "/api/v9/channels/1/messages/2/reactions";

#[tokio::test]
async fn reaction_requests_match_the_web_client() {
    let (mock, rest, _rx) = serve(&[empty(), ok(r#"[{"id":"9","username":"nine"}]"#)]).await;
    use ReactionLocation::*;
    rest.add_reaction(
        1,
        2,
        &thumbs(),
        ReactionType::Normal,
        MessageContextMenu,
        &UNKNOWN,
    )
    .await
    .unwrap();
    rest.add_reaction(
        1,
        2,
        &blobhaj(),
        ReactionType::Burst,
        MessageHoverBar,
        &UNKNOWN,
    )
    .await
    .unwrap();
    rest.remove_reaction(
        1,
        2,
        &thumbs(),
        ReactionType::Normal,
        None,
        MessageInlineButton,
        &UNKNOWN,
    )
    .await
    .unwrap();
    rest.remove_reaction(
        1,
        2,
        &blobhaj(),
        ReactionType::Burst,
        Some(99),
        Message,
        &UNKNOWN,
    )
    .await
    .unwrap();
    rest.remove_emoji_reactions(1, 2, &thumbs(), &UNKNOWN)
        .await
        .unwrap();
    rest.remove_all_reactions(1, 2, &UNKNOWN).await.unwrap();
    let r = requests(&mock);
    check(
        &r[0],
        &format!("PUT {R}/%F0%9F%91%8D/@me?location=Message%20Context%20Menu&type=0"),
        "",
        None,
    );
    check(
        &r[1],
        &format!("PUT {R}/blobhaj:123/@me?location=Message%20Hover%20Bar&type=1"),
        "",
        None,
    );
    check(
        &r[2],
        &format!("DELETE {R}/%F0%9F%91%8D/0/@me?location=Message%20Inline%20Button&burst=false"),
        "",
        None,
    );
    check(
        &r[3],
        &format!("DELETE {R}/blobhaj:123/1/99?location=Message&burst=true"),
        "",
        None,
    );
    check(&r[4], &format!("DELETE {R}/%F0%9F%91%8D"), "", None);
    check(&r[5], &format!("DELETE {R}"), "", None);

    let (mock, rest, _rx) =
        serve(&[ok(r#"[{"id":"9","username":"nine","global_name":"Nine"}]"#)]).await;
    let users = rest
        .list_reactors(1, 2, &thumbs(), ReactionType::Normal, 100, None)
        .await
        .unwrap();
    assert_eq!(
        (users[0].id, users[0].global_name.as_deref()),
        (9, Some("Nine"))
    );
    rest.list_reactors(1, 2, &blobhaj(), ReactionType::Burst, 25, Some(7))
        .await
        .unwrap();
    for bad in [0, 101] {
        assert_eq!(
            rest.list_reactors(1, 2, &thumbs(), ReactionType::Normal, bad, None)
                .await
                .unwrap_err(),
            ActionError::InvalidInput
        );
    }
    let r = requests(&mock);
    assert_eq!(r.len(), 2);
    check(
        &r[0],
        &format!("GET {R}/%F0%9F%91%8D?limit=100&type=0"),
        "",
        None,
    );
    check(
        &r[1],
        &format!("GET {R}/blobhaj:123?limit=25&after=7&type=1"),
        "",
        None,
    );
}

#[tokio::test]
async fn message_pin_ack_and_saved_requests_match_the_web_client() {
    let saved = r#"{"message":{},"save_data":{"channel_id":"1","message_id":"2","saved_at":"s","due_at":"2026-10-17T09:30:05.123000+00:00"}}"#;
    let (mock, rest, _rx) = serve(&[
        ok(r#"{"id":"2"}"#),
        ok(r#"{"id":"2"}"#),
        empty(),
        empty(),
        empty(),
        ok(r#"{"items":[{"pinned_at":"2026-10-17T09:30:05.123000+00:00","message":{"id":"2"}}],"has_more":true}"#),
        ok(r#"{"items":[],"has_more":false}"#),
        empty(),
        ok(r#"{"token":"abc"}"#),
        ok(r#"{"token":null}"#),
        ok(r#"{"token":"x"}"#),
        ok(saved),
        ok(saved),
        empty(),
        ok(&format!(r#"{{"results":[{saved}]}}"#)),
    ])
    .await;
    rest.edit_message(1, 2, "new \"text\"", false)
        .await
        .unwrap();
    rest.edit_message(1, 2, "new", true).await.unwrap();
    rest.delete_message(1, 2, &UNKNOWN).await.unwrap();
    rest.pin_message(1, 2, &UNKNOWN).await.unwrap();
    rest.unpin_message(1, 2).await.unwrap();
    let page = rest.list_pins(1, None).await.unwrap();
    assert!(page.has_more);
    assert_eq!(page.items[0].message_id, 2);
    let cursor = page.items[0].pinned_at.clone();
    rest.list_pins(1, Some(&cursor)).await.unwrap();
    rest.ack_pins(1).await.unwrap();
    assert_eq!(
        rest.ack_message(1, 2, None, Some(3500), None)
            .await
            .unwrap(),
        Some("abc".into())
    );
    assert_eq!(
        rest.ack_message(1, 2, Some("abc"), Some(3500), Some(AckFlags::Thread))
            .await
            .unwrap(),
        None
    );
    rest.mark_unread(1, 2, 3, &UNKNOWN).await.unwrap();
    let record = rest
        .save_message(1, 2, None, SaveSource::ContextMenu)
        .await
        .unwrap();
    assert_eq!((record.channel_id, record.message_id), (1, 2));
    rest.save_message(1, 2, Some(1_792_229_405_123), SaveSource::MessageToolbar)
        .await
        .unwrap();
    rest.unsave_message(1, 2).await.unwrap();
    assert_eq!(rest.list_saved_messages().await.unwrap(), [record]);
    let r = requests(&mock);
    let m = "/api/v9/channels/1/messages/2";
    check(
        &r[0],
        &format!("PATCH {m}"),
        r#"{"content":"new \"text\""}"#,
        None,
    );
    check(
        &r[1],
        &format!("PATCH {m}"),
        r#"{"content":"new","allowed_mentions":{"parse":["users","roles","everyone"],"replied_user":false}}"#,
        None,
    );
    check(&r[2], &format!("DELETE {m}"), "", None);
    check(&r[3], "PUT /api/v9/channels/1/messages/pins/2", "", None);
    check(&r[4], "DELETE /api/v9/channels/1/messages/pins/2", "", None);
    check(
        &r[5],
        "GET /api/v9/channels/1/messages/pins?limit=25",
        "",
        None,
    );
    check(
        &r[6],
        "GET /api/v9/channels/1/messages/pins?limit=25&before=2026-10-17T09%3A30%3A05.123000%2B00%3A00",
        "",
        None,
    );
    check(&r[7], "POST /api/v9/channels/1/pins/ack", "", None);
    check(
        &r[8],
        &format!("POST {m}/ack"),
        r#"{"token":null,"last_viewed":3500}"#,
        None,
    );
    check(
        &r[9],
        &format!("POST {m}/ack"),
        r#"{"token":"abc","last_viewed":3500,"flags":2}"#,
        None,
    );
    check(
        &r[10],
        &format!("POST {m}/ack"),
        r#"{"manual":true,"mention_count":3}"#,
        None,
    );
    let s = "/api/v9/users/@me/saved-messages";
    check(
        &r[11],
        &format!("PUT {s}/1/2"),
        r#"{"source":"context_menu"}"#,
        None,
    );
    check(
        &r[12],
        &format!("PUT {s}/1/2"),
        r#"{"due_at":"2026-10-17T09:30:05.123Z","source":"message_toolbar"}"#,
        None,
    );
    check(&r[13], &format!("DELETE {s}/1/2"), "", None);
    check(&r[14], &format!("GET {s}"), "", None);
    assert_eq!(r.len(), 15);
    // A cursor that is not an ISO instant, a date out of range: refused, nothing sent.
    assert_eq!(
        rest.list_pins(1, Some("a&b=c")).await.unwrap_err(),
        ActionError::InvalidInput
    );
    assert_eq!(
        rest.save_message(1, 2, Some(-1), SaveSource::ContextMenu)
            .await
            .unwrap_err(),
        ActionError::InvalidInput
    );
    assert_eq!(mock.count(), 15);
}

#[tokio::test]
async fn sends_follow_the_official_body_order_and_context() {
    let (mock, rest, _rx) = serve(&[ok(r#"{"id":"77"}"#)]).await;
    rest.send_message(5, "hi", "N1", None).await.unwrap();
    let reply = |guild_id, mention| Reply {
        guild_id,
        message_id: 30,
        mention,
    };
    rest.send_message(5, "hi", "N2", Some(reply(Some(9), false)))
        .await
        .unwrap();
    rest.send_message(5, "hi", "N3", Some(reply(None, true)))
        .await
        .unwrap();
    rest.forward_message(6, Some(9), 5, 30, "N4", true)
        .await
        .unwrap();
    rest.forward_message(6, None, 5, 30, "N5", false)
        .await
        .unwrap();
    rest.forward_comment(6, "look", "N6", true).await.unwrap();
    rest.create_message(5, Some(9), "old", "N7", Some(30))
        .await
        .unwrap();
    let r = requests(&mock);
    let to = |c: u64| format!("POST /api/v9/channels/{c}/messages");
    check(
        &r[0],
        &to(5),
        r#"{"mobile_network_type":"unknown","content":"hi","nonce":"N1","tts":false,"flags":0}"#,
        Some(CHAT_INPUT),
    );
    check(
        &r[1],
        &to(5),
        r#"{"mobile_network_type":"unknown","content":"hi","nonce":"N2","tts":false,"message_reference":{"guild_id":"9","channel_id":"5","message_id":"30"},"allowed_mentions":{"parse":["users","roles","everyone"],"replied_user":false},"flags":0}"#,
        Some(CHAT_INPUT),
    );
    check(
        &r[2],
        &to(5),
        r#"{"mobile_network_type":"unknown","content":"hi","nonce":"N3","tts":false,"message_reference":{"channel_id":"5","message_id":"30"},"flags":0}"#,
        Some(CHAT_INPUT),
    );
    check(
        &r[3],
        &to(6),
        r#"{"mobile_network_type":"unknown","content":"","nonce":"N4","tts":false,"message_reference":{"guild_id":"9","channel_id":"5","message_id":"30","type":1},"flags":4096}"#,
        Some(FORWARDING),
    );
    check(
        &r[4],
        &to(6),
        r#"{"mobile_network_type":"unknown","content":"","nonce":"N5","tts":false,"message_reference":{"channel_id":"5","message_id":"30","type":1},"flags":0}"#,
        Some(FORWARDING),
    );
    check(
        &r[5],
        &to(6),
        r#"{"mobile_network_type":"unknown","content":"look","nonce":"N6","tts":false,"flags":4096}"#,
        Some(FORWARDING),
    );
    check(
        &r[6],
        &to(5),
        r#"{"mobile_network_type":"unknown","content":"old","nonce":"N7","tts":false,"message_reference":{"guild_id":"9","channel_id":"5","message_id":"30"},"flags":0}"#,
        Some(CHAT_INPUT),
    );
}

#[test]
fn nonces_are_snowflakes_that_never_repeat() {
    let a: u64 = crate::rest_messages::nonce().parse().unwrap();
    let b: u64 = crate::rest_messages::nonce().parse().unwrap();
    assert!(b > a && a & (1 << 21) != 0);
}

#[tokio::test]
async fn emoji_segments_are_encoded_or_refused() {
    let (mock, rest, _rx) = serve(&[empty()]).await;
    let add = |e: Emoji| {
        let rest = rest.clone();
        async move {
            rest.add_reaction(
                1,
                2,
                &e,
                ReactionType::Normal,
                ReactionLocation::Message,
                &UNKNOWN,
            )
            .await
        }
    };
    for (emoji, segment) in [
        ("\u{1F44D}", "%F0%9F%91%8D"),
        ("\u{1F44D}\u{1F3FD}", "%F0%9F%91%8D%F0%9F%8F%BD"),
        ("#\u{FE0F}\u{20E3}", "%23%EF%B8%8F%E2%83%A3"),
        ("*\u{FE0F}\u{20E3}", "*%EF%B8%8F%E2%83%A3"),
        ("1\u{FE0F}\u{20E3}", "1%EF%B8%8F%E2%83%A3"),
        (
            "\u{1F468}\u{200D}\u{1F469}\u{200D}\u{1F467}",
            "%F0%9F%91%A8%E2%80%8D%F0%9F%91%A9%E2%80%8D%F0%9F%91%A7",
        ),
    ] {
        add(Emoji::Unicode(emoji.into())).await.unwrap();
        let r = requests(&mock);
        assert_eq!(
            r.last().unwrap().line,
            format!("PUT {R}/{segment}/@me?location=Message&type=0 HTTP/1.1")
        );
    }
    let sent = mock.count();
    for bad in [
        ".",
        "..",
        "%2e%2e",
        "%2E",
        "a/b",
        "%",
        "",
        "a",
        "1",
        "\u{1F44D}/",
        "\u{1F44D}\n",
        "\u{1F44D}?",
        "\u{1F44D}\u{85}",
    ] {
        assert_eq!(
            add(Emoji::Unicode(bad.into())).await.unwrap_err(),
            ActionError::InvalidInput,
            "{bad:?}"
        );
    }
    for name in ["", "a:b", "a/b", "..", "%2e", &"x".repeat(33)] {
        let emoji = Emoji::Custom {
            name: name.into(),
            id: 1,
        };
        assert_eq!(
            add(emoji).await.unwrap_err(),
            ActionError::InvalidInput,
            "{name:?}"
        );
    }
    assert_eq!(mock.count(), sent, "a refused emoji reached the server");
}

/// S-1 at the `Call` level: a composed path the parser would change (dot segments, an encoded
/// dot, a re-encoded byte) fails with InvalidUrl and nothing is sent.
#[tokio::test]
async fn a_composed_path_that_parses_differently_is_refused() {
    let (mock, rest, _rx) = serve(&[empty()]).await;
    for path in [
        "/channels/1/messages/2/reactions/../../../../users/@me",
        "/channels/1/messages/2/reactions/%2e%2e/@me",
        "/channels/1/messages/2/reactions/./@me",
        "/channels/1/messages/2/reactions/a b/@me",
        "/channels/1/messages/2/reactions/\u{1F44D}/@me",
        "/channels/1/messages?location=a b",
        "/channels/1/messages/2#x",
    ] {
        assert!(
            matches!(
                Call::new(Method::Delete, path),
                Err(ActionError::InvalidUrl)
            ),
            "{path}"
        );
    }
    let call = Call::new(Method::Delete, "/channels/1/messages/2").unwrap();
    rest.call::<serde::de::IgnoredAny>(&call, Retry::None)
        .await
        .unwrap();
    assert_eq!(mock.count(), 1);
}

// ---- Retry table ----

async fn add_once(rest: &Rest) -> Result<(), ActionError> {
    rest.add_reaction(
        1,
        2,
        &thumbs(),
        ReactionType::Normal,
        ReactionLocation::Message,
        &UNKNOWN,
    )
    .await
}

#[tokio::test(start_paused = true)]
async fn reaction_429_with_a_readable_wait_waits_then_retries_once() {
    let (mock, rest, _rx) = serve(&[
        json_status(429, r#"{"retry_after":0.05,"global":false}"#),
        empty(),
    ])
    .await;
    let started = Instant::now();
    add_once(&rest).await.unwrap();
    assert_eq!(mock.count(), 2);
    // The per-route block (max(wait, 1 s backoff)) is what the retry waits for.
    assert!(started.elapsed() >= Duration::from_secs(1));
    assert_eq!(requests(&mock)[1].header("x-failed-requests"), None);
    // Twice limited: one retry only.
    let (mock, rest, _rx) = serve(&[response(429, "Retry-After: 1\r\n", "")]).await;
    assert_eq!(add_once(&rest).await.unwrap_err(), ActionError::RateLimited);
    assert_eq!(mock.count(), 2);
}

/// S-5: a 429 over the 10 s cap is not retried and still arms the global block from its body;
/// an unreadable one (no header, no JSON) is not retried.
#[tokio::test(start_paused = true)]
async fn reaction_429_over_the_cap_or_unreadable_is_not_retried_and_global_still_arms() {
    let (mock, rest, _rx) = serve(&[
        json_status(429, r#"{"retry_after":30,"global":true}"#),
        empty(),
    ])
    .await;
    assert_eq!(add_once(&rest).await.unwrap_err(), ActionError::RateLimited);
    assert_eq!(mock.count(), 1);
    assert_eq!(
        rest.ack_pins(9).await.unwrap_err(),
        ActionError::RateLimited
    );
    assert_eq!(mock.count(), 1, "the global block did not hold");
    tokio::time::advance(Duration::from_secs(31)).await;
    rest.ack_pins(9).await.unwrap();

    let (mock, rest, _rx) = serve(&[response(429, "", "slow down"), empty()]).await;
    assert_eq!(add_once(&rest).await.unwrap_err(), ActionError::RateLimited);
    assert_eq!(mock.count(), 1);

    // The global flag arms whatever the wait source: here the header's.
    let (mock, rest, _rx) = serve(&[
        response(429, "Retry-After: 20\r\n", r#"{"global":true}"#),
        empty(),
    ])
    .await;
    assert_eq!(
        rest.ack_pins(1).await.unwrap_err(),
        ActionError::RateLimited
    );
    assert_eq!(
        rest.ack_pins(9).await.unwrap_err(),
        ActionError::RateLimited
    );
    assert_eq!(mock.count(), 1);
}

#[tokio::test]
async fn reaction_403_30010_and_90001_and_redirects_are_not_retried() {
    for (body, code) in [
        (r#"{"code":30010,"message":"x"}"#, ApiCode::TooManyReactions),
        (r#"{"code":90001}"#, ApiCode::ReactionBlocked),
    ] {
        let (mock, rest, _rx) = serve(&[json_status(403, body), empty()]).await;
        assert_eq!(
            add_once(&rest).await.unwrap_err(),
            ActionError::Api {
                status: 403,
                code,
                message: None
            }
        );
        assert_eq!(mock.count(), 1);
    }
    let (mock, rest, _rx) = serve(&[
        response(302, "Location: https://discord.com/x\r\n", ""),
        empty(),
    ])
    .await;
    assert!(matches!(
        add_once(&rest).await.unwrap_err(),
        ActionError::Api { status: 302, .. }
    ));
    assert_eq!(mock.count(), 1);
}

#[tokio::test]
async fn reaction_other_failures_retry_once_immediately() {
    for first in [
        response(500, "", ""),
        json_status(403, r#"{"code":50001}"#),
        "ERROR timeout".into(),
        "ERROR connect".into(),
    ] {
        let (mock, rest, _rx) = serve(&[first, empty()]).await;
        add_once(&rest).await.unwrap();
        assert_eq!(mock.count(), 2);
    }
    let (mock, rest, _rx) = serve(&[response(500, "", "")]).await;
    assert!(matches!(
        add_once(&rest).await.unwrap_err(),
        ActionError::Api { status: 500, .. }
    ));
    assert_eq!(mock.count(), 2);
    // Remove own and remove all use the same handler.
    let (mock, rest, _rx) = serve(&[response(500, "", ""), empty()]).await;
    rest.remove_reaction(
        1,
        2,
        &thumbs(),
        ReactionType::Normal,
        None,
        ReactionLocation::Message,
        &UNKNOWN,
    )
    .await
    .unwrap();
    rest.remove_all_reactions(1, 3, &UNKNOWN).await.unwrap();
    assert_eq!(mock.count(), 3);
}

#[tokio::test]
async fn remove_one_emoji_and_another_users_reaction_are_never_retried() {
    let (mock, rest, _rx) = serve(&[response(502, "", ""), empty()]).await;
    assert!(rest
        .remove_emoji_reactions(1, 2, &thumbs(), &UNKNOWN)
        .await
        .is_err());
    assert_eq!(mock.count(), 1);
    let (mock, rest, _rx) = serve(&[response(502, "", ""), empty()]).await;
    assert!(rest
        .remove_reaction(
            1,
            2,
            &thumbs(),
            ReactionType::Normal,
            Some(5),
            ReactionLocation::Message,
            &UNKNOWN
        )
        .await
        .is_err());
    assert_eq!(mock.count(), 1);
}

#[tokio::test(start_paused = true)]
async fn list_pins_retries_502_twice_with_x_failed_requests() {
    let page = ok(r#"{"items":[],"has_more":false}"#);
    let (mock, rest, _rx) = serve(&[response(502, "", ""), page.clone()]).await;
    rest.list_pins(1, None).await.unwrap();
    let r = requests(&mock);
    assert_eq!(
        (
            r.len(),
            r[0].header("x-failed-requests"),
            r[1].header("x-failed-requests")
        ),
        (2, None, Some("1"))
    );
    let (mock, rest, _rx) = serve(&[
        response(504, "", ""),
        "ERROR timeout".into(),
        response(599, "", ""),
    ])
    .await;
    assert!(rest.list_pins(1, None).await.is_err());
    let r = requests(&mock);
    assert_eq!(r.len(), 3, "retries:2 is three requests at most");
    assert_eq!(r[2].header("x-failed-requests"), Some("2"));
    for status in [500, 503, 429, 400] {
        let (mock, rest, _rx) = serve(&[response(status, "", ""), page.clone()]).await;
        assert!(rest.list_pins(1, None).await.is_err(), "{status}");
        assert_eq!(mock.count(), 1, "{status} was retried");
    }
}

#[tokio::test]
async fn a_2xx_with_an_unparsable_body_is_not_retried() {
    let (mock, rest, _rx) = serve(&[ok("not json"), ok(r#"{"items":[],"has_more":false}"#)]).await;
    assert_eq!(
        rest.list_pins(1, None).await.unwrap_err(),
        ActionError::BadResponse
    );
    assert_eq!(mock.count(), 1);
    let (mock, rest, _rx) = serve(&[ok("{"), empty()]).await;
    assert_eq!(add_once(&rest).await.unwrap_err(), ActionError::BadResponse);
    assert_eq!(mock.count(), 1);
}

#[tokio::test]
async fn gets_without_retries_and_writes_without_retries_send_once() {
    let (mock, rest, _rx) = serve(&[response(502, "", ""), empty()]).await;
    assert!(rest
        .list_reactors(1, 2, &thumbs(), ReactionType::Normal, 10, None)
        .await
        .is_err());
    assert!(rest.list_saved_messages().await.is_err());
    assert!(rest.frecency_get().await.is_err());
    assert_eq!(mock.count(), 3);
    for i in 0..9u64 {
        let (mock, rest, _rx) = serve(&[response(502, "", ""), empty()]).await;
        let result = match i {
            0 => rest.delete_message(1, 2, &UNKNOWN).await,
            1 => rest.pin_message(1, 2, &UNKNOWN).await,
            2 => rest.unpin_message(1, 2).await,
            3 => rest.ack_pins(1).await,
            4 => rest.ack_message(1, 2, None, None, None).await.map(drop),
            5 => rest.mark_unread(1, 2, 0, &UNKNOWN).await,
            6 => rest
                .save_message(1, 2, None, SaveSource::ContextMenu)
                .await
                .map(drop),
            7 => rest.unsave_message(1, 2).await,
            _ => rest.frecency_patch(|_| None).await.map(drop),
        };
        assert!(result.is_err(), "{i}");
        assert_eq!(mock.count(), 1, "action {i} was retried");
    }
}

#[tokio::test(start_paused = true)]
async fn edit_retries_a_502_once_and_re_sends_once_after_a_429() {
    let (mock, rest, _rx) = serve(&[response(502, "", ""), empty()]).await;
    rest.edit_message(1, 2, "e", false).await.unwrap();
    let r = requests(&mock);
    assert_eq!((r.len(), r[1].header("x-failed-requests")), (2, Some("1")));
    assert_eq!(r[0].body, r[1].body);
    let (mock, rest, _rx) = serve(&[json_status(429, r#"{"retry_after":0.2}"#), empty()]).await;
    rest.edit_message(1, 2, "e", false).await.unwrap();
    let r = requests(&mock);
    assert_eq!((r.len(), r[1].header("x-failed-requests")), (2, None));
    let (mock, rest, _rx) = serve(&[response(500, "", ""), empty()]).await;
    assert!(rest.edit_message(1, 2, "e", false).await.is_err());
    assert_eq!(mock.count(), 1);
    let (mock, rest, _rx) = serve(&[response(502, "", "")]).await;
    assert!(rest.edit_message(1, 2, "e", false).await.is_err());
    assert_eq!(mock.count(), 2);
}

/// D11: a send is retried only when the connection never opened (or 522 / 523), with the same
/// bytes and nonce and `X-Failed-Requests: N`; never after a 504, 502, 429, 500 or a timeout.
#[tokio::test(start_paused = true)]
async fn a_send_is_retried_only_when_provably_unsent() {
    let (mock, rest, _rx) = serve(&[
        "ERROR connect".into(),
        "ERROR connect".into(),
        ok(r#"{"id":"1"}"#),
    ])
    .await;
    rest.send_message(5, "hi", "N", None).await.unwrap();
    let r = requests(&mock);
    assert_eq!(r.len(), 3);
    assert!(r.iter().all(|x| x.body == r[0].body && x.line == r[0].line));
    assert!(r[0].body.contains(r#""nonce":"N""#));
    assert_eq!(
        r.iter()
            .map(|x| x.header("x-failed-requests"))
            .collect::<Vec<_>>(),
        [None, Some("1"), Some("2")]
    );
    assert!(r
        .iter()
        .all(|x| x.header("x-context-properties") == Some(CHAT_INPUT)));
    for status in [522, 523] {
        let (mock, rest, _rx) = serve(&[response(status, "", ""), ok(r#"{"id":"1"}"#)]).await;
        rest.send_message(5, "hi", "N", None).await.unwrap();
        assert_eq!(mock.count(), 2);
    }
    for first in [
        response(504, "", ""),
        response(502, "", ""),
        response(500, "", ""),
        json_status(429, r#"{"retry_after":0.01}"#),
        "ERROR timeout".into(),
        "ERROR other".into(),
    ] {
        let (mock, rest, _rx) = serve(&[first.clone(), ok(r#"{"id":"1"}"#)]).await;
        assert!(
            rest.send_message(5, "hi", "N", None).await.is_err(),
            "{first}"
        );
        assert_eq!(mock.count(), 1, "{first} was retried");
    }
    let (mock, rest, _rx) = serve(&["ERROR connect".into()]).await;
    assert_eq!(
        rest.forward_message(6, None, 5, 30, "N", false)
            .await
            .unwrap_err(),
        ActionError::Network
    );
    assert_eq!(mock.count(), 4, "three retries at most");
}

/// S-4: a 429 blocks its path (any method, query excluded) for max(wait, backoff); the backoff
/// doubles on the next 429; other paths are untouched.
#[tokio::test(start_paused = true)]
async fn a_429_blocks_its_path_for_max_wait_and_backoff() {
    let (mock, rest, _rx) = serve(&[json_status(429, r#"{"retry_after":0.01}"#), empty()]).await;
    assert_eq!(
        rest.delete_message(1, 2, &UNKNOWN).await.unwrap_err(),
        ActionError::RateLimited
    );
    tokio::time::advance(Duration::from_millis(500)).await;
    // Same path, another method: blocked, nothing sent.
    assert_eq!(
        rest.edit_message(1, 2, "e", false).await.unwrap_err(),
        ActionError::RateLimited
    );
    assert_eq!(mock.count(), 1);
    rest.delete_message(1, 3, &UNKNOWN).await.unwrap();
    assert_eq!(mock.count(), 2);
    tokio::time::advance(Duration::from_millis(500)).await;
    rest.delete_message(1, 2, &UNKNOWN).await.unwrap();
    assert_eq!(mock.count(), 3);

    let (mock, rest, _rx) = serve(&[json_status(429, r#"{"retry_after":0.01}"#)]).await;
    rest.unpin_message(1, 2).await.unwrap_err();
    tokio::time::advance(Duration::from_millis(1001)).await;
    rest.unpin_message(1, 2).await.unwrap_err();
    tokio::time::advance(Duration::from_millis(1500)).await;
    assert_eq!(
        rest.unpin_message(1, 2).await.unwrap_err(),
        ActionError::RateLimited
    );
    assert_eq!(mock.count(), 2, "the second backoff is 2 s");
    tokio::time::advance(Duration::from_millis(501)).await;
    rest.unpin_message(1, 2).await.unwrap_err();
    assert_eq!(mock.count(), 3);

    // A success on the path ends its backoff: the next 429 starts again at 1 s.
    let limited = json_status(429, r#"{"retry_after":0.01}"#);
    let (mock, rest, _rx) = serve(&[limited.clone(), empty(), limited]).await;
    rest.unpin_message(1, 2).await.unwrap_err();
    tokio::time::advance(Duration::from_millis(1001)).await;
    rest.unpin_message(1, 2).await.unwrap();
    rest.unpin_message(1, 2).await.unwrap_err();
    tokio::time::advance(Duration::from_millis(1001)).await;
    rest.unpin_message(1, 2).await.unwrap_err();
    assert_eq!(mock.count(), 4);
}

/// Yields until `mock` has recorded `n` requests (the spawned write is then in flight).
async fn in_flight(mock: &MockHttp, n: usize) {
    while mock.count() < n {
        tokio::task::yield_now().await;
    }
}

/// S-16: a second identical write while one is in flight is `Busy`, without a request.
#[tokio::test]
async fn a_second_write_while_one_is_in_flight_is_busy() {
    let (mock, rest, _rx) = serve(&[HANG.into()]).await;
    let spawn = |f: fn(Rest) -> tokio::task::JoinHandle<Result<(), ActionError>>| f(rest.clone());
    let first = spawn(|r| tokio::spawn(async move { add_once(&r).await }));
    in_flight(&mock, 1).await;
    assert_eq!(add_once(&rest).await.unwrap_err(), ActionError::Busy);
    assert_eq!(
        rest.remove_reaction(
            1,
            2,
            &thumbs(),
            ReactionType::Normal,
            None,
            ReactionLocation::Message,
            &UNKNOWN
        )
        .await
        .unwrap_err(),
        ActionError::Busy
    );
    assert_eq!(mock.count(), 1);
    // Another emoji of the same message is another key.
    let other = spawn(|r| {
        tokio::spawn(async move {
            r.add_reaction(
                1,
                2,
                &blobhaj(),
                ReactionType::Normal,
                ReactionLocation::Message,
                &UNKNOWN,
            )
            .await
        })
    });
    in_flight(&mock, 2).await;
    first.abort();
    other.abort();
    let _ = (first.await, other.await);
    // Freed once the first ends (here: cancelled).
    let again = spawn(|r| tokio::spawn(async move { add_once(&r).await }));
    in_flight(&mock, 3).await;
    again.abort();
    let _ = again.await;

    let edit = spawn(|r| tokio::spawn(async move { r.edit_message(1, 2, "a", false).await }));
    in_flight(&mock, 4).await;
    assert_eq!(
        rest.edit_message(1, 2, "b", false).await.unwrap_err(),
        ActionError::Busy
    );
    let delete = spawn(|r| tokio::spawn(async move { r.delete_message(1, 2, &UNKNOWN).await }));
    in_flight(&mock, 5).await;
    assert_eq!(
        rest.delete_message(1, 2, &UNKNOWN).await.unwrap_err(),
        ActionError::Busy
    );
    let pin = spawn(|r| tokio::spawn(async move { r.pin_message(1, 2, &UNKNOWN).await }));
    in_flight(&mock, 6).await;
    assert_eq!(
        rest.unpin_message(1, 2).await.unwrap_err(),
        ActionError::Busy
    );
    assert_eq!(mock.count(), 6);
    for task in [edit, delete, pin] {
        task.abort();
        let _ = task.await;
    }
}

// ---- Thread preconditions ----

fn archived(manage: Tri, locked: Tri, joined: Tri) -> ThreadState {
    ThreadState {
        archived: Tri::Yes,
        locked,
        joined,
        manage_threads: manage,
    }
}

#[tokio::test]
async fn an_archived_thread_is_unarchived_before_the_action() {
    for thread in [
        archived(Tri::Yes, Tri::Unknown, Tri::Unknown),
        archived(Tri::No, Tri::No, Tri::Unknown),
    ] {
        let (mock, rest, _rx) = serve(&[ok("{}"), empty()]).await;
        rest.add_reaction(
            7,
            2,
            &thumbs(),
            ReactionType::Normal,
            ReactionLocation::Message,
            &thread,
        )
        .await
        .unwrap();
        rest.delete_message(7, 2, &thread).await.unwrap();
        rest.pin_message(7, 2, &thread).await.unwrap();
        let r = requests(&mock);
        check(
            &r[0],
            "PATCH /api/v9/channels/7",
            r#"{"archived":false}"#,
            None,
        );
        assert!(r[1]
            .line
            .starts_with("PUT /api/v9/channels/7/messages/2/reactions/"));
        check(
            &r[2],
            "PATCH /api/v9/channels/7",
            r#"{"archived":false}"#,
            None,
        );
        assert!(r[3]
            .line
            .starts_with("DELETE /api/v9/channels/7/messages/2 "));
        check(
            &r[4],
            "PATCH /api/v9/channels/7",
            r#"{"archived":false}"#,
            None,
        );
        assert!(r[5]
            .line
            .starts_with("PUT /api/v9/channels/7/messages/pins/2 "));
    }
}

#[tokio::test]
async fn a_failed_unarchive_stops_the_action_with_its_error() {
    let thread = archived(Tri::Yes, Tri::Unknown, Tri::Unknown);
    let (mock, rest, _rx) = serve(&[json_status(403, r#"{"code":50001}"#), empty()]).await;
    let err = rest.delete_message(7, 2, &thread).await.unwrap_err();
    assert_eq!(
        err,
        ActionError::Api {
            status: 403,
            code: ApiCode::InvalidAccess,
            message: None
        }
    );
    assert_eq!(
        mock.count(),
        1,
        "the action was sent after a failed unarchive"
    );
}

/// With Unknown (all F2 has until F1), nothing is unarchived or joined; a 50083 is typed.
#[tokio::test]
async fn unknown_thread_state_sends_neither_unarchive_nor_join() {
    assert_eq!(thread_state(&State::default(), 7), UNKNOWN);
    assert_eq!(ThreadState::default(), UNKNOWN);
    // Archived but neither MANAGE_THREADS nor "not locked" known: no unarchive either.
    for thread in [
        UNKNOWN,
        archived(Tri::Unknown, Tri::Unknown, Tri::No),
        archived(Tri::No, Tri::Yes, Tri::No),
        // Archived unknown: no unarchive, whatever else is known.
        ThreadState {
            archived: Tri::Unknown,
            locked: Tri::No,
            joined: Tri::No,
            manage_threads: Tri::Yes,
        },
    ] {
        let (mock, rest, _rx) = serve(&[json_status(400, r#"{"code":50083}"#)]).await;
        let err = rest.pin_message(7, 2, &thread).await.unwrap_err();
        assert_eq!(
            err,
            ActionError::Api {
                status: 400,
                code: ApiCode::ArchivedThread,
                message: None
            }
        );
        let r = requests(&mock);
        assert_eq!(r.len(), 1);
        assert!(r[0]
            .line
            .starts_with("PUT /api/v9/channels/7/messages/pins/2 "));
    }
    let (mock, rest, _rx) = serve(&[empty()]).await;
    rest.mark_unread(7, 2, 0, &UNKNOWN).await.unwrap();
    let r = requests(&mock);
    assert_eq!(r.len(), 1);
    assert!(r[0]
        .line
        .starts_with("POST /api/v9/channels/7/messages/2/ack "));
}

#[tokio::test]
async fn mark_unread_unarchives_joins_and_acks_even_when_the_join_fails() {
    // No MANAGE_THREADS or locked check for Mark Unread (§17.4).
    let thread = archived(Tri::No, Tri::Yes, Tri::No);
    let (mock, rest, _rx) =
        serve(&[ok("{}"), json_status(403, r#"{"code":50001}"#), empty()]).await;
    rest.mark_unread(7, 2, 1, &thread).await.unwrap();
    let r = requests(&mock);
    assert_eq!(r.len(), 3);
    check(
        &r[0],
        "PATCH /api/v9/channels/7",
        r#"{"archived":false}"#,
        None,
    );
    check(
        &r[1],
        "POST /api/v9/channels/7/thread-members/@me?location=Mark%20Unread",
        "",
        None,
    );
    check(
        &r[2],
        "POST /api/v9/channels/7/messages/2/ack",
        r#"{"manual":true,"mention_count":1}"#,
        None,
    );
    // A failed unarchive stops it.
    let (mock, rest, _rx) = serve(&[response(500, "", ""), empty()]).await;
    assert!(rest.mark_unread(7, 2, 1, &thread).await.is_err());
    assert_eq!(mock.count(), 1);
}

// ---- 401, MFA ----

const MFA_BODY: &str = r#"{"code":60003,"message":"m","mfa":{"ticket":"TICKET-1","methods":[{"type":"totp"},{"type":"webauthn","challenge":"CH"},{"type":"carrier-pigeon"}]}}"#;

#[tokio::test]
async fn mfa_required_does_not_latch_and_the_answer_re_sends_the_request_unchanged() {
    let (mock, rest, mut rx) = serve(&[
        json_status(401, MFA_BODY),
        ok(r#"{"phone":"+1"}"#),
        ok(r#"{"token":"MFA-TOKEN-XYZ"}"#),
        empty(),
        empty(),
    ])
    .await;
    let Err(ActionError::MfaRequired(challenge)) = rest.delete_message(1, 2, &UNKNOWN).await else {
        panic!("no MFA challenge");
    };
    assert_eq!(
        challenge
            .methods
            .iter()
            .map(|m| (m.kind, m.challenge.as_deref()))
            .collect::<Vec<_>>(),
        [(MfaKind::Totp, None), (MfaKind::Webauthn, Some("CH"))]
    );
    assert!(rx.try_recv().is_err(), "MFA latched REST off");
    let shown = format!(
        "{:?} {}",
        ActionError::MfaRequired(challenge.clone()),
        ActionError::MfaRequired(challenge.clone())
    );
    assert!(
        !shown.contains("TICKET") && !shown.contains("CH"),
        "{shown}"
    );
    rest.mfa_send_sms(challenge.id).await.unwrap();
    rest.mfa_submit(challenge.id, MfaKind::Totp, "123456")
        .await
        .unwrap();
    let r = requests(&mock);
    check(
        &r[1],
        "POST /api/v9/auth/mfa/sms/send",
        r#"{"ticket":"TICKET-1"}"#,
        None,
    );
    check(
        &r[2],
        "POST /api/v9/mfa/finish",
        r#"{"ticket":"TICKET-1","mfa_type":"totp","data":"123456"}"#,
        None,
    );
    assert_eq!(r[3].line, r[0].line);
    assert_eq!(r[3].body, r[0].body);
    assert_eq!(
        r[3].header("x-discord-mfa-authorization"),
        Some("MFA-TOKEN-XYZ")
    );
    // Answered: the challenge is gone, and the token was used once only.
    assert_eq!(
        rest.mfa_submit(challenge.id, MfaKind::Totp, "1")
            .await
            .unwrap_err(),
        ActionError::InvalidInput
    );
    rest.ack_pins(1).await.unwrap();
    assert_eq!(
        requests(&mock)[4].header("x-discord-mfa-authorization"),
        None
    );
    assert_eq!(mock.count(), 5);
}

#[tokio::test]
async fn an_invalid_mfa_code_on_the_re_send_keeps_the_challenge() {
    let (mock, rest, _rx) = serve(&[
        json_status(401, MFA_BODY),
        ok(r#"{"token":"T1"}"#),
        json_status(400, r#"{"code":60008,"message":"Invalid two-factor code"}"#),
        ok(r#"{"token":"T2"}"#),
        empty(),
    ])
    .await;
    let Err(ActionError::MfaRequired(c)) = rest.pin_message(1, 2, &UNKNOWN).await else {
        panic!("no MFA challenge");
    };
    assert!(matches!(
        rest.mfa_submit(c.id, MfaKind::Backup, "abcd-efgh")
            .await
            .unwrap_err(),
        ActionError::Api {
            code: ApiCode::MfaInvalidCode,
            ..
        }
    ));
    rest.mfa_submit(c.id, MfaKind::Totp, "654321")
        .await
        .unwrap();
    let r = requests(&mock);
    assert_eq!(r[4].header("x-discord-mfa-authorization"), Some("T2"));
    assert_eq!(mock.count(), 5);
}

#[tokio::test]
async fn a_401_latches_every_action_off() {
    let (mock, rest, mut rx) = serve(&[response(401, "", ""), empty()]).await;
    assert_eq!(
        add_once(&rest).await.unwrap_err(),
        ActionError::Unauthorized
    );
    assert_eq!(
        rest.ack_pins(1).await.unwrap_err(),
        ActionError::Unauthorized
    );
    assert_eq!(
        rest.send_message(5, "hi", "N", None).await.unwrap_err(),
        ActionError::Unauthorized
    );
    assert_eq!(mock.count(), 1);
    assert_eq!(rx.try_recv(), Ok(()));
    // A 401 with code 60003 but no `mfa` object is a plain 401.
    let (mock, rest, mut rx) = serve(&[json_status(401, r#"{"code":60003}"#), empty()]).await;
    assert_eq!(
        rest.ack_pins(1).await.unwrap_err(),
        ActionError::Unauthorized
    );
    assert_eq!((mock.count(), rx.try_recv()), (1, Ok(())));
}

#[tokio::test]
async fn error_bodies_give_typed_codes_and_only_saved_messages_keep_the_message() {
    for (code, want) in [
        (10008, ApiCode::UnknownMessage),
        (30003, ApiCode::TooManyPins),
        (50019, ApiCode::InvalidPinChannel),
        (50021, ApiCode::SystemMessage),
        (50035, ApiCode::InvalidFormBody),
        (12345, ApiCode::Other),
    ] {
        let (_mock, rest, _rx) = serve(&[json_status(
            400,
            &format!(r#"{{"code":{code},"message":"server text"}}"#),
        )])
        .await;
        assert_eq!(
            rest.pin_message(1, 2, &UNKNOWN).await.unwrap_err(),
            ActionError::Api {
                status: 400,
                code: want,
                message: None
            }
        );
    }
    let long = "y".repeat(600);
    let (_mock, rest, _rx) = serve(&[json_status(
        400,
        &format!(r#"{{"code":30074,"message":"{long}"}}"#),
    )])
    .await;
    let err = rest
        .save_message(1, 2, None, SaveSource::ContextMenu)
        .await
        .unwrap_err();
    let ActionError::Api {
        code: ApiCode::TooManySavedMessages,
        message: Some(message),
        ..
    } = &err
    else {
        panic!("{err:?}");
    };
    assert_eq!(message.chars().count(), 512);
    assert!(!format!("{err:?} {err}").contains("yyy"));
    let (_mock, rest, _rx) = serve(&[json_status(400, r#"{"captcha_key":["x"]}"#)]).await;
    assert_eq!(rest.ack_pins(1).await.unwrap_err(), ActionError::Captcha);
}

#[tokio::test]
async fn without_a_mock_nothing_is_sent_anywhere() {
    crate::rest::test_support::TEST_HTTP.with(|h| *h.borrow_mut() = None);
    let (tx, _rx) = mpsc::unbounded_channel();
    let rest = Rest::test(Arc::new(Token::new(MARKER.into()).unwrap()), 7, tx);
    assert_eq!(rest.ack_pins(1).await.unwrap_err(), ActionError::Network);
}

// ---- Settings-proto ----

fn item(total: u32, recent: &[u64]) -> FrecencyItem {
    FrecencyItem {
        total_uses: total,
        recent_uses: recent.to_vec(),
        frecency: -3,
        score: 7,
        unknown: vec![],
    }
}

/// A canonical blob: field 1 (versions, unmodelled), 5, 6 with an unknown field inside a
/// FrecencyItem, 13, and an unknown top-level field 99.
fn blob() -> Vec<u8> {
    let mut with_unknown = item(2, &[1_700_000_000_000, 5]);
    with_unknown.unknown.push(Field {
        number: 7,
        value: PValue::Varint(42),
    });
    let settings = FrecencySettings {
        fields: FrecencyPatch {
            favorite_stickers: None,
            favorite_emojis: Some(FavoriteEmojis {
                emojis: vec!["thumbsup".into(), "123".into()],
                unknown: vec![],
            }),
            emoji_frecency: Some(EmojiFrecency {
                emojis: vec![("eyes".into(), with_unknown)],
                unknown: vec![],
            }),
            emoji_reaction_frecency: Some(EmojiFrecency {
                emojis: vec![("100".into(), item(4, &[9]))],
                unknown: vec![],
            }),
        },
        other: vec![
            Field {
                number: 1,
                value: PValue::Len(vec![8, 1]),
            },
            Field {
                number: 99,
                value: PValue::Fixed32(5),
            },
        ],
    };
    settings.encode()
}

fn settings_body(bytes: &[u8], out_of_date: bool) -> String {
    let flag = if out_of_date {
        r#","out_of_date":true"#
    } else {
        ""
    };
    ok(&format!(
        r#"{{"settings":"{}"{flag}}}"#,
        base64_encode(bytes)
    ))
}

#[test]
fn frecency_round_trips_unknown_fields_byte_for_byte() {
    let bytes = blob();
    let decoded = FrecencySettings::decode(&bytes).unwrap();
    let inner = &decoded.fields.emoji_frecency.as_ref().unwrap().emojis[0].1;
    assert_eq!(
        inner.unknown,
        [Field {
            number: 7,
            value: PValue::Varint(42)
        }]
    );
    assert_eq!((inner.total_uses, inner.frecency, inner.score), (2, -3, 7));
    assert_eq!(
        decoded.other.iter().map(|f| f.number).collect::<Vec<_>>(),
        [1, 99]
    );
    assert_eq!(decoded.encode(), bytes);
    // Unpacked recent_uses are read too.
    let mut m = Message::new();
    m.push_varint(2, 11);
    m.push_varint(2, 12);
    let mut entry = Message::new();
    entry.push_string(1, "k");
    entry.push_message(2, &m);
    let mut map = Message::new();
    map.push_message(1, &entry);
    let mut top = Message::new();
    top.push_message(13, &map);
    let d = FrecencySettings::decode(&top.encode()).unwrap();
    assert_eq!(
        d.fields.emoji_reaction_frecency.unwrap().emojis[0]
            .1
            .recent_uses,
        [11, 12]
    );
}

#[test]
fn the_frecency_decoder_is_bounded() {
    let bytes = blob();
    for cut in 1..bytes.len() {
        // Every truncation either fails or decodes a prefix that is itself whole; none panics.
        let _ = FrecencySettings::decode(&bytes[..cut]);
    }
    assert!(FrecencySettings::decode(&bytes[..bytes.len() - 1]).is_err());
    // 2000 levels of nesting under an unmodelled field: kept as bytes, never descended (PB's
    // depth limit guards every descent), and written back unchanged.
    let mut deep = Message::new();
    deep.push_varint(1, 1);
    for _ in 0..2000 {
        let mut outer = Message::new();
        outer.push_message(1, &deep);
        deep = outer;
    }
    let mut top = Message::new();
    top.push_message(99, &deep);
    let bytes = top.encode();
    assert_eq!(FrecencySettings::decode(&bytes).unwrap().encode(), bytes);
    // The same nesting where a typed field is expected is a type error, not a deeper walk.
    let mut top = Message::new();
    top.push_message(5, &deep);
    assert!(FrecencySettings::decode(&top.encode()).is_err());
    assert_eq!(
        FrecencySettings::decode(&[0x0a, 0xff, 0xff, 0xff, 0xff, 0x0f]).unwrap_err(),
        ProtoError::LengthOutOfBounds
    );
    assert_eq!(
        FrecencySettings::from_base64("!!").unwrap_err(),
        ProtoError::Base64
    );
}

#[tokio::test]
async fn no_settings_patch_before_a_successful_decode_and_a_decode_failure_disables_writes() {
    let (mock, rest, _rx) =
        serve(&[ok(r#"{"settings":"!!!"}"#), settings_body(&blob(), false)]).await;
    let mut merged = false;
    let err = rest
        .frecency_patch(|_| {
            merged = true;
            Some(FrecencyPatch::default())
        })
        .await
        .unwrap_err();
    assert_eq!(err, ActionError::BadResponse);
    assert!(!merged);
    assert_eq!(
        requests(&mock)
            .iter()
            .map(|r| r.line.clone())
            .collect::<Vec<_>>(),
        ["GET /api/v9/users/@me/settings-proto/2 HTTP/1.1"]
    );
    // Off for the session, even though the server would now answer well: nothing is sent.
    assert_eq!(
        rest.frecency_patch(|_| Some(FrecencyPatch::default()))
            .await
            .unwrap_err(),
        ActionError::SettingsUnavailable
    );
    assert_eq!(mock.count(), 1);
    // A truncated proto disables writes as well.
    let (mock, rest, _rx) = serve(&[settings_body(&blob()[..5], false)]).await;
    assert!(rest.frecency_get().await.is_err());
    assert_eq!(
        rest.frecency_patch(|_| None).await.unwrap_err(),
        ActionError::SettingsUnavailable
    );
    assert_eq!(mock.count(), 1);
}

#[tokio::test]
async fn every_settings_patch_follows_a_get_of_type_2_and_sends_only_the_partial_proto() {
    let server = blob();
    let (mock, rest, _rx) = serve(&[settings_body(&server, false)]).await;
    let favourites = FavoriteEmojis {
        emojis: vec!["eyes".into()],
        unknown: vec![],
    };
    for _ in 0..2 {
        let outcome = rest
            .frecency_patch(|current| {
                // The merge sees the server's fresh copy.
                assert_eq!(current.encode(), server);
                Some(FrecencyPatch {
                    favorite_emojis: Some(favourites.clone()),
                    ..Default::default()
                })
            })
            .await
            .unwrap();
        assert!(matches!(outcome, PatchOutcome::Applied(_)));
    }
    let r = requests(&mock);
    let lines: Vec<_> = r
        .iter()
        .map(|x| x.line.split(' ').take(2).collect::<Vec<_>>().join(" "))
        .collect();
    assert_eq!(
        lines,
        [
            "GET /api/v9/users/@me/settings-proto/2",
            "PATCH /api/v9/users/@me/settings-proto/2",
            "GET /api/v9/users/@me/settings-proto/2",
            "PATCH /api/v9/users/@me/settings-proto/2"
        ]
    );
    // Field 5 only: FavoriteEmojis {1: "eyes"}; no required_data_version.
    let mut fav = Message::new();
    fav.push_string(1, "eyes");
    let mut partial = Message::new();
    partial.push_message(5, &fav);
    check(
        &r[1],
        "PATCH /api/v9/users/@me/settings-proto/2",
        &format!(r#"{{"settings":"{}"}}"#, base64_encode(&partial.encode())),
        None,
    );
    // A merge that writes nothing sends no PATCH.
    let before = mock.count();
    assert!(matches!(
        rest.frecency_patch(|_| None).await.unwrap(),
        PatchOutcome::Unchanged(_)
    ));
    assert_eq!(mock.count(), before + 1);
}

#[tokio::test]
async fn out_of_date_or_400_discards_the_change_and_reloads_without_retrying() {
    let newer = FrecencySettings::decode(&blob()).unwrap();
    let (mock, rest, _rx) = serve(&[settings_body(&[], false), settings_body(&blob(), true)]).await;
    let outcome = rest
        .frecency_patch(|_| Some(FrecencyPatch::default()))
        .await
        .unwrap();
    assert_eq!(outcome, PatchOutcome::OutOfDate(newer.clone()));
    assert_eq!(mock.count(), 2);
    let (mock, rest, _rx) = serve(&[
        settings_body(&[], false),
        json_status(400, r#"{"code":50035}"#),
        settings_body(&blob(), false),
    ])
    .await;
    let outcome = rest
        .frecency_patch(|_| Some(FrecencyPatch::default()))
        .await
        .unwrap();
    assert_eq!(outcome, PatchOutcome::OutOfDate(newer));
    let r = requests(&mock);
    assert_eq!(r.len(), 3);
    assert!(r[2].line.starts_with("GET "));
}

/// The guild a send or forward names comes from current state: a guild channel's guild, none
/// for a DM or an unknown channel (§18.4: `guild_id` omitted in DMs).
#[test]
fn guild_of_reads_the_channels_guild_from_state() {
    let mut s = State::default();
    assert!(s.apply(
        "READY",
        &serde_json::json!({
            "user": {"id": "1", "username": "me"},
            "guilds": [{"id": "100", "name": "g", "channels": [{"id": "10", "type": 0, "name": "general"}]}],
            "private_channels": [{"id": "500", "type": 1, "recipient_ids": ["2"]}],
        })
    ));
    assert_eq!(crate::rest_actions::guild_of(&s, 10), Some(100));
    assert_eq!(crate::rest_actions::guild_of(&s, 500), None);
    assert_eq!(crate::rest_actions::guild_of(&s, 999), None);
}
