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

# (rel_src, rel_in_staging) — files copied verbatim from outside book/
EXTERNAL_FILES = [
    ("concepts/dag.md", "concepts/dag.md"),
    ("concepts/glossary.md", "concepts/glossary.md"),
    ("code/sim/SPEC.md", "code/sim/SPEC.md"),
]

# (env_var, rel_in_staging) — files referenced via env var (in `.env`).
# When the env var is set and the file exists, this *overwrites* whatever
# was already in the staging area at `rel_in_staging`. The vendored copy
# under `book/<rel_in_staging>` (already copied by the book/ tree copy
# above) is therefore the fallback when the env var is unset; the live
# source is the override when it is.
ENV_FILES = [
    ("SIMLOG_PATH", "simlog/logger.py"),  # vendored fallback at book/simlog/logger.py
]


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
        f'style="width: 100%; border-left: 3px solid #aaa; '
        f'background: #f9f9f9; margin: 1em 0;">\n'
        f'<tr>\n'
        f'<td style="width: 110px; vertical-align: top; padding: 0.6em;">\n'
        f'<img src="{icons_prefix}icons/{icon_file}" alt="{label}" '
        f'style="max-width: 96px; max-height: 96px; display: block; margin: 0 auto;">\n'
        f'</td>\n'
        f'<td style="vertical-align: top; padding: 0.6em 0.8em;">\n\n'
        f'**{label}** — {body}\n\n'
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

    # 2b. Copy env-var-referenced files (e.g. SIMLOG_PATH for logger.py).
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

    # 4. Adjust SUMMARY.md: source uses `../concepts/...` (relative to
    #    book/SUMMARY.md, pointing to repo-root concepts/); staging uses
    #    `concepts/...` (sibling).
    summary = STAGING / "SUMMARY.md"
    if summary.exists():
        text = summary.read_text(encoding="utf-8")
        text = text.replace("../concepts/", "concepts/")
        text = text.replace("../code/", "code/")
        summary.write_text(text, encoding="utf-8")

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


def main() -> None:
    p = argparse.ArgumentParser()
    p.add_argument("--stage-only", action="store_true", help="skip the mdbook render step")
    args = p.parse_args()
    stage()
    if not args.stage_only:
        render()


if __name__ == "__main__":
    main()
