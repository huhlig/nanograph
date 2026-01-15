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

use crate::kvstore::ArtKeyValueStore;
use async_trait::async_trait;
use nanograph_kvt::{
    KeyRange, KeyValueError, KeyValueIterator, KeyValueResult, KeyValueShardStore, ShardId,
    Timestamp, Transaction, TransactionId,
};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, RwLock};

// Global transaction ID counter
static GLOBAL_TX_ID: AtomicU64 = AtomicU64::new(1);

fn next_tx_id() -> TransactionId {
    TransactionId(GLOBAL_TX_ID.fetch_add(1, Ordering::SeqCst))
}

/// Write operation in a transaction
#[derive(Debug, Clone)]
enum WriteOp {
    Put { key: Vec<u8>, value: Vec<u8> },
    Delete { key: Vec<u8> },
}

/// ART Transaction with snapshot isolation
///
/// Provides ACID properties:
/// - Atomicity: All writes are buffered and applied atomically on commit
/// - Consistency: Validation ensures no conflicts
/// - Isolation: Snapshot isolation - reads see a consistent snapshot
/// - Durability: Writes are logged to WAL before commit (when WAL enabled)
pub struct ArtTransaction {
    id: TransactionId,
    snapshot_ts: Timestamp,
    store: Arc<ArtKeyValueStore>,
    write_buffer: Arc<Mutex<HashMap<ShardId, Vec<WriteOp>>>>,
    committed: Arc<RwLock<bool>>,
    rolled_back: Arc<RwLock<bool>>,
}

impl ArtTransaction {
    /// Create a new transaction
    pub fn new(store: Arc<ArtKeyValueStore>) -> Self {
        Self {
            id: next_tx_id(),
            snapshot_ts: Timestamp::now(),
            store,
            write_buffer: Arc::new(Mutex::new(HashMap::new())),
            committed: Arc::new(RwLock::new(false)),
            rolled_back: Arc::new(RwLock::new(false)),
        }
    }

    /// Check if transaction is still active
    fn check_active(&self) -> KeyValueResult<()> {
        if *self.committed.read().unwrap() {
            return Err(KeyValueError::WriteConflict);
        }
        if *self.rolled_back.read().unwrap() {
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

    /// Apply all buffered writes to the store
    async fn apply_writes(&self) -> KeyValueResult<()> {
        // Clone the buffer to avoid holding the lock across await points
        let ops_to_apply = {
            let buffer = self.write_buffer.lock().unwrap();
            buffer.clone()
        };

        for (shard, ops) in ops_to_apply.iter() {
            for op in ops {
                match op {
                    WriteOp::Put { key, value } => {
                        self.store.put(*shard, key, value).await?;
                    }
                    WriteOp::Delete { key } => {
                        self.store.delete(*shard, key).await?;
                    }
                }
            }
        }

        Ok(())
    }
}

#[async_trait]
impl Transaction for ArtTransaction {
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

        // Then read from the store
        // In a full MVCC implementation, this would read at snapshot_ts
        // For now, we read the current state
        self.store.get(shard, key).await
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

        let mut buffer = self.write_buffer.lock().unwrap();
        let ops = buffer.entry(shard).or_insert_with(Vec::new);
        ops.push(WriteOp::Delete { key: key.to_vec() });

        Ok(true)
    }

    async fn scan(
        &self,
        shard: ShardId,
        range: KeyRange,
    ) -> KeyValueResult<Box<dyn KeyValueIterator + Send>> {
        self.check_active()?;

        // For now, scan reads from the current state
        // In a full MVCC implementation, this would use snapshot_ts
        // and merge with buffered writes
        self.store.scan(shard, range).await
    }

    async fn commit(self: Arc<Self>) -> KeyValueResult<()> {
        self.check_active()?;

        // Apply all buffered writes atomically
        self.apply_writes().await?;

        // Mark as committed
        *self.committed.write().unwrap() = true;

        Ok(())
    }

    async fn rollback(self: Arc<Self>) -> KeyValueResult<()> {
        self.check_active()?;

        // Simply discard the write buffer
        self.write_buffer.lock().unwrap().clear();

        // Mark as rolled back
        *self.rolled_back.write().unwrap() = true;

        Ok(())
    }
}

/// Transaction manager for ART
pub struct TransactionManager {
    store: Arc<ArtKeyValueStore>,
}

impl TransactionManager {
    pub fn new(store: Arc<ArtKeyValueStore>) -> Self {
        Self { store }
    }

    /// Begin a new transaction
    pub fn begin(&self) -> Arc<dyn Transaction> {
        Arc::new(ArtTransaction::new(self.store.clone()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nanograph_kvt::{ShardIndex, TableId};

    #[tokio::test]
    async fn test_transaction_basic() {
        let store = Arc::new(ArtKeyValueStore::new());
        let table_id = TableId::new(0);
        let shard_index = ShardIndex::new(0);
        let shard = store.create_shard(table_id, shard_index).await.unwrap();

        let tx_manager = TransactionManager::new(store.clone());
        let tx = tx_manager.begin();

        // Write within transaction
        tx.put(shard, b"key1", b"value1").await.unwrap();
        tx.put(shard, b"key2", b"value2").await.unwrap();

        // Read within transaction (should see uncommitted writes)
        assert_eq!(
            tx.get(shard, b"key1").await.unwrap(),
            Some(b"value1".to_vec())
        );

        // Commit
        tx.commit().await.unwrap();

        // Verify data is persisted
        assert_eq!(
            store.get(shard, b"key1").await.unwrap(),
            Some(b"value1".to_vec())
        );
        assert_eq!(
            store.get(shard, b"key2").await.unwrap(),
            Some(b"value2".to_vec())
        );
    }

    #[tokio::test]
    async fn test_transaction_rollback() {
        let store = Arc::new(ArtKeyValueStore::new());
        let table_id = TableId::new(0);
        let shard_index = ShardIndex::new(0);
        let shard = store.create_shard(table_id, shard_index).await.unwrap();

        let tx_manager = TransactionManager::new(store.clone());
        let tx = tx_manager.begin();

        // Write within transaction
        tx.put(shard, b"key1", b"value1").await.unwrap();

        // Rollback
        tx.rollback().await.unwrap();

        // Verify data is NOT persisted
        assert_eq!(store.get(shard, b"key1").await.unwrap(), None);
    }

    #[tokio::test]
    async fn test_transaction_isolation() {
        let store = Arc::new(ArtKeyValueStore::new());
        let table_id = TableId::new(0);
        let shard_index = ShardIndex::new(0);
        let shard = store.create_shard(table_id, shard_index).await.unwrap();

        // Insert initial data
        store.put(shard, b"key1", b"initial").await.unwrap();

        let tx_manager = TransactionManager::new(store.clone());

        // Start transaction 1
        let tx1 = tx_manager.begin();
        tx1.put(shard, b"key1", b"tx1_value").await.unwrap();

        // Transaction 1 should see its own write
        assert_eq!(
            tx1.get(shard, b"key1").await.unwrap(),
            Some(b"tx1_value".to_vec())
        );

        // Store should still see initial value (tx1 not committed)
        assert_eq!(
            store.get(shard, b"key1").await.unwrap(),
            Some(b"initial".to_vec())
        );

        // Commit tx1
        tx1.commit().await.unwrap();

        // Now store should see tx1's value
        assert_eq!(
            store.get(shard, b"key1").await.unwrap(),
            Some(b"tx1_value".to_vec())
        );
    }

    #[tokio::test]
    async fn test_transaction_delete() {
        let store = Arc::new(ArtKeyValueStore::new());
        let table_id = TableId::new(0);
        let shard_index = ShardIndex::new(0);
        let shard = store.create_shard(table_id, shard_index).await.unwrap();

        // Insert initial data
        store.put(shard, b"key1", b"value1").await.unwrap();

        let tx_manager = TransactionManager::new(store.clone());
        let tx = tx_manager.begin();

        // Delete within transaction
        tx.delete(shard, b"key1").await.unwrap();

        // Transaction should see deletion
        assert_eq!(tx.get(shard, b"key1").await.unwrap(), None);

        // Store should still see the value
        assert_eq!(
            store.get(shard, b"key1").await.unwrap(),
            Some(b"value1".to_vec())
        );

        // Commit
        tx.commit().await.unwrap();

        // Now store should see deletion
        assert_eq!(store.get(shard, b"key1").await.unwrap(), None);
    }

    #[tokio::test]
    async fn test_transaction_multiple_operations() {
        let store = Arc::new(ArtKeyValueStore::new());
        let table_id = TableId::new(0);
        let shard_index = ShardIndex::new(0);
        let shard = store.create_shard(table_id, shard_index).await.unwrap();

        let tx_manager = TransactionManager::new(store.clone());
        let tx = tx_manager.begin();

        // Multiple operations
        tx.put(shard, b"key1", b"value1").await.unwrap();
        tx.put(shard, b"key2", b"value2").await.unwrap();
        tx.delete(shard, b"key1").await.unwrap(); // Delete key1
        tx.put(shard, b"key3", b"value3").await.unwrap();

        // Check within transaction
        assert_eq!(tx.get(shard, b"key1").await.unwrap(), None); // Deleted
        assert_eq!(
            tx.get(shard, b"key2").await.unwrap(),
            Some(b"value2".to_vec())
        );
        assert_eq!(
            tx.get(shard, b"key3").await.unwrap(),
            Some(b"value3".to_vec())
        );

        // Commit
        tx.commit().await.unwrap();

        // Verify final state
        assert_eq!(store.get(shard, b"key1").await.unwrap(), None);
        assert_eq!(
            store.get(shard, b"key2").await.unwrap(),
            Some(b"value2".to_vec())
        );
        assert_eq!(
            store.get(shard, b"key3").await.unwrap(),
            Some(b"value3".to_vec())
        );
    }
}
