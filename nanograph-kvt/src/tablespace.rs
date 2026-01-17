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
use nanograph_core::object::{ShardId, StorageEngineType, TableId, TablespaceId};
use nanograph_vfs::Path;
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

/// Configuration for a tablespace on this node
#[derive(Debug, Clone)]
pub struct TablespaceConfig {
    /// Tablespace ID
    pub id: TablespaceId,

    /// Base path for this tablespace on this node
    pub base_path: Path,

    /// Storage tier (hot, warm, cold, archive)
    pub tier: StorageTier,

    /// Whether this tablespace is available on this node
    pub available: bool,
}

/// Storage tier classification
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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

/// Resolves storage paths for shards based on tablespace configuration
pub struct StoragePathResolver {
    /// Tablespace configurations for this node
    tablespaces: Arc<RwLock<HashMap<TablespaceId, TablespaceConfig>>>,

    /// Default tablespace ID
    default_tablespace: TablespaceId,
}

impl StoragePathResolver {
    /// Create a new storage path resolver
    pub fn new(default_tablespace: TablespaceId) -> Self {
        Self {
            tablespaces: Arc::new(RwLock::new(HashMap::new())),
            default_tablespace,
        }
    }

    /// Register a tablespace configuration
    pub fn register_tablespace(&self, config: TablespaceConfig) -> KeyValueResult<()> {
        let mut tablespaces = self.tablespaces.write().unwrap();
        tablespaces.insert(config.id, config);
        Ok(())
    }

    /// Unregister a tablespace
    pub fn unregister_tablespace(&self, id: TablespaceId) -> KeyValueResult<()> {
        let mut tablespaces = self.tablespaces.write().unwrap();
        tablespaces.remove(&id);
        Ok(())
    }

    /// Get tablespace configuration
    pub fn get_tablespace(&self, id: TablespaceId) -> KeyValueResult<TablespaceConfig> {
        let tablespaces = self.tablespaces.read().unwrap();
        tablespaces
            .get(&id)
            .cloned()
            .ok_or_else(|| KeyValueError::InvalidValue(format!("Tablespace {:?} not found", id)))
    }

    /// List all registered tablespaces
    pub fn list_tablespaces(&self) -> Vec<TablespaceConfig> {
        let tablespaces = self.tablespaces.read().unwrap();
        tablespaces.values().cloned().collect()
    }

    /// Resolve data directory path for a shard
    pub fn resolve_data_path(
        &self,
        tablespace_id: TablespaceId,
        table_id: TableId,
        shard_id: ShardId,
        engine_type: StorageEngineType,
    ) -> KeyValueResult<Path> {
        let tablespace = self.get_tablespace(tablespace_id)?;

        if !tablespace.available {
            return Err(KeyValueError::InvalidValue(format!(
                "Tablespace {:?} is not available on this node",
                tablespace_id
            )));
        }

        // Build path: <base_path>/<engine_type>/table_<table_id>/shard_<shard_id>/data
        let mut path = tablespace.base_path.clone();
        path.push(format!("{:?}", engine_type).to_lowercase());
        path.push(format!("table_{}", table_id.0));
        path.push(format!("shard_{}", shard_id.0));
        path.push("data");

        Ok(path)
    }

    /// Resolve WAL directory path for a shard
    pub fn resolve_wal_path(
        &self,
        tablespace_id: TablespaceId,
        table_id: TableId,
        shard_id: ShardId,
        engine_type: StorageEngineType,
    ) -> KeyValueResult<Path> {
        let tablespace = self.get_tablespace(tablespace_id)?;

        if !tablespace.available {
            return Err(KeyValueError::InvalidValue(format!(
                "Tablespace {:?} is not available on this node",
                tablespace_id
            )));
        }

        // Build path: <base_path>/<engine_type>/table_<table_id>/shard_<shard_id>/wal
        let mut path = tablespace.base_path.clone();
        path.push(format!("{:?}", engine_type).to_lowercase());
        path.push(format!("table_{}", table_id.0));
        path.push(format!("shard_{}", shard_id.0));
        path.push("wal");

        Ok(path)
    }

    /// Get the default tablespace ID
    pub fn default_tablespace(&self) -> TablespaceId {
        self.default_tablespace
    }

    /// Set the default tablespace ID
    pub fn set_default_tablespace(&mut self, id: TablespaceId) {
        self.default_tablespace = id;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_path_resolution() {
        let resolver = StoragePathResolver::new(TablespaceId::DEFAULT);

        // Register a tablespace
        let config = TablespaceConfig {
            id: TablespaceId::DEFAULT,
            base_path: Path::from("/mnt/ssd"),
            tier: StorageTier::Hot,
            available: true,
        };
        resolver.register_tablespace(config).unwrap();

        // Resolve paths
        let table_id = TableId::new(1);
        let shard_id = ShardId(0);

        let engine_type = StorageEngineType::from("lsm");

        let data_path = resolver
            .resolve_data_path(
                TablespaceId::DEFAULT,
                table_id,
                shard_id,
                engine_type.clone(),
            )
            .unwrap();

        assert_eq!(data_path, Path::from("/mnt/ssd/lsm/table_1/shard_0/data"));

        let wal_path = resolver
            .resolve_wal_path(TablespaceId::DEFAULT, table_id, shard_id, engine_type)
            .unwrap();

        assert_eq!(wal_path, Path::from("/mnt/ssd/lsm/table_1/shard_0/wal"));
    }

    #[test]
    fn test_unavailable_tablespace() {
        let resolver = StoragePathResolver::new(TablespaceId::DEFAULT);

        // Register an unavailable tablespace
        let config = TablespaceConfig {
            id: TablespaceId(1),
            base_path: Path::from("/mnt/unavailable"),
            tier: StorageTier::Cold,
            available: false,
        };
        resolver.register_tablespace(config).unwrap();

        // Try to resolve path
        let result = resolver.resolve_data_path(
            TablespaceId(1),
            TableId::new(1),
            ShardId(0),
            StorageEngineType::from("lsm"),
        );

        assert!(result.is_err());
    }
}
