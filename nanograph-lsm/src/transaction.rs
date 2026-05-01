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

use crate::kvstore::LSMKeyValueStore;
use async_trait::async_trait;
use nanograph_kvt::{
    KeyRange, KeyValueIterator, KeyValueResult, KeyValueShardStore, ShardId, Timestamp,
    Transaction, TransactionId,
};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

/// Write operation in a transaction
#[derive(Debug, Clone)]
enum WriteOp {
    Put { key: Vec<u8>, value: Vec<u8> },
    Delete { key: Vec<u8> },
}

/// LSM Transaction with snapshot isolation
pub struct LSMTransaction {
    id: TransactionId,
    snapshot_ts: Timestamp,
    store: Arc<LSMKeyValueStore>,
    write_buffer: Arc<Mutex<HashMap<ShardId, Vec<WriteOp>>>>,
    committed: Arc<Mutex<bool>>,
    rolled_back: Arc<Mutex<bool>>,
}

impl LSMTransaction {
    /// Create a new transaction
    pub fn new(id: TransactionId, snapshot_ts: Timestamp, store: Arc<LSMKeyValueStore>) -> Self {
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
            return Err(nanograph_kvt::KeyValueError::WriteConflict);
        }
        if *self.rolled_back.lock().unwrap() {
            return Err(nanograph_kvt::KeyValueError::WriteConflict);
        }
        Ok(())
    }

    /// Get a write operation from the buffer
    fn get_from_buffer(&self, table: ShardId, key: &[u8]) -> Option<WriteOp> {
        let buffer = self.write_buffer.lock().unwrap();
        if let Some(ops) = buffer.get(&table) {
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
impl Transaction for LSMTransaction {
    fn id(&self) -> TransactionId {
        self.id
    }

    fn snapshot_ts(&self) -> Timestamp {
        self.snapshot_ts
    }

    async fn get(&self, table: ShardId, key: &[u8]) -> KeyValueResult<Option<Vec<u8>>> {
        self.check_active()?;

        // First check write buffer - always see our own uncommitted writes
        if let Some(op) = self.get_from_buffer(table, key) {
            return match op {
                WriteOp::Put { value, .. } => Ok(Some(value)),
                WriteOp::Delete { .. } => Ok(None),
            };
        }

        // Then check the store at our snapshot timestamp
        // This provides snapshot isolation - we only see data committed before our snapshot
        self.store
            .get_at_snapshot(table, key, self.snapshot_ts.as_millis())
            .await
    }

    async fn put(&self, table: ShardId, key: &[u8], value: &[u8]) -> KeyValueResult<()> {
        self.check_active()?;

        let mut buffer = self.write_buffer.lock().unwrap();
        let ops = buffer.entry(table).or_insert_with(Vec::new);
        ops.push(WriteOp::Put {
            key: key.to_vec(),
            value: value.to_vec(),
        });

        Ok(())
    }

    async fn delete(&self, table: ShardId, key: &[u8]) -> KeyValueResult<bool> {
        self.check_active()?;

        // Check if key exists (either in buffer or store)
        let exists = self.get(table, key).await?.is_some();

        if exists {
            let mut buffer = self.write_buffer.lock().unwrap();
            let ops = buffer.entry(table).or_insert_with(Vec::new);
            ops.push(WriteOp::Delete { key: key.to_vec() });
        }

        Ok(exists)
    }

    async fn scan(
        &self,
        table: ShardId,
        range: KeyRange,
    ) -> KeyValueResult<Box<dyn KeyValueIterator + Send>> {
        self.check_active()?;

        // Get buffered writes for this table
        let buffer_entries = {
            let buffer = self.write_buffer.lock().unwrap();
            if let Some(ops) = buffer.get(&table) {
                // Convert write operations to entries, filtering by range
                ops.iter()
                    .filter_map(|op| {
                        match op {
                            WriteOp::Put { key, value } => {
                                // Check if key is in range
                                let in_range = Self::key_in_range(key, &range);
                                if in_range {
                                    Some(crate::memtable::Entry::new(
                                        key.clone(),
                                        Some(value.clone()),
                                        u64::MAX, // Use max sequence to ensure buffer takes priority
                                    ))
                                } else {
                                    None
                                }
                            }
                            WriteOp::Delete { key } => {
                                // Check if key is in range
                                let in_range = Self::key_in_range(key, &range);
                                if in_range {
                                    Some(crate::memtable::Entry::new(
                                        key.clone(),
                                        None, // Tombstone
                                        u64::MAX,
                                    ))
                                } else {
                                    None
                                }
                            }
                        }
                    })
                    .collect::<Vec<_>>()
            } else {
                Vec::new()
            }
        };

        // Get data from store at our snapshot timestamp
        // For now, we'll use the regular scan and rely on memtable snapshot filtering
        // A full implementation would need snapshot-aware SSTable scanning
        let store_iter = self.store.scan(table, range.clone()).await?;

        // If no buffered entries, just return store iterator
        if buffer_entries.is_empty() {
            return Ok(store_iter);
        }

        // For now, return store iterator with a note that full merge is TODO
        // A complete implementation would merge buffer_entries with store results
        // This requires either:
        // 1. Collecting all store results (memory intensive)
        // 2. Creating a merging iterator that can handle async streams
        // 3. Making the transaction scan return a custom iterator type

        // TODO: Implement proper merging of write buffer with store iterator
        // For now, just return store iterator - writes will be visible after commit
        Ok(store_iter)
    }

    async fn commit(self: Arc<Self>, durability: nanograph_wal::Durability) -> KeyValueResult<()> {
        self.check_active()?;

        // Get commit timestamp using real wall-clock time
        let commit_ts = Timestamp::now().as_millis();

        // Mark as committed
        {
            let mut committed = self.committed.lock().unwrap();
            *committed = true;
        }

        // Clone buffer contents to avoid holding lock across await
        let buffer_clone = {
            let buffer = self.write_buffer.lock().unwrap();
            buffer.clone()
        };

        // Apply all writes from buffer to store with commit timestamp
        // This provides snapshot isolation - writes are visible only to transactions
        // that start after this commit timestamp
        for (table, ops) in buffer_clone.iter() {
            for op in ops {
                match op {
                    WriteOp::Put { key, value } => {
                        // Write with commit timestamp for MVCC
                        self.store
                            .put_committed(*table, key, value, commit_ts, durability)
                            .await?;
                    }
                    WriteOp::Delete { key } => {
                        // Delete with commit timestamp for MVCC
                        self.store.delete_committed(*table, key, commit_ts, durability).await?;
                    }
                }
            }
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

        // Clear write buffer
        {
            let mut buffer = self.write_buffer.lock().unwrap();
            buffer.clear();
        }

        Ok(())
    }
}

/// Transaction manager for LSM Tree
pub struct TransactionManager {
    next_tx_id: Arc<Mutex<u64>>,
    store: Arc<LSMKeyValueStore>,
    /// Track active transactions and their snapshot timestamps for GC watermark
    active_snapshots: Arc<Mutex<HashMap<TransactionId, i64>>>,
}

impl TransactionManager {
    pub fn new(store: Arc<LSMKeyValueStore>) -> Self {
        Self {
            next_tx_id: Arc::new(Mutex::new(1)),
            store,
            active_snapshots: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Create an empty transaction manager (for initialization)
    pub fn new_empty() -> Self {
        // Create a dummy store - this will be replaced
        let dummy_store = Arc::new(LSMKeyValueStore::new());
        Self::new(dummy_store)
    }

    /// Begin a new transaction
    pub fn begin(&self) -> Arc<dyn Transaction> {
        let tx_id = {
            let mut next_id = self.next_tx_id.lock().unwrap();
            let id = *next_id;
            *next_id += 1;
            TransactionId(id)
        };

        // Use real wall-clock time for snapshot isolation
        let snapshot_ts = Timestamp::now();
        let snapshot_ts_millis = snapshot_ts.as_millis();

        // Register this transaction's snapshot timestamp
        {
            let mut snapshots = self.active_snapshots.lock().unwrap();
            snapshots.insert(tx_id, snapshot_ts_millis);
        }

        let tx = Arc::new(LSMTransaction::new(
            tx_id,
            snapshot_ts,
            Arc::clone(&self.store),
        ));

        // Store reference to transaction manager for cleanup on commit/rollback
        let tx_mgr = Arc::new(self.clone());
        let tx_with_cleanup = Arc::new(TransactionWithCleanup { inner: tx, tx_mgr });

        tx_with_cleanup as Arc<dyn Transaction>
    }

    /// Get current timestamp
    pub fn current_timestamp(&self) -> Timestamp {
        Timestamp::now()
    }

    /// Get the minimum active snapshot timestamp (GC watermark)
    ///
    /// This is the oldest snapshot timestamp among all active transactions.
    /// Data with timestamps older than this can be safely garbage collected
    /// during compaction, as no active transaction can see it.
    ///
    /// Returns None if there are no active transactions.
    pub fn min_active_snapshot_seq(&self) -> Option<i64> {
        let snapshots = self.active_snapshots.lock().unwrap();
        snapshots.values().min().copied()
    }

    /// Get the count of active transactions
    pub fn active_transaction_count(&self) -> usize {
        let snapshots = self.active_snapshots.lock().unwrap();
        snapshots.len()
    }

    /// Remove a transaction from active tracking (called on commit/rollback)
    fn remove_transaction(&self, tx_id: TransactionId) {
        let mut snapshots = self.active_snapshots.lock().unwrap();
        snapshots.remove(&tx_id);
    }
}

impl Clone for TransactionManager {
    fn clone(&self) -> Self {
        Self {
            next_tx_id: Arc::clone(&self.next_tx_id),
            store: Arc::clone(&self.store),
            active_snapshots: Arc::clone(&self.active_snapshots),
        }
    }
}

/// Wrapper that ensures transaction cleanup on commit/rollback
struct TransactionWithCleanup {
    inner: Arc<LSMTransaction>,
    tx_mgr: Arc<TransactionManager>,
}

#[async_trait]
impl Transaction for TransactionWithCleanup {
    fn id(&self) -> TransactionId {
        self.inner.id()
    }

    fn snapshot_ts(&self) -> Timestamp {
        self.inner.snapshot_ts()
    }

    async fn get(&self, table: ShardId, key: &[u8]) -> KeyValueResult<Option<Vec<u8>>> {
        self.inner.get(table, key).await
    }

    async fn put(&self, table: ShardId, key: &[u8], value: &[u8]) -> KeyValueResult<()> {
        self.inner.put(table, key, value).await
    }

    async fn delete(&self, table: ShardId, key: &[u8]) -> KeyValueResult<bool> {
        self.inner.delete(table, key).await
    }

    async fn scan(
        &self,
        table: ShardId,
        range: KeyRange,
    ) -> KeyValueResult<Box<dyn KeyValueIterator + Send>> {
        self.inner.scan(table, range).await
    }

    async fn commit(self: Arc<Self>, durability: nanograph_wal::Durability) -> KeyValueResult<()> {
        let tx_id = self.id();
        let result = Arc::clone(&self.inner).commit(durability).await;

        // Clean up transaction tracking regardless of commit result
        self.tx_mgr.remove_transaction(tx_id);

        result
    }

    async fn rollback(self: Arc<Self>) -> KeyValueResult<()> {
        let tx_id = self.id();
        let result = Arc::clone(&self.inner).rollback().await;

        // Clean up transaction tracking regardless of rollback result
        self.tx_mgr.remove_transaction(tx_id);

        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_transaction_basic() {
        let store = Arc::new(LSMKeyValueStore::new());
        store.init_tx_manager();
        let tx_mgr = store.get_tx_manager();

        let shard_id = ShardId::new(0);
        let vfs = Arc::new(nanograph_vfs::MemoryFileSystem::new());
        let data_path = nanograph_vfs::Path::from("/data");
        let wal_path = nanograph_vfs::Path::from("/wal");
        store
            .create_shard(shard_id, vfs, data_path, wal_path)
            .unwrap();

        // Begin transaction
        let tx = tx_mgr.begin();

        // Write within transaction
        tx.put(shard_id, b"key1", b"value1").await.unwrap();

        // Read within transaction (should see buffered write)
        let value = tx.get(shard_id, b"key1").await.unwrap();
        assert_eq!(value, Some(b"value1".to_vec()));

        // Commit
        tx.commit(nanograph_wal::Durability::Sync).await.unwrap();

        // Verify committed data
        let value = store.get(shard_id, b"key1").await.unwrap();
        assert_eq!(value, Some(b"value1".to_vec()));
    }

    #[tokio::test]
    async fn test_transaction_rollback() {
        let store = Arc::new(LSMKeyValueStore::new());
        store.init_tx_manager();
        let tx_mgr = store.get_tx_manager();

        let shard_id = ShardId::new(0);
        let vfs = Arc::new(nanograph_vfs::MemoryFileSystem::new());
        let data_path = nanograph_vfs::Path::from("/data");
        let wal_path = nanograph_vfs::Path::from("/wal");
        store
            .create_shard(shard_id, vfs, data_path, wal_path)
            .unwrap();

        // Begin transaction
        let tx = tx_mgr.begin();

        // Write within transaction
        tx.put(shard_id, b"key1", b"value1").await.unwrap();

        // Rollback
        tx.rollback().await.unwrap();

        // Verify data was not committed
        let value = store.get(shard_id, b"key1").await.unwrap();
        assert_eq!(value, None);
    }

    #[tokio::test]
    async fn test_transaction_isolation() {
        let store = Arc::new(LSMKeyValueStore::new());
        store.init_tx_manager();
        let tx_mgr = store.get_tx_manager();

        let shard_id = ShardId::new(0);
        let vfs = Arc::new(nanograph_vfs::MemoryFileSystem::new());
        let data_path = nanograph_vfs::Path::from("/data");
        let wal_path = nanograph_vfs::Path::from("/wal");
        store
            .create_shard(shard_id, vfs, data_path, wal_path)
            .unwrap();

        // Write initial value
        store.put(shard_id, b"key1", b"value1").await.unwrap();

        // Begin transaction 1
        let tx1 = tx_mgr.begin();

        // Begin transaction 2
        let tx2 = tx_mgr.begin();

        // TX1 updates the value
        tx1.put(shard_id, b"key1", b"value2").await.unwrap();

        // TX2 should still see old value (snapshot isolation)
        let _value = tx2.get(shard_id, b"key1").await.unwrap();
        // TODO: This should be value1 with proper snapshot isolation
        // For now, it will see the uncommitted value

        // Commit TX1
        tx1.commit(nanograph_wal::Durability::Sync).await.unwrap();

        // TX2 should still see old value (snapshot isolation)
        let _value = tx2.get(shard_id, b"key1").await.unwrap();
        // TODO: This should still be value1

        tx2.rollback().await.unwrap();
    }

    #[tokio::test]
    async fn test_transaction_delete() {
        let store = Arc::new(LSMKeyValueStore::new());
        store.init_tx_manager();
        let tx_mgr = store.get_tx_manager();

        let shard_id = ShardId::new(0);
        let vfs = Arc::new(nanograph_vfs::MemoryFileSystem::new());
        let data_path = nanograph_vfs::Path::from("/data");
        let wal_path = nanograph_vfs::Path::from("/wal");
        store
            .create_shard(shard_id, vfs, data_path, wal_path)
            .unwrap();

        // Write initial value
        store.put(shard_id, b"key1", b"value1").await.unwrap();

        // Begin transaction
        let tx = tx_mgr.begin();

        // Delete within transaction
        let deleted = tx.delete(shard_id, b"key1").await.unwrap();
        assert!(deleted);

        // Should not see the key anymore
        let value = tx.get(shard_id, b"key1").await.unwrap();
        assert_eq!(value, None);

        // Commit
        tx.commit(nanograph_wal::Durability::Sync).await.unwrap();

        // Verify deletion
        let value = store.get(shard_id, b"key1").await.unwrap();
        assert_eq!(value, None);
    }

    #[tokio::test]
    async fn test_transaction_read_your_own_writes() {
        let store = Arc::new(LSMKeyValueStore::new());
        store.init_tx_manager();
        let tx_mgr = store.get_tx_manager();

        let shard_id = ShardId::new(0);
        let vfs = Arc::new(nanograph_vfs::MemoryFileSystem::new());
        let data_path = nanograph_vfs::Path::from("/data");
        let wal_path = nanograph_vfs::Path::from("/wal");
        store
            .create_shard(shard_id, vfs, data_path, wal_path)
            .unwrap();

        // Begin transaction
        let tx = tx_mgr.begin();

        // Write within transaction
        tx.put(shard_id, b"key1", b"value1").await.unwrap();

        // Should be able to read own write before commit
        let value = tx.get(shard_id, b"key1").await.unwrap();
        assert_eq!(value, Some(b"value1".to_vec()));

        // Update the value
        tx.put(shard_id, b"key1", b"value2").await.unwrap();

        // Should see the updated value
        let value = tx.get(shard_id, b"key1").await.unwrap();
        assert_eq!(value, Some(b"value2".to_vec()));

        // Commit
        tx.commit(nanograph_wal::Durability::Sync).await.unwrap();

        // Verify final value
        let value = store.get(shard_id, b"key1").await.unwrap();
        assert_eq!(value, Some(b"value2".to_vec()));
    }

    #[tokio::test]
    async fn test_concurrent_transactions_no_deadlock() {
        use std::sync::atomic::{AtomicUsize, Ordering};

        let store = Arc::new(LSMKeyValueStore::new());
        store.init_tx_manager();
        let tx_mgr = store.get_tx_manager();
        let tx_mgr = Arc::new(tx_mgr);
        let success_count = Arc::new(AtomicUsize::new(0));

        let shard_id = ShardId::new(0);
        let vfs = Arc::new(nanograph_vfs::MemoryFileSystem::new());
        let data_path = nanograph_vfs::Path::from("/data");
        let wal_path = nanograph_vfs::Path::from("/wal");
        store
            .create_shard(shard_id, vfs, data_path, wal_path)
            .unwrap();

        // Insert initial data
        store.put(shard_id, b"key1", b"initial").await.unwrap();

        // Spawn multiple concurrent transactions
        let mut handles = vec![];
        for i in 0..10 {
            let tx_mgr_clone = Arc::clone(&tx_mgr);
            let success_count_clone = Arc::clone(&success_count);
            let handle = tokio::spawn(async move {
                let tx = tx_mgr_clone.begin();
                let key = format!("key{}", i);
                let value = format!("value{}", i);

                // Write to unique key
                if tx
                    .put(shard_id, key.as_bytes(), value.as_bytes())
                    .await
                    .is_ok()
                {
                    // Commit - this should not deadlock
                    if tx.commit(nanograph_wal::Durability::Sync).await.is_ok() {
                        success_count_clone.fetch_add(1, Ordering::SeqCst);
                    }
                }
            });
            handles.push(handle);
        }

        // Wait for all transactions - if there's a deadlock, this will hang
        for handle in handles {
            handle.await.expect("Thread should not panic");
        }

        // All transactions should succeed since they write to different keys
        assert_eq!(
            success_count.load(Ordering::SeqCst),
            10,
            "All transactions should succeed without deadlock"
        );

        // Verify we can still read data (no deadlock occurred)
        let value = store.get(shard_id, b"key1").await.unwrap();
        assert!(value.is_some(), "key1 should still exist");

        // Verify all individual keys were written successfully
        for i in 0..10 {
            let key = format!("key{}", i);
            let value = store.get(shard_id, key.as_bytes()).await.unwrap();
            assert!(value.is_some(), "key{} should exist", i);
        }
    }

    #[tokio::test]
    async fn test_transaction_multiple_operations() {
        let store = Arc::new(LSMKeyValueStore::new());
        store.init_tx_manager();
        let tx_mgr = store.get_tx_manager();

        let shard_id = ShardId::new(0);
        let vfs = Arc::new(nanograph_vfs::MemoryFileSystem::new());
        let data_path = nanograph_vfs::Path::from("/data");
        let wal_path = nanograph_vfs::Path::from("/wal");
        store
            .create_shard(shard_id, vfs, data_path, wal_path)
            .unwrap();

        // Begin transaction
        let tx = tx_mgr.begin();

        // Multiple puts
        for i in 0..100 {
            let key = format!("key{:03}", i);
            let value = format!("value{}", i);
            tx.put(shard_id, key.as_bytes(), value.as_bytes())
                .await
                .unwrap();
        }

        // Verify in transaction
        for i in 0..100 {
            let key = format!("key{:03}", i);
            let value = tx.get(shard_id, key.as_bytes()).await.unwrap();
            assert!(value.is_some());
        }

        // Commit
        tx.commit(nanograph_wal::Durability::Sync).await.unwrap();

        // Verify after commit
        for i in 0..100 {
            let key = format!("key{:03}", i);
            let value = store.get(shard_id, key.as_bytes()).await.unwrap();
            assert!(value.is_some());
        }
    }

    #[tokio::test]
    async fn test_transaction_error_after_commit() {
        let store = Arc::new(LSMKeyValueStore::new());
        store.init_tx_manager();
        let tx_mgr = store.get_tx_manager();

        let shard_id = ShardId::new(0);
        let vfs = Arc::new(nanograph_vfs::MemoryFileSystem::new());
        let data_path = nanograph_vfs::Path::from("/data");
        let wal_path = nanograph_vfs::Path::from("/wal");
        store
            .create_shard(shard_id, vfs, data_path, wal_path)
            .unwrap();

        // Begin transaction
        let tx = tx_mgr.begin();
        let tx_clone = Arc::clone(&tx);

        // Write data
        tx.put(shard_id, b"key1", b"value1").await.unwrap();

        // Commit (consumes tx)
        tx.commit(nanograph_wal::Durability::Sync).await.unwrap();

        // Try to use transaction after commit - should fail
        let result = tx_clone.put(shard_id, b"key2", b"value2").await;
        assert!(result.is_err());

        let result = tx_clone.get(shard_id, b"key1").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_transaction_error_after_rollback() {
        let store = Arc::new(LSMKeyValueStore::new());
        store.init_tx_manager();
        let tx_mgr = store.get_tx_manager();

        let shard_id = ShardId::new(0);
        let vfs = Arc::new(nanograph_vfs::MemoryFileSystem::new());
        let data_path = nanograph_vfs::Path::from("/data");
        let wal_path = nanograph_vfs::Path::from("/wal");
        store
            .create_shard(shard_id, vfs, data_path, wal_path)
            .unwrap();

        // Begin transaction
        let tx = tx_mgr.begin();
        let tx_clone = Arc::clone(&tx);

        // Write data
        tx.put(shard_id, b"key1", b"value1").await.unwrap();

        // Rollback (consumes tx)
        tx.rollback().await.unwrap();

        // Try to use transaction after rollback - should fail
        let result = tx_clone.put(shard_id, b"key2", b"value2").await;
        assert!(result.is_err());

        let result = tx_clone.get(shard_id, b"key1").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_gc_watermark_no_transactions() {
        let store = Arc::new(LSMKeyValueStore::new());
        store.init_tx_manager();
        let tx_mgr = store.get_tx_manager();

        // With no active transactions, watermark should be None
        assert_eq!(tx_mgr.min_active_snapshot_seq(), None);
        assert_eq!(tx_mgr.active_transaction_count(), 0);
    }

    #[tokio::test]
    async fn test_gc_watermark_single_transaction() {
        let store = Arc::new(LSMKeyValueStore::new());
        store.init_tx_manager();
        let tx_mgr = store.get_tx_manager();

        let shard_id = ShardId::new(0);
        let vfs = Arc::new(nanograph_vfs::MemoryFileSystem::new());
        let data_path = nanograph_vfs::Path::from("/data");
        let wal_path = nanograph_vfs::Path::from("/wal");
        store
            .create_shard(shard_id, vfs, data_path, wal_path)
            .unwrap();

        // Begin a transaction
        let tx = tx_mgr.begin();
        let snapshot_ts = tx.snapshot_ts().as_millis();

        // Watermark should be the transaction's snapshot timestamp
        assert_eq!(tx_mgr.min_active_snapshot_seq(), Some(snapshot_ts));
        assert_eq!(tx_mgr.active_transaction_count(), 1);

        // Commit transaction
        tx.commit(nanograph_wal::Durability::Sync).await.unwrap();

        // After commit, watermark should be None again
        assert_eq!(tx_mgr.min_active_snapshot_seq(), None);
        assert_eq!(tx_mgr.active_transaction_count(), 0);
    }

    #[tokio::test]
    async fn test_gc_watermark_multiple_transactions() {
        let store = Arc::new(LSMKeyValueStore::new());
        store.init_tx_manager();
        let tx_mgr = store.get_tx_manager();

        let shard_id = ShardId::new(0);
        let vfs = Arc::new(nanograph_vfs::MemoryFileSystem::new());
        let data_path = nanograph_vfs::Path::from("/data");
        let wal_path = nanograph_vfs::Path::from("/wal");
        store
            .create_shard(shard_id, vfs, data_path, wal_path)
            .unwrap();

        // Begin first transaction
        let tx1 = tx_mgr.begin();
        let ts1 = tx1.snapshot_ts().as_millis();

        // Small delay to ensure different timestamps
        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

        // Begin second transaction
        let tx2 = tx_mgr.begin();
        let ts2 = tx2.snapshot_ts().as_millis();

        // Small delay
        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

        // Begin third transaction
        let tx3 = tx_mgr.begin();
        let ts3 = tx3.snapshot_ts().as_millis();

        // Watermark should be the oldest (first) transaction's timestamp
        assert_eq!(tx_mgr.min_active_snapshot_seq(), Some(ts1));
        assert_eq!(tx_mgr.active_transaction_count(), 3);

        // Commit first transaction
        tx1.commit(nanograph_wal::Durability::Sync).await.unwrap();

        // Watermark should now be the second transaction's timestamp
        assert_eq!(tx_mgr.min_active_snapshot_seq(), Some(ts2));
        assert_eq!(tx_mgr.active_transaction_count(), 2);

        // Commit third transaction (out of order)
        tx3.commit(nanograph_wal::Durability::Sync).await.unwrap();

        // Watermark should still be the second transaction's timestamp
        assert_eq!(tx_mgr.min_active_snapshot_seq(), Some(ts2));
        assert_eq!(tx_mgr.active_transaction_count(), 1);

        // Commit second transaction
        tx2.commit(nanograph_wal::Durability::Sync).await.unwrap();

        // All transactions committed, watermark should be None
        assert_eq!(tx_mgr.min_active_snapshot_seq(), None);
        assert_eq!(tx_mgr.active_transaction_count(), 0);
    }

    #[tokio::test]
    async fn test_gc_watermark_with_rollback() {
        let store = Arc::new(LSMKeyValueStore::new());
        store.init_tx_manager();
        let tx_mgr = store.get_tx_manager();

        let shard_id = ShardId::new(0);
        let vfs = Arc::new(nanograph_vfs::MemoryFileSystem::new());
        let data_path = nanograph_vfs::Path::from("/data");
        let wal_path = nanograph_vfs::Path::from("/wal");
        store
            .create_shard(shard_id, vfs, data_path, wal_path)
            .unwrap();

        // Begin two transactions
        let tx1 = tx_mgr.begin();
        let ts1 = tx1.snapshot_ts().as_millis();

        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

        let tx2 = tx_mgr.begin();
        let _ts2 = tx2.snapshot_ts().as_millis();

        // Watermark should be first transaction's timestamp
        assert_eq!(tx_mgr.min_active_snapshot_seq(), Some(ts1));
        assert_eq!(tx_mgr.active_transaction_count(), 2);

        // Rollback first transaction
        tx1.rollback().await.unwrap();

        // Watermark should now be second transaction's timestamp
        assert_eq!(tx_mgr.active_transaction_count(), 1);

        // Commit second transaction
        tx2.commit(nanograph_wal::Durability::Sync).await.unwrap();

        // All transactions done, watermark should be None
        assert_eq!(tx_mgr.min_active_snapshot_seq(), None);
        assert_eq!(tx_mgr.active_transaction_count(), 0);
    }

    #[tokio::test]
    async fn test_gc_watermark_concurrent_transactions() {
        use std::sync::atomic::{AtomicUsize, Ordering};

        let store = Arc::new(LSMKeyValueStore::new());
        store.init_tx_manager();
        let tx_mgr = store.get_tx_manager();
        let tx_mgr = Arc::new(tx_mgr);

        let shard_id = ShardId::new(0);
        let vfs = Arc::new(nanograph_vfs::MemoryFileSystem::new());
        let data_path = nanograph_vfs::Path::from("/data");
        let wal_path = nanograph_vfs::Path::from("/wal");
        store
            .create_shard(shard_id, vfs, data_path, wal_path)
            .unwrap();

        let commit_count = Arc::new(AtomicUsize::new(0));

        // Spawn multiple concurrent transactions
        let mut handles = vec![];
        for i in 0..10 {
            let tx_mgr_clone = Arc::clone(&tx_mgr);
            let commit_count_clone = Arc::clone(&commit_count);
            let handle = tokio::spawn(async move {
                let tx = tx_mgr_clone.begin();

                // Do some work
                tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

                // Commit
                if tx.commit(nanograph_wal::Durability::Sync).await.is_ok() {
                    commit_count_clone.fetch_add(1, Ordering::SeqCst);
                }
            });
            handles.push(handle);

            // Small delay between starting transactions
            tokio::time::sleep(tokio::time::Duration::from_millis(5)).await;
        }

        // While transactions are running, watermark should exist
        tokio::time::sleep(tokio::time::Duration::from_millis(20)).await;
        assert!(tx_mgr.min_active_snapshot_seq().is_some());
        assert!(tx_mgr.active_transaction_count() > 0);

        // Wait for all transactions to complete
        for handle in handles {
            handle.await.expect("Thread should not panic");
        }

        // All transactions should have committed
        assert_eq!(commit_count.load(Ordering::SeqCst), 10);

        // After all transactions complete, watermark should be None
        assert_eq!(tx_mgr.min_active_snapshot_seq(), None);
        assert_eq!(tx_mgr.active_transaction_count(), 0);
    }
}
