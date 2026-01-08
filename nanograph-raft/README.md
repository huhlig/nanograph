# nanograph-raft

Raft-based distributed consensus layer for Nanograph.

## Overview

This crate provides the distributed consensus foundation for Nanograph, enabling multi-node deployment with strong consistency guarantees. It implements a **shard-per-Raft-group** architecture where each data shard is an independent Raft consensus group.

## Architecture

### Key Design Decisions

1. **Shard-per-Raft-group**: Each shard is an independent Raft group with its own leader election and log replication
2. **Metadata Raft group**: Separate Raft group manages cluster metadata (node membership, shard assignments)
3. **Hash-based partitioning**: Keys are routed to shards using consistent hashing
4. **Configurable replication**: Typically 3 or 5 replicas per shard for fault tolerance

### Components

```
┌─────────────────────────────────────────────────────────┐
│                    Nanograph Cluster                     │
├─────────────────────────────────────────────────────────┤
│                                                          │
│  ┌──────────────────────────────────────────────────┐  │
│  │         Metadata Raft Group (All Nodes)          │  │
│  │  - Cluster configuration                          │  │
│  │  - Shard assignments                              │  │
│  │  - Node membership                                │  │
│  └──────────────────────────────────────────────────┘  │
│                         │                                │
│         ┌───────────────┼───────────────┐               │
│         ▼               ▼               ▼               │
│  ┌──────────────┐ ┌──────────────┐ ┌──────────────┐   │
│  │   Node 1     │ │   Node 2     │ │   Node 3     │   │
│  ├──────────────┤ ├──────────────┤ ├──────────────┤   │
│  │ Shard 0 (L)  │ │ Shard 0 (F)  │ │ Shard 0 (F)  │   │
│  │ Shard 1 (F)  │ │ Shard 1 (L)  │ │ Shard 1 (F)  │   │
│  │ Shard 2 (F)  │ │ Shard 2 (F)  │ │ Shard 2 (L)  │   │
│  └──────────────┘ └──────────────┘ └──────────────┘   │
│                                                          │
│  L = Leader, F = Follower                               │
└─────────────────────────────────────────────────────────┘
```

## Core Types

### Router

The main entry point for distributed operations. Routes requests to the correct shard.

```rust
use nanograph_raft::{Router, ReplicationConfig, NodeId};

let config = ReplicationConfig::default();
let router = Router::new(NodeId::new(1), config);

// Add shards
router.add_shard(shard_id, storage, peers).await?;

// Perform operations
router.put(b"key".to_vec(), b"value".to_vec()).await?;
let value = router.get(b"key").await?;
```

### ShardRaftGroup

Manages Raft consensus for a single shard.

```rust
use nanograph_raft::{ShardRaftGroup, NodeId, Operation};

let shard_group = ShardRaftGroup::new(
    shard_id,
    local_node_id,
    storage_adapter,
    peers,
    config,
);

// Propose a write
let op = Operation::Put {
    key: b"key".to_vec(),
    value: b"value".to_vec(),
};
shard_group.propose_write(op).await?;

// Read with consistency level
let value = shard_group.read(b"key", ReadConsistency::Linearizable).await?;
```

### MetadataRaftGroup

Manages cluster metadata via Raft consensus.

```rust
use nanograph_raft::{MetadataRaftGroup, NodeInfo};

let metadata_group = MetadataRaftGroup::new(local_node_id);

// Add a node
metadata_group.add_node(node_info).await?;

// Update shard assignment
metadata_group.update_shard_assignment(shard_id, replicas).await?;

// Get current metadata
let metadata = metadata_group.get_metadata().await;
```

## Read Consistency Levels

Three consistency levels are supported:

1. **Linearizable** (default): Strongest consistency, requires quorum
   - Uses ReadIndex protocol
   - Guarantees reading committed data
   - Adds ~1 RTT latency

2. **Lease**: Fast leader reads with clock synchronization
   - Leader serves reads without quorum if it has a valid lease
   - Requires NTP for clock sync
   - Sub-millisecond latency

3. **Follower**: Fastest, potentially stale
   - Read from any replica without consistency guarantees
   - Useful for analytics or non-critical reads
   - Microsecond latency

```rust
use nanograph_raft::ReadConsistency;

// Linearizable (strongest)
let value = router.get_with_consistency(key, ReadConsistency::Linearizable).await?;

// Lease-based (fast)
let value = router.get_with_consistency(key, ReadConsistency::Lease).await?;

// Follower (fastest, potentially stale)
let value = router.get_with_consistency(key, ReadConsistency::Follower).await?;
```

## Replication Configuration

```rust
use nanograph_raft::ReplicationConfig;

let config = ReplicationConfig {
    replication_factor: 3,        // 3 replicas per shard
    min_sync_replicas: 2,         // Quorum = 2
    election_timeout_ms: 1000,    // 1 second
    heartbeat_interval_ms: 100,   // 100ms
    max_append_entries: 100,      // Batch size
    snapshot_threshold: 10000,    // Snapshot after 10k entries
};

// Calculate quorum
assert_eq!(config.quorum_size(), 2);

// Calculate tolerable failures
assert_eq!(config.tolerable_failures(), 1);
```

## Integration with Storage Engines

The `RaftStorageAdapter` bridges openraft with Nanograph's `KeyValueStore` trait:

```rust
use nanograph_raft::RaftStorageAdapter;
use nanograph_lsm::LSMTreeEngine;

// Create storage engine
let storage = Box::new(LSMTreeEngine::new(options)?);

// Wrap in Raft adapter
let adapter = Arc::new(RaftStorageAdapter::new(storage, shard_id));

// Use in Raft group
let shard_group = ShardRaftGroup::new(
    shard_id,
    local_node_id,
    adapter,
    peers,
    config,
);
```

## Wiring into Architecture

### Phase 1: Single-Node (Current)

```rust
// Single-node mode (no Raft)
let storage = LSMTreeEngine::new(options)?;
storage.put(key, value).await?;
```

### Phase 2: Distributed (Target)

```rust
// Multi-node mode with Raft
let router = Router::new(node_id, config);

// Add local shards
for shard_id in local_shards {
    let storage = LSMTreeEngine::new(options)?;
    router.add_shard(shard_id, Box::new(storage), peers).await?;
}

// Operations now go through Raft
router.put(key, value).await?;
```

### Integration Points

1. **nanograph-lsm**: LSM storage engine implements `KeyValueStore` trait
2. **nanograph-btree**: B+Tree storage engine implements `KeyValueStore` trait
3. **nanograph-wal**: WAL backs Raft log for durability
4. **nanograph-kvt**: Common traits and types for all storage engines

## Current Status

**Phase 2 Implementation Status:**

- [x] Core types defined (`types.rs`)
- [x] Error handling (`error.rs`)
- [x] Storage adapter (`storage.rs`)
- [x] Shard Raft group (`shard_group.rs`)
- [x] Metadata Raft group (`metadata.rs`)
- [x] Router for distributed ops (`router.rs`)
- [ ] Full openraft integration (pending)
- [ ] Network layer (pending)
- [ ] Snapshot transfer (pending)
- [ ] Membership changes (pending)

## Next Steps

1. **Integrate openraft library** (Week 9)
   - Implement full `RaftStorage` trait
   - Add network transport layer
   - Implement snapshot streaming

2. **Testing** (Week 10-11)
   - Unit tests for each component
   - Integration tests for multi-node scenarios
   - Jepsen-style linearizability tests

3. **Rebalancing** (Week 12-14)
   - Shard migration logic
   - Load balancing
   - Graceful node removal

## References

- [ADR-0007: Clustering, Sharding, Replication, and Consensus](../docs/ADR/ADR-0007-Clustering-Sharding-Replication-Consensus.md)
- [openraft documentation](https://docs.rs/openraft/)
- [Raft paper](https://raft.github.io/raft.pdf)

## License

Apache License 2.0 - See LICENSE.md for details