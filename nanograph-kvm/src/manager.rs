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

use crate::config::KeyValueDatabaseConfig;
use crate::context::KeyValueDatabaseContext;
use crate::handle::{ContainerHandle, SystemHandle, TableHandle, TenantHandle};
use nanograph_core::object::{
    ClusterId, ClusterMetadata, DatabaseCreate, DatabaseId, DatabaseMetadata, DatabaseUpdate,
    IndexCreate, IndexUpdate, NamespaceCreate, NamespaceId, NamespaceRecord, NamespaceUpdate,
    NodeId, RegionId, RegionMetadata, SecurityPrincipal, ServerMetadata, SystemUserCreate,
    SystemUserMetadata, SystemUserUpdate, TableCreate, TableRecord, TableUpdate, TablespaceCreate,
    TablespaceId, TablespaceRecord, TablespaceUpdate, TenantCreate, TenantId, TenantMetadata,
    TenantUpdate, TenantUserCreate, TenantUserMetadata, TenantUserUpdate, UserId,
};
use nanograph_core::object::{ContainerId, IndexId, TableId};
use nanograph_kvt::KeyValueResult;
use nanograph_raft::{
    ClusterCreate, ClusterUpdate, ConsensusManager, RegionCreate, RegionUpdate, ServerCreate,
    ServerUpdate,
};
use std::sync::Arc;

/// The main entry point for managing a key-value database system.
///
/// `KeyValueDatabaseManager` provides high-level operations for managing the entire
/// database system, including clusters, tenants, databases, tablespaces, and users.
///
/// # Operating Modes
///
/// The manager can operate in two modes:
///
/// - **Single-node mode**: All data is stored locally with direct shard access
/// - **Distributed mode**: Data is distributed across multiple nodes with Raft consensus
///
/// # Hierarchy
///
/// The database system is organized hierarchically:
///
/// ```text
/// Cluster
///  ├─ Regions (geographical/logical groupings)
///  │   └─ Servers (nodes in the cluster)
///  ├─ Tenants (isolated customer environments)
///  │   └─ Databases (containers for tables)
///  │       ├─ Namespaces (logical organization)
///  │       └─ Tables (key-value storage)
///  ├─ Tablespaces (storage locations)
///  └─ Users (authentication and authorization)
/// ```
///
/// # Usage
///
/// ## Single-Node Setup
///
/// ```ignore
/// use nanograph_kvm::{KeyValueDatabaseManager, KeyValueDatabaseConfig};
///
/// let config = KeyValueDatabaseConfig::default();
/// let manager = KeyValueDatabaseManager::new_standalone(config);
/// ```
///
/// ## Distributed Setup
///
/// ```ignore
/// let config = KeyValueDatabaseConfig::default();
/// let raft_router = Arc::new(ConsensusRouter::new(/* ... */));
/// let manager = KeyValueDatabaseManager::new_distributed(config, raft_router);
/// ```
///
/// ## Working with Data
///
/// ```ignore
/// // Create a tenant
/// let tenant = manager.create_tenant(TenantCreate {
///     name: "acme-corp".to_string(),
///     options: HashMap::new(),
///     metadata: HashMap::new(),
/// }).await?;
///
/// // Create a database
/// let database = manager.create_database(&tenant.id, DatabaseCreate {
///     name: "production".to_string(),
///     options: HashMap::new(),
///     metadata: HashMap::new(),
/// }).await?;
///
/// // Get a container handle
/// let container_id = ContainerId::new(tenant.id, database.id);
/// let container = manager.get_container(&container_id).await?;
///
/// // Create and use a table
/// let table_id = container.create_table(TableCreate { /* ... */ }).await?;
/// let table = container.get_table_handle(&table_id).await?;
/// table.put(b"key", b"value").await?;
/// ```
///
/// # Thread Safety
///
/// `KeyValueDatabaseManager` is safe to clone and share across threads. All operations
/// are internally synchronized.
///
/// # TODO
///
/// - Handle Table and Shard Allocation more intelligently
pub struct KeyValueDatabaseManager {
    context: Arc<KeyValueDatabaseContext>,
}

impl KeyValueDatabaseManager {
    /// Create a new database manager in single-node mode.
    ///
    /// In single-node mode, all data is stored locally without distributed consensus.
    /// This is suitable for development, testing, or single-server deployments.
    ///
    /// # Arguments
    ///
    /// * `config` - Configuration for the database system
    ///
    /// # Returns
    ///
    /// A new `KeyValueDatabaseManager` configured for standalone operation
    ///
    /// # Example
    ///
    /// ```ignore
    /// let config = KeyValueDatabaseConfig::default();
    /// let manager = KeyValueDatabaseManager::new_standalone(config);
    /// ```
    pub async fn new_standalone(config: KeyValueDatabaseConfig) -> KeyValueResult<Self> {
        let context = KeyValueDatabaseContext::new_standalone(config);
        context.bootstrap_standalone().await?;
        Ok(KeyValueDatabaseManager {
            context: Arc::new(context),
        })
    }

    /// Create a new database manager in distributed mode.
    ///
    /// In distributed mode, data is replicated across multiple nodes using Raft consensus.
    /// This provides high availability and fault tolerance.
    ///
    /// # Arguments
    ///
    /// * `config` - Configuration for the database system
    /// * `raft_router` - The Raft consensus router for coordinating distributed operations
    ///
    /// # Returns
    ///
    /// A new `KeyValueDatabaseManager` configured for distributed operation
    ///
    /// # Example
    ///
    /// ```ignore
    /// let config = KeyValueDatabaseConfig::default();
    /// let raft_router = Arc::new(ConsensusRouter::new(/* ... */));
    /// let manager = KeyValueDatabaseManager::new_distributed(config, raft_router);
    /// ```
    pub fn new_distributed(
        config: KeyValueDatabaseConfig,
        raft_router: Arc<ConsensusManager>,
    ) -> Self {
        Self {
            context: Arc::new(KeyValueDatabaseContext::new_distributed(
                config,
                raft_router,
            )),
        }
    }

    /// Check if the manager is running in distributed mode.
    ///
    /// # Returns
    ///
    /// * `true` - Running in distributed mode with Raft consensus
    /// * `false` - Running in standalone single-node mode
    pub fn is_distributed(&self) -> bool {
        self.context.is_distributed()
    }

    /// Get the local node ID.
    ///
    /// # Returns
    ///
    /// * `Some(NodeId)` - The ID of this node in distributed mode
    /// * `None` - Not applicable in standalone mode
    pub fn node_id(&self) -> Option<NodeId> {
        Some(self.context.node_id())
    }

    /// Get the cluster ID.
    ///
    /// # Returns
    ///
    /// The ID of the cluster this manager belongs to
    pub fn cluster_id(&self) -> ClusterId {
        self.context.cluster_id()
    }

    /// Get the Raft consensus manager.
    ///
    /// # Returns
    ///
    /// * `Some(Arc<ConsensusManager>)` - The consensus manager in distributed mode
    /// * `None` - Not applicable in standalone mode
    pub fn consensus_router(&self) -> Option<Arc<ConsensusManager>> {
        self.context.consensus_router()
    }

    /**********************************************************************************************\
     * Cluster Management                                                                         *
    \**********************************************************************************************/

    /// Initialize a new cluster with the given configuration.
    ///
    /// # What it does
    /// Creates and persists cluster metadata for a new cluster. This should be called once
    /// when setting up a new cluster.
    ///
    /// # How it works
    /// 1. Creates [`ClusterRecord`] with the current timestamp and version 1
    /// 2. Stores metadata in system_metacache for fast access
    /// 3. Serializes and persists metadata to system shard (ShardId 0)
    /// 4. Uses [`SystemKeys::cluster_key`] for the storage key
    ///
    /// # Access Control
    /// - Requires [`Permission::ClusterAlter`] on [`ResourceScope::System`]
    ///
    /// # Parameters
    /// - `principal`: [`SecurityPrincipal`] for authorization
    /// - `config`: [`ClusterCreate`] containing name, options, and metadata
    ///
    /// # Returns
    /// - `Ok(())` if cluster initialization succeeds
    /// - `Err(KeyValueError)` if lock poisoned or persistence fails
    pub async fn initialize_cluster(
        &self,
        principal: &SecurityPrincipal,
        config: ClusterCreate,
    ) -> KeyValueResult<()> {
        self.context.initialize_cluster(principal, config).await?;
        Ok(())
    }

    /// Get the cluster metadata.
    ///
    /// # What it does
    /// Retrieves the current cluster metadata from the system.
    ///
    /// # How it works
    /// Reads from system_metacache or persists to system shard if not found.
    ///
    /// # Access Control
    /// - Requires [`Permission::ClusterView`] on [`ResourceScope::System`]
    ///
    /// # Parameters
    /// - `principal`: [`SecurityPrincipal`] for authorization
    ///
    /// # Returns
    /// The metadata for this cluster including name, version, and configuration
    pub async fn get_cluster(
        &self,
        principal: &SecurityPrincipal,
    ) -> KeyValueResult<ClusterMetadata> {
        self.context.get_cluster(principal).await
    }

    /// Update the cluster metadata.
    ///
    /// # What it does
    /// Updates cluster metadata with new values from the configuration.
    ///
    /// # How it works
    /// 1. Updates metadata in system_metacache
    /// 2. Serializes and persists updated metadata to system shard (ShardId 0)
    /// 3. Increments version and updates last_modified timestamp
    ///
    /// # Access Control
    /// - Requires [`Permission::ClusterAlter`] on [`ResourceScope::System`]
    ///
    /// # Parameters
    /// - `principal`: [`SecurityPrincipal`] for authorization
    /// - `cluster`: [`ClusterUpdate`] containing metadata updates
    ///
    /// # Returns
    /// - `Ok(())` if update successful
    /// - `Err(KeyValueError)` if update failed
    pub async fn update_cluster(
        &self,
        principal: &SecurityPrincipal,
        cluster: ClusterUpdate,
    ) -> KeyValueResult<()> {
        self.context.update_cluster(principal, cluster).await?;
        Ok(())
    }

    /// Get metadata about all regions in the cluster.
    ///
    /// # What it does
    /// Returns an iterator over all region metadata records.
    ///
    /// # How it works
    /// Reads from system_metacache and returns all cached region records.
    ///
    /// # Access Control
    /// - Requires [`Permission::ClusterView`] on [`ResourceScope::System`]
    ///
    /// # Parameters
    /// - `principal`: [`SecurityPrincipal`] for authorization
    ///
    /// # Returns
    /// An iterator over [`RegionMetadata`] for all regions
    pub async fn get_regions(
        &self,
        principal: &SecurityPrincipal,
    ) -> KeyValueResult<impl IntoIterator<Item = RegionMetadata>> {
        self.context.get_regions(principal).await
    }

    /// Get metadata about a specific region.
    ///
    /// # What it does
    /// Retrieves metadata for a single region by its ID.
    ///
    /// # How it works
    /// Reads from system_metacache to find the region record.
    ///
    /// # Access Control
    /// - Requires [`Permission::RegionView`] on [`ResourceScope::System`]
    ///
    /// # Parameters
    /// - `principal`: [`SecurityPrincipal`] for authorization
    /// - `region`: [`RegionId`] to look up
    ///
    /// # Returns
    /// - `Some(RegionMetadata)` if region exists
    /// - `None` if region not found
    pub async fn get_region(
        &self,
        principal: &SecurityPrincipal,
        region: RegionId,
    ) -> KeyValueResult<Option<RegionMetadata>> {
        self.context.get_region(principal, region).await
    }

    /// Add a new region to the cluster.
    ///
    /// # What it does
    /// Creates a new region with the provided configuration.
    ///
    /// # How it works
    /// 1. Generates a new [`RegionId`]
    /// 2. Creates [`RegionRecord`] with current timestamp and version 1
    /// 3. Stores in system_metacache
    /// 4. Serializes and persists to system shard using SystemKeys::region_key
    ///
    /// # Access Control
    /// - Requires [`Permission::ClusterAlter`] on [`ResourceScope::System`]
    ///
    /// # Parameters
    /// - `principal`: [`SecurityPrincipal`] for authorization
    /// - `config`: [`RegionCreate`] containing name, cluster, options, and metadata
    ///
    /// # Returns
    /// - `Ok(RegionMetadata)` with the created region information
    pub async fn add_region(
        &self,
        principal: &SecurityPrincipal,
        config: RegionCreate,
    ) -> KeyValueResult<RegionMetadata> {
        self.context.add_region(principal, config).await
    }

    /// Update a region's metadata.
    ///
    /// # What it does
    /// Updates region metadata with new values from the configuration.
    ///
    /// # Access Control
    /// - Requires [`Permission::ClusterAlter`] on [`ResourceScope::System`]
    ///
    /// # Parameters
    /// - `principal`: [`SecurityPrincipal`] for authorization
    /// - `region`: [`RegionId`] of the region to update
    /// - `config`: [`RegionUpdate`] containing updates
    ///
    /// # Returns
    /// The updated [`RegionMetadata`]
    pub async fn update_region(
        &self,
        principal: &SecurityPrincipal,
        region: &RegionId,
        config: RegionUpdate,
    ) -> KeyValueResult<RegionMetadata> {
        self.context.update_region(principal, region, config).await
    }

    /// Remove a region from the cluster.
    ///
    /// # What it does
    /// Deletes a region record from the system.
    ///
    /// # Access Control
    /// - Requires [`Permission::ClusterAlter`] on [`ResourceScope::System`]
    ///
    /// # Parameters
    /// - `principal`: [`SecurityPrincipal`] for authorization
    /// - `region`: [`RegionId`] of the region to remove
    ///
    /// # Returns
    /// `Ok(())` if successful
    pub async fn remove_region(
        &self,
        principal: &SecurityPrincipal,
        region: &RegionId,
    ) -> KeyValueResult<()> {
        self.context.remove_region(principal, region).await
    }

    /// Get metadata about all servers in the cluster.
    ///
    /// # What it does
    /// Returns an iterator over all server metadata records.
    ///
    /// # How it works
    /// Reads from system_metacache and returns all cached server records.
    ///
    /// # Access Control
    /// - Requires [`Permission::ClusterView`] on [`ResourceScope::System`]
    ///
    /// # Parameters
    /// - `principal`: [`SecurityPrincipal`] for authorization
    ///
    /// # Returns
    /// An iterator over [`ServerMetadata`] for all servers
    pub async fn get_servers(
        &self,
        principal: &SecurityPrincipal,
    ) -> KeyValueResult<impl IntoIterator<Item = ServerMetadata>> {
        self.context.get_servers(principal).await
    }

    /// Get metadata about all servers in a specific region.
    ///
    /// # What it does
    /// Returns an iterator over server metadata records for servers belonging to the given region.
    ///
    /// # How it works
    /// Reads from system_metacache, filters cached server records by region ID.
    ///
    /// # Access Control
    /// - Requires [`Permission::ClusterView`] on [`ResourceScope::System`]
    ///
    /// # Parameters
    /// - `principal`: [`SecurityPrincipal`] for authorization
    /// - `region`: [`RegionId`] to filter servers by
    ///
    /// # Returns
    /// An iterator over [`NodeId`] for matching servers
    pub async fn get_servers_by_region(
        &self,
        principal: &SecurityPrincipal,
        region: &RegionId,
    ) -> KeyValueResult<impl IntoIterator<Item = NodeId>> {
        self.context.get_servers_by_region(principal, region).await
    }

    /// Get metadata about a specific server.
    ///
    /// # What it does
    /// Retrieves metadata for a single server by its node identifier.
    ///
    /// # How it works
    /// 1. Checks system_metacache for the server record
    /// 2. If not found, reads from system shard (ShardId 0)
    ///
    /// # Access Control
    /// - Requires [`Permission::ClusterView`] on [`ResourceScope::System`]
    ///
    /// # Parameters
    /// - `principal`: [`SecurityPrincipal`] for authorization
    /// - `server`: [`NodeId`] of the server to look up
    ///
    /// # Returns
    /// - `Some(ServerMetadata)` if server exists
    /// - `None` if server not found
    pub async fn get_server(
        &self,
        principal: &SecurityPrincipal,
        server: &NodeId,
    ) -> KeyValueResult<Option<ServerMetadata>> {
        self.context.get_server(principal, server).await
    }

    /// Add a new server to the cluster.
    ///
    /// # What it does
    /// Registers a new server node in the cluster metadata.
    ///
    /// # How it works
    /// 1. Creates a [`ServerRecord`] from the provided config
    /// 2. Stores in system_metacache
    /// 3. Persists to system shard
    ///
    /// # Access Control
    /// - Requires [`Permission::ClusterAlter`] on [`ResourceScope::System`]
    ///
    /// # Parameters
    /// - `principal`: [`SecurityPrincipal`] for authorization
    /// - `config`: [`ServerCreate`] containing server details
    ///
    /// # Returns
    /// The created [`ServerMetadata`]
    pub async fn add_server(
        &self,
        principal: &SecurityPrincipal,
        config: ServerCreate,
    ) -> KeyValueResult<ServerMetadata> {
        self.context.add_server(principal, config).await
    }

    /// Update a server's metadata.
    ///
    /// # What it does
    /// Updates server details such as endpoint or status.
    ///
    /// # Access Control
    /// - Requires [`Permission::ClusterAlter`] on [`ResourceScope::System`]
    ///
    /// # Parameters
    /// - `principal`: [`SecurityPrincipal`] for authorization
    /// - `server`: [`NodeId`] of the server to update
    /// - `config`: [`ServerUpdate`] containing updates
    ///
    /// # Returns
    /// The updated [`ServerMetadata`]
    pub async fn update_server(
        &self,
        principal: &SecurityPrincipal,
        server: &NodeId,
        config: ServerUpdate,
    ) -> KeyValueResult<ServerMetadata> {
        self.context.update_server(principal, server, config).await
    }

    /// Remove a server from the cluster.
    ///
    /// # What it does
    /// Deletes a server record from the cluster metadata.
    ///
    /// # Access Control
    /// - Requires [`Permission::ClusterAlter`] on [`ResourceScope::System`]
    ///
    /// # Parameters
    /// - `principal`: [`SecurityPrincipal`] for authorization
    /// - `server`: [`NodeId`] of the server to remove
    ///
    /// # Returns
    /// `Ok(())` if successful
    pub async fn remove_server(
        &self,
        principal: &SecurityPrincipal,
        server: &NodeId,
    ) -> KeyValueResult<()> {
        self.context.remove_server(principal, server).await
    }

    /**********************************************************************************************\
     * User Management                                                                            *
    \**********************************************************************************************/

    /// Get metadata about all users.
    ///
    /// # What it does
    /// Returns an iterator over all user metadata records.
    ///
    /// # How it works
    /// Reads from system_metacache and returns all cached user records.
    ///
    /// # Access Control
    /// - Requires [`Permission::UserManagement`] on [`ResourceScope::System`]
    ///
    /// # Parameters
    /// - `principal`: [`SecurityPrincipal`] for authorization
    ///
    /// # Returns
    /// An iterator over [`SystemUserMetadata`] for all users
    pub async fn get_system_users(
        &self,
        principal: &SecurityPrincipal,
    ) -> KeyValueResult<impl IntoIterator<Item = SystemUserMetadata>> {
        self.context.get_system_users(principal).await
    }

    /// Get metadata for a specific user.
    ///
    /// # What it does
    /// Retrieves metadata for a single user by ID, checking cache first then disk.
    ///
    /// # How it works
    /// 1. Checks system_metacache for cached user record
    /// 2. If not in cache, reads from system shard using SystemKeys::user_key
    ///
    /// # Access Control
    /// - Requires [`Permission::UserManagement`] on [`ResourceScope::System`]
    ///
    /// # Parameters
    /// - `principal`: [`SecurityPrincipal`] for authorization
    /// - `user_id`: [`UserId`] to look up
    ///
    /// # Returns
    /// - `Some(SystemUserMetadata)` if user exists
    /// - `None` if user not found
    pub async fn get_system_user(
        &self,
        principal: &SecurityPrincipal,
        user_id: &UserId,
    ) -> KeyValueResult<Option<SystemUserMetadata>> {
        self.context.get_system_user(principal, user_id).await
    }

    /// Get user ID by username.
    ///
    /// # What it does
    /// Finds a user ID associated with the given username.
    ///
    /// # Access Control
    /// - Requires [`Permission::UserManagement`] on [`ResourceScope::System`]
    ///
    /// # Parameters
    /// - `principal`: [`SecurityPrincipal`] for authorization
    /// - `username`: Username to look up
    ///
    /// # Returns
    /// - `Some(UserId)` if user exists
    /// - `None` if user not found
    pub async fn get_user_by_username(
        &self,
        principal: &SecurityPrincipal,
        username: &str,
    ) -> KeyValueResult<Option<UserId>> {
        self.context.get_user_by_username(principal, username).await
    }

    /// Create a new system user.
    ///
    /// # What it does
    /// Creates a new system user with the provided configuration.
    ///
    /// # How it works
    /// 1. Generates a new [`UserId`]
    /// 2. Creates [`SystemUserRecord`] with current timestamp and version 1
    /// 3. Stores in system_metacache and persists to system shard
    ///
    /// # Access Control
    /// - Requires [`Permission::UserManagement`] on [`ResourceScope::System`]
    ///
    /// # Parameters
    /// - `principal`: [`SecurityPrincipal`] for authorization
    /// - `config`: [`SystemUserCreate`] containing username and details
    ///
    /// # Returns
    /// The created [`SystemUserMetadata`]
    pub async fn create_system_user(
        &self,
        principal: &SecurityPrincipal,
        config: SystemUserCreate,
    ) -> KeyValueResult<SystemUserMetadata> {
        self.context.create_system_user(principal, config).await
    }

    /// Update an existing system user's metadata.
    ///
    /// # What it does
    /// Updates user metadata such as options or metadata maps.
    ///
    /// # Access Control
    /// - Requires [`Permission::UserManagement`] on [`ResourceScope::System`]
    ///
    /// # Parameters
    /// - `principal`: [`SecurityPrincipal`] for authorization
    /// - `user_id`: [`UserId`] of the user to update
    /// - `config`: [`SystemUserUpdate`] containing updates
    ///
    /// # Returns
    /// The updated [`SystemUserMetadata`]
    pub async fn update_system_user(
        &self,
        principal: &SecurityPrincipal,
        user_id: &UserId,
        config: SystemUserUpdate,
    ) -> KeyValueResult<SystemUserMetadata> {
        self.context
            .update_system_user(principal, user_id, config)
            .await
    }

    /// Remove a system user from the system.
    ///
    /// # What it does
    /// Deletes a system user record from the system.
    ///
    /// # Access Control
    /// - Requires [`Permission::UserManagement`] on [`ResourceScope::System`]
    ///
    /// # Parameters
    /// - `principal`: [`SecurityPrincipal`] for authorization
    /// - `user_id`: [`UserId`] of the user to remove
    ///
    /// # Returns
    /// `Ok(())` if successful
    pub async fn remove_system_user(
        &self,
        principal: &SecurityPrincipal,
        user_id: &UserId,
    ) -> KeyValueResult<()> {
        self.context.remove_system_user(principal, user_id).await
    }

    /// Get metadata about all users for a specific tenant.
    ///
    /// # Access Control
    /// - Requires [`Permission::UserManagement`] on [`ResourceScope::Tenant`]
    ///
    /// # Parameters
    /// - `principal`: [`SecurityPrincipal`] for authorization
    /// - `tenant_id`: [`TenantId`] to get users for
    ///
    /// # Returns
    /// An iterator over [`TenantUserMetadata`] for all matching users
    pub async fn get_tenant_users(
        &self,
        principal: &SecurityPrincipal,
        tenant_id: &TenantId,
    ) -> KeyValueResult<impl IntoIterator<Item = TenantUserMetadata>> {
        self.context.get_tenant_users(principal, tenant_id).await
    }

    /// Get metadata for a specific tenant user.
    ///
    /// # Access Control
    /// - Requires [`Permission::UserManagement`] on [`ResourceScope::Tenant`]
    ///
    /// # Parameters
    /// - `principal`: [`SecurityPrincipal`] for authorization
    /// - `tenant_id`: [`TenantId`] that owns the user
    /// - `user_id`: [`UserId`] to look up
    ///
    /// # Returns
    /// - `Some(TenantUserMetadata)` if user exists
    /// - `None` if user not found
    pub async fn get_tenant_user(
        &self,
        principal: &SecurityPrincipal,
        tenant_id: &TenantId,
        user_id: &UserId,
    ) -> KeyValueResult<Option<TenantUserMetadata>> {
        self.context
            .get_tenant_user(principal, tenant_id, user_id)
            .await
    }

    /// Create a new tenant user.
    ///
    /// # Access Control
    /// - Requires [`Permission::UserManagement`] on [`ResourceScope::Tenant`]
    ///
    /// # Parameters
    /// - `principal`: [`SecurityPrincipal`] for authorization
    /// - `tenant_id`: [`TenantId`] that will own the user
    /// - `config`: [`TenantUserCreate`] containing username and details
    ///
    /// # Returns
    /// The created [`TenantUserMetadata`]
    pub async fn create_tenant_user(
        &self,
        principal: &SecurityPrincipal,
        tenant_id: &TenantId,
        config: TenantUserCreate,
    ) -> KeyValueResult<TenantUserMetadata> {
        self.context
            .create_tenant_user(principal, tenant_id, config)
            .await
    }

    /// Update an existing tenant user's metadata.
    ///
    /// # Access Control
    /// - Requires [`Permission::UserManagement`] on [`ResourceScope::Tenant`]
    ///
    /// # Parameters
    /// - `principal`: [`SecurityPrincipal`] for authorization
    /// - `tenant_id`: [`TenantId`] that owns the user
    /// - `user_id`: [`UserId`] of the user to update
    /// - `config`: [`TenantUserUpdate`] containing updates
    ///
    /// # Returns
    /// The updated [`TenantUserMetadata`]
    pub async fn update_tenant_user(
        &self,
        principal: &SecurityPrincipal,
        tenant_id: &TenantId,
        user_id: &UserId,
        config: TenantUserUpdate,
    ) -> KeyValueResult<TenantUserMetadata> {
        self.context
            .update_tenant_user(principal, tenant_id, user_id, config)
            .await
    }

    /// Remove a tenant user.
    ///
    /// # Access Control
    /// - Requires [`Permission::UserManagement`] on [`ResourceScope::Tenant`]
    ///
    /// # Parameters
    /// - `principal`: [`SecurityPrincipal`] for authorization
    /// - `tenant_id`: [`TenantId`] that owns the user
    /// - `user_id`: [`UserId`] of the user to remove
    ///
    /// # Returns
    /// `Ok(())` if successful
    pub async fn remove_tenant_user(
        &self,
        principal: &SecurityPrincipal,
        tenant_id: &TenantId,
        user_id: &UserId,
    ) -> KeyValueResult<()> {
        self.context
            .remove_tenant_user(principal, tenant_id, user_id)
            .await
    }

    /**********************************************************************************************\
     * Handles                                                                                    *
    \**********************************************************************************************/

    /// Get a handle for working with system-level metadata.
    ///
    /// The system handle provides access to cluster, region, server, and tenant management.
    ///
    /// # Parameters
    /// - `principal`: [`SecurityPrincipal`] for authorization
    ///
    /// # Returns
    /// A [`SystemHandle`] for system-level operations
    pub async fn get_system_handle(
        &self,
        principal: &SecurityPrincipal,
    ) -> KeyValueResult<SystemHandle> {
        Ok(SystemHandle::new(self.context.clone(), principal.clone()))
    }

    /// Get a handle for working with a specific tenant.
    ///
    /// The tenant handle provides access to database and user management for that tenant.
    ///
    /// # Parameters
    /// - `principal`: [`SecurityPrincipal`] for authorization
    /// - `tenant_id`: [`TenantId`] of the tenant
    ///
    /// # Returns
    /// A [`TenantHandle`] for tenant-level operations
    pub async fn get_tenant_handle(
        &self,
        principal: &SecurityPrincipal,
        tenant_id: &TenantId,
    ) -> KeyValueResult<TenantHandle> {
        // Authenticate

        Ok(TenantHandle::new(
            self.context.clone(),
            principal.clone(),
            *tenant_id,
        ))
    }

    /// Get a handle for working with a specific database container.
    ///
    /// A container represents a tenant's database and provides access to namespaces,
    /// tables, and data operations.
    ///
    /// # Arguments
    ///
    /// * `principal` - Security principal for authorization
    /// * `container_id` - The container ID (combines tenant and database IDs)
    ///
    /// # Returns
    ///
    /// A `ContainerHandle` for the specified container
    ///
    /// # Example
    ///
    /// ```ignore
    /// let container_id = ContainerId::new(tenant_id, database_id);
    /// let container = manager.get_container(principal, &container_id).await?;
    /// ```
    pub async fn get_container_handle(
        &self,
        principal: &SecurityPrincipal,
        container_id: &ContainerId,
    ) -> KeyValueResult<ContainerHandle> {
        Ok(ContainerHandle::new(
            self.context.clone(),
            principal.clone(),
            *container_id,
        ))
    }

    /// Get a handle for working with a specific table.
    ///
    /// The table handle provides access to direct table operations like get, put, and delete.
    ///
    /// # Parameters
    /// - `principal`: [`SecurityPrincipal`] for authorization
    /// - `container_id`: [`ContainerId`] of the database container
    /// - `table_id`: [`ObjectId`] of the table
    ///
    /// # Returns
    /// A [`TableHandle`] for table operations
    pub async fn get_table_handle(
        &self,
        principal: &SecurityPrincipal,
        container_id: &ContainerId,
        table_id: &TableId,
    ) -> KeyValueResult<TableHandle> {
        Ok(TableHandle::new(
            self.context.clone(),
            principal.clone(),
            *container_id,
            *table_id,
        ))
    }

    /**********************************************************************************************\
     * Container Management                                                                       *
    \**********************************************************************************************/

    /// Get metadata about all tenants.
    ///
    /// # What it does
    /// Returns an iterator over all tenant metadata records.
    ///
    /// # How it works
    /// Reads from system_metacache and returns all cached tenant records.
    ///
    /// # Access Control
    /// - Requires [`Permission::TenantList`] on [`ResourceScope::System`]
    ///
    /// # Parameters
    /// - `principal`: [`SecurityPrincipal`] for authorization
    ///
    /// # Returns
    /// An iterator over ([`TenantId`], name) for all tenants
    pub async fn get_tenants(
        &self,
        principal: &SecurityPrincipal,
    ) -> KeyValueResult<impl IntoIterator<Item = (TenantId, String)>> {
        self.context.get_tenants(principal).await
    }

    /// Get metadata for a specific tenant.
    ///
    /// # What it does
    /// Retrieves metadata for a single tenant by ID, checking cache first then disk.
    ///
    /// # How it works
    /// 1. Checks system_metacache for cached tenant record
    /// 2. If not in cache, reads from system shard using [`SystemKeys::tenant_key`]
    ///
    /// # Access Control
    /// - Requires [`Permission::TenantView`] on [`ResourceScope::System`]
    ///
    /// # Parameters
    /// - `principal`: [`SecurityPrincipal`] for authorization
    /// - `tenant`: [`TenantId`] to look up
    ///
    /// # Returns
    /// - `Some(TenantMetadata)` if tenant exists
    /// - `None` if tenant not found
    pub async fn get_tenant(
        &self,
        principal: &SecurityPrincipal,
        tenant: &TenantId,
    ) -> KeyValueResult<Option<TenantMetadata>> {
        self.context.get_tenant(principal, tenant).await
    }

    /// Create a new tenant.
    ///
    /// # What it does
    /// Creates a new tenant with the provided configuration.
    ///
    /// # How it works
    /// 1. Generates a new [`TenantId`]
    /// 2. Creates [`TenantRecord`] with current timestamp and version 1
    /// 3. Stores in system_metacache and persists to system shard
    ///
    /// # Access Control
    /// - Requires [`Permission::TenantCreate`] on [`ResourceScope::System`]
    ///
    /// # Parameters
    /// - `principal`: [`SecurityPrincipal`] for authorization
    /// - `config`: [`TenantCreate`] containing name, options, and metadata
    ///
    /// # Returns
    /// The created [`TenantMetadata`]
    pub async fn create_tenant(
        &self,
        principal: &SecurityPrincipal,
        config: TenantCreate,
    ) -> KeyValueResult<TenantMetadata> {
        self.context.create_tenant(principal, config).await
    }

    /// Update an existing tenant's metadata.
    ///
    /// # Access Control
    /// - Requires [`Permission::TenantAlter`] on [`ResourceScope::System`]
    ///
    /// # Parameters
    /// - `principal`: [`SecurityPrincipal`] for authorization
    /// - `tenant`: [`TenantId`] of the tenant to update
    /// - `config`: [`TenantUpdate`] containing updates
    ///
    /// # Returns
    /// The updated [`TenantMetadata`]
    pub async fn update_tenant(
        &self,
        principal: &SecurityPrincipal,
        tenant: &TenantId,
        config: TenantUpdate,
    ) -> KeyValueResult<TenantMetadata> {
        self.context.update_tenant(principal, tenant, config).await
    }

    /// Delete a tenant.
    ///
    /// # Access Control
    /// - Requires [`Permission::TenantDelete`] on [`ResourceScope::System`]
    ///
    /// # Parameters
    /// - `principal`: [`SecurityPrincipal`] for authorization
    /// - `tenant`: [`TenantId`] of the tenant to delete
    ///
    /// # Returns
    /// `Ok(())` if successful
    pub async fn delete_tenant(
        &self,
        principal: &SecurityPrincipal,
        tenant: &TenantId,
    ) -> KeyValueResult<()> {
        self.context.delete_tenant(principal, tenant).await
    }

    /// Get metadata about all databases for a specific tenant.
    ///
    /// # What it does
    /// Returns an iterator over all database metadata records belonging to the given tenant.
    ///
    /// # How it works
    /// Reads from system_metacache, filters cached database records by tenant ID.
    ///
    /// # Access Control
    /// - Requires [`Permission::DatabaseList`] on [`ResourceScope::Tenant`]
    ///
    /// # Parameters
    /// - `principal`: [`SecurityPrincipal`] for authorization
    /// - `tenant`: [`TenantId`] to filter databases by
    ///
    /// # Returns
    /// An iterator over ([`DatabaseId`], name) for all matching databases
    pub async fn get_databases(
        &self,
        principal: &SecurityPrincipal,
        tenant: &TenantId,
    ) -> KeyValueResult<impl IntoIterator<Item = (DatabaseId, String)>> {
        self.context.get_databases(principal, tenant).await
    }

    /// Get metadata for a specific database.
    ///
    /// # What it does
    /// Retrieves metadata for a single database by ID, checking cache first then disk.
    ///
    /// # How it works
    /// 1. Checks system_metacache for cached database record
    /// 2. If not in cache, reads from system shard using [`SystemKeys::database_key`]
    ///
    /// # Access Control
    /// - Requires [`Permission::DatabaseView`] on [`ResourceScope::Tenant`]
    ///
    /// # Parameters
    /// - `principal`: [`SecurityPrincipal`] for authorization
    /// - `tenant`: [`TenantId`] that owns the database
    /// - `database`: [`DatabaseId`] to look up
    ///
    /// # Returns
    /// - `Some(DatabaseMetadata)` if database exists
    /// - `None` if database not found
    pub async fn get_database(
        &self,
        principal: &SecurityPrincipal,
        tenant: &TenantId,
        database: &DatabaseId,
    ) -> KeyValueResult<Option<DatabaseMetadata>> {
        self.context.get_database(principal, tenant, database).await
    }

    /// Create a new database for a tenant.
    ///
    /// # What it does
    /// Creates a new database for a tenant with the provided configuration.
    ///
    /// # How it works
    /// 1. Generates a new [`DatabaseId`]
    /// 2. Creates [`DatabaseRecord`] with current timestamp and version 1
    /// 3. Stores in system_metacache and persists to system shard
    ///
    /// # Access Control
    /// - Requires [`Permission::DatabaseCreate`] on [`ResourceScope::Tenant`]
    ///
    /// # Parameters
    /// - `principal`: [`SecurityPrincipal`] for authorization
    /// - `tenant`: [`TenantId`] that will own the database
    /// - `config`: [`DatabaseCreate`] containing name, options, and metadata
    ///
    /// # Returns
    /// The created [`DatabaseMetadata`]
    pub async fn create_database(
        &self,
        principal: &SecurityPrincipal,
        tenant: &TenantId,
        config: DatabaseCreate,
    ) -> KeyValueResult<DatabaseMetadata> {
        self.context
            .create_database(principal, tenant, config)
            .await
    }

    /// Update an existing database's metadata.
    ///
    /// # Access Control
    /// - Requires [`Permission::DatabaseAlter`] on [`ResourceScope::Tenant`]
    ///
    /// # Parameters
    /// - `principal`: [`SecurityPrincipal`] for authorization
    /// - `tenant`: [`TenantId`] that owns the database
    /// - `database`: [`DatabaseId`] of the database to update
    /// - `config`: [`DatabaseUpdate`] containing updates
    ///
    /// # Returns
    /// The updated [`DatabaseMetadata`]
    pub async fn update_database(
        &self,
        principal: &SecurityPrincipal,
        tenant: &TenantId,
        database: &DatabaseId,
        config: DatabaseUpdate,
    ) -> KeyValueResult<DatabaseMetadata> {
        self.context
            .update_database(principal, tenant, database, config)
            .await
    }

    /// Delete a database.
    ///
    /// # Access Control
    /// - Requires [`Permission::DatabaseDelete`] on [`ResourceScope::Tenant`]
    ///
    /// # Parameters
    /// - `principal`: [`SecurityPrincipal`] for authorization
    /// - `tenant`: [`TenantId`] that owns the database
    /// - `database`: [`DatabaseId`] of the database to delete
    ///
    /// # Returns
    /// `Ok(())` if successful
    pub async fn delete_database(
        &self,
        principal: &SecurityPrincipal,
        tenant: &TenantId,
        database: &DatabaseId,
    ) -> KeyValueResult<()> {
        self.context
            .delete_database(principal, tenant, database)
            .await
    }

    /**********************************************************************************************\
     * Tablespace Management                                                                      *
    \**********************************************************************************************/

    /// Get metadata about all tablespaces.
    ///
    /// # What it does
    /// Returns an iterator over all tablespace records in the system.
    ///
    /// # How it works
    /// Reads from system_metacache and returns all cached tablespace records.
    ///
    /// # Access Control
    /// - Requires [`Permission::TablespaceList`] on [`ResourceScope::System`]
    ///
    /// # Parameters
    /// - `principal`: [`SecurityPrincipal`] for authorization
    ///
    /// # Returns
    /// An iterator over ([`TablespaceId`], name) for all tablespaces
    pub async fn get_tablespaces(
        &self,
        principal: &SecurityPrincipal,
    ) -> KeyValueResult<impl IntoIterator<Item = (TablespaceId, String)>> {
        self.context.get_tablespaces(principal).await
    }

    /// Get metadata for a specific tablespace.
    ///
    /// # What it does
    /// Retrieves metadata for a single tablespace by ID from cache.
    ///
    /// # How it works
    /// Reads from system_metacache to find the tablespace record.
    ///
    /// # Access Control
    /// - Requires [`Permission::TablespaceView`] on [`ResourceScope::System`]
    ///
    /// # Parameters
    /// - `principal`: [`SecurityPrincipal`] for authorization
    /// - `tablespace`: [`TablespaceId`] to look up
    ///
    /// # Returns
    /// - `Some(TablespaceRecord)` if tablespace exists
    /// - `None` if tablespace not found
    pub async fn get_tablespace(
        &self,
        principal: &SecurityPrincipal,
        tablespace: &TablespaceId,
    ) -> KeyValueResult<Option<TablespaceRecord>> {
        self.context.get_tablespace(principal, tablespace).await
    }

    /// Get tablespace ID by name.
    ///
    /// # What it does
    /// Finds a tablespace by its name string and returns its ID.
    ///
    /// # How it works
    /// Iterates through cached tablespace records to find one matching the name.
    ///
    /// # Access Control
    /// - Requires [`Permission::TablespaceView`] on [`ResourceScope::System`]
    ///
    /// # Parameters
    /// - `principal`: [`SecurityPrincipal`] for authorization
    /// - `name`: Tablespace name to search for
    ///
    /// # Returns
    /// - `Some(TablespaceId)` if tablespace with that name exists
    /// - `None` if no matching tablespace found
    pub async fn get_tablespace_by_name(
        &self,
        principal: &SecurityPrincipal,
        name: &str,
    ) -> KeyValueResult<Option<TablespaceId>> {
        self.context.get_tablespace_by_name(principal, name).await
    }

    /// Create a new tablespace.
    ///
    /// Tablespaces define storage locations for table data, allowing you to
    /// control where data is physically stored.
    ///
    /// # Arguments
    ///
    /// * `principal` - Security principal for authorization
    /// * `config` - Configuration including name, storage path, and tier
    ///
    /// # Returns
    ///
    /// The created tablespace metadata
    pub async fn create_tablespace(
        &self,
        principal: &SecurityPrincipal,
        config: TablespaceCreate,
    ) -> KeyValueResult<TablespaceRecord> {
        self.context.create_tablespace(principal, config).await
    }

    /// Update an existing tablespace's metadata.
    ///
    /// # Access Control
    /// - Requires [`Permission::TablespaceAlter`] on [`ResourceScope::System`]
    ///
    /// # Parameters
    /// - `principal`: [`SecurityPrincipal`] for authorization
    /// - `tablespace`: [`TablespaceId`] of the tablespace to update
    /// - `config`: [`TablespaceUpdate`] containing updates
    ///
    /// # Returns
    /// The updated [`TablespaceRecord`]
    pub async fn update_tablespace(
        &self,
        principal: &SecurityPrincipal,
        tablespace: &TablespaceId,
        config: TablespaceUpdate,
    ) -> KeyValueResult<TablespaceRecord> {
        self.context
            .update_tablespace(principal, tablespace, config)
            .await
    }

    /// Delete a tablespace.
    ///
    /// # Access Control
    /// - Requires [`Permission::TablespaceDelete`] on [`ResourceScope::System`]
    ///
    /// # Parameters
    /// - `principal`: [`SecurityPrincipal`] for authorization
    /// - `tablespace`: [`TablespaceId`] of the tablespace to delete
    ///
    /// # Returns
    /// `Ok(())` if successful
    pub async fn delete_tablespace(
        &self,
        principal: &SecurityPrincipal,
        tablespace: &TablespaceId,
    ) -> KeyValueResult<()> {
        self.context.delete_tablespace(principal, tablespace).await
    }

    /**********************************************************************************************\
     * Namespace Management                                                                       *
    \**********************************************************************************************/

    /// Get all namespaces within a container.
    ///
    /// # Access Control
    /// - Requires [`Permission::NamespaceList`] on [`ResourceScope::Container`]
    ///
    /// # Parameters
    /// - `principal`: [`SecurityPrincipal`] for authorization
    /// - `container_id`: [`ContainerId`] to list namespaces from
    ///
    /// # Returns
    /// An iterator over [`NamespaceRecord`] for all namespaces in the container
    pub async fn get_namespaces(
        &self,
        principal: &SecurityPrincipal,
        container_id: &ContainerId,
    ) -> KeyValueResult<impl IntoIterator<Item = NamespaceRecord>> {
        self.context.get_namespaces(principal, container_id).await
    }

    /// Get namespaces within a container that match a prefix.
    ///
    /// # Access Control
    /// - Requires [`Permission::NamespaceList`] on [`ResourceScope::Container`]
    ///
    /// # Parameters
    /// - `principal`: [`SecurityPrincipal`] for authorization
    /// - `container_id`: [`ContainerId`] to search within
    /// - `prefix`: String prefix to match namespace names against
    ///
    /// # Returns
    /// An iterator over [`NamespaceRecord`] for matching namespaces
    pub async fn get_namespaces_by_prefix(
        &self,
        principal: &SecurityPrincipal,
        container_id: &ContainerId,
        prefix: &str,
    ) -> KeyValueResult<impl IntoIterator<Item = NamespaceRecord>> {
        self.context
            .get_namespaces_by_prefix(principal, container_id, prefix)
            .await
    }

    /// Get metadata for a specific namespace.
    ///
    /// # Access Control
    /// - Requires [`Permission::NamespaceView`] on [`ResourceScope::Container`]
    ///
    /// # Parameters
    /// - `principal`: [`SecurityPrincipal`] for authorization
    /// - `container_id`: [`ContainerId`] that owns the namespace
    /// - `namespace_id`: [`NamespaceId`] to look up
    ///
    /// # Returns
    /// - `Some(NamespaceRecord)` if namespace exists
    /// - `None` if namespace not found
    pub async fn get_namespace(
        &self,
        principal: &SecurityPrincipal,
        container_id: &ContainerId,
        namespace_id: &NamespaceId,
    ) -> KeyValueResult<Option<NamespaceRecord>> {
        self.context
            .get_namespace(principal, container_id, namespace_id)
            .await
    }

    /// Create a new namespace within a container.
    ///
    /// # Access Control
    /// - Requires [`Permission::NamespaceCreate`] on [`ResourceScope::Container`]
    ///
    /// # Parameters
    /// - `principal`: [`SecurityPrincipal`] for authorization
    /// - `container_id`: [`ContainerId`] that will own the namespace
    /// - `config`: [`NamespaceCreate`] containing name and options
    ///
    /// # Returns
    /// The created [`NamespaceRecord`]
    pub async fn create_namespace(
        &self,
        principal: &SecurityPrincipal,
        container_id: &ContainerId,
        config: NamespaceCreate,
    ) -> KeyValueResult<NamespaceRecord> {
        self.context
            .create_namespace(principal, container_id, config)
            .await
    }

    /// Update an existing namespace's metadata.
    ///
    /// # Access Control
    /// - Requires [`Permission::NamespaceAlter`] on [`ResourceScope::Container`]
    ///
    /// # Parameters
    /// - `principal`: [`SecurityPrincipal`] for authorization
    /// - `container_id`: [`ContainerId`] that owns the namespace
    /// - `namespace_id`: [`NamespaceId`] of the namespace to update
    /// - `config`: [`NamespaceUpdate`] containing updates
    ///
    /// # Returns
    /// The updated [`NamespaceRecord`]
    pub async fn update_namespace(
        &self,
        principal: &SecurityPrincipal,
        container_id: &ContainerId,
        namespace_id: &NamespaceId,
        config: NamespaceUpdate,
    ) -> KeyValueResult<NamespaceRecord> {
        self.context
            .update_namespace(principal, container_id, namespace_id, config)
            .await
    }

    /// Delete a namespace.
    ///
    /// # Access Control
    /// - Requires [`Permission::NamespaceDelete`] on [`ResourceScope::Container`]
    ///
    /// # Parameters
    /// - `principal`: [`SecurityPrincipal`] for authorization
    /// - `container_id`: [`ContainerId`] that owns the namespace
    /// - `namespace_id`: [`NamespaceId`] of the namespace to delete
    ///
    /// # Returns
    /// `Ok(())` if successful
    pub async fn delete_namespace(
        &self,
        principal: &SecurityPrincipal,
        container_id: &ContainerId,
        namespace_id: &NamespaceId,
    ) -> KeyValueResult<()> {
        self.context
            .delete_namespace(principal, container_id, namespace_id)
            .await
    }

    /**********************************************************************************************\
     * Table Management                                                                           *
    \**********************************************************************************************/

    /// Get all tables within a container.
    ///
    /// # Access Control
    /// - Requires [`Permission::TableRead`] on [`ResourceScope::Database`]
    ///
    /// # Parameters
    /// - `principal`: [`SecurityPrincipal`] for authorization
    /// - `container_id`: [`ContainerId`] to list tables from
    ///
    /// # Returns
    /// An iterator over [`TableRecord`] for all tables in the container
    pub async fn get_tables(
        &self,
        principal: &SecurityPrincipal,
        container_id: &ContainerId,
    ) -> KeyValueResult<impl IntoIterator<Item = TableRecord>> {
        self.context.get_tables(principal, container_id).await
    }

    /// Get all tables within a specific namespace.
    ///
    /// # Access Control
    /// - Requires [`Permission::NamespaceList`] on [`ResourceScope::Database`]
    ///
    /// # Parameters
    /// - `principal`: [`SecurityPrincipal`] for authorization
    /// - `container_id`: [`ContainerId`] to search within
    /// - `namespace_id`: [`NamespaceId`] of the namespace
    ///
    /// # Returns
    /// An iterator over [`TableRecord`] for tables in the namespace
    pub async fn get_tables_by_namespace(
        &self,
        principal: &SecurityPrincipal,
        container_id: &ContainerId,
        namespace_id: &NamespaceId,
    ) -> KeyValueResult<impl IntoIterator<Item = TableRecord>> {
        self.context
            .get_tables_by_namespace(principal, container_id, namespace_id)
            .await
    }

    /// Get tables within a container that match a prefix.
    ///
    /// # Access Control
    /// - Requires [`Permission::TableRead`] on [`ResourceScope::Database`]
    ///
    /// # Parameters
    /// - `principal`: [`SecurityPrincipal`] for authorization
    /// - `container_id`: [`ContainerId`] to search within
    /// - `prefix`: String prefix to match table names against
    ///
    /// # Returns
    /// An iterator over [`TableRecord`] for matching tables
    pub async fn get_tables_by_prefix(
        &self,
        principal: &SecurityPrincipal,
        container_id: &ContainerId,
        prefix: &str,
    ) -> KeyValueResult<impl IntoIterator<Item = TableRecord>> {
        self.context
            .get_tables_by_prefix(principal, container_id, prefix)
            .await
    }

    /// Get metadata for a specific table.
    ///
    /// # Access Control
    /// - Requires [`Permission::TableRead`] on [`ResourceScope::Database`]
    ///
    /// # Parameters
    /// - `principal`: [`SecurityPrincipal`] for authorization
    /// - `container_id`: [`ContainerId`] that owns the table
    /// - `table_id`: [`ObjectId`] of the table to look up
    ///
    /// # Returns
    /// - `Some(TableRecord)` if table exists
    /// - `None` if table not found
    pub async fn get_table(
        &self,
        principal: &SecurityPrincipal,
        container_id: &ContainerId,
        table_id: &TableId,
    ) -> KeyValueResult<Option<TableRecord>> {
        self.context
            .get_table(principal, container_id, table_id)
            .await
    }

    /// Create a new table within a container.
    ///
    /// # Access Control
    /// - Requires [`Permission::TableCreate`] on [`ResourceScope::Database`]
    ///
    /// # Parameters
    /// - `principal`: [`SecurityPrincipal`] for authorization
    /// - `container_id`: [`ContainerId`] that will own the table
    /// - `config`: [`TableCreate`] containing name and table settings
    ///
    /// # Returns
    /// The created [`TableRecord`]
    pub async fn create_table(
        &self,
        principal: &SecurityPrincipal,
        container_id: &ContainerId,
        config: TableCreate,
    ) -> KeyValueResult<TableRecord> {
        self.context
            .create_table(principal, container_id, config)
            .await
    }

    /// Update an existing table's metadata.
    ///
    /// # Access Control
    /// - Requires [`Permission::TableAlter`] on [`ResourceScope::Database`]
    ///
    /// # Parameters
    /// - `principal`: [`SecurityPrincipal`] for authorization
    /// - `container_id`: [`ContainerId`] that owns the table
    /// - `table_id`: [`ObjectId`] of the table to update
    /// - `config`: [`TableUpdate`] containing updates
    ///
    /// # Returns
    /// The updated [`TableRecord`]
    pub async fn update_table(
        &self,
        principal: &SecurityPrincipal,
        container_id: &ContainerId,
        table_id: &TableId,
        config: TableUpdate,
    ) -> KeyValueResult<TableRecord> {
        self.context
            .update_table(principal, container_id, table_id, config)
            .await
    }

    /// Delete a table.
    ///
    /// # Access Control
    /// - Requires [`Permission::TableDelete`] on [`ResourceScope::Database`]
    ///
    /// # Parameters
    /// - `principal`: [`SecurityPrincipal`] for authorization
    /// - `container_id`: [`ContainerId`] that owns the table
    /// - `table_id`: [`ObjectId`] of the table to delete
    ///
    /// # Returns
    /// `Ok(())` if successful
    pub async fn delete_table(
        &self,
        principal: &SecurityPrincipal,
        container_id: &ContainerId,
        table_id: &TableId,
    ) -> KeyValueResult<()> {
        self.context
            .delete_table(principal, container_id, table_id)
            .await
    }
    /**********************************************************************************************\
     * Index Operations                                                                           *
    \**********************************************************************************************/

    /// Get all indexes for a table.
    ///
    /// # Access Control
    /// - Requires [`Permission::TableDataQuery`] on [`ResourceScope::Database`]
    ///
    /// # Parameters
    /// - `principal`: [`SecurityPrincipal`] for authorization
    /// - `container_id`: [`ContainerId`] that owns the table
    /// - `table_id`: [`ObjectId`] to list indexes for
    ///
    /// # Returns
    /// An iterator over [`IndexRecord`] for all indexes on the table
    pub async fn get_indexes(
        &self,
        principal: &SecurityPrincipal,
        container_id: &ContainerId,
    ) -> KeyValueResult<impl IntoIterator<Item = nanograph_core::object::IndexRecord>> {
        self.context.get_indexes(principal, container_id).await
    }

    /// Get metadata for a specific index.
    ///
    /// # Access Control
    /// - Requires [`Permission::TableDataQuery`] on [`ResourceScope::Database`]
    ///
    /// # Parameters
    /// - `principal`: [`SecurityPrincipal`] for authorization
    /// - `container_id`: [`ContainerId`] that owns the table
    /// - `table_id`: [`ObjectId`] that owns the index
    /// - `index_id`: [`IndexId`] to look up
    ///
    /// # Returns
    /// - `Some(IndexRecord)` if index exists
    /// - `None` if index not found
    pub async fn get_index(
        &self,
        principal: &SecurityPrincipal,
        container_id: &ContainerId,
        index_id: &IndexId,
    ) -> KeyValueResult<Option<nanograph_core::object::IndexRecord>> {
        self.context
            .get_index(principal, container_id, index_id)
            .await
    }

    /// Create a new index on a table.
    ///
    /// # Access Control
    /// - Requires [`Permission::TableCreate`] on [`ResourceScope::Database`]
    ///
    /// # Parameters
    /// - `principal`: [`SecurityPrincipal`] for authorization
    /// - `container_id`: [`ContainerId`] that owns the table
    /// - `table_id`: [`ObjectId`] to create index on
    /// - `config`: [`IndexCreate`] containing name, type, columns, options
    ///
    /// # Returns
    /// `Ok(IndexRecord)` with the created index metadata
    pub async fn create_index(
        &self,
        principal: &SecurityPrincipal,
        container_id: &ContainerId,
        config: IndexCreate,
    ) -> KeyValueResult<nanograph_core::object::IndexRecord> {
        self.context
            .create_index(principal, container_id, config)
            .await
    }

    /// Update an existing index.
    ///
    /// # Access Control
    /// - Requires [`Permission::TableCreate`] on [`ResourceScope::Database`]
    ///
    /// # Parameters
    /// - `principal`: [`SecurityPrincipal`] for authorization
    /// - `container_id`: [`ContainerId`] that owns the table
    /// - `table_id`: [`ObjectId`] that owns the index
    /// - `index_id`: [`IndexId`] to update
    /// - `config`: [`IndexUpdate`] with changes
    ///
    /// # Returns
    /// `Ok(IndexRecord)` with updated metadata
    pub async fn update_index(
        &self,
        principal: &SecurityPrincipal,
        container_id: &ContainerId,
        index_id: &IndexId,
        config: IndexUpdate,
    ) -> KeyValueResult<nanograph_core::object::IndexRecord> {
        self.context
            .update_index(principal, container_id, index_id, config)
            .await
    }

    /// Delete an index from a table.
    ///
    /// # Access Control
    /// - Requires [`Permission::TableDelete`] on [`ResourceScope::Database`]
    ///
    /// # Parameters
    /// - `principal`: [`SecurityPrincipal`] for authorization
    /// - `container_id`: [`ContainerId`] that owns the table
    /// - `table_id`: [`ObjectId`] that owns the index
    /// - `index_id`: [`IndexId`] to delete
    ///
    /// # Returns
    /// `Ok(())` if successful
    pub async fn delete_index(
        &self,
        principal: &SecurityPrincipal,
        container_id: &ContainerId,
        index_id: &IndexId,
    ) -> KeyValueResult<()> {
        self.context
            .delete_index(principal, container_id, index_id)
            .await
    }

    /**********************************************************************************************\
     * Data Operations                                                                            *
    \**********************************************************************************************/

    /// Insert or update a key-value pair in a table.
    ///
    /// # Access Control
    /// - Requires [`Permission::TableWrite`] on [`ResourceScope::Database`]
    ///
    /// # Parameters
    /// - `principal`: [`SecurityPrincipal`] for authorization
    /// - `container_id`: [`ContainerId`] that owns the table
    /// - `table_id`: [`ObjectId`] of the target table
    /// - `key`: Byte array key
    /// - `value`: Byte array value
    ///
    /// # Returns
    /// `Ok(())` if the operation was successful
    pub async fn put(
        &self,
        principal: &SecurityPrincipal,
        container_id: &ContainerId,
        table_id: &TableId,
        key: &[u8],
        value: &[u8],
    ) -> KeyValueResult<()> {
        self.context
            .table_entry_put(principal, container_id, table_id, key, value)
            .await
    }

    /// Retrieve a value from a table by its key.
    ///
    /// # Access Control
    /// - Requires [`Permission::TableRead`] on [`ResourceScope::Database`]
    ///
    /// # Parameters
    /// - `principal`: [`SecurityPrincipal`] for authorization
    /// - `container_id`: [`ContainerId`] that owns the table
    /// - `table_id`: [`ObjectId`] of the target table
    /// - `key`: Byte array key to look up
    ///
    /// # Returns
    /// - `Ok(Some(Vec<u8>))` containing the value if found
    /// - `Ok(None)` if the key does not exist
    pub async fn get(
        &self,
        principal: &SecurityPrincipal,
        container_id: &ContainerId,
        table_id: &TableId,
        key: &[u8],
    ) -> KeyValueResult<Option<Vec<u8>>> {
        self.context
            .table_entry_get(principal, container_id, table_id, key)
            .await
    }

    /// Delete a key-value pair from a table.
    ///
    /// # Access Control
    /// - Requires [`Permission::TableWrite`] on [`ResourceScope::Database`]
    ///
    /// # Parameters
    /// - `principal`: [`SecurityPrincipal`] for authorization
    /// - `container_id`: [`ContainerId`] that owns the table
    /// - `table_id`: [`ObjectId`] of the target table
    /// - `key`: Byte array key to delete
    ///
    /// # Returns
    /// - `Ok(true)` if the key was found and deleted
    /// - `Ok(false)` if the key did not exist
    pub async fn table_entry_delete(
        &self,
        principal: &SecurityPrincipal,
        container_id: &ContainerId,
        table_id: &TableId,
        key: &[u8],
    ) -> KeyValueResult<bool> {
        self.context
            .table_entry_delete(principal, container_id, table_id, key)
            .await
    }

    /// Perform multiple put operations in a single batch.
    ///
    /// # Access Control
    /// - Requires [`Permission::TableWrite`] on [`ResourceScope::Database`]
    ///
    /// # Parameters
    /// - `principal`: [`SecurityPrincipal`] for authorization
    /// - `container_id`: [`ContainerId`] that owns the table
    /// - `table_id`: [`ObjectId`] of the target table
    /// - `pairs`: Slice of key-value byte array pairs to insert
    ///
    /// # Returns
    /// `Ok(())` if all operations in the batch were successful
    pub async fn table_entry_batch_put(
        &self,
        principal: &SecurityPrincipal,
        container_id: &ContainerId,
        table_id: &TableId,
        pairs: &[(&[u8], &[u8])],
    ) -> KeyValueResult<()> {
        self.context
            .table_entry_batch_put(principal, container_id, table_id, pairs)
            .await
    }

    /// Register a storage engine.
    pub async fn register_engine(
        &self,
        engine_type: nanograph_kvt::StorageEngineType,
        engine: Arc<dyn nanograph_kvt::KeyValueShardStore>,
    ) -> nanograph_kvt::KeyValueResult<()> {
        self.context.register_engine(engine_type, engine).await
    }
}
