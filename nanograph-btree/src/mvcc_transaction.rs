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

//! MVCC-aware transaction management
//!
//! This module provides transaction support with snapshot isolation
//! using the MVCC infrastructure.

use crate::error::BTreeResult;
use crate::mvcc_tree::MvccBPlusTree;
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

/// Transaction ID
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TxId(pub u64);

/// Write operation in a transaction
#[derive(Debug, Clone)]
enum WriteOp {
    Put { key: Vec<u8>, value: Vec<u8> },
    Delete { key: Vec<u8> },
}

/// MVCC transaction with snapshot isolation
pub struct MvccTransaction {
    /// Transaction ID (used as created_ts)
    id: TxId,

    /// Snapshot timestamp (when transaction started)
    snapshot_ts: u64,

    /// Reference to the tree
    tree: Arc<MvccBPlusTree>,

    /// Buffered writes (not yet applied to tree)
    write_buffer: RwLock<HashMap<Vec<u8>, WriteOp>>,

    /// Whether the transaction is active
    active: RwLock<bool>,
}

impl MvccTransaction {
    /// Create a new transaction
    pub fn new(id: TxId, snapshot_ts: u64, tree: Arc<MvccBPlusTree>) -> Self {
        Self {
            id,
            snapshot_ts,
            tree,
            write_buffer: RwLock::new(HashMap::new()),
            active: RwLock::new(true),
        }
    }

    /// Get transaction ID
    pub fn id(&self) -> TxId {
        self.id
    }

    /// Get snapshot timestamp
    pub fn snapshot_ts(&self) -> u64 {
        self.snapshot_ts
    }

    /// Check if transaction is active
    pub fn is_active(&self) -> bool {
        *self.active.read().unwrap()
    }

    /// Get a value (checks write buffer first, then tree at snapshot)
    pub fn get(&self, key: &[u8]) -> BTreeResult<Option<Vec<u8>>> {
        if !self.is_active() {
            return Err(crate::error::BTreeError::Internal(
                "Transaction is not active".to_string(),
            ));
        }

        // Check write buffer first (read-your-own-writes)
        let buffer = self.write_buffer.read().unwrap();
        if let Some(op) = buffer.get(key) {
            return Ok(match op {
                WriteOp::Put { value, .. } => Some(value.clone()),
                WriteOp::Delete { .. } => None,
            });
        }

        // Read from tree at snapshot timestamp
        self.tree.get(key, self.snapshot_ts)
    }

    /// Put a key-value pair (buffered until commit)
    pub fn put(&self, key: Vec<u8>, value: Vec<u8>) -> BTreeResult<()> {
        if !self.is_active() {
            return Err(crate::error::BTreeError::Internal(
                "Transaction is not active".to_string(),
            ));
        }

        let mut buffer = self.write_buffer.write().unwrap();
        buffer.insert(key.clone(), WriteOp::Put { key, value });
        Ok(())
    }

    /// Delete a key (buffered until commit)
    pub fn delete(&self, key: Vec<u8>) -> BTreeResult<()> {
        if !self.is_active() {
            return Err(crate::error::BTreeError::Internal(
                "Transaction is not active".to_string(),
            ));
        }

        let mut buffer = self.write_buffer.write().unwrap();
        buffer.insert(key.clone(), WriteOp::Delete { key });
        Ok(())
    }

    /// Commit the transaction
    ///
    /// Applies all buffered writes to the tree and checks for conflicts.
    pub fn commit(self: Arc<Self>, commit_ts: u64) -> BTreeResult<()> {
        if !self.is_active() {
            return Err(crate::error::BTreeError::Internal(
                "Transaction is not active".to_string(),
            ));
        }

        // Get all buffered writes and release lock immediately
        let writes: Vec<_> = {
            let buffer = self.write_buffer.read().unwrap();
            buffer.values().cloned().collect()
        };

        // Convert writes to the format expected by atomic_commit
        let atomic_writes: Vec<(Vec<u8>, Option<Vec<u8>>)> = writes
            .iter()
            .map(|op| match op {
                WriteOp::Put { key, value } => (key.clone(), Some(value.clone())),
                WriteOp::Delete { key } => (key.clone(), None),
            })
            .collect();

        // Use atomic_commit to prevent deadlocks
        let result =
            self.tree
                .atomic_commit(&atomic_writes, self.snapshot_ts, self.id.0, commit_ts);

        // Mark transaction as committed/aborted
        *self.active.write().unwrap() = false;

        result
    }

    /// Rollback the transaction
    pub fn rollback(self: Arc<Self>) -> BTreeResult<()> {
        if !self.is_active() {
            return Err(crate::error::BTreeError::Internal(
                "Transaction is not active".to_string(),
            ));
        }

        // Rollback any versions that were created
        self.tree.rollback_versions(self.id.0)?;

        // Clear write buffer
        self.write_buffer.write().unwrap().clear();

        // Mark transaction as aborted
        *self.active.write().unwrap() = false;

        Ok(())
    }
}

/// MVCC transaction manager
pub struct MvccTransactionManager {
    /// Next transaction ID
    next_tx_id: RwLock<u64>,

    /// Next commit timestamp
    next_commit_ts: RwLock<u64>,

    /// Active transactions
    active_transactions: RwLock<HashMap<TxId, Arc<MvccTransaction>>>,

    /// The MVCC tree
    tree: Arc<MvccBPlusTree>,

    /// GC counter (trigger GC every N commits)
    gc_counter: RwLock<u64>,

    /// GC interval
    gc_interval: u64,
}

impl MvccTransactionManager {
    /// Create a new transaction manager
    pub fn new(tree: Arc<MvccBPlusTree>) -> Self {
        Self {
            next_tx_id: RwLock::new(1),
            next_commit_ts: RwLock::new(1),
            active_transactions: RwLock::new(HashMap::new()),
            tree,
            gc_counter: RwLock::new(0),
            gc_interval: 100, // Run GC every 100 commits
        }
    }

    /// Begin a new transaction
    pub fn begin(&self) -> Arc<MvccTransaction> {
        // Get transaction ID and release lock immediately
        let tx_id = {
            let mut next_id = self.next_tx_id.write().unwrap();
            let id = TxId(*next_id);
            *next_id += 1;
            id
        };

        // Get snapshot timestamp - use current commit timestamp minus 1
        // This ensures the transaction sees all committed data up to this point
        let snapshot_ts = {
            let next_commit = self.next_commit_ts.read().unwrap();
            // Snapshot should be the last committed timestamp
            // If next_commit is 1, no commits yet, so snapshot is 0
            if *next_commit > 0 {
                *next_commit - 1
            } else {
                0
            }
        };

        let tx = Arc::new(MvccTransaction::new(tx_id, snapshot_ts, self.tree.clone()));

        // Add to active transactions
        {
            let mut active = self.active_transactions.write().unwrap();
            active.insert(tx_id, tx.clone());
        }

        // Update minimum snapshot timestamp for GC
        self.update_min_snapshot_ts();

        tx
    }

    /// Commit a transaction
    pub fn commit(&self, tx: Arc<MvccTransaction>) -> BTreeResult<()> {
        let tx_id = tx.id();

        // Get next commit timestamp and release lock immediately to prevent deadlock
        let commit_ts = {
            let mut next_commit = self.next_commit_ts.write().unwrap();
            let ts = *next_commit;
            *next_commit += 1;
            ts
        };

        // Commit the transaction (this acquires tree lock)
        let result = tx.commit(commit_ts);

        // Remove from active transactions only after commit succeeds/fails
        {
            let mut active = self.active_transactions.write().unwrap();
            active.remove(&tx_id);
        }

        // Update minimum snapshot timestamp
        self.update_min_snapshot_ts();

        // Check if we should run GC
        let should_gc = {
            let mut gc_counter = self.gc_counter.write().unwrap();
            *gc_counter += 1;
            let should_gc = *gc_counter >= self.gc_interval;
            if should_gc {
                *gc_counter = 0;
            }
            should_gc
        };

        if should_gc {
            self.tree.gc_versions()?;
        }

        result
    }

    /// Rollback a transaction
    pub fn rollback(&self, tx: Arc<MvccTransaction>) -> BTreeResult<()> {
        let tx_id = tx.id();

        // Rollback the transaction (this acquires tree lock)
        let result = tx.rollback();

        // Remove from active transactions only after rollback completes
        {
            let mut active = self.active_transactions.write().unwrap();
            active.remove(&tx_id);
        }

        // Update minimum snapshot timestamp
        self.update_min_snapshot_ts();

        result
    }

    /// Update the minimum snapshot timestamp for GC
    fn update_min_snapshot_ts(&self) {
        // Get the default value first (without holding any locks)
        let default_ts = *self.next_commit_ts.read().unwrap();

        // Then get the minimum from active transactions
        let active = self.active_transactions.read().unwrap();
        let min_snapshot = active
            .values()
            .map(|tx| tx.snapshot_ts())
            .min()
            .unwrap_or(default_ts);
        drop(active);

        self.tree.update_min_snapshot_ts(min_snapshot);
    }

    /// Get the number of active transactions
    pub fn active_count(&self) -> usize {
        self.active_transactions.read().unwrap().len()
    }

    /// Manually trigger garbage collection
    pub fn gc(&self) -> BTreeResult<()> {
        self.tree.gc_versions()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mvcc_tree::{MvccBPlusTree, MvccTreeConfig};

    #[test]
    fn test_transaction_basic_operations() {
        let tree = Arc::new(MvccBPlusTree::new(MvccTreeConfig::default()));
        let manager = MvccTransactionManager::new(tree);

        // Begin transaction
        let tx = manager.begin();
        assert_eq!(manager.active_count(), 1);

        // Put and get
        tx.put(b"key1".to_vec(), b"value1".to_vec()).unwrap();
        assert_eq!(tx.get(b"key1").unwrap(), Some(b"value1".to_vec()));

        // Commit
        manager.commit(tx).unwrap();
        assert_eq!(manager.active_count(), 0);
    }

    #[test]
    fn test_transaction_snapshot_isolation() {
        let tree = Arc::new(MvccBPlusTree::new(MvccTreeConfig::default()));
        let manager = MvccTransactionManager::new(tree.clone());

        // Transaction 1: Insert key1=v1
        let tx1 = manager.begin();
        tx1.put(b"key1".to_vec(), b"v1".to_vec()).unwrap();
        manager.commit(tx1).unwrap();

        // Transaction 2 starts (sees v1)
        let tx2 = manager.begin();
        assert_eq!(tx2.get(b"key1").unwrap(), Some(b"v1".to_vec()));

        // Transaction 3: Update key1=v2
        let tx3 = manager.begin();
        tx3.put(b"key1".to_vec(), b"v2".to_vec()).unwrap();
        manager.commit(tx3).unwrap();

        // Transaction 2 still sees v1 (snapshot isolation)
        assert_eq!(tx2.get(b"key1").unwrap(), Some(b"v1".to_vec()));
        manager.commit(tx2).unwrap();

        // New transaction sees v2
        let tx4 = manager.begin();
        assert_eq!(tx4.get(b"key1").unwrap(), Some(b"v2".to_vec()));
        manager.commit(tx4).unwrap();
    }

    #[test]
    fn test_transaction_write_conflict() {
        let tree = Arc::new(MvccBPlusTree::new(MvccTreeConfig::default()));
        let manager = MvccTransactionManager::new(tree);

        // Transaction 1: Insert key1=v1
        let tx1 = manager.begin();
        tx1.put(b"key1".to_vec(), b"v1".to_vec()).unwrap();
        manager.commit(tx1).unwrap();

        // Transaction 2 and 3 start at same time
        let tx2 = manager.begin();
        let tx3 = manager.begin();

        // Both try to update key1
        tx2.put(b"key1".to_vec(), b"v2".to_vec()).unwrap();
        tx3.put(b"key1".to_vec(), b"v3".to_vec()).unwrap();

        // First commit succeeds
        assert!(manager.commit(tx2).is_ok());

        // Second commit fails (write conflict)
        assert!(manager.commit(tx3).is_err());
    }

    #[test]
    fn test_transaction_rollback() {
        let tree = Arc::new(MvccBPlusTree::new(MvccTreeConfig::default()));
        let manager = MvccTransactionManager::new(tree.clone());

        // Transaction 1: Insert and commit
        let tx1 = manager.begin();
        tx1.put(b"key1".to_vec(), b"v1".to_vec()).unwrap();
        manager.commit(tx1).unwrap();

        // Transaction 2: Update but rollback
        let tx2 = manager.begin();
        tx2.put(b"key1".to_vec(), b"v2".to_vec()).unwrap();
        manager.rollback(tx2).unwrap();

        // Should still see v1
        let tx3 = manager.begin();
        assert_eq!(tx3.get(b"key1").unwrap(), Some(b"v1".to_vec()));
        manager.commit(tx3).unwrap();
    }

    #[test]
    fn test_transaction_read_your_own_writes() {
        let tree = Arc::new(MvccBPlusTree::new(MvccTreeConfig::default()));
        let manager = MvccTransactionManager::new(tree);

        let tx = manager.begin();

        // Write is buffered
        tx.put(b"key1".to_vec(), b"value1".to_vec()).unwrap();

        // Can read own write before commit
        assert_eq!(tx.get(b"key1").unwrap(), Some(b"value1".to_vec()));

        manager.commit(tx).unwrap();
    }

    #[test]
    fn test_concurrent_transaction_commits_no_deadlock() {
        use std::sync::atomic::{AtomicUsize, Ordering};

        let tree = Arc::new(MvccBPlusTree::new(MvccTreeConfig::default()));
        let manager = Arc::new(MvccTransactionManager::new(tree));
        let success_count = Arc::new(AtomicUsize::new(0));

        // Insert initial data
        let tx = manager.begin();
        tx.put(b"key1".to_vec(), b"initial".to_vec()).unwrap();
        manager.commit(tx).unwrap();

        // Spawn multiple concurrent transactions that all try to commit
        // Each transaction writes to its own key to avoid conflicts
        let mut handles = vec![];
        for i in 0..10 {
            let manager_clone = manager.clone();
            let success_count_clone = success_count.clone();
            let handle = std::thread::spawn(move || {
                let tx = manager_clone.begin();
                let key = format!("key{}", i);
                let value = format!("value{}", i);
                tx.put(key.into_bytes(), value.into_bytes()).unwrap();

                // Commit - this should not deadlock
                if manager_clone.commit(tx).is_ok() {
                    success_count_clone.fetch_add(1, Ordering::SeqCst);
                }
            });
            handles.push(handle);
        }

        // Wait for all transactions - if there's a deadlock, this will hang
        for handle in handles {
            handle.join().expect("Thread should not panic");
        }

        // All transactions should succeed since they write to different keys
        assert_eq!(
            success_count.load(Ordering::SeqCst),
            10,
            "All transactions should succeed without deadlock"
        );

        // Verify we can still read data (no deadlock occurred)
        // Note: key1 may have been overwritten by one of the concurrent transactions
        let tx = manager.begin();
        let value = tx.get(b"key1").unwrap();
        assert!(value.is_some(), "key1 should still exist");
        manager.commit(tx).unwrap();

        // Verify all individual keys were written successfully
        for i in 0..10 {
            let tx = manager.begin();
            let key = format!("key{}", i);
            let value = tx.get(key.as_bytes()).unwrap();
            assert!(value.is_some(), "key{} should exist", i);
            manager.commit(tx).unwrap();
        }
    }
}
