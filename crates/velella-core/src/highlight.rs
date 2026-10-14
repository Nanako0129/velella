//! Code-block highlighting (docs/plans/markdown.md, K6 revision 3.1). Untrusted input.
//!
//! syntect's bundled Sublime grammars (`SyntaxSet::load_defaults_newlines`, the only loader called)
//! with VS Code's Dark+ theme, vendored as strict JSON in `assets/vscode-dark-plus/`. Only the
//! fence selects a grammar, through the language table below.
//!
//! Two halves. The table lookups (`header`, and `language` inside this module) are pure: they are
//! the only part the session path reaches. `warm`, `compute_job` and the statics run only on a
//! message store's worker thread (messages.rs) and in tests: the worker takes one message's code
//! blocks as a `Job`, computes them here off the store lock, and the store applies each result to
//! its own block only if that block is still the same.
//!
//! Limits, all failing to `None` (the block renders plain): per block, a line over
//! `MAX_LINE_BYTES` or more than `MAX_LINES` lines; per message, `MAX_SPANS` spans and one
//! `BUDGET`, checked before each line. A panic inside a block's computation is caught and gives
//! `None`. Nothing here logs.
use serde_json::Value;
use std::{
    panic::{catch_unwind, AssertUnwindSafe},
    str::FromStr,
    sync::{Arc, OnceLock},
    time::{Duration, Instant},
};
use syntect::{
    highlighting::{
        Color, FontStyle, HighlightState, Highlighter, RangedHighlightIterator, ScopeSelectors,
        Style, StyleModifier, Theme, ThemeItem, ThemeSettings,
    },
    parsing::{ParseState, ScopeStack, SyntaxReference, SyntaxSet},
};

/// A block with a longer line (bytes) is not highlighted. This caps a line's length, not its
/// cost: fancy-regex stops one search after 1,000,000 backtrack steps (syntect has no setter), and
/// a line runs many searches, so one capped line can still take hundreds of milliseconds (the
/// release corpus test records the worst per grammar). That time is the worker's alone.
const MAX_LINE_BYTES: usize = 1024;
/// A block with more lines is not highlighted (the Swift view drops line numbers past it too).
const MAX_LINES: usize = 2000;
/// Spans per message, after merging. The block that would pass it, and every later block of the
/// message, get `None`.
const MAX_SPANS: usize = 4096;
/// Wall-clock time per message, shared by its blocks and checked before each line; once spent,
/// every remaining block is `None`. A single line can overrun it (fancy-regex cannot be stopped).
const BUDGET: Duration = Duration::from_millis(250);

/// A highlighted code block: `lines` joined with `\n` give back the block's code exactly.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Highlighted {
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

/// One message's code blocks, (fence, code) in the store's walk order, mapped or not.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Job {
    pub(crate) id: u64,
    pub(crate) blocks: Vec<(Option<String>, String)>,
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

/// (syntect syntax name, header name) for a fence language. Pure: no static, no syntect.
fn language(fence: &str) -> Option<(&'static str, &'static str)> {
    let fence = fence.to_ascii_lowercase();
    LANGUAGES
        .iter()
        .find(|(aliases, ..)| aliases.contains(&fence.as_str()))
        .map(|&(_, syntax, name)| (syntax, name))
}

/// A code block's header: the table's name for its fence, `None` for none or an unmapped one.
/// Pure (the same lookup): the FFI conversion and the store's pending check call it.
pub fn header(language: Option<&str>) -> Option<&'static str> {
    language.and_then(self::language).map(|(_, name)| name)
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

// ---- Engine (the worker thread and tests only) ----

static SYNTAXES: OnceLock<SyntaxSet> = OnceLock::new();
static THEME: OnceLock<Option<Theme>> = OnceLock::new();

/// Builds the syntax set and the theme once; later calls return them. The worker calls it first;
/// nothing on the session path does. `None` only if the vendored theme does not build (the build
/// test pins that it does).
pub(crate) fn warm() -> Option<(&'static SyntaxSet, &'static Theme)> {
    let theme = THEME
        .get_or_init(|| build_theme(DARK_PLUS, DARK_VS).map(|(theme, _)| theme))
        .as_ref()?;
    Some((
        SYNTAXES.get_or_init(SyntaxSet::load_defaults_newlines),
        theme,
    ))
}

/// One result per block of `job`, in order: each block computed from its own code, under one
/// `BUDGET` and one `MAX_SPANS` allowance for the whole message.
pub(crate) fn compute_job(job: &Job) -> Vec<Option<Arc<Highlighted>>> {
    compute_job_with(job, &mut Instant::now)
}

/// `compute_job` with an injectable clock (tests pass a fake one; a frozen one lifts the budget).
pub(crate) fn compute_job_with(
    job: &Job,
    now: &mut dyn FnMut() -> Instant,
) -> Vec<Option<Arc<Highlighted>>> {
    let Some((set, theme)) = warm() else {
        return vec![None; job.blocks.len()];
    };
    let start = now();
    let mut spans_left = MAX_SPANS;
    let mut stopped = false;
    job.blocks
        .iter()
        .map(|(fence, code)| {
            if stopped {
                return None;
            }
            let (syntax, _) = fence.as_deref().and_then(language)?;
            let syntax = set.find_syntax_by_name(syntax)?;
            let spent = &mut || now().saturating_duration_since(start) >= BUDGET;
            match guarded(code, syntax, set, theme, spans_left, spent) {
                Outcome::Done(h) => {
                    spans_left -= h.lines.iter().map(Vec::len).sum::<usize>();
                    Some(Arc::new(h))
                }
                Outcome::Plain => None,
                Outcome::Stop => {
                    stopped = true;
                    None
                }
            }
        })
        .collect()
}

/// One block's computation.
enum Outcome {
    Done(Highlighted),
    /// This block stays plain (a cap, a parse error, a panic); the next one still runs.
    Plain,
    /// The budget or the span allowance ran out: this block and every later one stay plain.
    Stop,
}

/// `compute` with a panic caught as `Plain`. Only the computation is inside: it writes to locals
/// and returns its value, and no velella state is touched.
fn guarded(
    code: &str,
    syntax: &SyntaxReference,
    set: &SyntaxSet,
    theme: &Theme,
    spans_left: usize,
    spent: &mut dyn FnMut() -> bool,
) -> Outcome {
    catch_unwind(AssertUnwindSafe(|| {
        compute(code, syntax, set, theme, spans_left, spent)
    }))
    .unwrap_or(Outcome::Plain)
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

/// One block: `Plain` over a per-block cap or on a parse error, `Stop` once `spent()` or past
/// `spans_left`.
fn compute(
    code: &str,
    syntax: &SyntaxReference,
    set: &SyntaxSet,
    theme: &Theme,
    spans_left: usize,
    spent: &mut dyn FnMut() -> bool,
) -> Outcome {
    let lines: Vec<&str> = code.split('\n').collect();
    if lines.len() > MAX_LINES || lines.iter().any(|l| l.len() > MAX_LINE_BYTES) {
        return Outcome::Plain;
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
            return Outcome::Stop;
        }
        buf.clear();
        buf.push_str(line);
        buf.push('\n');
        let Ok(ops) = parser.parse_line(&buf, set) else {
            return Outcome::Plain;
        };
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
        if total > spans_left {
            return Outcome::Stop;
        }
        out.push(spans);
    }
    let highlighted = Highlighted { lines: out };
    // syntect's iterator stops early on a scope-stack error; such a result would not be the code.
    if highlighted.text() == code {
        Outcome::Done(highlighted)
    } else {
        Outcome::Plain
    }
}

#[cfg(test)]
#[path = "highlight_tests.rs"]
mod tests;
