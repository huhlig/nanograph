# ADR-0023: Testing, Fault Injection, and Simulation Strategy

## Status

Proposed

## Context

Nanograph is a distributed system where many failures only emerge under rare timing and fault conditions.

## Decision

Adopt a **multi-layered testing strategy** with built-in fault injection and deterministic simulation.

## Decision Drivers

* Reliability
* Confidence in correctness
* Regression prevention

## Design

* Unit and property-based testing
* Deterministic simulation of Raft and storage
* Fault injection:

    * Node crashes
    * Network partitions
    * Disk corruption
* End-to-end integration tests

## Consequences

### Positive

* High confidence in correctness
* Easier debugging of rare bugs

### Negative

* Higher test infrastructure cost

## Alternatives Considered

* Manual testing only (rejected)

## Related ADRs

* [ADR-0007: Clustering, Sharding, Replication, and Consensus](ADR-0007-Clustering-Sharding-Replication-Consensus.md)
