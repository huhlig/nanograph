# Tablespace Implementation Guide

This document provides detailed instructions for modifying existing engines and managers to support tablespaces.

## Overview

The tablespace implementation requires changes across multiple layers:
1. **Core Types** - Add tablespace identifiers
2. **Storage Engines** - Accept VFS and paths from tablespace config
3. **Shard Manager** - Resolve tablespace paths and create engines
4. **Database Manager** - Coordinate tablespace-aware shard placement
5. **Raft Integration** - Store and replicate tablespace metadata

## 1. Core Types Modifications

### File: `nanograph-core/src/types.rs`

Add the tablespace identifier type:

```rust
/// Tablespace identifier (cluster-wide)
///
/// Represents a logical storage location that can be configured
/// differently on each node in the cluster.
#[derive(
    Copy, Clone, Debug, Default, Ord, PartialOrd, Eq, PartialEq, Hash, Serialize, Deserialize,
)]
pub struct TablespaceId(pub u32);

impl TablespaceId {
    /// Create a new tablespace identifier.
    pub fn new(id: u32) -> Self {
        Self(id)
    }

    /// Return the tablespace identifier as a u32.
    pub fn as_u32(&self) -> u32 {
        self.0
    }
    
    /// Default tablespace (always ID 0)
    pub const DEFAULT: TablespaceId = TablespaceId(0);
}

impl From<u32> for TablespaceId {
    fn from(id: u32) -> Self {
        Self(id)
    }
}

impl std::fmt::Display for TablespaceId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Tablespace({})", self.0)
    }
}
```

## 2. Storage Engine Modifications

All three storage engines (LSM, B+Tree, ART) need similar modifications to accept VFS and paths.

### File: `nanograph-lsm/src/kvstore.rs`

**Current Constructor:**
```rust
impl LSMKVStore {
    pub fn new() -> Self {
        // Uses hardcoded paths or global config
    }
}
```

**Modified Constructor:**
```rust
use nanograph_vfs::{FileSystem, DynamicFileSystem};
use std::sync::Arc;

/// Configuration for LSM storage with tablespace support
pub struct LSMStorageConfig {
    /// Data directory path (resolved from tablespace)
    pub data_dir: String,
    
    /// WAL directory path (resolved from tablespace)
    pub wal_dir: String,
    
    /// Other LSM-specific config
    pub memtable_size_mb: usize,
    pub block_cache_mb: usize,
    pub compaction_style: CompactionStyle,
}

impl LSMKVStore {
    /// Create a new LSM store with VFS and tablespace-resolved paths
    pub fn new(
        vfs: Arc<dyn DynamicFileSystem>,
        config: LSMStorageConfig,
    ) -> KeyValueResult<Self> {
        // Ensure directories exist
        vfs.create_directory_all(&config.data_dir)?;
        vfs.create_directory_all(&config.wal_dir)?;
        
        // Initialize LSM engine with VFS
        let engine = LSMTreeEngine::new(vfs.clone(), &config.data_dir)?;
        
        // Initialize WAL with VFS
        let wal_path = format!("{}/wal", config.wal_dir);
        let wal_config = WriteAheadLogConfig::default();
        let wal = WriteAheadLogManager::new(vfs.clone(), &wal_path, wal_config)?;
        
        Ok(Self {
            engine: Arc::new(RwLock::new(engine)),
            wal: Arc::new(RwLock::new(wal)),
            vfs,
            config,
        })
    }
}
```

### File: `nanograph-btree/src/kvstore.rs`

Similar modifications for B+Tree:

```rust
pub struct BTreeStorageConfig {
    pub data_dir: String,
    pub wal_dir: String,
    pub order: usize,
    pub cache_size_mb: usize,
}

impl BTreeKVStore {
    pub fn new(
        vfs: Arc<dyn DynamicFileSystem>,
        config: BTreeStorageConfig,
    ) -> KeyValueResult<Self> {
        vfs.create_directory_all(&config.data_dir)?;
        vfs.create_directory_all(&config.wal_dir)?;
        
        let tree = MvccBPlusTree::new(config.order);
        
        let wal_path = format!("{}/wal", config.wal_dir);
        let wal_config = WriteAheadLogConfig::default();
        let wal = WriteAheadLogManager::new(vfs.clone(), &wal_path, wal_config)?;
        
        Ok(Self {
            tree: Arc::new(RwLock::new(tree)),
            wal: Arc::new(RwLock::new(wal)),
            vfs,
            config,
        })
    }
}
```

### File: `nanograph-art/src/kvstore.rs`

Similar modifications for ART:

```rust
pub struct ARTStorageConfig {
    pub data_dir: String,
    pub wal_dir: String,
    pub cache_size_mb: usize,
}

impl ARTKVStore {
    pub fn new(
        vfs: Arc<dyn DynamicFileSystem>,
        config: ARTStorageConfig,
    ) -> KeyValueResult<Self> {
        vfs.create_directory_all(&config.data_dir)?;
        vfs.create_directory_all(&config.wal_dir)?;
        
        let tree = AdaptiveRadixTree::new();
        
        let wal_path = format!("{}/wal", config.wal_dir);
        let wal_config = WriteAheadLogConfig::default();
        let wal = WriteAheadLogManager::new(vfs.clone(), &wal_path, wal_config)?;
        
        Ok(Self {
            tree: Arc::new(RwLock::new(tree)),
            wal: Arc::new(RwLock::new(wal)),
            vfs,
            config,
        })
    }
}
```

## 3. Shard Manager Modifications

### File: `nanograph-kvm/src/shardmgr.rs`

Add tablespace path resolution and VFS management:

```rust
use nanograph_core::types::{ShardId, TablespaceId};
use nanograph_vfs::{FileSystemManager, DynamicFileSystem};
use std::collections::HashMap;
use std::sync::Arc;

/// Node-local tablespace configuration
#[derive(Clone, Debug)]
pub struct NodeTablespaceConfig {
    pub tablespace_id: TablespaceId,
    pub data_path: String,
    pub wal_path: Option<String>,
    pub vfs_scheme: String,
}

/// Resolves storage paths for shards based on tablespace configuration
pub struct StoragePathResolver {
    /// Node-local tablespace configurations
    node_configs: HashMap<TablespaceId, NodeTablespaceConfig>,
    
    /// VFS manager for accessing different filesystems
    vfs_manager: Arc<FileSystemManager>,
}

impl StoragePathResolver {
    pub fn new(vfs_manager: Arc<FileSystemManager>) -> Self {
        Self {
            node_configs: HashMap::new(),
            vfs_manager,
        }
    }
    
    /// Register a tablespace configuration for this node
    pub fn register_tablespace(&mut self, config: NodeTablespaceConfig) {
        self.node_configs.insert(config.tablespace_id, config);
    }
    
    /// Get the data directory path for a shard
    pub fn get_shard_data_path(
        &self,
        shard_id: ShardId,
        tablespace_id: TablespaceId,
    ) -> KeyValueResult<String> {
        let config = self.node_configs.get(&tablespace_id)
            .ok_or_else(|| KeyValueError::InvalidKey(
                format!("Tablespace {} not configured on this node", tablespace_id)
            ))?;
        
        // Format: {data_path}/shard_{table_id}_{shard_index}
        Ok(format!(
            "{}/shard_{}_{}", 
            config.data_path,
            shard_id.table().as_u64(),
            shard_id.index().as_u32()
        ))
    }
    
    /// Get the WAL directory path for a shard
    pub fn get_shard_wal_path(
        &self,
        shard_id: ShardId,
        tablespace_id: TablespaceId,
    ) -> KeyValueResult<String> {
        let config = self.node_configs.get(&tablespace_id)
            .ok_or_else(|| KeyValueError::InvalidKey(
                format!("Tablespace {} not configured on this node", tablespace_id)
            ))?;
        
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
    ) -> KeyValueResult<Arc<dyn DynamicFileSystem>> {
        let config = self.node_configs.get(&tablespace_id)
            .ok_or_else(|| KeyValueError::InvalidKey(
                format!("Tablespace {} not configured on this node", tablespace_id)
            ))?;
        
        self.vfs_manager.get_filesystem(&config.vfs_scheme)
            .ok_or_else(|| KeyValueError::InvalidKey(
                format!("VFS scheme '{}' not found", config.vfs_scheme)
            ))
    }
}

/// Modified KeyValueShardManager with tablespace support
pub struct KeyValueShardManager {
    /// Storage engines for each shard
    shards: HashMap<ShardId, Box<dyn KeyValueShardStore>>,
    
    /// Path resolver for tablespace-aware storage
    path_resolver: StoragePathResolver,
    
    /// VFS manager
    vfs_manager: Arc<FileSystemManager>,
}

impl KeyValueShardManager {
    pub fn new_standalone() -> Self {
        let vfs_manager = Arc::new(FileSystemManager::new());
        
        // Register default VFS schemes
        vfs_manager.register("local", Arc::new(LocalFilesystem::new("/")));
        vfs_manager.register("memory", Arc::new(MemoryFileSystem::new()));
        
        Self {
            shards: HashMap::new(),
            path_resolver: StoragePathResolver::new(vfs_manager.clone()),
            vfs_manager,
        }
    }
    
    /// Register a tablespace configuration for this node
    pub fn register_tablespace(&mut self, config: NodeTablespaceConfig) {
        self.path_resolver.register_tablespace(config);
    }
    
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
        let store: Box<dyn KeyValueShardStore> = match engine_type {
            EngineType::LSM => {
                let config = LSMStorageConfig {
                    data_dir: data_path,
                    wal_dir: wal_path,
                    memtable_size_mb: 64,
                    block_cache_mb: 128,
                    compaction_style: CompactionStyle::Leveled,
                };
                Box::new(LSMKVStore::new(vfs, config)?)
            }
            EngineType::BTree => {
                let config = BTreeStorageConfig {
                    data_dir: data_path,
                    wal_dir: wal_path,
                    order: 128,
                    cache_size_mb: 128,
                };
                Box::new(BTreeKVStore::new(vfs, config)?)
            }
            EngineType::ART => {
                let config = ARTStorageConfig {
                    data_dir: data_path,
                    wal_dir: wal_path,
                    cache_size_mb: 128,
                };
                Box::new(ARTKVStore::new(vfs, config)?)
            }
        };
        
        self.shards.insert(shard_id, store);
        Ok(())
    }
    
    // Existing methods remain unchanged (get, put, delete, etc.)
    // They just use self.shards as before
}
```

## 4. Database Manager Modifications

### File: `nanograph-kvm/src/database.rs`

Add tablespace metadata management and shard placement:

```rust
use nanograph_core::types::{TablespaceId, ShardId, TableId};

/// Tablespace metadata (stored in System Metadata Raft Group)
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TablespaceMetadata {
    pub id: TablespaceId,
    pub name: String,
    pub tier: StorageTier,
    pub max_size_bytes: Option<u64>,
    pub used_bytes: u64,
    pub available_nodes: Vec<NodeId>,
    pub created_at: Timestamp,
    pub last_modified: Timestamp,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum StorageTier {
    Hot,    // NVMe/Optane
    Warm,   // SSD
    Cold,   // HDD
    Archive, // Tape/Object Storage
}

impl KeyValueDatabaseManager {
    /// Create a new tablespace (cluster-wide operation via Raft)
    pub async fn create_tablespace(
        &self,
        name: String,
        tier: StorageTier,
        max_size_bytes: Option<u64>,
    ) -> KeyValueResult<TablespaceId> {
        // Allocate new tablespace ID
        let tablespace_id = self.allocate_tablespace_id().await?;
        
        let metadata = TablespaceMetadata {
            id: tablespace_id,
            name,
            tier,
            max_size_bytes,
            used_bytes: 0,
            available_nodes: Vec::new(),
            created_at: Timestamp::now(),
            last_modified: Timestamp::now(),
        };
        
        // Store in System Metadata Raft Group
        if let Some(router) = &self.raft_router {
            router
                .system_metadata()
                .create_tablespace(metadata)
                .await
                .map_err(|e| KeyValueError::Consensus(e.to_string()))?;
        } else {
            // Single-node mode: store locally
            let key = SystemKeys::tablespace_key(tablespace_id);
            let value = serialize(&metadata)?;
            let lock = self.shard_manager.read()
                .map_err(|_| KeyValueError::LockPoisoned)?;
            lock.put(ShardId::from(0), &key, &value).await?;
        }
        
        Ok(tablespace_id)
    }
    
    /// Register this node's tablespace configuration
    pub async fn register_node_tablespace(
        &self,
        config: NodeTablespaceConfig,
    ) -> KeyValueResult<()> {
        // Validate local paths exist and are writable
        self.validate_tablespace_paths(&config)?;
        
        // Register with local shard manager
        let mut lock = self.shard_manager.write()
            .map_err(|_| KeyValueError::LockPoisoned)?;
        lock.register_tablespace(config.clone());
        
        // Register with cluster via Raft
        if let Some(router) = &self.raft_router {
            let node_id = self.node_id().unwrap();
            router
                .system_metadata()
                .register_node_tablespace(node_id, config)
                .await
                .map_err(|e| KeyValueError::Consensus(e.to_string()))?;
        }
        
        Ok(())
    }
    
    /// Modified create_table to accept tablespace
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
        
        match config.sharding_config {
            TableSharding::Single => {
                let shard_id = ShardId::from_parts(table_id, ShardIndex::new(0));
                
                // Select nodes that have this tablespace configured
                let replicas = self.select_replica_nodes_for_tablespace(
                    tablespace_id,
                    config.replication_factor,
                ).await?;
                
                // Create shard via Raft with tablespace assignment
                if let Some(router) = &self.raft_router {
                    router
                        .system_metadata()
                        .create_shard(
                            shard_id,
                            tablespace_id,
                            (vec![], vec![0xFF; 32]),
                            replicas.clone(),
                        )
                        .await
                        .map_err(|e| KeyValueError::Consensus(e.to_string()))?;
                }
                
                // Create shard locally if this node is a replica
                if replicas.contains(&self.node_id().unwrap_or_default()) {
                    let mut lock = self.shard_manager.write()
                        .map_err(|_| KeyValueError::LockPoisoned)?;
                    lock.create_shard(shard_id, tablespace_id, config.engine_type).await?;
                }
            }
            
            TableSharding::Multiple { shard_count, .. } => {
                for shard_index in 0..shard_count {
                    let shard_id = ShardId::from_parts(table_id, ShardIndex::new(shard_index));
                    
                    let replicas = self.select_replica_nodes_for_tablespace(
                        tablespace_id,
                        config.replication_factor,
                    ).await?;
                    
                    if let Some(router) = &self.raft_router {
                        router
                            .system_metadata()
                            .create_shard(
                                shard_id,
                                tablespace_id,
                                self.calculate_key_range(shard_index, shard_count),
                                replicas.clone(),
                            )
                            .await
                            .map_err(|e| KeyValueError::Consensus(e.to_string()))?;
                    }
                    
                    if replicas.contains(&self.node_id().unwrap_or_default()) {
                        let mut lock = self.shard_manager.write()
                            .map_err(|_| KeyValueError::LockPoisoned)?;
                        lock.create_shard(shard_id, tablespace_id, config.engine_type).await?;
                    }
                }
            }
        }
        
        Ok(table_id)
    }
    
    /// Select replica nodes that have the required tablespace
    async fn select_replica_nodes_for_tablespace(
        &self,
        tablespace_id: TablespaceId,
        replication_factor: usize,
    ) -> KeyValueResult<Vec<NodeId>> {
        // Get tablespace metadata
        let tablespace = self.get_tablespace_metadata(tablespace_id).await?;
        
        // Filter nodes that have this tablespace configured
        let mut candidates = tablespace.available_nodes.clone();
        
        if candidates.len() < replication_factor {
            return Err(KeyValueError::InvalidKey(format!(
                "Insufficient nodes with tablespace {}: need {}, have {}",
                tablespace_id, replication_factor, candidates.len()
            )));
        }
        
        // Simple selection: take first N nodes
        // TODO: Implement rack-aware, zone-aware placement
        candidates.truncate(replication_factor);
        
        Ok(candidates)
    }
    
    /// Validate tablespace paths are accessible
    fn validate_tablespace_paths(&self, config: &NodeTablespaceConfig) -> KeyValueResult<()> {
        // Check if paths exist and are writable
        // This is a simplified version - production would do more checks
        Ok(())
    }
    
    /// Validate tablespace exists and has capacity
    async fn validate_tablespace(&self, tablespace_id: TablespaceId) -> KeyValueResult<()> {
        let metadata = self.get_tablespace_metadata(tablespace_id).await?;
        
        if let Some(max_size) = metadata.max_size_bytes {
            if metadata.used_bytes >= max_size {
                return Err(KeyValueError::InvalidKey(format!(
                    "Tablespace {} is full", tablespace_id
                )));
            }
        }
        
        Ok(())
    }
    
    /// Get tablespace metadata
    async fn get_tablespace_metadata(
        &self,
        tablespace_id: TablespaceId,
    ) -> KeyValueResult<TablespaceMetadata> {
        let key = SystemKeys::tablespace_key(tablespace_id);
        let lock = self.shard_manager.read()
            .map_err(|_| KeyValueError::LockPoisoned)?;
        
        if let Some(value) = lock.get(ShardId::from(0), &key).await? {
            Ok(deserialize(&value)?)
        } else {
            Err(KeyValueError::InvalidKey(format!(
                "Tablespace {} not found", tablespace_id
            )))
        }
    }
    
    /// Allocate a new tablespace ID
    async fn allocate_tablespace_id(&self) -> KeyValueResult<TablespaceId> {
        // TODO: Implement proper ID allocation
        // For now, use a simple counter
        Ok(TablespaceId::new(1))
    }
}
```

## 5. Configuration Loading

### File: `nanograph-kvm/src/config.rs`

Add tablespace configuration:

```rust
use nanograph_core::types::{TablespaceId, NodeId, ClusterId};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct KeyValueDatabaseConfig {
    pub node_id: NodeId,
    pub cluster_id: ClusterId,
    
    /// Node-local tablespace configurations
    pub tablespaces: Vec<NodeTablespaceConfig>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct NodeTablespaceConfig {
    pub tablespace_id: TablespaceId,
    pub data_path: String,
    pub wal_path: Option<String>,
    pub vfs_scheme: String,
}

impl KeyValueDatabaseConfig {
    /// Load configuration from TOML file
    pub fn from_file(path: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let contents = std::fs::read_to_string(path)?;
        let config: Self = toml::from_str(&contents)?;
        Ok(config)
    }
}
```

Example TOML configuration:

```toml
node_id = 1
cluster_id = 0

[[tablespaces]]
tablespace_id = 0
data_path = "/var/lib/nanograph/default"
vfs_scheme = "local"

[[tablespaces]]
tablespace_id = 1
data_path = "/mnt/nvme0/nanograph"
wal_path = "/mnt/nvme0/nanograph/wal"
vfs_scheme = "local"

[[tablespaces]]
tablespace_id = 2
data_path = "/mnt/hdd0/nanograph"
vfs_scheme = "local"
```

## 6. Initialization Flow

### Startup Sequence

```rust
// 1. Load configuration
let config = KeyValueDatabaseConfig::from_file("nanograph.toml")?;

// 2. Create database manager
let mut db_manager = if distributed_mode {
    KeyValueDatabaseManager::new_distributed(config.clone(), raft_router)
} else {
    KeyValueDatabaseManager::new_standalone(config.clone())
};

// 3. Register tablespaces
for tablespace_config in config.tablespaces {
    db_manager.register_node_tablespace(tablespace_config).await?;
}

// 4. Ready to create tables
let table_id = db_manager.create_table(
    "/mydb/users",
    "users".to_string(),
    TableCreate {
        engine_type: EngineType::LSM,
        sharding_config: TableSharding::Single,
        replication_factor: 3,
    },
    TablespaceId::new(1), // Use fast SSD tablespace
).await?;
```

## Summary of Changes

### Modified Files

1. **nanograph-core/src/types.rs**
   - Add `TablespaceId` type

2. **nanograph-lsm/src/kvstore.rs**
   - Add `LSMStorageConfig` struct
   - Modify constructor to accept VFS and config

3. **nanograph-btree/src/kvstore.rs**
   - Add `BTreeStorageConfig` struct
   - Modify constructor to accept VFS and config

4. **nanograph-art/src/kvstore.rs**
   - Add `ARTStorageConfig` struct
   - Modify constructor to accept VFS and config

5. **nanograph-kvm/src/shardmgr.rs**
   - Add `StoragePathResolver` struct
   - Add `NodeTablespaceConfig` struct
   - Modify `KeyValueShardManager` to use path resolver
   - Add `create_shard` with tablespace parameter

6. **nanograph-kvm/src/database.rs**
   - Add `TablespaceMetadata` struct
   - Add `create_tablespace` method
   - Add `register_node_tablespace` method
   - Modify `create_table` to accept tablespace parameter
   - Add tablespace-aware replica selection

7. **nanograph-kvm/src/config.rs**
   - Add tablespace configuration structs
   - Add configuration loading

### Key Principles

1. **VFS Abstraction** - All engines use VFS, no direct filesystem access
2. **Path Resolution** - Paths resolved from tablespace config at shard creation
3. **Distributed Coordination** - Tablespace metadata in System Metadata Raft Group
4. **Node-Local Storage** - Each node configures its own paths for each tablespace
5. **Placement Awareness** - Shards only placed on nodes with required tablespace

This design maintains backward compatibility by using a default tablespace (ID 0) if not specified.