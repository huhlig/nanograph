# MVCC Design for Snapshot Isolation

## Overview
This document describes the Multi-Version Concurrency Control (MVCC) implementation for nanograph-btree and nanograph-lsm to provide true snapshot isolation.

## Key Concepts

### Snapshot Isolation
- Each transaction sees a consistent snapshot of the database as of its start time
- Transactions don't see uncommitted changes from other transactions
- Transactions don't see changes committed after they started
- Write-write conflicts are detected and prevented

### Timestamps
- **Transaction ID**: Unique identifier for each transaction
- **Snapshot Timestamp**: The logical time when the transaction started
- **Commit Timestamp**: The logical time when the transaction committed
- **Sequence Number**: Monotonically increasing number for each write operation

## Architecture

### Common Components (nanograph-kvt)
```rust
pub struct TransactionId(pub u64);
pub struct Timestamp(pub u64);

// Already defined in nanograph-kvt
```

### LSM Implementation

#### Current State
- MemTable entries already have sequence numbers
- Sequence numbers are monotonically increasing
- Need to map sequence numbers to commit timestamps

#### Changes Needed
1. **Add timestamp to Entry**
   - Store commit timestamp with each entry
   - Use sequence number as version identifier

2. **Transaction reads filter by snapshot timestamp**
   - Only return entries with commit_ts <= snapshot_ts
   - Skip entries with commit_ts > snapshot_ts

3. **Write conflict detection**
   - Check if any key was modified after snapshot_ts
   - Abort transaction if conflict detected

### B+Tree Implementation

#### Current State
- No versioning in place
- Single value per key
- Need to add version tracking

#### Changes Needed
1. **Add versioned values**
   - Store multiple versions per key
   - Each version has: value, commit_ts, deleted flag

2. **Version storage**
   ```rust
   struct VersionedValue {
       value: Option<Vec<u8>>,  // None = deleted
       commit_ts: Timestamp,
       tx_id: TransactionId,
   }
   
   // Store Vec<VersionedValue> per key
   ```

3. **Transaction reads**
   - Find the latest version where commit_ts <= snapshot_ts
   - Return that version's value

4. **Garbage collection**
   - Keep only versions needed by active transactions
   - Remove old versions when no transaction needs them

## Implementation Plan

### Phase 1: LSM Snapshot Isolation
1. Add commit_timestamp to memtable Entry
2. Update transaction.get() to filter by snapshot_ts
3. Add write conflict detection
4. Update tests

### Phase 2: B+Tree Snapshot Isolation  
1. Create VersionedValue structure
2. Modify Node to store Vec<VersionedValue> per key
3. Update get/put/delete to handle versions
4. Implement version cleanup
5. Update tests

### Phase 3: Testing
1. Test concurrent transactions
2. Test snapshot isolation guarantees
3. Test write conflict detection
4. Performance testing

## Trade-offs

### Storage Overhead
- LSM: Minimal (just timestamp per entry)
- B+Tree: Higher (multiple versions per key)

### Read Performance
- LSM: Slight overhead (filter by timestamp)
- B+Tree: Moderate overhead (find correct version)

### Write Performance
- Both: Conflict detection adds overhead
- B+Tree: Version cleanup adds overhead

### Complexity
- LSM: Lower (leverages existing sequence numbers)
- B+Tree: Higher (new versioning system)

## Future Enhancements
1. Optimistic concurrency control
2. Read-only transaction optimization
3. Long-running transaction handling
4. Configurable isolation levels