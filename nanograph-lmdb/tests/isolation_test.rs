//! Minimal test to debug transaction isolation issue

use nanograph_kvt::{KeyValueShardStore, ShardId};
use nanograph_lmdb::LMDBKeyValueStore;
use nanograph_vfs::{MemoryFileSystem, Path};
use std::sync::Arc;
use tempfile::TempDir;

#[tokio::test]
async fn test_transaction_isolation() {
    // Use a unique temp directory for this test
    let temp_dir = tempfile::tempdir().unwrap();
    let store = LMDBKeyValueStore::new().with_base_dir(temp_dir.path().to_path_buf());
    
    let shard = ShardId(4);
    
    let vfs = Arc::new(MemoryFileSystem::new());
    // Use a path within the temp directory
    let data_path = Path::from("shard4_data");
    let wal_path = Path::from("shard4_wal");
    
    store.create_shard(shard, vfs, data_path, wal_path).unwrap();
    
    // Setup initial data
    store.put(shard, b"txn_key1", b"initial").await.unwrap();
    
    // Create transaction and buffer writes
    let txn = store.begin_transaction().await.unwrap();
    txn.put(shard, b"txn_key1", b"updated").await.unwrap();
    txn.put(shard, b"txn_key2", b"new").await.unwrap();
    
    // Transaction should see its own writes
    let txn_result1 = txn.get(shard, b"txn_key1").await.unwrap();
    println!("Transaction sees txn_key1: {:?}", txn_result1);
    assert_eq!(txn_result1, Some(b"updated".to_vec()));
    
    let txn_result2 = txn.get(shard, b"txn_key2").await.unwrap();
    println!("Transaction sees txn_key2: {:?}", txn_result2);
    assert_eq!(txn_result2, Some(b"new".to_vec()));
    
    // Store should NOT see uncommitted writes
    let store_result1 = store.get(shard, b"txn_key1").await.unwrap();
    println!("Store sees txn_key1: {:?}", store_result1);
    assert_eq!(store_result1, Some(b"initial".to_vec()));
    
    let store_result2 = store.get(shard, b"txn_key2").await.unwrap();
    println!("Store sees txn_key2: {:?}", store_result2);
    assert_eq!(store_result2, None, "Store should not see uncommitted transaction data!");
}

// Made with Bob
