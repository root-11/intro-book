#!/usr/bin/env -S uv run --script
# /// script
# requires-python = ">=3.11"
# dependencies = ["playwright"]
# ///
"""Render the built book to a single PDF.

mdbook already emits `dist/print.html` - the whole book concatenated on one
page with print CSS. This loads that page in a headless Chromium (bundled by
Playwright, so nothing touches the global environment) and prints it to PDF.
A real browser is used because the mermaid diagram and the playground styling
are produced by JavaScript at page-load; pandoc/LaTeX would lose both.

Run after a build:

    uv run build.py
    uv run tools/render_pdf.py                  # -> dist/intro-book.pdf
    uv run tools/render_pdf.py --out book.pdf   # custom output path
    uv run tools/render_pdf.py --html dist/print.html
"""
from __future__ import annotations

import argparse
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent


def _print_to_pdf(html: Path, out: Path) -> None:
    from playwright.sync_api import sync_playwright

    url = html.resolve().as_uri()
    with sync_playwright() as p:
        browser = p.chromium.launch()
        page = browser.new_page()
        # print.html pulls in mermaid + MathJax + images; give the JS time to
        # draw before snapshotting. networkidle covers asset loads; the settle
        # delay covers post-load rendering (mermaid svg, MathJax typeset).
        page.goto(url, wait_until="networkidle")
        page.wait_for_timeout(2000)
        page.pdf(
            path=str(out),
            format="A4",
            print_background=True,
            margin={"top": "18mm", "bottom": "18mm", "left": "16mm", "right": "16mm"},
        )
        browser.close()


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("--html", default=str(ROOT / "dist" / "print.html"),
                    help="source print page (default: dist/print.html)")
    ap.add_argument("--out", default=str(ROOT / "dist" / "intro-book.pdf"),
                    help="output PDF path (default: dist/intro-book.pdf)")
    args = ap.parse_args()

    html = Path(args.html)
    out = Path(args.out)
    if not html.exists():
        sys.exit(f"{html} not found. Build the book first: uv run build.py")

    try:
        _print_to_pdf(html, out)
    except Exception as exc:  # noqa: BLE001 - first run needs the browser binary
        msg = str(exc)
        if "Executable doesn't exist" in msg or "playwright install" in msg:
            print("Installing Chromium for Playwright (one-time)...", file=sys.stderr)
            subprocess.run(
                [sys.executable, "-m", "playwright", "install", "chromium"],
                check=True,
            )
            _print_to_pdf(html, out)
        else:
            raise

    size_mb = out.stat().st_size / 1e6
    print(f"Wrote {out} ({size_mb:.1f} MB)")


if __name__ == "__main__":
    main()
