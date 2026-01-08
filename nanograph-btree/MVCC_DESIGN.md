# MVCC Snapshot Isolation Design for nanograph-btree

## Overview
This document describes the Multi-Version Concurrency Control (MVCC) implementation for nanograph-btree to provide snapshot isolation.

## Architecture

### 1. Versioned Values
Each key in a leaf node maintains a version chain of values:

```rust
pub struct VersionedValue {
    value: Option<Vec<u8>>,  // None = deletion marker
    created_ts: u64,          // Transaction ID that created this version
    commit_ts: u64,           // Timestamp when committed (0 = uncommitted)
}
```

### 2. Leaf Node Structure
```rust
pub struct LeafNode {
    id: NodeId,
    entries: Vec<(Vec<u8>, Vec<VersionedValue>)>,  // Key -> version chain
    next: Option<NodeId>,
    prev: Option<NodeId>,
    parent: Option<NodeId>,
}
```

### 3. Transaction Timestamps
- **Transaction ID**: Unique monotonic counter, used as `created_ts`
- **Snapshot Timestamp**: Timestamp when transaction began, used for reads
- **Commit Timestamp**: Timestamp when transaction commits

### 4. Visibility Rules
A version is visible to a transaction if:
1. `commit_ts > 0` (version is committed)
2. `commit_ts <= snapshot_ts` (committed before transaction started)

### 5. Write Operations
- **Insert/Update**: Add new version with `created_ts = tx_id`, `commit_ts = 0`
- **Delete**: Add version with `value = None`, `created_ts = tx_id`, `commit_ts = 0`
- **Commit**: Set `commit_ts` for all versions with matching `created_ts`
- **Rollback**: Remove all versions with matching `created_ts`

### 6. Read Operations
- Find first version in chain where `is_visible(snapshot_ts)` returns true
- If version has `value = None`, key is deleted (return None)

### 7. Conflict Detection
Before commit, check if any key being written has been modified:
- Get latest committed version for each key
- If `latest.commit_ts > snapshot_ts`, abort with write conflict

### 8. Garbage Collection
Periodically clean old versions:
- Track minimum active snapshot timestamp across all transactions
- Remove versions where `commit_ts < min_snapshot_ts` (keep at least one)
- Limit version chain length per key

## Implementation Steps

1. ✅ Add `VersionedValue` struct to node.rs
2. ✅ Modify `LeafNode` to use version chains
3. ✅ Update `LeafNode` methods (insert, remove, get)
4. ✅ Add version management methods (commit, rollback, gc)
5. ✅ Update `BPlusTreeNode` wrapper methods
6. ✅ Update `BPlusTree` methods to accept timestamps
7. ✅ Update transaction layer to use timestamps
8. ✅ Implement write conflict detection
9. ✅ Add garbage collection
10. ✅ Update all tests
11. ✅ Add snapshot isolation tests

## Testing Strategy

### Unit Tests
- Version visibility logic
- Version chain management
- Commit/rollback operations
- Garbage collection

### Integration Tests
- Concurrent transactions
- Write conflict detection
- Phantom read prevention
- Snapshot isolation guarantees

### Performance Tests
- Version chain overhead
- Garbage collection impact
- Concurrent transaction throughput