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

//! Hash index implementation for unique constraints with persistence
//!
//! Hash indexes are ideal for:
//! - Fast O(1) point lookups
//! - Uniqueness constraint enforcement
//! - Equality checks
//!
//! This implementation uses:
//! - PersistentIndexStore for durable storage
//! - KeyValueShardStore for distributed sharding
//! - WriteAheadLog for crash recovery
//! - LRU cache for hot entries

use crate::error::{IndexError, IndexResult};
use crate::index::ordered::UniqueIndex;
use crate::index::{IndexEntry, IndexQuery, IndexStats, IndexStore};
use crate::persistence::{PersistenceConfig, PersistentIndexStore};
use crate::serialization::{deserialize_entry, serialize_entry};
use async_trait::async_trait;
use nanograph_core::object::{IndexRecord, IndexStatus};
use nanograph_kvt::KeyValueShardStore;
use nanograph_wal::WriteAheadLogManager;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Hash index implementation with persistence and distributed shard support
///
/// This index uses persistent storage for O(1) lookups and enforces uniqueness constraints
/// with durability guarantees.
///
/// # Features
/// - Persistent storage via KeyValueShardStore
/// - Write-ahead logging for crash recovery
/// - LRU cache for hot entries
/// - Distributed shard support
/// - O(1) exact match queries
/// - Unique constraint enforcement
///
/// # Example
///
/// ```ignore
/// use nanograph_idx::HashIndex;
/// use nanograph_core::object::{IndexRecord, IndexType};
///
/// let index = HashIndex::new(
///     metadata,
///     store,
///     Some(wal),
///     config,
/// ).await?;
/// ```
pub struct HashIndex {
    /// Index metadata
    metadata: Arc<RwLock<IndexRecord>>,
    /// Persistent storage layer
    storage: Arc<PersistentIndexStore>,
}

impl HashIndex {
    /// Create a new hash index with persistence
    ///
    /// # Arguments
    /// * `metadata` - Index metadata
    /// * `store` - Underlying key-value store
    /// * `wal` - Optional write-ahead log for durability
    /// * `config` - Persistence configuration
    pub async fn new(
        metadata: IndexRecord,
        store: Arc<dyn KeyValueShardStore>,
        wal: Option<Arc<WriteAheadLogManager>>,
        config: PersistenceConfig,
    ) -> IndexResult<Self> {
        let storage = Arc::new(PersistentIndexStore::new(config, store, wal));

        Ok(Self {
            metadata: Arc::new(RwLock::new(metadata)),
            storage,
        })
    }

    /// Extract indexed value from row data
    fn extract_indexed_value(&self, _row_data: &[u8]) -> IndexResult<Vec<u8>> {
        // TODO: Implement proper key extraction based on index columns
        Ok(_row_data.to_vec())
    }

    /// Update index status
    async fn update_status(&self, status: IndexStatus) -> IndexResult<()> {
        let mut metadata = self.metadata.write().await;
        metadata.status = status;
        Ok(())
    }
}

#[async_trait]
impl IndexStore for HashIndex {
    fn metadata(&self) -> &IndexRecord {
        unimplemented!("Use async metadata access instead")
    }

    async fn build<I>(&mut self, table_data: I) -> IndexResult<()>
    where
        I: Iterator<Item = (Vec<u8>, Vec<u8>)> + Send,
    {
        // Update status to building
        self.update_status(IndexStatus::Building).await?;

        // Build index from table data
        let mut count = 0u64;
        for (primary_key, row_data) in table_data {
            // Extract indexed value from row
            let indexed_value = self.extract_indexed_value(&row_data)?;

            // Check uniqueness
            if self.storage.exists(&indexed_value, &primary_key).await? {
                return Err(IndexError::UniqueViolation(format!(
                    "Duplicate value during index build: {:?}",
                    indexed_value
                )));
            }

            // Serialize entry
            let entry_data = serialize_entry(&indexed_value, &primary_key, None)?;

            // Write to storage
            self.storage
                .write_entry(&indexed_value, &primary_key, &entry_data)
                .await?;

            count += 1;

            // Flush periodically
            if count % 10000 == 0 {
                self.storage.flush().await?;
            }
        }

        // Final flush
        self.storage.flush().await?;

        // Update status to active
        self.update_status(IndexStatus::Active).await?;

        Ok(())
    }

    async fn insert(&mut self, entry: IndexEntry) -> IndexResult<()> {
        // Check for uniqueness violation - check if any entry exists with this indexed value
        let query = IndexQuery::exact(entry.indexed_value.clone()).with_limit(1);
        let existing = self.query(query).await?;

        // If an entry exists with a different primary key, it's a violation
        if let Some(existing_entry) = existing.first() {
            if existing_entry.primary_key != entry.primary_key {
                return Err(IndexError::UniqueViolation(format!(
                    "Value already exists in unique index: {:?}",
                    entry.indexed_value
                )));
            }
        }

        // Serialize entry
        let entry_data = serialize_entry(
            &entry.indexed_value,
            &entry.primary_key,
            entry.included_columns.as_deref(),
        )?;

        // Write to storage
        self.storage
            .write_entry(&entry.indexed_value, &entry.primary_key, &entry_data)
            .await?;

        Ok(())
    }

    async fn delete(&mut self, primary_key: &[u8]) -> IndexResult<()> {
        // Find and delete the entry
        // This requires scanning to find the indexed value for this primary key
        let query = IndexQuery::all();
        let entries = self.query(query).await?;

        for entry in entries {
            if entry.primary_key == primary_key {
                self.storage
                    .delete_entry(&entry.indexed_value, primary_key)
                    .await?;
                return Ok(());
            }
        }

        Ok(())
    }

    async fn query(&self, query: IndexQuery) -> IndexResult<Vec<IndexEntry>> {
        // Hash indexes only support exact match queries
        match (&query.start, &query.end) {
            (std::ops::Bound::Included(start), std::ops::Bound::Included(end)) if start == end => {
                // Exact match query - use storage scan
                let results = self
                    .storage
                    .scan_range(
                        std::ops::Bound::Included(start.clone()),
                        std::ops::Bound::Included(end.clone()),
                        query.limit,
                        false, // Hash index doesn't support reverse
                    )
                    .await?;

                // Deserialize entries
                let mut entries = Vec::with_capacity(results.len());
                for (_key, value) in results {
                    let serialized = deserialize_entry(&value)?;
                    entries.push(IndexEntry {
                        indexed_value: serialized.indexed_value,
                        primary_key: serialized.primary_key,
                        included_columns: serialized.included_columns,
                    });
                }

                Ok(entries)
            }
            _ => Err(IndexError::QueryFailed(
                "Hash indexes only support exact match queries".to_string(),
            )),
        }
    }

    async fn get(&self, primary_key: &[u8]) -> IndexResult<Option<IndexEntry>> {
        // Requires scanning to find entry by primary key
        let query = IndexQuery::all();
        let entries = self.query(query).await?;

        Ok(entries.into_iter().find(|e| e.primary_key == primary_key))
    }

    async fn exists(&self, indexed_value: &[u8]) -> IndexResult<bool> {
        // Check if any entry exists with this indexed value
        let query = IndexQuery::exact(indexed_value.to_vec()).with_limit(1);
        let entries = self.query(query).await?;
        Ok(!entries.is_empty())
    }

    async fn stats(&self) -> IndexResult<IndexStats> {
        // Scan all entries directly from storage to calculate statistics
        let results = self
            .storage
            .scan_range(
                std::ops::Bound::Unbounded,
                std::ops::Bound::Unbounded,
                None,
                false,
            )
            .await?;

        let entry_count = results.len() as u64;
        let total_size: usize = results.iter().map(|(_key, value)| value.len()).sum();

        let avg_entry_size = if entry_count > 0 {
            total_size as u64 / entry_count
        } else {
            0
        };

        Ok(IndexStats {
            entry_count,
            size_bytes: total_size as u64,
            levels: None,
            avg_entry_size,
            fragmentation: None,
        })
    }

    async fn optimize(&mut self) -> IndexResult<()> {
        // Flush and clear cache
        self.storage.flush().await?;
        self.storage.clear_cache();
        Ok(())
    }

    async fn flush(&mut self) -> IndexResult<()> {
        self.storage.flush().await
    }
}

#[async_trait]
impl UniqueIndex for HashIndex {
    async fn lookup_unique(&self, indexed_value: &[u8]) -> IndexResult<Option<Vec<u8>>> {
        let query = IndexQuery::exact(indexed_value.to_vec()).with_limit(1);
        let entries = self.query(query).await?;
        Ok(entries.first().map(|e| e.primary_key.clone()))
    }

    async fn validate_unique(&self, indexed_value: &[u8]) -> IndexResult<()> {
        if self.exists(indexed_value).await? {
            Err(IndexError::UniqueViolation(format!(
                "Value already exists: {:?}",
                indexed_value
            )))
        } else {
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nanograph_core::object::{
        DatabaseId, IndexId, IndexSharding, IndexStatus, IndexType, ObjectId, ShardId, ShardNumber,
        TenantId,
    };
    use nanograph_core::types::Timestamp;
    use std::collections::HashMap as StdHashMap;

    fn create_test_metadata() -> IndexRecord {
        IndexRecord {
            index_id: IndexId::new(ObjectId::new(1)),
            name: "test_unique_idx".to_string(),
            version: 0,
            index_type: IndexType::Unique,
            created_at: Timestamp::now(),
            updated_at: Timestamp::now(),
            columns: vec!["email".to_string()],
            key_extractor: None,
            options: StdHashMap::new(),
            metadata: StdHashMap::new(),
            status: IndexStatus::Building,
            sharding: IndexSharding::Single,
        }
    }

    fn create_test_config() -> PersistenceConfig {
        PersistenceConfig {
            shard_id: ShardId::from_parts(
                TenantId::new(1),
                DatabaseId::new(1),
                ObjectId::new(1),
                ShardNumber::new(0),
            ),
            index_id: IndexId::new(ObjectId::new(1)),
            cache_size: 100,
            durability: nanograph_wal::Durability::Buffered,
            enable_wal: false,
        }
    }

    #[tokio::test]
    async fn test_hash_index_creation() {
        let metadata = create_test_metadata();
        let store = Arc::new(nanograph_kvt::MemoryKeyValueShardStore::new());
        let config = create_test_config();

        let index = HashIndex::new(metadata, store, None, config).await;
        assert!(index.is_ok());
    }

    #[tokio::test]
    async fn test_hash_index_unique_constraint() {
        let metadata = create_test_metadata();
        let store = Arc::new(nanograph_kvt::MemoryKeyValueShardStore::new());
        let config = create_test_config();

        let mut index = HashIndex::new(metadata, store, None, config).await.unwrap();

        let entry1 = IndexEntry {
            indexed_value: b"test@example.com".to_vec(),
            primary_key: b"key1".to_vec(),
            included_columns: None,
        };

        // First insert should succeed
        assert!(index.insert(entry1.clone()).await.is_ok());

        let entry2 = IndexEntry {
            indexed_value: b"test@example.com".to_vec(),
            primary_key: b"key2".to_vec(),
            included_columns: None,
        };

        // Second insert with same value should fail
        let result = index.insert(entry2).await;
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            IndexError::UniqueViolation(_)
        ));
    }

    #[tokio::test]
    async fn test_hash_index_exists() {
        let metadata = create_test_metadata();
        let store = Arc::new(nanograph_kvt::MemoryKeyValueShardStore::new());
        let config = create_test_config();

        let mut index = HashIndex::new(metadata, store, None, config).await.unwrap();

        let entry = IndexEntry {
            indexed_value: b"test@example.com".to_vec(),
            primary_key: b"key1".to_vec(),
            included_columns: None,
        };

        index.insert(entry).await.unwrap();

        assert!(index.exists(b"test@example.com").await.unwrap());
        assert!(!index.exists(b"other@example.com").await.unwrap());
    }

    #[tokio::test]
    async fn test_hash_index_lookup_unique() {
        let metadata = create_test_metadata();
        let store = Arc::new(nanograph_kvt::MemoryKeyValueShardStore::new());
        let config = create_test_config();

        let mut index = HashIndex::new(metadata, store, None, config).await.unwrap();

        let entry = IndexEntry {
            indexed_value: b"test@example.com".to_vec(),
            primary_key: b"user123".to_vec(),
            included_columns: None,
        };

        index.insert(entry).await.unwrap();

        let result = index.lookup_unique(b"test@example.com").await.unwrap();
        assert_eq!(result, Some(b"user123".to_vec()));

        let not_found = index.lookup_unique(b"other@example.com").await.unwrap();
        assert_eq!(not_found, None);
    }
}

// Made with Bob
