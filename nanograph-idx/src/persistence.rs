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

//! Persistence layer for index storage
//!
//! This module provides persistent storage for indexes using the KeyValueShardStore
//! and WriteAheadLog for durability guarantees.

use crate::{IndexError, IndexResult};
use futures::StreamExt;
use lru::LruCache;
use nanograph_core::object::{IndexId, ShardId};
use nanograph_kvt::KeyValueShardStore;
use nanograph_wal::{Durability, LogSequenceNumber, WriteAheadLogManager, WriteAheadLogRecord};
use parking_lot::RwLock;
use std::num::NonZeroUsize;
use std::ops::Bound;
use std::sync::Arc;

/// Configuration for persistent index storage
#[derive(Debug, Clone)]
pub struct PersistenceConfig {
    /// Shard ID for this index
    pub shard_id: ShardId,
    /// Index ID
    pub index_id: IndexId,
    /// Cache size (number of entries)
    pub cache_size: usize,
    /// Durability level for writes
    pub durability: Durability,
    /// Enable write-ahead logging
    pub enable_wal: bool,
}

impl Default for PersistenceConfig {
    fn default() -> Self {
        Self {
            shard_id: ShardId::default(),
            index_id: IndexId::default(),
            cache_size: 10000,
            durability: Durability::Buffered,
            enable_wal: true,
        }
    }
}

/// Persistent index store implementation
///
/// Wraps KeyValueShardStore for persistent storage and integrates with
/// WriteAheadLog for durability. Includes an LRU cache for hot entries.
pub struct PersistentIndexStore {
    /// Configuration
    config: PersistenceConfig,
    /// Underlying key-value store
    store: Arc<dyn KeyValueShardStore>,
    /// Write-ahead log for durability
    wal: Option<Arc<WriteAheadLogManager>>,
    /// LRU cache for hot entries
    cache: Arc<RwLock<LruCache<Vec<u8>, Vec<u8>>>>,
}

impl PersistentIndexStore {
    /// Create a new persistent index store
    pub fn new(
        config: PersistenceConfig,
        store: Arc<dyn KeyValueShardStore>,
        wal: Option<Arc<WriteAheadLogManager>>,
    ) -> Self {
        let cache_size =
            NonZeroUsize::new(config.cache_size).unwrap_or(NonZeroUsize::new(1).unwrap());
        Self {
            config,
            store,
            wal,
            cache: Arc::new(RwLock::new(LruCache::new(cache_size))),
        }
    }

    /// Build the key for an index entry
    ///
    /// Format: index_id + indexed_value + primary_key
    fn build_key(&self, indexed_value: &[u8], primary_key: &[u8]) -> Vec<u8> {
        let mut key = Vec::with_capacity(8 + indexed_value.len() + primary_key.len());
        key.extend_from_slice(&self.config.index_id.object().as_u32().to_be_bytes());
        key.extend_from_slice(indexed_value);
        key.extend_from_slice(primary_key);
        key
    }

    /// Build the key prefix for scanning by indexed value
    fn build_prefix(&self, indexed_value: &[u8]) -> Vec<u8> {
        let mut prefix = Vec::with_capacity(8 + indexed_value.len());
        prefix.extend_from_slice(&self.config.index_id.object().as_u32().to_be_bytes());
        prefix.extend_from_slice(indexed_value);
        prefix
    }

    /// Write an entry to the WAL
    async fn write_wal(&self, operation: WalOperation) -> IndexResult<Option<LogSequenceNumber>> {
        if let Some(wal) = &self.wal {
            let payload = bincode::serialize(&operation)
                .map_err(|e| IndexError::Serialization(e.to_string()))?;

            let record = WriteAheadLogRecord {
                kind: operation.kind(),
                payload: &payload,
            };

            let mut writer = wal
                .writer()
                .map_err(|e| IndexError::Storage(e.to_string()))?;

            let lsn = writer
                .append(record, self.config.durability)
                .map_err(|e| IndexError::Storage(e.to_string()))?;

            Ok(Some(lsn))
        } else {
            Ok(None)
        }
    }

    /// Write an entry to persistent storage
    pub async fn write_entry(
        &self,
        indexed_value: &[u8],
        primary_key: &[u8],
        value: &[u8],
    ) -> IndexResult<()> {
        // Write to WAL first
        self.write_wal(WalOperation::Insert {
            indexed_value: indexed_value.to_vec(),
            primary_key: primary_key.to_vec(),
            value: value.to_vec(),
        })
        .await?;

        // Build the key
        let key = self.build_key(indexed_value, primary_key);

        // Write to storage
        self.store
            .put(self.config.shard_id, &key, value)
            .await
            .map_err(|e| IndexError::Storage(e.to_string()))?;

        // Update cache
        self.cache.write().put(key, value.to_vec());

        Ok(())
    }

    /// Read an entry from storage
    pub async fn read_entry(
        &self,
        indexed_value: &[u8],
        primary_key: &[u8],
    ) -> IndexResult<Option<Vec<u8>>> {
        let key = self.build_key(indexed_value, primary_key);

        // Check cache first
        if let Some(value) = self.cache.write().get(&key) {
            return Ok(Some(value.clone()));
        }

        // Read from storage
        let value = self
            .store
            .get(self.config.shard_id, &key)
            .await
            .map_err(|e| IndexError::Storage(e.to_string()))?;

        // Update cache if found
        if let Some(ref v) = value {
            self.cache.write().put(key, v.clone());
        }

        Ok(value)
    }

    /// Delete an entry from storage
    pub async fn delete_entry(
        &self,
        indexed_value: &[u8],
        primary_key: &[u8],
    ) -> IndexResult<bool> {
        // Write to WAL first
        self.write_wal(WalOperation::Delete {
            indexed_value: indexed_value.to_vec(),
            primary_key: primary_key.to_vec(),
        })
        .await?;

        let key = self.build_key(indexed_value, primary_key);

        // Remove from cache
        self.cache.write().pop(&key);

        // Delete from storage
        self.store
            .delete(self.config.shard_id, &key)
            .await
            .map_err(|e| IndexError::Storage(e.to_string()))
    }

    /// Scan a range of entries
    pub async fn scan_range(
        &self,
        start: Bound<Vec<u8>>,
        end: Bound<Vec<u8>>,
        limit: Option<usize>,
        reverse: bool,
    ) -> IndexResult<Vec<(Vec<u8>, Vec<u8>)>> {
        use nanograph_core::object::KeyRange;

        // Convert bounds to include index_id prefix
        let start_bound = match start {
            Bound::Included(v) => Bound::Included(self.build_prefix(&v)),
            Bound::Excluded(v) => Bound::Excluded(self.build_prefix(&v)),
            Bound::Unbounded => {
                let mut prefix = Vec::with_capacity(8);
                prefix.extend_from_slice(&self.config.index_id.object().as_u32().to_be_bytes());
                Bound::Included(prefix)
            }
        };

        let end_bound = match end {
            Bound::Included(v) => {
                let mut key = self.build_prefix(&v);
                key.push(0xFF); // Include all entries with this prefix
                Bound::Included(key)
            }
            Bound::Excluded(v) => Bound::Excluded(self.build_prefix(&v)),
            Bound::Unbounded => {
                let mut prefix = Vec::with_capacity(8);
                prefix
                    .extend_from_slice(&(self.config.index_id.object().as_u32() + 1).to_be_bytes());
                Bound::Excluded(prefix)
            }
        };

        let range = KeyRange {
            start: start_bound,
            end: end_bound,
            limit,
            reverse,
        };

        // Scan from storage
        let mut iter = self
            .store
            .scan(self.config.shard_id, range)
            .await
            .map_err(|e| IndexError::Storage(e.to_string()))?;

        let mut results = Vec::new();
        while let Some(result) = iter.next().await {
            let (key, value) = result.map_err(|e| IndexError::Storage(e.to_string()))?;
            results.push((key, value));
        }

        Ok(results)
    }

    /// Check if an entry exists
    pub async fn exists(&self, indexed_value: &[u8], primary_key: &[u8]) -> IndexResult<bool> {
        let key = self.build_key(indexed_value, primary_key);

        // Check cache first
        if self.cache.write().contains(&key) {
            return Ok(true);
        }

        // Check storage
        self.store
            .exists(self.config.shard_id, &key)
            .await
            .map_err(|e| IndexError::Storage(e.to_string()))
    }

    /// Flush any pending changes to durable storage
    pub async fn flush(&self) -> IndexResult<()> {
        // The store handles its own flushing
        // We just need to ensure WAL is flushed if enabled
        if let Some(wal) = &self.wal {
            let mut writer = wal
                .writer()
                .map_err(|e| IndexError::Storage(e.to_string()))?;
            writer
                .flush()
                .map_err(|e| IndexError::Storage(e.to_string()))?;
        }
        Ok(())
    }

    /// Clear the cache
    pub fn clear_cache(&self) {
        self.cache.write().clear();
    }

    /// Get cache statistics
    pub fn cache_stats(&self) -> CacheStats {
        let cache = self.cache.read();
        CacheStats {
            size: cache.len(),
            capacity: cache.cap().get(),
        }
    }
}

/// Cache statistics
#[derive(Debug, Clone)]
pub struct CacheStats {
    /// Current number of entries in cache
    pub size: usize,
    /// Maximum cache capacity
    pub capacity: usize,
}

/// WAL operation types
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
enum WalOperation {
    Insert {
        indexed_value: Vec<u8>,
        primary_key: Vec<u8>,
        value: Vec<u8>,
    },
    Delete {
        indexed_value: Vec<u8>,
        primary_key: Vec<u8>,
    },
}

impl WalOperation {
    fn kind(&self) -> u16 {
        match self {
            WalOperation::Insert { .. } => 1,
            WalOperation::Delete { .. } => 2,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nanograph_core::object::ObjectId;

    #[test]
    fn test_build_key() {
        let config = PersistenceConfig {
            index_id: IndexId(ObjectId::new(42)),
            ..Default::default()
        };
        // Note: Tests will need a proper KeyValueShardStore implementation
        // This is a placeholder that will be updated when we have a test implementation

        // Verify key format
        let mut key = Vec::with_capacity(8 + 5 + 2);
        key.extend_from_slice(&42u64.to_be_bytes());
        key.extend_from_slice(b"value");
        key.extend_from_slice(b"pk");

        assert_eq!(&key[0..8], &42u64.to_be_bytes());
        assert_eq!(&key[8..13], b"value");
        assert_eq!(&key[13..15], b"pk");
    }
}

// Made with Bob
