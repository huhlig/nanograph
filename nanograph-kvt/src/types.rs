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

use nanograph_core::types::ShardIndex;
use std::collections::Bound;

/// Partitioning strategy for distributing keys across shards
#[derive(Debug, Clone)]
pub enum Partitioner {
    /// Hash-based partitioning using a hash function.
    Hash {
        /// Hash function to use.
        hash_fn: HashFunction,
    },
    /// Range-based partitioning with explicit key ranges.
    Range {
        /// Key ranges for each shard: (start_key, end_key).
        ranges: Vec<(Vec<u8>, Vec<u8>)>,
    },
    /// List-based partitioning with explicit key lists.
    List {
        /// Keys assigned to each shard.
        keys: Vec<Vec<Vec<u8>>>,
    },
    /// Time-based partitioning (for time-series data).
    Time {
        /// Time interval for each shard in seconds.
        interval_seconds: u64,
    },
}

impl Partitioner {
    /// Determine which shard index a key belongs to.
    pub fn get_shard_index(&self, key: &[u8], shard_count: u32) -> ShardIndex {
        match self {
            Partitioner::Hash { hash_fn } => {
                let hash = hash_fn.hash(key);
                ShardIndex((hash % shard_count as u64) as u32)
            }
            Partitioner::Range { ranges } => {
                // Find the range that contains this key
                for (idx, (start, end)) in ranges.iter().enumerate() {
                    if key >= start.as_slice() && key < end.as_slice() {
                        return ShardIndex(idx as u32);
                    }
                }
                // Default to shard 0 if not found
                ShardIndex(0)
            }
            Partitioner::List { keys } => {
                // Find which list contains this key
                for (idx, key_list) in keys.iter().enumerate() {
                    if key_list.iter().any(|k| k.as_slice() == key) {
                        return ShardIndex(idx as u32);
                    }
                }
                // Default to shard 0 if not found
                ShardIndex(0)
            }
            Partitioner::Time { interval_seconds } => {
                // Assume key contains timestamp (first 8 bytes as i64)
                if key.len() >= 8 {
                    let timestamp = i64::from_be_bytes([
                        key[0], key[1], key[2], key[3], key[4], key[5], key[6], key[7],
                    ]);
                    let shard_idx = (timestamp / *interval_seconds as i64) % shard_count as i64;
                    ShardIndex(shard_idx.abs() as u32)
                } else {
                    ShardIndex(0)
                }
            }
        }
    }
}

impl Default for Partitioner {
    fn default() -> Self {
        Partitioner::Hash {
            hash_fn: HashFunction::Murmur3,
        }
    }
}

/// Hash function for partitioning
#[derive(Debug, Clone, Copy)]
pub enum HashFunction {
    /// Murmur3 hash (fast, good distribution).
    Murmur3,
    /// XXHash (very fast).
    XXHash,
    /// CityHash (good for short keys).
    CityHash,
}

impl HashFunction {
    /// Hash a key to a u64 value.
    pub fn hash(&self, key: &[u8]) -> u64 {
        match self {
            HashFunction::Murmur3 => {
                // Simple FNV-1a hash as placeholder (replace with murmur3 crate)
                let mut hash = 0xcbf29ce484222325u64;
                for &byte in key {
                    hash ^= byte as u64;
                    hash = hash.wrapping_mul(0x100000001b3);
                }
                hash
            }
            HashFunction::XXHash => {
                // Simple FNV-1a hash as placeholder (replace with xxhash crate)
                let mut hash = 0xcbf29ce484222325u64;
                for &byte in key {
                    hash ^= byte as u64;
                    hash = hash.wrapping_mul(0x100000001b3);
                }
                hash
            }
            HashFunction::CityHash => {
                // Simple FNV-1a hash as placeholder (replace with cityhash crate)
                let mut hash = 0xcbf29ce484222325u64;
                for &byte in key {
                    hash ^= byte as u64;
                    hash = hash.wrapping_mul(0x100000001b3);
                }
                hash
            }
        }
    }
}

/// Range Scan Bounds
///
/// Defines the bounds for a range scan operation.
#[derive(Debug, Clone)]
pub struct KeyRange {
    /// Start bound (inclusive or exclusive)
    pub start: Bound<Vec<u8>>,
    /// End bound (inclusive or exclusive)
    pub end: Bound<Vec<u8>>,
    /// Maximum number of results to return
    pub limit: Option<usize>,
    /// Scan in reverse order
    pub reverse: bool,
}

impl KeyRange {
    /// Create a range that scans all keys.
    pub fn all() -> Self {
        Self {
            start: Bound::Unbounded,
            end: Bound::Unbounded,
            limit: None,
            reverse: false,
        }
    }

    /// Create a range with specified bounds.
    pub fn new(start: Bound<Vec<u8>>, end: Bound<Vec<u8>>) -> Self {
        Self {
            start,
            end,
            limit: None,
            reverse: false,
        }
    }

    /// Create a range from start (inclusive) to end (exclusive).
    pub fn from_to(start: Vec<u8>, end: Vec<u8>) -> Self {
        Self {
            start: Bound::Included(start),
            end: Bound::Excluded(end),
            limit: None,
            reverse: false,
        }
    }

    /// Create a range with a prefix.
    pub fn prefix(prefix: Vec<u8>) -> Self {
        let mut end = prefix.clone();
        if let Some(last) = end.last_mut() {
            if *last < 255 {
                *last += 1;
            } else {
                end.push(0);
            }
        }
        Self {
            start: Bound::Included(prefix),
            end: Bound::Excluded(end),
            limit: None,
            reverse: false,
        }
    }

    /// Set a limit on the maximum number of results.
    pub fn with_limit(mut self, limit: usize) -> Self {
        self.limit = Some(limit);
        self
    }

    /// Set the scan to be in reverse order.
    pub fn reversed(mut self) -> Self {
        self.reverse = true;
        self
    }
}

impl Default for KeyRange {
    fn default() -> Self {
        Self::all()
    }
}
