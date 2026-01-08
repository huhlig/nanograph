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

use crate::KeyValueIterator;
use crate::kvstore::KeyValueStore;
use crate::kvstore::KeyValueTableId;
use crate::result::{KeyValueError, KeyValueResult};
use crate::transaction::Timestamp;
use crate::transaction::Transaction;
use crate::types::{KeyRange, TableStats};
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

/// Storage engine type identifier
///
/// This is a string-based type to allow for pluggable storage engines.
/// Third-party engines can register with custom type names without
/// modifying this crate.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct StorageEngineType(String);

impl StorageEngineType {
    /// Create a new storage engine type
    pub fn new(name: impl Into<String>) -> Self {
        Self(name.into())
    }

    /// Get the engine type name
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// LSM-tree based storage (write-optimized)
    pub const fn lsm() -> Self {
        Self(String::new()) // Will be replaced with "lsm" at runtime
    }

    /// B+Tree based storage (balanced read/write)
    pub const fn btree() -> Self {
        Self(String::new()) // Will be replaced with "btree" at runtime
    }

    /// Adaptive Radix Tree (read-optimized, in-memory)
    pub const fn art() -> Self {
        Self(String::new()) // Will be replaced with "art" at runtime
    }
}

// Helper constants for built-in engine types
impl StorageEngineType {
    /// LSM-tree engine type
    pub fn lsm_type() -> Self {
        Self("lsm".to_string())
    }

    /// B+Tree engine type
    pub fn btree_type() -> Self {
        Self("btree".to_string())
    }

    /// ART engine type
    pub fn art_type() -> Self {
        Self("art".to_string())
    }
}

impl From<&str> for StorageEngineType {
    fn from(s: &str) -> Self {
        Self(s.to_string())
    }
}

impl From<String> for StorageEngineType {
    fn from(s: String) -> Self {
        Self(s)
    }
}

impl std::fmt::Display for StorageEngineType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Table metadata
#[derive(Debug, Clone)]
pub struct TableMetadata {
    pub id: KeyValueTableId,
    pub name: String,
    pub engine_type: StorageEngineType,
    pub created_at: Timestamp,
    pub last_modified: Option<Timestamp>,
    /// Number of shards for distributed tables (1 for single-node)
    pub shard_count: u32,
    /// Replication factor for each shard (1 for single-node)
    pub replication_factor: usize,
}

/// Configuration for table creation
#[derive(Debug, Clone)]
pub struct TableConfig {
    pub name: String,
    pub engine_type: StorageEngineType,
    /// Number of shards to distribute data across (default: 1 for single-node)
    pub shard_count: u32,
    /// Number of replicas per shard (default: 1 for single-node)
    pub replication_factor: usize,
    pub options: HashMap<String, String>,
}

impl TableConfig {
    pub fn new(name: impl Into<String>, engine_type: StorageEngineType) -> Self {
        Self {
            name: name.into(),
            engine_type,
            shard_count: 1,        // Default to single shard
            replication_factor: 1, // Default to no replication
            options: HashMap::new(),
        }
    }

    pub fn with_option(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.options.insert(key.into(), value.into());
        self
    }

    pub fn with_shards(mut self, shard_count: u32) -> Self {
        self.shard_count = shard_count;
        self
    }

    pub fn with_replication(mut self, replication_factor: usize) -> Self {
        self.replication_factor = replication_factor;
        self
    }
}

/// KeyValueTableManager manages multiple storage engines and provides a unified API
///
/// This is the primary interface for higher-level applications. It:
/// - Manages multiple storage engines (LSM, B+Tree, ART)
/// - Routes operations to the appropriate engine based on table configuration
/// - Provides table lifecycle management
/// - Coordinates cross-engine transactions (future)
/// - Maintains table metadata
pub struct KeyValueTableManager {
    /// Registered storage engines by type
    engines: HashMap<StorageEngineType, Arc<dyn KeyValueStore>>,

    /// Table metadata: table_id -> (metadata, engine_type)
    tables: Arc<RwLock<HashMap<KeyValueTableId, (TableMetadata, StorageEngineType)>>>,

    /// Table name to ID mapping for lookups
    table_names: Arc<RwLock<HashMap<String, KeyValueTableId>>>,

    /// Next table ID to assign
    next_table_id: Arc<RwLock<u128>>,
}

impl KeyValueTableManager {
    /// Create a new table manager
    pub fn new() -> Self {
        Self {
            engines: HashMap::new(),
            tables: Arc::new(RwLock::new(HashMap::new())),
            table_names: Arc::new(RwLock::new(HashMap::new())),
            next_table_id: Arc::new(RwLock::new(1)),
        }
    }

    /// Calculate which shard a key belongs to for a given table
    ///
    /// Uses hash-based partitioning to distribute keys across shards.
    /// For single-shard tables (shard_count=1), always returns shard 0.
    pub fn get_shard_for_key(
        &self,
        table: KeyValueTableId,
        key: &[u8],
    ) -> KeyValueResult<crate::types::ShardId> {
        let tables = self.tables.read().unwrap();
        let (metadata, _) = tables.get(&table).ok_or(KeyValueError::InvalidKey(format!(
            "Table {:?} not found",
            table
        )))?;

        if metadata.shard_count == 1 {
            // Single shard - no hashing needed
            return Ok(crate::types::ShardId::new(0));
        }

        // Hash the key and mod by shard count
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let mut hasher = DefaultHasher::new();
        key.hash(&mut hasher);
        let hash = hasher.finish();

        let shard_id = (hash % metadata.shard_count as u64) as u64;
        Ok(crate::types::ShardId::new(shard_id))
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
        engine: Arc<dyn KeyValueStore>,
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

    /// Get the storage engine for a table
    fn get_engine_for_table(
        &self,
        table: KeyValueTableId,
    ) -> KeyValueResult<Arc<dyn KeyValueStore>> {
        let tables = self.tables.read().unwrap();
        let (_, engine_type) = tables.get(&table).ok_or(KeyValueError::InvalidKey(format!(
            "Table {:?} not found",
            table
        )))?;

        self.engines
            .get(engine_type)
            .cloned()
            .ok_or(KeyValueError::InvalidValue(format!(
                "Engine type {:?} not registered",
                engine_type
            )))
    }

    /// Create a new table with the specified configuration
    pub async fn create_table(&self, config: TableConfig) -> KeyValueResult<KeyValueTableId> {
        // Check if table name already exists
        {
            let names = self.table_names.read().unwrap();
            if names.contains_key(&config.name) {
                return Err(KeyValueError::InvalidKey(format!(
                    "Table '{}' already exists",
                    config.name
                )));
            }
        }

        // Get the engine for this table type
        let engine = self
            .engines
            .get(&config.engine_type)
            .ok_or(KeyValueError::InvalidValue(format!(
                "Engine type {:?} not registered",
                config.engine_type
            )))?;

        // Allocate table ID
        let table_id = {
            let mut next_id = self.next_table_id.write().unwrap();
            let id = KeyValueTableId::new(*next_id);
            *next_id += 1;
            id
        };

        // Create table in the underlying engine
        let _engine_table_id = engine.create_table(&config.name).await?;

        // Store metadata
        let metadata = TableMetadata {
            id: table_id,
            name: config.name.clone(),
            engine_type: config.engine_type.clone(),
            created_at: Timestamp(0), // TODO: Use actual timestamp
            last_modified: None,
            shard_count: config.shard_count,
            replication_factor: config.replication_factor,
        };

        {
            let mut tables = self.tables.write().unwrap();
            tables.insert(table_id, (metadata.clone(), config.engine_type.clone()));
        }

        {
            let mut names = self.table_names.write().unwrap();
            names.insert(config.name, table_id);
        }

        Ok(table_id)
    }

    /// Get table by name
    pub fn get_table(&self, name: &str) -> KeyValueResult<KeyValueTableId> {
        let names = self.table_names.read().unwrap();
        names
            .get(name)
            .copied()
            .ok_or(KeyValueError::InvalidKey(format!(
                "Table '{}' not found",
                name
            )))
    }

    /// Get table metadata
    pub fn get_table_metadata(&self, table: KeyValueTableId) -> KeyValueResult<TableMetadata> {
        let tables = self.tables.read().unwrap();
        tables
            .get(&table)
            .map(|(metadata, _)| metadata.clone())
            .ok_or(KeyValueError::InvalidKey(format!(
                "Table {:?} not found",
                table
            )))
    }

    /// Drop a table
    pub async fn drop_table(&self, table: KeyValueTableId) -> KeyValueResult<()> {
        let engine = self.get_engine_for_table(table)?;

        // Get table name before removing
        let table_name = {
            let tables = self.tables.read().unwrap();
            tables
                .get(&table)
                .map(|(metadata, _)| metadata.name.clone())
                .ok_or(KeyValueError::InvalidKey(format!(
                    "Table {:?} not found",
                    table
                )))?
        };

        // Drop from engine
        engine.drop_table(table).await?;

        // Remove from metadata
        {
            let mut tables = self.tables.write().unwrap();
            tables.remove(&table);
        }

        {
            let mut names = self.table_names.write().unwrap();
            names.remove(&table_name);
        }

        Ok(())
    }

    /// List all tables
    pub fn list_tables(&self) -> KeyValueResult<Vec<TableMetadata>> {
        let tables = self.tables.read().unwrap();
        Ok(tables
            .values()
            .map(|(metadata, _)| metadata.clone())
            .collect())
    }

    /// Check if table exists
    pub fn table_exists(&self, table: KeyValueTableId) -> bool {
        let tables = self.tables.read().unwrap();
        tables.contains_key(&table)
    }

    /// Check if table name exists
    pub fn table_name_exists(&self, name: &str) -> bool {
        let names = self.table_names.read().unwrap();
        names.contains_key(name)
    }
}

/// Implement KeyValueStore trait for the manager to provide unified API
#[async_trait]
impl KeyValueStore for KeyValueTableManager {
    async fn get(&self, table: KeyValueTableId, key: &[u8]) -> KeyValueResult<Option<Vec<u8>>> {
        let engine = self.get_engine_for_table(table)?;
        engine.get(table, key).await
    }

    async fn put(&self, table: KeyValueTableId, key: &[u8], value: &[u8]) -> KeyValueResult<()> {
        let engine = self.get_engine_for_table(table)?;
        engine.put(table, key, value).await
    }

    async fn delete(&self, table: KeyValueTableId, key: &[u8]) -> KeyValueResult<bool> {
        let engine = self.get_engine_for_table(table)?;
        engine.delete(table, key).await
    }

    async fn exists(&self, table: KeyValueTableId, key: &[u8]) -> KeyValueResult<bool> {
        let engine = self.get_engine_for_table(table)?;
        engine.exists(table, key).await
    }

    async fn batch_get(
        &self,
        table: KeyValueTableId,
        keys: &[&[u8]],
    ) -> KeyValueResult<Vec<Option<Vec<u8>>>> {
        let engine = self.get_engine_for_table(table)?;
        engine.batch_get(table, keys).await
    }

    async fn batch_put(
        &self,
        table: KeyValueTableId,
        pairs: &[(&[u8], &[u8])],
    ) -> KeyValueResult<()> {
        let engine = self.get_engine_for_table(table)?;
        engine.batch_put(table, pairs).await
    }

    async fn batch_delete(&self, table: KeyValueTableId, keys: &[&[u8]]) -> KeyValueResult<usize> {
        let engine = self.get_engine_for_table(table)?;
        engine.batch_delete(table, keys).await
    }

    async fn scan(
        &self,
        table: KeyValueTableId,
        range: KeyRange,
    ) -> KeyValueResult<Box<dyn KeyValueIterator + Send>> {
        let engine = self.get_engine_for_table(table)?;
        engine.scan(table, range).await
    }

    async fn scan_prefix(
        &self,
        table: KeyValueTableId,
        prefix: &[u8],
        limit: Option<usize>,
    ) -> KeyValueResult<Box<dyn KeyValueIterator + Send>> {
        let engine = self.get_engine_for_table(table)?;
        engine.scan_prefix(table, prefix, limit).await
    }

    async fn key_count(&self, table: KeyValueTableId) -> KeyValueResult<u64> {
        let engine = self.get_engine_for_table(table)?;
        engine.key_count(table).await
    }

    async fn table_stats(&self, table: KeyValueTableId) -> KeyValueResult<TableStats> {
        let engine = self.get_engine_for_table(table)?;
        engine.table_stats(table).await
    }

    async fn begin_transaction(&self) -> KeyValueResult<Arc<dyn Transaction>> {
        // For now, transactions are single-engine
        // Future: implement cross-engine distributed transactions
        Err(KeyValueError::InvalidValue(
            "Transactions must be started on a specific table's engine".to_string(),
        ))
    }

    async fn create_table(&self, name: &str) -> KeyValueResult<KeyValueTableId> {
        // Default to LSM engine
        let config = TableConfig::new(name, StorageEngineType::lsm_type());
        KeyValueTableManager::create_table(self, config).await
    }

    async fn drop_table(&self, table: KeyValueTableId) -> KeyValueResult<()> {
        KeyValueTableManager::drop_table(self, table).await
    }

    async fn list_tables(&self) -> KeyValueResult<Vec<(KeyValueTableId, String)>> {
        let metadata = KeyValueTableManager::list_tables(self)?;
        Ok(metadata.into_iter().map(|m| (m.id, m.name)).collect())
    }

    async fn table_exists(&self, table: KeyValueTableId) -> KeyValueResult<bool> {
        Ok(KeyValueTableManager::table_exists(self, table))
    }

    async fn flush(&self) -> KeyValueResult<()> {
        // Flush all engines
        for engine in self.engines.values() {
            engine.flush().await?;
        }
        Ok(())
    }

    async fn compact(&self, table: Option<KeyValueTableId>) -> KeyValueResult<()> {
        if let Some(table_id) = table {
            // Compact specific table
            let engine = self.get_engine_for_table(table_id)?;
            engine.compact(Some(table_id)).await
        } else {
            // Compact all engines
            for engine in self.engines.values() {
                engine.compact(None).await?;
            }
            Ok(())
        }
    }
}

impl Default for KeyValueTableManager {
    fn default() -> Self {
        Self::new()
    }
}
