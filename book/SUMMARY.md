# Summary

[Introduction](front_matter.md)
[Nomenclature](nomenclature.md)

---

# Part 1: Foundation

- [The machine model](trunk/01_the_machine_model.md)
  - [Solutions](trunk/01_the_machine_model_solutions.md)
- [Numbers and how they fit](trunk/02_numbers_and_how_they_fit.md)
  - [Solutions](trunk/02_numbers_and_how_they_fit_solutions.md)
- [The `Vec` is a table](trunk/03_the_vec_is_a_table.md)
  - [Solutions](trunk/03_the_vec_is_a_table_solutions.md)
- [Cost is layout, and you have a budget](trunk/04_cost_and_budget.md)
  - [Solutions](trunk/04_cost_and_budget_solutions.md)

# Part 2: Identity & structure

- [Identity is an integer](trunk/05_identity_is_an_integer.md)
  - [Solutions](trunk/05_identity_is_an_integer_solutions.md)
- [A row is a tuple](trunk/06_a_row_is_a_tuple.md)
  - [Solutions](trunk/06_a_row_is_a_tuple_solutions.md)
- [Structure of arrays (SoA)](trunk/07_structure_of_arrays.md)
  - [Solutions](trunk/07_structure_of_arrays_solutions.md)
- [Where there's one, there's many](trunk/08_where_theres_one_theres_many.md)
  - [Solutions](trunk/08_where_theres_one_theres_many_solutions.md)
- [Sort breaks indices](trunk/09_sort_breaks_indices.md)
  - [Solutions](trunk/09_sort_breaks_indices_solutions.md)
- [Stable IDs and generations](trunk/10_stable_ids_and_generations.md)
  - [Solutions](trunk/10_stable_ids_and_generations_solutions.md)

# Part 3: Time & passes

- [The tick](trunk/11_the_tick.md)
  - [Solutions](trunk/11_the_tick_solutions.md)
- [Event time vs tick time](trunk/12_event_time_vs_tick_time.md)
  - [Solutions](trunk/12_event_time_vs_tick_time_solutions.md)
- [A system is a function over tables](trunk/13_system_as_function.md)
  - [Solutions](trunk/13_system_as_function_solutions.md)
- [Systems compose into a DAG](trunk/14_systems_compose_into_a_dag.md)
  - [Solutions](trunk/14_systems_compose_into_a_dag_solutions.md)
- [State changes between ticks](trunk/15_state_changes_between_ticks.md)
  - [Solutions](trunk/15_state_changes_between_ticks_solutions.md)
- [Determinism by order](trunk/16_determinism_by_order.md)
  - [Solutions](trunk/16_determinism_by_order_solutions.md)

# Part 4: Existence-based processing

- [Presence replaces flags](trunk/17_presence_replaces_flags.md)
  - [Solutions](trunk/17_presence_replaces_flags_solutions.md)
- [Add/remove = insert/delete](trunk/18_add_remove_insert_delete.md)
  - [Solutions](trunk/18_add_remove_insert_delete_solutions.md)
- [EBP dispatch](trunk/19_ebp_dispatch.md)
  - [Solutions](trunk/19_ebp_dispatch_solutions.md)
- [Empty tables are free](trunk/20_empty_tables_are_free.md)
  - [Solutions](trunk/20_empty_tables_are_free_solutions.md)

# Part 5: Memory & lifecycle

- [`swap_remove`](trunk/21_swap_remove.md)
  - [Solutions](trunk/21_swap_remove_solutions.md)
- [Mutations buffer; cleanup is batched](trunk/22_mutations_buffer.md)
  - [Solutions](trunk/22_mutations_buffer_solutions.md)
- [Index maps](trunk/23_index_maps.md)
  - [Solutions](trunk/23_index_maps_solutions.md)
- [Append-only and recycling](trunk/24_append_only_and_recycling.md)
  - [Solutions](trunk/24_append_only_and_recycling_solutions.md)
- [Ownership of tables](trunk/25_ownership_of_tables.md)
  - [Solutions](trunk/25_ownership_of_tables_solutions.md)

# Part 6: Scale

- [Hot/cold splits](trunk/26_hot_cold_splits.md)
  - [Solutions](trunk/26_hot_cold_splits_solutions.md)
- [Working set vs cache](trunk/27_working_set_vs_cache.md)
  - [Solutions](trunk/27_working_set_vs_cache_solutions.md)
- [Sort for locality](trunk/28_sort_for_locality.md)
  - [Solutions](trunk/28_sort_for_locality_solutions.md)
- [The wall at 10K → 1M](trunk/29_wall_10k_to_1m.md)
  - [Solutions](trunk/29_wall_10k_to_1m_solutions.md)
- [Moving beyond the wall](trunk/30_streaming_wall.md)
  - [Solutions](trunk/30_streaming_wall_solutions.md)

# Part 7: Concurrency

- [Disjoint write-sets parallelize freely](trunk/31_disjoint_writes_parallelize.md)
  - [Solutions](trunk/31_disjoint_writes_parallelize_solutions.md)
- [Partition, don't lock](trunk/32_partition_dont_lock.md)
  - [Solutions](trunk/32_partition_dont_lock_solutions.md)
- [False sharing](trunk/33_false_sharing.md)
  - [Solutions](trunk/33_false_sharing_solutions.md)
- [Order is the contract](trunk/34_order_is_the_contract.md)
  - [Solutions](trunk/34_order_is_the_contract_solutions.md)

# Part 8: I/O & persistence

- [The boundary is the queue](trunk/35_boundary_is_the_queue.md)
  - [Solutions](trunk/35_boundary_is_the_queue_solutions.md)
- [Persistence is table serialization](trunk/36_persistence_is_serialization.md)
  - [Solutions](trunk/36_persistence_is_serialization_solutions.md)
- [The log is the world](trunk/37_log_is_world.md)
  - [Solutions](trunk/37_log_is_world_solutions.md)
- [Storage systems: bandwidth and IOPS](trunk/38_storage_systems.md)
  - [Solutions](trunk/38_storage_systems_solutions.md)

# Part 9: System of systems

- [System of systems](trunk/39_system_of_systems.md)
  - [Solutions](trunk/39_system_of_systems_solutions.md)

# Part 10: Discipline

- [Mechanism vs policy](trunk/40_mechanism_vs_policy.md)
  - [Solutions](trunk/40_mechanism_vs_policy_solutions.md)
- [Compression-oriented programming](trunk/41_compression_oriented.md)
  - [Solutions](trunk/41_compression_oriented_solutions.md)
- [You can only fix what you wrote](trunk/42_you_can_only_fix_what_you_wrote.md)
  - [Solutions](trunk/42_you_can_only_fix_what_you_wrote_solutions.md)
- [Tests are systems; TDD from day one](trunk/43_tests_are_systems.md)
  - [Solutions](trunk/43_tests_are_systems_solutions.md)

# Closure

- [What you have built](trunk/44_closure.md)

---

# For lecturers

- [Concept DAG](../concepts/dag.md)
- [Glossary](../concepts/glossary.md)
- [Simulator specification](../code/sim/SPEC.md)
