# 45 - Living with it

[§44](44_closure.md) closed the first act. The simulator runs: deterministic, scaled past the 1M wall, parallel on disjoint writes, persisted to disk and replayable from its log. On your machine, today, with you watching, it works.

That sentence has three load-bearing qualifiers. *On your machine. Today. With you watching.* The first act earns the verb "works" and stops exactly where those qualifiers bite. The second act is what it costs to remove them - to run the thing on a machine you have never seen, a year from now, while you are asleep.

That cost has a name: **cost of ownership**. It is the sum of every expense a system charges you *after* it first runs - the price of changing it, trusting it, observing it, recovering it, and handing it to someone else. The first act is a capital expense, paid once. The second act is the operating expense, paid for as long as the system lives. For anything that survives, most of the lifetime cost is in the second column.

And operating cost is margin you do not keep. Every byte stored forever, every managed service kept running, every layer someone has to watch and restart is a charge that recurs for the life of the system. This is where the not-free abstractions hide their price: a managed queue, a per-request database, an orchestration tier, a metrics vendor - each bought once for the convenience and billed ever after, in money, in latency, and in the people paid to keep it breathing. The single-node, in-memory discipline of the first act, read as an economic choice, is an operating-cost strategy: fewer parts to rent, fewer boundaries to watch, fewer ways to fail at 3 AM. Hold the moving parts down and the saving falls straight through to margin. That is the same dependency-pricing rule from [§41](41_compression_oriented.md), now read off the balance sheet instead of the source tree.

## Software dies of cost, not bugs

Here is the fact the rest of the book is built around: programs rarely die of bugs. They die of cost. A program becomes too expensive to change, so it ossifies. Too opaque to debug, so every incident is an outage. Too fragile to restart, so nobody dares deploy it. Too entangled with one person's memory, so it dies when they leave. None of those is a failed test. Each is a cost of ownership that grew without bound until the system was cheaper to abandon than to keep.

The first act produced something that works. Whether it *survives* is decided entirely in the second.

## The leverage

This is not a chapter about hygiene, and it is not a chapter about virtue. Nobody here will tell you to write maintainable code because it is the responsible thing to do. The argument is leverage, the same argument as the rest of the book.

A system you can recover, observe, evolve, and hand off is worth far more than one that merely runs - not a little more, far more - because it lives longer and it changes cheaper. Lifespan and change-cost are the two numbers that decide what a piece of software is worth, and the second act is where you set both. For the solo builder or the small team this book is written for, the discipline ahead is the multiplier that turns a demo into an asset you keep. It is the difference between owning one system that lasts a decade and rewriting a worse one every year.

The good news is that the first act already did the hard part. Almost every move in the second act is a payoff of a decision you have already made. The log is the world ([§37](37_log_is_world.md)), so recovery and audit are reading, not rebuilding. Systems are functions over tables ([§13](13_system_as_function.md)), so a metrics collector is just another read-only system. The boundary is a queue ([§35](35_boundary_is_the_queue.md)), so the storage system and the metrics sink hang off the same hook. Tables are columns ([§7](07_structure_of_arrays.md)), so the hardware you have not reached for yet - SIMD, a GPU - is one transfer away. You are not learning a second architecture. You are collecting what the first one already earned.

## What the second act asks

Five questions the first act never had to answer. The chapters ahead take them one at a time.

- **Can you run it unattended?** The human watching the console is gone. The log has to survive power loss, not just a clean shutdown. The system has to say what it is doing at 2 AM with nobody reading `print!`. It has to give the same answer on a machine with a different core count, and - when a missed deadline is a fault and not just a dropped frame - it has to finish on time, every time. This is *operations*, and it is the spine of the second act.
- **Can you change it after it ships?** The first save file you write into the world is a hostage to today's column layout. Renaming a field, splitting one, changing a unit, back-filling a derived column - each is a migration, not an edit. This is *extendibility*, and the triple-store you already built is the start of the answer.
- **Can you reach for more hardware when one core runs out?** The book stops at one core's bandwidth. The structure-of-arrays layout is the precondition for everything past it, and crossing the bus to a vector unit or an accelerator has its own cost model - transfer bandwidth and launch latency - that wants the same dollars-and-cents treatment [§4](04_cost_and_budget.md) gave the cache. This is *performance*, past the wall the first act hit.
- **Do you know where your own advice stops?** Columns are a default, not a law. There are shapes - recursive, topology-heavy, very small, or bound for a non-ECS consumer - where they cost more than they save. And layout cannot rescue you from numerical fragility: a perfectly columnar geometry kernel is still wrong on a degenerate input. Honesty about the limits is part of *maintainability*; advice you cannot bound is advice you cannot trust.
- **Can someone who is not you keep it alive?** Code review, ownership transfer, deprecation, the runbook for the incident at 3 AM. "Onboardable because the data is visible" was one bullet in the closure; the rest of the team-scale layer is where every criterion above degrades fastest under turnover. This is the part of the cost no benchmark reaches, and the book says so plainly when it gets there.

## A note on trust

The first act earned its claims by measuring them on four machines and printing the numbers. The second act keeps that bargain wherever it can. Recovery throughput, hash divergence as a function of core count, the GPU break-even size, the N where columns stop paying - all measured, all reproducible on your own hardware. Where a topic cannot be measured on four boxes - true worst-case timing needs a real-time OS, and the social layer needs a team, not a benchmark - the book says so and argues in the open instead of dressing an opinion as a result. The exclusions are named, not hidden.

The first act was the harder problem, and the book finished it. The second act is the longer one. It is where what you built stops being a thing that ran once and becomes a thing you own.

## Orient yourself

These are not coding exercises; they are an audit. Run them against the simulator you finished in the first act, or against any system you currently maintain.

1. **Kill it.** Stop the process with `kill -9` (or pull the power) midway through a tick that writes the log. Restart. Does it come back to a consistent world, or a half-written one? Note what you would have to build to make the answer "yes". You will build it in the crash-consistency chapter.
2. **Go dark.** Without adding a debugger or a `print!`, answer: how many creatures are alive right now, and how fast is the population changing? If you cannot, you have no observability. Write down the three numbers you would most want on a dashboard.
3. **Count the cost of one change.** Pick a column. Rename it. Count every place that breaks: the struct, the systems, the serializer, every save file already on disk. That count is your cost of ownership for one trivial edit. The second act is about driving it down.

## What's next

The first chapter of the second act takes the unattended question head-on: [§46](46_log_survives_power_loss.md). The console human is gone, and the first thing that breaks without them is recovery - "the log is the world" only while the log survives the stop.
