//! Discord message markdown (docs/plans/markdown.md, slice K1). Untrusted input.
//!
//! A simple-markdown-style ordered rule engine run in inline mode over the whole message, as
//! Discord's parser is (discord-markdown-parser `parse(input, {inline: true})`). The rule order,
//! quality selection, previous-capture inheritance into nested parses, preprocessing and each
//! regex's backtracking result are adapted from simple-markdown (MIT) and discord-markdown-parser
//! (Apache-2.0); the limits and the spoiler-safe fallback follow Serein; see THIRD_PARTY_NOTICES.md.
//! markdown_tests.rs checks the result against that oracle's output for a fixed corpus.
//!
//! Deliberate deviations from the oracle (the plan's D1-D7): lists are parsed (D1); ids must be
//! nonzero `u64` and the nameless slash command stays text (D2); timestamps are bounded (D3);
//! links need a valid http(s) URL, and a rejected candidate falls through to the later rules (D4);
//! limits fall back (D5); link labels hold neither links nor blocks (D7). Line start is the plan's:
//! no previous capture, or one ending in `\n` (`\n` and spaces for quotes and lists).
//!
//! Linear time: every matcher is O(1) amortised per position of a nesting level, through
//! next-candidate tables built once per level and memos that persist across the attempts of one
//! level (the link rule's label walk, target and validation). Nesting is checked before each
//! recursive call. Nothing here logs.
use std::{collections::HashMap, ops::Range};

/// Content longer than this (bytes) falls back without parsing (4000 characters × 4).
const MAX_INPUT: usize = 16384;
/// Nested parses allowed below the top level; checked before each recursive call.
const MAX_DEPTH: usize = 16;
/// AST nodes allowed (adjacent text merged).
const MAX_NODES: usize = 8192;
/// `<…>` tokens look at most this many bytes ahead (a 3-word slash command of 32 four-byte
/// letters per word plus its id is 411).
const TOKEN_WINDOW: usize = 512;
/// Absent table entry.
const NONE: usize = usize::MAX;
/// Memo entry not computed yet (`NONE` = computed, no result).
const UNKNOWN: usize = usize::MAX - 1;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Block {
    Paragraph(Vec<Inline>),
    /// `level` is 1-3.
    Heading {
        level: u8,
        content: Vec<Inline>,
    },
    Subtext(Vec<Inline>),
    /// Never contains a `Quote`.
    Quote(Vec<Block>),
    List {
        ordered: bool,
        start: u32,
        items: Vec<Vec<Block>>,
    },
    CodeBlock {
        language: Option<String>,
        code: String,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Inline {
    Text(String),
    Bold(Vec<Inline>),
    Italic(Vec<Inline>),
    Underline(Vec<Inline>),
    Strike(Vec<Inline>),
    Spoiler(Vec<Inline>),
    Code(String),
    /// `url` is canonical (`url::Url::as_str`), `host` its ASCII host; `label` holds no `Link`.
    Link {
        url: String,
        host: String,
        label: Vec<Inline>,
        masked: bool,
    },
    UserMention(u64),
    RoleMention(u64),
    ChannelMention(u64),
    Everyone,
    Here,
    CustomEmoji {
        id: u64,
        name: String,
        animated: bool,
    },
    Timestamp {
        unix: i64,
        style: TimestampStyle,
    },
    SlashCommand {
        name: String,
        id: u64,
    },
    GuildNavigation {
        guild: u64,
        target: NavTarget,
    },
    LineBreak,
    /// A block that matched inside an inline container; it stays in place (never hoisted out).
    Block(Box<Block>),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TimestampStyle {
    /// `t`
    ShortTime,
    /// `T`
    MediumTime,
    /// `d`
    ShortDate,
    /// `D`
    LongDate,
    /// `f`, also the default
    LongDateShortTime,
    /// `F`
    FullDateShortTime,
    /// `s`
    ShortDateShortTime,
    /// `S`
    ShortDateMediumTime,
    /// `R`
    Relative,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NavTarget {
    Customize,
    Browse,
    Guide,
    LinkedRoles(Option<u64>),
}

/// Parses message content. Over a limit, the whole content is one paragraph of text, inside one
/// spoiler if it contains `||`, so a limit never reveals spoiler text.
pub fn parse(content: &str) -> Vec<Block> {
    match flat(content, true) {
        Some(nodes) => blocks(nodes),
        None => fallback(content),
    }
}

fn fallback(content: &str) -> Vec<Block> {
    let text = Inline::Text(content.to_owned());
    let inline = if content.contains("||") {
        Inline::Spoiler(vec![text])
    } else {
        text
    };
    vec![Block::Paragraph(vec![inline])]
}

/// The pre-grouping stream (`None` when a limit is hit). `capped: false` lifts the input and node
/// limits (the step-bound test); the depth limit always holds.
fn flat(content: &str, capped: bool) -> Option<Vec<Node>> {
    if capped && content.len() > MAX_INPUT {
        return None;
    }
    let src = preprocess(content);
    let mut ctx = Ctx { capped, nodes: 0 };
    parse_level(&src, State::TOP, &mut ctx).ok()
}

/// simple-markdown's `preprocess`: `\r\n` and `\r` → `\n`, form feeds dropped, tab → 4 spaces.
fn preprocess(s: &str) -> String {
    s.replace("\r\n", "\n")
        .replace('\r', "\n")
        .replace('\u{c}', "")
        .replace('\t', "    ")
}

// ---- Pre-grouping stream ----

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum LinkKind {
    /// `<https://…>`
    Auto,
    /// A bare `https://…`
    Url,
    /// `[label](target)`
    Masked,
}

/// One node of the flat stream: the oracle's tree shape, before block grouping.
#[derive(Clone, Debug, PartialEq, Eq)]
enum Node {
    Text(String),
    Em(Vec<Node>),
    Strong(Vec<Node>),
    Underline(Vec<Node>),
    Strike(Vec<Node>),
    Spoiler(Vec<Node>),
    Code(String),
    Link {
        kind: LinkKind,
        url: String,
        host: String,
        label: Vec<Node>,
    },
    /// Mentions, emoji, timestamps, slash commands, guild navigation, @everyone/@here.
    Token(Inline),
    Br,
    Heading {
        level: u8,
        content: Vec<Node>,
    },
    Subtext(Vec<Node>),
    Quote(Vec<Node>),
    CodeBlock {
        language: Option<String>,
        code: String,
    },
    List {
        ordered: bool,
        start: u32,
        items: Vec<Vec<Node>>,
    },
}

fn is_block(n: &Node) -> bool {
    matches!(
        n,
        Node::Heading { .. }
            | Node::Subtext(_)
            | Node::Quote(_)
            | Node::CodeBlock { .. }
            | Node::List { .. }
    )
}

// ---- Block grouping ----

/// Drops a line break directly before or after a block (a heading, subtext or quote has already
/// consumed its own newline; a code block's following newline arrives as a break).
fn drop_breaks(nodes: Vec<Node>) -> Vec<Node> {
    let block_at = |i: Option<usize>| i.and_then(|i| nodes.get(i)).is_some_and(is_block);
    let keep: Vec<bool> = (0..nodes.len())
        .map(|i| {
            !(matches!(nodes[i], Node::Br) && (block_at(i.checked_sub(1)) || block_at(Some(i + 1))))
        })
        .collect();
    nodes
        .into_iter()
        .zip(keep)
        .filter_map(|(n, k)| k.then_some(n))
        .collect()
}

/// A block-list level (top level, quote content, list item): runs of inline nodes become
/// paragraphs.
fn blocks(nodes: Vec<Node>) -> Vec<Block> {
    let mut out = Vec::new();
    let mut run = Vec::new();
    for n in drop_breaks(nodes) {
        if is_block(&n) {
            if !run.is_empty() {
                out.push(Block::Paragraph(std::mem::take(&mut run)));
            }
            out.push(block(n));
        } else {
            run.push(inline(n));
        }
    }
    if !run.is_empty() {
        out.push(Block::Paragraph(run));
    }
    out
}

/// An inline container's content: a block keeps its position as `Inline::Block`.
fn inlines(nodes: Vec<Node>) -> Vec<Inline> {
    drop_breaks(nodes).into_iter().map(inline).collect()
}

fn block(n: Node) -> Block {
    match n {
        Node::Heading { level, content } => Block::Heading {
            level,
            content: inlines(content),
        },
        Node::Subtext(c) => Block::Subtext(inlines(c)),
        Node::Quote(c) => Block::Quote(blocks(c)),
        Node::CodeBlock { language, code } => Block::CodeBlock { language, code },
        Node::List {
            ordered,
            start,
            items,
        } => Block::List {
            ordered,
            start,
            items: items.into_iter().map(blocks).collect(),
        },
        other => Block::Paragraph(vec![inline(other)]),
    }
}

fn inline(n: Node) -> Inline {
    match n {
        Node::Text(t) => Inline::Text(t),
        Node::Em(c) => Inline::Italic(inlines(c)),
        Node::Strong(c) => Inline::Bold(inlines(c)),
        Node::Underline(c) => Inline::Underline(inlines(c)),
        Node::Strike(c) => Inline::Strike(inlines(c)),
        Node::Spoiler(c) => Inline::Spoiler(inlines(c)),
        Node::Code(c) => Inline::Code(c),
        Node::Link {
            kind,
            url,
            host,
            label,
        } => Inline::Link {
            url,
            host,
            label: inlines(label),
            masked: kind == LinkKind::Masked,
        },
        Node::Token(t) => t,
        Node::Br => Inline::LineBreak,
        b => Inline::Block(Box::new(block(b))),
    }
}

// ---- Engine ----

struct Abort;

struct Ctx {
    capped: bool,
    nodes: usize,
}

/// Parse state, copied into nested parses (which therefore inherit the previous capture).
#[derive(Clone, Copy)]
struct State {
    depth: usize,
    in_quote: bool,
    in_emphasis: bool,
    /// Inside a link label: no links and no blocks (D7).
    in_link: bool,
    /// Inside a list item: no heading, subtext or quote.
    in_list: bool,
    /// Inside heading or subtext content: no list.
    in_heading: bool,
    /// No previous capture, or it ends in `\n` (heading, subtext).
    line_start: bool,
    /// No previous capture, or it ends in `\n` and spaces (quote, list).
    quote_start: bool,
}

impl State {
    const TOP: State = State {
        depth: 0,
        in_quote: false,
        in_emphasis: false,
        in_link: false,
        in_list: false,
        in_heading: false,
        line_start: true,
        quote_start: true,
    };
}

/// Step-bound bookkeeping for markdown_tests.rs (test builds only).
#[cfg(test)]
#[derive(Clone, Copy)]
struct Steps {
    steps: u64,
    /// Deepest nested level reached.
    depth: usize,
    /// Steps allowed per level; past `per_level × levels` the parse panics, so a quadratic
    /// mutant fails at once instead of running for hours.
    per_level: u64,
}

#[cfg(test)]
thread_local! {
    static STEPS: std::cell::Cell<Steps> = const {
        std::cell::Cell::new(Steps { steps: 0, depth: 0, per_level: u64::MAX })
    };
}

/// Counts matcher steps (test builds only).
#[inline(always)]
fn tick(_n: usize) {
    #[cfg(test)]
    STEPS.with(|s| {
        let mut v = s.get();
        v.steps += _n as u64;
        s.set(v);
        let bound = v.per_level.saturating_mul(v.depth as u64 + 1);
        assert!(
            v.steps <= bound,
            "step bound exceeded: {} > {bound}",
            v.steps
        );
    });
}

/// Appends a node, merging adjacent text; counts it against the node limit.
fn push(out: &mut Vec<Node>, node: Node, ctx: &mut Ctx) -> Result<(), Abort> {
    if let Node::Text(t) = &node {
        if let Some(Node::Text(last)) = out.last_mut() {
            last.push_str(t);
            return Ok(());
        }
    }
    ctx.nodes += 1;
    if ctx.capped && ctx.nodes > MAX_NODES {
        return Err(Abort);
    }
    out.push(node);
    Ok(())
}

/// A nested parse: depth is checked before recursing.
fn nested(s: &str, st: State, ctx: &mut Ctx) -> Result<Vec<Node>, Abort> {
    let depth = st.depth + 1;
    if depth > MAX_DEPTH {
        return Err(Abort);
    }
    #[cfg(test)]
    STEPS.with(|c| {
        let mut v = c.get();
        v.depth = v.depth.max(depth);
        c.set(v);
    });
    parse_level(s, State { depth, ..st }, ctx)
}

fn parse_level(s: &str, mut st: State, ctx: &mut Ctx) -> Result<Vec<Node>, Abort> {
    let mut lv = Level::new(s);
    let mut out = Vec::new();
    let mut pos = 0;
    while pos < s.len() {
        tick(1);
        let end = rule(&mut lv, pos, &st, ctx, &mut out)?;
        let cap = &s[pos..end];
        st.line_start = cap.ends_with('\n');
        st.quote_start = cap.trim_end_matches(' ').ends_with('\n');
        pos = end;
    }
    Ok(out)
}

/// Tries the rules at `pos` in the oracle's order and applies the first match (the best quality
/// among em, strong and underline); returns the end of the capture.
fn rule(
    lv: &mut Level,
    pos: usize,
    st: &State,
    ctx: &mut Ctx,
    out: &mut Vec<Node>,
) -> Result<usize, Abort> {
    let s = lv.s;
    let b = lv.b;
    let c = b[pos];
    let blocks_ok = !st.in_link;

    // 1-2 (order -0.5, 0): subtext, heading.
    if blocks_ok && st.line_start && !st.in_list {
        let head = State {
            in_heading: true,
            ..*st
        };
        if c == b'-' && b[pos..].starts_with(b"-#") {
            if let Some((end, content)) = marker_line(b, pos + 2) {
                tick(end - pos);
                let inner = nested(&s[content], head, ctx)?;
                push(out, Node::Subtext(inner), ctx)?;
                return Ok(end);
            }
        }
        if c == b'#' {
            let level = b[pos..].iter().take_while(|&&x| x == b'#').count();
            if level <= 3 {
                if let Some((end, content)) = marker_line(b, pos + level) {
                    tick(end - pos);
                    let inner = nested(js_trim(&s[content]), head, ctx)?;
                    push(
                        out,
                        Node::Heading {
                            level: level as u8,
                            content: inner,
                        },
                        ctx,
                    )?;
                    return Ok(end);
                }
            }
        }
    }
    // 3 (0): spoiler.
    if c == b'|' {
        if let Some((end, content)) = lv.spoiler(pos) {
            let inner = nested(&s[content], *st, ctx)?;
            push(out, Node::Spoiler(inner), ctx)?;
            return Ok(end);
        }
    }
    // 4: code block.
    if c == b'`' && blocks_ok {
        if let Some((end, language, code)) = lv.code_block(pos) {
            tick(end - pos);
            let code = s[code].to_owned();
            push(out, Node::CodeBlock { language, code }, ctx)?;
            return Ok(end);
        }
    }
    // 6: block quote.
    if (c == b' ' || c == b'>') && blocks_ok && st.quote_start && !st.in_quote && !st.in_list {
        if let Some((end, content)) = block_quote(s, pos) {
            tick(end - pos);
            let quote = State {
                in_quote: true,
                ..*st
            };
            let inner = nested(&content, quote, ctx)?;
            push(out, Node::Quote(inner), ctx)?;
            return Ok(end);
        }
    }
    // 7: list (velella, D1).
    if blocks_ok
        && st.quote_start
        && !st.in_heading
        && matches!(c, b' ' | b'-' | b'*' | b'0'..=b'9')
    {
        if let Some(list) = list(s, pos) {
            tick(list.end - pos);
            let item = State {
                in_list: true,
                line_start: true,
                quote_start: true,
                ..*st
            };
            let mut items = Vec::with_capacity(list.items.len());
            for content in &list.items {
                items.push(nested(content, item, ctx)?);
            }
            let node = Node::List {
                ordered: list.ordered,
                start: list.start,
                items,
            };
            push(out, node, ctx)?;
            return Ok(list.end);
        }
    }
    // 12: escape.
    if c == b'\\' {
        if let Some(ch) =
            char_at(s, pos + 1).filter(|ch| !ch.is_ascii_alphanumeric() && !is_js_space(*ch))
        {
            push(out, Node::Text(ch.to_string()), ctx)?;
            return Ok(pos + 1 + ch.len_utf8());
        }
    }
    // 14, 16, 17: autolink, url, link; never inside a label (D7). A candidate whose target fails
    // validation is not a link: the later rules run at the same position (D4).
    if !st.in_link {
        let found = match c {
            b'<' => lv
                .autolink(pos)
                .map(|(end, r, t)| (end, LinkKind::Auto, r, t)),
            b'h' => lv.url(pos).map(|(end, r, t)| (end, LinkKind::Url, r, t)),
            b'[' => lv.link(pos),
            _ => None,
        };
        if let Some((end, kind, label_src, (url, host))) = found {
            let label = match kind {
                LinkKind::Masked => {
                    let label = State {
                        in_link: true,
                        ..*st
                    };
                    nested(&s[label_src], label, ctx)?
                }
                _ => vec![Node::Text(s[label_src].to_owned())],
            };
            let node = Node::Link {
                kind,
                url,
                host,
                label,
            };
            push(out, node, ctx)?;
            return Ok(end);
        }
    }
    // 21 (quality): em, strong, underline. Longest capture wins; ties go em, strong, underline.
    if c == b'*' || c == b'_' {
        #[derive(Clone, Copy)]
        enum Kind {
            Em,
            Strong,
            Under,
        }
        let (em, strong, under) = if c == b'*' {
            (lv.em_star(pos), lv.strong(pos), None)
        } else {
            (lv.em_under(pos), None, lv.underline(pos))
        };
        let mut best: Option<(usize, u8, Kind, Range<usize>)> = None;
        for (cand, bonus, kind) in [
            (em, 2, Kind::Em),
            (strong, 1, Kind::Strong),
            (under, 0, Kind::Under),
        ] {
            if let Some((end, content)) = cand {
                if best
                    .as_ref()
                    .is_none_or(|(e, q, ..)| (end, bonus) > (*e, *q))
                {
                    best = Some((end, bonus, kind, content));
                }
            }
        }
        if let Some((end, _, kind, content)) = best {
            match kind {
                Kind::Em => {
                    let em = State {
                        in_emphasis: true,
                        ..*st
                    };
                    let inner = nested(&s[content], em, ctx)?;
                    if st.in_emphasis {
                        // An em inside an em yields its content only (rules/em.ts).
                        ctx.nodes -= inner.len();
                        for n in inner {
                            push(out, n, ctx)?;
                        }
                    } else {
                        push(out, Node::Em(inner), ctx)?;
                    }
                }
                Kind::Strong => {
                    let inner = nested(&s[content], *st, ctx)?;
                    push(out, Node::Strong(inner), ctx)?;
                }
                Kind::Under => {
                    let inner = nested(&s[content], *st, ctx)?;
                    push(out, Node::Underline(inner), ctx)?;
                }
            }
            return Ok(end);
        }
    }
    // 21: mentions and the other Discord tokens.
    if c == b'<' || c == b'@' {
        if let Some((end, tok)) = token(s, pos) {
            tick(end - pos);
            push(out, Node::Token(tok), ctx)?;
            return Ok(end);
        }
    }
    // 22: strikethrough.
    if c == b'~' {
        if let Some((end, content)) = lv.strike(pos) {
            let inner = nested(&s[content], *st, ctx)?;
            push(out, Node::Strike(inner), ctx)?;
            return Ok(end);
        }
    }
    // 23: inline code.
    if c == b'`' {
        if let Some((end, content)) = lv.inline_code(pos) {
            tick(end - pos);
            push(out, Node::Code(code_content(&s[content])), ctx)?;
            return Ok(end);
        }
    }
    // 24: line break.
    if c == b'\n' {
        push(out, Node::Br, ctx)?;
        return Ok(pos + 1);
    }
    // 25: emoticon, then text.
    const SHRUG: &str = "¯\\_(ツ)_/¯";
    if s[pos..].starts_with(SHRUG) {
        push(out, Node::Text(SHRUG.to_owned()), ctx)?;
        return Ok(pos + SHRUG.len());
    }
    let end = lv.text_end(pos);
    push(out, Node::Text(s[pos..end].to_owned()), ctx)?;
    Ok(end)
}

// ---- Characters ----

/// JavaScript's `\s` (and `String.prototype.trim`'s set).
fn is_js_space(c: char) -> bool {
    matches!(
        c,
        '\t'..='\r'
            | ' '
            | '\u{a0}'
            | '\u{1680}'
            | '\u{2000}'..='\u{200a}'
            | '\u{2028}'
            | '\u{2029}'
            | '\u{202f}'
            | '\u{205f}'
            | '\u{3000}'
            | '\u{feff}'
    )
}

/// JavaScript's `\w` without the `u` flag.
fn is_word(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_'
}

fn char_at(s: &str, i: usize) -> Option<char> {
    s.get(i..)?.chars().next()
}

/// Byte length of the UTF-8 character starting at `i` (`i` must be a char boundary).
fn clen(b: &[u8], i: usize) -> usize {
    match b[i] {
        0..=0x7f => 1,
        0xc0..=0xdf => 2,
        0xe0..=0xef => 3,
        _ => 4,
    }
}

fn space_at(s: &str, i: usize) -> bool {
    char_at(s, i).is_some_and(is_js_space)
}

fn js_trim(s: &str) -> &str {
    s.trim_matches(is_js_space)
}

/// Unicode general category Cf (format), Unicode 17 (generated from Node 24's `\p{Cf}`).
const FORMAT: &[(u32, u32)] = &[
    (0xAD, 0xAD),
    (0x600, 0x605),
    (0x61C, 0x61C),
    (0x6DD, 0x6DD),
    (0x70F, 0x70F),
    (0x890, 0x891),
    (0x8E2, 0x8E2),
    (0x180E, 0x180E),
    (0x200B, 0x200F),
    (0x202A, 0x202E),
    (0x2060, 0x2064),
    (0x2066, 0x206F),
    (0xFEFF, 0xFEFF),
    (0xFFF9, 0xFFFB),
    (0x110BD, 0x110BD),
    (0x110CD, 0x110CD),
    (0x13430, 0x1343F),
    (0x1BCA0, 0x1BCA3),
    (0x1D173, 0x1D17A),
    (0xE0001, 0xE0001),
    (0xE0020, 0xE007F),
];

fn in_ranges(table: &[(u32, u32)], c: char) -> bool {
    let c = c as u32;
    table
        .binary_search_by(|&(lo, hi)| {
            if hi < c {
                std::cmp::Ordering::Less
            } else if lo > c {
                std::cmp::Ordering::Greater
            } else {
                std::cmp::Ordering::Equal
            }
        })
        .is_ok()
}

/// C0, C1 or Cf: never allowed in a link target (D4).
fn forbidden_in_url(c: char) -> bool {
    matches!(c, '\u{0}'..='\u{1f}' | '\u{80}'..='\u{9f}') || in_ranges(FORMAT, c)
}

/// The oracle's slash-command name class `[-_'\p{L}\p{N}\p{sc=Deva}\p{sc=Thai}]`.
fn slash_name_char(c: char) -> bool {
    matches!(c, '-' | '_' | '\'') || in_ranges(SLASH_NAME, c)
}

// ---- Line rules ----

/// After a heading or subtext marker: ` +([^\n]+?)(\n|$)`. Returns (end, content).
fn marker_line(b: &[u8], from: usize) -> Option<(usize, Range<usize>)> {
    let i = from + b[from..].iter().take_while(|&&x| x == b' ').count();
    let spaces = i - from;
    if spaces == 0 {
        return None;
    }
    let eol = b[i..]
        .iter()
        .position(|&x| x == b'\n')
        .map_or(b.len(), |n| i + n);
    let content = if eol > i {
        i..eol
    } else if spaces >= 2 {
        // `[^\n]+?` needs a character: the spaces give their last one back.
        i - 1..eol
    } else {
        return None;
    };
    Some((if eol < b.len() { eol + 1 } else { eol }, content))
}

/// `BlockQuoteRegex`: ` *>>> ` takes the rest; otherwise consecutive ` *> ` lines (and the
/// newline after them). Returns (end, content with the markers removed).
fn block_quote(s: &str, pos: usize) -> Option<(usize, String)> {
    let b = s.as_bytes();
    let i = pos + b[pos..].iter().take_while(|&&x| x == b' ').count();
    if b[i..].starts_with(b">>> ") {
        return Some((s.len(), s[i + 4..].to_owned()));
    }
    if !b[i..].starts_with(b"> ") {
        return None;
    }
    let mut content = String::new();
    let mut from = i + 2;
    loop {
        let eol = b[from..]
            .iter()
            .position(|&x| x == b'\n')
            .map_or(b.len(), |n| from + n);
        content.push_str(&s[from..eol]);
        if eol == b.len() {
            return Some((eol, content));
        }
        content.push('\n');
        let j = eol + 1 + b[eol + 1..].iter().take_while(|&&x| x == b' ').count();
        if !b[j..].starts_with(b"> ") {
            return Some((eol + 1, content));
        }
        from = j + 2;
    }
}

struct ListMatch {
    end: usize,
    ordered: bool,
    start: u32,
    items: Vec<String>,
}

struct Marker {
    indent: usize,
    ordered: bool,
    number: u32,
    /// Column where the item's content starts (marker and its spaces included).
    content: usize,
}

/// Largest ordered-list number: the official client shows `1234567890.` as `1000000000.` (K0,
/// `refs/discord-ground-truth/captures/k0-edge.png`), and `start` plus any item count fits `u32`.
const MAX_LIST_NUMBER: u32 = 1_000_000_000;

/// A list item line: up to 11 spaces, `-`, `*` or one or more digits and `.`, then at least one
/// space (`+` is text in the official client, K0). The number saturates; leading zeros count.
fn list_marker(line: &[u8]) -> Option<Marker> {
    let indent = line.iter().take_while(|&&x| x == b' ').count();
    tick(indent + 1);
    if indent > 11 {
        return None;
    }
    let rest = &line[indent..];
    let (ordered, number, len) = match rest.first()? {
        b'-' | b'*' => (false, 0, 1),
        _ => {
            let digits = rest.iter().take_while(|x| x.is_ascii_digit()).count();
            tick(digits);
            if digits == 0 || rest.get(digits) != Some(&b'.') {
                return None;
            }
            let number = rest[..digits]
                .iter()
                .fold(0u32, |n, d| {
                    n.saturating_mul(10).saturating_add(u32::from(d - b'0'))
                })
                .min(MAX_LIST_NUMBER);
            (true, number, digits + 1)
        }
    };
    let spaces = rest[len..].iter().take_while(|&&x| x == b' ').count();
    tick(spaces);
    (spaces > 0).then_some(Marker {
        indent,
        ordered,
        number,
        content: indent + len + spaces,
    })
}

/// The list grammar (plan D1). Items continue on lines indented to their content column or
/// holding a deeper marker; a marker at the list's indent starts the next item, of either kind
/// (the list keeps its first item's kind: the official client continues `- b` with `1. a` as
/// bullets, K0); a blank line or any other line ends the list.
fn list(s: &str, pos: usize) -> Option<ListMatch> {
    let b = s.as_bytes();
    let line_end = |from: usize| {
        let end = b[from..]
            .iter()
            .position(|&x| x == b'\n')
            .map_or(b.len(), |n| from + n);
        tick(end - from + 1);
        end
    };
    let mut eol = line_end(pos);
    let first = list_marker(&b[pos..eol])?;
    let mut items = Vec::new();
    let mut item = s[pos + first.content..eol].to_owned();
    let mut column = first.content;
    let end = loop {
        if eol == b.len() {
            break eol;
        }
        let next = eol + 1;
        let next_eol = line_end(next);
        let line = &b[next..next_eol];
        if line.iter().all(|&x| x == b' ') {
            break eol + 1;
        }
        let indent = line.iter().take_while(|&&x| x == b' ').count();
        if let Some(m) = list_marker(line) {
            if m.indent == first.indent {
                items.push(std::mem::take(&mut item));
                item.push_str(&s[next + m.content..next_eol]);
                column = m.content;
                eol = next_eol;
                continue;
            }
            if m.indent < first.indent {
                break eol + 1;
            }
        } else if indent < column {
            break eol + 1;
        }
        item.push('\n');
        item.push_str(&s[next + indent.min(column)..next_eol]);
        eol = next_eol;
    };
    items.push(item);
    Some(ListMatch {
        end,
        ordered: first.ordered,
        start: first.number,
        items,
    })
}

// ---- Tokens ----

/// Inline code content: simple-markdown drops a space before a leading backtick and after a
/// trailing one (`INLINE_CODE_ESCAPE_BACKTICKS_R`).
fn code_content(c: &str) -> String {
    let lead = c.starts_with(' ') && c[1..].trim_start_matches(' ').starts_with('`');
    let trimmed = c.trim_end_matches(' ');
    let trail = trimmed.len() < c.len() && trimmed.ends_with('`');
    let start = usize::from(lead);
    let end = c.len() - usize::from(trail);
    c.get(start..end.max(start)).unwrap_or_default().to_owned()
}

/// `n` ASCII digits starting at `i`, with `min <= n <= max`, as a nonzero `u64` (D2).
fn id_at(t: &[u8], i: usize, min: usize, max: usize) -> Option<(u64, usize)> {
    let n = t
        .get(i..)?
        .iter()
        .take_while(|x| x.is_ascii_digit())
        .count();
    if n < min || n > max {
        return None;
    }
    let id: u64 = std::str::from_utf8(&t[i..i + n]).ok()?.parse().ok()?;
    (id != 0).then_some((id, n))
}

/// Discord tokens (discord-markdown-parser `utils/regex.ts`) with velella's id, slash-command and
/// timestamp grammars (D2, D3). Returns (end, token).
fn token(s: &str, pos: usize) -> Option<(usize, Inline)> {
    let b = s.as_bytes();
    let t = &b[pos..b.len().min(pos + TOKEN_WINDOW)];
    if t.starts_with(b"@everyone") {
        return Some((pos + 9, Inline::Everyone));
    }
    if t.starts_with(b"@here") {
        return Some((pos + 5, Inline::Here));
    }
    if t.first() != Some(&b'<') {
        return None;
    }
    let close = |i: usize, tok: Inline| (t.get(i) == Some(&b'>')).then_some((pos + i + 1, tok));
    if t.starts_with(b"<#") {
        let (id, n) = id_at(t, 2, 17, 20)?;
        return close(2 + n, Inline::ChannelMention(id));
    }
    if t.starts_with(b"<@&") {
        let (id, n) = id_at(t, 3, 17, 20)?;
        return close(3 + n, Inline::RoleMention(id));
    }
    if t.starts_with(b"<@") {
        let i = if t.get(2) == Some(&b'!') { 3 } else { 2 };
        let (id, n) = id_at(t, i, 17, 20)?;
        return close(i + n, Inline::UserMention(id));
    }
    if t.starts_with(b"<a:") || t.starts_with(b"<:") {
        let (animated, i) = if t[1] == b'a' { (true, 3) } else { (false, 2) };
        let name = t[i..].iter().take_while(|&&x| is_word(x)).count();
        if !(2..=32).contains(&name) || t.get(i + name) != Some(&b':') {
            return None;
        }
        let (id, n) = id_at(t, i + name + 1, 17, 21)?;
        let name = String::from_utf8_lossy(&t[i..i + name]).into_owned();
        return close(
            i + name.len() + 1 + n,
            Inline::CustomEmoji { id, name, animated },
        );
    }
    if t.starts_with(b"</") {
        // 1-3 words of 1-32 name characters, single spaces between; the nameless form stays text.
        let mut i = 2;
        let mut words = 0;
        loop {
            let mut n = 0;
            let mut j = i;
            while let Some(ch) = char_at(s, pos + j).filter(|c| slash_name_char(*c)) {
                if pos + j + ch.len_utf8() > pos + t.len() {
                    return None;
                }
                n += 1;
                j += ch.len_utf8();
            }
            if n == 0 || n > 32 {
                return None;
            }
            words += 1;
            i = j;
            if words < 3 && t.get(i) == Some(&b' ') {
                i += 1;
                continue;
            }
            break;
        }
        if t.get(i) != Some(&b':') {
            return None;
        }
        let name = s[pos + 2..pos + i].to_owned();
        let (id, n) = id_at(t, i + 1, 17, 21)?;
        return close(i + 1 + n, Inline::SlashCommand { name, id });
    }
    if t.starts_with(b"<t:") {
        let neg = t.get(3) == Some(&b'-');
        let i = 3 + usize::from(neg);
        let n = t[i..].iter().take_while(|x| x.is_ascii_digit()).count();
        // D3: at most 17 digits and |unix| <= 8.64e12 s (the JS Date range).
        if n == 0 || n > 17 {
            return None;
        }
        let v: i64 = std::str::from_utf8(&t[i..i + n]).ok()?.parse().ok()?;
        if v > 8_640_000_000_000 {
            return None;
        }
        let unix = if neg { -v } else { v };
        let j = i + n;
        let (style, k) = match (t.get(j), t.get(j + 1).copied().and_then(style)) {
            (Some(b':'), Some(st)) => (st, j + 2),
            _ => (TimestampStyle::LongDateShortTime, j),
        };
        return close(k, Inline::Timestamp { unix, style });
    }
    if t.get(1).is_some_and(u8::is_ascii_digit) {
        let (guild, n) = id_at(t, 1, 17, 20)?;
        let i = 1 + n;
        if t.get(i) != Some(&b':') {
            return None;
        }
        let rest = &t[i + 1..];
        for (word, target) in [
            (&b"customize"[..], NavTarget::Customize),
            (b"browse", NavTarget::Browse),
            (b"guide", NavTarget::Guide),
        ] {
            if rest.starts_with(word) {
                return close(
                    i + 1 + word.len(),
                    Inline::GuildNavigation { guild, target },
                );
            }
        }
        if rest.starts_with(b"linked-roles") {
            let j = i + 1 + 12;
            if t.get(j) == Some(&b':') {
                let (role, n) = id_at(t, j + 1, 17, 20)?;
                let target = NavTarget::LinkedRoles(Some(role));
                return close(j + 1 + n, Inline::GuildNavigation { guild, target });
            }
            let target = NavTarget::LinkedRoles(None);
            return close(j, Inline::GuildNavigation { guild, target });
        }
    }
    None
}

fn style(letter: u8) -> Option<TimestampStyle> {
    Some(match letter {
        b't' => TimestampStyle::ShortTime,
        b'T' => TimestampStyle::MediumTime,
        b'd' => TimestampStyle::ShortDate,
        b'D' => TimestampStyle::LongDate,
        b'f' => TimestampStyle::LongDateShortTime,
        b'F' => TimestampStyle::FullDateShortTime,
        b's' => TimestampStyle::ShortDateShortTime,
        b'S' => TimestampStyle::ShortDateMediumTime,
        b'R' => TimestampStyle::Relative,
        _ => return None,
    })
}

// ---- Link validation (D4) ----

/// simple-markdown's `unescapeUrl`: `\x` → `x` for any x not alphanumeric or space.
fn unescape_url(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    let mut chars = raw.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\\' {
            if let Some(&n) = chars
                .peek()
                .filter(|n| !n.is_ascii_alphanumeric() && !is_js_space(**n))
            {
                out.push(n);
                chars.next();
                continue;
            }
        }
        out.push(c);
    }
    out
}

/// A validated link target: the canonical URL and its ASCII host.
type Target = (String, String);

/// D4: an http(s) target that `url::Url` parses with a host and no userinfo (forbidden characters
/// are checked by the caller).
fn validate(target: &str) -> Option<Target> {
    let colon = target.find(':')?;
    let scheme = &target[..colon];
    if !scheme.eq_ignore_ascii_case("http") && !scheme.eq_ignore_ascii_case("https") {
        return None;
    }
    // The authority alone first, so a rejection costs O(authority), not O(target): repeated
    // candidates overlapping one long run then stay linear.
    let after = target[colon + 1..].trim_start_matches(['/', '\\']);
    let authority = after.find(['/', '\\', '?', '#']).unwrap_or(after.len());
    let prefix = &target[..target.len() - after.len() + authority];
    tick(prefix.len());
    let ok = |u: &url::Url| {
        matches!(u.scheme(), "http" | "https")
            && u.host_str().is_some_and(|h| !h.is_empty())
            && u.username().is_empty()
            && u.password().is_none()
    };
    if !ok(&url::Url::parse(prefix).ok()?) {
        return None;
    }
    tick(target.len());
    let url = url::Url::parse(target).ok().filter(ok)?;
    let host = url.host_str()?.to_owned();
    Some((url.as_str().to_owned(), host))
}

// ---- Per-level matchers and tables ----

/// One nesting level's source with its lazily built next-candidate tables and memos. Positions
/// are byte offsets into `s`; tables have `s.len() + 1` entries.
struct Level<'s> {
    s: &'s str,
    b: &'s [u8],
    pipes: Option<Vec<usize>>,
    tildes: Option<Vec<usize>>,
    stars: Option<Vec<usize>>,
    unders: Option<Vec<usize>>,
    fences: Option<Vec<usize>>,
    ticks: Option<HashMap<usize, (Vec<usize>, usize)>>,
    auto: Option<[Vec<usize>; 2]>,
    url: Option<[Vec<usize>; 2]>,
    forbidden: Option<Vec<usize>>,
    link: Option<LinkTables>,
    /// Text rule: end of the word run last looked at (`\w+:\S` check), and where it was taken.
    word: (usize, usize),
}

struct LinkTables {
    /// Next `]`, next `[`, next `)`, next non-space, next title quote closing the href.
    close: Vec<usize>,
    open: Vec<usize>,
    paren: Vec<usize>,
    solid: Vec<usize>,
    quote: Vec<usize>,
    /// Memo: last label exit along the label walk from a position.
    exit: Vec<usize>,
    /// Memo: end of the shortest target from a position (the lazy target loop).
    target: Vec<usize>,
    /// Memo: validation result per label exit.
    valid: HashMap<usize, Option<Target>>,
}

/// `t[i]` = the smallest `j >= i` with `hit(j)`, else `NONE`.
fn next_table(len: usize, hit: impl Fn(usize) -> bool) -> Vec<usize> {
    tick(len);
    let mut t = vec![NONE; len + 1];
    for i in (0..len).rev() {
        t[i] = if hit(i) { i } else { t[i + 1] };
    }
    t
}

impl<'s> Level<'s> {
    fn new(s: &'s str) -> Level<'s> {
        Level {
            s,
            b: s.as_bytes(),
            pipes: None,
            tildes: None,
            stars: None,
            unders: None,
            fences: None,
            ticks: None,
            auto: None,
            url: None,
            forbidden: None,
            link: None,
            word: (0, 0),
        }
    }

    fn pair_at(&self, i: usize, x: u8) -> bool {
        self.b[i] == x && self.b.get(i + 1) == Some(&x)
    }

    /// `^\|\|([\s\S]+?)\|\|`
    fn spoiler(&mut self, pos: usize) -> Option<(usize, Range<usize>)> {
        if !self.pair_at(pos, b'|') || pos + 2 >= self.b.len() {
            return None;
        }
        let first = pos + 2 + clen(self.b, pos + 2);
        let b = self.b;
        let e = self.pipes.get_or_insert_with(|| {
            next_table(b.len(), |i| b[i] == b'|' && b.get(i + 1) == Some(&b'|'))
        })[first];
        (e != NONE).then(|| (e + 2, pos + 2..e))
    }

    /// `^~~([\s\S]+?)~~(?!_)`
    fn strike(&mut self, pos: usize) -> Option<(usize, Range<usize>)> {
        if !self.pair_at(pos, b'~') || pos + 2 >= self.b.len() {
            return None;
        }
        let first = pos + 2 + clen(self.b, pos + 2);
        let b = self.b;
        let e = self.tildes.get_or_insert_with(|| {
            next_table(b.len(), |i| {
                b[i] == b'~' && b.get(i + 1) == Some(&b'~') && b.get(i + 2) != Some(&b'_')
            })
        })[first];
        (e != NONE).then(|| (e + 2, pos + 2..e))
    }

    /// strong `^\*\*((?:\\[\s\S]|[^\\])+?)\*\*(?!\*)` and u `^__((?:\\[\s\S]|[^\\])+?)__(?!_)`:
    /// the first unescaped closer after one content token.
    fn double(&mut self, pos: usize, x: u8) -> Option<(usize, Range<usize>)> {
        let b = self.b;
        if !self.pair_at(pos, x) || pos + 2 >= b.len() {
            return None;
        }
        let cs = pos + 2;
        let first = if b[cs] == b'\\' {
            cs + 1 + char_at(self.s, cs + 1)?.len_utf8()
        } else {
            cs + clen(b, cs)
        };
        let table = if x == b'*' {
            &mut self.stars
        } else {
            &mut self.unders
        };
        let e = table.get_or_insert_with(|| {
            // Escape-aware: a position after an unpaired backslash is never a closer.
            tick(b.len());
            let mut escaped = vec![false; b.len() + 1];
            for i in 0..b.len() {
                escaped[i + 1] = b[i] == b'\\' && !escaped[i];
            }
            next_table(b.len(), |i| {
                !escaped[i] && b[i] == x && b.get(i + 1) == Some(&x) && b.get(i + 2) != Some(&x)
            })
        })[first];
        (e != NONE).then(|| (e + 2, cs..e))
    }

    fn strong(&mut self, pos: usize) -> Option<(usize, Range<usize>)> {
        self.double(pos, b'*')
    }

    fn underline(&mut self, pos: usize) -> Option<(usize, Range<usize>)> {
        self.double(pos, b'_')
    }

    /// em, `*` form: `^\*(?=\S)((?:\*\*|\\[\s\S]|\s+(?:\\[\s\S]|[^\s\*\\]|\*\*)|[^\s\*\\])+?)\*(?!\*)`.
    /// The token alternatives are exclusive by their first character, so the walk is
    /// deterministic; a dead end is a failure.
    fn em_star(&self, pos: usize) -> Option<(usize, Range<usize>)> {
        let (s, b) = (self.s, self.b);
        let cs = pos + 1;
        if char_at(s, cs).is_none_or(is_js_space) {
            return None;
        }
        let token = |i: usize| -> Option<usize> {
            tick(1);
            let escape = |j: usize| char_at(s, j + 1).map(|c| j + 1 + c.len_utf8());
            if b[i..].starts_with(b"**") {
                return Some(i + 2);
            }
            if b[i] == b'\\' {
                return escape(i);
            }
            if space_at(s, i) {
                let mut j = i;
                while let Some(c) = char_at(s, j).filter(|c| is_js_space(*c)) {
                    tick(1);
                    j += c.len_utf8();
                }
                if j >= b.len() || (b[j] == b'*' && !b[j..].starts_with(b"**")) {
                    return None;
                }
                if b[j..].starts_with(b"**") {
                    return Some(j + 2);
                }
                if b[j] == b'\\' {
                    return escape(j);
                }
                return Some(j + clen(b, j));
            }
            (b[i] != b'*').then(|| i + clen(b, i))
        };
        let mut i = token(cs)?;
        loop {
            if i >= b.len() {
                return None;
            }
            if b[i] == b'*' && b.get(i + 1) != Some(&b'*') {
                return Some((i + 1, cs..i));
            }
            i = token(i)?;
        }
    }

    /// em, `_` form: `^\b_((?:__|\\[\s\S]|[^\\_])+?)_\b` (the `\b` before is always true on
    /// the remaining source; after, the closer must precede a non-word character or the end).
    fn em_under(&self, pos: usize) -> Option<(usize, Range<usize>)> {
        let b = self.b;
        let cs = pos + 1;
        let token = |i: usize| -> Option<usize> {
            tick(1);
            if i >= b.len() {
                None
            } else if b[i..].starts_with(b"__") {
                Some(i + 2)
            } else if b[i] == b'\\' {
                char_at(self.s, i + 1).map(|c| i + 1 + c.len_utf8())
            } else {
                (b[i] != b'_').then(|| i + clen(b, i))
            }
        };
        let mut i = token(cs)?;
        loop {
            if i < b.len() && b[i] == b'_' && b.get(i + 1).is_none_or(|&n| !is_word(n)) {
                return Some((i + 1, cs..i));
            }
            i = token(i)?;
        }
    }

    /// `CodeBlockRegex` `^```(([a-z0-9_+\-.#]+?)\n+)?\n*([^]+?)\n*```` (i), with the regex's
    /// backtracking order. Returns (end, language, content).
    fn code_block(&mut self, pos: usize) -> Option<(usize, Option<String>, Range<usize>)> {
        let b = self.b;
        if !b[pos..].starts_with(b"```") {
            return None;
        }
        let fences = self
            .fences
            .get_or_insert_with(|| next_table(b.len(), |i| b[i..].starts_with(b"```")));
        // The shortest content from `cs` that is followed by newlines and a fence.
        let end_from = |cs: usize| -> Option<(usize, usize)> {
            tick(1);
            if cs >= b.len() {
                return None;
            }
            let first = cs + clen(b, cs);
            let f = fences[first];
            if f == NONE {
                return None;
            }
            let mut e = f;
            while e > first && b[e - 1] == b'\n' {
                e -= 1;
            }
            Some((e, f))
        };
        let newlines = |from: usize| b[from..].iter().take_while(|&&x| x == b'\n').count();
        let p0 = pos + 3;
        let lang = b[p0..]
            .iter()
            .take_while(|x| x.is_ascii_alphanumeric() || b"_+-.#".contains(x))
            .count();
        if lang > 0 && b.get(p0 + lang) == Some(&b'\n') {
            let base = p0 + lang;
            for cs in (base + 1..=base + newlines(base)).rev() {
                if let Some((e, f)) = end_from(cs) {
                    let language = (lang <= 32).then(|| self.s[p0..base].to_owned());
                    return Some((f + 3, language, cs..e));
                }
            }
        }
        for cs in (p0..=p0 + newlines(p0)).rev() {
            if let Some((e, f)) = end_from(cs) {
                return Some((f + 3, None, cs..e));
            }
        }
        None
    }

    /// `^(`+)([\s\S]*?[^`])\1(?!`)`: for the opening run's length down to 1, the first later run
    /// of exactly that length.
    fn inline_code(&mut self, pos: usize) -> Option<(usize, Range<usize>)> {
        let b = self.b;
        let runs = self.ticks.get_or_insert_with(|| {
            tick(b.len());
            let mut runs: HashMap<usize, (Vec<usize>, usize)> = HashMap::new();
            let mut i = 0;
            while i < b.len() {
                if b[i] == b'`' {
                    let n = b[i..].iter().take_while(|&&x| x == b'`').count();
                    runs.entry(n).or_default().0.push(i);
                    i += n;
                } else {
                    i += 1;
                }
            }
            runs
        });
        let open = b[pos..].iter().take_while(|&&x| x == b'`').count();
        let from = pos + open + 1;
        for k in (1..=open).rev() {
            tick(1);
            let Some((starts, cursor)) = runs.get_mut(&k) else {
                continue;
            };
            // Attempts within a level move forward, so each cursor only advances.
            while *cursor < starts.len() && starts[*cursor] < from {
                tick(1);
                *cursor += 1;
            }
            if let Some(&e) = starts.get(*cursor) {
                return Some((e + k, pos + k..e));
            }
        }
        None
    }

    /// Position of the first C0/C1/Cf character at or after `i`.
    fn forbidden_from(&mut self, i: usize) -> usize {
        let s = self.s;
        self.forbidden.get_or_insert_with(|| {
            next_table(s.len(), |j| {
                s.is_char_boundary(j) && char_at(s, j).is_some_and(forbidden_in_url)
            })
        })[i]
    }

    /// A raw target range, validated (D4).
    fn checked(&mut self, raw: Range<usize>, unescape: bool) -> Option<Target> {
        if self.forbidden_from(raw.start) < raw.end {
            return None;
        }
        let raw = &self.s[raw];
        if unescape {
            validate(&unescape_url(raw))
        } else {
            validate(raw)
        }
    }

    /// autolink `^<([^: >]+:\/[^ >]+)>`, http(s) only. Returns (end, (inner range), target).
    fn autolink(&mut self, pos: usize) -> Option<(usize, Range<usize>, Target)> {
        let b = self.b;
        let [colon, gt] = self.auto.get_or_insert_with(|| {
            [
                next_table(b.len(), |i| matches!(b[i], b':' | b' ' | b'>')),
                next_table(b.len(), |i| matches!(b[i], b' ' | b'>')),
            ]
        });
        let c = colon[pos + 1];
        if c == NONE || c == pos + 1 || b[c] != b':' || b.get(c + 1) != Some(&b'/') {
            return None;
        }
        let r = gt[c + 2];
        if r == NONE || r == c + 2 || b[r] != b'>' {
            return None;
        }
        let target = self.checked(pos + 1..r, false)?;
        Some((r + 1, pos + 1..r, target))
    }

    /// url `^(https?:\/\/[^\s<]+[^<.,:;"')\]\s])`.
    fn url(&mut self, pos: usize) -> Option<(usize, Range<usize>, Target)> {
        let (s, b) = (self.s, self.b);
        let q = if b[pos..].starts_with(b"https://") {
            pos + 8
        } else if b[pos..].starts_with(b"http://") {
            pos + 7
        } else {
            return None;
        };
        let [run_end, last_ok] = self.url.get_or_insert_with(|| {
            tick(2 * b.len());
            let run = next_table(b.len(), |i| {
                s.is_char_boundary(i) && (b[i] == b'<' || space_at(s, i))
            });
            // last_ok[i]: the last character start before i that may end a URL.
            let mut last = vec![NONE; b.len() + 1];
            let mut prev = NONE;
            for (i, c) in s.char_indices() {
                last[i] = prev;
                if !is_js_space(c) && !"<.,:;\"')]".contains(c) {
                    prev = i;
                }
            }
            last[b.len()] = prev;
            [run, last]
        });
        if q >= b.len() {
            return None;
        }
        let r = run_end[q].min(b.len());
        let j = last_ok[r];
        // `[^\s<]+` and the final class each take at least one UTF-16 unit: one character
        // outside the BMP (two units) satisfies both.
        if r == q || j == NONE || j < q || (j == q && clen(b, q) < 4) {
            return None;
        }
        let end = j + clen(b, j);
        let target = self.checked(pos..end, false)?;
        Some((end, pos..end, target))
    }

    /// link `^\[(LINK_INSIDE)\]\(LINK_HREF_AND_TITLE\)` with simple-markdown's regexes. Returns
    /// (end, kind, label range, validated target).
    fn link(&mut self, pos: usize) -> Option<(usize, LinkKind, Range<usize>, Target)> {
        let exit = self.label_exit(pos + 1)?;
        let (ts, te, end) = self.href(exit + 2)?;
        let cached = self.link_tables().valid.get(&exit).cloned();
        let target = match cached {
            Some(t) => t,
            None => {
                let t = self.checked(ts..te, true);
                self.link_tables().valid.insert(exit, t.clone());
                t
            }
        }?;
        Some((end, LinkKind::Masked, pos + 1..exit, target))
    }

    fn link_tables(&mut self) -> &mut LinkTables {
        let (s, b) = (self.s, self.b);
        self.link.get_or_insert_with(|| {
            let solid = next_table(b.len(), |i| s.is_char_boundary(i) && !space_at(s, i));
            let quote = next_table(b.len(), |m| {
                matches!(b[m], b'"' | b'\'') && b.get(solid[m + 1]) == Some(&b')')
            });
            LinkTables {
                close: next_table(b.len(), |i| b[i] == b']'),
                open: next_table(b.len(), |i| b[i] == b'['),
                paren: next_table(b.len(), |i| b[i] == b')'),
                solid,
                quote,
                exit: vec![UNKNOWN; b.len() + 1],
                target: vec![UNKNOWN; b.len() + 1],
                valid: HashMap::new(),
            }
        })
    }

    /// One iteration of `LINK_INSIDE` = `(?:\[[^\]]*\]|[^\[\]]|\](?=[^\[]*\]))` at `q`.
    fn label_step(&mut self, q: usize) -> Option<usize> {
        let b = self.b;
        if q >= b.len() {
            return None;
        }
        let t = self.link_tables();
        match b[q] {
            b'[' => (t.close[q + 1] != NONE).then(|| t.close[q + 1] + 1),
            b']' => {
                let (c, o) = (t.close[q + 1], t.open[q + 1]);
                (c != NONE && c < o).then_some(q + 1)
            }
            _ => Some(q + clen(b, q)),
        }
    }

    /// The label's end: the greedy loop's last exit (`](` and a matching href) along the
    /// deterministic walk from `q0`. Memoised per position across attempts.
    fn label_exit(&mut self, q0: usize) -> Option<usize> {
        let mut path = Vec::new();
        let mut q = q0;
        let mut found = loop {
            tick(1);
            let known = self.link_tables().exit[q];
            if known != UNKNOWN {
                break known;
            }
            path.push(q);
            match self.label_step(q) {
                Some(next) => q = next,
                None => break NONE,
            }
        };
        for &q in path.iter().rev() {
            if found == NONE && self.b[q..].starts_with(b"](") && self.href(q + 2).is_some() {
                found = q;
            }
            self.link_tables().exit[q] = found;
        }
        (found != NONE).then_some(found)
    }

    /// `\s*<?(target)>?(?:\s+['"]title['"])?\s*\)` from `h`: (target start, target end, end).
    fn href(&mut self, h: usize) -> Option<(usize, usize, usize)> {
        let w = self.link_tables().solid[h].min(self.b.len());
        // `\s*` gives back one character at a time; `<?` is tried with, then without.
        let mut starts = Vec::new();
        if self.b.get(w) == Some(&b'<') {
            starts.push(w + 1);
        }
        starts.push(w);
        if let Some(c) = self.s[h..w].chars().next_back() {
            // Any shorter `\s*` leaves a space, where only an empty target can match.
            starts.push(w - c.len_utf8());
        }
        for ts in starts {
            if let Some(te) = self.target_end(ts) {
                let end = self.after_target(te)?;
                return Some((ts, te, end));
            }
        }
        None
    }

    /// `>?(?:\s+['"]([\s\S]*?)['"])?\s*\)` at `t`: the match end.
    fn after_target(&mut self, t: usize) -> Option<usize> {
        let (s, b) = (self.s, self.b);
        let u = if b.get(t) == Some(&b'>') { t + 1 } else { t };
        let tables = self.link_tables();
        if space_at(s, u) {
            let k = tables.solid[u];
            if matches!(b.get(k), Some(b'"' | b'\'')) {
                let m = tables.quote[k + 1];
                if m != NONE {
                    return Some(tables.solid[m + 1] + 1);
                }
            }
        }
        let v = tables.solid[u.min(b.len())];
        (b.get(v) == Some(&b')')).then(|| v + 1)
    }

    /// The lazy target loop `(?:\([^)]*\)|[^\s\\]|\\.)*?` from `t0`: the first position where the
    /// rest of the href matches, trying a paren group before a single character. Memoised per
    /// position across attempts (an explicit stack instead of recursion).
    fn target_end(&mut self, t0: usize) -> Option<usize> {
        let (s, b) = (self.s, self.b);
        let children = |lv: &mut Level, t: usize| -> [Option<usize>; 3] {
            let paren = (b.get(t) == Some(&b'(')).then(|| lv.link_tables().paren[t + 1]);
            let paren = paren.filter(|&p| p != NONE).map(|p| p + 1);
            let c = char_at(s, t);
            let single = c
                .filter(|c| !is_js_space(*c) && *c != '\\')
                .map(|c| t + c.len_utf8());
            let escape = (c == Some('\\'))
                .then(|| char_at(s, t + 1))
                .flatten()
                .filter(|n| !matches!(n, '\n' | '\r' | '\u{2028}' | '\u{2029}'))
                .map(|n| t + 1 + n.len_utf8());
            [paren, single, escape]
        };
        let mut stack: Vec<(usize, usize)> = vec![(t0, 0)];
        while let Some(&(t, stage)) = stack.last() {
            tick(1);
            if self.link_tables().target[t] != UNKNOWN {
                stack.pop();
                continue;
            }
            if stage == 0 && self.after_target(t).is_some() {
                self.link_tables().target[t] = t;
                stack.pop();
                continue;
            }
            let kids = children(self, t);
            // Stage k > 0: child k-1 has finished; take its result if it matched.
            if stage > 0 {
                if let Some(child) = kids[stage - 1] {
                    let r = self.link_tables().target[child];
                    if r != NONE {
                        self.link_tables().target[t] = r;
                        stack.pop();
                        continue;
                    }
                }
            }
            if stage == 3 {
                self.link_tables().target[t] = NONE;
                stack.pop();
                continue;
            }
            if let Some(top) = stack.last_mut() {
                top.1 = stage + 1;
            }
            if let Some(child) = kids[stage] {
                stack.push((child, 0));
            }
        }
        let r = self.link_tables().target[t0];
        (r != NONE).then_some(r)
    }

    /// `TextRegex` `^[\s\S]+?(?=[^0-9A-Za-z\s]|\n\n|\n|\w+:\S|$)`, linear: the `\w+:\S` check
    /// reuses the end of the current word run.
    fn text_end(&mut self, pos: usize) -> usize {
        let (s, b) = (self.s, self.b);
        let mut k = pos + clen(b, pos);
        while k < b.len() {
            tick(1);
            let c = b[k];
            if c >= 0x80 {
                match char_at(s, k).filter(|c| is_js_space(*c)) {
                    Some(c) => k += c.len_utf8(),
                    None => break,
                }
                continue;
            }
            if c == b'\n' || !(c.is_ascii_alphanumeric() || c.is_ascii_whitespace() || c == 0x0b) {
                break;
            }
            if c.is_ascii_alphanumeric() {
                let (from, end) = self.word;
                let end = if from <= k && k < end {
                    end
                } else {
                    let end = k + b[k..].iter().take_while(|&&x| is_word(x)).count();
                    tick(end - k);
                    self.word = (k, end);
                    end
                };
                if b.get(end) == Some(&b':') && char_at(s, end + 1).is_some_and(|n| !is_js_space(n))
                {
                    break;
                }
            }
            k += 1;
        }
        k
    }
}

/// `[\p{L}\p{N}\p{sc=Deva}\p{sc=Thai}]`, Unicode 17, generated from Node 24 (V8/ICU 78.3) with
/// a scan of every code point against that `/u` class.
#[rustfmt::skip]
const SLASH_NAME: &[(u32, u32)] = &[
    (0x30, 0x39), (0x41, 0x5A), (0x61, 0x7A), (0xAA, 0xAA), (0xB2, 0xB3), (0xB5, 0xB5),
    (0xB9, 0xBA), (0xBC, 0xBE), (0xC0, 0xD6), (0xD8, 0xF6), (0xF8, 0x2C1), (0x2C6, 0x2D1),
    (0x2E0, 0x2E4), (0x2EC, 0x2EC), (0x2EE, 0x2EE), (0x370, 0x374), (0x376, 0x377), (0x37A, 0x37D),
    (0x37F, 0x37F), (0x386, 0x386), (0x388, 0x38A), (0x38C, 0x38C), (0x38E, 0x3A1), (0x3A3, 0x3F5),
    (0x3F7, 0x481), (0x48A, 0x52F), (0x531, 0x556), (0x559, 0x559), (0x560, 0x588), (0x5D0, 0x5EA),
    (0x5EF, 0x5F2), (0x620, 0x64A), (0x660, 0x669), (0x66E, 0x66F), (0x671, 0x6D3), (0x6D5, 0x6D5),
    (0x6E5, 0x6E6), (0x6EE, 0x6FC), (0x6FF, 0x6FF), (0x710, 0x710), (0x712, 0x72F), (0x74D, 0x7A5),
    (0x7B1, 0x7B1), (0x7C0, 0x7EA), (0x7F4, 0x7F5), (0x7FA, 0x7FA), (0x800, 0x815), (0x81A, 0x81A),
    (0x824, 0x824), (0x828, 0x828), (0x840, 0x858), (0x860, 0x86A), (0x870, 0x887), (0x889, 0x88F),
    (0x8A0, 0x8C9), (0x900, 0x950), (0x955, 0x963), (0x966, 0x980), (0x985, 0x98C), (0x98F, 0x990),
    (0x993, 0x9A8), (0x9AA, 0x9B0), (0x9B2, 0x9B2), (0x9B6, 0x9B9), (0x9BD, 0x9BD), (0x9CE, 0x9CE),
    (0x9DC, 0x9DD), (0x9DF, 0x9E1), (0x9E6, 0x9F1), (0x9F4, 0x9F9), (0x9FC, 0x9FC), (0xA05, 0xA0A),
    (0xA0F, 0xA10), (0xA13, 0xA28), (0xA2A, 0xA30), (0xA32, 0xA33), (0xA35, 0xA36), (0xA38, 0xA39),
    (0xA59, 0xA5C), (0xA5E, 0xA5E), (0xA66, 0xA6F), (0xA72, 0xA74), (0xA85, 0xA8D), (0xA8F, 0xA91),
    (0xA93, 0xAA8), (0xAAA, 0xAB0), (0xAB2, 0xAB3), (0xAB5, 0xAB9), (0xABD, 0xABD), (0xAD0, 0xAD0),
    (0xAE0, 0xAE1), (0xAE6, 0xAEF), (0xAF9, 0xAF9), (0xB05, 0xB0C), (0xB0F, 0xB10), (0xB13, 0xB28),
    (0xB2A, 0xB30), (0xB32, 0xB33), (0xB35, 0xB39), (0xB3D, 0xB3D), (0xB5C, 0xB5D), (0xB5F, 0xB61),
    (0xB66, 0xB6F), (0xB71, 0xB77), (0xB83, 0xB83), (0xB85, 0xB8A), (0xB8E, 0xB90), (0xB92, 0xB95),
    (0xB99, 0xB9A), (0xB9C, 0xB9C), (0xB9E, 0xB9F), (0xBA3, 0xBA4), (0xBA8, 0xBAA), (0xBAE, 0xBB9),
    (0xBD0, 0xBD0), (0xBE6, 0xBF2), (0xC05, 0xC0C), (0xC0E, 0xC10), (0xC12, 0xC28), (0xC2A, 0xC39),
    (0xC3D, 0xC3D), (0xC58, 0xC5A), (0xC5C, 0xC5D), (0xC60, 0xC61), (0xC66, 0xC6F), (0xC78, 0xC7E),
    (0xC80, 0xC80), (0xC85, 0xC8C), (0xC8E, 0xC90), (0xC92, 0xCA8), (0xCAA, 0xCB3), (0xCB5, 0xCB9),
    (0xCBD, 0xCBD), (0xCDC, 0xCDE), (0xCE0, 0xCE1), (0xCE6, 0xCEF), (0xCF1, 0xCF2), (0xD04, 0xD0C),
    (0xD0E, 0xD10), (0xD12, 0xD3A), (0xD3D, 0xD3D), (0xD4E, 0xD4E), (0xD54, 0xD56), (0xD58, 0xD61),
    (0xD66, 0xD78), (0xD7A, 0xD7F), (0xD85, 0xD96), (0xD9A, 0xDB1), (0xDB3, 0xDBB), (0xDBD, 0xDBD),
    (0xDC0, 0xDC6), (0xDE6, 0xDEF), (0xE01, 0xE3A), (0xE40, 0xE5B), (0xE81, 0xE82), (0xE84, 0xE84),
    (0xE86, 0xE8A), (0xE8C, 0xEA3), (0xEA5, 0xEA5), (0xEA7, 0xEB0), (0xEB2, 0xEB3), (0xEBD, 0xEBD),
    (0xEC0, 0xEC4), (0xEC6, 0xEC6), (0xED0, 0xED9), (0xEDC, 0xEDF), (0xF00, 0xF00), (0xF20, 0xF33),
    (0xF40, 0xF47), (0xF49, 0xF6C), (0xF88, 0xF8C), (0x1000, 0x102A), (0x103F, 0x1049), (0x1050, 0x1055),
    (0x105A, 0x105D), (0x1061, 0x1061), (0x1065, 0x1066), (0x106E, 0x1070), (0x1075, 0x1081), (0x108E, 0x108E),
    (0x1090, 0x1099), (0x10A0, 0x10C5), (0x10C7, 0x10C7), (0x10CD, 0x10CD), (0x10D0, 0x10FA), (0x10FC, 0x1248),
    (0x124A, 0x124D), (0x1250, 0x1256), (0x1258, 0x1258), (0x125A, 0x125D), (0x1260, 0x1288), (0x128A, 0x128D),
    (0x1290, 0x12B0), (0x12B2, 0x12B5), (0x12B8, 0x12BE), (0x12C0, 0x12C0), (0x12C2, 0x12C5), (0x12C8, 0x12D6),
    (0x12D8, 0x1310), (0x1312, 0x1315), (0x1318, 0x135A), (0x1369, 0x137C), (0x1380, 0x138F), (0x13A0, 0x13F5),
    (0x13F8, 0x13FD), (0x1401, 0x166C), (0x166F, 0x167F), (0x1681, 0x169A), (0x16A0, 0x16EA), (0x16EE, 0x16F8),
    (0x1700, 0x1711), (0x171F, 0x1731), (0x1740, 0x1751), (0x1760, 0x176C), (0x176E, 0x1770), (0x1780, 0x17B3),
    (0x17D7, 0x17D7), (0x17DC, 0x17DC), (0x17E0, 0x17E9), (0x17F0, 0x17F9), (0x1810, 0x1819), (0x1820, 0x1878),
    (0x1880, 0x1884), (0x1887, 0x18A8), (0x18AA, 0x18AA), (0x18B0, 0x18F5), (0x1900, 0x191E), (0x1946, 0x196D),
    (0x1970, 0x1974), (0x1980, 0x19AB), (0x19B0, 0x19C9), (0x19D0, 0x19DA), (0x1A00, 0x1A16), (0x1A20, 0x1A54),
    (0x1A80, 0x1A89), (0x1A90, 0x1A99), (0x1AA7, 0x1AA7), (0x1B05, 0x1B33), (0x1B45, 0x1B4C), (0x1B50, 0x1B59),
    (0x1B83, 0x1BA0), (0x1BAE, 0x1BE5), (0x1C00, 0x1C23), (0x1C40, 0x1C49), (0x1C4D, 0x1C7D), (0x1C80, 0x1C8A),
    (0x1C90, 0x1CBA), (0x1CBD, 0x1CBF), (0x1CE9, 0x1CEC), (0x1CEE, 0x1CF3), (0x1CF5, 0x1CF6), (0x1CFA, 0x1CFA),
    (0x1D00, 0x1DBF), (0x1E00, 0x1F15), (0x1F18, 0x1F1D), (0x1F20, 0x1F45), (0x1F48, 0x1F4D), (0x1F50, 0x1F57),
    (0x1F59, 0x1F59), (0x1F5B, 0x1F5B), (0x1F5D, 0x1F5D), (0x1F5F, 0x1F7D), (0x1F80, 0x1FB4), (0x1FB6, 0x1FBC),
    (0x1FBE, 0x1FBE), (0x1FC2, 0x1FC4), (0x1FC6, 0x1FCC), (0x1FD0, 0x1FD3), (0x1FD6, 0x1FDB), (0x1FE0, 0x1FEC),
    (0x1FF2, 0x1FF4), (0x1FF6, 0x1FFC), (0x2070, 0x2071), (0x2074, 0x2079), (0x207F, 0x2089), (0x2090, 0x209C),
    (0x2102, 0x2102), (0x2107, 0x2107), (0x210A, 0x2113), (0x2115, 0x2115), (0x2119, 0x211D), (0x2124, 0x2124),
    (0x2126, 0x2126), (0x2128, 0x2128), (0x212A, 0x212D), (0x212F, 0x2139), (0x213C, 0x213F), (0x2145, 0x2149),
    (0x214E, 0x214E), (0x2150, 0x2189), (0x2460, 0x249B), (0x24EA, 0x24FF), (0x2776, 0x2793), (0x2C00, 0x2CE4),
    (0x2CEB, 0x2CEE), (0x2CF2, 0x2CF3), (0x2CFD, 0x2CFD), (0x2D00, 0x2D25), (0x2D27, 0x2D27), (0x2D2D, 0x2D2D),
    (0x2D30, 0x2D67), (0x2D6F, 0x2D6F), (0x2D80, 0x2D96), (0x2DA0, 0x2DA6), (0x2DA8, 0x2DAE), (0x2DB0, 0x2DB6),
    (0x2DB8, 0x2DBE), (0x2DC0, 0x2DC6), (0x2DC8, 0x2DCE), (0x2DD0, 0x2DD6), (0x2DD8, 0x2DDE), (0x2E2F, 0x2E2F),
    (0x3005, 0x3007), (0x3021, 0x3029), (0x3031, 0x3035), (0x3038, 0x303C), (0x3041, 0x3096), (0x309D, 0x309F),
    (0x30A1, 0x30FA), (0x30FC, 0x30FF), (0x3105, 0x312F), (0x3131, 0x318E), (0x3192, 0x3195), (0x31A0, 0x31BF),
    (0x31F0, 0x31FF), (0x3220, 0x3229), (0x3248, 0x324F), (0x3251, 0x325F), (0x3280, 0x3289), (0x32B1, 0x32BF),
    (0x3400, 0x4DBF), (0x4E00, 0xA48C), (0xA4D0, 0xA4FD), (0xA500, 0xA60C), (0xA610, 0xA62B), (0xA640, 0xA66E),
    (0xA67F, 0xA69D), (0xA6A0, 0xA6EF), (0xA717, 0xA71F), (0xA722, 0xA788), (0xA78B, 0xA7DC), (0xA7F1, 0xA801),
    (0xA803, 0xA805), (0xA807, 0xA80A), (0xA80C, 0xA822), (0xA830, 0xA835), (0xA840, 0xA873), (0xA882, 0xA8B3),
    (0xA8D0, 0xA8D9), (0xA8E0, 0xA925), (0xA930, 0xA946), (0xA960, 0xA97C), (0xA984, 0xA9B2), (0xA9CF, 0xA9D9),
    (0xA9E0, 0xA9E4), (0xA9E6, 0xA9FE), (0xAA00, 0xAA28), (0xAA40, 0xAA42), (0xAA44, 0xAA4B), (0xAA50, 0xAA59),
    (0xAA60, 0xAA76), (0xAA7A, 0xAA7A), (0xAA7E, 0xAAAF), (0xAAB1, 0xAAB1), (0xAAB5, 0xAAB6), (0xAAB9, 0xAABD),
    (0xAAC0, 0xAAC0), (0xAAC2, 0xAAC2), (0xAADB, 0xAADD), (0xAAE0, 0xAAEA), (0xAAF2, 0xAAF4), (0xAB01, 0xAB06),
    (0xAB09, 0xAB0E), (0xAB11, 0xAB16), (0xAB20, 0xAB26), (0xAB28, 0xAB2E), (0xAB30, 0xAB5A), (0xAB5C, 0xAB69),
    (0xAB70, 0xABE2), (0xABF0, 0xABF9), (0xAC00, 0xD7A3), (0xD7B0, 0xD7C6), (0xD7CB, 0xD7FB), (0xF900, 0xFA6D),
    (0xFA70, 0xFAD9), (0xFB00, 0xFB06), (0xFB13, 0xFB17), (0xFB1D, 0xFB1D), (0xFB1F, 0xFB28), (0xFB2A, 0xFB36),
    (0xFB38, 0xFB3C), (0xFB3E, 0xFB3E), (0xFB40, 0xFB41), (0xFB43, 0xFB44), (0xFB46, 0xFBB1), (0xFBD3, 0xFD3D),
    (0xFD50, 0xFD8F), (0xFD92, 0xFDC7), (0xFDF0, 0xFDFB), (0xFE70, 0xFE74), (0xFE76, 0xFEFC), (0xFF10, 0xFF19),
    (0xFF21, 0xFF3A), (0xFF41, 0xFF5A), (0xFF66, 0xFFBE), (0xFFC2, 0xFFC7), (0xFFCA, 0xFFCF), (0xFFD2, 0xFFD7),
    (0xFFDA, 0xFFDC), (0x10000, 0x1000B), (0x1000D, 0x10026), (0x10028, 0x1003A), (0x1003C, 0x1003D), (0x1003F, 0x1004D),
    (0x10050, 0x1005D), (0x10080, 0x100FA), (0x10107, 0x10133), (0x10140, 0x10178), (0x1018A, 0x1018B), (0x10280, 0x1029C),
    (0x102A0, 0x102D0), (0x102E1, 0x102FB), (0x10300, 0x10323), (0x1032D, 0x1034A), (0x10350, 0x10375), (0x10380, 0x1039D),
    (0x103A0, 0x103C3), (0x103C8, 0x103CF), (0x103D1, 0x103D5), (0x10400, 0x1049D), (0x104A0, 0x104A9), (0x104B0, 0x104D3),
    (0x104D8, 0x104FB), (0x10500, 0x10527), (0x10530, 0x10563), (0x10570, 0x1057A), (0x1057C, 0x1058A), (0x1058C, 0x10592),
    (0x10594, 0x10595), (0x10597, 0x105A1), (0x105A3, 0x105B1), (0x105B3, 0x105B9), (0x105BB, 0x105BC), (0x105C0, 0x105F3),
    (0x10600, 0x10736), (0x10740, 0x10755), (0x10760, 0x10767), (0x10780, 0x10785), (0x10787, 0x107B0), (0x107B2, 0x107BA),
    (0x10800, 0x10805), (0x10808, 0x10808), (0x1080A, 0x10835), (0x10837, 0x10838), (0x1083C, 0x1083C), (0x1083F, 0x10855),
    (0x10858, 0x10876), (0x10879, 0x1089E), (0x108A7, 0x108AF), (0x108E0, 0x108F2), (0x108F4, 0x108F5), (0x108FB, 0x1091B),
    (0x10920, 0x10939), (0x10940, 0x10959), (0x10980, 0x109B7), (0x109BC, 0x109CF), (0x109D2, 0x10A00), (0x10A10, 0x10A13),
    (0x10A15, 0x10A17), (0x10A19, 0x10A35), (0x10A40, 0x10A48), (0x10A60, 0x10A7E), (0x10A80, 0x10A9F), (0x10AC0, 0x10AC7),
    (0x10AC9, 0x10AE4), (0x10AEB, 0x10AEF), (0x10B00, 0x10B35), (0x10B40, 0x10B55), (0x10B58, 0x10B72), (0x10B78, 0x10B91),
    (0x10BA9, 0x10BAF), (0x10C00, 0x10C48), (0x10C80, 0x10CB2), (0x10CC0, 0x10CF2), (0x10CFA, 0x10D23), (0x10D30, 0x10D39),
    (0x10D40, 0x10D65), (0x10D6F, 0x10D85), (0x10E60, 0x10E7E), (0x10E80, 0x10EA9), (0x10EB0, 0x10EB1), (0x10EC2, 0x10EC7),
    (0x10F00, 0x10F27), (0x10F30, 0x10F45), (0x10F51, 0x10F54), (0x10F70, 0x10F81), (0x10FB0, 0x10FCB), (0x10FE0, 0x10FF6),
    (0x11003, 0x11037), (0x11052, 0x1106F), (0x11071, 0x11072), (0x11075, 0x11075), (0x11083, 0x110AF), (0x110D0, 0x110E8),
    (0x110F0, 0x110F9), (0x11103, 0x11126), (0x11136, 0x1113F), (0x11144, 0x11144), (0x11147, 0x11147), (0x11150, 0x11172),
    (0x11176, 0x11176), (0x11183, 0x111B2), (0x111C1, 0x111C4), (0x111D0, 0x111DA), (0x111DC, 0x111DC), (0x111E1, 0x111F4),
    (0x11200, 0x11211), (0x11213, 0x1122B), (0x1123F, 0x11240), (0x11280, 0x11286), (0x11288, 0x11288), (0x1128A, 0x1128D),
    (0x1128F, 0x1129D), (0x1129F, 0x112A8), (0x112B0, 0x112DE), (0x112F0, 0x112F9), (0x11305, 0x1130C), (0x1130F, 0x11310),
    (0x11313, 0x11328), (0x1132A, 0x11330), (0x11332, 0x11333), (0x11335, 0x11339), (0x1133D, 0x1133D), (0x11350, 0x11350),
    (0x1135D, 0x11361), (0x11380, 0x11389), (0x1138B, 0x1138B), (0x1138E, 0x1138E), (0x11390, 0x113B5), (0x113B7, 0x113B7),
    (0x113D1, 0x113D1), (0x113D3, 0x113D3), (0x11400, 0x11434), (0x11447, 0x1144A), (0x11450, 0x11459), (0x1145F, 0x11461),
    (0x11480, 0x114AF), (0x114C4, 0x114C5), (0x114C7, 0x114C7), (0x114D0, 0x114D9), (0x11580, 0x115AE), (0x115D8, 0x115DB),
    (0x11600, 0x1162F), (0x11644, 0x11644), (0x11650, 0x11659), (0x11680, 0x116AA), (0x116B8, 0x116B8), (0x116C0, 0x116C9),
    (0x116D0, 0x116E3), (0x11700, 0x1171A), (0x11730, 0x1173B), (0x11740, 0x11746), (0x11800, 0x1182B), (0x118A0, 0x118F2),
    (0x118FF, 0x11906), (0x11909, 0x11909), (0x1190C, 0x11913), (0x11915, 0x11916), (0x11918, 0x1192F), (0x1193F, 0x1193F),
    (0x11941, 0x11941), (0x11950, 0x11959), (0x119A0, 0x119A7), (0x119AA, 0x119D0), (0x119E1, 0x119E1), (0x119E3, 0x119E3),
    (0x11A00, 0x11A00), (0x11A0B, 0x11A32), (0x11A3A, 0x11A3A), (0x11A50, 0x11A50), (0x11A5C, 0x11A89), (0x11A9D, 0x11A9D),
    (0x11AB0, 0x11AF8), (0x11B00, 0x11B09), (0x11BC0, 0x11BE0), (0x11BF0, 0x11BF9), (0x11C00, 0x11C08), (0x11C0A, 0x11C2E),
    (0x11C40, 0x11C40), (0x11C50, 0x11C6C), (0x11C72, 0x11C8F), (0x11D00, 0x11D06), (0x11D08, 0x11D09), (0x11D0B, 0x11D30),
    (0x11D46, 0x11D46), (0x11D50, 0x11D59), (0x11D60, 0x11D65), (0x11D67, 0x11D68), (0x11D6A, 0x11D89), (0x11D98, 0x11D98),
    (0x11DA0, 0x11DA9), (0x11DB0, 0x11DDB), (0x11DE0, 0x11DE9), (0x11EE0, 0x11EF2), (0x11F02, 0x11F02), (0x11F04, 0x11F10),
    (0x11F12, 0x11F33), (0x11F50, 0x11F59), (0x11FB0, 0x11FB0), (0x11FC0, 0x11FD4), (0x12000, 0x12399), (0x12400, 0x1246E),
    (0x12480, 0x12543), (0x12F90, 0x12FF0), (0x13000, 0x1342F), (0x13441, 0x13446), (0x13460, 0x143FA), (0x14400, 0x14646),
    (0x16100, 0x1611D), (0x16130, 0x16139), (0x16800, 0x16A38), (0x16A40, 0x16A5E), (0x16A60, 0x16A69), (0x16A70, 0x16ABE),
    (0x16AC0, 0x16AC9), (0x16AD0, 0x16AED), (0x16B00, 0x16B2F), (0x16B40, 0x16B43), (0x16B50, 0x16B59), (0x16B5B, 0x16B61),
    (0x16B63, 0x16B77), (0x16B7D, 0x16B8F), (0x16D40, 0x16D6C), (0x16D70, 0x16D79), (0x16E40, 0x16E96), (0x16EA0, 0x16EB8),
    (0x16EBB, 0x16ED3), (0x16F00, 0x16F4A), (0x16F50, 0x16F50), (0x16F93, 0x16F9F), (0x16FE0, 0x16FE1), (0x16FE3, 0x16FE3),
    (0x16FF2, 0x16FF6), (0x17000, 0x18CD5), (0x18CFF, 0x18D1E), (0x18D80, 0x18DF2), (0x1AFF0, 0x1AFF3), (0x1AFF5, 0x1AFFB),
    (0x1AFFD, 0x1AFFE), (0x1B000, 0x1B122), (0x1B132, 0x1B132), (0x1B150, 0x1B152), (0x1B155, 0x1B155), (0x1B164, 0x1B167),
    (0x1B170, 0x1B2FB), (0x1BC00, 0x1BC6A), (0x1BC70, 0x1BC7C), (0x1BC80, 0x1BC88), (0x1BC90, 0x1BC99), (0x1CCF0, 0x1CCF9),
    (0x1D2C0, 0x1D2D3), (0x1D2E0, 0x1D2F3), (0x1D360, 0x1D378), (0x1D400, 0x1D454), (0x1D456, 0x1D49C), (0x1D49E, 0x1D49F),
    (0x1D4A2, 0x1D4A2), (0x1D4A5, 0x1D4A6), (0x1D4A9, 0x1D4AC), (0x1D4AE, 0x1D4B9), (0x1D4BB, 0x1D4BB), (0x1D4BD, 0x1D4C3),
    (0x1D4C5, 0x1D505), (0x1D507, 0x1D50A), (0x1D50D, 0x1D514), (0x1D516, 0x1D51C), (0x1D51E, 0x1D539), (0x1D53B, 0x1D53E),
    (0x1D540, 0x1D544), (0x1D546, 0x1D546), (0x1D54A, 0x1D550), (0x1D552, 0x1D6A5), (0x1D6A8, 0x1D6C0), (0x1D6C2, 0x1D6DA),
    (0x1D6DC, 0x1D6FA), (0x1D6FC, 0x1D714), (0x1D716, 0x1D734), (0x1D736, 0x1D74E), (0x1D750, 0x1D76E), (0x1D770, 0x1D788),
    (0x1D78A, 0x1D7A8), (0x1D7AA, 0x1D7C2), (0x1D7C4, 0x1D7CB), (0x1D7CE, 0x1D7FF), (0x1DF00, 0x1DF1E), (0x1DF25, 0x1DF2A),
    (0x1E030, 0x1E06D), (0x1E100, 0x1E12C), (0x1E137, 0x1E13D), (0x1E140, 0x1E149), (0x1E14E, 0x1E14E), (0x1E290, 0x1E2AD),
    (0x1E2C0, 0x1E2EB), (0x1E2F0, 0x1E2F9), (0x1E4D0, 0x1E4EB), (0x1E4F0, 0x1E4F9), (0x1E5D0, 0x1E5ED), (0x1E5F0, 0x1E5FA),
    (0x1E6C0, 0x1E6DE), (0x1E6E0, 0x1E6E2), (0x1E6E4, 0x1E6E5), (0x1E6E7, 0x1E6ED), (0x1E6F0, 0x1E6F4), (0x1E6FE, 0x1E6FF),
    (0x1E7E0, 0x1E7E6), (0x1E7E8, 0x1E7EB), (0x1E7ED, 0x1E7EE), (0x1E7F0, 0x1E7FE), (0x1E800, 0x1E8C4), (0x1E8C7, 0x1E8CF),
    (0x1E900, 0x1E943), (0x1E94B, 0x1E94B), (0x1E950, 0x1E959), (0x1EC71, 0x1ECAB), (0x1ECAD, 0x1ECAF), (0x1ECB1, 0x1ECB4),
    (0x1ED01, 0x1ED2D), (0x1ED2F, 0x1ED3D), (0x1EE00, 0x1EE03), (0x1EE05, 0x1EE1F), (0x1EE21, 0x1EE22), (0x1EE24, 0x1EE24),
    (0x1EE27, 0x1EE27), (0x1EE29, 0x1EE32), (0x1EE34, 0x1EE37), (0x1EE39, 0x1EE39), (0x1EE3B, 0x1EE3B), (0x1EE42, 0x1EE42),
    (0x1EE47, 0x1EE47), (0x1EE49, 0x1EE49), (0x1EE4B, 0x1EE4B), (0x1EE4D, 0x1EE4F), (0x1EE51, 0x1EE52), (0x1EE54, 0x1EE54),
    (0x1EE57, 0x1EE57), (0x1EE59, 0x1EE59), (0x1EE5B, 0x1EE5B), (0x1EE5D, 0x1EE5D), (0x1EE5F, 0x1EE5F), (0x1EE61, 0x1EE62),
    (0x1EE64, 0x1EE64), (0x1EE67, 0x1EE6A), (0x1EE6C, 0x1EE72), (0x1EE74, 0x1EE77), (0x1EE79, 0x1EE7C), (0x1EE7E, 0x1EE7E),
    (0x1EE80, 0x1EE89), (0x1EE8B, 0x1EE9B), (0x1EEA1, 0x1EEA3), (0x1EEA5, 0x1EEA9), (0x1EEAB, 0x1EEBB), (0x1F100, 0x1F10C),
    (0x1FBF0, 0x1FBF9), (0x20000, 0x2A6DF), (0x2A700, 0x2B81D), (0x2B820, 0x2CEAD), (0x2CEB0, 0x2EBE0), (0x2EBF0, 0x2EE5D),
    (0x2F800, 0x2FA1D), (0x30000, 0x3134A), (0x31350, 0x33479),
];

#[cfg(test)]
#[path = "markdown_tests.rs"]
mod tests;
