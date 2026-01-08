# ADR-0024: Backup, Restore, Import, and Export

## Status

Proposed

## Context

Nanograph is intended for use in both embedded and distributed production environments where data durability, portability, and disaster recovery are critical. Users must be able to reliably back up data, restore systems after failure, and move data between environments or clusters.

## Decision

Provide **first-class, versioned backup and restore mechanisms**, along with explicit **import and export workflows**, integrated with Nanograph’s storage, WAL, and shard architecture.

## Decision Drivers

* Disaster recovery and business continuity
* Data portability between environments
* Support for migrations, testing, and analytics
* Compliance and auditing requirements

## Design

### Backup

* Support **logical and physical backups**:

    * Physical backups: shard-level snapshots of on-disk state
    * Logical backups: table- or namespace-level exports
* Backups are:

    * Consistent at a specific WAL position
    * Snapshot-based and non-blocking
* Metadata captured:

    * Schema and table definitions
    * Index definitions
    * Embedding and model metadata
    * Cluster and shard topology (where applicable)

### Restore

* Restore supports:

    * Full cluster restore
    * Single-node or embedded restore
    * Selective table or shard restore (logical backups)
* WAL replay is used to reach a consistent post-snapshot state
* Restore operations are idempotent and resumable

### Import

* Import workflows ingest external data into Nanograph tables
* Supported sources:

    * Logical Nanograph exports
    * Structured formats (e.g., JSON, CSV, binary)
* Import is performed via batch ingestion pipelines
* Optional validation and transformation hooks

### Export

* Export workflows allow data extraction for:

    * Migration to other systems
    * Offline analysis
    * Debugging and testing
* Export formats:

    * Logical Nanograph format (versioned)
    * Standard structured formats (e.g., JSON, CSV)
* Export supports filtering, projection, and pagination

### Consistency & Safety

* Backup and export operations operate at **snapshot isolation**
* Distributed backups coordinate across shards to produce a consistent view
* All formats are versioned and backward compatible

## Consequences

### Positive

* Strong disaster recovery story
* Enables environment cloning and testing
* Supports long-term data portability

### Negative

* Increased implementation and maintenance complexity
* Requires careful coordination with WAL and compaction

## Alternatives Considered

* Relying solely on filesystem-level backups (rejected)
* External backup tooling only (rejected)

## Related ADRs

* [ADR-0005: Write Ahead Log Support](ADR-0005-Write-Ahead-Log-Support.md)
* [ADR-0007: Clustering, Sharding, Replication, and Consensus](ADR-0007-Clustering-Sharding-Replication-Consensus.md)
* [ADR-0021: Upgrade Migration and Backward Compatibility](ADR-0021-Upgrade-Migration-and-Backward-Compatibility.md)
* [ADR-0023: Testing, Fault Injection, and Simulation Strategy](ADR-0023-Testing-Fault-Injection-and-Simulation-Strategy.md)
