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

use crate::kviter::KeyValueIterator;
use crate::metrics::ShardStats;
use crate::result::KeyValueResult;
use crate::transaction::Transaction;
use crate::types::KeyRange;
use async_trait::async_trait;
use nanograph_core::types::{ShardId, ShardIndex, TableId};
use std::ops::Bound;
use std::sync::Arc;

/// Core Key-Value Operations
///
/// This trait defines the fundamental storage operations for a key-value store.
/// All operations are async and support MVCC through optional transaction contexts.
#[async_trait]
pub trait KeyValueShardStore: Send + Sync {
    // ===== Basic Operations =====

    /// Get a value by key
    ///
    /// Returns `None` if the key doesn't exist.
    /// Reads are performed at the current timestamp or within a transaction context.
    async fn get(&self, shard: ShardId, key: &[u8]) -> KeyValueResult<Option<Vec<u8>>>;

    /// Put a key-value pair
    ///
    /// Overwrites any existing value. The write is buffered if within a transaction,
    /// otherwise it's immediately committed.
    async fn put(&self, shard: ShardId, key: &[u8], value: &[u8]) -> KeyValueResult<()>;

    /// Delete a key
    ///
    /// Returns `true` if the key existed and was deleted, `false` if it didn't exist.
    async fn delete(&self, shard: ShardId, key: &[u8]) -> KeyValueResult<bool>;

    /// Check if key exists
    ///
    /// More efficient than `get()` when you only need to check existence.
    async fn exists(&self, shard: ShardId, key: &[u8]) -> KeyValueResult<bool>;

    // ===== Batch Operations =====

    /// Batch get multiple keys
    ///
    /// Returns a vector with the same length as `keys`, where each element is
    /// `Some(value)` if the key exists or `None` if it doesn't.
    async fn batch_get(
        &self,
        shard: ShardId,
        keys: &[&[u8]],
    ) -> KeyValueResult<Vec<Option<Vec<u8>>>>;

    /// Batch put multiple key-value pairs
    ///
    /// All writes are applied atomically within a transaction.
    async fn batch_put(&self, shard: ShardId, pairs: &[(&[u8], &[u8])]) -> KeyValueResult<()>;

    /// Batch delete multiple keys
    ///
    /// Returns the number of keys that were actually deleted.
    async fn batch_delete(&self, shard: ShardId, keys: &[&[u8]]) -> KeyValueResult<usize>;

    // ===== Range Operations =====

    /// Range scan with optional bounds
    ///
    /// Returns an iterator over key-value pairs within the specified range.
    /// The iterator provides a consistent snapshot view.
    async fn scan(
        &self,
        shard: ShardId,
        range: KeyRange,
    ) -> KeyValueResult<Box<dyn KeyValueIterator + Send>>;

    /// Prefix scan - get all keys with a given prefix
    ///
    /// Convenience method for common prefix queries.
    async fn scan_prefix(
        &self,
        shard: ShardId,
        prefix: &[u8],
        limit: Option<usize>,
    ) -> KeyValueResult<Box<dyn KeyValueIterator + Send>> {
        let mut end = prefix.to_vec();
        // Increment the last byte to create an exclusive upper bound
        if let Some(last) = end.last_mut() {
            if *last < 255 {
                *last += 1;
            } else {
                end.push(0);
            }
        }

        self.scan(
            shard,
            KeyRange {
                start: Bound::Included(prefix.to_vec()),
                end: Bound::Excluded(end),
                limit,
                reverse: false,
            },
        )
        .await
    }

    // ===== Statistics & Metadata =====

    /// Get an approximate count of keys in the table
    async fn key_count(&self, shard: ShardId) -> KeyValueResult<u64>;

    /// Get comprehensive table statistics
    async fn shard_stats(&self, shard: ShardId) -> KeyValueResult<ShardStats>;

    // ===== Transaction Support =====

    /// Begin a new transaction
    ///
    /// Returns a transaction handle that can be used for transactional operations.
    /// The transaction provides snapshot isolation.
    async fn begin_transaction(&self) -> KeyValueResult<Arc<dyn Transaction>>;

    // ===== Shard Management =====

    /// Create a new shard with explicit table and index
    ///
    /// The ShardId is deterministically derived from TableId and ShardIndex.
    /// The KeyValueDatabaseManager is responsible for:
    /// - Ensuring TableId uniqueness across the cluster (via consensus)
    /// - Coordinating shard creation across replicas
    /// - Managing shard assignments to servers
    ///
    /// Storage engines simply create the physical storage for the given ShardId.
    async fn create_shard(&self, table: TableId, index: ShardIndex) -> KeyValueResult<ShardId>;

    /// Drop a shard and all its data
    async fn drop_shard(&self, shard: ShardId) -> KeyValueResult<()>;

    /// List all shards
    async fn list_shards(&self) -> KeyValueResult<Vec<ShardId>>;

    /// Check if a shard exists
    async fn shard_exists(&self, shard: ShardId) -> KeyValueResult<bool>;

    // ===== Maintenance Operations =====

    /// Flush any pending writes to durable storage
    async fn flush(&self) -> KeyValueResult<()>;

    /// Trigger compaction (for LSM-based stores)
    async fn compact(&self, shard: Option<ShardId>) -> KeyValueResult<()>;
}
