
# Embeddable Multi-Model Database in Rust
## Architecture & Implementation Plan

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

---

# 2. High-Level Architecture

```
+---------------------------+
|        Query Layer        |
+---------------------------+
|      Logical Models       |
|  - Tables                |
|  - Documents             |
|  - Graph                 |
+---------------------------+
|     Indexing Engine      |
+---------------------------+
|     Catalog System       |
+---------------------------+
|   LSM KV Storage Core    |
+---------------------------+
|      File System         |
+---------------------------+
```

---

# 3. Storage Engine (LSM KV Core)

The foundation is an LSM-tree key-value store.

## Core API

```rust
trait KvStore {
    fn get(&self, key: &[u8]) -> Option<Vec<u8>>;
    fn put(&mut self, key: &[u8], value: &[u8]);
    fn delete(&mut self, key: &[u8]);

    fn scan_prefix(&self, prefix: &[u8]) -> Iterator<Item=(Vec<u8>, Vec<u8>)>;
}
```

## Characteristics

- Append-only writes (WAL)
- Memtable + SSTables
- Background compaction
- Prefix-based iteration

Inspired by:
- RocksDB
- LevelDB
- sled (Rust)

---

# 4. Keyspace Design

All data is stored in a unified ordered keyspace.

## Key Format

```
[namespace_id][type][logical_path...]
```

## Logical Layout

### Tables
```
/table/{table_id}/row/{primary_key}
/table/{table_id}/index/{index_id}/{index_key}/{primary_key}
```

### Documents
```
/doc/{collection_id}/{doc_id}
```

### Graph (logical view; backed by tables)
```
/table/{node_table_id}/row/{node_id}
/table/{edge_table_id}/row/{edge_id}
```

---

# 5. Catalog System

The catalog defines all logical structures.

## Tables

```
/catalog/tables/{table_id}
/catalog/tables_by_name/{name} → table_id
```

## Graphs

```
/catalog/graphs/{graph_id}
```

## Definitions

```rust
struct TableDef {
    id: u32,
    name: String,
    kind: TableKind,
    schema: Option<Schema>,
    indexes: Vec<IndexDef>,
}
```

---

# 6. Data Models

## 6.1 Tables (Relational Style)

- Strong schema optional
- Row-based storage

```rust
struct Row {
    schema_id: u32,
    values: Vec<Value>,
}
```

---

## 6.2 Documents

Flexible JSON-like structure:

```rust
enum Value {
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

---

## 6.3 Graph Model

Graphs are NOT a special storage type.

They are composed of:

- Node table
- Edge table

```rust
struct Edge {
    from: u64,
    to: u64,
    label: String,
}
```

Indexes:
- by `from`
- by `to`
- by `label`

---

# 7. Indexing System

Indexes are stored in KV space.

## Format

```
/table/{table_id}/index/{index_id}/{key}/{primary_key}
```

## Types

- Primary index (implicit)
- Secondary indexes
- Inverted indexes (optional)

---

# 8. Multi-Table Design

Each table is isolated by ID:

- No physical separation required
- Logical isolation via key prefixing

Example:

```
[table_id][ROW][primary_key]
[table_id][INDEX][index_id][key]
```

---

# 9. Graph Design (Important Decision)

## Recommended Design

Graphs are NOT standalone storage units.

Instead:

- Graph = metadata + two tables

```rust
struct GraphDef {
    graph_id: u32,
    node_table: u32,
    edge_table: u32,
}
```

### Why this design:

- avoids duplication
- uses existing indexing system
- simplifies compaction
- enables multiple graphs easily

---

# 10. LSM Considerations

## Compaction

- size-tiered or leveled
- table-aware prefix isolation

## Write Amplification

Minimized via:
- prefix grouping
- separate logical tables

---

# 11. File Layout

## Option A: Multi-file (recommended first)

```
/wal.log
/sstable_1.db
/sstable_2.db
/manifest
```

## Option B: Single file (SQLite-style)

```
header | WAL | SSTables | metadata
```

---

# 12. API Design

```rust
let db = Database::open("data.db");

let users = db.table("users");

users.insert(key, row);
users.get(key);
users.scan();
```

Graph:

```rust
let graph = db.graph("social");

graph.add_node(...);
graph.add_edge(...);
graph.traverse(node_id);
```

---

# 13. Implementation Phases

## Phase 1: Core Engine
- LSM KV store
- WAL
- SSTables
- prefix scanning

## Phase 2: Tables
- catalog system
- table abstraction
- row serialization

## Phase 3: Indexes
- secondary indexes
- index maintenance

## Phase 4: Documents
- dynamic schema
- JSON-like Value system

## Phase 5: Graph
- node/edge tables
- traversal APIs

## Phase 6: Query Layer (optional)
- fluent API or DSL
- join-like operations

---

# 14. Key Design Principles

- Everything is KV underneath
- Tables are logical partitions, not storage units
- Graphs are compositions, not primitives
- Indexes are first-class KV entries
- No special-case storage paths

---

# 15. Inspiration Systems

- SQLite (file format simplicity)
- RocksDB (LSM storage)
- FoundationDB (layered architecture)
- ArangoDB (multi-model approach)

---

# 16. Summary

This design prioritizes:

- simplicity of storage engine
- flexibility of data models
- composability over specialization
- long-term extensibility

The key insight:

> A database is not multiple storage systems, but one ordered keyspace with multiple interpretations.