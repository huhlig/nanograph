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

//! Network adapter for OpenRaft integration
//!
//! This module provides the adapter layer between OpenRaft's network traits
//! and our gRPC-based Raft RPC implementation.

use crate::network::client::RaftRpcClient;
use crate::types::{
    ConsensusAppendEntriesRequest, ConsensusAppendEntriesResponse, ConsensusRPCError,
    ConsensusSnapshot, ConsensusSnapshotResponse, ConsensusStreamingError, ConsensusTypeConfig,
    ConsensusVoteOf, ConsensusVoteRequest, ConsensusVoteResponse, NodeInfo,
};
use nanograph_core::object::{NodeId, ShardId};
use openraft::error::ReplicationClosed;
use openraft::network::RPCOption;
use openraft::{OptionalSend, RaftNetworkFactory, RaftNetworkV2};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Network adapter for a Raft group
///
/// Implements OpenRaft's `RaftNetworkFactory` trait to create network clients
/// for communicating with peer nodes in a Raft group. Manages node addresses
/// and delegates actual RPC calls to the gRPC client.
pub struct ConsensusNetworkAdapter {
    /// Shared gRPC client for making RPC calls
    client: Arc<RaftRpcClient>,

    /// Map of node IDs to their information
    nodes: Arc<RwLock<HashMap<NodeId, NodeInfo>>>,

    /// The Raft group ID this adapter serves
    group_id: ShardId,
}

impl ConsensusNetworkAdapter {
    /// Create a new network adapter for a Raft group
    ///
    /// # Arguments
    /// * `group_id` - The shard/group ID this adapter will serve
    ///
    /// # Returns
    /// A new network adapter instance
    pub fn new(group_id: ShardId) -> Self {
        Self {
            client: Arc::new(RaftRpcClient::new()),
            nodes: Arc::new(RwLock::new(HashMap::new())),
            group_id,
        }
    }

    /// Add a node to this adapter's registry
    ///
    /// Registers a node's information and address for future RPC calls.
    ///
    /// # Arguments
    /// * `node_id` - The unique identifier of the node
    /// * `node_info` - Information about the node including its Raft address
    pub async fn add_node(&self, node_id: NodeId, node_info: NodeInfo) {
        {
            let mut nodes = self.nodes.write().await;
            nodes.insert(node_id, node_info.clone());
        }

        // Register the node's address with the client
        self.client
            .register_node(node_id, format!("http://{}", node_info.raft_addr))
            .await;
    }
}

impl RaftNetworkFactory<ConsensusTypeConfig> for ConsensusNetworkAdapter {
    type Network = ConsensusNodeNetwork;

    async fn new_client(&mut self, target: NodeId, node: &NodeInfo) -> Self::Network {
        // Register the node's address with the client
        self.client
            .register_node(target, format!("http://{}", node.raft_addr))
            .await;

        ConsensusNodeNetwork {
            target,
            group_id: self.group_id,
            client: self.client.clone(),
        }
    }
}

/// Network client for communicating with a specific peer node
///
/// Implements OpenRaft's `RaftNetworkV2` trait to handle RPC calls to a
/// specific target node within a Raft group.
pub struct ConsensusNodeNetwork {
    /// The target node ID for RPC calls
    target: NodeId,

    /// The Raft group ID
    group_id: ShardId,

    /// Shared gRPC client for making RPC calls
    client: Arc<RaftRpcClient>,
}

impl RaftNetworkV2<ConsensusTypeConfig> for ConsensusNodeNetwork {
    async fn append_entries(
        &mut self,
        rpc: ConsensusAppendEntriesRequest,
        _option: RPCOption,
    ) -> Result<ConsensusAppendEntriesResponse, ConsensusRPCError> {
        self.client
            .append_entries(self.target, self.group_id, rpc)
            .await
            .map_err(|e| ConsensusRPCError::Network(openraft::error::NetworkError::new(&e)))
    }

    async fn vote(
        &mut self,
        rpc: ConsensusVoteRequest,
        _option: RPCOption,
    ) -> Result<ConsensusVoteResponse, ConsensusRPCError> {
        self.client
            .vote(self.target, self.group_id, rpc)
            .await
            .map_err(|e| ConsensusRPCError::Network(openraft::error::NetworkError::new(&e)))
    }

    async fn full_snapshot(
        &mut self,
        vote: ConsensusVoteOf,
        snapshot: ConsensusSnapshot,
        _cancel: impl std::future::Future<Output = ReplicationClosed> + OptionalSend + 'static,
        _option: RPCOption,
    ) -> Result<ConsensusSnapshotResponse, ConsensusStreamingError> {
        // Convert snapshot to InstallSnapshotRequest
        let request = openraft::raft::InstallSnapshotRequest {
            vote,
            meta: snapshot.meta.clone(),
            offset: 0,
            data: vec![], // TODO: Read snapshot data from snapshot.snapshot
            done: true,
        };

        let install_response = self
            .client
            .install_snapshot(self.target, self.group_id, request)
            .await
            .map_err(|e| {
                ConsensusStreamingError::Network(openraft::error::NetworkError::new(&e))
            })?;

        // Convert InstallSnapshotResponse to SnapshotResponse
        Ok(ConsensusSnapshotResponse {
            vote: install_response.vote,
        })
    }
}
