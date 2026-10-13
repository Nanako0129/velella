//! REST tests against a local mock (see `rest::test_support`). Each named property fails if its
//! clause in rest.rs is removed. Session-level 401 tests live in session_tests.rs.
use crate::{
    identify,
    rest::{
        test_support::{response, MockHttp, TEST_REST_BASE},
        validated, Rest, RestError,
    },
    Token,
};
use base64::Engine as _;
use reqwest::Method;
use serde_json::{json, Value};
use std::{sync::Arc, time::Duration};
use tokio::{io::AsyncReadExt, net::TcpListener, sync::mpsc, time::timeout};

const MARKER: &str = "SYNTHETIC_TOKEN_MARKER_0123456789";

/// A `Rest` aimed at `mock`, and the receiving end of its 401 notice.
fn rest_for(mock: &MockHttp) -> (Rest, mpsc::UnboundedReceiver<()>) {
    mock.install();
    let (tx, rx) = mpsc::unbounded_channel();
    let rest = Rest::new(Arc::new(Token::new(MARKER.into()).unwrap()), 7, tx);
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

async fn untouched(listener: &TcpListener) -> bool {
    timeout(Duration::from_millis(300), listener.accept())
        .await
        .is_err()
}

/// Both rustls users (gateway, REST) build configs from the process-wide provider choice. With two
/// providers compiled in and none installed, this panics; with exactly one (ring), it does not.
#[test]
fn rustls_client_config_builds_under_the_unified_features() {
    let _ = rustls::ClientConfig::builder();
}

#[tokio::test]
async fn shipped_client_sends_a_tls_client_hello_and_fails_with_fixed_text() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let peer = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.unwrap();
        let mut first = [0u8; 1];
        stream.read_exact(&mut first).await.unwrap();
        first[0] // then drop: the handshake can never finish
    });
    let (tx, _rx) = mpsc::unbounded_channel();
    TEST_REST_BASE.with(|b| *b.borrow_mut() = None); // the shipped configuration
    let rest = Rest::new(Arc::new(Token::new(MARKER.into()).unwrap()), 7, tx);
    let url = format!("https://127.0.0.1:{port}/api/v9/users/@me");
    // `request` would refuse this host; send through the same client and error mapping instead.
    let err = rest.send_unvalidated(Method::GET, &url).await.unwrap_err();
    assert_eq!(
        peer.await.unwrap(),
        0x16,
        "first byte is not a TLS handshake record"
    );
    assert_eq!(err, RestError::Network);
    assert_eq!(err.to_string(), "network error");
}

#[tokio::test]
async fn shipped_client_refuses_plain_http() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let (tx, _rx) = mpsc::unbounded_channel();
    TEST_REST_BASE.with(|b| *b.borrow_mut() = None);
    let rest = Rest::new(Arc::new(Token::new(MARKER.into()).unwrap()), 7, tx);
    let url = format!("http://127.0.0.1:{port}/api/v9/users/@me");
    assert_eq!(
        rest.send_unvalidated(Method::GET, &url).await.unwrap_err(),
        RestError::Network
    );
    assert!(
        untouched(&listener).await,
        "https_only let a plain-http request out"
    );
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
        let sent: Result<Value, _> = rest.request(Method::GET, bad, None).await;
        assert_eq!(sent.unwrap_err(), RestError::InvalidUrl, "{bad}");
    }
    // The typed path cannot climb out of the base either.
    let sent: Result<Value, _> = rest.get("/../../oauth2/authorize").await;
    assert_eq!(sent.unwrap_err(), RestError::InvalidUrl);
    assert_eq!(mock.count(), 0, "a rejected URL reached the server");
}

#[tokio::test]
async fn a_redirect_is_not_followed() {
    let second = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let location = format!(
        "Location: http://{}/api/v9/users/@me\r\n",
        second.local_addr().unwrap()
    );
    let (mock, rest, _rx) = serve(&[response(302, &location, "")]).await;
    let err = get(&rest).await.unwrap_err();
    assert_eq!(
        (err, err.to_string().as_str()),
        (RestError::Failed, "request failed")
    );
    assert_eq!(mock.count(), 1);
    assert!(
        untouched(&second).await,
        "the redirect target was contacted"
    );
}

#[tokio::test]
async fn proxy_environment_is_never_used() {
    let proxy = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let proxy_url = format!("http://{}", proxy.local_addr().unwrap());
    let vars = [
        "HTTPS_PROXY",
        "https_proxy",
        "ALL_PROXY",
        "all_proxy",
        "HTTP_PROXY",
        "http_proxy",
    ];
    for var in vars {
        std::env::set_var(var, &proxy_url);
    }
    std::env::remove_var("NO_PROXY");
    std::env::remove_var("no_proxy");
    // The client reads proxy variables when it is built, i.e. in `Rest::new`.
    let (mock, rest, _rx) = serve(&[ok("{}")]).await;
    for var in vars {
        std::env::remove_var(var);
    }
    assert_eq!(get(&rest).await.unwrap(), json!({}));
    assert_eq!(mock.count(), 1);
    assert!(
        untouched(&proxy).await,
        "the proxy from the environment was contacted"
    );
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
    let client = reqwest::Client::new();
    let url = validated("https://discord.com/api/v9/users/@me").unwrap();
    let built = rest
        .build_request(&client, Method::POST, url, Some(b"{}".to_vec()))
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
    ];
    texts.extend(errors.iter().map(|e| format!("{e} {e:?}")));
    for text in &texts {
        assert!(!text.contains("SYNTHETIC"), "{text}");
    }
    // The guard is not vacuous: the built request does carry the token, as a sensitive header.
    let auth = built.headers().get(reqwest::header::AUTHORIZATION).unwrap();
    assert!(auth.is_sensitive() && auth.as_bytes() == MARKER.as_bytes());

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
