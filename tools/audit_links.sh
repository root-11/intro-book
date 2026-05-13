#!/usr/bin/env bash
# audit_links.sh — list every markdown link in the book, classified by target.
#
# Usage:
#   tools/audit_links.sh                  # offline, columnar (default)
#   tools/audit_links.sh --check          # also verify external URLs return 200/30x
#   tools/audit_links.sh --chapters-only  # skip solutions and top-level pages
#   tools/audit_links.sh --kind GITHUB    # filter to one classification
#
# Designed so the reviewer can scan every link without clicking page-by-page.
# Classifications: GITHUB, CODEBERG, EXTERNAL, ANCHOR, MAILTO, LOCAL-MD, LOCAL-ASSET.
#
# Run from the repo root or anywhere; the script cds to repo root before scanning.

set -uo pipefail   # NOT -e: grep exits 1 when a file has no matches, which is fine

cd "$(dirname "$0")/.."

mode=offline
scope=all
filter=""
for arg in "$@"; do
    case "$arg" in
        --check|--online)   mode=online ;;
        --chapters-only)    scope=chapters ;;
        --kind)             ;;   # consumed below
        --kind=*)           filter="${arg#--kind=}" ;;
        *)
            if [[ "$arg" =~ ^(GITHUB|CODEBERG|EXTERNAL|ANCHOR|MAILTO|LOCAL-MD|LOCAL-ASSET)$ ]]; then
                filter="$arg"
            else
                echo "audit_links: unknown arg: $arg" >&2
                exit 2
            fi
            ;;
    esac
done

if [[ "$scope" == chapters ]]; then
    mapfile -t files < <(find book/trunk -name '[0-9]*.md' -not -name '*_solutions.md' | sort)
else
    mapfile -t files < <(find book -name '*.md' | sort)
fi

classify() {
    case "$1" in
        http*://github.com/*)   echo GITHUB ;;
        http*://codeberg.org/*) echo CODEBERG ;;
        http*://*)              echo EXTERNAL ;;
        \#*)                    echo ANCHOR ;;
        mailto:*)               echo MAILTO ;;
        *.md|*.md#*)            echo LOCAL-MD ;;
        *)                      echo LOCAL-ASSET ;;
    esac
}

emit_offline() {
    printf 'file\tline\tkind\ttext\turl\n'
    for f in "${files[@]}"; do
        # Single pass over the file: markdown links, HTML img tags, HTML a tags.
        # Alternation keeps everything in line order.
        grep -noE '!?\[[^]]*\]\([^)]+\)|<img[^>]+src="[^"]+"|<a[^>]+href="[^"]+"' "$f" | \
        while IFS=: read -r line match; do
            case "$match" in
                '<img'*)
                    text='<img>'
                    url="${match##*src=\"}"; url="${url%\"}"
                    ;;
                '<a'*)
                    text='<a>'
                    url="${match##*href=\"}"; url="${url%\"}"
                    ;;
                *)
                    # Split on `](`, the markdown text/URL boundary, so link
                    # texts containing `(` or `)` don't fool the parser.
                    text="${match%%\](*}"; text="${text#!}"; text="${text#\[}"
                    url="${match##*\](}"; url="${url%\)}"
                    ;;
            esac
            kind="$(classify "$url")"
            if [[ -n "$filter" && "$kind" != "$filter" ]]; then
                continue
            fi
            printf '%s\t%s\t%s\t%s\t%s\n' \
                "${f#book/}" "$line" "$kind" "$text" "$url"
        done
    done
}

emit_online() {
    printf 'status\tfile\tline\turl\n'
    for f in "${files[@]}"; do
        grep -noE '\]\(https?://[^)]+\)' "$f" | while IFS=: read -r line match; do
            url="${match#*\(}"
            url="${url%\)}"
            code=$(curl -s -o /dev/null -L -w '%{http_code}' --max-time 15 "$url" 2>/dev/null || echo "ERR")
            printf '%s\t%s\t%s\t%s\n' "$code" "${f#book/}" "$line" "$url"
        done
    done
}

if [[ "$mode" == online ]]; then
    emit() { emit_online; }
else
    emit() { emit_offline; }
fi

if [[ -t 1 ]]; then
    emit | column -t -s $'\t'         # interactive: align columns for reading
else
    emit                              # piped: emit raw TSV so awk/grep/sort can chain
fi
