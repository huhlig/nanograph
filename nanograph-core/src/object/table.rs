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

use crate::object::{NodeId, ObjectId};
use crate::types::{PropertyUpdate, Timestamp};
use serde::{Deserialize, Serialize};
use std::collections::{Bound, HashMap};

/// Table identifier
///
/// Uses u64 for globally unique identification within a schema.
/// Names are stored separately in metadata and mapped to IDs.
#[derive(
    Copy, Clone, Debug, Default, Ord, PartialOrd, Eq, PartialEq, Hash, Serialize, Deserialize,
)]
pub struct TableId(pub ObjectId);

impl TableId {
    /// Create a new table identifier.
    pub fn new(id: u32) -> Self {
        Self(id)
    }

    /// Return the table identifier as a u64.
    pub fn as_u64(&self) -> u32 {
        self.0
    }
}

impl From<u32> for TableId {
    fn from(id: u32) -> Self {
        Self(id)
    }
}

impl std::fmt::Display for TableId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Table({})", self.0)
    }
}

/// Shard Index, Unique within a table.
#[derive(
    Copy, Clone, Debug, Default, Ord, PartialOrd, Eq, PartialEq, Hash, Serialize, Deserialize,
)]
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
#[derive(
    Copy, Clone, Debug, Default, Ord, PartialOrd, Eq, PartialEq, Hash, Serialize, Deserialize,
)]
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
        TableId((self.0 >> 32) as u32)
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

/// Configuration for table creation
#[derive(Debug, Clone)]
pub struct TableCreate {
    /// Name of the Table
    pub name: String,
    /// Path of the Table within the namespace hierarchy
    pub path: String,
    /// Engine Type
    pub engine_type: StorageEngineType,
    /// Sharding configuration
    pub sharding_config: TableSharding,
    /// Additional engine-specific options
    pub options: HashMap<String, String>,
    /// Table Metadata (Informative)
    pub metadata: HashMap<String, String>,
}

impl TableCreate {
    /// Create a new Table creation configuration.
    ///
    /// # Arguments
    ///
    /// * `name`: The name of the new Table.
    /// * `engine_type`: The storage engine type to use for the Table.
    pub fn new(
        name: impl Into<String>,
        path: impl Into<String>,
        engine_type: StorageEngineType,
    ) -> Self {
        Self {
            name: name.into(),
            path: path.into(),
            engine_type,
            sharding_config: TableSharding::Single,
            options: HashMap::new(),
            metadata: HashMap::new(),
        }
    }

    /// Add or update a configuration option for the Table.
    ///
    /// # Arguments
    ///
    /// * `key`: The key of the option to set.
    /// * `value`: The value to assign to the option.
    pub fn with_option(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.options.insert(key.into(), value.into());
        self
    }

    /// Set the sharding configuration for the Table.
    ///
    /// # Arguments
    ///
    /// * `shard_count`: The number of shards to create.
    /// * `partitioner`: The partitioning strategy to use.
    /// * `replication_factor`: The number of replicas for each shard.
    pub fn with_sharding(
        mut self,
        shard_count: u32,
        partitioner: Partitioner,
        replication_factor: usize,
    ) -> Self {
        self.sharding_config = TableSharding::Multiple {
            shard_count,
            partitioner,
            replication_factor,
        };
        self
    }
}

/// Configuration for table update
#[derive(Clone, Debug, Default)]
pub struct TableUpdate {
    /// New name for the Table
    pub name: Option<String>,
    /// New engine type for the Table
    pub engine_type: Option<StorageEngineType>,
    /// New sharding configuration for the Table
    pub sharding_config: Option<TableSharding>,
    /// Table configuration options to update
    pub options: Vec<PropertyUpdate>,
    /// Table metadata to update
    pub metadata: Vec<PropertyUpdate>,
}

impl TableUpdate {
    /// Set a new name for the Table.
    ///
    /// # Arguments
    ///
    /// * `name`: The new name to set.
    pub fn set_name(&mut self, name: impl Into<String>) -> &mut Self {
        self.name = Some(name.into());
        self
    }
    /// Set a new engine type for the Table.
    ///
    /// # Arguments
    ///
    /// * `engine_type`: The new engine type to set.
    pub fn set_engine_type(&mut self, engine_type: StorageEngineType) -> &mut Self {
        self.engine_type = Some(engine_type);
        self
    }
    /// Set a new sharding configuration for the Table.
    ///
    /// # Arguments
    ///
    /// * `sharding_config`: The new sharding configuration to set.
    pub fn set_sharding_config(&mut self, sharding_config: TableSharding) -> &mut Self {
        self.sharding_config = Some(sharding_config);
        self
    }
    /// Add or update a configuration option for the Table.
    ///
    /// # Arguments
    ///
    /// * `key`: The key of the option to set.
    /// * `value`: The value to assign to the option.
    pub fn set_option(&mut self, key: impl Into<String>, value: impl Into<String>) -> &mut Self {
        self.options
            .push(PropertyUpdate::Set(key.into(), value.into()));
        self
    }
    /// Clear a configuration option from the Table.
    ///
    /// # Arguments
    ///
    /// * `key`: The key of the option to clear.
    pub fn clear_option(&mut self, key: impl Into<String>) -> &mut Self {
        self.options.push(PropertyUpdate::Clear(key.into()));
        self
    }
}

/// Metadata for a table.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TableMetadata {
    /// Unique identifier for the table
    pub id: TableId,
    /// Name of the table
    pub name: String,
    /// Path of the table within the namespace hierarchy
    pub path: String,
    /// Version of the Table Record
    pub version: u64,
    /// Timestamp when the table was created
    pub created_at: Timestamp,
    /// Type of storage engine used by the table
    pub engine_type: StorageEngineType,
    /// Timestamp when the table was last modified
    pub last_modified: Timestamp,
    /// Distributed table Config (Single or Multiple)
    pub sharding: TableSharding,
    /// Additional engine-specific options
    pub options: HashMap<String, String>,
    /// Table Metadata (Informative)
    pub metadata: HashMap<String, String>,
}

/// Table Sharding Configuration
#[derive(Clone, Debug, Default, Ord, PartialOrd, Eq, PartialEq, Serialize, Deserialize)]
pub enum TableSharding {
    /// Single Shard
    #[default]
    Single,
    /// Multiple Shards with Partitioning and Replication
    Multiple {
        /// Number of Shards
        shard_count: u32,
        /// Key Partitioner
        partitioner: Partitioner,
        /// Number of replicas per shard
        replication_factor: usize,
    },
}

/// Configuration for shard creation
#[derive(Debug, Clone)]
pub struct ShardCreate {
    /// Table ID for which the shard is being created
    pub table: TableId,
    /// Shard Index for which the shard is being created
    pub index: ShardIndex,
    /// Storage engine type for the shard
    pub engine_type: StorageEngineType,
    /// Number of replicas per shard (default: 1 for single-node)
    pub replication_factor: usize,
}

impl ShardCreate {
    /// Create a new Shard creation configuration.
    ///
    /// # Arguments
    ///
    /// * `table`: The ID of the table the shard belongs to.
    /// * `index`: The index of the shard within the table.
    /// * `engine_type`: The storage engine type to use for the shard.
    pub fn new(table: TableId, index: ShardIndex, engine_type: StorageEngineType) -> Self {
        Self {
            table,
            index,
            engine_type,
            replication_factor: 1, // Default to no replication
        }
    }
    /// Set the replication factor for the shard.
    ///
    /// # Arguments
    ///
    /// * `replication_factor`: The number of replicas for the shard.
    pub fn with_replication(mut self, replication_factor: usize) -> Self {
        self.replication_factor = replication_factor;
        self
    }
}

/// Configuration for shard updating
#[derive(Debug, Clone)]
pub struct ShardUpdate {
    /// Number of replicas per shard
    pub replication_factor: usize,
}

impl ShardUpdate {
    /// Create a new Shard update configuration.
    ///
    /// # Arguments
    ///
    /// * `replication_factor`: The new replication factor for the shard.
    pub fn new(replication_factor: usize) -> Self {
        Self { replication_factor }
    }
}

/// Metadata for a shard.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ShardMetadata {
    /// Unique identifier for the shard (TableId + ShardIndex)
    pub id: ShardId,
    /// Name of the shard
    pub name: String,
    /// Version of the Shard Record
    pub version: u64,
    /// Type of storage engine used by the shard
    pub engine_type: StorageEngineType,
    /// Timestamp when the shard was created
    pub created_at: Timestamp,
    /// Timestamp when the shard was last modified
    pub last_modified: Timestamp,
    /// Key range covered by this shard
    pub range: (Vec<u8>, Vec<u8>),
    /// Current leader node (if known)
    pub leader: Option<NodeId>,
    /// All replica nodes for this shard
    pub replicas: Vec<NodeId>,
    /// Current shard status
    pub status: ShardStatus,
    /// Raft term (for debugging)
    pub term: u64,
    /// Approximate size in bytes
    pub size_bytes: u64,
}

/// Runtime state of a shard
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ShardState {
    /// Unique identifier for the shard
    pub id: ShardId,
    /// Storage engine type for the shard
    pub engine_type: StorageEngineType,
    /// Number of replicas for the shard
    pub replication_factor: usize,
}

/// Shard status
#[derive(
    Copy, Clone, Debug, Default, Ord, PartialOrd, Eq, PartialEq, Hash, Serialize, Deserialize,
)]
pub enum ShardStatus {
    /// Shard is active and serving requests
    #[default]
    Active,
    /// Shard is being rebalanced
    Rebalancing,
    /// Shard is being split into multiple shards
    Splitting,
    /// Shard is being merged with another shard
    Merging,
    /// Shard is offline (no quorum)
    Offline,
}

/// Storage engine type identifier
///
/// This is a string-based type to allow for pluggable storage engines.
/// Third-party engines can register with custom type names without
/// modifying this crate.
#[derive(Clone, Debug, Default, Ord, PartialOrd, Eq, PartialEq, Hash, Serialize, Deserialize)]
pub struct StorageEngineType(String);

impl StorageEngineType {
    /// Create a new storage engine type
    pub fn new(name: impl Into<String>) -> Self {
        Self(name.into())
    }

    /// Get the engine type name
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<&str> for StorageEngineType {
    fn from(s: &str) -> Self {
        Self(s.to_string())
    }
}

impl From<String> for StorageEngineType {
    fn from(s: String) -> Self {
        Self(s)
    }
}

impl std::fmt::Display for StorageEngineType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Partitioning strategy for distributing keys across shards
#[derive(Clone, Debug, Ord, PartialOrd, Eq, PartialEq, Serialize, Deserialize)]
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
#[derive(Debug, Clone, Copy, Ord, PartialOrd, Eq, PartialEq, Hash, Serialize, Deserialize)]
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
