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

use crate::metacache::MetadataCache;
use crate::shardmgr::KeyValueShardManager;
use nanograph_core::types::{NodeId, ShardId, ShardIndex, TableId};
use nanograph_kvt::{
    KeyValueError, KeyValueResult, Partitioner, StorageEngineType, TableConfig, TableSharding,
};
use nanograph_raft::ConsensusRouter;
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

/// Responsible for managing key-value databases, including creating, managing, and querying tables.
/// Keeps system metadata and manages shard allocation for tables.
///
/// Can operate in two modes:
/// - **Single-node mode**: Direct shard access (default)
/// - **Distributed mode**: Operations go through Raft consensus
///
/// TODO: Handle Table and Shard Allocation
pub struct KeyValueDatabaseManager {
    /// Local Shard Storage Manager
    shard_manager: Arc<RwLock<KeyValueShardManager>>,
    /// System Metadata Cache
    /// TODO: Ensure Data is in a KeyValue Table. This should only be a cache for metadata
    /// TODO: Identify if this should merge into the ConsensusRouter's Metadata
    metadata_manager: Arc<RwLock<MetadataCache>>,
    /// Raft router for distributed mode
    raft_router: Option<Arc<ConsensusRouter>>,
}

impl KeyValueDatabaseManager {
    /// Create a new database manager in single-node mode
    pub fn new_standalone(
        shard_manager: Arc<RwLock<KeyValueShardManager>>,
        metadata_manager: Arc<RwLock<MetadataCache>>,
    ) -> Self {
        Self {
            shard_manager,
            metadata_manager,
            raft_router: None,
        }
    }

    /// Create a new database manager in distributed mode
    pub fn new_distributed(
        shard_manager: Arc<RwLock<KeyValueShardManager>>,
        metadata_manager: Arc<RwLock<MetadataCache>>,
        raft_router: Arc<ConsensusRouter>,
    ) -> Self {
        Self {
            shard_manager,
            metadata_manager,
            raft_router: Some(raft_router),
        }
    }

    /// Check if running in distributed mode
    pub fn is_distributed(&self) -> bool {
        self.raft_router.is_some()
    }

    /// Get the local node ID (if in distributed mode)
    pub fn node_id(&self) -> Option<NodeId> {
        self.raft_router
            .as_ref()
            .map(|raft_router| raft_router.node_id())
    }

    /// Get the Raft router (if in distributed mode)
    pub fn consensus_router(&self) -> Option<&Arc<ConsensusRouter>> {
        self.raft_router.as_ref()
    }

    /// Create a new namespace
    pub async fn create_namespace(&self, path: &str, name: String) -> KeyValueResult<()> {
        // TODO: Implement namespace creation
        // For now, this is a placeholder
        Ok(())
    }

    /// Create a new table
    pub async fn create_table(
        &self,
        path: &str,
        name: String,
        config: TableConfig,
    ) -> KeyValueResult<TableId> {
        // TODO: Get/Create actual new table ID
        let table_id = TableId::new(0);

        // In distributed mode, coordinate table creation via Raft
        if let Some(router) = &self.raft_router {
            match config.sharding_config {
                // Single Shard Replication
                TableSharding::Single => {
                    let shard_id = ShardId::from_parts(table_id, ShardIndex::new(0));
                    // TODO: Implement proper replica placement strategy, Single Shard should be fully replicated
                    let replicas = vec![router.node_id()];

                    // Create Shard on all nodes
                    router
                        .metadata()
                        .create_shard(
                            shard_id,
                            (vec![], vec![0xFF; 32]), // Full key range
                            replicas,
                        )
                        .await
                        .map_err(|e| {
                            KeyValueError::Consensus(format!(
                                "Failed to create shard via Raft: {}",
                                e
                            ))
                        })?;

                    // Add table to metadata cache
                    let mut metadata = self.metadata_manager.write().unwrap();
                    let mut table_config = TableConfig::new(name.clone(), engine_type);
                    table_config.shard_count = shard_count;
                    table_config.replication_factor = replication_factor;
                    if shard_count > 1 {
                        table_config.partitioner = Some(Partitioner::default());
                    }
                    metadata.add_table(path, table_config);

                    // TODO: Return actual table ID
                    Ok(TableId::new(0))
                }
                // Multiple Shards with Partitioning and Replication
                TableSharding::Multiple {
                    shard_count,
                    partitioner,
                    replication_factor,
                } => {
                    // Create shards for this table via Raft
                    for shard_index in 0..shard_count {
                        let shard_id = ShardId::from_parts(table_id, ShardIndex::new(shard_index));

                        // Determine replica nodes for this shard
                        // TODO: Implement proper replica placement strategy
                        let replicas = vec![router.node_id()];

                        // Create shard via metadata Raft group
                        router
                            .metadata()
                            .create_shard(
                                shard_id,
                                (vec![], vec![0xFF; 32]), // Full key range
                                replicas,
                            )
                            .await
                            .map_err(|e| {
                                KeyValueError::Consensus(format!(
                                    "Failed to create shard via Raft: {}",
                                    e
                                ))
                            })?;
                    }

                    // Add table to metadata cache
                    let mut metadata = self.metadata_manager.write().unwrap();
                    let mut table_config = TableConfig::new(name.clone(), engine_type);
                    table_config.shard_count = shard_count;
                    table_config.replication_factor = replication_factor;
                    if shard_count > 1 {
                        table_config.partitioner = Some(Partitioner::default());
                    }
                    metadata.add_table(path, table_config);

                    // TODO: Return actual table ID
                    Ok(TableId::new(0))
                }
            }
        }

    }

    /// Put a key-value pair into a table
    pub async fn put(&self, table: TableId, key: &[u8], value: &[u8]) -> KeyValueResult<()> {
        let shard_id = self.get_shard_for_key(table, key)?;

        // In distributed mode, coordinate put key via Raft Consensus
        if let Some(router) = &self.raft_router {
            // Route through Raft for distributed consensus
            router
                .put(key.to_vec(), value.to_vec())
                .await
                .map_err(|e| KeyValueError::Consensus(format!("Raft put failed: {}", e)))
        } else {
            // Single-node mode: direct shard access
            let shard_manager = self.shard_manager.read().unwrap();
            shard_manager.put(shard_id, key, value).await
        }
    }

    /// Get a value from a table
    pub async fn get(&self, table: TableId, key: &[u8]) -> KeyValueResult<Option<Vec<u8>>> {
        let shard_id = self.get_shard_for_key(table, key)?;

        // In distributed mode, coordinate get key via Raft Consensus
        if let Some(router) = &self.raft_router {
            // Route through Raft for distributed reads
            router
                .get(key)
                .await
                .map_err(|e| KeyValueError::Consensus(format!("Raft get failed: {}", e)))
        } else {
            // Single-node mode: direct shard access
            let shard_manager = self.shard_manager.read().unwrap();
            shard_manager.get(shard_id, key).await
        }
    }

    /// Delete a key from a table
    pub async fn delete(&self, table: TableId, key: &[u8]) -> KeyValueResult<bool> {
        let shard_id = self.get_shard_for_key(table, key)?;

        // In distributed mode, coordinate delete key via Raft Consensus
        if let Some(router) = &self.raft_router {
            // Route through Raft for distributed deletes
            router
                .delete(key.to_vec())
                .await
                .map_err(|e| KeyValueError::Consensus(format!("Raft delete failed: {}", e)))?;
            Ok(true)
        } else {
            // Single-node mode: direct shard access
            let shard_manager = self.shard_manager.read().unwrap();
            shard_manager.delete(shard_id, key).await
        }
    }

    /// Batch put operations
    pub async fn batch_put(&self, table: TableId, pairs: &[(&[u8], &[u8])]) -> KeyValueResult<()> {
        // In distributed mode, coordinate batch put key via Raft Consensus
        if let Some(router) = &self.raft_router {
            // Convert to Raft operations
            let operations: Vec<nanograph_raft::Operation> = pairs
                .iter()
                .map(|(k, v)| nanograph_raft::Operation::Put {
                    key: k.to_vec(),
                    value: v.to_vec(),
                })
                .collect();

            return router
                .batch(operations)
                .await
                .map_err(|e| KeyValueError::Consensus(format!("Raft put failed: {}", e)));
        } else {
            // Single-node mode: group by shard and batch
            let mut shard_batches: HashMap<ShardId, Vec<(&[u8], &[u8])>> = HashMap::new();

            for &(key, value) in pairs {
                let shard_id = self.get_shard_for_key(table, key)?;
                shard_batches
                    .entry(shard_id)
                    .or_insert_with(Vec::new)
                    .push((key, value));
            }

            let shard_manager = self.shard_manager.read().unwrap();
            for (shard_id, batch) in shard_batches {
                shard_manager.batch_put(shard_id, &batch).await?;
            }

            Ok(())
        }
    }

    /// Calculate which shard a key belongs to for a given table
    ///
    /// Uses hash-based partitioning to distribute keys across shards.
    /// For single-shard tables (shard_count=1), always returns shard 0.
    ///
    /// TODO: Handle Namespacing
    /// TODO: Make Partitioning algorithm configurable
    fn get_shard_for_key(&self, table: TableId, key: &[u8]) -> KeyValueResult<ShardId> {
        let metadata_manager = self.metadata_manager.read().unwrap();
        let table_metadata =
            metadata_manager
                .get_table_metadata(&table)
                .ok_or(KeyValueError::InvalidKey(format!(
                    "Table {:?} not found",
                    table
                )))?;

        if table_metadata.shard_count == 1 {
            // Single shard - no hashing needed
            return Ok(ShardId::new(0));
        } else {
            match table_metadata.partitioner {}
        }

        // Hash the key and mod by shard count
        // TODO: Use configured Table Partitioner
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let mut hasher = DefaultHasher::new();
        key.hash(&mut hasher);
        let hash = hasher.finish();

        let shard_index = (hash % table_metadata.shard_count as u64) as u32;
        Ok(ShardId::from_parts(table, ShardIndex(shard_index)))
    }
}
