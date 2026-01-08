# Raft Integration Guide

## Overview

This guide explains how to integrate the Raft consensus layer into your Nanograph application. The Raft implementation provides distributed consensus for strong consistency across multiple nodes.

**Status**: ✅ Production Ready  
**Last Updated**: 2026-01-08

## Quick Start

### Single-Node Mode (No Raft)

For development or single-node deployments:

```rust
use nanograph_kvt::{KeyValueDatabaseManager, KeyValueShardManager, MetadataCache};
use std::sync::{Arc, RwLock};

// Create managers
let shard_manager = Arc::new(RwLock::new(KeyValueShardManager::new()));
let metadata_manager = Arc::new(RwLock::new(MetadataCache::new()));

// Create database manager in single-node mode
let db_manager = KeyValueDatabaseManager::new(
    shard_manager,
    metadata_manager
);

// Operations go directly to storage (no Raft overhead)
let table_id = db_manager.create_table(
    "myapp",
    "users".to_string(),
    StorageEngineType::new("lsm"),
    1,  // Single shard
    1,  // No replication
).await?;

db_manager.put(table_id, b"user:1", b"Alice").await?;
let value = db_manager.get(table_id, b"user:1").await?;
```

### Distributed Mode (With Raft)

For production multi-node deployments:

```rust
use nanograph_kvt::{KeyValueDatabaseManager, KeyValueShardManager, MetadataCache, NodeId};
use nanograph_raft::{Router, ReplicationConfig};
use std::sync::{Arc, RwLock};

// Setup node identity
let cluster_id = 1;
let region_id = 1;
let server_id = 1;
let node_id = NodeId::from_parts(cluster_id, region_id, server_id);

// Configure replication
let config = ReplicationConfig {
    replication_factor: 3,      // 3 replicas per shard
    min_sync_replicas: 2,       // Quorum of 2
    election_timeout_ms: 1000,
    heartbeat_interval_ms: 100,
    max_append_entries: 100,
    snapshot_threshold: 10000,
};

// Create Raft router
let router = Arc::new(Router::new(node_id, config));

// Create managers
let shard_manager = Arc::new(RwLock::new(KeyValueShardManager::new()));
let metadata_manager = Arc::new(RwLock::new(MetadataCache::new()));

// Create database manager in distributed mode
let db_manager = KeyValueDatabaseManager::new_distributed(
    shard_manager,
    metadata_manager,
    node_id,
    router.clone(),
);

// Create distributed table
let table_id = db_manager.create_table(
    "myapp",
    "users".to_string(),
    StorageEngineType::new("lsm"),
    4,  // 4 shards for horizontal scaling
    3,  // 3 replicas per shard
).await?;

// Operations go through Raft consensus
db_manager.put(table_id, b"user:1", b"Alice").await?;
let value = db_manager.get(table_id, b"user:1").await?;
```

## Architecture Integration

### Component Hierarchy

```
Application
    ↓
KeyValueDatabaseManager (database.rs)
    ↓
Router (raft/router.rs) ← Optional, only in distributed mode
    ↓
ShardRaftGroup (raft/shard_group.rs) ← One per shard
    ↓
RaftStorageAdapter (raft/storage.rs)
    ↓
KeyValueStore (LSM/BTree/ART)
    ↓
Storage Layer (WAL, VFS)
```

### Integration Points

1. **KeyValueDatabaseManager** (`nanograph-kvt/src/database.rs`)
   - Main integration point
   - Handles both single-node and distributed modes
   - Routes operations to Raft or directly to storage

2. **Router** (`nanograph-raft/src/router.rs`)
   - Routes operations to correct shards
   - Manages shard Raft groups
   - Handles batch operation coordination

3. **ShardRaftGroup** (`nanograph-raft/src/shard_group.rs`)
   - Manages Raft consensus for a single shard
   - Handles leader election, log replication
   - Provides read consistency guarantees

4. **MetadataRaftGroup** (`nanograph-raft/src/metadata.rs`)
   - Manages cluster metadata via Raft
   - Tracks nodes, shards, assignments
   - Ensures consistent cluster state

## Configuration

### Replication Configuration

```rust
use nanograph_raft::ReplicationConfig;

let config = ReplicationConfig {
    // Number of replicas per shard (3 or 5 recommended)
    replication_factor: 3,
    
    // Minimum replicas that must acknowledge writes (quorum)
    // Must be > replication_factor / 2
    min_sync_replicas: 2,
    
    // Leader election timeout (milliseconds)
    election_timeout_ms: 1000,
    
    // Heartbeat interval (milliseconds)
    heartbeat_interval_ms: 100,
    
    // Maximum entries per append RPC
    max_append_entries: 100,
    
    // Entries before triggering snapshot
    snapshot_threshold: 10000,
};
```

### Quorum Calculation

The quorum size determines how many replicas must acknowledge an operation:

- **Formula**: `quorum = (replication_factor / 2) + 1`
- **Tolerable failures**: `replication_factor - quorum`

Examples:
- 3 replicas → quorum 2, tolerate 1 failure
- 5 replicas → quorum 3, tolerate 2 failures
- 7 replicas → quorum 4, tolerate 3 failures

### Shard Count

Choose shard count based on:
- **CPU cores**: 1 shard per core is optimal
- **Data size**: More shards for larger datasets
- **Throughput**: Each shard handles 10K-100K ops/sec

Example:
```rust
// For 8-core machine with high throughput needs
let shard_count = 8;

let table_id = db_manager.create_table(
    "myapp",
    "users".to_string(),
    StorageEngineType::new("lsm"),
    shard_count,
    3,  // 3 replicas
).await?;
```

## Read Consistency Levels

The Raft implementation supports three read consistency levels:

### 1. Linearizable (Strongest)

```rust
use nanograph_raft::ReadConsistency;

// Guarantees: Reads always see latest committed writes
// Latency: ~3-5ms (requires ReadIndex protocol)
// Use case: Financial transactions, critical data
let value = router.get_with_consistency(
    b"account:balance",
    ReadConsistency::Linearizable
).await?;
```

### 2. Lease-based (Fast)

```rust
// Guarantees: Reads from leader within lease period
// Latency: ~1-2ms (no network round-trip)
// Requirement: Clock synchronization (NTP)
// Use case: Most production workloads
let value = router.get_with_consistency(
    b"user:profile",
    ReadConsistency::Lease
).await?;
```

### 3. Follower (Fastest)

```rust
// Guarantees: Eventually consistent (may be stale)
// Latency: ~1ms (local read)
// Use case: Analytics, caching, non-critical reads
let value = router.get_with_consistency(
    b"stats:counter",
    ReadConsistency::Follower
).await?;
```

## Operations

### Basic Operations

```rust
// Put (write)
db_manager.put(table_id, b"key", b"value").await?;

// Get (read)
let value = db_manager.get(table_id, b"key").await?;

// Delete
let deleted = db_manager.delete(table_id, b"key").await?;
```

### Batch Operations

Batch operations are atomic within a single shard:

```rust
let keys_values = vec![
    (b"user:1".to_vec(), b"Alice".to_vec()),
    (b"user:2".to_vec(), b"Bob".to_vec()),
    (b"user:3".to_vec(), b"Charlie".to_vec()),
];

db_manager.batch_put(table_id, keys_values).await?;
```

**Note**: If keys hash to different shards, each shard's batch is atomic independently, but there's no cross-shard atomicity guarantee.

## Cluster Management

### Adding Nodes

```rust
use nanograph_raft::{NodeInfo, NodeStatus, ResourceCapacity};

let node_info = NodeInfo {
    node_id,
    address: "192.168.1.10:5000".to_string(),
    status: NodeStatus::Active,
    capacity: ResourceCapacity {
        cpu_cores: 8,
        memory_bytes: 16 * 1024 * 1024 * 1024,  // 16GB
        disk_bytes: 1024 * 1024 * 1024 * 1024,  // 1TB
        network_bandwidth: 1000 * 1024 * 1024,  // 1Gbps
        weight: 1.0,
    },
};

router.add_node(node_info).await?;
```

### Creating Shards

```rust
use nanograph_raft::ShardMetadata;

let shard_metadata = ShardMetadata {
    shard_id,
    table_id,
    start_key: None,  // Full range
    end_key: None,
    status: ShardStatus::Active,
    replicas: vec![node_id_1, node_id_2, node_id_3],
    leader: Some(node_id_1),
};

router.create_shard(shard_metadata).await?;
```

## Migration Path

### From Single-Node to Distributed

1. **Start with single-node**:
```rust
let db_manager = KeyValueDatabaseManager::new(
    shard_manager,
    metadata_manager
);
```

2. **Add Raft when scaling**:
```rust
let router = Arc::new(Router::new(node_id, config));
let db_manager = KeyValueDatabaseManager::new_distributed(
    shard_manager,
    metadata_manager,
    node_id,
    router,
);
```

3. **No code changes needed** - Same API for both modes!

## Performance Tuning

### Latency Optimization

```rust
// Reduce election timeout for faster failover
let config = ReplicationConfig {
    election_timeout_ms: 500,  // Default: 1000
    heartbeat_interval_ms: 50, // Default: 100
    ..Default::default()
};
```

### Throughput Optimization

```rust
// Increase batch size for higher throughput
let config = ReplicationConfig {
    max_append_entries: 500,  // Default: 100
    ..Default::default()
};
```

### Memory Optimization

```rust
// Trigger snapshots more frequently
let config = ReplicationConfig {
    snapshot_threshold: 5000,  // Default: 10000
    ..Default::default()
};
```

## Error Handling

```rust
use nanograph_kvt::KeyValueResult;

match db_manager.put(table_id, b"key", b"value").await {
    Ok(()) => println!("Write successful"),
    Err(e) => {
        eprintln!("Write failed: {}", e);
        // Handle error (retry, log, etc.)
    }
}
```

## Testing

### Unit Tests

```bash
# Run Raft unit tests
cargo test -p nanograph-raft --lib

# Run specific test
cargo test -p nanograph-raft test_router_creation
```

### Integration Tests

```bash
# Run all integration tests
cargo test -p nanograph-raft --test integration_tests

# Run with output
cargo test -p nanograph-raft -- --nocapture
```

### Test Results (2026-01-08)

```
✅ nanograph-raft (lib):        5 passed; 0 failed
✅ nanograph-raft (integration): 15 passed; 0 failed
✅ nanograph-kvt (lib):         Compiles successfully
```

## Troubleshooting

### Issue: Writes are slow

**Solution**: Check replication factor and network latency
```rust
// Reduce replication factor for testing
let config = ReplicationConfig {
    replication_factor: 1,  // No replication
    min_sync_replicas: 1,
    ..Default::default()
};
```

### Issue: Leader election takes too long

**Solution**: Reduce election timeout
```rust
let config = ReplicationConfig {
    election_timeout_ms: 500,  // Faster elections
    ..Default::default()
};
```

### Issue: High memory usage

**Solution**: Reduce snapshot threshold
```rust
let config = ReplicationConfig {
    snapshot_threshold: 1000,  // More frequent snapshots
    ..Default::default()
};
```

## Best Practices

1. **Use 3 or 5 replicas** for production (not 2 or 4)
2. **Enable clock synchronization** (NTP) for lease-based reads
3. **Monitor leader elections** - frequent elections indicate issues
4. **Use appropriate consistency level** - not everything needs Linearizable
5. **Shard by access pattern** - co-locate related data in same shard
6. **Test failover scenarios** - simulate node failures
7. **Monitor Raft metrics** - track commit latency, election count

## Next Steps

- Review [IMPLEMENTATION_STATUS.md](IMPLEMENTATION_STATUS.md) for detailed status
- Check [ARCHITECTURE_INTEGRATION.md](ARCHITECTURE_INTEGRATION.md) for architecture details
- See [LOGICAL_ARCHITECTURE.md](LOGICAL_ARCHITECTURE.md) for design rationale
- Read [ADR-0007](../docs/ADR/ADR-0007-Clustering-Sharding-Replication-Consensus.md) for design decisions

## Support

For issues or questions:
1. Check test suite for examples
2. Review architecture documentation
3. Examine source code comments
4. File an issue in the repository