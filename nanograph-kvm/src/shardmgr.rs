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

use nanograph_core::types::{NodeId, ShardId};
use nanograph_kvt::KeyRange;
use nanograph_kvt::KeyValueShardStore;
use nanograph_kvt::ShardConfig;
use nanograph_kvt::metrics::ShardStats;
use nanograph_kvt::{KeyValueError, KeyValueResult};
use nanograph_kvt::{KeyValueIterator, ShardState, StorageEngineType};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

/// KeyValueShardManager manages multiple storage engines and provides a unified API
///
/// This is the primary interface for higher-level applications. It:
/// - Manages multiple storage engines (LSM, B+Tree, ART)
/// - Routes operations to the appropriate engine based on table configuration
/// - Provides table lifecycle management
/// - Coordinates cross-engine transactions (future)
/// - Maintains table metadata
/// - Optionally provides distributed consensus via Raft
pub struct KeyValueShardManager {
    /// Registered storage engines by type
    engines: HashMap<StorageEngineType, Arc<dyn KeyValueShardStore>>,

    /// Shards Managed by this instance
    shards: Arc<RwLock<HashMap<ShardId, ShardState>>>,

    /// Local node ID (for distributed mode)
    node_id: Option<NodeId>,

    /// Distributed mode flag
    distributed_mode: bool,
}

impl KeyValueShardManager {
    /// Create a new shard manager in single-node mode
    pub fn new() -> Self {
        Self {
            engines: HashMap::new(),
            shards: Arc::new(RwLock::new(HashMap::new())),
            node_id: None,
            distributed_mode: false,
        }
    }

    /// Create a new shard manager in distributed mode
    pub fn new_distributed(node_id: NodeId) -> Self {
        Self {
            engines: HashMap::new(),
            shards: Arc::new(RwLock::new(HashMap::new())),
            node_id: Some(node_id),
            distributed_mode: true,
        }
    }

    /// Check if running in distributed mode
    pub fn is_distributed(&self) -> bool {
        self.distributed_mode
    }

    /// Get the local node ID (if in distributed mode)
    pub fn node_id(&self) -> Option<NodeId> {
        self.node_id
    }

    /// Register a storage engine
    ///
    /// Multiple engines can be registered for different use cases:
    /// - LSM for write-heavy workloads
    /// - B+Tree for balanced workloads
    /// - ART for read-heavy, in-memory workloads
    pub fn register_engine(
        &mut self,
        engine_type: StorageEngineType,
        engine: Arc<dyn KeyValueShardStore>,
    ) -> KeyValueResult<()> {
        if self.engines.contains_key(&engine_type) {
            return Err(KeyValueError::InvalidValue(format!(
                "Engine type {:?} already registered",
                engine_type
            )));
        }
        self.engines.insert(engine_type, engine);
        Ok(())
    }

    /// Get the storage engine for a specific shard
    fn get_engine_for_shard(&self, shard: ShardId) -> KeyValueResult<Arc<dyn KeyValueShardStore>> {
        let shards = self.shards.read().unwrap();
        let shard = shards.get(&shard).ok_or(KeyValueError::InvalidKey(format!(
            "Shard {:?} not found",
            shard
        )))?;

        self.engines
            .get(&shard.engine_type)
            .cloned()
            .ok_or(KeyValueError::InvalidValue(format!(
                "Engine type {:?} not registered",
                &shard.engine_type
            )))
    }

    /// Create a new shard with the specified configuration
    pub async fn create_shard(&self, config: ShardConfig) -> KeyValueResult<ShardId> {
        // Get the engine for this table type
        let engine = self
            .engines
            .get(&config.engine_type)
            .ok_or(KeyValueError::InvalidValue(format!(
                "Engine type {:?} not registered",
                config.engine_type
            )))?;

        // Create shard in the underlying engine
        let shard_id = engine.create_shard(config.table, config.index).await?;

        let shard_state = ShardState {
            id: shard_id,
            engine_type: config.engine_type.clone(),
            replication_factor: config.replication_factor,
        };

        {
            let mut shards = self.shards.write().unwrap();
            shards.insert(shard_id, shard_state);
        }

        Ok(shard_id)
    }

    /// Drop a shard
    pub async fn drop_shard(&self, table: ShardId) -> KeyValueResult<()> {
        let engine = self.get_engine_for_shard(table)?;

        // Drop from engine
        engine.drop_shard(table).await?;

        // Remove from metadata
        {
            let mut tables = self.shards.write().unwrap();
            tables.remove(&table);
        }

        Ok(())
    }

    /// List all shards managed by this instance
    pub fn list_shards(&self) -> KeyValueResult<Vec<ShardState>> {
        let shards = self.shards.read().unwrap();
        Ok(shards.values().cloned().collect())
    }

    /// Check if a shard exists
    pub fn shard_exists(&self, shard: ShardId) -> bool {
        let shards = self.shards.read().unwrap();
        shards.contains_key(&shard)
    }

    /// Get the value for a key in a specific shard
    pub async fn get(&self, shard: ShardId, key: &[u8]) -> KeyValueResult<Option<Vec<u8>>> {
        let engine = self.get_engine_for_shard(shard)?;
        engine.get(shard, key).await
    }

    /// Put a key-value pair into a specific shard
    pub async fn put(&self, shard: ShardId, key: &[u8], value: &[u8]) -> KeyValueResult<()> {
        let engine = self.get_engine_for_shard(shard)?;
        engine.put(shard, key, value).await
    }

    /// Delete a key from a specific shard
    ///
    /// Returns true if the key existed and was deleted, false otherwise.
    pub async fn delete(&self, shard: ShardId, key: &[u8]) -> KeyValueResult<bool> {
        let engine = self.get_engine_for_shard(shard)?;
        engine.delete(shard, key).await
    }

    /// Check if a key exists in a specific shard
    pub async fn exists(&self, shard: ShardId, key: &[u8]) -> KeyValueResult<bool> {
        let engine = self.get_engine_for_shard(shard)?;
        engine.exists(shard, key).await
    }

    /// Get multiple values from a specific shard
    pub async fn batch_get(
        &self,
        shard: ShardId,
        keys: &[&[u8]],
    ) -> KeyValueResult<Vec<Option<Vec<u8>>>> {
        let engine = self.get_engine_for_shard(shard)?;
        engine.batch_get(shard, keys).await
    }

    /// Put multiple key-value pairs into a specific shard
    pub async fn batch_put(&self, shard: ShardId, pairs: &[(&[u8], &[u8])]) -> KeyValueResult<()> {
        let engine = self.get_engine_for_shard(shard)?;
        engine.batch_put(shard, pairs).await
    }

    /// Delete multiple keys from a specific shard
    ///
    /// Returns the number of keys deleted.
    pub async fn batch_delete(&self, shard: ShardId, keys: &[&[u8]]) -> KeyValueResult<usize> {
        let engine = self.get_engine_for_shard(shard)?;
        engine.batch_delete(shard, keys).await
    }

    /// Scan a range of keys in a specific shard
    pub async fn scan(
        &self,
        shard: ShardId,
        range: KeyRange,
    ) -> KeyValueResult<Box<dyn KeyValueIterator + Send>> {
        let engine = self.get_engine_for_shard(shard)?;
        engine.scan(shard, range).await
    }

    /// Scan keys with a specific prefix in a specific shard
    pub async fn scan_prefix(
        &self,
        shard: ShardId,
        prefix: &[u8],
        limit: Option<usize>,
    ) -> KeyValueResult<Box<dyn KeyValueIterator + Send>> {
        let engine = self.get_engine_for_shard(shard)?;
        engine.scan_prefix(shard, prefix, limit).await
    }

    /// Get the total number of keys in a specific shard
    pub async fn key_count(&self, shard: ShardId) -> KeyValueResult<u64> {
        let engine = self.get_engine_for_shard(shard)?;
        engine.key_count(shard).await
    }

    /// Get statistics for a specific shard
    pub async fn shard_stats(&self, shard: ShardId) -> KeyValueResult<ShardStats> {
        let engine = self.get_engine_for_shard(shard)?;
        engine.shard_stats(shard).await
    }

    /// Flush all registered storage engines to disk
    pub async fn flush(&self) -> KeyValueResult<()> {
        // Flush all engines
        for engine in self.engines.values() {
            engine.flush().await?;
        }
        Ok(())
    }

    /// Compact a specific shard or all registered storage engines
    ///
    /// If `shard` is `Some`, only that shard is compacted.
    /// If `shard` is `None`, all engines are compacted.
    pub async fn compact(&self, shard: Option<ShardId>) -> KeyValueResult<()> {
        if let Some(shard_id) = shard {
            // Compact specific table
            let engine = self.get_engine_for_shard(shard_id)?;
            engine.compact(Some(shard_id)).await
        } else {
            // Compact all engines
            for engine in self.engines.values() {
                engine.compact(None).await?;
            }
            Ok(())
        }
    }
}

impl Default for KeyValueShardManager {
    fn default() -> Self {
        Self::new()
    }
}
