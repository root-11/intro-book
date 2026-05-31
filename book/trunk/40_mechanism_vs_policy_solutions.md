# Solutions: 40 - Mechanism vs policy

## Exercise 1 - Find the mechanism

| system            | mechanism                                       | policy                                  |
|-------------------|-------------------------------------------------|-----------------------------------------|
| `motion`          | apply position + velocity update                | none (velocity comes from elsewhere)    |
| `food_spawn`      | none                                            | when and where food appears             |
| `next_event`      | compute next collision/event time per creature  | the event categorisation rules          |
| `apply_eat`       | apply consumption mechanics                     | eats rule (collision detection)         |
| `apply_reproduce` | apply fission, split fuel                       | reproduction threshold and offspring count |
| `apply_starve`    | mark for removal                                | starvation rule (`energy <= 0`)         |
| `cleanup`         | apply mutations                                 | none (pure mechanism)                   |
| `inspect`         | read world state                                | none (pure mechanism for observation)   |

`cleanup` is the cleanest mechanism in the simulator: no decisions, just commits. `inspect` is the cleanest read-only mechanism. `apply_starve` and `apply_reproduce` are mostly policy with a thin mechanism layer.

## Exercise 2 - Replace a policy

```rust,no_run
fn apply_starve(/* ... */) {
    for i in 0..energy.len() {
        if energy[i] < -10.0 && age[i] > 100 {  // changed from `<= 0.0`
            to_remove.push(ids[i]);
        }
    }
}
```

`cleanup` does not change. The simulator behaves differently; the kernel is unmoved. This is the test of separation: did the change touch only one file?

## Exercise 3 - Add a new policy

```rust,no_run
fn apply_predation(/* ... */) {
    for i in 0..creatures.len() {
        if is_predated(creatures[i], world) {
            to_remove.push(ids[i]);
        }
    }
}
```

Both `apply_starve` and `apply_predation` push to `to_remove`. Cleanup applies the union without distinction. The two policies compose because they produce the same shape of output (an id to remove); the mechanism does not care why.

## Exercise 4 - Anti-pattern

Common offender: a logger system that writes directly to disk inside the system body, rather than pushing to an output queue. It mixes "decide what to log" (policy) with "write to disk" (mechanism). Refactor the disk write into a queue + cleanup-style flush.

## Exercise 5 - A second mechanism

A `cleanup_with_archive` mechanism reads `to_remove` and, instead of `swap_remove`-ing the row, copies it into a `dead` archive table before removing it from `creatures`. Policies (`apply_starve`, `apply_predation`) are unchanged - they still push to `to_remove`. Switch between the two mechanisms by swapping which one is in the DAG, not by editing any policy.
