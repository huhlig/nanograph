---
parent: ADR
nav_order: 0028
title: Tablespace Storage Management
status: proposed
date: 2026-01-14
deciders: Hans W. Uhlig
---

# ADR-0028: Tablespace Storage Management

## Status

Proposed

## Context

Nanograph currently uses a single global storage configuration (`data_dir` and `wal_dir`) for all tables across the entire cluster. This creates several limitations:

1. **No Storage Tiering** - Cannot place hot tables on fast SSD and cold tables on cheaper HDD
2. **Limited Multi-Tenancy** - Cannot isolate tenant data on separate storage volumes
3. **Capacity Constraints** - Single filesystem limits total database size
4. **Operational Inflexibility** - Cannot independently backup, restore, or maintain different table groups
5. **Performance Bottlenecks** - All I/O contends for the same storage devices
6. **Distributed Complexity** - Each node must have identical storage layout

In a distributed system with Raft-based replication, we need a storage abstraction that:
- Works consistently across all nodes in a Raft group
- Supports heterogeneous storage configurations per node
- Enables storage-aware shard placement
- Maintains strong consistency guarantees

## Decision

Introduce **Tablespaces** as first-class storage management entities that:

1. **Define storage locations** for tables and their shards
2. **Support multiple storage backends** via VFS abstraction
3. **Enable per-node storage configuration** while maintaining cluster-wide metadata
4. **Integrate with Raft consensus** for distributed coordination
5. **Provide storage-aware shard placement** for optimal performance

## Architecture

### Hierarchical Storage Model

```
Cluster
  ├─ Node 1
  │   ├─ Tablespace: fast_ssd (local: /mnt/nvme0)
  │   ├─ Tablespace: bulk_hdd (local: /mnt/hdd0)
  │   └─ Tablespace: archive (local: /mnt/archive)
  │
  ├─ Node 2
  │   ├─ Tablespace: fast_ssd (local: /mnt/nvme1)
  │   ├─ Tablespace: bulk_hdd (local: /mnt/hdd1)
  │   └─ Tablespace: archive (local: /mnt/archive)
  │
  └─ Node 3
      ├─ Tablespace: fast_ssd (local: /mnt/nvme2)
      ├─ Tablespace: bulk_hdd (local: /mnt/hdd2)
      └─ Tablespace: archive (local: /mnt/archive)
```

### Core Types

```rust
/// Tablespace identifier (cluster-wide)
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash, Serialize, Deserialize)]
pub struct TablespaceId(pub u32);

/// Tablespace metadata (stored in System Metadata Raft Group)
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TablespaceMetadata {
    /// Unique tablespace identifier
    pub id: TablespaceId,
    
    /// Human-readable name
    pub name: String,
    
    /// Storage tier classification
    pub tier: StorageTier,
    
    /// Storage backend type
    pub backend: StorageBackend,
    
    /// Maximum size in bytes (None = unlimited)
    pub max_size_bytes: Option<u64>,
    
    /// Current usage in bytes
    pub used_bytes: u64,
    
    /// Nodes that have this tablespace configured
    pub available_nodes: Vec<NodeId>,
    
    /// Creation timestamp
    pub created_at: Timestamp,
    
    /// Last modified timestamp
    pub last_modified: Timestamp,
    
    /// Custom properties
    pub properties: HashMap<String, String>,
}

/// Storage tier for performance classification
#[derive(Copy, Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum StorageTier {
    /// Ultra-fast NVMe/Optane storage
    Hot,
    
    /// Standard SSD storage
    Warm,
    
    /// HDD or network storage
    Cold,
    
    /// Archive/backup storage
    Archive,
}

/// Storage backend type
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum StorageBackend {
    /// Local filesystem
    LocalFS,
    
    /// Network filesystem (NFS, etc)
    NetworkFS,
    
    /// Object storage (S3, etc)
    ObjectStorage { endpoint: String },
    
    /// In-memory (testing only)
    Memory,
}

/// Per-node tablespace configuration
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct NodeTablespaceConfig {
    /// Tablespace ID
    pub tablespace_id: TablespaceId,
    
    /// Local data directory path
    pub data_path: String,
    
    /// Local WAL directory path (optional, defaults to data_path/wal)
    pub wal_path: Option<String>,
    
    /// VFS scheme for this tablespace
    pub vfs_scheme: String,
    
    /// Mount options
    pub mount_options: HashMap<String, String>,
}
```

### Table-to-Tablespace Assignment

```rust
/// Extended table metadata with tablespace assignment
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TableMetadata {
    pub id: TableId,
    pub name: String,
    pub container_id: ContainerId,
    pub namespace_id: NamespaceId,
    
    /// Tablespace where this table's data is stored
    pub tablespace_id: TablespaceId,
    
    /// Shard configuration
    pub shard_config: ShardConfig,
    
    // ... other fields
}

/// Shard metadata with storage location
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ShardMetadata {
    pub shard_id: ShardId,
    pub table_id: TableId,
    
    /// Tablespace for this shard
    pub tablespace_id: TablespaceId,
    
    /// Key range
    pub key_range: (Vec<u8>, Vec<u8>),
    
    /// Replica nodes
    pub replicas: Vec<NodeId>,
    
    /// Current leader
    pub leader: Option<NodeId>,
    
    pub status: ShardStatus,
    
    // ... other fields
}
```

### Integration with Raft

#### System Metadata Raft Group Operations

```rust
/// Metadata operations for tablespace management
pub enum SystemMetadataOperation {
    /// Create a new tablespace
    CreateTablespace {
        metadata: TablespaceMetadata,
    },
    
    /// Update tablespace metadata
    UpdateTablespace {
        tablespace_id: TablespaceId,
        updates: TablespaceUpdate,
    },
    
    /// Delete a tablespace (must be empty)
    DeleteTablespace {
        tablespace_id: TablespaceId,
    },
    
    /// Register node's tablespace configuration
    RegisterNodeTablespace {
        node_id: NodeId,
        config: NodeTablespaceConfig,
    },
    
    /// Update tablespace usage statistics
    UpdateTablespaceUsage {
        tablespace_id: TablespaceId,
        used_bytes: u64,
    },
}
```

#### Shard Placement with Tablespace Awareness

```rust
/// Shard placement strategy considering tablespaces
pub struct TablespaceAwarePlacement {
    /// Cluster state
    cluster_state: Arc<RwLock<RaftClusterState>>,
    
    /// Tablespace metadata
    tablespaces: Arc<RwLock<HashMap<TablespaceId, TablespaceMetadata>>>,
}

impl TablespaceAwarePlacement {
    /// Select nodes for shard replicas based on tablespace availability
    pub fn select_replica_nodes(
        &self,
        tablespace_id: TablespaceId,
        replication_factor: usize,
        placement_strategy: PlacementStrategy,
    ) -> Result<Vec<NodeId>> {
        let tablespaces = self.tablespaces.read().unwrap();
        let tablespace = tablespaces.get(&tablespace_id)
            .ok_or(Error::TablespaceNotFound)?;
        
        // Filter nodes that have this tablespace configured
        let mut candidate_nodes: Vec<NodeId> = tablespace
            .available_nodes
            .iter()
            .copied()
            .collect();
        
        // Apply placement strategy (rack-aware, zone-aware, etc)
        let selected = match placement_strategy {
            PlacementStrategy::Random => {
                self.select_random(&candidate_nodes, replication_factor)
            }
            PlacementStrategy::RackAware => {
                self.select_rack_aware(&candidate_nodes, replication_factor)
            }
            PlacementStrategy::ZoneAware => {
                self.select_zone_aware(&candidate_nodes, replication_factor)
            }
            PlacementStrategy::Custom => {
                self.select_custom(&candidate_nodes, replication_factor)
            }
        };
        
        if selected.len() < replication_factor {
            return Err(Error::InsufficientNodes {
                required: replication_factor,
                available: selected.len(),
            });
        }
        
        Ok(selected)
    }
}
```

### Storage Path Resolution

```rust
/// Resolve storage paths for a shard on a specific node
pub struct StoragePathResolver {
    /// Node-local tablespace configurations
    node_configs: HashMap<TablespaceId, NodeTablespaceConfig>,
    
    /// VFS manager
    vfs_manager: Arc<FileSystemManager>,
}

impl StoragePathResolver {
    /// Get the data path for a shard
    pub fn get_shard_data_path(
        &self,
        shard_id: ShardId,
        tablespace_id: TablespaceId,
    ) -> Result<String> {
        let config = self.node_configs.get(&tablespace_id)
            .ok_or(Error::TablespaceNotConfigured)?;
        
        // Format: {data_path}/shard_{table_id}_{shard_index}
        Ok(format!(
            "{}/shard_{}_{}", 
            config.data_path,
            shard_id.table().as_u64(),
            shard_id.index().as_u32()
        ))
    }
    
    /// Get the WAL path for a shard
    pub fn get_shard_wal_path(
        &self,
        shard_id: ShardId,
        tablespace_id: TablespaceId,
    ) -> Result<String> {
        let config = self.node_configs.get(&tablespace_id)
            .ok_or(Error::TablespaceNotConfigured)?;
        
        let wal_base = config.wal_path.as_ref()
            .unwrap_or(&format!("{}/wal", config.data_path));
        
        // Format: {wal_path}/shard_{table_id}_{shard_index}
        Ok(format!(
            "{}/shard_{}_{}", 
            wal_base,
            shard_id.table().as_u64(),
            shard_id.index().as_u32()
        ))
    }
    
    /// Get VFS instance for a tablespace
    pub fn get_vfs(
        &self,
        tablespace_id: TablespaceId,
    ) -> Result<Arc<dyn DynamicFileSystem>> {
        let config = self.node_configs.get(&tablespace_id)
            .ok_or(Error::TablespaceNotConfigured)?;
        
        self.vfs_manager.get_filesystem(&config.vfs_scheme)
            .ok_or(Error::VFSNotFound)
    }
}
```

### Configuration

#### Cluster-Level Configuration

```toml
# System-wide tablespace definitions
[[tablespaces]]
id = 1
name = "fast_ssd"
tier = "Hot"
backend = "LocalFS"
max_size_bytes = 1099511627776  # 1TB

[[tablespaces]]
id = 2
name = "bulk_hdd"
tier = "Cold"
backend = "LocalFS"
max_size_bytes = 10995116277760  # 10TB

[[tablespaces]]
id = 3
name = "archive"
tier = "Archive"
backend = { ObjectStorage = { endpoint = "s3://backup-bucket" } }
```

#### Node-Level Configuration

```toml
# Node-specific tablespace paths
[[node.tablespaces]]
tablespace_id = 1
data_path = "/mnt/nvme0/nanograph"
wal_path = "/mnt/nvme0/nanograph/wal"
vfs_scheme = "local"

[[node.tablespaces]]
tablespace_id = 2
data_path = "/mnt/hdd0/nanograph"
wal_path = "/mnt/hdd0/nanograph/wal"
vfs_scheme = "local"

[[node.tablespaces]]
tablespace_id = 3
data_path = "/mnt/archive/nanograph"
vfs_scheme = "s3"

[node.tablespaces.mount_options]
region = "us-east-1"
```

### API Changes

#### Database Manager API

```rust
impl KeyValueDatabaseManager {
    /// Create a tablespace (cluster-wide operation via Raft)
    pub async fn create_tablespace(
        &self,
        name: String,
        tier: StorageTier,
        backend: StorageBackend,
        max_size_bytes: Option<u64>,
    ) -> KeyValueResult<TablespaceId> {
        let tablespace_id = self.allocate_tablespace_id().await?;
        
        let metadata = TablespaceMetadata {
            id: tablespace_id,
            name,
            tier,
            backend,
            max_size_bytes,
            used_bytes: 0,
            available_nodes: Vec::new(),
            created_at: Timestamp::now(),
            last_modified: Timestamp::now(),
            properties: HashMap::new(),
        };
        
        // Coordinate via System Metadata Raft Group
        if let Some(router) = &self.raft_router {
            router
                .system_metadata()
                .create_tablespace(metadata)
                .await?;
        }
        
        Ok(tablespace_id)
    }
    
    /// Register this node's tablespace configuration
    pub async fn register_node_tablespace(
        &self,
        config: NodeTablespaceConfig,
    ) -> KeyValueResult<()> {
        // Validate local paths exist
        self.validate_tablespace_paths(&config)?;
        
        // Register with cluster via Raft
        if let Some(router) = &self.raft_router {
            router
                .system_metadata()
                .register_node_tablespace(self.node_id().unwrap(), config)
                .await?;
        }
        
        Ok(())
    }
    
    /// Create a table in a specific tablespace
    pub async fn create_table(
        &self,
        path: &str,
        name: String,
        config: TableCreate,
        tablespace_id: TablespaceId,  // NEW PARAMETER
    ) -> KeyValueResult<TableId> {
        // Validate tablespace exists and has capacity
        self.validate_tablespace(tablespace_id).await?;
        
        let table_id = self.allocate_table_id().await?;
        
        // Create shards with tablespace-aware placement
        match config.sharding_config {
            TableSharding::Single => {
                let shard_id = ShardId::from_parts(table_id, ShardIndex::new(0));
                
                // Select nodes that have this tablespace
                let replicas = self.select_replica_nodes(
                    tablespace_id,
                    config.replication_factor,
                    config.placement_strategy,
                ).await?;
                
                // Create shard via Raft
                if let Some(router) = &self.raft_router {
                    router
                        .system_metadata()
                        .create_shard(
                            shard_id,
                            tablespace_id,  // NEW
                            (vec![], vec![0xFF; 32]),
                            replicas,
                        )
                        .await?;
                }
            }
            
            TableSharding::Multiple { shard_count, .. } => {
                for shard_index in 0..shard_count {
                    let shard_id = ShardId::from_parts(table_id, ShardIndex::new(shard_index));
                    
                    let replicas = self.select_replica_nodes(
                        tablespace_id,
                        config.replication_factor,
                        config.placement_strategy,
                    ).await?;
                    
                    if let Some(router) = &self.raft_router {
                        router
                            .system_metadata()
                            .create_shard(
                                shard_id,
                                tablespace_id,  // NEW
                                self.calculate_key_range(shard_index, shard_count),
                                replicas,
                            )
                            .await?;
                    }
                }
            }
        }
        
        Ok(table_id)
    }
}
```

#### Shard Manager Integration

```rust
impl KeyValueShardManager {
    /// Create a shard in a specific tablespace
    pub async fn create_shard(
        &mut self,
        shard_id: ShardId,
        tablespace_id: TablespaceId,
        engine_type: EngineType,
    ) -> KeyValueResult<()> {
        // Resolve storage paths for this tablespace
        let data_path = self.path_resolver.get_shard_data_path(shard_id, tablespace_id)?;
        let wal_path = self.path_resolver.get_shard_wal_path(shard_id, tablespace_id)?;
        
        // Get VFS for this tablespace
        let vfs = self.path_resolver.get_vfs(tablespace_id)?;
        
        // Create shard with tablespace-specific storage
        let store = match engine_type {
            EngineType::LSM => {
                let config = LSMConfig {
                    data_dir: data_path,
                    wal_dir: wal_path,
                    // ... other config
                };
                Box::new(LSMKVStore::new(vfs, config)?) as Box<dyn KeyValueShardStore>
            }
            EngineType::BTree => {
                let config = BTreeConfig {
                    data_dir: data_path,
                    wal_dir: wal_path,
                    // ... other config
                };
                Box::new(BTreeKVStore::new(vfs, config)?) as Box<dyn KeyValueShardStore>
            }
            EngineType::ART => {
                let config = ARTConfig {
                    data_dir: data_path,
                    wal_dir: wal_path,
                    // ... other config
                };
                Box::new(ARTKVStore::new(vfs, config)?) as Box<dyn KeyValueShardStore>
            }
        };
        
        self.shards.insert(shard_id, store);
        Ok(())
    }
}
```

## Implementation Plan

### Phase 1: Core Types and Metadata (Week 1)
- [ ] Add `TablespaceId` to `nanograph-core/src/types.rs`
- [ ] Create `TablespaceMetadata` and related types
- [ ] Add tablespace fields to `TableMetadata` and `ShardMetadata`
- [ ] Update serialization/deserialization

### Phase 2: System Metadata Integration (Week 2)
- [ ] Add tablespace operations to System Metadata Raft Group
- [ ] Implement tablespace creation/deletion via Raft
- [ ] Add node tablespace registration
- [ ] Implement tablespace usage tracking

### Phase 3: Storage Path Resolution (Week 3)
- [ ] Implement `StoragePathResolver`
- [ ] Add per-node tablespace configuration loading
- [ ] Integrate with VFS manager
- [ ] Add path validation and error handling

### Phase 4: Shard Placement (Week 4)
- [ ] Implement `TablespaceAwarePlacement`
- [ ] Update shard creation to use tablespace-aware placement
- [ ] Add tablespace capacity checking
- [ ] Implement node filtering by tablespace availability

### Phase 5: Database Manager API (Week 5)
- [ ] Update `create_table` to accept `tablespace_id`
- [ ] Add `create_tablespace` API
- [ ] Add `register_node_tablespace` API
- [ ] Update table creation flow

### Phase 6: Shard Manager Integration (Week 6)
- [ ] Update `KeyValueShardManager` to use tablespace paths
- [ ] Modify shard creation to resolve tablespace storage
- [ ] Update WAL initialization with tablespace paths
- [ ] Add tablespace validation on shard operations

### Phase 7: Configuration and CLI (Week 7)
- [ ] Add tablespace configuration to TOML files
- [ ] Implement configuration parsing
- [ ] Add CLI commands for tablespace management
- [ ] Create configuration validation

### Phase 8: Testing and Documentation (Week 8)
- [ ] Unit tests for all tablespace components
- [ ] Integration tests with distributed setup
- [ ] Performance benchmarks
- [ ] Update documentation and examples

## Migration Strategy

Since this is a new system without backward compatibility requirements:

1. **Default Tablespace**: Create a default tablespace on cluster initialization
2. **Explicit Assignment**: All tables must specify a tablespace
3. **Node Registration**: Nodes must register their tablespace configurations on startup
4. **Validation**: Fail fast if tablespace is not available on required nodes

## Consequences

### Positive

* **Storage Flexibility** - Different tables can use different storage tiers
* **Multi-Tenancy** - Isolate tenant data on separate storage volumes
* **Performance Optimization** - Place hot data on fast storage
* **Capacity Management** - Distribute data across multiple volumes
* **Operational Control** - Independent backup/restore per tablespace
* **Cost Optimization** - Use cheaper storage for cold data
* **Distributed Awareness** - Proper integration with Raft consensus

### Negative

* **Configuration Complexity** - More configuration required per node
* **Operational Overhead** - Must manage multiple storage locations
* **Placement Constraints** - Shard placement limited by tablespace availability
* **Migration Complexity** - Moving tables between tablespaces requires data movement

### Risks

* **Misconfiguration** - Nodes with missing tablespace configurations
* **Capacity Planning** - Running out of space in a tablespace
* **Performance Variance** - Different tablespaces may have different performance
* **Replication Challenges** - Ensuring all replicas have required tablespaces

## Alternatives Considered

### 1. Single Global Storage Path
**Rejected** - Too limiting for production deployments, no storage tiering

### 2. Per-Table Storage Paths
**Rejected** - Doesn't integrate well with distributed consensus, hard to manage

### 3. Storage Pools (like Ceph)
**Rejected** - Too complex for initial implementation, can be added later

### 4. Automatic Storage Tiering
**Rejected** - Requires complex heuristics, better to start with explicit assignment

## Related ADRs

* [ADR-0003: Virtual File System Abstraction](ADR-0003-Virtual-File-System-Abstraction.md)
* [ADR-0006: Key-Value, Document, and Graph Support](ADR-0006-Key-Value-Document-Graph-Support.md)
* [ADR-0007: Clustering, Sharding, Replication, and Consensus](ADR-0007-Clustering-Sharding-Replication-Consensus.md)

## References

* PostgreSQL Tablespaces
* Oracle Tablespaces
* CockroachDB Localities
* TiDB Placement Rules

---

**Next Steps:**
1. Review and approve this ADR
2. Create implementation tasks
3. Begin Phase 1 implementation
4. Update related documentation