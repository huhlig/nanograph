# Embeddable Multi-Model Database in Rust
## Architecture & Implementation Plan (v4)

---

# 1. Overview

This document describes the architecture for a small embeddable database library in Rust inspired by SQLite, using an LSM-backed key-value store as the core storage engine. It supports:

- Multi-table storage
- Key-value semantics
- Document storage (strong & weak typing)
- Graph data model
- Secondary indexing
- Single or multi-file persistence

The design prioritizes:
- Composability over specialization
- A unified KV foundation
- Explicit logical layers over physical complexity

> **Guiding principle:** A database is not multiple storage systems, but one ordered keyspace with multiple interpretations.

**Scope note:** This is an *embeddable library*, like SQLite or sled — linked into a host process, not a separate server. Server concerns (wire protocols, network auth, replication, sharding) are explicitly out of scope. They can be built on top by a host application.

---

# 2. High-Level Architecture

```
+---------------------------+
|        Query Layer        |
+---------------------------+
|      Logical Models       |
|  - Tables / Documents    |
|  - Graph                 |
+---------------------------+
|     Indexing Engine      |
+---------------------------+
|     Catalog System       |
+---------------------------+
|   Transaction Manager    |
+---------------------------+
|   LSM KV Storage Core    |
+---------------------------+
|      File System         |
+---------------------------+
```

The **Transaction Manager** is called out explicitly because every layer above it depends on its semantics. Indexing, catalog updates, and multi-row writes all need atomic batches and snapshot reads to be correct.

---

# 3. Build vs. Buy

Before designing the storage core, a deliberate choice: **why are we writing this and not using `sled`, `redb`, or `rocksdb` (via bindings)?**

Reasonable answers include:

- Pedagogical / engineering exercise
- Need single-file portability that current Rust LSMs don't offer
- Want a license, footprint, or feature set unavailable elsewhere
- Want a layered multi-model design no existing engine offers natively

If none of those apply, building on top of `sled` or `redb` and skipping straight to Phase 2 (catalog and tables) gets a usable system in a fraction of the time. **The rest of this document assumes we are deliberately building the storage core.**

Reference implementations worth reading regardless of what we choose:
- `sled`, `redb` — Rust idioms in embedded KV
- `toydb` (Erik Grinaker) — pedagogical Rust SQL DB with MVCC
- `GraniteDB` (Kritarth Agrawal) — page-based document DB, useful as a contrast to the LSM approach taken here

---

# 4. Consistency and Concurrency Model

This section pins down semantics that everything else depends on. It must be settled before storage details.

## 4.1 Chosen model: MVCC with snapshot isolation

Rationale:
- Readers never block writers; writers never block readers.
- A natural fit for LSM, where multiple versions per key already exist on disk.
- Sufficient for the embedded use case without paying for full serializability.

Higher isolation levels (Repeatable Read, Serializable) can be layered on later via predicate locking or SSI. Lower levels (Read Committed, Read Uncommitted) are not useful here — snapshot isolation already gives us what RC offers without the downsides.

## 4.2 Versioning

Every logical key is encoded with a monotonic sequence number:

```
physical_key = user_key || seq_no_be
```

`seq_no` is a 64-bit big-endian integer assigned at commit time by the transaction manager. On read, the iterator skips entries with `seq_no > snapshot_seq` and returns the highest-versioned entry per `user_key`.

Tombstones (deletes) are also versioned and only physically removed during compaction, once no live snapshot can observe them. The mechanism for tracking this is in §4.5.

## 4.3 Transactions

```rust
trait Database {
    fn begin_read(&self) -> ReadTxn;
    fn begin_write(&self) -> WriteTxn;
}
```

- **Read transactions** hold a snapshot sequence number. They are cheap and freely concurrent.
- **Write transactions** are single-writer for v1 (one in flight at a time, others queue). This is the SQLite WAL model and is sufficient for an embedded DB. Optimistic concurrency with conflict detection at commit time can be added later without changing the on-disk format.

Write transactions must be the **only** path through which mutations reach the KV core. Index maintenance, catalog updates, and row writes happen inside the same transaction, atomically.

## 4.4 Durability

`commit()` accepts a `Durability` flag:

- `Sync` — fsync the WAL before returning (default).
- `Buffered` — return after WAL append; group-commit fsync on a timer.
- `None` — no durability; useful for tests and bulk loads.

## 4.5 GC watermark and long-running readers

A subtle correctness/operational issue: tombstones and superseded versions can only be physically removed when no live reader could observe them. Concretely:

- The transaction manager maintains `min_active_snapshot_seq`, the minimum `seq_no` of any live read transaction.
- Compaction removes tombstones and superseded versions only when their `seq_no < min_active_snapshot_seq`.
- A read transaction that lives forever pins the watermark and prevents *all* compaction GC from making progress. SSTable size and disk usage grow unbounded.

Mitigation:

- **Read transactions have a configurable deadline** (default 5 minutes). Past it, the snapshot is dropped and reads against it fail with `SnapshotExpired`. Hosts that need long-running reads must opt in explicitly.
- The watermark is exposed via a metric so operators can detect a reader pinning it.

This is the kind of bug you only find after running in anger. Designing the watermark in from the start is much cheaper than retrofitting it.

## 4.6 Iterator semantics

Iterators are bound to a snapshot. SSTable deletion is deferred (via reference counts on the manifest) until no iterator holds a snapshot referencing the deleted file. The manifest tracks these references; retrofitting this later is painful and error-prone.

---

# 5. Storage Engine (LSM KV Core)

## 5.1 Core API

```rust
pub trait KvStore: Send + Sync {
    type Snapshot: Snapshot;

    fn snapshot(&self) -> Self::Snapshot;
    fn write(&self, batch: WriteBatch) -> Result<SeqNo>;
}

pub trait Snapshot: Send + Sync {
    fn get(&self, key: &[u8]) -> Result<Option<Vec<u8>>>;
    fn scan<'a>(&'a self, range: Range<&[u8]>)
        -> Result<Box<dyn Iterator<Item = Result<(Vec<u8>, Vec<u8>)>> + 'a>>;
    fn scan_prefix<'a>(&'a self, prefix: &[u8])
        -> Result<Box<dyn Iterator<Item = Result<(Vec<u8>, Vec<u8>)>> + 'a>>;
}

pub struct WriteBatch {
    ops: Vec<WriteOp>,
}

pub enum WriteOp {
    Put { key: Vec<u8>, value: Vec<u8> },
    Delete { key: Vec<u8> },
}
```

Notes:

- All fallible ops return `Result`; I/O can fail.
- `&self` everywhere — interior mutability inside the engine. The store is `Sync` and shareable.
- Iterators are lifetime-bound to a snapshot.
- No bare `put` / `delete`. All mutations go through `write(batch)` so atomicity of multi-key updates (row + indexes + catalog) is guaranteed.

## 5.2 WAL format

The WAL is **segmented**, with rollover at a configurable size (default 64 MB):

```
wal/
  00000001.log   (closed, replayed-from-checkpoint)
  00000002.log   (closed)
  00000003.log   (active)
```

Each entry has the structure:

```
[ LSN: u64 ] [ length: u32 ] [ CRC32: u32 ] [ payload ]
```

- **LSN** — log sequence number, monotonic across all segments. Recovery resumes from `last_checkpoint_lsn`.
- **CRC32** — covers length + payload. Mismatches stop replay; the partial entry is treated as torn write.
- **Length** — explicit length lets recovery validate framing without parsing the payload.

The payload is a serialized `WriteBatch` plus its assigned `seq_no`.

**Recovery algorithm:**

1. Read `MANIFEST` and `CURRENT` to identify SSTables and `last_checkpoint_lsn`.
2. Open each WAL segment with `lsn >= last_checkpoint_lsn` in order.
3. For each entry: validate CRC. If valid, apply to memtable. If invalid, stop — torn write at end of log is normal and expected.
4. Truncate the segment at the last good entry.

**Checkpoints** are taken when the memtable flushes to L0. The checkpoint records `(flushed_lsn, sstable_id)` in the manifest. WAL segments entirely below `flushed_lsn` are eligible for deletion after a grace period.

## 5.3 Memtable and SSTables

- **Memtable**: skiplist or B-tree map, keyed by `(user_key, seq_no)`.
- **SSTables**: sorted, immutable. Each SSTable is structured as data blocks + index block + bloom filter + footer. Every block has its own CRC32 — block-level checksums catch bit rot and partial reads, and let us fail a single read instead of an entire SSTable.
- **Bloom filters per SSTable** — required, not optional. Without them, point reads degrade catastrophically as levels grow.
- **Block compression**: LZ4 by default; Zstd as an option for cold data.
- **Block cache**: LRU over decompressed blocks. Pinned blocks (referenced by an in-flight iterator) are exempt from eviction. Configurable size; defaults proportional to memtable size.
- **Manifest**: append-only log of "SSTable N added/removed at level L, references snapshot seq S." Recovery replays the manifest, then the WAL.
- **Compaction**: leveled compaction (RocksDB-style) by default. Tombstones are dropped per the watermark in §4.5.

## 5.3.1 Large values

LSM has a write-amplification problem with large values: every compaction rewrites the value, even if the key is untouched. Storing a 10 MB document inline means rewriting 10 MB on every compaction pass through that level.

**Solution: value separation, threshold-based.**

- Values smaller than a threshold (default 4 KB) are stored inline in SSTables.
- Values above the threshold are stored in a separate **blob log** (append-only file). The SSTable entry contains `(blob_file_id, offset, length)` instead of the value.
- Compaction of SSTables doesn't touch blob storage. Blob GC happens separately, walking SSTable indexes to find live blob references.

This is the WiscKey approach (Lu et al., FAST '16). It avoids the overflow-page complexity of page-based engines while keeping write amplification bounded.

## 5.4 Inspiration

- RocksDB, LevelDB (LSM mechanics)
- WiscKey (value separation)
- sled, redb (Rust idioms)
- FoundationDB (layered architecture, tuple encoding)

---

# 6. Key Encoding

The KV store sees only byte slices, but the **lexicographic byte order of keys must match the logical order of the data**. This is non-negotiable for range scans and ordered indexes to work.

## 6.1 Tuple encoding

Keys are encoded tuples. The encoding is order-preserving for arbitrary tuples of typed components — borrowing directly from the FoundationDB tuple layer.

| Type | Encoding |
|---|---|
| `u64` | 8-byte big-endian |
| `i64` | 8-byte big-endian with sign bit flipped (so negatives sort before positives) |
| `f64` | IEEE 754 with sign-flip for positives, full bitwise inversion for negatives |
| `&str` | length-prefixed UTF-8 with a fixed terminator byte and escape rule, *or* fixed-width null-terminated with `0x00` escaped to `0x00 0xFF` |
| `&[u8]` | same escaping as strings |
| tuple | concatenation of above with a separator byte |

A small Rust API:

```rust
pub struct KeyBuilder { buf: Vec<u8> }

impl KeyBuilder {
    pub fn ns(self, ns: NamespaceTag) -> Self { ... }
    pub fn u32(self, v: u32) -> Self { ... }
    pub fn u64(self, v: u64) -> Self { ... }
    pub fn str(self, s: &str) -> Self { ... }
    pub fn bytes(self, b: &[u8]) -> Self { ... }
    pub fn finish(self) -> Vec<u8> { self.buf }
}
```

## 6.2 Namespace tags

A leading byte distinguishes top-level keyspaces. Reserved values:

```
0x01  catalog
0x02  table data and indexes
0x03  WAL bookkeeping (internal)
0x04  blob references (if value separation is in use)
0x05  free for future use
```

All physical keys end with the MVCC sequence number (8 bytes BE), which is appended by the transaction manager — *not* by user-facing key builders.

---

# 7. Catalog System

The catalog defines all logical structures and lives in keyspace `0x01`.

## 7.1 Layout

```
/catalog/tables/{table_id}            -> TableDef (encoded)
/catalog/tables_by_name/{name}        -> table_id
/catalog/indexes/{table_id}/{idx_id}  -> IndexDef
/catalog/graphs/{graph_id}            -> GraphDef
/catalog/seq                          -> next available table_id, index_id, etc.
```

## 7.2 Bootstrap

The catalog itself lives in KV, but reading it requires opening the KV store first. To avoid circularity:

- `table_id = 0` is reserved for the catalog table. Its schema is hardcoded in the binary.
- On open: replay manifest + WAL → bring the KV core online → read `table_id = 0` to discover all other tables.
- The catalog table is itself indexed by name via the same machinery used for user tables, but only after bootstrap completes.

## 7.3 Definitions

```rust
pub struct TableDef {
    pub id: u32,
    pub name: String,
    pub kind: TableKind,                // Relational | Document
    pub schema: Option<Schema>,         // None for Document tables
    pub indexes: Vec<IndexDef>,
    pub schema_version: u32,            // bumps on schema migration
}

pub struct IndexDef {
    pub id: u32,
    pub name: String,
    pub fields: Vec<FieldPath>,         // supports dotted paths into Value
    pub unique: bool,
    pub sparse: bool,                   // skip rows where indexed field is missing
}
```

---

# 8. Data Model: Unified Tables

Tables and document collections are the **same thing**. A document collection is a table with `kind = Document` and no fixed schema. They share:

- One catalog entry type
- One key prefix scheme
- One indexing path
- One serialization format

Tables differ only in whether the schema is enforced at write time and whether rows are projected into typed columns or stored as a `Value`.

## 8.1 Value type

```rust
pub enum Value {
    Null,
    Bool(bool),
    Int(i64),
    Float(f64),
    String(String),
    Bytes(Vec<u8>),
    Array(Vec<Value>),
    Map(BTreeMap<String, Value>),
}
```

## 8.2 Row storage

Serialization format: **CBOR** (RFC 8949). Reasons: deterministic, self-describing, supports binary, has good Rust libraries (`ciborium`), and is forward/backward compatible — important for schema evolution.

```rust
pub struct Row {
    pub schema_version: u32,
    pub values: Value,   // Map for relational rows, anything for documents
}
```

Indexed fields are extracted from `Value` at write time using `FieldPath` (dotted-path lookup) and written as separate index entries inside the same WriteBatch.

## 8.3 Schema evolution

- Each row carries the `schema_version` it was written under.
- Adding a column: new schema version, old rows read with missing fields defaulted to `Null` (or the schema's declared default).
- Removing a column: marked as deprecated in the new schema; old data ignored on read.
- Type changes: not supported in-place. Require a migration that rewrites rows under a new schema version.

---

# 9. Keyspace Layout

All under namespace `0x02` (table data):

```
/{table_id}/row/{primary_key}
/{table_id}/idx/{index_id}/{indexed_value}/{primary_key}
```

For graphs (which are just two tables, see §11), edge indexes are arranged for traversal-friendly prefix scans:

```
/{edge_table}/idx/by_from/{from_id}/{label}/{edge_id}
/{edge_table}/idx/by_to/{to_id}/{label}/{edge_id}
/{edge_table}/idx/by_label/{label}/{edge_id}
```

This means "all out-edges of node X with label Y" is a single prefix scan, cost proportional to matching edges.

---

# 10. Indexing System

## 10.1 Mechanics

Indexes are first-class KV entries, written and deleted **in the same WriteBatch** as the row they describe. This is the whole reason §4 mandated transactional batches.

For a row insert:

1. Encode primary key.
2. Read existing row at PK (within the txn) to compute index deltas if updating.
3. Build WriteBatch:
   - Put `/{table}/row/{pk}` = serialized row
   - For each index: Put `/{table}/idx/{idx}/{value}/{pk}` = empty (the PK is in the key)
   - For each index entry no longer matching (on update): Delete the old entry
4. Commit.

> **Index maintenance is a correctness problem, not a performance problem.** It's tempting to think of indexes as an optimization layer added later. Once writes are concurrent, ensuring indexes stay consistent with the underlying data under partial failures is a correctness requirement. Hence: indexes live in the same WriteBatch as the row, full stop.

## 10.2 Index types (v1)

- **Primary** — implicit, the row store itself.
- **Secondary B-tree-style** — single or composite columns; ordered, supports range scans. Implemented as ordered KV prefix scans.
- **Unique** — enforced at write time by reading the index prefix inside the txn.
- **Sparse** — skip rows where the indexed field is missing (default for nullable fields).

## 10.3 Future index types

- **Inverted indexes** for full-text. Tokens become index keys; values are PKs.
- **Hash indexes** for exact-match-only fields. Marginal benefit over ordered indexes in an LSM (since point lookups already use bloom filters), so deferred.

## 10.4 Field extraction

```rust
pub enum FieldPath {
    Root,
    Field(String, Box<FieldPath>),    // "user.address.city"
    Index(usize, Box<FieldPath>),     // arrays
}
```

Missing fields produce no index entry (sparse by default).

---

# 11. Graph Model

Graphs remain a **composition**, not a primitive:

```rust
pub struct GraphDef {
    pub id: u32,
    pub node_table: u32,
    pub edge_table: u32,
}
```

The edge table has, by convention, columns `from: u64`, `to: u64`, `label: String`, plus user-defined properties. It carries the three indexes shown in §9. This gives:

- O(out-degree) traversal of a node's outgoing edges
- O(in-degree) traversal of incoming edges
- O(matching) lookup of edges by label
- Multiple graphs in one DB at no extra cost

## 11.1 Traversal API

```rust
let g = db.graph("social");
let txn = db.begin_read();

for edge in g.out_edges(&txn, node_id, Some("FOLLOWS"))? {
    // prefix scan: /{edges}/idx/by_from/{node_id}/FOLLOWS/...
}
```

A multi-hop traversal is just nested prefix scans inside one read snapshot. BFS/DFS helpers can be built on top.

---

# 12. File Layout

**Multi-file only for v1.** Single-file is dropped.

```
/wal/
  {lsn_segment}.log
/sst/
  sst_{level}_{id}.db
/blob/
  blob_{id}.dat
/MANIFEST-{N}
/CURRENT
```

Single-file (SQLite-style) is much harder for an LSM than a B-tree because SSTables vary in size and number. If single-file portability becomes a real requirement later, the right move is probably a different storage backend (B-tree variant) rather than squeezing LSM into one file.

---

# 13. Public API

```rust
let db = Database::open("data.db", Options::default())?;

// Read
{
    let txn = db.begin_read();
    let users = db.table("users");
    let row = users.get(&txn, &user_id)?;
    for (k, v) in users.scan_prefix(&txn, &"alice")? { ... }
}

// Write
{
    let mut txn = db.begin_write();
    let users = db.table("users");
    users.insert(&mut txn, &user_id, &row)?;
    txn.commit(Durability::Sync)?;
}

// Graph
let g = db.graph("social");
let mut txn = db.begin_write();
g.add_edge(&mut txn, alice_id, bob_id, "FOLLOWS")?;
txn.commit(Durability::Sync)?;
```

Every operation takes a transaction. There is no implicit autocommit — it makes index maintenance footguns harder to introduce.

## 13.1 Query introspection (EXPLAIN)

Once the query layer (Phase 6) lands, it must expose plan introspection from the start:

```rust
let plan = users.find(&txn, &filter).explain()?;
// PrimaryKeyLookup { key: ... }
// IndexScan { index: "by_email", range: ..., est_rows: 12 }
// FullScan { est_rows: 1_000_000 }   <-- the one you want to see
```

> **The query planner is where the database's personality lives.** Even a simple "index scan vs. full scan" choice has edge cases — an index scan with poor selectivity can be slower than a full scan because of random I/O. EXPLAIN lets users diagnose this without reading source.

---

# 14. Observability

Cheap, always-on metrics. Reading from these costs nothing on the hot path.

- **Atomic counters** (`AtomicU64`) for: writes committed, reads served, WAL bytes written, SSTable bytes read/written, bloom filter hits/misses, block cache hits/misses, compactions run, transactions aborted, snapshots active.
- **Atomic gauges** for: `min_active_snapshot_seq`, current memtable size, level sizes, blob log size.
- A `Stats::snapshot() -> StatsSnapshot` method returns a coherent struct of all counters at one moment.

Mutexes for counters on hot paths would be measurable overhead. Atomics are correct here because we don't need happens-before across threads — just visibility, which `Relaxed` ordering gives us.

A long-running reader pinning the GC watermark is the kind of bug that's only visible through metrics. `min_active_snapshot_seq` standing still while writes accumulate is the symptom.

---

# 15. Implementation Phases

## Phase 1: Storage core
- Segmented WAL with LSN + CRC32 + crash recovery (must include torn-write handling)
- Memtable
- SSTable writer/reader with bloom filters, block compression, block-level CRC
- Manifest and SSTable lifecycle (with snapshot-aware deletion)
- Leveled compaction
- **Property tests** for the merge iterator
- **Crash injection tests** for WAL recovery

## Phase 2: Transactions
- WriteBatch and snapshot machinery
- Single-writer write txns
- GC watermark tracking
- Read transaction deadlines
- Durability flags
- **Concurrency tests** (loom or shuttle)

## Phase 3: Catalog and tables
- Tuple key encoder
- Catalog bootstrap
- Table abstraction over KV
- CBOR row serialization
- Schema-version-aware reads

## Phase 4: Indexes and value separation
- Secondary indexes with field-path extraction
- Unique constraint enforcement
- Index maintenance inside WriteBatch
- Blob log for large values, with reference-walking GC

## Phase 5: Graph
- Edge table conventions and indexes
- Traversal helpers (out_edges, in_edges, by_label)
- BFS/DFS iterators

## Phase 6 (optional): Query layer
- **Decision required up front:** fluent Rust DSL? SQL subset? Datalog (good fit for graph)?
- This decision affects which indexes we want in Phase 4.
- **EXPLAIN must ship with the first executable query** — retrofitting plan introspection is painful.

---

# 16. Testing Strategy

LSM bugs are silent in normal use and catastrophic in production. Testing is not optional.

- **Unit tests** per component (WAL, SSTable, memtable, encoder).
- **Property tests** (`proptest`) for: tuple encoding round-trip and order preservation; merge iterator output equals a reference `BTreeMap`; index maintenance invariants (every row's indexes are consistent after arbitrary write sequences).
- **Crash injection**: pause writes mid-fsync, kill, reopen, verify durability matches the durability flag. CRC validation on torn writes.
- **Concurrency**: `loom` for the transaction manager and snapshot ref-counting.
- **Long-running reader test**: spawn a read txn, do millions of writes, verify GC waits and that the deadline eventually fires.
- **Fuzzing** the SSTable and WAL readers against malformed inputs.

---

# 17. Lessons That Bite You

These are gotchas worth internalizing before writing code, drawn from the experience of others who've built similar engines:

- **WAL replay is where crash recovery actually lives.** The concept is simple. The detail that bites is ensuring WAL writes are *truly* durable before acknowledging the commit. fsync semantics, buffered I/O, and write ordering matter in ways easy to handwave in architecture diagrams.
- **MVCC GC is harder than MVCC reads.** Knowing when a version is safe to delete requires knowing the minimum snapshot held by any active reader. Get that wrong and you either leak unbounded disk or break long-running reads. (See §4.5.)
- **Index maintenance is a correctness problem, not a performance problem.** Discussed in §10.1. Once writes are concurrent, indexes drifting from data is a silent corruption bug.
- **The query planner is where the database's personality lives.** Same query, different plan, 1000× speed difference. Ship EXPLAIN from day one.
- **Long-running readers are an operational hazard.** Without deadlines, a forgotten read transaction silently disables compaction GC. (See §4.5.)
- **Rust eliminates data races, not concurrency bugs.** The borrow checker won't catch deadlocks in the transaction manager, off-by-one in seq_no comparisons, or race conditions in the manifest. It just makes them tractable rather than Heisenbugs.

---

# 18. Key Design Principles

- Everything is KV underneath.
- Tables are logical partitions, not storage units.
- Document collections are tables with no fixed schema — same code path.
- Graphs are compositions, not primitives.
- Indexes are first-class KV entries, maintained transactionally.
- All mutations go through WriteBatch; there is no non-transactional path.
- Lexicographic byte order is the contract that everything above the KV core relies on.
- Large values are separated from the LSM; small values stay inline.
- Observability is a first-class concern, not an afterthought.

---

# 19. Summary

This design prioritizes:

- A small, focused storage core with well-defined consistency
- A unified data model (tables ≈ collections; graphs = two tables)
- Composability of indexes, transactions, and models
- Long-term extensibility through a stable key encoding

Out of scope, by design: wire protocols, network auth, replication, sharding, encryption-at-rest. Those are server-layer concerns and belong in a project built on top of this library, not in the library itself.

The unchanged north star:

> A database is not multiple storage systems, but one ordered keyspace with multiple interpretations.

---

# Appendix A: Changes from v2

| Area | v2 | v3 |
|---|---|---|
| WAL format | "append-only, fsync" | Segmented, LSN + CRC32 per entry, explicit recovery algorithm (§5.2) |
| SSTables | Bloom filters mentioned | Block-level CRCs added; large-value separation via blob log (§5.3.1) |
| MVCC GC | "no live snapshot can observe" | Explicit `min_active_snapshot_seq` watermark, exposed as a metric (§4.5) |
| Long-running readers | Not addressed | Read transaction deadlines, default 5 min (§4.5) |
| Observability | Implied via tests | Dedicated section with atomic counters/gauges (§14) |
| Query introspection | Phase 6 mention | EXPLAIN ships with first executable query (§13.1) |
| Lessons / gotchas | Spread through doc | Consolidated section §17 |
| Scope | Implicit | Explicit "embeddable library, not server" boundary (§1) |
| Reference implementations | sled, redb mentioned | Added toydb, GraniteDB as contrasting designs (§3) |

**v3 → v4:** Appendix B expanded from a GraniteDB-only table into a comprehensive notes section covering every idea considered and rejected or deferred across all three reviews, with category grouping and revisit conditions for each.

# Appendix B: Notes on Ideas Considered and Not Adopted

This section consolidates design choices where alternatives were considered and rejected or deferred, with reasoning. The intent is to preserve the *why* so future contributors don't reopen settled debates without new information — and so they have the right starting point if they do want to revisit.

## B.1 Storage architecture

### B-tree / page-based storage with buffer pool
**What:** Fixed-size pages on disk, an LRU buffer pool managing in-memory page frames with pin/unpin lifecycle, B-tree or B+tree as the index structure. The classic SQLite / Postgres / GraniteDB approach.
**Why someone wants it:** Mature, well-understood, low write amplification on small updates, natural fit for single-file format.
**Why we didn't adopt:** We chose LSM in §1, which is a fundamentally different architecture. LSM gives better write throughput and simpler crash recovery (immutable SSTables) at the cost of write amplification during compaction. Switching would be a complete redesign of the storage core, not an incremental change.
**To revisit:** Would require restarting at Phase 1. The layers above the KV core (catalog, indexes, models) would mostly survive.

### Single-file format (SQLite-style)
**What:** All data, WAL, and metadata packed into one file via an internal page allocator and free-list.
**Why someone wants it:** Trivial deployment — one file to copy, email, version-control. SQLite's killer feature.
**Why we didn't adopt:** Single-file is much harder for an LSM than a B-tree. SSTables vary in size and number; squeezing them into one file requires inventing a small filesystem (page allocator, free-list, fragmentation handling) on top. The complexity isn't worth it for v1.
**To revisit:** If portability becomes a hard requirement, the right move is probably a B-tree backend (see above) rather than retrofitting LSM.

## B.2 Concurrency and isolation

### Multi-writer / optimistic concurrency control in v1
**What:** Multiple write transactions in flight simultaneously, with conflict detection at commit time.
**Why someone wants it:** Higher write throughput on multi-core systems, especially for workloads with mostly non-conflicting writes.
**Why we didn't adopt:** Single-writer is the SQLite WAL model and is sufficient for an embedded DB. OCC adds significant complexity (read sets, write sets, conflict detection, retry policies) for benefit that's only realized in specific high-contention workloads.
**To revisit:** The on-disk format does not need to change — sequence numbers and snapshots already support this. Adding OCC later is purely a transaction-manager change.

### Full serializability / SSI
**What:** Serializable Snapshot Isolation prevents write skew and other anomalies that snapshot isolation allows.
**Why someone wants it:** Strongest isolation level, prevents subtle correctness bugs in concurrent application logic.
**Why we didn't adopt:** Snapshot isolation is sufficient for the embedded use case and is dramatically simpler. SSI requires tracking read-write dependencies between transactions, which is a significant addition.
**To revisit:** Layerable on top of MVCC + OCC. Would require predicate locking or SSI-style dependency tracking.

### Read Uncommitted, Read Committed, Repeatable Read as configurable levels
**What:** Multiple isolation levels selectable per transaction, as offered by GraniteDB and most SQL databases.
**Why someone wants it:** Familiarity with SQL semantics; some workloads tolerate weaker isolation for performance.
**Why we didn't adopt:** Snapshot isolation already gives Read Committed-or-better semantics. The weaker levels exist mostly as historical artifacts of lock-based engines and offer no real benefit on an MVCC base.

## B.3 Indexing

### Hash indexes
**What:** Hash-table-based index for O(1) point lookups, no range support.
**Why someone wants it:** Marginal speedup on exact-match-only access patterns.
**Why we didn't adopt:** Point lookups in our LSM are already accelerated by per-SSTable bloom filters. The marginal gain over an ordered index doesn't justify a separate index type with separate code paths and maintenance logic.
**To revisit:** If profiling shows point lookups dominating and bloom filters are insufficient, a hash-indexed keyspace could be added without disturbing the rest of the design.

### Inverted indexes for full-text search
**What:** Token-to-document indexes for text queries.
**Why someone wants it:** Search functionality, common in document databases.
**Why we didn't adopt:** Out of scope for v1. The mechanism (tokens as keys, PKs as values) fits naturally in our keyspace, but tokenization, ranking, stemming, and language handling are a substantial body of work on their own.
**To revisit:** Phase 4+ extension. The catalog and indexing system already accommodate the keyspace; this is mostly a tokenizer + scorer.

## B.4 Data model

### Separate keyspaces for documents vs. tables (v1's original approach)
**What:** `/doc/{collection}/{id}` and `/table/{id}/row/{pk}` as separate top-level keyspaces with separate handling.
**Why someone wants it:** Clearer mental separation between schema-on-write and schema-on-read data.
**Why we didn't adopt:** Tables and document collections share all the same machinery — keys, indexes, transactions, serialization. Separate keyspaces would mean duplicating the indexing path and catalog handling. v3 unifies them as `TableKind::Relational` vs `TableKind::Document` on a single code path.

## B.5 Large-value handling

### Overflow pages (page-based engine approach)
**What:** When a document exceeds page size, chain additional pages and store a pointer in the primary page.
**Why someone wants it:** Standard solution in B-tree engines; keeps everything in one file.
**Why we didn't adopt:** Doesn't apply to LSM — we don't have pages. The LSM-equivalent problem is solved by value separation (§5.3.1, WiscKey-style blob log), which avoids rewriting large values on every compaction pass.

## B.6 Server-layer features (out of scope by design)

These belong in a server built on top of this library, not in the library itself. They share a single justification: **we're an embeddable library, like SQLite**. A host process links us in. There's no network, no separate process, no remote clients to authenticate.

### Wire protocol (TCP + JSON or binary)
**Why we didn't adopt:** No network. The Rust API is the interface.
**To revisit:** A separate `db-server` crate could expose this library over TCP if desired.

### Authentication and RBAC (Argon2, role-based permissions)
**Why we didn't adopt:** Authentication against a library linked into your own process is meaningless — the host already has full memory access. Authorization is the host's responsibility.
**To revisit:** Belongs in any server layer built on top.

### Encryption at rest (AES-256-GCM)
**Why we didn't adopt:** v1 scope. The host can encrypt the data directory using OS-level tools (LUKS, FileVault, BitLocker) which is generally preferable — same threat model, broader battle-testing.
**To revisit:** If we want per-database encryption for shared-disk scenarios, the natural place is at the SSTable block layer with a pluggable cipher.

### Replication via oplog
**Why we didn't adopt:** No network. Multi-node replication requires a network layer, leader election, and operational tooling we're not building.
**To revisit:** The WAL itself can serve as a replication/CDC stream — a future change-feed API could expose committed batches in order.

### Consistent-hash sharding with virtual nodes
**Why we didn't adopt:** Sharding requires multiple nodes, which requires a network, which we don't have. Single-process embedded DBs don't shard.

## B.7 Query layer choices (Phase 6)

### Committing to a query language up front
**What:** Picking SQL, Datalog, or a fluent Rust DSL before Phase 4.
**Why someone wants it:** Indexing decisions in Phase 4 should serve the query language; deferring this risks rework.
**Why we didn't adopt:** Without a clear primary use case (graph-heavy? relational? document?), picking now would be guessing. We've flagged it as a Phase 6 prerequisite and noted that EXPLAIN must ship with the first executable query.
**To revisit:** As soon as a primary use case is identified.

### Aggregation pipeline as an engine-level feature
**What:** MongoDB-style `$match`, `$group`, `$sort`, `$lookup` stages as a built-in pipeline.
**Why someone wants it:** Powerful, composable analytical queries directly against documents.
**Why we didn't adopt:** This is a query-layer concern, not a storage-engine concern. The engine provides scans, indexes, and projections; aggregation is built on those.
**To revisit:** Natural Phase 6 feature once the query layer is chosen. Would map cleanly to operators on iterators.
