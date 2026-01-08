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

//! Test utilities for ART integration tests

use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};

/// Generate sequential keys for testing
pub fn generate_sequential_keys(count: usize) -> Vec<Vec<u8>> {
    (0..count)
        .map(|i| format!("key{:08}", i).into_bytes())
        .collect()
}

/// Generate random keys for testing
pub fn generate_random_keys(count: usize, seed: u64) -> Vec<Vec<u8>> {
    let mut rng = StdRng::seed_from_u64(seed);
    (0..count)
        .map(|_| {
            let len = rng.random_range(5..20);
            (0..len).map(|_| rng.random()).collect()
        })
        .collect()
}

/// Generate sequential key-value pairs
pub fn generate_sequential_kvs(count: usize) -> Vec<(Vec<u8>, Vec<u8>)> {
    (0..count)
        .map(|i| {
            let key = format!("key{:08}", i).into_bytes();
            let value = format!("value_{}", i).into_bytes();
            (key, value)
        })
        .collect()
}

/// Generate random key-value pairs
pub fn generate_random_kvs(count: usize, seed: u64) -> Vec<(Vec<u8>, Vec<u8>)> {
    let mut rng = StdRng::seed_from_u64(seed);
    (0..count)
        .map(|_| {
            let key_len = rng.random_range(5..20);
            let key: Vec<u8> = (0..key_len).map(|_| rng.random()).collect();

            let value_len = rng.random_range(10..100);
            let value: Vec<u8> = (0..value_len).map(|_| rng.random()).collect();

            (key, value)
        })
        .collect()
}

/// Generate keys with common prefixes
pub fn generate_prefix_keys(count: usize, prefix: &str) -> Vec<Vec<u8>> {
    (0..count)
        .map(|i| format!("{}{:06}", prefix, i).into_bytes())
        .collect()
}

/// Generate keys that will trigger node growth
pub fn generate_node_growth_keys() -> Vec<Vec<u8>> {
    let mut keys = Vec::new();

    // Generate keys that will cause Node4 -> Node16 transition
    for i in 0..5 {
        keys.push(format!("a{}", i).into_bytes());
    }

    // Generate keys that will cause Node16 -> Node48 transition
    for i in 0..20 {
        keys.push(format!("b{}", i).into_bytes());
    }

    // Generate keys that will cause Node48 -> Node256 transition
    for i in 0..60 {
        keys.push(format!("c{}", i).into_bytes());
    }

    keys
}

/// Generate keys with varying lengths
pub fn generate_variable_length_keys(count: usize) -> Vec<Vec<u8>> {
    (0..count)
        .map(|i| {
            // Create unique keys with varying lengths
            // Use a separator to ensure uniqueness, then add variable padding
            let padding_len = i % 20;
            let padding = "x".repeat(padding_len);
            format!("key_{:06}_{}", i, padding).into_bytes()
        })
        .collect()
}

/// Create a temporary directory for testing
pub fn create_temp_dir() -> tempfile::TempDir {
    tempfile::tempdir().expect("Failed to create temp directory")
}

/// Assert that two byte slices are equal
#[allow(dead_code)]
pub fn assert_bytes_eq(actual: &[u8], expected: &[u8]) {
    assert_eq!(
        actual, expected,
        "Bytes mismatch:\nActual:   {:?}\nExpected: {:?}",
        actual, expected
    );
}

/// Measure memory usage of a closure
#[allow(dead_code)]
pub fn measure_memory<F, R>(f: F) -> (R, usize)
where
    F: FnOnce() -> R,
{
    // This is a simplified version - in production you'd use more sophisticated tools
    let result = f();
    let memory = 0; // Placeholder - would need platform-specific code
    (result, memory)
}

// Made with Bob
