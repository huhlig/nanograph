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

//! Consensus Manager for distributed operations
//!
//! Routes operations to the correct shard based on key hashing and manages
//! Raft groups for system, container, and table shards.

use crate::error::{ConsensusError, ConsensusResult};
use crate::group::{ContainerShardRaftGroup, SystemShardRaftGroup, TableShardRaftGroup};
use crate::network::server::RaftService;
use crate::storage::{ConsensusLogStore, ConsensusStateStore};
use crate::types::{NodeInfo, Operation, ReadConsistency, ReplicationConfig};
use nanograph_core::object::{ContainerId, NodeId, ShardId};
use std::collections::HashMap;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::{Mutex, RwLock};
use tokio::task::JoinHandle;
use tracing::{debug, error, info, warn};

/// Router for distributed operations
///
/// The router is responsible for:
/// - Routing operations to the correct shard based on key
/// - Managing shard Raft groups
/// - Coordinating with metadata Raft group
/// - Handling cross-shard operations
/// - Managing network server lifecycle
pub struct ConsensusManager {
    /// Local node ID
    local_node_id: NodeId,

    /// Replication configuration
    config: ReplicationConfig,

    /// Peer Nodes
    peers: Arc<RwLock<HashMap<NodeId, NodeInfo>>>,

    /// Metadata Raft group
    system_metadata: Arc<SystemShardRaftGroup>,

    /// Container Metadata Raft groups (container_id -> group)
    container_metadata: Arc<RwLock<HashMap<ContainerId, Arc<ContainerShardRaftGroup>>>>,

    /// Table Shard Raft groups (shard_id -> group)
    table_shards: Arc<RwLock<HashMap<ShardId, Arc<TableShardRaftGroup>>>>,

    /// Total number of shards in the cluster
    shard_count: Arc<RwLock<u32>>,

    /// Network server state
    server_state: Arc<Mutex<ServerState>>,
}

/// State of the network server
///
/// Tracks the lifecycle of the gRPC server including its task handle,
/// shutdown signal, and bind address. This state is protected by a mutex
/// to ensure thread-safe access to server lifecycle operations.
struct ServerState {
    /// gRPC server task handle
    ///
    /// When present, indicates the server is running. The handle can be
    /// awaited to detect server completion or errors.
    server_handle: Option<JoinHandle<ConsensusResult<()>>>,

    /// Shutdown signal sender
    ///
    /// Used to gracefully signal the server to shut down. Sending on this
    /// channel triggers the server's shutdown sequence.
    shutdown_tx: Option<tokio::sync::oneshot::Sender<()>>,

    /// Server bind address
    ///
    /// The socket address the server is bound to, if running.
    bind_addr: Option<SocketAddr>,
}

impl Default for ServerState {
    fn default() -> Self {
        Self {
            server_handle: None,
            shutdown_tx: None,
            bind_addr: None,
        }
    }
}

impl ConsensusManager {
    /// Create a new consensus manager
    ///
    /// Initializes a new consensus manager for the local node with the specified
    /// replication configuration. The manager starts with no peers, a single shard,
    /// and no running server.
    ///
    /// # Arguments
    /// * `local_node_id` - The unique identifier for this node
    /// * `config` - Replication configuration including quorum sizes and timeouts
    ///
    /// # Returns
    /// A new `ConsensusManager` instance ready to be configured and started
    ///
    /// # Example
    /// ```no_run
    /// use nanograph_raft::manager::ConsensusManager;
    /// use nanograph_raft::types::ReplicationConfig;
    /// use nanograph_core::object::NodeId;
    ///
    /// let manager = ConsensusManager::new(
    ///     NodeId::new(1),
    ///     ReplicationConfig::default()
    /// );
    /// ```
    pub fn new(local_node_id: NodeId, config: ReplicationConfig) -> Self {
        info!("Creating Consensus Manager on node {}", local_node_id);

        Self {
            local_node_id,
            config,
            peers: Arc::new(RwLock::new(HashMap::new())),
            system_metadata: Arc::new(SystemShardRaftGroup::new(local_node_id)),
            container_metadata: Arc::new(RwLock::new(HashMap::new())),
            table_shards: Arc::new(RwLock::new(HashMap::new())),
            shard_count: Arc::new(RwLock::new(1)), // Default to single shard
            server_state: Arc::new(Mutex::new(ServerState::default())),
        }
    }

    /// Start the gRPC server for Raft communication
    ///
    /// This method binds to the specified address and starts serving Raft RPC requests.
    /// The server runs on the provided tokio runtime (or current runtime if None).
    ///
    /// # Arguments
    /// * `bind_addr` - The socket address to bind the server to
    ///
    /// # Returns
    /// * `Ok(())` if the server started successfully
    /// * `Err(ConsensusError)` if the server failed to start
    pub async fn start_server(self: Arc<Self>, bind_addr: SocketAddr) -> ConsensusResult<()> {
        let mut state = self.server_state.lock().await;

        // Check if server is already running
        if state.server_handle.is_some() {
            return Err(ConsensusError::Internal {
                message: "Server is already running".to_string(),
            });
        }

        info!("Starting Raft gRPC server on {}", bind_addr);

        // Create shutdown channel
        let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel();

        // Clone self for the server task
        let manager = self.clone();

        // Spawn the server task
        let server_handle = tokio::spawn(async move {
            let service = RaftService::new(manager);
            let server = service.into_server();

            tonic::transport::Server::builder()
                .add_service(server)
                .serve_with_shutdown(bind_addr, async {
                    shutdown_rx.await.ok();
                    info!("Shutting down Raft gRPC server");
                })
                .await
                .map_err(|e| ConsensusError::Network {
                    message: format!("gRPC server error: {}", e),
                })?;

            Ok(())
        });

        // Store server state
        state.server_handle = Some(server_handle);
        state.shutdown_tx = Some(shutdown_tx);
        state.bind_addr = Some(bind_addr);

        info!("Raft gRPC server started successfully on {}", bind_addr);
        Ok(())
    }

    /// Stop the gRPC server
    ///
    /// This method gracefully shuts down the gRPC server if it's running.
    ///
    /// # Returns
    /// * `Ok(())` if the server stopped successfully or wasn't running
    /// * `Err(ConsensusError)` if there was an error stopping the server
    pub async fn stop_server(&self) -> ConsensusResult<()> {
        let mut state = self.server_state.lock().await;

        if let Some(shutdown_tx) = state.shutdown_tx.take() {
            info!("Stopping Raft gRPC server");

            // Send shutdown signal
            if shutdown_tx.send(()).is_err() {
                warn!("Failed to send shutdown signal (server may have already stopped)");
            }

            // Wait for server to finish
            if let Some(handle) = state.server_handle.take() {
                match handle.await {
                    Ok(Ok(())) => {
                        info!("Raft gRPC server stopped successfully");
                    }
                    Ok(Err(e)) => {
                        error!("Server task returned error: {:?}", e);
                        return Err(e);
                    }
                    Err(e) => {
                        error!("Failed to join server task: {}", e);
                        return Err(ConsensusError::Internal {
                            message: format!("Failed to join server task: {}", e),
                        });
                    }
                }
            }

            state.bind_addr = None;
        } else {
            debug!("Server is not running");
        }

        Ok(())
    }

    /// Check if the server is running
    ///
    /// Returns `true` if the gRPC server is currently running, `false` otherwise.
    ///
    /// # Returns
    /// Boolean indicating server running state
    pub async fn is_server_running(&self) -> bool {
        let state = self.server_state.lock().await;
        state.server_handle.is_some()
    }

    /// Get the server bind address if running
    ///
    /// Returns the socket address the server is bound to, or `None` if the
    /// server is not running.
    ///
    /// # Returns
    /// * `Some(SocketAddr)` - The address the server is bound to
    /// * `None` - Server is not running
    pub async fn server_address(&self) -> Option<SocketAddr> {
        let state = self.server_state.lock().await;
        state.bind_addr
    }

    /// Get the local node ID
    ///
    /// Returns the unique identifier for this node in the cluster.
    ///
    /// # Returns
    /// The local node's ID
    pub fn node_id(&self) -> NodeId {
        self.local_node_id
    }

    /// Get the replication configuration
    ///
    /// Returns a reference to the replication configuration used by this manager.
    ///
    /// # Returns
    /// Reference to the replication configuration
    pub fn config(&self) -> &ReplicationConfig {
        &self.config
    }

    /// Add a peer node to the cluster
    ///
    /// This registers the peer node's information for future communication.
    ///
    /// # Arguments
    /// * `node_id` - The unique identifier of the peer node
    /// * `node_info` - Information about the peer node including addresses
    pub async fn add_peer(&self, node_id: NodeId, node_info: NodeInfo) {
        let mut peers = self.peers.write().await;
        peers.insert(node_id, node_info);
        info!("Added peer node {} to cluster", node_id);
    }

    /// Remove a peer node from the cluster
    ///
    /// # Arguments
    /// * `node_id` - The unique identifier of the peer node to remove
    pub async fn remove_peer(&self, node_id: NodeId) -> Option<NodeInfo> {
        let mut peers = self.peers.write().await;
        let removed = peers.remove(&node_id);
        if removed.is_some() {
            info!("Removed peer node {} from cluster", node_id);
        }
        removed
    }

    /// Get information about a specific peer node
    ///
    /// # Arguments
    /// * `node_id` - The unique identifier of the peer node
    ///
    /// # Returns
    /// * `Some(NodeInfo)` if the peer exists
    /// * `None` if the peer is not registered
    pub async fn get_peer(&self, node_id: NodeId) -> Option<NodeInfo> {
        let peers = self.peers.read().await;
        peers.get(&node_id).cloned()
    }

    /// Get a list of all peer node IDs
    ///
    /// Returns a vector containing the IDs of all registered peer nodes.
    ///
    /// # Returns
    /// Vector of peer node IDs
    pub async fn peer_nodes(&self) -> Vec<NodeId> {
        self.peers.read().await.keys().cloned().collect::<Vec<_>>()
    }

    /// Get all peer node information
    ///
    /// Returns a complete map of all peer nodes and their information.
    ///
    /// # Returns
    /// HashMap mapping node IDs to their information
    pub async fn all_peers(&self) -> HashMap<NodeId, NodeInfo> {
        self.peers.read().await.clone()
    }

    /// Set the total number of shards
    ///
    /// Configures the total number of shards in the cluster. This affects
    /// how keys are distributed across shards using hash-based partitioning.
    ///
    /// # Arguments
    /// * `count` - Total number of shards in the cluster
    pub async fn set_shard_count(&self, count: u32) {
        let mut shard_count = self.shard_count.write().await;
        *shard_count = count;
        info!("Set shard count to {}", count);
    }

    /// Add a table shard to this node
    ///
    /// Registers a new table shard on this node with the specified storage
    /// backends and peer configuration. Creates a new Raft group for the shard.
    ///
    /// # Arguments
    /// * `shard_id` - Unique identifier for the shard
    /// * `log_store` - Log storage backend for Raft
    /// * `state_store` - State storage backend for Raft
    /// * `peers` - List of peer node IDs in this shard's Raft group
    ///
    /// # Returns
    /// * `Ok(())` - Shard added successfully
    /// * `Err(ConsensusError)` - Failed to create or add shard
    pub async fn add_table_shard(
        &self,
        shard_id: ShardId,
        log_store: Arc<ConsensusLogStore>,
        state_store: Arc<ConsensusStateStore>,
        peers: Vec<NodeId>,
    ) -> ConsensusResult<()> {
        info!("Adding shard {} to node {}", shard_id, self.local_node_id);

        // Clone the storage to get owned values for Raft
        let log_store_owned = (*log_store).clone();
        let state_store_owned = (*state_store).clone();

        let shard_group = Arc::new(
            TableShardRaftGroup::new(
                shard_id,
                self.local_node_id,
                log_store_owned,
                state_store_owned,
                peers,
                self.config.clone(),
            )
            .await?,
        );

        let mut shards = self.table_shards.write().await;
        shards.insert(shard_id, shard_group);

        Ok(())
    }

    /// Remove a table shard from this node
    ///
    /// Removes a table shard from this node's management. The shard's Raft
    /// group will be dropped and no longer participate in consensus.
    ///
    /// # Arguments
    /// * `shard_id` - Unique identifier of the shard to remove
    ///
    /// # Returns
    /// * `Ok(())` - Shard removed successfully
    /// * `Err(ConsensusError)` - Failed to remove shard
    pub async fn remove_table_shard(&self, shard_id: ShardId) -> ConsensusResult<()> {
        info!(
            "Removing shard {} from node {}",
            shard_id, self.local_node_id
        );

        let mut shards = self.table_shards.write().await;
        shards.remove(&shard_id);

        Ok(())
    }

    /// Get shard for a key using hash-based partitioning
    ///
    /// Determines which shard a key belongs to using consistent hashing.
    /// If only one shard exists, returns shard 0. Otherwise, hashes the key
    /// and uses modulo to determine the shard.
    ///
    /// # Arguments
    /// * `key` - The key to determine shard for
    ///
    /// # Returns
    /// The shard ID that should handle this key
    pub async fn get_table_shard_for_key(&self, key: &[u8]) -> ShardId {
        let shard_count = *self.shard_count.read().await;

        if shard_count == 1 {
            return ShardId::new(0);
        }

        let mut hasher = DefaultHasher::new();
        key.hash(&mut hasher);
        let hash = hasher.finish();

        ShardId::new((hash % shard_count as u64) as u128)
    }

    /// Get shard group by ID
    ///
    /// Internal method to retrieve a table shard's Raft group.
    ///
    /// # Arguments
    /// * `shard_id` - The shard ID to look up
    ///
    /// # Returns
    /// * `Ok(Arc<TableShardRaftGroup>)` - The shard group
    /// * `Err(ConsensusError::ShardNotFound)` - Shard not found on this node
    async fn get_table_shard_group(
        &self,
        shard_id: ShardId,
    ) -> ConsensusResult<Arc<TableShardRaftGroup>> {
        let shards = self.table_shards.read().await;
        shards
            .get(&shard_id)
            .cloned()
            .ok_or_else(|| ConsensusError::ShardNotFound { shard_id })
    }

    /// Put a key-value pair
    ///
    /// Writes a key-value pair to the appropriate shard using Raft consensus.
    /// The operation is routed to the correct shard based on the key's hash.
    ///
    /// # Arguments
    /// * `key` - The key to write
    /// * `value` - The value to write
    ///
    /// # Returns
    /// * `Ok(())` - Write committed successfully
    /// * `Err(ConsensusError)` - Write failed (shard not found, consensus error, etc.)
    pub async fn put(&self, key: Vec<u8>, value: Vec<u8>) -> ConsensusResult<()> {
        let shard_id = self.get_table_shard_for_key(&key).await;
        debug!("Routing PUT to shard {}", shard_id);

        let shard = self.get_table_shard_group(shard_id).await?;
        let operation = Operation::Put { key, value };

        shard.propose_write(operation).await?;
        Ok(())
    }

    /// Get a value by key
    ///
    /// Reads a value from the appropriate shard with linearizable consistency.
    /// This is equivalent to calling `get_with_consistency` with
    /// `ReadConsistency::Linearizable`.
    ///
    /// # Arguments
    /// * `key` - The key to read
    ///
    /// # Returns
    /// * `Ok(Some(value))` - Key found with its value
    /// * `Ok(None)` - Key not found
    /// * `Err(ConsensusError)` - Read failed
    pub async fn get(&self, key: &[u8]) -> ConsensusResult<Option<Vec<u8>>> {
        self.get_with_consistency(key, ReadConsistency::Linearizable)
            .await
    }

    /// Get a value with specified consistency level
    ///
    /// Reads a value from the appropriate shard with the specified consistency
    /// guarantee. Different consistency levels trade off between performance
    /// and freshness guarantees.
    ///
    /// # Arguments
    /// * `key` - The key to read
    /// * `consistency` - The consistency level for the read
    ///
    /// # Returns
    /// * `Ok(Some(value))` - Key found with its value
    /// * `Ok(None)` - Key not found
    /// * `Err(ConsensusError)` - Read failed
    pub async fn get_with_consistency(
        &self,
        key: &[u8],
        consistency: ReadConsistency,
    ) -> ConsensusResult<Option<Vec<u8>>> {
        let shard_id = self.get_table_shard_for_key(key).await;
        debug!("Routing GET to shard {} with {:?}", shard_id, consistency);

        let shard = self.get_table_shard_group(shard_id).await?;
        shard.read(key, consistency).await
    }

    /// Delete a key
    ///
    /// Deletes a key from the appropriate shard using Raft consensus.
    /// The operation is routed to the correct shard based on the key's hash.
    ///
    /// # Arguments
    /// * `key` - The key to delete
    ///
    /// # Returns
    /// * `Ok(())` - Delete committed successfully
    /// * `Err(ConsensusError)` - Delete failed
    pub async fn delete(&self, key: Vec<u8>) -> ConsensusResult<()> {
        let shard_id = self.get_table_shard_for_key(&key).await;
        debug!("Routing DELETE to shard {}", shard_id);

        let shard = self.get_table_shard_group(shard_id).await?;
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

            let shard_id = self.get_table_shard_for_key(key).await;
            shard_ops.entry(shard_id).or_insert_with(Vec::new).push(op);
        }

        // Execute batches per shard
        for (shard_id, ops) in shard_ops {
            debug!("Routing batch of {} ops to shard {}", ops.len(), shard_id);

            let shard = self.get_table_shard_group(shard_id).await?;
            let batch_op = Operation::Batch { operations: ops };

            shard.propose_write(batch_op).await?;
        }

        Ok(())
    }

    /// Get the system metadata Raft group
    ///
    /// Returns the Raft group responsible for system-level metadata.
    ///
    /// # Returns
    /// Arc reference to the system metadata Raft group
    pub fn system_metadata(&self) -> Arc<SystemShardRaftGroup> {
        self.system_metadata.clone()
    }
    /// Get the container metadata Raft group
    ///
    /// Returns the Raft group responsible for a specific container's metadata.
    ///
    /// # Arguments
    /// * `container_id` - The container ID to look up
    ///
    /// # Returns
    /// * `Ok(Arc<ContainerShardRaftGroup>)` - The container metadata group
    /// * `Err(ConsensusError::ShardNotFound)` - Container not found
    pub async fn container_metadata(
        &self,
        container_id: ContainerId,
    ) -> ConsensusResult<Arc<ContainerShardRaftGroup>> {
        let lock = self.container_metadata.read().await;
        lock.get(&container_id)
            .cloned()
            .ok_or_else(|| ConsensusError::ShardNotFound {
                shard_id: ShardId::new(0),
            })
    }

    /// Get all local table shards
    ///
    /// Returns a list of all table shard IDs currently managed by this node.
    ///
    /// # Returns
    /// Vector of shard IDs
    pub async fn local_shards(&self) -> Vec<ShardId> {
        let shards = self.table_shards.read().await;
        shards.keys().copied().collect()
    }

    /// Get a table shard group for advanced operations
    ///
    /// Returns the Raft group for a specific table shard. This provides
    /// direct access to the shard's Raft operations for advanced use cases.
    ///
    /// # Arguments
    /// * `shard_id` - The shard ID to look up
    ///
    /// # Returns
    /// * `Ok(Arc<TableShardRaftGroup>)` - The shard group
    /// * `Err(ConsensusError::ShardNotFound)` - Shard not found on this node
    pub async fn shard_group(
        &self,
        shard_id: ShardId,
    ) -> ConsensusResult<Arc<TableShardRaftGroup>> {
        self.get_table_shard_group(shard_id).await
    }
}

impl std::fmt::Debug for ConsensusManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ConsensusRouter")
            .field("node_id", &self.local_node_id)
            // Todo: Add More pertinent fields
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_router_creation() {
        let router = ConsensusManager::new(NodeId::new(1), ReplicationConfig::default());
        assert_eq!(router.local_node_id, NodeId::new(1));
    }

    #[tokio::test]
    async fn test_shard_routing() {
        let router = ConsensusManager::new(NodeId::new(1), ReplicationConfig::default());
        router.set_shard_count(4).await;

        let key1 = b"test_key_1";
        let key2 = b"test_key_2";

        let shard1 = router.get_table_shard_for_key(key1).await;
        let shard2 = router.get_table_shard_for_key(key2).await;

        // Same key should always route to same shard
        assert_eq!(shard1, router.get_table_shard_for_key(key1).await);
        assert_eq!(shard2, router.get_table_shard_for_key(key2).await);
    }
}
