#!/usr/bin/env -S uv run --script
# /// script
# requires-python = ">=3.11"
# dependencies = ["pymupdf"]
# ///
"""Extract text from a PDF.

Usage:
  uv run tools/read_pdf.py <path>                 # all pages
  uv run tools/read_pdf.py <path> --pages 1-10    # range
  uv run tools/read_pdf.py <path> --pages 5       # single page
  uv run tools/read_pdf.py <path> --toc           # outline only
"""
import argparse
import sys

import fitz  # pymupdf


def main() -> None:
    p = argparse.ArgumentParser()
    p.add_argument("path")
    p.add_argument("--pages", help="e.g. 1-10 or 5")
    p.add_argument("--toc", action="store_true", help="print outline only")
    args = p.parse_args()

    doc = fitz.open(args.path)
    n = doc.page_count

    if args.toc:
        for level, title, page in doc.get_toc():
            print(f"{'  ' * (level - 1)}{title}  (p.{page})")
        return

    if args.pages:
        if "-" in args.pages:
            a, b = args.pages.split("-")
            start, end = int(a) - 1, int(b)
        else:
            start = int(args.pages) - 1
            end = start + 1
    else:
        start, end = 0, n

    print(f"# {args.path}  pages {start + 1}-{min(end, n)} of {n}", file=sys.stderr)
    for i in range(start, min(end, n)):
        print(f"\n--- page {i + 1} ---")
        print(doc[i].get_text())


if __name__ == "__main__":
    main()
