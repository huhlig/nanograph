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

//! Router for distributed operations
//!
//! Routes operations to the correct shard based on key hashing.

use crate::error::{ConsensusError, ConsensusResult};
use crate::metadata::MetadataRaftGroup;
use crate::shard_group::ShardRaftGroup;
use crate::storage::RaftStorageAdapter;
use crate::types::{Operation, ReadConsistency, ReplicationConfig};
use nanograph_core::types::{NodeId, RegionId};
use nanograph_kvt::{KeyValueShardStore, ShardId};
use std::collections::HashMap;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info};
use crate::NodeInfo;

/// Router for distributed operations
///
/// The router is responsible for:
/// - Routing operations to the correct shard based on key
/// - Managing shard Raft groups
/// - Coordinating with metadata Raft group
/// - Handling cross-shard operations
pub struct ConsensusRouter {
    /// Local node ID
    local_node_id: NodeId,

    /// Replication configuration
    config: ReplicationConfig,

    /// Metadata Raft group
    metadata: Arc<MetadataRaftGroup>,

    /// Peer Nodes
    peers: Arc<RwLock<HashMap<NodeId, NodeInfo>>>,

    /// Shard Raft groups (shard_id -> group)
    shards: Arc<RwLock<HashMap<ShardId, Arc<ShardRaftGroup>>>>,

    /// Total number of shards in the cluster
    shard_count: Arc<RwLock<u32>>,
}

impl ConsensusRouter {
    /// Create a new router
    pub fn new(local_node_id: NodeId, config: ReplicationConfig) -> Self {
        info!("Creating router on node {}", local_node_id);

        Self {
            local_node_id,
            config,
            metadata: Arc::new(MetadataRaftGroup::new(local_node_id)),
            peers: Arc::new(RwLock::new(HashMap::new())),
            shards: Arc::new(RwLock::new(HashMap::new())),
            shard_count: Arc::new(RwLock::new(1)), // Default to single shard
        }
    }

    /// Get Local Node Id
    pub fn node_id(&self) -> NodeId {
        self.local_node_id
    }

    /// Get a List of all peers
    pub async fn peer_nodes(&self) -> Vec<NodeId> {
        self.peers.read().await.keys().cloned().collect::<Vec<_>>()
    }

    /// Set the total number of shards
    pub async fn set_shard_count(&self, count: u32) {
        let mut shard_count = self.shard_count.write().await;
        *shard_count = count;
        info!("Set shard count to {}", count);
    }

    /// Add a shard to this node
    pub async fn add_shard(
        &self,
        shard_id: ShardId,
        storage: Box<dyn KeyValueShardStore>,
        peers: Vec<NodeId>,
    ) -> ConsensusResult<()> {
        info!("Adding shard {} to node {}", shard_id, self.local_node_id);

        let storage_adapter = Arc::new(RaftStorageAdapter::new(storage, shard_id));

        let shard_group = Arc::new(ShardRaftGroup::new(
            shard_id,
            self.local_node_id,
            storage_adapter,
            peers,
            self.config.clone(),
        ));

        let mut shards = self.shards.write().await;
        shards.insert(shard_id, shard_group);

        Ok(())
    }

    /// Remove a shard from this node
    pub async fn remove_shard(&self, shard_id: ShardId) -> ConsensusResult<()> {
        info!(
            "Removing shard {} from node {}",
            shard_id, self.local_node_id
        );

        let mut shards = self.shards.write().await;
        shards.remove(&shard_id);

        Ok(())
    }

    /// Get shard for a key using hash-based partitioning
    pub async fn get_shard_for_key(&self, key: &[u8]) -> ShardId {
        let shard_count = *self.shard_count.read().await;

        if shard_count == 1 {
            return ShardId::new(0);
        }

        let mut hasher = DefaultHasher::new();
        key.hash(&mut hasher);
        let hash = hasher.finish();

        ShardId::new(hash % shard_count as u64)
    }

    /// Get shard group by ID
    async fn get_shard_group(&self, shard_id: ShardId) -> ConsensusResult<Arc<ShardRaftGroup>> {
        let shards = self.shards.read().await;
        shards
            .get(&shard_id)
            .cloned()
            .ok_or_else(|| ConsensusError::ShardNotFound { shard_id })
    }

    /// Put a key-value pair
    pub async fn put(&self, key: Vec<u8>, value: Vec<u8>) -> ConsensusResult<()> {
        let shard_id = self.get_shard_for_key(&key).await;
        debug!("Routing PUT to shard {}", shard_id);

        let shard = self.get_shard_group(shard_id).await?;
        let operation = Operation::Put { key, value };

        shard.propose_write(operation).await?;
        Ok(())
    }

    /// Get a value by key
    pub async fn get(&self, key: &[u8]) -> ConsensusResult<Option<Vec<u8>>> {
        self.get_with_consistency(key, ReadConsistency::Linearizable)
            .await
    }

    /// Get a value with specified consistency level
    pub async fn get_with_consistency(
        &self,
        key: &[u8],
        consistency: ReadConsistency,
    ) -> ConsensusResult<Option<Vec<u8>>> {
        let shard_id = self.get_shard_for_key(key).await;
        debug!("Routing GET to shard {} with {:?}", shard_id, consistency);

        let shard = self.get_shard_group(shard_id).await?;
        shard.read(key, consistency).await
    }

    /// Delete a key
    pub async fn delete(&self, key: Vec<u8>) -> ConsensusResult<()> {
        let shard_id = self.get_shard_for_key(&key).await;
        debug!("Routing DELETE to shard {}", shard_id);

        let shard = self.get_shard_group(shard_id).await?;
        let operation = Operation::Delete { key };

        shard.propose_write(operation).await?;
        Ok(())
    }

    /// Execute a batch of operations
    ///
    /// Note: This only provides atomicity within a single shard.
    /// Cross-shard atomicity is not supported in Phase 2.
    pub async fn batch(&self, operations: Vec<Operation>) -> ConsensusResult<()> {
        // Group operations by shard
        let mut shard_ops: HashMap<ShardId, Vec<Operation>> = HashMap::new();

        for op in operations {
            let key = match &op {
                Operation::Put { key, .. } => key,
                Operation::Delete { key } => key,
                Operation::Batch { .. } => {
                    return Err(ConsensusError::Internal {
                        message: "Nested batch operations not supported".to_string(),
                    });
                }
            };

            let shard_id = self.get_shard_for_key(key).await;
            shard_ops.entry(shard_id).or_insert_with(Vec::new).push(op);
        }

        // Execute batches per shard
        for (shard_id, ops) in shard_ops {
            debug!("Routing batch of {} ops to shard {}", ops.len(), shard_id);

            let shard = self.get_shard_group(shard_id).await?;
            let batch_op = Operation::Batch { operations: ops };

            shard.propose_write(batch_op).await?;
        }

        Ok(())
    }

    /// Get metadata Raft group
    pub fn metadata(&self) -> &MetadataRaftGroup {
        &self.metadata
    }

    /// Get all local shards
    pub async fn local_shards(&self) -> Vec<ShardId> {
        let shards = self.shards.read().await;
        shards.keys().copied().collect()
    }

    /// Get shard group (for advanced operations)
    pub async fn shard_group(&self, shard_id: ShardId) -> ConsensusResult<Arc<ShardRaftGroup>> {
        self.get_shard_group(shard_id).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_router_creation() {
        let router = ConsensusRouter::new(NodeId::new(1), ReplicationConfig::default());
        assert_eq!(router.local_node_id, NodeId::new(1));
    }

    #[tokio::test]
    async fn test_shard_routing() {
        let router = ConsensusRouter::new(NodeId::new(1), ReplicationConfig::default());
        router.set_shard_count(4).await;

        let key1 = b"test_key_1";
        let key2 = b"test_key_2";

        let shard1 = router.get_shard_for_key(key1).await;
        let shard2 = router.get_shard_for_key(key2).await;

        // Same key should always route to same shard
        assert_eq!(shard1, router.get_shard_for_key(key1).await);
        assert_eq!(shard2, router.get_shard_for_key(key2).await);
    }
}
