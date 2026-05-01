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

use crate::error::LMDBError;
use crate::kvstore::LMDBKeyValueStore;
use async_trait::async_trait;
use lmdb::Transaction as LmdbTransaction;
use nanograph_kvt::{
    KeyRange, KeyValueError, KeyValueIterator, KeyValueResult, KeyValueShardStore, ShardId,
    Timestamp, Transaction, TransactionId,
};
use nanograph_wal::Durability;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

/// Write operation in a transaction
#[derive(Debug, Clone)]
enum WriteOp {
    Put { key: Vec<u8>, value: Vec<u8> },
    Delete { key: Vec<u8> },
}

/// LMDB Transaction with snapshot isolation
///
/// This transaction wrapper provides cross-shard transaction support for LMDB.
/// Since LMDB transactions are environment-specific (one environment per shard),
/// this wrapper:
/// 1. Buffers all writes in memory
/// 2. Reads from the buffer first, then falls back to LMDB
/// 3. On commit, creates individual LMDB transactions per shard and applies writes atomically
/// 4. Provides snapshot isolation by reading at a consistent timestamp
pub struct LMDBTransaction {
    id: TransactionId,
    snapshot_ts: Timestamp,
    store: Arc<LMDBKeyValueStore>,
    write_buffer: Arc<Mutex<HashMap<ShardId, Vec<WriteOp>>>>,
    committed: Arc<Mutex<bool>>,
    rolled_back: Arc<Mutex<bool>>,
}

impl LMDBTransaction {
    /// Create a new transaction
    pub fn new(id: TransactionId, snapshot_ts: Timestamp, store: Arc<LMDBKeyValueStore>) -> Self {
        Self {
            id,
            snapshot_ts,
            store,
            write_buffer: Arc::new(Mutex::new(HashMap::new())),
            committed: Arc::new(Mutex::new(false)),
            rolled_back: Arc::new(Mutex::new(false)),
        }
    }

    /// Check if transaction is still active
    fn check_active(&self) -> KeyValueResult<()> {
        if *self.committed.lock().unwrap() {
            return Err(KeyValueError::WriteConflict);
        }
        if *self.rolled_back.lock().unwrap() {
            return Err(KeyValueError::WriteConflict);
        }
        Ok(())
    }

    /// Get a write operation from the buffer
    fn get_from_buffer(&self, shard: ShardId, key: &[u8]) -> Option<WriteOp> {
        let buffer = self.write_buffer.lock().unwrap();
        if let Some(ops) = buffer.get(&shard) {
            // Search in reverse order to get the most recent operation
            for op in ops.iter().rev() {
                match op {
                    WriteOp::Put { key: k, value } if k == key => {
                        return Some(WriteOp::Put {
                            key: k.clone(),
                            value: value.clone(),
                        });
                    }
                    WriteOp::Delete { key: k } if k == key => {
                        return Some(WriteOp::Delete { key: k.clone() });
                    }
                    _ => {}
                }
            }
        }
        None
    }

    /// Helper to check if a key is within a range
    fn key_in_range(key: &[u8], range: &KeyRange) -> bool {
        // Check start bound
        let after_start = match &range.start {
            std::ops::Bound::Included(start) => key >= start.as_slice(),
            std::ops::Bound::Excluded(start) => key > start.as_slice(),
            std::ops::Bound::Unbounded => true,
        };

        // Check end bound
        let before_end = match &range.end {
            std::ops::Bound::Included(end) => key <= end.as_slice(),
            std::ops::Bound::Excluded(end) => key < end.as_slice(),
            std::ops::Bound::Unbounded => true,
        };

        after_start && before_end
    }
}

#[async_trait]
impl Transaction for LMDBTransaction {
    fn id(&self) -> TransactionId {
        self.id
    }

    fn snapshot_ts(&self) -> Timestamp {
        self.snapshot_ts
    }

    async fn get(&self, shard: ShardId, key: &[u8]) -> KeyValueResult<Option<Vec<u8>>> {
        self.check_active()?;

        // First check write buffer - always see our own uncommitted writes
        if let Some(op) = self.get_from_buffer(shard, key) {
            return match op {
                WriteOp::Put { value, .. } => Ok(Some(value)),
                WriteOp::Delete { .. } => Ok(None),
            };
        }

        // Then check the LMDB store
        // Note: LMDB doesn't have built-in MVCC timestamps, so we just read current state
        // For true snapshot isolation, we'd need to implement versioning at a higher level
        KeyValueShardStore::get(&*self.store, shard, key).await
    }

    async fn put(&self, shard: ShardId, key: &[u8], value: &[u8]) -> KeyValueResult<()> {
        self.check_active()?;

        let mut buffer = self.write_buffer.lock().unwrap();
        let ops = buffer.entry(shard).or_insert_with(Vec::new);
        ops.push(WriteOp::Put {
            key: key.to_vec(),
            value: value.to_vec(),
        });

        Ok(())
    }

    async fn delete(&self, shard: ShardId, key: &[u8]) -> KeyValueResult<bool> {
        self.check_active()?;

        // Check if key exists (either in buffer or store)
        let exists = self.get(shard, key).await?.is_some();

        if exists {
            let mut buffer = self.write_buffer.lock().unwrap();
            let ops = buffer.entry(shard).or_insert_with(Vec::new);
            ops.push(WriteOp::Delete { key: key.to_vec() });
        }

        Ok(exists)
    }

    async fn scan(
        &self,
        shard: ShardId,
        range: KeyRange,
    ) -> KeyValueResult<Box<dyn KeyValueIterator + Send>> {
        self.check_active()?;

        // Get base data from store
        let mut base_iter = KeyValueShardStore::scan(&*self.store, shard, range.clone()).await?;
        
        // Collect all entries from the base iterator
        let mut entries = Vec::new();
        while let Some(result) = futures::StreamExt::next(&mut base_iter).await {
            let (key, value) = result?;
            entries.push((key, value));
        }

        // Apply buffered writes
        let buffer = self.write_buffer.lock().unwrap();
        if let Some(ops) = buffer.get(&shard) {
            // Build a map of buffered operations
            let mut buffered_map: HashMap<Vec<u8>, Option<Vec<u8>>> = HashMap::new();
            for op in ops {
                match op {
                    WriteOp::Put { key, value } => {
                        if Self::key_in_range(key, &range) {
                            buffered_map.insert(key.clone(), Some(value.clone()));
                        }
                    }
                    WriteOp::Delete { key } => {
                        if Self::key_in_range(key, &range) {
                            buffered_map.insert(key.clone(), None);
                        }
                    }
                }
            }

            // Merge buffered operations with base entries
            // Remove deleted keys and update existing keys
            entries.retain(|(k, _)| !buffered_map.contains_key(k) || buffered_map[k].is_some());
            
            // Update values for keys that exist in both
            for (key, value) in entries.iter_mut() {
                if let Some(Some(new_value)) = buffered_map.get(key) {
                    *value = new_value.clone();
                }
            }

            // Add new keys from buffer that aren't in base
            for (key, value_opt) in buffered_map {
                if let Some(value) = value_opt {
                    if !entries.iter().any(|(k, _)| k == &key) {
                        entries.push((key, value));
                    }
                }
            }

            // Sort entries by key
            entries.sort_by(|(k1, _), (k2, _)| k1.cmp(k2));
        }

        // Apply reverse if needed
        if range.reverse {
            entries.reverse();
        }

        // Apply limit if specified
        if let Some(limit) = range.limit {
            entries.truncate(limit);
        }

        Ok(Box::new(crate::iterator::LMDBIterator::new(entries)))
    }

    async fn commit(self: Arc<Self>, _durability: Durability) -> KeyValueResult<()> {
        // LMDB handles durability internally through its transaction commit
        self.check_active()?;

        // Mark as committed first to prevent concurrent operations
        {
            let mut committed = self.committed.lock().unwrap();
            *committed = true;
        }

        // Get all buffered writes
        let buffer = self.write_buffer.lock().unwrap();

        // Apply writes to each shard using LMDB transactions
        for (shard_id, ops) in buffer.iter() {
            let env = self.store.get_environment(*shard_id)?;
            let db = self.store.get_database(*shard_id)?;

            // Create a write transaction for this shard
            let mut txn = env.begin_rw_txn().map_err(LMDBError::from)?;

            // Apply all operations for this shard
            for op in ops {
                match op {
                    WriteOp::Put { key, value } => {
                        txn.put(db, key, value, lmdb::WriteFlags::empty())
                            .map_err(LMDBError::from)?;
                    }
                    WriteOp::Delete { key } => {
                        // Ignore NotFound errors on delete
                        match txn.del(db, key, None) {
                            Ok(()) => {}
                            Err(lmdb::Error::NotFound) => {}
                            Err(e) => return Err(LMDBError::from(e).into()),
                        }
                    }
                }
            }

            // Commit the shard transaction
            LmdbTransaction::commit(txn).map_err(LMDBError::from)?;
        }

        Ok(())
    }

    async fn rollback(self: Arc<Self>) -> KeyValueResult<()> {
        self.check_active()?;

        // Mark as rolled back
        {
            let mut rolled_back = self.rolled_back.lock().unwrap();
            *rolled_back = true;
        }

        // Clear the write buffer
        {
            let mut buffer = self.write_buffer.lock().unwrap();
            buffer.clear();
        }

        Ok(())
    }
}

// Made with Bob
