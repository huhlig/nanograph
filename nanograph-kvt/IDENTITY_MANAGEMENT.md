# Identity Management Strategy

## Overview

Nanograph uses a hierarchical identity system where IDs are managed at different levels to ensure global uniqueness across a distributed cluster.

## Identity Hierarchy

```
Cluster (managed by cluster coordinator)
  ├─ ClusterId: u32 (globally unique, assigned once)
  │
  ├─ RegionId: u32 (per cluster, assigned by cluster coordinator)
  │   └─ Regions are full replicas of all data
  │
  ├─ ServerId: u64 (per region, assigned by region coordinator)
  │   └─ Servers host shard replicas
  │
  ├─ InstanceIdentifier: u128 (per region, assigned by region coordinator)
  │   └─ Servers host shard replicas
  │
  └─ Logical Data (managed by consensus)
      ├─ NamespaceId: u64 (cluster-wide, via consensus)
      ├─ TableId: u64 (cluster-wide, via consensus)
      └─ ShardId: u64 = f(TableId, ShardIndex)
          └─ Deterministically derived, no allocation needed
```

## Key Principles

### 1. **Deterministic Shard IDs**

ShardId is **not allocated** - it's **computed** from TableId and ShardIndex:

```rust
// ShardId encoding (64 bits total):
// Bits 63-32: TableId (upper 32 bits of u64)
// Bits 31-0:  ShardIndex (u32)

let shard_id = ShardId::from_parts(
    TableId(12345),      // Unique table
    ShardIndex(7)        // 8th shard of this table
);

// Result: ShardId with deterministic value
// Can be recomputed anywhere in the cluster
assert_eq!(shard_id.table(), TableId(12345));
assert_eq!(shard_id.index(), ShardIndex(7));
```

**Benefits**:
- No need for distributed shard ID allocation
- Any node can compute the correct ShardId
- Shard IDs are stable and predictable
- No coordination needed for shard ID generation

### 2. **Consensus-Based Table IDs**

TableId allocation requires cluster-wide consensus:

```rust
impl KeyValueDatabaseManager {
    async fn create_table(&self, namespace: NamespaceId, config: TableConfig) 
        -> KeyValueResult<TableId> 
    {
        // 1. Propose table creation to metadata Raft group
        let proposal = MetadataChange::CreateTable {
            namespace,
            name: config.name.clone(),
            config: config.clone(),
        };
        
        // 2. Wait for consensus (all regions must agree)
        let table_id = self.metadata_raft
            .propose(proposal)
            .await?;
        
        // 3. TableId is now globally unique and known to all nodes
        // 4. Create shards using this TableId
        for shard_index in 0..config.shard_count {
            self.create_shard_replicas(table_id, ShardIndex(shard_index)).await?;
        }
        
        Ok(table_id)
    }
}
```

### 3. **Storage Engine Responsibility**

Storage engines (LSM, B+Tree) are **passive** - they don't allocate IDs:

```rust
#[async_trait]
impl KeyValueShardStore for LSMKeyValueStore {
    async fn create_shard(&self, table: TableId, index: ShardIndex) 
        -> KeyValueResult<ShardId> 
    {
        // 1. Compute deterministic ShardId
        let shard_id = ShardId::from_parts(table, index);
        
        // 2. Create physical storage for this shard
        let engine = self.create_engine_for_shard(shard_id)?;
        
        // 3. Register in local shard map
        self.engines.write().unwrap().insert(shard_id, engine);
        
        // 4. Return the ShardId (same value on all replicas)
        Ok(shard_id)
    }
}
```

**Key Points**:
- Storage engine receives TableId and ShardIndex as parameters
- It computes ShardId deterministically
- No local ID generation or coordination needed
- Same ShardId on all replica servers

## Identity Allocation Flow

### Creating a Sharded Table

```
┌─────────────────────────────────────────────────────────────┐
│ Step 1: Client Request                                       │
│ manager.create_table(namespace, config.with_shards(4))      │
└────────────────────────┬────────────────────────────────────┘
                         │
┌────────────────────────▼────────────────────────────────────┐
│ Step 2: Metadata Raft Consensus                             │
│ - Propose: CreateTable { namespace, name, config }          │
│ - All regions participate in consensus                      │
│ - Allocate globally unique TableId = 12345                  │
│ - Store in metadata: (namespace, name) → TableId            │
└────────────────────────┬────────────────────────────────────┘
                         │
┌────────────────────────▼────────────────────────────────────┐
│ Step 3: Shard Creation (per region)                         │
│ For each shard_index in 0..4:                               │
│   shard_id = ShardId::from_parts(TableId(12345), index)    │
│   - Shard 0: ShardId(12345 << 32 | 0)                      │
│   - Shard 1: ShardId(12345 << 32 | 1)                      │
│   - Shard 2: ShardId(12345 << 32 | 2)                      │
│   - Shard 3: ShardId(12345 << 32 | 3)                      │
└────────────────────────┬────────────────────────────────────┘
                         │
┌────────────────────────▼────────────────────────────────────┐
│ Step 4: Replica Placement                                   │
│ For each shard, create replicas on selected servers:        │
│   - Select servers using placement strategy                 │
│   - Call storage_engine.create_shard(table, index)          │
│   - Storage engine creates physical files/structures        │
│   - All replicas use same ShardId                           │
└─────────────────────────────────────────────────────────────┘
```

## Metadata Raft Group

The metadata Raft group manages cluster-wide identity allocation:

```rust
pub struct MetadataRaftGroup {
    /// Current state
    state: MetadataState,
    
    /// Raft consensus
    raft: RaftNode,
}

pub struct MetadataState {
    /// ID generators (monotonically increasing)
    next_namespace_id: u64,
    next_table_id: u64,
    
    /// Name mappings
    namespace_names: HashMap<String, NamespaceId>,
    table_names: HashMap<(NamespaceId, String), TableId>,
    
    /// Table configurations
    tables: HashMap<TableId, TableMetadata>,
    
    /// Shard assignments (which servers host which shards)
    shard_assignments: HashMap<ShardId, Vec<ServerId>>,
}

impl MetadataRaftGroup {
    async fn allocate_table_id(&mut self) -> TableId {
        let id = self.state.next_table_id;
        self.state.next_table_id += 1;
        TableId(id)
    }
    
    async fn create_table(&mut self, namespace: NamespaceId, name: String, config: TableConfig) 
        -> Result<TableId> 
    {
        // Check if table already exists
        if self.state.table_names.contains_key(&(namespace, name.clone())) {
            return Err(Error::TableAlreadyExists(name));
        }
        
        // Allocate new TableId
        let table_id = self.allocate_table_id().await;
        
        // Store metadata
        self.state.table_names.insert((namespace, name.clone()), table_id);
        self.state.tables.insert(table_id, TableMetadata {
            id: table_id,
            name,
            namespace,
            shard_count: config.shard_count,
            partitioner: config.partitioner,
            // ... other fields
        });
        
        Ok(table_id)
    }
}
```

## Uniqueness Guarantees

### TableId Uniqueness
- ✅ **Cluster-wide**: Allocated via Raft consensus
- ✅ **Monotonic**: Always increasing, never reused
- ✅ **Persistent**: Stored in metadata log
- ✅ **Replicated**: All regions have same TableId for same table

### ShardId Uniqueness
- ✅ **Deterministic**: Computed from TableId + ShardIndex
- ✅ **No allocation**: No coordination needed
- ✅ **Stable**: Same computation gives same result
- ✅ **Collision-free**: TableId uniqueness ensures ShardId uniqueness

### NamespaceId Uniqueness
- ✅ **Cluster-wide**: Allocated via Raft consensus
- ✅ **Monotonic**: Always increasing
- ✅ **Name-mapped**: Human-readable names map to IDs

## Storage Implications

### File System Layout

```
/data/
  ├─ metadata/
  │   └─ raft_log/          # Metadata Raft log
  │
  ├─ shards/
  │   ├─ shard_529755813888/  # ShardId(12345 << 32 | 0)
  │   │   ├─ wal/
  │   │   └─ sstables/
  │   │
  │   ├─ shard_529755813889/  # ShardId(12345 << 32 | 1)
  │   │   ├─ wal/
  │   │   └─ sstables/
  │   │
  │   └─ shard_529755813890/  # ShardId(12345 << 32 | 2)
  │       ├─ wal/
  │       └─ sstables/
```

**Benefits**:
- Shard directories have stable, predictable names
- Easy to identify which table a shard belongs to
- Can reconstruct metadata from directory structure if needed
- Backup/restore is straightforward

### WAL Segment Naming

```rust
// WAL segments use ShardId for naming
let wal_path = format!("/wal/shard_{}/segment_{:08}.wal", shard_id.as_u64(), segment_num);

// Example:
// /wal/shard_529755813888/segment_00000001.wal
// /wal/shard_529755813888/segment_00000002.wal
```

### SSTable Naming

```rust
// SSTables use ShardId in their names
let sstable_path = format!("/data/shard_{}/level_{}/table_{:08}.sst", 
    shard_id.as_u64(), level, table_num);

// Example:
// /data/shard_529755813888/level_0/table_00000001.sst
// /data/shard_529755813888/level_1/table_00000001.sst
```

## Recovery and Consistency

### Metadata Recovery

If metadata is lost, it can be reconstructed:

```rust
async fn recover_metadata_from_disk() -> Result<MetadataState> {
    let mut state = MetadataState::default();
    
    // Scan shard directories
    for shard_dir in scan_shard_directories()? {
        let shard_id = parse_shard_id_from_path(&shard_dir)?;
        let table_id = shard_id.table();
        let shard_index = shard_id.index();
        
        // Reconstruct table metadata
        if !state.tables.contains_key(&table_id) {
            // Read table config from shard metadata
            let config = read_shard_metadata(&shard_dir)?;
            state.tables.insert(table_id, config);
        }
        
        // Update next_table_id to avoid collisions
        state.next_table_id = state.next_table_id.max(table_id.0 + 1);
    }
    
    Ok(state)
}
```

### Consistency Checks

```rust
async fn verify_shard_consistency() -> Result<()> {
    for (shard_id, replicas) in shard_assignments {
        // Verify all replicas have same ShardId
        for server in replicas {
            let remote_shard_id = query_shard_id(server, shard_id).await?;
            assert_eq!(shard_id, remote_shard_id, "ShardId mismatch on replica");
        }
        
        // Verify ShardId matches table configuration
        let table_id = shard_id.table();
        let shard_index = shard_id.index();
        let table_meta = get_table_metadata(table_id)?;
        assert!(shard_index.0 < table_meta.shard_count, "Invalid shard index");
    }
    
    Ok(())
}
```

## Summary

### Identity Management Responsibilities

| Component | Manages | Method |
|-----------|---------|--------|
| **Metadata Raft** | TableId, NamespaceId | Consensus allocation |
| **KeyValueDatabaseManager** | Table creation, shard coordination | Orchestrates via Raft |
| **KeyValueShardStore** | Physical storage | Receives IDs, creates storage |
| **ShardId** | Shard identification | Computed from TableId + ShardIndex |

### Key Advantages

1. **No Distributed Shard ID Allocation** - ShardIds are computed, not allocated
2. **Consensus Only for Tables** - Reduces coordination overhead
3. **Deterministic and Stable** - Same inputs always produce same ShardId
4. **Simple Recovery** - Can reconstruct metadata from disk
5. **Clear Separation** - Metadata layer handles IDs, storage layer handles data

This design ensures global uniqueness while minimizing coordination overhead and maintaining simplicity.