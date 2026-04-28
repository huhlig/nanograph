# Ordered Indexes - Implementation Guide

This document provides comprehensive documentation for the ordered index implementations in Nanograph, including B-Tree and Hash indexes with persistence and distributed shard support.

## Table of Contents

1. [Overview](#overview)
2. [Architecture](#architecture)
3. [Index Types](#index-types)
4. [Usage Examples](#usage-examples)
5. [Persistence](#persistence)
6. [Distributed Replication](#distributed-replication)
7. [Performance Characteristics](#performance-characteristics)
8. [Best Practices](#best-practices)
9. [Troubleshooting](#troubleshooting)

## Overview

Nanograph provides two types of ordered indexes with full persistence and distributed shard support:

- **B-Tree Index**: For range queries, sorted scans, and prefix matching
- **Hash Index**: For O(1) exact match queries and unique constraints

Both index types support:
- ✅ Persistent storage via KeyValueShardStore
- ✅ Write-ahead logging (WAL) for crash recovery
- ✅ LRU caching for hot entries
- ✅ Distributed replication via Raft consensus
- ✅ Unique constraint enforcement
- ✅ Async/await operations

## Architecture

### Layered Architecture

```
┌─────────────────────────────────────────────────────────┐
│      Application Layer                                  │
│  - Uses IndexStore trait                                │
│  - Agnostic to index type                               │
└─────────────────────────────────────────────────────────┘
                      ↓
┌─────────────────────────────────────────────────────────┐
│      DistributedIndex (Optional)                        │
│  - Raft-based replication                               │
│  - Leader election                                      │
│  - Strong consistency                                   │
└─────────────────────────────────────────────────────────┘
                      ↓
┌─────────────────────────────────────────────────────────┐
│      Index Implementation                               │
│  ┌─────────────────┐  ┌─────────────────┐             │
│  │  BTreeIndex     │  │  HashIndex      │             │
│  │  - Range queries│  │  - O(1) lookups │             │
│  │  - Sorted scans │  │  - Unique only  │             │
│  └─────────────────┘  └─────────────────┘             │
└─────────────────────────────────────────────────────────┘
                      ↓
┌─────────────────────────────────────────────────────────┐
│      PersistentIndexStore                               │
│  - KeyValueShardStore (storage)                         │
│  - WriteAheadLog (durability)                           │
│  - LRU Cache (performance)                              │
└─────────────────────────────────────────────────────────┘
```

## Index Types

### B-Tree Index

**Use Cases:**
- Range queries (e.g., find all users with age between 25 and 35)
- Sorted scans (e.g., get top 10 highest scores)
- Prefix matching (e.g., find all emails starting with "admin")
- Min/max operations
- Ordered data access

**Traits Implemented:**
- `IndexStore` - Basic index operations
- `OrderedIndex` - Range queries and sorted access
- `UniqueIndex` - Optional unique constraint enforcement

**Example:**
```rust
use nanograph_idx::btree::BTreeIndex;
use nanograph_idx::{IndexStore, OrderedIndex};

// Create B-Tree index
let index = BTreeIndex::new(metadata, store, wal, config).await?;

// Range query
let results = index.range_scan(
    Bound::Included(b"age_25".to_vec()),
    Bound::Included(b"age_35".to_vec()),
    Some(100),  // limit
    false,      // not reversed
).await?;

// Prefix scan
let admins = index.prefix_scan(b"admin", None).await?;

// Min/max
let min_age = index.min_key().await?;
let max_age = index.max_key().await?;
```

### Hash Index

**Use Cases:**
- Unique constraints (e.g., unique email addresses)
- Fast exact match lookups (e.g., find user by email)
- Primary key alternatives
- Equality checks

**Traits Implemented:**
- `IndexStore` - Basic index operations
- `UniqueIndex` - Unique constraint enforcement

**Example:**
```rust
use nanograph_idx::hash::HashIndex;
use nanograph_idx::{IndexStore, UniqueIndex};

// Create Hash index
let index = HashIndex::new(metadata, store, wal, config).await?;

// Exact match query
let query = IndexQuery::exact(b"john@example.com".to_vec());
let results = index.query(query).await?;

// Unique lookup
let user_id = index.lookup_unique(b"john@example.com").await?;

// Validate uniqueness
index.validate_unique(b"new@example.com").await?;
```

## Usage Examples

### Creating an Index

```rust
use nanograph_idx::btree::BTreeIndex;
use nanograph_idx::PersistenceConfig;
use nanograph_core::object::{IndexRecord, IndexType, IndexStatus};
use nanograph_kvt::memory::InMemoryShardStore;
use std::sync::Arc;

// Create index metadata
let metadata = IndexRecord {
    index_id: IndexId::new(ObjectId::new(1)),
    name: "users_age_idx".to_string(),
    index_type: IndexType::Secondary,
    columns: vec!["age".to_string()],
    status: IndexStatus::Building,
    // ... other fields
};

// Create persistence configuration
let config = PersistenceConfig {
    shard_id: ShardId::from_parts(/* ... */),
    index_id: IndexId::new(ObjectId::new(1)),
    cache_size: 10000,
    durability: Durability::Flush,
    enable_wal: true,
};

// Create storage backend
let store = Arc::new(InMemoryShardStore::new());

// Optional: Create WAL for durability
let wal = Some(Arc::new(WriteAheadLogManager::new(/* ... */)));

// Create index
let mut index = BTreeIndex::new(metadata, store, wal, config).await?;
```

### Inserting Data

```rust
use nanograph_idx::IndexEntry;

// Insert single entry
let entry = IndexEntry {
    indexed_value: b"age_30".to_vec(),
    primary_key: b"user_123".to_vec(),
    included_columns: None,
};
index.insert(entry).await?;

// Insert multiple entries
for i in 0..1000 {
    let entry = IndexEntry {
        indexed_value: format!("value_{:06}", i).into_bytes(),
        primary_key: format!("key_{}", i).into_bytes(),
        included_columns: None,
    };
    index.insert(entry).await?;
}

// Flush to ensure durability
index.flush().await?;
```

### Querying Data

```rust
use nanograph_idx::IndexQuery;
use std::ops::Bound;

// Exact match
let query = IndexQuery::exact(b"age_30".to_vec());
let results = index.query(query).await?;

// Range query
let query = IndexQuery::range(
    Bound::Included(b"age_25".to_vec()),
    Bound::Excluded(b"age_40".to_vec()),
).with_limit(100);
let results = index.query(query).await?;

// Unbounded scan (all entries)
let query = IndexQuery::all().with_limit(1000);
let results = index.query(query).await?;

// Reverse scan
let query = IndexQuery::all().reversed().with_limit(10);
let results = index.query(query).await?;
```

### Building from Table Data

```rust
// Build index from existing table data
let table_data = vec![
    (b"key1".to_vec(), b"row_data_1".to_vec()),
    (b"key2".to_vec(), b"row_data_2".to_vec()),
    // ... more rows
];

index.build(table_data.into_iter()).await?;
```

## Persistence

### Storage Layer

Both index types use `PersistentIndexStore` which provides:

1. **Durable Storage**: All index entries are persisted to KeyValueShardStore
2. **Write-Ahead Logging**: Optional WAL for crash recovery
3. **LRU Caching**: Hot entries cached in memory for performance
4. **Serialization**: Efficient binary serialization with versioning

### Key Format

Index entries are stored with a composite key:
```
[index_id:8 bytes][indexed_value:N bytes][primary_key:M bytes]
```

This format enables:
- Efficient range scans by indexed value
- Isolation between different indexes
- Support for composite keys

### Crash Recovery

When WAL is enabled:

1. All write operations are logged before being applied
2. On crash, the WAL is replayed to restore state
3. Periodic checkpoints reduce recovery time
4. Automatic cleanup of old WAL entries

```rust
// Enable WAL for crash recovery
let config = PersistenceConfig {
    enable_wal: true,
    durability: Durability::Flush,  // Sync to disk
    // ... other fields
};
```

## Distributed Replication

### Using DistributedIndex

Wrap any index implementation with `DistributedIndex` for Raft-based replication:

```rust
use nanograph_idx::DistributedIndex;

// Create local index
let local_index = BTreeIndex::new(metadata, store, wal, config).await?;

// Wrap with distributed layer
let distributed_index = DistributedIndex::new(
    local_index,
    consensus_group,
);

// All writes now go through Raft consensus
distributed_index.insert(entry).await?;

// Reads can be served locally (eventual consistency)
let results = distributed_index.query(query).await?;
```

### Consistency Guarantees

- **Writes**: Strong consistency via Raft consensus
  - All writes go through the leader
  - Replicated to majority before commit
  - Linearizable semantics

- **Reads**: Configurable consistency
  - **Stale reads**: Serve from local replica (fast, eventual consistency)
  - **Consistent reads**: Read from leader (slower, strong consistency)

```rust
// Enable stale reads for better performance
let distributed_index = DistributedIndex::new(local_index, consensus_group)
    .with_stale_reads(true);
```

### Failover

- Automatic leader election on failure
- Reads continue on all replicas
- Writes blocked until new leader elected
- Typical failover time: <30 seconds

## Performance Characteristics

### B-Tree Index

| Operation | Time Complexity | Notes |
|-----------|----------------|-------|
| Insert | O(log n) | Includes persistence |
| Delete | O(log n) | Requires scan to find entry |
| Exact match | O(log n) | Binary search |
| Range scan | O(log n + k) | k = result size |
| Prefix scan | O(log n + k) | k = matching entries |
| Min/Max | O(log n) | Cached in practice |

### Hash Index

| Operation | Time Complexity | Notes |
|-----------|----------------|-------|
| Insert | O(1) | Average case |
| Delete | O(n) | Requires full scan |
| Exact match | O(1) | Average case |
| Range scan | Not supported | Use B-Tree instead |

### Cache Performance

- **Cache hit**: ~100ns (in-memory lookup)
- **Cache miss**: ~1-10ms (disk I/O + deserialization)
- **Cache size**: Configurable (default: 10,000 entries)
- **Eviction**: LRU policy

### Recommended Cache Sizes

- Small indexes (<10K entries): 1,000 entries
- Medium indexes (10K-1M entries): 10,000 entries
- Large indexes (>1M entries): 100,000 entries

## Best Practices

### Index Selection

**Use B-Tree when:**
- You need range queries
- You need sorted access
- You need prefix matching
- You need min/max operations
- You have ordered data

**Use Hash when:**
- You only need exact match queries
- You need unique constraints
- You need O(1) lookups
- You don't need range queries

### Performance Optimization

1. **Choose appropriate cache size**
   ```rust
   let config = PersistenceConfig {
       cache_size: 100_000,  // Larger for hot indexes
       // ...
   };
   ```

2. **Batch inserts when possible**
   ```rust
   for entry in entries {
       index.insert(entry).await?;
   }
   index.flush().await?;  // Flush once at end
   ```

3. **Use limits on queries**
   ```rust
   let query = IndexQuery::all().with_limit(100);
   ```

4. **Enable stale reads for read-heavy workloads**
   ```rust
   let index = DistributedIndex::new(local, consensus)
       .with_stale_reads(true);
   ```

### Maintenance

1. **Regular optimization**
   ```rust
   // Periodically optimize indexes
   index.optimize().await?;
   ```

2. **Monitor statistics**
   ```rust
   let stats = index.stats().await?;
   println!("Entries: {}", stats.entry_count);
   println!("Size: {} bytes", stats.size_bytes);
   ```

3. **Rebuild if fragmented**
   ```rust
   if stats.fragmentation.unwrap_or(0.0) > 0.5 {
       // Rebuild index
       index.build(table_data).await?;
   }
   ```

## Troubleshooting

### Common Issues

**Issue: Slow queries**
- Check cache hit rate
- Increase cache size
- Add appropriate indexes
- Use query limits

**Issue: High memory usage**
- Reduce cache size
- Enable compression
- Use smaller included columns

**Issue: Unique constraint violations**
- Check for duplicate data
- Verify key extraction logic
- Review concurrent insert patterns

**Issue: Replication lag**
- Check network latency
- Tune Raft parameters
- Consider read replicas
- Enable stale reads

### Debugging

Enable detailed logging:
```rust
use tracing::Level;

tracing_subscriber::fmt()
    .with_max_level(Level::DEBUG)
    .init();
```

Check index statistics:
```rust
let stats = index.stats().await?;
println!("Index stats: {:?}", stats);

let cache_stats = storage.cache_stats();
println!("Cache stats: {:?}", cache_stats);
```

### Getting Help

- Check the [API documentation](https://docs.rs/nanograph-idx)
- Review [examples](../examples/)
- File an issue on [GitHub](https://github.com/huhlig/nanograph)

## License

Licensed under the Apache License, Version 2.0.

---

**Made with Bob**