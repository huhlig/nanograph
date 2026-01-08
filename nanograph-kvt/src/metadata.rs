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

use crate::types::{ClusterId, NamespaceId, RegionId, ServerId, TableId};
use crate::{NodeId, ShardId, StorageEngineType};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Metadata for a cluster.
#[derive(Clone, Debug)]
pub struct ClusterMetadata {
    /// Unique identifier for the cluster
    pub id: ClusterId,
    /// Name of the cluster
    pub name: String,
    /// Metadata version (incremented on each change)
    pub version: u64,
    /// Timestamp when the cluster was created
    pub created_at: DateTime<Utc>,
    /// Timestamp when the schema was last modified
    pub last_modified: DateTime<Utc>,
}

/// Metadata for a region.
#[derive(Clone, Debug)]
pub struct RegionMetadata {
    /// Unique identifier for the region
    pub id: RegionId,
    /// Name of the region
    pub name: String,
    /// Identifier of the cluster this region belongs to
    pub cluster: ClusterId,
    /// Timestamp when the Region was created
    pub created_at: DateTime<Utc>,
    /// Timestamp when the schema was last modified
    pub last_modified: DateTime<Utc>,
}

/// Metadata for a server.
#[derive(Clone, Debug)]
pub struct ServerMetadata {
    /// Unique identifier for the server
    pub id: ServerId,
    /// Name of the server
    pub name: String,
    /// Identifier of the region this server belongs to
    pub region: RegionId,
    /// Identifier of the cluster this server belongs to
    pub cluster: ClusterId,
    /// Timestamp when the Server was created
    pub created_at: DateTime<Utc>,
    /// Timestamp when the schema was last modified
    pub last_modified: DateTime<Utc>,
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
    pub created_at: DateTime<Utc>,
    /// Timestamp when the schema was last modified
    pub last_modified: DateTime<Utc>,
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
    pub created_at: DateTime<Utc>,
    /// Type of storage engine used by the table
    pub engine_type: StorageEngineType,
    /// Timestamp when the table was last modified
    pub last_modified: DateTime<Utc>,
    /// Number of shards for distributed tables (1 for single-node)
    pub shard_count: u32,
    /// Replication factor for each shard (1 for single-node)
    pub replication_factor: usize,
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
    /// Timestamp when the shard was created
    pub created_at: DateTime<Utc>,
    /// Type of storage engine used by the shard
    pub engine_type: StorageEngineType,
    /// Timestamp when the shard was last modified
    pub last_modified: DateTime<Utc>,
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

/// Shard status
#[derive(Copy, Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
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
