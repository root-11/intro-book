//! A self-contained, dependency-free columnar simulation logger - the §37
//! specimen ([the log is the world](../../book/trunk/37_log_is_world.md)).
//!
//! The shape is the one the chapter teaches:
//!
//! - **Triple-store (COO).** Each `log()` call is one record; only the fields
//!   it provides are stored, as `(rid, key, val)` triples - `rid` is the
//!   record index, `key` is the field's small integer code, `val` is one `f64`.
//! - **Type inference into one `f64` stream.** Integers up to 2^53 round-trip
//!   exactly; floats are stored as-is; strings are interned through an evolving
//!   **codebook** (string -> small integer code) and stored as that code.
//! - **Double-buffered revolver.** Two buffers cycle between the foreground
//!   (filling) and a background thread (flushing). Writing a record is a few
//!   pushes to a `Vec`, never a wait on disk - until the foreground outruns the
//!   writer, when the empty-buffer channel applies backpressure.
//! - **Disk format: raw little-endian column bytes.** No `.npz`, no `serde`.
//!   A chunk file is its triples' bytes; the schema and codebook are one
//!   sidecar (`_meta.bin`). The bytes on disk are the bytes in memory (§36).
//!
//! The logger never interprets a record. It stores triples; the consumer
//! decides what `rid` and `key` mean - which is exactly why the same code is a
//! simulation log, an audit trail, and a replay source.
//!
//! `to_sqlite` from the Python original is intentionally omitted: SQLite is not
//! in Rust's std, and adding it would break this crate's dependency-free
//! property. It is a post-processing export, not part of the live log path; a
//! `rusqlite`-backed converter is the natural extension if a project wants it.

use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::thread::{self, JoinHandle};

/// A value handed to [`SimLog::log`]. Everything lands in one `f64` stream.
#[derive(Clone, Copy, Debug)]
pub enum Value<'a> {
    Int(i64),
    Float(f64),
    Str(&'a str),
}

/// One buffer of the revolver: parallel COO columns plus the record count.
struct Container {
    rids: Vec<u32>,
    keys: Vec<u16>,
    vals: Vec<f64>,
    row_count: u32,
}

impl Container {
    fn new() -> Self {
        Container { rids: Vec::new(), keys: Vec::new(), vals: Vec::new(), row_count: 0 }
    }
    fn reset(&mut self) {
        self.rids.clear();
        self.keys.clear();
        self.vals.clear();
        self.row_count = 0;
    }
}

/// Field code -> recorded type, for decoding on read.
#[derive(Clone, Copy, PartialEq)]
enum KeyType {
    Int,
    Float,
    Str,
}

/// The background writer thread plus the channels that feed it.
struct Revolver {
    full_tx: Sender<Option<Container>>, // foreground -> writer (None = shutdown)
    empty_rx: Receiver<Container>,      // writer -> foreground (recycled, reset)
    handle: Option<JoinHandle<()>>,
}

impl Revolver {
    fn spawn(dir: PathBuf) -> Revolver {
        let (full_tx, full_rx) = channel::<Option<Container>>();
        let (empty_tx, empty_rx) = channel::<Container>();
        // Pre-seed the second buffer so the first swap never blocks.
        empty_tx.send(Container::new()).unwrap();
        let handle = thread::spawn(move || {
            let mut seq: u32 = 0;
            while let Ok(Some(mut c)) = full_rx.recv() {
                write_chunk(&dir, seq, &c);
                seq += 1;
                c.reset();
                if empty_tx.send(c).is_err() {
                    break;
                }
            }
        });
        Revolver { full_tx, empty_rx, handle: Some(handle) }
    }
}

/// The columnar logger. Open with [`SimLog::create`], write with [`SimLog::log`],
/// finish with [`SimLog::close`] (or drop, which closes).
pub struct SimLog {
    dir: PathBuf,
    buffer_size: u32,

    names: Vec<String>,              // key code == index into names
    name_to_key: HashMap<String, u16>,
    key_types: HashMap<u16, KeyType>,
    str_keys: HashSet<u16>,

    str_codes: HashMap<String, u32>, // codebook: string -> code
    str_list: Vec<String>,           // codebook: code -> string

    active: Container,
    revolver: Option<Revolver>,
}

impl SimLog {
    /// Create a writer over an (emptied) directory of chunk files.
    pub fn create<P: AsRef<Path>>(dir: P, buffer_size: u32) -> std::io::Result<SimLog> {
        let dir = dir.as_ref().to_path_buf();
        fs::create_dir_all(&dir)?;
        Ok(SimLog {
            dir: dir.clone(),
            buffer_size,
            names: Vec::new(),
            name_to_key: HashMap::new(),
            key_types: HashMap::new(),
            str_keys: HashSet::new(),
            str_codes: HashMap::new(),
            str_list: Vec::new(),
            active: Container::new(),
            revolver: Some(Revolver::spawn(dir)),
        })
    }

    /// Append one record. Only the fields present in `record` are stored; an
    /// omitted field is simply absent for this row (the read side reports it as
    /// missing). The hot path: discover/lookup each field's code, infer its
    /// type on first sight, intern strings, and push one triple per field.
    pub fn log(&mut self, record: &[(&str, Value)]) {
        let rid = self.active.row_count;
        for &(name, v) in record {
            let k = self.key_for(name);
            let fv = self.encode(k, v);
            self.active.rids.push(rid);
            self.active.keys.push(k);
            self.active.vals.push(fv);
        }
        self.active.row_count = rid + 1;
        if self.active.row_count >= self.buffer_size {
            self.swap();
        }
    }

    fn key_for(&mut self, name: &str) -> u16 {
        if let Some(&k) = self.name_to_key.get(name) {
            return k;
        }
        let k = self.names.len() as u16;
        self.names.push(name.to_string());
        self.name_to_key.insert(name.to_string(), k);
        k
    }

    fn encode(&mut self, k: u16, v: Value) -> f64 {
        match v {
            Value::Str(s) => {
                self.str_keys.insert(k);
                self.key_types.entry(k).or_insert(KeyType::Str);
                self.intern(s) as f64
            }
            Value::Int(i) => {
                self.key_types.entry(k).or_insert(KeyType::Int);
                i as f64
            }
            Value::Float(f) => {
                self.key_types.entry(k).or_insert(KeyType::Float);
                f
            }
        }
    }

    fn intern(&mut self, s: &str) -> u32 {
        if let Some(&code) = self.str_codes.get(s) {
            return code;
        }
        let code = self.str_list.len() as u32;
        self.str_codes.insert(s.to_string(), code);
        self.str_list.push(s.to_string());
        code
    }

    /// Hand the full active buffer to the writer and take a recycled empty one.
    /// `empty_rx.recv()` is the backpressure point: it blocks only if the writer
    /// has not finished the previous flush.
    fn swap(&mut self) {
        let r = self.revolver.as_ref().expect("writer is live");
        let new_active = r.empty_rx.recv().expect("writer alive");
        let full = std::mem::replace(&mut self.active, new_active);
        r.full_tx.send(Some(full)).expect("writer alive");
    }

    /// Flush the tail, stop the writer thread, and write the `_meta.bin` sidecar.
    pub fn close(&mut self) {
        if let Some(r) = self.revolver.take() {
            if self.active.row_count > 0 {
                let full = std::mem::replace(&mut self.active, Container::new());
                r.full_tx.send(Some(full)).expect("writer alive");
            }
            r.full_tx.send(None).expect("writer alive"); // shutdown
            if let Some(h) = r.handle {
                let _ = h.join();
            }
        }
        self.write_meta();
    }

    fn write_meta(&self) {
        let mut b = ByteWriter::new();
        b.u32(self.buffer_size);
        b.u32(self.names.len() as u32);
        for n in &self.names {
            b.string(n);
        }
        b.u32(self.key_types.len() as u32);
        for (&k, &t) in &self.key_types {
            b.u16(k);
            b.u8(match t {
                KeyType::Int => b'i',
                KeyType::Float => b'f',
                KeyType::Str => b's',
            });
        }
        b.u32(self.str_keys.len() as u32);
        for &k in &self.str_keys {
            b.u16(k);
        }
        b.u32(self.str_list.len() as u32);
        for s in &self.str_list {
            b.string(s);
        }
        fs::write(self.dir.join("_meta.bin"), &b.0).expect("write meta");
    }
}

impl Drop for SimLog {
    fn drop(&mut self) {
        if self.revolver.is_some() {
            self.close();
        }
    }
}

/// Raw little-endian column bytes for one chunk: `[n_triples u64][row_count u32]`
/// then the `rids`, `keys`, `vals` blocks back to back.
fn write_chunk(dir: &Path, seq: u32, c: &Container) {
    let n = c.rids.len();
    let mut b = ByteWriter::with_capacity(12 + n * (4 + 2 + 8));
    b.u64(n as u64);
    b.u32(c.row_count);
    for &r in &c.rids {
        b.u32(r);
    }
    for &k in &c.keys {
        b.u16(k);
    }
    for &v in &c.vals {
        b.f64(v);
    }
    fs::write(dir.join(format!("chunk_{seq:06}.bin")), &b.0).expect("write chunk");
}

// ---------------------------------------------------------------------------
// Reader
// ---------------------------------------------------------------------------

/// Reads back a directory of chunk files plus the `_meta.bin` sidecar.
pub struct SimLogReader {
    dir: PathBuf,
    names: Vec<String>,
    key_types: HashMap<u16, KeyType>,
    str_keys: HashSet<u16>,
    str_list: Vec<String>,
}

/// One decoded value from a row: numbers stay numbers, codes become strings.
#[derive(Clone, Debug, PartialEq)]
pub enum Decoded {
    Int(i64),
    Float(f64),
    Str(String),
}

impl SimLogReader {
    pub fn open<P: AsRef<Path>>(dir: P) -> std::io::Result<SimLogReader> {
        let dir = dir.as_ref().to_path_buf();
        let mut r = ByteReader::new(fs::read(dir.join("_meta.bin"))?);
        let _buffer_size = r.u32();
        let n_names = r.u32();
        let names: Vec<String> = (0..n_names).map(|_| r.string()).collect();
        let n_kt = r.u32();
        let mut key_types = HashMap::new();
        for _ in 0..n_kt {
            let k = r.u16();
            let t = match r.u8() {
                b'i' => KeyType::Int,
                b'f' => KeyType::Float,
                _ => KeyType::Str,
            };
            key_types.insert(k, t);
        }
        let n_sk = r.u32();
        let str_keys: HashSet<u16> = (0..n_sk).map(|_| r.u16()).collect();
        let n_str = r.u32();
        let str_list: Vec<String> = (0..n_str).map(|_| r.string()).collect();
        Ok(SimLogReader { dir, names, key_types, str_keys, str_list })
    }

    pub fn field_names(&self) -> &[String] {
        &self.names
    }
    pub fn codebook(&self) -> &[String] {
        &self.str_list
    }

    fn chunk_paths(&self) -> Vec<PathBuf> {
        let mut v: Vec<PathBuf> = fs::read_dir(&self.dir)
            .expect("read dir")
            .filter_map(|e| e.ok().map(|e| e.path()))
            .filter(|p| {
                p.file_name()
                    .and_then(|n| n.to_str())
                    .map(|n| n.starts_with("chunk_") && n.ends_with(".bin"))
                    .unwrap_or(false)
            })
            .collect();
        v.sort();
        v
    }

    /// Reconstruct dense column arrays plus presence masks, field by field.
    /// `cols[name][rid]` is the value; `masks[name][rid]` is whether it was set.
    pub fn to_arrays(&self) -> (HashMap<String, Vec<f64>>, HashMap<String, Vec<bool>>) {
        let mut cols: HashMap<String, Vec<f64>> =
            self.names.iter().map(|n| (n.clone(), Vec::new())).collect();
        let mut masks: HashMap<String, Vec<bool>> =
            self.names.iter().map(|n| (n.clone(), Vec::new())).collect();

        if self.names.is_empty() {
            return (cols, masks);
        }
        for p in self.chunk_paths() {
            let (row_count, rids, keys, vals) = read_chunk(&p);
            let rc = row_count as usize;
            // Columns grow in lockstep, so the base offset for this chunk is the
            // current length of any one of them.
            let base = cols[&self.names[0]].len();
            for name in &self.names {
                cols.get_mut(name).unwrap().resize(base + rc, 0.0);
                masks.get_mut(name).unwrap().resize(base + rc, false);
            }
            for j in 0..rids.len() {
                let name = &self.names[keys[j] as usize];
                let idx = base + rids[j] as usize;
                cols.get_mut(name).unwrap()[idx] = vals[j];
                masks.get_mut(name).unwrap()[idx] = true;
            }
        }
        (cols, masks)
    }

    /// Iterate decoded rows in order: strings decoded through the codebook,
    /// ints/floats coerced to their recorded type, missing fields absent.
    pub fn rows(&self) -> Vec<HashMap<String, Decoded>> {
        let mut out: Vec<HashMap<String, Decoded>> = Vec::new();
        for p in self.chunk_paths() {
            let (row_count, rids, keys, vals) = read_chunk(&p);
            let base = out.len();
            out.resize(base + row_count as usize, HashMap::new());
            for j in 0..rids.len() {
                let k = keys[j];
                let v = vals[j];
                let name = self.names[k as usize].clone();
                let decoded = if self.str_keys.contains(&k) {
                    Decoded::Str(self.str_list[v as usize].clone())
                } else if self.key_types.get(&k) == Some(&KeyType::Int) {
                    Decoded::Int(v as i64)
                } else {
                    Decoded::Float(v)
                };
                out[base + rids[j] as usize].insert(name, decoded);
            }
        }
        out
    }

    /// Export to CSV (one column per field, blank for missing). Post-processing,
    /// std-only - the dependency-free half of the Python original's exports.
    pub fn to_csv<P: AsRef<Path>>(&self, out: P) -> std::io::Result<()> {
        let mut s = String::new();
        s.push_str(&self.names.join(","));
        s.push('\n');
        for row in self.rows() {
            let parts: Vec<String> = self
                .names
                .iter()
                .map(|n| match row.get(n) {
                    None => String::new(),
                    Some(Decoded::Int(i)) => i.to_string(),
                    Some(Decoded::Float(f)) => f.to_string(),
                    Some(Decoded::Str(st)) => st.clone(),
                })
                .collect();
            s.push_str(&parts.join(","));
            s.push('\n');
        }
        fs::write(out, s)
    }
}

fn read_chunk(p: &Path) -> (u32, Vec<u32>, Vec<u16>, Vec<f64>) {
    let mut r = ByteReader::new(fs::read(p).expect("read chunk"));
    let n = r.u64() as usize;
    let row_count = r.u32();
    let rids: Vec<u32> = (0..n).map(|_| r.u32()).collect();
    let keys: Vec<u16> = (0..n).map(|_| r.u16()).collect();
    let vals: Vec<f64> = (0..n).map(|_| r.f64()).collect();
    (row_count, rids, keys, vals)
}

// ---------------------------------------------------------------------------
// Minimal little-endian byte writer / reader (no serde, no bincode)
// ---------------------------------------------------------------------------

struct ByteWriter(Vec<u8>);
impl ByteWriter {
    fn new() -> Self {
        ByteWriter(Vec::new())
    }
    fn with_capacity(n: usize) -> Self {
        ByteWriter(Vec::with_capacity(n))
    }
    fn u8(&mut self, v: u8) {
        self.0.push(v);
    }
    fn u16(&mut self, v: u16) {
        self.0.extend_from_slice(&v.to_le_bytes());
    }
    fn u32(&mut self, v: u32) {
        self.0.extend_from_slice(&v.to_le_bytes());
    }
    fn u64(&mut self, v: u64) {
        self.0.extend_from_slice(&v.to_le_bytes());
    }
    fn f64(&mut self, v: f64) {
        self.0.extend_from_slice(&v.to_le_bytes());
    }
    fn string(&mut self, s: &str) {
        self.u32(s.len() as u32);
        self.0.extend_from_slice(s.as_bytes());
    }
}

struct ByteReader {
    buf: Vec<u8>,
    pos: usize,
}
impl ByteReader {
    fn new(buf: Vec<u8>) -> Self {
        ByteReader { buf, pos: 0 }
    }
    fn take(&mut self, n: usize) -> &[u8] {
        let start = self.pos;
        self.pos += n;
        &self.buf[start..start + n]
    }
    fn u8(&mut self) -> u8 {
        let v = self.buf[self.pos];
        self.pos += 1;
        v
    }
    fn u16(&mut self) -> u16 {
        u16::from_le_bytes(self.take(2).try_into().unwrap())
    }
    fn u32(&mut self) -> u32 {
        u32::from_le_bytes(self.take(4).try_into().unwrap())
    }
    fn u64(&mut self) -> u64 {
        u64::from_le_bytes(self.take(8).try_into().unwrap())
    }
    fn f64(&mut self) -> f64 {
        f64::from_le_bytes(self.take(8).try_into().unwrap())
    }
    fn string(&mut self) -> String {
        let n = self.u32() as usize;
        String::from_utf8(self.take(n).to_vec()).unwrap()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    const ACTIVITIES: [&str; 4] = ["picking", "putaway", "replen", "count"];

    fn tmpdir(tag: &str) -> PathBuf {
        let mut p = std::env::temp_dir();
        p.push(format!("logger_test_{tag}_{}", std::process::id()));
        let _ = fs::remove_dir_all(&p);
        p
    }

    fn write_n(dir: &Path, n: u32, buffer_size: u32) {
        let mut lg = SimLog::create(dir, buffer_size).unwrap();
        for i in 0..n {
            lg.log(&[
                ("time", Value::Float(i as f64 * 0.001)),
                ("value", Value::Float(i as f64 * 1.23)),
                ("activity", Value::Str(ACTIVITIES[(i as usize) % ACTIVITIES.len()])),
                ("entity_id", Value::Int((i % 10_000) as i64)),
            ]);
        }
        lg.close();
    }

    #[test]
    fn roundtrip_to_arrays() {
        let dir = tmpdir("roundtrip");
        let n = 10_000u32;
        write_n(&dir, n, 5_000); // forces 2 buffer swaps -> exercises the revolver
        let r = SimLogReader::open(&dir).unwrap();
        let (cols, masks) = r.to_arrays();
        assert_eq!(cols["time"].len(), n as usize);
        for idx in [0usize, 1, 5_000, 9_999] {
            assert!((cols["time"][idx] - idx as f64 * 0.001).abs() < 1e-9);
            assert!((cols["value"][idx] - idx as f64 * 1.23).abs() < 1e-9);
            assert_eq!(cols["entity_id"][idx], (idx as u32 % 10_000) as f64);
            assert!(masks["time"][idx]);
        }
        let cb = r.codebook();
        for a in ACTIVITIES {
            assert!(cb.contains(&a.to_string()), "codebook missing {a}");
        }
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn sparse_rows_and_missing() {
        let dir = tmpdir("sparse");
        {
            let mut lg = SimLog::create(&dir, 5_000).unwrap();
            lg.log(&[
                ("time", Value::Float(1.0)),
                ("value", Value::Float(2.0)),
                ("activity", Value::Str("picking")),
            ]);
            lg.log(&[("time", Value::Float(2.0))]); // sparse: only one field
            lg.close();
        }
        let r = SimLogReader::open(&dir).unwrap();
        let rows = r.rows();
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0]["activity"], Decoded::Str("picking".into()));
        assert_eq!(rows[0]["value"], Decoded::Float(2.0));
        assert!(rows[1].get("value").is_none());
        assert!(rows[1].get("activity").is_none());
        assert_eq!(rows[1]["time"], Decoded::Float(2.0));
        let (_cols, masks) = r.to_arrays();
        assert!(!masks["value"][1]);
        assert!(masks["time"][1]);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn type_coercion_on_read() {
        let dir = tmpdir("coerce");
        write_n(&dir, 1, 5_000);
        let rows = SimLogReader::open(&dir).unwrap().rows();
        assert!(matches!(rows[0]["time"], Decoded::Float(_)));
        assert!(matches!(rows[0]["entity_id"], Decoded::Int(_)));
        assert_eq!(rows[0]["activity"], Decoded::Str("picking".into()));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn csv_export() {
        let dir = tmpdir("csv");
        write_n(&dir, 500, 5_000);
        let csv = dir.with_extension("csv");
        SimLogReader::open(&dir).unwrap().to_csv(&csv).unwrap();
        let text = fs::read_to_string(&csv).unwrap();
        let lines: Vec<&str> = text.lines().collect();
        assert_eq!(lines.len(), 501); // header + 500
        assert!(lines[0].contains("time") && lines[0].contains("activity"));
        let _ = fs::remove_dir_all(&dir);
        let _ = fs::remove_file(&csv);
    }
}
