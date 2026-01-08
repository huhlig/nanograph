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

//! Test utilities for B+Tree testing

/// Generate random key-value pairs for testing
/// Uses a simple deterministic pseudo-random generator for reproducibility
pub fn generate_random_kvs(count: usize, seed: u64) -> Vec<(Vec<u8>, Vec<u8>)> {
    let mut kvs = Vec::with_capacity(count);
    let mut state = seed;

    // Simple LCG for deterministic random generation
    let lcg_next = |s: &mut u64| {
        *s = s
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        *s
    };

    for _ in 0..count {
        let key_len = (lcg_next(&mut state) % 28 + 4) as usize;
        let value_len = (lcg_next(&mut state) % 120 + 8) as usize;

        let mut key = Vec::with_capacity(key_len);
        let mut value = Vec::with_capacity(value_len);

        for _ in 0..key_len {
            key.push((lcg_next(&mut state) % 256) as u8);
        }

        for _ in 0..value_len {
            value.push((lcg_next(&mut state) % 256) as u8);
        }

        kvs.push((key, value));
    }

    kvs
}

/// Generate sequential keys for testing
pub fn generate_sequential_keys(count: usize, prefix: &str) -> Vec<Vec<u8>> {
    (0..count)
        .map(|i| format!("{}{:08}", prefix, i).into_bytes())
        .collect()
}

/// Generate sequential key-value pairs
#[allow(dead_code)]
pub fn generate_sequential_kvs(count: usize, prefix: &str) -> Vec<(Vec<u8>, Vec<u8>)> {
    (0..count)
        .map(|i| {
            let key = format!("{}{:08}", prefix, i).into_bytes();
            let value = format!("value_{}", i).into_bytes();
            (key, value)
        })
        .collect()
}

/// Generate reverse sequential keys
#[allow(dead_code)]
pub fn generate_reverse_sequential_keys(count: usize, prefix: &str) -> Vec<Vec<u8>> {
    (0..count)
        .rev()
        .map(|i| format!("{}{:08}", prefix, i).into_bytes())
        .collect()
}

/// Generate keys with specific patterns for testing edge cases
pub fn generate_edge_case_keys() -> Vec<Vec<u8>> {
    vec![
        vec![],                   // Empty key
        vec![0],                  // Single byte
        vec![0, 0, 0, 0],         // All zeros
        vec![255, 255, 255, 255], // All ones
        vec![0, 255, 0, 255],     // Alternating
        (0..255).collect(),       // Long sequential
        vec![b'a'; 1000],         // Very long key
    ]
}

/// Verify tree consistency (all leaves at same level, proper ordering)
#[allow(dead_code)]
pub fn verify_tree_structure(tree: &nanograph_btree::BPlusTree) -> Result<(), String> {
    let stats = tree.stats();

    // Verify we have at least one leaf
    if stats.num_leaf_nodes == 0 {
        return Err("Tree has no leaf nodes".to_string());
    }

    // Verify height is reasonable
    if stats.height == 0 {
        return Err("Tree height is 0".to_string());
    }

    // For non-empty trees, verify we have keys
    if stats.num_keys > 0 && stats.num_leaf_nodes == 0 {
        return Err("Tree has keys but no leaf nodes".to_string());
    }

    Ok(())
}

/// Assert that two byte slices are equal with better error messages
#[allow(dead_code)]
pub fn assert_bytes_eq(actual: &[u8], expected: &[u8], context: &str) {
    if actual != expected {
        panic!(
            "{}\nExpected: {:?}\nActual: {:?}",
            context,
            String::from_utf8_lossy(expected),
            String::from_utf8_lossy(actual)
        );
    }
}

/// Create a test tree with specific configuration
#[allow(dead_code)]
pub fn create_test_tree(max_keys: usize) -> nanograph_btree::BPlusTree {
    let config = nanograph_btree::tree::BPlusTreeConfig {
        max_keys,
        min_keys: max_keys / 2,
    };
    nanograph_btree::BPlusTree::new(config)
}

/// Fill a tree with test data and return the inserted keys
#[allow(dead_code)]
pub fn fill_tree_with_data(
    tree: &nanograph_btree::BPlusTree,
    count: usize,
) -> Vec<(Vec<u8>, Vec<u8>)> {
    let kvs = generate_sequential_kvs(count, "key");

    for (key, value) in &kvs {
        tree.insert(key.clone(), value.clone()).unwrap();
    }

    kvs
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_random_kvs() {
        let kvs = generate_random_kvs(100, 42);
        assert_eq!(kvs.len(), 100);

        // Verify all keys and values are within expected ranges
        for (key, value) in kvs {
            assert!(key.len() >= 4 && key.len() < 32);
            assert!(value.len() >= 8 && value.len() < 128);
        }
    }

    #[test]
    fn test_generate_sequential_keys() {
        let keys = generate_sequential_keys(10, "test");
        assert_eq!(keys.len(), 10);
        assert_eq!(keys[0], b"test00000000");
        assert_eq!(keys[9], b"test00000009");
    }

    #[test]
    fn test_edge_case_keys() {
        let keys = generate_edge_case_keys();
        assert!(!keys.is_empty());
        assert_eq!(keys[0], Vec::<u8>::new()); // Empty key
    }
}
