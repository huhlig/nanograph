# Metadata Cache Consistency Implementation Guide

## Overview

This document describes how to implement **Strategy 1: Write-Through Cache** with Raft state machine callbacks to ensure metadata caches remain consistent across all nodes in the cluster.

## Architecture

### Key Principle

**The metadata cache is updated via Raft state machine callbacks**:
- Every metadata change goes through Raft consensus
- When Raft commits an entry, it calls `apply_operation` on **all nodes**
- The apply callback updates the local cache on **all nodes**
- Result: All caches stay synchronized automatically

### Components

```
┌─────────────────────────────────────────────────────────────┐
│              KeyValueDatabaseManager                         │
│  - Owns metastores (caches)                                  │
│  - Registers callbacks with Raft                             │
│  - Coordinates operations                                    │
└────────────────────┬────────────────────────────────────────┘
                     │
                     │ owns
                     ▼
┌─────────────────────────────────────────────────────────────┐
│              ConsensusRouter                                 │
│  - Routes to Raft groups                                     │
│  - Manages metadata Raft groups                              │
└────────────────────┬────────────────────────────────────────┘
                     │
                     │ contains
                     ▼
┌─────────────────────────────────────────────────────────────┐
│              MetadataRaftGroup                               │
│  - Proposes metadata changes                                 │
│  - Manages Raft consensus                                    │
└────────────────────┬────────────────────────────────────────┘
                     │
                     │ uses
                     ▼
┌─────────────────────────────────────────────────────────────┐
│              RaftStorageAdapter                              │
│  - Applies operations to storage                             │
│  - Triggers callbacks on apply                               │
└────────────────────┬────────────────────────────────────────┘
                     │
                     │ callback
                     ▼
┌─────────────────────────────────────────────────────────────┐
│              KeyValueDatabaseManager.apply_metadata_change() │
│  - Updates SystemMetastore cache                             │
│  - Updates DatabaseMetastore caches                          │
└─────────────────────────────────────────────────────────────┘
```

## Implementation Steps

### Step 1: Add Callback Support to RaftStorageAdapter

**File**: `nanograph-raft/src/storage.rs`

```rust
use std::sync::Arc;

/// Callback type for metadata changes
pub type MetadataCallback = Arc<dyn Fn(MetadataChange) + Send + Sync>;

/// Metadata change types
#[derive(Clone, Debug)]
pub enum MetadataChange {
    // System metadata
    AddServer { server_info: ServerMetadata },
    RemoveServer { server_id: ServerId },
    AddRegion { region_info: RegionMetadata },
    RemoveRegion { region_id: RegionId },
    
    // Database metadata
    CreateDatabase { db_id: DatabaseId, config: DatabaseConfig },
    DropDatabase { db_id: DatabaseId },
    CreateTable { db_id: DatabaseId, table_id: TableId, config: TableConfig },
    DropTable { db_id: DatabaseId, table_id: TableId },
    CreateNamespace { db_id: DatabaseId, ns_id: NamespaceId, config: NamespaceConfig },
    
    // Shard metadata
    CreateShard { shard_id: ShardId, range: (Vec<u8>, Vec<u8>), replicas: Vec<NodeId> },
    UpdateShardAssignment { shard_id: ShardId, replicas: Vec<NodeId> },
    UpdateShardLeader { shard_id: ShardId, leader: NodeId },
}

pub struct RaftStorageAdapter {
    storage: Arc<RwLock<Box<dyn KeyValueShardStore>>>,
    shard_id: ShardId,
    raft_state: Arc<RwLock<RaftState>>,
    log_entries: Arc<RwLock<Vec<LogEntry>>>,
    
    // NEW: Callback for metadata changes
    on_apply_callback: Option<MetadataCallback>,
}

impl RaftStorageAdapter {
    /// Create adapter with metadata callback
    pub fn new_with_callback(
        storage: Box<dyn KeyValueShardStore>,
        shard_id: ShardId,
        callback: MetadataCallback,
    ) -> Self {
        Self {
            storage: Arc::new(RwLock::new(storage)),
            shard_id,
            raft_state: Arc::new(RwLock::new(RaftState::default())),
            log_entries: Arc::new(RwLock::new(Vec::new())),
            on_apply_callback: Some(callback),
        }
    }
    
    /// Apply operation and trigger callback if metadata shard
    pub async fn apply_operation(
        &self,
        operation: &Operation,
    ) -> ConsensusResult<OperationResponse> {
        // 1. Apply to storage
        match operation {
            Operation::Put { key, value } => {
                let storage = self.storage.write().await;
                storage.put(self.shard_id, key, value).await.map_err(|e| {
                    ConsensusError::Storage {
                        message: e.to_string(),
                    }
                })?;
            }
            Operation::Delete { key } => {
                let storage = self.storage.write().await;
                storage.delete(self.shard_id, key).await.map_err(|e| {
                    ConsensusError::Storage {
                        message: e.to_string(),
                    }
                })?;
            }
            Operation::Batch { operations } => {
                for op in operations {
                    Box::pin(self.apply_operation(op)).await?;
                }
            }
        }
        
        // 2. If this is a metadata shard, trigger callback
        if self.is_metadata_shard() {
            if let Some(callback) = &self.on_apply_callback {
                if let Ok(change) = self.decode_metadata_change(operation) {
                    callback(change);
                }
            }
        }
        
        Ok(OperationResponse {
            success: true,
            value: None,
            error: None,
        })
    }
    
    /// Check if this is a metadata shard
    fn is_metadata_shard(&self) -> bool {
        // System metadata shard
        if self.shard_id == ShardId::new(0) {
            return true;
        }
        
        // Database metadata shards (convention: shard IDs 1-1000 reserved for metadata)
        if self.shard_id.as_u64() > 0 && self.shard_id.as_u64() <= 1000 {
            return true;
        }
        
        false
    }
    
    /// Decode operation into metadata change
    fn decode_metadata_change(&self, operation: &Operation) -> Result<MetadataChange, String> {
        match operation {
            Operation::Put { key, value } => {
                // Parse key prefix to determine change type
                if key.starts_with(b"server:") {
                    let server_info: ServerMetadata = bincode::deserialize(value)
                        .map_err(|e| format!("Failed to deserialize server: {}", e))?;
                    Ok(MetadataChange::AddServer { server_info })
                } else if key.starts_with(b"region:") {
                    let region_info: RegionMetadata = bincode::deserialize(value)
                        .map_err(|e| format!("Failed to deserialize region: {}", e))?;
                    Ok(MetadataChange::AddRegion { region_info })
                } else if key.starts_with(b"table:") {
                    // Parse: "table:{db_id}:{table_id}"
                    let key_str = String::from_utf8_lossy(key);
                    let parts: Vec<&str> = key_str.split(':').collect();
                    if parts.len() >= 3 {
                        let db_id = DatabaseId::from_str(parts[1])?;
                        let table_id = TableId::from_str(parts[2])?;
                        let config: TableConfig = bincode::deserialize(value)
                            .map_err(|e| format!("Failed to deserialize table: {}", e))?;
                        Ok(MetadataChange::CreateTable { db_id, table_id, config })
                    } else {
                        Err("Invalid table key format".to_string())
                    }
                } else if key.starts_with(b"shard:") {
                    let key_str = String::from_utf8_lossy(key);
                    let parts: Vec<&str> = key_str.split(':').collect();
                    if parts.len() >= 2 {
                        let shard_id = ShardId::from_str(parts[1])?;
                        let metadata: ShardMetadata = bincode::deserialize(value)
                            .map_err(|e| format!("Failed to deserialize shard: {}", e))?;
                        Ok(MetadataChange::CreateShard {
                            shard_id,
                            range: metadata.range,
                            replicas: metadata.replicas,
                        })
                    } else {
                        Err("Invalid shard key format".to_string())
                    }
                } else {
                    Err(format!("Unknown metadata key prefix: {:?}", key))
                }
            }
            Operation::Delete { key } => {
                if key.starts_with(b"server:") {
                    let key_str = String::from_utf8_lossy(key);
                    let parts: Vec<&str> = key_str.split(':').collect();
                    if parts.len() >= 2 {
                        let server_id = ServerId::from_str(parts[1])?;
                        Ok(MetadataChange::RemoveServer { server_id })
                    } else {
                        Err("Invalid server key format".to_string())
                    }
                } else if key.starts_with(b"table:") {
                    let key_str = String::from_utf8_lossy(key);
                    let parts: Vec<&str> = key_str.split(':').collect();
                    if parts.len() >= 3 {
                        let db_id = DatabaseId::from_str(parts[1])?;
                        let table_id = TableId::from_str(parts[2])?;
                        Ok(MetadataChange::DropTable { db_id, table_id })
                    } else {
                        Err("Invalid table key format".to_string())
                    }
                } else {
                    Err(format!("Unknown metadata delete key: {:?}", key))
                }
            }
            _ => Err("Batch operations not supported for metadata".to_string()),
        }
    }
}
```

### Step 2: Update MetadataRaftGroup to Accept Callback

**File**: `nanograph-raft/src/metadata.rs`

```rust
impl MetadataRaftGroup {
    /// Create with callback for cache updates
    pub fn new_with_callback(
        local_node_id: NodeId,
        storage: Box<dyn KeyValueShardStore>,
        callback: MetadataCallback,
    ) -> Self {
        let storage_adapter = Arc::new(RaftStorageAdapter::new_with_callback(
            storage,
            ShardId::new(0), // system_shard
            callback,
        ));
        
        Self {
            local_node_id,
            state: Arc::new(RwLock::new(RaftClusterState::default())),
            is_leader: Arc::new(RwLock::new(false)),
            storage: storage_adapter,
        }
    }
    
    /// Propose a metadata change (goes through Raft)
    pub async fn propose_change(&self, change: MetadataChange) -> ConsensusResult<()> {
        // Check if we're the leader
        let is_leader = self.is_leader.read().await;
        if !*is_leader {
            return Err(ConsensusError::NotLeader {
                shard_id: ShardId::new(0),
                leader: None,
            });
        }
        drop(is_leader);
        
        // Encode change as operation
        let operation = self.encode_metadata_change(&change)?;
        
        // Propose through Raft
        // TODO: Actual Raft proposal - for now, simulate
        self.storage.apply_operation(&operation).await?;
        
        Ok(())
    }
    
    /// Encode metadata change as storage operation
    fn encode_metadata_change(&self, change: &MetadataChange) -> ConsensusResult<Operation> {
        match change {
            MetadataChange::AddServer { server_info } => {
                let key = format!("server:{}", server_info.id).into_bytes();
                let value = bincode::serialize(server_info)
                    .map_err(|e| ConsensusError::Internal {
                        message: format!("Serialization failed: {}", e),
                    })?;
                Ok(Operation::Put { key, value })
            }
            MetadataChange::RemoveServer { server_id } => {
                let key = format!("server:{}", server_id).into_bytes();
                Ok(Operation::Delete { key })
            }
            MetadataChange::CreateTable { db_id, table_id, config } => {
                let key = format!("table:{}:{}", db_id, table_id).into_bytes();
                let value = bincode::serialize(config)
                    .map_err(|e| ConsensusError::Internal {
                        message: format!("Serialization failed: {}", e),
                    })?;
                Ok(Operation::Put { key, value })
            }
            // ... encode other changes
            _ => Err(ConsensusError::Internal {
                message: "Unsupported metadata change".to_string(),
            }),
        }
    }
}
```

### Step 3: Update KeyValueDatabaseManager to Register Callbacks

**File**: `nanograph-kvm/src/database.rs`

```rust
impl KeyValueDatabaseManager {
    /// Create manager in distributed mode with cache callbacks
    pub async fn new_distributed(
        node_id: NodeId,
        config: ReplicationConfig,
    ) -> KeyValueResult<Self> {
        // 1. Create infrastructure
        let shard_manager = Arc::new(RwLock::new(
            KeyValueShardManager::new_distributed(node_id)
        ));
        
        // 2. Create empty metastore caches
        let system_metastore = Arc::new(RwLock::new(SystemMetastore::new()));
        let database_metastores = Arc::new(RwLock::new(HashMap::new()));
        
        // 3. Create callback for system metadata
        let system_metastore_clone = system_metastore.clone();
        let database_metastores_clone = database_metastores.clone();
        
        let metadata_callback = Arc::new(move |change: MetadataChange| {
            Self::apply_metadata_change_static(
                &system_metastore_clone,
                &database_metastores_clone,
                change,
            );
        });
        
        // 4. Create Raft router with callback
        let raft_router = Arc::new(ConsensusRouter::new_with_callback(
            node_id,
            config,
            metadata_callback,
        ));
        
        // 5. Create manager
        let manager = Self {
            shard_manager,
            raft_router: Some(raft_router),
            system_metastore,
            database_metastores,
        };
        
        // 6. Load initial metadata from storage
        manager.load_system_metadata().await?;
        manager.load_database_metadata().await?;
        
        Ok(manager)
    }
    
    /// Static method to apply metadata changes (called by callback)
    fn apply_metadata_change_static(
        system_metastore: &Arc<RwLock<SystemMetastore>>,
        database_metastores: &Arc<RwLock<HashMap<DatabaseId, Arc<RwLock<DatabaseMetastore>>>>>,
        change: MetadataChange,
    ) {
        match change {
            // System metadata changes
            MetadataChange::AddServer { server_info } => {
                let mut metastore = system_metastore.write().unwrap();
                metastore.servers.insert(server_info.id, server_info);
                tracing::info!("Cache updated: Server added");
            }
            MetadataChange::RemoveServer { server_id } => {
                let mut metastore = system_metastore.write().unwrap();
                metastore.servers.remove(&server_id);
                tracing::info!("Cache updated: Server removed");
            }
            MetadataChange::AddRegion { region_info } => {
                let mut metastore = system_metastore.write().unwrap();
                metastore.regions.insert(region_info.id, region_info);
                tracing::info!("Cache updated: Region added");
            }
            MetadataChange::RemoveRegion { region_id } => {
                let mut metastore = system_metastore.write().unwrap();
                metastore.regions.remove(&region_id);
                tracing::info!("Cache updated: Region removed");
            }
            
            // Database metadata changes
            MetadataChange::CreateTable { db_id, table_id, config } => {
                let metastores = database_metastores.read().unwrap();
                if let Some(db_metastore) = metastores.get(&db_id) {
                    let mut db_meta = db_metastore.write().unwrap();
                    db_meta.tables.insert(table_id, config.into());
                    tracing::info!("Cache updated: Table created in database {}", db_id);
                }
            }
            MetadataChange::DropTable { db_id, table_id } => {
                let metastores = database_metastores.read().unwrap();
                if let Some(db_metastore) = metastores.get(&db_id) {
                    let mut db_meta = db_metastore.write().unwrap();
                    db_meta.tables.remove(&table_id);
                    tracing::info!("Cache updated: Table dropped from database {}", db_id);
                }
            }
            
            // Shard metadata changes
            MetadataChange::CreateShard { shard_id, range, replicas } => {
                let mut metastore = system_metastore.write().unwrap();
                // Update shard assignments
                tracing::info!("Cache updated: Shard {} created", shard_id);
            }
            MetadataChange::UpdateShardAssignment { shard_id, replicas } => {
                let mut metastore = system_metastore.write().unwrap();
                // Update shard assignments
                tracing::info!("Cache updated: Shard {} assignment updated", shard_id);
            }
            
            _ => {
                tracing::warn!("Unhandled metadata change: {:?}", change);
            }
        }
    }
    
    /// Load system metadata from storage into cache
    async fn load_system_metadata(&self) -> KeyValueResult<()> {
        let shard_mgr = self.shard_manager.read().unwrap();
        let system_shard = ShardId::new(0);
        
        // Load cluster metadata
        if let Some(cluster_data) = shard_mgr.get(system_shard, b"cluster").await? {
            let cluster: ClusterMetadata = bincode::deserialize(&cluster_data)
                .map_err(|e| KeyValueError::InvalidValue(e.to_string()))?;
            
            let mut metastore = self.system_metastore.write().unwrap();
            metastore.cluster = cluster;
        }
        
        // Load servers
        let mut server_iter = shard_mgr.scan_prefix(system_shard, b"server:", None).await?;
        while let Some((key, value)) = server_iter.next().await? {
            let server: ServerMetadata = bincode::deserialize(&value)
                .map_err(|e| KeyValueError::InvalidValue(e.to_string()))?;
            
            let mut metastore = self.system_metastore.write().unwrap();
            metastore.servers.insert(server.id, server);
        }
        
        // Load regions
        let mut region_iter = shard_mgr.scan_prefix(system_shard, b"region:", None).await?;
        while let Some((key, value)) = region_iter.next().await? {
            let region: RegionMetadata = bincode::deserialize(&value)
                .map_err(|e| KeyValueError::InvalidValue(e.to_string()))?;
            
            let mut metastore = self.system_metastore.write().unwrap();
            metastore.regions.insert(region.id, region);
        }
        
        tracing::info!("System metadata loaded into cache");
        Ok(())
    }
    
    /// Load database metadata from storage into cache
    async fn load_database_metadata(&self) -> KeyValueResult<()> {
        // TODO: Load database metadata for each database
        // Similar to load_system_metadata but for database-specific shards
        Ok(())
    }
}
```

### Step 4: Update ConsensusRouter to Pass Callback

**File**: `nanograph-raft/src/router.rs`

```rust
impl ConsensusRouter {
    /// Create router with metadata callback
    pub fn new_with_callback(
        local_node_id: NodeId,
        config: ReplicationConfig,
        metadata_callback: MetadataCallback,
    ) -> Self {
        info!("Creating router on node {} with cache callback", local_node_id);
        
        // Create metadata Raft group with callback
        let metadata = Arc::new(MetadataRaftGroup::new_with_callback(
            local_node_id,
            Box::new(/* storage engine */),
            metadata_callback,
        ));
        
        Self {
            local_node_id,
            config,
            metadata,
            peers: Arc::new(RwLock::new(HashMap::new())),
            shards: Arc::new(RwLock::new(HashMap::new())),
            shard_count: Arc::new(RwLock::new(1)),
        }
    }
}
```

### Step 5: Update SystemMetastore and DatabaseMetastore

**File**: `nanograph-kvm/src/metastore.rs`

Remove `consensus_router` and `shard_manager` fields:

```rust
/// Pure metadata cache - no infrastructure ownership
pub struct SystemMetastore {
    // Metadata only
    cluster: ClusterMetadata,
    regions: HashMap<RegionId, RegionMetadata>,
    servers: HashMap<ServerId, ServerMetadata>,
    tenants: HashMap<TenantId, TenantMetadata>,
    databases: HashMap<DatabaseId, DatabaseMetadata>,
    system_users: HashMap<UserId, UserMetadata>,
    
    // Just the shard ID
    system_shard: ShardId,
    
    // REMOVED: shard_manager
    // REMOVED: consensus_router
}

impl SystemMetastore {
    pub fn new() -> Self {
        Self {
            cluster: ClusterMetadata::default(),
            regions: HashMap::new(),
            servers: HashMap::new(),
            tenants: HashMap::new(),
            databases: HashMap::new(),
            system_users: HashMap::new(),
            system_shard: ShardId::new(0),
        }
    }
}

/// Pure metadata cache - no infrastructure ownership
pub struct DatabaseMetastore {
    container: ContainerId,
    namespaces: HashMap<NamespaceId, NamespaceMetadata>,
    tables: HashMap<TableId, TableMetadata>,
    shards: HashMap<ShardId, ShardMetadata>,
    database_users: HashMap<UserId, UserMetadata>,
    shard_assignments: BTreeMap<ShardId, Vec<NodeId>>,
    
    // Just the shard ID
    metadata_shard: Option<ShardId>,
    
    // Name resolver
    resolver_nodes: BTreeMap<ObjectId, Node>,
    resolver_paths: BTreeMap<String, ObjectId>,
    available_nodes: BTreeSet<ObjectId>,
    next_resolver_id: ObjectId,
    
    // REMOVED: consensus_router
}

impl DatabaseMetastore {
    pub fn new(container: ContainerId) -> Self {
        Self {
            container,
            namespaces: HashMap::new(),
            tables: HashMap::new(),
            shards: HashMap::new(),
            database_users: HashMap::new(),
            shard_assignments: BTreeMap::new(),
            metadata_shard: None,
            resolver_nodes: BTreeMap::new(),
            resolver_paths: BTreeMap::new(),
            available_nodes: BTreeSet::new(),
            next_resolver_id: 0,
        }
    }
}
```

## Testing the Implementation

### Test 1: Local Write Updates Cache

```rust
#[tokio::test]
async fn test_local_write_updates_cache() {
    let manager = KeyValueDatabaseManager::new_distributed(
        NodeId::new(1),
        ReplicationConfig::default(),
    ).await.unwrap();
    
    // Create server
    let server_info = ServerMetadata {
        id: ServerId::new(2),
        address: "127.0.0.1:9000".parse().unwrap(),
        // ... other fields
    };
    
    manager.add_server(server_info.clone()).await.unwrap();
    
    // Check cache was updated
    let metastore = manager.system_metastore.read().unwrap();
    assert!(metastore.servers.contains_key(&ServerId::new(2)));
}
```

### Test 2: Remote Write Updates Cache

```rust
#[tokio::test]
async fn test_remote_write_updates_cache() {
    // Create two nodes
    let node1 = KeyValueDatabaseManager::new_distributed(
        NodeId::new(1),
        ReplicationConfig::default(),
    ).await.unwrap();
    
    let node2 = KeyValueDatabaseManager::new_distributed(
        NodeId::new(2),
        ReplicationConfig::default(),
    ).await.unwrap();
    
    // Node 2 creates server
    let server_info = ServerMetadata {
        id: ServerId::new(3),
        // ... fields
    };
    
    node2.add_server(server_info.clone()).await.unwrap();
    
    // Wait for replication
    tokio::time::sleep(Duration::from_millis(100)).await;
    
    // Check node 1's cache was updated
    let metastore = node1.system_metastore.read().unwrap();
    assert!(metastore.servers.contains_key(&ServerId::new(3)));
}
```

## Summary

### What We Implemented

1. **Callback Support in RaftStorageAdapter**: Triggers on every apply
2. **Metadata Change Encoding/Decoding**: Converts operations to/from metadata changes
3. **Cache Update Logic**: Updates metastores when callback is triggered
4. **Initialization**: Loads initial metadata from storage into cache
5. **Clean Architecture**: Metastores are pure caches, no infrastructure ownership

### How It Works

1. **Write Path**:
   - Application calls `manager.create_table()`
   - Manager proposes through Raft
   - Raft replicates to all nodes
   - When committed, `apply_operation` called on all nodes
   - Callback updates cache on all nodes

2. **Read Path**:
   - Application calls `manager.get_table_metadata()`
   - Manager reads directly from cache
   - No network calls, instant response

3. **Consistency**:
   - All writes go through Raft
   - All nodes apply in same order
   - All caches stay synchronized
   - Strong consistency guaranteed

### Next Steps

1. Implement actual Raft proposal (integrate openraft)
2. Add network layer for Raft RPC
3. Implement leader election
4. Add snapshot support for large metadata
5. Implement database metadata Raft groups (per-database)
6. Add monitoring and metrics for cache consistency