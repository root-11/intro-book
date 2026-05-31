#!/usr/bin/env -S uv run --script
# /// script
# requires-python = ">=3.11"
# dependencies = []
# ///
"""One-shot insertion of artwork into chapter markdown.

Each entry below names: target file, anchor text already in the file, the
HTML to insert, and whether the insert goes before or after the anchor.
The script is idempotent - if the HTML is already present, the file is
not changed.
"""
from __future__ import annotations

from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
BOOK = ROOT / "book"


def _img(src_relative: str, alt: str, width: int) -> str:
    return (
        f'<p align="center">'
        f'<img src="{src_relative}" alt="{alt}" width="{width}">'
        f'</p>\n\n'
    )


# Phase dividers: same shape for every phase, inserted before the
# `> *Concept node:` line in the first chapter of each phase.
PHASE_DIVIDERS = [
    ("trunk/01_the_machine_model.md",            "phase_foundation.jpg",          "Foundation"),
    ("trunk/05_identity_is_an_integer.md",       "phase_identity_structure.jpg",  "Identity & structure"),
    ("trunk/11_the_tick.md",                     "phase_time_passes.jpg",         "Time & passes"),
    ("trunk/17_presence_replaces_flags.md",      "phase_ebp.jpg",                 "Existence-based processing"),
    ("trunk/21_swap_remove.md",                  "phase_memory_lifecycle.jpg",    "Memory & lifecycle"),
    ("trunk/26_hot_cold_splits.md",              "phase_scale.jpg",               "Scale"),
    ("trunk/31_disjoint_writes_parallelize.md",  "phase_concurrency.jpg",         "Concurrency"),
    ("trunk/35_boundary_is_the_queue.md",        "phase_io_persistence.jpg",      "I/O & persistence"),
    ("trunk/39_system_of_systems.md",            "phase_system_of_systems.jpg",   "System of systems"),
    ("trunk/40_mechanism_vs_policy.md",          "phase_discipline.jpg",          "Discipline"),
]


# In-line illustrations: each entry names file, anchor, image, alt, width.
# `before` means insert immediately before the anchor line.
INLINE = [
    # §4 - cost & budget - Ohm's Law alongside the "millivolts and microamps" line.
    ("trunk/04_cost_and_budget.md",
     "*Good design is measured in millivolts and microamps*",
     "../illustrations/ohms_law.jpg", "Ohm's Law: V = I·R", 380, "before-paragraph"),

    # §6 - a row is a tuple - CAD bearing drawing where rows are first explained.
    ("trunk/06_a_row_is_a_tuple.md",
     "A creature at index 17 has its position",
     "../illustrations/cad_bearing.jpg", "A bearing's dimensioned drawing names every field", 400, "before-paragraph"),

    # §12 - event time vs tick time - oscilloscope showing sine wave.
    ("trunk/12_event_time_vs_tick_time.md",
     "The tick rate is *how often the loop runs*",
     "../illustrations/oscilloscope_sine.jpg", "An oscilloscope: sample rate is independent of signal frequency", 420, "before-paragraph"),

    # §14 - DAG - planning checklist illustration.
    ("trunk/14_systems_compose_into_a_dag.md",
     "Draw the dependency graph.",
     "../illustrations/dag_planning_checklist.jpg", "PLAN / ANALYZE / DESIGN / BUILD / TEST / IMPROVE - the planning DAG", 420, "before-paragraph"),

    # §14 - second illustration at chapter close: visualize the problem.
    ("trunk/14_systems_compose_into_a_dag.md",
     "## What's next",
     "../illustrations/tip_visualize_full.jpg", "Visualize the problem. A good diagram can reveal the solution.", 280, "before"),

    # §16 - determinism - assumptions-define-the-model illustration.
    ("trunk/16_determinism_by_order.md",
     "In an ECS architecture, determinism is structural",
     "../illustrations/note_assumptions_full.jpg", "Assumptions define the model. Know them, question them, and test them.", 280, "before-paragraph"),

    # §38 - storage systems - power supply with components.
    ("trunk/38_storage_systems.md",
     "Three concrete examples worth keeping in mind",
     "../illustrations/power_supply_components.jpg", "Storage systems have bandwidth and IOPS - counted like power and current", 420, "before-paragraph"),

    # §41 - compression-oriented - break complex problems into smaller parts.
    ("trunk/41_compression_oriented.md",
     "The discipline is structural, not stylistic.",
     "../illustrations/tip_simplify_full.jpg", "Break complex problems into smaller parts. Simplicity leads to clarity.", 280, "before-paragraph"),

    # §43 - closing - mathematics describes / models the real world.
    ("trunk/43_tests_are_systems.md",
     "If this book changed how you think about programs",
     "../illustrations/mathematics_describes.jpg", "Mathematics describes, models, implements, and improves the world.", 380, "before-paragraph"),
    ("trunk/43_tests_are_systems.md",
     "If this book changed how you think about programs",
     "../illustrations/model_real_world.jpg", "Model the real world.", 380, "after-paragraph"),
]


def insert_phase_dividers() -> None:
    for rel_path, image, label in PHASE_DIVIDERS:
        p = BOOK / rel_path
        text = p.read_text(encoding="utf-8")
        img_html = _img(f"../covers/{image}", f"{label} phase", 700)
        if img_html in text:
            continue  # idempotent
        # Insert before the "> *Concept node:" line.
        marker = "> *Concept node:"
        idx = text.find(marker)
        if idx < 0:
            print(f"!! {rel_path}: marker not found, skipping")
            continue
        new = text[:idx] + img_html + text[idx:]
        p.write_text(new, encoding="utf-8")
        print(f"inserted phase divider in {rel_path}")


def insert_title_banners() -> None:
    p = BOOK / "front_matter.md"
    text = p.read_text(encoding="utf-8")
    banners = (
        _img("covers/title_top.jpg",    "Entity Component Systems", 900) +
        _img("covers/title_bottom.jpg", "Existence Based Processing", 900)
    )
    if "covers/title_top.jpg" in text:
        return
    # Insert at the very top, before the H1.
    new = banners + text
    p.write_text(new, encoding="utf-8")
    print("inserted title banners in front_matter.md")


def insert_inline() -> None:
    for rel_path, anchor, src, alt, width, position in INLINE:
        p = BOOK / rel_path
        text = p.read_text(encoding="utf-8")
        img_html = _img(src, alt, width)
        if img_html in text:
            continue
        idx = text.find(anchor)
        if idx < 0:
            print(f"!! {rel_path}: anchor {anchor!r} not found")
            continue
        if position == "before":
            new = text[:idx] + img_html + text[idx:]
        elif position == "before-paragraph":
            # Find the start of the paragraph containing the anchor.
            para_start = text.rfind("\n\n", 0, idx)
            if para_start < 0:
                para_start = 0
            else:
                para_start += 2  # past the "\n\n"
            new = text[:para_start] + img_html + text[para_start:]
        elif position == "after-paragraph":
            # Find the end of the paragraph containing the anchor.
            para_end = text.find("\n\n", idx)
            if para_end < 0:
                para_end = len(text)
            else:
                para_end += 2
            new = text[:para_end] + img_html + text[para_end:]
        else:
            raise ValueError(f"unknown position {position!r}")
        p.write_text(new, encoding="utf-8")
        print(f"inserted {Path(src).name} in {rel_path}")


def main() -> None:
    insert_title_banners()
    insert_phase_dividers()
    insert_inline()


if __name__ == "__main__":
    main()
