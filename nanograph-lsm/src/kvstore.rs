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

use crate::engine::LSMTreeEngine;
use crate::iterator::LSMIterator;
use crate::options::LSMTreeOptions;
use crate::transaction::TransactionManager;
use async_trait::async_trait;
use nanograph_kvt::{
    EngineStats as KvEngineStats, KeyRange, KeyValueIterator, KeyValueResult, KeyValueStore,
    KeyValueTableId, LsmStats, TableStats, Transaction,
};
use nanograph_vfs::{MemoryFileSystem, Path};
use nanograph_wal::{WriteAheadLogConfig, WriteAheadLogManager};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

/// LSM Tree implementation of KeyValueStore
pub struct LSMKeyValueStore {
    engines: Arc<RwLock<HashMap<KeyValueTableId, Arc<LSMTreeEngine>>>>,
    table_names: Arc<RwLock<HashMap<KeyValueTableId, String>>>,
    next_table_id: Arc<RwLock<u128>>,
    next_shard_id: Arc<RwLock<u64>>,
    tx_manager: RwLock<Option<Arc<TransactionManager>>>,
}

impl LSMKeyValueStore {
    pub fn new() -> Self {
        // Create store components
        let engines = Arc::new(RwLock::new(HashMap::new()));
        let table_names = Arc::new(RwLock::new(HashMap::new()));
        let next_table_id = Arc::new(RwLock::new(1));
        let next_shard_id = Arc::new(RwLock::new(0));

        Self {
            engines,
            table_names,
            next_table_id,
            next_shard_id,
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

    fn get_engine(&self, table: KeyValueTableId) -> KeyValueResult<Arc<LSMTreeEngine>> {
        let engines = self.engines.read().unwrap();
        engines
            .get(&table)
            .cloned()
            .ok_or(nanograph_kvt::KeyValueError::KeyNotFound)
    }
    
    fn create_engine_for_table(&self, table: KeyValueTableId) -> KeyValueResult<Arc<LSMTreeEngine>> {
        // Get next shard ID
        let shard_id = {
            let mut next_id = self.next_shard_id.write().unwrap();
            let id = *next_id;
            *next_id += 1;
            id
        };
        
        // Create memory filesystems for WAL and SSTables
        let wal_fs = MemoryFileSystem::new();
        let sstable_fs: Arc<dyn nanograph_vfs::DynamicFileSystem> = Arc::new(MemoryFileSystem::new());
        let wal_path_str = format!("/wal_{}", table.0);
        let wal_path = Path::from(wal_path_str.as_str());
        
        // Create WAL manager
        let wal_config = WriteAheadLogConfig::new(shard_id);
        let wal = WriteAheadLogManager::new(wal_fs, wal_path, wal_config)
            .map_err(|e| nanograph_kvt::KeyValueError::StorageCorruption(e.to_string()))?;
        
        // Create LSM options with matching shard_id
        let options = LSMTreeOptions::default().with_shard_id(shard_id);
        
        // Create base path for this table (in VFS)
        let base_path = format!("/lsm/{}", table.0);
        
        // Create engine with VFS
        let engine = LSMTreeEngine::new(sstable_fs, base_path, options, wal)?;
        
        Ok(Arc::new(engine))
    }
    
    /// Get a value at a specific snapshot timestamp (for MVCC)
    pub async fn get_at_snapshot(
        &self,
        table: KeyValueTableId,
        key: &[u8],
        snapshot_ts: u64,
    ) -> KeyValueResult<Option<Vec<u8>>> {
        let engine = self.get_engine(table)?;
        engine.get_at_snapshot(key, snapshot_ts)
    }
    
    /// Mark an entry as committed with the given timestamp (for MVCC)
    pub async fn commit_entry(
        &self,
        table: KeyValueTableId,
        key: &[u8],
        commit_ts: u64,
    ) -> KeyValueResult<()> {
        let engine = self.get_engine(table)?;
        engine.commit_entry(key, commit_ts)
    }
    
    /// Put with commit timestamp (for MVCC transactions)
    pub async fn put_committed(
        &self,
        table: KeyValueTableId,
        key: &[u8],
        value: &[u8],
        commit_ts: u64,
    ) -> KeyValueResult<()> {
        let engine = self.get_engine(table)?;
        engine.put_committed(key.to_vec(), value.to_vec(), commit_ts)
    }
    
    /// Delete with commit timestamp (for MVCC transactions)
    pub async fn delete_committed(
        &self,
        table: KeyValueTableId,
        key: &[u8],
        commit_ts: u64,
    ) -> KeyValueResult<()> {
        let engine = self.get_engine(table)?;
        engine.delete_committed(key.to_vec(), commit_ts)
    }
}

impl Default for LSMKeyValueStore {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl KeyValueStore for LSMKeyValueStore {
    // ===== Basic Operations =====

    async fn get(&self, table: KeyValueTableId, key: &[u8]) -> KeyValueResult<Option<Vec<u8>>> {
        let engine = self.get_engine(table)?;
        engine.get(key)
    }

    async fn put(&self, table: KeyValueTableId, key: &[u8], value: &[u8]) -> KeyValueResult<()> {
        let engine = self.get_engine(table)?;
        engine.put(key.to_vec(), value.to_vec())
    }

    async fn delete(&self, table: KeyValueTableId, key: &[u8]) -> KeyValueResult<bool> {
        let engine = self.get_engine(table)?;
        engine.delete(key.to_vec())?;
        // TODO: Return true if key existed, false otherwise
        Ok(true)
    }

    async fn exists(&self, table: KeyValueTableId, key: &[u8]) -> KeyValueResult<bool> {
        let engine = self.get_engine(table)?;
        Ok(engine.get(key)?.is_some())
    }

    // ===== Batch Operations =====

    async fn batch_get(
        &self,
        table: KeyValueTableId,
        keys: &[&[u8]],
    ) -> KeyValueResult<Vec<Option<Vec<u8>>>> {
        let engine = self.get_engine(table)?;
        let mut results = Vec::with_capacity(keys.len());
        for key in keys {
            results.push(engine.get(key)?);
        }
        Ok(results)
    }

    async fn batch_put(
        &self,
        table: KeyValueTableId,
        pairs: &[(&[u8], &[u8])],
    ) -> KeyValueResult<()> {
        let engine = self.get_engine(table)?;
        for (key, value) in pairs {
            engine.put(key.to_vec(), value.to_vec())?;
        }
        Ok(())
    }

    async fn batch_delete(&self, table: KeyValueTableId, keys: &[&[u8]]) -> KeyValueResult<usize> {
        let engine = self.get_engine(table)?;
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
        table: KeyValueTableId,
        range: KeyRange,
    ) -> KeyValueResult<Box<dyn KeyValueIterator + Send>> {
        let engine = self.get_engine(table)?;

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

    async fn key_count(&self, table: KeyValueTableId) -> KeyValueResult<u64> {
        let engine = self.get_engine(table)?;
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

    async fn table_stats(&self, table: KeyValueTableId) -> KeyValueResult<TableStats> {
        let engine = self.get_engine(table)?;
        let stats = engine.stats();

        // Calculate total bytes
        let mut total_bytes = stats.memtable_size as u64 + stats.immutable_memtable_size as u64;
        let mut data_bytes = total_bytes;

        for level_stats in &stats.levels {
            total_bytes += level_stats.total_size;
            data_bytes += level_stats.total_size;
        }

        // Build LSM-specific stats
        let lsm_stats = LsmStats {
            num_levels: stats.levels.len(),
            sstables_per_level: stats.levels.iter().map(|l| l.num_sstables).collect(),
            bytes_per_level: stats.levels.iter().map(|l| l.total_size).collect(),
            memtable_bytes: stats.memtable_size as u64,
            pending_compactions: 0, // TODO: Track this
            total_compactions: stats.total_compactions,
            write_amplification: 0.0, // TODO: Calculate from metrics
            read_amplification: 0.0,  // TODO: Calculate from metrics
            bloom_filter_false_positives: 0.0, // TODO: Get from metrics
        };

        Ok(TableStats {
            key_count: self.key_count(table).await?,
            total_bytes,
            data_bytes,
            index_bytes: 0,      // TODO: Calculate index overhead
            last_modified: None, // TODO: Track modification time
            engine_stats: KvEngineStats::Lsm(lsm_stats),
        })
    }

    // ===== Transaction Support =====

    async fn begin_transaction(&self) -> KeyValueResult<Arc<dyn Transaction>> {
        Ok(self.get_tx_manager().begin())
    }

    // ===== Table Management =====

    async fn create_table(&self, name: &str) -> KeyValueResult<KeyValueTableId> {
        let mut next_id = self.next_table_id.write().unwrap();
        let table_id = KeyValueTableId::new(*next_id);
        *next_id += 1;

        // Store table name
        let mut table_names = self.table_names.write().unwrap();
        table_names.insert(table_id, name.to_string());

        // Create LSMTreeEngine for this table
        let engine = self.create_engine_for_table(table_id)?;
        
        // Store engine
        let mut engines = self.engines.write().unwrap();
        engines.insert(table_id, engine);

        Ok(table_id)
    }

    async fn drop_table(&self, table: KeyValueTableId) -> KeyValueResult<()> {
        let mut engines = self.engines.write().unwrap();
        engines.remove(&table);

        let mut table_names = self.table_names.write().unwrap();
        table_names.remove(&table);

        Ok(())
    }

    async fn list_tables(&self) -> KeyValueResult<Vec<(KeyValueTableId, String)>> {
        let table_names = self.table_names.read().unwrap();
        Ok(table_names
            .iter()
            .map(|(id, name)| (*id, name.clone()))
            .collect())
    }

    async fn table_exists(&self, table: KeyValueTableId) -> KeyValueResult<bool> {
        let table_names = self.table_names.read().unwrap();
        Ok(table_names.contains_key(&table))
    }

    // ===== Maintenance Operations =====

    async fn flush(&self) -> KeyValueResult<()> {
        let engines = self.engines.read().unwrap();
        for engine in engines.values() {
            engine.flush()?;
        }
        Ok(())
    }

    async fn compact(&self, table: Option<KeyValueTableId>) -> KeyValueResult<()> {
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

        // Create table
        let table_id = store.create_table("test_table").await.unwrap();
        assert!(store.table_exists(table_id).await.unwrap());

        // List tables
        let tables = store.list_tables().await.unwrap();
        assert_eq!(tables.len(), 1);
        assert_eq!(tables[0].1, "test_table");

        // Drop table
        store.drop_table(table_id).await.unwrap();
        assert!(!store.table_exists(table_id).await.unwrap());
    }

    #[tokio::test]
    async fn test_basic_operations() {
        let store = LSMKeyValueStore::new();
        let _table_id = store.create_table("test").await.unwrap();

        // Note: These tests will fail until we properly initialize engines
        // TODO: Add engine initialization in create_table
    }
}

// Made with Bob
