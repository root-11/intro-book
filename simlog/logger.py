"""SimLogSparse — high-throughput sparse simulation logger.

Why this design?
----------------
We benchmarked seven logger architectures head-to-head on realistic
simulation workloads (2 M rows, 11–34 fields, ~5 populated per row,
2 string fields, 9 numeric).  Results on a single core (ns per log() call):

    Architecture                 5 kw    11 kw   Strings?  Sparse?
    ─────────────────────────── ────── ──────── ───────── ────────
    list.append (baseline)          12                –        –
    SimLogFast   (numpy SoA)       810    1 495      no*      no
    SimLogLists  (list→numpy)      811    1 550      no*      no
    SimLogSparse (list COO)        934    1 906      yes     yes  ◄─ winner
    SimLogCodebook (dual COO)      967    1 950      yes     yes
    SimLogSparseNP (numpy COO)   1 161    2 199      yes     yes
    SimLog (numpy SoA, kwargs)   2 271    2 271      no      no
    SimLogText (logging)         6 548                yes      –

    * disqualified: caller must pre-encode strings to ints.

Key findings:
  • SimLogFast/SimLogLists are fastest but cannot accept strings natively,
    pushing encoding work onto the caller — unfair in a general-purpose API.
  • Pre-allocated numpy arrays (SimLogSparseNP) are *slower* than Python
    lists: numpy scalar writes incur indexing/dtype overhead per element,
    while list.append() is a single amortised C call.
  • Dual-stream COO (SimLogCodebook) adds ~3 % overhead over single-stream
    (SimLogSparse) with no benefit — float64 covers both int and float.
  • At typical fill rates (5 of 34 fields), sparse storage uses ~6× less
    working memory than dense column arrays.

SimLogSparse wins on: speed among string-capable loggers, memory at low
fill rates, and API simplicity (just pass kwargs with any mix of str/int/float).

Architecture
------------
  • Schema-free: field names and types auto-discovered from kwargs.
  • Sparse COO format: only populated fields stored per row.
  • Single float64 value stream (integers ±2^53 round-trip exactly).
  • Evolving string codebook (strings → integer codes, stored once).
  • Double-buffered Python-list containers with background flush thread.
  • Disk format: .npz chunks + _codebook.json.

Quick start
-----------
    # write — field names and types discovered from kwargs
    with SimLogSparse('logs/', mode='w') as log:
        log.log(time=1.0, value=42.0, activity='picking', entity_id=7)
        log.log(time=2.0, value=13.0)  # sparse: only 2 of 4 fields

    # read back as numpy arrays + boolean masks
    arrays, masks = SimLogSparse('logs/', mode='r').to_arrays()

    # iterate rows (strings decoded automatically)
    for row in SimLogSparse('logs/', mode='r'):
        print(row)  # {'time': 1.0, 'value': 42.0, 'activity': 'picking', ...}

    # export
    SimLogSparse('logs/', mode='r').to_csv('out.csv')
    SimLogSparse('logs/', mode='r').to_sqlite('out.db')  # indexed columns
"""

import json
import sqlite3
import threading
from pathlib import Path

import numpy as np


# ---------------------------------------------------------------------------
# Container
# ---------------------------------------------------------------------------

class _SparseF64Container:
    __slots__ = ('rids', 'keys', 'vals', 'row_count')

    def __init__(self):
        self.rids = []
        self.keys = []
        self.vals = []
        self.row_count = 0

    def reset(self):
        self.rids.clear()
        self.keys.clear()
        self.vals.clear()
        self.row_count = 0


# ---------------------------------------------------------------------------
# SimLogSparse
# ---------------------------------------------------------------------------

class SimLogSparse:
    """Sparse codebook logger with a single float64 value stream.
    All values stored as float64. Integers round-trip exactly for |v| < 2^53."""

    def __init__(self, path, fields: dict[str, str] | None = None, mode='r', buffer_size=200_000):
        self._path = Path(path)
        self._mode = mode
        self._buffer_size = buffer_size

        if mode == 'w':
            if fields is not None:
                self._names = list(fields.keys())
                self._dtypes = [np.dtype(v) for v in fields.values()]
            else:
                self._names = []
                self._dtypes = []
            self._n = len(self._names)
            self._init_writer()
        elif mode == 'r':
            self._init_reader()
        else:
            raise ValueError(f"mode must be 'r' or 'w', got {mode!r}")

    # ------------------------------------------------------------------
    # writer init
    # ------------------------------------------------------------------

    def _init_writer(self):
        self._path.mkdir(parents=True, exist_ok=True)
        self._seq = 0

        self._kc = {name: i for i, name in enumerate(self._names)}

        # type tracking: key_code -> 'i' or 'f' (recorded on first encounter)
        self._key_types = {}

        # set of key codes known to be string-valued
        self._str_keys = set()

        # evolving string codebook
        self._str_codes = {}
        self._str_list = []

        # Note: Two containers for double-buffering: one active for logging, one for flushing.
        # If writer threads appears as the bottleneck, we could increase to a pool of N buffers and a queue of flush-ready buffers.
        self._buf = [_SparseF64Container(), _SparseF64Container()]
        self._active = 0

        self._flush_ready = threading.Event()
        self._flush_ready.set()
        self._flush_needed = threading.Event()
        self._flush_ref = None
        self._flush_count = 0
        self._done = False

        self._writer = threading.Thread(target=self._drain, daemon=True)
        self._writer.start()

    # ------------------------------------------------------------------
    # reader init
    # ------------------------------------------------------------------

    def _init_reader(self):
        if not self._path.exists():
            raise FileNotFoundError(self._path)
        chunks = sorted(self._path.glob('chunk_*.npz'))
        if not chunks:
            raise FileNotFoundError(f"no chunk files in {self._path}")
        cb_path = self._path / '_codebook.json'
        if not cb_path.exists():
            raise FileNotFoundError(
                f"no _codebook.json in {self._path} — was the writer closed properly?")
        with open(cb_path) as f:
            cb = json.load(f)
        with np.load(chunks[0]) as data:
            chunk0_meta = json.loads(data['_meta'].tobytes().decode())
        self._buffer_size = chunk0_meta['buffer_size']
        self._str_list = cb.get('codebook', [])
        self._str_keys = set(cb.get('str_keys', []))
        self._key_types = {int(k): v for k, v in cb.get('key_types', {}).items()}
        fields = cb['fields']
        self._names = list(fields.keys())
        self._dtypes = [np.dtype(d) for d in fields.values()]
        self._n = len(self._names)

    @property
    def codebook(self):
        return list(self._str_list)

    # ------------------------------------------------------------------
    # schema introspection
    # ------------------------------------------------------------------

    def _fields_dict(self):
        """Current field->dtype-string mapping (for metadata / codebook)."""
        result = {}
        for i, name in enumerate(self._names):
            if i < len(self._dtypes):
                result[name] = str(self._dtypes[i])
            elif i in self._str_keys:
                result[name] = 'int32'
            elif self._key_types.get(i) == 'i':
                result[name] = 'int64'
            else:
                result[name] = 'float64'
        return result

    # ------------------------------------------------------------------
    # hot path
    # ------------------------------------------------------------------

    def log(self, **kwargs):
        """Append one record.  Only provided non-None fields are stored."""
        c = self._buf[self._active]
        rid = c.row_count
        rids = c.rids
        keys = c.keys
        vals = c.vals
        kc = self._kc
        kt = self._key_types
        sk = self._str_keys
        sc = self._str_codes
        sl = self._str_list
        for name, v in kwargs.items():
            k = kc.get(name)
            if k is None:
                k = self._n
                self._names.append(name)
                kc[name] = k
                self._n += 1
            if v is not None:
                if k in sk:
                    sv = sc.get(v)
                    if sv is None:
                        sv = len(sl)
                        sc[v] = sv
                        sl.append(v)
                    v = sv
                elif k not in kt:
                    if isinstance(v, str):
                        sk.add(k)
                        kt[k] = 'i'
                        sv = len(sl)
                        sc[v] = sv
                        sl.append(v)
                        v = sv
                    else:
                        kt[k] = 'i' if isinstance(v, int) else 'f'
                rids.append(rid)
                keys.append(k)
                vals.append(v)
        c.row_count = rid + 1
        if c.row_count >= self._buffer_size:
            self._swap()

    # ------------------------------------------------------------------
    # double-buffer swap + background writer
    # ------------------------------------------------------------------

    def _swap(self):
        self._flush_ready.wait()
        self._flush_ready.clear()
        self._flush_ref = self._buf[self._active]
        self._flush_count = self._flush_ref.row_count
        self._active ^= 1
        self._buf[self._active].reset()
        self._flush_needed.set()

    def _drain(self):
        while True:
            self._flush_needed.wait()
            self._flush_needed.clear()
            ref = self._flush_ref
            count = self._flush_count
            self._flush_ref = None
            if ref is not None:
                self._write_chunk(ref, count)
            self._flush_ready.set()
            if self._done:
                return

    def _write_chunk(self, container, count):
        path = self._path / f'chunk_{self._seq:06d}.npz'
        self._seq += 1
        meta = {
            'buffer_size': self._buffer_size,
            'row_count': count,
        }
        meta_bytes = np.frombuffer(json.dumps(meta).encode(), dtype=np.uint8)

        def to_arr(lst, dtype):
            return np.array(lst, dtype=dtype) if lst else np.array([], dtype=dtype)

        np.savez(path,
            _meta=meta_bytes,
            rids=to_arr(container.rids, np.int32),
            keys=to_arr(container.keys, np.int16),
            vals=to_arr(container.vals, np.float64),
        )

    # ------------------------------------------------------------------
    # read path
    # ------------------------------------------------------------------

    def _iter_chunks(self):
        for chunk_path in sorted(self._path.glob('chunk_*.npz')):
            with np.load(chunk_path) as data:
                meta = json.loads(data['_meta'].tobytes().decode())
                count = meta['row_count']
                yield count, data['rids'], data['keys'], data['vals']

    def __iter__(self):
        if self._mode != 'r':
            raise RuntimeError("open in 'r' mode to iterate")
        kt = self._key_types
        sk = self._str_keys
        inv_cb = self._str_list  # index → string
        for count, rids, keys, vals in self._iter_chunks():
            rows = [{} for _ in range(count)]
            for j in range(len(rids)):
                kc = keys[j].item()
                v = vals[j].item()
                if kc in sk:
                    v = inv_cb[int(v)]
                elif kt.get(kc) == 'i':
                    v = int(v)
                rows[rids[j]][self._names[kc]] = v
            for row in rows:
                for name in self._names:
                    if name not in row:
                        row[name] = None
                yield row

    def to_arrays(self):
        """Return (arrays, masks) dicts keyed by field name."""
        if self._mode != 'r':
            raise RuntimeError("open in 'r' mode to read")
        accum = {n: [] for n in self._names}
        mask_accum = {n: [] for n in self._names}
        for count, rids, keys, vals in self._iter_chunks():
            arrays = {}
            masks = {}
            for name, dtype in zip(self._names, self._dtypes):
                arrays[name] = np.zeros(count, dtype=dtype)
                masks[name] = np.zeros(count, dtype=np.bool_)
            for ki, name in enumerate(self._names):
                if len(rids) > 0:
                    sel = keys == ki
                    if sel.any():
                        arrays[name][rids[sel]] = vals[sel]
                        masks[name][rids[sel]] = True
            for name in self._names:
                accum[name].append(arrays[name])
                mask_accum[name].append(masks[name])
        return (
            {n: np.concatenate(arrs) for n, arrs in accum.items()},
            {n: np.concatenate(arrs) for n, arrs in mask_accum.items()},
        )

    # ------------------------------------------------------------------
    # export: CSV
    # ------------------------------------------------------------------

    def to_csv(self, out_path):
        """Export log data to a CSV file."""
        if self._mode != 'r':
            raise RuntimeError("open in 'r' mode to export")
        with open(out_path, 'w') as f:
            f.write(','.join(self._names) + '\n')
            for row in self:
                parts = []
                for name in self._names:
                    v = row[name]
                    parts.append('' if v is None else str(v))
                f.write(','.join(parts) + '\n')

    # ------------------------------------------------------------------
    # export: SQLite3
    # ------------------------------------------------------------------

    def to_sqlite(self, db_path, table='simlog', page_size=4096, batch_size=50_000):
        """Export log data to a SQLite3 database with indexes on every column.

        Parameters
        ----------
        db_path : str or Path
            Output .db file (created / overwritten).
        table : str
            Table name (default ``simlog``).
        page_size : int
            SQLite page size hint (default 4096).
        batch_size : int
            Rows per INSERT transaction (default 50 000).
        """
        if self._mode != 'r':
            raise RuntimeError("open in 'r' mode to export")

        db_path = Path(db_path)
        if db_path.exists():
            db_path.unlink()

        # map numpy dtype chars to SQLite affinities
        _affinity = {'f': 'REAL', 'i': 'INTEGER', 'u': 'INTEGER'}
        # override for string-coded fields: store the decoded string
        str_field_names = {self._names[k] for k in self._str_keys}
        inv_codebook = {i: s for i, s in enumerate(self._str_list)} if self._str_list else {}

        col_defs = []
        for name, dtype in zip(self._names, self._dtypes):
            if name in str_field_names:
                col_defs.append(f'"{name}" TEXT')
            else:
                aff = _affinity.get(dtype.kind, 'REAL')
                col_defs.append(f'"{name}" {aff}')

        con = sqlite3.connect(str(db_path))
        try:
            cur = con.cursor()
            cur.execute(f'PRAGMA page_size = {page_size}')
            cur.execute('PRAGMA journal_mode = WAL')
            cur.execute('PRAGMA synchronous = NORMAL')

            create_sql = f'CREATE TABLE "{table}" ({", ".join(col_defs)})'
            cur.execute(create_sql)

            placeholders = ', '.join(['?'] * self._n)
            insert_sql = f'INSERT INTO "{table}" VALUES ({placeholders})'

            kt = self._key_types
            buf = []
            for count, rids, keys, vals in self._iter_chunks():
                rows = [dict.fromkeys(self._names) for _ in range(count)]
                for j in range(len(rids)):
                    kc = keys[j].item()
                    v = vals[j].item()
                    name = self._names[kc]
                    if name in str_field_names:
                        v = inv_codebook.get(int(v), str(int(v)))
                    elif kt.get(kc) == 'i':
                        v = int(v)
                    rows[rids[j]][name] = v

                for row in rows:
                    buf.append(tuple(row[n] for n in self._names))
                    if len(buf) >= batch_size:
                        cur.execute('BEGIN')
                        cur.executemany(insert_sql, buf)
                        cur.execute('COMMIT')
                        buf.clear()

            if buf:
                cur.execute('BEGIN')
                cur.executemany(insert_sql, buf)
                cur.execute('COMMIT')
                buf.clear()

            # create one index per column
            for name in self._names:
                idx_name = f'idx_{table}_{name}'
                cur.execute(f'CREATE INDEX "{idx_name}" ON "{table}" ("{name}")')

            con.execute('PRAGMA wal_checkpoint(TRUNCATE)')
        finally:
            con.close()

    # ------------------------------------------------------------------
    # context manager
    # ------------------------------------------------------------------

    def __enter__(self):
        return self

    def __exit__(self, *exc):
        if self._mode == 'w':
            self.close()

    def close(self):
        if self._buf[self._active].row_count > 0:
            self._swap()
        self._flush_ready.wait()
        self._done = True
        self._flush_needed.set()
        self._writer.join()
        cb_path = self._path / '_codebook.json'
        with open(cb_path, 'w') as f:
            json.dump({'codebook': self._str_list,
                       'str_keys': list(self._str_keys),
                       'key_types': {str(k): v for k, v in self._key_types.items()},
                       'fields': self._fields_dict()}, f)


# ===========================================================================
# Tests  (run with: pytest logger.py -v)
# ===========================================================================

if __name__ != '__pytest_main__':  # allow both pytest and direct execution
    pass

import csv
import pytest

ACTIVITIES   = ['picking', 'putaway', 'replen', 'consolidation', 'count', 'transport', 'staging']
ENTITY_TYPES = ['bot', 'human', 'conveyor', 'station']

_NAMES = ['time', 'value', 'activity', 'entity_type', 'entity_id',
          'mission_id', 'lp', 'task_id', 'priority', 'priority_group',
          'derived_priority']


def _make_row(i):
    return dict(
        time=i * 0.001, value=i * 1.23,
        activity=ACTIVITIES[i % len(ACTIVITIES)],
        entity_type=ENTITY_TYPES[i % len(ENTITY_TYPES)],
        entity_id=i % 10_000, mission_id=i % 500, lp=i % 3,
        task_id=i % 200, priority=i % 5, priority_group=i % 3,
        derived_priority=(i % 5) * (i % 3),
    )


@pytest.fixture
def log_dir(tmp_path):
    return str(tmp_path / 'testlog')


# -- correctness -----------------------------------------------------------

def test_roundtrip(log_dir):
    """Write 10k rows, read back via to_arrays, spot-check values."""
    N = 10_000
    with SimLogSparse(log_dir, mode='w', buffer_size=5_000) as lg:
        for i in range(N):
            lg.log(**_make_row(i))

    reader = SimLogSparse(log_dir, mode='r')
    arrays, masks = reader.to_arrays()
    assert all(len(a) == N for a in arrays.values())
    for idx in (0, 1, 5000, 9999):
        row = _make_row(idx)
        for name in ('time', 'value', 'entity_id', 'lp', 'priority'):
            assert np.isclose(arrays[name][idx], row[name]), f"row {idx} {name}"
    cb = reader.codebook
    assert set(ACTIVITIES).issubset(set(cb))
    assert set(ENTITY_TYPES).issubset(set(cb))


def test_none_and_missing(log_dir):
    """Explicit None and omitted fields both produce mask=False."""
    with SimLogSparse(log_dir, mode='w', buffer_size=5_000) as lg:
        lg.log(time=1.0, value=2.0, activity='picking', entity_type='bot',
               entity_id=10, mission_id=None, lp=1, task_id=3,
               priority=2, priority_group=1, derived_priority=4)
        lg.log(time=2.0, value=3.0)  # everything else missing

    arrays, masks = SimLogSparse(log_dir, mode='r').to_arrays()
    assert len(arrays['time']) == 2
    assert masks['mission_id'][0] == False
    assert masks['entity_id'][1] == False
    assert masks['time'][0] == True
    assert masks['time'][1] == True


def test_iterator_coercion(log_dir):
    """Iterator yields int for int-typed fields, float for float-typed."""
    with SimLogSparse(log_dir, mode='w', buffer_size=5_000) as lg:
        lg.log(**_make_row(42))

    row = next(iter(SimLogSparse(log_dir, mode='r')))
    assert isinstance(row['time'], float)
    assert isinstance(row['entity_id'], int)
    assert isinstance(row['activity'], str)  # decoded string
    assert row['activity'] == ACTIVITIES[42 % len(ACTIVITIES)]


def test_iter_roundtrip(log_dir):
    """__iter__ yields every row with strings decoded and None for missing fields."""
    N = 200
    with SimLogSparse(log_dir, mode='w', buffer_size=5_000) as lg:
        for i in range(N):
            lg.log(**_make_row(i))
        lg.log(time=99.0)  # sparse row: most fields missing

    rows = list(SimLogSparse(log_dir, mode='r'))
    assert len(rows) == N + 1

    # spot-check full rows
    for idx in (0, 1, 100, N - 1):
        expected = _make_row(idx)
        r = rows[idx]
        assert r['time'] == pytest.approx(expected['time'])
        assert r['value'] == pytest.approx(expected['value'])
        assert r['activity'] == expected['activity']        # decoded string
        assert r['entity_type'] == expected['entity_type']  # decoded string
        assert r['entity_id'] == expected['entity_id']

    # sparse row: present field correct, missing fields are None
    last = rows[N]
    assert last['time'] == pytest.approx(99.0)
    assert last['value'] is None
    assert last['activity'] is None
    assert last['entity_id'] is None


# -- CSV export -------------------------------------------------------------

def test_csv_export(log_dir):
    N = 500
    with SimLogSparse(log_dir, mode='w', buffer_size=5_000) as lg:
        for i in range(N):
            lg.log(**_make_row(i))

    csv_path = log_dir + '.csv'
    SimLogSparse(log_dir, mode='r').to_csv(csv_path)

    with open(csv_path) as f:
        rows = list(csv.DictReader(f))
    assert len(rows) == N
    assert set(rows[0].keys()) == set(_NAMES)


# -- SQLite3 export ---------------------------------------------------------

def test_sqlite_export(log_dir):
    """Round-trip: write → SQLite → verify row count, types, and indexes."""
    N = 1_000
    with SimLogSparse(log_dir, mode='w', buffer_size=5_000) as lg:
        for i in range(N):
            lg.log(**_make_row(i))

    db_path = log_dir + '.db'
    SimLogSparse(log_dir, mode='r').to_sqlite(db_path)

    con = sqlite3.connect(db_path)
    cur = con.cursor()

    # row count
    cur.execute('SELECT COUNT(*) FROM simlog')
    assert cur.fetchone()[0] == N

    # spot-check first row
    cur.execute('SELECT time, value, activity, entity_type, entity_id FROM simlog LIMIT 1')
    row = cur.fetchone()
    expected = _make_row(0)
    assert abs(row[0] - expected['time']) < 1e-9
    assert abs(row[1] - expected['value']) < 1e-9
    assert row[2] == expected['activity']       # decoded string
    assert row[3] == expected['entity_type']    # decoded string
    assert row[4] == expected['entity_id']

    # verify indexes exist for every column
    cur.execute("SELECT name FROM sqlite_master WHERE type='index' AND tbl_name='simlog'")
    idx_names = {r[0] for r in cur.fetchall()}
    for name in _NAMES:
        assert f'idx_simlog_{name}' in idx_names, f"missing index for {name}"

    # verify string fields are stored as text, not codes
    cur.execute('SELECT DISTINCT activity FROM simlog ORDER BY activity')
    activities = [r[0] for r in cur.fetchall()]
    assert set(activities) == set(ACTIVITIES)

    cur.execute('SELECT DISTINCT entity_type FROM simlog ORDER BY entity_type')
    entity_types = [r[0] for r in cur.fetchall()]
    assert set(entity_types) == set(ENTITY_TYPES)

    con.close()


def test_sqlite_none_fields(log_dir):
    """NULL values in SQLite for missing/None fields."""
    with SimLogSparse(log_dir, mode='w', buffer_size=5_000) as lg:
        lg.log(time=1.0, value=2.0, activity='picking')
        lg.log(time=3.0)

    db_path = log_dir + '.db'
    SimLogSparse(log_dir, mode='r').to_sqlite(db_path)

    con = sqlite3.connect(db_path)
    cur = con.cursor()

    # use ORDER BY rowid to get insertion order (SQLite may use indexes otherwise)
    cur.execute('SELECT value, activity FROM simlog ORDER BY rowid')
    rows = cur.fetchall()
    # row 0: both present
    assert rows[0] == (2.0, 'picking')
    # row 1: only time was provided → value and activity are NULL
    assert rows[1] == (None, None)
    con.close()


def test_sqlite_indexes_queryable(log_dir):
    """Indexes are usable: EXPLAIN QUERY PLAN should mention the index."""
    N = 500
    with SimLogSparse(log_dir, mode='w', buffer_size=5_000) as lg:
        for i in range(N):
            lg.log(**_make_row(i))

    db_path = log_dir + '.db'
    SimLogSparse(log_dir, mode='r').to_sqlite(db_path)

    con = sqlite3.connect(db_path)
    cur = con.cursor()
    cur.execute("EXPLAIN QUERY PLAN SELECT * FROM simlog WHERE time > 0.5")
    plan = ' '.join(str(r) for r in cur.fetchall())
    assert 'idx_simlog_time' in plan
    con.close()


# -- direct execution -------------------------------------------------------

if __name__ == '__main__':
    pytest.main([__file__, '-v'])
