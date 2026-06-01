# /// script
# requires-python = ">=3.10"
# dependencies = ["numpy"]
# ///
"""Reproduce the §37 simlog throughput numbers.

    uv run book/simlog/benchmark.py

Times `SimLogSparse.log()` (the winner from logger.py's head-to-head) at 5 and
11 populated fields per row, 2 million rows each - the workload §37 quotes. The
book reports ~934 ns (5 fields) and ~1906 ns (11 fields) on the author's box;
your machine will differ, but the shape (sub-2 us, wider rows cost more) is the
claim. Run it, and you do not have to take the number on trust.
"""

import shutil
import tempfile
import time

from logger import SimLogSparse

N_ROWS = 2_000_000
BUFFER = 200_000

# 11-field schema: 2 strings, the rest numeric (the simulation-event shape).
FIELDS = dict(
    time="f8", value="f8", activity="U16", entity_type="U12", entity_id="i8",
    mission_id="i8", lp="i8", task_id="i8", priority="i8", priority_group="i8",
    derived_priority="i8",
)

ROW_11 = dict(
    time=0.042, value=51.6, activity="picking", entity_type="bot", entity_id=42,
    mission_id=500, lp=2, task_id=100, priority=3, priority_group=1, derived_priority=6,
)
# 5 populated fields; the other six stay unset (sparse storage skips them).
ROW_5 = dict(time=0.042, value=51.6, activity="picking", entity_type="bot", entity_id=42)


def bench(label, row):
    d = tempfile.mkdtemp(prefix="simlog_bench_")
    try:
        with SimLogSparse(d, fields=FIELDS, buffer_size=BUFFER, mode="w") as lg:
            log = lg.log  # bind once; attribute lookup is not what we are timing
            t0 = time.perf_counter()
            for _ in range(N_ROWS):
                log(**row)
            elapsed = time.perf_counter() - t0
    finally:
        shutil.rmtree(d, ignore_errors=True)
    print(f"  {label:<12} {elapsed / N_ROWS * 1e9:7.0f} ns/call   ({elapsed:.2f} s for {N_ROWS:,} rows)")


def main():
    print(f"SimLogSparse.log(), {N_ROWS:,} rows, buffer_size={BUFFER:,}")
    bench("5 fields", ROW_5)
    bench("11 fields", ROW_11)


if __name__ == "__main__":
    main()
