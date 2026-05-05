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

use crate::config::{LMDBConfig, LMDBStorageConfig};
use crate::error::{LMDBError, LMDBResult};
use crate::transaction::LMDBTransaction;
use async_trait::async_trait;
use lmdb::{
    Database, DatabaseFlags, Environment, EnvironmentFlags, Transaction, WriteFlags,
};
use nanograph_kvt::metrics::{ShardStats, StatValue};
use nanograph_kvt::{
    KeyRange, KeyValueError, KeyValueIterator, KeyValueResult, KeyValueShardStore, ShardId,
    Timestamp, Transaction as KvTransaction, TransactionId,
};
use nanograph_vfs::DynamicFileSystem;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, RwLock};

/// LMDB implementation of KeyValueShardStore
///
/// This is a read-optimized storage engine using LMDB (Lightning Memory-Mapped Database).
/// LMDB provides:
/// - Single-file format (data.mdb + lock.mdb)
/// - Memory-mapped I/O for fast reads
/// - ACID transactions with MVCC
/// - Copy-on-write B+tree structure
/// - Zero-copy reads
///
/// Best suited for:
/// - Read-heavy workloads
/// - Small to medium datasets that fit in memory
/// - Applications requiring fast point lookups
/// - Embedded use cases
#[derive(Clone)]
pub struct LMDBKeyValueStore {
    /// LMDB environments for each shard (one environment per shard)
    environments: Arc<RwLock<HashMap<ShardId, Arc<Environment>>>>,

    /// Databases within environments (one database per shard)
    databases: Arc<RwLock<HashMap<ShardId, Database>>>,

    /// Configuration
    config: LMDBConfig,

    /// Base directory for all LMDB databases
    base_dir: PathBuf,

    /// Transaction ID counter
    next_txn_id: Arc<AtomicU64>,
}

impl LMDBKeyValueStore {
    /// Create a new LMDB key-value store with default configuration
    pub fn new() -> Self {
        Self::with_config(LMDBConfig::default())
    }

    /// Create a new LMDB key-value store with custom configuration
    pub fn with_config(config: LMDBConfig) -> Self {
        Self {
            environments: Arc::new(RwLock::new(HashMap::new())),
            databases: Arc::new(RwLock::new(HashMap::new())),
            config,
            base_dir: PathBuf::from("./data/lmdb"),
            next_txn_id: Arc::new(AtomicU64::new(1)),
        }
    }

    /// Set the base directory for LMDB databases
    pub fn with_base_dir(mut self, base_dir: PathBuf) -> Self {
        self.base_dir = base_dir;
        self
    }

    /// Get the LMDB environment for a shard (public for transaction access)
    pub(crate) fn get_environment(&self, shard: ShardId) -> LMDBResult<Arc<Environment>> {
        let environments = self.environments.read().unwrap();
        environments
            .get(&shard)
            .cloned()
            .ok_or(LMDBError::ShardNotFound(shard.0))
    }

    /// Get the LMDB database for a shard (public for transaction access)
    pub(crate) fn get_database(&self, shard: ShardId) -> LMDBResult<Database> {
        let databases = self.databases.read().unwrap();
        databases
            .get(&shard)
            .copied()
            .ok_or(LMDBError::ShardNotFound(shard.0))
    }

    /// Create an LMDB environment and database for a shard
    fn create_environment_for_shard(
        &self,
        shard: ShardId,
        path: PathBuf,
    ) -> LMDBResult<(Arc<Environment>, Database)> {
        // Create directory if it doesn't exist
        if self.config.create_if_missing {
            std::fs::create_dir_all(&path)?;
        }

        // Build environment flags
        let mut flags = EnvironmentFlags::empty();
        if self.config.use_writemap {
            flags |= EnvironmentFlags::WRITE_MAP;
        }
        if !self.config.sync_on_commit {
            flags |= EnvironmentFlags::NO_SYNC;
        }
        if self.config.read_only {
            flags |= EnvironmentFlags::READ_ONLY;
        }

        // Create environment
        let env = Environment::new()
            .set_flags(flags)
            .set_max_dbs(self.config.max_dbs)
            .set_max_readers(self.config.max_readers)
            .set_map_size(self.config.max_db_size)
            .open(&path)?;

        let env = Arc::new(env);

        // Create or open the database
        let db = env.create_db(None, DatabaseFlags::empty())?;

        Ok((env, db))
    }

    /// Create an environment with VFS and tablespace-resolved configuration
    pub fn create_environment_with_config(
        &self,
        shard: ShardId,
        _vfs: Arc<dyn DynamicFileSystem>,
        config: LMDBStorageConfig,
    ) -> LMDBResult<(Arc<Environment>, Database)> {
        let path = PathBuf::from(&config.data_dir);
        self.create_environment_for_shard(shard, path)
    }
}

impl Default for LMDBKeyValueStore {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl KeyValueShardStore for LMDBKeyValueStore {
    // ===== Basic Operations =====

    async fn get(&self, shard: ShardId, key: &[u8]) -> KeyValueResult<Option<Vec<u8>>> {
        let env = self.get_environment(shard)?;
        let db = self.get_database(shard)?;

        let txn = env.begin_ro_txn().map_err(LMDBError::from)?;
        let result = txn.get(db, &key);

        match result {
            Ok(value) => Ok(Some(value.to_vec())),
            Err(lmdb::Error::NotFound) => Ok(None),
            Err(e) => Err(LMDBError::from(e).into()),
        }
    }

    async fn put(&self, shard: ShardId, key: &[u8], value: &[u8]) -> KeyValueResult<()> {
        let env = self.get_environment(shard)?;
        let db = self.get_database(shard)?;

        let mut txn = env.begin_rw_txn().map_err(LMDBError::from)?;
        txn.put(db, &key, &value, WriteFlags::empty())
            .map_err(LMDBError::from)?;
        txn.commit().map_err(LMDBError::from)?;

        Ok(())
    }

    async fn delete(&self, shard: ShardId, key: &[u8]) -> KeyValueResult<bool> {
        let env = self.get_environment(shard)?;
        let db = self.get_database(shard)?;

        let mut txn = env.begin_rw_txn().map_err(LMDBError::from)?;
        let result = txn.del(db, &key, None);

        match result {
            Ok(()) => {
                txn.commit().map_err(LMDBError::from)?;
                Ok(true)
            }
            Err(lmdb::Error::NotFound) => {
                txn.commit().map_err(LMDBError::from)?;
                Ok(false)
            }
            Err(e) => Err(LMDBError::from(e).into()),
        }
    }

    async fn exists(&self, shard: ShardId, key: &[u8]) -> KeyValueResult<bool> {
        Ok(self.get(shard, key).await?.is_some())
    }

    // ===== Batch Operations =====

    async fn batch_get(
        &self,
        shard: ShardId,
        keys: &[&[u8]],
    ) -> KeyValueResult<Vec<Option<Vec<u8>>>> {
        let env = self.get_environment(shard)?;
        let db = self.get_database(shard)?;

        let txn = env.begin_ro_txn().map_err(LMDBError::from)?;
        let mut results = Vec::with_capacity(keys.len());

        for key in keys {
            let result = txn.get(db, key);
            match result {
                Ok(value) => results.push(Some(value.to_vec())),
                Err(lmdb::Error::NotFound) => results.push(None),
                Err(e) => return Err(LMDBError::from(e).into()),
            }
        }

        Ok(results)
    }

    async fn batch_put(&self, shard: ShardId, pairs: &[(&[u8], &[u8])]) -> KeyValueResult<()> {
        let env = self.get_environment(shard)?;
        let db = self.get_database(shard)?;

        let mut txn = env.begin_rw_txn().map_err(LMDBError::from)?;

        for (key, value) in pairs {
            txn.put(db, key, value, WriteFlags::empty())
                .map_err(LMDBError::from)?;
        }

        txn.commit().map_err(LMDBError::from)?;
        Ok(())
    }

    async fn batch_delete(&self, shard: ShardId, keys: &[&[u8]]) -> KeyValueResult<usize> {
        let env = self.get_environment(shard)?;
        let db = self.get_database(shard)?;

        let mut txn = env.begin_rw_txn().map_err(LMDBError::from)?;
        let mut count = 0;

        for key in keys {
            match txn.del(db, key, None) {
                Ok(()) => count += 1,
                Err(lmdb::Error::NotFound) => {}
                Err(e) => return Err(LMDBError::from(e).into()),
            }
        }

        txn.commit().map_err(LMDBError::from)?;
        Ok(count)
    }

    // ===== Range Operations =====

    async fn scan(
        &self,
        shard: ShardId,
        range: KeyRange,
    ) -> KeyValueResult<Box<dyn KeyValueIterator + Send>> {
        let env = self.get_environment(shard)?;
        let db = self.get_database(shard)?;

        // Create streaming iterator that fetches data in chunks
        let iterator = crate::iterator::LMDBIterator::new(env, db, range);

        Ok(Box::new(iterator))
    }

    // ===== Statistics & Metadata =====

    async fn key_count(&self, shard: ShardId) -> KeyValueResult<u64> {
        let env = self.get_environment(shard)?;
        let db = self.get_database(shard)?;

        let txn = env.begin_ro_txn().map_err(LMDBError::from)?;
        let stat = txn.stat(db).map_err(LMDBError::from)?;

        Ok(stat.entries() as u64)
    }

    async fn shard_stats(&self, shard: ShardId) -> KeyValueResult<ShardStats> {
        let env = self.get_environment(shard)?;
        let db = self.get_database(shard)?;

        let txn = env.begin_ro_txn().map_err(LMDBError::from)?;
        let stat = txn.stat(db).map_err(LMDBError::from)?;
        let env_info = env.info().map_err(LMDBError::from)?;

        let key_count = stat.entries() as u64;
        let page_size = stat.page_size() as u64;
        let total_pages = (stat.branch_pages() + stat.leaf_pages() + stat.overflow_pages()) as u64;
        let total_bytes = total_pages * page_size;

        let mut shard_stats = ShardStats {
            key_count,
            total_bytes,
            data_bytes: total_bytes,
            index_bytes: 0,
            last_modified: None,
            engine_stats: Default::default(),
        };

        // Add LMDB-specific stats
        shard_stats
            .engine_stats
            .insert("page_size", StatValue::from_u64(page_size));
        shard_stats.engine_stats.insert(
            "branch_pages",
            StatValue::from_u64(stat.branch_pages() as u64),
        );
        shard_stats
            .engine_stats
            .insert("leaf_pages", StatValue::from_u64(stat.leaf_pages() as u64));
        shard_stats.engine_stats.insert(
            "overflow_pages",
            StatValue::from_u64(stat.overflow_pages() as u64),
        );
        shard_stats
            .engine_stats
            .insert("depth", StatValue::from_u64(stat.depth() as u64));
        shard_stats
            .engine_stats
            .insert("map_size", StatValue::from_usize(env_info.map_size()));
        shard_stats
            .engine_stats
            .insert("last_pgno", StatValue::from_usize(env_info.last_pgno()));

        Ok(shard_stats)
    }

    // ===== Transaction Support =====

    async fn begin_transaction(&self) -> KeyValueResult<Arc<dyn KvTransaction>> {
        // Generate a unique transaction ID
        let txn_id = TransactionId(self.next_txn_id.fetch_add(1, Ordering::SeqCst));
        
        // Get current timestamp for snapshot isolation
        let snapshot_ts = Timestamp::now();
        
        // Create a new LMDB transaction wrapper
        let txn = LMDBTransaction::new(txn_id, snapshot_ts, Arc::new(self.clone()));
        
        Ok(Arc::new(txn))
    }

    // ===== Shard Management =====

    fn create_shard(
        &self,
        shard_id: ShardId,
        _vfs: Arc<dyn DynamicFileSystem>,
        data_path: nanograph_vfs::Path,
        _wal_path: nanograph_vfs::Path,
    ) -> KeyValueResult<()> {
        // Convert VFS path to system path and join with base_dir
        let data_path_str = data_path.to_string();
        // Remove leading slash if present to make it relative
        let relative_path = data_path_str.trim_start_matches('/');
        let path = self.base_dir.join(relative_path);

        // Create environment and database
        let (env, db) = self.create_environment_for_shard(shard_id, path)?;

        // Store environment and database
        {
            let mut environments = self.environments.write().unwrap();
            environments.insert(shard_id, env);
        }
        {
            let mut databases = self.databases.write().unwrap();
            databases.insert(shard_id, db);
        }

        Ok(())
    }

    async fn drop_shard(&self, shard: ShardId) -> KeyValueResult<()> {
        // Remove from maps
        {
            let mut environments = self.environments.write().unwrap();
            environments.remove(&shard);
        }
        {
            let mut databases = self.databases.write().unwrap();
            databases.remove(&shard);
        }

        Ok(())
    }

    async fn clear(&self, shard: ShardId) -> KeyValueResult<()> {
        let env = self.get_environment(shard)?;
        let db = self.get_database(shard)?;

        let mut txn = env.begin_rw_txn().map_err(LMDBError::from)?;
        txn.clear_db(db).map_err(LMDBError::from)?;
        txn.commit().map_err(LMDBError::from)?;

        Ok(())
    }

    async fn list_shards(&self) -> KeyValueResult<Vec<ShardId>> {
        let environments = self.environments.read().unwrap();
        Ok(environments.keys().copied().collect())
    }

    async fn shard_exists(&self, shard: ShardId) -> KeyValueResult<bool> {
        let environments = self.environments.read().unwrap();
        Ok(environments.contains_key(&shard))
    }

    // ===== Maintenance Operations =====

    async fn flush(&self) -> KeyValueResult<()> {
        // LMDB syncs on commit by default if sync_on_commit is true
        // Force sync all environments
        let environments = self.environments.read().unwrap();
        for env in environments.values() {
            env.sync(true).map_err(LMDBError::from)?;
        }
        Ok(())
    }

    async fn compact(&self, _shard: Option<ShardId>) -> KeyValueResult<()> {
        // LMDB doesn't support online compaction
        // Would need to copy to a new database
        Err(KeyValueError::InvalidValue(
            "LMDB does not support online compaction".to_string(),
        ))
    }
}

// Made with Bob
