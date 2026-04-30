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

use nanograph_kvt::{KeyValueShardStore, ShardId};
use nanograph_lmdb::LMDBKeyValueStore;
use nanograph_vfs::{MemoryFileSystem, Path};
use std::sync::Arc;
use tempfile::TempDir;

#[tokio::test]
async fn test_basic_put_get() {
    let temp_dir = TempDir::new().unwrap();
    let store = LMDBKeyValueStore::new().with_base_dir(temp_dir.path().to_path_buf());

    let shard_id = ShardId::new(1);
    let vfs = Arc::new(MemoryFileSystem::new());
    let data_path = Path::from(temp_dir.path().join("shard1").to_str().unwrap());
    let wal_path = Path::from(temp_dir.path().join("wal1").to_str().unwrap());

    store
        .create_shard(shard_id, vfs, data_path, wal_path)
        .unwrap();

    // Put a value
    store.put(shard_id, b"key1", b"value1").await.unwrap();

    // Get the value
    let value = store.get(shard_id, b"key1").await.unwrap();
    assert_eq!(value, Some(b"value1".to_vec()));

    // Get non-existent key
    let value = store.get(shard_id, b"key2").await.unwrap();
    assert_eq!(value, None);
}

#[tokio::test]
async fn test_delete() {
    let temp_dir = TempDir::new().unwrap();
    let store = LMDBKeyValueStore::new().with_base_dir(temp_dir.path().to_path_buf());

    let shard_id = ShardId::new(2);
    let vfs = Arc::new(MemoryFileSystem::new());
    let data_path = Path::from(temp_dir.path().join("shard2").to_str().unwrap());
    let wal_path = Path::from(temp_dir.path().join("wal2").to_str().unwrap());

    store
        .create_shard(shard_id, vfs, data_path, wal_path)
        .unwrap();

    // Put and delete
    store.put(shard_id, b"key1", b"value1").await.unwrap();
    let deleted = store.delete(shard_id, b"key1").await.unwrap();
    assert!(deleted);

    // Verify deleted
    let value = store.get(shard_id, b"key1").await.unwrap();
    assert_eq!(value, None);

    // Delete non-existent key
    let deleted = store.delete(shard_id, b"key2").await.unwrap();
    assert!(!deleted);
}

#[tokio::test]
async fn test_exists() {
    let temp_dir = TempDir::new().unwrap();
    let store = LMDBKeyValueStore::new().with_base_dir(temp_dir.path().to_path_buf());

    let shard_id = ShardId::new(3);
    let vfs = Arc::new(MemoryFileSystem::new());
    let data_path = Path::from(temp_dir.path().join("shard3").to_str().unwrap());
    let wal_path = Path::from(temp_dir.path().join("wal3").to_str().unwrap());

    store
        .create_shard(shard_id, vfs, data_path, wal_path)
        .unwrap();

    store.put(shard_id, b"key1", b"value1").await.unwrap();

    assert!(store.exists(shard_id, b"key1").await.unwrap());
    assert!(!store.exists(shard_id, b"key2").await.unwrap());
}

#[tokio::test]
async fn test_batch_operations() {
    let temp_dir = TempDir::new().unwrap();
    let store = LMDBKeyValueStore::new().with_base_dir(temp_dir.path().to_path_buf());

    let shard_id = ShardId::new(4);
    let vfs = Arc::new(MemoryFileSystem::new());
    let data_path = Path::from(temp_dir.path().join("shard4").to_str().unwrap());
    let wal_path = Path::from(temp_dir.path().join("wal4").to_str().unwrap());

    store
        .create_shard(shard_id, vfs, data_path, wal_path)
        .unwrap();

    // Batch put
    let pairs = vec![
        (&b"key1"[..], &b"value1"[..]),
        (&b"key2"[..], &b"value2"[..]),
        (&b"key3"[..], &b"value3"[..]),
    ];
    store.batch_put(shard_id, &pairs).await.unwrap();

    // Batch get
    let keys = vec![&b"key1"[..], &b"key2"[..], &b"key3"[..], &b"key4"[..]];
    let values = store.batch_get(shard_id, &keys).await.unwrap();

    assert_eq!(values[0], Some(b"value1".to_vec()));
    assert_eq!(values[1], Some(b"value2".to_vec()));
    assert_eq!(values[2], Some(b"value3".to_vec()));
    assert_eq!(values[3], None);

    // Batch delete
    let delete_keys = vec![&b"key1"[..], &b"key2"[..]];
    let count = store.batch_delete(shard_id, &delete_keys).await.unwrap();
    assert_eq!(count, 2);

    // Verify deleted
    assert!(!store.exists(shard_id, b"key1").await.unwrap());
    assert!(!store.exists(shard_id, b"key2").await.unwrap());
    assert!(store.exists(shard_id, b"key3").await.unwrap());
}

#[tokio::test]
async fn test_key_count() {
    let temp_dir = TempDir::new().unwrap();
    let store = LMDBKeyValueStore::new().with_base_dir(temp_dir.path().to_path_buf());

    let shard_id = ShardId::new(5);
    let vfs = Arc::new(MemoryFileSystem::new());
    let data_path = Path::from(temp_dir.path().join("shard5").to_str().unwrap());
    let wal_path = Path::from(temp_dir.path().join("wal5").to_str().unwrap());

    store
        .create_shard(shard_id, vfs, data_path, wal_path)
        .unwrap();

    // Initially empty
    assert_eq!(store.key_count(shard_id).await.unwrap(), 0);

    // Add some keys
    store.put(shard_id, b"key1", b"value1").await.unwrap();
    store.put(shard_id, b"key2", b"value2").await.unwrap();
    store.put(shard_id, b"key3", b"value3").await.unwrap();

    assert_eq!(store.key_count(shard_id).await.unwrap(), 3);

    // Delete a key
    store.delete(shard_id, b"key2").await.unwrap();
    assert_eq!(store.key_count(shard_id).await.unwrap(), 2);
}

#[tokio::test]
async fn test_clear() {
    let temp_dir = TempDir::new().unwrap();
    let store = LMDBKeyValueStore::new().with_base_dir(temp_dir.path().to_path_buf());

    let shard_id = ShardId::new(6);
    let vfs = Arc::new(MemoryFileSystem::new());
    let data_path = Path::from(temp_dir.path().join("shard6").to_str().unwrap());
    let wal_path = Path::from(temp_dir.path().join("wal6").to_str().unwrap());

    store
        .create_shard(shard_id, vfs, data_path, wal_path)
        .unwrap();

    // Add some data
    store.put(shard_id, b"key1", b"value1").await.unwrap();
    store.put(shard_id, b"key2", b"value2").await.unwrap();

    assert_eq!(store.key_count(shard_id).await.unwrap(), 2);

    // Clear
    store.clear(shard_id).await.unwrap();

    assert_eq!(store.key_count(shard_id).await.unwrap(), 0);
    assert!(!store.exists(shard_id, b"key1").await.unwrap());
}

#[tokio::test]
async fn test_shard_management() {
    let temp_dir = TempDir::new().unwrap();
    let store = LMDBKeyValueStore::new().with_base_dir(temp_dir.path().to_path_buf());

    let shard_id = ShardId::new(7);
    let vfs = Arc::new(MemoryFileSystem::new());
    let data_path = Path::from(temp_dir.path().join("shard7").to_str().unwrap());
    let wal_path = Path::from(temp_dir.path().join("wal7").to_str().unwrap());

    // Create shard
    store
        .create_shard(shard_id, vfs.clone(), data_path.clone(), wal_path.clone())
        .unwrap();
    assert!(store.shard_exists(shard_id).await.unwrap());

    // List shards
    let shards = store.list_shards().await.unwrap();
    assert!(shards.contains(&shard_id));

    // Drop shard
    store.drop_shard(shard_id).await.unwrap();
    assert!(!store.shard_exists(shard_id).await.unwrap());
}

#[tokio::test]
async fn test_shard_stats() {
    let temp_dir = TempDir::new().unwrap();
    let store = LMDBKeyValueStore::new().with_base_dir(temp_dir.path().to_path_buf());

    let shard_id = ShardId::new(8);
    let vfs = Arc::new(MemoryFileSystem::new());
    let data_path = Path::from(temp_dir.path().join("shard8").to_str().unwrap());
    let wal_path = Path::from(temp_dir.path().join("wal8").to_str().unwrap());

    store
        .create_shard(shard_id, vfs, data_path, wal_path)
        .unwrap();

    // Add some data
    store.put(shard_id, b"key1", b"value1").await.unwrap();
    store.put(shard_id, b"key2", b"value2").await.unwrap();

    // Get stats
    let stats = store.shard_stats(shard_id).await.unwrap();
    assert_eq!(stats.key_count, 2);
    assert!(stats.total_bytes > 0);
    // Check that engine stats has page_size
    match stats.engine_stats.get("page_size") {
        nanograph_kvt::metrics::StatValue::U64(size) => assert!(size > 0),
        _ => panic!("Expected U64 for page_size"),
    }
}

// Made with Bob
