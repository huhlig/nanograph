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

use crate::types::Partitioner;
use nanograph_core::types::{NamespaceId, NodeId, ShardId, ShardIndex, TableId, Timestamp};
use std::collections::HashMap;

/// Configuration for Namespace creation
pub struct NamespaceConfig {
    /// Name of the Namespace
    pub name: String,
}

impl NamespaceConfig {
    pub fn new(name: impl Into<String>) -> Self {
        Self { name: name.into() }
    }
}

/// Metadata for a schema.
#[derive(Clone, Debug)]
pub struct NamespaceMetadata {
    /// Unique identifier for the Namespace
    pub id: NamespaceId,
    /// Name of the Namespace
    pub name: String,
    /// Path of the schema within the namespace hierarchy
    pub path: String,
    /// Timestamp when the schema was created
    pub created_at: Timestamp,
    /// Timestamp when the schema was last modified
    pub last_modified: Timestamp,
}

/// Configuration for table creation
#[derive(Debug, Clone)]
pub struct TableConfig {
    /// Name of the Table
    pub name: String,
    /// Engine Type
    pub engine_type: StorageEngineType,
    /// Sharding configuration
    pub sharding_config: TableSharding,
    /// Additional engine-specific options
    pub options: HashMap<String, String>,
}

impl TableConfig {
    pub fn new(name: impl Into<String>, engine_type: StorageEngineType) -> Self {
        Self {
            name: name.into(),
            engine_type,
            sharding_config: TableSharding::Single,
            options: HashMap::new(),
        }
    }

    pub fn with_option(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.options.insert(key.into(), value.into());
        self
    }

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

/// Metadata for a table.
#[derive(Debug, Clone)]
pub struct TableMetadata {
    /// Unique identifier for the table
    pub id: TableId,
    /// Name of the table
    pub name: String,
    /// Path of the table within the namespace hierarchy
    pub path: String,
    /// Timestamp when the table was created
    pub created_at: Timestamp,
    /// Type of storage engine used by the table
    pub engine_type: StorageEngineType,
    /// Timestamp when the table was last modified
    pub last_modified: Timestamp,
    /// Distributed table Config (Single or Multiple)
    pub sharding: TableSharding,
}

/// Table Sharding Configuration
#[derive(Debug, Clone)]
pub enum TableSharding {
    /// Single Shard
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
pub struct ShardConfig {
    /// Table ID for which the shard is being created
    pub table: TableId,
    /// Shard Index for which the shard is being created
    pub index: ShardIndex,
    /// Storage engine type for the shard
    pub engine_type: StorageEngineType,
    /// Number of replicas per shard (default: 1 for single-node)
    pub replication_factor: usize,
}

impl ShardConfig {
    pub fn new(table: TableId, index: ShardIndex, engine_type: StorageEngineType) -> Self {
        Self {
            table,
            index,
            engine_type,
            replication_factor: 1, // Default to no replication
        }
    }
    pub fn with_replication(mut self, replication_factor: usize) -> Self {
        self.replication_factor = replication_factor;
        self
    }
}

/// Metadata for a shard.
#[derive(Clone, Debug)]
pub struct ShardMetadata {
    /// Unique identifier for the shard
    pub id: ShardId,
    /// Name of the shard
    pub name: String,
    /// Identifier of the table this shard belongs to
    pub table: TableId,
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

#[derive(Clone, Debug)]
pub struct ShardState {
    pub id: ShardId,
    pub engine_type: StorageEngineType,
    pub replication_factor: usize,
}

/// Shard status
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum ShardStatus {
    /// Shard is active and serving requests
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
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
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
