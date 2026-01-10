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

//! # Nanograph Raft - Distributed Consensus Layer
//!
//! This crate provides Raft-based distributed consensus for Nanograph.
//! It enables multi-node deployment with strong consistency guarantees.
//!
//! ## Architecture
//!
//! - **Shard-per-Raft-group**: Each shard is an independent Raft group
//! - **Metadata Raft group**: Separate group for cluster metadata
//! - **Hash-based partitioning**: Keys are routed to shards via hashing
//! - **Configurable replication**: Typically 3 or 5 replicas per shard
//!
//! ## Key Components
//!
//! - [`ShardRaftGroup`]: Manages consensus for a single shard
//! - [`MetadataRaftGroup`]: Manages cluster metadata
//! - [`ConsensusRouter`]: Routes operations to the correct shard
//! - [`RaftStorageAdapter`]: Bridges Raft with KeyValueStore trait
//!
//! ## Examples
//!
//! ### Basic Router Setup
//!
//! ```rust,ignore
//! use nanograph_raft::{Router, ReplicationConfig};
//! use nanograph_kvt::ShardId;
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! // Create a router for distributed operations
//! let node_id = 1;
//! let config = ReplicationConfig::default();
//! let router = Router::new(node_id, config).await?;
//!
//! // Add shards with their storage backends
//! let shard_id = ShardId::new(0);
//! let peers = vec![2, 3]; // Other node IDs in the Raft group
//! router.add_shard(shard_id, storage, peers).await?;
//!
//! // Route operations to the correct shard
//! let key = b"my_key";
//! router.put(key, b"my_value").await?;
//! let value = router.get(key).await?;
//! # Ok(())
//! # }
//! ```
//!
//! ### Configuring Replication
//!
//! ```rust
//! use nanograph_raft::ReplicationConfig;
//!
//! let mut config = ReplicationConfig::default();
//! config.replication_factor = 3; // 3 replicas per shard
//! config.election_timeout_ms = 300;
//! config.heartbeat_interval_ms = 100;
//! config.max_append_entries = 100;
//!
//! println!("Replication factor: {}", config.replication_factor);
//! println!("Quorum size: {}", config.quorum_size());
//! ```
//!
//! ### Creating a Shard Raft Group
//!
//! ```rust,ignore
//! use nanograph_raft::{ShardRaftGroup, RaftStorageAdapter};
//! use nanograph_kvt::ShardId;
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! let node_id = 1;
//! let shard_id = ShardId::new(0);
//! let storage = RaftStorageAdapter::new(backend_store);
//! let peers = vec![2, 3]; // Other nodes in this Raft group
//!
//! let shard_group = ShardRaftGroup::new(
//!     node_id,
//!     shard_id,
//!     storage,
//!     peers,
//! ).await?;
//!
//! // Propose an operation
//! let operation = Operation::Put {
//!     key: b"key1".to_vec(),
//!     value: b"value1".to_vec(),
//! };
//! let response = shard_group.propose(operation).await?;
//! # Ok(())
//! # }
//! ```
//!
//! ### Managing Cluster Metadata
//!
//! ```rust,ignore
//! use nanograph_raft::{MetadataRaftGroup, MetadataChange};
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! let node_id = 1;
//! let peers = vec![2, 3];
//! let metadata_group = MetadataRaftGroup::new(node_id, peers).await?;
//!
//! // Add a new node to the cluster
//! let change = MetadataChange::AddNode {
//!     node_id: 4,
//!     address: "192.168.1.4:8080".to_string(),
//! };
//! metadata_group.propose_change(change).await?;
//!
//! // Query cluster state
//! let cluster_state = metadata_group.get_cluster_state().await?;
//! println!("Active nodes: {}", cluster_state.active_nodes.len());
//! # Ok(())
//! # }
//! ```
//!
//! ### Read Consistency Levels
//!
//! ```rust
//! use nanograph_raft::ReadConsistency;
//!
//! // Linearizable read - strongest consistency, requires quorum
//! let linearizable = ReadConsistency::Linearizable;
//!
//! // Lease-based read - fast, requires clock sync
//! let lease = ReadConsistency::Lease;
//!
//! // Follower read - fastest, potentially stale
//! let follower = ReadConsistency::Follower;
//!
//! // Default is linearizable
//! assert_eq!(ReadConsistency::default(), ReadConsistency::Linearizable);
//! ```

mod config;
mod error;
mod metadata;
mod router;
mod shard_group;
mod storage;
mod types;

// Re-export public API
pub use config::{
    ClusterConfig, ClusterMetadata, RegionConfig, RegionMetadata, ServerConfig, ServerMetadata,
};
pub use error::{ConsensusError, ConsensusResult};
pub use metadata::MetadataRaftGroup;
pub use nanograph_core::types::{NodeId, RegionId, ServerId, ShardId};
pub use router::ConsensusRouter;
pub use shard_group::{RaftRole, ShardRaftGroup};
pub use storage::{LogEntry, RaftStorageAdapter, ShardSnapshot, SnapshotMeta};
pub use types::{
    MetadataChange, NodeInfo, NodeStatus, Operation, OperationResponse, PlacementStrategy,
    RaftClusterState, ReadConsistency, ReplicationConfig, ResourceCapacity,
};
