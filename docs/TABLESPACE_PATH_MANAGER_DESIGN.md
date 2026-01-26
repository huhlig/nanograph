# Tablespace Path Manager Design

## Overview

The `TablespacePathManager` is a centralized component responsible for all filesystem path resolution in Nanograph. It provides a unified interface for generating, validating, and managing paths across the entire data directory hierarchy.

## Current State Analysis

### Existing Implementation

The current `StoragePathResolver` in `nanograph-kvt/src/tablespace.rs` provides basic path resolution:

**Strengths:**
- Simple tablespace registration
- Basic data/WAL path resolution
- Storage tier awareness

**Limitations:**
- Only handles shard-level paths
- No support for system metadata, containers, indexes, embeddings, etc.
- No path validation or creation
- No support for snapshots, backups, temp files
- Limited error handling
- No path normalization

## Proposed Design

### Architecture

```
TablespacePathManager
├── Core Path Resolution
│   ├── System Metadata Paths
│   ├── Container Metadata Paths
│   ├── Tenant Data Paths
│   ├── Shard Data Paths
│   ├── Index Paths
│   └── Embedding Paths
├── Auxiliary Path Resolution
│   ├── Snapshot Paths
│   ├── Backup Paths
│   ├── Temporary Paths
│   ├── Cache Paths
│   └── Log Paths
├── Path Operations
│   ├── Validation
│   ├── Creation
│   ├── Cleanup
│   └── Migration
└── Configuration Management
    ├── Tablespace Registration
    ├── Tier Management
    └── Quota Enforcement
```

### Core Types

```rust
use nanograph_core::object::{
    ClusterId, ContainerId, DatabaseId, IndexId, NamespaceId, 
    ShardId, TableId, TablespaceId, TenantId, StorageEngineType
};
use nanograph_vfs::Path;
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

/// Storage tier classification
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum StorageTier {
    /// Hot tier - fastest storage (NVMe, RAM)
    Hot,
    /// Warm tier - balanced storage (SSD)
    Warm,
    /// Cold tier - slower storage (HDD)
    Cold,
    /// Archive tier - archival storage (object storage, tape)
    Archive,
}

/// Tablespace configuration for a node
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TablespaceConfig {
    /// Tablespace ID (cluster-wide)
    pub id: TablespaceId,
    
    /// Human-readable name
    pub name: String,
    
    /// Base path for this tablespace on this node
    pub base_path: Path,
    
    /// Storage tier
    pub tier: StorageTier,
    
    /// Whether this tablespace is available on this node
    pub available: bool,
    
    /// Maximum size in bytes (None = unlimited)
    pub max_size_bytes: Option<u64>,
    
    /// Current usage in bytes
    pub used_bytes: u64,
    
    /// Separate WAL path (optional, defaults to base_path/wal)
    pub wal_path: Option<Path>,
    
    /// VFS scheme for this tablespace (e.g., "local", "s3", "memory")
    pub vfs_scheme: String,
}

/// Path type enumeration for different components
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PathType {
    /// System metadata
    System,
    /// Container metadata
    Container,
    /// Tenant data
    Tenant,
    /// Shard data
    Shard,
    /// Index data
    Index,
    /// Embedding models
    Embedding,
    /// Snapshot
    Snapshot,
    /// Backup
    Backup,
    /// Temporary files
    Temp,
    /// Cache
    Cache,
    /// Logs
    Log,
}

/// Path component for building hierarchical paths
#[derive(Debug, Clone)]
pub enum PathComponent {
    System,
    Container(ContainerId),
    Tenant(TenantId),
    Database(DatabaseId),
    Namespace(NamespaceId),
    Table(TableId),
    Shard(ShardId),
    Index(IndexId),
    Engine(StorageEngineType),
    Data,
    Wal,
    Raft,
    Snapshot(String), // timestamp
    Backup(String),   // backup_id
    Temp(String),     // operation_id
}
```

### Main Manager Implementation

```rust
/// Centralized path management for all Nanograph components
pub struct TablespacePathManager {
    /// Registered tablespace configurations
    tablespaces: Arc<RwLock<HashMap<TablespaceId, TablespaceConfig>>>,
    
    /// Default tablespace for new objects
    default_tablespace: TablespaceId,
    
    /// VFS instance for filesystem operations
    vfs: Arc<dyn DynamicFileSystem>,
    
    /// Path cache for frequently accessed paths
    path_cache: Arc<RwLock<HashMap<String, Path>>>,
    
    /// Enable path validation
    validate_paths: bool,
}

impl TablespacePathManager {
    /// Create a new path manager
    pub fn new(
        default_tablespace: TablespaceId,
        vfs: Arc<dyn DynamicFileSystem>,
    ) -> Self {
        Self {
            tablespaces: Arc::new(RwLock::new(HashMap::new())),
            default_tablespace,
            vfs,
            path_cache: Arc::new(RwLock::new(HashMap::new())),
            validate_paths: true,
        }
    }
    
    /// Register a tablespace configuration
    pub fn register_tablespace(&self, config: TablespaceConfig) -> Result<()> {
        // Validate configuration
        self.validate_tablespace_config(&config)?;
        
        // Create base directory if it doesn't exist
        if config.available {
            self.vfs.create_directory_all(&config.base_path)?;
        }
        
        let mut tablespaces = self.tablespaces.write().unwrap();
        tablespaces.insert(config.id, config);
        
        Ok(())
    }
    
    /// Get tablespace configuration
    pub fn get_tablespace(&self, id: TablespaceId) -> Result<TablespaceConfig> {
        let tablespaces = self.tablespaces.read().unwrap();
        tablespaces
            .get(&id)
            .cloned()
            .ok_or_else(|| Error::TablespaceNotFound(id))
    }
    
    /// Update tablespace usage
    pub fn update_usage(&self, id: TablespaceId, delta_bytes: i64) -> Result<()> {
        let mut tablespaces = self.tablespaces.write().unwrap();
        if let Some(config) = tablespaces.get_mut(&id) {
            if delta_bytes > 0 {
                let new_usage = config.used_bytes.saturating_add(delta_bytes as u64);
                
                // Check quota
                if let Some(max_size) = config.max_size_bytes {
                    if new_usage > max_size {
                        return Err(Error::TablespaceQuotaExceeded(id, max_size));
                    }
                }
                
                config.used_bytes = new_usage;
            } else {
                config.used_bytes = config.used_bytes.saturating_sub((-delta_bytes) as u64);
            }
            Ok(())
        } else {
            Err(Error::TablespaceNotFound(id))
        }
    }
}
```

### System Metadata Paths

```rust
impl TablespacePathManager {
    /// Get system metadata base path
    pub fn system_base_path(&self, tablespace_id: TablespaceId) -> Result<Path> {
        let config = self.get_tablespace(tablespace_id)?;
        let mut path = config.base_path.clone();
        path.push("system");
        Ok(path)
    }
    
    /// Get system data path
    pub fn system_data_path(&self, tablespace_id: TablespaceId) -> Result<Path> {
        let mut path = self.system_base_path(tablespace_id)?;
        path.push("data");
        Ok(path)
    }
    
    /// Get system WAL path
    pub fn system_wal_path(&self, tablespace_id: TablespaceId) -> Result<Path> {
        let mut path = self.system_base_path(tablespace_id)?;
        path.push("wal");
        Ok(path)
    }
    
    /// Get system Raft path
    pub fn system_raft_path(&self, tablespace_id: TablespaceId) -> Result<Path> {
        let mut path = self.system_base_path(tablespace_id)?;
        path.push("raft");
        Ok(path)
    }
    
    /// Get system Raft logs path
    pub fn system_raft_logs_path(&self, tablespace_id: TablespaceId) -> Result<Path> {
        let mut path = self.system_raft_path(tablespace_id)?;
        path.push("logs");
        Ok(path)
    }
    
    /// Get system Raft snapshots path
    pub fn system_raft_snapshots_path(&self, tablespace_id: TablespaceId) -> Result<Path> {
        let mut path = self.system_raft_path(tablespace_id)?;
        path.push("snapshots");
        Ok(path)
    }
}
```

### Container Metadata Paths

```rust
impl TablespacePathManager {
    /// Get container base path
    pub fn container_base_path(
        &self,
        tablespace_id: TablespaceId,
        container_id: ContainerId,
    ) -> Result<Path> {
        let config = self.get_tablespace(tablespace_id)?;
        let mut path = config.base_path.clone();
        path.push("containers");
        path.push(format!("{}", container_id.0));
        Ok(path)
    }
    
    /// Get container data path
    pub fn container_data_path(
        &self,
        tablespace_id: TablespaceId,
        container_id: ContainerId,
    ) -> Result<Path> {
        let mut path = self.container_base_path(tablespace_id, container_id)?;
        path.push("data");
        Ok(path)
    }
    
    /// Get container Raft path
    pub fn container_raft_path(
        &self,
        tablespace_id: TablespaceId,
        container_id: ContainerId,
    ) -> Result<Path> {
        let mut path = self.container_base_path(tablespace_id, container_id)?;
        path.push("raft");
        Ok(path)
    }
}
```

### Tenant/Database/Namespace/Table Hierarchy

```rust
impl TablespacePathManager {
    /// Get tenant base path
    pub fn tenant_base_path(
        &self,
        tablespace_id: TablespaceId,
        tenant_id: TenantId,
    ) -> Result<Path> {
        let config = self.get_tablespace(tablespace_id)?;
        let mut path = config.base_path.clone();
        path.push("tenants");
        path.push(format!("{}", tenant_id.0));
        Ok(path)
    }
    
    /// Get database base path
    pub fn database_base_path(
        &self,
        tablespace_id: TablespaceId,
        tenant_id: TenantId,
        database_id: DatabaseId,
    ) -> Result<Path> {
        let mut path = self.tenant_base_path(tablespace_id, tenant_id)?;
        path.push("databases");
        path.push(format!("{}", database_id.0));
        Ok(path)
    }
    
    /// Get namespace base path
    pub fn namespace_base_path(
        &self,
        tablespace_id: TablespaceId,
        tenant_id: TenantId,
        database_id: DatabaseId,
        namespace_id: NamespaceId,
    ) -> Result<Path> {
        let mut path = self.database_base_path(tablespace_id, tenant_id, database_id)?;
        path.push("namespaces");
        path.push(format!("{}", namespace_id.0));
        Ok(path)
    }
    
    /// Get table base path
    pub fn table_base_path(
        &self,
        tablespace_id: TablespaceId,
        tenant_id: TenantId,
        database_id: DatabaseId,
        namespace_id: NamespaceId,
        table_id: TableId,
    ) -> Result<Path> {
        let mut path = self.namespace_base_path(
            tablespace_id,
            tenant_id,
            database_id,
            namespace_id,
        )?;
        path.push("tables");
        path.push(format!("{}", table_id.0));
        Ok(path)
    }
    
    /// Get table metadata path
    pub fn table_metadata_path(
        &self,
        tablespace_id: TablespaceId,
        tenant_id: TenantId,
        database_id: DatabaseId,
        namespace_id: NamespaceId,
        table_id: TableId,
    ) -> Result<Path> {
        let mut path = self.table_base_path(
            tablespace_id,
            tenant_id,
            database_id,
            namespace_id,
            table_id,
        )?;
        path.push("metadata.json");
        Ok(path)
    }
}
```

### Shard Data Paths

```rust
impl TablespacePathManager {
    /// Get shard base path
    pub fn shard_base_path(
        &self,
        tablespace_id: TablespaceId,
        tenant_id: TenantId,
        database_id: DatabaseId,
        namespace_id: NamespaceId,
        table_id: TableId,
        shard_id: ShardId,
    ) -> Result<Path> {
        let mut path = self.table_base_path(
            tablespace_id,
            tenant_id,
            database_id,
            namespace_id,
            table_id,
        )?;
        path.push("shards");
        path.push(format!("{}", shard_id.index().as_u32()));
        Ok(path)
    }
    
    /// Get shard engine base path
    pub fn shard_engine_path(
        &self,
        tablespace_id: TablespaceId,
        tenant_id: TenantId,
        database_id: DatabaseId,
        namespace_id: NamespaceId,
        table_id: TableId,
        shard_id: ShardId,
        engine_type: StorageEngineType,
    ) -> Result<Path> {
        let mut path = self.shard_base_path(
            tablespace_id,
            tenant_id,
            database_id,
            namespace_id,
            table_id,
            shard_id,
        )?;
        path.push(engine_type.to_string().to_lowercase());
        Ok(path)
    }
    
    /// Get shard data path
    pub fn shard_data_path(
        &self,
        tablespace_id: TablespaceId,
        tenant_id: TenantId,
        database_id: DatabaseId,
        namespace_id: NamespaceId,
        table_id: TableId,
        shard_id: ShardId,
        engine_type: StorageEngineType,
    ) -> Result<Path> {
        let mut path = self.shard_engine_path(
            tablespace_id,
            tenant_id,
            database_id,
            namespace_id,
            table_id,
            shard_id,
            engine_type,
        )?;
        path.push("data");
        Ok(path)
    }
    
    /// Get shard WAL path
    pub fn shard_wal_path(
        &self,
        tablespace_id: TablespaceId,
        tenant_id: TenantId,
        database_id: DatabaseId,
        namespace_id: NamespaceId,
        table_id: TableId,
        shard_id: ShardId,
        engine_type: StorageEngineType,
    ) -> Result<Path> {
        let config = self.get_tablespace(tablespace_id)?;
        
        // Check if tablespace has separate WAL path
        if let Some(wal_base) = &config.wal_path {
            let mut path = wal_base.clone();
            path.push("tenants");
            path.push(format!("{}", tenant_id.0));
            path.push("databases");
            path.push(format!("{}", database_id.0));
            path.push("namespaces");
            path.push(format!("{}", namespace_id.0));
            path.push("tables");
            path.push(format!("{}", table_id.0));
            path.push("shards");
            path.push(format!("{}", shard_id.index().as_u32()));
            path.push(engine_type.to_string().to_lowercase());
            path.push("wal");
            Ok(path)
        } else {
            // Use default location under data path
            let mut path = self.shard_engine_path(
                tablespace_id,
                tenant_id,
                database_id,
                namespace_id,
                table_id,
                shard_id,
                engine_type,
            )?;
            path.push("wal");
            Ok(path)
        }
    }
    
    /// Get shard Raft path (for replicated shards)
    pub fn shard_raft_path(
        &self,
        tablespace_id: TablespaceId,
        tenant_id: TenantId,
        database_id: DatabaseId,
        namespace_id: NamespaceId,
        table_id: TableId,
        shard_id: ShardId,
        engine_type: StorageEngineType,
    ) -> Result<Path> {
        let mut path = self.shard_engine_path(
            tablespace_id,
            tenant_id,
            database_id,
            namespace_id,
            table_id,
            shard_id,
            engine_type,
        )?;
        path.push("raft");
        Ok(path)
    }
    
    /// Get LSM level path
    pub fn lsm_level_path(
        &self,
        tablespace_id: TablespaceId,
        tenant_id: TenantId,
        database_id: DatabaseId,
        namespace_id: NamespaceId,
        table_id: TableId,
        shard_id: ShardId,
        level: u8,
    ) -> Result<Path> {
        let mut path = self.shard_data_path(
            tablespace_id,
            tenant_id,
            database_id,
            namespace_id,
            table_id,
            shard_id,
            StorageEngineType::LSM,
        )?;
        path.push(format!("l{}", level));
        Ok(path)
    }
}
```

### Snapshot Paths

```rust
impl TablespacePathManager {
    /// Get shard snapshot base path
    pub fn shard_snapshots_path(
        &self,
        tablespace_id: TablespaceId,
        tenant_id: TenantId,
        database_id: DatabaseId,
        namespace_id: NamespaceId,
        table_id: TableId,
        shard_id: ShardId,
        engine_type: StorageEngineType,
    ) -> Result<Path> {
        let mut path = self.shard_engine_path(
            tablespace_id,
            tenant_id,
            database_id,
            namespace_id,
            table_id,
            shard_id,
            engine_type,
        )?;
        path.push("snapshots");
        Ok(path)
    }
    
    /// Get specific snapshot path
    pub fn shard_snapshot_path(
        &self,
        tablespace_id: TablespaceId,
        tenant_id: TenantId,
        database_id: DatabaseId,
        namespace_id: NamespaceId,
        table_id: TableId,
        shard_id: ShardId,
        engine_type: StorageEngineType,
        timestamp: &str,
    ) -> Result<Path> {
        let mut path = self.shard_snapshots_path(
            tablespace_id,
            tenant_id,
            database_id,
            namespace_id,
            table_id,
            shard_id,
            engine_type,
        )?;
        path.push(timestamp);
        Ok(path)
    }
    
    /// Generate snapshot timestamp
    pub fn generate_snapshot_timestamp() -> String {
        use chrono::Utc;
        Utc::now().format("%Y-%m-%dT%H-%M-%SZ").to_string()
    }
}
```

### Index Paths

```rust
impl TablespacePathManager {
    /// Get index base path
    pub fn index_base_path(
        &self,
        tablespace_id: TablespaceId,
        tenant_id: TenantId,
        database_id: DatabaseId,
        namespace_id: NamespaceId,
        table_id: TableId,
        index_id: IndexId,
    ) -> Result<Path> {
        let config = self.get_tablespace(tablespace_id)?;
        let mut path = config.base_path.clone();
        path.push("indexes");
        path.push(format!("{}", tenant_id.0));
        path.push(format!("{}", database_id.0));
        path.push(format!("{}", namespace_id.0));
        path.push(format!("{}", table_id.0));
        path.push(format!("{}", index_id.0));
        Ok(path)
    }
    
    /// Get vector index path
    pub fn vector_index_path(
        &self,
        tablespace_id: TablespaceId,
        tenant_id: TenantId,
        database_id: DatabaseId,
        namespace_id: NamespaceId,
        table_id: TableId,
        index_id: IndexId,
        index_type: &str, // "hnsw" or "ivf"
    ) -> Result<Path> {
        let mut path = self.index_base_path(
            tablespace_id,
            tenant_id,
            database_id,
            namespace_id,
            table_id,
            index_id,
        )?;
        path.push("vector");
        path.push(index_type);
        Ok(path)
    }
    
    /// Get text index path
    pub fn text_index_path(
        &self,
        tablespace_id: TablespaceId,
        tenant_id: TenantId,
        database_id: DatabaseId,
        namespace_id: NamespaceId,
        table_id: TableId,
        index_id: IndexId,
    ) -> Result<Path> {
        let mut path = self.index_base_path(
            tablespace_id,
            tenant_id,
            database_id,
            namespace_id,
            table_id,
            index_id,
        )?;
        path.push("text");
        Ok(path)
    }
}
```

### Embedding Model Paths

```rust
impl TablespacePathManager {
    /// Get embeddings base path
    pub fn embeddings_base_path(&self, tablespace_id: TablespaceId) -> Result<Path> {
        let config = self.get_tablespace(tablespace_id)?;
        let mut path = config.base_path.clone();
        path.push("embeddings");
        path.push("models");
        Ok(path)
    }
    
    /// Get model base path
    pub fn model_base_path(
        &self,
        tablespace_id: TablespaceId,
        model_id: &str,
    ) -> Result<Path> {
        let mut path = self.embeddings_base_path(tablespace_id)?;
        path.push(model_id);
        Ok(path)
    }
    
    /// Get model version path
    pub fn model_version_path(
        &self,
        tablespace_id: TablespaceId,
        model_id: &str,
        version: &str,
    ) -> Result<Path> {
        let mut path = self.model_base_path(tablespace_id, model_id)?;
        path.push(version);
        Ok(path)
    }
}
```

### Backup and Temporary Paths

```rust
impl TablespacePathManager {
    /// Get backups base path
    pub fn backups_base_path(&self, tablespace_id: TablespaceId) -> Result<Path> {
        let config = self.get_tablespace(tablespace_id)?;
        let mut path = config.base_path.clone();
        path.push("backups");
        Ok(path)
    }
    
    /// Get specific backup path
    pub fn backup_path(
        &self,
        tablespace_id: TablespaceId,
        backup_id: &str,
    ) -> Result<Path> {
        let mut path = self.backups_base_path(tablespace_id)?;
        path.push(backup_id);
        Ok(path)
    }
    
    /// Get temp base path
    pub fn temp_base_path(&self, tablespace_id: TablespaceId) -> Result<Path> {
        let config = self.get_tablespace(tablespace_id)?;
        let mut path = config.base_path.clone();
        path.push("temp");
        Ok(path)
    }
    
    /// Get compaction temp path
    pub fn compaction_temp_path(
        &self,
        tablespace_id: TablespaceId,
        compaction_id: &str,
    ) -> Result<Path> {
        let mut path = self.temp_base_path(tablespace_id)?;
        path.push("compaction");
        path.push(compaction_id);
        Ok(path)
    }
    
    /// Get cache base path
    pub fn cache_base_path(&self, tablespace_id: TablespaceId) -> Result<Path> {
        let config = self.get_tablespace(tablespace_id)?;
        let mut path = config.base_path.clone();
        path.push("cache");
        Ok(path)
    }
    
    /// Get logs base path
    pub fn logs_base_path(&self, tablespace_id: TablespaceId) -> Result<Path> {
        let config = self.get_tablespace(tablespace_id)?;
        let mut path = config.base_path.clone();
        path.push("logs");
        Ok(path)
    }
}
```

### Path Operations

```rust
impl TablespacePathManager {
    /// Ensure a path exists (create if necessary)
    pub fn ensure_path_exists(&self, path: &Path) -> Result<()> {
        if self.validate_paths {
            self.vfs.create_directory_all(path)?;
        }
        Ok(())
    }
    
    /// Validate tablespace configuration
    fn validate_tablespace_config(&self, config: &TablespaceConfig) -> Result<()> {
        // Check base path is absolute
        if !config.base_path.is_absolute() {
            return Err(Error::InvalidPath(
                "Tablespace base path must be absolute".to_string()
            ));
        }
        
        // Check VFS scheme is valid
        if config.vfs_scheme.is_empty() {
            return Err(Error::InvalidConfiguration(
                "VFS scheme cannot be empty".to_string()
            ));
        }
        
        // Check quota is reasonable
        if let Some(max_size) = config.max_size_bytes {
            if max_size == 0 {
                return Err(Error::InvalidConfiguration(
                    "Tablespace max size cannot be zero".to_string()
                ));
            }
        }
        
        Ok(())
    }
    
    /// Clean up temporary files older than specified duration
    pub fn cleanup_temp_files(
        &self,
        tablespace_id: TablespaceId,
        max_age_seconds: u64,
    ) -> Result<usize> {
        let temp_path = self.temp_base_path(tablespace_id)?;
        let mut cleaned = 0;
        
        // Implementation would iterate through temp directories
        // and remove old files
        
        Ok(cleaned)
    }
    
    /// Calculate directory size
    pub fn calculate_directory_size(&self, path: &Path) -> Result<u64> {
        // Implementation would recursively calculate size
        Ok(0)
    }
    
    /// List all paths for a shard (for backup/migration)
    pub fn list_shard_paths(
        &self,
        tablespace_id: TablespaceId,
        tenant_id: TenantId,
        database_id: DatabaseId,
        namespace_id: NamespaceId,
        table_id: TableId,
        shard_id: ShardId,
        engine_type: StorageEngineType,
    ) -> Result<Vec<Path>> {
        let mut paths = Vec::new();
        
        paths.push(self.shard_data_path(
            tablespace_id, tenant_id, database_id, namespace_id,
            table_id, shard_id, engine_type.clone()
        )?);
        
        paths.push(self.shard_wal_path(
            tablespace_id, tenant_id, database_id, namespace_id,
            table_id, shard_id, engine_type.clone()
        )?);
        
        paths.push(self.shard_snapshots_path(
            tablespace_id, tenant_id, database_id, namespace_id,
            table_id, shard_id, engine_type.clone()
        )?);
        
        if self.is_replicated() {
            paths.push(self.shard_raft_path(
                tablespace_id, tenant_id, database_id, namespace_id,
                table_id, shard_id, engine_type
            )?);
        }
        
        Ok(paths)
    }
    
    fn is_replicated(&self) -> bool {
        // Check if system is in distributed mode
        true
    }
}
```

### Builder Pattern for Complex Paths

```rust
/// Builder for constructing paths with fluent API
pub struct PathBuilder<'a> {
    manager: &'a TablespacePathManager,
    tablespace_id: TablespaceId,
    components: Vec<PathComponent>,
}

impl<'a> PathBuilder<'a> {
    pub fn new(manager: &'a TablespacePathManager, tablespace_id: TablespaceId) -> Self {
        Self {
            manager,
            tablespace_id,
            components: Vec::new(),
        }
    }
    
    pub fn system(mut self) -> Self {
        self.components.push(PathComponent::System);
        self
    }
    
    pub fn tenant(mut self, id: TenantId) -> Self {
        self.components.push(PathComponent::Tenant(id));
        self
    }
    
    pub fn database(mut self, id: DatabaseId) -> Self {
        self.components.push(PathComponent::Database(id));
        self
    }
    
    pub fn namespace(mut self, id: NamespaceId) -> Self {
        self.components.push(PathComponent::Namespace(id));
        self
    }
    
    pub fn table(mut self, id: TableId) -> Self {
        self.components.push(PathComponent::Table(id));
        self
    }
    
    pub fn shard(mut self, id: ShardId) -> Self {
        self.components.push(PathComponent::Shard(id));
        self
    }
    
    pub fn engine(mut self, engine_type: StorageEngineType) -> Self {
        self.components.push(PathComponent::Engine(engine_type));
        self
    }
    
    pub fn data(mut self) -> Self {
        self.components.push(PathComponent::Data);
        self
    }
    
    pub fn wal(mut self) -> Self {
        self.components.push(PathComponent::Wal);
        self
    }
    
    pub fn build(self) -> Result<Path> {
        let config = self.manager.get_tablespace(self.tablespace_id)?;
        let mut path = config.base_path.clone();
        
        for component in self.components {
            match component {
                PathComponent::System => path.push("system"),
                PathComponent::Tenant(id) => {
                    path.push("tenants");
                    path.push(format!("{}", id.0));
                }
                PathComponent::Database(id) => {
                    path.push("databases");
                    path.push(format!("{}", id.0));
                }
                PathComponent::Namespace(id) => {
                    path.push("namespaces");
                    path.push(format!("{}", id.0));
                }
                PathComponent::Table(id) => {
                    path.push("tables");
                    path.push(format!("{}", id.0));
                }
                PathComponent::Shard(id) => {
                    path.push("shards");
                    path.push(format!("{}", id.index().as_u32()));
                }
                PathComponent::Engine(engine_type) => {
                    path.push(engine_type.to_string().to_lowercase());
                }
                PathComponent::Data => path.push("data"),
                PathComponent::Wal => path.push("wal"),
                PathComponent::Raft => path.push("raft"),
                PathComponent::Snapshot(ts) => {
                    path.push("snapshots");
                    path.push(ts);
                }
                _ => {}
            }
        }
        
        Ok(path)
    }
}

// Usage example:
// let path = PathBuilder::new(&manager, tablespace_id)
//     .tenant(tenant_id)
//     .database(db_id)
//     .namespace(ns_id)
//     .table(table_id)
//     .shard(shard_id)
//     .engine(StorageEngineType::LSM)
//     .data()
//     .build()?;
```

## Integration Points

### With ShardManager

```rust
// In KeyValueShardManager
impl KeyValueShardManager {
    pub fn new_with_path_manager(
        path_manager: Arc<TablespacePathManager>,
        node_id: Option<NodeId>,
    ) -> Self {
        Self {
            engines: HashMap::new(),
            shards: Arc::new(RwLock::new(HashMap::new())),
            path_manager,
            node_id,
            distributed_mode: node_id.is_some(),
        }
    }
    
    pub async fn create_shard_with_context(
        &self,
        config: ShardCreate,
        context: ShardContext,
    ) -> KeyValueResult<ShardId> {
        // Use path manager to resolve all paths
        let data_path = self.path_manager.shard_data_path(
            context.tablespace_id,
            context.tenant_id,
            context.database_id,
            context.namespace_id,
            context.table_id,
            context.shard_id,
            config.engine_type.clone(),
        )?;
        
        let wal_path = self.path_manager.shard_wal_path(
            context.tablespace_id,
            context.tenant_id,
            context.database_id,
            context.namespace_id,
            context.table_id,
            context.shard_id,
            config.engine_type.clone(),
        )?;
        
        // Ensure paths exist
        self.path_manager.ensure_path_exists(&data_path)?;
        self.path_manager.ensure_path_exists(&wal_path)?;
        
        // Create shard with resolved paths
        // ...
    }
}
```

### With VFS

```rust
// Path manager uses VFS for all filesystem operations
impl TablespacePathManager {
    pub fn create_all_paths_for_shard(
        &self,
        tablespace_id: TablespaceId,
        tenant_id: TenantId,
        database_id: DatabaseId,
        namespace_id: NamespaceId,
        table_id: TableId,
        shard_id: ShardId,
        engine_type: StorageEngineType,
    ) -> Result<()> {
        let paths = self.list_shard_paths(
            tablespace_id, tenant_id, database_id, namespace_id,
            table_id, shard_id, engine_type
        )?;
        
        for path in paths {
            self.vfs.create_directory_all(&path)?;
        }
        
        Ok(())
    }
}
```

## Benefits

1. **Centralization**: Single source of truth for all path logic
2. **Consistency**: Uniform path structure across all components
3. **Validation**: Built-in path validation and error handling
4. **Flexibility**: Easy to change path structure in one place
5. **Testing**: Easy to mock and test path generation
6. **Type Safety**: Strongly typed path components
7. **Caching**: Optional path caching for performance
8. **Quota Management**: Integrated storage quota tracking
9. **Multi-Tier Support**: Seamless support for storage tiers
10. **Migration**: Easy to migrate data between tablespaces

## Migration from Current Implementation

1. **Phase 1**: Implement `TablespacePathManager` alongside existing `StoragePathResolver`
2. **Phase 2**: Update `KeyValueShardManager` to use new manager
3. **Phase 3**: Add support for all path types (indexes, embeddings, etc.)
4. **Phase 4**: Deprecate `StoragePathResolver`
5. **Phase 5**: Remove old implementation

## Testing Strategy

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use nanograph_vfs::MemoryFileSystem;
    
    #[test]
    fn test_shard_path_generation() {
        let vfs = Arc::new(MemoryFileSystem::new());
        let manager = TablespacePathManager::new(TablespaceId::DEFAULT, vfs);
        
        // Register tablespace
        let config = TablespaceConfig {
            id: TablespaceId::DEFAULT,
            name: "default".to_string(),
            base_path: Path::from("/data"),
            tier: StorageTier::Hot,
            available: true,
            max_size_bytes: None,
            used_bytes: 0,
            wal_path: None,
            vfs_scheme: "memory".to_string(),
        };
        manager.register_tablespace(config).unwrap();
        
        // Test path generation
        let path = manager.shard_data_path(
            TablespaceId::DEFAULT,
            TenantId(1),
            DatabaseId(2),
            NamespaceId(3),
            TableId(4),
            ShardId::from_parts(TableId(4), ShardIndex::new(0)),
            StorageEngineType::LSM,
        ).unwrap();
        
        assert_eq!(
            path,
            Path::from("/data/tenants/1/databases/2/namespaces/3/tables/4/shards/0/lsm/data")
        );
    }
    
    #[test]
    fn test_quota_enforcement() {
        let vfs = Arc::new(MemoryFileSystem::new());
        let manager = TablespacePathManager::new(TablespaceId::DEFAULT, vfs);
        
        let config = TablespaceConfig {
            id: TablespaceId::DEFAULT,
            name: "default".to_string(),
            base_path: Path::from("/data"),
            tier: StorageTier::Hot,
            available: true,
            max_size_bytes: Some(1000),
            used_bytes: 0,
            wal_path: None,
            vfs_scheme: "memory".to_string(),
        };
        manager.register_tablespace(config).unwrap();
        
        // Should succeed
        manager.update_usage(TablespaceId::DEFAULT, 500).unwrap();
        
        // Should fail (exceeds quota)
        let result = manager.update_usage(TablespaceId::DEFAULT, 600);
        assert!(result.is_err());
    }
}
```

## Conclusion

The `TablespacePathManager` provides a comprehensive, type-safe, and maintainable solution for managing all filesystem paths in Nanograph. It centralizes path logic, enforces consistency, and provides a clean API for all components that need to interact with the filesystem.