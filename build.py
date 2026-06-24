#!/usr/bin/env -S uv run --script
# /// script
# requires-python = ">=3.11"
# dependencies = []
# ///
"""Stage and render the book.

mdbook can only see files inside its `src` directory. Our canonical sources
live outside `book/` (in `concepts/` and `code/`), so this script stages
everything into `.mdbook/` with cross-link paths adjusted, then invokes the
locally-installed `mdbook` (in `.cargo/bin/`) to render `dist/`.

Run as:

    uv run build.py

To skip the mdbook render step (just stage), pass `--stage-only`.
"""
from __future__ import annotations

import argparse
import os
import re
import shutil
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent
STAGING = ROOT / ".mdbook"
LOCAL_CARGO_BIN = ROOT / ".cargo" / "bin"

# (rel_src, rel_in_staging) - files copied verbatim from outside book/
EXTERNAL_FILES = [
    ("concepts/dag.md", "concepts/dag.md"),
    ("concepts/glossary.md", "concepts/glossary.md"),
    ("code/sim/SPEC.md", "code/sim/SPEC.md"),
]

# (env_var, rel_in_staging) - optional files referenced via env var (in `.env`)
# that *overwrite* their staged copy when the env var is set and the file
# exists (the in-tree copy under `book/<rel_in_staging>` is the fallback).
# Currently none: the simlog reference implementation lives only in the
# Python edition now; the Rust edition's §37 specimen is the `code/logger`
# crate. The mechanism is kept dormant for any future env-referenced file.
ENV_FILES: list[tuple[str, str]] = []


def _read_dotenv(path: Path) -> dict[str, str]:
    out: dict[str, str] = {}
    if not path.exists():
        return out
    for line in path.read_text(encoding="utf-8").splitlines():
        line = line.strip()
        if not line or line.startswith("#"):
            continue
        if "=" not in line:
            continue
        key, _, value = line.partition("=")
        out[key.strip()] = value.strip()
    return out

# Path rewrites applied to staged markdown.
# Source files use the GitHub-friendly form (e.g. `../../concepts/dag.md`
# from `book/trunk/foo.md`); the staging tree flattens by one level so
# `concepts/` is a sibling of `trunk/`.
PATH_REWRITES = [
    ("../../concepts/", "../concepts/"),
    ("../../code/", "../code/"),
]

# Callout type → (icon filename in book/icons/, display label).
# `> [!NOTE]` blocks in the source markdown get rewritten into HTML tables
# that load these icons. GitHub renders the source `> [!NOTE]` natively;
# mdbook (after our preprocessor) renders the HTML table with the mouse icon.
# Callout types not listed here pass through unchanged.
CALLOUTS = {
    "NOTE":    ("note_assumptions.webp",    "Note"),
    "TIP":     ("tip_simplify.webp",        "Tip"),
    "WARNING": ("warning_units.webp",       "Warning"),
}

# Match a GitHub-style callout block:
#   > [!TYPE]
#   > body line 1
#   > body line 2
# A blank line (without `>`) terminates the block.
_CALLOUT_RE = re.compile(
    r'^> \[!(?P<type>NOTE|TIP|WARNING)\][ \t]*\n'
    r'(?P<body>(?:^>(?:[ \t].*)?\n)*)',
    re.MULTILINE,
)


def _render_callout(match: re.Match[str], icons_prefix: str) -> str:
    ctype = match.group("type")
    icon_file, label = CALLOUTS.get(ctype, (None, None))
    if icon_file is None:
        return match.group(0)
    body_lines = match.group("body").splitlines()
    body = "\n".join(
        line[2:] if line.startswith("> ")
        else line[1:] if line.startswith(">")
        else line
        for line in body_lines
    ).strip()
    return (
        f'<table class="callout callout-{ctype.lower()}" '
        f'style="width: 100%; border-left: 3px solid var(--quote-border, #aaa); '
        f'background: var(--quote-bg, #f9f9f9); color: var(--fg, inherit); margin: 1em 0;">\n'
        f'<tr>\n'
        f'<td style="width: 110px; vertical-align: top; padding: 0.6em;">\n'
        f'<img src="{icons_prefix}icons/{icon_file}" alt="{label}" '
        f'style="max-width: 96px; max-height: 96px; display: block; margin: 0 auto;">\n'
        f'</td>\n'
        f'<td style="vertical-align: top; padding: 0.6em 0.8em;">\n\n'
        f'**{label}** - {body}\n\n'
        f'</td>\n'
        f'</tr>\n'
        f'</table>\n'
    )


def _icons_prefix_for(md_path: Path, staging_root: Path) -> str:
    """Compute the relative prefix from a staged file's location to the
    staging root, so `<prefix>icons/foo.webp` resolves correctly regardless
    of the file's directory depth."""
    rel = md_path.relative_to(staging_root)
    depth = len(rel.parts) - 1
    return "../" * depth


# The Concept DAG is a ~4600px-wide mermaid graph. Rendered inline, mdbook fits
# it to the content column and its labels shrink to ~3px. So in the staged copy
# (only) we replace the mermaid block with a pre-rendered SVG (checked in at
# book/illustrations/dag.svg via tools/render_dag.sh), shown as a clickable
# preview that opens the full-resolution vector in a new tab. The source
# ```mermaid stays canonical for GitHub/Forgejo, which render it natively.
_MERMAID_BLOCK_RE = re.compile(r"^```mermaid\n.*?\n```\n", re.DOTALL | re.MULTILINE)

_DAG_SVG_EMBED = (
    '<a href="../illustrations/dag.svg" target="_blank" rel="noopener" '
    'title="Open the full-resolution DAG in a new tab">\n'
    '<img src="../illustrations/dag.svg" '
    'alt="The concept DAG: 43 nodes across 8 phases" '
    'style="max-width: 100%; height: auto;">\n'
    '</a>\n\n'
    '*[Open the full-resolution DAG in a new tab.](../illustrations/dag.svg)*\n'
)


def _embed_dag_svg(staging_root: Path) -> None:
    dag = staging_root / "concepts" / "dag.md"
    if not dag.exists():
        return
    text = dag.read_text(encoding="utf-8")
    new_text, n = _MERMAID_BLOCK_RE.subn(_DAG_SVG_EMBED, text, count=1)
    if n:
        dag.write_text(new_text, encoding="utf-8")
        print("Embedded book/illustrations/dag.svg in staged concepts/dag.md")
    else:
        print("WARNING: no mermaid block found in staged concepts/dag.md - DAG not embedded")


def stage() -> None:
    if STAGING.exists():
        shutil.rmtree(STAGING)
    STAGING.mkdir()

    # 1. Copy the entire book/ tree into staging.
    shutil.copytree(ROOT / "book", STAGING, dirs_exist_ok=True)

    # 2. Copy external sources into staging at their re-mapped locations.
    for src_rel, dst_rel in EXTERNAL_FILES:
        src = ROOT / src_rel
        dst = STAGING / dst_rel
        dst.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy(src, dst)

    # 2b. Copy any env-var-referenced override files (ENV_FILES; currently none).
    dotenv = _read_dotenv(ROOT / ".env")
    for env_var, dst_rel in ENV_FILES:
        path_str = dotenv.get(env_var) or os.environ.get(env_var)
        if not path_str:
            continue
        src = Path(path_str).expanduser()
        if not src.exists():
            continue
        dst = STAGING / dst_rel
        dst.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy(src, dst)

    # 3. Rewrite cross-link paths and callout blocks in every staged file.
    for md in STAGING.rglob("*.md"):
        text = md.read_text(encoding="utf-8")
        for old, new in PATH_REWRITES:
            text = text.replace(old, new)
        prefix = _icons_prefix_for(md, STAGING)
        text = _CALLOUT_RE.sub(lambda m: _render_callout(m, prefix), text)
        md.write_text(text, encoding="utf-8")

    # 4. Adjust top-level book pages (SUMMARY.md, front_matter.md,
    #    nomenclature.md, ...): source uses `../concepts/` and `../code/`
    #    (relative to book/, pointing at repo-root). After staging these
    #    files sit at the staging root, so the prefix becomes a no-op.
    #    Trunk chapters live one level deeper and use `../../X/`; those
    #    were already rewritten to `../X/` in step 3.
    for top_md in STAGING.glob("*.md"):
        text = top_md.read_text(encoding="utf-8")
        text = text.replace("../concepts/", "concepts/")
        text = text.replace("../code/", "code/")
        top_md.write_text(text, encoding="utf-8")

    # 5. Swap the inline DAG mermaid for the pre-rendered SVG embed (mdbook only).
    _embed_dag_svg(STAGING)

    n_md = sum(1 for _ in STAGING.rglob("*.md"))
    print(f"Staged {n_md} markdown file(s) to {STAGING.relative_to(ROOT)}")


def render() -> None:
    mdbook = LOCAL_CARGO_BIN / "mdbook"
    if not mdbook.exists():
        sys.exit(
            f"mdbook not found at {mdbook}. "
            "Install with: CARGO_INSTALL_ROOT=$(pwd)/.cargo cargo install mdbook mdbook-mermaid --locked"
        )
    env = os.environ.copy()
    env["PATH"] = f"{LOCAL_CARGO_BIN}:{env.get('PATH', '')}"
    subprocess.run([str(mdbook), "build"], cwd=ROOT, env=env, check=True)


# --- README generation ---------------------------------------------------
#
# README.md at repo root is regenerated from the same sources as the website.
# It is the entire book in one file: front matter + 43 chapter files
# concatenated, with cross-paths rewritten so everything resolves from the
# repo root.
#
# The reader who lands on the source repo gets the full book inline, GitHub-
# /Forgejo-rendered. The reader who wants the polished experience follows
# the live URL in the README header.

README_BEGIN = "<!-- BOOK_BEGIN -->"
README_END   = "<!-- BOOK_END -->"
LIVE_URL     = "https://root-11.codeberg.page/intro-book/"


def _slugify(heading: str) -> str:
    """GitHub-style heading anchor (lowercase, drop punctuation, spaces → hyphen)."""
    s = heading.lower().strip()
    s = re.sub(r"[*`]", "", s)        # markdown emphasis (keep underscores - they're word chars)
    s = re.sub(r"[^\w\s\-]", "", s)   # drop everything else except word chars / whitespace / hyphen
    s = re.sub(r"\s", "-", s)         # each whitespace char → one hyphen (preserves doubles around em-dash)
    return s


def _parse_summary() -> list[dict]:
    """Return ordered SUMMARY.md entries.

    "intro"   - top-level book pages (front_matter.md, nomenclature.md, ...)
    "chapter" - trunk/NN_*.md (excludes solutions files)

    Anything reached via `../` (concepts/, code/) is skipped - those are
    rendered by mdbook but not concatenated into the README."""
    text = (ROOT / "book" / "SUMMARY.md").read_text(encoding="utf-8")
    out: list[dict] = []
    for line in text.splitlines():
        m = re.match(r"\s*-?\s*\[([^\]]+)\]\(([^)]+)\)", line)
        if not m:
            continue
        title, path = m.group(1), m.group(2)
        if path.startswith("../") or "_solutions" in path:
            continue
        if path.startswith("trunk/"):
            out.append({"kind": "chapter", "title": title, "path": path})
        else:
            out.append({"kind": "intro", "title": title, "path": path})
    return out


def _build_anchor_map(chapters: list[dict]) -> dict[str, str]:
    """Map chapter filename stem → GitHub anchor for that chapter's first h1."""
    out: dict[str, str] = {}
    for entry in chapters:
        if entry["kind"] != "chapter":
            continue
        path = ROOT / "book" / entry["path"]
        text = path.read_text(encoding="utf-8")
        m = re.search(r"^# (.+)$", text, flags=re.MULTILINE)
        if m:
            out[path.stem] = _slugify(m.group(1))
    return out


def _resolve_url(url: str, source: Path) -> str:
    """Resolve a relative URL against the source file, return path-from-repo-root."""
    if not url or url.startswith(("http://", "https://", "#", "/", "mailto:")):
        return url
    base, sep, frag = url.partition("#")
    if not base:
        return url
    try:
        full = (source.parent / base).resolve()
        rel = full.relative_to(ROOT)
        return f"{rel}{sep}{frag}"
    except (ValueError, OSError):
        return url


_HTML_IMG_SRC_RE = re.compile(r'(<img\s+[^>]*?src=")([^"]+)(")', re.DOTALL)
_MD_LINK_RE      = re.compile(r'(!?\[(?:[^\[\]]|\[[^\]]*\])*\]\()([^()\s]+)(\))')
_CHAPTER_LINK_RE = re.compile(r"\[([^\]]+)\]\(([0-9]+_[a-z0-9_]+)\.md(#[\w-]+)?\)")


def _rewrite_chapter_links(text: str, anchor_map: dict[str, str]) -> str:
    """Sibling links between chapter files → anchor links inside README.
    Solutions files don't have anchors here, so they point to the live site instead."""
    def repl(m: re.Match[str]) -> str:
        label, stem, frag = m.group(1), m.group(2), m.group(3) or ""
        if stem.endswith("_solutions"):
            return f"[{label}]({LIVE_URL}trunk/{stem}.html{frag})"
        anchor = anchor_map.get(stem)
        if not anchor:
            return m.group(0)
        if frag:
            # Link to a sub-heading: in the single-file README every heading
            # lives in one document, so the fragment is already its own anchor.
            # Prepending the chapter anchor would yield an invalid double-anchor
            # (#chapter#frag). (A bare fragment resolves to the first heading
            # with that slug, so cross-chapter sub-heading targets must be
            # unique or the earliest occurrence - both current ones are.)
            return f"[{label}]({frag})"
        return f"[{label}](#{anchor})"
    return _CHAPTER_LINK_RE.sub(repl, text)


def _rewrite_paths(text: str, source: Path) -> str:
    text = _HTML_IMG_SRC_RE.sub(
        lambda m: m.group(1) + _resolve_url(m.group(2), source) + m.group(3),
        text,
    )
    text = _MD_LINK_RE.sub(
        lambda m: m.group(1) + _resolve_url(m.group(2), source) + m.group(3),
        text,
    )
    return text


_SKIP_RE             = re.compile(r"<!-- START_SKIP_FOR_README -->.*?<!-- STOP_SKIP_FOR_README -->\s*\n?", re.DOTALL)
_CONCEPT_NODE_RE     = re.compile(r"^> \*Concept node:[^\n]*\n", re.MULTILINE)
_REFERENCE_NOTES_RE  = re.compile(r"^Reference notes in \[[^\]]+\]\([^)]+\)\.[^\n]*\n", re.MULTILINE)


def _render_for_readme(path: Path, anchor_map: dict[str, str], strip_h1: bool) -> str:
    text = path.read_text(encoding="utf-8")
    text = _SKIP_RE.sub("", text)
    text = _CONCEPT_NODE_RE.sub("", text)
    text = _REFERENCE_NOTES_RE.sub("", text)
    text = _rewrite_chapter_links(text, anchor_map)
    text = _rewrite_paths(text, path)
    if strip_h1:
        text = re.sub(r"^# [^\n]*\n+", "", text, count=1)
    text = re.sub(r"\n{3,}", "\n\n", text)  # collapse runs of blank lines from stripped content
    return text.rstrip() + "\n"


def generate_readme() -> None:
    readme = ROOT / "README.md"
    text = readme.read_text(encoding="utf-8")
    if README_BEGIN not in text or README_END not in text:
        print(f"README.md missing {README_BEGIN}/{README_END} markers - skipping README generation")
        return

    entries = _parse_summary()
    chapters = [e for e in entries if e["kind"] == "chapter"]
    intros   = [e for e in entries if e["kind"] == "intro"]
    anchor_map = _build_anchor_map(chapters)

    parts = []
    # First intro is the title page (front_matter.md): strip its h1 because
    # the README's own h1 already serves as the title. Subsequent intros
    # (nomenclature, future appendices) keep their h1 as a section header.
    for i, entry in enumerate(intros):
        parts.append(_render_for_readme(
            ROOT / "book" / entry["path"], anchor_map, strip_h1=(i == 0)
        ))
    for entry in chapters:
        parts.append(_render_for_readme(ROOT / "book" / entry["path"], anchor_map, strip_h1=False))

    body = "\n\n".join(parts).rstrip() + "\n"

    pre, _, rest = text.partition(README_BEGIN)
    _, _, post = rest.partition(README_END)
    new_text = (
        f"{pre}{README_BEGIN}\n\n"
        f"<!-- This block is generated by build.py - do not edit by hand. -->\n\n"
        f"{body}\n"
        f"{README_END}{post}"
    )
    readme.write_text(new_text, encoding="utf-8")
    print(f"Regenerated README.md ({len(chapters)} chapter(s) inserted)")


# --- Dist-side README (for Codeberg repo view + SEO) ---------------------
#
# After mdbook renders, copy the source README.md into dist/ with paths
# rewritten for the static-site context:
#   - mdbook flattens the staging tree, so `book/illustrations/...` collapses
#     to `illustrations/...` in dist.
#   - Cross-document .md links (concepts/, code/) become absolute URLs into
#     the live mdbook site, so they resolve when the README is rendered by
#     Codeberg's repo viewer.
#
# The result: visiting `codeberg.org/root-11/intro-book` shows the full book
# inline as a normal markdown page (which search engines crawl), while the
# Codeberg Pages site at `root-11.codeberg.page/intro-book/` continues to
# serve the polished mdbook output.

DIST = ROOT / "dist"

_DIST_README_LIVE_LINK_RE = re.compile(
    r"(\]\()((?:concepts|code)/[^)\s]+\.md(?:#[^)\s]*)?)(\))"
)


def stage_readme_in_dist() -> None:
    src = ROOT / "README.md"
    if not src.exists() or not DIST.exists():
        return
    text = src.read_text(encoding="utf-8")
    # mdbook flattens the staging tree, so any `book/X/` path becomes `X/` in dist.
    # The asset directories that surface in the README via image / link refs:
    for sub in ("illustrations", "covers", "icons"):
        text = text.replace(f'"book/{sub}/', f'"{sub}/')
        text = text.replace(f'](book/{sub}/', f']({sub}/')
    # Cross-doc .md links → live-URL .html so they resolve on the static site
    def to_live(m: re.Match[str]) -> str:
        path = m.group(2).replace(".md", ".html")
        return f"{m.group(1)}{LIVE_URL}{path}{m.group(3)}"
    text = _DIST_README_LIVE_LINK_RE.sub(to_live, text)
    (DIST / "README.md").write_text(text, encoding="utf-8")
    # Copy LICENSE files into dist/ so the README's License section resolves
    # in the published Codeberg view (the source repo is private).
    for name in ("LICENSE", "LICENSE-CC-BY-4.0", "LICENSE-MIT", "LICENSE-APACHE-2.0"):
        p = ROOT / name
        if p.exists():
            shutil.copy(p, DIST / name)
    print(f"Wrote dist/README.md and LICENSE files")


def render_pdf() -> None:
    """Print the rendered dist/print.html to dist/intro-book.pdf.

    Delegates to tools/render_pdf.py (its own PEP-723 script with the Playwright
    dependency), so the PDF toolchain stays out of this script's environment.
    """
    script = ROOT / "tools" / "render_pdf.py"
    subprocess.run(["uv", "run", str(script)], cwd=ROOT, check=True)


def main() -> None:
    p = argparse.ArgumentParser()
    p.add_argument("--stage-only", action="store_true", help="skip the mdbook render step")
    p.add_argument("--no-readme",  action="store_true", help="skip README.md regeneration")
    p.add_argument("--pdf",        action="store_true", help="also render dist/intro-book.pdf")
    args = p.parse_args()
    stage()
    if not args.no_readme:
        generate_readme()
    if not args.stage_only:
        render()
        stage_readme_in_dist()
        if args.pdf:
            render_pdf()


if __name__ == "__main__":
    main()
