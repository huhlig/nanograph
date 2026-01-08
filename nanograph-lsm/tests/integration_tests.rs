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

//! Integration tests for nanograph-lsm

use nanograph_lsm::{LSMTreeEngine, LSMTreeOptions};
use nanograph_vfs::{DynamicFileSystem, MemoryFileSystem, Path};
use nanograph_wal::{WriteAheadLogConfig, WriteAheadLogManager};
use std::sync::Arc;
use tempfile::TempDir;

/// Helper to create a test engine
fn create_test_engine() -> (LSMTreeEngine, TempDir) {
    let temp_dir = TempDir::new().unwrap();
    let base_path = temp_dir.path().to_str().unwrap().to_string();

    // Create memory filesystems for WAL and SSTables
    let wal_fs = MemoryFileSystem::new();
    let sstable_fs: Arc<dyn DynamicFileSystem> = Arc::new(MemoryFileSystem::new());
    let wal_path = Path::from("/wal");
    let wal_config = WriteAheadLogConfig::new(0);
    let wal = WriteAheadLogManager::new(wal_fs, wal_path, wal_config).unwrap();

    let options = LSMTreeOptions::default();
    let engine = LSMTreeEngine::new(sstable_fs, base_path, options, wal).unwrap();

    (engine, temp_dir)
}

#[test]
fn test_basic_put_get() {
    let (engine, _temp_dir) = create_test_engine();

    // Put a key-value pair
    engine.put(b"key1".to_vec(), b"value1".to_vec()).unwrap();

    // Get the value
    let value = engine.get(b"key1").unwrap();
    assert_eq!(value, Some(b"value1".to_vec()));
}

#[test]
fn test_put_update() {
    let (engine, _temp_dir) = create_test_engine();

    // Put initial value
    engine.put(b"key1".to_vec(), b"value1".to_vec()).unwrap();

    // Update the value
    engine.put(b"key1".to_vec(), b"value2".to_vec()).unwrap();

    // Get the updated value
    let value = engine.get(b"key1").unwrap();
    assert_eq!(value, Some(b"value2".to_vec()));
}

#[test]
fn test_delete() {
    let (engine, _temp_dir) = create_test_engine();

    // Put a key-value pair
    engine.put(b"key1".to_vec(), b"value1".to_vec()).unwrap();

    // Verify it exists
    assert!(engine.get(b"key1").unwrap().is_some());

    // Delete the key
    engine.delete(b"key1".to_vec()).unwrap();

    // Verify it's gone
    assert!(engine.get(b"key1").unwrap().is_none());
}

#[test]
fn test_multiple_keys() {
    let (engine, _temp_dir) = create_test_engine();

    // Put multiple keys
    for i in 0..100 {
        let key = format!("key{:03}", i);
        let value = format!("value{:03}", i);
        engine.put(key.into_bytes(), value.into_bytes()).unwrap();
    }

    // Verify all keys
    for i in 0..100 {
        let key = format!("key{:03}", i);
        let expected_value = format!("value{:03}", i);
        let value = engine.get(key.as_bytes()).unwrap();
        assert_eq!(value, Some(expected_value.into_bytes()));
    }
}

#[test]
fn test_nonexistent_key() {
    let (engine, _temp_dir) = create_test_engine();

    // Try to get a key that doesn't exist
    let value = engine.get(b"nonexistent").unwrap();
    assert_eq!(value, None);
}

#[test]
fn test_memtable_flush() {
    let (engine, _temp_dir) = create_test_engine();

    // Fill memtable with enough data to trigger flush
    let large_value = vec![b'x'; 1024]; // 1KB value
    for i in 0..100 {
        let key = format!("key{:03}", i);
        engine.put(key.into_bytes(), large_value.clone()).unwrap();
    }

    // Force flush
    engine.flush().unwrap();

    // Verify data is still accessible after flush
    for i in 0..100 {
        let key = format!("key{:03}", i);
        let value = engine.get(key.as_bytes()).unwrap();
        assert_eq!(value, Some(large_value.clone()));
    }
}

#[test]
fn test_wal_recovery() {
    let (engine, _temp_dir) = create_test_engine();

    // Write some data
    engine.put(b"key1".to_vec(), b"value1".to_vec()).unwrap();
    engine.put(b"key2".to_vec(), b"value2".to_vec()).unwrap();
    engine.delete(b"key3".to_vec()).unwrap();

    // Recover from WAL (simulates restart)
    engine.recover().unwrap();

    // Verify data is still accessible after recovery
    assert_eq!(engine.get(b"key1").unwrap(), Some(b"value1".to_vec()));
    assert_eq!(engine.get(b"key2").unwrap(), Some(b"value2".to_vec()));
    assert_eq!(engine.get(b"key3").unwrap(), None);
}

#[test]
fn test_large_values() {
    let (engine, _temp_dir) = create_test_engine();

    // Test with 1MB value
    let large_value = vec![b'x'; 1024 * 1024];
    engine
        .put(b"large_key".to_vec(), large_value.clone())
        .unwrap();

    let retrieved = engine.get(b"large_key").unwrap();
    assert_eq!(retrieved, Some(large_value));
}

#[test]
fn test_empty_key_value() {
    let (engine, _temp_dir) = create_test_engine();

    // Test empty key
    engine.put(b"".to_vec(), b"value".to_vec()).unwrap();
    assert_eq!(engine.get(b"").unwrap(), Some(b"value".to_vec()));

    // Test empty value
    engine.put(b"key".to_vec(), b"".to_vec()).unwrap();
    assert_eq!(engine.get(b"key").unwrap(), Some(b"".to_vec()));
}

#[test]
fn test_stats() {
    let (engine, _temp_dir) = create_test_engine();

    // Write some data
    for i in 0..10 {
        let key = format!("key{}", i);
        let value = format!("value{}", i);
        engine.put(key.into_bytes(), value.into_bytes()).unwrap();
    }

    // Read some data
    for i in 0..5 {
        let key = format!("key{}", i);
        engine.get(key.as_bytes()).unwrap();
    }

    // Check stats
    let stats = engine.stats();
    assert_eq!(stats.total_writes, 10);
    assert_eq!(stats.total_reads, 5);
    assert!(stats.memtable_size > 0);
}

#[test]
fn test_sequential_writes() {
    let (engine, _temp_dir) = create_test_engine();

    // Write keys in sequential order
    for i in 0..1000 {
        let key = format!("{:010}", i); // Zero-padded for sorting
        let value = format!("value{}", i);
        engine.put(key.into_bytes(), value.into_bytes()).unwrap();
    }

    // Verify all keys
    for i in 0..1000 {
        let key = format!("{:010}", i);
        let expected_value = format!("value{}", i);
        let value = engine.get(key.as_bytes()).unwrap();
        assert_eq!(value, Some(expected_value.into_bytes()));
    }
}

#[test]
fn test_random_access() {
    let (engine, _temp_dir) = create_test_engine();

    // Write keys
    for i in 0..100 {
        let key = format!("key{}", i);
        let value = format!("value{}", i);
        engine.put(key.into_bytes(), value.into_bytes()).unwrap();
    }

    // Random access pattern
    let access_pattern = vec![50, 10, 90, 5, 75, 25, 95, 15];
    for i in access_pattern {
        let key = format!("key{}", i);
        let expected_value = format!("value{}", i);
        let value = engine.get(key.as_bytes()).unwrap();
        assert_eq!(value, Some(expected_value.into_bytes()));
    }
}

#[test]
fn test_overwrite_pattern() {
    let (engine, _temp_dir) = create_test_engine();

    // Write initial values
    for i in 0..50 {
        let key = format!("key{}", i);
        let value = format!("value{}_v1", i);
        engine.put(key.into_bytes(), value.into_bytes()).unwrap();
    }

    // Overwrite with new values
    for i in 0..50 {
        let key = format!("key{}", i);
        let value = format!("value{}_v2", i);
        engine.put(key.into_bytes(), value.into_bytes()).unwrap();
    }

    // Verify latest values
    for i in 0..50 {
        let key = format!("key{}", i);
        let expected_value = format!("value{}_v2", i);
        let value = engine.get(key.as_bytes()).unwrap();
        assert_eq!(value, Some(expected_value.into_bytes()));
    }
}

#[test]
fn test_delete_nonexistent() {
    let (engine, _temp_dir) = create_test_engine();

    // Delete a key that doesn't exist (should not error)
    engine.delete(b"nonexistent".to_vec()).unwrap();

    // Verify it's still not there
    assert_eq!(engine.get(b"nonexistent").unwrap(), None);
}

#[test]
fn test_mixed_operations() {
    let (engine, _temp_dir) = create_test_engine();

    // Mix of puts, gets, and deletes
    engine.put(b"key1".to_vec(), b"value1".to_vec()).unwrap();
    assert_eq!(engine.get(b"key1").unwrap(), Some(b"value1".to_vec()));

    engine.put(b"key2".to_vec(), b"value2".to_vec()).unwrap();
    engine.delete(b"key1".to_vec()).unwrap();

    assert_eq!(engine.get(b"key1").unwrap(), None);
    assert_eq!(engine.get(b"key2").unwrap(), Some(b"value2".to_vec()));

    engine
        .put(b"key1".to_vec(), b"value1_new".to_vec())
        .unwrap();
    assert_eq!(engine.get(b"key1").unwrap(), Some(b"value1_new".to_vec()));
}

// Made with Bob
