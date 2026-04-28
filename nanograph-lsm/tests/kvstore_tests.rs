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

//! KeyValueStore trait integration tests for LSM

use futures::StreamExt;
use nanograph_kvt::{KeyRange, KeyValueShardStore, ShardId};
use nanograph_lsm::LSMKeyValueStore;
use nanograph_vfs::{MemoryFileSystem, Path};
use std::ops::Bound;
use std::sync::Arc;

// Helper function to create a shard with dummy VFS and paths
fn create_test_shard(store: &LSMKeyValueStore, shard_id: ShardId) {
    let vfs = Arc::new(MemoryFileSystem::new());
    let data_path = Path::from("/data");
    let wal_path = Path::from("/wal");
    store.create_shard(shard_id, vfs, data_path, wal_path).unwrap();
}

// ============================================================================
// Basic Operations Tests
// ============================================================================

#[tokio::test]
async fn test_basic_put_get() {
    let store = LSMKeyValueStore::new();

    let shard_id = ShardId::new(1);
    create_test_shard(&store, shard_id);

    // Insert a key-value pair
    store.put(shard_id, b"key1", b"value1").await.unwrap();

    // Retrieve it
    let value = store.get(shard_id, b"key1").await.unwrap();
    assert_eq!(value, Some(b"value1".to_vec()));
}

#[tokio::test]
async fn test_put_update() {
    let store = LSMKeyValueStore::new();
    let shard_id = ShardId::new(1);
    create_test_shard(&store, shard_id);

    // Insert initial value
    store.put(shard_id, b"key1", b"value1").await.unwrap();

    // Update the value
    store.put(shard_id, b"key1", b"value2").await.unwrap();

    // Verify update
    let value = store.get(shard_id, b"key1").await.unwrap();
    assert_eq!(value, Some(b"value2".to_vec()));
}

#[tokio::test]
async fn test_delete() {
    let store = LSMKeyValueStore::new();

    let shard_id = ShardId::new(1);
    create_test_shard(&store, shard_id);

    // Insert and verify
    store.put(shard_id, b"key1", b"value1").await.unwrap();
    assert!(store.exists(shard_id, b"key1").await.unwrap());

    // Delete and verify
    let deleted = store.delete(shard_id, b"key1").await.unwrap();
    assert!(deleted);
    assert!(!store.exists(shard_id, b"key1").await.unwrap());
}

// ============================================================================
// Batch Operations Tests
// ============================================================================

#[tokio::test]
async fn test_batch_put() {
    let store = LSMKeyValueStore::new();

    let shard_id = ShardId::new(1);
    create_test_shard(&store, shard_id);

    let pairs = vec![
        (&b"key1"[..], &b"value1"[..]),
        (&b"key2"[..], &b"value2"[..]),
        (&b"key3"[..], &b"value3"[..]),
    ];

    store.batch_put(shard_id, &pairs).await.unwrap();

    // Verify all keys
    assert_eq!(
        store.get(shard_id, b"key1").await.unwrap(),
        Some(b"value1".to_vec())
    );
    assert_eq!(
        store.get(shard_id, b"key2").await.unwrap(),
        Some(b"value2".to_vec())
    );
    assert_eq!(
        store.get(shard_id, b"key3").await.unwrap(),
        Some(b"value3".to_vec())
    );
}

#[tokio::test]
async fn test_batch_get() {
    let store = LSMKeyValueStore::new();

    let shard_id = ShardId::new(1);
    create_test_shard(&store, shard_id);

    // Insert test data
    store.put(shard_id, b"key1", b"value1").await.unwrap();
    store.put(shard_id, b"key2", b"value2").await.unwrap();

    // Batch get
    let keys = vec![&b"key1"[..], &b"key2"[..], &b"key3"[..]];
    let results = store.batch_get(shard_id, &keys).await.unwrap();

    assert_eq!(results[0], Some(b"value1".to_vec()));
    assert_eq!(results[1], Some(b"value2".to_vec()));
    assert_eq!(results[2], None);
}

// ============================================================================
// Range Scan Tests
// ============================================================================

#[tokio::test]
async fn test_basic_scan() {
    let store = LSMKeyValueStore::new();

    let shard_id = ShardId::new(1);
    create_test_shard(&store, shard_id);

    // Insert test data
    store.put(shard_id, b"key1", b"value1").await.unwrap();
    store.put(shard_id, b"key2", b"value2").await.unwrap();
    store.put(shard_id, b"key3", b"value3").await.unwrap();

    let range = KeyRange {
        start: Bound::Unbounded,
        end: Bound::Unbounded,
        limit: None,
        reverse: false,
    };

    let iter = store.scan(shard_id, range).await.unwrap();
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
    let store = LSMKeyValueStore::new();
    let shard_id = ShardId::new(1);
    create_test_shard(&store, shard_id);

    for i in 0..10 {
        let key = format!("key{:02}", i);
        let value = format!("value{}", i);
        store
            .put(shard_id, key.as_bytes(), value.as_bytes())
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

    let iter = store.scan(shard_id, range).await.unwrap();
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
    let store = LSMKeyValueStore::new();
    let shard_id = ShardId::new(1);
    create_test_shard(&store, shard_id);

    for i in 0..100 {
        let key = format!("key{:03}", i);
        store.put(shard_id, key.as_bytes(), b"value").await.unwrap();
    }

    let range = KeyRange {
        start: Bound::Unbounded,
        end: Bound::Unbounded,
        limit: Some(10),
        reverse: false,
    };

    let iter = store.scan(shard_id, range).await.unwrap();
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
    let store = LSMKeyValueStore::new();
    let shard_id = ShardId::new(1);
    create_test_shard(&store, shard_id);

    for i in 0..5 {
        let key = format!("key{}", i);
        let value = format!("value{}", i);
        store
            .put(shard_id, key.as_bytes(), value.as_bytes())
            .await
            .unwrap();
    }

    let range = KeyRange {
        start: Bound::Unbounded,
        end: Bound::Unbounded,
        limit: None,
        reverse: true,
    };

    let iter = store.scan(shard_id, range).await.unwrap();
    let mut iter = Box::pin(iter);
    let mut results = Vec::new();

    while let Some(result) = iter.as_mut().next().await {
        match result {
            Ok((key, value)) => results.push((key, value)),
            Err(_) => break,
        }
    }

    // Should be in reverse order
    assert_eq!(results.len(), 5);
    assert_eq!(results[0].0, b"key4");
    assert_eq!(results[4].0, b"key0");
}

// ============================================================================
// Statistics Tests
// ============================================================================

#[tokio::test]
async fn test_key_count() {
    let store = LSMKeyValueStore::new();
    let shard_id = ShardId::new(1);
    create_test_shard(&store, shard_id);

    assert_eq!(store.key_count(shard_id).await.unwrap(), 0);

    store.put(shard_id, b"key1", b"value1").await.unwrap();
    assert!(store.key_count(shard_id).await.unwrap() > 0);

    store.put(shard_id, b"key2", b"value2").await.unwrap();
    assert!(store.key_count(shard_id).await.unwrap() > 0);
}

#[tokio::test]
async fn test_table_stats() {
    let store = LSMKeyValueStore::new();
    let shard_id = ShardId::new(1);
    create_test_shard(&store, shard_id);

    // Insert some data
    for i in 0..100 {
        let key = format!("key{:03}", i);
        let value = format!("value{}", i);
        store
            .put(shard_id, key.as_bytes(), value.as_bytes())
            .await
            .unwrap();
    }

    let stats = store.shard_stats(shard_id).await.unwrap();

    assert!(stats.key_count > 0);
    assert!(stats.total_bytes > 0);
}

// ============================================================================
// Multiple Tables Tests
// ============================================================================

#[tokio::test]
async fn test_multiple_tables() {
    let store = LSMKeyValueStore::new();

    let shard1 = ShardId::new(1);
    create_test_shard(&store, shard1);
    let shard2 = ShardId::new(2);
    create_test_shard(&store, shard2);

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
    let store = LSMKeyValueStore::new();

    let shard1 = ShardId::new(1);
    create_test_shard(&store, shard1);
    let shard2 = ShardId::new(2);
    create_test_shard(&store, shard2);
    let shard3 = ShardId::new(3);
    create_test_shard(&store, shard3);

    let shards = store.list_shards().await.unwrap();

    assert_eq!(shards.len(), 3);
    assert!(shards.contains(&shard1));
    assert!(shards.contains(&shard2));
    assert!(shards.contains(&shard3));
}

#[tokio::test]
async fn test_drop_table() {
    let store = LSMKeyValueStore::new();

    let shard_id = ShardId::new(1);
    create_test_shard(&store, shard_id);
    store.put(shard_id, b"key1", b"value1").await.unwrap();

    // Drop the shard
    store.drop_shard(shard_id).await.unwrap();

    // Shard should no longer exist
    let shards = store.list_shards().await.unwrap();
    assert!(!shards.contains(&shard_id));
}

// ============================================================================
// Concurrent Access Tests
// ============================================================================

#[tokio::test]
async fn test_concurrent_reads() {
    let store = Arc::new(LSMKeyValueStore::new());
    let shard_id = ShardId::new(1);
    create_test_shard(&store, shard_id);

    // Insert test data
    for i in 0..100 {
        let key = format!("key{:03}", i);
        let value = format!("value{}", i);
        store
            .put(shard_id, key.as_bytes(), value.as_bytes())
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
                let value = store_clone.get(shard_id, key.as_bytes()).await.unwrap();
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
