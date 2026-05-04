# PLAN.md

A proposal for review. Amend in place.

## Goal
Write an entry-level programming textbook in Rust, taught from first principles of ECS and existence-based processing, that delivers students into a state where Bjorn's monograph (`MONOGRAPH_PATH`) reads naturally. The textbook is the on-ramp; the monograph is the destination.

## What the textbook is *not*
- Not a replacement for the monograph. Distributed ECS, API-Compiler, temporal patterns, log-as-database — all monograph territory. The textbook stays single-node and in-memory.
- Not a Rust language tour. Rust enters through a narrow door (`Vec<T>`, structs, indices, `for`, `match`, `&mut [T]`). Lifetimes and traits arrive late, as answers to felt problems.
- Not an ECS framework manual. The student's own ECS skeleton is the only ECS taught. A "further reading" pointer at the back lists Bevy and other production engines as specimens to read on their own.

## Workflow
- **Author/review loop**: Claude writes; Bjorn reviews.
- Every deliverable is markdown in this repo.
- One section ≈ one commit. Small enough to review in a sitting.
- Review: inline HTML comments in the markdown, or PR comments once we're on Codeberg. Resolved notes get deleted; unresolved ones stay.
- Decisions captured in `notes/decisions.md`. Open questions accumulate in `notes/open-questions.md` and get resolved in batches.

## Proposed repo layout
```
intro/
├── CLAUDE.md                # durable context
├── PLAN.md                  # this file
├── README.md                # public-facing intro (rewrite later)
├── .env                     # gitignored, holds MONOGRAPH_PATH
├── .gitignore
├── tools/
│   └── read_pdf.py          # uv inline-deps script
├── concepts/
│   ├── dag.md               # the published concept DAG (front-matter artifact)
│   └── glossary.md          # trunk vocabulary, named things and their definitions
├── book/
│   ├── 00-front/            # title page, preface, "what do you want to build?" sorter
│   ├── tracks/              # four ~3-week track openings
│   │   ├── multicore/       # parallel sum hook
│   │   ├── data/            # taxi-CSV viz hook
│   │   ├── multiplayer/     # rollback-replicated sim hook
│   │   └── twitter/         # social feed at scale hook
│   ├── trunk/               # shared core, ordered by DAG
│   └── 99-back/             # afterword + bridge to monograph
├── code/
│   ├── sim/                 # through-line simulator (anchor 1)
│   └── tabular/             # CSV/schema/query tool (anchor 2)
├── exercises/               # per-section exercise specs + reference solutions
└── notes/
    ├── decisions.md         # ADR-style log
    └── open-questions.md
```

## Order of work

**M0 — Alignment.** This plan, accepted or amended. Output: `PLAN.md` v1, `CLAUDE.md` v1.

**M1 — The spine.**
- `concepts/dag.md`: ~40 nodes, edges drawn, Mermaid or plain.
- `concepts/glossary.md`: trunk vocabulary. Each named thing gets a one-paragraph definition at the level the textbook will use it. This is what every track must converge on.
- This is the biggest review milestone. Once accepted, every later chapter must obey it.

**M2 — Through-line simulator skeleton.**
- `code/sim/`: a working Rust program that uses every trunk concept at least once. Built deliberately so chapter authoring can refer to "the version of the simulator at this scale".
- This is the *autobiography reference* the book is written backwards from.

**M3 — Format calibration.**
- One fully worked trunk section (proposal: §2.4 — Identity is an integer). Prose + 4-12 exercises + reference solutions.
- Purpose: validate that the 2-3 page + exercise format actually carries the weight we want it to. Adjust template before scaling out.

**M4 — Trunk in DAG order.** Sections written one at a time, each a small commit. Pace set by review bandwidth.

**M5 — Five track openings.** Multicore, data, multiplayer, twitter, multi-agent. Written *after* the trunk, as constrained writing exercises that must converge on it. Trunk-first means we know exactly what intuitions each opening must deliver.

**M6 — Chapter Zero, front matter, bridge to monograph.** The sorter, the preface, the published DAG, the afterword that hands the reader to the monograph.

**M7 — Read-through pass and Codeberg publish.** Single coherent read of the whole book. Fix what wobbles. Push.

## Tooling decisions to make now
- **Build system**: my recommendation is `mdbook` for HTML, `pandoc` for PDF, both driven from the same markdown. Cheap, standard, works on Codeberg's CI if needed.
- **Mermaid for diagrams**: renders in mdbook with a plugin, renders on Codeberg's web view, plain text in git.
- **Reference solutions**: live alongside exercise specs in `exercises/<section>/`. Solutions in their own files so the student-facing markdown can omit them, but they're versioned together.
- **Rust edition**: 2024 (current stable). One toolchain pinned via `rust-toolchain.toml` in `code/`.
- **License**: CC BY-SA 4.0 for prose, MIT-or-Apache-2.0 dual for code samples. Easy to change before publish.

## Decisions
1. **Hosting**: develop in this local repo. Codeberg is a mirror, configured later, not a development concern.
2. **Title**: *An Introduction to Programming, using ECS & EBP in Rust*.
3. **No framework dependency.** Bevy is dropped. Unlike OR-tools for constraint programming — where hand-rolling a solver isn't viable, so a teaching companion is justified — the student's own ECS *is* a real ECS. A framework chapter would dilute the spine. A one-paragraph "further reading" pointer at the back is enough.
4. **API-Compiler**: out of scope. The bridge to the monograph at the end says "here's the next book," not a teaser of its concepts.
5. **Five tracks** (was four): multicore, data, multiplayer, twitter, **multi-agent**. Proposed hook for multi-agent: 10,000 delivery drones in a city, scale to 100,000 in real-time.
6. **Review style**: Bjorn comments interactively in chat, referenced by line number, headline, paragraph, or figure id.
7. **Cadence**: async, no fixed slot.
8. **Build target**: HTML book in the style of Daniel Krupke's CP-SAT primer — `chapters/*.md` source, `mdbook` rendering, `build.py` generates a single-file README and adds callout boxes. Source cloned to `/tmp/cpsat-primer/` for reference.
9. **License**: CC BY 4.0 for prose, MIT-or-Apache-2.0 for code samples.
10. **Mascot**: a mouse with glasses and a small butterfly. Krupke uses a platypus; we have our own.
11. **Through-line simulator subject**: a sub-critical fissile assembly under closed-loop control. Spec at `code/sim/SPEC.md`.
12. **Codeberg mirror**: agreed in principle; account/org name still to be supplied before first push.
13. **Order of glossary vs section prose**: parallel, not sequential. The book is HTML; cross-linking is cheap. Glossary entries reference sections, sections reference glossary entries, both grow in parallel.

## What I propose to do next
- M1 — glossary completion: 36 remaining entries against the calibrated format.
- M2 — through-line simulator: spec drafted at `code/sim/SPEC.md` (this milestone is partially complete; the working code follows once the spec is reviewed).
- M3 (parallel, not sequential): one fully-worked trunk section as format calibration.
- Build infrastructure: `chapters/` directory, `book.toml` for mdbook, `build.py` adapted from Krupke. Set up when there is at least one chapter to render.
