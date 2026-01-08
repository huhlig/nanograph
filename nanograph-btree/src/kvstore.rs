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

use crate::iterator::BPlusTreeIterator;
use crate::metrics::BTreeMetrics;
use crate::transaction::TransactionManager;
use crate::tree::{BPlusTree, BPlusTreeConfig};
use async_trait::async_trait;
use nanograph_kvt::{
    BTreeStats as KvBTreeStats, EngineStats, KeyRange, KeyValueError, KeyValueIterator,
    KeyValueResult, KeyValueStore, KeyValueTableId, TableStats, Transaction,
};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

/// B+Tree implementation of KeyValueStore
pub struct BTreeKeyValueStore {
    /// B+Trees for each table
    trees: Arc<RwLock<HashMap<KeyValueTableId, Arc<BPlusTree>>>>,

    /// Table names
    table_names: Arc<RwLock<HashMap<KeyValueTableId, String>>>,

    /// Next table ID
    next_table_id: Arc<RwLock<u128>>,

    /// Transaction manager
    tx_manager: Arc<TransactionManager>,

    /// Metrics for each table
    metrics: Arc<RwLock<HashMap<KeyValueTableId, Arc<BTreeMetrics>>>>,

    /// Default B+Tree configuration
    config: BPlusTreeConfig,
}

impl BTreeKeyValueStore {
    /// Create a new B+Tree key-value store
    pub fn new(config: BPlusTreeConfig) -> Self {
        Self {
            trees: Arc::new(RwLock::new(HashMap::new())),
            table_names: Arc::new(RwLock::new(HashMap::new())),
            next_table_id: Arc::new(RwLock::new(1)),
            tx_manager: Arc::new(TransactionManager::new()),
            metrics: Arc::new(RwLock::new(HashMap::new())),
            config,
        }
    }

    /// Get the tree for a table
    fn get_tree(&self, table: KeyValueTableId) -> KeyValueResult<Arc<BPlusTree>> {
        let trees = self.trees.read().unwrap();
        trees
            .get(&table)
            .cloned()
            .ok_or(nanograph_kvt::KeyValueError::KeyNotFound)
    }

    /// Get the metrics for a table
    fn get_metrics(&self, table: KeyValueTableId) -> Arc<BTreeMetrics> {
        let mut metrics = self.metrics.write().unwrap();
        metrics
            .entry(table)
            .or_insert_with(|| Arc::new(BTreeMetrics::new()))
            .clone()
    }
}

impl Default for BTreeKeyValueStore {
    fn default() -> Self {
        Self::new(BPlusTreeConfig::default())
    }
}

#[async_trait]
impl KeyValueStore for BTreeKeyValueStore {
    // ===== Basic Operations =====

    async fn get(&self, table: KeyValueTableId, key: &[u8]) -> KeyValueResult<Option<Vec<u8>>> {
        let tree = self.get_tree(table)?;
        let metrics = self.get_metrics(table);

        let result = tree.get(key).map_err(Into::<KeyValueError>::into)?;
        metrics.record_read(result.is_some());

        Ok(result)
    }

    async fn put(&self, table: KeyValueTableId, key: &[u8], value: &[u8]) -> KeyValueResult<()> {
        let tree = self.get_tree(table)?;
        let metrics = self.get_metrics(table);

        // Check if key exists (for update tracking)
        let exists = tree
            .get(key)
            .map_err(Into::<KeyValueError>::into)?
            .is_some();

        tree.insert(key.to_vec(), value.to_vec())
            .map_err(Into::<KeyValueError>::into)?;

        metrics.record_write(exists);

        Ok(())
    }

    async fn delete(&self, table: KeyValueTableId, key: &[u8]) -> KeyValueResult<bool> {
        let tree = self.get_tree(table)?;
        let metrics = self.get_metrics(table);

        let deleted = tree.delete(key).map_err(Into::<KeyValueError>::into)?;

        if deleted {
            metrics.record_delete();
        }

        Ok(deleted)
    }

    async fn exists(&self, table: KeyValueTableId, key: &[u8]) -> KeyValueResult<bool> {
        let tree = self.get_tree(table)?;
        let result = tree.get(key).map_err(Into::<KeyValueError>::into)?;
        Ok(result.is_some())
    }

    // ===== Batch Operations =====

    async fn batch_get(
        &self,
        table: KeyValueTableId,
        keys: &[&[u8]],
    ) -> KeyValueResult<Vec<Option<Vec<u8>>>> {
        let tree = self.get_tree(table)?;
        let metrics = self.get_metrics(table);

        let mut results = Vec::with_capacity(keys.len());
        for key in keys {
            let result = tree.get(key).map_err(Into::<KeyValueError>::into)?;
            metrics.record_read(result.is_some());
            results.push(result);
        }

        Ok(results)
    }

    async fn batch_put(
        &self,
        table: KeyValueTableId,
        pairs: &[(&[u8], &[u8])],
    ) -> KeyValueResult<()> {
        let tree = self.get_tree(table)?;
        let metrics = self.get_metrics(table);

        for (key, value) in pairs {
            let exists = tree
                .get(key)
                .map_err(Into::<KeyValueError>::into)?
                .is_some();
            tree.insert(key.to_vec(), value.to_vec())
                .map_err(Into::<KeyValueError>::into)?;
            metrics.record_write(exists);
        }

        Ok(())
    }

    async fn batch_delete(&self, table: KeyValueTableId, keys: &[&[u8]]) -> KeyValueResult<usize> {
        let tree = self.get_tree(table)?;
        let metrics = self.get_metrics(table);

        let mut count = 0;
        for key in keys {
            if tree.delete(key).map_err(Into::<KeyValueError>::into)? {
                count += 1;
                metrics.record_delete();
            }
        }

        Ok(count)
    }

    // ===== Range Operations =====

    async fn scan(
        &self,
        table: KeyValueTableId,
        range: KeyRange,
    ) -> KeyValueResult<Box<dyn KeyValueIterator + Send>> {
        let tree = self.get_tree(table)?;
        let metrics = self.get_metrics(table);

        metrics.record_scan(0); // Will be updated as iteration progresses

        let iterator =
            BPlusTreeIterator::new(tree, range.start, range.end, range.reverse, range.limit)
                .map_err(|e| Into::<KeyValueError>::into(e))?;

        // Return the concrete type directly - trait object will handle pinning
        Ok(Box::new(iterator) as Box<dyn KeyValueIterator + Send>)
    }

    // ===== Statistics & Metadata =====

    async fn key_count(&self, table: KeyValueTableId) -> KeyValueResult<u64> {
        let tree = self.get_tree(table)?;
        let stats = tree.stats();
        Ok(stats.num_keys as u64)
    }

    async fn table_stats(&self, table: KeyValueTableId) -> KeyValueResult<TableStats> {
        let tree = self.get_tree(table)?;
        let tree_stats = tree.stats();
        let metrics = self.get_metrics(table);
        let metrics_snapshot = metrics.snapshot();

        // Estimate bytes (rough calculation)
        let avg_key_size = 32; // Assume average key size
        let avg_value_size = 128; // Assume average value size
        let data_bytes = (tree_stats.num_keys as u64) * (avg_key_size + avg_value_size);

        // Estimate index overhead (internal nodes)
        let index_bytes = (tree_stats.num_internal_nodes as u64) * 1024; // Rough estimate

        let btree_stats = KvBTreeStats {
            tree_height: tree_stats.height,
            total_nodes: (tree_stats.num_internal_nodes + tree_stats.num_leaf_nodes) as u64,
            leaf_nodes: tree_stats.num_leaf_nodes as u64,
            internal_nodes: tree_stats.num_internal_nodes as u64,
            avg_node_utilization: if tree_stats.num_leaf_nodes > 0 {
                tree_stats.num_keys as f64 / tree_stats.num_leaf_nodes as f64
            } else {
                0.0
            },
            total_splits: metrics_snapshot.node_splits,
            total_merges: metrics_snapshot.node_merges,
            page_size: self.config.max_keys * 64, // Rough estimate: max_keys * avg_entry_size
        };

        Ok(TableStats {
            key_count: tree_stats.num_keys as u64,
            total_bytes: data_bytes + index_bytes,
            data_bytes,
            index_bytes,
            last_modified: None,
            engine_stats: EngineStats::BTree(btree_stats),
        })
    }

    // ===== Transaction Support =====

    async fn begin_transaction(&self) -> KeyValueResult<Arc<dyn Transaction>> {
        // For now, create a transaction on the first table
        // In a real implementation, transactions should span multiple tables
        let tables = self.table_names.read().unwrap();
        let (table_id, tree) = if let Some((id, _)) = tables.iter().next() {
            let trees = self.trees.read().unwrap();
            let tree = trees.get(id).ok_or(KeyValueError::KeyNotFound)?;
            (*id, tree.clone())
        } else {
            return Err(KeyValueError::StorageCorruption(
                "No tables available for transaction".to_string(),
            ));
        };

        let tx = self.tx_manager.begin(table_id, tree);
        Ok(tx as Arc<dyn Transaction>)
    }

    // ===== Table Management =====

    async fn create_table(&self, name: &str) -> KeyValueResult<KeyValueTableId> {
        // Check if table name already exists
        {
            let table_names = self.table_names.read().unwrap();
            if table_names.values().any(|n| n == name) {
                return Err(KeyValueError::StorageCorruption(format!(
                    "Table '{}' already exists",
                    name
                )));
            }
        }

        let mut next_id = self.next_table_id.write().unwrap();
        let table_id = KeyValueTableId::new(*next_id);
        *next_id += 1;

        // Create a new B+Tree for this table
        let tree = Arc::new(BPlusTree::new(self.config.clone()));

        // Store table
        let mut trees = self.trees.write().unwrap();
        trees.insert(table_id, tree);

        // Store table name
        let mut table_names = self.table_names.write().unwrap();
        table_names.insert(table_id, name.to_string());

        Ok(table_id)
    }

    async fn drop_table(&self, table: KeyValueTableId) -> KeyValueResult<()> {
        let mut trees = self.trees.write().unwrap();
        trees.remove(&table);

        let mut table_names = self.table_names.write().unwrap();
        table_names.remove(&table);

        let mut metrics = self.metrics.write().unwrap();
        metrics.remove(&table);

        Ok(())
    }

    async fn list_tables(&self) -> KeyValueResult<Vec<(KeyValueTableId, String)>> {
        let table_names = self.table_names.read().unwrap();
        Ok(table_names
            .iter()
            .map(|(id, name)| (*id, name.clone()))
            .collect())
    }

    async fn table_exists(&self, table: KeyValueTableId) -> KeyValueResult<bool> {
        let table_names = self.table_names.read().unwrap();
        Ok(table_names.contains_key(&table))
    }

    // ===== Maintenance Operations =====

    async fn flush(&self) -> KeyValueResult<()> {
        // B+Tree is in-memory, so flush is a no-op
        // In a persistent implementation, this would write to disk
        Ok(())
    }

    async fn compact(&self, _table: Option<KeyValueTableId>) -> KeyValueResult<()> {
        // B+Tree doesn't need compaction like LSM trees
        // This is a no-op
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_table_management() {
        let store = BTreeKeyValueStore::default();

        // Create table
        let table_id = store.create_table("test_table").await.unwrap();
        assert!(store.table_exists(table_id).await.unwrap());

        // List tables
        let tables = store.list_tables().await.unwrap();
        assert_eq!(tables.len(), 1);
        assert_eq!(tables[0].1, "test_table");

        // Drop table
        store.drop_table(table_id).await.unwrap();
        assert!(!store.table_exists(table_id).await.unwrap());
    }

    #[tokio::test]
    async fn test_basic_operations() {
        let store = BTreeKeyValueStore::default();
        let table_id = store.create_table("test").await.unwrap();

        // Put
        store.put(table_id, b"key1", b"value1").await.unwrap();
        store.put(table_id, b"key2", b"value2").await.unwrap();

        // Get
        assert_eq!(
            store.get(table_id, b"key1").await.unwrap(),
            Some(b"value1".to_vec())
        );
        assert_eq!(
            store.get(table_id, b"key2").await.unwrap(),
            Some(b"value2".to_vec())
        );
        assert_eq!(store.get(table_id, b"key3").await.unwrap(), None);

        // Exists
        assert!(store.exists(table_id, b"key1").await.unwrap());
        assert!(!store.exists(table_id, b"key3").await.unwrap());

        // Delete
        assert!(store.delete(table_id, b"key1").await.unwrap());
        assert!(!store.delete(table_id, b"key3").await.unwrap());
        assert_eq!(store.get(table_id, b"key1").await.unwrap(), None);
    }

    #[tokio::test]
    async fn test_batch_operations() {
        let store = BTreeKeyValueStore::default();
        let table_id = store.create_table("test").await.unwrap();

        // Batch put
        let pairs = vec![
            (&b"key1"[..], &b"value1"[..]),
            (&b"key2"[..], &b"value2"[..]),
            (&b"key3"[..], &b"value3"[..]),
        ];
        store.batch_put(table_id, &pairs).await.unwrap();

        // Batch get
        let keys = vec![&b"key1"[..], &b"key2"[..], &b"key3"[..], &b"key4"[..]];
        let results = store.batch_get(table_id, &keys).await.unwrap();
        assert_eq!(results[0], Some(b"value1".to_vec()));
        assert_eq!(results[1], Some(b"value2".to_vec()));
        assert_eq!(results[2], Some(b"value3".to_vec()));
        assert_eq!(results[3], None);

        // Batch delete
        let delete_keys = vec![&b"key1"[..], &b"key2"[..], &b"key4"[..]];
        let deleted = store.batch_delete(table_id, &delete_keys).await.unwrap();
        assert_eq!(deleted, 2);
    }

    #[tokio::test]
    async fn test_statistics() {
        let store = BTreeKeyValueStore::default();
        let table_id = store.create_table("test").await.unwrap();

        // Insert data
        for i in 0..100 {
            let key = format!("key{:03}", i);
            let value = format!("value{}", i);
            store
                .put(table_id, key.as_bytes(), value.as_bytes())
                .await
                .unwrap();
        }

        // Check stats
        let count = store.key_count(table_id).await.unwrap();
        assert_eq!(count, 100);

        let stats = store.table_stats(table_id).await.unwrap();
        assert_eq!(stats.key_count, 100);
        assert!(stats.total_bytes > 0);
    }

    #[tokio::test]
    async fn test_transactions() {
        let store = BTreeKeyValueStore::default();
        let _table_id = store.create_table("test").await.unwrap();

        // Begin transaction
        let tx = store.begin_transaction().await.unwrap();
        assert!(tx.id().0 > 0);

        // Commit
        let tx_id = tx.id();
        tx.commit().await.unwrap();

        // Begin another transaction
        let tx2 = store.begin_transaction().await.unwrap();
        assert!(tx2.id().0 > tx_id.0);

        // Rollback
        tx2.rollback().await.unwrap();
    }
}

// Made with Bob
