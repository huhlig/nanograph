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

use crate::config::BTreeStorageConfig;
use crate::iterator::BPlusTreeIterator;
use crate::metrics::BTreeMetrics;
use crate::transaction::TransactionManager;
use crate::tree::{BPlusTree, BPlusTreeConfig};
use crate::wal_record::{
    WalRecordKind, decode_delete, decode_put, encode_checkpoint, encode_clear, encode_delete,
    encode_put,
};
use async_trait::async_trait;
use nanograph_kvt::metrics::{ShardStats, StatValue};
use nanograph_kvt::{
    KeyRange, KeyValueError, KeyValueIterator, KeyValueResult, KeyValueShardStore, ShardId,
    Transaction,
};
use nanograph_vfs::{DynamicFileSystem, MemoryFileSystem, Path};
use nanograph_wal::{
    LogSequenceNumber, WriteAheadLogConfig, WriteAheadLogManager, WriteAheadLogRecord,
};
use std::collections::HashMap;
use std::sync::{Arc, Mutex, RwLock};

/// Shard data including tree and WAL
struct ShardData {
    tree: Arc<BPlusTree>,
    wal: Option<Arc<WriteAheadLogManager>>,
    wal_writer: Option<Arc<Mutex<nanograph_wal::WriteAheadLogWriter>>>,
    flushed_lsn: Arc<RwLock<Option<LogSequenceNumber>>>,
}

/// B+Tree implementation of KeyValueStore
///
/// This is a low-level storage engine that manages physical shards.
/// It does NOT manage table names or allocate IDs - that's the responsibility
/// of KeyValueDatabaseManager at a higher level.
pub struct BTreeKeyValueStore {
    /// Shard data (trees + WAL) for each shard
    shards: Arc<RwLock<HashMap<ShardId, Arc<ShardData>>>>,

    /// Transaction manager
    tx_manager: Arc<TransactionManager>,

    /// Metrics for each shard
    metrics: Arc<RwLock<HashMap<ShardId, Arc<BTreeMetrics>>>>,

    /// Default B+Tree configuration
    config: BPlusTreeConfig,

    /// Enable WAL for durability
    wal_enabled: bool,
}

impl BTreeKeyValueStore {
    /// Create a new B+Tree key-value store without WAL
    pub fn new(config: BPlusTreeConfig) -> Self {
        Self {
            shards: Arc::new(RwLock::new(HashMap::new())),
            tx_manager: Arc::new(TransactionManager::new()),
            metrics: Arc::new(RwLock::new(HashMap::new())),
            config,
            wal_enabled: false,
        }
    }

    /// Create a new B+Tree key-value store with WAL enabled
    pub fn with_wal(config: BPlusTreeConfig) -> Self {
        Self {
            shards: Arc::new(RwLock::new(HashMap::new())),
            tx_manager: Arc::new(TransactionManager::new()),
            metrics: Arc::new(RwLock::new(HashMap::new())),
            config,
            wal_enabled: true,
        }
    }

    /// Create a shard with VFS and tablespace-resolved configuration
    /// This is the new tablespace-aware method that will be used by the shard manager
    pub fn create_shard_with_config(
        &self,
        shard: ShardId,
        vfs: Arc<dyn DynamicFileSystem>,
        config: BTreeStorageConfig,
    ) -> KeyValueResult<()> {
        // Ensure directories exist
        vfs.create_directory_all(&config.data_dir)
            .map_err(|e| KeyValueError::StorageCorruption(e.to_string()))?;
        vfs.create_directory_all(&config.wal_dir)
            .map_err(|e| KeyValueError::StorageCorruption(e.to_string()))?;

        // Create B+Tree with custom configuration
        let tree_config = BPlusTreeConfig {
            max_keys: config.order * 2,
            min_keys: config.order,
        };
        let tree = Arc::new(BPlusTree::new(tree_config));

        // Create WAL if enabled
        let (wal, wal_writer) = if self.wal_enabled {
            let wal_fs = vfs.clone();
            let wal_path = Path::from(config.wal_dir.as_str());
            let wal_config = WriteAheadLogConfig::new(shard.0);

            let wal_manager = WriteAheadLogManager::new(wal_fs, wal_path, wal_config)
                .map_err(|e| KeyValueError::StorageCorruption(e.to_string()))?;

            let writer = wal_manager
                .writer()
                .map_err(|e| KeyValueError::StorageCorruption(e.to_string()))?;

            (
                Some(Arc::new(wal_manager)),
                Some(Arc::new(Mutex::new(writer))),
            )
        } else {
            (None, None)
        };

        // Create shard data
        let shard_data = Arc::new(ShardData {
            tree: tree.clone(),
            wal,
            wal_writer,
            flushed_lsn: Arc::new(RwLock::new(None)),
        });

        // Recover from WAL if it exists
        if self.wal_enabled {
            self.recover_from_wal(shard, &tree)?;
        }

        // Store shard data
        let mut shards = self.shards.write().unwrap();
        shards.insert(shard, shard_data);

        // Initialize metrics
        let mut metrics = self.metrics.write().unwrap();
        metrics.insert(shard, Arc::new(BTreeMetrics::new()));

        Ok(())
    }

    /// Get the shard data
    fn get_shard(&self, shard: ShardId) -> KeyValueResult<Arc<ShardData>> {
        let shards = self.shards.read().unwrap();
        shards
            .get(&shard)
            .cloned()
            .ok_or(KeyValueError::ShardNotFound(shard))
    }

    /// Get the tree for a shard
    fn get_tree(&self, shard: ShardId) -> KeyValueResult<Arc<BPlusTree>> {
        let shard_data = self.get_shard(shard)?;
        Ok(shard_data.tree.clone())
    }

    /// Get the metrics for a shard
    fn get_metrics(&self, shard: ShardId) -> Arc<BTreeMetrics> {
        let mut metrics = self.metrics.write().unwrap();
        metrics
            .entry(shard)
            .or_insert_with(|| Arc::new(BTreeMetrics::new()))
            .clone()
    }

    /// Write a WAL record for a put operation
    fn wal_write_put(&self, shard: ShardId, key: &[u8], value: &[u8]) -> KeyValueResult<()> {
        if !self.wal_enabled {
            return Ok(());
        }

        let shard_data = self.get_shard(shard)?;
        if let Some(wal_writer) = &shard_data.wal_writer {
            let record_data = encode_put(key, value);
            let record = WriteAheadLogRecord {
                kind: WalRecordKind::Put as u16,
                payload: &record_data,
            };

            let mut writer = wal_writer.lock().unwrap();
            writer
                .append(record, nanograph_wal::Durability::Sync)
                .map_err(|e| {
                    KeyValueError::StorageCorruption(format!("WAL write failed: {}", e))
                })?;
        }

        Ok(())
    }

    /// Write a WAL record for a delete operation
    fn wal_write_delete(&self, shard: ShardId, key: &[u8]) -> KeyValueResult<()> {
        if !self.wal_enabled {
            return Ok(());
        }

        let shard_data = self.get_shard(shard)?;
        if let Some(wal_writer) = &shard_data.wal_writer {
            let record_data = encode_delete(key);
            let record = WriteAheadLogRecord {
                kind: WalRecordKind::Delete as u16,
                payload: &record_data,
            };

            let mut writer = wal_writer.lock().unwrap();
            writer
                .append(record, nanograph_wal::Durability::Sync)
                .map_err(|e| {
                    KeyValueError::StorageCorruption(format!("WAL write failed: {}", e))
                })?;
        }

        Ok(())
    }

    /// Recover a shard from WAL by replaying all records
    fn recover_from_wal(&self, shard: ShardId, tree: &Arc<BPlusTree>) -> KeyValueResult<()> {
        if !self.wal_enabled {
            return Ok(());
        }

        let shard_data = self.get_shard(shard)?;
        if let Some(wal) = &shard_data.wal {
            // Get WAL reader starting from the beginning
            let mut reader = wal.reader_from(LogSequenceNumber::ZERO).map_err(|e| {
                KeyValueError::StorageCorruption(format!("Failed to create WAL reader: {}", e))
            })?;

            let mut recovered_count = 0;

            // Replay all WAL records
            while let Some(entry) = reader
                .next()
                .map_err(|e| KeyValueError::StorageCorruption(format!("WAL read error: {}", e)))?
            {
                match WalRecordKind::from_u16(entry.kind) {
                    Some(WalRecordKind::Put) => {
                        let (key, value) = decode_put(&entry.payload).map_err(|e| {
                            KeyValueError::StorageCorruption(format!("Failed to decode put: {}", e))
                        })?;
                        tree.insert(key, value)
                            .map_err(|e| KeyValueError::StorageCorruption(e.to_string()))?;
                        recovered_count += 1;
                    }
                    Some(WalRecordKind::Delete) => {
                        let key = decode_delete(&entry.payload).map_err(|e| {
                            KeyValueError::StorageCorruption(format!(
                                "Failed to decode delete: {}",
                                e
                            ))
                        })?;
                        let _ = tree.delete(&key);
                        recovered_count += 1;
                    }
                    Some(WalRecordKind::Checkpoint) => {
                        // Checkpoint marker - we can stop here if we want
                        // For now, continue replaying
                    }
                    Some(WalRecordKind::Clear) => {
                        tree.clear().map_err(|e| {
                            KeyValueError::StorageCorruption(format!("Failed to clear: {}", e))
                        })?;
                    }
                    None => {
                        // Unknown record type - skip it
                        continue;
                    }
                }
            }

            if recovered_count > 0 {
                // Log recovery success
                eprintln!(
                    "INFO: Recovered {} operations from WAL for shard {:?}",
                    recovered_count, shard
                );
            }
        }

        Ok(())
    }

    /// Create a checkpoint for a shard
    /// This saves the current tree state and writes a checkpoint marker to the WAL
    pub async fn checkpoint_shard(&self, shard: ShardId) -> KeyValueResult<()> {
        let shard_data = self.get_shard(shard)?;

        // Write checkpoint marker to WAL
        if self.wal_enabled {
            if let Some(wal_writer) = &shard_data.wal_writer {
                let checkpoint_data = encode_checkpoint();
                let record = WriteAheadLogRecord {
                    kind: WalRecordKind::Checkpoint as u16,
                    payload: &checkpoint_data,
                };
                let mut writer = wal_writer.lock().unwrap();
                writer
                    .append(record, nanograph_wal::Durability::Sync)
                    .map_err(|e| {
                        KeyValueError::StorageCorruption(format!("Checkpoint write failed: {}", e))
                    })?;
            }
        }

        Ok(())
    }

    /// Create checkpoints for all shards
    pub async fn checkpoint_all(&self) -> KeyValueResult<()> {
        let shard_ids: Vec<ShardId> = {
            let shards = self.shards.read().unwrap();
            shards.keys().copied().collect()
        };

        for shard_id in shard_ids {
            self.checkpoint_shard(shard_id).await?;
        }

        Ok(())
    }
}

#[async_trait]
impl KeyValueShardStore for BTreeKeyValueStore {
    // ===== Basic Operations =====

    async fn get(&self, table: ShardId, key: &[u8]) -> KeyValueResult<Option<Vec<u8>>> {
        let tree = self.get_tree(table)?;
        let metrics = self.get_metrics(table);

        let result = tree.get(key).map_err(Into::<KeyValueError>::into)?;
        metrics.record_read(result.is_some());

        Ok(result)
    }

    async fn put(&self, table: ShardId, key: &[u8], value: &[u8]) -> KeyValueResult<()> {
        // Write to WAL first
        self.wal_write_put(table, key, value)?;

        let tree = self.get_tree(table)?;
        let metrics = self.get_metrics(table);

        // Check if key exists (for update tracking)
        let exists = tree
            .get(key)
            .map_err(Into::<KeyValueError>::into)?
            .is_some();

        tree.insert(key.to_vec(), value.to_vec())
            .map_err(Into::<KeyValueError>::into)?;

        metrics.record_write(exists);

        Ok(())
    }

    async fn delete(&self, table: ShardId, key: &[u8]) -> KeyValueResult<bool> {
        // Write to WAL first
        self.wal_write_delete(table, key)?;

        let tree = self.get_tree(table)?;
        let metrics = self.get_metrics(table);

        let deleted = tree.delete(key).map_err(Into::<KeyValueError>::into)?;

        if deleted {
            metrics.record_delete();
        }

        Ok(deleted)
    }

    async fn exists(&self, table: ShardId, key: &[u8]) -> KeyValueResult<bool> {
        let tree = self.get_tree(table)?;
        let result = tree.get(key).map_err(Into::<KeyValueError>::into)?;
        Ok(result.is_some())
    }

    // ===== Batch Operations =====

    async fn batch_get(
        &self,
        table: ShardId,
        keys: &[&[u8]],
    ) -> KeyValueResult<Vec<Option<Vec<u8>>>> {
        let tree = self.get_tree(table)?;
        let metrics = self.get_metrics(table);

        let mut results = Vec::with_capacity(keys.len());
        for key in keys {
            let result = tree.get(key).map_err(Into::<KeyValueError>::into)?;
            metrics.record_read(result.is_some());
            results.push(result);
        }

        Ok(results)
    }

    async fn batch_put(&self, table: ShardId, pairs: &[(&[u8], &[u8])]) -> KeyValueResult<()> {
        // Write all operations to WAL first
        for (key, value) in pairs {
            self.wal_write_put(table, key, value)?;
        }

        let tree = self.get_tree(table)?;
        let metrics = self.get_metrics(table);

        for (key, value) in pairs {
            let exists = tree
                .get(key)
                .map_err(Into::<KeyValueError>::into)?
                .is_some();
            tree.insert(key.to_vec(), value.to_vec())
                .map_err(Into::<KeyValueError>::into)?;
            metrics.record_write(exists);
        }

        Ok(())
    }

    async fn batch_delete(&self, table: ShardId, keys: &[&[u8]]) -> KeyValueResult<usize> {
        // Write all operations to WAL first
        for key in keys {
            self.wal_write_delete(table, key)?;
        }

        let tree = self.get_tree(table)?;
        let metrics = self.get_metrics(table);

        let mut count = 0;
        for key in keys {
            if tree.delete(key).map_err(Into::<KeyValueError>::into)? {
                count += 1;
                metrics.record_delete();
            }
        }

        Ok(count)
    }

    // ===== Range Operations =====

    async fn scan(
        &self,
        table: ShardId,
        range: KeyRange,
    ) -> KeyValueResult<Box<dyn KeyValueIterator + Send>> {
        let tree = self.get_tree(table)?;
        let metrics = self.get_metrics(table);

        metrics.record_scan(0); // Will be updated as iteration progresses

        let iterator =
            BPlusTreeIterator::new(tree, range.start, range.end, range.reverse, range.limit)
                .map_err(|e| Into::<KeyValueError>::into(e))?;

        // Return the concrete type directly - trait object will handle pinning
        Ok(Box::new(iterator) as Box<dyn KeyValueIterator + Send>)
    }

    // ===== Statistics & Metadata =====

    async fn key_count(&self, table: ShardId) -> KeyValueResult<u64> {
        let tree = self.get_tree(table)?;
        let stats = tree.stats();
        Ok(stats.num_keys as u64)
    }

    async fn shard_stats(&self, table: ShardId) -> KeyValueResult<ShardStats> {
        let tree = self.get_tree(table)?;
        let tree_stats = tree.stats();
        let metrics = self.get_metrics(table);
        let metrics_snapshot = metrics.snapshot();

        // Estimate bytes (rough calculation)
        let avg_key_size = 32; // Assume average key size
        let avg_value_size = 128; // Assume average value size
        let data_bytes = (tree_stats.num_keys as u64) * (avg_key_size + avg_value_size);

        // Estimate index overhead (internal nodes)
        let index_bytes = (tree_stats.num_internal_nodes as u64) * 1024; // Rough estimate

        let mut shard_stats = ShardStats {
            key_count: tree_stats.num_keys as u64,
            total_bytes: data_bytes + index_bytes,
            data_bytes,
            index_bytes,
            last_modified: None,
            engine_stats: Default::default(),
        };

        // Build B+Tree specific stats
        shard_stats
            .engine_stats
            .insert("tree_height", StatValue::from_usize(tree_stats.height));
        shard_stats.engine_stats.insert(
            "total_nodes",
            StatValue::from_usize(tree_stats.num_internal_nodes + tree_stats.num_leaf_nodes),
        );
        shard_stats.engine_stats.insert(
            "internal_nodes",
            StatValue::from_usize(tree_stats.num_internal_nodes),
        );
        shard_stats.engine_stats.insert(
            "leaf_nodes",
            StatValue::from_usize(tree_stats.num_leaf_nodes),
        );
        shard_stats.engine_stats.insert(
            "avg_node_utilization",
            StatValue::from_f64(if tree_stats.num_leaf_nodes > 0 {
                tree_stats.num_keys as f64 / tree_stats.num_leaf_nodes as f64
            } else {
                0.0
            }),
        );
        shard_stats.engine_stats.insert(
            "total_splits",
            StatValue::from_u64(metrics_snapshot.node_splits),
        );
        shard_stats.engine_stats.insert(
            "total_merges",
            StatValue::from_u64(metrics_snapshot.node_merges),
        );
        shard_stats.engine_stats.insert(
            "total_merges",
            StatValue::from_usize(self.config.max_keys * 64),
        );

        Ok(shard_stats)
    }

    // ===== Transaction Support =====

    async fn begin_transaction(&self) -> KeyValueResult<Arc<dyn Transaction>> {
        // For now, create a transaction on the first shard
        // In a real implementation, transactions should span multiple shards
        let shards = self.shards.read().unwrap();
        let (shard_id, shard_data) = if let Some((id, data)) = shards.iter().next() {
            (*id, data.clone())
        } else {
            return Err(KeyValueError::StorageCorruption(
                "No shards available for transaction".to_string(),
            ));
        };

        let tx = self.tx_manager.begin(shard_id, shard_data.tree.clone());
        Ok(tx as Arc<dyn Transaction>)
    }

    // ===== Shard Management =====

    fn create_shard(
        &self,
        shard_id: ShardId,
        _vfs: Arc<dyn nanograph_vfs::DynamicFileSystem>,
        _data_path: nanograph_vfs::Path,
        _wal_path: nanograph_vfs::Path,
    ) -> KeyValueResult<()> {
        // B+Tree is an in-memory store, so we ignore the tablespace paths for now
        // In a real implementation, you might use these paths for persistence
        
        // Create a new B+Tree for this shard
        let tree = Arc::new(BPlusTree::new(self.config.clone()));

        // Create WAL if enabled
        let (wal, wal_writer) = if self.wal_enabled {
            // Create memory filesystem for WAL
            let wal_fs = MemoryFileSystem::new();
            let wal_path_str = format!("/wal_{}", shard_id.0);
            let wal_path = Path::from(wal_path_str.as_str());

            // Create WAL manager with config
            let wal_config = WriteAheadLogConfig::new(shard_id.0);
            let wal_mgr = WriteAheadLogManager::new(wal_fs, wal_path, wal_config).map_err(|e| {
                KeyValueError::StorageCorruption(format!("Failed to create WAL: {}", e))
            })?;

            // Create WAL writer
            let wal_writer = wal_mgr.writer().map_err(|e| {
                KeyValueError::StorageCorruption(format!("Failed to create WAL writer: {}", e))
            })?;

            (
                Some(Arc::new(wal_mgr)),
                Some(Arc::new(Mutex::new(wal_writer))),
            )
        } else {
            (None, None)
        };

        let shard_data = Arc::new(ShardData {
            tree: tree.clone(),
            wal,
            wal_writer,
            flushed_lsn: Arc::new(RwLock::new(None)),
        });

        // Store shard data
        {
            let mut shards = self.shards.write().unwrap();
            shards.insert(shard_id, shard_data);
        }

        // Initialize metrics for this shard
        {
            let mut metrics = self.metrics.write().unwrap();
            metrics.insert(shard_id, Arc::new(BTreeMetrics::new()));
        }

        // Recover from WAL if it exists
        self.recover_from_wal(shard_id, &tree)?;

        Ok(())
    }

    async fn drop_shard(&self, shard: ShardId) -> KeyValueResult<()> {
        let mut shards = self.shards.write().unwrap();
        shards.remove(&shard);

        let mut metrics = self.metrics.write().unwrap();
        metrics.remove(&shard);

        Ok(())
    }

    async fn clear(&self, shard: ShardId) -> KeyValueResult<()> {
        let shard_data = self.get_shard(shard)?;

        // Clear the tree
        shard_data
            .tree
            .clear()
            .map_err(|e| KeyValueError::StorageCorruption(e.to_string()))?;

        // Write clear record to WAL if enabled
        if let Some(ref wal_writer_mutex) = shard_data.wal_writer {
            let mut wal_writer = wal_writer_mutex.lock().unwrap();
            let payload = encode_clear();
            let record = WriteAheadLogRecord {
                kind: WalRecordKind::Clear.to_u16(),
                payload: &payload,
            };

            wal_writer
                .append(record, nanograph_wal::Durability::Sync)
                .map_err(|e| KeyValueError::StorageCorruption(e.to_string()))?;
        }

        // Reset metrics
        let metrics = self.get_metrics(shard);
        metrics.reset();

        Ok(())
    }

    async fn list_shards(&self) -> KeyValueResult<Vec<ShardId>> {
        let shards = self.shards.read().unwrap();
        Ok(shards.keys().copied().collect())
    }

    async fn shard_exists(&self, shard: ShardId) -> KeyValueResult<bool> {
        let shards = self.shards.read().unwrap();
        Ok(shards.contains_key(&shard))
    }

    // ===== Maintenance Operations =====

    async fn flush(&self) -> KeyValueResult<()> {
        // B+Tree is in-memory, so flush is a no-op
        // In a persistent implementation, this would write to disk
        Ok(())
    }

    async fn compact(&self, _table: Option<ShardId>) -> KeyValueResult<()> {
        // B+Tree doesn't need compaction like LSM trees
        // This is a no-op
        Ok(())
    }
}

impl Default for BTreeKeyValueStore {
    fn default() -> Self {
        Self::new(BPlusTreeConfig::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_table_management() {
        let store = BTreeKeyValueStore::default();

        // Create table
        let shard_id = ShardId::new(0);
        let vfs = Arc::new(nanograph_vfs::MemoryFileSystem::new());
        let data_path = nanograph_vfs::Path::from("/data");
        let wal_path = nanograph_vfs::Path::from("/wal");
        store.create_shard(shard_id, vfs, data_path, wal_path).unwrap();

        assert!(store.shard_exists(shard_id).await.unwrap());

        // List tables
        let shards = store.list_shards().await.unwrap();
        assert_eq!(shards.len(), 1);

        // Drop table
        store.drop_shard(shard_id).await.unwrap();
        assert!(!store.shard_exists(shard_id).await.unwrap());
    }

    #[tokio::test]
    async fn test_clear_operation() {
        let store = BTreeKeyValueStore::default();

        let shard_id = ShardId::new(0);
        let vfs = Arc::new(nanograph_vfs::MemoryFileSystem::new());
        let data_path = nanograph_vfs::Path::from("/data");
        let wal_path = nanograph_vfs::Path::from("/wal");
        store.create_shard(shard_id, vfs, data_path, wal_path).unwrap();

        // Put some data
        store.put(shard_id, b"key1", b"value1").await.unwrap();
        store.put(shard_id, b"key2", b"value2").await.unwrap();
        assert_eq!(store.key_count(shard_id).await.unwrap(), 2);

        // Clear the shard
        store.clear(shard_id).await.unwrap();

        // Verify it's empty
        assert_eq!(store.key_count(shard_id).await.unwrap(), 0);
        assert_eq!(store.get(shard_id, b"key1").await.unwrap(), None);
        assert_eq!(store.get(shard_id, b"key2").await.unwrap(), None);

        // Verify shard still exists
        assert!(store.shard_exists(shard_id).await.unwrap());

        // Put data again
        store.put(shard_id, b"key3", b"value3").await.unwrap();
        assert_eq!(store.key_count(shard_id).await.unwrap(), 1);
        assert_eq!(
            store.get(shard_id, b"key3").await.unwrap(),
            Some(b"value3".to_vec())
        );
    }

    #[tokio::test]
    async fn test_basic_operations() {
        let store = BTreeKeyValueStore::default();

        let shard_id = ShardId::new(0);
        let vfs = Arc::new(nanograph_vfs::MemoryFileSystem::new());
        let data_path = nanograph_vfs::Path::from("/data");
        let wal_path = nanograph_vfs::Path::from("/wal");
        store.create_shard(shard_id, vfs, data_path, wal_path).unwrap();

        // Put
        store.put(shard_id, b"key1", b"value1").await.unwrap();
        store.put(shard_id, b"key2", b"value2").await.unwrap();

        // Get
        assert_eq!(
            store.get(shard_id, b"key1").await.unwrap(),
            Some(b"value1".to_vec())
        );
        assert_eq!(
            store.get(shard_id, b"key2").await.unwrap(),
            Some(b"value2".to_vec())
        );
        assert_eq!(store.get(shard_id, b"key3").await.unwrap(), None);

        // Exists
        assert!(store.exists(shard_id, b"key1").await.unwrap());
        assert!(!store.exists(shard_id, b"key3").await.unwrap());

        // Delete
        assert!(store.delete(shard_id, b"key1").await.unwrap());
        assert!(!store.delete(shard_id, b"key3").await.unwrap());
        assert_eq!(store.get(shard_id, b"key1").await.unwrap(), None);
    }

    #[tokio::test]
    async fn test_batch_operations() {
        let store = BTreeKeyValueStore::default();

        let shard_id = ShardId::new(0);
        let vfs = Arc::new(nanograph_vfs::MemoryFileSystem::new());
        let data_path = nanograph_vfs::Path::from("/data");
        let wal_path = nanograph_vfs::Path::from("/wal");
        store.create_shard(shard_id, vfs, data_path, wal_path).unwrap();

        // Batch put
        let pairs = vec![
            (&b"key1"[..], &b"value1"[..]),
            (&b"key2"[..], &b"value2"[..]),
            (&b"key3"[..], &b"value3"[..]),
        ];
        store.batch_put(shard_id, &pairs).await.unwrap();

        // Batch get
        let keys = vec![&b"key1"[..], &b"key2"[..], &b"key3"[..], &b"key4"[..]];
        let results = store.batch_get(shard_id, &keys).await.unwrap();
        assert_eq!(results[0], Some(b"value1".to_vec()));
        assert_eq!(results[1], Some(b"value2".to_vec()));
        assert_eq!(results[2], Some(b"value3".to_vec()));
        assert_eq!(results[3], None);

        // Batch delete
        let delete_keys = vec![&b"key1"[..], &b"key2"[..], &b"key4"[..]];
        let deleted = store.batch_delete(shard_id, &delete_keys).await.unwrap();
        assert_eq!(deleted, 2);
    }

    #[tokio::test]
    async fn test_statistics() {
        let store = BTreeKeyValueStore::default();
        let shard_id = ShardId::new(0);
        let vfs = Arc::new(nanograph_vfs::MemoryFileSystem::new());
        let data_path = nanograph_vfs::Path::from("/data");
        let wal_path = nanograph_vfs::Path::from("/wal");
        store.create_shard(shard_id, vfs, data_path, wal_path).unwrap();

        // Insert data
        for i in 0..100 {
            let key = format!("key{:03}", i);
            let value = format!("value{}", i);
            store
                .put(shard_id, key.as_bytes(), value.as_bytes())
                .await
                .unwrap();
        }

        // Check stats
        let count = store.key_count(shard_id).await.unwrap();
        assert_eq!(count, 100);

        let stats = store.shard_stats(shard_id).await.unwrap();
        assert_eq!(stats.key_count, 100);
        assert!(stats.total_bytes > 0);
    }

    #[tokio::test]
    async fn test_transactions() {
        let store = BTreeKeyValueStore::default();

        let shard_id = ShardId::new(0);
        let vfs = Arc::new(nanograph_vfs::MemoryFileSystem::new());
        let data_path = nanograph_vfs::Path::from("/data");
        let wal_path = nanograph_vfs::Path::from("/wal");
        store.create_shard(shard_id, vfs, data_path, wal_path).unwrap();

        // Begin transaction
        let tx = store.begin_transaction().await.unwrap();
        assert!(tx.id().0 > 0);

        // Commit
        let tx_id = tx.id();
        tx.commit().await.unwrap();

        // Begin another transaction
        let tx2 = store.begin_transaction().await.unwrap();
        assert!(tx2.id().0 > tx_id.0);

        // Rollback
        tx2.rollback().await.unwrap();
    }
}
