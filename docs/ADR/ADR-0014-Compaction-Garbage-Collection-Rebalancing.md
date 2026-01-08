# ADR-0014: Compaction, Garbage Collection, and Rebalancing

## Status

Proposed

## Context

Storage engines and shards accumulate obsolete data over time due to updates, deletes, and re-sharding.

## Decision

Implement **background compaction and garbage collection**, coordinated with shard rebalancing.

## Decision Drivers

* Storage efficiency
* Performance stability
* Long-running cluster health

## Design

* LSM compaction tiers
* Snapshot-aware GC
* Online shard rebalancing with throttling

## Consequences

### Positive

* Bounded storage growth
* Reduced performance cliffs

### Negative

* Background resource usage

## Alternatives Considered

* Manual compaction only (rejected)

## Related ADRs

* [ADR-0007: Clustering, Sharding, Replication, and Consensus](ADR-0007-Clustering-Sharding-Replication-Consensus.md)
