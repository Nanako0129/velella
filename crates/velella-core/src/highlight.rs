//! Code-block highlighting (docs/plans/markdown.md, K6). Untrusted input.
//!
//! syntect's bundled Sublime grammars (`SyntaxSet::load_defaults_newlines`, the only loader called)
//! with VS Code's Dark+ theme, vendored as strict JSON in `assets/vscode-dark-plus/`. Only the
//! fence selects a grammar, through the language table below. A highlight lives on its own
//! `Block::CodeBlock` node: `fill` walks a body in place and computes each block from that block's
//! own `code`, so spans can never land on another block.
//!
//! Limits, all failing to `None` (the block renders plain): a line over `MAX_LINE_BYTES`, more
//! than `MAX_LINES` lines or `MAX_SPANS` spans, and one `BUDGET` per message shared by its blocks,
//! checked before each line. A panic inside the computation is caught and also gives `None`.
//! Nothing here logs.
use crate::markdown::{Block, Inline};
use serde_json::Value;
use std::{
    panic::{catch_unwind, AssertUnwindSafe},
    str::FromStr,
    sync::OnceLock,
    time::{Duration, Instant},
};
use syntect::{
    highlighting::{
        Color, FontStyle, HighlightState, Highlighter, RangedHighlightIterator, ScopeSelectors,
        Style, StyleModifier, Theme, ThemeItem, ThemeSettings,
    },
    parsing::{ParseState, ScopeStack, SyntaxReference, SyntaxSet},
};

/// A block with a longer line (bytes) is not highlighted. fancy-regex bounds one search (1,000,000
/// backtrack steps, syntect has no setter), not one line, so this is the real bound per line; the
/// release timing test measures the worst capped line.
const MAX_LINE_BYTES: usize = 1024;
/// A block with more lines is not highlighted (the Swift view drops line numbers past it too).
const MAX_LINES: usize = 2000;
/// A block with more spans after merging is not highlighted.
const MAX_SPANS: usize = 4096;
/// Wall-clock time per message, shared by its blocks; once spent, the rest are `None`.
const BUDGET: Duration = Duration::from_millis(25);

/// A highlighted code block: `name` is the header from the language table (never the fence
/// text); `lines` joined with `\n` give back the block's code exactly.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Highlighted {
    pub name: String,
    pub lines: Vec<Vec<Span>>,
}

/// A run of one style; adjacent runs of the same style are merged.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Span {
    pub text: String,
    /// 0xRRGGBB.
    pub rgb: u32,
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
}

impl Highlighted {
    /// The spans' text, lines joined with `\n`.
    pub fn text(&self) -> String {
        let lines: Vec<String> = self
            .lines
            .iter()
            .map(|l| l.iter().map(|s| s.text.as_str()).collect())
            .collect();
        lines.join("\n")
    }
}

/// Fence aliases (matched case-insensitively), syntect syntax name, header name: the plan's K6
/// language table. Every other fence, and none, is plain. No TypeScript grammar is bundled, so
/// `ts` takes the JavaScript one under its own name.
const LANGUAGES: &[(&[&str], &str, &str)] = &[
    (&["py", "python", "py3"], "Python", "Python"),
    (
        &["js", "javascript", "jsx", "mjs", "cjs"],
        "JavaScript",
        "JavaScript",
    ),
    (&["ts", "typescript", "tsx"], "JavaScript", "TypeScript"),
    (&["rs", "rust"], "Rust", "Rust"),
    (
        &["sh", "bash", "shell", "zsh", "console"],
        "Bourne Again Shell (bash)",
        "Shell",
    ),
    (&["c", "h"], "C", "C"),
    (&["cpp", "c++", "cc", "hpp", "cxx"], "C++", "C++"),
    (&["cs", "csharp", "c#"], "C#", "C#"),
    (&["go", "golang"], "Go", "Go"),
    (&["java"], "Java", "Java"),
    (&["json", "jsonc"], "JSON", "JSON"),
    (&["html", "htm", "xhtml"], "HTML", "HTML"),
    (&["css"], "CSS", "CSS"),
    (&["sql"], "SQL", "SQL"),
    (&["yaml", "yml"], "YAML", "YAML"),
    (&["xml", "svg"], "XML", "XML"),
    (&["rb", "ruby"], "Ruby", "Ruby"),
    (&["php"], "PHP", "PHP"),
    (&["lua"], "Lua", "Lua"),
    (&["diff", "patch"], "Diff", "Diff"),
    (&["md", "markdown"], "Markdown", "Markdown"),
    (&["hs", "haskell"], "Haskell", "Haskell"),
    (&["scala"], "Scala", "Scala"),
    (&["pl", "perl"], "Perl", "Perl"),
    (&["r"], "R", "R"),
    (&["makefile", "make", "mk"], "Makefile", "Makefile"),
    (&["tex", "latex"], "LaTeX", "LaTeX"),
    (&["clj", "clojure"], "Clojure", "Clojure"),
    (&["erl", "erlang"], "Erlang", "Erlang"),
    (&["ml", "ocaml"], "OCaml", "OCaml"),
    (&["m", "objc", "objective-c"], "Objective-C", "Objective-C"),
    (&["groovy", "gradle"], "Groovy", "Groovy"),
    (&["bat", "cmd", "batch"], "Batch File", "Batch"),
    (&["d"], "D", "D"),
    (&["pas", "pascal"], "Pascal", "Pascal"),
    (&["tcl"], "Tcl", "Tcl"),
    (&["lisp", "el", "scm"], "Lisp", "Lisp"),
];

/// (syntect syntax name, header name) for a fence language.
fn language(fence: &str) -> Option<(&'static str, &'static str)> {
    let fence = fence.to_ascii_lowercase();
    LANGUAGES
        .iter()
        .find(|(aliases, ..)| aliases.contains(&fence.as_str()))
        .map(|&(_, syntax, name)| (syntax, name))
}

// ---- Theme ----

const DARK_PLUS: &str = include_str!("../assets/vscode-dark-plus/dark_plus.json");
const DARK_VS: &str = include_str!("../assets/vscode-dark-plus/dark_vs.json");

/// `#RRGGBB` or `#RRGGBBAA` (alpha dropped).
fn hex(s: &str) -> Option<Color> {
    let digits = s.strip_prefix('#')?;
    if !matches!(digits.len(), 6 | 8) || !digits.is_ascii() {
        return None;
    }
    let byte = |i: usize| u8::from_str_radix(&digits[i..i + 2], 16).ok();
    Some(Color {
        r: byte(0)?,
        g: byte(2)?,
        b: byte(4)?,
        a: 0xFF,
    })
}

/// A VS Code `fontStyle`: space-separated words; `""` resets. syntect has no strikethrough.
fn font_style(s: &str) -> FontStyle {
    s.split_whitespace()
        .fold(FontStyle::empty(), |style, word| match word {
            "bold" => style | FontStyle::BOLD,
            "italic" => style | FontStyle::ITALIC,
            "underline" => style | FontStyle::UNDERLINE,
            _ => style,
        })
}

/// A syntect theme from a VS Code theme (`top`) and the one file it includes (`included`), with
/// the number of token-colour entries skipped because syntect cannot parse their selector. The
/// include resolves only to `included` (`./dark_vs.json`), which includes nothing itself.
fn build_theme(top: &str, included: &str) -> Option<(Theme, usize)> {
    let top: Value = serde_json::from_str(top).ok()?;
    let base: Value = serde_json::from_str(included).ok()?;
    if top.get("include").and_then(Value::as_str) != Some("./dark_vs.json")
        || base.get("include").is_some()
    {
        return None;
    }
    // VS Code's order: the included file's rules, then the including file's.
    let files = [&base, &top];
    let colour = |key: &str| {
        files
            .iter()
            .rev()
            .find_map(|f| f.get("colors")?.get(key)?.as_str())
            .and_then(hex)
    };
    let mut skipped = 0;
    let mut scopes = Vec::new();
    for entry in files
        .iter()
        .filter_map(|f| f.get("tokenColors")?.as_array())
        .flatten()
    {
        // A scope is a string or an array of strings.
        let selector = match entry.get("scope") {
            Some(Value::String(s)) => Some(s.clone()),
            Some(Value::Array(a)) => a
                .iter()
                .map(Value::as_str)
                .collect::<Option<Vec<_>>>()
                .map(|v| v.join(", ")),
            _ => None,
        };
        let Some(scope) = selector.and_then(|s| ScopeSelectors::from_str(&s).ok()) else {
            skipped += 1;
            continue;
        };
        let settings = entry.get("settings");
        let field = |key| settings.and_then(|s| s.get(key)?.as_str());
        scopes.push(ThemeItem {
            scope,
            style: StyleModifier {
                foreground: field("foreground").and_then(hex),
                background: None,
                font_style: field("fontStyle").map(font_style),
            },
        });
    }
    // syntect keeps the first of equally specific rules, VS Code the last.
    scopes.reverse();
    let theme = Theme {
        name: Some("Dark+".to_owned()),
        settings: ThemeSettings {
            foreground: Some(colour("editor.foreground")?),
            background: Some(colour("editor.background")?),
            ..ThemeSettings::default()
        },
        scopes,
        ..Theme::default()
    };
    Some((theme, skipped))
}

// ---- Engine ----

static SYNTAXES: OnceLock<SyntaxSet> = OnceLock::new();
static THEME: OnceLock<Option<Theme>> = OnceLock::new();
static GRAMMARS: OnceLock<()> = OnceLock::new();

/// Runs once through every grammar in the table, so the contexts and regexes syntect builds on
/// first use (measured in release: up to ~170 ms for one grammar, 35 ms for Python) are not
/// charged to the first message's budget.
const WARM_SAMPLE: &str = "import os\n# c\ndef f(x, y=1):\n    \"\"\"d\"\"\"\n    return 'a' + \"b\" if x else [1, 2.0, 0x1F]\nclass A(B):\n    pass\n// c /* d */ { let s = `t ${u}`; fn g<T>(a: &str) -> i32 { 0 } }\n<a href=\"x\">y</a> $v @w %z";

/// Builds the syntax set and the theme once, then runs `WARM_SAMPLE` through each grammar of the
/// table; later calls return at once. `MessageStore::new` runs it on a background thread, so the
/// first message's budget is not spent on any of this; a `fill` that comes first waits for it.
/// `None` only if the vendored theme does not build (the build test pins that it does).
pub fn warm() -> Option<(&'static SyntaxSet, &'static Theme)> {
    let theme = THEME
        .get_or_init(|| build_theme(DARK_PLUS, DARK_VS).map(|(theme, _)| theme))
        .as_ref()?;
    let set = SYNTAXES.get_or_init(SyntaxSet::load_defaults_newlines);
    GRAMMARS.get_or_init(|| {
        for (_, name, _) in LANGUAGES {
            if let Some(syntax) = set.find_syntax_by_name(name) {
                guarded(WARM_SAMPLE, syntax, name, set, theme, &mut || false);
            }
        }
    });
    Some((set, theme))
}

/// Fills the `highlight` of every code block in `blocks` (also inside `Inline::Block`, in
/// document order) from that block's own code, sharing one `BUDGET` that starts once the engine
/// is warm. A body without a block to highlight neither waits for `warm` nor reads the clock.
pub fn fill(blocks: &mut [Block]) {
    fill_with(blocks, &mut Instant::now);
}

/// `fill` with an injectable clock (tests pass a fake one; a frozen one lifts the budget).
pub fn fill_with(blocks: &mut [Block], now: &mut dyn FnMut() -> Instant) {
    let mut start = None;
    each_code_block(blocks, &mut |code, language| {
        let (syntax, name) = language.and_then(self::language)?;
        let (set, theme) = warm()?;
        let syntax = set.find_syntax_by_name(syntax)?;
        // One budget for the whole message, started at its first block to highlight.
        let start = *start.get_or_insert_with(&mut *now);
        guarded(code, syntax, name, set, theme, &mut || {
            now().saturating_duration_since(start) >= BUDGET
        })
    });
}

/// `compute` with a panic caught as `None`. Only the computation is inside: it writes to locals
/// and returns its value, and no velella state is touched.
fn guarded(
    code: &str,
    syntax: &SyntaxReference,
    name: &str,
    set: &SyntaxSet,
    theme: &Theme,
    spent: &mut dyn FnMut() -> bool,
) -> Option<Highlighted> {
    catch_unwind(AssertUnwindSafe(|| {
        compute(code, syntax, name, set, theme, spent)
    }))
    .ok()
    .flatten()
}

/// Sets each code block's `highlight` to `f(code, language)`, walking every block list and inline
/// container. Depth is bounded by the parser's nesting limit.
fn each_code_block(
    blocks: &mut [Block],
    f: &mut impl FnMut(&str, Option<&str>) -> Option<Highlighted>,
) {
    for b in blocks {
        match b {
            Block::Paragraph(c) | Block::Subtext(c) | Block::Heading { content: c, .. } => {
                each_code_block_in(c, f)
            }
            Block::Quote(inner) => each_code_block(inner, f),
            Block::List { items, .. } => items.iter_mut().for_each(|i| each_code_block(i, f)),
            Block::CodeBlock {
                language,
                code,
                highlight,
            } => *highlight = f(code, language.as_deref()),
        }
    }
}

fn each_code_block_in(
    inlines: &mut [Inline],
    f: &mut impl FnMut(&str, Option<&str>) -> Option<Highlighted>,
) {
    for i in inlines {
        match i {
            Inline::Bold(c)
            | Inline::Italic(c)
            | Inline::Underline(c)
            | Inline::Strike(c)
            | Inline::Spoiler(c)
            | Inline::Link { label: c, .. } => each_code_block_in(c, f),
            Inline::Block(b) => each_code_block(std::slice::from_mut(&mut **b), f),
            _ => {}
        }
    }
}

fn span(text: &str, style: Style) -> Span {
    let c = style.foreground;
    Span {
        text: text.to_owned(),
        rgb: u32::from(c.r) << 16 | u32::from(c.g) << 8 | u32::from(c.b),
        bold: style.font_style.contains(FontStyle::BOLD),
        italic: style.font_style.contains(FontStyle::ITALIC),
        underline: style.font_style.contains(FontStyle::UNDERLINE),
    }
}

/// One block's highlight, or `None` over a limit, on a parse error, or once `spent()`.
fn compute(
    code: &str,
    syntax: &SyntaxReference,
    name: &str,
    set: &SyntaxSet,
    theme: &Theme,
    spent: &mut dyn FnMut() -> bool,
) -> Option<Highlighted> {
    let lines: Vec<&str> = code.split('\n').collect();
    if lines.len() > MAX_LINES || lines.iter().any(|l| l.len() > MAX_LINE_BYTES) {
        return None;
    }
    let highlighter = Highlighter::new(theme);
    let mut parser = ParseState::new(syntax);
    let mut state = HighlightState::new(&highlighter, ScopeStack::new());
    let mut out = Vec::with_capacity(lines.len());
    let mut total = 0;
    // The grammars are the "newlines" set: each line is parsed with its `\n`, which no span keeps.
    let mut buf = String::with_capacity(MAX_LINE_BYTES + 1);
    for line in lines {
        if spent() {
            return None;
        }
        buf.clear();
        buf.push_str(line);
        buf.push('\n');
        let ops = parser.parse_line(&buf, set).ok()?;
        let mut spans: Vec<Span> = Vec::new();
        for (style, _, range) in RangedHighlightIterator::new(&mut state, &ops, &buf, &highlighter)
        {
            let text = &line[range.start.min(line.len())..range.end.min(line.len())];
            if text.is_empty() {
                continue;
            }
            let next = span(text, style);
            match spans.last_mut() {
                Some(last)
                    if (last.rgb, last.bold, last.italic, last.underline)
                        == (next.rgb, next.bold, next.italic, next.underline) =>
                {
                    last.text.push_str(text)
                }
                _ => spans.push(next),
            }
        }
        total += spans.len();
        if total > MAX_SPANS {
            return None;
        }
        out.push(spans);
    }
    let highlighted = Highlighted {
        name: name.to_owned(),
        lines: out,
    };
    // syntect's iterator stops early on a scope-stack error; such a result would not be the code.
    (highlighted.text() == code).then_some(highlighted)
}

#[cfg(test)]
#[path = "highlight_tests.rs"]
mod tests;
