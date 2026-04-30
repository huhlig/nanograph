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
//! - [`TableShardRaftGroup`]: Manages consensus for a single table shard
//! - [`SystemShardRaftGroup`]: Manages cluster system metadata
//! - [`ContainerShardRaftGroup`]: Manages container metadata
//! - [`ConsensusManager`]: Routes operations to the correct shard and manages server lifecycle
//! - [`ConsensusLogStore`]: Raft log storage implementation
//! - [`ConsensusStateStore`]: Raft state storage implementation
//!
//! ## Examples
//!
//! ### Basic Manager Setup with Runtime Integration
//!
//! ```rust,ignore
//! use nanograph_raft::{ConsensusManager, ReplicationConfig, NodeInfo};
//! use nanograph_core::object::NodeId;
//! use std::sync::Arc;
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! // Create a consensus manager
//! let node_id = NodeId::new(1);
//! let config = ReplicationConfig::default();
//! let manager = Arc::new(ConsensusManager::new(node_id, config));
//!
//! // Start the gRPC server for Raft communication
//! let bind_addr = "127.0.0.1:50051".parse()?;
//! manager.clone().start_server(bind_addr).await?;
//!
//! // Add peer nodes
//! let peer_info = NodeInfo {
//!     node: NodeId::new(2),
//!     raft_addr: "127.0.0.1:50052".parse()?,
//!     api_addr: "127.0.0.1:8082".parse()?,
//!     status: Default::default(),
//!     capacity: Default::default(),
//!     availability_zone: None,
//! };
//! manager.add_peer(NodeId::new(2), peer_info).await;
//!
//! // Add shards with their storage backends
//! let shard_id = ShardId::new(0);
//! let peers = vec![NodeId::new(2), NodeId::new(3)];
//! manager.add_table_shard(shard_id, log_store, state_store, peers).await?;
//!
//! // Route operations to the correct shard
//! let key = b"my_key";
//! manager.put(key.to_vec(), b"my_value".to_vec()).await?;
//! let value = manager.get(key).await?;
//!
//! // Gracefully shutdown
//! manager.stop_server().await?;
//! # Ok(())
//! # }
//! ```
//!
//! ### Using with Tokio Runtime
//!
//! ```rust,ignore
//! use nanograph_raft::{ConsensusManager, ReplicationConfig};
//! use nanograph_core::object::NodeId;
//! use std::sync::Arc;
//!
//! fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     // Create a tokio runtime
//!     let runtime = tokio::runtime::Runtime::new()?;
//!
//!     runtime.block_on(async {
//!         let node_id = NodeId::new(1);
//!         let config = ReplicationConfig::default();
//!         let manager = Arc::new(ConsensusManager::new(node_id, config));
//!
//!         // Start server on the runtime
//!         let bind_addr = "127.0.0.1:50051".parse()?;
//!         manager.clone().start_server(bind_addr).await?;
//!
//!         // Server is now running and handling requests
//!         println!("Server running on {}", bind_addr);
//!
//!         // Do work...
//!
//!         // Shutdown when done
//!         manager.stop_server().await?;
//!         Ok::<_, Box<dyn std::error::Error>>(())
//!     })?;
//!
//!     Ok(())
//! }
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
mod group;
mod grpc;
mod manager;
mod network;
mod storage;
mod types;

// Re-export public API
pub use self::error::{ConsensusError, ConsensusResult};
pub use self::group::{ContainerShardRaftGroup, SystemShardRaftGroup, TableShardRaftGroup};
pub use self::manager::ConsensusManager;
pub use self::network::ConsensusNetworkFactory;
pub use self::storage::{ConsensusLogStore, ConsensusStateStore, SnapshotConfig, SnapshotManager};
pub use self::types::{
    ConsensusTypeConfig, MetadataChange, NodeInfo, NodeStatus, Operation, OperationResponse,
    PlacementStrategy, RaftClusterState, ReadConsistency, ReplicationConfig, ResourceCapacity,
};
pub use nanograph_core::object::{
    ClusterCreate, ClusterId, ClusterRecord, ClusterUpdate, ContainerId, NodeId, RegionCreate,
    RegionId, RegionRecord, RegionUpdate, ServerCreate, ServerId, ServerRecord, ServerUpdate,
    ShardId,
};
