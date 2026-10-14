//! Parser tests (docs/plans/markdown.md, K1): the differential test against the oracle fixture,
//! then one named test per rule, pairing, grouping example, list clause, token grammar, link
//! deviation, limit and the step bound.
use super::*;
use serde_json::{json, Value};

fn t(s: &str) -> Inline {
    Inline::Text(s.to_owned())
}

fn para(content: Vec<Inline>) -> Block {
    Block::Paragraph(content)
}

/// The whole message as one paragraph.
fn one(content: Vec<Inline>) -> Vec<Block> {
    vec![para(content)]
}

fn link(url: &str, host: &str, label: Vec<Inline>, masked: bool) -> Inline {
    Inline::Link {
        url: url.to_owned(),
        host: host.to_owned(),
        label,
        masked,
    }
}

fn list(ordered: bool, start: u32, items: Vec<Vec<Block>>) -> Block {
    Block::List {
        ordered,
        start,
        items,
    }
}

// ---- Differential test against the oracle (plan: "The oracle") ----

const FIXTURE: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/fixtures/markdown_oracle.jsonl"
);

/// velella's pre-grouping stream in the oracle's JSON shape (`inQuote` from the ancestry, as the
/// oracle records it from its state).
fn velella_json(nodes: &[Node], in_quote: bool) -> Value {
    Value::Array(nodes.iter().map(|n| node_json(n, in_quote)).collect())
}

fn node_json(n: &Node, in_quote: bool) -> Value {
    let kids = |c: &[Node]| velella_json(c, in_quote);
    match n {
        Node::Text(s) => json!({"type": "text", "content": s}),
        Node::Em(c) => json!({"type": "em", "content": kids(c)}),
        Node::Strong(c) => json!({"type": "strong", "content": kids(c)}),
        Node::Underline(c) => json!({"type": "underline", "content": kids(c)}),
        Node::Strike(c) => json!({"type": "strikethrough", "content": kids(c)}),
        Node::Spoiler(c) => json!({"type": "spoiler", "content": kids(c)}),
        Node::Code(c) => json!({"type": "inlineCode", "content": c}),
        Node::Link {
            kind, url, label, ..
        } => {
            let ty = match kind {
                LinkKind::Auto => "autolink",
                LinkKind::Url => "url",
                LinkKind::Masked => "link",
            };
            json!({"type": ty, "content": kids(label), "target": url})
        }
        Node::Token(tok) => token_json(tok),
        Node::Br => json!({"type": "br"}),
        Node::Heading { level, content } => {
            json!({"type": "heading", "level": level, "content": kids(content)})
        }
        Node::Subtext(c) => json!({"type": "subtext", "content": kids(c)}),
        Node::Quote(c) => json!({"type": "blockQuote", "content": velella_json(c, true)}),
        Node::CodeBlock { language, code } => json!({
            "type": "codeBlock", "lang": language.clone().unwrap_or_default(),
            "content": code, "inQuote": in_quote,
        }),
        Node::List { .. } => json!({"type": "list"}),
    }
}

fn token_json(tok: &Inline) -> Value {
    match tok {
        Inline::UserMention(id) => json!({"type": "user", "id": id.to_string()}),
        Inline::RoleMention(id) => json!({"type": "role", "id": id.to_string()}),
        Inline::ChannelMention(id) => json!({"type": "channel", "id": id.to_string()}),
        Inline::Everyone => json!({"type": "everyone"}),
        Inline::Here => json!({"type": "here"}),
        Inline::CustomEmoji { id, name, animated } => json!({
            "type": "emoji", "id": id.to_string(), "name": name, "animated": animated,
        }),
        Inline::Timestamp { unix, style } => {
            let letter = match style {
                TimestampStyle::ShortTime => "t",
                TimestampStyle::MediumTime => "T",
                TimestampStyle::ShortDate => "d",
                TimestampStyle::LongDate => "D",
                TimestampStyle::LongDateShortTime => "f",
                TimestampStyle::FullDateShortTime => "F",
                TimestampStyle::ShortDateShortTime => "s",
                TimestampStyle::ShortDateMediumTime => "S",
                TimestampStyle::Relative => "R",
            };
            json!({"type": "timestamp", "timestamp": unix.to_string(), "format": letter})
        }
        Inline::SlashCommand { name, id } => {
            let parts: Vec<&str> = name.split(' ').collect();
            let (group, sub) = match parts.len() {
                2 => (Value::Null, json!(parts[1])),
                3 => (json!(parts[1]), json!(parts[2])),
                _ => (Value::Null, Value::Null),
            };
            json!({
                "type": "slashCommand", "fullName": name, "name": parts[0],
                "subcommand": sub, "subcommandGroup": group, "id": id.to_string(),
            })
        }
        Inline::GuildNavigation { guild, target } => {
            let mut v = json!({"type": "guildNavigation", "id": guild.to_string()});
            let nav = match target {
                NavTarget::Customize => "customize",
                NavTarget::Browse => "browse",
                NavTarget::Guide => "guide",
                NavTarget::LinkedRoles(role) => {
                    v["roleId"] = role.map_or(Value::Null, |r| json!(r.to_string()));
                    "linked-roles"
                }
            };
            v["navigation"] = json!(nav);
            v
        }
        other => json!({"type": format!("unexpected {other:?}")}),
    }
}

/// The oracle's AST with numbers compared as numbers: ids as `u64`, timestamps as `i64` (velella
/// stores them typed; the oracle keeps the digit string), and a missing timestamp style as `f`.
fn oracle_json(v: &Value) -> Value {
    match v {
        Value::Array(a) => Value::Array(a.iter().map(oracle_json).collect()),
        Value::Object(o) => {
            let mut o = o.clone();
            for key in ["id", "roleId"] {
                if let Some(id) = o.get(key).and_then(Value::as_str) {
                    let n: u64 = id.parse().expect("ids out of u64 range are D2");
                    o.insert(key.into(), json!(n.to_string()));
                }
            }
            if o.get("type") == Some(&json!("timestamp")) {
                let n: i64 = o["timestamp"].as_str().unwrap().parse().unwrap();
                o.insert("timestamp".into(), json!(n.to_string()));
                o.entry("format").or_insert(json!("f"));
            }
            if let Some(c) = o.get("content").filter(|c| c.is_array()) {
                let c = oracle_json(c);
                o.insert("content".into(), c);
            }
            Value::Object(o)
        }
        other => other.clone(),
    }
}

/// Every node of an oracle tree, with whether it lies inside a masked link's label.
fn oracle_nodes<'a>(nodes: &'a Value, in_label: bool, out: &mut Vec<(&'a Value, bool)>) {
    for n in nodes.as_array().into_iter().flatten() {
        out.push((n, in_label));
        if let Some(c) = n.get("content").filter(|c| c.is_array()) {
            oracle_nodes(c, in_label || n["type"] == "link", out);
        }
    }
}

fn oracle_depth(nodes: &Value) -> usize {
    nodes
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|n| n.get("content").filter(|c| c.is_array()))
        .map(|c| 1 + oracle_depth(c))
        .max()
        .unwrap_or(0)
}

/// D1: a list marker and a space anywhere a line could start, after nothing but ASCII
/// punctuation and spaces on its line (generous: containers may open before a list).
fn d1_lists(input: &str) -> bool {
    // As the oracle's preprocess sees it (written out here, not velella's function).
    let input = input
        .replace("\r\n", "\n")
        .replace('\r', "\n")
        .replace('\u{c}', "")
        .replace('\t', "    ");
    input.split('\n').any(|line| {
        let b = line.as_bytes();
        (0..b.len())
            .take_while(|&i| i == 0 || b[i - 1].is_ascii_punctuation() || b[i - 1] == b' ')
            .any(|i| {
                let rest = &b[i..];
                let digits = rest.iter().take_while(|x| x.is_ascii_digit()).count();
                rest.starts_with(b"- ")
                    || rest.starts_with(b"* ")
                    || rest.starts_with(b"+ ")
                    || ((1..=9).contains(&digits) && rest[digits..].starts_with(b". "))
            })
    })
}

/// Maximal ASCII digit runs of `s` as (start, run).
fn digit_runs(s: &str) -> Vec<(usize, &str)> {
    let b = s.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < b.len() {
        if b[i].is_ascii_digit() {
            let n = b[i..].iter().take_while(|x| x.is_ascii_digit()).count();
            out.push((i, &s[i..i + n]));
            i += n;
        } else {
            i += 1;
        }
    }
    out
}

/// D2: an id-length digit run that is zero or above `u64::MAX`, or the nameless slash command.
fn d2_ids(input: &str) -> bool {
    input.contains("</:")
        || digit_runs(input).iter().any(|(_, run)| {
            (17..=21).contains(&run.len()) && run.parse::<u64>().map_or(true, |n| n == 0)
        })
}

/// D3: `<t:` with more than 17 digits or a value beyond 8.64e12.
fn d3_timestamps(input: &str) -> bool {
    digit_runs(input).iter().any(|&(i, run)| {
        let before = &input[..i];
        (before.ends_with("<t:") || before.ends_with("<t:-"))
            && (run.len() > 17 || run.parse::<u64>().map_or(true, |n| n > 8_640_000_000_000))
    })
}

/// An http(s) URL with a host, no userinfo, and only printable ASCII or alphanumeric characters
/// (broader than C0/C1/Cf on purpose: this only decides what is filtered).
fn plain_http(target: &str) -> bool {
    target
        .chars()
        .all(|c| c.is_ascii_graphic() || c.is_alphanumeric())
        && url::Url::parse(target).is_ok_and(|u| {
            matches!(u.scheme(), "http" | "https")
                && u.host_str().is_some_and(|h| !h.is_empty())
                && u.username().is_empty()
                && u.password().is_none()
        })
}

/// D4: an oracle link whose target (or, for plain links, the raw text) velella must reject; a
/// masked target's raw text is not in the fixture, so one that may hide a control or format
/// character behind an escape or a paren group counts when the input holds such a character.
fn d4_links(input: &str, ast: &Value) -> bool {
    let mut nodes = Vec::new();
    oracle_nodes(ast, false, &mut nodes);
    let odd_input = input
        .chars()
        .any(|c| c.is_control() || !(c.is_ascii() || c.is_alphanumeric() || c.is_whitespace()));
    nodes.iter().any(|(n, _)| match n["type"].as_str() {
        Some("url" | "autolink") => {
            !plain_http(n["target"].as_str().unwrap_or_default())
                || !plain_http(n["content"][0]["content"].as_str().unwrap_or_default())
        }
        Some("link") => {
            let target = n["target"].as_str().unwrap_or_default();
            !plain_http(target) || (odd_input && (target.contains('%') || target.contains('(')))
        }
        _ => false,
    })
}

/// D5: over a limit in the input or the oracle's tree.
fn d5_limits(input: &str, ast: &Value) -> bool {
    let mut nodes = Vec::new();
    oracle_nodes(ast, false, &mut nodes);
    input.len() > MAX_INPUT || oracle_depth(ast) > MAX_DEPTH || nodes.len() > MAX_NODES
}

/// D7: a link or a block inside a masked link's label.
fn d7_labels(ast: &Value) -> bool {
    let mut nodes = Vec::new();
    oracle_nodes(ast, false, &mut nodes);
    nodes.iter().any(|(n, in_label)| {
        *in_label
            && matches!(
                n["type"].as_str(),
                Some(
                    "url"
                        | "autolink"
                        | "link"
                        | "codeBlock"
                        | "heading"
                        | "subtext"
                        | "blockQuote"
                )
            )
    })
}

#[test]
fn the_pre_grouping_stream_matches_the_oracle_fixture_outside_d1_to_d7() {
    let fixture = std::fs::read_to_string(FIXTURE).expect("main's oracle fixture");
    let mut compared = 0;
    let mut filtered = [0usize; 7];
    let mut any_filtered = 0;
    let mut mismatches = Vec::new();
    for line in fixture.lines() {
        let entry: Value = serde_json::from_str(line).unwrap();
        let input = entry["input"].as_str().unwrap();
        let ast = &entry["ast"];
        // D6 (K0 overrides) has no captures yet, so no entry is in it.
        let classes = [
            d1_lists(input),
            d2_ids(input),
            d3_timestamps(input),
            d4_links(input, ast),
            d5_limits(input, ast),
            false,
            d7_labels(ast),
        ];
        for (n, hit) in filtered.iter_mut().zip(classes) {
            *n += usize::from(hit);
        }
        if classes.contains(&true) {
            any_filtered += 1;
            continue;
        }
        compared += 1;
        let ours = flat(input, true).map(|nodes| velella_json(&nodes, false));
        if ours.as_ref() != Some(&oracle_json(ast)) {
            mismatches.push(format!(
                "{} {input:?}\n  oracle  {}\n  velella {}",
                entry["source"],
                oracle_json(ast),
                ours.unwrap_or(Value::Null)
            ));
        }
    }
    println!(
        "oracle fixture: {} entries, {compared} compared, {any_filtered} filtered \
         (D1 {}, D2 {}, D3 {}, D4 {}, D5 {}, D6 {}, D7 {}), {} mismatches",
        compared + any_filtered,
        filtered[0],
        filtered[1],
        filtered[2],
        filtered[3],
        filtered[4],
        filtered[5],
        filtered[6],
        mismatches.len()
    );
    assert_eq!(
        compared + any_filtered,
        3118,
        "the fixture is main's 3118 lines"
    );
    assert!(
        mismatches.is_empty(),
        "{} mismatches:\n{}",
        mismatches.len(),
        mismatches[..mismatches.len().min(20)].join("\n")
    );
}

#[test]
fn the_deviation_predicates_catch_their_cases_and_spare_plain_ones() {
    assert!(d1_lists("- a") && d1_lists("x\n  * a") && d1_lists("**1. a**"));
    assert!(!d1_lists("x - a") && !d1_lists("1234567890. a") && !d1_lists("-a"));
    assert!(d2_ids("<@00000000000000000>") && d2_ids("<@18446744073709551616>"));
    assert!(d2_ids("</:12345678901234567>") && !d2_ids("<@18446744073709551615>"));
    assert!(d3_timestamps("<t:99999999999999999:f>") && d3_timestamps("<t:-8640000000001>"));
    assert!(!d3_timestamps("<t:8640000000000:D> <t:-1:f>"));
    let link = |target: &str| json!([{"type": "link", "target": target, "content": []}]);
    assert!(d4_links("", &link("t:1")) && d4_links("", &link("https://u@x.y/")));
    assert!(!d4_links("", &link("https://x.y/")));
    let label = json!([{"type": "link", "target": "https://c.d/",
        "content": [{"type": "url", "target": "https://a.b/", "content": []}]}]);
    assert!(d7_labels(&label) && !d7_labels(&link("https://x.y/")));
}

// ---- Rule table (one test per row) ----

#[test]
fn subtext_needs_line_start_and_consumes_its_newline() {
    assert_eq!(
        parse("-# small\nnext"),
        vec![Block::Subtext(vec![t("small")]), para(vec![t("next")])]
    );
    assert_eq!(parse("a -# small"), one(vec![t("a -# small")]));
    assert_eq!(parse("-#small"), one(vec![t("-#small")]));
}

#[test]
fn headings_are_levels_one_to_three_with_trimmed_content() {
    let h = |level, s: &str| Block::Heading {
        level,
        content: vec![t(s)],
    };
    assert_eq!(parse("# a  "), vec![h(1, "a")]);
    assert_eq!(parse("## b"), vec![h(2, "b")]);
    assert_eq!(parse("### c"), vec![h(3, "c")]);
    assert_eq!(parse("#### d"), one(vec![t("#### d")]));
    assert_eq!(parse("#e"), one(vec![t("#e")]));
    assert_eq!(parse("x # e"), one(vec![t("x # e")]));
}

#[test]
fn spoilers_take_the_first_closing_pair() {
    assert_eq!(
        parse("||a|| b ||c||"),
        one(vec![
            Inline::Spoiler(vec![t("a")]),
            t(" b "),
            Inline::Spoiler(vec![t("c")])
        ])
    );
    assert_eq!(parse("||||"), one(vec![t("||||")]));
}

#[test]
fn code_blocks_keep_a_short_language_and_their_content() {
    let code = |language: Option<&str>, code: &str| Block::CodeBlock {
        language: language.map(str::to_owned),
        code: code.to_owned(),
    };
    assert_eq!(
        parse("```py\nprint(1)\n```"),
        vec![code(Some("py"), "print(1)")]
    );
    assert_eq!(parse("```x```"), vec![code(None, "x")]);
    // The regex's backtracking: no newline after the word, so it is content, not a language.
    assert_eq!(parse("```js\n```"), vec![code(None, "js")]);
    let long = "a".repeat(33);
    assert_eq!(
        parse(&format!("```{long}\nx\n```")),
        vec![code(None, "x")],
        "a language over 32 characters is dropped"
    );
    assert_eq!(
        parse("a\n```x```\nb"),
        vec![para(vec![t("a")]), code(None, "x"), para(vec![t("b")])]
    );
}

#[test]
fn quotes_take_marked_lines_or_the_rest_and_never_nest() {
    let q = |content: Vec<Block>| Block::Quote(content);
    assert_eq!(
        parse("> a\n> b\nc"),
        vec![
            q(vec![para(vec![
                t("a"),
                Inline::LineBreak,
                t("b"),
                Inline::LineBreak
            ])]),
            para(vec![t("c")])
        ]
    );
    assert_eq!(
        parse(">>> a\nb"),
        vec![q(vec![para(vec![t("a"), Inline::LineBreak, t("b")])])]
    );
    assert_eq!(parse(">a"), one(vec![t(">a")]));
    assert_eq!(parse("a > b"), one(vec![t("a > b")]));
    assert_eq!(parse("> > x"), vec![q(vec![para(vec![t("> x")])])]);
}

#[test]
fn escapes_make_punctuation_literal_but_not_letters() {
    assert_eq!(parse("\\*a\\*"), one(vec![t("*a*")]));
    assert_eq!(parse("\\a"), one(vec![t("\\a")]));
}

#[test]
fn autolinks_and_plain_urls_are_canonical_unmasked_links() {
    let plain = link(
        "https://example.com/a",
        "example.com",
        vec![t("https://Example.com/a")],
        false,
    );
    assert_eq!(
        parse("<https://Example.com/a b>"),
        one(vec![t("<"), plain, t(" b>")]),
        "a space ends the autolink; the url rule takes the bare URL"
    );
    assert_eq!(
        parse("<https://Example.com/a>"),
        one(vec![link(
            "https://example.com/a",
            "example.com",
            vec![t("https://Example.com/a")],
            false
        )])
    );
    assert_eq!(
        parse("see https://x.y/a)."),
        one(vec![
            t("see "),
            link("https://x.y/a", "x.y", vec![t("https://x.y/a")], false),
            t(").")
        ])
    );
}

#[test]
fn masked_links_parse_the_label_and_canonicalise_the_target() {
    assert_eq!(
        parse("[**a**](https://例え.jp/x)"),
        one(vec![link(
            "https://xn--r8jz45g.jp/x",
            "xn--r8jz45g.jp",
            vec![Inline::Bold(vec![t("a")])],
            true
        )])
    );
    assert_eq!(
        parse("[a](<https://x.y> \"title\")"),
        one(vec![link("https://x.y/", "x.y", vec![t("a")], true)])
    );
}

#[test]
fn emphasis_picks_the_longest_capture_and_em_wins_ties() {
    assert_eq!(parse("*a*"), one(vec![Inline::Italic(vec![t("a")])]));
    assert_eq!(parse("_a_"), one(vec![Inline::Italic(vec![t("a")])]));
    assert_eq!(parse("**a**"), one(vec![Inline::Bold(vec![t("a")])]));
    assert_eq!(parse("__a__"), one(vec![Inline::Underline(vec![t("a")])]));
    assert_eq!(
        parse("***a***"),
        one(vec![Inline::Italic(vec![Inline::Bold(vec![t("a")])])])
    );
    // `_` closes only before a non-word character; an em inside an em yields its content.
    assert_eq!(parse("snake_case_name"), one(vec![t("snake_case_name")]));
    assert_eq!(
        parse("*a _b_ c*"),
        one(vec![Inline::Italic(vec![t("a b c")])])
    );
    assert_eq!(parse("*a *"), one(vec![t("*a *")]));
}

#[test]
fn tokens_parse_with_their_id_and_name_grammars() {
    let id = 123_456_789_012_345_678_u64;
    assert_eq!(
        parse("<@123456789012345678> <@!123456789012345678> <@&123456789012345678> <#123456789012345678>"),
        one(vec![
            Inline::UserMention(id),
            t(" "),
            Inline::UserMention(id),
            t(" "),
            Inline::RoleMention(id),
            t(" "),
            Inline::ChannelMention(id)
        ])
    );
    assert_eq!(
        parse("@everyone @here"),
        one(vec![Inline::Everyone, t(" "), Inline::Here])
    );
    assert_eq!(
        parse("@123456789012345678:customize>"),
        one(vec![t("@123456789012345678:customize>")]),
        "guild navigation needs its `<`"
    );
    assert_eq!(
        parse("<a:wave:123456789012345678><:ok:123456789012345678>"),
        one(vec![
            Inline::CustomEmoji {
                id,
                name: "wave".into(),
                animated: true
            },
            Inline::CustomEmoji {
                id,
                name: "ok".into(),
                animated: false
            }
        ])
    );
    assert_eq!(
        parse("</ping 設定 ก:123456789012345678>"),
        one(vec![Inline::SlashCommand {
            name: "ping 設定 ก".into(),
            id
        }])
    );
    assert_eq!(
        parse("<123456789012345678:customize><123456789012345678:linked-roles:123456789012345678>"),
        one(vec![
            Inline::GuildNavigation {
                guild: id,
                target: NavTarget::Customize
            },
            Inline::GuildNavigation {
                guild: id,
                target: NavTarget::LinkedRoles(Some(id))
            }
        ])
    );
    assert_eq!(
        parse("<t:1618953630:R><t:1618953630>"),
        one(vec![
            Inline::Timestamp {
                unix: 1_618_953_630,
                style: TimestampStyle::Relative
            },
            Inline::Timestamp {
                unix: 1_618_953_630,
                style: TimestampStyle::LongDateShortTime
            }
        ])
    );
}

#[test]
fn strikethrough_does_not_close_before_an_underscore() {
    assert_eq!(parse("~~a~~"), one(vec![Inline::Strike(vec![t("a")])]));
    assert_eq!(
        parse("~~a~~_b~~"),
        one(vec![Inline::Strike(vec![t("a~~_b")])])
    );
}

#[test]
fn inline_code_matches_runs_of_equal_length_and_drops_padding() {
    assert_eq!(parse("`a`"), one(vec![Inline::Code("a".into())]));
    assert_eq!(parse("`` a`b ``"), one(vec![Inline::Code(" a`b ".into())]));
    assert_eq!(parse("`` `a` ``"), one(vec![Inline::Code("`a`".into())]));
    // The opening run gives back backticks until a closing run of that length exists.
    assert_eq!(parse("``a`"), one(vec![Inline::Code("`a".into())]));
    assert_eq!(parse("``a"), one(vec![t("``a")]));
}

#[test]
fn newlines_are_line_breaks_and_the_shrug_keeps_its_backslash() {
    assert_eq!(parse("a\nb"), one(vec![t("a"), Inline::LineBreak, t("b")]));
    assert_eq!(parse("¯\\_(ツ)_/¯"), one(vec![t("¯\\_(ツ)_/¯")]));
}

#[test]
fn text_merges_and_stops_before_a_scheme() {
    assert_eq!(parse("字字 ab"), one(vec![t("字字 ab")]));
    assert_eq!(
        parse("xhttps://x.y"),
        one(vec![
            t("x"),
            link("https://x.y/", "x.y", vec![t("https://x.y")], false)
        ])
    );
}

// ---- Paired precedence ----

#[test]
fn spoiler_comes_before_strong() {
    assert_eq!(
        parse("||**a||**"),
        one(vec![Inline::Spoiler(vec![t("**a")]), t("**")])
    );
}

#[test]
fn a_link_nests_inside_strong() {
    assert_eq!(
        parse("**[x](https://a.b)**"),
        one(vec![Inline::Bold(vec![link(
            "https://a.b/",
            "a.b",
            vec![t("x")],
            true
        )])])
    );
}

#[test]
fn a_heading_matches_inside_a_quote() {
    assert_eq!(
        parse("> # h"),
        vec![Block::Quote(vec![Block::Heading {
            level: 1,
            content: vec![t("h")]
        }])]
    );
}

#[test]
fn star_space_at_line_start_is_a_list_and_star_word_star_is_italic() {
    assert_eq!(
        parse("* a"),
        vec![list(false, 0, vec![vec![para(vec![t("a")])]])]
    );
    assert_eq!(parse("*a*"), one(vec![Inline::Italic(vec![t("a")])]));
}

#[test]
fn a_timestamp_is_not_an_autolink() {
    assert_eq!(
        parse("<t:1:R>"),
        one(vec![Inline::Timestamp {
            unix: 1,
            style: TimestampStyle::Relative
        }])
    );
}

#[test]
fn dash_hash_is_subtext_and_dash_space_is_a_list() {
    assert_eq!(parse("-# x"), vec![Block::Subtext(vec![t("x")])]);
    assert_eq!(
        parse("- x"),
        vec![list(false, 0, vec![vec![para(vec![t("x")])]])]
    );
}

// ---- Blocks inside inline containers ----

fn code_x() -> Block {
    Block::CodeBlock {
        language: None,
        code: "x".into(),
    }
}

#[test]
fn a_code_block_stays_inside_its_spoiler() {
    assert_eq!(
        parse("||```x```||"),
        one(vec![Inline::Spoiler(vec![Inline::Block(Box::new(
            code_x()
        ))])])
    );
}

#[test]
fn a_heading_matches_at_the_start_of_bold_at_line_start() {
    assert_eq!(
        parse("**# h**"),
        one(vec![Inline::Bold(vec![Inline::Block(Box::new(
            Block::Heading {
                level: 1,
                content: vec![t("h")]
            }
        ))])])
    );
}

#[test]
fn a_quote_stays_inside_its_spoiler() {
    assert_eq!(
        parse("||> q||"),
        one(vec![Inline::Spoiler(vec![Inline::Block(Box::new(
            Block::Quote(vec![para(vec![t("q")])])
        ))])])
    );
}

#[test]
fn a_spoilered_code_block_stays_inside_its_list_item() {
    assert_eq!(
        parse("- ||```x```||"),
        vec![list(
            false,
            0,
            vec![vec![para(vec![Inline::Spoiler(vec![Inline::Block(
                Box::new(code_x())
            )])])]]
        )]
    );
}

// ---- Block grouping ----

#[test]
fn the_stream_groups_into_paragraphs_around_blocks() {
    let stream = flat("a\n# h\nb", true).unwrap();
    assert_eq!(
        stream,
        vec![
            Node::Text("a".into()),
            Node::Br,
            Node::Heading {
                level: 1,
                content: vec![Node::Text("h".into())]
            },
            Node::Text("b".into())
        ]
    );
    assert_eq!(
        blocks(stream),
        vec![
            para(vec![t("a")]),
            Block::Heading {
                level: 1,
                content: vec![t("h")]
            },
            para(vec![t("b")])
        ]
    );
}

#[test]
fn a_quoted_quote_marker_and_a_bad_emoji_stay_text() {
    assert_eq!(
        parse("> > x"),
        vec![Block::Quote(vec![para(vec![t("> x")])])]
    );
    assert_eq!(parse("<:n:notdigits>"), one(vec![t("<:n:notdigits>")]));
}

#[test]
fn a_line_break_beside_a_block_inside_a_container_is_dropped() {
    assert_eq!(
        parse("||```x```\nb||"),
        one(vec![Inline::Spoiler(vec![
            Inline::Block(Box::new(code_x())),
            t("b")
        ])])
    );
}

// ---- List grammar (D1), one test per clause ----

fn item(s: &str) -> Vec<Block> {
    vec![para(vec![t(s)])]
}

#[test]
fn list_markers_are_dash_star_plus_or_up_to_nine_digits_and_a_dot() {
    for marker in ["- a", "* a", "+ a"] {
        assert_eq!(
            parse(marker),
            vec![list(false, 0, vec![item("a")])],
            "{marker}"
        );
    }
    assert_eq!(parse("3. a"), vec![list(true, 3, vec![item("a")])]);
    assert_eq!(
        parse("999999999. a"),
        vec![list(true, 999_999_999, vec![item("a")])]
    );
    assert_eq!(parse("1234567890. a"), one(vec![t("1234567890. a")]));
    assert_eq!(parse("4294967295. a"), one(vec![t("4294967295. a")]));
}

#[test]
fn no_list_inside_heading_or_subtext_content() {
    let h = Block::Heading {
        level: 1,
        content: vec![t("- a")],
    };
    assert_eq!(parse("# - a"), vec![h]);
    assert_eq!(parse("-# - a"), vec![Block::Subtext(vec![t("- a")])]);
}

#[test]
fn a_list_marker_needs_a_space_after_it() {
    assert_eq!(parse("-a"), one(vec![t("-a")]));
}

#[test]
fn a_list_starts_only_at_line_start_after_up_to_eleven_spaces() {
    assert_eq!(parse("x - a"), one(vec![t("x - a")]));
    assert_eq!(
        parse("           - a"),
        vec![list(false, 0, vec![item("a")])]
    );
    assert_eq!(parse("            - a"), one(vec![t("            - a")]));
}

#[test]
fn a_deeper_marker_starts_a_nested_list() {
    assert_eq!(
        parse("- a\n  - b"),
        vec![list(
            false,
            0,
            vec![vec![para(vec![t("a")]), list(false, 0, vec![item("b")])]]
        )]
    );
}

#[test]
fn an_ordered_list_starts_at_its_first_number() {
    assert_eq!(
        parse("3. x\n4. y"),
        vec![list(true, 3, vec![item("x"), item("y")])]
    );
}

#[test]
fn a_line_indented_to_the_content_column_continues_the_item() {
    assert_eq!(
        parse("- a\n  b"),
        vec![list(
            false,
            0,
            vec![vec![para(vec![t("a"), Inline::LineBreak, t("b")])]]
        )]
    );
    assert_eq!(
        parse("- a\nb"),
        vec![list(false, 0, vec![item("a")]), para(vec![t("b")])]
    );
}

#[test]
fn a_blank_line_ends_the_list() {
    assert_eq!(
        parse("- a\n\n- b"),
        vec![
            list(false, 0, vec![item("a")]),
            list(false, 0, vec![item("b")])
        ]
    );
}

#[test]
fn list_items_hold_inline_rules_and_nested_lists_only() {
    assert_eq!(parse("- # h"), vec![list(false, 0, vec![item("# h")])]);
    assert_eq!(parse("- > q"), vec![list(false, 0, vec![item("> q")])]);
    assert_eq!(parse("- -# s"), vec![list(false, 0, vec![item("-# s")])]);
    assert_eq!(
        parse("- - a"),
        vec![list(false, 0, vec![vec![list(false, 0, vec![item("a")])]])],
        "a nested marker at the item's start nests"
    );
    assert_eq!(
        parse("- *a*"),
        vec![list(
            false,
            0,
            vec![vec![para(vec![Inline::Italic(vec![t("a")])])]]
        )]
    );
}

/// Readings of the D1 table where it is silent (reported to main for a ruling; K0 may narrow).
#[test]
fn list_cases_the_grammar_table_leaves_open() {
    let ul = |items| list(false, 0, items);
    assert_eq!(
        parse("a\n- b"),
        vec![para(vec![t("a")]), ul(vec![item("b")])]
    );
    assert_eq!(parse("- a\n- b"), vec![ul(vec![item("a"), item("b")])]);
    assert_eq!(
        parse("- a\n* b"),
        vec![ul(vec![item("a"), item("b")])],
        "bullets may differ"
    );
    assert_eq!(
        parse("- a\n1. b"),
        vec![ul(vec![item("a")]), list(true, 1, vec![item("b")])],
        "an ordered marker ends a bullet list"
    );
    assert_eq!(
        parse("  - a\n- b"),
        vec![ul(vec![item("a")]), ul(vec![item("b")])],
        "a shallower marker ends the list"
    );
    assert_eq!(
        parse("- a\n    b"),
        vec![ul(vec![vec![para(vec![
            t("a"),
            Inline::LineBreak,
            t("  b")
        ])]])],
        "indentation past the content column stays"
    );
    assert_eq!(parse("- "), vec![ul(vec![vec![]])], "an empty item");
    assert_eq!(
        parse("> - a"),
        vec![Block::Quote(vec![ul(vec![item("a")])])]
    );
    assert_eq!(
        parse("||- a||"),
        one(vec![Inline::Spoiler(vec![Inline::Block(Box::new(ul(
            vec![item("a")]
        )))])])
    );
}

// ---- Tokens and numbers (P1-3, P3-2) ----

#[test]
fn ids_must_be_nonzero_u64() {
    assert_eq!(
        parse("<@99999999999999999999999>"),
        one(vec![t("<@99999999999999999999999>")])
    );
    assert_eq!(
        parse("<@18446744073709551616>"),
        one(vec![t("<@18446744073709551616>")])
    );
    assert_eq!(
        parse("<@00000000000000000>"),
        one(vec![t("<@00000000000000000>")])
    );
    assert_eq!(
        parse("<@18446744073709551615>"),
        one(vec![Inline::UserMention(u64::MAX)])
    );
    assert_eq!(
        parse("<#1234567890123456>"),
        one(vec![t("<#1234567890123456>")])
    );
    assert_eq!(
        parse("<:x:123456789012345678>"),
        one(vec![t("<:x:123456789012345678>")]),
        "emoji names have 2-32 characters"
    );
}

#[test]
fn slash_commands_need_one_to_three_named_words() {
    assert_eq!(
        parse("</:123456789012345678>"),
        one(vec![t("</:123456789012345678>")])
    );
    assert_eq!(
        parse("</a b c d:123456789012345678>"),
        one(vec![t("</a b c d:123456789012345678>")])
    );
    let long = "a".repeat(33);
    let source = format!("</{long}:123456789012345678>");
    assert_eq!(parse(&source), one(vec![t(&source)]));
    assert!(!slash_name_char('\u{345}') && slash_name_char('\u{93c}') && slash_name_char('é'));
}

#[test]
fn timestamps_stay_within_the_js_date_range() {
    assert_eq!(
        parse("<t:9223372036854775807>"),
        one(vec![t("<t:9223372036854775807>")])
    );
    assert_eq!(
        parse("<t:99999999999999999:f>"),
        one(vec![t("<t:99999999999999999:f>")])
    );
    assert_eq!(
        parse("<t:8640000000001>"),
        one(vec![t("<t:8640000000001>")])
    );
    let ts = |unix, style| Inline::Timestamp { unix, style };
    assert_eq!(
        parse("<t:8640000000000:D><t:-8640000000000:s><t:-1>"),
        one(vec![
            ts(8_640_000_000_000, TimestampStyle::LongDate),
            ts(-8_640_000_000_000, TimestampStyle::ShortDateShortTime),
            ts(-1, TimestampStyle::LongDateShortTime)
        ])
    );
    assert_eq!(parse("<t:1:x>"), one(vec![t("<t:1:x>")]));
}

// ---- Links (D4, D7) ----

#[test]
fn only_valid_http_targets_become_links() {
    for source in ["[a](file:///etc)", "[a](javascript:x)", "[a](t:1)"] {
        assert_eq!(parse(source), one(vec![t(source)]), "{source}");
    }
    assert_eq!(parse("<ftp://x/y>"), one(vec![t("<ftp://x/y>")]));
    for source in [
        "[a](https://user@x.y)",
        "[a](https://x.y/\u{1}z)",
        "[a](https://x.y/\u{200b}z)",
        "[a](https://x.y/\u{85}z)",
        "[a](https://[x)",
        "https://user:pw@x.y",
    ] {
        assert!(
            !format!("{:?}", parse(source)).contains("Link"),
            "{source:?} must not be a link"
        );
    }
}

#[test]
fn a_rejected_link_falls_through_to_the_later_rules() {
    assert_eq!(
        parse("[*a*](t:1)"),
        one(vec![t("["), Inline::Italic(vec![t("a")]), t("](t:1)")])
    );
}

#[test]
fn link_labels_hold_no_links_or_blocks() {
    assert_eq!(
        parse("[https://a.b](https://c.d)"),
        one(vec![link(
            "https://c.d/",
            "c.d",
            vec![t("https://a.b")],
            true
        )])
    );
    assert_eq!(
        parse("[> a](https://x.y)"),
        one(vec![link("https://x.y/", "x.y", vec![t("> a")], true)])
    );
    for (source, label) in [
        ("[# a](https://x.y)", "# a"),
        ("[-# a](https://x.y)", "-# a"),
        ("[- a](https://x.y)", "- a"),
        ("[<https://a.b>](https://x.y)", "<https://a.b>"),
        ("[[a](https://a.b)](https://x.y)", "[a](https://a.b)"),
    ] {
        assert_eq!(
            parse(source),
            one(vec![link("https://x.y/", "x.y", vec![t(label)], true)]),
            "{source}"
        );
    }
    // No code block in a label; the backticks are then inline code.
    assert_eq!(
        parse("[```x```](https://x.y)"),
        one(vec![link(
            "https://x.y/",
            "x.y",
            vec![Inline::Code("x".into())],
            true
        )])
    );
}

// ---- Limits and fallback (P1-1, P3-3) ----

/// No `Text` holding `secret` outside a spoiler.
fn secret_hidden(blocks: &[Block]) -> bool {
    fn inline_ok(i: &Inline) -> bool {
        match i {
            Inline::Text(s) => !s.contains("secret"),
            Inline::Spoiler(_) => true,
            Inline::Bold(c) | Inline::Italic(c) | Inline::Underline(c) | Inline::Strike(c) => {
                c.iter().all(inline_ok)
            }
            Inline::Link { label, .. } => label.iter().all(inline_ok),
            Inline::Block(b) => secret_hidden(std::slice::from_ref(b)),
            Inline::Code(s) => !s.contains("secret"),
            _ => true,
        }
    }
    blocks.iter().all(|b| match b {
        Block::Paragraph(c) | Block::Subtext(c) | Block::Heading { content: c, .. } => {
            c.iter().all(inline_ok)
        }
        Block::Quote(c) => secret_hidden(c),
        Block::List { items, .. } => items.iter().all(|i| secret_hidden(i)),
        Block::CodeBlock { code, .. } => !code.contains("secret"),
    })
}

#[test]
fn a_long_cjk_message_still_parses() {
    let source = format!("{}**b**{}", "字".repeat(2000), "字".repeat(1995));
    assert_eq!(source.chars().count(), 4000);
    let blocks = parse(&source);
    assert_eq!(
        blocks,
        one(vec![
            t(&"字".repeat(2000)),
            Inline::Bold(vec![t("b")]),
            t(&"字".repeat(1995))
        ])
    );
}

#[test]
fn over_limit_messages_fall_back_without_revealing_spoilers() {
    // Each `# ` opens one more nested parse (a heading's content inherits line start).
    let deep = format!("{}||secret||", "# ".repeat(MAX_DEPTH + 1));
    let long = format!("||secret|| {}", "a".repeat(MAX_INPUT));
    // Two nodes per `a\n` line: past the node cap while well under the input cap.
    let wide = format!("||secret|| {}", "a\n".repeat(MAX_NODES / 2 + 1));
    assert!(
        wide.len() < MAX_INPUT,
        "wide must hit the node limit, not the input limit"
    );
    for source in [&deep, &long, &wide] {
        assert!(flat(source, true).is_none(), "the limit applies");
        let blocks = parse(source);
        assert!(secret_hidden(&blocks), "{blocks:?}");
        assert_eq!(blocks, one(vec![Inline::Spoiler(vec![t(source)])]));
    }
    assert_eq!(
        parse(&"a".repeat(MAX_INPUT + 1)),
        one(vec![t(&"a".repeat(MAX_INPUT + 1))])
    );
    // Exactly the node cap still parses.
    assert!(flat(&"a\n".repeat(MAX_NODES / 2), true).is_some());
    // The depth check runs before the recursive call: exactly 16 levels still parse.
    assert!(flat(&format!("{}a", "# ".repeat(MAX_DEPTH)), true).is_some());
    assert!(flat(&format!("{}a", "# ".repeat(MAX_DEPTH + 1)), true).is_none());
}

#[test]
fn preprocessing_normalises_newlines_tabs_and_form_feeds() {
    assert_eq!(
        parse("a\r\nb\rc"),
        one(vec![
            t("a"),
            Inline::LineBreak,
            t("b"),
            Inline::LineBreak,
            t("c")
        ])
    );
    assert_eq!(parse("\ta\u{c}"), one(vec![t("    a")]));
}

// ---- Linear time (P1-1) ----

/// The step bound: 64 matcher steps per input byte per nesting level.
const STEPS_PER_BYTE: u64 = 64;

/// Matcher steps and levels for one uncapped parse; `tick` panics as soon as the bound is passed.
fn measure(input: &str) -> (u64, usize) {
    let per_level = STEPS_PER_BYTE * input.len() as u64;
    STEPS.with(|s| {
        s.set(Steps {
            steps: 0,
            depth: 0,
            per_level,
        })
    });
    let _ = flat(input, false);
    let v = STEPS.with(|s| {
        s.replace(Steps {
            steps: 0,
            depth: 0,
            per_level: u64::MAX,
        })
    });
    (v.steps, v.depth + 1)
}

fn repeat_to(unit: &str, n: usize) -> String {
    let mut s = unit.repeat(n / unit.len() + 1);
    s.truncate(n);
    while !s.is_char_boundary(s.len()) {
        s.pop();
    }
    s
}

/// The plan's patterns at 1 MiB; each must stay within 64 steps per byte per nesting level.
fn step_patterns(n: usize) -> Vec<(String, String)> {
    let mut patterns: Vec<(String, String)> = [
        "*",
        "_",
        "**",
        "__",
        "~~",
        "||",
        "*a",
        "_a",
        "**a",
        "__a",
        "~~a",
        "||a",
        "*_~|",
        "[",
        "[a](",
        "<",
        "<t:",
        "<@",
        "\\",
        "# ",
        "a",
        "<https:/",
        "https://[",
        "[a",
    ]
    .iter()
    .map(|u| (format!("{u:?} repeated"), repeat_to(u, n)))
    .collect();
    let mut ticks = String::new();
    let mut k = 1;
    while ticks.len() < n {
        ticks.push_str(&"`".repeat(k));
        ticks.push('a');
        k += 1;
    }
    patterns.push(("backtick runs of increasing length".into(), ticks));
    patterns.push(("nested > lines".into(), repeat_to("> > > > x\n", n)));
    patterns.push(("# lines".into(), repeat_to("# h\n", n)));
    // Indentation cycling 8 deep (within the depth limit), and growing until the limit cuts in.
    for (name, cycle) in [
        ("nested list indentation", 8),
        ("list nesting past the limit", 40),
    ] {
        let mut lines = String::new();
        let mut depth = 0;
        while lines.len() < n {
            lines.push_str(&" ".repeat(2 * (depth % cycle)));
            lines.push_str("- a\n");
            depth += 1;
        }
        patterns.push((name.into(), lines));
    }
    patterns.push((
        "unclosed fence".into(),
        format!("```\n{}", repeat_to("a\n", n)),
    ));
    // The label memo: every `[`'s walk jumps to the one `]` and then crosses the same long tail.
    patterns.push((
        "[a repeated, then a long href".into(),
        format!("{}](https://x.y/{}", "[a".repeat(n / 4), "b".repeat(n / 2)),
    ));
    // The validation memo: every `[` reaches the same exit, whose long authority is rejected.
    patterns.push((
        "nested brackets, then a rejected target".into(),
        format!(
            "{}a{}(https://{}[)",
            "[".repeat(n / 4),
            "]".repeat(n / 4),
            "b".repeat(n / 2)
        ),
    ));
    patterns
}

#[test]
fn every_pattern_parses_within_the_step_bound_at_one_mebibyte() {
    let n = 1 << 20;
    for (name, input) in step_patterns(n) {
        let (steps, levels) = measure(&input);
        let bound = STEPS_PER_BYTE * input.len() as u64 * levels as u64;
        println!(
            "{name}: {steps} steps, {levels} levels, {:.1} per byte per level",
            steps as f64 / (input.len() * levels) as f64
        );
        assert!(
            steps <= bound,
            "{name}: {steps} steps > {bound} ({levels} levels)"
        );
    }
}

/// Main's ruling on K1's open decision 1 (docs/plans/markdown.md, K1 rulings): "line start" is the
/// previous capture ending in `\n`, as the plan states, not the oracle's check of the previous
/// capture's last regex group. No fixture input tells the two apart; these pin velella's side.
#[test]
fn line_start_follows_the_previous_capture_text_not_its_last_group() {
    let heading = |s: &str| Block::Heading {
        level: 1,
        content: vec![t(s)],
    };
    // After a quote (which consumes its newline) a heading still starts a line.
    assert!(parse("> a\n# h").ends_with(&[heading("h")]));
    // Inline code ending in a newline does not end with `\n` as text, so `# h` stays literal.
    assert!(!parse("`\n`# h")
        .iter()
        .any(|b| matches!(b, Block::Heading { .. })));
}
