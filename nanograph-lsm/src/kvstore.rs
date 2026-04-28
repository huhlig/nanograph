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

use crate::config::LSMStorageConfig;
use crate::engine::LSMTreeEngine;
use crate::iterator::LSMIterator;
use crate::options::LSMTreeOptions;
use crate::transaction::TransactionManager;
use async_trait::async_trait;
use nanograph_kvt::metrics::{ShardStats, StatValue};
use nanograph_kvt::{
    KeyRange, KeyValueError, KeyValueIterator, KeyValueResult, KeyValueShardStore, ShardId,
    Transaction,
};
use nanograph_vfs::{DynamicFileSystem, MemoryFileSystem, Path};
use nanograph_wal::{WriteAheadLogConfig, WriteAheadLogManager};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

/// LSM Tree implementation of KeyValueStore
///
/// This is a low-level storage engine that manages physical shards.
/// It does NOT manage table names or allocate IDs - that's the responsibility
/// of KeyValueDatabaseManager at a higher level.
pub struct LSMKeyValueStore {
    /// Storage engines for each shard
    engines: Arc<RwLock<HashMap<ShardId, Arc<LSMTreeEngine>>>>,

    /// Transaction manager
    tx_manager: RwLock<Option<Arc<TransactionManager>>>,
}

impl LSMKeyValueStore {
    pub fn new() -> Self {
        Self {
            engines: Arc::new(RwLock::new(HashMap::new())),
            tx_manager: RwLock::new(None),
        }
    }

    /// Initialize the transaction manager (must be called after store is wrapped in Arc)
    pub fn init_tx_manager(self: &Arc<Self>) {
        let tx_manager = Arc::new(TransactionManager::new(Arc::clone(self)));
        *self.tx_manager.write().unwrap() = Some(tx_manager);
    }

    pub fn get_tx_manager(&self) -> Arc<TransactionManager> {
        self.tx_manager.read().unwrap().as_ref().unwrap().clone()
    }

    fn get_engine(&self, shard: ShardId) -> KeyValueResult<Arc<LSMTreeEngine>> {
        let engines = self.engines.read().unwrap();
        engines
            .get(&shard)
            .cloned()
            .ok_or(KeyValueError::ShardNotFound(shard))
    }

    fn create_engine_for_table(&self, shard: ShardId) -> KeyValueResult<Arc<LSMTreeEngine>> {
        // ShardId is already provided - no allocation needed!
        // The shard_id parameter IS the unique identifier for this shard

        // Create memory filesystems for WAL and SSTables
        let wal_fs = MemoryFileSystem::new();
        let sstable_fs: Arc<dyn nanograph_vfs::DynamicFileSystem> =
            Arc::new(MemoryFileSystem::new());
        let wal_path_str = format!("/wal_{}", shard.0);
        let wal_path = Path::from(wal_path_str.as_str());

        // Create WAL manager - use the ShardId directly
        let wal_config = WriteAheadLogConfig::new(shard.0);
        let wal = WriteAheadLogManager::new(wal_fs, wal_path, wal_config)
            .map_err(|e| nanograph_kvt::KeyValueError::StorageCorruption(e.to_string()))?;

        // Create LSM options with the ShardId
        let options = LSMTreeOptions::default().with_shard_id(shard.0);

        // Create base path for this shard (in VFS)
        let base_path = format!("/lsm/{}", shard.0);

        // Create engine with VFS
        let engine = LSMTreeEngine::new(sstable_fs, base_path, options, wal)?;

        Ok(Arc::new(engine))
    }

    /// Create an engine with VFS and tablespace-resolved configuration
    /// This is the new tablespace-aware method that will be used by the shard manager
    pub fn create_engine_with_config(
        &self,
        shard: ShardId,
        vfs: Arc<dyn DynamicFileSystem>,
        config: LSMStorageConfig,
    ) -> KeyValueResult<Arc<LSMTreeEngine>> {
        // Ensure directories exist
        vfs.create_directory_all(&config.data_dir)
            .map_err(|e| KeyValueError::StorageCorruption(e.to_string()))?;
        vfs.create_directory_all(&config.wal_dir)
            .map_err(|e| KeyValueError::StorageCorruption(e.to_string()))?;

        // Create WAL filesystem (can be same as data or separate)
        let wal_fs = vfs.clone();
        let wal_path = Path::from(config.wal_dir.as_str());

        // Create WAL manager with shard ID
        let wal_config = WriteAheadLogConfig::new(shard.0);
        let wal = WriteAheadLogManager::new(wal_fs, wal_path, wal_config)
            .map_err(|e| KeyValueError::StorageCorruption(e.to_string()))?;

        // Create LSM options with shard ID
        let mut options = config.options.clone();
        options.shard_id = shard.0;

        // Create engine with VFS and resolved paths
        let engine = LSMTreeEngine::new(vfs, config.data_dir.clone(), options, wal)?;

        Ok(Arc::new(engine))
    }

    /// Get a value at a specific snapshot timestamp (for MVCC)
    pub async fn get_at_snapshot(
        &self,
        shard: ShardId,
        key: &[u8],
        snapshot_ts: i64,
    ) -> KeyValueResult<Option<Vec<u8>>> {
        let engine = self.get_engine(shard)?;
        engine.get_at_snapshot(key, snapshot_ts)
    }

    /// Mark an entry as committed with the given timestamp (for MVCC)
    pub async fn commit_entry(
        &self,
        shard: ShardId,
        key: &[u8],
        commit_ts: i64,
    ) -> KeyValueResult<()> {
        let engine = self.get_engine(shard)?;
        engine.commit_entry(key, commit_ts)
    }

    /// Put with commit timestamp (for MVCC transactions)
    pub async fn put_committed(
        &self,
        shard: ShardId,
        key: &[u8],
        value: &[u8],
        commit_ts: i64,
    ) -> KeyValueResult<()> {
        let engine = self.get_engine(shard)?;
        engine.put_committed(key.to_vec(), value.to_vec(), commit_ts)
    }

    /// Delete with commit timestamp (for MVCC transactions)
    pub async fn delete_committed(
        &self,
        shard: ShardId,
        key: &[u8],
        commit_ts: i64,
    ) -> KeyValueResult<()> {
        let engine = self.get_engine(shard)?;
        engine.delete_committed(key.to_vec(), commit_ts)
    }

    /// Create a checkpoint for a shard
    /// This saves the current LSM tree state and writes a checkpoint marker to the WAL
    pub async fn checkpoint_shard(&self, shard: ShardId) -> KeyValueResult<()> {
        let engine = self.get_engine(shard)?;
        engine.checkpoint()
    }

    /// Create checkpoints for all shards
    pub async fn checkpoint_all(&self) -> KeyValueResult<()> {
        let shard_ids: Vec<ShardId> = {
            let engines = self.engines.read().unwrap();
            engines.keys().copied().collect()
        };

        for shard_id in shard_ids {
            self.checkpoint_shard(shard_id).await?;
        }

        Ok(())
    }
}

impl Default for LSMKeyValueStore {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl KeyValueShardStore for LSMKeyValueStore {
    // ===== Basic Operations =====

    async fn get(&self, shard: ShardId, key: &[u8]) -> KeyValueResult<Option<Vec<u8>>> {
        let engine = self.get_engine(shard)?;
        engine.get(key)
    }

    async fn put(&self, shard: ShardId, key: &[u8], value: &[u8]) -> KeyValueResult<()> {
        let engine = self.get_engine(shard)?;
        engine.put(key.to_vec(), value.to_vec())
    }

    async fn delete(&self, shard: ShardId, key: &[u8]) -> KeyValueResult<bool> {
        let engine = self.get_engine(shard)?;
        engine.delete(key.to_vec())?;
        // TODO: Return true if key existed, false otherwise
        Ok(true)
    }

    async fn exists(&self, shard: ShardId, key: &[u8]) -> KeyValueResult<bool> {
        let engine = self.get_engine(shard)?;
        Ok(engine.get(key)?.is_some())
    }

    // ===== Batch Operations =====

    async fn batch_get(
        &self,
        shard: ShardId,
        keys: &[&[u8]],
    ) -> KeyValueResult<Vec<Option<Vec<u8>>>> {
        let engine = self.get_engine(shard)?;
        let mut results = Vec::with_capacity(keys.len());
        for key in keys {
            results.push(engine.get(key)?);
        }
        Ok(results)
    }

    async fn batch_put(&self, shard: ShardId, pairs: &[(&[u8], &[u8])]) -> KeyValueResult<()> {
        let engine = self.get_engine(shard)?;
        for (key, value) in pairs {
            engine.put(key.to_vec(), value.to_vec())?;
        }
        Ok(())
    }

    async fn batch_delete(&self, shard: ShardId, keys: &[&[u8]]) -> KeyValueResult<usize> {
        let engine = self.get_engine(shard)?;
        let mut count = 0;
        for key in keys {
            engine.delete(key.to_vec())?;
            count += 1;
        }
        Ok(count)
    }

    // ===== Range Operations =====

    async fn scan(
        &self,
        shard: ShardId,
        range: KeyRange,
    ) -> KeyValueResult<Box<dyn KeyValueIterator + Send>> {
        let engine = self.get_engine(shard)?;

        // Collect entries from all sources for merging
        let mut sources = Vec::new();

        // 1. Get entries from active memtable (highest priority)
        {
            let memtable = engine.memtable.read().unwrap();
            let mut memtable_entries = memtable.entries();
            memtable_entries.retain(|entry| {
                let key = &entry.key;
                let after_start = match &range.start {
                    std::ops::Bound::Included(start) => key >= start,
                    std::ops::Bound::Excluded(start) => key > start,
                    std::ops::Bound::Unbounded => true,
                };
                let before_end = match &range.end {
                    std::ops::Bound::Included(end) => key <= end,
                    std::ops::Bound::Excluded(end) => key < end,
                    std::ops::Bound::Unbounded => true,
                };
                after_start && before_end
            });
            sources.push(memtable_entries);
        }

        // 2. Get entries from immutable memtable if it exists
        {
            let immutable = engine.immutable_memtable.read().unwrap();
            if let Some(ref imm_memtable) = *immutable {
                let mut imm_entries = imm_memtable.entries();
                imm_entries.retain(|entry| {
                    let key = &entry.key;
                    let after_start = match &range.start {
                        std::ops::Bound::Included(start) => key >= start,
                        std::ops::Bound::Excluded(start) => key > start,
                        std::ops::Bound::Unbounded => true,
                    };
                    let before_end = match &range.end {
                        std::ops::Bound::Included(end) => key <= end,
                        std::ops::Bound::Excluded(end) => key < end,
                        std::ops::Bound::Unbounded => true,
                    };
                    after_start && before_end
                });
                sources.push(imm_entries);
            }
        }

        // 3. SSTables would be added here in a complete implementation
        // For now, we have memtable + immutable memtable coverage

        // Merge all sources with proper priority ordering
        let mut iterator = LSMIterator::merge(sources, range.reverse);

        // Apply limit if specified
        if let Some(limit) = range.limit {
            iterator.set_limit(limit);
        }

        Ok(Box::new(iterator))
    }

    // ===== Statistics & Metadata =====

    async fn key_count(&self, shard: ShardId) -> KeyValueResult<u64> {
        let engine = self.get_engine(shard)?;
        let stats = engine.stats();

        // Sum up entries across all levels
        let mut count = 0u64;
        for level_stats in &stats.levels {
            // This is approximate - would need to read SSTables for exact count
            count += level_stats.num_sstables as u64 * 1000; // Rough estimate
        }

        // Add memtable entries
        count += engine.memtable.read().unwrap().entry_count() as u64;

        Ok(count)
    }

    async fn shard_stats(&self, shard: ShardId) -> KeyValueResult<ShardStats> {
        let engine = self.get_engine(shard)?;
        let stats = engine.stats();

        // Calculate total bytes
        let mut total_bytes = stats.memtable_size as u64 + stats.immutable_memtable_size as u64;
        let mut data_bytes = total_bytes;

        for level_stats in &stats.levels {
            total_bytes += level_stats.total_size;
            data_bytes += level_stats.total_size;
        }

        let mut shard_stats = ShardStats {
            key_count: self.key_count(shard).await?,
            total_bytes,
            data_bytes,
            index_bytes: 0,      // TODO: Calculate index overhead
            last_modified: None, // TODO: Track modification time
            engine_stats: Default::default(),
        };

        // Build LSM-specific stats
        shard_stats
            .engine_stats
            .insert("num_levels", StatValue::from_u64(stats.levels.len() as u64));
        shard_stats.engine_stats.insert(
            "sstables_per_level",
            StatValue::from_list(
                stats
                    .levels
                    .iter()
                    .map(|l| StatValue::from_usize(l.num_sstables)),
            ),
        );
        shard_stats.engine_stats.insert(
            "bytes_per_level",
            StatValue::from_list(
                stats
                    .levels
                    .iter()
                    .map(|l| StatValue::from_u64(l.total_size)),
            ),
        );
        shard_stats
            .engine_stats
            .insert("memtable_bytes", StatValue::from_usize(stats.memtable_size));
        shard_stats
            .engine_stats // TODO: Track this
            .insert("pending_compactions", StatValue::from_usize(0));
        shard_stats.engine_stats.insert(
            "total_compactions",
            StatValue::from_u64(stats.total_compactions),
        );
        shard_stats
            .engine_stats // TODO: Calculate from metrics
            .insert("write_amplification", StatValue::from_f64(0.0));
        shard_stats
            .engine_stats // TODO: Calculate from metrics
            .insert("read_amplification", StatValue::from_f64(0.0));
        shard_stats
            .engine_stats // TODO: Get from metrics
            .insert("bloom_filter_false_positives", StatValue::from_f64(0.0));

        Ok(shard_stats)
    }

    // ===== Transaction Support =====

    async fn begin_transaction(&self) -> KeyValueResult<Arc<dyn Transaction>> {
        Ok(self.get_tx_manager().begin())
    }

    // ===== Shard Management =====

    fn create_shard(
        &self,
        shard_id: ShardId,
        _vfs: Arc<dyn nanograph_vfs::DynamicFileSystem>,
        _data_path: nanograph_vfs::Path,
        _wal_path: nanograph_vfs::Path,
    ) -> KeyValueResult<()> {
        // LSM is an in-memory store for now, so we ignore the tablespace paths
        // In a real implementation, you would use these paths for SSTable storage
        
        // Create LSMTreeEngine for this shard
        let engine = self.create_engine_for_table(shard_id)?;

        // Store engine
        let mut engines = self.engines.write().unwrap();
        engines.insert(shard_id, engine);

        Ok(())
    }

    async fn drop_shard(&self, shard: ShardId) -> KeyValueResult<()> {
        let mut engines = self.engines.write().unwrap();
        engines.remove(&shard);

        Ok(())
    }

    async fn clear(&self, shard: ShardId) -> KeyValueResult<()> {
        let engine = self.get_engine(shard)?;
        engine.clear()
    }

    async fn list_shards(&self) -> KeyValueResult<Vec<ShardId>> {
        let engines = self.engines.read().unwrap();
        Ok(engines.keys().copied().collect())
    }

    async fn shard_exists(&self, shard: ShardId) -> KeyValueResult<bool> {
        let engines = self.engines.read().unwrap();
        Ok(engines.contains_key(&shard))
    }

    // ===== Maintenance Operations =====

    async fn flush(&self) -> KeyValueResult<()> {
        let engines = self.engines.read().unwrap();
        for engine in engines.values() {
            engine.flush()?;
        }
        Ok(())
    }

    async fn compact(&self, table: Option<ShardId>) -> KeyValueResult<()> {
        if let Some(table_id) = table {
            let engine = self.get_engine(table_id)?;
            engine.compact()?;
        } else {
            // Compact all tables
            let engines = self.engines.read().unwrap();
            for engine in engines.values() {
                engine.compact()?;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_table_management() {
        let store = LSMKeyValueStore::new();

        // Create Shard
        let shard_id = ShardId::new(0);
        let vfs = Arc::new(nanograph_vfs::MemoryFileSystem::new());
        let data_path = nanograph_vfs::Path::from("/data");
        let wal_path = nanograph_vfs::Path::from("/wal");
        store.create_shard(shard_id, vfs, data_path, wal_path).unwrap();

        assert!(store.shard_exists(shard_id).await.unwrap());

        // List tables
        let tables = store.list_shards().await.unwrap();
        assert_eq!(tables.len(), 1);

        // Drop table
        store.drop_shard(shard_id).await.unwrap();
        assert!(!store.shard_exists(shard_id).await.unwrap());
    }

    #[tokio::test]
    async fn test_clear_operation() {
        let store = LSMKeyValueStore::new();

        let shard_id = ShardId::new(1);
        let vfs = Arc::new(nanograph_vfs::MemoryFileSystem::new());
        let data_path = nanograph_vfs::Path::from("/data");
        let wal_path = nanograph_vfs::Path::from("/wal");
        store.create_shard(shard_id, vfs, data_path, wal_path).unwrap();

        // Put some data
        store.put(shard_id, b"key1", b"value1").await.unwrap();
        assert_eq!(
            store.get(shard_id, b"key1").await.unwrap(),
            Some(b"value1".to_vec())
        );

        // Clear
        store.clear(shard_id).await.unwrap();

        // Verify cleared
        assert_eq!(store.get(shard_id, b"key1").await.unwrap(), None);
    }
}
