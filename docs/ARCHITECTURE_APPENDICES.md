# Nanograph Architecture Appendices

This document contains cross-cutting architectural constraints and execution guidance that apply across all Nanograph
ADRs. These sections are intentionally non-ADR to avoid churn and to serve as long-lived reference material.

---

## Appendix A: System Invariants

The following invariants are **non-negotiable architectural guarantees**. Any future design or implementation change
must preserve these properties.

### A.1 Data Safety & Durability

1. **No acknowledged write may be lost**
    * Any write acknowledged to a client MUST be recoverable after a crash.
    * WAL durability semantics may not be bypassed.

2. **Recovery is deterministic**
    * Given the same on-disk state and WAL, recovery MUST produce the same logical state.

3. **Backups are consistent**
    * Backups MUST correspond to a well-defined snapshot or WAL position.

---

### A.2 Consistency & Consensus

4. **Shard-level linearizability**
    * All writes within a shard MUST be linearizable.

5. **Single source of truth per shard**
    * At any moment, exactly one Raft leader may accept writes for a shard.

6. **No split-brain writes**
    * The system MUST prefer unavailability to inconsistency.

---

### A.3 Storage & Data Models

7. **KV is the foundational abstraction**
    * All higher-level models (document, graph, vector) MUST compile to KV operations.

8. **No hidden cross-shard transactions**
    * Atomicity across shards MUST be explicit and opt-in.

9. **On-disk formats are versioned forever**

    * No released on-disk format may be modified incompatibly.

---

### A.4 Query & Execution

10. **All queries are bounded**
    * No query may implicitly scan an unbounded dataset.

11. **Execution plans are explicit**
    * Hybrid queries MUST compile into an operator graph.

12. **Vector search is approximate by default**
    * Exact search MUST be explicit and opt-in.

---

### A.5 AI & Embeddings

13. **Embeddings are derived data**
    * Embeddings MUST be reproducible from source data and model metadata.

14. **Model changes never corrupt primary data**
    * Re-embedding MUST NOT invalidate stored documents or graphs.

---

### A.6 Multi-Tenancy & Isolation

17. **Tenant boundaries are inviolable**
    * No operation may access data across tenant boundaries without explicit authorization.
    * Data isolation MUST be enforced at the storage layer via key prefixing.

18. **Resource quotas are enforced**
    * All tenant operations MUST be subject to quota checks.
    * Quota violations MUST result in graceful degradation or rejection.

19. **Tenant metadata is protected**
    * Tenant configuration and metadata MUST be stored securely.
    * Only authorized administrators may modify tenant settings.

---

### A.7 Operations & Upgrades

20. **Upgrades must not require full cluster downtime**

21. **Observability is not optional**
    * All critical subsystems MUST expose metrics and logs.

---

## Appendix B: Implementation Mapping

This section maps ADR decisions to **concrete implementation boundaries**. It is intended to guide repository layout,
ownership, and dependency direction.

---

### B.1 Core Crates / Modules

#### `nanograph-core`

* KV abstractions
* Table interfaces
* Transaction primitives
* Invariants enforcement
* **Multi-tenancy types** (TenantId, DatabaseId, ResourcePath)
* **Tenant metadata structures**

**Related ADRs:**
[ADR-0004](ADR/ADR-0004-Storage-File-Formats.md), [ADR-0006](ADR/ADR-0006-Key-Value-Document-Graph-Support.md), [ADR-0012](ADR/ADR-0012-Transaction-Model-and-Isolation-Levels.md), [ADR-0025](ADR/ADR-0025-Multi-Tenancy-and-Isolation.md)

---

#### `nanograph-storage-vfs`

* Virtual File System traits
* OS and in-memory implementations

**Related ADRs:** [ADR-0003](ADR/ADR-0003-Virtual-File-System-Abstraction.md)

---

#### `nanograph-wal`

* WAL encoding/decoding
* Snapshot coordination
* Recovery logic

**Related ADRs:
** [ADR-0005](ADR/ADR-0005-Write-Ahead-Log-Support.md), [ADR-0021](ADR/ADR-0021-Upgrade-Migration-and-Backward-Compatibility.md), [ADR-0024](ADR/ADR-0024-Backup-Restore-Import-Export.md)

---

### B.2 Storage Engines

#### `nanograph-art`

* Adaptive Radix Tree implementation

#### `nanograph-btree`

* B+Tree implementation

#### `nanograph-lsm`

* LSM tree implementation

**Related ADRs:
** [ADR-0004](ADR/ADR-0004-Storage-File-Formats.md), [ADR-0014](ADR/ADR-0014-Compaction-Garbage-Collection-Rebalancing.md)

---

### B.3 Distributed Systems

#### `nanograph-raft`

* Raft consensus integration
* Log replication

#### `nanograph-shard-manager`

* Shard placement
* Rebalancing
* Metadata tables

**Related ADRs:
** [ADR-0007](ADR/ADR-0007-Clustering-Sharding-Replication-Consensus.md), [ADR-0014](ADR/ADR-0014-Compaction-Garbage-Collection-Rebalancing.md)

---

### B.4 Query & Execution

#### `nanograph-query`

* Query AST / operator graph
* Rule-based optimizer

#### `nanograph-exec`

* Operator execution engine
* Backpressure and scheduling

**Related ADRs:
** [ADR-0015](ADR/ADR-0015-Query-Interface-Strategy.md), [ADR-0016](ADR/ADR-0016-Graph-Query-Semantics.md), [ADR-0017](ADR/ADR-0017-Hybrid-Query-Execution.md)

---

### B.5 Indexing & Search

#### `nanograph-index`

* Index interfaces
* Lifecycle management

#### `nanograph-index-btree`

* Ordered Data

#### `nanograph-index-vector`

* Vector indexes (HNSW, IVF, PQ)

#### `nanograph-index-text`

* Full-text indexing

**Related ADRs:
** [ADR-0008](ADR/ADR-0008-Indexing-Options.md), [ADR-0019](ADR/ADR-0019-Semantic-Ranking-and-Scoring-Strategy.md)

---

### B.6 AI & Embeddings

#### `nanograph-embedding`

* Embedding pipelines
* Model adapters
* Version tracking

**Related ADRs:** [ADR-0018](ADR/ADR-0018-Embedding-Lifecycle-and-Model-Integration.md)

---

### B.7 API & Frontend

#### `nanograph-api`

* Public API definitions
* Auth hooks

#### `nanograph-transport`

* gRPC / HTTP
* mTLS integration

**Related ADRs:
** [ADR-0009](ADR/ADR-0009-Frontend-Backend-Separation-Embedding-Security.md), [ADR-0010](ADR/ADR-0010-Authentication-Authorization-Access-Control.md), [ADR-0022](ADR/ADR-0022-SDK-Design-and-Language-Bindings.md)

---

### B.8 Operations & Tooling

#### `nanograph-observability`

* Metrics
* Logging
* Auditing

#### `nanograph-tools`

* Backup / restore
* Import / export
* Migration tooling

**Related ADRs:
** [ADR-0011](ADR/ADR-0011-Observability-Telemetry-Auditing.md), [ADR-0021](ADR/ADR-0021-Upgrade-Migration-and-Backward-Compatibility.md), [ADR-0024](ADR/ADR-0024-Backup-Restore-Import-Export.md)

---

### B.9 Testing & Simulation

#### `nanograph-testkit`

* Deterministic simulation
* Fault injection
* Property testing

**Related ADRs:** [ADR-0023](ADR/ADR-0023-Testing-Fault-Injection-and-Simulation-Strategy.md)

---

## Closing Notes

* Dependency direction MUST flow inward (tools → API → core → storage)
* No storage engine may depend on query or API layers
* Invariants in Appendix A override all other guidance

---

---

## Appendix C: Threat Model

This appendix enumerates security threats, trust boundaries, and mitigations for Nanograph. It assumes hostile networks,
partial node compromise, and untrusted clients.

---

### C.1 Trust Boundaries

1. **Client ↔ API Boundary**

    * Untrusted input
    * Authenticated via mTLS + identity credentials

2. **Node ↔ Node Boundary**

    * Mutually authenticated
    * Encrypted transport only

3. **Process ↔ Storage Boundary**

    * OS-level isolation assumed
    * Disk contents treated as sensitive

4. **Embedded Mode Boundary**

    * Host application is semi-trusted
    * Defensive validation required

---

### C.2 Threat Categories

#### C.2.1 Network Attacks

* Man-in-the-middle
* Replay attacks
* Unauthorized cluster membership

**Mitigations:**

* Mandatory mTLS
* Certificate pinning
* Raft peer identity validation

---

#### C.2.2 Data Corruption & Tampering

* WAL truncation
* On-disk file modification
* Partial writes

**Mitigations:**

* Checksummed WAL entries
* Versioned file formats
* Atomic fsync boundaries

---

#### C.2.3 Authentication & Authorization Abuse

* Credential reuse
* Privilege escalation

**Mitigations:**

* Short-lived credentials
* ABAC / PBAC enforcement at query compile time
* No implicit admin paths

---

#### C.2.4 Denial of Service

* Unbounded queries
* Vector index explosion
* WAL growth exhaustion

**Mitigations:**

* Query budgets
* Resource quotas
* Backpressure and admission control

---

#### C.2.5 Supply Chain & Plugin Risk

* Malicious embedding models
* Untrusted extensions

**Mitigations:**

* Signed model artifacts
* Explicit capability grants
* Sandboxed execution

---

### C.3 Non-Goals

* Protecting against kernel-level compromise
* Side-channel resistance beyond basic isolation

---

## Appendix D: Failure Mode Catalog

This appendix documents known failure modes and the system’s expected behavior. These are **design targets**, not
implementation accidents.

---

### D.1 Storage Failures

| Failure           | Expected Behavior                     |
|-------------------|---------------------------------------|
| Disk full         | Writes rejected before WAL corruption |
| Partial WAL write | Entry discarded, recovery continues   |
| Corrupt SST       | Shard enters degraded read-only mode  |

---

### D.2 Process Failures

| Failure                 | Expected Behavior              |
|-------------------------|--------------------------------|
| Crash during write      | WAL replay restores state      |
| Crash during compaction | Old files remain authoritative |

---

### D.3 Consensus Failures

| Failure           | Expected Behavior                    |
|-------------------|--------------------------------------|
| Leader loss       | New leader elected                   |
| Network partition | Minority unavailable, no split-brain |

---

### D.4 Query & Execution Failures

| Failure              | Expected Behavior           |
|----------------------|-----------------------------|
| Query exceeds budget | Deterministic cancellation  |
| Operator panic       | Query aborted, shard intact |

---

### D.5 Index & AI Failures

| Failure                 | Expected Behavior              |
|-------------------------|--------------------------------|
| Corrupt vector index    | Index rebuilt asynchronously   |
| Embedding model removed | Reads continue, writes blocked |

---

### D.6 Backup & Restore Failures

| Failure           | Expected Behavior       |
|-------------------|-------------------------|
| Incomplete backup | Restore rejected        |
| WAL mismatch      | Explicit operator error |

---

### D.7 Embedded Mode Failures

| Failure      | Expected Behavior             |
|--------------|-------------------------------|
| Host panic   | Same guarantees as standalone |
| ABI mismatch | Startup refusal               |

---

## Closing Principle

Nanograph MUST fail **safe, explicit, and observable**. Silent corruption, partial success, or ambiguous state is always
a bug.

---
