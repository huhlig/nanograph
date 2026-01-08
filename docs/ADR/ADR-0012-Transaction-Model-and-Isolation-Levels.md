---
parent: ADR
nav_order: 0012
title: Transaction Model and Isolation Levels
status: accepted
date: 2026-01-05
deciders: Hans W. Uhlig
---

# ADR-0012: Transaction Model and Isolation Levels

## Status

Accepted

## Context

Nanograph must provide ACID transaction guarantees while balancing:

1. **Correctness** - Prevent data corruption and anomalies
2. **Performance** - Minimize transaction overhead
3. **Scalability** - Support distributed operations
4. **Complexity** - Keep implementation manageable
5. **Usability** - Provide intuitive transaction semantics

Traditional transaction models face trade-offs:
- **Serializable** - Strongest guarantees but poor performance
- **Snapshot Isolation** - Good balance but allows write skew
- **Read Committed** - Weak guarantees, allows non-repeatable reads
- **Distributed 2PC** - Strong cross-shard atomicity but complex and slow

The challenge is providing strong guarantees for common cases while maintaining acceptable performance in distributed scenarios.

## Decision

Implement a **tiered transaction model**:

1. **Single-shard ACID transactions** with Snapshot Isolation (SI)
   - Full ACID guarantees within a shard
   - Linearizable writes via Raft
   - Efficient implementation

2. **Best-effort multi-shard transactions**
   - Coordinated writes with explicit failure handling
   - Application-visible partial failures
   - No hidden distributed transactions

3. **Future: Optional 2PC for cross-shard atomicity**
   - Opt-in for applications requiring it

## Architecture

### MVCC (Multi-Version Concurrency Control) Overview

```
Timeline of Versions:

Key: "user:123"

T1: ┌──────────────────────────────────┐
    │ Value: "Alice"                   │
    │ Version: 1                       │
    │ Created: T1                      │
    │ Deleted: ∞                       │
    └──────────────────────────────────┘

T5: ┌──────────────────────────────────┐
    │ Value: "Alice Smith"             │
    │ Version: 5                       │
    │ Created: T5                      │
    │ Deleted: ∞                       │
    └──────────────────────────────────┘
    ┌──────────────────────────────────┐
    │ Value: "Alice"                   │
    │ Version: 1                       │
    │ Created: T1                      │
    │ Deleted: T5                      │ ← Marked as deleted
    └──────────────────────────────────┘

T9: ┌──────────────────────────────────┐
    │ Value: "Alice Johnson"           │
    │ Version: 9                       │
    │ Created: T9                      │
    │ Deleted: ∞                       │
    └──────────────────────────────────┘
    ┌──────────────────────────────────┐
    │ Value: "Alice Smith"             │
    │ Version: 5                       │
    │ Created: T5                      │
    │ Deleted: T9                      │
    └──────────────────────────────────┘
    ┌──────────────────────────────────┐
    │ Value: "Alice"                   │
    │ Version: 1                       │
    │ Created: T1                      │
    │ Deleted: T5                      │
    └──────────────────────────────────┘

Readers at different timestamps see different versions:
• Reader at T3: sees "Alice" (version 1)
• Reader at T7: sees "Alice Smith" (version 5)
• Reader at T10: sees "Alice Johnson" (version 9)
```

### Transaction Lifecycle

```
┌─────────────────────────────────────────────────────────────┐
│                    BEGIN TRANSACTION                         │
└─────────────────────────────────────────────────────────────┘
                            │
                            ▼
                ┌───────────────────────┐
                │  Allocate Timestamp   │
                │  (snapshot_ts)        │
                └───────────────────────┘
                            │
                            ▼
                ┌───────────────────────┐
                │  Read Phase           │
                │  • Read at snapshot_ts│
                │  • See consistent view│
                │  • Track read set     │
                └───────────────────────┘
                            │
                            ▼
                ┌───────────────────────┐
                │  Write Phase          │
                │  • Buffer writes      │
                │  • Track write set    │
                │  • No visibility yet  │
                └───────────────────────┘
                            │
                            ▼
                ┌───────────────────────┐
                │  COMMIT               │
                └───────────────────────┘
                            │
                ┌───────────┴───────────┐
                ▼                       ▼
    ┌───────────────────┐   ┌───────────────────┐
    │ Validation Phase  │   │  Write to WAL     │
    │ • Check conflicts │   │  (durability)     │
    │ • Allocate        │   └───────────────────┘
    │   commit_ts       │               │
    └───────────────────┘               │
                │                       │
                ├───────────────────────┘
                │
        ┌───────┴───────┐
        ▼               ▼
┌──────────────┐  ┌──────────────┐
│   SUCCESS    │  │    ABORT     │
│              │  │  (conflict)  │
│ • Apply      │  │              │
│   writes     │  │ • Rollback   │
│ • Release    │  │ • Retry      │
│   locks      │  └──────────────┘
└──────────────┘
```

### Snapshot Isolation Example

```
Timeline:

T0: Initial state: balance = 100

T1: ┌─────────────────────────────────┐
    │ Transaction A begins            │
    │ snapshot_ts = 1                 │
    └─────────────────────────────────┘

T2: ┌─────────────────────────────────┐
    │ Transaction B begins            │
    │ snapshot_ts = 2                 │
    └─────────────────────────────────┘

T3: ┌─────────────────────────────────┐
    │ A: READ balance → 100           │
    │ (sees version at T0)            │
    └─────────────────────────────────┘

T4: ┌─────────────────────────────────┐
    │ B: READ balance → 100           │
    │ (sees version at T0)            │
    └─────────────────────────────────┘

T5: ┌─────────────────────────────────┐
    │ A: WRITE balance = 150          │
    │ (buffered, not visible)         │
    └─────────────────────────────────┘

T6: ┌─────────────────────────────────┐
    │ B: WRITE balance = 200          │
    │ (buffered, not visible)         │
    └─────────────────────────────────┘

T7: ┌─────────────────────────────────┐
    │ A: COMMIT                       │
    │ commit_ts = 7                   │
    │ ✓ No conflicts                  │
    │ balance = 150 (visible)         │
    └─────────────────────────────────┘

T8: ┌─────────────────────────────────┐
    │ B: COMMIT                       │
    │ ✗ Conflict detected!            │
    │ (balance modified at T7)        │
    │ ABORT and RETRY                 │
    └─────────────────────────────────┘

Final state: balance = 150
```

### Conflict Detection

```
Write-Write Conflict:

Transaction A:                Transaction B:
┌──────────────┐             ┌──────────────┐
│ BEGIN (T1)   │             │ BEGIN (T2)   │
└──────────────┘             └──────────────┘
       │                            │
       ▼                            ▼
┌──────────────┐             ┌──────────────┐
│ READ key1    │             │ READ key1    │
│ value = 10   │             │ value = 10   │
└──────────────┘             └──────────────┘
       │                            │
       ▼                            ▼
┌──────────────┐             ┌──────────────┐
│ WRITE key1   │             │ WRITE key1   │
│ value = 20   │             │ value = 30   │
└──────────────┘             └──────────────┘
       │                            │
       ▼                            │
┌──────────────┐                    │
│ COMMIT (T5)  │                    │
│ ✓ SUCCESS    │                    │
└──────────────┘                    │
                                    ▼
                             ┌──────────────┐
                             │ COMMIT (T6)  │
                             │ ✗ CONFLICT!  │
                             │ (key1 changed│
                             │  at T5)      │
                             └──────────────┘

Conflict Check:
• B's snapshot_ts (T2) < A's commit_ts (T5)
• B wrote to key1, which A also wrote to
• B must abort and retry
```

### Single-Shard Transaction Flow

```
┌─────────────────────────────────────────────────────────────┐
│                         Client                               │
└─────────────────────────────────────────────────────────────┘
                            │
                            │ 1. BEGIN
                            ▼
                ┌───────────────────────┐
                │   Shard Leader        │
                │   (Raft Leader)       │
                └───────────────────────┘
                            │
                            │ 2. Allocate snapshot_ts
                            ▼
                ┌───────────────────────┐
                │   Read Operations     │
                │   • Local reads       │
                │   • Consistent view   │
                └───────────────────────┘
                            │
                            │ 3. Buffer writes
                            ▼
                ┌───────────────────────┐
                │   Write Buffer        │
                │   (in-memory)         │
                └───────────────────────┘
                            │
                            │ 4. COMMIT
                            ▼
                ┌───────────────────────┐
                │   Validation          │
                │   • Check conflicts   │
                │   • Allocate commit_ts│
                └───────────────────────┘
                            │
                            │ 5. Propose to Raft
                            ▼
                ┌───────────────────────┐
                │   Raft Consensus      │
                │   • Replicate to      │
                │     followers         │
                │   • Wait for quorum   │
                └───────────────────────┘
                            │
                            │ 6. Apply to state machine
                            ▼
                ┌───────────────────────┐
                │   Storage Engine      │
                │   • Write to memtable │
                │   • Update indexes    │
                └───────────────────────┘
                            │
                            │ 7. Return success
                            ▼
                ┌───────────────────────┐
                │   Client              │
                └───────────────────────┘
```

### Multi-Shard Transaction Flow (Best-Effort)

```
┌─────────────────────────────────────────────────────────────┐
│                         Client                               │
└─────────────────────────────────────────────────────────────┘
                            │
                            │ 1. BEGIN
                            ▼
                ┌───────────────────────┐
                │   Coordinator         │
                │   (client-side)       │
                └───────────────────────┘
                            │
                            │ 2. Route operations
                ┌───────────┴───────────┐
                ▼                       ▼
        ┌──────────────┐        ┌──────────────┐
        │  Shard A     │        │  Shard B     │
        │  (Leader)    │        │  (Leader)    │
        └──────────────┘        └──────────────┘
                │                       │
                │ 3. Local transactions │
                ▼                       ▼
        ┌──────────────┐        ┌──────────────┐
        │  Read/Write  │        │  Read/Write  │
        │  Operations  │        │  Operations  │
        └──────────────┘        └──────────────┘
                │                       │
                │ 4. COMMIT             │
                ▼                       ▼
        ┌──────────────┐        ┌──────────────┐
        │  Commit A    │        │  Commit B    │
        │  ✓ Success   │        │  ✗ Failure   │
        └──────────────┘        └──────────────┘
                │                       │
                └───────────┬───────────┘
                            ▼
                ┌───────────────────────┐
                │   Partial Failure!    │
                │                       │
                │   Options:            │
                │   1. Compensate (undo │
                │      Shard A)         │
                │   2. Retry Shard B    │
                │   3. Return error to  │
                │      application      │
                └───────────────────────┘
```

### Isolation Level Comparison

```
Anomaly Prevention:

                    Read      Phantom   Non-       Write
                    Uncommit  Reads     Repeatable Skew
                                        Reads

Read Uncommitted    ✗         ✗         ✗          ✗
Read Committed      ✓         ✗         ✗          ✗
Repeatable Read     ✓         ✗         ✓          ✗
Snapshot Isolation  ✓         ✓         ✓          ✗
Serializable        ✓         ✓         ✓          ✓

Nanograph Default: Snapshot Isolation
• Prevents most anomalies
• Good performance
• Write skew possible (rare in practice)
```

### Write Skew Example (Allowed in SI)

```
Scenario: Two doctors on-call, at least one must be available

Initial state:
• Doctor A: on_call = true
• Doctor B: on_call = true

Transaction 1:                Transaction 2:
┌──────────────────┐         ┌──────────────────┐
│ BEGIN            │         │ BEGIN            │
└──────────────────┘         └──────────────────┘
        │                            │
        ▼                            ▼
┌──────────────────┐         ┌──────────────────┐
│ READ A: on_call  │         │ READ B: on_call  │
│ → true           │         │ → true           │
│ READ B: on_call  │         │ READ A: on_call  │
│ → true           │         │ → true           │
└──────────────────┘         └──────────────────┘
        │                            │
        │ Check: B is on_call        │ Check: A is on_call
        │ → OK to go off-call        │ → OK to go off-call
        ▼                            ▼
┌──────────────────┐         ┌──────────────────┐
│ WRITE A:         │         │ WRITE B:         │
│ on_call = false  │         │ on_call = false  │
└──────────────────┘         └──────────────────┘
        │                            │
        ▼                            ▼
┌──────────────────┐         ┌──────────────────┐
│ COMMIT           │         │ COMMIT           │
│ ✓ SUCCESS        │         │ ✓ SUCCESS        │
└──────────────────┘         └──────────────────┘

Final state:
• Doctor A: on_call = false
• Doctor B: on_call = false
• ✗ Constraint violated! (No doctor on-call)

This is write skew - both transactions succeed but
violate a constraint that depends on multiple rows.

Solution: Use explicit locking or serializable isolation
for such cases.
```

   - Explicit performance trade-off

## Decision Drivers

* **Predictable performance** - Single-shard transactions are fast
* **Raft alignment** - Leverages existing consensus for consistency
* **Simpler failure handling** - No hidden distributed transaction failures
* **Incremental complexity** - Start simple, add features as needed
* **Common case optimization** - Most transactions are single-shard
* **Explicit trade-offs** - Applications choose their consistency level

## Design

### 1. Single-Shard Transactions

#### Transaction Lifecycle

```rust
pub struct Transaction {
    shard_id: ShardId,
    start_timestamp: Timestamp,
    isolation: IsolationLevel,
    read_set: HashSet<Vec<u8>>,
    write_set: HashMap<Vec<u8>, WriteOp>,
    status: TxStatus,
}

enum TxStatus {
    Active,
    Preparing,
    Committed,
    Aborted,
}

enum WriteOp {
    Put(Vec<u8>),
    Delete,
}

impl Transaction {
    pub async fn begin(shard_id: ShardId, options: TxOptions) -> Result<Self> {
        let start_timestamp = get_current_timestamp().await?;
        
        Ok(Transaction {
            shard_id,
            start_timestamp,
            isolation: options.isolation,
            read_set: HashSet::new(),
            write_set: HashMap::new(),
            status: TxStatus::Active,
        })
    }
    
    pub async fn get(&mut self, key: &[u8]) -> Result<Option<Vec<u8>>> {
        // Check write set first
        if let Some(op) = self.write_set.get(key) {
            return Ok(match op {
                WriteOp::Put(value) => Some(value.clone()),
                WriteOp::Delete => None,
            });
        }
        
        // Read from snapshot
        let value = self.read_from_snapshot(key).await?;
        
        // Track read for conflict detection
        self.read_set.insert(key.to_vec());
        
        Ok(value)
    }
    
    pub fn put(&mut self, key: &[u8], value: &[u8]) -> Result<()> {
        if self.status != TxStatus::Active {
            return Err(Error::TransactionNotActive);
        }
        
        self.write_set.insert(key.to_vec(), WriteOp::Put(value.to_vec()));
        Ok(())
    }
    
    pub fn delete(&mut self, key: &[u8]) -> Result<()> {
        if self.status != TxStatus::Active {
            return Err(Error::TransactionNotActive);
        }
        
        self.write_set.insert(key.to_vec(), WriteOp::Delete);
        Ok(())
    }
    
    pub async fn commit(mut self) -> Result<CommitResult> {
        self.status = TxStatus::Preparing;
        
        // Validate no conflicts
        self.check_conflicts().await?;
        
        // Propose writes to Raft
        let commit_timestamp = self.propose_writes().await?;
        
        self.status = TxStatus::Committed;
        
        Ok(CommitResult {
            timestamp: commit_timestamp,
            shard_id: self.shard_id,
        })
    }
    
    pub async fn rollback(mut self) -> Result<()> {
        self.status = TxStatus::Aborted;
        self.write_set.clear();
        Ok(())
    }
}
```

#### Snapshot Isolation Implementation

```rust
struct SnapshotIsolation {
    mvcc: MultiVersionConcurrencyControl,
}

impl SnapshotIsolation {
    async fn read_from_snapshot(&self, key: &[u8], timestamp: Timestamp) -> Result<Option<Vec<u8>>> {
        // Find latest version <= timestamp
        self.mvcc.get_version(key, timestamp).await
    }
    
    async fn check_conflicts(&self, tx: &Transaction) -> Result<()> {
        // Check for write-write conflicts
        for key in tx.write_set.keys() {
            let latest_version = self.mvcc.get_latest_version(key).await?;
            
            if latest_version.timestamp > tx.start_timestamp {
                // Another transaction modified this key
                return Err(Error::WriteConflict {
                    key: key.clone(),
                    our_timestamp: tx.start_timestamp,
                    conflict_timestamp: latest_version.timestamp,
                });
            }
        }
        
        Ok(())
    }
}
```

#### Multi-Version Concurrency Control (MVCC)

```rust
struct MultiVersionConcurrencyControl {
    versions: BTreeMap<Vec<u8>, Vec<Version>>,
    gc_threshold: Timestamp,
}

struct Version {
    timestamp: Timestamp,
    value: Option<Vec<u8>>,  // None = deleted
}

impl MultiVersionConcurrencyControl {
    fn put(&mut self, key: Vec<u8>, value: Vec<u8>, timestamp: Timestamp) {
        let versions = self.versions.entry(key).or_insert_with(Vec::new);
        versions.push(Version {
            timestamp,
            value: Some(value),
        });
        
        // Keep versions sorted by timestamp
        versions.sort_by_key(|v| v.timestamp);
    }
    
    fn get_version(&self, key: &[u8], timestamp: Timestamp) -> Option<Vec<u8>> {
        let versions = self.versions.get(key)?;
        
        // Binary search for latest version <= timestamp
        let idx = versions.binary_search_by(|v| {
            if v.timestamp <= timestamp {
                std::cmp::Ordering::Less
            } else {
                std::cmp::Ordering::Greater
            }
        }).unwrap_or_else(|i| i.saturating_sub(1));
        
        versions.get(idx)?.value.clone()
    }
    
    fn garbage_collect(&mut self, safe_timestamp: Timestamp) {
        // Remove versions older than safe_timestamp
        for versions in self.versions.values_mut() {
            versions.retain(|v| v.timestamp >= safe_timestamp);
        }
        
        // Remove empty entries
        self.versions.retain(|_, versions| !versions.is_empty());
    }
}
```

### 2. Isolation Levels

```rust
pub enum IsolationLevel {
    /// Snapshot Isolation - default
    /// - Reads see consistent snapshot
    /// - Prevents dirty reads, non-repeatable reads, phantom reads
    /// - Allows write skew
    SnapshotIsolation,
    
    /// Read Committed
    /// - Each read sees latest committed value
    /// - Prevents dirty reads
    /// - Allows non-repeatable reads and phantom reads
    ReadCommitted,
    
    /// Serializable (future)
    /// - Strongest isolation
    /// - Prevents all anomalies including write skew
    /// - Requires additional conflict detection
    Serializable,
}

impl Transaction {
    async fn read_with_isolation(&mut self, key: &[u8]) -> Result<Option<Vec<u8>>> {
        match self.isolation {
            IsolationLevel::SnapshotIsolation => {
                // Read from snapshot at start_timestamp
                self.read_from_snapshot(key).await
            }
            IsolationLevel::ReadCommitted => {
                // Read latest committed value
                self.read_latest_committed(key).await
            }
            IsolationLevel::Serializable => {
                // Read from snapshot + track for serialization graph
                let value = self.read_from_snapshot(key).await?;
                self.track_read_for_serialization(key);
                Ok(value)
            }
        }
    }
}
```

### 3. Multi-Shard Transactions

#### Best-Effort Coordination

```rust
pub struct MultiShardTransaction {
    shards: HashMap<ShardId, Transaction>,
    coordinator: ShardId,
    status: MultiShardTxStatus,
}

enum MultiShardTxStatus {
    Active,
    Preparing,
    PartiallyCommitted(Vec<ShardId>),
    FullyCommitted,
    Aborted,
}

impl MultiShardTransaction {
    pub async fn begin(shards: Vec<ShardId>) -> Result<Self> {
        let mut txs = HashMap::new();
        
        for shard_id in shards {
            let tx = Transaction::begin(shard_id, TxOptions::default()).await?;
            txs.insert(shard_id, tx);
        }
        
        Ok(MultiShardTransaction {
            shards: txs,
            coordinator: shards[0], // First shard is coordinator
            status: MultiShardTxStatus::Active,
        })
    }
    
    pub async fn get(&mut self, shard_id: ShardId, key: &[u8]) -> Result<Option<Vec<u8>>> {
        let tx = self.shards.get_mut(&shard_id)
            .ok_or(Error::ShardNotInTransaction)?;
        
        tx.get(key).await
    }
    
    pub fn put(&mut self, shard_id: ShardId, key: &[u8], value: &[u8]) -> Result<()> {
        let tx = self.shards.get_mut(&shard_id)
            .ok_or(Error::ShardNotInTransaction)?;
        
        tx.put(key, value)
    }
    
    pub async fn commit(mut self) -> Result<MultiShardCommitResult> {
        self.status = MultiShardTxStatus::Preparing;
        
        let mut committed_shards = Vec::new();
        let mut failed_shards = Vec::new();
        
        // Attempt to commit each shard
        for (shard_id, tx) in self.shards {
            match tx.commit().await {
                Ok(_) => committed_shards.push(shard_id),
                Err(e) => {
                    failed_shards.push((shard_id, e));
                    // Continue trying other shards
                }
            }
        }
        
        if failed_shards.is_empty() {
            self.status = MultiShardTxStatus::FullyCommitted;
            Ok(MultiShardCommitResult::Success {
                committed_shards,
            })
        } else {
            self.status = MultiShardTxStatus::PartiallyCommitted(committed_shards.clone());
            Ok(MultiShardCommitResult::PartialFailure {
                committed_shards,
                failed_shards,
            })
        }
    }
}

pub enum MultiShardCommitResult {
    Success {
        committed_shards: Vec<ShardId>,
    },
    PartialFailure {
        committed_shards: Vec<ShardId>,
        failed_shards: Vec<(ShardId, Error)>,
    },
}
```

#### Compensating Transactions

```rust
impl MultiShardTransaction {
    pub async fn commit_with_compensation(
        mut self,
        compensation_fn: impl Fn(ShardId) -> CompensationAction
    ) -> Result<()> {
        let result = self.commit().await?;
        
        match result {
            MultiShardCommitResult::Success { .. } => Ok(()),
            MultiShardCommitResult::PartialFailure { committed_shards, failed_shards } => {
                // Execute compensation for committed shards
                for shard_id in committed_shards {
                    let action = compensation_fn(shard_id);
                    action.execute().await?;
                }
                
                Err(Error::PartialCommit { failed_shards })
            }
        }
    }
}

pub trait CompensationAction {
    async fn execute(&self) -> Result<()>;
}
```

### 4. Transaction Options

```rust
pub struct TxOptions {
    pub isolation: IsolationLevel,
    pub timeout: Option<Duration>,
    pub read_only: bool,
    pub retry_on_conflict: bool,
    pub max_retries: usize,
}

impl Default for TxOptions {
    fn default() -> Self {
        TxOptions {
            isolation: IsolationLevel::SnapshotIsolation,
            timeout: Some(Duration::from_secs(30)),
            read_only: false,
            retry_on_conflict: true,
            max_retries: 3,
        }
    }
}
```

### 5. Automatic Retry

```rust
pub async fn with_transaction<F, R>(
    shard_id: ShardId,
    options: TxOptions,
    f: F
) -> Result<R>
where
    F: Fn(&mut Transaction) -> BoxFuture<'_, Result<R>>,
{
    let mut attempts = 0;
    let max_attempts = if options.retry_on_conflict {
        options.max_retries + 1
    } else {
        1
    };
    
    loop {
        attempts += 1;
        
        let mut tx = Transaction::begin(shard_id, options.clone()).await?;
        
        match f(&mut tx).await {
            Ok(result) => {
                match tx.commit().await {
                    Ok(_) => return Ok(result),
                    Err(Error::WriteConflict { .. }) if attempts < max_attempts => {
                        // Retry on conflict
                        tokio::time::sleep(Duration::from_millis(10 * attempts as u64)).await;
                        continue;
                    }
                    Err(e) => return Err(e),
                }
            }
            Err(e) => {
                tx.rollback().await?;
                return Err(e);
            }
        }
    }
}
```

### 6. Read-Only Transactions

```rust
pub struct ReadOnlyTransaction {
    snapshot_timestamp: Timestamp,
    shard_id: ShardId,
}

impl ReadOnlyTransaction {
    pub async fn begin(shard_id: ShardId) -> Result<Self> {
        let snapshot_timestamp = get_current_timestamp().await?;
        
        Ok(ReadOnlyTransaction {
            snapshot_timestamp,
            shard_id,
        })
    }
    
    pub async fn get(&self, key: &[u8]) -> Result<Option<Vec<u8>>> {
        // Read from snapshot, no conflict checking needed
        read_from_snapshot(self.shard_id, key, self.snapshot_timestamp).await
    }
    
    // No commit needed for read-only transactions
}
```

### 7. Deadlock Detection

```rust
struct DeadlockDetector {
    wait_graph: HashMap<TransactionId, HashSet<TransactionId>>,
}

impl DeadlockDetector {
    fn add_wait(&mut self, waiter: TransactionId, holder: TransactionId) {
        self.wait_graph.entry(waiter)
            .or_insert_with(HashSet::new)
            .insert(holder);
    }
    
    fn detect_cycle(&self) -> Option<Vec<TransactionId>> {
        // Use DFS to detect cycles
        for start_tx in self.wait_graph.keys() {
            if let Some(cycle) = self.dfs_cycle(*start_tx, &mut HashSet::new(), &mut Vec::new()) {
                return Some(cycle);
            }
        }
        None
    }
    
    fn resolve_deadlock(&mut self, cycle: Vec<TransactionId>) -> TransactionId {
        // Abort youngest transaction in cycle
        *cycle.iter().max().unwrap()
    }
}
```

## Consequences

### Positive

* **Strong guarantees for common case** - Single-shard transactions are fully ACID
* **Predictable performance** - No hidden distributed transaction overhead
* **Simple failure model** - Explicit handling of partial failures
* **Efficient implementation** - Leverages Raft for consistency
* **Flexible isolation levels** - Applications choose appropriate level
* **Automatic retry** - Handles transient conflicts transparently

### Negative

* **Cross-shard atomicity not guaranteed** - Applications must handle partial failures
* **Write skew possible** - Snapshot Isolation allows some anomalies
* **MVCC overhead** - Multiple versions consume storage
* **Garbage collection needed** - Old versions must be cleaned up

### Risks

* **Application complexity** - Multi-shard transactions require careful handling
* **Version bloat** - Long-running transactions can prevent GC
* **Conflict rate** - High contention can cause many retries

## Alternatives Considered

### 1. Global Serializable Transactions

**Rejected** - Too expensive for distributed system. Would require global lock manager or expensive validation.

### 2. Eventual Consistency

**Rejected** - Violates ACID requirements and makes application development harder.

### 3. Always Use 2PC

**Rejected** - Too slow for common single-shard case. Better as opt-in feature.

### 4. Optimistic Concurrency Control Only

**Rejected** - High conflict rates would cause excessive retries.

## Implementation Notes

### Phase 1: Single-Shard Transactions (Week 8)
- Implement MVCC
- Add Snapshot Isolation
- Create transaction API

### Phase 2: Multi-Shard Coordination (Week 14)
- Implement best-effort multi-shard transactions
- Add compensation framework
- Create examples

### Phase 3: Advanced Features (Future)
- Add Serializable isolation
- Implement 2PC for cross-shard atomicity
- Add distributed deadlock detection

## Related ADRs

* [ADR-0006: Key-Value, Document, and Graph Support](ADR-0006-Key-Value-Document-Graph-Support.md)
* [ADR-0007: Clustering, Sharding, Replication, and Consensus](ADR-0007-Clustering-Sharding-Replication-Consensus.md)
* [ADR-0013: Memory Management and Caching Strategy](ADR-0013-Memory-Management-and-Caching-Strategy.md)
* [ADR-0025: Core API Specifications](ADR-0025-Core-API-Specifications.md)

## References

* "A Critique of ANSI SQL Isolation Levels" paper
* PostgreSQL MVCC implementation
* CockroachDB transaction model
* Percolator transaction system
* Snapshot Isolation theory

---

**Next Steps:**
1. Implement MVCC storage layer
2. Add Snapshot Isolation logic
3. Create transaction API
4. Implement conflict detection
5. Add automatic retry mechanism
6. Create multi-shard coordination
