//
// Copyright 2026 Hans W. Uhlig, IBM. All Rights Reserved.
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//      http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.
//

//! Storage path resolution for tablespace-aware shard creation
//!
//! This module provides utilities to resolve storage paths based on tablespace
//! configuration, enabling storage tiering and flexible storage management.

use crate::{KeyValueError, KeyValueResult};
use nanograph_core::config::StorageConfig;
use nanograph_core::object::{
    DatabaseId, TablespaceRecord, IndexNumber, ShardNumber, TableId, TablespaceId,
    TablespaceMetadata, TenantId,
};
use nanograph_vfs::{DynamicFileSystem, FileSystemError, Path};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

/// Resolves storage paths for shards based on tablespace configuration
pub struct StoragePathResolver {
    /// VFS instance for filesystem operations
    vfs: Arc<dyn DynamicFileSystem>,

    /// Storage Configuration
    config: StorageConfig,

    /// Tablespace configurations for this node
    tablespaces: Arc<RwLock<HashMap<TablespaceId, TablespaceStorage>>>,

    /// Default tablespace ID
    default_tablespace: TablespaceId,

    /// Enable path validation
    validate_paths: bool,
}

/// Main Methods
impl StoragePathResolver {
    /// Create a new storage path resolver
    /// Create a new path manager
    pub fn new(vfs: Arc<dyn DynamicFileSystem>, storage_config: StorageConfig) -> Self {
        let system_path = Path::parse(storage_config.system_path.as_str());
        Self {
            vfs,
            config: storage_config,
            tablespaces: Arc::new(RwLock::new(HashMap::from_iter(vec![(
                TablespaceId::new(0),
                TablespaceStorage {
                    id: TablespaceId::new(0),
                    name: String::from("system"),
                    base_path: Some(system_path),
                },
            )]))),
            default_tablespace: TablespaceId::new(0),
            validate_paths: true,
        }
    }

    /// Register a tablespace configuration from the cluster, match it to a node config if available
    pub fn register_tablespace(&self, metadata: TablespaceRecord) -> KeyValueResult<()> {
        let mut tablespaces = self.tablespaces.write().unwrap();
        let path = self
            .config
            .tablespaces
            .get(metadata.name.as_str())
            .map(|config| Path::from(config.storage_path.as_str()));
        tablespaces.insert(
            metadata.id,
            TablespaceStorage {
                id: metadata.id,
                name: metadata.name,
                base_path: path,
            },
        );
        Ok(())
    }

    /// Unregister a tablespace
    pub fn unregister_tablespace(&self, id: TablespaceId) -> KeyValueResult<()> {
        let mut tablespaces = self.tablespaces.write().unwrap();
        tablespaces.remove(&id);
        Ok(())
    }

    /// Get tablespace configuration
    pub fn get_tablespace(&self, id: TablespaceId) -> KeyValueResult<TablespaceStorage> {
        let tablespaces = self.tablespaces.read().unwrap();
        tablespaces
            .get(&id)
            .cloned()
            .ok_or_else(|| KeyValueError::InvalidValue(format!("Tablespace {:?} not found", id)))
    }

    /// List all registered tablespaces
    pub fn list_tablespaces(&self) -> Vec<TablespaceStorage> {
        let tablespaces = self.tablespaces.read().unwrap();
        tablespaces.values().cloned().collect()
    }

    /// Get the default tablespace ID
    pub fn default_tablespace(&self) -> TablespaceId {
        self.default_tablespace
    }

    /// Set the default tablespace ID
    pub fn set_default_tablespace(&mut self, id: TablespaceId) {
        self.default_tablespace = id;
    }

    fn validate_path(&self, path: Path) -> KeyValueResult<Path> {
        if self.validate_paths && !self.vfs.exists(path.to_string().as_str())? {
            Err(KeyValueError::IoError(FileSystemError::InvalidPath(
                path.to_string(),
            )))
        } else {
            Ok(path)
        }
    }
}

/// System Metadata Paths
impl StoragePathResolver {
    /// Get system metadata base path
    #[inline(always)]
    pub fn system_log_path(&self) -> KeyValueResult<Path> {
        let path = Path::parse(self.config.log_path.as_str());
        self.validate_path(path)
    }
}

/// System Metadata Paths
impl StoragePathResolver {
    /// Get system metadata base path
    pub fn system_base_path(&self, tablespace_id: TablespaceId) -> KeyValueResult<Path> {
        let config = self.get_tablespace(tablespace_id)?;
        if let Some(base_path) = config.base_path.as_ref() {
            let mut path = base_path.clone();
            path.push("system");
            self.validate_path(path)
        } else {
            Err(KeyValueError::InvalidValue(format!(
                "Tablespace {:?} is not available on this node",
                tablespace_id
            )))
        }
    }

    /// Get system metadata base path
    pub fn system_metadata_path(&self, tablespace_id: TablespaceId) -> KeyValueResult<Path> {
        let mut path = self.system_base_path(tablespace_id)?;
        path.push("metadata");
        self.validate_path(path)
    }

    /// Get system data path (Shard Storage)
    pub fn system_data_path(&self, tablespace_id: TablespaceId) -> KeyValueResult<Path> {
        let mut path = self.system_metadata_path(tablespace_id)?;
        path.push("data");
        self.validate_path(path)
    }

    /// Get system WAL path
    pub fn system_wal_path(&self, tablespace_id: TablespaceId) -> KeyValueResult<Path> {
        let mut path = self.system_metadata_path(tablespace_id)?;
        path.push("wal");
        self.validate_path(path)
    }

    /// Get system Raft logs path
    pub fn system_raft_logs_path(&self, tablespace_id: TablespaceId) -> KeyValueResult<Path> {
        let mut path = self.system_metadata_path(tablespace_id)?;
        path.push("logs");
        self.validate_path(path)
    }

    /// Get system Raft snapshots path
    pub fn system_raft_snapshots_path(&self, tablespace_id: TablespaceId) -> KeyValueResult<Path> {
        let mut path = self.system_metadata_path(tablespace_id)?;
        path.push("snapshots");
        self.validate_path(path)
    }
}

/// Tenant Metadata Paths
impl StoragePathResolver {
    /// Get tenant base path
    pub fn tenant_base_path(
        &self,
        tablespace_id: TablespaceId,
        tenant_id: TenantId,
    ) -> KeyValueResult<Path> {
        let config = self.get_tablespace(tablespace_id)?;
        if let Some(base_path) = config.base_path.as_ref() {
            let mut path = base_path.clone();
            path.push("containers");
            path.push(format!("tenant_{}", tenant_id.0));
            self.validate_path(path)
        } else {
            Err(KeyValueError::InvalidValue(format!(
                "Tablespace {:?} is not available on this node",
                tablespace_id
            )))
        }
    }

    /// Get container base path
    pub fn tenant_metadata_path(
        &self,
        tablespace_id: TablespaceId,
        tenant_id: TenantId,
    ) -> KeyValueResult<Path> {
        let mut path = self.tenant_base_path(tablespace_id, tenant_id)?;
        path.push("data");
        self.validate_path(path)
    }

    /// Get container data path
    pub fn tenant_data_path(
        &self,
        tablespace_id: TablespaceId,
        tenant_id: TenantId,
    ) -> KeyValueResult<Path> {
        let mut path = self.tenant_metadata_path(tablespace_id, tenant_id)?;
        path.push("data");
        self.validate_path(path)
    }

    /// Get container Raft path
    pub fn tenant_wal_path(
        &self,
        tablespace_id: TablespaceId,
        tenant_id: TenantId,
    ) -> KeyValueResult<Path> {
        let mut path = self.tenant_metadata_path(tablespace_id, tenant_id)?;
        path.push("wal");
        self.validate_path(path)
    }
    /// Get container Raft path
    pub fn tenant_raft_logs_path(
        &self,
        tablespace_id: TablespaceId,
        tenant_id: TenantId,
    ) -> KeyValueResult<Path> {
        let mut path = self.tenant_metadata_path(tablespace_id, tenant_id)?;
        path.push("logs");
        self.validate_path(path)
    }
    /// Get container Raft path
    pub fn container_snapshots_path(
        &self,
        tablespace_id: TablespaceId,
        tenant_id: TenantId,
    ) -> KeyValueResult<Path> {
        let mut path = self.tenant_metadata_path(tablespace_id, tenant_id)?;
        path.push("snapshots");
        self.validate_path(path)
    }
}

/// Database/Container Metadata Paths
impl StoragePathResolver {
    /// Get Database/Container base path
    pub fn database_base_path(
        &self,
        tablespace_id: TablespaceId,
        tenant_id: TenantId,
        database_id: DatabaseId,
    ) -> KeyValueResult<Path> {
        let mut path = self.tenant_base_path(tablespace_id, tenant_id)?;
        path.push(format!("database_{}", database_id.0));
        self.validate_path(path)
    }

    /// Get container base path
    pub fn database_metadata_path(
        &self,
        tablespace_id: TablespaceId,
        tenant_id: TenantId,
        database_id: DatabaseId,
    ) -> KeyValueResult<Path> {
        let mut path = self.database_base_path(tablespace_id, tenant_id, database_id)?;
        path.push("metadata");
        self.validate_path(path)
    }

    /// Get container data path
    pub fn database_data_path(
        &self,
        tablespace_id: TablespaceId,
        tenant_id: TenantId,
        database_id: DatabaseId,
    ) -> KeyValueResult<Path> {
        let mut path = self.database_metadata_path(tablespace_id, tenant_id, database_id)?;
        path.push("data");
        self.validate_path(path)
    }

    /// Get container Raft path
    pub fn database_wal_path(
        &self,
        tablespace_id: TablespaceId,
        tenant_id: TenantId,
        database_id: DatabaseId,
    ) -> KeyValueResult<Path> {
        let mut path = self.database_metadata_path(tablespace_id, tenant_id, database_id)?;
        path.push("wal");
        self.validate_path(path)
    }
    /// Get container Raft path
    pub fn database_raft_logs_path(
        &self,
        tablespace_id: TablespaceId,
        tenant_id: TenantId,
        database_id: DatabaseId,
    ) -> KeyValueResult<Path> {
        let mut path = self.database_metadata_path(tablespace_id, tenant_id, database_id)?;
        path.push("logs");
        self.validate_path(path)
    }
    /// Get container Snapshots
    pub fn database_snapshots_path(
        &self,
        tablespace_id: TablespaceId,
        tenant_id: TenantId,
        database_id: DatabaseId,
    ) -> KeyValueResult<Path> {
        let mut path = self.database_metadata_path(tablespace_id, tenant_id, database_id)?;
        path.push("snapshots");
        self.validate_path(path)
    }
}

///
impl StoragePathResolver {
    /// Get tenant base path
    pub fn table_base_path(
        &self,
        tablespace_id: TablespaceId,
        tenant_id: TenantId,
        database_id: DatabaseId,
        table_id: TableId,
    ) -> KeyValueResult<Path> {
        let mut path = self.database_base_path(tablespace_id, tenant_id, database_id)?;
        path.push(format!("table_{}", table_id.0));
        self.validate_path(path)
    }

    /// Get database base path
    pub fn table_metadata_path(
        &self,
        tablespace_id: TablespaceId,
        tenant_id: TenantId,
        database_id: DatabaseId,
        table_id: TableId,
    ) -> KeyValueResult<Path> {
        let mut path = self.table_base_path(tablespace_id, tenant_id, database_id, table_id)?;
        path.push("metadata");
        self.validate_path(path)
    }

    /// Get container data path
    pub fn table_data_path(
        &self,
        tablespace_id: TablespaceId,
        tenant_id: TenantId,
        database_id: DatabaseId,
        table_id: TableId,
    ) -> KeyValueResult<Path> {
        let mut path = self.table_metadata_path(tablespace_id, tenant_id, database_id, table_id)?;
        path.push("data");
        self.validate_path(path)
    }

    /// Get container Raft path
    pub fn table_wal_path(
        &self,
        tablespace_id: TablespaceId,
        tenant_id: TenantId,
        database_id: DatabaseId,
        table_id: TableId,
    ) -> KeyValueResult<Path> {
        let mut path = self.table_metadata_path(tablespace_id, tenant_id, database_id, table_id)?;
        path.push("wal");
        self.validate_path(path)
    }
    /// Get container Raft path
    pub fn table_raft_logs_path(
        &self,
        tablespace_id: TablespaceId,
        tenant_id: TenantId,
        database_id: DatabaseId,
        table_id: TableId,
    ) -> KeyValueResult<Path> {
        let mut path = self.table_metadata_path(tablespace_id, tenant_id, database_id, table_id)?;
        path.push("logs");
        self.validate_path(path)
    }
    /// Get container Snapshots
    pub fn table_snapshots_path(
        &self,
        tablespace_id: TablespaceId,
        tenant_id: TenantId,
        database_id: DatabaseId,
        table_id: TableId,
    ) -> KeyValueResult<Path> {
        let mut path = self.table_metadata_path(tablespace_id, tenant_id, database_id, table_id)?;
        path.push("snapshots");
        self.validate_path(path)
    }
}

impl StoragePathResolver {
    /// Get shard base path
    pub fn shard_base_path(
        &self,
        tablespace_id: TablespaceId,
        tenant_id: TenantId,
        database_id: DatabaseId,
        table_id: TableId,
        shard_number: ShardNumber,
    ) -> KeyValueResult<Path> {
        let mut path = self.table_base_path(tablespace_id, tenant_id, database_id, table_id)?;
        path.push(format!("shard_{}", shard_number.as_u32()));
        self.validate_path(path)
    }

    /// Get shard data path
    pub fn shard_data_path(
        &self,
        tablespace_id: TablespaceId,
        tenant_id: TenantId,
        database_id: DatabaseId,
        table_id: TableId,
        shard_number: ShardNumber,
    ) -> KeyValueResult<Path> {
        let mut path = self.shard_base_path(
            tablespace_id,
            tenant_id,
            database_id,
            table_id,
            shard_number,
        )?;
        path.push("data");
        self.validate_path(path)
    }

    /// Get shard WAL path
    pub fn shard_wal_path(
        &self,
        tablespace_id: TablespaceId,
        tenant_id: TenantId,
        database_id: DatabaseId,
        table_id: TableId,
        shard_number: ShardNumber,
    ) -> KeyValueResult<Path> {
        let mut path = self.shard_base_path(
            tablespace_id,
            tenant_id,
            database_id,
            table_id,
            shard_number,
        )?;
        path.push("wal");
        self.validate_path(path)
    }

    /// Get shard Raft path (for replicated shards)
    pub fn shard_raft_logs_path(
        &self,
        tablespace_id: TablespaceId,
        tenant_id: TenantId,
        database_id: DatabaseId,
        table_id: TableId,
        shard_number: ShardNumber,
    ) -> KeyValueResult<Path> {
        let mut path = self.shard_base_path(
            tablespace_id,
            tenant_id,
            database_id,
            table_id,
            shard_number,
        )?;
        path.push("logs");
        self.validate_path(path)
    }
    pub fn shard_snapshots_path(
        &self,
        tablespace_id: TablespaceId,
        tenant_id: TenantId,
        database_id: DatabaseId,
        table_id: TableId,
        shard_number: ShardNumber,
    ) -> KeyValueResult<Path> {
        let mut path = self.shard_base_path(
            tablespace_id,
            tenant_id,
            database_id,
            table_id,
            shard_number,
        )?;
        path.push("snapshots");
        self.validate_path(path)
    }

    /// Get LSM level path
    pub fn lsm_level_path(
        &self,
        tablespace_id: TablespaceId,
        tenant_id: TenantId,
        database_id: DatabaseId,
        table_id: TableId,
        shard_number: ShardNumber,
        level: u8,
    ) -> KeyValueResult<Path> {
        let mut path = self.shard_data_path(
            tablespace_id,
            tenant_id,
            database_id,
            table_id,
            shard_number,
        )?;
        path.push(format!("l{}", level));
        self.validate_path(path)
    }
}

/// Index Path Functions
impl StoragePathResolver {
    /// Get index base path
    pub fn index_base_path(
        &self,
        tablespace_id: TablespaceId,
        tenant_id: TenantId,
        database_id: DatabaseId,
        table_id: TableId,
        index_number: IndexNumber,
    ) -> KeyValueResult<Path> {
        let mut path = self.table_base_path(tablespace_id, tenant_id, database_id, table_id)?;
        path.push(format!("index_{}", index_number.as_u32()));
        self.validate_path(path)
    }
}

#[derive(Debug, Clone)]
pub struct TablespaceStorage {
    id: TablespaceId,
    name: String,
    base_path: Option<Path>,
}

impl TablespaceStorage {
    pub fn new(id: TablespaceId, name: String, base_path: Option<Path>) -> Self {
        Self {
            id,
            name,
            base_path,
        }
    }
    pub fn id(&self) -> TablespaceId {
        self.id
    }
    pub fn name(&self) -> &str {
        &self.name
    }
    pub fn path(&self) -> Option<&Path> {
        self.base_path.as_ref()
    }
    pub fn is_available(&self) -> bool {
        self.base_path.is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nanograph_core::config::TablespaceConfig;
    use nanograph_core::object::{TablespaceRecord, StorageTier};
    use nanograph_vfs::MemoryFileSystem;

    fn setup_resolver() -> StoragePathResolver {
        let vfs = Arc::new(MemoryFileSystem::new());
        let default_tablespace = TablespaceId(1);
        let resolver = StoragePathResolver::new(
            vfs.clone(),
            StorageConfig {
                system_path: "file:///data/system".to_string(),
                log_path: "file:///data/system".to_string(),
                tablespaces: HashMap::from_iter(vec![
                    (
                        String::from("hot_data"),
                        TablespaceConfig {
                            storage_path: String::from("file:///data/ts1"),
                        },
                    ),
                    (
                        String::from("warm_data"),
                        TablespaceConfig {
                            storage_path: String::from("file:///data/ts2"),
                        },
                    ),
                    (
                        String::from("cold_data"),
                        TablespaceConfig {
                            storage_path: String::from("file:///data/ts3"),
                        },
                    ),
                ]),
            },
        );

        resolver
            .register_tablespace(TablespaceRecord {
                id: TablespaceId(1),
                tier: StorageTier::Hot,
                created_at: Default::default(),
                updated_at: Default::default(),
                tenants: vec![],
                options: Default::default(),
                name: "hot_data".to_string(),
                metadata: Default::default(),
                version: 0,
            })
            .unwrap();

        resolver
            .register_tablespace(TablespaceRecord {
                id: TablespaceId(2),
                tier: StorageTier::Warm,
                created_at: Default::default(),
                updated_at: Default::default(),
                tenants: vec![],
                options: Default::default(),
                name: "warm_data".to_string(),
                metadata: Default::default(),
                version: 0,
            })
            .unwrap();

        resolver
            .register_tablespace(TablespaceRecord {
                id: TablespaceId(3),
                tier: StorageTier::Cold,
                created_at: Default::default(),
                updated_at: Default::default(),
                tenants: vec![],
                options: Default::default(),
                name: "cold_data".to_string(),
                metadata: Default::default(),
                version: 0,
            })
            .unwrap();

        resolver
    }

    #[test]
    fn test_system_paths() {
        let resolver = setup_resolver();
        let ts = TablespaceId(1);

        assert_eq!(
            resolver.system_base_path(ts).unwrap().to_string(),
            "file:///data/ts1/system"
        );
        assert_eq!(
            resolver.system_metadata_path(ts).unwrap().to_string(),
            "file:///data/ts1/system/metadata"
        );
        assert_eq!(
            resolver.system_data_path(ts).unwrap().to_string(),
            "file:///data/ts1/system/metadata/data"
        );
        assert_eq!(
            resolver.system_wal_path(ts).unwrap().to_string(),
            "file:///data/ts1/system/metadata/wal"
        );
        assert_eq!(
            resolver.system_raft_logs_path(ts).unwrap().to_string(),
            "file:///data/ts1/system/metadata/logs"
        );
        assert_eq!(
            resolver.system_raft_snapshots_path(ts).unwrap().to_string(),
            "file:///data/ts1/system/metadata/snapshots"
        );
    }

    #[test]
    fn test_tenant_paths() {
        let resolver = setup_resolver();
        let ts = TablespaceId(1);
        let tenant = TenantId(0x123);

        assert_eq!(
            resolver.tenant_base_path(ts, tenant).unwrap().to_string(),
            "file:///data/ts1/containers/tenant_291"
        );
        assert_eq!(
            resolver
                .tenant_metadata_path(ts, tenant)
                .unwrap()
                .to_string(),
            "file:///data/ts1/containers/tenant_291/data"
        );
        assert_eq!(
            resolver.tenant_data_path(ts, tenant).unwrap().to_string(),
            "file:///data/ts1/containers/tenant_291/data/data"
        );
        assert_eq!(
            resolver.tenant_wal_path(ts, tenant).unwrap().to_string(),
            "file:///data/ts1/containers/tenant_291/data/wal"
        );
        assert_eq!(
            resolver
                .tenant_raft_logs_path(ts, tenant)
                .unwrap()
                .to_string(),
            "file:///data/ts1/containers/tenant_291/data/logs"
        );
        assert_eq!(
            resolver
                .container_snapshots_path(ts, tenant)
                .unwrap()
                .to_string(),
            "file:///data/ts1/containers/tenant_291/data/snapshots"
        );
    }

    #[test]
    fn test_database_paths() {
        let resolver = setup_resolver();
        let ts = TablespaceId(1);
        let tenant = TenantId(0x123);
        let db = DatabaseId(0x456);

        assert_eq!(
            resolver
                .database_base_path(ts, tenant, db)
                .unwrap()
                .to_string(),
            "file:///data/ts1/containers/tenant_291/database_1110"
        );
        assert_eq!(
            resolver
                .database_metadata_path(ts, tenant, db)
                .unwrap()
                .to_string(),
            "file:///data/ts1/containers/tenant_291/database_1110/metadata"
        );
        assert_eq!(
            resolver
                .database_data_path(ts, tenant, db)
                .unwrap()
                .to_string(),
            "file:///data/ts1/containers/tenant_291/database_1110/metadata/data"
        );
        assert_eq!(
            resolver
                .database_wal_path(ts, tenant, db)
                .unwrap()
                .to_string(),
            "file:///data/ts1/containers/tenant_291/database_1110/metadata/wal"
        );
        assert_eq!(
            resolver
                .database_raft_logs_path(ts, tenant, db)
                .unwrap()
                .to_string(),
            "file:///data/ts1/containers/tenant_291/database_1110/metadata/logs"
        );
        assert_eq!(
            resolver
                .database_snapshots_path(ts, tenant, db)
                .unwrap()
                .to_string(),
            "file:///data/ts1/containers/tenant_291/database_1110/metadata/snapshots"
        );
    }

    #[test]
    fn test_table_paths() {
        let resolver = setup_resolver();
        let ts = TablespaceId(1);
        let tenant = TenantId(0x123);
        let db = DatabaseId(0x456);
        let table = TableId(0x789);

        assert_eq!(
            resolver
                .table_base_path(ts, tenant, db, table)
                .unwrap()
                .to_string(),
            "file:///data/ts1/containers/tenant_291/database_1110/table_1929"
        );
        assert_eq!(
            resolver
                .table_metadata_path(ts, tenant, db, table)
                .unwrap()
                .to_string(),
            "file:///data/ts1/containers/tenant_291/database_1110/table_1929/metadata"
        );
        assert_eq!(
            resolver
                .table_data_path(ts, tenant, db, table)
                .unwrap()
                .to_string(),
            "file:///data/ts1/containers/tenant_291/database_1110/table_1929/metadata/data"
        );
        assert_eq!(
            resolver
                .table_wal_path(ts, tenant, db, table)
                .unwrap()
                .to_string(),
            "file:///data/ts1/containers/tenant_291/database_1110/table_1929/metadata/wal"
        );
        assert_eq!(
            resolver
                .table_raft_logs_path(ts, tenant, db, table)
                .unwrap()
                .to_string(),
            "file:///data/ts1/containers/tenant_291/database_1110/table_1929/metadata/logs"
        );
        assert_eq!(
            resolver
                .table_snapshots_path(ts, tenant, db, table)
                .unwrap()
                .to_string(),
            "file:///data/ts1/containers/tenant_291/database_1110/table_1929/metadata/snapshots"
        );
    }

    #[test]
    fn test_shard_paths() {
        let resolver = setup_resolver();
        let ts = TablespaceId(1);
        let tenant = TenantId(0x123);
        let db = DatabaseId(0x456);
        let table = TableId(0x789);
        let shard = ShardNumber(1);

        assert_eq!(
            resolver
                .shard_base_path(ts, tenant, db, table, shard)
                .unwrap()
                .to_string(),
            "file:///data/ts1/containers/tenant_291/database_1110/table_1929/shard_1"
        );
        assert_eq!(
            resolver
                .shard_data_path(ts, tenant, db, table, shard)
                .unwrap()
                .to_string(),
            "file:///data/ts1/containers/tenant_291/database_1110/table_1929/shard_1/data"
        );
        assert_eq!(
            resolver
                .shard_wal_path(ts, tenant, db, table, shard)
                .unwrap()
                .to_string(),
            "file:///data/ts1/containers/tenant_291/database_1110/table_1929/shard_1/wal"
        );
        assert_eq!(
            resolver
                .shard_raft_logs_path(ts, tenant, db, table, shard)
                .unwrap()
                .to_string(),
            "file:///data/ts1/containers/tenant_291/database_1110/table_1929/shard_1/logs"
        );
        assert_eq!(
            resolver
                .shard_snapshots_path(ts, tenant, db, table, shard)
                .unwrap()
                .to_string(),
            "file:///data/ts1/containers/tenant_291/database_1110/table_1929/shard_1/snapshots"
        );
        assert_eq!(
            resolver
                .lsm_level_path(ts, tenant, db, table, shard, 0)
                .unwrap()
                .to_string(),
            "file:///data/ts1/containers/tenant_291/database_1110/table_1929/shard_1/data/l0"
        );
    }

    #[test]
    fn test_index_paths() {
        let resolver = setup_resolver();
        let ts = TablespaceId(1);
        let tenant = TenantId(0x123);
        let db = DatabaseId(0x456);
        let table = TableId(0x789);
        let index = IndexNumber(2);

        assert_eq!(
            resolver
                .index_base_path(ts, tenant, db, table, index)
                .unwrap()
                .to_string(),
            "file:///data/ts1/containers/tenant_291/database_1110/table_1929/index_2"
        );
    }

    #[test]
    fn test_create_2_of_each_path_type() {
        let resolver = setup_resolver();
        let ts1 = TablespaceId(1);
        let ts2 = TablespaceId(2);
        let tenant1 = TenantId(1);
        let tenant2 = TenantId(2);
        let db1 = DatabaseId(1);
        let db2 = DatabaseId(2);
        let table1 = TableId(1);
        let table2 = TableId(2);
        let shard1 = ShardNumber(1);
        let shard2 = ShardNumber(2);
        let index1 = IndexNumber(1);
        let index2 = IndexNumber(2);

        println!("--- Path Type 1 ---");
        println!("System 1: {}", resolver.system_base_path(ts1).unwrap());
        println!(
            "Tenant 1: {}",
            resolver.tenant_base_path(ts1, tenant1).unwrap()
        );
        println!(
            "Database 1: {}",
            resolver.database_base_path(ts1, tenant1, db1).unwrap()
        );
        println!(
            "Table 1: {}",
            resolver.table_base_path(ts1, tenant1, db1, table1).unwrap()
        );
        println!(
            "Shard 1: {}",
            resolver
                .shard_base_path(ts1, tenant1, db1, table1, shard1)
                .unwrap()
        );
        println!(
            "Index 1: {}",
            resolver
                .index_base_path(ts1, tenant1, db1, table1, index1)
                .unwrap()
        );
        println!(
            "LSM 1: {}",
            resolver
                .lsm_level_path(ts1, tenant1, db1, table1, shard1, 0)
                .unwrap()
        );

        println!("\n--- Path Type 2 ---");
        println!("System 2: {}", resolver.system_base_path(ts2).unwrap());
        println!(
            "Tenant 2: {}",
            resolver.tenant_base_path(ts2, tenant2).unwrap()
        );
        println!(
            "Database 2: {}",
            resolver.database_base_path(ts2, tenant2, db2).unwrap()
        );
        println!(
            "Table 2: {}",
            resolver.table_base_path(ts2, tenant2, db2, table2).unwrap()
        );
        println!(
            "Shard 2: {}",
            resolver
                .shard_base_path(ts2, tenant2, db2, table2, shard2)
                .unwrap()
        );
        println!(
            "Index 2: {}",
            resolver
                .index_base_path(ts2, tenant2, db2, table2, index2)
                .unwrap()
        );
        println!(
            "LSM 2: {}",
            resolver
                .lsm_level_path(ts2, tenant2, db2, table2, shard2, 1)
                .unwrap()
        );
    }
}
