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

use crate::cache::SystemMetadataCache;
use crate::config::KeyValueDatabaseConfig;
use crate::container::ContainerHandle;
use crate::context::KeyValueDatabaseContext;
use nanograph_core::object::ContainerId;
use nanograph_core::object::{
    ClusterId, DatabaseCreate, DatabaseId, DatabaseRecord, DatabaseUpdate, NodeId, RegionId,
    ShardId, TablespaceCreate, TablespaceId, TablespaceRecord, TablespaceUpdate, TenantCreate,
    TenantId, TenantRecord, TenantUpdate,
};
use nanograph_kvt::KeyValueResult;
use nanograph_raft::{
    ClusterCreate, ClusterRecord, ClusterUpdate, ConsensusRouter, RegionCreate, RegionRecord,
    RegionUpdate, ServerCreate, ServerRecord, ServerUpdate,
};

use std::sync::{Arc, RwLock};

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
    /// System Metadata Cache
    system_metacache: Arc<RwLock<SystemMetadataCache>>,
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
    pub fn new_standalone(config: KeyValueDatabaseConfig) -> Self {
        KeyValueDatabaseManager {
            context: Arc::new(KeyValueDatabaseContext::new_standalone(config)),
            system_metacache: Arc::new(RwLock::new(SystemMetadataCache::new(ShardId::from(0)))),
        }
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
        raft_router: Arc<ConsensusRouter>,
    ) -> Self {
        Self {
            context: Arc::new(KeyValueDatabaseContext::new_distributed(
                config,
                raft_router,
            )),
            system_metacache: Arc::new(RwLock::new(SystemMetadataCache::new(ShardId::from(0)))),
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
        self.context.node_id()
    }

    /// Get the cluster ID.
    ///
    /// # Returns
    ///
    /// The ID of the cluster this manager belongs to
    pub fn cluster_id(&self) -> ClusterId {
        self.context.cluster_id()
    }

    /// Get the Raft consensus router.
    ///
    /// # Returns
    ///
    /// * `Some(&Arc<ConsensusRouter>)` - The router in distributed mode
    /// * `None` - Not applicable in standalone mode
    pub fn consensus_router(&self) -> Option<&Arc<ConsensusRouter>> {
        self.context.consensus_router()
    }

    /**********************************************************************************************\
     * Cluster Management                                                                         *
    \**********************************************************************************************/

    /// Initialize a new cluster.
    ///
    /// This should be called once when setting up a new cluster to create the
    /// cluster metadata.
    ///
    /// # Arguments
    ///
    /// * `config` - Configuration for the new cluster (name, options, metadata)
    ///
    /// # Returns
    ///
    /// * `Ok(())` - Cluster initialized successfully
    /// * `Err(KeyValueError)` - Initialization failed
    pub async fn initialize_cluster(&self, config: ClusterCreate) -> KeyValueResult<()> {
        self.context.initialize_cluster(config).await
    }

    /// Get the cluster metadata.
    ///
    /// # Returns
    ///
    /// The metadata for this cluster including name, version, and configuration
    pub async fn get_cluster(&self) -> KeyValueResult<ClusterRecord> {
        self.context.get_cluster().await
    }
    /// Update the cluster metadata.
    ///
    /// # Arguments
    ///
    /// * `cluster` - Update configuration (optional name and other changes)
    ///
    /// # Returns
    ///
    /// * `Ok(())` - Update successful
    /// * `Err(KeyValueError)` - Update failed
    pub async fn update_cluster(&self, cluster: ClusterUpdate) -> KeyValueResult<()> {
        self.context.update_cluster(cluster).await
    }

    /// Get Metadata about All Regions
    pub async fn get_regions(&self) -> KeyValueResult<impl IntoIterator<Item =RegionRecord>> {
        self.context.get_regions().await
    }

    /// Get Metadata about Region
    pub async fn get_region(&self, region: RegionId) -> KeyValueResult<Option<RegionRecord>> {
        self.context.get_region(region).await
    }
    /// Add a new region to the cluster
    pub async fn add_region(&self, config: RegionCreate) -> KeyValueResult<RegionRecord> {
        self.context.add_region(config).await
    }
    pub async fn update_region(
        &self,
        region: &RegionId,
        config: RegionUpdate,
    ) -> KeyValueResult<RegionRecord> {
        self.context.update_region(region, config).await
    }
    pub async fn remove_region(&self, region: &RegionId) -> KeyValueResult<()> {
        self.context.remove_region(region).await
    }

    pub async fn get_servers(&self) -> KeyValueResult<impl IntoIterator<Item =ServerRecord>> {
        self.context.get_servers().await
    }

    pub async fn get_servers_by_region(
        &self,
        region: &RegionId,
    ) -> KeyValueResult<impl IntoIterator<Item =ServerRecord>> {
        self.context.get_servers_by_region(region).await
    }

    pub async fn get_server(&self, server: &NodeId) -> KeyValueResult<Option<ServerRecord>> {
        self.context.get_server(server).await
    }

    pub async fn add_server(&self, config: ServerCreate) -> KeyValueResult<ServerRecord> {
        self.context.add_server(config).await
    }

    pub async fn update_server(
        &self,
        server: &NodeId,
        config: ServerUpdate,
    ) -> KeyValueResult<ServerRecord> {
        self.context.update_server(server, config).await
    }
    pub async fn remove_server(&self, server: &NodeId) -> KeyValueResult<()> {
        self.context.remove_server(server).await
    }

    /**********************************************************************************************\
     * Container Management                                                                       *
    \**********************************************************************************************/

    /// Get a handle for working with a specific database container.
    ///
    /// A container represents a tenant's database and provides access to namespaces,
    /// tables, and data operations.
    ///
    /// # Arguments
    ///
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
    /// let container = manager.get_container(&container_id).await?;
    /// ```
    pub async fn get_container(
        &self,
        container_id: &ContainerId,
    ) -> KeyValueResult<ContainerHandle> {
        Ok(ContainerHandle::new(
            *container_id,
            ShardId(0),
            self.context.clone(),
        ))
    }
    /// List all tenants in the system.
    ///
    /// Tenants provide isolation between different customers or organizational units.
    ///
    /// # Returns
    ///
    /// An iterator over all tenant metadata records
    pub async fn get_tenants(&self) -> KeyValueResult<impl IntoIterator<Item =TenantRecord>> {
        self.context.get_tenants().await
    }

    /// Get metadata for a specific tenant.
    ///
    /// # Arguments
    ///
    /// * `tenant` - The tenant ID to look up
    ///
    /// # Returns
    ///
    /// * `Ok(Some(metadata))` - The tenant exists
    /// * `Ok(None)` - The tenant does not exist
    pub async fn get_tenant(&self, tenant: &TenantId) -> KeyValueResult<Option<TenantRecord>> {
        self.context.get_tenant(tenant).await
    }

    /// Create a new tenant.
    ///
    /// # Arguments
    ///
    /// * `config` - Configuration for the new tenant (name, options, metadata)
    ///
    /// # Returns
    ///
    /// The created tenant metadata
    pub async fn create_tenant(&self, config: TenantCreate) -> KeyValueResult<TenantRecord> {
        self.context.create_tenant(config).await
    }

    pub async fn update_tenant(
        &self,
        tenant: &TenantId,
        config: TenantUpdate,
    ) -> KeyValueResult<TenantRecord> {
        self.context.update_tenant(tenant, config).await
    }
    pub async fn delete_tenant(&self, tenant: &TenantId) -> KeyValueResult<()> {
        self.context.delete_tenant(tenant).await
    }

    /// List all databases for a specific tenant.
    ///
    /// # Arguments
    ///
    /// * `tenant` - The tenant ID to query
    ///
    /// # Returns
    ///
    /// An iterator over database metadata records for the tenant
    pub async fn get_databases(
        &self,
        tenant: &TenantId,
    ) -> KeyValueResult<impl IntoIterator<Item =DatabaseRecord>> {
        self.context.get_databases(tenant).await
    }

    pub async fn get_database(
        &self,
        tenant: &TenantId,
        database: &DatabaseId,
    ) -> KeyValueResult<Option<DatabaseRecord>> {
        self.context.get_database(tenant, database).await
    }

    /// Create a new database for a tenant.
    ///
    /// # Arguments
    ///
    /// * `tenant` - The tenant ID that will own the database
    /// * `config` - Configuration for the new database (name, options, metadata)
    ///
    /// # Returns
    ///
    /// The created database metadata
    pub async fn create_database(
        &self,
        tenant: &TenantId,
        config: DatabaseCreate,
    ) -> KeyValueResult<DatabaseRecord> {
        self.context.create_database(tenant, config).await
    }

    pub async fn update_database(
        &self,
        tenant: &TenantId,
        database: &DatabaseId,
        config: DatabaseUpdate,
    ) -> KeyValueResult<DatabaseRecord> {
        self.context.update_database(tenant, database, config).await
    }
    pub async fn delete_database(
        &self,
        tenant: &TenantId,
        database: &DatabaseId,
    ) -> KeyValueResult<()> {
        self.context.delete_database(tenant, database).await
    }

    /**********************************************************************************************\
     * Tablespace Management                                                                      *
    \**********************************************************************************************/

    pub async fn get_tablespaces(
        &self,
    ) -> KeyValueResult<impl IntoIterator<Item =TablespaceRecord>> {
        self.context.get_tablespaces().await
    }

    pub async fn get_tablespace(
        &self,
        tablespace: &TablespaceId,
    ) -> KeyValueResult<Option<TablespaceRecord>> {
        self.context.get_tablespace(tablespace).await
    }

    pub async fn get_tablespace_by_name(&self, name: &str) -> KeyValueResult<Option<TablespaceId>> {
        self.context.get_tablespace_by_name(name).await
    }

    /// Create a new tablespace.
    ///
    /// Tablespaces define storage locations for table data, allowing you to
    /// control where data is physically stored.
    ///
    /// # Arguments
    ///
    /// * `config` - Configuration including name, storage path, and tier
    ///
    /// # Returns
    ///
    /// The created tablespace metadata
    pub async fn create_tablespace(
        &self,
        config: TablespaceCreate,
    ) -> KeyValueResult<TablespaceRecord> {
        self.context.create_tablespace(config).await
    }

    pub async fn update_tablespace(
        &self,
        tablespace: &TablespaceId,
        config: TablespaceUpdate,
    ) -> KeyValueResult<TablespaceRecord> {
        self.context.update_tablespace(tablespace, config).await
    }

    pub async fn delete_tablespace(&self, tablespace: &TablespaceId) -> KeyValueResult<()> {
        self.context.delete_tablespace(tablespace).await
    }
}
