#!/usr/bin/env -S uv run --script
# /// script
# requires-python = ">=3.11"
# dependencies = ["pillow"]
# ///
"""Crop callout icons from artwork canvases.

Subcommands:

  info <image>
      Print image dimensions and basic info.

  crop <image> <x> <y> <w> <h> <out> [--size N]
      Crop the region (x, y, w, h) and write to out.
      With --size N, resize the cropped region to NxN before writing.
      Output format is inferred from the out path's extension
      (.webp, .png, .jpg).

  show <image> <x> <y> <w> <h>
      Print a small ASCII preview of where the crop region sits in the
      image (useful for sanity-checking bounds before cropping).
"""
import argparse
import sys
from pathlib import Path

from PIL import Image


def cmd_info(path: Path) -> None:
    img = Image.open(path)
    print(f"{path.name}: {img.width}x{img.height} mode={img.mode} format={img.format}")


def cmd_crop(path: Path, box: tuple[int, int, int, int], out: Path, size: int | None) -> None:
    x, y, w, h = box
    img = Image.open(path).convert("RGBA")
    crop = img.crop((x, y, x + w, y + h))
    if size is not None:
        crop = crop.resize((size, size), Image.LANCZOS)
    out.parent.mkdir(parents=True, exist_ok=True)
    crop.save(out)
    print(f"wrote {out} ({crop.width}x{crop.height})")


def cmd_show(path: Path, box: tuple[int, int, int, int]) -> None:
    """Print a 40x20 ASCII map of the image with the crop region highlighted."""
    x, y, w, h = box
    img = Image.open(path)
    cols, rows = 40, 20
    sx = img.width / cols
    sy = img.height / rows
    bx0, by0 = int(x / sx), int(y / sy)
    bx1, by1 = int((x + w) / sx), int((y + h) / sy)
    for r in range(rows):
        line = []
        for c in range(cols):
            if bx0 <= c < bx1 and by0 <= r < by1:
                line.append("#")
            else:
                line.append(".")
        print("".join(line))
    print(f"image {img.width}x{img.height}; box ({x},{y},{w},{h})")


def main() -> None:
    p = argparse.ArgumentParser()
    sub = p.add_subparsers(dest="cmd", required=True)

    pi = sub.add_parser("info")
    pi.add_argument("image")

    pc = sub.add_parser("crop")
    pc.add_argument("image")
    pc.add_argument("x", type=int)
    pc.add_argument("y", type=int)
    pc.add_argument("w", type=int)
    pc.add_argument("h", type=int)
    pc.add_argument("out")
    pc.add_argument("--size", type=int, default=None)

    ps = sub.add_parser("show")
    ps.add_argument("image")
    ps.add_argument("x", type=int)
    ps.add_argument("y", type=int)
    ps.add_argument("w", type=int)
    ps.add_argument("h", type=int)

    args = p.parse_args()
    if args.cmd == "info":
        cmd_info(Path(args.image))
    elif args.cmd == "crop":
        cmd_crop(
            Path(args.image),
            (args.x, args.y, args.w, args.h),
            Path(args.out),
            args.size,
        )
    elif args.cmd == "show":
        cmd_show(Path(args.image), (args.x, args.y, args.w, args.h))


if __name__ == "__main__":
    main()
