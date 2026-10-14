// Builds the K1 differential fixture (docs/plans/markdown.md, "The oracle").
// Usage: node gen.mjs <oracle lib dir from build-lib.sh> <velella repo root> <output .jsonl>
// Each output line is {"input": string, "source": string, "ast": normalised oracle AST}.
import { readFileSync, writeFileSync, mkdtempSync } from 'node:fs';
import { join } from 'node:path';
import { tmpdir } from 'node:os';
import { pathToFileURL } from 'node:url';

const [lib, repo, out] = process.argv.slice(2);
if (!lib || !repo || !out) throw new Error('usage: gen.mjs <lib> <repo> <out>');
const { default: parse } = await import(pathToFileURL(join(lib, 'index.ts')).href);

const inputs = [];
const add = (source, text) => inputs.push({ source, input: text });

// (a) every parse() input of discord-markdown-parser tests/parse.test.ts, recorded by running the
// test file against a recording parse with no-op describe/test/expect.
{
  const dir = mkdtempSync(join(tmpdir(), 'oracle-tests-'));
  const recorded = [];
  globalThis.__oracleRecord = (text) => recorded.push(text);
  writeFileSync(join(dir, 'recorder.ts'), 'export function parse(text: string, _type?: string) { (globalThis as any).__oracleRecord(text); return []; }\n');
  const test = readFileSync(join(repo, 'refs/discord-markdown-parser/tests/parse.test.ts'), 'utf8')
    .replace("import { parse } from '../src';", "import { parse } from './recorder.ts';");
  writeFileSync(join(dir, 'parse.test.ts'), test);
  globalThis.describe = (_name, body) => body();
  globalThis.test = (_name, body) => body();
  globalThis.expect = () => new Proxy({}, { get: () => () => {} });
  await import(pathToFileURL(join(dir, 'parse.test.ts')).href);
  recorded.forEach((text, i) => add(`itzderock:parse.test.ts#${i}`, text));
}

// (b) SimpleAST MarkdownRulesTest header inputs (Kotlin trimIndent applied by hand).
add('simpleast:headers', 'Title\n# Header 1\nsome content\n## Header 2\n### Header 3');
add('simpleast:headersConsumeNewlines', 'Title\n# Header 1\n\n\n# Header 2');
add('simpleast:headersInvalidPrefix', 'a# Header 1\nsome content\n-## Header 2\nother content for listing # of items');
add('simpleast:headerParseBlockPunctuations', 'Intro text\n\nShould Succeed. Alt Header\n======\nsome content');

// (c) Serein behaviour-case inputs (refs/serein/crates/ui/src/markdown.rs).
for (const text of ['>text', '> text', '>>> text', '>>>text', '>> text', '>', '  > text', '  >text',
  '> a\n>b', '>>> a\n>b', '```\n>code\n```', 'a >b']) add('serein:2692', text);
add('serein:3818', '**Visible ||secret** still hidden|| end ||<@42> <#43> [link](https://hidden.example) 👩🏽‍💻||.');
add('serein:3818', '||hidden **also hidden|| visible**');
for (const text of ['intro\n```js\nconst a = 1;\n```\nafter', '```py\nprint(1)``` trailing',
  '```echo hi``` and `inline`', '```elixir\nIO.puts 1\n```', 'text\n\n    not code']) add('serein:4982', text);
add('serein:5407', '@everyone @here `@everyone` @everyone_else \\@here');

// (d) the posted K0 samples: each line, then each whole sample.
for (const name of ['times', 'markdown', 'edge']) {
  const text = readFileSync(join(repo, `refs/discord-ground-truth/samples/${name}.txt`), 'utf8');
  text.split('\n').forEach((line, i) => add(`k0:${name}:${i + 1}`, line));
  add(`k0:${name}:whole`, text);
}

// (e) 3000 generated strings, seed 20261014 (mulberry32), 1-24 tokens each.
{
  const alphabet = ['*', '_', '~', '|', '`', '\\', '>', '#', '-', '[', ']', '(', ')', '<', '@', 't', ':',
    '1', 'a', '字', ' ', '\n', 'https://x.y'];
  let s = 20261014 >>> 0;
  const rand = () => {
    s = (s + 0x6d2b79f5) >>> 0;
    let t = s;
    t = Math.imul(t ^ (t >>> 15), t | 1);
    t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
  for (let i = 0; i < 3000; i++) {
    const n = 1 + Math.floor(rand() * 24);
    let text = '';
    for (let j = 0; j < n; j++) text += alphabet[Math.floor(rand() * alphabet.length)];
    add(`gen:${i}`, text);
  }
}

// Normalisation: adjacent text merged; twemoji and emoticon become text; link targets through
// WHATWG URL (as the url crate); everything else as the oracle emits it, keys sorted.
function canonical(target) {
  try { return new URL(target).href; } catch { return target; }
}
function normalise(nodes) {
  const outNodes = [];
  for (const node of nodes) {
    let n = { ...node };
    if (n.type === 'twemoji') n = { type: 'text', content: n.name };
    if (n.type === 'emoticon') n = { type: 'text', content: typeof n.content === 'string' ? n.content : '¯\\_(ツ)_/¯' };
    if (n.target !== undefined) n.target = canonical(n.target);
    for (const key of Object.keys(n)) {
      if (Array.isArray(n[key]) && n[key].every((c) => c && typeof c === 'object' && 'type' in c)) n[key] = normalise(n[key]);
    }
    const last = outNodes[outNodes.length - 1];
    if (n.type === 'text' && typeof n.content === 'string' && last && last.type === 'text' && typeof last.content === 'string') {
      last.content += n.content;
    } else {
      outNodes.push(n);
    }
  }
  return outNodes.map(sortKeys);
}
function sortKeys(v) {
  if (Array.isArray(v)) return v.map(sortKeys);
  if (v && typeof v === 'object') return Object.fromEntries(Object.keys(v).sort().map((k) => [k, sortKeys(v[k])]));
  return v;
}

const lines = inputs.map(({ source, input }) =>
  JSON.stringify({ source, input, ast: normalise(parse(input, 'extended')) }));
writeFileSync(out, lines.join('\n') + '\n');
console.log(`${lines.length} lines`);
