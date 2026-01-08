# ADR-0021: Upgrade, Migration, and Backward Compatibility

## Status

Proposed

## Context

Nanograph clusters and embedded deployments must be upgradeable without data loss or prolonged downtime.

## Decision

Support **rolling upgrades** with explicit migration paths and strict backward compatibility guarantees.

## Decision Drivers

* Production operability
* Long-lived data
* User trust

## Design

* Backward-compatible WAL and storage readers
* Explicit on-disk format versions
* Rolling upgrade support for clusters
* Offline and online migration tooling

## Consequences

### Positive

* Minimal downtime
* Predictable upgrades

### Negative

* Long-term maintenance burden

## Alternatives Considered

* Breaking upgrades (rejected)

## Related ADRs

* [ADR-0020: Configuration, Feature Flags, and Versioning](ADR-0020-Configuration-Feature-Flags-and-Versioning.md)
