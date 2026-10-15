//! Wave 3 track E: embeds and media. Included from messages_tests.rs, so it shares that file's
//! helpers. Every test goes through `parse`/`merge`/`observe` and `row()`, the path a snapshot
//! takes.
use super::*;
use crate::embeds::{self, Embed, EmbedImage, EmbedLink};

fn with_embeds(content: &str, embeds: Value) -> Value {
    let mut m = msg(10, 5, content);
    m["embeds"] = embeds;
    m
}

fn row_of(m: &Value) -> MessageRow {
    row(&parse(m, false).unwrap(), &Context::default(), false, None)
}

fn embeds_of(m: &Value) -> Vec<Embed> {
    row_of(m).embeds
}

/// Every `Text` of `blocks` with whether it sits inside a spoiler.
fn texts(blocks: &[Block]) -> Vec<(String, bool)> {
    fn inlines(v: &[Inline], spoiler: bool, out: &mut Vec<(String, bool)>) {
        for i in v {
            match i {
                Inline::Text(t) | Inline::Code(t) => out.push((t.clone(), spoiler)),
                Inline::Bold(c)
                | Inline::Italic(c)
                | Inline::Underline(c)
                | Inline::Strike(c)
                | Inline::Link { label: c, .. } => inlines(c, spoiler, out),
                Inline::Spoiler(c) => inlines(c, true, out),
                Inline::Block(b) => blocks_in(std::slice::from_ref(b), spoiler, out),
                _ => {}
            }
        }
    }
    fn blocks_in(v: &[Block], spoiler: bool, out: &mut Vec<(String, bool)>) {
        for b in v {
            match b {
                Block::Paragraph(c) | Block::Subtext(c) | Block::Heading { content: c, .. } => {
                    inlines(c, spoiler, out)
                }
                Block::Quote(q) => blocks_in(q, spoiler, out),
                Block::List { items, .. } => items.iter().for_each(|i| blocks_in(i, spoiler, out)),
                Block::CodeBlock { code, .. } => out.push((code.clone(), spoiler)),
            }
        }
    }
    let mut out = Vec::new();
    blocks_in(blocks, false, &mut out);
    out
}

fn chars(blocks: &[Block]) -> usize {
    texts(blocks).iter().map(|(t, _)| t.chars().count()).sum()
}

/// Characters of the budgeted text of `e`.
fn budgeted(e: &Embed) -> usize {
    let n = |s: &Option<String>| s.as_deref().map_or(0, |s| s.chars().count());
    n(&e.title)
        + n(&e.author_name)
        + n(&e.footer_text)
        + chars(&e.description)
        + e.fields
            .iter()
            .map(|f| f.name.chars().count() + chars(&f.value))
            .sum::<usize>()
}

fn parses() -> usize {
    embeds::PARSES.with(|p| p.get())
}

// ---- What is parsed ----

#[test]
fn original_image_icon_and_video_urls_never_reach_the_row() {
    let r = row_of(&with_embeds(
        "x",
        json!([{
            "type": "video",
            "url": "https://example.test/watch",
            "title": "t",
            "author": {"name": "a", "icon_url": "https://evil.test/a.png",
                       "proxy_icon_url": "https://images-ext-1.discordapp.net/external/a.png"},
            "footer": {"text": "f", "icon_url": "https://evil.test/f.png",
                       "proxy_icon_url": "https://images-ext-2.discordapp.net/external/f.png"},
            "thumbnail": {"url": "https://evil.test/t.png", "width": 80, "height": 60,
                          "proxy_url": "https://images-ext-1.discordapp.net/external/t.png"},
            "image": {"url": "https://evil.test/i.png", "width": 400, "height": 300,
                      "proxy_url": "https://media.discordapp.net/attachments/1/2/i.png"},
            "video": {"url": "https://evil.test/v.mp4", "proxy_url": "https://evil.test/pv.mp4",
                      "width": 640, "height": 360},
        }]),
    ));
    assert!(!format!("{r:?}").contains("evil.test"), "{r:?}");
    let e = &r.embeds[0];
    assert_eq!(
        e.author_icon_url.as_deref(),
        Some("https://images-ext-1.discordapp.net/external/a.png")
    );
    assert_eq!(
        e.footer_icon_url.as_deref(),
        Some("https://images-ext-2.discordapp.net/external/f.png")
    );
    assert_eq!(
        e.thumbnail.as_ref().map(|t| t.url.as_str()),
        Some("https://images-ext-1.discordapp.net/external/t.png")
    );
    assert_eq!(
        e.image.as_ref().map(|t| t.url.as_str()),
        Some("https://media.discordapp.net/attachments/1/2/i.png")
    );
    assert!(e.is_video);
}

#[test]
fn an_evil_embed_url_reaches_the_row_only_as_the_checked_link_target() {
    let r = row_of(&with_embeds(
        "x",
        json!([{"url": "https://evil.test/", "title": "t"}]),
    ));
    assert_eq!(
        r.embeds[0].url,
        Some(EmbedLink {
            url: "https://evil.test/".into(),
            host: "evil.test".into()
        })
    );
    // Once as the target, once as its host; nowhere else.
    assert_eq!(format!("{r:?}").matches("evil.test").count(), 2);
}

#[test]
fn images_need_a_media_proxy_url_and_sides_up_to_16384() {
    let image = |v: Value| {
        embeds_of(&with_embeds("x", json!([{ "image": v }])))[0]
            .image
            .clone()
    };
    let proxy = "https://media.discordapp.net/attachments/1/2/i.png";
    assert_eq!(
        image(json!({"proxy_url": proxy, "width": 16384, "height": 1, "description": "alt"})),
        Some(EmbedImage {
            url: proxy.into(),
            width: 16384,
            height: 1,
            description: Some("alt".into())
        })
    );
    for v in [
        // An allowed `url` with no `proxy_url`.
        json!({"url": "https://media.discordapp.net/a.png", "width": 10, "height": 10}),
        json!({"proxy_url": "https://evil.test/a.png", "width": 10, "height": 10}),
        json!({"proxy_url": "http://media.discordapp.net/a.png", "width": 10, "height": 10}),
        json!({"proxy_url": proxy, "width": 0, "height": 10}),
        json!({"proxy_url": proxy, "width": 16385, "height": 10}),
        json!({"proxy_url": proxy, "width": -1, "height": 10}),
        json!({"proxy_url": proxy, "width": 10}),
        json!({"proxy_url": proxy, "width": 1.5, "height": 10}),
    ] {
        assert_eq!(image(v.clone()), None, "{v}");
    }
}

#[test]
fn video_and_gifv_are_marked_and_other_types_are_not() {
    let is_video = |v: Value| embeds_of(&with_embeds("x", json!([v])))[0].is_video;
    assert!(is_video(json!({"type": "video"})));
    assert!(is_video(json!({"type": "gifv"})));
    assert!(is_video(json!({"type": "rich", "video": {"width": 1}})));
    assert!(!is_video(json!({"type": "rich"})));
    assert!(!is_video(json!({"type": "image", "video": null})));
}

#[test]
fn at_most_10_embeds_and_25_fields() {
    let field = json!({"name": "n", "value": "v"});
    let embed = json!({"fields": vec![field; 30]});
    let e = embeds_of(&with_embeds("x", json!(vec![embed; 12])));
    assert_eq!(e.len(), 10);
    assert!(e.iter().all(|e| e.fields.len() == 25));
}

// ---- Budgets ----

#[test]
fn each_field_is_cut_to_its_cap_by_char() {
    let a = |n: usize| "字".repeat(n);
    let e = &embeds_of(&with_embeds(
        "x",
        json!([
            {"title": a(300), "author": {"name": a(300)}},
            {"fields": [{"name": a(300), "value": a(2000), "inline": true}]},
            {"footer": {"text": a(3000)}},
        ]),
    ));
    assert_eq!(e[0].title.as_ref().unwrap().chars().count(), 256);
    assert_eq!(e[0].author_name.as_ref().unwrap().chars().count(), 256);
    assert_eq!(e[1].fields[0].name.chars().count(), 256);
    assert_eq!(chars(&e[1].fields[0].value), 1024);
    assert!(e[1].fields[0].inline);
    assert_eq!(e[2].footer_text.as_ref().unwrap().chars().count(), 2048);
    let d = &embeds_of(&with_embeds("x", json!([{ "description": a(5000) }])))[0];
    assert_eq!(chars(&d.description), 4096);
}

#[test]
fn ten_embeds_of_25_full_fields_keep_at_most_6000_characters() {
    let full = |n: usize| "x".repeat(n);
    let embed = json!({
        "author": {"name": full(256)}, "title": full(256), "description": full(4096),
        "fields": vec![json!({"name": full(256), "value": full(1024)}); 25],
        "footer": {"text": full(2048)},
    });
    let e = embeds_of(&with_embeds("x", json!(vec![embed; 10])));
    let total: usize = e.iter().map(budgeted).sum();
    assert_eq!(total, 6000);
    // Display order: the first embed's author name, title, description, then fields.
    assert_eq!(e[0].author_name.as_ref().unwrap().len(), 256);
    assert_eq!(chars(&e[0].description), 4096);
    assert_eq!(e[0].fields[0].name.len(), 256);
    assert_eq!(chars(&e[0].fields[0].value), 1024);
    // 6000 - 256 - 256 - 4096 - 256 - 1024 = 112 left for the second field's name.
    assert_eq!(e[0].fields.len(), 2);
    assert_eq!(e[0].fields[1].name.len(), 112);
    assert!(e[0].fields[1].value.is_empty());
    assert_eq!(e[0].footer_text, None);
    assert!(e[1..].iter().all(|e| budgeted(e) == 0));
}

#[test]
fn a_cut_description_or_value_never_shows_spoiler_text_outside_a_spoiler() {
    let leaked = |blocks: &[Block]| {
        let t = texts(blocks);
        let hidden = t.iter().any(|(s, sp)| *sp && s.contains("secr"));
        let shown = t.iter().any(|(s, sp)| !*sp && s.contains("secr"));
        (hidden, shown)
    };
    // The cap keeps `||secr` of `||secret||`.
    let d = &embeds_of(&with_embeds(
        "x",
        json!([{"description": format!("{}||secret||", "a".repeat(4090))}]),
    ))[0];
    assert_eq!(leaked(&d.description), (true, false));
    let v = &embeds_of(&with_embeds(
        "x",
        json!([{"fields": [{"name": "n", "value": format!("{}||secret||", "a".repeat(1018))}]}]),
    ))[0];
    assert_eq!(leaked(&v.fields[0].value), (true, false));
    // The budget cuts the second embed's description inside its spoiler.
    let e = embeds_of(&with_embeds(
        "x",
        json!([
            {"description": "a".repeat(4096), "footer": {"text": "b".repeat(1898)}},
            {"description": "||secret||"},
        ]),
    ));
    assert_eq!(leaked(&e[1].description), (true, false));
    // Uncut, the spoiler is parsed as one.
    let whole = &embeds_of(&with_embeds(
        "x",
        json!([{"description": "a ||secret|| b"}]),
    ))[0];
    assert_eq!(leaked(&whole.description), (true, false));
}

#[test]
fn a_4097_character_cjk_description_is_cut_without_panicking() {
    let d = &embeds_of(&with_embeds(
        "x",
        json!([{"description": "字".repeat(4097)}]),
    ))[0];
    assert_eq!(chars(&d.description), 4096);
}

#[tokio::test]
async fn snapshots_never_parse_embeds() {
    let (_mock, store) = serve(&[ok(&json!([]))]).await;
    let epoch = store.open(5);
    settled(&store, epoch).await;
    let before = parses();
    let mut m = with_embeds(
        "x",
        json!([{"description": "**d**", "fields": [{"name": "n", "value": "*v*"}]}]),
    );
    m["channel_id"] = json!("5");
    store.observe("MESSAGE_CREATE", &m);
    assert_eq!(
        parses() - before,
        2,
        "the description and the value, once each"
    );
    let before = parses();
    for _ in 0..5 {
        assert_eq!(now(&store, epoch).rows[0].embeds.len(), 1);
    }
    assert_eq!(parses(), before);
}

#[test]
fn an_embed_code_block_never_sets_pending() {
    let m = with_embeds(
        "plain",
        json!([{"description": "```py\nx = 1\n```", "fields": [{"name": "n", "value": "```rs\nfn f() {}\n```"}]}]),
    );
    let mut stored = parse(&m, false).unwrap();
    assert!(!stored.pending);
    stored.merge(&json!({"embeds": [{"description": "```py\ny = 2\n```"}]}));
    assert!(!stored.pending);
    let r = row(&stored, &Context::default(), false, None);
    assert!(matches!(
        &r.embeds[0].description[..],
        [Block::CodeBlock {
            highlight: None,
            ..
        }]
    ));
}

// ---- Links ----

#[test]
fn only_checked_http_links_become_targets() {
    let link = |raw: &str| {
        embeds_of(&with_embeds(
            "x",
            json!([{"title": "t", "url": raw, "author": {"name": "a", "url": raw},
                    "provider": {"name": "p", "url": raw}}]),
        ))[0]
            .clone()
    };
    let ok = link("https://a.test/x?y=1");
    let target = Some(EmbedLink {
        url: "https://a.test/x?y=1".into(),
        host: "a.test".into(),
    });
    assert_eq!(
        (&ok.url, &ok.author_url, &ok.provider_url),
        (&target, &target, &target)
    );
    let long = format!("https://a.test/{}", "x".repeat(2048 - 15));
    assert_eq!(long.len(), 2048);
    assert!(link(&long).url.is_some());
    for raw in [
        "javascript:alert(1)",
        "ftp://a.test/x",
        "file:///etc/passwd",
        "https://user@a.test/",
        "https://user:pw@a.test/",
        "https://a.test/\u{202E}x",
        "https://a.test/\u{7}",
        "https://a.test:8443/",
        "//a.test/x",
        &format!("{long}x"),
    ] {
        let e = link(raw);
        assert_eq!(
            (&e.url, &e.author_url, &e.provider_url),
            (&None, &None, &None),
            "{raw:?}"
        );
        // The text stays, as plain text.
        assert_eq!(
            (
                e.title.as_deref(),
                e.author_name.as_deref(),
                e.provider_name.as_deref()
            ),
            (Some("t"), Some("a"), Some("p"))
        );
    }
}

// ---- Spoilers ----

#[test]
fn an_embed_of_a_spoilered_link_is_covered_and_the_same_link_unspoilered_is_not() {
    let embed = json!([{"url": "https://a.test/x", "title": "t",
                        "image": {"proxy_url": "https://media.discordapp.net/i.png", "width": 9, "height": 9}}]);
    for content in [
        "look ||https://a.test/x||",
        "||[here](https://a.test/x)||",
        "||**<https://a.test/x>**||",
    ] {
        assert!(
            embeds_of(&with_embeds(content, embed.clone()))[0].spoiler,
            "{content}"
        );
    }
    for content in [
        "look https://a.test/x",
        "||https://a.test/y|| https://a.test/x",
        "",
    ] {
        assert!(
            !embeds_of(&with_embeds(content, embed.clone()))[0].spoiler,
            "{content}"
        );
    }
}

#[test]
fn is_spoiler_and_spoiler_filenames_cover_image_attachments() {
    let mut m = msg(10, 5, "x");
    let url = "https://cdn.discordapp.com/attachments/1/2/f.png";
    m["attachments"] = json!([
        {"filename": "a.png", "url": url, "width": 1, "height": 1, "flags": 8},
        {"filename": "SPOILER_b.png", "url": url, "width": 1, "height": 1},
        {"filename": "c.png", "url": url, "width": 1, "height": 1, "flags": 4},
    ]);
    let r = row_of(&m);
    assert_eq!(
        r.images.iter().map(|i| i.spoiler).collect::<Vec<_>>(),
        [true, true, false]
    );
}

#[test]
fn image_attachments_load_only_from_media_hosts_preferring_the_proxy() {
    let mut m = msg(10, 5, "x");
    m["attachments"] = json!([
        {"filename": "a.png", "url": "https://cdn.discordapp.com/attachments/1/2/a.png",
         "proxy_url": "https://media.discordapp.net/attachments/1/2/a.png", "width": 4, "height": 3},
        {"filename": "b.png", "url": "https://evil.test/b.png", "width": 4, "height": 3},
        {"filename": "c.png", "url": "https://cdn.discordapp.com/c.png",
         "proxy_url": "https://evil.test/c.png", "width": 4, "height": 3},
    ]);
    let r = row_of(&m);
    assert_eq!(
        r.images.iter().map(|i| i.url.as_str()).collect::<Vec<_>>(),
        [
            "https://media.discordapp.net/attachments/1/2/a.png",
            "https://cdn.discordapp.com/c.png"
        ]
    );
    assert_eq!(r.files, ["b.png"]);
}

// ---- Updates ----

#[tokio::test]
async fn updates_replace_keep_suppress_and_show_embeds() {
    let (_mock, store) = serve(&[ok(&json!([]))]).await;
    let epoch = store.open(5);
    settled(&store, epoch).await;
    let titles = |s: &MessagesSnapshot| {
        s.rows[0]
            .embeds
            .iter()
            .filter_map(|e| e.title.clone())
            .collect::<Vec<_>>()
    };
    let mut m = with_embeds("x", json!([{"title": "one"}]));
    m["channel_id"] = json!("5");
    store.observe("MESSAGE_CREATE", &m);
    assert_eq!(titles(&now(&store, epoch)), ["one"]);
    let update = |extra: Value| {
        let mut d = json!({"id": "10", "channel_id": "5"});
        d.as_object_mut()
            .unwrap()
            .extend(extra.as_object().unwrap().clone());
        store.observe("MESSAGE_UPDATE", &d);
        titles(&now(&store, epoch))
    };
    assert_eq!(update(json!({"content": "edited"})), ["one"]);
    assert_eq!(update(json!({"embeds": [{"title": "two"}]})), ["two"]);
    assert_eq!(update(json!({"flags": 4})), Vec::<String>::new());
    assert_eq!(update(json!({"content": "again"})), Vec::<String>::new());
    assert_eq!(update(json!({"flags": 0})), ["two"]);
    assert_eq!(update(json!({"embeds": []})), Vec::<String>::new());
    // A MESSAGE_CREATE re-insert takes the new copy's embeds.
    let mut again = with_embeds("x", json!([{"title": "three"}]));
    again["channel_id"] = json!("5");
    store.observe("MESSAGE_CREATE", &again);
    assert_eq!(titles(&now(&store, epoch)), ["three"]);
    // Created suppressed.
    let mut hidden = with_embeds("x", json!([{"title": "four"}]));
    hidden["channel_id"] = json!("5");
    hidden["flags"] = json!(4);
    store.observe("MESSAGE_CREATE", &hidden);
    assert_eq!(titles(&now(&store, epoch)), Vec::<String>::new());
}

// ---- Timestamp and colour ----

#[test]
fn timestamps_follow_the_fixed_grammar() {
    let t = embeds::timestamp_ms;
    assert_eq!(
        t("2026-10-15T12:34:56.789000+00:00"),
        Some(1_792_067_696_789)
    );
    assert_eq!(t("2026-10-15T12:34:56Z"), Some(1_792_067_696_000));
    assert_eq!(t("2026-10-15T20:34:56.7+08:00"), Some(1_792_067_696_700));
    assert_eq!(
        t("2026-10-15T12:34:56.123456789-01:30"),
        Some(1_792_073_096_123)
    );
    assert_eq!(t("1970-01-01T00:00:00Z"), Some(0));
    assert_eq!(t("2024-02-29T00:00:00Z"), Some(1_709_164_800_000));
    assert_eq!(t("9999-12-31T23:59:59.999Z"), Some(253_402_300_799_999));
    let mega = "9".repeat(1 << 20);
    for bad in [
        "",
        "garbage",
        mega.as_str(),
        "2026-10-15",
        "2026-10-15T12:34:56",
        "2026-10-15 12:34:56Z",
        "2026-10-15T12:34:56.Z",
        "2026-10-15T12:34:56.1234567890Z",
        "2026-10-15T12:34:56+0000",
        "2026-10-15T12:34:56+24:00",
        "2026-10-15T12:34:56Zjunk",
        "2026-13-01T00:00:00Z",
        "2023-02-29T00:00:00Z",
        "2026-04-31T00:00:00Z",
        "2026-10-15T24:00:00Z",
        "2026-10-15T12:60:00Z",
        "2026-10-15T12:34:60Z",
        "1969-12-31T23:59:59Z",
        "+2026-10-15T12:34:56Z",
        "２０２６-10-15T12:34:56Z",
    ] {
        assert_eq!(t(bad), None, "{bad:?}");
    }
    // Through the message path.
    let e = &embeds_of(&with_embeds(
        "x",
        json!([{"timestamp": "2026-10-15T12:34:56Z"}, {"timestamp": 5}]),
    ));
    assert_eq!(
        (e[0].timestamp_ms, e[1].timestamp_ms),
        (Some(1_792_067_696_000), None)
    );
}

#[test]
fn colours_outside_24_bits_give_the_default() {
    let colour = |v: Value| embeds_of(&with_embeds("x", json!([{ "color": v }])))[0].color;
    assert_eq!(colour(json!(0xFF_FFFF)), Some(0xFF_FFFF));
    assert_eq!(colour(json!(0)), Some(0));
    for v in [
        json!(-1),
        json!(1u64 << 32),
        json!(0x100_0000),
        json!(1.5),
        json!("red"),
        json!(null),
    ] {
        assert_eq!(colour(v.clone()), None, "{v}");
    }
}

// ---- Emoji and mentions ----

/// A snowflake-sized id (the parser wants Discord's digit counts).
const ID: u64 = 100_000_000_000_000_000;

#[test]
fn embed_emoji_count_toward_the_rows_64_images() {
    let emoji = |from: u64, n: u64| {
        (from..from + n)
            .map(|i| format!("<:ee:{}>", ID + i))
            .collect::<String>()
    };
    let r = row_of(&with_embeds(
        &emoji(0, 60),
        json!([{"description": emoji(60, 5), "fields": [{"name": "n", "value": emoji(65, 5)}]}]),
    ));
    assert_eq!(r.emoji.len(), 64);
    assert_eq!(r.emoji.last().map(|e| e.id), Some(ID + 63));
}

#[test]
fn embed_mentions_resolve_with_the_body() {
    let r = row_of(&with_embeds(
        "",
        json!([{"description": format!("<#{}>", ID + 1), "fields": [{"name": "n", "value": format!("<@&{}>", ID + 2)}]}]),
    ));
    assert_eq!(
        r.mentions
            .iter()
            .map(|m| (m.kind, m.id))
            .collect::<Vec<_>>(),
        [(MentionKind::Channel, ID + 1), (MentionKind::Role, ID + 2)]
    );
}
