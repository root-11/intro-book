#!/usr/bin/env -S uv run --script
# /// script
# requires-python = ">=3.11"
# dependencies = ["beautifulsoup4", "lxml"]
# ///
"""Extract readable text from one or more HTML files.

Usage:
  uv run tools/read_html.py <path>                  # one file
  uv run tools/read_html.py <dir> --pattern '*.html'  # all matching in dir
"""
import argparse
import sys
from pathlib import Path

from bs4 import BeautifulSoup


def extract(path: Path) -> str:
    html = path.read_text(encoding="utf-8", errors="replace")
    soup = BeautifulSoup(html, "lxml")
    for tag in soup(["script", "style", "nav"]):
        tag.decompose()
    text = soup.get_text(separator="\n")
    lines = [ln.rstrip() for ln in text.splitlines()]
    out = []
    blank = False
    for ln in lines:
        if ln.strip():
            out.append(ln)
            blank = False
        elif not blank:
            out.append("")
            blank = True
    return "\n".join(out).strip()


def main() -> None:
    p = argparse.ArgumentParser()
    p.add_argument("path")
    p.add_argument("--pattern", default=None, help="glob pattern when path is a dir")
    args = p.parse_args()

    target = Path(args.path)
    if target.is_dir():
        pattern = args.pattern or "*.html"
        files = sorted(
            target.glob(pattern),
            key=lambda f: (len(f.stem), f.stem),
        )
    else:
        files = [target]

    for f in files:
        print(f"\n===== {f.name} =====", file=sys.stderr)
        print(f"\n===== {f.name} =====")
        print(extract(f))


if __name__ == "__main__":
    main()
