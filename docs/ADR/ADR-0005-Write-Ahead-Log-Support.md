# ADR-0005: Write Ahead Log (WAL) Support

## Status

Proposed

## Context

Nanograph requires strong durability guarantees and crash recovery across embedded and distributed modes.

## Decision

Implement a **Write Ahead Log** as a first-class subsystem. All mutating operations must be appended to the WAL before becoming visible.

## Decision Drivers

* Durability guarantees
* Crash recovery
* Raft replication compatibility

## Design

* Append-only segmented log
* Checksummed entries
* Periodic snapshots
* Truncation after compaction

## Consequences

### Positive

* Strong durability
* Simplified recovery

### Negative

* Write amplification

## Alternatives Considered

* Shadow paging (rejected)

## Related ADRs

* [ADR-0004: Storage File Formats](ADR-0004-Storage-File-Formats.md)
* [ADR-0007: Clustering, Sharding, Replication, and Consensus](ADR-0007-Clustering-Sharding-Replication-Consensus.md)
