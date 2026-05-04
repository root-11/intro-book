# An Introduction to Programming

*using ECS & EBP in `rust`*

<p align="center"><img src="illustrations/classroom.jpg" alt="A classroom: Understand, Model, Solve, Validate, Improve" style="max-height: 360px; max-width: 100%;"></p>

This book teaches programming from first principles of data-oriented design, entity-component-systems (ECS), and existence-based processing (EBP). It assumes no prior programming experience and uses Rust as the only language.

The book is structured around forty-three concepts ([the DAG](../concepts/dag.md)) and their canonical wording ([the glossary](../concepts/glossary.md)). Sections are short — two to three pages of prose followed by four to twelve compounding exercises. Concepts are *named* only after they are *built*: every section earns its vocabulary through working code, not the other way around.

The through-line is a small ecosystem simulator built in stages from one hundred wandering creatures to a hundred million streamed ones. The simulator's specification is at [`code/sim/SPEC.md`](../code/sim/SPEC.md).

This is a work in progress. Section ordering is by the DAG; reading order can be linear (front to back) or by following the cross-links wherever they lead.

## Who this book is for

You want to build something. You are either coming to programming from another field, or you tried it before and found that what got taught did not match what you wanted to make. You can read code; you may have written some; you have not been bitten enough to feel that programming is *yours* yet.

The book is for people who learn by building artifacts and want technical depth that *compounds* — where each new idea makes the previous one more useful, not just adds another tool to a pile. The through-line is a small ecosystem simulator that grows from a hundred creatures to a hundred million; everything you learn earns its keep on that one program, then transfers everywhere else.

It is not aimed at the median CRUD-application job market. If your goal is "any programming job, fastest," there are faster paths. If your goal is "the kind of programmer whose programs work," this is one of them.

## Background

You should be comfortable with high-school algebra and a command line — running a command, changing directories, reading error messages without panic. A laptop with internet is enough for the first ten sections; for the rest, you will install a Rust toolchain locally.

You do *not* need prior programming experience, calculus, a maths degree, or any prior contact with Rust. The book teaches Rust syntax as each section needs it; the language is a vehicle, not the subject.

## A first taste

Before any vocabulary is named, here is what an ECS world looks like in fifteen lines of Rust. One hundred creatures, each with a position and a velocity, moving for thirty ticks of simulated time. No structs, no traits, no libraries — four `Vec`s indexed in lockstep, and a function (the `for i in 0..x.len()` loop) that advances every creature one step.

```rust,editable
fn main() {
    let mut x:  Vec<f32> = (0..100).map(|i| (i as f32) * 0.1).collect();
    let mut y:  Vec<f32> = (0..100).map(|i| (i as f32).sin()).collect();
    let     vx: Vec<f32> = (0..100).map(|i| ((i * 7) % 11) as f32 * 0.01 - 0.05).collect();
    let     vy: Vec<f32> = (0..100).map(|i| ((i * 13) % 7) as f32 * 0.01 - 0.03).collect();

    for tick in 0..30 {
        for i in 0..x.len() {
            x[i] += vx[i];
            y[i] += vy[i];
        }
        if tick % 10 == 0 {
            println!("tick {tick}: creature 17 at ({:.2}, {:.2})", x[17], y[17]);
        }
    }
}
```

Click play. The simulator runs in your browser, prints three lines, and stops. That is the entire shape of what the rest of the book grows: tables (the `Vec`s), a tick (the outer loop), a system (the inner loop). Everything that follows is the discipline that lets this same shape carry a hundred million creatures without falling apart.

## Running the code

Most code blocks in the early chapters have a play button that runs the code in your browser via the [Rust Playground](https://play.rust-lang.org). Click it, edit, see the result. No setup required. The deck-game exercises in §5, §9, and §10 are designed to be run this way — open the page, hit play, work through the exercises in the editor that appears.

From the simulator chapters onward, the exercises stop being self-contained snippets. They build the through-line: a working Rust program that grows from one hundred wandering creatures to a hundred million streamed ones. Running them needs a local Rust toolchain, a project that holds state between runs, and the ability to time loops on your own hardware. By that point you will want a clone of the book's repo:

```sh
git clone <repo-url>
cd intro
cargo run --release --bin sim
```

For the timing exercises in §1, the play button works but the numbers it produces are not yours — they come from a shared server the playground happens to be running on. The exercise asks "how fast does *your* machine run this?", and that question only has a real answer locally. Click play for a first taste; then run on your own hardware for the numbers the rest of the book references.

The threshold between *playground* and *local* is fuzzy by intent. A reader on a phone or in a classroom can stay in the browser through §10. Beyond that, treat a local toolchain as part of the curriculum.
