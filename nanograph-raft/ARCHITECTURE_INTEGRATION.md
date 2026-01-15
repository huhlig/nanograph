# Raft Architecture Integration Guide

This document describes how the Raft consensus layer integrates with the rest of Nanograph's architecture.

## System Architecture Overview

Nanograph uses a **3-tier Raft group architecture** for hierarchical metadata and data management:

```
┌─────────────────────────────────────────────────────────────────┐
│                        Application Layer                         │
│                    (nanograph-api, SDKs)                         │
└────────────────────────────┬────────────────────────────────────┘
                             │
┌────────────────────────────▼────────────────────────────────────┐
│                 KeyValueDatabaseManager                          │
│  - Routes operations through consensus router                    │
│  - Manages system and database metastores (caches)              │
│  - Coordinates with shard manager                                │
└────────────────────────────┬────────────────────────────────────┘
                             │
                ┌────────────┴────────────┐
                │   ConsensusRouter       │
                │  - Routes to Raft groups│
                └────────────┬────────────┘
                             │
        ┌────────────────────┼────────────────────┐
        │                    │                    │
        ▼                    ▼                    ▼
┌──────────────────┐ ┌──────────────────┐ ┌──────────────────┐
│ System Metadata  │ │ Database Metadata│ │  Data Shard      │
│   Raft Group     │ │   Raft Groups    │ │  Raft Groups     │
│   (1 per cluster)│ │ (1 per database) │ │ (N per table)    │
└────────┬─────────┘ └────────┬─────────┘ └────────┬─────────┘
         │                    │                    │
         │ Stores:            │ Stores:            │ Stores:
         │ - Clusters         │ - Namespaces       │ - User data
         │ - Regions          │ - Tables           │ - KV pairs
         │ - Servers          │ - Shards metadata  │ - High volume
         │ - Tenants          │ - DB users         │
         │ - System users     │ - Name resolver    │
         │                    │                    │
┌────────▼────────┐  ┌────────▼────────┐  ┌────────▼────────┐
│ RaftStorage     │  │ RaftStorage     │  │ RaftStorage     │
│   Adapter       │  │   Adapter       │  │   Adapter       │
└────────┬────────┘  └────────┬────────┘  └────────┬────────┘
         │                    │                    │
┌────────▼────────┐  ┌────────▼────────┐  ┌────────▼────────┐
│  LSM Engine     │  │  LSM Engine     │  │  LSM Engine     │
│ (KeyValueStore) │  │ (KeyValueStore) │  │ (KeyValueStore) │
└────────┬────────┘  └────────┬────────┘  └────────┬────────┘
         │                    │                    │
         └────────────────────┼────────────────────┘
                              │
┌─────────────────────────────▼───────────────────────────────────┐
│                    Storage Layer                                 │
│              (nanograph-wal, nanograph-vfs)                      │
└─────────────────────────────────────────────────────────────────┘
```

## Three-Tier Raft Group Architecture

### Tier 1: System Metadata Raft Group

**Scope**: One per cluster (global)
**Purpose**: Manages cluster-wide system configuration
**Shard**: `system_shard` (dedicated shard ID)

**Data Managed**:
- Cluster configuration and version
- Regions (geographic/data center locations)
- Servers (Nanograph instances)
- Tenants (multi-tenancy root)
- System-level users and permissions
- Database registry (references only)

**Update Frequency**: Very low (minutes to hours)
**Size**: Small (KB to low MB)
**Replication**: Fully replicated across all nodes

**Implementation**: `SystemMetastore` in `nanograph-kvm/src/metastore.rs`

```rust
pub struct SystemMetastore {
    cluster: ClusterMetadata,
    regions: HashMap<RegionId, RegionMetadata>,
    servers: HashMap<ServerId, ServerMetadata>,
    tenants: HashMap<TenantId, TenantMetadata>,
    databases: HashMap<DatabaseId, DatabaseMetadata>,
    system_users: HashMap<UserId, UserMetadata>,
    system_shard: ShardId,  // Dedicated shard for system metadata
    consensus_router: Option<Arc<ConsensusRouter>>,
}
```

### Tier 2: Database Metadata Raft Groups

**Scope**: One per database (container = tenant + database)
**Purpose**: Manages database-specific metadata and schema
**Shard**: `metadata_shard` per database (dedicated shard ID per database)

**Data Managed**:
- Namespaces within the database
- Tables and their configurations
- Shard metadata for this database's tables
- Database-level users and permissions
- Shard assignments (which nodes host which shards)
- Name resolver (hierarchical object paths)

**Update Frequency**: Low (seconds to minutes)
**Size**: Medium (MB range)
**Replication**: Fully replicated per database

**Implementation**: `DatabaseMetastore` in `nanograph-kvm/src/metastore.rs`

```rust
pub struct DatabaseMetastore {
    container: ContainerId,  // Tenant + Database
    namespaces: HashMap<NamespaceId, NamespaceMetadata>,
    tables: HashMap<TableId, TableMetadata>,
    shards: HashMap<ShardId, ShardMetadata>,
    database_users: HashMap<UserId, UserMetadata>,
    metadata_shard: Option<ShardId>,  // Dedicated shard for this DB's metadata
    consensus_router: Option<Arc<ConsensusRouter>>,
    shard_assignments: BTreeMap<ShardId, Vec<NodeId>>,
    resolver_nodes: BTreeMap<ObjectId, Node>,  // Hierarchical name resolution
    resolver_paths: BTreeMap<String, ObjectId>,
}
```

### Tier 3: Data Shard Raft Groups

**Scope**: Many per database (N shards per table)
**Purpose**: Manages actual user data
**Shard**: One Raft group per data shard

**Data Managed**:
- User key-value pairs
- Application data
- High-volume, high-frequency operations

**Update Frequency**: Very high (milliseconds)
**Size**: Large (GB to TB per shard)
**Replication**: Configurable per shard (typically 3-5 replicas)

**Implementation**: `ShardRaftGroup` in `nanograph-raft/src/shard_group.rs`

```rust
pub struct ShardRaftGroup {
    shard_id: ShardId,
    local_node_id: NodeId,
    storage: Arc<RaftStorageAdapter>,
    config: ReplicationConfig,
    role: Arc<RwLock<RaftRole>>,
    peers: Arc<RwLock<Vec<NodeId>>>,
}
```

## Hierarchical Relationship

```
System Metadata Raft Group (1)
    │
    ├─ Manages: Cluster, Regions, Servers, Tenants
    │
    └─ Contains references to ──┐
                                │
Database Metadata Raft Groups (N) ◄─┘
    │
    ├─ Database 1 Metadata
    │   ├─ Manages: Namespaces, Tables, DB Users
    │   └─ Contains references to ──┐
    │                                │
    ├─ Database 2 Metadata           │
    │   └─ ...                       │
    │                                │
    └─ Database N Metadata           │
                                     │
Data Shard Raft Groups (M) ◄────────┘
    │
    ├─ Shard 0 (Table A, DB 1)
    ├─ Shard 1 (Table A, DB 1)
    ├─ Shard 2 (Table B, DB 1)
    ├─ Shard 3 (Table C, DB 2)
    └─ ...
```

## Why Three Tiers?

### 1. Isolation and Scope

- **System changes** (adding a region) don't require consensus from all databases
- **Database changes** (creating a table) don't require consensus from all shards
- **Data operations** are isolated to their shard's Raft group

### 2. Multi-Tenancy Support

- Each tenant's database metadata is in its own Raft group
- Tenant A's schema changes don't affect Tenant B
- Better isolation and security boundaries

### 3. Scalability

- System metadata: 1 group (low volume, cluster-wide)
- Database metadata: N groups (medium volume, per-database)
- Data shards: M groups (high volume, horizontally scalable)

### 4. Performance Characteristics

| Tier | Groups | Update Freq | Size | Replication |
|------|--------|-------------|------|-------------|
| System Metadata | 1 | Very Low | Small | Full |
| Database Metadata | N (per DB) | Low | Medium | Full per DB |
| Data Shards | M (many) | Very High | Large | Per shard |

### 5. Failure Isolation

- System metadata failure doesn't affect data operations
- Database metadata failure only affects that database
- Data shard failure only affects that shard's data

## Integration Points

### 1. Application ↔ KeyValueDatabaseManager

**Location**: Applications use `KeyValueDatabaseManager` as the main entry point

**Interface**:
```rust
// Application code
use nanograph_kvm::KeyValueDatabaseManager;

// Create manager in distributed mode
let manager = KeyValueDatabaseManager::new_distributed(raft_router);

// Create a table (goes through metadata consensus)
let table_id = manager.create_table(
    "/app/public",
    "users".to_string(),
    TableCreate::new("users", StorageEngineType::LSM)
        .with_sharding(TableSharding::Multiple {
            shard_count: 4,
            partitioner: Partitioner::Hash,
            replication_factor: 3,
        })
).await?;

// Data operations (go through data shard consensus)
manager.put(table_id, b"user:123", b"Alice").await?;
let value = manager.get(table_id, b"user:123").await?;
manager.delete(table_id, b"user:123").await?;
```

**Responsibilities**:
- Routes metadata operations to appropriate metadata Raft groups
- Routes data operations to appropriate data shard Raft groups
- Manages metastore caches
- Coordinates with ConsensusRouter

### 2. KeyValueDatabaseManager ↔ ConsensusRouter

**Location**: Manager delegates consensus operations to router

**Interface**:
```rust
impl KeyValueDatabaseManager {
    pub async fn create_table(&self, ...) -> Result<TableId> {
        if let Some(router) = &self.raft_router {
            // Create shards via metadata Raft group
            router.metadata()
                .create_shard(shard_id, range, replicas)
                .await?;
        }
        // ...
    }
    
    pub async fn put(&self, table: TableId, key: &[u8], value: &[u8]) -> Result<()> {
        let shard_id = self.get_shard_for_key(table, key)?;
        
        if let Some(router) = &self.raft_router {
            // Route through Raft for distributed consensus
            router.put(key.to_vec(), value.to_vec()).await?;
        } else {
            // Single-node mode: direct shard access
            let shard_manager = self.shard_manager.read().unwrap();
            shard_manager.put(shard_id, key, value).await?;
        }
        Ok(())
    }
}
```

### 3. ConsensusRouter ↔ Metadata Raft Groups

**Location**: Router manages system and database metadata groups

**Interface**:
```rust
impl ConsensusRouter {
    // Access system metadata Raft group
    pub fn metadata(&self) -> &MetadataRaftGroup {
        &self.metadata
    }
    
    // System metadata operations
    pub async fn add_region(&self, region: RegionInfo) -> Result<()> {
        self.metadata.add_node(node_info).await
    }
    
    // Database metadata operations (future)
    pub async fn create_table(&self, database: DatabaseId, table: TableConfig) -> Result<()> {
        // Route to appropriate database metadata Raft group
        let db_metadata_group = self.get_database_metadata_group(database).await?;
        db_metadata_group.create_table(table).await
    }
}
```

**Metadata Raft Group Interface**:
```rust
impl MetadataRaftGroup {
    // System-level operations
    pub async fn add_node(&self, node: NodeInfo) -> Result<()> {
        self.propose_change(MetadataChange::AddNode { node }).await
    }
    
    pub async fn remove_node(&self, node_id: NodeId) -> Result<()> {
        self.propose_change(MetadataChange::RemoveNode { node_id }).await
    }
    
    // Shard management
    pub async fn create_shard(
        &self,
        shard_id: ShardId,
        range: (Vec<u8>, Vec<u8>),
        replicas: Vec<NodeId>,
    ) -> Result<()> {
        self.propose_change(MetadataChange::CreateShard {
            shard_id,
            range,
            replicas,
        }).await
    }
    
    pub async fn update_shard_assignment(
        &self,
        shard_id: ShardId,
        replicas: Vec<NodeId>,
    ) -> Result<()> {
        self.propose_change(MetadataChange::UpdateShardAssignment {
            shard_id,
            replicas,
        }).await
    }
}
```

### 4. ConsensusRouter ↔ Data Shard Raft Groups

**Location**: Router creates and manages data shard Raft groups

**Interface**:
```rust
impl ConsensusRouter {
    pub async fn add_shard(
        &self,
        shard_id: ShardId,
        storage: Box<dyn KeyValueShardStore>,
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
    
    // Data operations route to appropriate shard
    pub async fn put(&self, key: Vec<u8>, value: Vec<u8>) -> Result<()> {
        let shard_id = self.get_shard_for_key(&key).await;
        let shard = self.get_shard_group(shard_id).await?;
        shard.propose_write(Operation::Put { key, value }).await?;
        Ok(())
    }
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

### Metadata Operation: Create Table

```
1. Application: manager.create_table("users", config)
                     │
2. KeyValueDatabaseManager: Determine database
                     │
3. Route to Database Metadata Raft Group
                     │
4. Database Metadata Leader: propose to Raft
                     │
5. Raft: replicate to followers
                     │
   ┌─────────────────┼─────────────────┐
   │                 │                 │
   ▼                 ▼                 ▼
DB Meta          DB Meta          DB Meta
Follower 1       Follower 2       Follower 3
   │                 │                 │
   └─────────────────┼─────────────────┘
                     │
6. Raft: quorum reached, commit
                     │
7. Storage Adapter: apply_operation(CreateTable)
                     │
8. Update database metadata shard
                     │
9. Create data shards via System Metadata Raft Group
                     │
10. System Metadata: Update shard assignments
                     │
11. Response: TableId → Application
```

### Data Operation: Write Path (Distributed)

```
1. Application: manager.put(table_id, b"user:123", b"Alice")
                     │
2. KeyValueDatabaseManager: Determine shard
   hash("user:123") → Shard 2
                     │
3. ConsensusRouter: Route to Shard 2 Raft Group
                     │
4. Shard 2 Leader: propose to Raft
                     │
5. Raft: replicate to followers
                     │
   ┌─────────────────┼─────────────────┐
   │                 │                 │
   ▼                 ▼                 ▼
Data Shard       Data Shard       Data Shard
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

### System Operation: Add Node

```
1. Admin: cluster.add_server(server_info)
                     │
2. Route to System Metadata Raft Group
                     │
3. System Metadata Leader: propose to Raft
                     │
4. Raft: replicate to all nodes
                     │
   ┌─────────────────┼─────────────────┐
   │                 │                 │
   ▼                 ▼                 ▼
System Meta      System Meta      System Meta
Follower 1       Follower 2       Follower 3
   │                 │                 │
   └─────────────────┼─────────────────┘
                     │
5. Raft: quorum reached, commit
                     │
6. Storage Adapter: apply_operation(AddServer)
                     │
7. Update system metadata shard
                     │
8. All nodes: Update local metadata cache
                     │
9. Trigger shard rebalancing (if needed)
                     │
10. Response: success → Admin
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