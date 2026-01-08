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

//
// Hierarchical Identifiers for Distributed Architecture
//

/// Cluster identifier (global)
///
/// Represents the entire Nanograph deployment across all regions.
/// Uses a 32-bit identifier for compactness and to avoid overflow.
#[derive(Copy, Clone, Debug, Ord, PartialOrd, Eq, PartialEq, Hash)]
pub struct ClusterId(pub u32);

impl ClusterId {
    /// Create a new cluster identifier.
    pub fn new(id: u32) -> Self {
        Self(id)
    }

    /// Return the cluster identifier as a u32.
    pub fn as_u32(&self) -> u32 {
        self.0
    }
}

impl From<u32> for ClusterId {
    fn from(id: u32) -> Self {
        Self(id)
    }
}

impl std::fmt::Display for ClusterId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Cluster({})", self.0)
    }
}

/// Region identifier (geographic/data center)
///
/// Each region is a full replica of all data for data locality.
/// Examples: us-east-1, eu-west-1, ap-south-1
/// Uses a 32-bit identifier for compactness and to avoid overflow.
#[derive(Copy, Clone, Debug, Ord, PartialOrd, Eq, PartialEq, Hash)]
pub struct RegionId(pub u32);

impl RegionId {
    /// Create a new region identifier.
    pub fn new(id: u32) -> Self {
        Self(id)
    }

    /// Return the region identifier as a u32.
    pub fn as_u32(&self) -> u32 {
        self.0
    }
}

impl From<u32> for RegionId {
    fn from(id: u32) -> Self {
        Self(id)
    }
}

impl std::fmt::Display for RegionId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Region({})", self.0)
    }
}

/// Server identifier (Nanograph instance)
///
/// Represents a single Nanograph server process within a region.
#[derive(Copy, Clone, Debug, Ord, PartialOrd, Eq, PartialEq, Hash)]
pub struct ServerId(pub u64);

impl ServerId {
    /// Create a new server identifier.
    pub fn new(id: u64) -> Self {
        Self(id)
    }

    /// Return the server identifier as a u64.
    pub fn as_u64(&self) -> u64 {
        self.0
    }
}

impl From<u64> for ServerId {
    fn from(id: u64) -> Self {
        Self(id)
    }
}

impl std::fmt::Display for ServerId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Server({})", self.0)
    }
}

/// Raft Node ID consisting of cluster, region, and server identifiers.
#[derive(Copy, Clone, Debug, Default, Ord, PartialOrd, Eq, PartialEq, Hash)]
pub struct NodeId(u128);

impl NodeId {
    /// Create a new NodeId from a u128 identifier.
    pub fn new(id: u128) -> Self {
        Self(id)
    }
    /// Create a new NodeId from cluster, region, and server identifiers.
    pub fn from_parts(cluster: ClusterId, region: RegionId, server: ServerId) -> Self {
        Self(((cluster.0 as u128) << 64) | ((region.0 as u128) << 32) | server.0 as u128)
    }

    /// Return the node identifier as a u128.
    pub fn as_u128(&self) -> u128 {
        self.0
    }

    /// Extract the ServerId from the NodeId.
    pub fn server_id(&self) -> ServerId {
        ServerId((self.0 >> 64) as u64)
    }

    /// Extract the ClusterId from the NodeId.
    pub fn cluster_id(&self) -> ClusterId {
        ClusterId((self.0 >> 96) as u32)
    }

    /// Extract the RegionId from the NodeId.
    pub fn region_id(&self) -> RegionId {
        RegionId((self.0 >> 32) as u32)
    }
}

impl std::fmt::Display for NodeId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Node({:X})", self.0)
    }
}

/// Object Identifier used by all Database Objects
pub type ObjectId = u64;

/// Namespace identifier
///
/// Names are stored separately in metadata and mapped to IDs.
#[derive(Copy, Clone, Debug, Ord, PartialOrd, Eq, PartialEq, Hash)]
pub struct NamespaceId(pub ObjectId);

impl NamespaceId {
    /// Create a new namespace identifier.
    pub fn new(id: u64) -> Self {
        Self(id)
    }

    /// Return the namespace identifier as a u64.
    pub fn as_u64(&self) -> u64 {
        self.0
    }
}

impl From<u64> for NamespaceId {
    fn from(id: u64) -> Self {
        Self(id)
    }
}

impl std::fmt::Display for NamespaceId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Namespace({})", self.0)
    }
}

/// Table identifier
///
/// Uses u64 for globally unique identification within a schema.
/// Names are stored separately in metadata and mapped to IDs.
#[derive(Copy, Clone, Debug, Ord, PartialOrd, Eq, PartialEq, Hash)]
pub struct TableId(pub ObjectId);

impl TableId {
    /// Create a new table identifier.
    pub fn new(id: u64) -> Self {
        Self(id)
    }

    /// Return the table identifier as a u64.
    pub fn as_u64(&self) -> u64 {
        self.0
    }
}

impl From<u64> for TableId {
    fn from(id: u64) -> Self {
        Self(id)
    }
}

impl std::fmt::Display for TableId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Table({})", self.0)
    }
}

/// Shard Index, Unique within a table.
#[derive(Copy, Clone, Debug, Ord, PartialOrd, Eq, PartialEq, Hash, Default)]
pub struct ShardIndex(pub u32);

impl ShardIndex {
    /// Create a new shard index.
    pub fn new(id: u32) -> Self {
        Self(id)
    }

    /// Return the shard index as a u32.
    pub fn as_u32(&self) -> u32 {
        self.0
    }
}

impl From<u32> for ShardIndex {
    fn from(id: u32) -> Self {
        Self(id)
    }
}

impl std::fmt::Display for ShardIndex {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Shard({})", self.0)
    }
}

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
    /// Create a new shard identifier.
    pub fn new(id: u64) -> Self {
        Self(id)
    }

    /// Create a ShardId from TableId and ShardIndex.
    pub fn from_parts(table: TableId, index: ShardIndex) -> Self {
        Self((table.0 as u64) << 32 | index.0 as u64)
    }

    /// Extract the TableId from the ShardId.
    pub fn table(&self) -> TableId {
        TableId(self.0 >> 32)
    }

    /// Extract the ShardIndex from the ShardId.
    pub fn index(&self) -> ShardIndex {
        ShardIndex(self.0 as u32)
    }

    /// Get the underlying u64 value.
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
pub struct ShardStats {
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

impl ShardStats {
    /// Get Stat by name
    pub fn get(&self, key: &str) -> StatValue {
        match key {
            "key_count" => StatValue::None,
            "total_bytes" => StatValue::None,
            "data_bytes" => StatValue::None,
            "index_bytes" => StatValue::None,
            "last_modified" => StatValue::None,
            _ => self.engine_stats.get(key),
        }
    }
    pub fn keys(&self) -> impl Iterator<Item = String> {
        let mut names = vec![
            String::from("key_count"),
            String::from("total_bytes"),
            String::from("data_bytes"),
            String::from("index_bytes"),
            String::from("last_modified"),
        ];
        names.extend(self.engine_stats.keys());
        names.into_iter()
    }
    /// Get Iterator of Stats
    pub fn iter(&self) -> impl Iterator<Item = (String, StatValue)> {
        let mut stats = vec![
            (String::from("key_count"), StatValue::U64(self.key_count)),
            (
                String::from("total_bytes"),
                StatValue::U64(self.total_bytes),
            ),
            (String::from("data_bytes"), StatValue::U64(self.data_bytes)),
            (
                String::from("index_bytes"),
                StatValue::U64(self.index_bytes),
            ),
            (
                String::from("last_modified"),
                StatValue::Timestamp(self.last_modified.unwrap_or(Timestamp::epoch())),
            ),
        ];
        stats.extend(self.engine_stats.iter());
        stats.into_iter()
    }
}

/// Shard Engine statistics
#[derive(Clone, Debug, Default)]
pub struct EngineStats(HashMap<String, StatValue>);

impl EngineStats {
    /// Insert an engine-specific statistic.
    pub fn insert(&mut self, key: &str, value: StatValue) {
        self.0.insert(key.to_owned(), value);
    }
    /// Retrieve an engine-specific statistic.
    pub fn get(&self, key: &str) -> StatValue {
        self.0.get(key).cloned().unwrap_or_default()
    }
    /// Iterator of engine-specific statistic names
    pub fn keys(&self) -> impl Iterator<Item = String> {
        self.0.keys().cloned()
    }
    /// Iterator of engine-specific statistics
    pub fn iter(&self) -> impl Iterator<Item = (String, StatValue)> {
        self.0.iter().map(|(k, v)| (k.clone(), v.clone()))
    }
}

/// Generic statistic value for extensibility
#[derive(Clone, Debug, Default)]
pub enum StatValue {
    /// None value
    #[default]
    None,
    /// Unsigned 64-bit integer
    U64(u64),
    /// Signed 64-bit integer
    I64(i64),
    /// 64-bit floating point
    F64(f64),
    /// Boolean value
    Bool(bool),
    /// String value
    String(String),
    /// List of statistic values
    List(Vec<StatValue>),
    /// Map of statistic values
    Map(HashMap<String, StatValue>),
    /// Timestamp
    Timestamp(Timestamp),
}

impl StatValue {
    /// Create a StatValue from a u64.
    pub fn from_u64(value: u64) -> Self {
        Self::U64(value)
    }
    /// Create a StatValue from a usize.
    pub fn from_usize(value: usize) -> Self {
        Self::U64(value as u64)
    }
    /// Create a StatValue from an i64.
    pub fn from_i64(value: i64) -> Self {
        Self::I64(value)
    }
    /// Create a StatValue from an f64.
    pub fn from_f64(value: f64) -> Self {
        Self::F64(value)
    }
    /// Create a StatValue from a bool.
    pub fn from_bool(value: bool) -> Self {
        Self::Bool(value)
    }
    /// Create a StatValue from a string.
    pub fn from_string(value: impl Into<String>) -> Self {
        Self::String(value.into())
    }
    /// Create a StatValue from an iterator of StatValues.
    pub fn from_list(values: impl IntoIterator<Item = Self>) -> Self {
        Self::List(values.into_iter().collect())
    }
    /// Create a StatValue from an iterator of key-value pairs.
    pub fn from_map(values: impl IntoIterator<Item = (String, Self)>) -> Self {
        Self::Map(values.into_iter().collect())
    }
}

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
