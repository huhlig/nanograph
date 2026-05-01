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

use crate::{KeyValueIterator, KeyValueResult};
use async_trait::async_trait;
use nanograph_core::object::{KeyRange, ShardId};
use nanograph_core::types::Timestamp;
use nanograph_wal::Durability;
use std::sync::Arc;

/// Transaction identifier for Multiversion Concurrency Control (MVCC)
#[derive(Copy, Clone, Debug, Ord, PartialOrd, Eq, PartialEq, Hash)]
pub struct TransactionId(pub u64);

/// Transaction interface for ACID operations
///
/// Transactions provide snapshot isolation and atomic commit/rollback.
#[async_trait]
pub trait Transaction: Send + Sync {
    /// Get the transaction ID
    fn id(&self) -> TransactionId;

    /// Get the snapshot timestamp
    fn snapshot_ts(&self) -> Timestamp;

    /// Get a value within this transaction
    async fn get(&self, shard: ShardId, key: &[u8]) -> KeyValueResult<Option<Vec<u8>>>;

    /// Put a key-value pair within this transaction
    async fn put(&self, shard: ShardId, key: &[u8], value: &[u8]) -> KeyValueResult<()>;

    /// Delete a key within this transaction
    async fn delete(&self, shard: ShardId, key: &[u8]) -> KeyValueResult<bool>;

    /// Scan within this transaction
    async fn scan(
        &self,
        shard: ShardId,
        range: KeyRange,
    ) -> KeyValueResult<Box<dyn KeyValueIterator + Send>>;

    /// Commit the transaction with specified durability level
    ///
    /// # Arguments
    /// * `durability` - The durability guarantee for this commit:
    ///   - `Sync`: fsync the WAL before returning (default, strongest guarantee)
    ///   - `Buffered`: return after WAL append; group-commit fsync on a timer
    ///   - `None`: no durability; useful for tests and bulk loads
    ///
    /// Returns an error if there are write conflicts.
    async fn commit(self: Arc<Self>, durability: Durability) -> KeyValueResult<()>;

    /// Rollback the transaction
    async fn rollback(self: Arc<Self>) -> KeyValueResult<()>;
}
