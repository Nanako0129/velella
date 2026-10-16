//! Wave 3 track R: reply sending and the typing indicator. Included from messages_tests.rs, so
//! it shares that file's helpers. Typing time comes from the store's injected clock; no test
//! sleeps or waits for the real ten seconds.
use super::*;
use crate::state::State;
use std::time::Duration;

/// A store whose clock is `*clock`, which the test moves by hand.
fn store_with_clock(mock: &MockHttp, clock: Arc<Mutex<Instant>>) -> Arc<MessageStore> {
    mock.install();
    let (tx, _rx) = mpsc::unbounded_channel();
    Arc::new(MessageStore {
        clock: Box::new(move || *clock.lock().unwrap()),
        ..MessageStore::build(Rest::test(token(), 7, tx), None)
    })
}

/// Open channel 5 (history already settled) on a store with a hand-moved clock.
async fn typing_rig(history: &Value) -> (MockHttp, Arc<MessageStore>, u64, Arc<Mutex<Instant>>) {
    let mock = MockHttp::start(vec![ok(history)]).await;
    let clock = Arc::new(Mutex::new(Instant::now()));
    let store = store_with_clock(&mock, clock.clone());
    let epoch = store.open(5);
    settled(&store, epoch).await;
    (mock, store, epoch, clock)
}

fn advance(clock: &Mutex<Instant>, ms: u64) {
    *clock.lock().unwrap() += Duration::from_millis(ms);
}

/// The self user is 1; user 9 is known to the state as "Nine".
fn state() -> State {
    let mut s = State {
        user_id: Some(1),
        ..State::default()
    };
    s.users.insert(1, me());
    s.users.insert(
        9,
        User {
            id: 9,
            username: "user9".into(),
            global_name: Some("Nine".into()),
            avatar: None,
            discriminator: None,
        },
    );
    s
}

fn typing_event(channel: u64, user: u64) -> Value {
    json!({"channel_id": channel.to_string(), "user_id": user.to_string(), "timestamp": 1})
}

fn typing_of(store: &MessageStore, epoch: u64) -> Vec<String> {
    now(store, epoch).typing
}

// ---- Reply ----

#[tokio::test]
async fn a_reply_posts_message_reference_and_leaves_mentions_at_the_default() {
    let (mock, store) = serve(&[ok(&json!([msg(30, 5, "question")])), ok(&json!({}))]).await;
    let epoch = store.open(5);
    settled(&store, epoch).await;
    let (_, nonce) = store.begin_send(5, "answer", me(), Some(30)).unwrap();
    let _ = store
        .rest
        .create_message(5, "answer", &nonce, Some(30))
        .await;
    let raw = &mock.requests()[1];
    assert!(raw.starts_with("POST /api/v9/channels/5/messages HTTP/1.1\r\n"));
    let body: Value = serde_json::from_str(raw.split("\r\n\r\n").nth(1).unwrap()).unwrap();
    assert_eq!(
        body,
        json!({
            "mobile_network_type": "unknown", "content": "answer", "nonce": nonce, "tts": false,
            "flags": 0, "message_reference": {"channel_id": "5", "message_id": "30"},
        })
    );
    assert!(body.get("allowed_mentions").is_none());
}

#[tokio::test]
async fn a_send_without_a_reply_has_no_message_reference() {
    let (mock, store) = serve(&[ok(&json!([])), ok(&json!({}))]).await;
    let epoch = store.open(5);
    settled(&store, epoch).await;
    let (_, nonce) = store.begin_send(5, "hi", me(), None).unwrap();
    let _ = store.rest.create_message(5, "hi", &nonce, None).await;
    let raw = &mock.requests()[1];
    let body: Value = serde_json::from_str(raw.split("\r\n\r\n").nth(1).unwrap()).unwrap();
    assert!(body.get("message_reference").is_none(), "{raw}");
    assert!(!raw.contains("message_reference"));
}

#[tokio::test]
async fn the_echo_of_a_reply_carries_the_preview_of_the_replied_message() {
    let mut question = msg(30, 5, "what **now**?");
    // A reply of the replied message is not copied into the preview.
    question["referenced_message"] = msg(20, 5, "older");
    let (_mock, store) = serve(&[ok(&json!([question, msg(20, 5, "older")]))]).await;
    let epoch = store.open(5);
    settled(&store, epoch).await;
    store.begin_send(5, "answer", me(), Some(30)).unwrap();
    let s = now(&store, epoch);
    let echo = s.rows.last().unwrap();
    assert!(echo.pending);
    assert_eq!(
        echo.reply,
        Some(MessageReply {
            author_name: "Nine".into(),
            content: "what now?".into()
        })
    );
    // The replied message itself keeps its own reply line.
    assert_eq!(s.rows[1].reply.as_ref().unwrap().content, "older");
    let inner = store.lock();
    let stored = &inner.open.as_ref().unwrap().echoes[0].message;
    assert!(stored.reply.as_ref().unwrap().reply.is_none());
}

#[tokio::test]
async fn a_reply_to_an_unbuffered_message_still_sends_but_shows_no_preview() {
    let (_mock, store) = serve(&[ok(&json!([]))]).await;
    let epoch = store.open(5);
    settled(&store, epoch).await;
    store.begin_send(5, "answer", me(), Some(99)).unwrap();
    assert_eq!(now(&store, epoch).rows[0].reply, None);
}

// ---- Typing ----

#[tokio::test]
async fn typing_adds_names_in_arrival_order_with_the_message_author_name_rules() {
    let (_mock, store, epoch, _clock) = typing_rig(&json!([])).await;
    let mut state = state();
    for (id, username, global) in [(10, "u10", Some("Ten")), (11, "u11", None)] {
        state.users.insert(
            id,
            User {
                id,
                username: username.into(),
                global_name: global.map(Into::into),
                avatar: None,
                discriminator: None,
            },
        );
    }
    // Member nick first (the payload's own member), then global name, then username.
    let mut nick = typing_event(5, 10);
    nick["member"] = json!({"user": {"id": "10", "username": "u10"}, "nick": "Tenner"});
    store.typing_start(&nick, &state);
    store.typing_start(&typing_event(5, 9), &state);
    store.typing_start(&typing_event(5, 11), &state);
    assert_eq!(typing_of(&store, epoch), ["Tenner", "Nine", "u11"]);

    // Without a state entry the payload's member names the user; without either, nobody.
    let mut member_only = typing_event(5, 12);
    member_only["member"] =
        json!({"user": {"id": "12", "username": "u12", "global_name": "Twelve"}});
    store.typing_start(&member_only, &State::default());
    store.typing_start(&typing_event(5, 13), &State::default());
    assert_eq!(
        typing_of(&store, epoch),
        ["Tenner", "Nine", "u11", "Twelve"]
    );
}

#[tokio::test]
async fn typing_refreshes_and_expires_ten_seconds_after_the_last_event() {
    let (_mock, store, epoch, clock) = typing_rig(&json!([])).await;
    let state = state();
    store.typing_start(&typing_event(5, 9), &state);
    advance(&clock, 8_000);
    store.typing_start(&typing_event(5, 9), &state);
    // 17.999 s after the first event, 9.999 s after the last.
    advance(&clock, 9_999);
    assert_eq!(typing_of(&store, epoch), ["Nine"]);
    advance(&clock, 1);
    assert!(typing_of(&store, epoch).is_empty());
}

#[tokio::test]
async fn an_expiry_bumps_the_store_once_and_only_when_something_went() {
    let (_mock, store, epoch, clock) = typing_rig(&json!([])).await;
    store.typing_start(&typing_event(5, 9), &state());
    let revision = store.revision.subscribe();
    let bumps = || *revision.borrow();
    let before = bumps();
    advance(&clock, 9_999);
    store.expire_typing(epoch);
    assert_eq!(bumps(), before, "nothing had lapsed");
    advance(&clock, 1);
    store.expire_typing(epoch);
    assert_eq!(bumps(), before + 1);
    store.expire_typing(epoch);
    assert_eq!(bumps(), before + 1, "nothing left to drop");
    // A timer of a channel since left does nothing.
    store.expire_typing(epoch + 1);
    assert_eq!(bumps(), before + 1);
}

#[tokio::test]
async fn that_users_message_ends_their_typing_at_once() {
    let (_mock, store, epoch, _clock) = typing_rig(&json!([])).await;
    let state = state();
    store.typing_start(&typing_event(5, 9), &state);
    let mut other = typing_event(5, 11);
    other["member"] = json!({"user": {"id": "11", "username": "u11"}});
    store.typing_start(&other, &state);
    // msg() is authored by user 9.
    store.observe("MESSAGE_CREATE", &msg(50, 5, "hello"));
    assert_eq!(typing_of(&store, epoch), ["u11"]);
    // A message in another channel does not end it.
    store.observe("MESSAGE_CREATE", &msg(51, 6, "elsewhere"));
    store.typing_start(&typing_event(5, 9), &state);
    store.observe("MESSAGE_CREATE", &msg(52, 6, "elsewhere"));
    assert_eq!(typing_of(&store, epoch), ["u11", "Nine"]);
}

#[tokio::test]
async fn the_self_user_and_other_channels_are_ignored() {
    let (_mock, store, epoch, _clock) = typing_rig(&json!([])).await;
    let state = state();
    let revision = store.revision.subscribe();
    let before = *revision.borrow();
    store.typing_start(&typing_event(5, 1), &state);
    store.typing_start(&typing_event(6, 9), &state);
    assert!(typing_of(&store, epoch).is_empty());
    assert_eq!(*revision.borrow(), before, "an ignored event woke the UI");
    // Leaving drops entries: a new open starts empty.
    store.typing_start(&typing_event(5, 9), &state);
    store.close(epoch);
    let next = store.open(5);
    assert!(store
        .snapshot(next, &Context::default())
        .unwrap()
        .typing
        .is_empty());
}
