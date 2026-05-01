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

//! Comprehensive integration tests for ordered indexes (B-Tree and Hash)

use nanograph_core::object::{
    DatabaseId, IndexId, IndexRecord, IndexSharding, IndexStatus, IndexType, ObjectId, ShardId,
    ShardNumber, TenantId,
};
use nanograph_core::types::Timestamp;
use nanograph_idx::btree::BTreeIndex;
use nanograph_idx::hash::HashIndex;
use nanograph_idx::{
    IndexEntry, IndexQuery, IndexStore, OrderedIndex, PersistenceConfig, UniqueIndex,
};
use nanograph_kvt::{KeyValueShardStore, MemoryKeyValueShardStore};
use std::collections::HashMap;
use std::ops::Bound;
use std::sync::Arc;

// Helper functions

fn create_btree_metadata() -> IndexRecord {
    IndexRecord {
        index_id: IndexId::new(ObjectId::new(1)),
        name: "test_btree_idx".to_string(),
        version: 0,
        index_type: IndexType::Secondary,
        created_at: Timestamp::now(),
        updated_at: Timestamp::now(),
        columns: vec!["value".to_string()],
        key_extractor: None,
        options: HashMap::new(),
        metadata: HashMap::new(),
        status: IndexStatus::Building,
        sharding: IndexSharding::Single,
    }
}

fn create_hash_metadata() -> IndexRecord {
    IndexRecord {
        index_id: IndexId::new(ObjectId::new(2)),
        name: "test_hash_idx".to_string(),
        version: 0,
        index_type: IndexType::Unique,
        created_at: Timestamp::now(),
        updated_at: Timestamp::now(),
        columns: vec!["email".to_string()],
        key_extractor: None,
        options: HashMap::new(),
        metadata: HashMap::new(),
        status: IndexStatus::Building,
        sharding: IndexSharding::Single,
    }
}

fn create_persistence_config(index_id: u32) -> PersistenceConfig {
    PersistenceConfig {
        shard_id: ShardId::from_parts(
            TenantId::new(1),
            DatabaseId::new(1),
            ObjectId::new(index_id),
            ShardNumber(0),
        ),
        index_id: IndexId::new(ObjectId::new(index_id)),
        cache_size: 1000,
        durability: nanograph_wal::Durability::Buffered,
        enable_wal: false, // Disable WAL for tests
    }
}

// Helper to create shard in store
fn create_test_shard(store: &MemoryKeyValueShardStore, shard_id: ShardId) {
    store
        .create_shard(
            shard_id,
            Arc::new(nanograph_vfs::MemoryFileSystem::new()),
            nanograph_vfs::Path::from("/data"),
            nanograph_vfs::Path::from("/wal"),
        )
        .unwrap();
}

// B-Tree Index Tests

#[tokio::test]
async fn test_btree_basic_operations() {
    let metadata = create_btree_metadata();
    let store = Arc::new(MemoryKeyValueShardStore::new());
    let config = create_persistence_config(1);
    create_test_shard(&store, config.shard_id);

    let mut index = BTreeIndex::new(metadata, store, None, config)
        .await
        .unwrap();

    // Insert entries
    for i in 0..10 {
        let entry = IndexEntry {
            indexed_value: format!("value{:03}", i).into_bytes(),
            primary_key: format!("key{}", i).into_bytes(),
            included_columns: None,
        };
        index.insert(entry).await.unwrap();
    }

    // Test exists
    assert!(index.exists(b"value005").await.unwrap());
    assert!(!index.exists(b"value999").await.unwrap());

    // Test stats
    let stats = index.stats().await.unwrap();
    assert_eq!(stats.entry_count, 10);
}

#[tokio::test]
async fn test_btree_range_queries() {
    let metadata = create_btree_metadata();
    let store = Arc::new(MemoryKeyValueShardStore::new());
    let config = create_persistence_config(1);
    create_test_shard(&store, config.shard_id);

    let mut index = BTreeIndex::new(metadata, store, None, config)
        .await
        .unwrap();

    // Insert test data
    for i in 0..20 {
        let entry = IndexEntry {
            indexed_value: format!("value{:03}", i).into_bytes(),
            primary_key: format!("key{}", i).into_bytes(),
            included_columns: None,
        };
        index.insert(entry).await.unwrap();
    }

    // Test range scan
    let results = index
        .range_scan(
            Bound::Included(b"value005".to_vec()),
            Bound::Included(b"value010".to_vec()),
            None,
            false,
        )
        .await
        .unwrap();

    assert_eq!(results.len(), 6); // value005 through value010

    // Test with limit
    let limited = index
        .range_scan(
            Bound::Included(b"value000".to_vec()),
            Bound::Included(b"value019".to_vec()),
            Some(5),
            false,
        )
        .await
        .unwrap();

    assert_eq!(limited.len(), 5);
}

#[tokio::test]
async fn test_btree_prefix_scan() {
    let metadata = create_btree_metadata();
    let store = Arc::new(MemoryKeyValueShardStore::new());
    let config = create_persistence_config(1);
    create_test_shard(&store, config.shard_id);

    let mut index = BTreeIndex::new(metadata, store, None, config)
        .await
        .unwrap();

    // Insert entries with common prefix
    for i in 0..5 {
        let entry = IndexEntry {
            indexed_value: format!("user_{}", i).into_bytes(),
            primary_key: format!("key{}", i).into_bytes(),
            included_columns: None,
        };
        index.insert(entry).await.unwrap();
    }

    for i in 0..3 {
        let entry = IndexEntry {
            indexed_value: format!("admin_{}", i).into_bytes(),
            primary_key: format!("admin_key{}", i).into_bytes(),
            included_columns: None,
        };
        index.insert(entry).await.unwrap();
    }

    // Test prefix scan
    let user_results = index.prefix_scan(b"user_", None).await.unwrap();
    assert_eq!(user_results.len(), 5);

    let admin_results = index.prefix_scan(b"admin_", None).await.unwrap();
    assert_eq!(admin_results.len(), 3);
}

#[tokio::test]
async fn test_btree_min_max_keys() {
    let metadata = create_btree_metadata();
    let store = Arc::new(MemoryKeyValueShardStore::new());
    let config = create_persistence_config(1);
    create_test_shard(&store, config.shard_id);

    let mut index = BTreeIndex::new(metadata, store, None, config)
        .await
        .unwrap();

    // Insert entries
    for i in [5, 2, 8, 1, 9, 3] {
        let entry = IndexEntry {
            indexed_value: format!("value{:03}", i).into_bytes(),
            primary_key: format!("key{}", i).into_bytes(),
            included_columns: None,
        };
        index.insert(entry).await.unwrap();
    }

    // Test min/max
    let min = index.min_key().await.unwrap();
    assert_eq!(min, Some(b"value001".to_vec()));

    let max = index.max_key().await.unwrap();
    assert_eq!(max, Some(b"value009".to_vec()));
}

#[tokio::test]
async fn test_btree_count_range() {
    let metadata = create_btree_metadata();
    let store = Arc::new(MemoryKeyValueShardStore::new());
    let config = create_persistence_config(1);
    create_test_shard(&store, config.shard_id);

    let mut index = BTreeIndex::new(metadata, store, None, config)
        .await
        .unwrap();

    // Insert entries
    for i in 0..100 {
        let entry = IndexEntry {
            indexed_value: format!("value{:03}", i).into_bytes(),
            primary_key: format!("key{}", i).into_bytes(),
            included_columns: None,
        };
        index.insert(entry).await.unwrap();
    }

    // Count range
    let count = index
        .count_range(
            Bound::Included(b"value010".to_vec()),
            Bound::Included(b"value020".to_vec()),
        )
        .await
        .unwrap();

    assert_eq!(count, 11); // value010 through value020
}

#[tokio::test]
async fn test_btree_unique_constraint() {
    let mut metadata = create_btree_metadata();
    metadata.index_type = IndexType::Unique;

    let store = Arc::new(MemoryKeyValueShardStore::new());
    let config = create_persistence_config(1);
    create_test_shard(&store, config.shard_id);

    let mut index = BTreeIndex::new(metadata, store, None, config)
        .await
        .unwrap();

    // Insert first entry
    let entry1 = IndexEntry {
        indexed_value: b"unique_value".to_vec(),
        primary_key: b"key1".to_vec(),
        included_columns: None,
    };
    assert!(index.insert(entry1).await.is_ok());

    // Try to insert duplicate
    let entry2 = IndexEntry {
        indexed_value: b"unique_value".to_vec(),
        primary_key: b"key2".to_vec(),
        included_columns: None,
    };
    assert!(index.insert(entry2).await.is_err());

    // Validate unique
    assert!(index.validate_unique(b"unique_value").await.is_err());
    assert!(index.validate_unique(b"other_value").await.is_ok());
}

// Hash Index Tests

#[tokio::test]
async fn test_hash_basic_operations() {
    let metadata = create_hash_metadata();
    let store = Arc::new(MemoryKeyValueShardStore::new());
    let config = create_persistence_config(2);
    create_test_shard(&store, config.shard_id);

    let mut index = HashIndex::new(metadata, store, None, config).await.unwrap();

    // Insert entries
    for i in 0..10 {
        let entry = IndexEntry {
            indexed_value: format!("email{}@example.com", i).into_bytes(),
            primary_key: format!("user{}", i).into_bytes(),
            included_columns: None,
        };
        index.insert(entry).await.unwrap();
    }

    // Test exists
    assert!(index.exists(b"email5@example.com").await.unwrap());
    assert!(!index.exists(b"notfound@example.com").await.unwrap());

    // Test stats
    let stats = index.stats().await.unwrap();
    assert_eq!(stats.entry_count, 10);
}

#[tokio::test]
async fn test_hash_unique_constraint() {
    let metadata = create_hash_metadata();
    let store = Arc::new(MemoryKeyValueShardStore::new());
    let config = create_persistence_config(2);
    create_test_shard(&store, config.shard_id);

    let mut index = HashIndex::new(metadata, store, None, config).await.unwrap();

    // Insert first entry
    let entry1 = IndexEntry {
        indexed_value: b"test@example.com".to_vec(),
        primary_key: b"user1".to_vec(),
        included_columns: None,
    };
    assert!(index.insert(entry1).await.is_ok());

    // Try to insert duplicate
    let entry2 = IndexEntry {
        indexed_value: b"test@example.com".to_vec(),
        primary_key: b"user2".to_vec(),
        included_columns: None,
    };
    assert!(index.insert(entry2).await.is_err());
}

#[tokio::test]
async fn test_hash_lookup_unique() {
    let metadata = create_hash_metadata();
    let store = Arc::new(MemoryKeyValueShardStore::new());
    let config = create_persistence_config(2);
    create_test_shard(&store, config.shard_id);

    let mut index = HashIndex::new(metadata, store, None, config).await.unwrap();

    // Insert entries
    let entry = IndexEntry {
        indexed_value: b"john@example.com".to_vec(),
        primary_key: b"user123".to_vec(),
        included_columns: None,
    };
    index.insert(entry).await.unwrap();

    // Lookup
    let result = index.lookup_unique(b"john@example.com").await.unwrap();
    assert_eq!(result, Some(b"user123".to_vec()));

    let not_found = index.lookup_unique(b"notfound@example.com").await.unwrap();
    assert_eq!(not_found, None);
}

#[tokio::test]
async fn test_hash_exact_match_query() {
    let metadata = create_hash_metadata();
    let store = Arc::new(MemoryKeyValueShardStore::new());
    let config = create_persistence_config(2);
    create_test_shard(&store, config.shard_id);

    let mut index = HashIndex::new(metadata, store, None, config).await.unwrap();

    // Insert entry
    let entry = IndexEntry {
        indexed_value: b"test@example.com".to_vec(),
        primary_key: b"user1".to_vec(),
        included_columns: None,
    };
    index.insert(entry).await.unwrap();

    // Exact match query
    let query = IndexQuery::exact(b"test@example.com".to_vec());
    let results = index.query(query).await.unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].primary_key, b"user1");
}

// Persistence Tests

#[tokio::test]
async fn test_btree_persistence_and_recovery() {
    let metadata = create_btree_metadata();
    let store = Arc::new(MemoryKeyValueShardStore::new());
    let config = create_persistence_config(1);
    create_test_shard(&store, config.shard_id);

    // Create index and insert data
    {
        let mut index = BTreeIndex::new(metadata.clone(), store.clone(), None, config.clone())
            .await
            .unwrap();

        for i in 0..10 {
            let entry = IndexEntry {
                indexed_value: format!("value{:03}", i).into_bytes(),
                primary_key: format!("key{}", i).into_bytes(),
                included_columns: None,
            };
            index.insert(entry).await.unwrap();
        }

        index.flush().await.unwrap();
    }

    // Create new index instance (simulating recovery)
    {
        let index = BTreeIndex::new(metadata, store, None, config)
            .await
            .unwrap();

        // Data should still be accessible
        assert!(index.exists(b"value005").await.unwrap());

        let stats = index.stats().await.unwrap();
        assert_eq!(stats.entry_count, 10);
    }
}

#[tokio::test]
async fn test_hash_persistence_and_recovery() {
    let metadata = create_hash_metadata();
    let store = Arc::new(MemoryKeyValueShardStore::new());
    let config = create_persistence_config(2);
    create_test_shard(&store, config.shard_id);

    // Create index and insert data
    {
        let mut index = HashIndex::new(metadata.clone(), store.clone(), None, config.clone())
            .await
            .unwrap();

        for i in 0..10 {
            let entry = IndexEntry {
                indexed_value: format!("email{}@example.com", i).into_bytes(),
                primary_key: format!("user{}", i).into_bytes(),
                included_columns: None,
            };
            index.insert(entry).await.unwrap();
        }

        index.flush().await.unwrap();
    }

    // Create new index instance (simulating recovery)
    {
        let index = HashIndex::new(metadata, store, None, config).await.unwrap();

        // Data should still be accessible
        assert!(index.exists(b"email5@example.com").await.unwrap());

        let stats = index.stats().await.unwrap();
        assert_eq!(stats.entry_count, 10);
    }
}

// Made with Bob
