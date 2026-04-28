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

//! B-Tree index implementation for secondary indexes with persistence
//!
//! B-Tree indexes are ideal for:
//! - Range queries
//! - Sorted scans
//! - Prefix matching
//! - Ordered data access
//!
//! This implementation uses:
//! - PersistentIndexStore for durable storage
//! - KeyValueShardStore for distributed sharding
//! - WriteAheadLog for crash recovery
//! - LRU cache for hot entries

use crate::error::{IndexError, IndexResult};
use crate::index::ordered::{OrderedIndex, UniqueIndex};
use crate::index::{IndexEntry, IndexQuery, IndexStats, IndexStore};
use crate::persistence::{PersistenceConfig, PersistentIndexStore};
use crate::serialization::{deserialize_entry, serialize_entry};
use async_trait::async_trait;
use nanograph_core::object::{IndexRecord, IndexStatus, IndexType};
use nanograph_kvt::KeyValueShardStore;
use nanograph_wal::WriteAheadLogManager;
use std::ops::Bound;
use std::sync::Arc;
use tokio::sync::RwLock;

/// B-Tree index implementation with persistence and distributed shard support
///
/// This index maintains sorted order of indexed values using persistent storage,
/// enabling efficient range queries and sorted scans with durability guarantees.
///
/// # Features
/// - Persistent storage via KeyValueShardStore
/// - Write-ahead logging for crash recovery
/// - LRU cache for hot entries
/// - Distributed shard support
/// - Range queries with bounds
/// - Prefix matching
/// - Unique constraint enforcement (optional)
///
/// # Example
///
/// ```ignore
/// use nanograph_idx::BTreeIndex;
/// use nanograph_core::object::{IndexRecord, IndexType};
///
/// let index = BTreeIndex::new(
///     metadata,
///     store,
///     Some(wal),
///     config,
/// ).await?;
/// ```
pub struct BTreeIndex {
    /// Index metadata
    metadata: Arc<RwLock<IndexRecord>>,
    /// Persistent storage layer
    storage: Arc<PersistentIndexStore>,
    /// Whether this is a unique index
    is_unique: bool,
}

impl BTreeIndex {
    /// Create a new B-Tree index with persistence
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
        let is_unique = matches!(metadata.index_type, IndexType::Unique);
        let storage = Arc::new(PersistentIndexStore::new(config, store, wal));

        Ok(Self {
            metadata: Arc::new(RwLock::new(metadata)),
            storage,
            is_unique,
        })
    }

    /// Extract indexed value from row data
    ///
    /// This is a placeholder - in production, this would use the key_extractor
    /// from the index metadata to extract the indexed columns from the row.
    fn extract_indexed_value(&self, _row_data: &[u8]) -> IndexResult<Vec<u8>> {
        // TODO: Implement proper key extraction based on index columns
        // For now, return the row data as-is
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
impl IndexStore for BTreeIndex {
    fn metadata(&self) -> &IndexRecord {
        // Note: This is a synchronous method but we have async metadata
        // In production, we'd need to refactor the trait or use a different approach
        // For now, we'll need to handle this carefully
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

            // Check uniqueness if required
            if self.is_unique {
                if self.storage.exists(&indexed_value, &primary_key).await? {
                    return Err(IndexError::UniqueViolation(format!(
                        "Duplicate value during index build: {:?}",
                        indexed_value
                    )));
                }
            }

            // Serialize entry
            let entry_data = serialize_entry(&indexed_value, &primary_key, None)?;

            // Write to storage
            self.storage
                .write_entry(&indexed_value, &primary_key, &entry_data)
                .await?;

            count += 1;

            // Flush periodically to avoid memory buildup
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
        // Check uniqueness if required
        if self.is_unique {
            // Check if any entry exists with this indexed value
            let query = IndexQuery::exact(entry.indexed_value.clone()).with_limit(1);
            let existing = self.query(query).await?;
            
            // If an entry exists with a different primary key, it's a violation
            if let Some(existing_entry) = existing.first() {
                if existing_entry.primary_key != entry.primary_key {
                    return Err(IndexError::UniqueViolation(format!(
                        "Duplicate indexed value: {:?}",
                        entry.indexed_value
                    )));
                }
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
        // We need to find the indexed value for this primary key
        // This requires a reverse lookup, which is expensive
        // In production, we'd maintain a reverse index or use a different approach

        // For now, we'll scan to find the entry
        // TODO: Optimize this with a reverse index
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
        // Scan the range from storage
        let results = self
            .storage
            .scan_range(query.start, query.end, query.limit, query.reverse)
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

    async fn get(&self, primary_key: &[u8]) -> IndexResult<Option<IndexEntry>> {
        // This requires scanning to find the entry with this primary key
        // TODO: Optimize with a reverse index
        let query = IndexQuery::all();
        let entries = self.query(query).await?;

        Ok(entries
            .into_iter()
            .find(|e| e.primary_key == primary_key))
    }

    async fn exists(&self, indexed_value: &[u8]) -> IndexResult<bool> {
        // Check if any entry exists with this indexed value
        let query = IndexQuery::exact(indexed_value.to_vec()).with_limit(1);
        let entries = self.query(query).await?;
        Ok(!entries.is_empty())
    }

    async fn stats(&self) -> IndexResult<IndexStats> {
        // Scan all entries to calculate statistics
        let query = IndexQuery::all();
        let entries = self.query(query).await?;

        let entry_count = entries.len() as u64;
        let total_size: usize = entries
            .iter()
            .map(|e| {
                e.indexed_value.len()
                    + e.primary_key.len()
                    + e.included_columns.as_ref().map(|c| c.len()).unwrap_or(0)
            })
            .sum();

        let avg_entry_size = if entry_count > 0 {
            total_size as u64 / entry_count
        } else {
            0
        };

        Ok(IndexStats {
            entry_count,
            size_bytes: total_size as u64,
            levels: None, // B-Tree levels not tracked in this implementation
            avg_entry_size,
            fragmentation: None,
        })
    }

    async fn optimize(&mut self) -> IndexResult<()> {
        // Flush to ensure all data is persisted
        self.storage.flush().await?;

        // Clear cache to free memory
        self.storage.clear_cache();

        Ok(())
    }

    async fn flush(&mut self) -> IndexResult<()> {
        self.storage.flush().await
    }
}

#[async_trait]
impl OrderedIndex for BTreeIndex {
    async fn range_scan(
        &self,
        start: Bound<Vec<u8>>,
        end: Bound<Vec<u8>>,
        limit: Option<usize>,
        reverse: bool,
    ) -> IndexResult<Vec<IndexEntry>> {
        let query = IndexQuery {
            start,
            end,
            limit,
            reverse,
        };
        self.query(query).await
    }

    async fn min_key(&self) -> IndexResult<Option<Vec<u8>>> {
        let query = IndexQuery::all().with_limit(1);
        let entries = self.query(query).await?;
        Ok(entries.first().map(|e| e.indexed_value.clone()))
    }

    async fn max_key(&self) -> IndexResult<Option<Vec<u8>>> {
        let query = IndexQuery::all().with_limit(1).reversed();
        let entries = self.query(query).await?;
        Ok(entries.first().map(|e| e.indexed_value.clone()))
    }

    async fn prefix_scan(
        &self,
        prefix: &[u8],
        limit: Option<usize>,
    ) -> IndexResult<Vec<IndexEntry>> {
        // Create a range that matches the prefix
        let mut end_prefix = prefix.to_vec();
        // Increment the last byte to create an exclusive upper bound
        if let Some(last) = end_prefix.last_mut() {
            if *last < 255 {
                *last += 1;
            } else {
                // If last byte is 255, we need to extend the prefix
                end_prefix.push(0);
            }
        }

        let query = IndexQuery {
            start: Bound::Included(prefix.to_vec()),
            end: Bound::Excluded(end_prefix),
            limit,
            reverse: false,
        };

        self.query(query).await
    }

    async fn count_range(
        &self,
        start: Bound<Vec<u8>>,
        end: Bound<Vec<u8>>,
    ) -> IndexResult<u64> {
        let query = IndexQuery {
            start,
            end,
            limit: None,
            reverse: false,
        };
        let entries = self.query(query).await?;
        Ok(entries.len() as u64)
    }
}

#[async_trait]
impl UniqueIndex for BTreeIndex {
    async fn lookup_unique(
        &self,
        indexed_value: &[u8],
    ) -> IndexResult<Option<Vec<u8>>> {
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
    use nanograph_core::object::{DatabaseId, IndexId, IndexSharding, IndexStatus, ObjectId, ShardId, ShardNumber, TenantId};
    use nanograph_core::types::Timestamp;
    use std::collections::HashMap;

    fn create_test_metadata() -> IndexRecord {
        IndexRecord {
            index_id: IndexId::new(ObjectId::new(1)),
            name: "test_idx".to_string(),
            version: 0,
            index_type: IndexType::Secondary,
            created_at: Timestamp::now(),
            updated_at: Timestamp::now(),
            columns: vec!["test_col".to_string()],
            key_extractor: None,
            options: HashMap::new(),
            metadata: HashMap::new(),
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
            durability: nanograph_wal::Durability::Flush,
            enable_wal: false, // Disable WAL for tests
        }
    }

    #[tokio::test]
    async fn test_btree_index_creation() {
        let metadata = create_test_metadata();
        let store = Arc::new(nanograph_kvt::MemoryKeyValueShardStore::new());
        let config = create_test_config();

        let index = BTreeIndex::new(metadata, store, None, config).await;
        assert!(index.is_ok());
    }

    #[tokio::test]
    async fn test_btree_index_insert() {
        let metadata = create_test_metadata();
        let store = Arc::new(nanograph_kvt::MemoryKeyValueShardStore::new());
        let config = create_test_config();

        let mut index = BTreeIndex::new(metadata, store, None, config).await.unwrap();

        let entry = IndexEntry {
            indexed_value: b"value1".to_vec(),
            primary_key: b"key1".to_vec(),
            included_columns: None,
        };

        let result = index.insert(entry).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_btree_index_range_scan() {
        let metadata = create_test_metadata();
        let store = Arc::new(nanograph_kvt::MemoryKeyValueShardStore::new());
        let config = create_test_config();

        let mut index = BTreeIndex::new(metadata, store, None, config).await.unwrap();

        // Insert test data
        for i in 0..10 {
            let entry = IndexEntry {
                indexed_value: format!("value{:02}", i).into_bytes(),
                primary_key: format!("key{}", i).into_bytes(),
                included_columns: None,
            };
            index.insert(entry).await.unwrap();
        }

        // Test range scan
        let results = index
            .range_scan(
                Bound::Included(b"value03".to_vec()),
                Bound::Included(b"value07".to_vec()),
                None,
                false,
            )
            .await
            .unwrap();

        assert_eq!(results.len(), 5); // value03 through value07
    }

    #[tokio::test]
    async fn test_btree_index_prefix_scan() {
        let metadata = create_test_metadata();
        let store = Arc::new(nanograph_kvt::MemoryKeyValueShardStore::new());
        let config = create_test_config();

        let mut index = BTreeIndex::new(metadata, store, None, config).await.unwrap();

        // Insert test data with common prefix
        for i in 0..5 {
            let entry = IndexEntry {
                indexed_value: format!("user_{}", i).into_bytes(),
                primary_key: format!("key{}", i).into_bytes(),
                included_columns: None,
            };
            index.insert(entry).await.unwrap();
        }

        // Test prefix scan
        let results = index.prefix_scan(b"user_", None).await.unwrap();
        assert_eq!(results.len(), 5);
    }

    #[tokio::test]
    async fn test_btree_unique_constraint() {
        let mut metadata = create_test_metadata();
        metadata.index_type = IndexType::Unique;

        let store = Arc::new(nanograph_kvt::MemoryKeyValueShardStore::new());
        let config = create_test_config();

        let mut index = BTreeIndex::new(metadata, store, None, config).await.unwrap();

        // Insert first entry
        let entry1 = IndexEntry {
            indexed_value: b"unique_value".to_vec(),
            primary_key: b"key1".to_vec(),
            included_columns: None,
        };
        assert!(index.insert(entry1).await.is_ok());

        // Try to insert duplicate
        let entry2 = IndexEntry {
            indexed_value: b"unique_value".to_vec(),
            primary_key: b"key2".to_vec(),
            included_columns: None,
        };
        let result = index.insert(entry2).await;
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), IndexError::UniqueViolation(_)));
    }
}

// Made with Bob
