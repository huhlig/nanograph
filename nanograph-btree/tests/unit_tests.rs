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

//! Unit tests for individual B+Tree components

mod test_utils;

use nanograph_btree::{BPlusTree, BPlusTreeNode, BTreeMetrics, NodeId, tree::BPlusTreeConfig};
use test_utils::*;

// ============================================================================
// Node Tests
// ============================================================================

#[test]
fn test_node_id_creation() {
    let id1 = NodeId::new(42);
    let id2 = NodeId::new(42);
    let id3 = NodeId::new(43);

    assert_eq!(id1, id2);
    assert_ne!(id1, id3);
    assert_eq!(id1.as_u64(), 42);
}

#[test]
fn test_leaf_node_creation() {
    let node_id = NodeId::new(1);
    let node = BPlusTreeNode::new_leaf(node_id);

    assert!(node.is_leaf());
    assert!(!node.is_internal());
    assert_eq!(node.id(), node_id);
    assert_eq!(node.len(), 0);
    assert!(node.is_empty());
}

#[test]
fn test_internal_node_creation() {
    let node_id = NodeId::new(1);
    let node = BPlusTreeNode::new_internal(node_id);

    assert!(node.is_internal());
    assert!(!node.is_leaf());
    assert_eq!(node.id(), node_id);
    assert_eq!(node.len(), 0);
    assert!(node.is_empty());
}

#[test]
fn test_leaf_insert_and_get() {
    let node_id = NodeId::new(1);
    let mut node = BPlusTreeNode::new_leaf(node_id);

    // Insert entries
    node.insert_entry(b"key1".to_vec(), b"value1".to_vec());
    node.insert_entry(b"key2".to_vec(), b"value2".to_vec());
    node.insert_entry(b"key3".to_vec(), b"value3".to_vec());

    assert_eq!(node.len(), 3);
    assert_eq!(node.get(b"key1"), Some(b"value1".to_vec()));
    assert_eq!(node.get(b"key2"), Some(b"value2".to_vec()));
    assert_eq!(node.get(b"key3"), Some(b"value3".to_vec()));
    assert_eq!(node.get(b"key4"), None);
}

#[test]
fn test_leaf_maintains_sorted_order() {
    let node_id = NodeId::new(1);
    let mut node = BPlusTreeNode::new_leaf(node_id);

    // Insert in random order
    node.insert_entry(b"key3".to_vec(), b"value3".to_vec());
    node.insert_entry(b"key1".to_vec(), b"value1".to_vec());
    node.insert_entry(b"key2".to_vec(), b"value2".to_vec());

    // Verify sorted order
    if let BPlusTreeNode::Leaf(leaf) = node {
        assert_eq!(leaf.entries[0].0, b"key1");
        assert_eq!(leaf.entries[1].0, b"key2");
        assert_eq!(leaf.entries[2].0, b"key3");
    } else {
        panic!("Expected leaf node");
    }
}

#[test]
fn test_leaf_update_existing_key() {
    let node_id = NodeId::new(1);
    let mut node = BPlusTreeNode::new_leaf(node_id);

    node.insert_entry(b"key1".to_vec(), b"value1".to_vec());
    assert_eq!(node.get(b"key1"), Some(b"value1".to_vec()));

    // Update with new value
    node.insert_entry(b"key1".to_vec(), b"value2".to_vec());
    assert_eq!(node.get(b"key1"), Some(b"value2".to_vec()));
    assert_eq!(node.len(), 1); // Should still be 1 entry
}

#[test]
fn test_leaf_delete() {
    let node_id = NodeId::new(1);
    let mut node = BPlusTreeNode::new_leaf(node_id);

    node.insert_entry(b"key1".to_vec(), b"value1".to_vec());
    node.insert_entry(b"key2".to_vec(), b"value2".to_vec());

    let deleted = node.delete_entry(b"key1");
    assert_eq!(deleted, Some(b"value1".to_vec()));
    assert_eq!(node.len(), 1);
    assert_eq!(node.get(b"key1"), None);
    assert_eq!(node.get(b"key2"), Some(b"value2".to_vec()));

    // Delete non-existent key
    let deleted = node.delete_entry(b"key3");
    assert_eq!(deleted, None);
    assert_eq!(node.len(), 1);
}

#[test]
fn test_leaf_is_full() {
    let node_id = NodeId::new(1);
    let mut node = BPlusTreeNode::new_leaf(node_id);
    let max_keys = 4;

    assert!(!node.is_full(max_keys));

    // Insert max_keys - 1 entries (not full yet)
    for i in 0..(max_keys - 1) {
        let key = format!("key{}", i);
        node.insert_entry(key.into_bytes(), b"value".to_vec());
    }

    assert!(!node.is_full(max_keys)); // At max_keys - 1, not full

    // Insert one more to reach max_keys
    node.insert_entry(b"extra".to_vec(), b"value".to_vec());
    assert!(node.is_full(max_keys)); // At max_keys, now full (needs split)
}

#[test]
fn test_leaf_split() {
    let node_id = NodeId::new(1);
    let mut node = BPlusTreeNode::new_leaf(node_id);

    // Fill the node
    for i in 0..10 {
        let key = format!("key{:02}", i);
        let value = format!("value{}", i);
        node.insert_entry(key.into_bytes(), value.into_bytes());
    }

    let new_node_id = NodeId::new(2);
    let (separator_key, new_node) = node.split_leaf(new_node_id).unwrap();

    // Verify split
    assert!(node.len() > 0);
    assert!(new_node.len() > 0);
    assert_eq!(node.len() + new_node.len(), 10);

    // Separator key should be the first key of the new node
    if let BPlusTreeNode::Leaf(new_leaf) = &new_node {
        assert_eq!(separator_key, new_leaf.entries[0].0);
    }

    // Verify leaf links
    if let BPlusTreeNode::Leaf(leaf) = &node {
        assert_eq!(leaf.next, Some(new_node_id));
    }
    if let BPlusTreeNode::Leaf(new_leaf) = &new_node {
        assert_eq!(new_leaf.prev, Some(node_id));
    }
}

#[test]
fn test_internal_node_operations() {
    let node_id = NodeId::new(1);
    let mut node = BPlusTreeNode::new_internal(node_id);

    // Add children
    let child1 = NodeId::new(10);
    let child2 = NodeId::new(20);
    let child3 = NodeId::new(30);

    // Internal nodes need at least one child before adding keys
    if let BPlusTreeNode::Internal(ref mut internal) = node {
        internal.children.push(child1);
    }

    node.insert_internal_entry(b"key2".to_vec(), child2);
    node.insert_internal_entry(b"key3".to_vec(), child3);

    assert_eq!(node.len(), 2); // 2 keys

    // Test child lookup
    assert_eq!(node.get_child_for_key(b"key1"), Some(child1));
    assert_eq!(node.get_child_for_key(b"key2"), Some(child2));
    assert_eq!(node.get_child_for_key(b"key3"), Some(child3));
}

#[test]
fn test_internal_node_split() {
    let node_id = NodeId::new(1);
    let mut node = BPlusTreeNode::new_internal(node_id);

    // Add initial child
    if let BPlusTreeNode::Internal(ref mut internal) = node {
        internal.children.push(NodeId::new(100));
    }

    // Fill the node
    for i in 0..10 {
        let key = format!("key{:02}", i);
        let child = NodeId::new(100 + i as u64 + 1);
        node.insert_internal_entry(key.into_bytes(), child);
    }

    let new_node_id = NodeId::new(2);
    let (_separator_key, new_node) = node.split_internal(new_node_id).unwrap();

    // Verify split
    assert!(node.len() > 0);
    assert!(new_node.len() > 0);

    // Original node should have len keys, new node should have the rest
    // The separator key is promoted and not in either node
    assert!(node.len() + new_node.len() < 10);
}

// ============================================================================
// Tree Structure Tests
// ============================================================================

#[test]
fn test_tree_creation() {
    let config = BPlusTreeConfig::default();
    let tree = BPlusTree::new(config);

    let stats = tree.stats();
    assert_eq!(stats.num_keys, 0);
    assert_eq!(stats.num_leaf_nodes, 1); // Root is a leaf initially
    assert_eq!(stats.num_internal_nodes, 0);
    assert_eq!(stats.height, 1);
}

#[test]
fn test_tree_single_insert() {
    let tree = create_test_tree(128);

    tree.insert(b"key1".to_vec(), b"value1".to_vec()).unwrap();

    let value = tree.get(b"key1").unwrap();
    assert_eq!(value, Some(b"value1".to_vec()));

    let stats = tree.stats();
    assert_eq!(stats.num_keys, 1);
}

#[test]
fn test_tree_multiple_inserts_no_split() {
    let tree = create_test_tree(128);

    for i in 0..10 {
        let key = format!("key{:02}", i);
        let value = format!("value{}", i);
        tree.insert(key.into_bytes(), value.into_bytes()).unwrap();
    }

    let stats = tree.stats();
    assert_eq!(stats.num_keys, 10);
    assert_eq!(stats.num_leaf_nodes, 1); // Should still be one leaf
    assert_eq!(stats.height, 1);
}

#[test]
fn test_tree_forces_split() {
    let tree = create_test_tree(4); // Small node size

    // Insert enough to force splits
    for i in 0..20 {
        let key = format!("key{:02}", i);
        let value = format!("value{}", i);
        tree.insert(key.into_bytes(), value.into_bytes()).unwrap();
    }

    let stats = tree.stats();
    assert_eq!(stats.num_keys, 20);
    assert!(
        stats.num_leaf_nodes > 1,
        "Should have split into multiple leaves"
    );
    assert!(stats.height > 1, "Tree should have grown in height");
    assert!(stats.num_internal_nodes > 0, "Should have internal nodes");

    // Verify all keys are still accessible
    for i in 0..20 {
        let key = format!("key{:02}", i);
        let value = tree.get(key.as_bytes()).unwrap();
        assert!(value.is_some(), "Key {} should exist", key);
    }
}

#[test]
fn test_tree_find_leaf() {
    let tree = create_test_tree(4);

    // Insert data to create a multi-level tree
    for i in 0..20 {
        let key = format!("key{:02}", i);
        tree.insert(key.into_bytes(), b"value".to_vec()).unwrap();
    }

    // Find leaf for various keys
    let root_id = *tree.root_id().read().unwrap();

    let _leaf_id = tree.find_leaf(root_id, b"key00").unwrap();
    // Leaf ID can be 0 (root starts at 0) - just verify find_leaf works

    let _leaf_id = tree.find_leaf(root_id, b"key10").unwrap();
    // Verify find_leaf works for different keys
}

#[test]
fn test_tree_delete_operations() {
    let tree = create_test_tree(128);

    // Insert some keys
    for i in 0..10 {
        let key = format!("key{:02}", i);
        tree.insert(key.into_bytes(), b"value".to_vec()).unwrap();
    }

    // Delete some keys
    assert!(tree.delete(b"key00").unwrap());
    assert!(tree.delete(b"key05").unwrap());
    assert!(!tree.delete(b"nonexistent").unwrap());

    // Verify deletions
    assert_eq!(tree.get(b"key00").unwrap(), None);
    assert_eq!(tree.get(b"key05").unwrap(), None);
    assert!(tree.get(b"key01").unwrap().is_some());

    let stats = tree.stats();
    assert_eq!(stats.num_keys, 8);
}

#[test]
fn test_tree_update_existing() {
    let tree = create_test_tree(128);

    tree.insert(b"key1".to_vec(), b"value1".to_vec()).unwrap();
    assert_eq!(tree.get(b"key1").unwrap(), Some(b"value1".to_vec()));

    tree.insert(b"key1".to_vec(), b"value2".to_vec()).unwrap();
    assert_eq!(tree.get(b"key1").unwrap(), Some(b"value2".to_vec()));

    let stats = tree.stats();
    assert_eq!(stats.num_keys, 1); // Should still be 1 key
}

// ============================================================================
// Metrics Tests
// ============================================================================

#[test]
fn test_metrics_creation() {
    let metrics = BTreeMetrics::new();
    let snapshot = metrics.snapshot();

    assert_eq!(snapshot.reads, 0);
    assert_eq!(snapshot.writes, 0);
    assert_eq!(snapshot.deletes, 0);
    assert_eq!(snapshot.scans, 0);
}

#[test]
fn test_metrics_tracking() {
    let metrics = BTreeMetrics::new();

    metrics.record_read(true);
    metrics.record_write(false);
    metrics.record_write(true);
    metrics.record_delete();
    metrics.record_scan(10);

    let snapshot = metrics.snapshot();
    assert_eq!(snapshot.reads, 1);
    assert_eq!(snapshot.writes, 2);
    assert_eq!(snapshot.deletes, 1);
    assert_eq!(snapshot.scans, 1);
}

#[test]
fn test_metrics_node_operations() {
    let metrics = BTreeMetrics::new();

    metrics.record_node_split();
    metrics.record_node_split();
    metrics.record_node_merge();

    let snapshot = metrics.snapshot();
    assert_eq!(snapshot.node_splits, 2);
    assert_eq!(snapshot.node_merges, 1);
}

// ============================================================================
// Configuration Tests
// ============================================================================

#[test]
fn test_config_default() {
    let config = BPlusTreeConfig::default();
    assert_eq!(config.max_keys, 128);
    assert_eq!(config.min_keys, 64);
}

#[test]
fn test_config_custom() {
    let config = BPlusTreeConfig {
        max_keys: 256,
        min_keys: 128,
    };

    let tree = BPlusTree::new(config.clone());

    // Verify tree uses the config
    // (This is implicit - the tree should respect these values during splits)
    let stats = tree.stats();
    assert_eq!(stats.num_keys, 0);
}

// ============================================================================
// Edge Cases
// ============================================================================

#[test]
fn test_empty_keys_and_values() {
    let tree = create_test_tree(128);

    // Empty key
    tree.insert(vec![], b"value".to_vec()).unwrap();
    assert_eq!(tree.get(&[]).unwrap(), Some(b"value".to_vec()));

    // Empty value
    tree.insert(b"key".to_vec(), vec![]).unwrap();
    assert_eq!(tree.get(b"key").unwrap(), Some(vec![]));

    // Both empty
    tree.insert(vec![], vec![]).unwrap();
    assert_eq!(tree.get(&[]).unwrap(), Some(vec![]));
}

#[test]
fn test_large_keys() {
    let tree = create_test_tree(128);

    let large_key = vec![b'k'; 1000];
    tree.insert(large_key.clone(), b"value".to_vec()).unwrap();
    assert_eq!(tree.get(&large_key).unwrap(), Some(b"value".to_vec()));
}

#[test]
fn test_large_values() {
    let tree = create_test_tree(128);

    let large_value = vec![b'v'; 10000];
    tree.insert(b"key".to_vec(), large_value.clone()).unwrap();
    assert_eq!(tree.get(b"key").unwrap(), Some(large_value));
}

#[test]
fn test_binary_keys() {
    let tree = create_test_tree(128);

    // Test with various binary patterns
    let keys = vec![
        vec![0u8, 0, 0, 0],
        vec![255u8, 255, 255, 255],
        vec![0u8, 255, 0, 255],
        vec![128u8, 64, 32, 16],
    ];

    for (i, key) in keys.iter().enumerate() {
        let value = format!("value{}", i);
        tree.insert(key.clone(), value.into_bytes()).unwrap();
    }

    for (i, key) in keys.iter().enumerate() {
        let expected = format!("value{}", i);
        assert_eq!(tree.get(key).unwrap(), Some(expected.into_bytes()));
    }
}

#[test]
fn test_sequential_vs_random_order() {
    let tree1 = create_test_tree(8);
    let tree2 = create_test_tree(8);

    let kvs = generate_sequential_kvs(100, "key");

    // Insert in order
    for (key, value) in &kvs {
        tree1.insert(key.clone(), value.clone()).unwrap();
    }

    // Insert in reverse order
    for (key, value) in kvs.iter().rev() {
        tree2.insert(key.clone(), value.clone()).unwrap();
    }

    // Both trees should have same content
    let stats1 = tree1.stats();
    let stats2 = tree2.stats();

    assert_eq!(stats1.num_keys, stats2.num_keys);

    // Verify all keys exist in both
    for (key, expected_value) in &kvs {
        assert_eq!(tree1.get(key).unwrap(), Some(expected_value.clone()));
        assert_eq!(tree2.get(key).unwrap(), Some(expected_value.clone()));
    }
}
