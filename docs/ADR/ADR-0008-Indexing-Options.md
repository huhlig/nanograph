# ADR-0008: Indexing Options

## Status

Proposed

## Context

Nanograph must support traditional and vector-based querying.

## Decision

Support **pluggable indexing strategies**:

* HNSW, IVF, PQ for vectors
* BTree for ordered data
* Full-text inverted indexes

## Decision Drivers

* Query flexibility
* Performance
* AI-native workloads

## Design

* Index interface with lifecycle hooks
* Background build and rebuild

## Consequences

### Positive

* Flexible query optimization

### Negative

* Index maintenance complexity

## Alternatives Considered

* Single index type (rejected)

## Related ADRs

* [ADR-0004: Storage File Formats](ADR-0004-Storage-File-Formats.md)
