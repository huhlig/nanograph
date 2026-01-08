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

use nanograph_art::AdaptiveRadixTree;

#[test]
fn test_empty_tree() {
    let tree: AdaptiveRadixTree<i32> = AdaptiveRadixTree::new();
    assert_eq!(tree.len(), 0);
    assert!(tree.is_empty());
    assert_eq!(tree.get(b"key"), None);
}

#[test]
fn test_single_insert_and_get() {
    let mut tree = AdaptiveRadixTree::new();
    assert!(tree.insert(b"hello".to_vec(), 42).unwrap().is_none());
    assert_eq!(tree.len(), 1);
    assert_eq!(tree.get(b"hello"), Some(42));
    assert_eq!(tree.get(b"world"), None);
}

#[test]
fn test_multiple_inserts() {
    let mut tree = AdaptiveRadixTree::new();

    tree.insert(b"apple".to_vec(), 1).unwrap();
    tree.insert(b"banana".to_vec(), 2).unwrap();
    tree.insert(b"cherry".to_vec(), 3).unwrap();

    assert_eq!(tree.len(), 3);
    assert_eq!(tree.get(b"apple"), Some(1));
    assert_eq!(tree.get(b"banana"), Some(2));
    assert_eq!(tree.get(b"cherry"), Some(3));
}

#[test]
fn test_update_existing_key() {
    let mut tree = AdaptiveRadixTree::new();

    assert!(tree.insert(b"key".to_vec(), 100).unwrap().is_none());
    assert_eq!(tree.get(b"key"), Some(100));

    let old_value = tree.insert(b"key".to_vec(), 200).unwrap();
    assert_eq!(old_value, Some(100));
    assert_eq!(tree.get(b"key"), Some(200));
    assert_eq!(tree.len(), 1);
}

#[test]
fn test_remove() {
    let mut tree = AdaptiveRadixTree::new();

    tree.insert(b"key1".to_vec(), 1).unwrap();
    tree.insert(b"key2".to_vec(), 2).unwrap();
    tree.insert(b"key3".to_vec(), 3).unwrap();

    assert_eq!(tree.remove(b"key2").unwrap(), Some(2));
    assert_eq!(tree.len(), 2);
    assert_eq!(tree.get(b"key2"), None);
    assert_eq!(tree.get(b"key1"), Some(1));
    assert_eq!(tree.get(b"key3"), Some(3));
}

#[test]
fn test_remove_nonexistent() {
    let mut tree = AdaptiveRadixTree::new();
    tree.insert(b"key".to_vec(), 1).unwrap();

    assert_eq!(tree.remove(b"nonexistent").unwrap(), None);
    assert_eq!(tree.len(), 1);
}

#[test]
fn test_contains_key() {
    let mut tree = AdaptiveRadixTree::new();
    tree.insert(b"exists".to_vec(), 42).unwrap();

    assert!(tree.contains_key(b"exists"));
    assert!(!tree.contains_key(b"not_exists"));
}

#[test]
fn test_common_prefix() {
    let mut tree = AdaptiveRadixTree::new();

    tree.insert(b"test".to_vec(), 1).unwrap();
    tree.insert(b"testing".to_vec(), 2).unwrap();
    tree.insert(b"tester".to_vec(), 3).unwrap();
    tree.insert(b"team".to_vec(), 4).unwrap();

    assert_eq!(tree.len(), 4);
    assert_eq!(tree.get(b"test"), Some(1));
    assert_eq!(tree.get(b"testing"), Some(2));
    assert_eq!(tree.get(b"tester"), Some(3));
    assert_eq!(tree.get(b"team"), Some(4));
}

#[test]
fn test_node_growth() {
    let mut tree = AdaptiveRadixTree::new();

    // Insert enough keys to trigger node growth from Node4 -> Node16 -> Node48
    for i in 0..50 {
        let key = format!("key{:03}", i);
        tree.insert(key.as_bytes().to_vec(), i).unwrap();
    }

    assert_eq!(tree.len(), 50);

    // Verify all keys are retrievable
    for i in 0..50 {
        let key = format!("key{:03}", i);
        assert_eq!(tree.get(key.as_bytes()), Some(i));
    }
}

#[test]
fn test_node_shrink() {
    let mut tree = AdaptiveRadixTree::new();

    // Insert keys
    for i in 0..20 {
        let key = format!("key{:03}", i);
        tree.insert(key.as_bytes().to_vec(), i).unwrap();
    }

    // Remove keys to trigger shrinking
    for i in 0..15 {
        let key = format!("key{:03}", i);
        tree.remove(key.as_bytes()).unwrap();
    }

    assert_eq!(tree.len(), 5);

    // Verify remaining keys
    for i in 15..20 {
        let key = format!("key{:03}", i);
        assert_eq!(tree.get(key.as_bytes()), Some(i));
    }
}

#[test]
fn test_iterator() {
    let mut tree = AdaptiveRadixTree::new();

    tree.insert(b"a".to_vec(), 1).unwrap();
    tree.insert(b"b".to_vec(), 2).unwrap();
    tree.insert(b"c".to_vec(), 3).unwrap();

    let items: Vec<_> = tree.iter().collect();
    assert_eq!(items.len(), 3);

    // Check that all items are present (order may vary)
    let values: Vec<_> = items.iter().map(|(_, v)| *v).collect();
    assert!(values.contains(&1));
    assert!(values.contains(&2));
    assert!(values.contains(&3));
}

#[test]
fn test_keys_iterator() {
    let mut tree = AdaptiveRadixTree::new();

    tree.insert(b"apple".to_vec(), 1).unwrap();
    tree.insert(b"banana".to_vec(), 2).unwrap();

    let keys: Vec<_> = tree.keys().collect();
    assert_eq!(keys.len(), 2);
}

#[test]
fn test_values_iterator() {
    let mut tree = AdaptiveRadixTree::new();

    tree.insert(b"a".to_vec(), 10).unwrap();
    tree.insert(b"b".to_vec(), 20).unwrap();
    tree.insert(b"c".to_vec(), 30).unwrap();

    let values: Vec<_> = tree.values().collect();
    assert_eq!(values.len(), 3);

    let sum: i32 = values.iter().sum();
    assert_eq!(sum, 60);
}

#[test]
fn test_empty_key_error() {
    let mut tree = AdaptiveRadixTree::new();
    assert!(tree.insert(vec![], 42).is_err());
    assert!(tree.remove(&[]).is_err());
}

#[test]
fn test_binary_keys() {
    let mut tree = AdaptiveRadixTree::new();

    let key1 = vec![0x00, 0xFF, 0x80];
    let key2 = vec![0xFF, 0x00, 0x7F];

    tree.insert(key1.clone(), 1).unwrap();
    tree.insert(key2.clone(), 2).unwrap();

    assert_eq!(tree.get(&key1), Some(1));
    assert_eq!(tree.get(&key2), Some(2));
}

#[test]
fn test_large_dataset() {
    let mut tree = AdaptiveRadixTree::new();
    let count = 1000;

    // Insert
    for i in 0..count {
        let key = format!("key_{:06}", i);
        tree.insert(key.as_bytes().to_vec(), i).unwrap();
    }

    assert_eq!(tree.len(), count);

    // Verify
    for i in 0..count {
        let key = format!("key_{:06}", i);
        assert_eq!(tree.get(key.as_bytes()), Some(i));
    }

    // Remove half
    for i in 0..count / 2 {
        let key = format!("key_{:06}", i);
        assert_eq!(tree.remove(key.as_bytes()).unwrap(), Some(i));
    }

    assert_eq!(tree.len(), count / 2);
}

#[test]
fn test_path_compression() {
    let mut tree = AdaptiveRadixTree::new();

    // Insert keys that should trigger path compression
    tree.insert(b"a".to_vec(), 1).unwrap();
    tree.insert(b"ab".to_vec(), 2).unwrap();
    tree.insert(b"abc".to_vec(), 3).unwrap();
    tree.insert(b"abcd".to_vec(), 4).unwrap();

    assert_eq!(tree.len(), 4);
    assert_eq!(tree.get(b"a"), Some(1));
    assert_eq!(tree.get(b"ab"), Some(2));
    assert_eq!(tree.get(b"abc"), Some(3));
    assert_eq!(tree.get(b"abcd"), Some(4));

    // Remove to test decompression
    tree.remove(b"abcd").unwrap();
    assert_eq!(tree.get(b"abc"), Some(3));
}

#[test]
fn test_memory_usage() {
    let mut tree = AdaptiveRadixTree::new();

    let initial_memory = tree.memory_usage();

    for i in 0..100 {
        let key = format!("key{}", i);
        tree.insert(key.as_bytes().to_vec(), i).unwrap();
    }

    let final_memory = tree.memory_usage();
    assert!(final_memory > initial_memory);
}

#[test]
fn test_string_keys() {
    let mut tree = AdaptiveRadixTree::new();

    let keys = vec!["apple", "application", "apply", "banana", "band", "can"];

    for (i, key) in keys.iter().enumerate() {
        tree.insert(key.as_bytes().to_vec(), i).unwrap();
    }

    assert_eq!(tree.len(), keys.len());

    for (i, key) in keys.iter().enumerate() {
        assert_eq!(tree.get(key.as_bytes()), Some(i));
    }
}

#[test]
fn test_clone() {
    let mut tree1 = AdaptiveRadixTree::new();
    tree1.insert(b"key1".to_vec(), 1).unwrap();
    tree1.insert(b"key2".to_vec(), 2).unwrap();

    let tree2 = tree1.clone();

    assert_eq!(tree2.len(), 2);
    assert_eq!(tree2.get(b"key1"), Some(1));
    assert_eq!(tree2.get(b"key2"), Some(2));
}

#[test]
fn test_default() {
    let tree: AdaptiveRadixTree<i32> = AdaptiveRadixTree::default();
    assert!(tree.is_empty());
}

// Made with Bob
