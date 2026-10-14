#!/bin/bash
# Rebuilds the oracle library: a copy of discord-markdown-parser's src/ that Node 24 runs with type
# stripping. Usage: build-lib.sh <refs/discord-markdown-parser> <output dir>
# The clone must be at 45d511fc882934b382852953f2c07a96d7635ae5 (docs/plans/markdown.md).
set -euo pipefail
src="$1"; out="$2"
want=45d511fc882934b382852953f2c07a96d7635ae5
have=$(git -C "$src" rev-parse HEAD)
[ "$have" = "$want" ] || { echo "discord-markdown-parser is at $have, expected $want" >&2; exit 1; }
rm -rf "$out"; mkdir -p "$out"; cp -R "$src/src/." "$out/"
cd "$out"
files=$(find . -name '*.ts')
# Node needs explicit extensions; directory imports point at index.ts.
perl -pi -e "s{(from\s+['\"])(\.{1,2}/[^'\"]+?)(['\"])}{\$1\$2.ts\$3}g unless /\.ts['\"]/" $files
perl -pi -e 's{(\.{1,2}(?:/\.\.)*)/simple-markdown\.ts}{$1/simple-markdown/index.ts}g' $files
# Type-only imports from simple-markdown must be marked `type` for type stripping.
perl -pi -e 'if (/simple-markdown\/index\.ts/ && /\{([^}]*)\}/) { my $in=$1; (my $o=$in) =~ s/\b(?<!type )([A-Za-z_]\w*)\b/type $1/g; $o =~ s/type type /type /g; s/\{\Q$in\E\}/{$o}/; }' $files
# The one behaviour patch: Discord's documented styles s and S (the clone predates them).
perl -pi -e 's{\(R\|t\|T\|d\|D\|f\|F\)}{(R|t|T|d|D|f|F|s|S)}' utils/regex.ts
grep -q '(R|t|T|d|D|f|F|s|S)' utils/regex.ts || { echo "timestamp patch did not apply" >&2; exit 1; }
