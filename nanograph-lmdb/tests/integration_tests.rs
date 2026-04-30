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

use futures::StreamExt;
use nanograph_kvt::{KeyRange, KeyValueShardStore, ShardId};
use nanograph_lmdb::LMDBKeyValueStore;
use nanograph_vfs::{MemoryFileSystem, Path};
use std::sync::Arc;
use tempfile::TempDir;

#[tokio::test]
async fn test_range_scan() {
    let temp_dir = TempDir::new().unwrap();
    let store = LMDBKeyValueStore::new().with_base_dir(temp_dir.path().to_path_buf());

    let shard_id = ShardId::new(1);
    let vfs = Arc::new(MemoryFileSystem::new());
    let data_path = Path::from(temp_dir.path().join("shard1").to_str().unwrap());
    let wal_path = Path::from(temp_dir.path().join("wal1").to_str().unwrap());

    store
        .create_shard(shard_id, vfs, data_path, wal_path)
        .unwrap();

    // Insert sorted data
    store
        .put(shard_id, b"product:001", b"Widget A")
        .await
        .unwrap();
    store
        .put(shard_id, b"product:002", b"Widget B")
        .await
        .unwrap();
    store
        .put(shard_id, b"product:003", b"Widget C")
        .await
        .unwrap();
    store.put(shard_id, b"user:001", b"Alice").await.unwrap();
    store.put(shard_id, b"user:002", b"Bob").await.unwrap();

    // Scan with prefix
    let range = KeyRange::prefix(b"product:".to_vec());
    let mut iter = store.scan(shard_id, range).await.unwrap();

    let mut count = 0;
    while let Some(result) = iter.next().await {
        let (key, _value) = result.unwrap();
        assert!(key.starts_with(b"product:"));
        count += 1;
    }
    assert_eq!(count, 3);
}

#[tokio::test]
async fn test_range_scan_with_limit() {
    let temp_dir = TempDir::new().unwrap();
    let store = LMDBKeyValueStore::new().with_base_dir(temp_dir.path().to_path_buf());

    let shard_id = ShardId::new(2);
    let vfs = Arc::new(MemoryFileSystem::new());
    let data_path = Path::from(temp_dir.path().join("shard2").to_str().unwrap());
    let wal_path = Path::from(temp_dir.path().join("wal2").to_str().unwrap());

    store
        .create_shard(shard_id, vfs, data_path, wal_path)
        .unwrap();

    // Insert data
    for i in 0..10 {
        let key = format!("key{:03}", i);
        let value = format!("value{}", i);
        store
            .put(shard_id, key.as_bytes(), value.as_bytes())
            .await
            .unwrap();
    }

    // Scan with limit
    let range = KeyRange::all().with_limit(5);
    let mut iter = store.scan(shard_id, range).await.unwrap();

    let mut count = 0;
    while let Some(_) = iter.next().await {
        count += 1;
    }
    assert_eq!(count, 5);
}

#[tokio::test]
async fn test_multiple_shards() {
    let temp_dir = TempDir::new().unwrap();
    let store = LMDBKeyValueStore::new().with_base_dir(temp_dir.path().to_path_buf());

    // Create multiple shards
    for i in 1..=3 {
        let shard_id = ShardId::new(i);
        let vfs = Arc::new(MemoryFileSystem::new());
        let data_path = Path::from(
            temp_dir
                .path()
                .join(format!("shard{}", i))
                .to_str()
                .unwrap(),
        );
        let wal_path = Path::from(temp_dir.path().join(format!("wal{}", i)).to_str().unwrap());

        store
            .create_shard(shard_id, vfs, data_path, wal_path)
            .unwrap();

        // Put data in each shard
        let key = format!("key{}", i);
        let value = format!("value{}", i);
        store
            .put(shard_id, key.as_bytes(), value.as_bytes())
            .await
            .unwrap();
    }

    // Verify each shard has its own data
    for i in 1..=3 {
        let shard_id = ShardId::new(i);
        let key = format!("key{}", i);
        let value = store.get(shard_id, key.as_bytes()).await.unwrap();
        assert_eq!(value, Some(format!("value{}", i).into_bytes()));
    }

    // Verify shards are isolated
    let shard1 = ShardId::new(1);
    let value = store.get(shard1, b"key2").await.unwrap();
    assert_eq!(value, None);
}

#[tokio::test]
async fn test_large_values() {
    let temp_dir = TempDir::new().unwrap();
    let store = LMDBKeyValueStore::new().with_base_dir(temp_dir.path().to_path_buf());

    let shard_id = ShardId::new(1);
    let vfs = Arc::new(MemoryFileSystem::new());
    let data_path = Path::from(temp_dir.path().join("shard1").to_str().unwrap());
    let wal_path = Path::from(temp_dir.path().join("wal1").to_str().unwrap());

    store
        .create_shard(shard_id, vfs, data_path, wal_path)
        .unwrap();

    // Create a large value (1MB)
    let large_value = vec![0u8; 1024 * 1024];
    store
        .put(shard_id, b"large_key", &large_value)
        .await
        .unwrap();

    // Retrieve and verify
    let retrieved = store.get(shard_id, b"large_key").await.unwrap();
    assert_eq!(retrieved, Some(large_value));
}

#[tokio::test]
async fn test_persistence() {
    let temp_dir = TempDir::new().unwrap();
    let shard_id = ShardId::new(1);
    let data_path_str = temp_dir.path().join("shard1").to_str().unwrap().to_string();
    let wal_path_str = temp_dir.path().join("wal1").to_str().unwrap().to_string();

    // Create store and add data
    {
        let store = LMDBKeyValueStore::new().with_base_dir(temp_dir.path().to_path_buf());
        let vfs = Arc::new(MemoryFileSystem::new());
        let data_path = Path::from(data_path_str.as_str());
        let wal_path = Path::from(wal_path_str.as_str());

        store
            .create_shard(shard_id, vfs, data_path, wal_path)
            .unwrap();
        store
            .put(shard_id, b"persistent_key", b"persistent_value")
            .await
            .unwrap();

        // Flush to ensure data is written
        store.flush().await.unwrap();
    }

    // Reopen and verify data persists
    {
        let store = LMDBKeyValueStore::new().with_base_dir(temp_dir.path().to_path_buf());
        let vfs = Arc::new(MemoryFileSystem::new());
        let data_path = Path::from(data_path_str.as_str());
        let wal_path = Path::from(wal_path_str.as_str());

        store
            .create_shard(shard_id, vfs, data_path, wal_path)
            .unwrap();

        let value = store.get(shard_id, b"persistent_key").await.unwrap();
        assert_eq!(value, Some(b"persistent_value".to_vec()));
    }
}

#[tokio::test]
async fn test_concurrent_reads() {
    let temp_dir = TempDir::new().unwrap();
    let store = Arc::new(LMDBKeyValueStore::new().with_base_dir(temp_dir.path().to_path_buf()));

    let shard_id = ShardId::new(1);
    let vfs = Arc::new(MemoryFileSystem::new());
    let data_path = Path::from(temp_dir.path().join("shard1").to_str().unwrap());
    let wal_path = Path::from(temp_dir.path().join("wal1").to_str().unwrap());

    store
        .create_shard(shard_id, vfs, data_path, wal_path)
        .unwrap();

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
        let store_clone = Arc::clone(&store);
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

#[tokio::test]
async fn test_update_existing_key() {
    let temp_dir = TempDir::new().unwrap();
    let store = LMDBKeyValueStore::new().with_base_dir(temp_dir.path().to_path_buf());

    let shard_id = ShardId::new(1);
    let vfs = Arc::new(MemoryFileSystem::new());
    let data_path = Path::from(temp_dir.path().join("shard1").to_str().unwrap());
    let wal_path = Path::from(temp_dir.path().join("wal1").to_str().unwrap());

    store
        .create_shard(shard_id, vfs, data_path, wal_path)
        .unwrap();

    // Insert initial value
    store.put(shard_id, b"key1", b"value1").await.unwrap();
    let value = store.get(shard_id, b"key1").await.unwrap();
    assert_eq!(value, Some(b"value1".to_vec()));

    // Update value
    store.put(shard_id, b"key1", b"value2").await.unwrap();
    let value = store.get(shard_id, b"key1").await.unwrap();
    assert_eq!(value, Some(b"value2".to_vec()));

    // Key count should still be 1
    assert_eq!(store.key_count(shard_id).await.unwrap(), 1);
}

// Made with Bob
