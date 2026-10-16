//! REST tests on the in-memory transport (see `rest::test_support`). Each named property fails if
//! its clause in rest.rs is removed. Session-level 401 tests live in session_tests.rs. What the
//! app's URLSession transport puts on the wire (redirects, cookies, proxy, TLS) is tested in
//! `Tests/VelellaTransportTests`.
use crate::{
    identify,
    rest::{
        test_support::{response, MockHttp, HANG},
        validated, Rest, RestError, TokenSlot,
    },
    transport::{
        BoxFuture, HttpRequest, HttpResponse, Method, Transport, TransportError, WebSocket,
        WebSocketRequest,
    },
    Token,
};
use base64::Engine as _;
use serde_json::{json, Value};
use std::{sync::Arc, time::Duration};
use tokio::{sync::mpsc, time::timeout};

const MARKER: &str = "SYNTHETIC_TOKEN_MARKER_0123456789";

/// A `Rest` aimed at `mock`, and the receiving end of its 401 notice.
fn rest_for(mock: &MockHttp) -> (Rest, mpsc::UnboundedReceiver<()>) {
    mock.install();
    let (tx, rx) = mpsc::unbounded_channel();
    let rest = Rest::test(Arc::new(Token::new(MARKER.into()).unwrap()), 7, tx);
    (rest, rx)
}

async fn serve(responses: &[String]) -> (MockHttp, Rest, mpsc::UnboundedReceiver<()>) {
    let mock = MockHttp::start(responses.to_vec()).await;
    let (rest, rx) = rest_for(&mock);
    (mock, rest, rx)
}

fn ok(body: &str) -> String {
    response(200, "", body)
}

fn too_many(body: &str) -> String {
    response(429, "Content-Type: application/json\r\n", body)
}

async fn get(rest: &Rest) -> Result<Value, RestError> {
    rest.get("/users/@me").await
}

#[tokio::test]
async fn url_validator_rejects_everything_but_discord_api_v9_before_any_request() {
    let (mock, rest, _rx) = serve(&[ok("{}")]).await;
    assert!(validated("https://discord.com/api/v9/channels/1/messages?limit=50").is_some());
    assert!(validated("https://discord.com:443/api/v9/users/@me").is_some());
    for bad in [
        "http://discord.com/api/v9/users/@me",
        "https://evil.test/api/v9/users/@me",
        "https://discord.com.evil/api/v9/users/@me",
        "https://discord.com.evil.test/api/v9/users/@me",
        "https://cdn.discordapp.com/api/v9/users/@me",
        "https://discord.com:8443/api/v9/users/@me",
        "https://user@discord.com/api/v9/users/@me",
        "https://user:pw@discord.com/api/v9/users/@me",
        // Password without a username: only the password clause rejects it.
        "https://:pw@discord.com/api/v9/users/@me",
        "https://discord.com@evil.test/api/v9/users/@me",
        "https://discord.com/api/v10/users/@me",
        "https://discord.com/api/v9",
        "https://discord.com/oauth2/authorize",
        "https://discord.com/api/v9/../oauth2/authorize",
        "https://discord.com/api/v9/%2e%2e/oauth2",
        "https://discord.com/api/v9/users/@me#frag",
        "discord.com/api/v9/users/@me",
        "",
    ] {
        assert!(validated(bad).is_none(), "{bad}");
        let sent: Result<Value, _> = rest.request(Method::Get, bad, None).await;
        assert_eq!(sent.unwrap_err(), RestError::InvalidUrl, "{bad}");
    }
    // The typed path cannot climb out of the base either.
    let sent: Result<Value, _> = rest.get("/../../oauth2/authorize").await;
    assert_eq!(sent.unwrap_err(), RestError::InvalidUrl);
    assert_eq!(mock.count(), 0, "a rejected URL reached the server");
}

/// S-1: a typed path the URL parser would normalise into another path (still under `/api/v9/`)
/// is refused, and nothing is sent.
#[tokio::test]
async fn a_typed_path_that_normalises_into_another_is_refused() {
    let (mock, rest, _rx) = serve(&[ok("{}")]).await;
    for path in [
        "/channels/1/messages/2/reactions/../../../../users/@me",
        "/channels/1/./messages",
        "/channels/1/%2e%2e/messages",
        "/channels/1/%2E/messages",
        "/channels/1/messages\\..\\x",
    ] {
        let sent: Result<Value, _> = rest.get(path).await;
        assert_eq!(sent.unwrap_err(), RestError::InvalidUrl, "{path}");
        let sent: Result<Value, _> = rest.post(path, &json!({})).await;
        assert_eq!(sent.unwrap_err(), RestError::InvalidUrl, "{path}");
    }
    assert_eq!(mock.count(), 0, "a normalised path reached the server");
    // The paths in use today pass unchanged.
    let _: Value = rest
        .get("/channels/1/messages?limit=50&before=2")
        .await
        .unwrap();
    let _: Value = rest.get("/users/@me").await.unwrap();
    assert_eq!(mock.count(), 2);
}

/// The transport hands a 3xx back as it is (the app's transport refuses to follow, see the Swift
/// wire tests); core treats it as a failure and sends nothing more.
#[tokio::test]
async fn a_redirect_is_a_failure_and_nothing_follows_it() {
    let location = "Location: https://discord.com/api/v9/users/@me/elsewhere\r\n";
    let (mock, rest, _rx) = serve(&[response(302, location, ""), ok("{}")]).await;
    let err = get(&rest).await.unwrap_err();
    assert_eq!(
        (err, err.to_string().as_str()),
        (RestError::Failed, "request failed")
    );
    assert_eq!(mock.count(), 1);
}

#[tokio::test]
async fn a_body_over_4_mib_is_a_fixed_error() {
    let cap = 4 << 20;
    let at_cap = format!("\"{}\"", "x".repeat(cap - 2));
    let over = format!("\"{}\"", "x".repeat(cap - 1));
    let (mock, rest, _rx) = serve(&[ok(&at_cap), ok(&over)]).await;
    assert_eq!(
        get(&rest).await.unwrap().as_str().map(str::len),
        Some(cap - 2)
    );
    let err = get(&rest).await.unwrap_err();
    assert_eq!(
        (err, err.to_string().as_str()),
        (RestError::TooLarge, "response too large")
    );
    assert_eq!(mock.count(), 2);
}

/// A transport that ignores the cap and returns a fixed status with an over-cap body.
struct Oversized(u16);

impl Transport for Oversized {
    fn http(&self, _: HttpRequest) -> BoxFuture<'_, Result<HttpResponse, TransportError>> {
        let status = self.0;
        Box::pin(async move {
            Ok(HttpResponse {
                status,
                headers: vec![],
                body: Some(br#"{"retry_after": 0.01, "captcha_key": ["x"]}"#.repeat(200_000)),
            })
        })
    }
    fn websocket(
        &self,
        _: WebSocketRequest,
    ) -> BoxFuture<'_, Result<Box<dyn WebSocket>, TransportError>> {
        Box::pin(async { Err(TransportError::Other) })
    }
}

/// Core re-checks the cap whatever the transport does; an over-cap error body is unreadable,
/// exactly as before (429 → RateLimited, never retried; other → Failed, no captcha read).
#[tokio::test]
async fn core_rechecks_the_body_cap_whatever_the_transport_returns() {
    for (status, want) in [
        (200, RestError::TooLarge),
        (429, RestError::RateLimited),
        (400, RestError::Failed),
    ] {
        let (tx, _rx) = mpsc::unbounded_channel();
        let rest = Rest::new(
            TokenSlot::new(Arc::new(Token::new(MARKER.into()).unwrap())),
            7,
            tx,
            Arc::new(Oversized(status)),
            Arc::new(identify::Identity::current()),
        );
        assert_eq!(get(&rest).await.unwrap_err(), want, "{status}");
    }
}

/// Core's deadline: a request the transport never answers fails with the fixed network error
/// once 30 s pass, and the dropped request future is what cancels it in the transport.
#[tokio::test(start_paused = true)]
async fn core_deadline_fails_a_silent_request_and_cancels_it() {
    let (mock, rest, _rx) = serve(&[HANG.to_owned()]).await;
    let cancelled = mock.cancelled();
    let started = tokio::time::Instant::now();
    assert_eq!(get(&rest).await.unwrap_err(), RestError::Network);
    let waited = started.elapsed();
    assert!(waited >= Duration::from_secs(30) && waited < Duration::from_secs(31));
    timeout(Duration::from_secs(1), cancelled)
        .await
        .expect("the request was not cancelled");
    assert_eq!(mock.count(), 1);
}

/// Every transport failure is the same fixed network error; none is retried by `Rest` (D11's
/// connect-failed retry for sends belongs to F2).
#[tokio::test]
async fn transport_failures_map_to_network_and_are_not_retried() {
    for kind in ["connect", "timeout", "other"] {
        let (mock, rest, _rx) = serve(&[format!("ERROR {kind}")]).await;
        assert_eq!(get(&rest).await.unwrap_err(), RestError::Network, "{kind}");
        assert_eq!(mock.count(), 1, "{kind}: GET retried");
        let sent: Result<Value, _> = rest
            .post("/channels/1/messages", &json!({"content": "hi"}))
            .await;
        assert_eq!(sent.unwrap_err(), RestError::Network, "{kind}");
        assert_eq!(mock.count(), 2, "{kind}: POST retried");
    }
}

#[tokio::test]
async fn a_short_429_then_200_is_retried_once_and_succeeds() {
    let (mock, rest, _rx) = serve(&[
        too_many(r#"{"retry_after": 0.05, "global": false}"#),
        ok(r#"{"id": "1"}"#),
    ])
    .await;
    assert_eq!(get(&rest).await.unwrap(), json!({"id": "1"}));
    assert_eq!(mock.count(), 2);
}

#[tokio::test]
async fn two_429s_give_up_after_two_requests() {
    let (mock, rest, _rx) = serve(&[too_many(r#"{"retry_after": 0.05, "global": false}"#)]).await;
    let err = get(&rest).await.unwrap_err();
    assert_eq!(
        (err, err.to_string().as_str()),
        (RestError::RateLimited, "rate limited")
    );
    assert_eq!(mock.count(), 2);
}

#[tokio::test]
async fn a_long_or_unreadable_429_is_not_retried() {
    let (mock, rest, _rx) = serve(&[too_many(r#"{"retry_after": 10.5, "global": false}"#)]).await;
    assert_eq!(get(&rest).await.unwrap_err(), RestError::RateLimited);
    assert_eq!(mock.count(), 1);
    let (mock, rest, _rx) = serve(&[response(429, "", "slow down"), ok("{}")]).await;
    assert_eq!(get(&rest).await.unwrap_err(), RestError::RateLimited);
    assert_eq!(mock.count(), 1);
}

#[tokio::test]
async fn post_is_never_retried() {
    let (mock, rest, _rx) = serve(&[response(500, "", ""), ok("{}")]).await;
    let sent: Result<Value, _> = rest
        .post("/channels/1/messages", &json!({"content": "hi"}))
        .await;
    assert_eq!(sent.unwrap_err(), RestError::Failed);
    assert_eq!(mock.count(), 1);
    let (mock, rest, _rx) = serve(&[
        too_many(r#"{"retry_after": 0.05, "global": false}"#),
        ok("{}"),
    ])
    .await;
    let sent: Result<Value, _> = rest
        .post("/channels/1/messages", &json!({"content": "hi"}))
        .await;
    assert_eq!(sent.unwrap_err(), RestError::RateLimited);
    assert_eq!(mock.count(), 1);
}

#[tokio::test]
async fn a_global_429_blocks_every_call_until_its_deadline() {
    let (mock, rest, _rx) =
        serve(&[too_many(r#"{"retry_after": 60, "global": true}"#), ok("{}")]).await;
    assert_eq!(get(&rest).await.unwrap_err(), RestError::RateLimited);
    // Another clone of the same session client is blocked too, without sending anything.
    let other = rest.clone();
    assert_eq!(get(&other).await.unwrap_err(), RestError::RateLimited);
    assert_eq!(mock.count(), 1);
}

#[tokio::test]
async fn captcha_and_403_are_never_retried_and_403_does_not_latch() {
    let captcha = r#"{"captcha_key": ["captcha-required"], "captcha_sitekey": "x", "captcha_service": "hcaptcha"}"#;
    let (mock, rest, mut rx) = serve(&[
        response(400, "Content-Type: application/json\r\n", captcha),
        response(
            403,
            "Content-Type: application/json\r\n",
            r#"{"message": "Missing Access", "code": 50001}"#,
        ),
        ok("{}"),
    ])
    .await;
    let err = get(&rest).await.unwrap_err();
    assert_eq!(
        (err, err.to_string().as_str()),
        (RestError::Captcha, "Discord asked for a captcha")
    );
    assert_eq!(mock.count(), 1);
    let err = get(&rest).await.unwrap_err();
    assert_eq!(
        (err, err.to_string().as_str()),
        (RestError::Forbidden, "no access")
    );
    assert_eq!(mock.count(), 2);
    assert_eq!(
        get(&rest).await.unwrap(),
        json!({}),
        "a 403 latched REST off"
    );
    assert_eq!(mock.count(), 3);
    assert!(rx.try_recv().is_err(), "a 403 sent the 401 notice");
}

#[tokio::test]
async fn the_token_appears_only_in_the_authorization_header() {
    let (mock, rest, _rx) = serve(&[ok("{}"), response(401, "", "")]).await;
    get(&rest).await.unwrap();
    // The 401 error and the latched state are formatted too.
    let unauthorized = get(&rest).await.unwrap_err();
    let url = validated("https://discord.com/api/v9/users/@me").unwrap();
    let built = rest
        .build_request(Method::Post, &url, Some(b"{}".to_vec()))
        .unwrap();
    let errors = [
        RestError::Unauthorized,
        RestError::RateLimited,
        RestError::Forbidden,
        RestError::Captcha,
        RestError::Failed,
        RestError::Network,
        RestError::TooLarge,
        RestError::BadResponse,
        RestError::InvalidUrl,
        unauthorized,
    ];
    let mut texts = vec![
        format!("{rest:?}"),
        format!("{rest:#?}"),
        format!("{built:?}"),
        format!("{built:#?}"),
    ];
    texts.extend(errors.iter().map(|e| format!("{e} {e:?}")));
    for text in &texts {
        assert!(!text.contains("SYNTHETIC"), "{text}");
    }
    // The guard is not vacuous: the built request does carry the token, in its own field and in
    // no header.
    assert_eq!(built.authorization().unwrap().reveal(), MARKER);
    assert!(built
        .headers()
        .iter()
        .all(|(n, v)| !v.contains(MARKER) && !n.eq_ignore_ascii_case("authorization")));

    let raw = &mock.requests()[0];
    assert_eq!(raw.matches(MARKER).count(), 1, "{raw}");
    let headers: Vec<(String, String)> = raw
        .split("\r\n\r\n")
        .next()
        .unwrap()
        .lines()
        .skip(1)
        .filter_map(|l| {
            let (k, v) = l.split_once(':')?;
            Some((k.trim().to_ascii_lowercase(), v.trim().to_owned()))
        })
        .collect();
    let header = |name: &str| {
        headers
            .iter()
            .find(|(k, _)| k == name)
            .map(|(_, v)| v.as_str())
    };
    assert_eq!(header("authorization"), Some(MARKER));
    assert_eq!(header("user-agent"), Some(identify::USER_AGENT));
    assert_eq!(header("x-discord-locale"), Some("en-US"));
    assert_eq!(header("accept-language"), Some("en-US,en;q=0.9"));
    assert_eq!(
        header("x-discord-timezone"),
        Some(identify::timezone().as_str())
    );
    let props = base64::engine::general_purpose::STANDARD
        .decode(header("x-super-properties").unwrap())
        .unwrap();
    let props: Value = serde_json::from_slice(&props).unwrap();
    assert_eq!(props["browser"], "Safari");
    assert_eq!(props["browser_user_agent"], identify::USER_AGENT);
    for (name, _) in &headers {
        assert!(!name.starts_with("sec-ch-ua"), "{name}");
        assert_ne!(name, "accept-encoding");
    }
}

/// The seam slice ID uses: after `replace_token`, every request carries the new token.
#[tokio::test]
async fn a_replaced_token_is_used_by_later_requests() {
    let (mock, rest, _rx) = serve(&[ok("{}")]).await;
    get(&rest).await.unwrap();
    let fresh = "SYNTHETIC_FRESH_TOKEN_9876543210";
    rest.clone()
        .replace_token(Arc::new(Token::new(fresh.into()).unwrap()));
    get(&rest).await.unwrap();
    let requests = mock.requests();
    assert!(requests[0].contains(&format!("authorization: {MARKER}\r\n")));
    assert!(requests[1].contains(&format!("authorization: {fresh}\r\n")));
    assert!(!requests[1].contains(MARKER));
}

/// The decoded X-Super-Properties JSON text of one recorded request.
fn raw_props(headers: &[(String, String)]) -> String {
    let raw = &headers
        .iter()
        .find(|(k, _)| k == "x-super-properties")
        .unwrap()
        .1;
    String::from_utf8(
        base64::engine::general_purpose::STANDARD
            .decode(raw)
            .unwrap(),
    )
    .unwrap()
}

/// The keys of a JSON object's top level, in the order written (serde_json's `Value` sorts).
fn top_level_keys(json: &str) -> Vec<String> {
    let (mut keys, mut depth, mut current, mut in_string, mut escaped) =
        (Vec::new(), 0, String::new(), false, false);
    let mut last_string = None;
    for c in json.chars() {
        if in_string {
            match (escaped, c) {
                (true, _) => escaped = false,
                (false, '\\') => escaped = true,
                (false, '"') => {
                    in_string = false;
                    last_string = Some(std::mem::take(&mut current));
                    continue;
                }
                _ => {}
            }
            current.push(c);
            continue;
        }
        match c {
            '"' => in_string = true,
            '{' | '[' => depth += 1,
            '}' | ']' => depth -= 1,
            ':' if depth == 1 => keys.extend(last_string.take()),
            _ => {}
        }
        if c != ':' && !c.is_whitespace() && c != '"' {
            last_string = None;
        }
    }
    keys
}

/// The identity's closed set of extras is read when each request is built: unset, nothing
/// changes; set, the super-properties carry them and `X-Installation-ID` appears.
#[tokio::test]
async fn identity_extras_reach_the_headers_at_send_time() {
    let headers = |raw: &str| -> Vec<(String, String)> {
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
    };
    let props = |h: &[(String, String)]| -> Value {
        let raw = &h.iter().find(|(k, _)| k == "x-super-properties").unwrap().1;
        serde_json::from_slice(
            &base64::engine::general_purpose::STANDARD
                .decode(raw)
                .unwrap(),
        )
        .unwrap()
    };
    let (mock, rest, _rx) = serve(&[ok("{}")]).await;
    get(&rest).await.unwrap();
    let identity = rest.identity();
    identity.set_client_app_state(identify::AppState::Focused);
    assert!(identity.set_client_heartbeat_session_id("hb-1".into()));
    assert!(identity.set_installation_id("inst.1".into()));
    assert!(!identity.set_installation_id("bad\r\nX-Evil: 1".into()));
    assert!(!identity.set_installation_id(String::new()));
    get(&rest).await.unwrap();
    let requests = mock.requests();
    let (before, after) = (headers(&requests[0]), headers(&requests[1]));
    assert!(before.iter().all(|(k, _)| k != "x-installation-id"));
    let base = props(&before);
    assert!(base.get("client_app_state").is_none());
    assert_eq!(
        after
            .iter()
            .find(|(k, _)| k == "x-installation-id")
            .map(|(_, v)| v.as_str()),
        Some("inst.1")
    );
    // Exact key order on the wire: the base keys as IDENTIFY sends them, extras appended.
    let base_keys = [
        "os",
        "browser",
        "device",
        "system_locale",
        "browser_user_agent",
        "browser_version",
        "os_version",
        "referrer",
        "referring_domain",
        "referrer_current",
        "referring_domain_current",
        "release_channel",
        "client_build_number",
        "client_event_source",
    ];
    assert_eq!(top_level_keys(&raw_props(&before)), base_keys);
    let mut with_extras = base_keys.to_vec();
    with_extras.extend(["client_app_state", "client_heartbeat_session_id"]);
    assert_eq!(top_level_keys(&raw_props(&after)), with_extras);
    let mut extended = props(&after);
    assert_eq!(extended["client_app_state"], "focused");
    assert_eq!(extended["client_heartbeat_session_id"], "hb-1");
    let map = extended.as_object_mut().unwrap();
    map.remove("client_app_state");
    map.remove("client_heartbeat_session_id");
    assert_eq!(extended, base, "the base properties changed");
}

#[tokio::test]
async fn a_401_latches_rest_off_and_sends_one_notice() {
    let (mock, rest, mut rx) = serve(&[response(401, "", ""), ok("{}")]).await;
    let err = get(&rest).await.unwrap_err();
    assert_eq!(err, RestError::Unauthorized);
    assert_eq!(err.to_string(), "Discord did not accept this sign-in");
    assert_eq!(
        get(&rest.clone()).await.unwrap_err(),
        RestError::Unauthorized
    );
    assert_eq!(mock.count(), 1, "a latched client sent a request");
    assert_eq!(rx.try_recv(), Ok(()));
    assert!(rx.try_recv().is_err(), "more than one notice");
}
