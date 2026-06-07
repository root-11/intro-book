# 50 - It runs without you

[§45](45_living_with_it.md) opened the second act with five questions. The four chapters since answered the first one - *can you run it unattended* - and it is worth stopping to see that they were not four tricks. They were one move, made four times.

[§46](46_log_survives_power_loss.md): the system survives the stop. [§47](47_observation_is_a_system.md): it says what it is doing. [§48](48_reductions_dont_parallelize_freely.md): it gives the same answer on every machine. [§49](49_worst_case_is_the_only_case.md): you know the deadline it can and cannot promise. Listed flat they look unrelated. They are not. Each one removed a dependency on the human who used to stand next to the machine.

The human who restarted it after a crash became the commit marker and the replay ([§46](46_log_survives_power_loss.md)). The human who watched the console became a read-only metrics system ([§47](47_observation_is_a_system.md)). The human who only ever ran it on the one laptop became a deterministic reduction ([§48](48_reductions_dont_parallelize_freely.md)). The human who hit stop in time became a bounded worst case ([§49](49_worst_case_is_the_only_case.md)). Act one made the system *work*. These four made it work *without you in the room*.

And the move was the same shape every time: **take a failure that used to be a person's vigilance to catch, and turn it into a property the system holds and a test you can run.** A torn write stopped being "hope the power doesn't go out" and became a commit marker you assert on. "Is it healthy" stopped being a feeling and became a metrics table you query. "Same seed, same world" stopped being true-on-my-machine and became a CI check across core counts. A missed deadline stopped being "it felt sluggish" and became a jitter histogram with a bound. Operations is not a toolbox. It is the systematic conversion of *someone is watching* into *something is asserted*.

That conversion is the whole economic point from [§45](45_living_with_it.md), paid down. A system that needs a human in the loop costs a salary for as long as it runs; a system that runs without one costs almost nothing to operate. Each chapter in this group retired a recurring cost - not a feature added, a person's standing attention no longer required. That is operating cost falling straight through to margin, exactly as promised, and it is why the unattended question was worth four chapters.

It is also the hardest of the second act's five promises to keep, which is why it came first. The system now survives, reports, agrees, and respects its deadlines. It is still, though, frozen in the shape you shipped it in. The remaining questions are about *change*: the schema it persists drifts the moment the world does; the single core it runs on is not the only hardware in the building; the advice this book has given has limits worth knowing before you trust it past them; and one day someone who is not you will own it. Those are the rest of the map - the [horizon](44_closure.md#the-horizon-living-with-it-at-production-scale) this book has charted and not yet walked, the road ahead for you or for a later volume.

The operations leg, though, is walked. The machine in the next room is running, nobody is watching it, and that is precisely the point.

## Where to go next

- **Read Mike Acton's "Data-Oriented Design and C++"** (CppCon 2014). Forty-five minutes; the most concentrated case for this approach you will find.
- **Read Casey Muratori's *Handmade Hero*** episodes on grid storage and cache locality. Another route to the same conclusions.
- **Open Bevy's `bevy_ecs` crate.** You will recognise every pattern. The names will differ; the shapes are identical.
- **Extend the simulator.** The genetics and predator-prey extensions flagged in the [simulator spec](../../code/sim/SPEC.md) break new ground without leaving the framework you have already built.

<p align="center"><img src="../illustrations/model_real_world.jpg" alt="Model the real world." style="max-height: 300px; max-width: 100%;"></p>

The book ends here. The simulator does not - it runs as long as you keep the discipline.
