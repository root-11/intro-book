# CLAUDE.md

Durable context for this repo. Don't restate the project plan here — that lives in `PLAN.md`.

## Project
Textbook: **An Introduction to Programming, using ECS & EBP in Rust**.
Audience: adult learners and motivated undergrads who want to build things they own. Frame: leverage, not virtue.

## Working mode
- Claude authors, Bjorn reviews. All output is markdown.
- Repo will publish to Codeberg.
- Sections are 2-3 pages of prose followed by 4-12 compounding exercises.
- Trunk-first: write the shared concept core before the four track openings.
- Concepts get named *after* they are built in the exercises, not before.

## Conventions
- No emojis.
- Terse prose. Exercises carry ~80% of the teaching weight; prose is connective tissue.
- Dependencies are *priced*, not banned: write the from-scratch version first, read the crate, then decide.
- The book aims at C, B5, B4, B1 (career changers, pre-disgusted, leverage seekers, artifact builders). B2 and B3 are reachable via framing. The median CRUD job market is out of scope.

## Tooling
- **Python**: `pyproject.toml` declares the project; `.venv/` is the project venv (created with `uv venv`, gitignored). All Python runs via `uv run` so nothing pollutes the global environment.
- **Cargo binaries are repo-local**: `mdbook` and `mdbook-mermaid` install into `.cargo/bin/` (gitignored) via `CARGO_INSTALL_ROOT=$(pwd)/.cargo cargo install mdbook mdbook-mermaid --locked`. Never installed globally.
- **PDF reading**: `tools/read_pdf.py` (PEP 723 inline-deps, run via `uv run tools/read_pdf.py <path>`).
- **HTML reading**: `tools/read_html.py` (same approach; uses BeautifulSoup).
- Reference monograph path is in `.env` (gitignored): `MONOGRAPH_PATH`. The monograph is Bjorn's existing technical reference and informs the trunk vocabulary.
- Simlog reference implementation: vendored at `book/simlog/logger.py` (tracked in git) so the link in §37 always resolves. `SIMLOG_PATH` in `.env` (gitignored) is an optional override — when set and the file exists, `build.py` copies the live source over the vendored copy at staging time. Refresh the vendored snapshot by re-copying from the source repo whenever you want.
- Reference for build style: Daniel Krupke's CP-SAT primer (`https://d-krupke.github.io/cpsat-primer/`, source cloned to `/tmp/cpsat-primer/`). Same pattern: `chapters/*.md` source files, `mdbook` for the rendered site.

## Build flow

The book is built by `build.py`:

    uv run build.py              # stage + render → dist/
    uv run build.py --stage-only # stage only (.mdbook/)

Layout: source markdown lives in `book/` plus `concepts/` and `code/sim/SPEC.md`. `build.py` stages everything into `.mdbook/` (with cross-link paths rewritten so everything resolves under one tree), then invokes the local `.cargo/bin/mdbook` to render into `dist/`. `book.toml` at repo root configures mdbook with `src = ".mdbook"`.

Mermaid diagrams (in `concepts/dag.md`) are rendered via the `mdbook-mermaid` preprocessor. Its assets (`mermaid.min.js`, `mermaid-init.js`) live at repo root because mdbook resolves `additional-js` relative to `book.toml`'s directory, not `src`.

For live preview: `PATH=$(pwd)/.cargo/bin:$PATH mdbook serve` after a stage run.

## Output target
- HTML book, served like the CP-SAT primer. Heavy cross-linking; no strict reading order beyond the DAG.
- `README.md` is the entire book in one file, generated.
- License: CC BY 4.0 for prose, MIT-or-Apache-2.0 for code samples.
- Mascot: a mouse with glasses and a small butterfly (Krupke uses a platypus). Used for callout boxes and chapter cover images once we render.

## Through-line simulator
- Subject: a simple ecosystem (creatures, food, hunger, reproduction, starvation) on a 2D grid under closed-loop control. Spec at `code/sim/SPEC.md`.
- The *shape* — variable quantity under closed-loop control — was inspired by Bjorn's twenty-year-old fission-control simulator. The book uses an ecosystem instead because the audience has the vocabulary for it without prior physics training. The fission anecdote survives as a sidebar.
- Variable-quantity by definition: population grows (reproduction) and shrinks (starvation) every tick.
- Predator/prey, sexual reproduction, and genetics are flagged as extensions for enthusiastic students — not in the main book.

## Agreed structural decisions
- **Through-line anchor**: little-world simulator that grows from "100 particles in a box" to an ecosystem with hunger, reproduction, predation. Secondary anchor for non-simulation sections: a tabular data tool (CSV in, schema, query).
- **Scale-as-spine**: each part of the book moves up one order of magnitude in problem size (100 → 10K → 1M → streaming). Each step forces the next technique.
- **Chapter Zero** is a "what do you want to build?" sorter into four track openings.
- **Track openings (~3 weeks each)**: parallel sum, taxi-CSV viz, rollback-replicated sim, Twitter-on-one-box, multi-agent swarm. Each delivers students into the trunk with the same intuitions in domain-native language.
- **Trunk** merges from week 4 onward.
- **Concept DAG** is published in the front matter.

## What not to do
- Don't write classical-textbook reference chapters ("here are the 12 integer types"). Concepts are introduced when an exercise demands them.
- Don't front-load machine-reality content (cache, branches, SIMD). It belongs wherever the simulation first hurts.
- Don't import OOP/FP/procedural framing as a baseline to contrast against. The book reads as if those traditions were never assumed.
- Don't add libraries to teaching code without applying the from-scratch-first rule.
