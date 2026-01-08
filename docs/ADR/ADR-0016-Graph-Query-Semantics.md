# ADR-0016: Graph Query Semantics

## Status

Proposed

## Context

Nanograph exposes graph capabilities built atop a KV-first storage model. Clear and constrained graph query semantics are required to ensure predictable performance, correctness, and composability with other query types.

## Decision

Nanograph will support **explicit, bounded graph queries** focused on traversals and neighborhood exploration rather than unrestricted declarative graph languages.

## Decision Drivers

* Predictable performance
* Alignment with KV and shard boundaries
* Avoiding unbounded or accidental full-graph scans

## Design

* Graph queries are expressed as traversal operations
* Traversals are:

    * Directional (in, out, both)
    * Depth-bounded
    * Filterable by node and edge attributes
* Traversals execute within a single shard where possible
* Cross-shard traversals are explicit and incremental

### Supported Operations

* Get node / edge by ID
* Neighbor expansion
* Bounded breadth-first and depth-first traversals
* Path existence and shortest-path (bounded)

## Consequences

### Positive

* Predictable query cost
* Easy composition with KV and document queries

### Negative

* No Cypher/Gremlin-style declarative language initially

## Alternatives Considered

* Full declarative graph query language (rejected for v1)

## Related ADRs

* [ADR-0006: Key-Value, Document, and Graph Support](ADR-0006-Key-Value-Document-Graph-Support.md)
* [ADR-0015: Query Interface Strategy](ADR-0015-Query-Interface-Strategy.md)
