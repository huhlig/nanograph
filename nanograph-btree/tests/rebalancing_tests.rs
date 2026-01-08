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

use nanograph_btree::{BPlusTree, BPlusTreeConfig};

#[test]
fn test_delete_without_underflow() {
    let config = BPlusTreeConfig {
        max_keys: 4,
        min_keys: 2,
    };
    let tree = BPlusTree::new(config);
    
    // Insert enough keys to avoid underflow
    for i in 0..10 {
        let key = format!("key{:02}", i).into_bytes();
        let value = format!("value{}", i).into_bytes();
        tree.insert(key, value).unwrap();
    }
    
    // Delete a key - should not trigger rebalancing
    let deleted = tree.delete(b"key05").unwrap();
    assert!(deleted);
    
    // Verify key is gone
    assert!(tree.get(b"key05").unwrap().is_none());
    
    // Other keys should still be present
    assert!(tree.get(b"key04").unwrap().is_some());
    assert!(tree.get(b"key06").unwrap().is_some());
}

#[test]
fn test_delete_nonexistent_key() {
    let config = BPlusTreeConfig {
        max_keys: 4,
        min_keys: 2,
    };
    let tree = BPlusTree::new(config);
    
    // Insert some keys
    for i in 0..5 {
        let key = format!("key{:02}", i).into_bytes();
        let value = format!("value{}", i).into_bytes();
        tree.insert(key, value).unwrap();
    }
    
    // Try to delete non-existent key
    let deleted = tree.delete(b"key99").unwrap();
    assert!(!deleted);
}

#[test]
fn test_delete_with_borrowing() {
    let config = BPlusTreeConfig {
        max_keys: 4,
        min_keys: 2,
    };
    let tree = BPlusTree::new(config);
    
    // Insert keys to create a tree with multiple nodes
    for i in 0..20 {
        let key = format!("key{:02}", i).into_bytes();
        let value = format!("value{}", i).into_bytes();
        tree.insert(key, value).unwrap();
    }
    
    // Delete keys to trigger borrowing from siblings
    for i in 0..5 {
        let key = format!("key{:02}", i).into_bytes();
        let deleted = tree.delete(&key).unwrap();
        assert!(deleted);
        
        // Verify deletion
        assert!(tree.get(&key).unwrap().is_none());
    }
    
    // Remaining keys should still be accessible
    for i in 5..20 {
        let key = format!("key{:02}", i).into_bytes();
        assert!(tree.get(&key).unwrap().is_some());
    }
}

#[test]
fn test_delete_with_merging() {
    let config = BPlusTreeConfig {
        max_keys: 4,
        min_keys: 2,
    };
    let tree = BPlusTree::new(config);
    
    // Insert keys
    for i in 0..15 {
        let key = format!("key{:02}", i).into_bytes();
        let value = format!("value{}", i).into_bytes();
        tree.insert(key, value).unwrap();
    }
    
    // Delete many keys to trigger node merging
    for i in 0..10 {
        let key = format!("key{:02}", i).into_bytes();
        tree.delete(&key).unwrap();
    }
    
    // Remaining keys should still be accessible
    for i in 10..15 {
        let key = format!("key{:02}", i).into_bytes();
        let value = tree.get(&key).unwrap();
        assert!(value.is_some());
        assert_eq!(value.unwrap(), format!("value{}", i).into_bytes());
    }
}

#[test]
fn test_alternating_insert_delete() {
    let config = BPlusTreeConfig {
        max_keys: 4,
        min_keys: 2,
    };
    let tree = BPlusTree::new(config);
    
    // Alternate between inserting and deleting
    for round in 0..5 {
        // Insert keys
        for i in 0..10 {
            let key = format!("key{:02}_{}", i, round).into_bytes();
            let value = format!("value{}_{}", i, round).into_bytes();
            tree.insert(key, value).unwrap();
        }
        
        // Delete half of them
        for i in 0..5 {
            let key = format!("key{:02}_{}", i, round).into_bytes();
            tree.delete(&key).unwrap();
        }
        
        // Verify remaining keys
        for i in 5..10 {
            let key = format!("key{:02}_{}", i, round).into_bytes();
            assert!(tree.get(&key).unwrap().is_some());
        }
    }
}

#[test]
fn test_delete_all_keys() {
    let config = BPlusTreeConfig {
        max_keys: 4,
        min_keys: 2,
    };
    let tree = BPlusTree::new(config);
    
    let num_keys = 20;
    
    // Insert keys
    for i in 0..num_keys {
        let key = format!("key{:02}", i).into_bytes();
        let value = format!("value{}", i).into_bytes();
        tree.insert(key, value).unwrap();
    }
    
    // Delete all keys
    for i in 0..num_keys {
        let key = format!("key{:02}", i).into_bytes();
        let deleted = tree.delete(&key).unwrap();
        assert!(deleted);
    }
    
    // Verify all keys are gone
    for i in 0..num_keys {
        let key = format!("key{:02}", i).into_bytes();
        assert!(tree.get(&key).unwrap().is_none());
    }
}

#[test]
fn test_delete_maintains_order() {
    let config = BPlusTreeConfig {
        max_keys: 4,
        min_keys: 2,
    };
    let tree = BPlusTree::new(config);
    
    // Insert keys
    for i in 0..30 {
        let key = format!("key{:03}", i).into_bytes();
        let value = format!("value{}", i).into_bytes();
        tree.insert(key, value).unwrap();
    }
    
    // Delete every other key
    for i in (0..30).step_by(2) {
        let key = format!("key{:03}", i).into_bytes();
        tree.delete(&key).unwrap();
    }
    
    // Verify remaining keys are still in order and accessible
    for i in (1..30).step_by(2) {
        let key = format!("key{:03}", i).into_bytes();
        let value = tree.get(&key).unwrap();
        assert!(value.is_some());
        assert_eq!(value.unwrap(), format!("value{}", i).into_bytes());
    }
}

#[test]
fn test_rebalancing_preserves_data() {
    let config = BPlusTreeConfig {
        max_keys: 4,
        min_keys: 2,
    };
    let tree = BPlusTree::new(config);
    
    // Insert a large number of keys
    let num_keys = 100;
    for i in 0..num_keys {
        let key = format!("key{:04}", i).into_bytes();
        let value = format!("value{}", i).into_bytes();
        tree.insert(key, value).unwrap();
    }
    
    // Delete a significant portion
    for i in (0..num_keys).step_by(3) {
        let key = format!("key{:04}", i).into_bytes();
        tree.delete(&key).unwrap();
    }
    
    // Verify all remaining keys are still correct
    for i in 0..num_keys {
        let key = format!("key{:04}", i).into_bytes();
        let value = tree.get(&key).unwrap();
        
        if i % 3 == 0 {
            // Should be deleted
            assert!(value.is_none());
        } else {
            // Should still exist with correct value
            assert!(value.is_some());
            assert_eq!(value.unwrap(), format!("value{}", i).into_bytes());
        }
    }
}

// Made with Bob
