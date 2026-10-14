//! The message-body AST across FFI (docs/plans/markdown.md, K2): a field-for-field mirror of
//! `velella_core::markdown`, plus the resolved mentions and emoji images a row carries. Names
//! carry an `Md` prefix so the Swift shell's own `MarkdownBlock`/`MarkdownInline` do not clash.
use velella_core::{
    highlight::{self, Highlighted, Span},
    markdown::{Block, Inline, NavTarget, TimestampStyle},
};

#[derive(Debug, PartialEq, uniffi::Enum)]
pub enum MdBlock {
    Paragraph {
        content: Vec<MdInline>,
    },
    /// `level` is 1-3.
    Heading {
        level: u8,
        content: Vec<MdInline>,
    },
    Subtext {
        content: Vec<MdInline>,
    },
    /// Never contains a `Quote`.
    Quote {
        blocks: Vec<MdBlock>,
    },
    List {
        ordered: bool,
        start: u32,
        items: Vec<Vec<MdBlock>>,
    },
    /// `header` is the language table's name for the fence (never the fence text), present
    /// whether or not the colours have arrived. `highlight` was computed in core from this block's
    /// own `code` (K6); `None` renders plain.
    CodeBlock {
        language: Option<String>,
        code: String,
        header: Option<String>,
        highlight: Option<MdHighlighted>,
    },
}

/// A code block's highlight: `lines` joined with `\n` give the block's code.
#[derive(Debug, PartialEq, uniffi::Record)]
pub struct MdHighlighted {
    pub lines: Vec<Vec<MdSpan>>,
}

/// A run of one style; `rgb` is 0xRRGGBB.
#[derive(Debug, PartialEq, uniffi::Record)]
pub struct MdSpan {
    pub text: String,
    pub rgb: u32,
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
}

#[derive(Debug, PartialEq, uniffi::Enum)]
pub enum MdInline {
    Text {
        text: String,
    },
    Bold {
        content: Vec<MdInline>,
    },
    Italic {
        content: Vec<MdInline>,
    },
    Underline {
        content: Vec<MdInline>,
    },
    Strike {
        content: Vec<MdInline>,
    },
    Spoiler {
        content: Vec<MdInline>,
    },
    Code {
        text: String,
    },
    /// `url` is canonical (http or https), `host` its ASCII host; `label` holds no `Link`.
    Link {
        url: String,
        host: String,
        label: Vec<MdInline>,
        masked: bool,
    },
    UserMention {
        id: u64,
    },
    RoleMention {
        id: u64,
    },
    ChannelMention {
        id: u64,
    },
    Everyone,
    Here,
    CustomEmoji {
        id: u64,
        name: String,
        animated: bool,
    },
    Timestamp {
        unix: i64,
        style: MdTimestampStyle,
    },
    SlashCommand {
        name: String,
        id: u64,
    },
    GuildNavigation {
        guild: u64,
        target: MdNavTarget,
    },
    LineBreak,
    /// A block inside an inline container, kept in place.
    Block {
        block: MdBlock,
    },
}

#[derive(Debug, PartialEq, uniffi::Enum)]
pub enum MdTimestampStyle {
    ShortTime,
    MediumTime,
    ShortDate,
    LongDate,
    LongDateShortTime,
    FullDateShortTime,
    ShortDateShortTime,
    ShortDateMediumTime,
    Relative,
}

#[derive(Debug, PartialEq, uniffi::Enum)]
pub enum MdNavTarget {
    Customize,
    Browse,
    Guide,
    LinkedRoles { role: Option<u64> },
}

#[derive(Debug, PartialEq, uniffi::Enum)]
pub enum MdMentionKind {
    User,
    Role,
    Channel,
}

/// `name` is `None` for an unknown user or channel or a deleted role; `color` is a role's
/// 0xRRGGBB (0 for none).
#[derive(Debug, PartialEq, uniffi::Record)]
pub struct MdMention {
    pub kind: MdMentionKind,
    pub id: u64,
    pub name: Option<String>,
    pub color: u32,
}

/// A custom emoji's image URL, built in core from the id.
#[derive(Debug, PartialEq, uniffi::Record)]
pub struct MdEmoji {
    pub id: u64,
    pub animated: bool,
    pub url: String,
}

fn inlines(v: Vec<Inline>) -> Vec<MdInline> {
    v.into_iter().map(Into::into).collect()
}

pub(crate) fn blocks(v: Vec<Block>) -> Vec<MdBlock> {
    v.into_iter().map(Into::into).collect()
}

impl From<Block> for MdBlock {
    fn from(b: Block) -> Self {
        match b {
            Block::Paragraph(c) => Self::Paragraph {
                content: inlines(c),
            },
            Block::Heading { level, content } => Self::Heading {
                level,
                content: inlines(content),
            },
            Block::Subtext(c) => Self::Subtext {
                content: inlines(c),
            },
            Block::Quote(b) => Self::Quote { blocks: blocks(b) },
            Block::List {
                ordered,
                start,
                items,
            } => Self::List {
                ordered,
                start,
                items: items.into_iter().map(blocks).collect(),
            },
            Block::CodeBlock {
                language,
                code,
                highlight,
            } => Self::CodeBlock {
                header: highlight::header(language.as_deref()).map(str::to_owned),
                language,
                code,
                // The spans are copied here, after `snapshot()` returned: off the store lock.
                highlight: highlight.as_deref().map(Into::into),
            },
        }
    }
}

impl From<Inline> for MdInline {
    fn from(i: Inline) -> Self {
        match i {
            Inline::Text(text) => Self::Text { text },
            Inline::Bold(c) => Self::Bold {
                content: inlines(c),
            },
            Inline::Italic(c) => Self::Italic {
                content: inlines(c),
            },
            Inline::Underline(c) => Self::Underline {
                content: inlines(c),
            },
            Inline::Strike(c) => Self::Strike {
                content: inlines(c),
            },
            Inline::Spoiler(c) => Self::Spoiler {
                content: inlines(c),
            },
            Inline::Code(text) => Self::Code { text },
            Inline::Link {
                url,
                host,
                label,
                masked,
            } => Self::Link {
                url,
                host,
                label: inlines(label),
                masked,
            },
            Inline::UserMention(id) => Self::UserMention { id },
            Inline::RoleMention(id) => Self::RoleMention { id },
            Inline::ChannelMention(id) => Self::ChannelMention { id },
            Inline::Everyone => Self::Everyone,
            Inline::Here => Self::Here,
            Inline::CustomEmoji { id, name, animated } => Self::CustomEmoji { id, name, animated },
            Inline::Timestamp { unix, style } => Self::Timestamp {
                unix,
                style: style.into(),
            },
            Inline::SlashCommand { name, id } => Self::SlashCommand { name, id },
            Inline::GuildNavigation { guild, target } => Self::GuildNavigation {
                guild,
                target: target.into(),
            },
            Inline::LineBreak => Self::LineBreak,
            Inline::Block(b) => Self::Block { block: (*b).into() },
        }
    }
}

impl From<TimestampStyle> for MdTimestampStyle {
    fn from(s: TimestampStyle) -> Self {
        match s {
            TimestampStyle::ShortTime => Self::ShortTime,
            TimestampStyle::MediumTime => Self::MediumTime,
            TimestampStyle::ShortDate => Self::ShortDate,
            TimestampStyle::LongDate => Self::LongDate,
            TimestampStyle::LongDateShortTime => Self::LongDateShortTime,
            TimestampStyle::FullDateShortTime => Self::FullDateShortTime,
            TimestampStyle::ShortDateShortTime => Self::ShortDateShortTime,
            TimestampStyle::ShortDateMediumTime => Self::ShortDateMediumTime,
            TimestampStyle::Relative => Self::Relative,
        }
    }
}

impl From<NavTarget> for MdNavTarget {
    fn from(t: NavTarget) -> Self {
        match t {
            NavTarget::Customize => Self::Customize,
            NavTarget::Browse => Self::Browse,
            NavTarget::Guide => Self::Guide,
            NavTarget::LinkedRoles(role) => Self::LinkedRoles { role },
        }
    }
}

impl From<&Highlighted> for MdHighlighted {
    fn from(h: &Highlighted) -> Self {
        Self {
            lines: h
                .lines
                .iter()
                .map(|l| l.iter().map(Into::into).collect())
                .collect(),
        }
    }
}

impl From<&Span> for MdSpan {
    fn from(s: &Span) -> Self {
        Self {
            text: s.text.clone(),
            rgb: s.rgb,
            bold: s.bold,
            italic: s.italic,
            underline: s.underline,
        }
    }
}

impl From<velella_core::ResolvedMention> for MdMention {
    fn from(m: velella_core::ResolvedMention) -> Self {
        Self {
            kind: match m.kind {
                velella_core::MentionKind::User => MdMentionKind::User,
                velella_core::MentionKind::Role => MdMentionKind::Role,
                velella_core::MentionKind::Channel => MdMentionKind::Channel,
            },
            id: m.id,
            name: m.name,
            color: m.color,
        }
    }
}

impl From<velella_core::EmojiImage> for MdEmoji {
    fn from(e: velella_core::EmojiImage) -> Self {
        Self {
            id: e.id,
            animated: e.animated,
            url: e.url,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use velella_core::markdown::parse;

    fn text(s: &str) -> Inline {
        Inline::Text(s.into())
    }

    fn mtext(s: &str) -> MdInline {
        MdInline::Text { text: s.into() }
    }

    /// Every `Block`, `Inline`, `TimestampStyle` and `NavTarget` variant converts to its mirror.
    #[test]
    fn every_ast_variant_converts_to_its_mirror() {
        use TimestampStyle as S;
        let styles = [
            (S::ShortTime, MdTimestampStyle::ShortTime),
            (S::MediumTime, MdTimestampStyle::MediumTime),
            (S::ShortDate, MdTimestampStyle::ShortDate),
            (S::LongDate, MdTimestampStyle::LongDate),
            (S::LongDateShortTime, MdTimestampStyle::LongDateShortTime),
            (S::FullDateShortTime, MdTimestampStyle::FullDateShortTime),
            (S::ShortDateShortTime, MdTimestampStyle::ShortDateShortTime),
            (
                S::ShortDateMediumTime,
                MdTimestampStyle::ShortDateMediumTime,
            ),
            (S::Relative, MdTimestampStyle::Relative),
        ];
        let (core_ts, md_ts): (Vec<_>, Vec<_>) = styles
            .into_iter()
            .enumerate()
            .map(|(i, (s, m))| {
                let unix = -(i as i64);
                (
                    Inline::Timestamp { unix, style: s },
                    MdInline::Timestamp { unix, style: m },
                )
            })
            .unzip();
        let targets = [
            (NavTarget::Customize, MdNavTarget::Customize),
            (NavTarget::Browse, MdNavTarget::Browse),
            (NavTarget::Guide, MdNavTarget::Guide),
            (
                NavTarget::LinkedRoles(None),
                MdNavTarget::LinkedRoles { role: None },
            ),
            (
                NavTarget::LinkedRoles(Some(5)),
                MdNavTarget::LinkedRoles { role: Some(5) },
            ),
        ];
        let (core_nav, md_nav): (Vec<_>, Vec<_>) = targets
            .into_iter()
            .map(|(t, m)| {
                (
                    Inline::GuildNavigation {
                        guild: 9,
                        target: t,
                    },
                    MdInline::GuildNavigation {
                        guild: 9,
                        target: m,
                    },
                )
            })
            .unzip();

        let core_inlines = [
            vec![
                text("t"),
                Inline::Bold(vec![text("b")]),
                Inline::Italic(vec![text("i")]),
                Inline::Underline(vec![text("u")]),
                Inline::Strike(vec![text("s")]),
                Inline::Spoiler(vec![Inline::Block(Box::new(Block::CodeBlock {
                    language: None,
                    code: "x".into(),
                    highlight: None,
                }))]),
                Inline::Code("c".into()),
                Inline::Link {
                    url: "https://a.b/".into(),
                    host: "a.b".into(),
                    label: vec![Inline::Bold(vec![text("l")])],
                    masked: true,
                },
                Inline::UserMention(u64::MAX),
                Inline::RoleMention(2),
                Inline::ChannelMention(3),
                Inline::Everyone,
                Inline::Here,
                Inline::CustomEmoji {
                    id: 4,
                    name: "cat".into(),
                    animated: true,
                },
                Inline::SlashCommand {
                    name: "cmd sub".into(),
                    id: 6,
                },
                Inline::LineBreak,
            ],
            core_ts,
            core_nav,
        ]
        .into_iter()
        .flatten()
        .collect::<Vec<_>>();
        let md_inlines = [
            vec![
                mtext("t"),
                MdInline::Bold {
                    content: vec![mtext("b")],
                },
                MdInline::Italic {
                    content: vec![mtext("i")],
                },
                MdInline::Underline {
                    content: vec![mtext("u")],
                },
                MdInline::Strike {
                    content: vec![mtext("s")],
                },
                MdInline::Spoiler {
                    content: vec![MdInline::Block {
                        block: MdBlock::CodeBlock {
                            language: None,
                            code: "x".into(),
                            header: None,
                            highlight: None,
                        },
                    }],
                },
                MdInline::Code { text: "c".into() },
                MdInline::Link {
                    url: "https://a.b/".into(),
                    host: "a.b".into(),
                    label: vec![MdInline::Bold {
                        content: vec![mtext("l")],
                    }],
                    masked: true,
                },
                MdInline::UserMention { id: u64::MAX },
                MdInline::RoleMention { id: 2 },
                MdInline::ChannelMention { id: 3 },
                MdInline::Everyone,
                MdInline::Here,
                MdInline::CustomEmoji {
                    id: 4,
                    name: "cat".into(),
                    animated: true,
                },
                MdInline::SlashCommand {
                    name: "cmd sub".into(),
                    id: 6,
                },
                MdInline::LineBreak,
            ],
            md_ts,
            md_nav,
        ]
        .into_iter()
        .flatten()
        .collect::<Vec<_>>();

        let core = vec![
            Block::Paragraph(core_inlines),
            Block::Heading {
                level: 3,
                content: vec![text("h")],
            },
            Block::Subtext(vec![text("s")]),
            Block::Quote(vec![Block::Paragraph(vec![text("q")])]),
            Block::List {
                ordered: true,
                start: u32::MAX,
                items: vec![vec![Block::Paragraph(vec![text("a")])], vec![]],
            },
            Block::CodeBlock {
                language: Some("rs".into()),
                code: "fn\n".into(),
                highlight: Some(Arc::new(Highlighted {
                    lines: vec![
                        vec![Span {
                            text: "fn".into(),
                            rgb: 0x569CD6,
                            bold: true,
                            italic: false,
                            underline: true,
                        }],
                        vec![],
                    ],
                })),
            },
            // An unmapped fence: no header.
            Block::CodeBlock {
                language: Some("swift".into()),
                code: "let".into(),
                highlight: None,
            },
        ];
        let md = vec![
            MdBlock::Paragraph {
                content: md_inlines,
            },
            MdBlock::Heading {
                level: 3,
                content: vec![mtext("h")],
            },
            MdBlock::Subtext {
                content: vec![mtext("s")],
            },
            MdBlock::Quote {
                blocks: vec![MdBlock::Paragraph {
                    content: vec![mtext("q")],
                }],
            },
            MdBlock::List {
                ordered: true,
                start: u32::MAX,
                items: vec![
                    vec![MdBlock::Paragraph {
                        content: vec![mtext("a")],
                    }],
                    vec![],
                ],
            },
            MdBlock::CodeBlock {
                language: Some("rs".into()),
                code: "fn\n".into(),
                header: Some("Rust".into()),
                highlight: Some(MdHighlighted {
                    lines: vec![
                        vec![MdSpan {
                            text: "fn".into(),
                            rgb: 0x569CD6,
                            bold: true,
                            italic: false,
                            underline: true,
                        }],
                        vec![],
                    ],
                }),
            },
            MdBlock::CodeBlock {
                language: Some("swift".into()),
                code: "let".into(),
                header: None,
                highlight: None,
            },
        ];
        assert_eq!(blocks(core), md);
    }

    /// K1 drops a break beside an `Inline::Block` only; one beside a spoiler holding a block
    /// reaches Swift, whose paragraph split drops it (`InlinePart.split`).
    #[test]
    fn a_break_beside_a_spoiler_holding_a_block_reaches_swift() {
        assert_eq!(
            blocks(parse("a\n||```\nx\n```||")),
            [MdBlock::Paragraph {
                content: vec![
                    mtext("a"),
                    MdInline::LineBreak,
                    MdInline::Spoiler {
                        content: vec![MdInline::Block {
                            block: MdBlock::CodeBlock {
                                language: None,
                                code: "x".into(),
                                header: None,
                                highlight: None,
                            }
                        }]
                    }
                ]
            }]
        );
    }

    /// The text of a converted code block's spans, lines joined with `\n`.
    fn joined(h: &MdHighlighted) -> String {
        let lines: Vec<String> = h
            .lines
            .iter()
            .map(|l| l.iter().map(|s| s.text.as_str()).collect())
            .collect();
        lines.join("\n")
    }

    /// One line's synthetic highlight: a span per character, so a misplaced one shows.
    fn spans_of(code: &str) -> Arc<Highlighted> {
        let line = |l: &str| {
            l.chars()
                .map(|c| Span {
                    text: c.to_string(),
                    rgb: 0x9CDCFE,
                    bold: false,
                    italic: false,
                    underline: false,
                })
                .collect()
        };
        Arc::new(Highlighted {
            lines: code.split('\n').map(line).collect(),
        })
    }

    /// Plan K6-1: a hidden-spoiler code block before a visible one. Each block's highlight sits on
    /// its own node (core's `apply` sets it there, messages_tests.rs), and the conversion keeps it
    /// there: each converted `MdBlock::CodeBlock`'s spans join to its own code, with its header.
    #[test]
    fn a_hidden_spoiler_code_block_never_lends_its_spans_to_a_visible_one() {
        let mut body = parse("||```py\nsecret = 1\n```|| then\n```rb\nshown = 2\n```");
        let [Block::Paragraph(content), Block::CodeBlock {
            code, highlight, ..
        }] = body.as_mut_slice()
        else {
            panic!("unexpected core shape");
        };
        *highlight = Some(spans_of(code));
        let [Inline::Spoiler(hidden), ..] = content.as_mut_slice() else {
            panic!("no spoiler first");
        };
        let [Inline::Block(b)] = hidden.as_mut_slice() else {
            panic!("no hidden block");
        };
        let Block::CodeBlock {
            code, highlight, ..
        } = &mut **b
        else {
            panic!("not a code block");
        };
        *highlight = Some(spans_of(code));

        let md = blocks(body);
        let [MdBlock::Paragraph { content }, MdBlock::CodeBlock {
            code: visible_code,
            header: visible_header,
            highlight: Some(visible),
            ..
        }] = md.as_slice()
        else {
            panic!("unexpected shape {md:?}");
        };
        assert_eq!(visible_code, "shown = 2");
        assert_eq!(joined(visible), "shown = 2");
        assert_eq!(visible_header.as_deref(), Some("Ruby"));
        let [MdInline::Spoiler { content: hidden }, ..] = content.as_slice() else {
            panic!("no spoiler first: {content:?}");
        };
        let [MdInline::Block {
            block:
                MdBlock::CodeBlock {
                    code: hidden_code,
                    header: hidden_header,
                    highlight: Some(hidden),
                    ..
                },
        }] = hidden.as_slice()
        else {
            panic!("no hidden code block: {hidden:?}");
        };
        assert_eq!(
            (hidden_code.as_str(), joined(hidden).as_str()),
            ("secret = 1", "secret = 1")
        );
        assert_eq!(hidden_header.as_deref(), Some("Python"));
    }

    /// What the Swift shell receives for the trap inputs of the plan's K2 acceptance: an
    /// over-long list number arrives capped at 1_000_000_000 (K5, the official client's value);
    /// out-of-range timestamp and id stay text (K1 D2, D3).
    #[test]
    fn trap_inputs_arrive_as_text_or_bounded_values() {
        let para = |c: Vec<MdInline>| vec![MdBlock::Paragraph { content: c }];
        assert_eq!(
            blocks(parse("4294967295. a")),
            vec![MdBlock::List {
                ordered: true,
                start: 1_000_000_000,
                items: vec![para(vec![mtext("a")])],
            }]
        );
        for input in ["<t:9223372036854775807>", "<@99999999999999999999999>"] {
            assert_eq!(blocks(parse(input)), para(vec![mtext(input)]), "{input}");
        }
        assert_eq!(
            blocks(parse("<t:-1>")),
            para(vec![MdInline::Timestamp {
                unix: -1,
                style: MdTimestampStyle::LongDateShortTime
            }])
        );
    }
}
