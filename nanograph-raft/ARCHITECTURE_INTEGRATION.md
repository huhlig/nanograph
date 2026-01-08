# Raft Architecture Integration Guide

This document describes how the Raft consensus layer integrates with the rest of Nanograph's architecture.

## System Architecture Overview

```
┌─────────────────────────────────────────────────────────────────┐
│                        Application Layer                         │
│                    (nanograph-api, SDKs)                         │
└────────────────────────────┬────────────────────────────────────┘
                             │
┌────────────────────────────▼────────────────────────────────────┐
│                      Router Layer                                │
│                   (nanograph-raft::Router)                       │
│  - Routes operations to correct shard                            │
│  - Manages shard Raft groups                                     │
│  - Coordinates with metadata                                     │
└────────────────────────────┬────────────────────────────────────┘
                             │
        ┌────────────────────┼────────────────────┐
        │                    │                    │
┌───────▼────────┐  ┌────────▼────────┐  ┌───────▼────────┐
│  Shard 0 Raft  │  │  Shard 1 Raft   │  │  Metadata Raft │
│     Group      │  │     Group       │  │     Group      │
└───────┬────────┘  └────────┬────────┘  └───────┬────────┘
        │                    │                    │
┌───────▼────────┐  ┌────────▼────────┐  ┌───────▼────────┐
│ RaftStorage    │  │ RaftStorage     │  │ RaftStorage    │
│   Adapter      │  │   Adapter       │  │   Adapter      │
└───────┬────────┘  └────────┬────────┘  └───────┬────────┘
        │                    │                    │
┌───────▼────────┐  ┌────────▼────────┐  ┌───────▼────────┐
│  LSM Engine    │  │  LSM Engine     │  │  LSM Engine    │
│ (KeyValueStore)│  │ (KeyValueStore) │  │ (KeyValueStore)│
└───────┬────────┘  └────────┬────────┘  └───────┬────────┘
        │                    │                    │
        └────────────────────┼────────────────────┘
                             │
┌────────────────────────────▼────────────────────────────────────┐
│                    Storage Layer                                 │
│              (nanograph-wal, nanograph-vfs)                      │
└─────────────────────────────────────────────────────────────────┘
```

## Integration Points

### 1. Router ↔ Application Layer

**Location**: Applications use `Router` as the main entry point

**Interface**:
```rust
// Application code
use nanograph_raft::Router;

let router = Router::new(node_id, config);

// Standard KV operations
router.put(key, value).await?;
let value = router.get(key).await?;
router.delete(key).await?;

// Batch operations
router.batch(vec![
    Operation::Put { key: k1, value: v1 },
    Operation::Put { key: k2, value: v2 },
]).await?;
```

**Responsibilities**:
- Router handles key-to-shard mapping
- Router manages Raft groups
- Router provides simple KV API to applications

### 2. Router ↔ Shard Raft Groups

**Location**: Router creates and manages shard Raft groups

**Interface**:
```rust
// Router creates shard groups
impl Router {
    pub async fn add_shard(
        &self,
        shard_id: ShardId,
        storage: Box<dyn KeyValueStore>,
        peers: Vec<NodeId>,
    ) -> Result<()> {
        let adapter = Arc::new(RaftStorageAdapter::new(storage, shard_id));
        let group = Arc::new(ShardRaftGroup::new(
            shard_id,
            self.local_node_id,
            adapter,
            peers,
            self.config.clone(),
        ));
        self.shards.write().await.insert(shard_id, group);
        Ok(())
    }
}
```

**Data Flow**:
1. Application calls `router.put(key, value)`
2. Router hashes key to determine shard
3. Router forwards to appropriate `ShardRaftGroup`
4. Shard group replicates via Raft
5. Response returned to application

### 3. Shard Raft Group ↔ Storage Adapter

**Location**: Each shard group uses a `RaftStorageAdapter`

**Interface**:
```rust
// Shard group applies operations via adapter
impl ShardRaftGroup {
    pub async fn propose_write(&self, operation: Operation) -> Result<OperationResponse> {
        // Check leadership, quorum, etc.
        
        // Apply via storage adapter
        self.storage.apply_operation(&operation).await
    }
}

// Storage adapter bridges to KeyValueStore
impl RaftStorageAdapter {
    pub async fn apply_operation(&self, operation: &Operation) -> Result<OperationResponse> {
        let mut storage = self.storage.write().await;
        
        match operation {
            Operation::Put { key, value } => {
                storage.put(key.clone(), value.clone()).await?;
                Ok(OperationResponse::default())
            }
            // ... other operations
        }
    }
}
```

**Responsibilities**:
- Adapter translates Raft operations to storage operations
- Adapter manages Raft state (term, voted_for, log)
- Adapter handles snapshots

### 4. Storage Adapter ↔ Storage Engines

**Location**: Adapter uses `KeyValueStore` trait

**Interface**:
```rust
// Any storage engine implementing KeyValueStore works
pub trait KeyValueStore: Send + Sync {
    async fn put(&mut self, key: Vec<u8>, value: Vec<u8>) -> Result<()>;
    async fn get(&self, key: &[u8]) -> Result<Option<Vec<u8>>>;
    async fn delete(&mut self, key: Vec<u8>) -> Result<()>;
    // ... other methods
}

// LSM engine
let lsm = LSMTreeEngine::new(options)?;
let adapter = RaftStorageAdapter::new(Box::new(lsm), shard_id);

// B+Tree engine
let btree = BTreeEngine::new(options)?;
let adapter = RaftStorageAdapter::new(Box::new(btree), shard_id);
```

**Flexibility**:
- Any storage engine can be used
- Storage engine choice is per-shard
- Can mix LSM and B+Tree in same cluster

### 5. Storage Engines ↔ WAL/VFS

**Location**: Storage engines use WAL and VFS

**Interface**:
```rust
// LSM engine uses WAL for durability
impl LSMTreeEngine {
    pub fn new(options: LSMTreeOptions) -> Result<Self> {
        let wal = WALManager::new(wal_config)?;
        let vfs = LocalFileSystem::new()?;
        
        Ok(Self {
            wal,
            vfs,
            // ... other fields
        })
    }
}
```

**Note**: Raft log is separate from storage engine WAL
- Raft log: Consensus log (replicated operations)
- Storage WAL: Durability log (applied operations)

## Data Flow Examples

### Write Path (Distributed)

```
1. Application: router.put(b"user:123", b"Alice")
                     │
2. Router: hash("user:123") → Shard 2
                     │
3. Router: forward to Shard 2 leader
                     │
4. Shard 2 Leader: propose to Raft
                     │
5. Raft: replicate to followers
                     │
   ┌─────────────────┼─────────────────┐
   │                 │                 │
   ▼                 ▼                 ▼
Follower 1       Follower 2       Follower 3
   │                 │                 │
   └─────────────────┼─────────────────┘
                     │
6. Raft: quorum reached, commit
                     │
7. Storage Adapter: apply_operation(Put)
                     │
8. LSM Engine: storage.put(key, value)
                     │
9. WAL: write entry
                     │
10. VFS: fsync to disk
                     │
11. Response: success → Application
```

### Read Path (Linearizable)

```
1. Application: router.get(b"user:123")
                     │
2. Router: hash("user:123") → Shard 2
                     │
3. Router: forward to Shard 2 leader
                     │
4. Shard 2 Leader: ReadIndex protocol
                     │
5. Raft: confirm leadership with quorum
                     │
   ┌─────────────────┼─────────────────┐
   │                 │                 │
   ▼                 ▼                 ▼
Follower 1       Follower 2       Follower 3
   │                 │                 │
   └─────────────────┼─────────────────┘
                     │
6. Shard 2 Leader: read from local storage
                     │
7. LSM Engine: storage.get(key)
                     │
8. Response: Some(b"Alice") → Application
```

### Metadata Change

```
1. Admin: metadata.add_node(new_node_info)
                     │
2. Metadata Raft Group: propose change
                     │
3. Raft: replicate to all nodes
                     │
   ┌─────────────────┼─────────────────┐
   │                 │                 │
   ▼                 ▼                 ▼
  Node 1           Node 2           Node 3
   │                 │                 │
   └─────────────────┼─────────────────┘
                     │
4. Raft: commit metadata change
                     │
5. All nodes: update local metadata cache
                     │
6. Response: success → Admin
```

## Configuration

### Single-Node Mode (Phase 1)

```rust
// No Raft, direct storage access
let storage = LSMTreeEngine::new(options)?;
storage.put(key, value).await?;
```

### Multi-Node Mode (Phase 2)

```rust
// With Raft consensus
let config = ReplicationConfig {
    replication_factor: 3,
    min_sync_replicas: 2,
    election_timeout_ms: 1000,
    heartbeat_interval_ms: 100,
    max_append_entries: 100,
    snapshot_threshold: 10000,
};

let router = Router::new(node_id, config);

// Add shards for this node
for shard_id in assigned_shards {
    let storage = LSMTreeEngine::new(options)?;
    router.add_shard(shard_id, Box::new(storage), peers).await?;
}
```

## Migration Path

### Phase 1 → Phase 2 Migration

1. **Start with single-node**:
   ```rust
   let storage = LSMTreeEngine::new(options)?;
   ```

2. **Add Raft wrapper** (backward compatible):
   ```rust
   let router = Router::new(node_id, config);
   router.add_shard(ShardId::new(0), Box::new(storage), vec![]).await?;
   // Single shard, no peers = single-node mode
   ```

3. **Scale to multi-node**:
   ```rust
   // Add more nodes
   metadata.add_node(node2_info).await?;
   metadata.add_node(node3_info).await?;
   
   // Add replicas to shard
   metadata.update_shard_assignment(
       ShardId::new(0),
       vec![node1, node2, node3]
   ).await?;
   ```

4. **Add more shards** (scale out):
   ```rust
   router.set_shard_count(4).await;
   
   // Create new shards
   for shard_id in 1..4 {
       metadata.create_shard(
           ShardId::new(shard_id),
           range,
           replicas
       ).await?;
   }
   ```

## Testing Integration

### Unit Tests
- Test each component in isolation
- Mock dependencies

### Integration Tests
- Test Router ↔ Shard Group interaction
- Test Shard Group ↔ Storage Adapter interaction
- Test end-to-end write/read paths

### Distributed Tests
- Multi-node scenarios
- Network partitions
- Leader failures
- Shard rebalancing

## Performance Considerations

### Latency
- Single-node: ~1ms (storage only)
- Distributed (linearizable): ~3-5ms (storage + Raft consensus)
- Distributed (lease): ~1-2ms (storage + lease check)
- Distributed (follower): ~1ms (storage only, potentially stale)

### Throughput
- Limited by Raft consensus (typically 10K-100K ops/sec per shard)
- Scale horizontally by adding shards
- Each shard is independent

### Resource Usage
- Memory: Raft log + storage engine
- Disk: Raft log + SST files + WAL
- Network: Heartbeats + log replication

## Monitoring

### Key Metrics
- Raft term, commit index, applied index
- Leader election count
- Replication lag per follower
- Proposal latency (p50, p99, p999)
- Snapshot transfer rate

### Integration with Observability
```rust
use metrics::{counter, histogram, gauge};

// In ShardRaftGroup
counter!("raft.proposals.total", 1);
histogram!("raft.proposal.latency", latency_ms);
gauge!("raft.commit_index", commit_index as f64);
```

## References

- [ADR-0007: Clustering, Sharding, Replication, and Consensus](../docs/ADR/ADR-0007-Clustering-Sharding-Replication-Consensus.md)
- [Implementation Plan - Phase 2](../docs/DEV/IMPLEMENTATION_PLAN.md#phase-2-distributed-consensus-weeks-9-14)
- [nanograph-raft README](README.md)