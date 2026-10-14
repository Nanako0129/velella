//! Highlighter tests (docs/plans/markdown.md, K6 "Tests"): the language table, the theme build,
//! Dark+ colours, the concatenation property, the limits and the shared budget, panics, every
//! bundled syntax, and the release timing corpus.
use super::*;
use crate::markdown::parse;
use std::time::Instant;

/// A clock that never moves: no budget is spent.
fn frozen() -> impl FnMut() -> Instant {
    let t = Instant::now();
    move || t
}

fn block(language: &str, code: &str) -> Block {
    Block::CodeBlock {
        language: Some(language.to_owned()),
        code: code.to_owned(),
        highlight: None,
    }
}

fn highlight_of(b: &Block) -> Option<&Highlighted> {
    match b {
        Block::CodeBlock { highlight, .. } => highlight.as_ref(),
        _ => None,
    }
}

/// One block highlighted with no budget.
fn highlighted(language: &str, code: &str) -> Option<Highlighted> {
    let mut b = [block(language, code)];
    fill_with(&mut b, &mut frozen());
    highlight_of(&b[0]).cloned()
}

/// Every code block in `blocks`, inline containers included, in document order.
fn code_blocks(blocks: &[Block]) -> Vec<&Block> {
    fn inl<'a>(i: &'a [Inline], out: &mut Vec<&'a Block>) {
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
    fn blk<'a>(b: &'a [Block], out: &mut Vec<&'a Block>) {
        for x in b {
            match x {
                Block::Paragraph(c) | Block::Subtext(c) | Block::Heading { content: c, .. } => {
                    inl(c, out)
                }
                Block::Quote(q) => blk(q, out),
                Block::List { items, .. } => items.iter().for_each(|i| blk(i, out)),
                Block::CodeBlock { .. } => out.push(x),
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
        }
    }
    let table: usize = LANGUAGES.iter().map(|(a, ..)| a.len()).sum();
    assert_eq!(aliases, table, "the code's table has rows the plan's lacks");
    assert_eq!(language("ts"), Some(("JavaScript", "TypeScript")));
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
    }
    // No language, or one off the table: no highlight, so no header.
    assert_eq!(highlighted("swift", "let x = 1"), None);
    let mut none = [Block::CodeBlock {
        language: None,
        code: "x = 1".into(),
        highlight: None,
    }];
    fill_with(&mut none, &mut frozen());
    assert_eq!(highlight_of(&none[0]), None);
    // The header is the table's name, not the fence text.
    assert_eq!(highlighted("TS", "let x = 1").unwrap().name, "TypeScript");
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
    assert_eq!(h.name, "Python");
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

fn assert_spans_are_the_code(b: &Block) -> bool {
    let Block::CodeBlock {
        code, highlight, ..
    } = b
    else {
        unreachable!()
    };
    if let Some(h) = highlight {
        assert_eq!(&h.text(), code);
    }
    highlight.is_some()
}

#[test]
fn spans_always_join_to_exactly_the_blocks_code() {
    // The oracle corpus, parsed: every code block it holds, highlighted as itself.
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
        let mut body = parse(input);
        fill_with(&mut body, &mut frozen());
        code_blocks(&body)
            .into_iter()
            .for_each(|b| _ = assert_spans_are_the_code(b));
    }
    // Every corpus input as the code of a block in several grammars.
    let fences = ["py", "md", "html", "sh", "js", "rs", "c", "diff"];
    let mut some = 0;
    for (i, input) in inputs.iter().enumerate() {
        let mut b = [block(fences[i % fences.len()], input)];
        fill_with(&mut b, &mut frozen());
        some += usize::from(assert_spans_are_the_code(&b[0]));
    }
    assert!(some * 10 > inputs.len() * 9, "{some} of {}", inputs.len());
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
            let mut b = [block(aliases[0], &code)];
            fill_with(&mut b, &mut frozen());
            fuzzed += usize::from(assert_spans_are_the_code(&b[0]));
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
}

#[test]
fn more_than_2000_lines_gives_no_highlight() {
    assert!(highlighted("py", &vec!["x"; 2000].join("\n")).is_some());
    assert_eq!(highlighted("py", &vec!["x"; 2001].join("\n")), None);
}

#[test]
fn more_than_4096_spans_gives_no_highlight() {
    // `1 1 … 1`: a number and a space alternate, two styles, so k numbers give 2k - 1 spans.
    let line = |k: usize| vec!["1"; k].join(" ");
    let mut lines = vec![line(121); 16]; // 16 × 241
    lines.push(format!("{} ", line(120))); // + 240, the trailing space its own span
    let code = lines.join("\n");
    let h = highlighted("py", &code).expect("4096 spans are allowed");
    assert_eq!(h.lines.iter().map(Vec::len).sum::<usize>(), 4096);
    assert_eq!(highlighted("py", &format!("{code}1")), None);
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

#[test]
fn blocks_of_one_message_share_one_budget() {
    // 10 ms per reading: the start, then one reading before each line.
    let mut body = parse("```py\na = 1\nb = 2\n``` ```py\nc = 3\n``` ```py\nd = 4\n```");
    fill_with(&mut body, &mut ticking(10));
    let got: Vec<bool> = code_blocks(&body)
        .iter()
        .map(|b| highlight_of(b).is_some())
        .collect();
    // Readings at 10 (start), 20 and 30 for the first block's lines: 20 ms spent, it fits. The
    // second block's first line reads 40, 30 ms after the start, so it and every later one fail.
    assert_eq!(got, [true, false, false]);
    // A block that runs out mid-way gets nothing (no partial highlight).
    let mut long = [block("py", "a\nb\nc\nd")];
    fill_with(&mut long, &mut ticking(10));
    assert_eq!(highlight_of(&long[0]), None);
    // With time to spare, a second message starts its own budget.
    let mut next = parse("```py\nc = 3\n```");
    fill_with(&mut next, &mut ticking(10));
    assert!(highlight_of(code_blocks(&next)[0]).is_some());
}

#[test]
fn a_block_without_a_language_spends_no_budget() {
    let mut body = parse("```\na\nb\nc\n``` ```py\nx = 1\n```");
    fill_with(&mut body, &mut ticking(10));
    let got: Vec<bool> = code_blocks(&body)
        .iter()
        .map(|b| highlight_of(b).is_some())
        .collect();
    assert_eq!(got, [false, true]);
}

#[test]
fn a_panic_while_highlighting_gives_none_and_the_next_block_still_runs() {
    let mut body = parse("```py\na = 1\nb = 2\n``` ```py\nc = 3\n```");
    let frozen_at = Instant::now();
    let mut readings = 0;
    // The third reading is inside the first block's computation (start, line 1, line 2).
    fill_with(&mut body, &mut || {
        readings += 1;
        if readings == 3 {
            panic!("injected");
        }
        frozen_at
    });
    let got: Vec<bool> = code_blocks(&body)
        .iter()
        .map(|b| highlight_of(b).is_some())
        .collect();
    assert_eq!(got, [false, true]);
}

#[test]
fn every_bundled_syntax_highlights_three_lines_without_panicking() {
    let (set, theme) = warm().unwrap();
    let sample = "a = (1, \"s\") # c\n  if x { y } // z\n<b>'q'</b> $v @w";
    let mut failed = Vec::new();
    for syntax in set.syntaxes() {
        match compute(sample, syntax, &syntax.name, set, theme, &mut || false) {
            Some(h) => assert_eq!(h.text(), sample, "{}", syntax.name),
            None => failed.push(syntax.name.clone()),
        }
    }
    assert!(failed.is_empty(), "no highlight from {failed:?}");
    assert!(set.syntaxes().len() > 50);
}

// ---- Release timing ----

/// The plan's adversarial corpus: 1024-byte runs of each unit.
fn adversarial() -> Vec<(&'static str, String)> {
    let run = |unit: &str| unit.repeat(1024 / unit.len() + 1)[..1024].to_owned();
    let word = "abcdefghij".repeat(103)[..1024].to_owned();
    ["(", "\"\\\"", "/*", "/[", "a=", "<a ", "${", "`", "\\\\"]
        .into_iter()
        .map(|u| (u, run(u)))
        .chain([("long word", word)])
        .collect()
}

#[test]
#[ignore = "timing, release only: cargo test --release -p velella-core -- --ignored highlight_timing --nocapture"]
fn highlight_timing_release() {
    if cfg!(debug_assertions) {
        panic!("run in release: the bounds are for the shipped build");
    }
    let (set, theme) = warm().unwrap();
    let mut syntaxes: Vec<&str> = LANGUAGES.iter().map(|(_, s, _)| *s).collect();
    syntaxes.dedup();
    let time_line = |syntax: &SyntaxReference, line: &str| {
        let start = Instant::now();
        let h = compute(line, syntax, "", set, theme, &mut || false);
        (start.elapsed(), h.is_some())
    };
    let mut worst = (Duration::ZERO, "", "");
    let mut worst_cold = (Duration::ZERO, "", "");
    for name in &syntaxes {
        let syntax = set.find_syntax_by_name(name).unwrap();
        let mut row = (Duration::ZERO, "");
        for (unit, line) in adversarial() {
            // The first run also compiles the grammar's regexes it reaches (cold); the second is
            // the steady state.
            let (cold, _) = time_line(syntax, &line);
            let (warm, _) = time_line(syntax, &line);
            if cold > worst_cold.0 {
                worst_cold = (cold, name, unit);
            }
            if warm > row.0 {
                row = (warm, unit);
            }
            if warm > worst.0 {
                worst = (warm, name, unit);
            }
        }
        println!(
            "{name:<28} worst line {:>8.3} ms ({})",
            row.0.as_secs_f64() * 1e3,
            row.1
        );
    }
    println!(
        "worst warm line: {:.3} ms ({} / {}); worst cold line: {:.3} ms ({} / {})",
        worst.0.as_secs_f64() * 1e3,
        worst.1,
        worst.2,
        worst_cold.0.as_secs_f64() * 1e3,
        worst_cold.1,
        worst_cold.2
    );

    // A message of 20 maximum-cost blocks: the worst pattern in its grammar, 2000 capped lines
    // each. The shared budget ends it. Both numbers are printed before either bound is checked.
    let (_, worst_syntax, worst_unit) = worst;
    let fence = LANGUAGES
        .iter()
        .find(|(_, s, _)| *s == worst_syntax)
        .unwrap()
        .0[0];
    let line = adversarial()
        .into_iter()
        .find(|(u, _)| *u == worst_unit)
        .unwrap()
        .1;
    let code = vec![line.as_str(); 2000].join("\n");
    let mut body: Vec<Block> = (0..20).map(|_| block(fence, &code)).collect();
    let start = Instant::now();
    fill(&mut body);
    let total = start.elapsed();
    println!(
        "20 maximum-cost blocks ({fence}, {worst_unit}): {:.3} ms",
        total.as_secs_f64() * 1e3
    );
    assert!(worst.0 <= Duration::from_millis(10), "{worst:?}");
    assert!(total < Duration::from_millis(40), "{total:?}");
}
