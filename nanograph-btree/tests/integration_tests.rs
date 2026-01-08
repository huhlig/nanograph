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

//! Integration tests for B+Tree implementation

mod test_utils;

use futures::StreamExt;
use nanograph_btree::{BPlusTree, BTreeKeyValueStore, tree::BPlusTreeConfig};
use nanograph_kvt::{KeyRange, KeyValueShardStore, ShardId, ShardIndex, TableId};
use std::ops::Bound;
use std::sync::Arc;
use test_utils::*;

// Helper function to create a test shard
async fn create_test_shard(store: &BTreeKeyValueStore, id: u64) -> ShardId {
    let table_id = TableId::new(id);
    let shard_index = ShardIndex::new(0);
    store.create_shard(table_id, shard_index).await.unwrap()
}

// ============================================================================
// Basic Operations Tests
// ============================================================================

#[tokio::test]
async fn test_basic_put_get() {
    let store = BTreeKeyValueStore::default();
    let shard = create_test_shard(&store, 1).await;

    // Insert a key-value pair
    store.put(shard, b"key1", b"value1").await.unwrap();

    // Retrieve it
    let value = store.get(shard, b"key1").await.unwrap();
    assert_eq!(value, Some(b"value1".to_vec()));

    // Non-existent key
    let value = store.get(shard, b"nonexistent").await.unwrap();
    assert_eq!(value, None);
}

#[tokio::test]
async fn test_update_existing_key() {
    let store = BTreeKeyValueStore::default();
    let shard = create_test_shard(&store, 1).await;

    store.put(shard, b"key1", b"value1").await.unwrap();
    assert_eq!(
        store.get(shard, b"key1").await.unwrap(),
        Some(b"value1".to_vec())
    );

    // Update the value
    store.put(shard, b"key1", b"value2").await.unwrap();
    assert_eq!(
        store.get(shard, b"key1").await.unwrap(),
        Some(b"value2".to_vec())
    );
}

#[tokio::test]
async fn test_delete() {
    let store = BTreeKeyValueStore::default();
    let shard = create_test_shard(&store, 1).await;

    store.put(shard, b"key1", b"value1").await.unwrap();
    store.put(shard, b"key2", b"value2").await.unwrap();

    // Delete key1
    let deleted = store.delete(shard, b"key1").await.unwrap();
    assert!(deleted);

    // Verify it's gone
    assert_eq!(store.get(shard, b"key1").await.unwrap(), None);

    // key2 should still exist
    assert_eq!(
        store.get(shard, b"key2").await.unwrap(),
        Some(b"value2".to_vec())
    );

    // Delete non-existent key
    let deleted = store.delete(shard, b"nonexistent").await.unwrap();
    assert!(!deleted);
}

#[tokio::test]
async fn test_empty_key_and_value() {
    let store = BTreeKeyValueStore::default();
    let shard = create_test_shard(&store, 1).await;

    // Empty key
    store.put(shard, b"", b"value").await.unwrap();
    assert_eq!(
        store.get(shard, b"").await.unwrap(),
        Some(b"value".to_vec())
    );

    // Empty value
    store.put(shard, b"key", b"").await.unwrap();
    assert_eq!(store.get(shard, b"key").await.unwrap(), Some(b"".to_vec()));

    // Both empty
    store.put(shard, b"", b"").await.unwrap();
    assert_eq!(store.get(shard, b"").await.unwrap(), Some(b"".to_vec()));
}

// ============================================================================
// Batch Operations Tests
// ============================================================================

#[tokio::test]
async fn test_batch_put() {
    let store = BTreeKeyValueStore::default();
    let shard = create_test_shard(&store, 1).await;

    let pairs = vec![
        (&b"key1"[..], &b"value1"[..]),
        (&b"key2"[..], &b"value2"[..]),
        (&b"key3"[..], &b"value3"[..]),
    ];

    store.batch_put(shard, &pairs).await.unwrap();

    // Verify all were inserted
    assert_eq!(
        store.get(shard, b"key1").await.unwrap(),
        Some(b"value1".to_vec())
    );
    assert_eq!(
        store.get(shard, b"key2").await.unwrap(),
        Some(b"value2".to_vec())
    );
    assert_eq!(
        store.get(shard, b"key3").await.unwrap(),
        Some(b"value3".to_vec())
    );
}

#[tokio::test]
async fn test_batch_get() {
    let store = BTreeKeyValueStore::default();
    let shard = create_test_shard(&store, 1).await;

    store.put(shard, b"key1", b"value1").await.unwrap();
    store.put(shard, b"key2", b"value2").await.unwrap();
    store.put(shard, b"key3", b"value3").await.unwrap();

    let keys = vec![&b"key1"[..], &b"key2"[..], &b"nonexistent"[..]];
    let results = store.batch_get(shard, &keys).await.unwrap();

    assert_eq!(results.len(), 3);
    assert_eq!(results[0], Some(b"value1".to_vec()));
    assert_eq!(results[1], Some(b"value2".to_vec()));
    assert_eq!(results[2], None);
}

#[tokio::test]
async fn test_batch_delete() {
    let store = BTreeKeyValueStore::default();
    let shard = create_test_shard(&store, 1).await;

    store.put(shard, b"key1", b"value1").await.unwrap();
    store.put(shard, b"key2", b"value2").await.unwrap();
    store.put(shard, b"key3", b"value3").await.unwrap();

    let keys = vec![&b"key1"[..], &b"key2"[..], &b"nonexistent"[..]];
    let deleted_count = store.batch_delete(shard, &keys).await.unwrap();

    assert_eq!(deleted_count, 2); // Only key1 and key2 existed

    assert_eq!(store.get(shard, b"key1").await.unwrap(), None);
    assert_eq!(store.get(shard, b"key2").await.unwrap(), None);
    assert_eq!(
        store.get(shard, b"key3").await.unwrap(),
        Some(b"value3".to_vec())
    );
}

// ============================================================================
// Range Scan Tests
// ============================================================================

#[tokio::test]
async fn test_scan_all() {
    let store = BTreeKeyValueStore::default();
    let shard = create_test_shard(&store, 1).await;

    // Insert in random order
    store.put(shard, b"key3", b"value3").await.unwrap();
    store.put(shard, b"key1", b"value1").await.unwrap();
    store.put(shard, b"key2", b"value2").await.unwrap();

    let range = KeyRange {
        start: Bound::Unbounded,
        end: Bound::Unbounded,
        limit: None,
        reverse: false,
    };

    let iter = store.scan(shard, range).await.unwrap();
    let mut iter = Box::pin(iter);
    let mut results = Vec::new();

    while let Some(result) = iter.as_mut().next().await {
        match result {
            Ok((key, value)) => results.push((key, value)),
            Err(_) => break,
        }
    }

    // Should be in sorted order
    assert_eq!(results.len(), 3);
    assert_eq!(results[0], (b"key1".to_vec(), b"value1".to_vec()));
    assert_eq!(results[1], (b"key2".to_vec(), b"value2".to_vec()));
    assert_eq!(results[2], (b"key3".to_vec(), b"value3".to_vec()));
}

#[tokio::test]
async fn test_scan_with_bounds() {
    let store = BTreeKeyValueStore::default();
    let shard = create_test_shard(&store, 1).await;

    for i in 0..10 {
        let key = format!("key{:02}", i);
        let value = format!("value{}", i);
        store
            .put(shard, key.as_bytes(), value.as_bytes())
            .await
            .unwrap();
    }

    // Scan from key03 to key07 (inclusive)
    let range = KeyRange {
        start: Bound::Included(b"key03".to_vec()),
        end: Bound::Included(b"key07".to_vec()),
        limit: None,
        reverse: false,
    };

    let iter = store.scan(shard, range).await.unwrap();
    let mut iter = Box::pin(iter);
    let mut results = Vec::new();

    while let Some(result) = iter.as_mut().next().await {
        match result {
            Ok((key, _)) => results.push(key),
            Err(_) => break,
        }
    }

    assert_eq!(results.len(), 5);
    assert_eq!(results[0], b"key03");
    assert_eq!(results[4], b"key07");
}

#[tokio::test]
async fn test_scan_with_limit() {
    let store = BTreeKeyValueStore::default();
    let shard = create_test_shard(&store, 1).await;

    for i in 0..100 {
        let key = format!("key{:03}", i);
        store.put(shard, key.as_bytes(), b"value").await.unwrap();
    }

    let range = KeyRange {
        start: Bound::Unbounded,
        end: Bound::Unbounded,
        limit: Some(10),
        reverse: false,
    };

    let iter = store.scan(shard, range).await.unwrap();
    let mut iter = Box::pin(iter);
    let mut count = 0;

    while let Some(result) = iter.as_mut().next().await {
        if result.is_ok() {
            count += 1;
        }
    }

    assert_eq!(count, 10);
}

#[tokio::test]
async fn test_scan_reverse() {
    let store = BTreeKeyValueStore::default();
    let shard = create_test_shard(&store, 1).await;

    store.put(shard, b"key1", b"value1").await.unwrap();
    store.put(shard, b"key2", b"value2").await.unwrap();
    store.put(shard, b"key3", b"value3").await.unwrap();

    let range = KeyRange {
        start: Bound::Unbounded,
        end: Bound::Unbounded,
        limit: None,
        reverse: true,
    };

    let iter = store.scan(shard, range).await.unwrap();
    let mut iter = Box::pin(iter);
    let mut results = Vec::new();

    while let Some(result) = iter.as_mut().next().await {
        match result {
            Ok((key, value)) => results.push((key, value)),
            Err(_) => break,
        }
    }

    // Should be in reverse sorted order
    assert_eq!(results.len(), 3);
    assert_eq!(results[0], (b"key3".to_vec(), b"value3".to_vec()));
    assert_eq!(results[1], (b"key2".to_vec(), b"value2".to_vec()));
    assert_eq!(results[2], (b"key1".to_vec(), b"value1".to_vec()));
}

// ============================================================================
// Large Dataset Tests
// ============================================================================

#[tokio::test]
async fn test_large_sequential_inserts() {
    let store = BTreeKeyValueStore::default();
    let shard = create_test_shard(&store, 1).await;

    let count = 10_000;

    // Insert sequential keys
    for i in 0..count {
        let key = format!("key{:08}", i);
        let value = format!("value{}", i);
        store
            .put(shard, key.as_bytes(), value.as_bytes())
            .await
            .unwrap();
    }

    // Verify count
    let key_count = store.key_count(shard).await.unwrap();
    assert_eq!(key_count, count);

    // Spot check some values
    assert_eq!(
        store.get(shard, b"key00000000").await.unwrap(),
        Some(b"value0".to_vec())
    );
    assert_eq!(
        store.get(shard, b"key00005000").await.unwrap(),
        Some(b"value5000".to_vec())
    );
    assert_eq!(
        store.get(shard, b"key00009999").await.unwrap(),
        Some(b"value9999".to_vec())
    );
}

#[tokio::test]
async fn test_large_random_inserts() {
    let store = BTreeKeyValueStore::default();
    let shard = create_test_shard(&store, 1).await;

    let kvs = generate_random_kvs(1000, 42);

    // Insert all key-value pairs
    for (key, value) in &kvs {
        store.put(shard, key, value).await.unwrap();
    }

    // Build a map of the final value for each key (last write wins)
    let mut final_values = std::collections::HashMap::new();
    for (key, value) in &kvs {
        final_values.insert(key.clone(), value.clone());
    }

    // Verify all unique keys can be retrieved with their final values
    for (key, expected_value) in &final_values {
        let value = store.get(shard, key).await.unwrap();
        assert_eq!(
            value.as_ref(),
            Some(expected_value),
            "Key mismatch: expected value of length {}, got {:?}",
            expected_value.len(),
            value.as_ref().map(|v| v.len())
        );
    }
}

#[tokio::test]
async fn test_tree_splits() {
    // Use small node size to force splits
    let config = BPlusTreeConfig {
        max_keys: 4,
        min_keys: 2,
    };
    let tree = Arc::new(BPlusTree::new(config));

    // Insert enough keys to trigger multiple splits
    for i in 0..100 {
        let key = format!("key{:03}", i);
        let value = format!("value{}", i);
        tree.insert(key.into_bytes(), value.into_bytes()).unwrap();
    }

    let stats = tree.stats();

    // Should have split into multiple nodes
    assert!(stats.height > 1, "Tree should have height > 1");
    assert!(stats.num_internal_nodes > 0, "Should have internal nodes");
    assert!(stats.num_leaf_nodes > 1, "Should have multiple leaf nodes");
    assert_eq!(stats.num_keys, 100);

    // Verify all keys are still accessible
    for i in 0..100 {
        let key = format!("key{:03}", i);
        let value = tree.get(key.as_bytes()).unwrap();
        assert!(value.is_some());
    }
}

// ============================================================================
// Statistics Tests
// ============================================================================

#[tokio::test]
async fn test_key_count() {
    let store = BTreeKeyValueStore::default();
    let shard = create_test_shard(&store, 1).await;

    assert_eq!(store.key_count(shard).await.unwrap(), 0);

    store.put(shard, b"key1", b"value1").await.unwrap();
    assert_eq!(store.key_count(shard).await.unwrap(), 1);

    store.put(shard, b"key2", b"value2").await.unwrap();
    assert_eq!(store.key_count(shard).await.unwrap(), 2);

    store.delete(shard, b"key1").await.unwrap();
    assert_eq!(store.key_count(shard).await.unwrap(), 1);
}

#[tokio::test]
async fn test_table_stats() {
    let store = BTreeKeyValueStore::default();
    let shard = create_test_shard(&store, 1).await;

    // Insert some data
    for i in 0..100 {
        let key = format!("key{:03}", i);
        let value = format!("value{}", i);
        store
            .put(shard, key.as_bytes(), value.as_bytes())
            .await
            .unwrap();
    }

    let stats = store.shard_stats(shard).await.unwrap();

    assert_eq!(stats.key_count, 100);
    assert!(stats.total_bytes > 0);
}

// ============================================================================
// Multiple Tables Tests
// ============================================================================

#[tokio::test]
async fn test_multiple_tables() {
    let store = BTreeKeyValueStore::default();

    let shard1 = create_test_shard(&store, 1).await;
    let shard2 = create_test_shard(&store, 2).await;

    // Insert different data in each shard
    store.put(shard1, b"key1", b"value1_shard1").await.unwrap();
    store.put(shard2, b"key1", b"value1_shard2").await.unwrap();

    // Verify isolation
    assert_eq!(
        store.get(shard1, b"key1").await.unwrap(),
        Some(b"value1_shard1".to_vec())
    );
    assert_eq!(
        store.get(shard2, b"key1").await.unwrap(),
        Some(b"value1_shard2".to_vec())
    );
}

#[tokio::test]
async fn test_list_tables() {
    let store = BTreeKeyValueStore::default();

    let shard1 = create_test_shard(&store, 1).await;
    let shard2 = create_test_shard(&store, 2).await;
    let shard3 = create_test_shard(&store, 3).await;

    let shards = store.list_shards().await.unwrap();

    assert_eq!(shards.len(), 3);
    assert!(shards.contains(&shard1));
    assert!(shards.contains(&shard2));
    assert!(shards.contains(&shard3));
}

#[tokio::test]
async fn test_drop_table() {
    let store = BTreeKeyValueStore::default();

    let shard = create_test_shard(&store, 1).await;
    store.put(shard, b"key1", b"value1").await.unwrap();

    // Drop the shard
    store.drop_shard(shard).await.unwrap();

    // Shard should no longer exist
    let shards = store.list_shards().await.unwrap();
    assert!(!shards.contains(&shard));
}

// ============================================================================
// Concurrent Access Tests
// ============================================================================

#[tokio::test]
async fn test_concurrent_reads() {
    let store = Arc::new(BTreeKeyValueStore::default());
    let shard = create_test_shard(&store, 1).await;

    // Insert test data
    for i in 0..100 {
        let key = format!("key{:03}", i);
        let value = format!("value{}", i);
        store
            .put(shard, key.as_bytes(), value.as_bytes())
            .await
            .unwrap();
    }

    // Spawn multiple concurrent readers
    let mut handles = vec![];
    for _ in 0..10 {
        let store_clone = store.clone();
        let handle = tokio::spawn(async move {
            for i in 0..100 {
                let key = format!("key{:03}", i);
                let value = store_clone.get(shard, key.as_bytes()).await.unwrap();
                assert!(value.is_some());
            }
        });
        handles.push(handle);
    }

    // Wait for all readers to complete
    for handle in handles {
        handle.await.unwrap();
    }
}

#[tokio::test]
async fn test_concurrent_writes() {
    let store = Arc::new(BTreeKeyValueStore::default());
    let shard = create_test_shard(&store, 1).await;

    // Spawn multiple concurrent writers
    let mut handles = vec![];
    for thread_id in 0..10 {
        let store_clone = store.clone();
        let handle = tokio::spawn(async move {
            for i in 0..100 {
                let key = format!("key_{}_{:03}", thread_id, i);
                let value = format!("value_{}_{}", thread_id, i);
                store_clone
                    .put(shard, key.as_bytes(), value.as_bytes())
                    .await
                    .unwrap();
            }
        });
        handles.push(handle);
    }

    // Wait for all writers to complete
    for handle in handles {
        handle.await.unwrap();
    }

    // Verify all writes succeeded
    let count = store.key_count(shard).await.unwrap();
    assert_eq!(count, 1000); // 10 threads * 100 keys each
}

// ============================================================================
// Edge Cases and Error Handling
// ============================================================================

#[tokio::test]
async fn test_very_large_keys() {
    let store = BTreeKeyValueStore::default();
    let shard = create_test_shard(&store, 1).await;

    let large_key = vec![b'k'; 10_000];
    let value = b"value";

    store.put(shard, &large_key, value).await.unwrap();
    assert_eq!(
        store.get(shard, &large_key).await.unwrap(),
        Some(value.to_vec())
    );
}

#[tokio::test]
async fn test_very_large_values() {
    let store = BTreeKeyValueStore::default();
    let shard = create_test_shard(&store, 1).await;

    let key = b"key";
    let large_value = vec![b'v'; 1_000_000];

    store.put(shard, key, &large_value).await.unwrap();
    assert_eq!(store.get(shard, key).await.unwrap(), Some(large_value));
}

#[tokio::test]
async fn test_duplicate_table_creation() {
    let store = BTreeKeyValueStore::default();

    let table_id = TableId::new(1);
    let shard_index = ShardIndex::new(0);

    let _shard1 = store.create_shard(table_id, shard_index).await.unwrap();

    // Creating a shard with the same table and index should succeed (idempotent)
    // but return the same ShardId
    let shard2 = store.create_shard(table_id, shard_index).await.unwrap();

    // Both should have the same ShardId
    assert_eq!(_shard1, shard2);
}

// ============================================================================
// Transaction Tests
// ============================================================================

#[tokio::test]
async fn test_transaction_basic_operations() {
    let store = Arc::new(BTreeKeyValueStore::default());
    let shard = create_test_shard(&store, 1).await;

    // Insert initial data
    store.put(shard, b"key1", b"value1").await.unwrap();
    store.put(shard, b"key2", b"value2").await.unwrap();

    // Begin transaction
    let tx = store.begin_transaction().await.unwrap();

    // Read existing data
    let val = tx.get(shard, b"key1").await.unwrap();
    assert_eq!(val, Some(b"value1".to_vec()));

    // Write in transaction
    tx.put(shard, b"key3", b"value3").await.unwrap();
    tx.put(shard, b"key1", b"updated1").await.unwrap();

    // Read from transaction buffer
    let val = tx.get(shard, b"key3").await.unwrap();
    assert_eq!(val, Some(b"value3".to_vec()));

    let val = tx.get(shard, b"key1").await.unwrap();
    assert_eq!(val, Some(b"updated1".to_vec()));

    // Data not yet visible outside transaction
    assert_eq!(store.get(shard, b"key3").await.unwrap(), None);
    assert_eq!(
        store.get(shard, b"key1").await.unwrap(),
        Some(b"value1".to_vec())
    );

    // Commit transaction
    tx.commit().await.unwrap();

    // Data now visible
    assert_eq!(
        store.get(shard, b"key3").await.unwrap(),
        Some(b"value3".to_vec())
    );
    assert_eq!(
        store.get(shard, b"key1").await.unwrap(),
        Some(b"updated1".to_vec())
    );
}

#[tokio::test]
async fn test_transaction_rollback() {
    let store = Arc::new(BTreeKeyValueStore::default());
    let shard = create_test_shard(&store, 1).await;

    // Insert initial data
    store.put(shard, b"key1", b"value1").await.unwrap();

    // Begin transaction
    let tx = store.begin_transaction().await.unwrap();

    // Modify data
    tx.put(shard, b"key1", b"updated1").await.unwrap();
    tx.put(shard, b"key2", b"value2").await.unwrap();
    tx.delete(shard, b"key1").await.unwrap();

    // Rollback
    tx.rollback().await.unwrap();

    // Original data unchanged
    assert_eq!(
        store.get(shard, b"key1").await.unwrap(),
        Some(b"value1".to_vec())
    );
    assert_eq!(store.get(shard, b"key2").await.unwrap(), None);
}

#[tokio::test]
async fn test_transaction_delete() {
    let store = Arc::new(BTreeKeyValueStore::default());
    let shard = create_test_shard(&store, 1).await;

    // Insert data
    store.put(shard, b"key1", b"value1").await.unwrap();
    store.put(shard, b"key2", b"value2").await.unwrap();

    // Begin transaction and delete
    let tx = store.begin_transaction().await.unwrap();
    tx.delete(shard, b"key1").await.unwrap();

    // Deleted in transaction
    assert_eq!(tx.get(shard, b"key1").await.unwrap(), None);

    // Still exists outside transaction
    assert_eq!(
        store.get(shard, b"key1").await.unwrap(),
        Some(b"value1".to_vec())
    );

    // Commit
    tx.commit().await.unwrap();

    // Now deleted
    assert_eq!(store.get(shard, b"key1").await.unwrap(), None);
    assert_eq!(
        store.get(shard, b"key2").await.unwrap(),
        Some(b"value2".to_vec())
    );
}

#[tokio::test]
async fn test_transaction_isolation() {
    let store = Arc::new(BTreeKeyValueStore::default());
    let shard = create_test_shard(&store, 1).await;

    // Insert initial data
    store.put(shard, b"key1", b"value1").await.unwrap();

    // Begin two transactions
    let tx1 = store.begin_transaction().await.unwrap();
    let tx2 = store.begin_transaction().await.unwrap();

    // Both see initial data
    assert_eq!(
        tx1.get(shard, b"key1").await.unwrap(),
        Some(b"value1".to_vec())
    );
    assert_eq!(
        tx2.get(shard, b"key1").await.unwrap(),
        Some(b"value1".to_vec())
    );

    // tx1 modifies in its buffer
    tx1.put(shard, b"key1", b"tx1_value").await.unwrap();

    // tx2 doesn't see tx1's uncommitted changes (still in tx1's buffer)
    assert_eq!(
        tx2.get(shard, b"key1").await.unwrap(),
        Some(b"value1".to_vec())
    );

    // tx1 commits - applies changes to tree
    tx1.commit().await.unwrap();

    // Note: Current implementation provides READ COMMITTED isolation
    // tx2 will see tx1's committed changes when reading from the tree
    // For true SNAPSHOT isolation, we would need MVCC with version tracking
    assert_eq!(
        tx2.get(shard, b"key1").await.unwrap(),
        Some(b"tx1_value".to_vec())
    );

    // New reads also see tx1's changes
    assert_eq!(
        store.get(shard, b"key1").await.unwrap(),
        Some(b"tx1_value".to_vec())
    );

    // tx2 can still make its own changes
    tx2.put(shard, b"key2", b"tx2_value").await.unwrap();
    tx2.commit().await.unwrap();

    // Verify both transactions' changes are persisted
    assert_eq!(
        store.get(shard, b"key1").await.unwrap(),
        Some(b"tx1_value".to_vec())
    );
    assert_eq!(
        store.get(shard, b"key2").await.unwrap(),
        Some(b"tx2_value".to_vec())
    );
}

#[tokio::test]
async fn test_transaction_multiple_operations() {
    let store = Arc::new(BTreeKeyValueStore::default());
    let shard = create_test_shard(&store, 1).await;

    let tx = store.begin_transaction().await.unwrap();

    // Multiple puts
    for i in 0..100 {
        let key = format!("key{:03}", i);
        let value = format!("value{}", i);
        tx.put(shard, key.as_bytes(), value.as_bytes())
            .await
            .unwrap();
    }

    // Verify in transaction
    for i in 0..100 {
        let key = format!("key{:03}", i);
        let value = tx.get(shard, key.as_bytes()).await.unwrap();
        assert!(value.is_some());
    }

    // Commit
    tx.commit().await.unwrap();

    // Verify after commit
    let count = store.key_count(shard).await.unwrap();
    assert_eq!(count, 100);
}

#[tokio::test]
async fn test_transaction_commit_applies_changes() {
    let store = Arc::new(BTreeKeyValueStore::default());
    let shard = create_test_shard(&store, 1).await;

    // Insert initial data
    store.put(shard, b"existing", b"value").await.unwrap();

    let tx = store.begin_transaction().await.unwrap();
    tx.put(shard, b"key1", b"value1").await.unwrap();
    tx.put(shard, b"key2", b"value2").await.unwrap();
    tx.delete(shard, b"existing").await.unwrap();

    // Commit
    tx.commit().await.unwrap();

    // Verify all changes applied
    assert_eq!(
        store.get(shard, b"key1").await.unwrap(),
        Some(b"value1".to_vec())
    );
    assert_eq!(
        store.get(shard, b"key2").await.unwrap(),
        Some(b"value2".to_vec())
    );
    assert_eq!(store.get(shard, b"existing").await.unwrap(), None);
}

#[tokio::test]
async fn test_transaction_rollback_discards_changes() {
    let store = Arc::new(BTreeKeyValueStore::default());
    let shard = create_test_shard(&store, 1).await;

    // Insert initial data
    store.put(shard, b"existing", b"value").await.unwrap();

    let tx = store.begin_transaction().await.unwrap();
    tx.put(shard, b"key1", b"value1").await.unwrap();
    tx.put(shard, b"key2", b"value2").await.unwrap();
    tx.delete(shard, b"existing").await.unwrap();

    // Rollback
    tx.rollback().await.unwrap();

    // Verify no changes applied
    assert_eq!(store.get(shard, b"key1").await.unwrap(), None);
    assert_eq!(store.get(shard, b"key2").await.unwrap(), None);
    assert_eq!(
        store.get(shard, b"existing").await.unwrap(),
        Some(b"value".to_vec())
    );
}
