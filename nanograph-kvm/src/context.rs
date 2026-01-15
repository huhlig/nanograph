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

use crate::cache::{ContainerMetadataCache, SystemMetadataCache};
use crate::config::KeyValueDatabaseConfig;
use crate::serialize;
use crate::shardmgr::KeyValueShardManager;
use crate::utility::{SystemKeys, deserialize};
use nanograph_core::object::{
    ClusterMetadata, DatabaseMetadata, RegionMetadata, ResourceScope, ServerMetadata,
    SystemUserRecord, TenantMetadata, TenantUserCreate, TenantUserMetadata, TenantUserRecord,
    TenantUserUpdate,
};
use nanograph_core::{
    object::{
        ClusterCreate, ClusterId, ClusterRecord, ClusterUpdate, ContainerId, DatabaseCreate,
        DatabaseId, DatabaseRecord, DatabaseUpdate, NamespaceCreate, NamespaceId, NamespaceRecord,
        NamespaceUpdate, NodeId, ObjectId, ObjectMetadata, ObjectType, Permission, RegionCreate,
        RegionId, RegionRecord, RegionUpdate, SecurityPrincipal, ServerCreate, ServerId,
        ServerRecord, ServerUpdate, ShardId, ShardIndex, ShardRecord, SystemUserCreate,
        SystemUserMetadata, SystemUserUpdate, TableCreate, TableId, TableRecord, TableSharding,
        TableUpdate, TablespaceCreate, TablespaceId, TablespaceRecord, TablespaceUpdate,
        TenantCreate, TenantId, TenantRecord, TenantUpdate, UserId,
    },
    types::{PropertyUpdate, Timestamp},
};
use nanograph_kvt::{KeyValueError, KeyValueResult};
use nanograph_raft::ConsensusRouter;
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

/// Inner Context Responsible for managing key-value databases, including creating, managing, and
/// querying tables. Keeps system metadata and manages shard allocation for tables.
///
/// Can operate in two modes:
/// - **Single-node mode**: Direct shard access (default)
/// - **Distributed mode**: Operations go through Raft consensus
///
/// TODO: Handle Table and Shard Allocation
pub struct KeyValueDatabaseContext {
    /// Node ID,
    node_id: NodeId,
    /// Local Shard Storage Manager
    shard_manager: Arc<RwLock<KeyValueShardManager>>,
    /// System Metadata Cache
    system_metacache: Arc<RwLock<SystemMetadataCache>>,
    /// Database Metadata Cache
    container_metacaches: Arc<RwLock<HashMap<DatabaseId, Arc<RwLock<ContainerMetadataCache>>>>>,
    /// Raft router for distributed mode
    raft_router: Option<Arc<ConsensusRouter>>,
}

impl KeyValueDatabaseContext {
    /// Create a new database context in single-node mode.
    ///
    /// # What it does
    /// Initializes a [`KeyValueDatabaseContext`] for standalone (non-distributed) operation.
    /// Creates a new shard manager, system metadata cache, and empty container metadata cache map.
    ///
    /// # How it works
    /// 1. Creates a standalone [`KeyValueShardManager`] for local shard storage
    /// 2. Initializes [`SystemMetadataCache`] with shard 0 for system metadata
    /// 3. Creates empty [`HashMap`] for container-specific metadata caches
    /// 4. Sets raft_router to None (no distributed consensus)
    ///
    /// # Parameters
    /// - `config`: [`KeyValueDatabaseConfig`] containing node_id and other configuration
    ///
    /// # Returns
    /// A new [`KeyValueDatabaseContext`] configured for single-node operation
    pub(crate) fn new_standalone(config: KeyValueDatabaseConfig) -> Self {
        let shard_manager = Arc::new(RwLock::new(KeyValueShardManager::new_standalone()));
        let system_metacache = Arc::new(RwLock::new(SystemMetadataCache::new(ShardId::from(0))));
        let container_metacaches = Arc::new(RwLock::new(HashMap::new()));
        Self {
            node_id: config.node_id,
            shard_manager,
            system_metacache,
            container_metacaches,
            raft_router: None,
        }
    }

    /// Create a new database context in distributed mode.
    ///
    /// # What it does
    /// Initializes a [`KeyValueDatabaseContext`] for distributed operation with Raft consensus.
    /// Similar to standalone mode but includes a Raft router for coordinating operations across nodes.
    ///
    /// # How it works
    /// 1. Creates a standalone [`KeyValueShardManager`] (will be coordinated via Raft)
    /// 2. Initializes [`SystemMetadataCache`] with shard 0 for system metadata
    /// 3. Creates empty [`HashMap`] for container-specific metadata caches
    /// 4. Stores the provided raft_router for distributed consensus operations
    ///
    /// # Parameters
    /// - `config`: [`KeyValueDatabaseConfig`] containing node_id and other configuration
    /// - `raft_router`: Arc-wrapped [`ConsensusRouter`] for distributed coordination
    ///
    /// # Returns
    /// A new [`KeyValueDatabaseContext`] configured for distributed operation
    pub fn new_distributed(
        config: KeyValueDatabaseConfig,
        raft_router: Arc<ConsensusRouter>,
    ) -> Self {
        let shard_manager = Arc::new(RwLock::new(KeyValueShardManager::new_standalone()));
        let system_metacache = Arc::new(RwLock::new(SystemMetadataCache::new(ShardId::from(0))));
        let container_metacaches = Arc::new(RwLock::new(HashMap::new()));
        Self {
            node_id: config.node_id,
            shard_manager,
            system_metacache,
            container_metacaches,
            raft_router: Some(raft_router),
        }
    }

    /// Check if running in distributed mode.
    ///
    /// # What it does
    /// Returns whether this context is configured for distributed operation.
    ///
    /// # How it works
    /// Simply checks if raft_router is Some (distributed) or None (standalone).
    ///
    /// # Returns
    /// - `true` if running in distributed mode with Raft consensus
    /// - `false` if running in standalone single-node mode
    pub fn is_distributed(&self) -> bool {
        self.raft_router.is_some()
    }

    /// Get the local [`NodeID`].
    ///
    /// # What it does
    /// Returns the [`NodeId`] of this node.
    ///
    /// # How it works
    /// Returns the node_id field that was set during construction from the config.
    ///
    /// # Returns
    /// - `NodeId` - Will likely be zero in standalone mode
    pub fn node_id(&self) -> NodeId {
        self.node_id
    }

    /// Get the [`ClusterId`].
    ///
    /// # What it does
    /// Returns the [`ClusterId`] that this context belongs to.
    ///
    /// # How it works
    /// Returns the cluster_id field that was set during construction from the config.
    ///
    /// # Returns
    /// The [`ClusterId`] for this database context
    pub fn cluster_id(&self) -> ClusterId {
        self.node_id.cluster_id()
    }

    /// Get the Raft router (if in distributed mode).
    ///
    /// # What it does
    /// Returns a reference to the [`ConsensusRouter`] for distributed operations.
    ///
    /// # How it works
    /// Returns a reference to the raft_router if it exists.
    ///
    /// # Returns
    /// - `Some(&Arc<ConsensusRouter>)` if running in distributed mode
    /// - `None` if running in standalone mode
    pub fn consensus_router(&self) -> Option<Arc<ConsensusRouter>> {
        self.raft_router.clone()
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
    /// # Parameters
    /// - `config`: [`ClusterCreate`] containing name, options, and metadata
    ///
    /// # Returns
    /// - `Ok(())` if cluster initialization succeeds
    /// - `Err(KeyValueError)` if lock poisoned or persistence fails
    #[tracing::instrument(skip(self))]
    pub async fn initialize_cluster(
        &self,
        config: ClusterCreate,
    ) -> KeyValueResult<ClusterMetadata> {
        let now = Timestamp::now();

        // Create cluster metadata
        let cluster = ClusterRecord {
            id: self.node_id.cluster_id(),
            name: config.name.clone(),
            version: 1,
            created_at: now,
            last_modified: now,
            options: config.options.clone(),
            metadata: config.metadata.clone(),
        };

        // Store in cache
        {
            let mut cache = self
                .system_metacache
                .write()
                .map_err(|_| KeyValueError::LockPoisoned)?;
            cache.set_cluster_record(cluster.clone());
        }

        // Persist to system shard
        let key = SystemKeys::cluster_key(self.node_id.cluster_id());
        let value = crate::utility::serialize(&cluster)?;

        let shard_manager = self
            .shard_manager
            .read()
            .map_err(|_| KeyValueError::LockPoisoned)?;
        shard_manager.put(ShardId::from(0), &key, &value).await?;

        Ok(ClusterMetadata::from(cluster))
    }

    /// Get the cluster metadata.
    ///
    /// # What it does
    /// Retrieves the metadata for this cluster, checking cache first then disk.
    ///
    /// # How it works
    /// 1. First, checks system_metacache for the cached cluster record
    /// 2. If not in cache, reads from system shard (ShardId 0) using [`SystemKeys::cluster_key`]
    /// 3. Deserializes the stored metadata
    /// 4. Returns error if the cluster metadata is not found
    ///
    /// # Access Control
    /// - Requires [`Permission::ClusterView`] on [`ResourceScope::System`]
    ///
    /// # Parameters
    /// - `principal`: [`SecurityPrincipal`] for authorization
    ///
    /// # Returns
    /// - `Ok(ClusterMetadata)` with the cluster information
    /// - `Err(KeyValueError)` if not found, lock poisoned, or deserialization fails
    #[tracing::instrument(skip(self))]
    pub async fn get_cluster(
        &self,
        principal: &SecurityPrincipal,
    ) -> KeyValueResult<ClusterMetadata> {
        if !principal.has_permission(&Permission::ClusterView) {
            return Err(KeyValueError::PermissionDenied {
                user_id: principal.user_id,
                permission: Permission::ClusterView,
                resource: ResourceScope::System,
            });
        }
        {
            // First Check the cache
            let lock = self
                .system_metacache
                .read()
                .map_err(|_| KeyValueError::LockPoisoned)?;
            if let Some(record) = lock.get_cluster_record() {
                return Ok(ClusterMetadata::from(record.clone()));
            }
        }
        {
            // Second read from Disk
            let lock = self
                .shard_manager
                .read()
                .map_err(|_| KeyValueError::LockPoisoned)?;
            if let Some(value) = lock
                .get(
                    ShardId::from(0),
                    &SystemKeys::cluster_key(self.node_id.cluster_id()),
                )
                .await?
            {
                return Ok(deserialize(&value)?);
            }
        }
        Err(KeyValueError::Internal(String::from(
            "Error getting Cluster Metadata",
        )))
    }

    /// Update cluster metadata.
    ///
    /// # What it does
    /// Updates the cluster metadata with new values from the config.
    ///
    /// # How it works
    /// 1. Acquires write lock on system_metacache
    /// 2. Retrieves existing cluster record from cache
    /// 3. Updates name if provided in config
    /// 4. Increments version and updates last_modified timestamp
    /// 5. Stores updated metadata back to cache
    /// 6. Serializes and persists to system shard (ShardId 0)
    ///
    /// # Access Control
    /// - Requires [`Permission::ClusterManage`] on [`ResourceScope::System`]
    ///
    /// # Parameters
    /// - `principal`: [`SecurityPrincipal`] for authorization
    /// - `config`: [`ClusterUpdate`] containing optional name and other updates
    ///
    /// # Returns
    /// - `Ok(())` if update succeeds
    /// - `Err(KeyValueError::InvalidValue)` if cluster is not initialized
    /// - `Err(KeyValueError)` if lock poisoned or persistence fails
    #[tracing::instrument(skip(self))]
    pub async fn update_cluster(
        &self,
        principal: &SecurityPrincipal,
        config: ClusterUpdate,
    ) -> KeyValueResult<ClusterMetadata> {
        if !principal.has_permission(&Permission::ClusterManage) {
            return Err(KeyValueError::PermissionDenied {
                user_id: principal.user_id,
                permission: Permission::ClusterManage,
                resource: ResourceScope::System,
            });
        }
        let mut cache = self
            .system_metacache
            .write()
            .map_err(|_| KeyValueError::LockPoisoned)?;

        if let Some(mut cluster) = cache.get_cluster_record().cloned() {
            // Update fields
            if let Some(name) = config.name {
                cluster.name = name;
            }

            // Update version and timestamp
            cluster.version += 1;
            cluster.last_modified = Timestamp::now();

            // Store in cache
            cache.set_cluster_record(cluster.clone());
            drop(cache);

            // Persist to system shard
            let key = SystemKeys::cluster_key(self.node_id.cluster_id());
            let value = serialize(&cluster)?;

            let shard_manager = self
                .shard_manager
                .read()
                .map_err(|_| KeyValueError::LockPoisoned)?;
            shard_manager.put(ShardId::from(0), &key, &value).await?;

            Ok(ClusterMetadata::from(cluster))
        } else {
            Err(KeyValueError::InvalidValue(
                "Cluster not initialized".to_string(),
            ))
        }
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
    /// An iterator over [`RegionRecord`] for all regions
    ///
    /// # TODO
    /// - Implement proper region enumeration from disk if not in cache
    /// - Add pagination support for large numbers of regions
    pub async fn get_regions(
        &self,
        principal: &SecurityPrincipal,
    ) -> KeyValueResult<impl IntoIterator<Item = RegionMetadata>> {
        if !principal.has_permission(&Permission::ClusterView) {
            return Err(KeyValueError::PermissionDenied {
                user_id: principal.user_id,
                permission: Permission::ClusterView,
                resource: ResourceScope::System,
            });
        }
        let lock = self.system_metacache.read().unwrap();
        Ok(lock
            .list_region_records()
            .cloned()
            .map(RegionMetadata::from)
            .collect::<Vec<_>>())
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
    /// - Requires [`Permission::ClusterView`] on [`ResourceScope::System`]
    ///
    /// # Parameters
    /// - `principal`: [`SecurityPrincipal`] for authorization
    /// - `region`: [`RegionId`] to look up
    ///
    /// # Returns
    /// - `Some(RegionMetadata)` if region exists
    /// - `None` if region not found
    ///
    /// # TODO
    /// - Implement fallback to disk if not in cache
    pub async fn get_region(
        &self,
        principal: &SecurityPrincipal,
        region: RegionId,
    ) -> KeyValueResult<Option<RegionMetadata>> {
        if !principal.has_permission(&Permission::ClusterView) {
            return Err(KeyValueError::PermissionDenied {
                user_id: principal.user_id,
                permission: Permission::ClusterView,
                resource: ResourceScope::System,
            });
        }
        let lock = self.system_metacache.read().unwrap();
        let region = lock
            .get_region_record(&region)
            .cloned()
            .map(RegionMetadata::from);
        Ok(region)
    }

    /// Get region metadata by name.
    ///
    /// # What it does
    /// Finds a region by its name string.
    ///
    /// # How it works
    /// Iterates through cached region records to find one matching the name.
    ///
    /// # Access Control
    /// - Requires [`Permission::ClusterView`] on [`ResourceScope::System`]
    ///
    /// # Parameters
    /// - `principal`: [`SecurityPrincipal`] for authorization
    /// - `name`: Region name to search for
    ///
    /// # Returns
    /// - `Some(RegionMetadata)` if region with that name exists
    /// - `None` if no matching region found
    ///
    /// # TODO
    /// - Implement proper region lookup from disk when cache is not available
    /// - Add index for name-based lookups
    pub async fn get_region_by_name(
        &self,
        principal: &SecurityPrincipal,
        name: &str,
    ) -> KeyValueResult<Option<RegionMetadata>> {
        if !principal.has_permission(&Permission::ClusterView) {
            return Err(KeyValueError::PermissionDenied {
                user_id: principal.user_id,
                permission: Permission::ClusterView,
                resource: ResourceScope::System,
            });
        }
        let lock = self.system_metacache.read().unwrap();
        let region = lock
            .list_region_records()
            .find(|r| r.name == name)
            .cloned()
            .map(RegionMetadata::from);
        Ok(region)
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
    /// - Requires [`Permission::SystemClusterManage`] on [`ResourceScope::System`]
    ///
    /// # Parameters
    /// - `principal`: [`SecurityPrincipal`] for authorization
    /// - `config`: [`RegionCreate`] containing name, cluster, options, and metadata
    ///
    /// # Returns
    /// - `Ok(RegionMetadata)` with the created region information
    /// - `Err(KeyValueError)` if lock poisoned or persistence fails
    ///
    /// # TODO
    /// - Generate proper unique region ID instead of hardcoded 0
    pub async fn add_region(
        &self,
        principal: &SecurityPrincipal,
        config: RegionCreate,
    ) -> KeyValueResult<RegionMetadata> {
        if !principal.has_permission(&Permission::ClusterManage) {
            return Err(KeyValueError::PermissionDenied {
                user_id: principal.user_id,
                permission: Permission::ClusterManage,
                resource: ResourceScope::System,
            });
        }
        let now = Timestamp::now();

        // TODO: Generate proper region ID
        let region_id = RegionId::from(0);

        // Create region metadata
        let region = RegionRecord {
            id: region_id,
            name: config.name.clone(),
            version: 1,
            cluster: config.cluster,
            created_at: now,
            last_modified: now,
            options: config.options.clone(),
            metadata: config.metadata.clone(),
        };

        // Store in cache
        {
            let mut cache = self
                .system_metacache
                .write()
                .map_err(|_| KeyValueError::LockPoisoned)?;
            cache.set_region_record(region.clone());
        }

        // Persist to system shard
        let key = SystemKeys::region_key(self.node_id.cluster_id(), region_id);
        let value = crate::utility::serialize(&region)?;

        let shard_manager = self
            .shard_manager
            .read()
            .map_err(|_| KeyValueError::LockPoisoned)?;
        shard_manager.put(ShardId::from(0), &key, &value).await?;

        Ok(RegionMetadata::from(region))
    }

    /// TODO: Add Documentation for update_region
    #[tracing::instrument(skip(self))]
    pub async fn update_region(
        &self,
        principal: &SecurityPrincipal,
        region: &RegionId,
        config: RegionUpdate,
    ) -> KeyValueResult<RegionMetadata> {
        if !principal.has_permission(&Permission::ClusterManage) {
            return Err(KeyValueError::PermissionDenied {
                user_id: principal.user_id,
                permission: Permission::ClusterManage,
                resource: ResourceScope::System,
            });
        }
        let mut cache = self
            .system_metacache
            .write()
            .map_err(|_| KeyValueError::LockPoisoned)?;

        if let Some(mut region_record) = cache.get_region_record(region).cloned() {
            // Update fields
            if let Some(name) = config.name {
                region_record.name = name;
            }

            // Update version and timestamp
            region_record.version += 1;
            region_record.last_modified = Timestamp::now();

            // Store in cache
            cache.set_region_record(region_record.clone());
            drop(cache);

            // Persist to system shard
            let key = SystemKeys::region_key(self.node_id.cluster_id(), *region);
            let value = serialize(&region_record)?;

            let shard_manager = self
                .shard_manager
                .read()
                .map_err(|_| KeyValueError::LockPoisoned)?;
            shard_manager.put(ShardId::from(0), &key, &value).await?;

            Ok(RegionMetadata::from(region_record))
        } else {
            Err(KeyValueError::InvalidValue(format!(
                "Region not found: {:?}",
                region
            )))
        }
    }

    /// TODO: Add Documentation for remove_region
    #[tracing::instrument(skip(self))]
    pub async fn remove_region(
        &self,
        principal: &SecurityPrincipal,
        region: &RegionId,
    ) -> KeyValueResult<()> {
        // Check permissions
        if !principal.has_permission(&Permission::ClusterManage) {
            return Err(KeyValueError::PermissionDenied {
                user_id: principal.user_id,
                permission: Permission::ClusterManage,
                resource: ResourceScope::System,
            });
        }
        // Remove from cache
        {
            let mut cache = self
                .system_metacache
                .write()
                .map_err(|_| KeyValueError::LockPoisoned)?;
            cache.clear_region_record(region);
        }

        // Generate Key
        let key = SystemKeys::region_key(self.node_id.cluster_id(), *region);

        // Delete record from system shard
        let shard_manager = self
            .shard_manager
            .read()
            .map_err(|_| KeyValueError::LockPoisoned)?;
        shard_manager.delete(ShardId::from(0), &key).await?;

        Ok(())
    }

    /// TODO: Add Documentation for get_servers
    #[tracing::instrument(skip(self))]
    pub async fn get_servers(
        &self,
        principal: &SecurityPrincipal,
    ) -> KeyValueResult<impl IntoIterator<Item = ServerMetadata>> {
        if !principal.has_permission(&Permission::ClusterView) {
            return Err(KeyValueError::PermissionDenied {
                user_id: principal.user_id,
                permission: Permission::ClusterView,
                resource: ResourceScope::System,
            });
        }
        // TODO: Implement Proper Get Server Logic
        let lock = self.system_metacache.read().unwrap();
        let servers = lock
            .list_server_records()
            .cloned()
            .map(ServerMetadata::from)
            .collect::<Vec<_>>();
        Ok(servers)
    }

    #[tracing::instrument(skip(self))]
    pub async fn get_servers_by_region(
        &self,
        principal: &SecurityPrincipal,
        region: &RegionId,
    ) -> KeyValueResult<impl IntoIterator<Item = ServerMetadata>> {
        if !principal.has_permission(&Permission::ClusterView) {
            return Err(KeyValueError::PermissionDenied {
                user_id: principal.user_id,
                permission: Permission::ClusterView,
                resource: ResourceScope::System,
            });
        }
        // TODO: Implement Proper Get Server By Region Logic
        let lock = self.system_metacache.read().unwrap();
        let servers = lock
            .list_server_records()
            .filter(|record| record.id.region_id() == *region)
            .cloned()
            .map(ServerMetadata::from)
            .collect::<Vec<ServerMetadata>>();
        Ok(servers)
    }

    /// TODO: Add Documentation for get_server
    #[tracing::instrument(skip(self))]
    pub async fn get_server(
        &self,
        principal: &SecurityPrincipal,
        server: &NodeId,
    ) -> KeyValueResult<Option<ServerMetadata>> {
        if !principal.has_permission(&Permission::ClusterView) {
            return Err(KeyValueError::PermissionDenied {
                user_id: principal.user_id,
                permission: Permission::ClusterView,
                resource: ResourceScope::System,
            });
        }
        // Check cache first
        {
            let cache = self
                .system_metacache
                .read()
                .map_err(|_| KeyValueError::LockPoisoned)?;
            if let Some(server_meta) = cache.get_server_record(&server.server_id()) {
                return Ok(Some(ServerMetadata::from(server_meta.clone())));
            }
        }

        // Read from disk
        let key = SystemKeys::server_key(
            self.node_id.cluster_id(),
            server.region_id(),
            server.server_id(),
        );
        let shard_manager = self
            .shard_manager
            .read()
            .map_err(|_| KeyValueError::LockPoisoned)?;

        if let Some(value) = shard_manager.get(ShardId::from(0), &key).await? {
            let server_meta: ServerRecord = deserialize(&value)?;

            // Update cache
            let mut cache = self
                .system_metacache
                .write()
                .map_err(|_| KeyValueError::LockPoisoned)?;
            cache.set_server_record(server_meta.clone());

            Ok(Some(ServerMetadata::from(server_meta.clone())))
        } else {
            Ok(None)
        }
    }

    /// TODO: Add Documentation for add_server
    #[tracing::instrument(skip(self))]
    pub async fn add_server(
        &self,
        principal: &SecurityPrincipal,
        config: ServerCreate,
    ) -> KeyValueResult<ServerMetadata> {
        // Check Permissions
        if !principal.has_permission(&Permission::ClusterManage) {
            return Err(KeyValueError::PermissionDenied {
                user_id: principal.user_id,
                permission: Permission::ClusterManage,
                resource: ResourceScope::System,
            });
        }
        let now = Timestamp::now();

        // Generate NodeId from region and cluster
        let node_id = NodeId::from_parts(config.cluster, config.region, ServerId::from(0));

        // Create server metadata
        let server = ServerRecord {
            id: node_id,
            name: config.name.clone(),
            version: 1,
            created_at: now,
            last_modified: now,
            options: config.options.clone(),
            metadata: config.metadata.clone(),
        };

        // Store in cache
        {
            let mut cache = self
                .system_metacache
                .write()
                .map_err(|_| KeyValueError::LockPoisoned)?;
            cache.set_server_record(server.clone());
        }

        // Persist to system shard
        let key =
            SystemKeys::server_key(self.node_id.cluster_id(), config.region, ServerId::from(0));
        let value = crate::utility::serialize(&server)?;

        let shard_manager = self
            .shard_manager
            .read()
            .map_err(|_| KeyValueError::LockPoisoned)?;
        shard_manager.put(ShardId::from(0), &key, &value).await?;

        Ok(ServerMetadata::from(server))
    }

    /// TODO: Add documentation for update_server
    #[tracing::instrument(skip(self))]
    pub async fn update_server(
        &self,
        principal: &SecurityPrincipal,
        server: &NodeId,
        config: ServerUpdate,
    ) -> KeyValueResult<ServerMetadata> {
        if !principal.has_permission(&Permission::ClusterManage) {
            return Err(KeyValueError::PermissionDenied {
                user_id: principal.user_id,
                permission: Permission::ClusterManage,
                resource: ResourceScope::System,
            });
        }
        let mut cache = self
            .system_metacache
            .write()
            .map_err(|_| KeyValueError::LockPoisoned)?;

        if let Some(mut server_record) = cache.get_server_record(&server.server_id()).cloned() {
            // Update fields
            if let Some(name) = config.name {
                server_record.name = name;
            }

            // Update version and timestamp
            server_record.version += 1;
            server_record.last_modified = Timestamp::now();

            // Store in cache
            cache.set_server_record(server_record.clone());
            drop(cache);

            // Persist to system shard
            let key = SystemKeys::server_key(
                self.node_id.cluster_id(),
                server.region_id(),
                server.server_id(),
            );
            let value = serialize(&server_record)?;

            let shard_manager = self
                .shard_manager
                .read()
                .map_err(|_| KeyValueError::LockPoisoned)?;
            shard_manager.put(ShardId::from(0), &key, &value).await?;

            Ok(ServerMetadata::from(server_record))
        } else {
            Err(KeyValueError::InvalidValue(format!(
                "Server not found: {:?}",
                server
            )))
        }
    }

    /// Remove a server from the cluster.
    ///
    /// # What it does
    /// Deletes a server and its metadata from the cluster.
    ///
    /// # How it works
    /// 1. Removes server record from system_metacache
    /// 2. Deletes persisted metadata from system shard using [`SystemKeys::server_key`]
    ///
    /// # Access Control
    /// - Requires [`Permission::ClusterManage`] on [`ResourceScope::System`]
    ///
    /// # Parameters
    /// - `principal`: [`SecurityPrincipal`] for authorization
    /// - `server`: [`NodeId`] to remove
    ///
    /// # Returns
    /// - `Ok(())` if removal succeeds
    /// - `Err(KeyValueError)` if lock poisoned or deletion fails
    ///
    /// # TODO
    /// - Add validation to prevent deletion if server is hosting shards
    /// - Implement graceful server removal with shard migration
    #[tracing::instrument(skip(self))]
    pub async fn remove_server(
        &self,
        principal: &SecurityPrincipal,
        server: &NodeId,
    ) -> KeyValueResult<()> {
        if !principal.has_permission(&Permission::ClusterManage) {
            return Err(KeyValueError::PermissionDenied {
                user_id: principal.user_id,
                permission: Permission::ClusterManage,
                resource: ResourceScope::System,
            });
        }
        // Remove from cache
        {
            let mut cache = self
                .system_metacache
                .write()
                .map_err(|_| KeyValueError::LockPoisoned)?;
            cache.clear_server_record(&server.server_id());
        }

        // Delete from system shard
        let key = SystemKeys::server_key(self.cluster_id(), server.region_id(), server.server_id());

        let shard_manager = self
            .shard_manager
            .read()
            .map_err(|_| KeyValueError::LockPoisoned)?;
        shard_manager.delete(ShardId::from(0), &key).await?;

        Ok(())
    }

    /// Get metadata about all users.
    ///
    /// # What it does
    /// Returns an iterator over all user metadata records.
    ///
    /// # How it works
    /// Reads from system_metacache and returns all cached user records.
    ///
    /// # Access Control
    /// - Requires [`Permission::SecurityManage`] on [`ResourceScope::System`]
    ///
    /// # Parameters
    /// - `principal`: [`SecurityPrincipal`] for authorization
    ///
    /// # Returns
    /// An iterator over [`SystemUserMetadata`] for all users
    ///
    /// # TODO
    /// - Implement pagination support for large numbers of users
    /// - Add disk fallback if not in cache
    #[tracing::instrument(skip(self))]
    pub async fn get_system_users(
        &self,
        principal: &SecurityPrincipal,
    ) -> KeyValueResult<impl IntoIterator<Item = SystemUserMetadata>> {
        if !principal.has_permission(&Permission::SecurityManage) {
            return Err(KeyValueError::PermissionDenied {
                user_id: principal.user_id,
                permission: Permission::SecurityManage,
                resource: ResourceScope::System,
            });
        }
        let cache = self
            .system_metacache
            .read()
            .map_err(|_| KeyValueError::LockPoisoned)?;
        let users = cache
            .list_system_user_records()
            .cloned()
            .map(SystemUserMetadata::from)
            .collect::<Vec<_>>();
        Ok(users)
    }

    /// Get metadata for a specific user.
    ///
    /// # What it does
    /// Retrieves metadata for a single user by ID, checking cache first then disk.
    ///
    /// # How it works
    /// 1. Checks system_metacache for cached user record
    /// 2. If not in cache, reads from system shard using SystemKeys::user_key
    /// 3. Deserializes the stored metadata
    /// 4. Updates cache with the retrieved metadata
    ///
    /// # Access Control
    /// - Requires [`Permission::SystemSecurityManage`] on [`ResourceScope::System`]
    ///
    /// # Parameters
    /// - `principal`: [`SecurityPrincipal`] for authorization
    /// - `user`: [`UserId`] to look up
    ///
    /// # Returns
    /// - `Some(SystemUserMetadata)` if user exists
    /// - `None` if user not found
    /// - `Err(KeyValueError)` if lock poisoned or deserialization fails
    #[tracing::instrument(skip(self))]
    pub async fn get_system_user(
        &self,
        principal: &SecurityPrincipal,
        user: UserId,
    ) -> KeyValueResult<Option<SystemUserMetadata>> {
        if !principal.has_permission(&Permission::SecurityManage) {
            return Err(KeyValueError::PermissionDenied {
                user_id: principal.user_id,
                permission: Permission::SecurityManage,
                resource: ResourceScope::System,
            });
        }
        // Check cache first
        {
            let cache = self
                .system_metacache
                .read()
                .map_err(|_| KeyValueError::LockPoisoned)?;
            if let Some(user_meta) = cache.get_system_user_record(&user) {
                return Ok(Some(SystemUserMetadata::from(user_meta.clone())));
            }
        }

        // Read from disk
        let key = SystemKeys::system_user_key(user);
        let shard_manager = self
            .shard_manager
            .read()
            .map_err(|_| KeyValueError::LockPoisoned)?;

        if let Some(value) = shard_manager.get(ShardId::from(0), &key).await? {
            let user_record: SystemUserRecord = deserialize(&value)?;

            // Update cache
            let mut cache = self
                .system_metacache
                .write()
                .map_err(|_| KeyValueError::LockPoisoned)?;
            cache.set_system_user_record(user_record.clone());

            Ok(Some(SystemUserMetadata::from(user_record)))
        } else {
            Ok(None)
        }
    }

    /// Get user metadata by login name.
    ///
    /// # What it does
    /// Finds a user by their username string.
    ///
    /// # How it works
    /// Iterates through cached user records to find one matching the login name.
    /// Currently uses the 'name' field as the login identifier.
    ///
    /// # Access Control
    /// - Requires [`Permission::SystemSecurityManage`] on [`ResourceScope::System`]
    ///
    /// # Parameters
    /// - `principal`: [`SecurityPrincipal`] for authorization
    /// - `username`: Username to search for
    ///
    /// # Returns
    /// - `Some(UserMetadata)` if user with that login exists
    /// - `None` if no matching user found
    ///
    /// # TODO
    /// - Add proper login field to UserMetadata (currently using name field)
    /// - Add index for login-based lookups
    /// - Implement disk fallback if not in cache
    #[tracing::instrument(skip(self))]
    pub async fn get_system_user_by_username(
        &self,
        principal: &SecurityPrincipal,
        username: &str,
    ) -> KeyValueResult<Option<SystemUserMetadata>> {
        if !principal.has_permission(&Permission::SecurityManage) {
            return Err(KeyValueError::PermissionDenied {
                user_id: principal.user_id,
                permission: Permission::SecurityManage,
                resource: ResourceScope::System,
            });
        }
        // Search in cache by name (UserMetadata doesn't have a login field, using name as identifier)
        let cache = self
            .system_metacache
            .read()
            .map_err(|_| KeyValueError::LockPoisoned)?;
        let user = cache
            .list_system_user_records()
            .find(|u| u.username == username)
            .cloned()
            .map(SystemUserMetadata::from);
        Ok(user)
    }

    /// Create a new user.
    ///
    /// # What it does
    /// Creates a new user with the provided configuration.
    ///
    /// # How it works
    /// 1. Generates a new [`UserId`]
    /// 2. Creates [`UserMetadata`] with current timestamp, version 1, and empty groups/roles/grants
    /// 3. Stores in system_metacache
    /// 4. Serializes and persists to system shard using [`SystemKeys::user_key`]
    ///
    /// # Access Control
    /// - Requires [`Permission::SystemSecurityManage`] on [`ResourceScope::System`]
    ///
    /// # Parameters
    /// - `principal`: [`SecurityPrincipal`] for authorization
    /// - `config`: [`SystemUserCreate`] containing name, options, and metadata
    ///
    /// # Returns
    /// - `Ok(UserMetadata)` with the created user information
    /// - `Err(KeyValueError)` if lock poisoned or persistence fails
    ///
    /// # TODO
    /// - Generate proper unique user ID instead of hardcoded 0
    /// - Add password hashing support (currently password_hash is None)
    /// - Implement email validation
    #[tracing::instrument(skip(self))]
    pub async fn create_system_user(
        &self,
        principal: &SecurityPrincipal,
        config: SystemUserCreate,
    ) -> KeyValueResult<SystemUserMetadata> {
        if !principal.has_permission(&Permission::SecurityManage) {
            return Err(KeyValueError::PermissionDenied {
                user_id: principal.user_id,
                permission: Permission::SecurityManage,
                resource: ResourceScope::System,
            });
        }
        let now = Timestamp::now();

        // TODO: Generate proper user ID
        let user_id = UserId::from(0);

        // Create user metadata
        let user = SystemUserRecord {
            id: user_id,
            username: config.username.clone(),
            version: 1,
            created_at: now,
            last_modified: now,
            groups: Vec::new(),
            roles: Vec::new(),
            grants: Vec::new(), // Start with no permission grants - grant via roles/groups
            enabled: true,
            password_hash: None, // TODO: Add password hashing
            options: config.options.clone(),
            metadata: config.metadata.clone(),
        };

        // Store in cache
        {
            let mut cache = self
                .system_metacache
                .write()
                .map_err(|_| KeyValueError::LockPoisoned)?;
            cache.set_system_user_record(user.clone());
        }

        // Persist to system shard
        let key = SystemKeys::system_user_key(user_id);
        let value = crate::utility::serialize(&user)?;

        let shard_manager = self
            .shard_manager
            .read()
            .map_err(|_| KeyValueError::LockPoisoned)?;
        shard_manager.put(ShardId::from(0), &key, &value).await?;

        Ok(SystemUserMetadata::from(user))
    }

    /// Update an existing user's metadata.
    ///
    /// # What it does
    /// Updates user metadata with new values from the config.
    ///
    /// # How it works
    /// 1. Acquires write lock on system_metacache
    /// 2. Retrieves existing user record
    /// 3. Updates name if provided in config
    /// 4. Applies PropertyUpdate operations to options and metadata maps
    /// 5. Increments version and updates last_modified timestamp
    /// 6. Stores updated metadata back to cache
    /// 7. Serializes and persists to system shard
    ///
    /// # Access Control
    /// - Requires [`Permission::SystemSecurityManage`] on [`ResourceScope::System`]
    ///
    /// # Parameters
    /// - `principal`: [`SecurityPrincipal`] for authorization
    /// - `user`: [`UserId`] to update
    /// - `config`: [`SystemUserUpdate`] containing optional name, options, and metadata updates
    ///
    /// # Returns
    /// - `Ok(UserMetadata)` with updated user information
    /// - `Err(KeyValueError::InvalidValue)` if user not found
    /// - `Err(KeyValueError)` if lock poisoned or persistence fails
    #[tracing::instrument(skip(self))]
    pub async fn update_system_user(
        &self,
        principal: &SecurityPrincipal,
        user: &UserId,
        config: SystemUserUpdate,
    ) -> KeyValueResult<SystemUserMetadata> {
        if !principal.has_permission(&Permission::SecurityManage) {
            return Err(KeyValueError::PermissionDenied {
                user_id: principal.user_id,
                permission: Permission::SecurityManage,
                resource: ResourceScope::System,
            });
        }
        let mut cache = self
            .system_metacache
            .write()
            .map_err(|_| KeyValueError::LockPoisoned)?;

        if let Some(mut user_record) = cache.get_system_user_record(user).cloned() {
            // Update fields
            if let Some(username) = config.username {
                user_record.username = username;
            }

            // Apply option updates
            for opt_update in &config.options {
                match opt_update {
                    PropertyUpdate::Set(key, value) => {
                        user_record.options.insert(key.clone(), value.clone());
                    }
                    PropertyUpdate::Clear(key) => {
                        user_record.options.remove(key);
                    }
                }
            }

            // Apply metadata updates
            for meta_update in &config.metadata {
                match meta_update {
                    PropertyUpdate::Set(key, value) => {
                        user_record.metadata.insert(key.clone(), value.clone());
                    }
                    PropertyUpdate::Clear(key) => {
                        user_record.metadata.remove(key);
                    }
                }
            }

            // Update version and timestamp
            user_record.version += 1;
            user_record.last_modified = Timestamp::now();

            // Store in cache
            cache.set_system_user_record(user_record.clone());
            drop(cache);

            // Persist to system shard
            let key = SystemKeys::system_user_key(*user);
            let value = crate::utility::serialize(&user_record)?;

            let shard_manager = self
                .shard_manager
                .read()
                .map_err(|_| KeyValueError::LockPoisoned)?;
            shard_manager.put(ShardId::from(0), &key, &value).await?;

            Ok(SystemUserMetadata::from(user_record))
        } else {
            Err(KeyValueError::InvalidValue(format!(
                "User not found: {:?}",
                user
            )))
        }
    }

    /// Remove a user.
    ///
    /// # What it does
    /// Deletes a user and their metadata.
    ///
    /// # How it works
    /// 1. Removes user record from system_metacache
    /// 2. Deletes persisted metadata from system shard using SystemKeys::user_key
    ///
    /// # Access Control
    /// - Requires [`Permission::SystemSecurityManage`] on [`ResourceScope::System`]
    ///
    /// # Parameters
    /// - `principal`: [`SecurityPrincipal`] for authorization
    /// - `user`: [`UserId`] to remove
    ///
    /// # Returns
    /// - `Ok(())` if removal succeeds
    /// - `Err(KeyValueError)` if lock poisoned or deletion fails
    ///
    /// # TODO
    /// - Add validation to check for user's active sessions
    /// - Implement cascade deletion or transfer of user-owned resources
    #[tracing::instrument(skip(self))]
    pub async fn remove_system_user(
        &self,
        principal: &SecurityPrincipal,
        user: &UserId,
    ) -> KeyValueResult<()> {
        if !principal.has_permission(&Permission::SecurityManage) {
            return Err(KeyValueError::PermissionDenied {
                user_id: principal.user_id,
                permission: Permission::SecurityManage,
                resource: ResourceScope::System,
            });
        }
        // Remove from cache
        {
            let mut cache = self
                .system_metacache
                .write()
                .map_err(|_| KeyValueError::LockPoisoned)?;
            cache.clear_system_user_record(user);
        }

        // Delete from system shard
        let key = SystemKeys::system_user_key(*user);

        let shard_manager = self
            .shard_manager
            .read()
            .map_err(|_| KeyValueError::LockPoisoned)?;
        shard_manager.delete(ShardId::from(0), &key).await?;

        Ok(())
    }

    /**********************************************************************************************\
     * Container Management                                                                       *
    \**********************************************************************************************/
    /// TODO: Add Documentation for get_tenants
    /// TODO: Read from disk if not in cache
    #[tracing::instrument(skip(self))]
    pub async fn get_tenants(
        &self,
        principal: &SecurityPrincipal,
    ) -> KeyValueResult<impl IntoIterator<Item = TenantMetadata>> {
        if !principal.has_permission(&Permission::TenantView) {
            return Err(KeyValueError::PermissionDenied {
                user_id: principal.user_id,
                permission: Permission::TenantView,
                resource: ResourceScope::System,
            });
        }
        let cache = self
            .system_metacache
            .read()
            .map_err(|_| KeyValueError::LockPoisoned)?;
        let tenants = cache
            .list_tenant_records()
            .cloned()
            .map(TenantMetadata::from)
            .collect::<Vec<_>>();
        Ok(tenants)
    }

    /// TODO: Add Documentation for get_tenant
    #[tracing::instrument(skip(self))]
    pub async fn get_tenant(
        &self,
        principal: &SecurityPrincipal,
        tenant: &TenantId,
    ) -> KeyValueResult<Option<TenantMetadata>> {
        if !principal.has_permission(&Permission::TenantView) {
            return Err(KeyValueError::PermissionDenied {
                user_id: principal.user_id,
                permission: Permission::TenantView,
                resource: ResourceScope::System,
            });
        }
        // Check cache first
        {
            let cache = self
                .system_metacache
                .read()
                .map_err(|_| KeyValueError::LockPoisoned)?;
            if let Some(tenant_meta) = cache.get_tenant_record(tenant) {
                return Ok(Some(TenantMetadata::from(tenant_meta.clone())));
            }
        }

        // Read from disk
        let key = SystemKeys::tenant_key(*tenant);
        let shard_manager = self
            .shard_manager
            .read()
            .map_err(|_| KeyValueError::LockPoisoned)?;

        if let Some(value) = shard_manager.get(ShardId::from(0), &key).await? {
            let tenant_meta: TenantRecord = deserialize(&value)?;

            // Update cache
            let mut cache = self
                .system_metacache
                .write()
                .map_err(|_| KeyValueError::LockPoisoned)?;
            cache.set_tenant_record(tenant_meta.clone());

            Ok(Some(TenantMetadata::from(tenant_meta)))
        } else {
            Ok(None)
        }
    }

    /// TODO: Add Documentation for get_tenant_by_name
    #[tracing::instrument(skip(self))]
    pub async fn get_tenant_by_name(
        &self,
        principal: &SecurityPrincipal,
        name: &str,
    ) -> KeyValueResult<Option<TenantMetadata>> {
        if !principal.has_permission(&Permission::TenantView) {
            return Err(KeyValueError::PermissionDenied {
                user_id: principal.user_id,
                permission: Permission::TenantView,
                resource: ResourceScope::System,
            });
        }
        // Search in cache
        let cache = self
            .system_metacache
            .read()
            .map_err(|_| KeyValueError::LockPoisoned)?;
        let tenant = cache
            .list_tenant_records()
            .find(|t| t.name == name)
            .cloned()
            .map(TenantMetadata::from);
        Ok(tenant)
    }

    /// TODO: Add Documentation for create_tenant
    #[tracing::instrument(skip(self))]
    pub async fn create_tenant(
        &self,
        principal: &SecurityPrincipal,
        config: TenantCreate,
    ) -> KeyValueResult<TenantMetadata> {
        if !principal.has_permission(&Permission::SystemTenantManage) {
            return Err(KeyValueError::PermissionDenied {
                user_id: principal.user_id,
                permission: Permission::SystemTenantManage,
                resource: ResourceScope::System,
            });
        }
        let now = Timestamp::now();

        // TODO: Generate proper tenant ID
        let tenant_id = TenantId::from(0);

        // Create tenant metadata
        let tenant = TenantRecord {
            id: tenant_id,
            name: config.name.clone(),
            version: 1,
            created_at: now,
            last_modified: now,
            default_tablespace: None,
            options: config.options.clone(),
            metadata: config.metadata.clone(),
        };

        // Store in cache
        {
            let mut cache = self
                .system_metacache
                .write()
                .map_err(|_| KeyValueError::LockPoisoned)?;
            cache.set_tenant_record(tenant.clone());
        }

        // Persist to system shard
        let key = SystemKeys::tenant_key(tenant_id);
        let value = serialize(&tenant)?;

        let shard_manager = self
            .shard_manager
            .read()
            .map_err(|_| KeyValueError::LockPoisoned)?;
        shard_manager.put(ShardId::from(0), &key, &value).await?;

        Ok(TenantMetadata::from(tenant))
    }

    /// TODO: Add Documentation for update_tenant
    #[tracing::instrument(skip(self))]
    pub async fn update_tenant(
        &self,
        principal: &SecurityPrincipal,
        tenant: &TenantId,
        config: TenantUpdate,
    ) -> KeyValueResult<TenantMetadata> {
        if !principal.has_permission(&Permission::SystemTenantManage) {
            return Err(KeyValueError::PermissionDenied {
                user_id: principal.user_id,
                permission: Permission::SystemTenantManage,
                resource: ResourceScope::System,
            });
        }
        let mut cache = self
            .system_metacache
            .write()
            .map_err(|_| KeyValueError::LockPoisoned)?;

        if let Some(mut tenant_record) = cache.get_tenant_record(tenant).cloned() {
            // Update fields
            if let Some(name) = config.name {
                tenant_record.name = name;
            }

            // Apply option updates
            for opt_update in &config.options {
                match opt_update {
                    PropertyUpdate::Set(key, value) => {
                        tenant_record.options.insert(key.clone(), value.clone());
                    }
                    PropertyUpdate::Clear(key) => {
                        tenant_record.options.remove(key);
                    }
                }
            }

            // Apply metadata updates
            for meta_update in &config.metadata {
                match meta_update {
                    PropertyUpdate::Set(key, value) => {
                        tenant_record.metadata.insert(key.clone(), value.clone());
                    }
                    PropertyUpdate::Clear(key) => {
                        tenant_record.metadata.remove(key);
                    }
                }
            }

            // Update version and timestamp
            tenant_record.version += 1;
            tenant_record.last_modified = Timestamp::now();

            // Store in cache
            cache.set_tenant_record(tenant_record.clone());
            drop(cache);

            // Persist to system shard
            let key = SystemKeys::tenant_key(*tenant);
            let value = crate::utility::serialize(&tenant_record)?;

            let shard_manager = self
                .shard_manager
                .read()
                .map_err(|_| KeyValueError::LockPoisoned)?;
            shard_manager.put(ShardId::from(0), &key, &value).await?;

            Ok(TenantMetadata::from(tenant_record))
        } else {
            Err(KeyValueError::InvalidValue(format!(
                "Tenant not found: {:?}",
                tenant
            )))
        }
    }

    /// TODO: Add Documentation for remove_tenant
    #[tracing::instrument(skip(self))]
    pub async fn delete_tenant(
        &self,
        principal: &SecurityPrincipal,
        tenant: &TenantId,
    ) -> KeyValueResult<()> {
        if !principal.has_permission(&Permission::SystemTenantManage) {
            return Err(KeyValueError::PermissionDenied {
                user_id: principal.user_id,
                permission: Permission::SystemTenantManage,
                resource: ResourceScope::System,
            });
        }
        // TODO: Check if any databases exist for this tenant
        // TODO: Prevent deletion if databases exist

        // Remove from cache
        {
            let mut cache = self
                .system_metacache
                .write()
                .map_err(|_| KeyValueError::LockPoisoned)?;
            cache.clear_tenant_record(tenant);
        }

        // Delete from system shard
        let key = SystemKeys::tenant_key(*tenant);

        let shard_manager = self
            .shard_manager
            .read()
            .map_err(|_| KeyValueError::LockPoisoned)?;
        shard_manager.delete(ShardId::from(0), &key).await?;

        Ok(())
    }

    /// Get metadata about all users with tenant permissions.
    ///
    /// # What it does
    /// Returns an iterator over all tenant user metadata records.
    /// Will not return a user if that user does not have the
    /// [`Permission::TenantUser`] on [`ResourceScope::Tenant`].
    ///
    /// # How it works
    /// Reads from system_metacache and returns all cached user records.
    ///
    /// # Access Control
    /// - Requires [`Permission::TenantSecurityManage`] on [`ResourceScope::Tenant`]
    ///
    /// # Parameters
    /// - `principal`: [`SecurityPrincipal`] for authorization
    /// - `tenant`: [`TenantId`] for tenant scope
    ///
    /// # Returns
    /// An iterator over [`UserMetadata`] for all tenant users
    ///
    /// # TODO
    /// - Return TenantUserMetadata instead of UserMetadata
    /// - Implement pagination support for large numbers of users
    /// - Add disk fallback if not in cache
    #[tracing::instrument(skip(self))]
    pub async fn get_tenant_users(
        &self,
        principal: &SecurityPrincipal,
        tenant: &TenantId,
    ) -> KeyValueResult<impl IntoIterator<Item = TenantUserMetadata>> {
        if !principal.has_permission(&Permission::TenantSecurityManage) {
            return Err(KeyValueError::PermissionDenied {
                user_id: principal.user_id,
                permission: Permission::TenantSecurityManage,
                resource: ResourceScope::Tenant(tenant.clone()),
            });
        }

        Ok(Vec::default())
    }

    /// Get metadata for a specific user with permissions to the specified tenant.
    ///
    /// # What it does
    /// Retrieves metadata for a single user by ID, checking cache first then disk.
    /// Will not return a user if that user does not have the
    /// [`Permission::TenantUser`] on [`ResourceScope::Tenant`].
    ///
    /// # How it works
    /// 1. Checks system_metacache for a cached user record
    /// 2. If not in cache, reads from system shard using SystemKeys::user_key
    /// 3. Deserializes the stored metadata
    /// 4. Updates cache with the retrieved metadata
    ///
    /// # Access Control
    /// - Requires [`Permission::TenantSecurityManage`] on [`ResourceScope::Tenant`]
    ///
    /// # Parameters
    /// - `principal`: [`SecurityPrincipal`] for authorization
    /// - `user`: [`UserId`] to look up
    ///
    /// # Returns
    /// - `Some(UserMetadata)` if user exists
    /// - `None` if user not found
    /// - `Err(KeyValueError)` if lock poisoned or deserialization fails
    ///
    /// # TODO
    /// - Return TenantUserMetadata instead of UserMetadata
    /// - Implement pagination support for large numbers of users
    /// - Add disk fallback if not in cache
    #[tracing::instrument(skip(self))]
    pub async fn get_tenant_user(
        &self,
        principal: &SecurityPrincipal,
        tenant: &TenantId,
        user: &UserId,
    ) -> KeyValueResult<Option<TenantUserMetadata>> {
        if !principal.has_permission(&Permission::TenantSecurityManage) {
            return Err(KeyValueError::PermissionDenied {
                user_id: principal.user_id,
                permission: Permission::TenantSecurityManage,
                resource: ResourceScope::Tenant(tenant.clone()),
            });
        }
        let mut system_cache = self
            .system_metacache
            .write()
            .map_err(|_| KeyValueError::LockPoisoned)?;
        let shard_manager = self
            .shard_manager
            .read()
            .map_err(|_| KeyValueError::LockPoisoned)?;

        let system_user_record =
            if let Some(system_user_record) = system_cache.get_system_user_record(user).cloned() {
                system_user_record
            } else {
                let system_user_key = SystemKeys::system_user_key(*user);
                let system_user_value = shard_manager
                    .get(ShardId::from(0), &system_user_key)
                    .await?;
                if let Some(value) = system_user_value {
                    let system_user_record: SystemUserRecord = deserialize(&value)?;
                    system_cache.set_system_user_record(system_user_record.clone());
                    system_user_record
                } else {
                    return Ok(None);
                }
            };
        let tenant_user_record = if let Some(tenant_user_record) =
            system_cache.get_tenant_user_record(tenant, user).cloned()
        {
            tenant_user_record
        } else {
            let tenant_user_key = SystemKeys::tenant_user_key(*tenant, *user);
            if let Some(value) = shard_manager
                .get(ShardId::from(0), &tenant_user_key)
                .await?
            {
                let tenant_user_record: TenantUserRecord = deserialize(&value)?;
                system_cache.set_tenant_user_record(tenant_user_record.clone());
                tenant_user_record
            } else {
                return Ok(None);
            }
        };

        Ok(Some(TenantUserMetadata::from((
            system_user_record,
            tenant_user_record,
        ))))
    }

    /// Get a tenant users metadata by login name.
    ///
    /// # What it does
    /// Finds a user by their username string. Will not return a user if that user does not have
    /// the [`Permission::TenantUser`] on [`ResourceScope::Tenant`].
    ///
    /// # How it works
    /// Iterates through cached user records to find one matching the login name.
    /// Currently uses the 'name' field as the login identifier.
    ///
    /// # Access Control
    /// - Requires [`Permission::TenantSecurityManage`] on [`ResourceScope::Tenant`]
    ///
    /// # Parameters
    /// - `principal`: [`SecurityPrincipal`] for authorization
    /// - `username`: Username to search for
    ///
    /// # Returns
    /// - `Some(UserMetadata)` if user with that login exists
    /// - `None` if no matching user found
    ///
    /// # TODO
    /// - Return TenantUserMetadata instead of UserMetadata
    /// - Add index for login-based lookups
    /// - Implement disk fallback if not in cache
    #[tracing::instrument(skip(self))]
    pub async fn get_tenant_user_by_username(
        &self,
        principal: &SecurityPrincipal,
        tenant: &TenantId,
        username: &str,
    ) -> KeyValueResult<Option<TenantUserMetadata>> {
        if !principal.has_permission(&Permission::TenantSecurityManage) {
            return Err(KeyValueError::PermissionDenied {
                user_id: principal.user_id,
                permission: Permission::TenantSecurityManage,
                resource: ResourceScope::Tenant(tenant.clone()),
            });
        }
        // TODO: Implement Cache Fetch and Disk Fallback
        Ok(None)
    }

    /// Create a new user with tenant permissions and a tenant profile.
    ///
    /// # What it does
    /// Creates a new user with the provided configuration OR adds tenant permissions to an existing
    /// user. Successfully added tenants will have the [`Permission::TenantUser`] on
    /// [`ResourceScope::Tenant`].
    ///
    /// # How it works
    /// 1. Generates a new [`UserId`]
    /// 2. Creates [`UserMetadata`] with current timestamp, version 1, and empty groups/roles/grants
    /// 3. Stores in system_metacache
    /// 4. Serializes and persists to system shard using [`SystemKeys::user_key`]
    ///
    /// # Access Control
    /// - Requires [`Permission::TenantSecurityManage`] on [`ResourceScope::Tenant`]
    ///
    /// # Parameters
    /// - `principal`: [`SecurityPrincipal`] for authorization
    /// - `config`: [`SystemUserCreate`] containing name, options, and metadata
    ///
    /// # Returns
    /// - `Ok(UserMetadata)` with the created user information
    /// - `Err(KeyValueError)` if lock poisoned or persistence fails
    ///
    /// # TODO
    /// - Determine when to bind a tenant user to a system user vs create a new user
    /// - Generate proper unique user ID instead of hardcoded 0
    /// - Add password hashing support (currently password_hash is None)
    /// - Implement email validation
    #[tracing::instrument(skip(self))]
    pub async fn create_tenant_user(
        &self,
        principal: &SecurityPrincipal,
        tenant: &TenantId,
        config: TenantUserCreate,
    ) -> KeyValueResult<TenantUserMetadata> {
        if !principal.has_permission(&Permission::TenantSecurityManage) {
            return Err(KeyValueError::PermissionDenied {
                user_id: principal.user_id,
                permission: Permission::TenantSecurityManage,
                resource: ResourceScope::Tenant(tenant.clone()),
            });
        }
        let now = Timestamp::now();

        // TODO: First lookup system user and use existing record if it exists.

        // TODO: Generate proper user ID
        let user_id = UserId::from(0);

        // Create System User metadata
        let system_user_record = SystemUserRecord {
            id: user_id,
            username: config.username.clone(),
            version: 1,
            created_at: now,
            last_modified: now,
            groups: Vec::new(),
            roles: Vec::new(),
            grants: Vec::new(), // Start with no permission grants - grant via roles/groups
            enabled: true,
            password_hash: None, // TODO: Add password hashing
            options: config.options.clone(),
            metadata: config.metadata.clone(),
        };

        // Create tenant user metadata
        let tenant_user_record = TenantUserRecord {
            user: user_id,
            tenant: tenant.clone(),
            version: 1,
            created_at: now,
            last_modified: now,
            groups: vec![],
            roles: vec![],
            options: config.options.clone(),
            metadata: config.metadata.clone(),
        };

        // Store in cache
        {
            let mut cache = self
                .system_metacache
                .write()
                .map_err(|_| KeyValueError::LockPoisoned)?;
            cache.set_system_user_record(system_user_record.clone());
            cache.set_tenant_user_record(tenant_user_record.clone());
        }

        // Persist to system shard
        let system_user_key = SystemKeys::system_user_key(user_id);
        let tenant_user_key = SystemKeys::system_user_key(user_id);
        let system_user_value = serialize(&system_user_record)?;
        let tenant_user_value = serialize(&tenant_user_record)?;
        {
            let shard_manager = self
                .shard_manager
                .read()
                .map_err(|_| KeyValueError::LockPoisoned)?;
            shard_manager
                .put(ShardId::from(0), &system_user_key, &system_user_value)
                .await?;
            shard_manager
                .put(ShardId::from(0), &tenant_user_key, &tenant_user_value)
                .await?;
        }
        Ok(TenantUserMetadata::from((
            system_user_record,
            tenant_user_record,
        )))
    }

    /// Update an existing tenant's user metadata.
    ///
    /// # What it does
    /// Updates user's tenant metadata with new values from the config.
    /// Only works on users with [`Permission::TenantUser`] on [`ResourceScope::Tenant`].
    ///
    /// # How it works
    /// 1. Acquires write lock on system_metacache
    /// 2. Retrieves existing user record
    /// 3. Updates name if provided in config
    /// 4. Applies PropertyUpdate operations to options and metadata maps
    /// 5. Increments version and updates last_modified timestamp
    /// 6. Stores updated metadata back to cache
    /// 7. Serializes and persists to system shard
    ///
    /// # Access Control
    /// - Requires [`Permission::TenantSecurityManage`] on [`ResourceScope::Tenant`]
    ///
    /// # Parameters
    /// - `principal`: [`SecurityPrincipal`] for authorization
    /// - `user`: [`UserId`] to update
    /// - `config`: [`SystemUserUpdate`] containing optional name, options, and metadata updates
    ///
    /// # Returns
    /// - `Ok(UserMetadata)` with updated user information
    /// - `Err(KeyValueError::InvalidValue)` if user not found
    /// - `Err(KeyValueError)` if lock poisoned or persistence fails
    ///
    /// # TODO
    /// -
    #[tracing::instrument(skip(self))]
    pub async fn update_tenant_user(
        &self,
        principal: &SecurityPrincipal,
        tenant: &TenantId,
        user: &UserId,
        config: TenantUserUpdate,
    ) -> KeyValueResult<TenantUserMetadata> {
        if !principal.has_permission(&Permission::TenantSecurityManage) {
            return Err(KeyValueError::PermissionDenied {
                user_id: principal.user_id,
                permission: Permission::TenantSecurityManage,
                resource: ResourceScope::Tenant(tenant.clone()),
            });
        }
        let mut cache = self
            .system_metacache
            .write()
            .map_err(|_| KeyValueError::LockPoisoned)?;

        if let Some(mut system_user_record) = cache.get_system_user_record(user).cloned() {
            // Apply option updates
            for opt_update in &config.options {
                match opt_update {
                    PropertyUpdate::Set(key, value) => {
                        system_user_record
                            .options
                            .insert(key.clone(), value.clone());
                    }
                    PropertyUpdate::Clear(key) => {
                        system_user_record.options.remove(key);
                    }
                }
            }

            // Apply metadata updates
            for meta_update in &config.metadata {
                match meta_update {
                    PropertyUpdate::Set(key, value) => {
                        system_user_record
                            .metadata
                            .insert(key.clone(), value.clone());
                    }
                    PropertyUpdate::Clear(key) => {
                        system_user_record.metadata.remove(key);
                    }
                }
            }

            // Update version and timestamp
            system_user_record.version += 1;
            system_user_record.last_modified = Timestamp::now();

            // Store in cache
            cache.set_system_user_record(system_user_record.clone());
            drop(cache);

            // Persist to system shard
            let key = SystemKeys::system_user_key(*user);
            let value = serialize(&system_user_record)?;

            let shard_manager = self
                .shard_manager
                .read()
                .map_err(|_| KeyValueError::LockPoisoned)?;
            shard_manager.put(ShardId::from(0), &key, &value).await?;

            self.get_tenant_user(principal, tenant, user)
                .await?
                .ok_or_else(|| {
                    KeyValueError::Internal(format!(
                        "Failed to find updated user record for user {}",
                        user
                    ))
                })
        } else {
            Err(KeyValueError::InvalidValue(format!(
                "User not found: {:?}",
                user
            )))
        }
    }

    /// Remove a user's tenant access
    ///
    /// # What it does
    /// Removes a users TenantUserMetadata and removes their [`Permission::TenantUser`] on
    /// [`ResourceScope::Tenant`]. If the user has no tenant or system permissions, they are removed
    /// entirely.
    ///
    /// # How it works
    /// 1. Removes user record from system_metacache
    /// 2. Deletes persisted metadata from system shard using SystemKeys::user_key
    ///
    /// # Access Control
    /// - Requires [`Permission::SystemSecurityManage`] on [`ResourceScope::System`]
    ///
    /// # Parameters
    /// - `principal`: [`SecurityPrincipal`] for authorization
    /// - `user`: [`UserId`] to remove
    ///
    /// # Returns
    /// - `Ok(())` if removal succeeds
    /// - `Err(KeyValueError)` if lock poisoned or deletion fails
    ///
    /// # TODO
    /// - Should only remove user if all permissions are revoked
    /// - Add validation to check for user's active sessions
    /// - Implement cascade deletion or transfer of user-owned resources
    #[tracing::instrument(skip(self))]
    pub async fn remove_tenant_user(
        &self,
        principal: &SecurityPrincipal,
        tenant: &TenantId,
        user: &UserId,
    ) -> KeyValueResult<()> {
        if !principal.has_permission(&Permission::TenantSecurityManage) {
            return Err(KeyValueError::PermissionDenied {
                user_id: principal.user_id,
                permission: Permission::TenantSecurityManage,
                resource: ResourceScope::Tenant(tenant.clone()),
            });
        }
        // Remove tenant user from cache
        {
            let mut cache = self
                .system_metacache
                .write()
                .map_err(|_| KeyValueError::LockPoisoned)?;
            cache.clear_tenant_user_record(tenant, user);
        }
        {
            let tenant_user_key = SystemKeys::tenant_user_key(*tenant, *user);
            let shard_manager = self
                .shard_manager
                .read()
                .map_err(|_| KeyValueError::LockPoisoned)?;
            shard_manager
                .delete(ShardId::from(0), &tenant_user_key)
                .await?;
        }

        // TODO: Remove System User if appropriate
        let remove_system_user = false;
        if remove_system_user {
            {
                let mut cache = self
                    .system_metacache
                    .write()
                    .map_err(|_| KeyValueError::LockPoisoned)?;
                cache.clear_system_user_record(user);
            }
            {
                let system_user_key = SystemKeys::tenant_user_key(*tenant, *user);
                let shard_manager = self
                    .shard_manager
                    .read()
                    .map_err(|_| KeyValueError::LockPoisoned)?;
                shard_manager
                    .delete(ShardId::from(0), &system_user_key)
                    .await?;
            }
        }

        Ok(())
    }

    /// TODO: Add Documentation for get_databases
    #[tracing::instrument(skip(self))]
    pub async fn get_databases(
        &self,
        principal: &SecurityPrincipal,
        tenant: &TenantId,
    ) -> KeyValueResult<impl IntoIterator<Item = DatabaseMetadata>> {
        if !principal.has_permission(&Permission::TenantView) {
            return Err(KeyValueError::PermissionDenied {
                user_id: principal.user_id,
                permission: Permission::TenantView,
                resource: ResourceScope::Tenant(tenant.clone()),
            });
        }
        let cache = self
            .system_metacache
            .read()
            .map_err(|_| KeyValueError::LockPoisoned)?;
        let records = cache
            .list_database_records()
            .filter(|d| &d.tenant == tenant)
            .cloned()
            .map(DatabaseMetadata::from)
            .collect::<Vec<_>>();
        Ok(records)
    }

    /// TODO: Add Documentation for get_database
    #[tracing::instrument(skip(self))]
    pub async fn get_database(
        &self,
        principal: &SecurityPrincipal,
        tenant: &TenantId,
        database: &DatabaseId,
    ) -> KeyValueResult<Option<DatabaseMetadata>> {
        if !principal.has_permission(&Permission::TenantView) {
            return Err(KeyValueError::PermissionDenied {
                user_id: principal.user_id,
                permission: Permission::TenantView,
                resource: ResourceScope::Tenant(tenant.clone()),
            });
        }
        // Check cache first
        {
            let cache = self
                .system_metacache
                .read()
                .map_err(|_| KeyValueError::LockPoisoned)?;
            if let Some(database_record) = cache.get_database_record(database) {
                if &database_record.tenant == tenant {
                    return Ok(Some(DatabaseMetadata::from(database_record.clone())));
                }
            }
        }
        {
            // Read from disk
            let key = SystemKeys::database_key(*tenant, *database);
            let shard_manager = self
                .shard_manager
                .read()
                .map_err(|_| KeyValueError::LockPoisoned)?;

            if let Some(value) = shard_manager.get(ShardId::from(0), &key).await? {
                let database_record: DatabaseRecord = deserialize(&value)?;

                // Update cache
                let mut cache = self
                    .system_metacache
                    .write()
                    .map_err(|_| KeyValueError::LockPoisoned)?;
                cache.set_database_record(database_record.clone());

                Ok(Some(DatabaseMetadata::from(database_record)))
            } else {
                Ok(None)
            }
        }
    }

    /// Create a new database for a tenant.
    ///
    /// # What it does
    /// Creates a new database with the provided configuration.
    ///
    /// # How it works
    /// 1. Generates a new [`DatabaseId`]
    /// 2. Creates a root namespace for the database
    /// 3. Creates DatabaseMetadata with current timestamp and version 1
    /// 4. Stores in system_metacache
    /// 5. Serializes and persists to system shard using SystemKeys::database_key
    ///
    /// # Parameters
    /// - `principal`: [`SecurityPrincipal`] for authorization
    /// - `tenant`: [`TenantId`] that will own the database
    /// - `config`: [`DatabaseCreate`] containing name, options, and metadata
    ///
    /// # Returns
    /// - `Ok(DatabaseMetadata)` with the created database information
    /// - `Err(KeyValueError)` if lock poisoned or persistence fails
    ///
    /// # TODO
    /// - Generate proper unique database ID instead of hardcoded 0
    /// - Create root namespace for database properly
    /// - Add database name uniqueness validation within tenant
    #[tracing::instrument(skip(self))]
    pub async fn create_database(
        &self,
        principal: &SecurityPrincipal,
        tenant: &TenantId,
        config: DatabaseCreate,
    ) -> KeyValueResult<DatabaseMetadata> {
        if !principal.has_permission(&Permission::TenantDatabaseCreate) {
            return Err(KeyValueError::PermissionDenied {
                user_id: principal.user_id,
                permission: Permission::TenantDatabaseCreate,
                resource: ResourceScope::Tenant(tenant.clone()),
            });
        }
        let now = Timestamp::now();

        // TODO: Generate proper database ID
        let database_id = DatabaseId::from(0);

        // Create database metadata
        // TODO: Create root namespace for database
        let root_namespace = NamespaceId::from(0);

        let database_record = DatabaseRecord {
            id: database_id,
            tenant: *tenant,
            name: config.name.clone(),
            version: 1,
            created_at: now,
            last_modified: now,
            root_namespace,
            default_tablespace: None,
            options: config.options.clone(),
            metadata: config.metadata.clone(),
        };

        // Store in cache
        {
            let mut cache = self
                .system_metacache
                .write()
                .map_err(|_| KeyValueError::LockPoisoned)?;
            cache.set_database_record(database_record.clone());
        }

        // Persist to system shard
        let key = SystemKeys::database_key(*tenant, database_id);
        let value = crate::utility::serialize(&database_record)?;

        let shard_manager = self
            .shard_manager
            .read()
            .map_err(|_| KeyValueError::LockPoisoned)?;
        shard_manager.put(ShardId::from(0), &key, &value).await?;

        Ok(DatabaseMetadata::from(database_record))
    }

    /// Update an existing database's metadata.
    ///
    /// # What it does
    /// Updates database metadata with new values from the config.
    /// Validates that the database belongs to the specified tenant.
    ///
    /// # How it works
    /// 1. Acquires write lock on system_metacache
    /// 2. Retrieves existing database record
    /// 3. Verifies database belongs to the specified tenant
    /// 4. Updates name if provided in config
    /// 5. Applies PropertyUpdate operations to options and metadata maps
    /// 6. Increments version and updates last_modified timestamp
    /// 7. Stores updated metadata back to cache
    /// 8. Serializes and persists to system shard
    ///
    /// # Parameters
    /// - `principal`: [`SecurityPrincipal`] for authorization
    /// - `tenant`: [`TenantId`] that should own the database
    /// - `database`: [`DatabaseId`] to update
    /// - `config`: [`DatabaseUpdate`] containing optional name, options, and metadata updates
    ///
    /// # Returns
    /// - `Ok(DatabaseMetadata)` with updated database information
    /// - `Err(KeyValueError::InvalidValue)` if database not found or belongs to different tenant
    /// - `Err(KeyValueError)` if lock poisoned or persistence fails
    #[tracing::instrument(skip(self))]
    pub async fn update_database(
        &self,
        principal: &SecurityPrincipal,
        tenant: &TenantId,
        database: &DatabaseId,
        config: DatabaseUpdate,
    ) -> KeyValueResult<DatabaseMetadata> {
        if !principal.has_permission(&Permission::DatabaseConfigManage) {
            return Err(KeyValueError::PermissionDenied {
                user_id: principal.user_id,
                permission: Permission::DatabaseConfigManage,
                resource: ResourceScope::Database(database.clone()),
            });
        }
        let mut cache = self
            .system_metacache
            .write()
            .map_err(|_| KeyValueError::LockPoisoned)?;

        if let Some(mut database_record) = cache.get_database_record(database).cloned() {
            // Verify tenant matches
            if &database_record.tenant != tenant {
                return Err(KeyValueError::InvalidValue(format!(
                    "Database {:?} does not belong to tenant {:?}",
                    database, tenant
                )));
            }

            // Update fields
            if let Some(name) = config.name {
                database_record.name = name;
            }

            // Apply option updates
            for opt_update in &config.options {
                match opt_update {
                    PropertyUpdate::Set(key, value) => {
                        database_record.options.insert(key.clone(), value.clone());
                    }
                    PropertyUpdate::Clear(key) => {
                        database_record.options.remove(key);
                    }
                }
            }

            // Apply metadata updates
            for meta_update in &config.metadata {
                match meta_update {
                    PropertyUpdate::Set(key, value) => {
                        database_record.metadata.insert(key.clone(), value.clone());
                    }
                    PropertyUpdate::Clear(key) => {
                        database_record.metadata.remove(key);
                    }
                }
            }

            // Update version and timestamp
            database_record.version += 1;
            database_record.last_modified = Timestamp::now();

            // Store in cache
            cache.set_database_record(database_record.clone());
            drop(cache);

            // Persist to system shard
            let key = SystemKeys::database_key(*tenant, *database);
            let value = crate::utility::serialize(&database_record)?;

            let shard_manager = self
                .shard_manager
                .read()
                .map_err(|_| KeyValueError::LockPoisoned)?;
            shard_manager.put(ShardId::from(0), &key, &value).await?;

            Ok(DatabaseMetadata::from(database_record))
        } else {
            Err(KeyValueError::InvalidValue(format!(
                "Database not found: {:?}",
                database
            )))
        }
    }

    /// Delete a database.
    ///
    /// # What it does
    /// Deletes a database and its metadata.
    ///
    /// # How it works
    /// 1. Removes database record from system_metacache
    /// 2. Deletes persisted metadata from system shard using SystemKeys::database_key
    ///
    /// # Parameters
    /// - `principal`: [`SecurityPrincipal`] for authorization
    /// - `tenant`: [`TenantId`] that owns the database
    /// - `database`: [`DatabaseId`] to delete
    ///
    /// # Returns
    /// - `Ok(())` if deletion succeeds
    /// - `Err(KeyValueError)` if lock poisoned or deletion fails
    ///
    /// # TODO
    /// - Check if any tables/namespaces exist for this database
    /// - Prevent deletion if objects exist
    /// - Implement cascade deletion or require explicit cleanup first
    #[tracing::instrument(skip(self))]
    pub async fn delete_database(
        &self,
        principal: &SecurityPrincipal,
        tenant: &TenantId,
        database: &DatabaseId,
    ) -> KeyValueResult<()> {
        if !principal.has_permission(&Permission::TenantDatabaseDelete) {
            return Err(KeyValueError::PermissionDenied {
                user_id: principal.user_id,
                permission: Permission::TenantDatabaseDelete,
                resource: ResourceScope::Tenant(tenant.clone()),
            });
        }
        // Remove from cache
        {
            let mut cache = self
                .system_metacache
                .write()
                .map_err(|_| KeyValueError::LockPoisoned)?;
            cache.clear_database_record(database);
        }

        // Delete from system shard
        let key = SystemKeys::database_key(*tenant, *database);

        let shard_manager = self
            .shard_manager
            .read()
            .map_err(|_| KeyValueError::LockPoisoned)?;
        shard_manager.delete(ShardId::from(0), &key).await?;

        Ok(())
    }

    // TODO: Do we need container methods here

    /**********************************************************************************************\
     * Tablespace Management                                                                      *
    \**********************************************************************************************/

    /// TODO: Add Documentation for get_tablespaces
    #[tracing::instrument(skip(self))]
    pub async fn get_tablespaces(
        &self,
        principal: &SecurityPrincipal,
    ) -> KeyValueResult<impl IntoIterator<Item = TablespaceRecord>> {
        if !principal.has_permission(&Permission::SystemConfigManage) {
            return Err(KeyValueError::PermissionDenied {
                user_id: principal.user_id,
                permission: Permission::SystemConfigManage,
                resource: ResourceScope::System,
            });
        }
        let cache = self.system_metacache.read().unwrap();
        let tablespaces: Vec<TablespaceRecord> = cache.list_tablespace_records().cloned().collect();
        Ok(tablespaces)
    }

    /// Get metadata for a specific tablespace.
    ///
    /// # What it does
    /// Retrieves metadata for a single tablespace by ID from cache.
    ///
    /// # How it works
    /// Reads from system_metacache to find the tablespace record.
    ///
    /// # Parameters
    /// - `principal`: [`SecurityPrincipal`] for authorization
    /// - `tablespace`: [`TablespaceId`] to look up
    ///
    /// # Returns
    /// - `Some(TablespaceMetadata)` if tablespace exists
    /// - `None` if tablespace not found
    ///
    /// # TODO
    /// - Add disk fallback if not in cache
    #[tracing::instrument(skip(self))]
    pub async fn get_tablespace(
        &self,
        principal: &SecurityPrincipal,
        tablespace: &TablespaceId,
    ) -> KeyValueResult<Option<TablespaceRecord>> {
        if !principal.has_permission(&Permission::SystemConfigManage) {
            return Err(KeyValueError::PermissionDenied {
                user_id: principal.user_id,
                permission: Permission::SystemConfigManage,
                resource: ResourceScope::System,
            });
        }
        let cache = self.system_metacache.read().unwrap();
        Ok(cache.get_tablespace_record(tablespace).cloned())
    }

    /// Get tablespace ID by name.
    ///
    /// # What it does
    /// Finds a tablespace by its name string and returns its ID.
    ///
    /// # How it works
    /// Iterates through cached tablespace records to find one matching the name.
    ///
    /// # Parameters
    /// - `principal`: [`SecurityPrincipal`] for authorization
    /// - `name`: Tablespace name to search for
    ///
    /// # Returns
    /// - `Some(TablespaceId)` if tablespace with that name exists
    /// - `None` if no matching tablespace found
    ///
    /// # TODO
    /// - Add index for name-based lookups
    /// - Implement disk fallback if not in cache
    #[tracing::instrument(skip(self))]
    pub async fn get_tablespace_by_name(
        &self,
        principal: &SecurityPrincipal,
        name: &str,
    ) -> KeyValueResult<Option<TablespaceId>> {
        if !principal.has_permission(&Permission::SystemConfigManage) {
            return Err(KeyValueError::PermissionDenied {
                user_id: principal.user_id,
                permission: Permission::SystemConfigManage,
                resource: ResourceScope::System,
            });
        }
        let cache = self
            .system_metacache
            .read()
            .map_err(|_| KeyValueError::LockPoisoned)?;
        let tablespaces = cache
            .list_tablespace_records()
            .find(|t| t.name == name)
            .map(|t| t.id);
        Ok(tablespaces)
    }

    /// Create a new tablespace.
    ///
    /// # What it does
    /// Creates a new tablespace with the provided configuration for storing table data.
    ///
    /// # How it works
    /// 1. Generates a new TablespaceId (currently hardcoded to 0)
    /// 2. Creates TablespaceMetadata with current timestamp and version 1
    /// 3. Stores in system_metacache
    ///
    /// # Parameters
    /// - `principal`: [`SecurityPrincipal`] for authorization
    /// - `config`: [`TablespaceCreate`] containing name, storage_path, tier, options, and metadata
    ///
    /// # Returns
    /// - `Ok(TablespaceMetadata)` with the created tablespace information
    /// - `Err(KeyValueError)` if lock poisoned
    ///
    /// # TODO
    /// - Get/Create actual new tablespace ID instead of hardcoded 0
    /// - Persist to system shard via Raft if in distributed mode
    /// - Update shard manager's path resolver with new tablespace config
    /// - Validate storage_path exists and is writable
    #[tracing::instrument(skip(self))]
    pub async fn create_tablespace(
        &self,
        principal: &SecurityPrincipal,
        config: TablespaceCreate,
    ) -> KeyValueResult<TablespaceRecord> {
        if !principal.has_permission(&Permission::SystemConfigManage) {
            return Err(KeyValueError::PermissionDenied {
                user_id: principal.user_id,
                permission: Permission::SystemConfigManage,
                resource: ResourceScope::System,
            });
        }
        let now = Timestamp::now();

        // TODO: Get/Create actual new tablespace ID
        let tablespace_id = TablespaceId::new(0);

        // Create tablespace metadata
        let tablespace = TablespaceRecord {
            id: tablespace_id,
            name: config.name.clone(),
            storage_path: config.storage_path.clone(),
            tier: config.tier.clone(),
            version: 1,
            created_at: now,
            last_modified: now,
            options: config.options.clone(),
            metadata: config.metadata.clone(),
        };

        // Store in cache
        let mut cache = self.system_metacache.write().unwrap();
        cache.set_tablespace_record(tablespace.clone());

        // TODO: Persist to system shard via Raft if in distributed mode
        // TODO: Update shard manager's path resolver with new tablespace config

        Ok(tablespace)
    }

    /// Update an existing tablespace's metadata.
    ///
    /// # What it does
    /// Updates tablespace metadata with new values from the config.
    ///
    /// # How it works
    /// 1. Acquires write lock on system_metacache
    /// 2. Retrieves existing tablespace record
    /// 3. Updates name, storage_path, and tier if provided in config
    /// 4. Increments version and updates last_modified timestamp
    /// 5. Stores updated metadata back to cache
    ///
    /// # Parameters
    /// - `principal`: [`SecurityPrincipal`] for authorization
    /// - `tablespace`: [`TablespaceId`] to update
    /// - `config`: [`TablespaceUpdate`] containing optional name, storage_path, tier updates
    ///
    /// # Returns
    /// - `Ok(TablespaceMetadata)` with updated tablespace information
    /// - `Err(KeyValueError::InvalidValue)` if tablespace not found
    ///
    /// # TODO
    /// - Persist to system shard via Raft if in distributed mode
    /// - Update shard manager's path resolver with updated tablespace config
    /// - Validate new storage_path if changed
    #[tracing::instrument(skip(self))]
    pub async fn update_tablespace(
        &self,
        principal: &SecurityPrincipal,
        tablespace: &TablespaceId,
        config: TablespaceUpdate,
    ) -> KeyValueResult<TablespaceRecord> {
        if !principal.has_permission(&Permission::SystemConfigManage) {
            return Err(KeyValueError::PermissionDenied {
                user_id: principal.user_id,
                permission: Permission::SystemConfigManage,
                resource: ResourceScope::System,
            });
        }
        let mut cache = self.system_metacache.write().unwrap();

        if let Some(mut tablespace) = cache.get_tablespace_record(&tablespace).cloned() {
            // Update fields
            if let Some(name) = config.name {
                tablespace.name = name;
            }
            if let Some(storage_path) = config.storage_path {
                tablespace.storage_path = storage_path;
            }
            if let Some(tier) = config.tier {
                tablespace.tier = tier;
            }

            // Update version and timestamp
            tablespace.version += 1;
            tablespace.last_modified = Timestamp::now();

            cache.set_tablespace_record(tablespace.clone());

            // TODO: Persist to system shard via Raft if in distributed mode
            // TODO: Update shard manager's path resolver with updated tablespace config

            Ok(tablespace)
        } else {
            Err(KeyValueError::InvalidValue(format!(
                "Tablespace not found: {:?}",
                tablespace
            )))
        }
    }

    /// Delete a tablespace.
    ///
    /// # What it does
    /// Deletes a tablespace and its metadata.
    ///
    /// # How it works
    /// Removes tablespace record from system_metacache.
    ///
    /// # Parameters
    /// - `principal`: [`SecurityPrincipal`] for authorization
    /// - `tablespace`: [`TablespaceId`] to delete
    ///
    /// # Returns
    /// - `Ok(())` if deletion succeeds
    ///
    /// # TODO
    /// - Check if any tables are using this tablespace
    /// - Prevent deletion if tables exist in this tablespace
    /// - Persist deletion to system shard via Raft if in distributed mode
    /// - Update shard manager's path resolver to remove tablespace config
    #[tracing::instrument(skip(self))]
    pub async fn delete_tablespace(
        &self,
        principal: &SecurityPrincipal,
        tablespace: &TablespaceId,
    ) -> KeyValueResult<()> {
        if !principal.has_permission(&Permission::SystemConfigManage) {
            return Err(KeyValueError::PermissionDenied {
                user_id: principal.user_id,
                permission: Permission::SystemConfigManage,
                resource: ResourceScope::System,
            });
        }
        let mut cache = self.system_metacache.write().unwrap();
        cache.clear_tablespace_record(tablespace);

        // TODO: Persist deletion to system shard via Raft if in distributed mode
        // TODO: Update shard manager's path resolver to remove tablespace config

        Ok(())
    }

    /**********************************************************************************************\
     * Database Management                                                                        *
    \**********************************************************************************************/

    /// TODO: Add Documentation for get_objects_by_namespace
    #[tracing::instrument(skip(self))]
    pub async fn get_objects_by_namespace(
        &self,
        principal: &SecurityPrincipal,
        container: &ContainerId,
        namespace: &NamespaceId,
    ) -> KeyValueResult<impl IntoIterator<Item = (ObjectId, ObjectType, ObjectMetadata)>> {
        if !principal.has_permission(&Permission::NamespaceObjectView) {
            return Err(KeyValueError::PermissionDenied {
                user_id: principal.user_id,
                permission: Permission::NamespaceObjectView,
                resource: ResourceScope::Namespace(namespace.clone()),
            });
        }
        // Get container metadata cache
        let container_caches = self
            .container_metacaches
            .read()
            .map_err(|_| KeyValueError::LockPoisoned)?;

        let container_cache = container_caches.get(&container.database()).ok_or_else(|| {
            KeyValueError::InvalidValue(format!("Container not found: {:?}", container))
        })?;

        let _cache = container_cache
            .read()
            .map_err(|_| KeyValueError::LockPoisoned)?;

        // TODO: Implement proper object enumeration when namespace hierarchy is finalized
        // TODO: Use NameResolver, For now, return empty list as placeholder
        let objects: Vec<(ObjectId, ObjectType, ObjectMetadata)> = Vec::new();

        Ok(objects)
    }

    /// Get all namespaces in a container.
    ///
    /// # What it does
    /// Returns an iterator over all namespace metadata records for the specified container.
    ///
    /// # How it works
    /// 1. Retrieves container metadata cache from container_metacaches map
    /// 2. Returns all namespace records from the container cache
    ///
    /// # Parameters
    /// - `principal`: [`SecurityPrincipal`] for authorization
    /// - `container`: [`ContainerId`] to get namespaces from
    ///
    /// # Returns
    /// An iterator over NamespaceMetadata for all namespaces in the container
    ///
    /// # Errors
    /// - `Err(KeyValueError::InvalidValue)` if container not found
    /// - `Err(KeyValueError::LockPoisoned)` if lock poisoned
    #[tracing::instrument(skip(self))]
    pub async fn get_namespaces(
        &self,
        principal: &SecurityPrincipal,
        container: &ContainerId,
    ) -> KeyValueResult<impl IntoIterator<Item = NamespaceRecord>> {
        // TODO: Fix Permissions
        if !principal.has_permission(&Permission::NamespaceObjectView) {
            return Err(KeyValueError::PermissionDenied {
                user_id: principal.user_id,
                permission: Permission::NamespaceObjectView,
                resource: ResourceScope::Namespace(namespace.clone()),
            });
        }
        // Get container metadata cache
        let container_caches = self
            .container_metacaches
            .read()
            .map_err(|_| KeyValueError::LockPoisoned)?;

        let container_cache = container_caches.get(&container.database()).ok_or_else(|| {
            KeyValueError::InvalidValue(format!("Container not found: {:?}", container))
        })?;

        let cache = container_cache
            .read()
            .map_err(|_| KeyValueError::LockPoisoned)?;

        // Return all namespaces from cache
        let namespaces: Vec<NamespaceRecord> = cache.list_namespace_records().cloned().collect();

        Ok(namespaces)
    }

    /// Get namespaces matching a name or path prefix.
    ///
    /// # What it does
    /// Returns namespaces whose name or path starts with the given prefix.
    ///
    /// # How it works
    /// 1. Retrieves container metadata cache
    /// 2. Filters namespace records by checking if name or path starts with prefix
    ///
    /// # Parameters
    /// - `principal`: [`SecurityPrincipal`] for authorization
    /// - `container`: [`ContainerId`] to search in
    /// - `prefix`: String prefix to match against namespace names and paths
    ///
    /// # Returns
    /// An iterator over NamespaceMetadata for matching namespaces
    ///
    /// # Errors
    /// - `Err(KeyValueError::InvalidValue)` if container not found
    /// - `Err(KeyValueError::LockPoisoned)` if lock poisoned
    #[tracing::instrument(skip(self))]
    pub async fn get_namespaces_by_prefix(
        &self,
        principal: &SecurityPrincipal,
        container: &ContainerId,
        prefix: &str,
    ) -> KeyValueResult<impl IntoIterator<Item = NamespaceRecord>> {
        // Get container metadata cache
        let container_caches = self
            .container_metacaches
            .read()
            .map_err(|_| KeyValueError::LockPoisoned)?;

        let container_cache = container_caches.get(&container.database()).ok_or_else(|| {
            KeyValueError::InvalidValue(format!("Container not found: {:?}", container))
        })?;

        let cache = container_cache
            .read()
            .map_err(|_| KeyValueError::LockPoisoned)?;

        // Filter namespaces by name or path prefix
        let namespaces: Vec<NamespaceRecord> = cache
            .list_namespace_records()
            .filter(|ns| ns.name.starts_with(prefix) || ns.path.starts_with(prefix))
            .cloned()
            .collect();

        Ok(namespaces)
    }

    /// Get metadata for a specific namespace.
    ///
    /// # What it does
    /// Retrieves metadata for a single namespace by ID from the container cache.
    ///
    /// # How it works
    /// 1. Retrieves container metadata cache
    /// 2. Looks up namespace record by ID
    ///
    /// # Parameters
    /// - `principal`: [`SecurityPrincipal`] for authorization
    /// - `container`: [`ContainerId`] containing the namespace
    /// - `namespace`: [`NamespaceId`] to look up
    ///
    /// # Returns
    /// - `Some(NamespaceMetadata)` if namespace exists
    /// - `None` if namespace not found
    ///
    /// # Errors
    /// - `Err(KeyValueError::InvalidValue)` if container not found
    /// - `Err(KeyValueError::LockPoisoned)` if lock poisoned
    #[tracing::instrument(skip(self))]
    pub async fn get_namespace(
        &self,
        principal: &SecurityPrincipal,
        container: &ContainerId,
        namespace: &NamespaceId,
    ) -> KeyValueResult<Option<NamespaceRecord>> {
        // Get container metadata cache
        let container_caches = self
            .container_metacaches
            .read()
            .map_err(|_| KeyValueError::LockPoisoned)?;

        let container_cache = container_caches.get(&container.database()).ok_or_else(|| {
            KeyValueError::InvalidValue(format!("Container not found: {:?}", container))
        })?;

        let cache = container_cache
            .read()
            .map_err(|_| KeyValueError::LockPoisoned)?;

        // Get namespace from cache
        Ok(cache.get_namespace_record(namespace).cloned())
    }

    /// Get namespace metadata by path.
    ///
    /// # What it does
    /// Finds a namespace by its path string.
    ///
    /// # How it works
    /// 1. Retrieves container metadata cache
    /// 2. Iterates through namespace records to find one matching the path
    ///
    /// # Parameters
    /// - `principal`: [`SecurityPrincipal`] for authorization
    /// - `container`: [`ContainerId`] to search in
    /// - `path`: Namespace path to search for
    ///
    /// # Returns
    /// - `Some(NamespaceMetadata)` if namespace with that path exists
    /// - `None` if no matching namespace found
    ///
    /// # Errors
    /// - `Err(KeyValueError::InvalidValue)` if container not found
    /// - `Err(KeyValueError::LockPoisoned)` if lock poisoned
    #[tracing::instrument(skip(self))]
    pub async fn get_namespace_by_path(
        &self,
        principal: &SecurityPrincipal,
        container: &ContainerId,
        path: &str,
    ) -> KeyValueResult<Option<NamespaceRecord>> {
        // Get container metadata cache
        let container_caches = self
            .container_metacaches
            .read()
            .map_err(|_| KeyValueError::LockPoisoned)?;

        let container_cache = container_caches.get(&container.database()).ok_or_else(|| {
            KeyValueError::InvalidValue(format!("Container not found: {:?}", container))
        })?;

        let cache = container_cache
            .read()
            .map_err(|_| KeyValueError::LockPoisoned)?;

        // Find namespace by path
        let namespaces = cache
            .list_namespace_records()
            .find(|ns| ns.path == path)
            .cloned();

        Ok(namespaces)
    }

    /// Create a new namespace in a container.
    ///
    /// # What it does
    /// Creates a new namespace with the provided configuration.
    ///
    /// # How it works
    /// 1. Retrieves container metadata cache
    /// 2. Generates a new NamespaceId (currently hardcoded to 0)
    /// 3. Creates NamespaceMetadata with current timestamp and version 0
    /// 4. Constructs simple path from name (e.g., "/name")
    /// 5. Stores in container cache
    ///
    /// # Parameters
    /// - `principal`: [`SecurityPrincipal`] for authorization
    /// - `container`: [`ContainerId`] to create namespace in
    /// - `config`: [`NamespaceCreate`] containing name, options, and metadata
    ///
    /// # Returns
    /// - `Ok(NamespaceMetadata)` with the created namespace information
    ///
    /// # Errors
    /// - `Err(KeyValueError::InvalidValue)` if container not found
    /// - `Err(KeyValueError::LockPoisoned)` if lock poisoned
    ///
    /// # TODO
    /// - Figure out best API for namespaces
    /// - Generate proper unique namespace ID instead of hardcoded 0
    /// - Implement proper namespace hierarchy and path construction
    /// - Persist to container metadata shard
    #[tracing::instrument(skip(self))]
    pub async fn create_namespace(
        &self,
        principal: &SecurityPrincipal,
        container: &ContainerId,
        config: NamespaceCreate,
    ) -> KeyValueResult<NamespaceRecord> {
        // Get container metadata cache
        let container_caches = self
            .container_metacaches
            .read()
            .map_err(|_| KeyValueError::LockPoisoned)?;

        let container_cache = container_caches.get(&container.database()).ok_or_else(|| {
            KeyValueError::InvalidValue(format!("Container not found: {:?}", container))
        })?;

        let mut cache = container_cache
            .write()
            .map_err(|_| KeyValueError::LockPoisoned)?;

        // TODO: Generate proper namespace ID
        let namespace_id = NamespaceId::new(0);

        // Create namespace metadata
        let namespace_meta = NamespaceRecord {
            id: namespace_id,
            name: config.name.clone(),
            version: 0,
            path: format!("/{}", config.name), // Simple path for now
            created_at: Timestamp::now(),
            last_modified: Timestamp::now(),
            options: config.options,
            metadata: config.metadata,
        };

        // Add to cache
        cache.set_namespace_record(namespace_meta.clone());

        // TODO: Persist to container metadata shard
        // let container_shard = cache.metadata_shard_id();
        // self.shard_manager.put(container_shard, key, serialized_data).await?;

        Ok(namespace_meta)
    }

    /// TODO: Add Documentation for update_namespace
    #[tracing::instrument(skip(self))]
    pub async fn update_namespace(
        &self,
        principal: &SecurityPrincipal,
        container: &ContainerId,
        namespace: &NamespaceId,
        config: NamespaceUpdate,
    ) -> KeyValueResult<NamespaceRecord> {
        // Get container metadata cache
        let container_caches = self
            .container_metacaches
            .read()
            .map_err(|_| KeyValueError::LockPoisoned)?;

        let container_cache = container_caches.get(&container.database()).ok_or_else(|| {
            KeyValueError::InvalidValue(format!("Container not found: {:?}", container))
        })?;

        let mut cache = container_cache
            .write()
            .map_err(|_| KeyValueError::LockPoisoned)?;

        // Get existing namespace metadata
        let mut namespace_meta =
            cache
                .get_namespace_record(namespace)
                .cloned()
                .ok_or_else(|| {
                    KeyValueError::InvalidValue(format!("Namespace not found: {:?}", namespace))
                })?;

        // Apply updates
        if let Some(name) = config.name {
            namespace_meta.name = name;
        }

        // Apply option updates
        for update in config.options {
            match update {
                PropertyUpdate::Set(key, value) => {
                    namespace_meta.options.insert(key, value);
                }
                PropertyUpdate::Clear(key) => {
                    namespace_meta.options.remove(&key);
                }
            }
        }

        // Apply metadata updates
        for update in config.metadata {
            match update {
                PropertyUpdate::Set(key, value) => {
                    namespace_meta.metadata.insert(key, value);
                }
                PropertyUpdate::Clear(key) => {
                    namespace_meta.metadata.remove(&key);
                }
            }
        }

        // Update version and timestamp
        namespace_meta.version += 1;
        namespace_meta.last_modified = Timestamp::now();

        // Update cache
        cache.set_namespace_record(namespace_meta.clone());

        // TODO: Persist to container metadata shard
        // let container_shard = cache.metadata_shard_id();
        // self.shard_manager.put(container_shard, key, serialized_data).await?;

        Ok(namespace_meta)
    }

    /// TODO: Add Documentation for delete_namespace
    #[tracing::instrument(skip(self))]
    pub async fn delete_namespace(
        &self,
        principal: &SecurityPrincipal,
        container: &ContainerId,
        namespace_id: &NamespaceId,
    ) -> KeyValueResult<()> {
        // Get container metadata cache
        let container_caches = self
            .container_metacaches
            .read()
            .map_err(|_| KeyValueError::LockPoisoned)?;

        let container_cache = container_caches.get(&container.database()).ok_or_else(|| {
            KeyValueError::InvalidValue(format!("Container not found: {:?}", container))
        })?;

        let mut cache = container_cache
            .write()
            .map_err(|_| KeyValueError::LockPoisoned)?;

        // Verify namespace exists
        if cache.get_namespace_record(namespace_id).is_none() {
            return Err(KeyValueError::InvalidValue(format!(
                "Namespace not found: {:?}",
                namespace_id
            )));
        }

        // Remove from cache
        cache.clear_namespace_record(*namespace_id);

        // TODO: Verify no objects exist in this namespace before deletion
        // TODO: Persist deletion to container metadata shard
        // let container_shard = cache.metadata_shard_id();
        // self.shard_manager.delete(container_shard, key).await?;

        Ok(())
    }

    /// Get all tables in a container.
    ///
    /// # What it does
    /// Returns an iterator over all table metadata records for the specified container.
    ///
    /// # How it works
    /// 1. Retrieves container metadata cache from container_metacaches map
    /// 2. Returns all table records from the container cache
    ///
    /// # Parameters
    /// - `principal`: [`SecurityPrincipal`] for authorization
    /// - `container`: [`ContainerId`] to get tables from
    ///
    /// # Returns
    /// An iterator over TableMetadata for all tables in the container
    ///
    /// # Errors
    /// - `Err(KeyValueError::InvalidValue)` if container not found
    /// - `Err(KeyValueError::LockPoisoned)` if lock poisoned
    #[tracing::instrument(skip(self))]
    pub async fn get_tables(
        &self,
        principal: &SecurityPrincipal,
        container: &ContainerId,
    ) -> KeyValueResult<impl IntoIterator<Item = TableRecord>> {
        // Get container metadata cache
        let container_caches = self
            .container_metacaches
            .read()
            .map_err(|_| KeyValueError::LockPoisoned)?;

        let container_cache = container_caches.get(&container.database()).ok_or_else(|| {
            KeyValueError::InvalidValue(format!("Container not found: {:?}", container))
        })?;

        let cache = container_cache
            .read()
            .map_err(|_| KeyValueError::LockPoisoned)?;

        // Return all tables from cache
        let tables: Vec<TableRecord> = cache.list_table_records().cloned().collect();

        Ok(tables)
    }

    /// Get all tables in a specific namespace.
    ///
    /// # What it does
    /// Returns tables that belong to the specified namespace.
    ///
    /// # How it works
    /// 1. Retrieves container metadata cache
    /// 2. Filters table records by checking if path contains namespace identifier
    ///
    /// # Parameters
    /// - `principal`: [`SecurityPrincipal`] for authorization
    /// - `container`: [`ContainerId`] to search in
    /// - `namespace`: [`NamespaceId`] to filter tables by
    ///
    /// # Returns
    /// An iterator over [`TableRecord`] for tables in the namespace
    ///
    /// # Errors
    /// - `Err(KeyValueError::InvalidValue)` if container not found
    /// - `Err(KeyValueError::LockPoisoned)` if lock poisoned
    ///
    /// # TODO
    /// - Implement proper namespace hierarchy when namespace structure is finalized
    /// - Current implementation uses placeholder path matching
    #[tracing::instrument(skip(self))]
    pub async fn get_tables_by_namespace(
        &self,
        principal: &SecurityPrincipal,
        container: &ContainerId,
        namespace: &NamespaceId,
    ) -> KeyValueResult<impl IntoIterator<Item = TableRecord>> {
        // Get container metadata cache
        let container_caches = self
            .container_metacaches
            .read()
            .map_err(|_| KeyValueError::LockPoisoned)?;

        let container_cache = container_caches.get(&container.database()).ok_or_else(|| {
            KeyValueError::InvalidValue(format!("Container not found: {:?}", container))
        })?;

        let cache = container_cache
            .read()
            .map_err(|_| KeyValueError::LockPoisoned)?;

        // Filter tables by namespace path
        // TODO: Implement proper namespace hierarchy when namespace structure is finalized
        let tables: Vec<TableRecord> = cache
            .list_table_records()
            .filter(|table| {
                // For now, match tables whose path starts with the namespace
                // This is a placeholder until proper namespace hierarchy is implemented
                table.path.contains(&format!("ns_{}", namespace.as_u64()))
            })
            .cloned()
            .collect();

        Ok(tables)
    }

    /// Get tables matching a name or path prefix.
    ///
    /// # What it does
    /// Returns tables whose name or path starts with the given prefix.
    ///
    /// # How it works
    /// 1. Retrieves container metadata cache
    /// 2. Filters table records by checking if name or path starts with prefix
    ///
    /// # Parameters
    /// - `principal`: [`SecurityPrincipal`] for authorization
    /// - `container`: [`ContainerId`] to search in
    /// - `prefix`: String prefix to match against table names and paths
    ///
    /// # Returns
    /// An iterator over TableMetadata for matching tables
    ///
    /// # Errors
    /// - `Err(KeyValueError::InvalidValue)` if container not found
    /// - `Err(KeyValueError::LockPoisoned)` if lock poisoned
    #[tracing::instrument(skip(self))]
    pub async fn get_tables_by_prefix(
        &self,
        principal: &SecurityPrincipal,
        container: &ContainerId,
        prefix: &str,
    ) -> KeyValueResult<impl IntoIterator<Item = TableRecord>> {
        // Get container metadata cache
        let container_caches = self
            .container_metacaches
            .read()
            .map_err(|_| KeyValueError::LockPoisoned)?;

        let container_cache = container_caches.get(&container.database()).ok_or_else(|| {
            KeyValueError::InvalidValue(format!("Container not found: {:?}", container))
        })?;

        let cache = container_cache
            .read()
            .map_err(|_| KeyValueError::LockPoisoned)?;

        // Filter tables by name or path prefix
        let tables: Vec<TableRecord> = cache
            .list_table_records()
            .filter(|table| table.name.starts_with(prefix) || table.path.starts_with(prefix))
            .cloned()
            .collect();

        Ok(tables)
    }

    /// Get metadata for a specific table.
    ///
    /// # What it does
    /// Retrieves metadata for a single table by ID from the container cache.
    ///
    /// # How it works
    /// 1. Retrieves container metadata cache
    /// 2. Looks up table record by ID
    ///
    /// # Parameters
    /// - `principal`: [`SecurityPrincipal`] for authorization
    /// - `container`: [`ContainerId`] containing the table
    /// - `table`: [`TableId`] to look up
    ///
    /// # Returns
    /// - `Some(TableMetadata)` if the table exists
    /// - `None` if the table is not found
    ///
    /// # Errors
    /// - `Err(KeyValueError::InvalidValue)` if container is not found
    /// - `Err(KeyValueError::LockPoisoned)` if lock poisoned
    #[tracing::instrument(skip(self))]
    pub async fn get_table(
        &self,
        principal: &SecurityPrincipal,
        container: &ContainerId,
        table: &TableId,
    ) -> KeyValueResult<Option<TableRecord>> {
        // Get container metadata cache
        let container_caches = self
            .container_metacaches
            .read()
            .map_err(|_| KeyValueError::LockPoisoned)?;

        let container_cache = container_caches.get(&container.database()).ok_or_else(|| {
            KeyValueError::InvalidValue(format!("Container not found: {:?}", container))
        })?;

        let cache = container_cache
            .read()
            .map_err(|_| KeyValueError::LockPoisoned)?;

        // Get table from cache
        Ok(cache.get_table_record(table).cloned())
    }

    /// Create a new table in a container.
    ///
    /// # What it does
    /// Creates a new table with the specified sharding configuration. Handles both single-shard
    /// and multi-shard tables with replication in distributed mode.
    ///
    /// # How it works
    /// 1. Generates a new TableId (currently hardcoded to 0)
    /// 2. In distributed mode:
    ///    - For Single sharding: Creates one fully-replicated shard
    ///    - For Multiple sharding: Creates multiple shards with partitioning
    ///    - Uses Raft consensus to coordinate shard creation across nodes
    ///    - Creates ShardMetadata for each shard
    ///    - Stores TableMetadata in container cache
    /// 3. In standalone mode: Not yet implemented
    ///
    /// # Parameters
    /// - `principal`: [`SecurityPrincipal`] for authorization
    /// - `container_id`: [`ContainerId`] to create table in
    /// - `config`: [`TableCreate`] containing name, path, engine_type, sharding_config, options, metadata
    ///
    /// # Returns
    /// - `Ok(TableMetadata)` with the created [`TableId`] (currently hardcoded to 0)
    ///
    /// # Errors
    /// - `Err(KeyValueError::Consensus)` if Raft shard creation fails
    ///
    /// # TODO
    /// - Get/Create actual new table ID instead of hardcoded 0
    /// - Implement proper replica placement strategy (Single Shard should be fully replicated)
    /// - Create shard via metadata Raft group
    /// - Persist shard metadata to raft group, shard manager, and metadata cache
    /// - Add table and shard to the container Cache
    /// - Return actual table ID
    /// - Implement Single-Node Table Creation Logic, possibly refactor
    #[tracing::instrument(skip(self))]
    pub async fn create_table(
        &self,
        principal: &SecurityPrincipal,
        container: &ContainerId,
        config: TableCreate,
    ) -> KeyValueResult<TableRecord> {
        // TODO: Get/Create actual new table ID
        let table_id = TableId::new(0);

        // In distributed mode, coordinate table creation via Raft
        if let Some(router) = &self.raft_router {
            match config.sharding_config {
                // Single Shard Replication
                TableSharding::Single => {
                    // TODO: Implement proper replica placement strategy, Single Shard should be fully replicated
                    let shard_id = ShardId::from_parts(table_id, ShardIndex::new(0));
                    let replicas = vec![router.node_id()];

                    // Populate Shard Metadata
                    let _shard_metadata = ShardRecord {
                        id: shard_id,
                        name: "".to_string(),
                        version: 0,
                        engine_type: config.engine_type.clone(),
                        created_at: Timestamp::now(),
                        last_modified: Default::default(),
                        range: (vec![], vec![0xFF; 32]), // Full key range
                        leader: None,
                        replicas: replicas.clone(),
                        status: Default::default(),
                        term: 0,
                        size_bytes: 0,
                    };

                    // Create Shard on all nodes
                    // TODO: Create shard via metadata Raft group
                    // TODO: Persist shard metadata to raft group, shard manager, and metadata cache
                    router
                        .system_metadata()
                        .create_shard(
                            shard_id,
                            (vec![], vec![0xFF; 32]), // Full key range
                            replicas,
                        )
                        .await
                        .map_err(|e| {
                            KeyValueError::Consensus(format!(
                                "Failed to create shard via Raft: {}",
                                e
                            ))
                        })?;

                    // Add table entry to container metadata cache
                    let table_metadata = TableRecord {
                        id: table_id,
                        name: config.name.to_string(),
                        path: config.path.to_string(),
                        version: 0,
                        created_at: Timestamp::now(),
                        engine_type: config.engine_type,
                        last_modified: Timestamp::now(),
                        sharding: config.sharding_config,
                        options: config.options,
                        metadata: config.metadata,
                    };

                    // TODO: Add table and shard to the container Cache
                    // metadata.add_table(path, table_metadata);
                    // metadata.add_shard(path, shard_metadata);

                    // TODO: Return actual table metadata
                    Ok(table_metadata)
                }
                // Multiple Shards with Partitioning and Replication
                TableSharding::Multiple {
                    shard_count,
                    partitioner,
                    replication_factor,
                } => {
                    // Create shards for this table via Raft
                    for shard_index in 0..shard_count {
                        let shard_id = ShardId::from_parts(table_id, ShardIndex::new(shard_index));

                        // Determine replica nodes for this shard
                        // TODO: Implement proper replica placement strategy
                        let replicas = vec![router.node_id()];

                        // Populate Shard Metadata
                        let _shard_metadata = ShardRecord {
                            id: shard_id,
                            name: "".to_string(),
                            version: 0,
                            engine_type: config.engine_type.clone(),
                            created_at: Timestamp::now(),
                            last_modified: Default::default(),
                            range: (vec![], vec![0xFF; 32]), // Full key range
                            leader: None,
                            replicas: replicas.clone(),
                            status: Default::default(),
                            term: 0,
                            size_bytes: 0,
                        };

                        // Create Shard on replica nodes via metadata Raft group
                        // TODO: Create shard via metadata Raft group
                        // TODO: Persist shard metadata to raft group, shard manager, and metadata cache

                        router
                            .system_metadata()
                            .create_shard(
                                shard_id,
                                (vec![], vec![0xFF; 32]), // Full key range
                                replicas,
                            )
                            .await
                            .map_err(|e| {
                                KeyValueError::Consensus(format!(
                                    "Failed to create shard via Raft: {}",
                                    e
                                ))
                            })?;

                        // metadata.add_shard(path, shard_metadata);
                    }

                    // Add table entry to container metadata cache
                    let table_metadata = TableRecord {
                        id: table_id,
                        name: config.name.to_string(),
                        path: config.path.to_string(),
                        version: 0,
                        created_at: Timestamp::now(),
                        engine_type: config.engine_type,
                        last_modified: Timestamp::now(),
                        sharding: TableSharding::Multiple {
                            shard_count,
                            partitioner,
                            replication_factor,
                        },
                        options: config.options,
                        metadata: config.metadata,
                    };

                    // TODO: Add table and shard to the container Cache
                    // metadata.add_table(path, table_metadata);

                    // TODO: Return actual table ID
                    Ok(table_metadata)
                }
            }
        } else {
            unimplemented!("TODO: Implement Single-Node Table Creation Logic, possibly refactor")
        }
    }

    /// Update an existing table's metadata.
    ///
    /// # What it does
    /// Updates table metadata with new values from the config.
    ///
    /// # How it works
    /// 1. Retrieves container metadata cache
    /// 2. Gets existing table record
    /// 3. Updates name, engine_type, and sharding_config if provided
    /// 4. Applies PropertyUpdate operations to options and metadata maps
    /// 5. Increments version and updates last_modified timestamp
    /// 6. Stores updated metadata back to cache
    ///
    /// # Parameters
    /// - `principal`: [`SecurityPrincipal`] for authorization
    /// - `container`: [`ContainerId`] containing the table
    /// - `table`: [`TableId`] to update
    /// - `config`: [`TableUpdate`] containing optional name, engine_type, sharding_config, options, metadata updates
    ///
    /// # Returns
    /// - `Ok(TableMetadata)` with updated table information
    ///
    /// # Errors
    /// - `Err(KeyValueError::InvalidValue)` if container or table not found
    /// - `Err(KeyValueError::LockPoisoned)` if lock poisoned
    ///
    /// # TODO
    /// - Persist to container metadata shard
    /// - Handle sharding_config changes (may require data migration)
    #[tracing::instrument(skip(self))]
    pub async fn update_table(
        &self,
        principal: &SecurityPrincipal,
        container: &ContainerId,
        table: &TableId,
        config: TableUpdate,
    ) -> KeyValueResult<TableRecord> {
        // Get container metadata cache
        let container_caches = self
            .container_metacaches
            .read()
            .map_err(|_| KeyValueError::LockPoisoned)?;

        let container_cache = container_caches.get(&container.database()).ok_or_else(|| {
            KeyValueError::InvalidValue(format!("Container not found: {:?}", container))
        })?;

        let mut cache = container_cache
            .write()
            .map_err(|_| KeyValueError::LockPoisoned)?;

        // Get existing table metadata
        let mut table_meta = cache
            .get_table_record(table)
            .cloned()
            .ok_or_else(|| KeyValueError::InvalidValue(format!("Table not found: {:?}", table)))?;

        // Apply updates
        if let Some(name) = config.name {
            table_meta.name = name;
        }
        if let Some(engine_type) = config.engine_type {
            table_meta.engine_type = engine_type;
        }
        if let Some(sharding_config) = config.sharding_config {
            table_meta.sharding = sharding_config;
        }

        // Apply option updates
        for update in config.options {
            match update {
                PropertyUpdate::Set(key, value) => {
                    table_meta.options.insert(key, value);
                }
                PropertyUpdate::Clear(key) => {
                    table_meta.options.remove(&key);
                }
            }
        }

        // Apply metadata updates
        for update in config.metadata {
            match update {
                PropertyUpdate::Set(key, value) => {
                    table_meta.metadata.insert(key, value);
                }
                PropertyUpdate::Clear(key) => {
                    table_meta.metadata.remove(&key);
                }
            }
        }

        // Update version and timestamp
        table_meta.version += 1;
        table_meta.last_modified = Timestamp::now();

        // Update cache
        cache.set_table_record(table_meta.clone());

        // TODO: Persist to container metadata shard
        // let container_shard = cache.metadata_shard_id();
        // self.shard_manager.put(container_shard, key, serialized_data).await?;

        Ok(table_meta)
    }

    /// Delete a table from a container.
    ///
    /// # What it does
    /// Deletes a table and its metadata.
    ///
    /// # How it works
    /// 1. Retrieves container metadata cache
    /// 2. Verifies table exists
    /// 3. Removes table record from cache
    ///
    /// # Parameters
    /// - `principal`: [`SecurityPrincipal`] for authorization
    /// - `container`: [`ContainerId`] containing the table
    /// - `table`: [`TableId`] to delete
    ///
    /// # Returns
    /// - `Ok(())` if deletion succeeds
    ///
    /// # Errors
    /// - `Err(KeyValueError::InvalidValue)` if container or table not found
    /// - `Err(KeyValueError::LockPoisoned)` if lock poisoned
    ///
    /// # TODO
    /// - Delete all shards associated with this table
    /// - Persist deletion to container metadata shard
    /// - Verify table is empty or add force flag
    #[tracing::instrument(skip(self))]
    pub async fn delete_table(
        &self,
        principal: &SecurityPrincipal,
        container: &ContainerId,
        table: &TableId,
    ) -> KeyValueResult<()> {
        // Get container metadata cache
        let container_caches = self
            .container_metacaches
            .read()
            .map_err(|_| KeyValueError::LockPoisoned)?;

        let container_cache = container_caches.get(&container.database()).ok_or_else(|| {
            KeyValueError::InvalidValue(format!("Container not found: {:?}", container))
        })?;

        let mut cache = container_cache
            .write()
            .map_err(|_| KeyValueError::LockPoisoned)?;

        // Verify table exists
        if cache.get_table_record(table).is_none() {
            return Err(KeyValueError::InvalidValue(format!(
                "Table not found: {:?}",
                table
            )));
        }

        // Remove from cache
        cache.clear_table_record(*table);

        // TODO: Delete all shards associated with this table
        // TODO: Persist deletion to container metadata shard
        // let container_shard = cache.metadata_shard_id();
        // self.shard_manager.delete(container_shard, key).await?;

        Ok(())
    }

    /**********************************************************************************************\
     * Data Management                                                                            *
    \**********************************************************************************************/

    /// TODO: Add Documentation for put
    /// TODO: Figure out how to handle distributed mode
    /// TODO: Figure out how to deal with tenants and containers
    #[tracing::instrument(skip(self))]
    pub async fn put(
        &self,
        principal: &SecurityPrincipal,
        container: &ContainerId,
        table: &TableId,
        key: &[u8],
        value: &[u8],
    ) -> KeyValueResult<()> {
        let shard_id = self.get_shard_for_key(container, table, key)?;

        // In distributed mode, coordinate put key via Raft Consensus
        if let Some(router) = &self.raft_router {
            // Route through Raft for distributed consensus
            router
                .put(key.to_vec(), value.to_vec())
                .await
                .map_err(|e| KeyValueError::Consensus(format!("Raft put failed: {}", e)))
        } else {
            // Single-node mode: direct shard access
            let shard_manager = self.shard_manager.read().unwrap();
            shard_manager.put(shard_id, key, value).await
        }
    }

    /// TODO: Add Documentation for get
    #[tracing::instrument(skip(self))]
    pub async fn get(
        &self,
        principal: &SecurityPrincipal,
        container: &ContainerId,
        table: &TableId,
        key: &[u8],
    ) -> KeyValueResult<Option<Vec<u8>>> {
        let shard_id = self.get_shard_for_key(container, table, key)?;

        // In distributed mode, coordinate get key via Raft Consensus
        if let Some(router) = &self.raft_router {
            // Route through Raft for distributed reads
            router
                .get(key)
                .await
                .map_err(|e| KeyValueError::Consensus(format!("Raft get failed: {}", e)))
        } else {
            // Single-node mode: direct shard access
            let shard_manager = self.shard_manager.read().unwrap();
            shard_manager.get(shard_id, key).await
        }
    }

    /// TODO: Add documentation for delete
    #[tracing::instrument(skip(self))]
    pub async fn delete(
        &self,
        principal: &SecurityPrincipal,
        container: &ContainerId,
        table: &TableId,
        key: &[u8],
    ) -> KeyValueResult<bool> {
        let shard_id = self.get_shard_for_key(container, table, key)?;

        // In distributed mode, coordinate delete key via Raft Consensus
        if let Some(router) = &self.raft_router {
            // Route through Raft for distributed deletes
            router
                .delete(key.to_vec())
                .await
                .map_err(|e| KeyValueError::Consensus(format!("Raft delete failed: {}", e)))?;
            Ok(true)
        } else {
            // Single-node mode: direct shard access
            let shard_manager = self.shard_manager.read().unwrap();
            shard_manager.delete(shard_id, key).await
        }
    }

    /// Batch put operations
    #[tracing::instrument(skip(self))]
    pub async fn batch_put(
        &self,
        principal: &SecurityPrincipal,
        container: &ContainerId,
        table: &TableId,
        pairs: &[(&[u8], &[u8])],
    ) -> KeyValueResult<()> {
        // In distributed mode, coordinate batch put key via Raft Consensus
        if let Some(router) = &self.raft_router {
            // Convert to Raft operations
            let operations: Vec<nanograph_raft::Operation> = pairs
                .iter()
                .map(|(k, v)| nanograph_raft::Operation::Put {
                    key: k.to_vec(),
                    value: v.to_vec(),
                })
                .collect();

            return router
                .batch(operations)
                .await
                .map_err(|e| KeyValueError::Consensus(format!("Raft put failed: {}", e)));
        } else {
            // Single-node mode: group by shard and batch
            let mut shard_batches: HashMap<ShardId, Vec<(&[u8], &[u8])>> = HashMap::new();

            for &(key, value) in pairs {
                let shard_id = self.get_shard_for_key(container, table, key)?;
                shard_batches
                    .entry(shard_id)
                    .or_insert_with(Vec::new)
                    .push((key, value));
            }

            let shard_manager = self.shard_manager.read().unwrap();
            for (shard_id, batch) in shard_batches {
                shard_manager.batch_put(shard_id, &batch).await?;
            }

            Ok(())
        }
    }

    /// Calculate which shard a key belongs to for a given table.
    ///
    /// # What it does
    /// Determines the appropriate shard for a key based on the table's sharding configuration.
    ///
    /// # How it works
    /// 1. Retrieves container metadata cache
    /// 2. Gets table metadata to determine sharding strategy
    /// 3. For Single sharding: Always returns shard index 0
    /// 4. For Multiple sharding: Uses the configured partitioner to calculate shard index
    ///    - Partitioner applies hash-based or other algorithm to distribute keys
    ///
    /// # Parameters
    /// - `container`: [`ContainerId`] containing the table
    /// - `table`: [`TableId`] to determine shard for
    /// - `key`: Key bytes to hash/partition
    ///
    /// # Returns
    /// - `Ok(ShardId)` with the calculated shard ID
    ///
    /// # Errors
    /// - `Err(KeyValueError::InvalidValue)` if container or table not found
    /// - `Err(KeyValueError::LockPoisoned)` if lock poisoned
    ///
    /// # TODO
    /// - Make Partitioning algorithm configurable per table
    #[tracing::instrument(skip(self))]
    fn get_shard_for_key(
        &self,
        container: &ContainerId,
        table: &TableId,
        key: &[u8],
    ) -> KeyValueResult<ShardId> {
        // Get container metadata cache
        let container_caches = self
            .container_metacaches
            .read()
            .map_err(|_| KeyValueError::LockPoisoned)?;

        let container_cache = container_caches.get(&container.database()).ok_or_else(|| {
            KeyValueError::InvalidValue(format!("Container not found: {:?}", container))
        })?;

        let cache = container_cache
            .read()
            .map_err(|_| KeyValueError::LockPoisoned)?;

        // Get table metadata to determine sharding strategy
        let table_meta = cache
            .get_table_record(table)
            .ok_or_else(|| KeyValueError::InvalidValue(format!("Table not found: {:?}", table)))?;

        match &table_meta.sharding {
            TableSharding::Single => {
                // Single shard table - always use shard index 0
                Ok(ShardId::from_parts(*table, ShardIndex::new(0)))
            }
            TableSharding::Multiple {
                shard_count,
                partitioner,
                ..
            } => {
                // Multi-shard table - use partitioner's built-in logic
                let shard_index = partitioner.get_shard_index(key, *shard_count);
                Ok(ShardId::from_parts(*table, shard_index))
            }
        }
    }
}
