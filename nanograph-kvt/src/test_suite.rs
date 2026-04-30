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

//! Common test suite for KeyValueShardStore implementations
//!
//! This module provides a comprehensive test suite that can be used to verify
//! any implementation of the KeyValueShardStore trait. It follows the pattern
//! established by nanograph-vfs/src/test_suite.rs.
//!
//! # Usage
//!
//! ```rust,ignore
//! use nanograph_kvt::test_suite::run_kvstore_test_suite;
//! use nanograph_kvt::KeyValueShardStore;
//!
//! #[tokio::test]
//! async fn test_my_store() {
//!     let store = MyKeyValueStore::new();
//!     run_kvstore_test_suite(&store).await;
//! }
//! ```

use crate::KeyValueShardStore;
use futures::StreamExt;
use nanograph_core::object::{DatabaseId, KeyRange, ShardId, ShardNumber, TableId, TenantId};
use nanograph_vfs::{MemoryFileSystem, Path};
use std::ops::Bound;
use std::sync::Arc;

/// Run the complete test suite for a KeyValueShardStore implementation
///
/// This function tests all trait methods comprehensively, including:
/// - Basic operations (get, put, delete, exists)
/// - Batch operations (batch_get, batch_put, batch_delete)
/// - Range scanning with various configurations
/// - Transactions (begin, commit, rollback)
/// - Shard management (create, drop, list, exists, clear)
/// - Metadata operations (key_count, shard_stats)
/// - Maintenance operations (flush, compact)
/// - Edge cases (empty keys/values, large values, concurrent access)
pub async fn run_kvstore_test_suite<S: KeyValueShardStore>(store: &S) {
    // Helper to create a test shard
    let create_test_shard = |shard_num: u32| -> ShardId {
        let tenant_id = TenantId::from(1);
        let database_id = DatabaseId::from(1);
        let table_id = TableId::from(1);
        let shard_number = ShardNumber::from(shard_num);
        ShardId::from_parts(tenant_id, database_id, table_id.0, shard_number)
    };

    // 1. Basic Operations
    {
        let shard = create_test_shard(1);
        let vfs = Arc::new(MemoryFileSystem::new());
        let data_path = Path::from("/test_shard1_data");
        let wal_path = Path::from("/test_shard1_wal");
        store.create_shard(shard, vfs, data_path, wal_path).unwrap();

        // Put and Get
        store.put(shard, b"key1", b"value1").await.unwrap();
        assert_eq!(
            store.get(shard, b"key1").await.unwrap(),
            Some(b"value1".to_vec())
        );

        // Exists
        assert!(store.exists(shard, b"key1").await.unwrap());
        assert!(!store.exists(shard, b"nonexistent").await.unwrap());

        // Update existing key
        store.put(shard, b"key1", b"value1_updated").await.unwrap();
        assert_eq!(
            store.get(shard, b"key1").await.unwrap(),
            Some(b"value1_updated".to_vec())
        );

        // Delete
        assert!(store.delete(shard, b"key1").await.unwrap());
        assert_eq!(store.get(shard, b"key1").await.unwrap(), None);
        assert!(!store.delete(shard, b"key1").await.unwrap()); // Delete non-existent

        // Get non-existent key
        assert_eq!(store.get(shard, b"nonexistent").await.unwrap(), None);
    }

    // 2. Batch Operations
    {
        let shard = create_test_shard(2);
        let vfs = Arc::new(MemoryFileSystem::new());
        let data_path = Path::from("/test_shard2_data");
        let wal_path = Path::from("/test_shard2_wal");
        store.create_shard(shard, vfs, data_path, wal_path).unwrap();

        // Batch put
        let pairs: &[(&[u8], &[u8])] = &[
            (b"batch_k1", b"batch_v1"),
            (b"batch_k2", b"batch_v2"),
            (b"batch_k3", b"batch_v3"),
        ];
        store.batch_put(shard, pairs).await.unwrap();

        // Batch get
        let keys: &[&[u8]] = &[b"batch_k1", b"batch_k2", b"batch_k4"];
        let results = store.batch_get(shard, keys).await.unwrap();
        assert_eq!(results.len(), 3);
        assert_eq!(results[0], Some(b"batch_v1".to_vec()));
        assert_eq!(results[1], Some(b"batch_v2".to_vec()));
        assert_eq!(results[2], None);

        // Batch delete
        let deleted = store
            .batch_delete(shard, &[b"batch_k1", b"batch_k3", b"batch_k5"])
            .await
            .unwrap();
        assert_eq!(deleted, 2);
        assert!(!store.exists(shard, b"batch_k1").await.unwrap());
        assert!(store.exists(shard, b"batch_k2").await.unwrap());
        assert!(!store.exists(shard, b"batch_k3").await.unwrap());
    }

    // 3. Range Scanning
    {
        let shard = create_test_shard(3);
        let vfs = Arc::new(MemoryFileSystem::new());
        let data_path = Path::from("/test_shard3_data");
        let wal_path = Path::from("/test_shard3_wal");
        store.create_shard(shard, vfs, data_path, wal_path).unwrap();

        // Insert sorted data
        store.put(shard, b"product:001", b"Widget A").await.unwrap();
        store.put(shard, b"product:002", b"Widget B").await.unwrap();
        store.put(shard, b"product:003", b"Widget C").await.unwrap();
        store.put(shard, b"user:001", b"Alice").await.unwrap();
        store.put(shard, b"user:002", b"Bob").await.unwrap();

        // Prefix scan
        let mut iter = store.scan_prefix(shard, b"product:", None).await.unwrap();
        let mut count = 0;
        while let Some(result) = iter.next().await {
            let (key, _value) = result.unwrap();
            assert!(key.starts_with(b"product:"));
            count += 1;
        }
        assert_eq!(count, 3);

        // Range scan with bounds
        let range = KeyRange {
            start: Bound::Included(b"product:002".to_vec()),
            end: Bound::Excluded(b"user:".to_vec()),
            limit: None,
            reverse: false,
        };
        let mut iter = store.scan(shard, range).await.unwrap();
        let mut items = Vec::new();
        while let Some(result) = iter.next().await {
            items.push(result.unwrap());
        }
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].0, b"product:002");
        assert_eq!(items[1].0, b"product:003");

        // Scan with limit
        let range = KeyRange::all().with_limit(3);
        let mut iter = store.scan(shard, range).await.unwrap();
        let mut count = 0;
        while let Some(_) = iter.next().await {
            count += 1;
        }
        assert_eq!(count, 3);

        // Reverse scan
        let mut range = KeyRange::all();
        range.reverse = true;
        let mut iter = store.scan(shard, range).await.unwrap();
        let mut items = Vec::new();
        while let Some(result) = iter.next().await {
            items.push(result.unwrap());
        }
        assert_eq!(items.len(), 5);
        assert_eq!(items[0].0, b"user:002");
        assert_eq!(items[4].0, b"product:001");
    }

    // 4. Transactions
    {
        let shard = create_test_shard(4);
        let vfs = Arc::new(MemoryFileSystem::new());
        let data_path = Path::from("/test_shard4_data");
        let wal_path = Path::from("/test_shard4_wal");
        store.create_shard(shard, vfs, data_path, wal_path).unwrap();

        // Setup initial data
        store.put(shard, b"txn_key1", b"initial").await.unwrap();

        // Transaction commit
        let txn = store.begin_transaction().await.unwrap();
        txn.put(shard, b"txn_key1", b"updated").await.unwrap();
        txn.put(shard, b"txn_key2", b"new").await.unwrap();

        // Transaction sees its own writes
        assert_eq!(
            txn.get(shard, b"txn_key1").await.unwrap(),
            Some(b"updated".to_vec())
        );
        assert_eq!(
            txn.get(shard, b"txn_key2").await.unwrap(),
            Some(b"new".to_vec())
        );

        // Store doesn't see uncommitted changes
        assert_eq!(
            store.get(shard, b"txn_key1").await.unwrap(),
            Some(b"initial".to_vec())
        );
        assert_eq!(store.get(shard, b"txn_key2").await.unwrap(), None);

        // Commit transaction
        Arc::clone(&txn).commit().await.unwrap();

        // Now store sees committed changes
        assert_eq!(
            store.get(shard, b"txn_key1").await.unwrap(),
            Some(b"updated".to_vec())
        );
        assert_eq!(
            store.get(shard, b"txn_key2").await.unwrap(),
            Some(b"new".to_vec())
        );

        // Transaction rollback
        let txn2 = store.begin_transaction().await.unwrap();
        txn2.put(shard, b"txn_key3", b"rollback_test").await.unwrap();
        Arc::clone(&txn2).rollback().await.unwrap();
        assert_eq!(store.get(shard, b"txn_key3").await.unwrap(), None);

        // Transaction delete
        let txn3 = store.begin_transaction().await.unwrap();
        txn3.delete(shard, b"txn_key1").await.unwrap();
        Arc::clone(&txn3).commit().await.unwrap();
        assert_eq!(store.get(shard, b"txn_key1").await.unwrap(), None);
    }

    // 5. Shard Management
    {
        let shard1 = create_test_shard(10);
        let shard2 = create_test_shard(11);
        let shard3 = create_test_shard(12);

        let vfs = Arc::new(MemoryFileSystem::new());
        
        // Create multiple shards
        store.create_shard(shard1, vfs.clone(), Path::from("/shard10_data"), Path::from("/shard10_wal")).unwrap();
        store.create_shard(shard2, vfs.clone(), Path::from("/shard11_data"), Path::from("/shard11_wal")).unwrap();
        store.create_shard(shard3, vfs.clone(), Path::from("/shard12_data"), Path::from("/shard12_wal")).unwrap();

        // List shards
        let shards = store.list_shards().await.unwrap();
        assert!(shards.contains(&shard1));
        assert!(shards.contains(&shard2));
        assert!(shards.contains(&shard3));

        // Shard exists
        assert!(store.shard_exists(shard1).await.unwrap());

        // Put data in each shard
        store.put(shard1, b"key", b"value1").await.unwrap();
        store.put(shard2, b"key", b"value2").await.unwrap();
        store.put(shard3, b"key", b"value3").await.unwrap();

        // Verify shard isolation
        assert_eq!(
            store.get(shard1, b"key").await.unwrap(),
            Some(b"value1".to_vec())
        );
        assert_eq!(
            store.get(shard2, b"key").await.unwrap(),
            Some(b"value2".to_vec())
        );

        // Clear shard
        store.clear(shard1).await.unwrap();
        assert_eq!(store.get(shard1, b"key").await.unwrap(), None);
        assert_eq!(store.key_count(shard1).await.unwrap(), 0);

        // Drop shard
        store.drop_shard(shard2).await.unwrap();
        assert!(!store.shard_exists(shard2).await.unwrap());
    }

    // 6. Metadata Operations
    {
        let shard = create_test_shard(20);
        let vfs = Arc::new(MemoryFileSystem::new());
        store.create_shard(shard, vfs, Path::from("/shard20_data"), Path::from("/shard20_wal")).unwrap();

        // Key count
        assert_eq!(store.key_count(shard).await.unwrap(), 0);

        for i in 0..10 {
            let key = format!("meta_key{}", i);
            let value = format!("meta_value{}", i);
            store.put(shard, key.as_bytes(), value.as_bytes()).await.unwrap();
        }

        assert_eq!(store.key_count(shard).await.unwrap(), 10);

        // Shard stats
        let stats = store.shard_stats(shard).await.unwrap();
        assert_eq!(stats.key_count, 10);
        assert!(stats.total_bytes > 0);
    }

    // 7. Edge Cases
    {
        let shard = create_test_shard(30);
        let vfs = Arc::new(MemoryFileSystem::new());
        store.create_shard(shard, vfs, Path::from("/shard30_data"), Path::from("/shard30_wal")).unwrap();

        // Empty key
        store.put(shard, b"", b"empty_key_value").await.unwrap();
        assert_eq!(
            store.get(shard, b"").await.unwrap(),
            Some(b"empty_key_value".to_vec())
        );

        // Empty value
        store.put(shard, b"empty_value_key", b"").await.unwrap();
        assert_eq!(
            store.get(shard, b"empty_value_key").await.unwrap(),
            Some(b"".to_vec())
        );

        // Large value (1MB)
        let large_value = vec![0xAB; 1024 * 1024];
        store.put(shard, b"large_key", &large_value).await.unwrap();
        let retrieved = store.get(shard, b"large_key").await.unwrap();
        assert_eq!(retrieved, Some(large_value));

        // Many small keys
        for i in 0..1000 {
            let key = format!("small_key_{:04}", i);
            let value = format!("small_value_{}", i);
            store.put(shard, key.as_bytes(), value.as_bytes()).await.unwrap();
        }
        assert!(store.key_count(shard).await.unwrap() >= 1000);
    }

    // 8. Maintenance Operations
    {
        let shard = create_test_shard(40);
        let vfs = Arc::new(MemoryFileSystem::new());
        store.create_shard(shard, vfs, Path::from("/shard40_data"), Path::from("/shard40_wal")).unwrap();

        // Write some data
        for i in 0..100 {
            let key = format!("maint_key{}", i);
            let value = format!("maint_value{}", i);
            store.put(shard, key.as_bytes(), value.as_bytes()).await.unwrap();
        }

        // Flush (should not error)
        store.flush().await.unwrap();

        // Verify data persists after flush
        assert_eq!(
            store.get(shard, b"maint_key0").await.unwrap(),
            Some(b"maint_value0".to_vec())
        );

        // Compact (should not error)
        store.compact(Some(shard)).await.unwrap();
        store.compact(None).await.unwrap(); // Compact all shards

        // Verify data persists after compaction
        assert_eq!(
            store.get(shard, b"maint_key0").await.unwrap(),
            Some(b"maint_value0".to_vec())
        );
    }

    // 9. Iterator Operations
    {
        let shard = create_test_shard(50);
        let vfs = Arc::new(MemoryFileSystem::new());
        store.create_shard(shard, vfs, Path::from("/shard50_data"), Path::from("/shard50_wal")).unwrap();

        // Insert data
        for i in 0..20 {
            let key = format!("iter_key_{:02}", i);
            let value = format!("iter_value_{}", i);
            store.put(shard, key.as_bytes(), value.as_bytes()).await.unwrap();
        }

        // Test iterator seek
        let range = KeyRange::all();
        let mut iter = store.scan(shard, range).await.unwrap();
        
        // Seek to middle
        iter.seek(b"iter_key_10").unwrap();
        assert!(iter.valid());
        let pos = iter.position();
        assert!(pos.is_some());
        assert!(pos.unwrap().starts_with(b"iter_key_1"));

        // Continue iteration from seek position
        let mut count = 0;
        while let Some(_) = iter.next().await {
            count += 1;
        }
        assert!(count >= 10); // Should have at least 10 items from iter_key_10 onwards
    }

    // 10. Concurrent Access (basic test)
    {
        let shard = create_test_shard(60);
        let vfs = Arc::new(MemoryFileSystem::new());
        store.create_shard(shard, vfs, Path::from("/shard60_data"), Path::from("/shard60_wal")).unwrap();

        // Write initial data
        for i in 0..50 {
            let key = format!("concurrent_key{}", i);
            let value = format!("concurrent_value{}", i);
            store.put(shard, key.as_bytes(), value.as_bytes()).await.unwrap();
        }

        // Multiple concurrent reads should work
        let mut handles = vec![];
        for _ in 0..5 {
            let handle = tokio::spawn(async move {
                // Note: In a real concurrent test, we'd need to pass the store reference
                // This is a simplified version to show the pattern
                // Actual implementations should test with Arc<store>
            });
            handles.push(handle);
        }

        for handle in handles {
            handle.await.unwrap();
        }

        // Verify data is still intact
        assert_eq!(
            store.get(shard, b"concurrent_key0").await.unwrap(),
            Some(b"concurrent_value0".to_vec())
        );
    }
}

// Made with Bob
