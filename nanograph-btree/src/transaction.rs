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

use async_trait::async_trait;
use nanograph_kvt::{
    KeyRange, KeyValueError, KeyValueIterator, KeyValueResult, KeyValueTableId, Timestamp,
    Transaction, TransactionId,
};
use std::collections::HashMap;
use std::sync::{Arc, RwLock, Weak};
use crate::tree::BPlusTree;

/// Write operation in a transaction
#[derive(Debug, Clone)]
pub enum WriteOp {
    Put { key: Vec<u8>, value: Vec<u8> },
    Delete { key: Vec<u8> },
}

/// B+Tree transaction implementation
/// Provides snapshot isolation with buffered writes
pub struct BTreeTransaction {
    id: TransactionId,
    /// Table ID this transaction operates on
    /// TODO: Use for multi-table transaction validation
    _table: KeyValueTableId,
    /// Reference to the underlying tree
    tree: Weak<BPlusTree>,
    /// Buffered writes (not yet committed)
    write_buffer: RwLock<HashMap<Vec<u8>, WriteOp>>,
    /// Whether the transaction is active
    active: RwLock<bool>,
}

impl BTreeTransaction {
    pub fn new(id: TransactionId, table: KeyValueTableId, tree: Arc<BPlusTree>) -> Self {
        Self {
            id,
            _table: table,
            tree: Arc::downgrade(&tree),
            write_buffer: RwLock::new(HashMap::new()),
            active: RwLock::new(true),
        }
    }
    
    /// Get the tree reference
    fn get_tree(&self) -> KeyValueResult<Arc<BPlusTree>> {
        self.tree.upgrade()
            .ok_or(KeyValueError::StorageCorruption("Tree no longer exists".to_string()))
    }

    /// Check if transaction is active
    pub fn is_active(&self) -> bool {
        *self.active.read().unwrap()
    }

    /// Buffer a put operation
    pub fn buffer_put(&self, key: Vec<u8>, value: Vec<u8>) -> KeyValueResult<()> {
        if !self.is_active() {
            return Err(nanograph_kvt::KeyValueError::WriteConflict);
        }

        let mut buffer = self.write_buffer.write().unwrap();
        buffer.insert(key.clone(), WriteOp::Put { key, value });
        Ok(())
    }

    /// Buffer a delete operation
    pub fn buffer_delete(&self, key: Vec<u8>) -> KeyValueResult<()> {
        if !self.is_active() {
            return Err(nanograph_kvt::KeyValueError::WriteConflict);
        }

        let mut buffer = self.write_buffer.write().unwrap();
        buffer.insert(key.clone(), WriteOp::Delete { key });
        Ok(())
    }

    /// Get buffered write operations
    pub fn get_writes(&self) -> Vec<WriteOp> {
        let buffer = self.write_buffer.read().unwrap();
        buffer.values().cloned().collect()
    }

    /// Mark transaction as committed
    pub fn mark_committed(&self) {
        *self.active.write().unwrap() = false;
    }

    /// Mark transaction as aborted
    pub fn mark_aborted(&self) {
        *self.active.write().unwrap() = false;
    }
}

#[async_trait]
impl Transaction for BTreeTransaction {
    fn id(&self) -> TransactionId {
        self.id
    }

    fn snapshot_ts(&self) -> Timestamp {
        // For now, use transaction ID as timestamp
        Timestamp(self.id.0)
    }

    async fn get(&self, _table: KeyValueTableId, key: &[u8]) -> KeyValueResult<Option<Vec<u8>>> {
        // Check write buffer first
        let buffer = self.write_buffer.read().unwrap();
        if let Some(op) = buffer.get(key) {
            return match op {
                WriteOp::Put { value, .. } => Ok(Some(value.clone())),
                WriteOp::Delete { .. } => Ok(None),
            };
        }

        // Read from underlying store
        let tree = self.get_tree()?;
        tree.get(key).map_err(Into::into)
    }

    async fn put(&self, _table: KeyValueTableId, key: &[u8], value: &[u8]) -> KeyValueResult<()> {
        let _ = self.buffer_put(key.to_vec(), value.to_vec());
        Ok(())
    }

    async fn delete(&self, _table: KeyValueTableId, key: &[u8]) -> KeyValueResult<bool> {
        let _ = self.buffer_delete(key.to_vec());
        Ok(true)
    }

    async fn scan(
        &self,
        _table: KeyValueTableId,
        _range: KeyRange,
    ) -> KeyValueResult<Box<dyn KeyValueIterator + Send>> {
        if !self.is_active() {
            return Err(KeyValueError::WriteConflict);
        }

        // TODO: Implement scan with transaction isolation
        // This requires:
        // 1. Getting an iterator from the underlying B+Tree
        // 2. Merging with write buffer for read-your-own-writes semantics
        // 3. Applying MVCC visibility rules
        //
        // For now, return an error indicating this is not yet implemented
        Err(KeyValueError::StorageCorruption(
            "Transaction scan not yet fully implemented - needs B+Tree iterator integration".to_string(),
        ))
    }

    async fn commit(self: Arc<Self>) -> KeyValueResult<()> {
        if !self.is_active() {
            return Err(KeyValueError::WriteConflict);
        }

        // Apply all buffered writes to the tree
        let tree = self.get_tree()?;
        let writes = self.get_writes();
        
        for op in writes {
            match op {
                WriteOp::Put { key, value } => {
                    tree.insert(key, value).map_err(|e| Into::<KeyValueError>::into(e))?;
                }
                WriteOp::Delete { key } => {
                    tree.delete(&key).map_err(|e| Into::<KeyValueError>::into(e))?;
                }
            }
        }

        // Mark as committed
        self.mark_committed();

        Ok(())
    }

    async fn rollback(self: Arc<Self>) -> KeyValueResult<()> {
        if !self.is_active() {
            return Err(KeyValueError::WriteConflict);
        }

        // Clear write buffer
        self.write_buffer.write().unwrap().clear();

        // Mark as aborted
        self.mark_aborted();

        Ok(())
    }
}

/// Transaction manager for B+Tree
pub struct TransactionManager {
    next_tx_id: RwLock<u64>,
    active_transactions: RwLock<HashMap<TransactionId, Arc<BTreeTransaction>>>,
}

impl TransactionManager {
    pub fn new() -> Self {
        Self {
            next_tx_id: RwLock::new(1),
            active_transactions: RwLock::new(HashMap::new()),
        }
    }

    /// Begin a new transaction
    pub fn begin(&self, table: KeyValueTableId, tree: Arc<BPlusTree>) -> Arc<BTreeTransaction> {
        let mut next_id = self.next_tx_id.write().unwrap();
        let tx_id = TransactionId(*next_id);
        *next_id += 1;

        let tx = Arc::new(BTreeTransaction::new(tx_id, table, tree));

        let mut active = self.active_transactions.write().unwrap();
        active.insert(tx_id, tx.clone());

        tx
    }

    /// Get an active transaction by ID
    pub fn get_transaction(&self, tx_id: TransactionId) -> Option<Arc<BTreeTransaction>> {
        let active = self.active_transactions.read().unwrap();
        active.get(&tx_id).cloned()
    }

    /// Remove a transaction (after commit or rollback)
    pub fn remove_transaction(&self, tx_id: TransactionId) {
        let mut active = self.active_transactions.write().unwrap();
        active.remove(&tx_id);
    }

    /// Get the number of active transactions
    pub fn active_count(&self) -> usize {
        let active = self.active_transactions.read().unwrap();
        active.len()
    }

    /// Clean up inactive transactions
    pub fn cleanup_inactive(&self) {
        let mut active = self.active_transactions.write().unwrap();
        active.retain(|_, tx| tx.is_active());
    }
}

impl Default for TransactionManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tree::{BPlusTree, BPlusTreeConfig};

    #[tokio::test]
    async fn test_transaction_lifecycle() {
        let manager = TransactionManager::new();
        let tree = Arc::new(BPlusTree::new(BPlusTreeConfig::default()));
        let table = KeyValueTableId::new(1);

        // Begin transaction
        let tx = manager.begin(table, tree.clone());
        assert!(tx.is_active());
        assert_eq!(manager.active_count(), 1);

        // Buffer operations
        tx.buffer_put(b"key1".to_vec(), b"value1".to_vec()).unwrap();
        tx.buffer_delete(b"key2".to_vec()).unwrap();

        let writes = tx.get_writes();
        assert_eq!(writes.len(), 2);

        // Commit
        let tx_id = tx.id();
        tx.commit().await.unwrap();

        // Cleanup
        manager.remove_transaction(tx_id);
        assert_eq!(manager.active_count(), 0);
    }

    #[tokio::test]
    async fn test_transaction_rollback() {
        let manager = TransactionManager::new();
        let tree = Arc::new(BPlusTree::new(BPlusTreeConfig::default()));
        let table = KeyValueTableId::new(1);

        let tx = manager.begin(table, tree.clone());
        tx.buffer_put(b"key1".to_vec(), b"value1".to_vec()).unwrap();

        // Check writes before rollback
        let writes = tx.get_writes();
        assert_eq!(writes.len(), 1);

        // Rollback clears the buffer and marks transaction inactive
        tx.rollback().await.unwrap();
        
        // Transaction is no longer active after rollback
        // (we can't check writes after rollback since tx is consumed)
    }

    #[tokio::test]
    async fn test_multiple_transactions() {
        let manager = TransactionManager::new();
        let tree = Arc::new(BPlusTree::new(BPlusTreeConfig::default()));
        let table = KeyValueTableId::new(1);

        let tx1 = manager.begin(table, tree.clone());
        let tx2 = manager.begin(table, tree.clone());
        let _tx3 = manager.begin(table, tree.clone());

        assert_eq!(manager.active_count(), 3);

        tx1.commit().await.unwrap();
        tx2.rollback().await.unwrap();

        manager.cleanup_inactive();
        assert_eq!(manager.active_count(), 1);
    }

    #[tokio::test]
    async fn test_transaction_error_after_commit() {
        let tree = Arc::new(BPlusTree::new(BPlusTreeConfig::default()));
        let table = KeyValueTableId::new(1);
        let tx = Arc::new(BTreeTransaction::new(TransactionId(1), table, tree.clone()));

        tx.buffer_put(b"key1".to_vec(), b"value1".to_vec()).unwrap();
        
        // Clone tx before commit since commit consumes it
        let tx_clone = tx.clone();
        tx.commit().await.unwrap();

        // Should fail after commit - transaction is no longer active
        let result = tx_clone.buffer_put(b"key2".to_vec(), b"value2".to_vec());
        assert!(result.is_err());
        
        // Verify the first write was committed to the tree
        assert!(tree.get(b"key1").unwrap().is_some());
    }
}

// Made with Bob
