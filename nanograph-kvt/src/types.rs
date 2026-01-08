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

use crate::transaction::Timestamp;
use std::collections::{Bound, HashMap};

/// Shard identifier for distributed data partitioning
///
/// Each shard represents a partition of the key space and is replicated
/// across multiple nodes using Raft consensus. The shard_id is used to:
/// - Identify WAL segments
/// - Route keys to the correct storage engine
/// - Coordinate replication and failover
#[derive(Copy, Clone, Debug, Ord, PartialOrd, Eq, PartialEq, Hash, Default)]
pub struct ShardId(pub u64);

impl ShardId {
    /// Create a new shard identifier
    pub fn new(id: u64) -> Self {
        Self(id)
    }

    /// Get the underlying u64 value
    pub fn as_u64(&self) -> u64 {
        self.0
    }
}

impl From<u64> for ShardId {
    fn from(id: u64) -> Self {
        Self(id)
    }
}

impl std::fmt::Display for ShardId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Shard({})", self.0)
    }
}

/// Common table statistics shared across all storage engines
#[derive(Debug, Clone)]
pub struct TableStats {
    /// Approximate number of keys in the table
    pub key_count: u64,

    /// Total bytes used by the table (data + metadata + indexes)
    pub total_bytes: u64,

    /// Bytes used for actual key-value data
    pub data_bytes: u64,

    /// Bytes used for indexes and metadata
    pub index_bytes: u64,

    /// Last modification timestamp
    pub last_modified: Option<Timestamp>,

    /// Engine-specific statistics
    pub engine_stats: EngineStats,
}

/// Engine-specific statistics
///
/// Different storage engines expose different metrics relevant to their architecture.
#[derive(Debug, Clone)]
pub enum EngineStats {
    /// LSM-tree specific statistics
    Lsm(LsmStats),

    /// B+Tree specific statistics
    BTree(BTreeStats),

    /// ART (Adaptive Radix Tree) specific statistics
    Art(ArtStats),

    /// Generic/unknown engine
    Generic(HashMap<String, StatValue>),
}

/// LSM-tree specific statistics
#[derive(Debug, Clone)]
pub struct LsmStats {
    /// Number of levels in the LSM tree
    pub num_levels: usize,

    /// Number of SSTables per level
    pub sstables_per_level: Vec<usize>,

    /// Bytes per level
    pub bytes_per_level: Vec<u64>,

    /// Current memtable size
    pub memtable_bytes: u64,

    /// Number of pending compactions
    pub pending_compactions: usize,

    /// Total compactions performed
    pub total_compactions: u64,

    /// Write amplification factor
    pub write_amplification: f64,

    /// Read amplification factor (avg SSTables read per query)
    pub read_amplification: f64,

    /// Bloom filter false positive rate
    pub bloom_filter_false_positives: f64,
}

/// B+Tree specific statistics
#[derive(Debug, Clone)]
pub struct BTreeStats {
    /// Height of the B+Tree
    pub tree_height: usize,

    /// Total number of nodes
    pub total_nodes: u64,

    /// Number of leaf nodes
    pub leaf_nodes: u64,

    /// Number of internal nodes
    pub internal_nodes: u64,

    /// Average node utilization (0.0 to 1.0)
    pub avg_node_utilization: f64,

    /// Number of node splits since creation
    pub total_splits: u64,

    /// Number of node merges since creation
    pub total_merges: u64,

    /// Page size in bytes
    pub page_size: usize,
}

/// ART (Adaptive Radix Tree) specific statistics
#[derive(Debug, Clone)]
pub struct ArtStats {
    /// Maximum depth of the trie
    pub max_depth: usize,

    /// Average depth of keys
    pub avg_depth: f64,

    /// Number of Node4 (up to 4 children)
    pub node4_count: u64,

    /// Number of Node16 (up to 16 children)
    pub node16_count: u64,

    /// Number of Node48 (up to 48 children)
    pub node48_count: u64,

    /// Number of Node256 (up to 256 children)
    pub node256_count: u64,

    /// Total memory used (bytes)
    pub memory_bytes: u64,

    /// Number of path compressions
    pub path_compressions: u64,
}

/// Generic statistic value for extensibility
#[derive(Debug, Clone)]
pub enum StatValue {
    U64(u64),
    I64(i64),
    F64(f64),
    Bool(bool),
    String(String),
}

impl Default for EngineStats {
    fn default() -> Self {
        EngineStats::Generic(HashMap::new())
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
    /// Create a range that scans all keys
    pub fn all() -> Self {
        Self {
            start: Bound::Unbounded,
            end: Bound::Unbounded,
            limit: None,
            reverse: false,
        }
    }

    /// Create a range from start (inclusive) to end (exclusive)
    pub fn from_to(start: Vec<u8>, end: Vec<u8>) -> Self {
        Self {
            start: Bound::Included(start),
            end: Bound::Excluded(end),
            limit: None,
            reverse: false,
        }
    }

    /// Create a range with a prefix
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

    /// Set a limit on the number of results
    pub fn with_limit(mut self, limit: usize) -> Self {
        self.limit = Some(limit);
        self
    }

    /// Scan in reverse order
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
