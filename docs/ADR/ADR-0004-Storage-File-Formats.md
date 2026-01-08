# ADR-0004: Storage File Formats

## Status

Proposed

## Context

Nanograph must support diverse access patterns: point lookups, range scans, writes under contention, and large-scale indexing. No single storage structure optimally satisfies all workloads.

## Decision

Support **multiple internal storage file formats**, selected per table or index:

* Adaptive Radix Tree (ART)
* B+ Tree
* Log-Structured Merge Tree (LSM)

## Decision Drivers

* Performance across workloads
* Flexibility per table/index
* Proven database designs

## Design

* Storage engines implement a common `TableEngine` interface
* Format choice defined at table or index creation

### Format Roles

* **ART**: In-memory or hot key-paths
* **B+ Tree**: Range queries, ordered scans
* **LSM**: High write throughput, WAL-friendly

## Consequences

### Positive

* Tailored performance
* Future extensibility

### Negative

* Increased implementation complexity
* Requires tuning guidance

## Alternatives Considered

* Single-format engine (rejected)

## Related ADRs

* [ADR-0003: Virtual File System Abstraction](ADR-0003-Virtual-File-System-Abstraction.md)
* [ADR-0005: Write Ahead Log Support](ADR-0005-Write-Ahead-Log-Support.md)
