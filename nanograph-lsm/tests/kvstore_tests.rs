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

use nanograph_lsm::LSMKeyValueStore;
use nanograph_kvt::{KeyRange, KeyValueStore};
use std::ops::Bound;
use std::sync::Arc;
use futures::StreamExt;

// ============================================================================
// Basic Operations Tests
// ============================================================================

#[tokio::test]
async fn test_basic_put_get() {
    let store = LSMKeyValueStore::new();
    let table = store.create_table("test").await.unwrap();
    
    // Insert a key-value pair
    store.put(table, b"key1", b"value1").await.unwrap();
    
    // Retrieve it
    let value = store.get(table, b"key1").await.unwrap();
    assert_eq!(value, Some(b"value1".to_vec()));
}

#[tokio::test]
async fn test_put_update() {
    let store = LSMKeyValueStore::new();
    let table = store.create_table("test").await.unwrap();
    
    // Insert initial value
    store.put(table, b"key1", b"value1").await.unwrap();
    
    // Update the value
    store.put(table, b"key1", b"value2").await.unwrap();
    
    // Verify update
    let value = store.get(table, b"key1").await.unwrap();
    assert_eq!(value, Some(b"value2".to_vec()));
}

#[tokio::test]
async fn test_delete() {
    let store = LSMKeyValueStore::new();
    let table = store.create_table("test").await.unwrap();
    
    // Insert and verify
    store.put(table, b"key1", b"value1").await.unwrap();
    assert!(store.exists(table, b"key1").await.unwrap());
    
    // Delete and verify
    let deleted = store.delete(table, b"key1").await.unwrap();
    assert!(deleted);
    assert!(!store.exists(table, b"key1").await.unwrap());
}

// ============================================================================
// Batch Operations Tests
// ============================================================================

#[tokio::test]
async fn test_batch_put() {
    let store = LSMKeyValueStore::new();
    let table = store.create_table("test").await.unwrap();
    
    let pairs = vec![
        (&b"key1"[..], &b"value1"[..]),
        (&b"key2"[..], &b"value2"[..]),
        (&b"key3"[..], &b"value3"[..]),
    ];
    
    store.batch_put(table, &pairs).await.unwrap();
    
    // Verify all keys
    assert_eq!(store.get(table, b"key1").await.unwrap(), Some(b"value1".to_vec()));
    assert_eq!(store.get(table, b"key2").await.unwrap(), Some(b"value2".to_vec()));
    assert_eq!(store.get(table, b"key3").await.unwrap(), Some(b"value3".to_vec()));
}

#[tokio::test]
async fn test_batch_get() {
    let store = LSMKeyValueStore::new();
    let table = store.create_table("test").await.unwrap();
    
    // Insert test data
    store.put(table, b"key1", b"value1").await.unwrap();
    store.put(table, b"key2", b"value2").await.unwrap();
    
    // Batch get
    let keys = vec![&b"key1"[..], &b"key2"[..], &b"key3"[..]];
    let results = store.batch_get(table, &keys).await.unwrap();
    
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
    let table = store.create_table("test").await.unwrap();
    
    // Insert test data
    store.put(table, b"key1", b"value1").await.unwrap();
    store.put(table, b"key2", b"value2").await.unwrap();
    store.put(table, b"key3", b"value3").await.unwrap();
    
    let range = KeyRange {
        start: Bound::Unbounded,
        end: Bound::Unbounded,
        limit: None,
        reverse: false,
    };
    
    let iter = store.scan(table, range).await.unwrap();
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
    let table = store.create_table("test").await.unwrap();
    
    for i in 0..10 {
        let key = format!("key{:02}", i);
        let value = format!("value{}", i);
        store.put(table, key.as_bytes(), value.as_bytes()).await.unwrap();
    }
    
    // Scan from key03 to key07 (inclusive)
    let range = KeyRange {
        start: Bound::Included(b"key03".to_vec()),
        end: Bound::Included(b"key07".to_vec()),
        limit: None,
        reverse: false,
    };
    
    let iter = store.scan(table, range).await.unwrap();
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
    let table = store.create_table("test").await.unwrap();
    
    for i in 0..100 {
        let key = format!("key{:03}", i);
        store.put(table, key.as_bytes(), b"value").await.unwrap();
    }
    
    let range = KeyRange {
        start: Bound::Unbounded,
        end: Bound::Unbounded,
        limit: Some(10),
        reverse: false,
    };
    
    let iter = store.scan(table, range).await.unwrap();
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
    let table = store.create_table("test").await.unwrap();
    
    for i in 0..5 {
        let key = format!("key{}", i);
        let value = format!("value{}", i);
        store.put(table, key.as_bytes(), value.as_bytes()).await.unwrap();
    }
    
    let range = KeyRange {
        start: Bound::Unbounded,
        end: Bound::Unbounded,
        limit: None,
        reverse: true,
    };
    
    let iter = store.scan(table, range).await.unwrap();
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
    let table = store.create_table("test").await.unwrap();
    
    assert_eq!(store.key_count(table).await.unwrap(), 0);
    
    store.put(table, b"key1", b"value1").await.unwrap();
    assert!(store.key_count(table).await.unwrap() > 0);
    
    store.put(table, b"key2", b"value2").await.unwrap();
    assert!(store.key_count(table).await.unwrap() > 0);
}

#[tokio::test]
async fn test_table_stats() {
    let store = LSMKeyValueStore::new();
    let table = store.create_table("test").await.unwrap();
    
    // Insert some data
    for i in 0..100 {
        let key = format!("key{:03}", i);
        let value = format!("value{}", i);
        store.put(table, key.as_bytes(), value.as_bytes()).await.unwrap();
    }
    
    let stats = store.table_stats(table).await.unwrap();
    
    assert!(stats.key_count > 0);
    assert!(stats.total_bytes > 0);
    
    // Check LSM specific stats
    if let nanograph_kvt::EngineStats::Lsm(lsm_stats) = stats.engine_stats {
        assert!(lsm_stats.num_levels > 0);
        assert!(lsm_stats.memtable_bytes > 0);
    } else {
        panic!("Expected LSM stats");
    }
}

// ============================================================================
// Multiple Tables Tests
// ============================================================================

#[tokio::test]
async fn test_multiple_tables() {
    let store = LSMKeyValueStore::new();
    
    let table1 = store.create_table("table1").await.unwrap();
    let table2 = store.create_table("table2").await.unwrap();
    
    // Insert different data in each table
    store.put(table1, b"key1", b"value1_table1").await.unwrap();
    store.put(table2, b"key1", b"value1_table2").await.unwrap();
    
    // Verify isolation
    assert_eq!(
        store.get(table1, b"key1").await.unwrap(),
        Some(b"value1_table1".to_vec())
    );
    assert_eq!(
        store.get(table2, b"key1").await.unwrap(),
        Some(b"value1_table2".to_vec())
    );
}

#[tokio::test]
async fn test_list_tables() {
    let store = LSMKeyValueStore::new();
    
    let _table1 = store.create_table("table1").await.unwrap();
    let _table2 = store.create_table("table2").await.unwrap();
    let _table3 = store.create_table("table3").await.unwrap();
    
    let tables = store.list_tables().await.unwrap();
    
    assert_eq!(tables.len(), 3);
    assert!(tables.iter().any(|(_, name)| name == "table1"));
    assert!(tables.iter().any(|(_, name)| name == "table2"));
    assert!(tables.iter().any(|(_, name)| name == "table3"));
}

#[tokio::test]
async fn test_drop_table() {
    let store = LSMKeyValueStore::new();
    
    let table = store.create_table("test").await.unwrap();
    store.put(table, b"key1", b"value1").await.unwrap();
    
    // Drop the table
    store.drop_table(table).await.unwrap();
    
    // Table should no longer exist
    let tables = store.list_tables().await.unwrap();
    assert!(!tables.iter().any(|(_, name)| name == "test"));
}

// ============================================================================
// Concurrent Access Tests
// ============================================================================

#[tokio::test]
async fn test_concurrent_reads() {
    let store = Arc::new(LSMKeyValueStore::new());
    let table = store.create_table("test").await.unwrap();
    
    // Insert test data
    for i in 0..100 {
        let key = format!("key{:03}", i);
        let value = format!("value{}", i);
        store.put(table, key.as_bytes(), value.as_bytes()).await.unwrap();
    }
    
    // Spawn multiple concurrent readers
    let mut handles = vec![];
    for _ in 0..10 {
        let store_clone = store.clone();
        let handle = tokio::spawn(async move {
            for i in 0..100 {
                let key = format!("key{:03}", i);
                let value = store_clone.get(table, key.as_bytes()).await.unwrap();
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

// Made with Bob