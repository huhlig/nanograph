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

//! Core types for Raft-based distributed consensus
use nanograph_core::{
    object::{ClusterId, ClusterMetadata, NodeId, RegionId, ShardId, ShardMetadata, ShardStatus},
    types::Timestamp,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::net::SocketAddr;

/// Node information in the cluster
#[derive(Clone, Debug)]
pub struct NodeInfo {
    /// Unique node identifier
    pub node: NodeId,

    /// Network address for Raft communication
    pub raft_addr: SocketAddr,

    /// Network address for client API
    pub api_addr: SocketAddr,

    /// Current node status
    pub status: NodeStatus,

    /// Resource capacity for shard placement
    pub capacity: ResourceCapacity,

    /// Availability zone (for rack-aware placement)
    pub zone: Option<String>,

    /// Rack identifier (for rack-aware placement)
    pub rack: Option<String>,
}

/// Node status in the cluster
#[derive(Copy, Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum NodeStatus {
    /// Node is active and serving requests
    Active,

    /// Node is draining (preparing for removal)
    Draining,

    /// Node is temporarily inactive
    Inactive,

    /// Node has failed
    Failed,
}

/// Resource capacity for shard placement decisions
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ResourceCapacity {
    /// CPU cores available
    pub cpu_cores: u32,

    /// Memory in bytes
    pub memory_bytes: u64,

    /// Disk space in bytes
    pub disk_bytes: u64,

    /// Network bandwidth in bytes/sec
    pub network_bandwidth: u64,

    /// Weight for placement (0.0 to 1.0, higher = more capacity)
    pub weight: f64,
}

impl Default for ResourceCapacity {
    fn default() -> Self {
        Self {
            cpu_cores: 1,
            memory_bytes: 1024 * 1024 * 1024,     // 1GB
            disk_bytes: 10 * 1024 * 1024 * 1024,  // 10GB
            network_bandwidth: 100 * 1024 * 1024, // 100MB/s
            weight: 1.0,
        }
    }
}

/// Read consistency level
#[derive(Copy, Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum ReadConsistency {
    /// Linearizable read (strongest consistency, requires quorum)
    /// Uses ReadIndex to ensure we're reading committed data
    Linearizable,

    /// Leader lease-based read (fast, requires clock sync)
    /// Leader can serve reads without quorum if it has a valid lease
    Lease,

    /// Follower read (fastest, potentially stale)
    /// Read from any replica without consistency guarantees
    Follower,
}

impl Default for ReadConsistency {
    fn default() -> Self {
        ReadConsistency::Linearizable
    }
}

/// KV operation to be replicated via Raft
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum Operation {
    /// Put a key-value pair
    Put { key: Vec<u8>, value: Vec<u8> },

    /// Delete a key
    Delete { key: Vec<u8> },

    /// Batch of operations (atomic within shard)
    Batch { operations: Vec<Operation> },
}

/// Response from applying an operation
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct OperationResponse {
    /// Whether the operation succeeded
    pub success: bool,

    /// Optional return value (e.g., previous value for Put)
    pub value: Option<Vec<u8>>,

    /// Optional error message
    pub error: Option<String>,
}

impl Default for OperationResponse {
    fn default() -> Self {
        Self {
            success: true,
            value: None,
            error: None,
        }
    }
}

/// Cluster metadata change
#[derive(Clone, Debug)]
pub enum MetadataChange {
    /// Add a new node to the cluster
    AddNode { node: NodeInfo },

    /// Remove a node from the cluster
    RemoveNode { node_id: NodeId },

    /// Update node status
    UpdateNodeStatus { node_id: NodeId, status: NodeStatus },

    /// Update shard assignment
    UpdateShardAssignment {
        shard_id: ShardId,
        replicas: Vec<NodeId>,
    },

    /// Update shard leader
    UpdateShardLeader { shard_id: ShardId, leader: NodeId },

    /// Create a new shard
    CreateShard {
        shard_id: ShardId,
        range: (Vec<u8>, Vec<u8>),
        replicas: Vec<NodeId>,
    },

    /// Delete a shard
    DeleteShard { shard_id: ShardId },
}

/// Replication configuration
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ReplicationConfig {
    /// Number of replicas per shard (typically 3 or 5)
    pub replication_factor: usize,

    /// Minimum replicas that must acknowledge writes
    /// Typically (replication_factor / 2) + 1 for quorum
    pub min_sync_replicas: usize,

    /// Election timeout in milliseconds
    pub election_timeout_ms: u64,

    /// Heartbeat interval in milliseconds
    pub heartbeat_interval_ms: u64,

    /// Maximum entries per append request
    pub max_append_entries: usize,

    /// Snapshot threshold (entries before snapshot)
    pub snapshot_threshold: u64,
}

impl ReplicationConfig {
    /// Calculate quorum size
    pub fn quorum_size(&self) -> usize {
        self.replication_factor / 2 + 1
    }

    /// Calculate how many failures can be tolerated
    pub fn tolerable_failures(&self) -> usize {
        self.replication_factor - self.quorum_size()
    }
}

impl Default for ReplicationConfig {
    fn default() -> Self {
        Self {
            replication_factor: 3,
            min_sync_replicas: 2,
            election_timeout_ms: 1000,
            heartbeat_interval_ms: 100,
            max_append_entries: 100,
            snapshot_threshold: 10000,
        }
    }
}

/// Replica placement strategy
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum PlacementStrategy {
    /// Random placement across available nodes
    Random,

    /// Ensure replicas are in different racks
    RackAware,

    /// Ensure replicas are in different availability zones
    ZoneAware,

    /// Custom placement (for testing or special requirements)
    Custom,
}

impl Default for PlacementStrategy {
    fn default() -> Self {
        PlacementStrategy::Random
    }
}

/// Raft-specific cluster state that extends the base ClusterMetadata
#[derive(Clone, Debug)]
pub struct RaftClusterState {
    /// Base cluster metadata
    pub cluster: ClusterMetadata,

    /// All nodes in the Raft cluster
    pub nodes: HashMap<NodeId, NodeInfo>,

    /// All shards with their metadata
    pub shards: HashMap<ShardId, ShardMetadata>,

    /// Shard assignments (shard_id -> replica nodes)
    pub shard_assignments: HashMap<ShardId, Vec<NodeId>>,
}

impl RaftClusterState {
    /// Create new empty Raft cluster state
    pub fn new(cluster: ClusterMetadata) -> Self {
        Self {
            cluster,
            nodes: HashMap::new(),
            shards: HashMap::new(),
            shard_assignments: HashMap::new(),
        }
    }

    /// Get node by ID
    pub fn get_node(&self, node_id: NodeId) -> Option<&NodeInfo> {
        self.nodes.get(&node_id)
    }

    /// Get shard metadata by ID
    pub fn get_shard(&self, shard_id: ShardId) -> Option<&ShardMetadata> {
        self.shards.get(&shard_id)
    }

    /// Get replicas for a shard
    pub fn get_shard_replicas(&self, shard_id: ShardId) -> Option<&Vec<NodeId>> {
        self.shard_assignments.get(&shard_id)
    }

    /// Get all active nodes
    pub fn active_nodes(&self) -> Vec<&NodeInfo> {
        self.nodes
            .values()
            .filter(|n| n.status == NodeStatus::Active)
            .collect()
    }

    /// Get all active shards
    pub fn active_shards(&self) -> Vec<&ShardMetadata> {
        self.shards
            .values()
            .filter(|s| s.status == ShardStatus::Active)
            .collect()
    }
}

impl Default for RaftClusterState {
    fn default() -> Self {
        Self::new(ClusterMetadata {
            id: ClusterId::new(0),
            name: String::new(),
            version: 0,
            created_at: Timestamp::now(),
            last_modified: Timestamp::now(),
            options: Default::default(),
            metadata: Default::default(),
        })
    }
}
