//! The identity fetch (menu plan ID, D12, D17; T-1, T-5, T-7, T-14, I-10, I-11): IDENTIFY's
//! `client_build_number` and `capabilities` read from the official web client at runtime.
//!
//! Outside `Rest`: exactly two requests on the session's transport, both built from a constant
//! base — `GET https://discord.com/app` and `GET https://discord.com/assets/fast-connect.<16 hex>.js`
//! (the hash taken from the page and the URL rebuilt, never used as written). No Authorization,
//! no X-Super-Properties, no X-Discord-* header; the transport keeps no cookies and refuses
//! redirects. Headers are Safari's for a page navigation and for a script load (identify.md
//! "Safari capture by main"); Host, Accept-Encoding and Connection are left to URLSession (its
//! Accept-Encoding adds `br` like Safari over HTTPS; unmeasured). Nothing fetched is executed or
//! kept: only typed values leave this file, and only after the cross-checks below.
use crate::identity_store::{build_ok, chunk_hash_ok, FetchCache};
use crate::transport::{HttpRequest, Method, Transport};
use std::time::Duration;

/// Built-in values (D12; I-O5): used when nothing better is accepted.
pub(crate) const BUILTIN_BUILD: u64 = 640_102;
/// (obfuscation variant, normal) — I§6's group order.
pub(crate) const BUILTIN_PAIR: (u64, u64) = (1_767_421, 1_734_653);
/// The capabilities pairs velella supports, newest last (D14).
pub(crate) const SUPPORTED_PAIRS: [(u64, u64); 1] = [BUILTIN_PAIR];
/// Accepted build numbers: from the built-in value to twice it (contract: [640102, 1280204]).
pub(crate) const BUILD_MIN: u64 = BUILTIN_BUILD;
pub(crate) const BUILD_MAX: u64 = 2 * BUILTIN_BUILD;

const APP_URL: &str = "https://discord.com/app";
const CHUNK_BASE: &str = "https://discord.com/assets/fast-connect.";
/// Decoded-size caps (contract; measured 76 KB and 33 KB).
const HTML_CAP: usize = 1 << 20;
const CHUNK_CAP: usize = 512 << 10;
/// The whole fetch (both requests) ends within this: the first IDENTIFY waits for it at most
/// this long (T-14), so a slow fetch is a failure, not a later surprise.
pub(crate) const FETCH_DEADLINE: Duration = Duration::from_secs(3);

/// What one fetch produced.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Fetched {
    /// Both values cross-checked. `cache` is the entry to store; `None` when the pair was not
    /// supported (then `pair` is the newest supported one and the launch shows the notice, and
    /// the next launch fetches again).
    Accepted {
        build: u64,
        pair: (u64, u64),
        unsupported: bool,
        cache: Option<FetchCache>,
    },
    /// Challenge page, non-200, timeout, over cap, no or duplicate match, mismatch, out of range.
    Failed,
}

/// Every occurrence of `prefix` + `<digits>` + `suffix`, as the digits (a regex `prefix(\d+)suffix`
/// with the length bounds given).
fn digits_between<'a>(
    text: &'a str,
    prefix: &'a str,
    suffix: &'a str,
    len: std::ops::RangeInclusive<usize>,
) -> impl Iterator<Item = &'a str> + 'a {
    text.match_indices(prefix).filter_map(move |(at, _)| {
        let rest = &text[at + prefix.len()..];
        let n = rest.bytes().take_while(u8::is_ascii_digit).count();
        (len.contains(&n) && rest[n..].starts_with(suffix)).then(|| &rest[..n])
    })
}

/// The single match of `iter`, parsed; `None` for zero, two or more, or an unparsable value.
fn single<'a>(mut iter: impl Iterator<Item = &'a str>) -> Option<u64> {
    let first = iter.next()?;
    iter.next().is_none().then_some(())?;
    first.parse().ok()
}

/// `"BUILD_NUMBER":"(\d{6,7})"` in the page's `GLOBAL_ENV`, exactly once (I-O5).
pub(crate) fn html_build(html: &str) -> Option<u64> {
    single(digits_between(html, "\"BUILD_NUMBER\":\"", "\"", 6..=7))
}

/// The one distinct `/assets/fast-connect.<16 hex>.js` the page names as a whole attribute value
/// (`src="…"` or `href="…"`): a reference on another host, with a dot segment or with a query
/// does not match, and two distinct hashes are refused.
pub(crate) fn fast_connect_hash(html: &str) -> Option<String> {
    const PREFIX: &str = "\"/assets/fast-connect.";
    let mut found: Option<&str> = None;
    for (at, _) in html.match_indices(PREFIX) {
        let rest = &html[at + PREFIX.len()..];
        let Some(hash) = rest.get(..16).filter(|h| chunk_hash_ok(h)) else {
            continue;
        };
        if !rest[16..].starts_with(".js\"") {
            continue;
        }
        match found {
            Some(seen) if seen != hash => return None,
            _ => found = Some(hash),
        }
    }
    found.map(str::to_owned)
}

/// `parseInt("(\d+)",10))||(\w+.client_build_number=` in the chunk, exactly once (I§6).
pub(crate) fn chunk_build(chunk: &str) -> Option<u64> {
    const PREFIX: &str = "parseInt(\"";
    const MID: &str = "\",10))||(";
    const FIELD: &str = ".client_build_number=";
    let matches = chunk.match_indices(PREFIX).filter_map(|(at, _)| {
        let rest = &chunk[at + PREFIX.len()..];
        let n = rest.bytes().take_while(u8::is_ascii_digit).count();
        let after = rest[n..].strip_prefix(MID)?;
        let ident = after
            .bytes()
            .take_while(|b| b.is_ascii_alphanumeric() || *b == b'_')
            .count();
        (n > 0 && ident > 0 && after[ident..].starts_with(FIELD)).then(|| &rest[..n])
    });
    single(matches)
}

/// I§6's capabilities pattern, exactly once: (obfuscation variant, normal).
pub(crate) fn chunk_capabilities(chunk: &str) -> Option<(u64, u64)> {
    const PREFIX: &str = "capabilities:function(e){let{useChannelObfuscation:t}=e;return t?";
    let mut matches = chunk.match_indices(PREFIX).filter_map(|(at, _)| {
        let rest = &chunk[at + PREFIX.len()..];
        let a = rest.bytes().take_while(u8::is_ascii_digit).count();
        let rest2 = rest[a..].strip_prefix(':')?;
        let b = rest2.bytes().take_while(u8::is_ascii_digit).count();
        (a > 0 && b > 0 && rest2[b..].starts_with('}')).then(|| (&rest[..a], &rest2[..b]))
    });
    let (a, b) = matches.next()?;
    matches.next().is_none().then_some(())?;
    Some((a.parse().ok()?, b.parse().ok()?))
}

/// Safari's page-navigation headers (identify.md "Safari capture by main"), in its order.
fn navigation_headers(user_agent: &str, accept_language: &str) -> Vec<(String, String)> {
    [
        ("Sec-Fetch-Dest", "document"),
        ("User-Agent", user_agent),
        ("Upgrade-Insecure-Requests", "1"),
        (
            "Accept",
            "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8",
        ),
        ("Sec-Fetch-Site", "none"),
        ("Sec-Fetch-Mode", "navigate"),
        ("Accept-Language", accept_language),
        ("Priority", "u=0, i"),
    ]
    .into_iter()
    .map(|(k, v)| (k.to_owned(), v.to_owned()))
    .collect()
}

/// Safari's script-load headers from the page's own origin.
fn script_headers(user_agent: &str, accept_language: &str) -> Vec<(String, String)> {
    [
        ("Sec-Fetch-Dest", "script"),
        ("User-Agent", user_agent),
        ("Accept", "*/*"),
        ("Referer", APP_URL),
        ("Sec-Fetch-Site", "same-origin"),
        ("Sec-Fetch-Mode", "no-cors"),
        ("Accept-Language", accept_language),
        ("Priority", "u=1, i"),
    ]
    .into_iter()
    .map(|(k, v)| (k.to_owned(), v.to_owned()))
    .collect()
}

/// One GET with no Authorization; the body only on a 200 within the cap.
async fn get(
    transport: &dyn Transport,
    url: String,
    headers: Vec<(String, String)>,
    cap: usize,
) -> Option<String> {
    let request =
        HttpRequest::new(Method::Get, url, headers, None, None, FETCH_DEADLINE, cap).ok()?;
    let response = transport.http(request).await.ok()?;
    let body = response.body.filter(|b| b.len() <= cap)?;
    (response.status == 200).then(|| String::from_utf8_lossy(&body).into_owned())
}

/// The fetch and its cross-checks (T-7, I-10): the page's build number is accepted only when it
/// equals the chunk's (or, for an unchanged chunk hash, the cached value) and lies in range; the
/// capabilities pair only when supported. Partial results are never accepted alone.
pub(crate) async fn fetch(
    transport: &dyn Transport,
    user_agent: &str,
    accept_language: &str,
    cached: Option<&FetchCache>,
    now_ms: u64,
) -> Fetched {
    let work = async {
        let html = get(
            transport,
            APP_URL.to_owned(),
            navigation_headers(user_agent, accept_language),
            HTML_CAP,
        )
        .await?;
        let build = html_build(&html).filter(|b| build_ok(*b))?;
        let hash = fast_connect_hash(&html)?;
        if let Some(c) = cached.filter(|c| c.chunk_hash == hash) {
            return (c.build == build).then(|| Fetched::Accepted {
                build,
                pair: c.pair,
                unsupported: false,
                cache: Some(FetchCache {
                    fetched_at_ms: now_ms,
                    ..c.clone()
                }),
            });
        }
        let chunk = get(
            transport,
            format!("{CHUNK_BASE}{hash}.js"),
            script_headers(user_agent, accept_language),
            CHUNK_CAP,
        )
        .await?;
        (chunk_build(&chunk)? == build).then_some(())?;
        let found = chunk_capabilities(&chunk)?;
        let supported = SUPPORTED_PAIRS.contains(&found);
        Some(Fetched::Accepted {
            build,
            pair: if supported { found } else { newest_supported() },
            unsupported: !supported,
            cache: supported.then_some(FetchCache {
                chunk_hash: hash,
                build,
                pair: found,
                fetched_at_ms: now_ms,
            }),
        })
    };
    tokio::time::timeout(FETCH_DEADLINE, work)
        .await
        .ok()
        .flatten()
        .unwrap_or(Fetched::Failed)
}

pub(crate) fn newest_supported() -> (u64, u64) {
    SUPPORTED_PAIRS[SUPPORTED_PAIRS.len() - 1]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transport::memory::{Http, MemoryTransport};
    use std::sync::Arc;

    const NOW: u64 = 1_800_000_000_000;
    const UA: &str = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/27.0.1 Safari/605.1.15";
    const AL: &str = "zh-TW,zh-Hant;q=0.9";
    /// Captured page regions: build 640102, fast-connect f0cfe6eb7552ae67.
    const HTML: &str = include_str!("../tests/fixtures/identity/app.html");
    /// Captured chunk regions (build 634304, the newest chunk on disk).
    const CHUNK: &str = include_str!("../tests/fixtures/identity/fast-connect.js");

    /// The chunk the page names (build 640102) was not captured; the newest captured chunk's
    /// build literal is substituted so the pair is consistent. Everything else is verbatim.
    fn chunk_640102() -> String {
        CHUNK.replace("\"634304\"", "\"640102\"")
    }

    fn ok(body: &str) -> String {
        format!(
            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\n\r\n{body}",
            body.len()
        )
    }

    async fn run(responses: Vec<String>, cached: Option<&FetchCache>) -> (Fetched, Vec<String>) {
        let http = Arc::new(Http::new(responses));
        let transport = MemoryTransport {
            http: Some(http.clone()),
            endpoints: None,
        };
        let out = fetch(&transport, UA, AL, cached, NOW).await;
        let requests = http.requests.lock().unwrap().clone();
        (out, requests)
    }

    /// Only the two allowed URLs, on discord.com, without Authorization or Discord headers.
    fn assert_allowed(requests: &[String]) {
        for (i, r) in requests.iter().enumerate() {
            let line = r.lines().next().unwrap();
            let expected = if i == 0 {
                "GET /app HTTP/1.1".to_owned()
            } else {
                "GET /assets/fast-connect.f0cfe6eb7552ae67.js HTTP/1.1".to_owned()
            };
            assert_eq!(line, expected);
            assert!(r.contains("\r\nhost: discord.com\r\n"), "{r}");
            let lower = r.to_ascii_lowercase();
            assert!(!lower.contains("authorization"), "{r}");
            assert!(!lower.contains("x-super-properties"), "{r}");
            assert!(!lower.contains("x-discord"), "{r}");
            assert!(!lower.contains("cookie"), "{r}");
        }
        assert!(requests.len() <= 2);
    }

    #[tokio::test]
    async fn the_captured_page_and_chunk_give_640102_and_the_pair() {
        let (out, requests) = run(vec![ok(HTML), ok(&chunk_640102())], None).await;
        assert_eq!(
            out,
            Fetched::Accepted {
                build: 640_102,
                pair: (1_767_421, 1_734_653),
                unsupported: false,
                cache: Some(FetchCache {
                    chunk_hash: "f0cfe6eb7552ae67".into(),
                    build: 640_102,
                    pair: (1_767_421, 1_734_653),
                    fetched_at_ms: NOW,
                }),
            }
        );
        assert_eq!(requests.len(), 2);
        assert_allowed(&requests);
        // Safari's navigation and script header sets, in order (names and values).
        let names = |r: &str| -> Vec<String> {
            r.split("\r\n\r\n")
                .next()
                .unwrap()
                .lines()
                .skip(2)
                .map(str::to_owned)
                .collect()
        };
        assert_eq!(
            names(&requests[0]),
            [
                "Sec-Fetch-Dest: document".to_owned(),
                format!("User-Agent: {UA}"),
                "Upgrade-Insecure-Requests: 1".into(),
                "Accept: text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8".into(),
                "Sec-Fetch-Site: none".into(),
                "Sec-Fetch-Mode: navigate".into(),
                format!("Accept-Language: {AL}"),
                "Priority: u=0, i".into(),
            ]
        );
        assert_eq!(
            names(&requests[1]),
            [
                "Sec-Fetch-Dest: script".to_owned(),
                format!("User-Agent: {UA}"),
                "Accept: */*".into(),
                "Referer: https://discord.com/app".into(),
                "Sec-Fetch-Site: same-origin".into(),
                "Sec-Fetch-Mode: no-cors".into(),
                format!("Accept-Language: {AL}"),
                "Priority: u=1, i".into(),
            ]
        );
    }

    /// The real captured pair disagrees (page 640102, newest captured chunk 634304): refused.
    #[tokio::test]
    async fn a_mismatched_chunk_build_falls_back() {
        let (out, requests) = run(vec![ok(HTML), ok(CHUNK)], None).await;
        assert_eq!(out, Fetched::Failed);
        assert_allowed(&requests);
    }

    #[tokio::test]
    async fn every_bad_page_or_chunk_falls_back_with_only_the_allowed_urls() {
        let hash = "f0cfe6eb7552ae67";
        let html_cases: Vec<(&str, String)> = vec![
            (
                "another host",
                HTML.replace(
                    &format!("\"/assets/fast-connect.{hash}.js\""),
                    &format!("\"https://evil.test/assets/fast-connect.{hash}.js\""),
                ),
            ),
            (
                "dot segment",
                HTML.replace(
                    &format!("\"/assets/fast-connect.{hash}.js\""),
                    &format!("\"/assets/../x/fast-connect.{hash}.js\""),
                ),
            ),
            (
                "two distinct hashes",
                HTML.replacen(
                    &format!("fast-connect.{hash}.js"),
                    "fast-connect.0123456789abcdef.js",
                    1,
                ),
            ),
            (
                "duplicate build literal",
                format!("{HTML}\"BUILD_NUMBER\":\"640103\""),
            ),
            (
                "out-of-range build",
                HTML.replace("\"BUILD_NUMBER\":\"640102\"", "\"BUILD_NUMBER\":\"640101\""),
            ),
            (
                "non-numeric build",
                HTML.replace("\"BUILD_NUMBER\":\"640102\"", "\"BUILD_NUMBER\":\"64O102\""),
            ),
            (
                "missing match",
                HTML.replace("fast-connect", "fast_connect"),
            ),
            (
                "challenge page",
                "<html><title>Just a moment...</title></html>".into(),
            ),
        ];
        for (name, html) in html_cases {
            let (out, requests) = run(vec![ok(&html), ok(&chunk_640102())], None).await;
            assert_eq!(out, Fetched::Failed, "{name}");
            assert_allowed(&requests);
        }
        let chunk_cases: Vec<(&str, String)> = vec![
            (
                "duplicate chunk literal",
                format!("{}isNaN(s=parseInt(\"640102\",10))||(I.client_build_number=s)", chunk_640102()),
            ),
            (
                "duplicate capabilities",
                format!(
                    "{}capabilities:function(e){{let{{useChannelObfuscation:t}}=e;return t?1767421:1734653}}",
                    chunk_640102()
                ),
            ),
            (
                "missing capabilities",
                chunk_640102().replace("useChannelObfuscation:t}", "obf:t}"),
            ),
            (
                "non-numeric capabilities",
                chunk_640102().replace("t?1767421:", "t?x:"),
            ),
        ];
        for (name, chunk) in chunk_cases {
            let (out, requests) = run(vec![ok(HTML), ok(&chunk)], None).await;
            assert_eq!(out, Fetched::Failed, "{name}");
            assert_allowed(&requests);
        }
        // Non-200 page, non-200 chunk, a refused connection.
        let not_found = "HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\n\r\n".to_owned();
        for responses in [
            vec![not_found.clone()],
            vec![ok(HTML), not_found],
            vec!["ERROR connect".to_owned()],
        ] {
            let (out, requests) = run(responses, None).await;
            assert_eq!(out, Fetched::Failed);
            assert_allowed(&requests);
        }
    }

    #[tokio::test]
    async fn over_cap_bodies_fall_back() {
        let big = format!("{HTML}{}", " ".repeat(HTML_CAP));
        assert_eq!(run(vec![ok(&big)], None).await.0, Fetched::Failed);
        let big = format!("{}{}", chunk_640102(), " ".repeat(CHUNK_CAP));
        assert_eq!(run(vec![ok(HTML), ok(&big)], None).await.0, Fetched::Failed);
    }

    #[tokio::test(start_paused = true)]
    async fn a_fetch_that_hangs_ends_at_the_deadline() {
        let started = tokio::time::Instant::now();
        let out = run(vec![crate::transport::memory::HANG.to_owned()], None)
            .await
            .0;
        assert_eq!(out, Fetched::Failed);
        assert_eq!(started.elapsed(), FETCH_DEADLINE);
    }

    #[tokio::test]
    async fn an_unsupported_pair_keeps_the_newest_supported_one_and_is_not_cached() {
        let chunk = chunk_640102().replace("t?1767421:1734653}", "t?1800189:1767421}");
        let (out, _) = run(vec![ok(HTML), ok(&chunk)], None).await;
        assert_eq!(
            out,
            Fetched::Accepted {
                build: 640_102,
                pair: BUILTIN_PAIR,
                unsupported: true,
                cache: None
            }
        );
    }

    #[tokio::test]
    async fn an_unchanged_chunk_hash_reuses_the_cached_values_without_the_chunk() {
        let cached = FetchCache {
            chunk_hash: "f0cfe6eb7552ae67".into(),
            build: 640_102,
            pair: BUILTIN_PAIR,
            fetched_at_ms: NOW - 90_000_000,
        };
        let (out, requests) = run(vec![ok(HTML)], Some(&cached)).await;
        assert!(
            matches!(out, Fetched::Accepted { build: 640_102, cache: Some(ref c), .. } if c.fetched_at_ms == NOW)
        );
        assert_eq!(requests.len(), 1);
        // A page build that disagrees with the cached value for the same chunk: refused.
        let stale = FetchCache {
            build: 640_200,
            ..cached
        };
        assert_eq!(run(vec![ok(HTML)], Some(&stale)).await.0, Fetched::Failed);
    }

    #[test]
    fn pattern_helpers_match_exactly_once() {
        assert_eq!(html_build(HTML), Some(640_102));
        assert_eq!(fast_connect_hash(HTML).as_deref(), Some("f0cfe6eb7552ae67"));
        assert_eq!(chunk_build(CHUNK), Some(634_304));
        assert_eq!(chunk_capabilities(CHUNK), Some(BUILTIN_PAIR));
        assert_eq!(html_build("\"BUILD_NUMBER\":\"12345\""), None);
        assert_eq!(html_build("\"BUILD_NUMBER\":\"12345678\""), None);
        assert_eq!(
            fast_connect_hash("\"/assets/fast-connect.f0cfe6eb7552ae67.js?x=1\""),
            None
        );
        assert_eq!(
            fast_connect_hash("\"/assets/fast-connect.F0CFE6EB7552AE67.js\""),
            None
        );
    }
}
