````markdown id="opcode_table_with_indexing_hnsw_v1"
# Common Graph Bytecode (CGB)

## Unified Opcode Table for Gremlin / Cypher / GQL

### + Multi-Index & HNSW Search Support

---

# 1. Design Goals

The opcode table must:

- Capture **semantic overlap** between:
    - Gremlin (imperative traversal)
    - Cypher (declarative pattern matching)
    - GQL (ISO declarative graph query)
- Remain language-neutral
- Be register-based
- Support logical algebra
- Support advanced search operators
- Support multiple index types (including HNSW vector search)

---

# 2. Semantic Normalization Strategy

| Concept        | Gremlin              | Cypher           | GQL              | Unified Model      |
|----------------|----------------------|------------------|------------------|--------------------|
| Node scan      | `g.V()`              | `MATCH (n)`      | `MATCH (n)`      | `SCAN_NODE`        |
| Label filter   | `hasLabel()`         | `(n:Label)`      | `(n:Label)`      | `SCAN_NODE(label)` |
| Edge traversal | `out()`              | `-[]->`          | `-[]->`          | `EXPAND`           |
| Optional       | `optional()`         | `OPTIONAL MATCH` | `OPTIONAL MATCH` | `OPTIONAL_EXPAND`  |
| Filter         | `has()`, `where()`   | `WHERE`          | `WHERE`          | `FILTER`           |
| Projection     | `select()`           | `RETURN`         | `RETURN`         | `PROJECT`          |
| Aggregation    | `group()`, `count()` | `GROUP BY`       | `GROUP`          | `AGGREGATE`        |
| Subquery       | `__.()`              | `CALL {}`        | `SUBQUERY`       | `APPLY`            |
| Path capture   | `path()`             | `p = (...)`      | `PATH`           | `PATH_CONSTRUCT`   |

---

# 3. Opcode Categories

1. Graph Navigation
2. Pattern Matching
3. Relational Algebra
4. Control Flow
5. Expression
6. Index & Search
7. Vector / Similarity Search (HNSW)

---

# 4. Core Opcode Table

---

## 4.1 Graph Navigation

| Opcode            | Operands                          | Description               |
|-------------------|-----------------------------------|---------------------------|
| `SCAN_NODE`       | out, label?                       | Full node scan            |
| `SCAN_EDGE`       | out, label?                       | Full edge scan            |
| `EXPAND`          | from, direction, edge_label?, out | Traverse edges            |
| `OPTIONAL_EXPAND` | from, direction, edge_label?, out | Nullable traversal        |
| `MATCH_PATTERN`   | pattern_id, bindings              | Declarative pattern match |
| `PATH_CONSTRUCT`  | nodes[], edges[], out             | Build path object         |

---

## 4.2 Relational Algebra

| Opcode      | Operands                 | Description       |
|-------------|--------------------------|-------------------|
| `FILTER`    | predicate                | Boolean filter    |
| `PROJECT`   | expressions              | Projection        |
| `AGGREGATE` | group_keys, aggregations | Group + aggregate |
| `SORT`      | sort_keys                | Order rows        |
| `LIMIT`     | count                    | Limit rows        |
| `DISTINCT`  | —                        | Remove duplicates |
| `UNION`     | subplans                 | Union results     |

---

## 4.3 Control Flow

| Opcode      | Operands              | Description         |
|-------------|-----------------------|---------------------|
| `APPLY`     | subplan, correlations | Correlated subquery |
| `EXISTS`    | subplan               | Existential check   |
| `JOIN`      | join_type, condition  | Logical join        |
| `SEMI_JOIN` | condition             | For EXISTS          |
| `ANTI_JOIN` | condition             | For NOT EXISTS      |

---

# 5. Index & Search Opcode Extensions

To support multi-index search and vector similarity search.

---

# 5.1 Index Abstraction

We support multiple index kinds:

| Index Type | Use Case                     |
|------------|------------------------------|
| B-Tree     | Equality, range              |
| Hash       | Exact lookup                 |
| Inverted   | Text search                  |
| Composite  | Multi-field lookup           |
| HNSW       | Approximate nearest neighbor |
| IVF        | Semantic Similarity          |
| Fulltext   | Keyword search               |

---

# 5.2 Index Opcodes

---

## INDEX_SEEK

Exact lookup via index.

```rust
INDEX_SEEK {
    index_id: IndexId,
    key_expr: ExprId,
    out: RegId,
}
````

Used for:

* `has("id", 42)`
* `WHERE n.id = 42`

---

## INDEX_RANGE_SCAN

```rust
INDEX_RANGE_SCAN {
    index_id: IndexId,
    lower: Option<ExprId>,
    upper: Option<ExprId>,
    out: RegId,
}
```

Used for:

* `age > 30`
* `BETWEEN`

---

## INDEX_SCAN

Full index scan (faster than full graph scan).

---

## FULLTEXT_SEARCH

```rust
FULLTEXT_SEARCH {
    index_id: IndexId,
    query: ExprId,
    out: RegId,
}
```

---

# 6. HNSW / Vector Search Support

---

## 6.1 HNSW Overview

HNSW (Hierarchical Navigable Small World) is used for:

* Vector similarity search
* Embedding search
* Semantic graph retrieval
* ANN (Approximate Nearest Neighbor)

---

## 6.2 VECTOR_SEARCH Opcode

```rust
VECTOR_SEARCH {
    index_id: IndexId,
    query_vector: ExprId,
    k: usize,
    ef_search: Option<u32>,
    out: RegId,
    score_out: Option<RegId>,
}
```

Semantics:

* Uses HNSW index
* Returns top-k nearest nodes
* Optionally outputs similarity score

---

## 6.3 VECTOR_FILTER

Post-filter after ANN search.

```rust
VECTOR_FILTER {
    predicate: ExprId,
}
```

---

## 6.4 VECTOR_JOIN

Join graph traversal with vector result set.

```rust
VECTOR_JOIN {
    left_reg: RegId,
    right_reg: RegId,
}
```

---

# 7. Example Cross-Language Mapping

---

## Cypher

```cypher
MATCH (n:Doc)
WHERE n.embedding <=> $query_vector < 0.2
RETURN n
LIMIT 10
```

Lowered to:

```
VECTOR_SEARCH(index=doc_embedding, k=10)
FILTER(score < 0.2)
PROJECT(n)
LIMIT(10)
```

---

## Gremlin

```groovy
g.V().hasLabel("Doc")
 .order().by(embeddingDistance(queryVec))
 .limit(10)
```

Lowered to:

```
VECTOR_SEARCH(...)
LIMIT(10)
```

---

# 8. Index-Aware Logical Plan

Logical plan can include:

* IndexScanNode
* IndexSeekNode
* VectorSearchNode
* HybridSearchNode

Optimizer responsibilities:

* Choose best index
* Decide between:

    * Graph scan
    * B-tree
    * HNSW
* Push filters into index operator
* Estimate ANN cost

---

# 9. Hybrid Query Example

Query:

```cypher
MATCH (p:Person)
WHERE p.age > 30
AND vectorDistance(p.embedding, $v) < 0.3
RETURN p
LIMIT 5
```

Logical Plan:

```
INDEX_RANGE_SCAN(age_index)
VECTOR_SEARCH(embedding_index)
INTERSECT
FILTER(...)
LIMIT(5)
```

Or:

```
VECTOR_SEARCH
FILTER(age > 30)
LIMIT(5)
```

Optimizer decides based on:

* Cardinality
* Selectivity
* HNSW ef_search cost
* Vector dimensionality

---

# 10. Execution Architecture Extensions

---

## 10.1 IndexManager Trait

```rust
pub trait IndexManager {
    fn seek(&self, index: IndexId, key: Value) -> NodeIterator;
    fn range_scan(&self, index: IndexId, lower: Option<Value>, upper: Option<Value>) -> NodeIterator;
    fn fulltext(&self, index: IndexId, query: &str) -> NodeIterator;
    fn vector_search(
        &self,
        index: IndexId,
        query: &[f32],
        k: usize,
        ef_search: Option<u32>,
    ) -> Vec<(NodeId, f32)>;
}
```

---

## 10.2 GraphStorage Extended

```rust
pub trait GraphStorage {
    fn index_manager(&self) -> &dyn IndexManager;
}
```

---

# 11. HNSW Integration Strategy

Options:

1. Embedded Rust HNSW crate
2. External vector DB adapter
3. Custom graph-native HNSW layer

Recommended MVP:

* Per-label vector index
* Memory-mapped HNSW
* Async rebuild support

---

# 12. Optimizer Requirements for Multi-Index

The optimizer must:

* Track index metadata:

    * Type
    * Cardinality
    * Selectivity
    * Vector dimensionality
* Support:

    * Cost-based decision
    * Hybrid index plans
    * ANN + filter pushdown
    * Early termination

---

# 13. Final Unified Opcode Set

### Core Graph

SCAN_NODE
SCAN_EDGE
EXPAND
OPTIONAL_EXPAND
MATCH_PATTERN
PATH_CONSTRUCT

### Relational

FILTER
PROJECT
AGGREGATE
SORT
LIMIT
DISTINCT
JOIN
SEMI_JOIN
ANTI_JOIN

### Control

APPLY
EXISTS
UNION

### Index

INDEX_SEEK
INDEX_RANGE_SCAN
INDEX_SCAN
FULLTEXT_SEARCH

### Vector / ANN

VECTOR_SEARCH
VECTOR_FILTER
VECTOR_JOIN

---

# 14. Strategic Outcome

This opcode table:

* Fully captures Gremlin/Cypher/GQL semantic overlap
* Supports traditional graph traversal
* Supports relational-style algebra
* Supports multiple index kinds
* Supports HNSW-based ANN vector search
* Enables hybrid graph + semantic search queries

This becomes:

A unified **Graph + Vector Query Runtime** in Rust.

---
