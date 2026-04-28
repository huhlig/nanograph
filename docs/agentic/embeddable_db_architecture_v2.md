# Embeddable Multi-Model Database in Rust
## Architecture & Implementation Plan (v2)

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
|   Transaction Manager    |   <-- new layer
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

---

# 4. Consistency and Concurrency Model

This section pins down semantics that everything else depends on. It must be settled before storage details.

## 4.1 Chosen model: MVCC with snapshot isolation

Rationale:
- Readers never block writers; writers never block readers.
- A natural fit for LSM, where multiple versions per key already exist on disk.
- Sufficient for the embedded use case without paying for full serializability.

## 4.2 Versioning

Every logical key is encoded with a monotonic sequence number:

```
physical_key = user_key || seq_no_be
```

`seq_no` is a 64-bit big-endian integer assigned at commit time by the transaction manager. On read, the iterator skips entries with `seq_no > snapshot_seq` and returns the highest-versioned entry per `user_key`.

Tombstones (deletes) are also versioned and only physically removed during compaction, once no live snapshot can observe them.

## 4.3 Transactions

```rust
trait Database {
    fn begin_read(&self) -> ReadTxn;
    fn begin_write(&self) -> WriteTxn;
}
```

- **Read transactions** hold a snapshot sequence number. They are cheap and freely concurrent.
- **Write transactions** are single-writer for v1 (one in flight at a time, others queue). This is the SQLite WAL model and is sufficient for an embedded DB. Optimistic concurrency with conflict detection can be added later without changing the on-disk format.

Write transactions must be the **only** path through which mutations reach the KV core. Index maintenance, catalog updates, and row writes happen inside the same transaction, atomically.

## 4.4 Durability

`commit()` accepts a `Durability` flag:

- `Sync` — fsync the WAL before returning (default).
- `Buffered` — return after WAL append; group-commit fsync on a timer.
- `None` — no durability; useful for tests and bulk loads.

## 4.5 Iterator semantics

Iterators are bound to a snapshot. Compaction and SSTable deletion are deferred (via reference counts on the manifest) until no iterator holds a snapshot referencing the deleted file. This is critical and must be designed into the manifest from day one — retrofitting it is painful.

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
    // CompareAndSwap can be added later; not in v1.
}
```

Notes on what changed from a naive design:

- All fallible ops return `Result`; I/O can fail.
- `&self` everywhere — interior mutability inside the engine. The store is `Sync` and shareable.
- Iterators are lifetime-bound to a snapshot.
- No bare `put` / `delete` on the trait. All mutations go through `write(batch)` so atomicity of multi-key updates (row + indexes + catalog) is guaranteed.

## 5.2 Internals

- **WAL**: append-only, fsync-on-commit per durability flag. Records are checksummed; recovery scans until the first invalid record (handling torn writes).
- **Memtable**: skiplist or B-tree map, keyed by `(user_key, seq_no)`.
- **SSTables**: sorted, immutable, with per-block compression (LZ4 default). Each SSTable carries a **bloom filter** to short-circuit point lookups; without bloom filters, point reads degrade as levels grow.
- **Block cache**: LRU over decompressed blocks. Size configurable; defaults proportional to memtable size.
- **Manifest**: an append-only log of "SSTable N added/removed at level L". Recovery replays the manifest, then the WAL.
- **Compaction**: leveled compaction (RocksDB-style) by default. Tombstones are dropped when no live snapshot can observe them and they have reached the bottom level.

## 5.3 Inspiration

- RocksDB, LevelDB (LSM mechanics)
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
0x04  free for future use
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
}
```

---

# 8. Data Model: Unified Tables

A key simplification from the previous version: **tables and document collections are the same thing.** A document collection is just a table with `kind = Document` and no fixed schema. They share:

- One catalog entry type
- One key prefix scheme
- One indexing path
- One serialization format

Tables differ only in whether the schema is enforced at write time and whether rows are projected into typed columns or stored as a `Value`.

## 8.1 Value type

Used for both document storage and dynamic columns:

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
- Type changes: not supported in-place. Require a migration that rewrites rows under a new schema version. Migration runs as a long-lived write transaction or as a background job that batches writes.

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

This means "all out-edges of node X with label Y" is a single prefix scan with cost proportional to the number of matching edges, not the size of the edge table.

---

# 10. Indexing System

## 10.1 Mechanics

Indexes are first-class KV entries. They are written and deleted **in the same WriteBatch** as the row they describe — this is the whole reason §4 mandated transactional batches.

For a row insert:

1. Encode primary key.
2. Read existing row at PK (within the txn) to compute index deltas if updating.
3. Build WriteBatch:
   - Put `/{table}/row/{pk}` = serialized row
   - For each index: Put `/{table}/idx/{idx}/{value}/{pk}` = empty (the PK is in the key)
   - For each index entry no longer matching (on update): Delete the old entry
4. Commit.

## 10.2 Index types

- **Primary** — implicit, the row store itself.
- **Secondary** — single or composite columns; ordered, supports range scans.
- **Unique** — enforced at write time by reading the index prefix inside the txn.
- **Inverted** — Phase 4+. Tokens become index keys; values are PKs.

## 10.3 Field extraction

```rust
pub enum FieldPath {
    Root,
    Field(String, Box<FieldPath>),    // "user.address.city"
    Index(usize, Box<FieldPath>),     // arrays
}
```

Missing fields produce no index entry (sparse indexes by default).

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

**Recommendation: multi-file only for v1.** Drop the single-file option.

```
/wal.log
/MANIFEST-N
/CURRENT
/sst_<level>_<id>.db
```

Single-file (SQLite-style) is much harder for an LSM than for a B-tree, because SSTables vary in size and number. Achieving it would require writing a page allocator and free-list on top of the file, effectively reinventing a small filesystem. If single-file portability becomes a real requirement later, the right move is probably a different storage backend (B-tree variant) rather than squeezing LSM into one file.

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

---

# 14. Implementation Phases

## Phase 1: Storage core
- WAL with crash recovery (must include torn-write handling)
- Memtable
- SSTable writer/reader with bloom filters and block compression
- Manifest and SSTable lifecycle (with snapshot-aware deletion)
- Leveled compaction
- **Property tests** for the merge iterator
- **Crash injection tests** for WAL recovery

## Phase 2: Transactions
- WriteBatch and snapshot machinery
- Single-writer write txns
- Durability flags
- **Concurrency tests** (loom or shuttle)

## Phase 3: Catalog and tables
- Tuple key encoder
- Catalog bootstrap
- Table abstraction over KV
- CBOR row serialization
- Schema-version-aware reads

## Phase 4: Indexes
- Secondary indexes with field-path extraction
- Unique constraint enforcement
- Index maintenance inside WriteBatch

## Phase 5: Graph
- Edge table conventions and indexes
- Traversal helpers (out_edges, in_edges, by_label)
- BFS/DFS iterators

## Phase 6 (optional): Query layer
- **Decision required up front:** fluent Rust DSL? SQL subset? Datalog (good fit for graph)?
- This decision affects which indexes we want in Phase 4.

---

# 15. Testing Strategy

Worth a section because LSM bugs are silent in normal use and catastrophic in production.

- **Unit tests** per component (WAL, SSTable, memtable, encoder).
- **Property tests** (`proptest`) for: tuple encoding round-trip and order preservation; merge iterator output equals reference `BTreeMap`; index maintenance invariants (every row's indexes are consistent after arbitrary write sequences).
- **Crash injection**: pause writes mid-fsync, kill, reopen, verify durability matches the durability flag.
- **Concurrency**: `loom` for the transaction manager and snapshot ref-counting.
- **Fuzzing** the SSTable reader against malformed inputs.

---

# 16. Key Design Principles

- Everything is KV underneath.
- Tables are logical partitions, not storage units.
- Document collections are tables with no fixed schema — same code path.
- Graphs are compositions, not primitives.
- Indexes are first-class KV entries, maintained transactionally.
- All mutations go through WriteBatch; there is no non-transactional path.
- Lexicographic byte order is the contract that everything above the KV core relies on.

---

# 17. Summary

This design prioritizes:

- A small, focused storage core with well-defined consistency
- A unified data model (tables ≈ collections; graphs = two tables)
- Composability of indexes, transactions, and models
- Long-term extensibility through a stable key encoding

The unchanged north star:

> A database is not multiple storage systems, but one ordered keyspace with multiple interpretations.

---

# Appendix A: Changes from v1

| Area | v1 | v2 |
|---|---|---|
| Concurrency | Unspecified | MVCC + snapshot isolation, single-writer v1 |
| `KvStore` trait | `&mut self`, no `Result`, ad-hoc iterator | `&self`, `Result`, snapshot-bound iterators, WriteBatch only |
| Key encoding | Hand-waved | Order-preserving tuple encoding (FoundationDB-style) |
| LSM details | Mentioned in passing | Bloom filters, block cache, manifest, snapshot-aware compaction |
| Documents vs tables | Separate keyspaces | Same machinery, `kind` field on TableDef |
| Graph indexes | "by from, by to, by label" | Concrete prefix layout for traversal |
| Catalog | Lives in KV (circular) | Reserved table_id 0, hardcoded bootstrap schema |
| Value serialization | Unspecified | CBOR |
| Schema evolution | Vague | `schema_version` per row, defaulting rules, explicit migration |
| Single-file format | Optional | Dropped for v1 |
| Testing | Not mentioned | Property tests, crash injection, loom concurrency tests |
| Build vs buy | Not addressed | Explicit section §3 |
