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

use crate::iterator::ArtIterator;
use crate::metrics::ArtMetrics;
use crate::tree::AdaptiveRadixTree;
use crate::wal_record::{WalRecordKind, decode_put};
use async_trait::async_trait;
use futures_core::Stream;
use nanograph_kvt::metrics::{ShardStats, StatValue};
use nanograph_kvt::{
    KeyRange, KeyValueError, KeyValueIterator, KeyValueResult, KeyValueShardStore, ShardId,
    ShardIndex, TableId, Transaction,
};
use nanograph_vfs::{MemoryFileSystem, Path};
use nanograph_wal::{
    LogSequenceNumber, WriteAheadLogConfig, WriteAheadLogManager, WriteAheadLogRecord,
};
use std::collections::HashMap;
use std::ops::Bound;
use std::pin::Pin;
use std::sync::{Arc, Mutex, RwLock};
use std::task::{Context, Poll};

/// Wrapper to adapt ArtIterator to KeyValueIterator trait
struct ArtKeyValueIterator {
    /// Inner ART iterator
    inner: ArtIterator<Vec<u8>>,
    /// Start bound for range scan
    start_bound: Bound<Vec<u8>>,
    /// End bound for range scan
    end_bound: Bound<Vec<u8>>,
    /// Whether to iterate in reverse order
    reverse: bool,
    /// Maximum number of items to return
    limit: Option<usize>,
    /// Number of items already returned
    count: usize,
    /// Current key-value pair
    current: Option<(Vec<u8>, Vec<u8>)>,
    /// Whether the iterator has reached the end
    exhausted: bool,
}

impl ArtKeyValueIterator {
    /// Create a new ART key-value iterator
    fn new(
        tree: AdaptiveRadixTree<Vec<u8>>,
        start: Bound<Vec<u8>>,
        end: Bound<Vec<u8>>,
        reverse: bool,
        limit: Option<usize>,
    ) -> Self {
        let root = tree.root();
        let inner = ArtIterator::new(root);

        Self {
            inner,
            start_bound: start,
            end_bound: end,
            reverse,
            limit,
            count: 0,
            current: None,
            exhausted: false,
        }
    }

    /// Check if a key is within the iterator bounds
    fn check_bounds(&self, key: &[u8]) -> bool {
        let after_start = match &self.start_bound {
            Bound::Included(start) => key >= start.as_slice(),
            Bound::Excluded(start) => key > start.as_slice(),
            Bound::Unbounded => true,
        };

        let before_end = match &self.end_bound {
            Bound::Included(end) => key <= end.as_slice(),
            Bound::Excluded(end) => key < end.as_slice(),
            Bound::Unbounded => true,
        };

        after_start && before_end
    }
}

impl Stream for ArtKeyValueIterator {
    type Item = KeyValueResult<(Vec<u8>, Vec<u8>)>;

    /// Poll for the next item in the stream
    fn poll_next(mut self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        if self.exhausted {
            return Poll::Ready(None);
        }

        // Check limit
        if let Some(limit) = self.limit {
            if self.count >= limit {
                self.exhausted = true;
                return Poll::Ready(None);
            }
        }

        // Get next item from inner iterator
        while let Some((key, value)) = self.inner.next() {
            if self.check_bounds(&key) {
                self.count += 1;
                self.current = Some((key.clone(), value.clone()));
                return Poll::Ready(Some(Ok((key, value))));
            }

            // If we're past the end bound, stop iterating
            // This handles the case where we've moved beyond the range
            match &self.end_bound {
                Bound::Excluded(end) => {
                    if &key >= end {
                        self.exhausted = true;
                        return Poll::Ready(None);
                    }
                }
                Bound::Included(end) => {
                    if &key > end {
                        self.exhausted = true;
                        return Poll::Ready(None);
                    }
                }
                Bound::Unbounded => {}
            }
        }

        self.exhausted = true;
        Poll::Ready(None)
    }
}

impl KeyValueIterator for ArtKeyValueIterator {
    /// Seek to the first key greater than or equal to the given key
    fn seek(&mut self, _key: &[u8]) -> KeyValueResult<()> {
        // TODO: Implement efficient seeking
        Ok(())
    }

    /// Get the current position of the iterator
    fn position(&self) -> Option<Vec<u8>> {
        self.current.as_ref().map(|(k, _)| k.clone())
    }

    /// Check if the iterator is still valid
    fn valid(&self) -> bool {
        !self.exhausted
    }
}

impl Unpin for ArtKeyValueIterator {}

/// Shard data including tree and WAL
struct ShardData {
    /// In-memory Adaptive Radix Tree
    tree: Arc<RwLock<AdaptiveRadixTree<Vec<u8>>>>,
    /// Optional Write-Ahead Log manager
    wal: Option<Arc<WriteAheadLogManager>>,
    /// Optional Write-Ahead Log writer
    wal_writer: Option<Arc<Mutex<nanograph_wal::WriteAheadLogWriter>>>,
    /// Last LSN that was flushed to disk
    flushed_lsn: Arc<RwLock<Option<LogSequenceNumber>>>,
}

/// ART implementation of KeyValueShardStore
///
/// This is a low-level storage engine that manages physical shards using
/// Adaptive Radix Trees. It does NOT manage table names or allocate IDs -
/// that's the responsibility of KeyValueDatabaseManager at a higher level.
pub struct ArtKeyValueStore {
    /// Shard data (trees + WAL) for each shard
    shards: Arc<RwLock<HashMap<ShardId, Arc<ShardData>>>>,

    /// Metrics for each shard
    metrics: Arc<RwLock<HashMap<ShardId, Arc<ArtMetrics>>>>,

    /// Enable WAL for durability
    wal_enabled: bool,

    /// Transaction manager (lazy initialized)
    tx_manager: Arc<RwLock<Option<Arc<crate::transaction::TransactionManager>>>>,
}

impl ArtKeyValueStore {
    /// Create a new ART key-value store without WAL
    pub fn new() -> Self {
        Self {
            shards: Arc::new(RwLock::new(HashMap::new())),
            metrics: Arc::new(RwLock::new(HashMap::new())),
            wal_enabled: false,
            tx_manager: Arc::new(RwLock::new(None)),
        }
    }

    /// Create a new ART key-value store with WAL enabled
    pub fn with_wal() -> Self {
        Self {
            shards: Arc::new(RwLock::new(HashMap::new())),
            metrics: Arc::new(RwLock::new(HashMap::new())),
            wal_enabled: true,
            tx_manager: Arc::new(RwLock::new(None)),
        }
    }

    /// Initialize transaction manager (must be called after wrapping in Arc)
    pub fn init_tx_manager(self: &Arc<Self>) {
        let tx_mgr = Arc::new(crate::transaction::TransactionManager::new(Arc::clone(
            self,
        )));
        *self.tx_manager.write().unwrap() = Some(tx_mgr);
    }

    /// Get transaction manager
    fn get_tx_manager(&self) -> Arc<crate::transaction::TransactionManager> {
        self.tx_manager.read().unwrap().as_ref().unwrap().clone()
    }

    /// Get the shard data
    fn get_shard(&self, shard: ShardId) -> KeyValueResult<Arc<ShardData>> {
        let shards = self.shards.read().unwrap();
        shards
            .get(&shard)
            .cloned()
            .ok_or(KeyValueError::KeyNotFound)
    }

    /// Get the tree for a shard
    fn get_tree(&self, shard: ShardId) -> KeyValueResult<Arc<RwLock<AdaptiveRadixTree<Vec<u8>>>>> {
        let shard_data = self.get_shard(shard)?;
        Ok(shard_data.tree.clone())
    }

    /// Get the metrics for a shard
    fn get_metrics(&self, shard: ShardId) -> Arc<ArtMetrics> {
        let mut metrics = self.metrics.write().unwrap();
        metrics
            .entry(shard)
            .or_insert_with(|| Arc::new(ArtMetrics::new()))
            .clone()
    }

    /// Write a WAL record for a put operation
    fn wal_write_put(&self, shard: ShardId, key: &[u8], value: &[u8]) -> KeyValueResult<()> {
        if !self.wal_enabled {
            return Ok(());
        }

        let shard_data = self.get_shard(shard)?;
        if let Some(wal_writer) = &shard_data.wal_writer {
            let record_data = crate::wal_record::encode_put(key, value);
            let record = WriteAheadLogRecord {
                kind: WalRecordKind::Put as u16,
                payload: &record_data,
            };
            let mut writer = wal_writer.lock().unwrap();
            writer
                .append(record, nanograph_wal::Durability::Flush)
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
            let record_data = crate::wal_record::encode_delete(key);
            let record = WriteAheadLogRecord {
                kind: WalRecordKind::Delete as u16,
                payload: &record_data,
            };
            let mut writer = wal_writer.lock().unwrap();
            writer
                .append(record, nanograph_wal::Durability::Flush)
                .map_err(|e| {
                    KeyValueError::StorageCorruption(format!("WAL write failed: {}", e))
                })?;
        }
        Ok(())
    }

    /// Recover a shard from WAL by replaying all records
    fn recover_from_wal(
        &self,
        shard: ShardId,
        tree: &Arc<RwLock<AdaptiveRadixTree<Vec<u8>>>>,
    ) -> KeyValueResult<()> {
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
            let mut tree_guard = tree.write().unwrap();

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
                        tree_guard
                            .insert(key, value)
                            .map_err(|e| KeyValueError::StorageCorruption(e.to_string()))?;
                        recovered_count += 1;
                    }
                    Some(WalRecordKind::Delete) => {
                        let key =
                            crate::wal_record::decode_delete(&entry.payload).map_err(|e| {
                                KeyValueError::StorageCorruption(format!(
                                    "Failed to decode delete: {}",
                                    e
                                ))
                            })?;
                        let _ = tree_guard.remove(&key);
                        recovered_count += 1;
                    }
                    Some(WalRecordKind::Checkpoint) => {
                        // Checkpoint marker - we can stop here if we want
                        // For now, continue replaying
                    }
                    None => {
                        // Unknown record type - skip it
                        continue;
                    }
                }
            }

            drop(tree_guard);

            if recovered_count > 0 {
                // Log recovery success (using println for now since tracing isn't available)
                eprintln!(
                    "INFO: Recovered {} operations from WAL for shard {:?}",
                    recovered_count, shard
                );
            }
        }

        Ok(())
    }

    /// Create a checkpoint for a shard
    /// This saves the current tree state to disk and writes a checkpoint marker to the WAL
    pub async fn checkpoint_shard(&self, shard: ShardId) -> KeyValueResult<()> {
        let shard_data = self.get_shard(shard)?;

        // Save tree to disk using persistence
        // Note: This requires VFS integration which we'll add later
        // For now, just write a checkpoint marker to WAL

        if self.wal_enabled {
            if let Some(wal_writer) = &shard_data.wal_writer {
                let checkpoint_data = crate::wal_record::encode_checkpoint();
                let record = nanograph_wal::WriteAheadLogRecord {
                    kind: crate::wal_record::WalRecordKind::Checkpoint as u16,
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

impl Default for ArtKeyValueStore {
    /// Create a default ART key-value store (no WAL)
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl KeyValueShardStore for ArtKeyValueStore {
    // ===== Basic Operations =====

    /// Get a value from a shard
    async fn get(&self, shard: ShardId, key: &[u8]) -> KeyValueResult<Option<Vec<u8>>> {
        let tree = self.get_tree(shard)?;
        let metrics = self.get_metrics(shard);

        let tree_guard = tree.read().unwrap();
        let result = tree_guard.get(key);

        metrics.record_read(result.is_some());

        Ok(result)
    }

    /// Put a key-value pair into a shard
    async fn put(&self, shard: ShardId, key: &[u8], value: &[u8]) -> KeyValueResult<()> {
        // Write to WAL first (write-ahead logging)
        self.wal_write_put(shard, key, value)?;

        let tree = self.get_tree(shard)?;
        let metrics = self.get_metrics(shard);

        {
            let mut tree_guard = tree.write().unwrap();

            // Check if key exists (for update tracking)
            let exists = tree_guard.get(key).is_some();

            tree_guard
                .insert(key.to_vec(), value.to_vec())
                .map_err(|e| KeyValueError::StorageCorruption(e.to_string()))?;

            metrics.record_write(exists);
        } // tree_guard dropped here

        Ok(())
    }

    /// Delete a key from a shard
    async fn delete(&self, shard: ShardId, key: &[u8]) -> KeyValueResult<bool> {
        // Write to WAL first (write-ahead logging)
        self.wal_write_delete(shard, key)?;

        let tree = self.get_tree(shard)?;
        let metrics = self.get_metrics(shard);

        let deleted = {
            let mut tree_guard = tree.write().unwrap();
            let deleted = tree_guard
                .remove(key)
                .map_err(|e| KeyValueError::StorageCorruption(e.to_string()))?
                .is_some();

            if deleted {
                metrics.record_delete();
            }

            deleted
        }; // tree_guard dropped here

        Ok(deleted)
    }

    /// Check if a key exists in a shard
    async fn exists(&self, shard: ShardId, key: &[u8]) -> KeyValueResult<bool> {
        let tree = self.get_tree(shard)?;
        let tree_guard = tree.read().unwrap();
        Ok(tree_guard.get(key).is_some())
    }

    // ===== Batch Operations =====

    /// Get multiple values from a shard
    async fn batch_get(
        &self,
        shard: ShardId,
        keys: &[&[u8]],
    ) -> KeyValueResult<Vec<Option<Vec<u8>>>> {
        let tree = self.get_tree(shard)?;
        let metrics = self.get_metrics(shard);

        let tree_guard = tree.read().unwrap();
        let mut results = Vec::with_capacity(keys.len());

        for key in keys {
            let result = tree_guard.get(key);
            metrics.record_read(result.is_some());
            results.push(result);
        }

        Ok(results)
    }

    /// Put multiple key-value pairs into a shard
    async fn batch_put(&self, shard: ShardId, pairs: &[(&[u8], &[u8])]) -> KeyValueResult<()> {
        // Write all operations to WAL first
        for (key, value) in pairs {
            self.wal_write_put(shard, key, value)?;
        }

        let tree = self.get_tree(shard)?;
        let metrics = self.get_metrics(shard);

        {
            let mut tree_guard = tree.write().unwrap();

            for (key, value) in pairs {
                let exists = tree_guard.get(key).is_some();
                tree_guard
                    .insert(key.to_vec(), value.to_vec())
                    .map_err(|e| KeyValueError::StorageCorruption(e.to_string()))?;
                metrics.record_write(exists);
            }
        } // tree_guard dropped here

        Ok(())
    }

    /// Delete multiple keys from a shard
    async fn batch_delete(&self, shard: ShardId, keys: &[&[u8]]) -> KeyValueResult<usize> {
        // Write all operations to WAL first
        for key in keys {
            self.wal_write_delete(shard, key)?;
        }

        let tree = self.get_tree(shard)?;
        let metrics = self.get_metrics(shard);

        let count = {
            let mut tree_guard = tree.write().unwrap();
            let mut count = 0;

            for key in keys {
                if tree_guard
                    .remove(key)
                    .map_err(|e| KeyValueError::StorageCorruption(e.to_string()))?
                    .is_some()
                {
                    count += 1;
                    metrics.record_delete();
                }
            }

            count
        }; // tree_guard dropped here

        Ok(count)
    }

    // ===== Range Operations =====

    /// Scan a range of keys in a shard
    async fn scan(
        &self,
        shard: ShardId,
        range: KeyRange,
    ) -> KeyValueResult<Box<dyn KeyValueIterator + Send>> {
        let tree = self.get_tree(shard)?;
        let metrics = self.get_metrics(shard);

        metrics.record_scan(0); // Will be updated as iteration progresses

        let tree_guard = tree.read().unwrap();

        // Create an ART iterator wrapper that implements KeyValueIterator
        let iterator = ArtKeyValueIterator::new(
            tree_guard.clone(),
            range.start,
            range.end,
            range.reverse,
            range.limit,
        );

        Ok(Box::new(iterator) as Box<dyn KeyValueIterator + Send>)
    }

    // ===== Statistics & Metadata =====

    /// Get the number of keys in a shard
    async fn key_count(&self, shard: ShardId) -> KeyValueResult<u64> {
        let tree = self.get_tree(shard)?;
        let tree_guard = tree.read().unwrap();
        Ok(tree_guard.len() as u64)
    }

    /// Get statistics for a shard
    async fn shard_stats(&self, shard: ShardId) -> KeyValueResult<ShardStats> {
        let tree = self.get_tree(shard)?;
        let tree_guard = tree.read().unwrap();
        let metrics = self.get_metrics(shard);
        let metrics_snapshot = metrics.snapshot();

        // Estimate bytes (rough calculation)
        let key_count = tree_guard.len() as u64;
        let avg_key_size = 32; // Assume average key size
        let avg_value_size = 128; // Assume average value size
        let data_bytes = key_count * (avg_key_size + avg_value_size);

        // Estimate index overhead (ART nodes)
        // ART is more memory efficient than B+Tree, but still has overhead
        let index_bytes = key_count * 64; // Rough estimate for node overhead

        let mut shard_stats = ShardStats {
            key_count,
            total_bytes: data_bytes + index_bytes,
            data_bytes,
            index_bytes,
            last_modified: None,
            engine_stats: Default::default(),
        };

        // Build ART specific stats
        shard_stats.engine_stats.insert(
            "total_reads",
            StatValue::from_u64(metrics_snapshot.total_reads),
        );
        shard_stats.engine_stats.insert(
            "total_writes",
            StatValue::from_u64(metrics_snapshot.total_writes),
        );
        shard_stats.engine_stats.insert(
            "total_deletes",
            StatValue::from_u64(metrics_snapshot.total_deletes),
        );
        shard_stats.engine_stats.insert(
            "cache_hits",
            StatValue::from_u64(metrics_snapshot.cache_hits),
        );
        shard_stats.engine_stats.insert(
            "cache_misses",
            StatValue::from_u64(metrics_snapshot.cache_misses),
        );
        shard_stats.engine_stats.insert(
            "hit_rate",
            StatValue::from_f64(if metrics_snapshot.total_reads > 0 {
                metrics_snapshot.cache_hits as f64 / metrics_snapshot.total_reads as f64
            } else {
                0.0
            }),
        );

        Ok(shard_stats)
    }

    // ===== Transaction Support =====

    /// Start a new transaction
    async fn begin_transaction(&self) -> KeyValueResult<Arc<dyn Transaction>> {
        // Get or create transaction manager
        let tx_mgr = self.get_tx_manager();
        Ok(tx_mgr.begin())
    }

    // ===== Shard Management =====

    /// Create a new shard
    async fn create_shard(&self, table: TableId, index: ShardIndex) -> KeyValueResult<ShardId> {
        // Compute deterministic ShardId from TableId and ShardIndex
        let shard_id = ShardId::from_parts(table, index);

        // Create a new ART for this shard
        let tree = Arc::new(RwLock::new(AdaptiveRadixTree::new()));

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
            metrics.insert(shard_id, Arc::new(ArtMetrics::new()));
        }

        // Recover from WAL if it exists
        self.recover_from_wal(shard_id, &tree)?;

        Ok(shard_id)
    }

    /// Drop a shard
    async fn drop_shard(&self, shard: ShardId) -> KeyValueResult<()> {
        let mut shards = self.shards.write().unwrap();
        shards.remove(&shard);

        let mut metrics = self.metrics.write().unwrap();
        metrics.remove(&shard);

        Ok(())
    }

    /// List all shards in the store
    async fn list_shards(&self) -> KeyValueResult<Vec<ShardId>> {
        let shards = self.shards.read().unwrap();
        Ok(shards.keys().copied().collect())
    }

    /// Check if a shard exists in the store
    async fn shard_exists(&self, shard: ShardId) -> KeyValueResult<bool> {
        let shards = self.shards.read().unwrap();
        Ok(shards.contains_key(&shard))
    }

    // ===== Maintenance Operations =====

    /// Flush all shards to disk
    async fn flush(&self) -> KeyValueResult<()> {
        // ART is in-memory, so flush is a no-op for now
        // In a persistent implementation, this would write to disk
        Ok(())
    }

    /// Compact one or all shards
    async fn compact(&self, _shard: Option<ShardId>) -> KeyValueResult<()> {
        // ART doesn't need compaction like LSM trees
        // This is a no-op
        Ok(())
    }

    /// Scan keys with a given prefix
    async fn scan_prefix(
        &self,
        shard: ShardId,
        prefix: &[u8],
        limit: Option<usize>,
    ) -> KeyValueResult<Box<dyn KeyValueIterator + Send>> {
        let tree = self.get_tree(shard)?;
        let tree_guard = tree.read().unwrap();

        let start = Bound::Included(prefix.to_vec());
        // Simple prefix end bound: increment last byte
        let mut end_vec = prefix.to_vec();
        let end = if let Some(last) = end_vec.last_mut() {
            *last += 1;
            Bound::Excluded(end_vec)
        } else {
            Bound::Unbounded
        };

        let iterator = ArtKeyValueIterator::new(tree_guard.clone(), start, end, false, limit);

        Ok(Box::new(iterator) as Box<dyn KeyValueIterator + Send>)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_shard_management() {
        let store = ArtKeyValueStore::default();

        // Create shard
        let table_id = TableId::new(0);
        let shard_index = ShardIndex::new(0);
        let shard = store.create_shard(table_id, shard_index).await.unwrap();

        assert!(store.shard_exists(shard).await.unwrap());

        // List shards
        let shards = store.list_shards().await.unwrap();
        assert_eq!(shards.len(), 1);

        // Drop shard
        store.drop_shard(shard).await.unwrap();
        assert!(!store.shard_exists(shard).await.unwrap());
    }

    #[tokio::test]
    async fn test_basic_operations() {
        let store = ArtKeyValueStore::default();

        let table_id = TableId::new(0);
        let shard_index = ShardIndex::new(0);
        let shard = store.create_shard(table_id, shard_index).await.unwrap();

        // Put
        store.put(shard, b"key1", b"value1").await.unwrap();
        store.put(shard, b"key2", b"value2").await.unwrap();

        // Get
        assert_eq!(
            store.get(shard, b"key1").await.unwrap(),
            Some(b"value1".to_vec())
        );
        assert_eq!(
            store.get(shard, b"key2").await.unwrap(),
            Some(b"value2".to_vec())
        );
        assert_eq!(store.get(shard, b"key3").await.unwrap(), None);

        // Exists
        assert!(store.exists(shard, b"key1").await.unwrap());
        assert!(!store.exists(shard, b"key3").await.unwrap());

        // Delete
        assert!(store.delete(shard, b"key1").await.unwrap());
        assert!(!store.delete(shard, b"key3").await.unwrap());
        assert_eq!(store.get(shard, b"key1").await.unwrap(), None);
    }

    #[tokio::test]
    async fn test_batch_operations() {
        let store = ArtKeyValueStore::default();

        let table_id = TableId::new(0);
        let shard_index = ShardIndex::new(0);
        let shard = store.create_shard(table_id, shard_index).await.unwrap();

        // Batch put
        let pairs = vec![
            (&b"key1"[..], &b"value1"[..]),
            (&b"key2"[..], &b"value2"[..]),
            (&b"key3"[..], &b"value3"[..]),
        ];
        store.batch_put(shard, &pairs).await.unwrap();

        // Batch get
        let keys = vec![&b"key1"[..], &b"key2"[..], &b"key3"[..], &b"key4"[..]];
        let results = store.batch_get(shard, &keys).await.unwrap();
        assert_eq!(results[0], Some(b"value1".to_vec()));
        assert_eq!(results[1], Some(b"value2".to_vec()));
        assert_eq!(results[2], Some(b"value3".to_vec()));
        assert_eq!(results[3], None);

        // Batch delete
        let delete_keys = vec![&b"key1"[..], &b"key2"[..], &b"key4"[..]];
        let deleted = store.batch_delete(shard, &delete_keys).await.unwrap();
        assert_eq!(deleted, 2);
    }

    #[tokio::test]
    async fn test_statistics() {
        let store = ArtKeyValueStore::default();
        let table_id = TableId::new(0);
        let shard_index = ShardIndex::new(0);
        let shard = store.create_shard(table_id, shard_index).await.unwrap();

        // Insert data
        for i in 0..100 {
            let key = format!("key{:03}", i);
            let value = format!("value{}", i);
            store
                .put(shard, key.as_bytes(), value.as_bytes())
                .await
                .unwrap();
        }

        // Check stats
        let count = store.key_count(shard).await.unwrap();
        assert_eq!(count, 100);

        let stats = store.shard_stats(shard).await.unwrap();
        assert_eq!(stats.key_count, 100);
        assert!(stats.total_bytes > 0);
    }

    #[tokio::test]
    async fn test_prefix_scan() {
        let store = ArtKeyValueStore::default();
        let table_id = TableId::new(0);
        let shard_index = ShardIndex::new(0);
        let shard = store.create_shard(table_id, shard_index).await.unwrap();

        // Insert data with common prefix
        store.put(shard, b"user:1:name", b"Alice").await.unwrap();
        store
            .put(shard, b"user:1:email", b"alice@example.com")
            .await
            .unwrap();
        store.put(shard, b"user:2:name", b"Bob").await.unwrap();
        store
            .put(shard, b"user:2:email", b"bob@example.com")
            .await
            .unwrap();

        // Scan with prefix - simplified test
        let _iter = store.scan_prefix(shard, b"user:1:", None).await.unwrap();

        // TODO: Implement proper async iteration test
        // For now, just verify the iterator was created successfully
    }
}
