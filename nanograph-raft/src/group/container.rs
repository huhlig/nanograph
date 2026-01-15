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

use crate::error::{ConsensusError, ConsensusResult};
use crate::types::{MetadataChange, NodeInfo, RaftClusterState};
use nanograph_core::{
    object::{
        ClusterMetadata, NodeId, RegionId, ShardId, ShardMetadata, ShardStatus, StorageEngineType,
    },
    types::Timestamp,
};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info};

/// Container Metadata Shard Raft Group
///
/// Manages Container (Tenant+Database) Shard metadata via Raft consensus. All metadata changes
/// go through this group to ensure consistency across the cluster.
pub struct ContainerShardRaftGroup {
    /// Local node ID
    local_node_id: NodeId,

    /// Current Raft cluster state
    state: Arc<RwLock<RaftClusterState>>,

    /// Whether this node is the metadata leader
    is_leader: Arc<RwLock<bool>>,
}

impl ContainerShardRaftGroup {
    /// Create a new Container Shard Metadata Raft group
    pub fn new(local_node_id: NodeId) -> Self {
        info!("Creating metadata Raft group on node {}", local_node_id);

        Self {
            local_node_id,
            state: Arc::new(RwLock::new(RaftClusterState::default())),
            is_leader: Arc::new(RwLock::new(false)),
        }
    }

    /// Get the current cluster state (read-only)
    pub async fn get_state(&self) -> RaftClusterState {
        let state = self.state.read().await;
        state.clone()
    }

    /// Get current cluster metadata (read-only)
    pub async fn get_metadata(&self) -> ClusterMetadata {
        let state = self.state.read().await;
        state.cluster.clone()
    }

    /// Propose a metadata change
    pub async fn propose_change(&self, change: MetadataChange) -> ConsensusResult<()> {
        // Check if we're the leader
        let is_leader = self.is_leader.read().await;
        if !*is_leader {
            return Err(ConsensusError::NotLeader {
                shard_id: ShardId::new(0), // Metadata group uses shard_id 0
                leader: None,
            });
        }
        drop(is_leader);

        debug!("Proposing metadata change: {:?}", change);

        // TODO: Implement actual Raft proposal
        // For now, apply locally
        self.apply_change(change).await
    }

    /// Apply a metadata change
    async fn apply_change(&self, change: MetadataChange) -> ConsensusResult<()> {
        let mut state = self.state.write().await;
        state.cluster.version += 1;

        match change {
            MetadataChange::AddNode { node } => {
                info!("Adding node {} to cluster", node.node);
                state.nodes.insert(node.node, node);
            }

            MetadataChange::RemoveNode { node_id } => {
                info!("Removing node {} from cluster", node_id);
                state.nodes.remove(&node_id);

                // Remove node from all shard assignments
                for replicas in state.shard_assignments.values_mut() {
                    replicas.retain(|n| *n != node_id);
                }
            }

            MetadataChange::UpdateNodeStatus { node_id, status } => {
                info!("Updating node {} status to {:?}", node_id, status);
                if let Some(node) = state.nodes.get_mut(&node_id) {
                    node.status = status;
                }
            }

            MetadataChange::UpdateShardAssignment { shard_id, replicas } => {
                info!("Updating shard {} assignment: {:?}", shard_id, replicas);
                state.shard_assignments.insert(shard_id, replicas.clone());

                // Update shard metadata if it exists
                if let Some(shard) = state.shards.get_mut(&shard_id) {
                    shard.replicas = replicas;
                }
            }

            MetadataChange::UpdateShardLeader { shard_id, leader } => {
                info!("Updating shard {} leader to {}", shard_id, leader);
                if let Some(shard) = state.shards.get_mut(&shard_id) {
                    shard.leader = Some(leader);
                }
            }

            MetadataChange::CreateShard {
                shard_id,
                range,
                replicas,
            } => {
                info!("Creating shard {} with replicas {:?}", shard_id, replicas);

                let shard_metadata = ShardMetadata {
                    id: shard_id,
                    name: format!("shard_{}", shard_id.as_u64()),
                    version: 0,
                    created_at: Timestamp::now(),
                    engine_type: StorageEngineType::new("lsm"),
                    last_modified: Timestamp::now(),
                    range,
                    leader: None,
                    replicas: replicas.clone(),
                    status: ShardStatus::Active,
                    term: 0,
                    size_bytes: 0,
                };

                state.shards.insert(shard_id, shard_metadata);
                state.shard_assignments.insert(shard_id, replicas);
            }

            MetadataChange::DeleteShard { shard_id } => {
                info!("Deleting shard {}", shard_id);
                state.shards.remove(&shard_id);
                state.shard_assignments.remove(&shard_id);
            }
        }

        Ok(())
    }

    /// Add a node to the cluster
    pub async fn add_node(&self, node: NodeInfo) -> ConsensusResult<()> {
        self.propose_change(MetadataChange::AddNode { node }).await
    }

    /// Remove a node from the cluster
    pub async fn remove_node(&self, node_id: NodeId) -> ConsensusResult<()> {
        self.propose_change(MetadataChange::RemoveNode { node_id })
            .await
    }

    /// Update shard assignment
    pub async fn update_shard_assignment(
        &self,
        shard_id: ShardId,
        replicas: Vec<NodeId>,
    ) -> ConsensusResult<()> {
        self.propose_change(MetadataChange::UpdateShardAssignment { shard_id, replicas })
            .await
    }

    /// Update shard leader
    pub async fn update_shard_leader(
        &self,
        shard_id: ShardId,
        leader: NodeId,
    ) -> ConsensusResult<()> {
        self.propose_change(MetadataChange::UpdateShardLeader { shard_id, leader })
            .await
    }

    /// Create a new shard
    pub async fn create_shard(
        &self,
        shard_id: ShardId,
        range: (Vec<u8>, Vec<u8>),
        replicas: Vec<NodeId>,
    ) -> ConsensusResult<()> {
        self.propose_change(MetadataChange::CreateShard {
            shard_id,
            range,
            replicas,
        })
        .await
    }

    /// Delete a shard
    pub async fn delete_shard(&self, shard_id: ShardId) -> ConsensusResult<()> {
        self.propose_change(MetadataChange::DeleteShard { shard_id })
            .await
    }

    /// Handle becoming leader
    pub async fn on_become_leader(&self) {
        info!("Node {} became metadata leader", self.local_node_id);
        let mut is_leader = self.is_leader.write().await;
        *is_leader = true;
    }

    /// Handle becoming follower
    pub async fn on_become_follower(&self) {
        info!("Node {} became metadata follower", self.local_node_id);
        let mut is_leader = self.is_leader.write().await;
        *is_leader = false;
    }
}

#[cfg(test)]
mod tests {
    use super::{ContainerShardRaftGroup, NodeId};

    #[tokio::test]
    async fn test_metadata_group() {
        let group = ContainerShardRaftGroup::new(NodeId::new(1));
        let metadata = group.get_metadata().await;
        assert_eq!(metadata.version, 0);
    }
}
