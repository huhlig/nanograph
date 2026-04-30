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

//! gRPC client for Raft RPC communication

use crate::error::{ConsensusError, ConsensusResult};
use crate::grpc::pb::raft_service_client::RaftServiceClient;
use crate::grpc::pb::*;
use crate::types::*;
use nanograph_core::object::{NodeId, ShardId};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tonic::transport::{Channel, Endpoint};

/// gRPC client for making Raft RPC calls to peer nodes
///
/// Manages connections to peer nodes and provides methods for making
/// Raft RPC calls (vote, append_entries, install_snapshot). Connections
/// are lazily established and cached for reuse.
#[derive(Clone)]
pub struct RaftRpcClient {
    /// Map of node IDs to gRPC clients
    ///
    /// Connections are established lazily on first use and cached for
    /// subsequent requests to the same node.
    connections: Arc<RwLock<HashMap<NodeId, RaftServiceClient<Channel>>>>,

    /// Map of node IDs to their addresses
    ///
    /// Addresses must be registered before making RPC calls to a node.
    node_addresses: Arc<RwLock<HashMap<NodeId, String>>>,
}

impl RaftRpcClient {
    /// Create a new RaftRpcClient
    ///
    /// Creates an empty client with no registered nodes or connections.
    /// Use `register_node` to add nodes before making RPC calls.
    ///
    /// # Returns
    /// A new RaftRpcClient instance
    pub fn new() -> Self {
        Self {
            connections: Arc::new(RwLock::new(HashMap::new())),
            node_addresses: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Register a node's address for future connections
    ///
    /// Stores the node's address for use when establishing connections.
    /// The address should be in the format "http://host:port".
    ///
    /// # Arguments
    /// * `node_id` - The unique identifier of the node
    /// * `address` - The gRPC endpoint address (e.g., "http://127.0.0.1:5000")
    pub async fn register_node(&self, node_id: NodeId, address: String) {
        let mut addresses = self.node_addresses.write().await;
        addresses.insert(node_id, address);
    }

    /// Get or create a gRPC client for the given node
    ///
    /// Returns a cached connection if available, otherwise establishes a new
    /// connection to the node. The node's address must be registered first.
    ///
    /// # Arguments
    /// * `node_id` - The node to get a client for
    ///
    /// # Returns
    /// * `Ok(RaftServiceClient)` - A gRPC client for the node
    /// * `Err(ConsensusError::Network)` - Node address not registered or connection failed
    async fn get_client(&self, node_id: NodeId) -> ConsensusResult<RaftServiceClient<Channel>> {
        // Check if we already have a connection
        {
            let connections = self.connections.read().await;
            if let Some(client) = connections.get(&node_id) {
                return Ok(client.clone());
            }
        }

        // Get the node's address
        let address = {
            let addresses = self.node_addresses.read().await;
            addresses
                .get(&node_id)
                .cloned()
                .ok_or_else(|| ConsensusError::Network {
                    message: format!("No address registered for node {:?}", node_id),
                })?
        };

        // Create a new connection
        let endpoint =
            Endpoint::from_shared(address.clone()).map_err(|e| ConsensusError::Network {
                message: format!("Invalid endpoint {}: {}", address, e),
            })?;

        let channel = endpoint
            .connect()
            .await
            .map_err(|e| ConsensusError::Network {
                message: format!("Failed to connect to {}: {}", address, e),
            })?;

        let client = RaftServiceClient::new(channel);

        // Store the connection
        {
            let mut connections = self.connections.write().await;
            connections.insert(node_id, client.clone());
        }

        Ok(client)
    }

    /// Send a vote request to a peer node
    ///
    /// Sends a Raft vote request to the specified node for the given group.
    ///
    /// # Arguments
    /// * `node_id` - The target node ID
    /// * `group_id` - The Raft group/shard ID
    /// * `request` - The vote request
    ///
    /// # Returns
    /// * `Ok(ConsensusVoteResponse)` - The vote response from the peer
    /// * `Err(ConsensusError)` - RPC call failed
    pub async fn vote(
        &self,
        target: NodeId,
        group_id: ShardId,
        request: ConsensusVoteRequest,
    ) -> ConsensusResult<ConsensusVoteResponse> {
        let mut client = self.get_client(target).await?;

        let pb_request = VoteRequestMessage {
            group_id: Some(group_id.into()),
            request: Some(request.into()),
        };

        let response = client
            .vote(pb_request)
            .await
            .map_err(|e| ConsensusError::Network {
                message: format!("Vote RPC failed: {}", e),
            })?;

        let pb_response = response.into_inner();
        let vote_response = pb_response
            .response
            .ok_or_else(|| ConsensusError::Protocol {
                message: "Missing response in VoteResponseMessage".to_string(),
            })?
            .try_into()?;

        Ok(vote_response)
    }

    /// Send an append entries request to a peer node
    ///
    /// Sends a Raft append entries request to the specified node for the given group.
    /// This is used for log replication and heartbeats.
    ///
    /// # Arguments
    /// * `target` - The target node ID
    /// * `group_id` - The Raft group/shard ID
    /// * `request` - The append entries request
    ///
    /// # Returns
    /// * `Ok(ConsensusAppendEntriesResponse)` - The append entries response from the peer
    /// * `Err(ConsensusError)` - RPC call failed
    pub async fn append_entries(
        &self,
        target: NodeId,
        group_id: ShardId,
        request: ConsensusAppendEntriesRequest,
    ) -> ConsensusResult<ConsensusAppendEntriesResponse> {
        let mut client = self.get_client(target).await?;

        let pb_request = AppendEntriesRequestMessage {
            group_id: Some(group_id.into()),
            request: Some(request.into()),
        };

        let response =
            client
                .append_entries(pb_request)
                .await
                .map_err(|e| ConsensusError::Network {
                    message: format!("AppendEntries RPC failed: {}", e),
                })?;

        let pb_response = response.into_inner();
        let append_response = pb_response
            .response
            .ok_or_else(|| ConsensusError::Protocol {
                message: "Missing response in AppendEntriesResponseMessage".to_string(),
            })?
            .try_into()?;

        Ok(append_response)
    }

    /// Send an install snapshot request to a peer node (streaming)
    ///
    /// Sends a Raft install snapshot request to the specified node for the given group.
    /// This is used to bring a lagging follower up to date by sending a snapshot.
    ///
    /// # Arguments
    /// * `target` - The target node ID
    /// * `group_id` - The Raft group/shard ID
    /// * `request` - The install snapshot request
    ///
    /// # Returns
    /// * `Ok(InstallSnapshotResponse)` - The install snapshot response from the peer
    /// * `Err(ConsensusError)` - RPC call failed
    ///
    /// # Note
    /// Currently sends snapshots as a single message. TODO: Implement proper
    /// streaming for large snapshots.
    pub async fn install_snapshot(
        &self,
        target: NodeId,
        group_id: ShardId,
        request: openraft::raft::InstallSnapshotRequest<ConsensusTypeConfig>,
    ) -> ConsensusResult<openraft::raft::InstallSnapshotResponse<ConsensusTypeConfig>> {
        let mut client = self.get_client(target).await?;

        // For now, send as a single message
        // TODO: Implement proper streaming for large snapshots
        let pb_request = InstallSnapshotRequestMessage {
            group_id: Some(group_id.into()),
            request: Some(InstallSnapshotRequest {
                term: request.vote.leader_id.term,
                leader_id: request.vote.leader_id.voted_for.map(|id| id.into()),
                metadata: None, // TODO: Convert snapshot metadata
                offset: 0,
                data: vec![], // TODO: Read snapshot data
                done: true,
            }),
        };

        let response = client
            .install_snapshot(tokio_stream::once(pb_request))
            .await
            .map_err(|e| ConsensusError::Network {
                message: format!("InstallSnapshot RPC failed: {}", e),
            })?;

        let _pb_response = response.into_inner();

        // TODO: Convert response properly
        Ok(openraft::raft::InstallSnapshotResponse { vote: request.vote })
    }

    /// Remove a node's connection
    ///
    /// Removes a node's cached connection and address. This should be called
    /// when a node is removed from the cluster or when a connection needs to
    /// be reset.
    ///
    /// # Arguments
    /// * `node_id` - The node ID to remove
    pub async fn remove_node(&self, node_id: NodeId) {
        let mut connections = self.connections.write().await;
        connections.remove(&node_id);

        let mut addresses = self.node_addresses.write().await;
        addresses.remove(&node_id);
    }
}

impl Default for RaftRpcClient {
    fn default() -> Self {
        Self::new()
    }
}
