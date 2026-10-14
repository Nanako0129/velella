//! Highlighter tests (docs/plans/markdown.md, K6 rev 2 "Tests" kept by rev 3.1, and rev 3.1's
//! per-job caps and release corpus): the language table and headers, the theme build, Dark+
//! colours, the concatenation property, the per-block and per-message limits, panics, every bundled
//! syntax, and the worst line per grammar.
use super::*;
use crate::markdown::{parse, Block, Inline};
use std::time::Instant;

/// A clock that never moves: no budget is spent.
fn frozen() -> impl FnMut() -> Instant {
    let t = Instant::now();
    move || t
}

/// A clock that moves `step_ms` on every reading.
fn ticking(step_ms: u64) -> impl FnMut() -> Instant {
    let base = Instant::now();
    let mut t = 0;
    move || {
        t += step_ms;
        base + Duration::from_millis(t)
    }
}

fn job(blocks: &[(Option<&str>, &str)]) -> Job {
    Job {
        id: 1,
        blocks: blocks
            .iter()
            .map(|(f, c)| (f.map(str::to_owned), (*c).to_owned()))
            .collect(),
    }
}

/// One block with no budget.
fn highlighted(fence: &str, code: &str) -> Option<Arc<Highlighted>> {
    compute_job_with(&job(&[(Some(fence), code)]), &mut frozen())
        .pop()
        .flatten()
}

fn some(results: &[Option<Arc<Highlighted>>]) -> Vec<bool> {
    results.iter().map(Option::is_some).collect()
}

/// Every code block's (fence, code) in `blocks`, inline containers included, in document order.
fn code_blocks(blocks: &[Block]) -> Vec<(Option<String>, String)> {
    fn inl(i: &[Inline], out: &mut Vec<(Option<String>, String)>) {
        for x in i {
            match x {
                Inline::Bold(c)
                | Inline::Italic(c)
                | Inline::Underline(c)
                | Inline::Strike(c)
                | Inline::Spoiler(c)
                | Inline::Link { label: c, .. } => inl(c, out),
                Inline::Block(b) => blk(std::slice::from_ref(b), out),
                _ => {}
            }
        }
    }
    fn blk(b: &[Block], out: &mut Vec<(Option<String>, String)>) {
        for x in b {
            match x {
                Block::Paragraph(c) | Block::Subtext(c) | Block::Heading { content: c, .. } => {
                    inl(c, out)
                }
                Block::Quote(q) => blk(q, out),
                Block::List { items, .. } => items.iter().for_each(|i| blk(i, out)),
                Block::CodeBlock { language, code, .. } => {
                    out.push((language.clone(), code.clone()))
                }
            }
        }
    }
    let mut out = Vec::new();
    blk(blocks, &mut out);
    out
}

fn rgb(h: &Highlighted, text: &str) -> u32 {
    h.lines
        .iter()
        .flatten()
        .find(|s| s.text.trim() == text)
        .unwrap_or_else(|| panic!("no span {text:?} in {h:?}"))
        .rgb
}

fn span_count(h: &Highlighted) -> usize {
    h.lines.iter().map(Vec::len).sum()
}

// ---- Language table ----

/// The plan's table, row for row: fence aliases, syntect syntax, header name.
const PLAN_TABLE: &[(&str, &str, &str)] = &[
    ("py python py3", "Python", "Python"),
    ("js javascript jsx mjs cjs", "JavaScript", "JavaScript"),
    ("ts typescript tsx", "JavaScript", "TypeScript"),
    ("rs rust", "Rust", "Rust"),
    (
        "sh bash shell zsh console",
        "Bourne Again Shell (bash)",
        "Shell",
    ),
    ("c h", "C", "C"),
    ("cpp c++ cc hpp cxx", "C++", "C++"),
    ("cs csharp c#", "C#", "C#"),
    ("go golang", "Go", "Go"),
    ("java", "Java", "Java"),
    ("json jsonc", "JSON", "JSON"),
    ("html htm xhtml", "HTML", "HTML"),
    ("css", "CSS", "CSS"),
    ("sql", "SQL", "SQL"),
    ("yaml yml", "YAML", "YAML"),
    ("xml svg", "XML", "XML"),
    ("rb ruby", "Ruby", "Ruby"),
    ("php", "PHP", "PHP"),
    ("lua", "Lua", "Lua"),
    ("diff patch", "Diff", "Diff"),
    ("md markdown", "Markdown", "Markdown"),
    ("hs haskell", "Haskell", "Haskell"),
    ("scala", "Scala", "Scala"),
    ("pl perl", "Perl", "Perl"),
    ("r", "R", "R"),
    ("makefile make mk", "Makefile", "Makefile"),
    ("tex latex", "LaTeX", "LaTeX"),
    ("clj clojure", "Clojure", "Clojure"),
    ("erl erlang", "Erlang", "Erlang"),
    ("ml ocaml", "OCaml", "OCaml"),
    ("m objc objective-c", "Objective-C", "Objective-C"),
    ("groovy gradle", "Groovy", "Groovy"),
    ("bat cmd batch", "Batch File", "Batch"),
    ("d", "D", "D"),
    ("pas pascal", "Pascal", "Pascal"),
    ("tcl", "Tcl", "Tcl"),
    ("lisp el scm", "Lisp", "Lisp"),
];

#[test]
fn every_row_of_the_language_table_maps_and_nothing_else_does() {
    let mut aliases = 0;
    for (fences, syntax, name) in PLAN_TABLE {
        for fence in fences.split(' ') {
            aliases += 1;
            assert_eq!(language(fence), Some((*syntax, *name)), "{fence}");
            // Case-insensitive.
            assert_eq!(language(&fence.to_uppercase()), Some((*syntax, *name)));
            // The header is the table's name, not the fence text.
            assert_eq!(header(Some(fence)), Some(*name), "{fence}");
        }
    }
    let table: usize = LANGUAGES.iter().map(|(a, ..)| a.len()).sum();
    assert_eq!(aliases, table, "the code's table has rows the plan's lacks");
    assert_eq!(language("ts"), Some(("JavaScript", "TypeScript")));
    assert_eq!(header(Some("TS")), Some("TypeScript"));
    for other in [
        "swift",
        "kotlin",
        "toml",
        "dockerfile",
        "",
        "pyy",
        "javascript ",
    ] {
        assert_eq!(language(other), None, "{other}");
        assert_eq!(header(Some(other)), None, "{other}");
    }
    assert_eq!(header(None), None);
    // No language, or one off the table: no highlight.
    assert_eq!(highlighted("swift", "let x = 1"), None);
    let none = compute_job_with(&job(&[(None, "x = 1")]), &mut frozen());
    assert_eq!(none, [None]);
}

#[test]
fn every_mapped_syntax_exists_in_the_bundled_set() {
    let set = SyntaxSet::load_defaults_newlines();
    for (_, syntax, _) in LANGUAGES {
        assert!(set.find_syntax_by_name(syntax).is_some(), "{syntax}");
    }
}

// ---- Theme ----

#[test]
fn the_vendored_theme_builds_whole() {
    let (theme, skipped) = build_theme(DARK_PLUS, DARK_VS).expect("Dark+ builds");
    assert_eq!(skipped, 0);
    assert!(theme.scopes.len() >= 60, "{} rules", theme.scopes.len());
    let colour = |c: Option<Color>| c.map(|c| (c.r, c.g, c.b));
    assert_eq!(colour(theme.settings.background), Some((0x1E, 0x1E, 0x1E)));
    assert_eq!(colour(theme.settings.foreground), Some((0xD4, 0xD4, 0xD4)));
    let style = |scopes: &str| {
        let stack = ScopeStack::from_str(scopes).unwrap();
        let c = Highlighter::new(&theme)
            .style_for_stack(stack.as_slice())
            .foreground;
        u32::from(c.r) << 16 | u32::from(c.g) << 8 | u32::from(c.b)
    };
    assert_eq!(style("keyword"), 0x569CD6);
    // dark_vs and dark_plus both style `keyword.control`; the including file wins, as in VS Code.
    assert_eq!(style("source.python keyword.control.flow"), 0xC586C0);
    // Plain text takes the editor foreground.
    assert_eq!(style("source.python"), 0xD4D4D4);
    // An include other than the one embedded file is refused, and so is a nested one.
    assert!(build_theme(&DARK_PLUS.replace("./dark_vs.json", "./x.json"), DARK_VS).is_none());
    assert!(build_theme(DARK_PLUS, DARK_PLUS).is_none());
    // A selector syntect cannot parse skips that entry only.
    let bad = DARK_PLUS.replacen("\"entity.name.function\"", "\"a.b.c.d.e.f.g.h.i.j\"", 1);
    let (_, skipped) = build_theme(&bad, DARK_VS).unwrap();
    assert_eq!(skipped, 1);
}

#[test]
fn a_python_snippet_takes_dark_plus_colours() {
    let h = highlighted("py", "def f(x):\n    return \"s\" + 1  # c").unwrap();
    assert_eq!(rgb(&h, "def"), 0x569CD6);
    assert_eq!(rgb(&h, "f"), 0xDCDCAA);
    assert_eq!(rgb(&h, "x"), 0x9CDCFE);
    assert_eq!(rgb(&h, "return"), 0xC586C0);
    assert_eq!(rgb(&h, "\"s\""), 0xCE9178);
    assert_eq!(rgb(&h, "1"), 0xB5CEA8);
    assert_eq!(rgb(&h, "# c"), 0x6A9955);
    // Adjacent runs of one style are merged.
    for line in &h.lines {
        for pair in line.windows(2) {
            let key = |s: &Span| (s.rgb, s.bold, s.italic, s.underline);
            assert_ne!(key(&pair[0]), key(&pair[1]), "{line:?}");
        }
    }
}

// ---- Concatenation property ----

/// A small seeded generator (no dependency): the fuzz set is the same on every run.
struct Lcg(u64);

impl Lcg {
    fn pick(&mut self) -> usize {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        (self.0 >> 33) as usize
    }
}

/// Every result of `job` spells its own block's code; returns how many were highlighted.
fn assert_results_spell_their_code(job: &Job) -> usize {
    let results = compute_job_with(job, &mut frozen());
    assert_eq!(results.len(), job.blocks.len());
    for ((_, code), r) in job.blocks.iter().zip(&results) {
        if let Some(h) = r {
            assert_eq!(&h.text(), code);
        }
    }
    results.iter().filter(|r| r.is_some()).count()
}

#[test]
fn spans_always_join_to_exactly_the_blocks_code() {
    // The oracle corpus, parsed: every code block it holds, as the store's job would carry it.
    let fixture = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/markdown_oracle.jsonl"
    ))
    .unwrap();
    let inputs: Vec<String> = fixture
        .lines()
        .map(|l| {
            let v: Value = serde_json::from_str(l).unwrap();
            v["input"].as_str().unwrap().to_owned()
        })
        .collect();
    for input in &inputs {
        assert_results_spell_their_code(&Job {
            id: 1,
            blocks: code_blocks(&parse(input)),
        });
    }
    // Every corpus input as the code of a block in several grammars.
    let fences = ["py", "md", "html", "sh", "js", "rs", "c", "diff"];
    let mut highlighted = 0;
    for (i, input) in inputs.iter().enumerate() {
        highlighted +=
            assert_results_spell_their_code(&job(&[(Some(fences[i % fences.len()]), input)]));
    }
    assert!(
        highlighted * 10 > inputs.len() * 9,
        "{highlighted} of {}",
        inputs.len()
    );
    // A fuzz set over every grammar in the table.
    const ALPHABET: &[&str] = &[
        "(", ")", "[", "]", "{", "}", "<", ">", "\"", "'", "`", "/", "*", "#", "$", "\\", "=", ";",
        ":", ",", ".", "a", "b", "1", "_", "-", "+", " ", "\n", "\t", "字", "é", "<!--", "/*",
        "*/", "${", "@", "%", "|", "&", "!", "?", "~", "^", "0x", "e", "\u{202E}",
    ];
    let mut rng = Lcg(20261014);
    let mut fuzzed = 0;
    for (aliases, ..) in LANGUAGES {
        for _ in 0..40 {
            let len = rng.pick() % 120;
            let code: String = (0..len)
                .map(|_| ALPHABET[rng.pick() % ALPHABET.len()])
                .collect();
            fuzzed += assert_results_spell_their_code(&job(&[(Some(aliases[0]), &code)]));
        }
    }
    assert!(fuzzed > LANGUAGES.len() * 40 * 9 / 10, "{fuzzed}");
}

// ---- Limits ----

#[test]
fn a_line_over_1024_bytes_gives_no_highlight() {
    let at = "a".repeat(1024);
    assert!(highlighted("py", &format!("x = 1\n{at}")).is_some());
    assert_eq!(highlighted("py", &format!("x = 1\n{at}a")), None);
    // Bytes, not characters: 342 three-byte characters are 1026 bytes.
    assert_eq!(highlighted("py", &"字".repeat(342)), None);
    // A per-block cap: the next block of the message still runs.
    let results = compute_job_with(
        &job(&[(Some("py"), &format!("{at}a")), (Some("py"), "x = 1")]),
        &mut frozen(),
    );
    assert_eq!(some(&results), [false, true]);
}

#[test]
fn more_than_2000_lines_gives_no_highlight() {
    assert!(highlighted("py", &vec!["x"; 2000].join("\n")).is_some());
    assert_eq!(highlighted("py", &vec!["x"; 2001].join("\n")), None);
}

/// About 3000 spans in Python: `1 1 … 1` alternates a number and a space, so a line of k numbers
/// is 2k - 1 spans; 12 lines of 121 and one of 54 make 12 × 241 + 107 = 2999.
fn about_3000_spans() -> String {
    let line = |k: usize| vec!["1"; k].join(" ");
    let mut lines = vec![line(121); 12];
    lines.push(line(54));
    lines.join("\n")
}

#[test]
fn one_message_shares_one_span_allowance() {
    let code = about_3000_spans();
    let results = compute_job_with(
        &job(&[
            (Some("py"), &code),
            (Some("py"), &code),
            (Some("py"), "x = 1"),
        ]),
        &mut frozen(),
    );
    let first = results[0].as_ref().expect("the first block fits");
    assert_eq!(span_count(first), 2999);
    // The second would take the message to 5998 > 4096; it and every later block stay plain.
    assert_eq!(some(&results), [true, false, false]);
    // On its own, a block of 4096 spans is allowed.
    let mut lines = vec![vec!["1"; 121].join(" "); 16]; // 16 × 241
    lines.push(format!("{} ", vec!["1"; 120].join(" "))); // + 240, the trailing space its own span
    let at = lines.join("\n");
    assert_eq!(span_count(&highlighted("py", &at).unwrap()), 4096);
    assert_eq!(highlighted("py", &format!("{at}1")), None);
}

#[test]
fn blocks_of_one_message_share_one_budget() {
    // 100 ms per reading: the start, then one reading before each line; the budget is 250 ms.
    let blocks = [
        (Some("py"), "a = 1\nb = 2"),
        (Some("py"), "c = 3"),
        (Some("py"), "d = 4"),
    ];
    let results = compute_job_with(&job(&blocks), &mut ticking(100));
    // Readings at 100 (start), 200 and 300 for the first block's lines: 200 ms spent, it fits. The
    // second block's first line reads 400, 300 ms after the start, so it and every later one fail.
    assert_eq!(some(&results), [true, false, false]);
    // A block that runs out mid-way gets nothing (no partial highlight).
    let long = compute_job_with(&job(&[(Some("py"), "a\nb\nc\nd")]), &mut ticking(100));
    assert_eq!(long, [None]);
    // The next message starts its own budget.
    let next = compute_job_with(&job(&[(Some("py"), "c = 3")]), &mut ticking(100));
    assert_eq!(some(&next), [true]);
}

#[test]
fn a_block_without_a_language_spends_no_budget() {
    let blocks = [(None, "a\nb\nc"), (Some("py"), "x = 1")];
    let results = compute_job_with(&job(&blocks), &mut ticking(100));
    assert_eq!(some(&results), [false, true]);
}

#[test]
fn a_panic_while_highlighting_gives_none_and_the_next_block_still_runs() {
    let frozen_at = Instant::now();
    let mut readings = 0;
    // The third reading is inside the first block's computation (start, line 1, line 2).
    let results = compute_job_with(
        &job(&[(Some("py"), "a = 1\nb = 2"), (Some("py"), "c = 3")]),
        &mut || {
            readings += 1;
            if readings == 3 {
                panic!("injected");
            }
            frozen_at
        },
    );
    assert_eq!(some(&results), [false, true]);
}

#[test]
fn every_bundled_syntax_highlights_three_lines_without_panicking() {
    let (set, theme) = warm().unwrap();
    let sample = "a = (1, \"s\") # c\n  if x { y } // z\n<b>'q'</b> $v @w";
    let mut failed = Vec::new();
    for syntax in set.syntaxes() {
        match compute(sample, syntax, set, theme, MAX_SPANS, &mut || false) {
            Outcome::Done(h) => assert_eq!(h.text(), sample, "{}", syntax.name),
            _ => failed.push(syntax.name.clone()),
        }
    }
    assert!(failed.is_empty(), "no highlight from {failed:?}");
    assert!(set.syntaxes().len() > 50);
}

// ---- Release corpus ----

/// Rev 2's adversarial corpus: 1024-byte runs of each unit.
fn adversarial() -> Vec<(&'static str, String)> {
    let run = |unit: &str| unit.repeat(1024 / unit.len() + 1)[..1024].to_owned();
    let word = "abcdefghij".repeat(103)[..1024].to_owned();
    ["(", "\"\\\"", "/*", "/[", "a=", "<a ", "${", "`", "\\\\"]
        .into_iter()
        .map(|u| (u, run(u)))
        .chain([("long word", word)])
        .collect()
}

/// Grammars whose end patterns use backreferences (syntect cannot cache those searches): an
/// opener that enters the construct, and a near-miss of its terminator, 1024 bytes.
fn backref_cases() -> Vec<(&'static str, &'static str, &'static str)> {
    vec![
        ("sh", "cat <<EOF", "EOF "),
        ("rb", "x = <<~EOF", "EOF "),
        ("pl", "print <<EOF;", "EOF "),
        ("php", "<?php\n$x = <<<EOF", "EOF "),
        ("lua", "x = [==[", "]=]"),
        ("cs", "var s = \"\"\"", "\"\" "),
        ("md", "```py", "`` "),
        ("md", "~~~~", "~~~ "),
    ]
}

#[test]
#[ignore = "timing, release only: cargo test --release -p velella-core -- --ignored --exact highlight::tests::release_corpus_worst_line_per_grammar --test-threads=1 --nocapture"]
fn release_corpus_worst_line_per_grammar() {
    if cfg!(debug_assertions) {
        panic!("run in release: the numbers are for the shipped build");
    }
    let (set, theme) = warm().unwrap();
    let run = |unit: &str| unit.repeat(1024 / unit.len() + 1)[..1024].to_owned();
    // (fence, opener, line) for every measured line: the plain corpus in every grammar, then the
    // corpus and each near-miss inside every backreference construct.
    let mut cases: Vec<(&str, String, String)> = Vec::new();
    for (aliases, ..) in LANGUAGES {
        for (_, line) in adversarial() {
            cases.push((aliases[0], String::new(), line));
        }
    }
    for (fence, opener, near_miss) in backref_cases() {
        for (_, line) in adversarial() {
            cases.push((fence, format!("{opener}\n"), line));
        }
        cases.push((fence, format!("{opener}\n"), run(near_miss)));
    }
    // syntax -> (worst warm, its line's first bytes, worst cold)
    let mut worst: Vec<(&str, Duration, String, Duration)> = Vec::new();
    for (fence, opener, line) in &cases {
        let (name, _) = language(fence).unwrap();
        let syntax = set.find_syntax_by_name(name).unwrap();
        let code = format!("{opener}{line}");
        let time = || {
            let start = Instant::now();
            let _ = compute(&code, syntax, set, theme, MAX_SPANS, &mut || false);
            start.elapsed()
        };
        // The first run also builds what the grammar reaches (cold); the second is the steady state.
        let cold = time();
        let warm = time();
        let label = format!(
            "{}{}",
            opener.replace('\n', "⏎"),
            &line[..line.len().min(6)]
        );
        match worst.iter_mut().find(|w| w.0 == name) {
            Some(w) => {
                if warm > w.1 {
                    (w.1, w.2) = (warm, label);
                }
                w.3 = w.3.max(cold);
            }
            None => worst.push((name, warm, label, cold)),
        }
    }
    worst.sort_by_key(|w| std::cmp::Reverse(w.1));
    for (name, warm, label, cold) in &worst {
        println!(
            "{name:<28} worst line {:>9.3} ms (cold {:>9.3} ms)  {label:?}",
            warm.as_secs_f64() * 1e3,
            cold.as_secs_f64() * 1e3
        );
    }
}
