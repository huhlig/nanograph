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
use crate::error::ConsensusError;
use nanograph_core::{
    object::{ClusterId, ClusterRecord, NodeId, ShardId, ShardRecord, ShardStatus, ShardType},
    types::Timestamp,
};
use nanograph_vfs::File;
use openraft::impls::leader_id_std::LeaderId;
use openraft::{OptionalSend, RaftTypeConfig};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt::Display;
use std::net::SocketAddr;

/// Request to append entries to a follower's log
pub type ConsensusAppendEntriesRequest = openraft::raft::AppendEntriesRequest<ConsensusTypeConfig>;
/// Response to an append entries request
pub type ConsensusAppendEntriesResponse =
    openraft::raft::AppendEntriesResponse<ConsensusTypeConfig>;
/// A single log entry in the consensus cluster
pub type ConsensusEntry = openraft::Entry<ConsensusTypeConfig>;
pub type ConsensusLeaderId = LeaderId<ConsensusTypeConfig>;
pub type ConsensusLogId = openraft::LogId<ConsensusTypeConfig>;
pub type ConsensusLogIdOf = openraft::type_config::alias::LogIdOf<ConsensusTypeConfig>;
/// Error occurring during an RPC call
pub type ConsensusRPCError = openraft::error::RPCError<ConsensusTypeConfig>;
pub type ConsensusStoredMembership = openraft::StoredMembership<ConsensusTypeConfig>;
/// A snapshot of the state machine
pub type ConsensusSnapshot = openraft::Snapshot<ConsensusTypeConfig>;
pub type ConsensusSnapshotData = std::io::Cursor<Vec<u8>>;
/// Response to a snapshot request
pub type ConsensusSnapshotResponse = openraft::raft::SnapshotResponse<ConsensusTypeConfig>;
/// Error occurring during snapshot streaming
pub type ConsensusStreamingError = openraft::error::StreamingError<ConsensusTypeConfig>;

pub type ConsensusVote = openraft::Vote<ConsensusTypeConfig>;
/// Request to vote for a candidate in an election
pub type ConsensusVoteRequest = openraft::raft::VoteRequest<ConsensusTypeConfig>;
/// Response to a vote request
pub type ConsensusVoteResponse = openraft::raft::VoteResponse<ConsensusTypeConfig>;
/// The vote type used in the consensus cluster
pub type ConsensusVoteOf = openraft::type_config::alias::VoteOf<ConsensusTypeConfig>;

/// Raft configuration for the Nanograph Consensus Cluster
#[derive(Debug, Clone, Copy, Default, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
pub struct ConsensusTypeConfig;

impl RaftTypeConfig for ConsensusTypeConfig {
    type D = Operation;
    type R = OperationResponse;
    type NodeId = NodeId;
    type Node = NodeInfo;
    type Term = u64;
    type LeaderId = LeaderId<Self>;
    type Vote = openraft::Vote<Self>;
    type Entry = openraft::entry::Entry<Self>;
    type SnapshotData = Box<dyn File>;
    type AsyncRuntime = openraft::TokioRuntime;
    type Responder<T: OptionalSend + 'static> = openraft::impls::OneshotResponder<Self, T>;
    type ErrorSource = ConsensusError;
}

/// Node information in the cluster
#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq)]
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

impl Default for NodeInfo {
    /// Create a default NodeInfo with placeholder values
    fn default() -> Self {
        Self {
            node: NodeId::new(0),
            raft_addr: "127.0.0.1:0".parse().unwrap(),
            api_addr: "127.0.0.1:0".parse().unwrap(),
            status: NodeStatus::default(),
            capacity: ResourceCapacity::default(),
            zone: None,
            rack: None,
        }
    }
}

impl Display for NodeInfo {
    /// Format NodeInfo for display
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Node(id={}, raft={}, api={})",
            self.node, self.raft_addr, self.api_addr
        )
    }
}

/// Node status in the cluster
#[derive(Copy, Clone, Debug, Eq, PartialEq, Serialize, Deserialize, Default)]
pub enum NodeStatus {
    /// Node is active and serving requests
    #[default]
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

impl PartialEq for ResourceCapacity {
    /// Compare two ResourceCapacity instances for equality
    fn eq(&self, other: &Self) -> bool {
        self.cpu_cores == other.cpu_cores
            && self.memory_bytes == other.memory_bytes
            && self.disk_bytes == other.disk_bytes
            && self.network_bandwidth == other.network_bandwidth
            && (self.weight - other.weight).abs() < f64::EPSILON
    }
}

impl Eq for ResourceCapacity {}

impl Default for ResourceCapacity {
    /// Create a default ResourceCapacity with reasonable baseline values
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
    /// Create a default Linearizable consistency level
    fn default() -> Self {
        ReadConsistency::Linearizable
    }
}

/// KV operation to be replicated via Raft
#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq)]
pub enum Operation {
    /// Put a key-value pair
    Put { key: Vec<u8>, value: Vec<u8> },

    /// Delete a key
    Delete { key: Vec<u8> },

    /// Batch of operations (atomic within shard)
    Batch { operations: Vec<Operation> },
}

impl Display for Operation {
    /// Format Operation for display
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Operation::Put { key, .. } => write!(f, "Put(key={:?})", key),
            Operation::Delete { key } => write!(f, "Delete(key={:?})", key),
            Operation::Batch { operations } => write!(f, "Batch(len={})", operations.len()),
        }
    }
}

/// Response from applying an operation
#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq)]
pub struct OperationResponse {
    /// Whether the operation succeeded
    pub success: bool,

    /// Optional return value (e.g., previous value for Put)
    pub value: Option<Vec<u8>>,

    /// Optional error message
    pub error: Option<String>,
}

impl Default for OperationResponse {
    /// Create a default successful OperationResponse
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
        shard_type: ShardType,
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
    /// Create a default ReplicationConfig with standard values for a 3-node cluster
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
    /// Create a default Random placement strategy
    fn default() -> Self {
        PlacementStrategy::Random
    }
}

/// Raft-specific cluster state that extends the base ClusterMetadata
#[derive(Clone, Debug)]
pub struct RaftClusterState {
    /// Base cluster metadata
    pub cluster: ClusterRecord,

    /// All nodes in the Raft cluster
    pub nodes: HashMap<NodeId, NodeInfo>,

    /// All shards with their metadata
    pub shards: HashMap<ShardId, ShardRecord>,

    /// Shard assignments (shard_id -> replica nodes)
    pub shard_assignments: HashMap<ShardId, Vec<NodeId>>,
}

impl RaftClusterState {
    /// Create new empty Raft cluster state
    pub fn new(cluster: ClusterRecord) -> Self {
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
    pub fn get_shard(&self, shard_id: ShardId) -> Option<&ShardRecord> {
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
    pub fn active_shards(&self) -> Vec<&ShardRecord> {
        self.shards
            .values()
            .filter(|s| s.status == ShardStatus::Active)
            .collect()
    }
}

impl Default for RaftClusterState {
    /// Create a default RaftClusterState with an empty cluster record
    fn default() -> Self {
        Self::new(ClusterRecord {
            cluster_id: ClusterId::new(0),
            name: String::new(),
            version: 0,
            created_at: Timestamp::now(),
            updated_at: Timestamp::now(),
            options: Default::default(),
            metadata: Default::default(),
        })
    }
}
