# Nomenclature

Quick reference for symbols, notation, and abbreviations the book uses. Concept *definitions* live in the [glossary](../concepts/glossary.md); this page covers the shorthand only.

## Symbols

| Symbol | Meaning |
|---|---|
| §N | Section number — e.g., §5 refers to section 5. |
| → | Leads to / becomes / transitions to. Appears in section titles (e.g., §29 "10K → 1M") and prose. |
| `[!NOTE]` / `[!TIP]` / `[!WARNING]` | Callout box — content the reader should pay particular attention to. |

## Text formatting

| Form | Meaning |
|---|---|
| `monospace` | Code: types, variable names, function names, file paths. |
| *italic* | First definition of a term, or emphasis. |
| **bold** | A term being highlighted as load-bearing in the current paragraph. |

## Variables you will see across chapters

| Variable | Meaning |
|---|---|
| `i`, `j` | Index into a table. `i` is the index of the row currently under discussion. |
| `t` or `tick` | Tick number — the simulator's step counter. |
| `id` | Stable entity identifier (an integer). |
| `gen` | Generation counter, paired with a slot index to detect stale references (§10). |
| `pos`, `vel` | Position and velocity of a creature. |
| `to_remove`, `to_insert` | Buffers of pending mutations applied at end-of-tick (§22). |

## Rust types used in code

| Type | What it is |
|---|---|
| `Vec<T>` | Heap-allocated, growable array of `T`. The book's "table." |
| `&[T]` | Read-only borrow of a contiguous slice. |
| `&mut [T]` | Mutable borrow of a contiguous slice. |
| `usize` | Pointer-sized unsigned integer. Used for table indices. |
| `u8` / `u16` / `u32` / `u64` | Unsigned integers, sized in bits. |
| `f32` / `f64` | 32-bit and 64-bit floats. |

## Abbreviations

| Acronym | Expanded |
|---|---|
| ECS | Entity-Component-Systems |
| EBP | Existence-Based Processing |
| DOD | Data-Oriented Design |
| SoA | Structure of Arrays — each field is its own column. |
| AoS | Array of Structures — each row is its own struct. |
| DAG | Directed Acyclic Graph |
| IOPS | I/O Operations Per Second |
| TDD | Test-Driven Development |
| LRU | Least Recently Used (cache eviction policy) |

## Naming in code

- `snake_case` for variables, functions, fields.
- `PascalCase` for types and traits.
- `SCREAMING_SNAKE` for constants.
- File names mirror their dominant content: `creatures.rs` defines the creature table, `motion.rs` the motion system.
