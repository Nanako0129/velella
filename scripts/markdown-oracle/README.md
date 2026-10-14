# Markdown oracle

The differential oracle for velella's markdown parser (`docs/plans/markdown.md`, "The oracle"). It runs discord-markdown-parser, which builds Discord's rule set on simple-markdown, under Node 24 type stripping. It always runs in `extended` mode, so masked links are on. It carries one patch: timestamp styles `s` and `S`. Main owns these files and the fixture; executors only read them.

## Regenerate

```bash
# refs/discord-markdown-parser must be at 45d511fc882934b382852953f2c07a96d7635ae5
scripts/markdown-oracle/build-lib.sh refs/discord-markdown-parser /tmp/oracle-lib
node scripts/markdown-oracle/gen.mjs /tmp/oracle-lib . crates/velella-core/tests/fixtures/markdown_oracle.jsonl
shasum -a 256 crates/velella-core/tests/fixtures/markdown_oracle.jsonl
```

`gen.mjs` also reads `refs/SimpleAST`, `refs/serein` (inputs copied into the script) and `refs/discord-ground-truth/samples/*.txt` (the three samples posted in the dev thread).

## Recorded output (2026-10-14)

| Item | Value |
|---|---|
| Lines | 3118 |
| SHA-256 | `330d36511320879ca1b4c3ba2e2e10aa6c4f602e3af17de262ea303184f1c6d1` |
| Node | v24.21.0 |

Each line is `{"source", "input", "ast"}`. The AST is normalised: adjacent text is merged, `twemoji` and `emoticon` nodes become text, link targets are WHATWG-canonical (`new URL(t).href`), and keys are sorted.
