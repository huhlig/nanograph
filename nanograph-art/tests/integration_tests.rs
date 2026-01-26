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

//! Comprehensive integration tests for Adaptive Radix Tree

mod test_utils;

use futures::StreamExt;
use nanograph_art::{AdaptiveRadixTree, ArtKeyValueStore};
use nanograph_kvt::{IndexNumber, KeyRange, KeyValueShardStore, ShardId, TableId};
use std::ops::Bound;
use std::sync::Arc;
use test_utils::*;
use tokio::task;

// ============================================================================
// Helper Functions
// ============================================================================

async fn create_test_shard(store: &ArtKeyValueStore) -> ShardId {
    let shard_id = ShardId::new(1);
    store.create_shard(shard_id).await.unwrap();
    shard_id
}

// ============================================================================
// Basic Operations Tests
// ============================================================================

#[tokio::test]
async fn test_kvstore_basic_put_get() {
    let store = ArtKeyValueStore::default();
    let shard = create_test_shard(&store).await;

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
async fn test_kvstore_update_existing_key() {
    let store = ArtKeyValueStore::default();
    let shard = create_test_shard(&store).await;

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
async fn test_kvstore_delete() {
    let store = ArtKeyValueStore::default();
    let shard = create_test_shard(&store).await;

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
async fn test_kvstore_multiple_operations() {
    let store = ArtKeyValueStore::default();
    let shard = create_test_shard(&store).await;

    let kvs = generate_sequential_kvs(100);

    // Insert all
    for (key, value) in &kvs {
        store.put(shard, key, value).await.unwrap();
    }

    // Verify all
    for (key, value) in &kvs {
        assert_eq!(store.get(shard, key).await.unwrap(), Some(value.clone()));
    }

    // Delete half
    for (key, _) in kvs.iter().take(50) {
        store.delete(shard, key).await.unwrap();
    }

    // Verify deletions
    for (key, _) in kvs.iter().take(50) {
        assert_eq!(store.get(shard, key).await.unwrap(), None);
    }

    // Verify remaining
    for (key, value) in kvs.iter().skip(50) {
        assert_eq!(store.get(shard, key).await.unwrap(), Some(value.clone()));
    }
}

// ============================================================================
// Range Query Tests
// ============================================================================

#[tokio::test]
async fn test_kvstore_range_query_full() {
    let store = ArtKeyValueStore::default();
    let shard = create_test_shard(&store).await;

    let kvs = generate_sequential_kvs(50);
    for (key, value) in &kvs {
        store.put(shard, key, value).await.unwrap();
    }

    let range = KeyRange::new(Bound::Unbounded, Bound::Unbounded);
    let mut stream = store.scan(shard, range).await.unwrap();

    let mut count = 0;
    while let Some(result) = stream.next().await {
        result.unwrap();
        count += 1;
    }

    assert_eq!(count, 50);
}

#[tokio::test]
async fn test_kvstore_range_query_bounded() {
    let store = ArtKeyValueStore::default();
    let shard = create_test_shard(&store).await;

    let kvs = generate_sequential_kvs(100);
    for (key, value) in &kvs {
        store.put(shard, key, value).await.unwrap();
    }

    let start_key = format!("key{:08}", 20).into_bytes();
    let end_key = format!("key{:08}", 30).into_bytes();
    let range = KeyRange::new(Bound::Included(start_key), Bound::Excluded(end_key));

    let mut stream = store.scan(shard, range).await.unwrap();
    let mut count = 0;

    while let Some(result) = stream.next().await {
        result.unwrap();
        count += 1;
    }

    assert_eq!(count, 10);
}

#[tokio::test]
async fn test_kvstore_prefix_scan() {
    let store = ArtKeyValueStore::default();
    let shard = create_test_shard(&store).await;

    // Insert keys with different prefixes
    store.put(shard, b"user:1:name", b"Alice").await.unwrap();
    store
        .put(shard, b"user:1:email", b"alice@example.com")
        .await
        .unwrap();
    store.put(shard, b"user:2:name", b"Bob").await.unwrap();
    store
        .put(shard, b"user:2:email", b"bob@example.com")
        .await
        .unwrap();
    store
        .put(shard, b"product:1:name", b"Widget")
        .await
        .unwrap();

    // Scan for user:1 prefix
    let range = KeyRange::new(
        Bound::Included(b"user:1:".to_vec()),
        Bound::Excluded(b"user:1;".to_vec()),
    );

    let mut stream = store.scan(shard, range).await.unwrap();
    let mut count = 0;

    while let Some(result) = stream.next().await {
        result.unwrap();
        count += 1;
    }

    assert_eq!(count, 2);
}

// ============================================================================
// Concurrent Operations Tests
// ============================================================================

#[tokio::test]
async fn test_concurrent_inserts() {
    let store = Arc::new(ArtKeyValueStore::default());
    let shard = create_test_shard(&store).await;

    let mut handles = vec![];

    for i in 0..10 {
        let store_clone = Arc::clone(&store);
        let handle = task::spawn(async move {
            for j in 0..100 {
                let key = format!("key_{}_{}", i, j).into_bytes();
                let value = format!("value_{}_{}", i, j).into_bytes();
                store_clone.put(shard, &key, &value).await.unwrap();
            }
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.await.unwrap();
    }

    // Verify all keys were inserted
    for i in 0..10 {
        for j in 0..100 {
            let key = format!("key_{}_{}", i, j).into_bytes();
            let value = store.get(shard, &key).await.unwrap();
            assert!(value.is_some());
        }
    }
}

#[tokio::test]
async fn test_concurrent_reads_writes() {
    let store = Arc::new(ArtKeyValueStore::default());
    let shard = create_test_shard(&store).await;

    // Pre-populate
    for i in 0..100 {
        let key = format!("key{:03}", i).into_bytes();
        let value = format!("value{}", i).into_bytes();
        store.put(shard, &key, &value).await.unwrap();
    }

    let mut handles = vec![];

    // Spawn readers
    for _ in 0..5 {
        let store_clone = Arc::clone(&store);
        let handle = task::spawn(async move {
            for i in 0..100 {
                let key = format!("key{:03}", i).into_bytes();
                let _ = store_clone.get(shard, &key).await.unwrap();
            }
        });
        handles.push(handle);
    }

    // Spawn writers
    for i in 0..5 {
        let store_clone = Arc::clone(&store);
        let handle = task::spawn(async move {
            for j in 0..100 {
                let key = format!("key{:03}", j).into_bytes();
                let value = format!("updated_{}_{}", i, j).into_bytes();
                store_clone.put(shard, &key, &value).await.unwrap();
            }
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.await.unwrap();
    }
}

// ============================================================================
// Stress Tests
// ============================================================================

#[tokio::test]
async fn test_large_dataset() {
    let store = ArtKeyValueStore::default();
    let shard = create_test_shard(&store).await;

    let count = 10_000;
    let kvs = generate_sequential_kvs(count);

    // Insert all
    for (key, value) in &kvs {
        store.put(shard, key, value).await.unwrap();
    }

    // Verify random samples
    for i in (0..count).step_by(100) {
        let key = format!("key{:08}", i).into_bytes();
        let value = store.get(shard, &key).await.unwrap();
        assert!(value.is_some());
    }
}

#[tokio::test]
async fn test_variable_length_keys() {
    let store = ArtKeyValueStore::default();
    let shard = create_test_shard(&store).await;

    let keys = generate_variable_length_keys(100);

    for (i, key) in keys.iter().enumerate() {
        let value = format!("value{}", i).into_bytes();
        store.put(shard, key, &value).await.unwrap();
    }

    for (i, key) in keys.iter().enumerate() {
        let value = store.get(shard, key).await.unwrap();
        assert_eq!(value, Some(format!("value{}", i).into_bytes()));
    }
}

#[tokio::test]
async fn test_binary_keys_and_values() {
    let store = ArtKeyValueStore::default();
    let shard = create_test_shard(&store).await;

    let binary_kvs = vec![
        (vec![0x00, 0xFF, 0x80], vec![0xDE, 0xAD, 0xBE, 0xEF]),
        (vec![0xFF, 0x00, 0x7F], vec![0xCA, 0xFE, 0xBA, 0xBE]),
        (vec![0x12, 0x34, 0x56, 0x78], vec![0x9A, 0xBC, 0xDE, 0xF0]),
    ];

    for (key, value) in &binary_kvs {
        store.put(shard, key, value).await.unwrap();
    }

    for (key, expected_value) in &binary_kvs {
        let value = store.get(shard, key).await.unwrap();
        assert_eq!(value, Some(expected_value.clone()));
    }
}

// ============================================================================
// Iterator Tests
// ============================================================================

#[tokio::test]
async fn test_iterator_ordering() {
    let mut tree = AdaptiveRadixTree::new();

    let keys = vec![
        b"apple".to_vec(),
        b"banana".to_vec(),
        b"cherry".to_vec(),
        b"date".to_vec(),
    ];

    for (i, key) in keys.iter().enumerate() {
        tree.insert(key.clone(), i).unwrap();
    }

    let collected: Vec<_> = tree.iter().collect();
    assert_eq!(collected.len(), 4);

    // Verify all keys are present
    let collected_keys: Vec<_> = collected.iter().map(|(k, _)| k.clone()).collect();
    for key in &keys {
        assert!(collected_keys.contains(key));
    }
}

#[tokio::test]
async fn test_range_iterator() {
    let mut tree = AdaptiveRadixTree::new();

    for i in 0..100 {
        let key = format!("key{:03}", i).into_bytes();
        tree.insert(key, i).unwrap();
    }

    let start = b"key020".to_vec();
    let end = b"key030".to_vec();

    let range_items: Vec<_> = tree.range(Some(start), Some(end), true).collect();

    assert_eq!(range_items.len(), 10);
}

// ============================================================================
// Edge Cases
// ============================================================================

#[tokio::test]
async fn test_empty_tree_operations() {
    let tree: AdaptiveRadixTree<i32> = AdaptiveRadixTree::new();

    assert_eq!(tree.len(), 0);
    assert!(tree.is_empty());
    assert_eq!(tree.get(b"any_key"), None);
    assert!(!tree.contains_key(b"any_key"));
}

#[tokio::test]
async fn test_single_key_operations() {
    let mut tree = AdaptiveRadixTree::new();

    tree.insert(b"only_key".to_vec(), 42).unwrap();
    assert_eq!(tree.len(), 1);
    assert_eq!(tree.get(b"only_key"), Some(42));

    tree.remove(b"only_key").unwrap();
    assert_eq!(tree.len(), 0);
    assert!(tree.is_empty());
}

#[tokio::test]
async fn test_duplicate_insertions() {
    let mut tree = AdaptiveRadixTree::new();

    assert!(tree.insert(b"key".to_vec(), 1).unwrap().is_none());
    assert_eq!(tree.insert(b"key".to_vec(), 2).unwrap(), Some(1));
    assert_eq!(tree.insert(b"key".to_vec(), 3).unwrap(), Some(2));

    assert_eq!(tree.len(), 1);
    assert_eq!(tree.get(b"key"), Some(3));
}

#[tokio::test]
async fn test_common_prefix_handling() {
    let mut tree = AdaptiveRadixTree::new();

    let keys = vec![
        b"test".to_vec(),
        b"testing".to_vec(),
        b"tester".to_vec(),
        b"tested".to_vec(),
        b"tea".to_vec(),
        b"team".to_vec(),
    ];

    for (i, key) in keys.iter().enumerate() {
        tree.insert(key.clone(), i).unwrap();
    }

    assert_eq!(tree.len(), keys.len());

    for (i, key) in keys.iter().enumerate() {
        assert_eq!(tree.get(key), Some(i));
    }
}

#[tokio::test]
async fn test_node_transitions() {
    let mut tree = AdaptiveRadixTree::new();

    // Insert enough keys to trigger all node type transitions
    let keys = generate_node_growth_keys();

    for (i, key) in keys.iter().enumerate() {
        tree.insert(key.clone(), i).unwrap();
    }

    // Verify all keys are retrievable
    for (i, key) in keys.iter().enumerate() {
        assert_eq!(tree.get(key), Some(i));
    }

    // Remove keys to trigger shrinking
    for key in keys.iter().take(keys.len() / 2) {
        tree.remove(key).unwrap();
    }

    // Verify remaining keys
    for (i, key) in keys.iter().enumerate().skip(keys.len() / 2) {
        assert_eq!(tree.get(key), Some(i));
    }
}

#[tokio::test]
async fn test_memory_usage_tracking() {
    let mut tree = AdaptiveRadixTree::new();

    let initial_memory = tree.memory_usage();

    for i in 0..1000 {
        let key = format!("key{:06}", i).into_bytes();
        tree.insert(key, i).unwrap();
    }

    let final_memory = tree.memory_usage();
    assert!(final_memory > initial_memory);
}

// ============================================================================
// Multi-Shard Tests
// ============================================================================

#[tokio::test]
async fn test_multiple_shards() {
    let store = ArtKeyValueStore::default();
    let shard1 = ShardId::new(1);
    let shard2 = ShardId::new(2);

    store.create_shard(shard1).await.unwrap();
    store.create_shard(shard2).await.unwrap();

    // Insert into shard1
    store.put(shard1, b"key1", b"value1").await.unwrap();

    // Insert into shard2
    store.put(shard2, b"key1", b"value2").await.unwrap();

    // Verify isolation
    assert_eq!(
        store.get(shard1, b"key1").await.unwrap(),
        Some(b"value1".to_vec())
    );
    assert_eq!(
        store.get(shard2, b"key1").await.unwrap(),
        Some(b"value2".to_vec())
    );
}
