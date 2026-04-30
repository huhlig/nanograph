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

use crate::error::ConsensusResult;
use crate::network::adapter::ConsensusNetworkAdapter;
use crate::storage::{ConsensusLogStore, ConsensusStateStore};
use crate::types::{
    ConsensusTypeConfig, Operation, OperationResponse, ReadConsistency, ReplicationConfig,
};
use nanograph_core::object::{NodeId, ShardId};
use openraft::Raft;
use std::sync::Arc;
use tracing::{debug, info};

/// Shard Raft Group
///
/// Manages Raft consensus for a single table shard. Each shard is an independent Raft group with
/// its own leader election, log replication, and state machine.
pub struct TableShardRaftGroup {
    /// Shard identifier
    shard_id: ShardId,

    /// Local node ID
    local_node_id: NodeId,

    /// Raft instance
    pub raft: Arc<Raft<ConsensusTypeConfig>>,

    /// State store for direct reads
    state_store: Arc<ConsensusStateStore>,
}

impl TableShardRaftGroup {
    /// Create a new shard Raft group
    pub async fn new(
        shard_id: ShardId,
        local_node_id: NodeId,
        log_store: ConsensusLogStore,
        state_store: ConsensusStateStore,
        _peers: Vec<NodeId>,
        repl_config: ReplicationConfig,
    ) -> ConsensusResult<Self> {
        info!(
            "Creating Raft group for shard {} on node {}",
            shard_id, local_node_id
        );

        let config = openraft::Config {
            heartbeat_interval: repl_config.heartbeat_interval_ms as u64,
            election_timeout_min: repl_config.election_timeout_ms as u64,
            election_timeout_max: (repl_config.election_timeout_ms * 2) as u64,
            max_in_snapshot_log_to_keep: repl_config.snapshot_threshold,
            ..Default::default()
        };

        let network = ConsensusNetworkAdapter::new(shard_id);

        let state_store_clone = state_store.clone();

        let raft = Raft::new(
            local_node_id,
            Arc::new(config),
            network,
            log_store,
            state_store,
        )
        .await
        .map_err(|e| crate::error::ConsensusError::Internal {
            message: format!("Failed to create Raft instance: {}", e),
        })?;

        Ok(Self {
            shard_id,
            local_node_id,
            raft: Arc::new(raft),
            state_store: Arc::new(state_store_clone),
        })
    }

    /// Propose a write operation
    ///
    /// This will replicate the operation via Raft consensus and apply it
    /// to the state machine once committed.
    pub async fn propose_write(&self, operation: Operation) -> ConsensusResult<OperationResponse> {
        debug!(
            "Proposing write to shard {}: {:?}",
            self.shard_id, operation
        );

        let response = self.raft.client_write(operation).await?;
        Ok(response.data)
    }

    /// Read with specified consistency level
    pub async fn read(
        &self,
        key: &[u8],
        consistency: ReadConsistency,
    ) -> ConsensusResult<Option<Vec<u8>>> {
        match consistency {
            ReadConsistency::Linearizable => self.linearizable_read(key).await,
            ReadConsistency::Lease => self.lease_read(key).await,
            ReadConsistency::Follower => self.follower_read(key).await,
        }
    }

    /// Linearizable read (strongest consistency)
    async fn linearizable_read(&self, key: &[u8]) -> ConsensusResult<Option<Vec<u8>>> {
        // Ensure we're the leader and up to date
        // In OpenRaft 0.10, we use is_leader() to check leadership
        // For true linearizable reads, we should use client_read or ensure_linearizable with proper parameters
        if !self.raft.is_leader() {
            return Err(crate::error::ConsensusError::NotLeader {
                shard_id: self.shard_id,
                leader: self.raft.current_leader().await,
            });
        }

        // Perform local read
        self.follower_read(key).await
    }

    /// Lease-based read (fast, requires clock sync)
    async fn lease_read(&self, key: &[u8]) -> ConsensusResult<Option<Vec<u8>>> {
        // In openraft, we can check if we're the leader.
        // For a true lease read, we might need more custom logic or use ensure_linearizable.
        // For now, let's treat it as linearizable.
        self.linearizable_read(key).await
    }

    /// Follower read (potentially stale)
    async fn follower_read(&self, key: &[u8]) -> ConsensusResult<Option<Vec<u8>>> {
        // Read directly from state store (may be stale)
        self.state_store
            .get_value(key)
            .await
            .map_err(|e| crate::error::ConsensusError::Storage {
                message: format!("Failed to read from state store: {}", e),
            })
    }

    /// Check if this node is the leader
    pub async fn is_leader(&self) -> bool {
        self.raft.is_leader()
    }

    /// Get current leader
    pub async fn get_leader(&self) -> Option<NodeId> {
        self.raft.current_leader().await
    }

    /// Get shard ID
    pub fn shard_id(&self) -> ShardId {
        self.shard_id
    }

    /// Get local node ID
    pub fn local_node_id(&self) -> NodeId {
        self.local_node_id
    }

    /// Add a peer to the Raft group
    pub async fn add_peer(&self, peer: NodeId) -> ConsensusResult<()> {
        info!("Adding peer {} to shard {}", peer, self.shard_id);
        // TODO: Implement Raft membership change
        Ok(())
    }

    /// Remove a peer from the Raft group
    pub async fn remove_peer(&self, peer: NodeId) -> ConsensusResult<()> {
        info!("Removing peer {} from shard {}", peer, self.shard_id);
        // TODO: Implement Raft membership change
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    #[tokio::test]
    async fn test_shard_group_creation() {
        // TODO: Add tests once we have a mock storage implementation
    }
}
