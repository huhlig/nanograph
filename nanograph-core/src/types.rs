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

use chrono::{Date, DateTime, TimeZone, Utc};
use std::collections::{Bound, HashMap};
use std::time::UNIX_EPOCH;
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

/// Millisecond Timestamp for Multiversion Concurrency Control (MVCC)
#[derive(Copy, Clone, Debug, Ord, PartialOrd, Eq, PartialEq, Hash)]
pub struct Timestamp(DateTime<Utc>);

impl Timestamp {
    /// Current Timestamp in Milliseconds
    pub fn now() -> Timestamp {
        Timestamp(Utc::now())
    }
    /// Epoch Timestamp in Milliseconds
    pub fn epoch() -> Timestamp {
        Timestamp(DateTime::<Utc>::UNIX_EPOCH)
    }
    /// Create a Timestamp from milliseconds since the Unix epoch
    pub fn from_millis(millis: i64) -> Timestamp {
        Timestamp(
            DateTime::<Utc>::from_timestamp_millis(millis).unwrap_or(DateTime::<Utc>::UNIX_EPOCH),
        )
    }
    /// Convert timestamp into milliseconds since the Unix epoch
    pub fn as_millis(&self) -> i64 {
        self.0.timestamp_millis()
    }
}

impl std::fmt::Display for Timestamp {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Timestamp({}ms)", self.0)
    }
}
