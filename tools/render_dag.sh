#!/usr/bin/env bash
# Render the Concept DAG to a checked-in SVG asset.
#
# Why: the published mdbook site fits inline mermaid to the ~750px content
# column. The DAG is ~4600px wide, so it is scaled down ~6x and its labels
# render at ~3px — unreadable (reported by a reader on python-discourse).
# We render the DAG once to book/illustrations/dag.svg and embed it
# (clickable -> full-resolution in a new tab) during staging; see build.py's
# _embed_dag_svg(). The ```mermaid source in concepts/dag.md stays canonical
# (GitHub/Forgejo render it natively), so this asset is purely for mdbook.
#
# Re-run this whenever concepts/dag.md's diagram changes. Local-only: needs a
# headless browser for mermaid-cli, which CI never touches.
set -euo pipefail
cd "$(dirname "$0")/.."

SRC=concepts/dag.md
OUT=book/illustrations/dag.svg
TMP=$(mktemp -d)
trap 'rm -rf "$TMP"' EXIT

# Extract the first ```mermaid fenced block.
awk '/^```mermaid$/{f=1;next} /^```$/{if(f)exit} f' "$SRC" > "$TMP/dag.mmd"
[ -s "$TMP/dag.mmd" ] || { echo "no mermaid block found in $SRC" >&2; exit 1; }

# Ensure a headless browser for mermaid-cli (puppeteer).
npx --yes puppeteer browsers install chrome-headless-shell >/dev/null
CHROME=$(ls -d "$HOME"/.cache/puppeteer/chrome-headless-shell/*/chrome-headless-shell-linux64/chrome-headless-shell \
         | sort -V | tail -1)
printf '{ "executablePath": "%s", "args": ["--no-sandbox"] }\n' "$CHROME" > "$TMP/pp.json"

npx --yes @mermaid-js/mermaid-cli -p "$TMP/pp.json" -b white -i "$TMP/dag.mmd" -o "$OUT"
echo "wrote $OUT"
