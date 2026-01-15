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
use crate::storage::RaftStorageAdapter;
use crate::types::{Operation, OperationResponse, ReadConsistency, ReplicationConfig};
use nanograph_core::object::{NodeId, ShardId};
use std::sync::Arc;
use tokio::sync::RwLock;
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

    /// Storage adapter
    storage: Arc<RaftStorageAdapter>,

    /// Replication configuration
    config: ReplicationConfig,

    /// Current role (Leader, Follower, Candidate)
    role: Arc<RwLock<RaftRole>>,

    /// Peer nodes in this Raft group
    peers: Arc<RwLock<Vec<NodeId>>>,

    /// Current leader (if known)
    leader: Arc<RwLock<Option<NodeId>>>,

    /// Leader lease expiration (for lease-based reads)
    lease_expiry: Arc<RwLock<Option<std::time::Instant>>>,
}

/// Raft role
#[derive(Clone, Debug, PartialEq)]
pub enum RaftRole {
    /// Follower - receives log entries from leader
    Follower,

    /// Candidate - attempting to become leader
    Candidate,

    /// Leader - accepts writes and replicates to followers
    Leader,
}

impl TableShardRaftGroup {
    /// Create a new shard Raft group
    pub fn new(
        shard_id: ShardId,
        local_node_id: NodeId,
        storage: Arc<RaftStorageAdapter>,
        peers: Vec<NodeId>,
        config: ReplicationConfig,
    ) -> Self {
        info!(
            "Creating Raft group for shard {} on node {}",
            shard_id, local_node_id
        );

        Self {
            shard_id,
            local_node_id,
            storage,
            config,
            role: Arc::new(RwLock::new(RaftRole::Follower)),
            peers: Arc::new(RwLock::new(peers)),
            leader: Arc::new(RwLock::new(None)),
            lease_expiry: Arc::new(RwLock::new(None)),
        }
    }

    /// Propose a write operation
    ///
    /// This will replicate the operation via Raft consensus and apply it
    /// to the state machine once committed.
    pub async fn propose_write(&self, operation: Operation) -> ConsensusResult<OperationResponse> {
        // Check if we're the leader
        let role = self.role.read().await;
        if *role != RaftRole::Leader {
            let leader = self.leader.read().await;
            return Err(ConsensusError::NotLeader {
                shard_id: self.shard_id,
                leader: leader.map(|n| n),
            });
        }
        drop(role);

        // Check if we have quorum
        if !self.has_quorum().await {
            return Err(ConsensusError::NoQuorum {
                shard_id: self.shard_id,
                required: self.config.quorum_size(),
                available: self.count_active_peers().await,
            });
        }

        debug!(
            "Proposing write to shard {}: {:?}",
            self.shard_id, operation
        );

        // TODO: Implement actual Raft proposal
        // For now, just apply locally (single-node mode)
        self.storage.apply_operation(&operation).await
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
        // Must be leader
        let role = self.role.read().await;
        if *role != RaftRole::Leader {
            let leader = self.leader.read().await;
            return Err(ConsensusError::NotLeader {
                shard_id: self.shard_id,
                leader: leader.map(|n| n),
            });
        }
        drop(role);

        // TODO: Implement ReadIndex protocol
        // For now, just read locally
        self.follower_read(key).await
    }

    /// Lease-based read (fast, requires clock sync)
    async fn lease_read(&self, key: &[u8]) -> ConsensusResult<Option<Vec<u8>>> {
        // Must be leader with valid lease
        let role = self.role.read().await;
        if *role != RaftRole::Leader {
            let leader = self.leader.read().await;
            return Err(ConsensusError::NotLeader {
                shard_id: self.shard_id,
                leader: leader.map(|n| n),
            });
        }
        drop(role);

        // Check lease validity
        let lease = self.lease_expiry.read().await;
        if let Some(expiry) = *lease {
            if std::time::Instant::now() > expiry {
                return Err(ConsensusError::Internal {
                    message: "Leader lease expired".to_string(),
                });
            }
        } else {
            return Err(ConsensusError::Internal {
                message: "No leader lease".to_string(),
            });
        }
        drop(lease);

        self.follower_read(key).await
    }

    /// Follower read (potentially stale)
    async fn follower_read(&self, _key: &[u8]) -> ConsensusResult<Option<Vec<u8>>> {
        // TODO: Implement actual read from storage
        // For now, return None
        Ok(None)
    }

    /// Check if we have quorum
    async fn has_quorum(&self) -> bool {
        let active = self.count_active_peers().await;
        active >= self.config.quorum_size()
    }

    /// Count active peers (including self)
    async fn count_active_peers(&self) -> usize {
        let peers = self.peers.read().await;
        // TODO: Actually check peer health
        // For now, assume all peers are active
        peers.len() + 1 // +1 for self
    }

    /// Handle becoming leader
    pub async fn on_become_leader(&self) {
        info!(
            "Node {} became leader for shard {}",
            self.local_node_id, self.shard_id
        );

        let mut role = self.role.write().await;
        *role = RaftRole::Leader;
        drop(role);

        let mut leader = self.leader.write().await;
        *leader = Some(self.local_node_id);
        drop(leader);

        // Establish leader lease
        self.establish_lease().await;
    }

    /// Handle becoming follower
    pub async fn on_become_follower(&self, new_leader: Option<NodeId>) {
        info!(
            "Node {} became follower for shard {} (leader: {:?})",
            self.local_node_id, self.shard_id, new_leader
        );

        let mut role = self.role.write().await;
        *role = RaftRole::Follower;
        drop(role);

        let mut leader = self.leader.write().await;
        *leader = new_leader;
        drop(leader);

        // Clear leader lease
        let mut lease = self.lease_expiry.write().await;
        *lease = None;
    }

    /// Establish leader lease
    async fn establish_lease(&self) {
        let lease_duration =
            std::time::Duration::from_millis(self.config.heartbeat_interval_ms * 2);
        let expiry = std::time::Instant::now() + lease_duration;

        let mut lease = self.lease_expiry.write().await;
        *lease = Some(expiry);

        debug!(
            "Established leader lease for shard {} until {:?}",
            self.shard_id, expiry
        );
    }

    /// Check if this node is the leader
    pub async fn is_leader(&self) -> bool {
        let role = self.role.read().await;
        *role == RaftRole::Leader
    }

    /// Get current leader
    pub async fn get_leader(&self) -> Option<NodeId> {
        let leader = self.leader.read().await;
        *leader
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

        let mut peers = self.peers.write().await;
        if !peers.contains(&peer) {
            peers.push(peer);
        }

        // TODO: Implement Raft membership change
        Ok(())
    }

    /// Remove a peer from the Raft group
    pub async fn remove_peer(&self, peer: NodeId) -> ConsensusResult<()> {
        info!("Removing peer {} from shard {}", peer, self.shard_id);

        let mut peers = self.peers.write().await;
        peers.retain(|p| *p != peer);

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
