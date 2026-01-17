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
    ShardCreate, SystemUserRecord, TenantMetadata, TenantUserCreate, TenantUserMetadata,
    TenantUserRecord, TenantUserUpdate,
};
use nanograph_kvt::{KeyValueError, KeyValueResult, KeyValueShardStore};
use std::time::Duration;
use nanograph_core::{
    object::{
        ClusterCreate, ClusterId, ClusterRecord, ClusterUpdate, ContainerId, DatabaseCreate,
        DatabaseId, DatabaseRecord, DatabaseUpdate, NamespaceCreate, NamespaceId, NamespaceRecord,
        NamespaceUpdate, NodeId, ObjectId, ObjectMetadata, ObjectType, Permission, RegionCreate,
        RegionId, RegionRecord, RegionUpdate, SecurityPrincipal, ServerCreate, ServerId,
        ServerRecord, ServerUpdate, ShardId, ShardIndex, ShardRecord, StorageEngineType,
        SystemUserCreate, SystemUserMetadata, SystemUserUpdate, TableCreate, TableId, TableRecord,
        TableSharding, TableUpdate, TablespaceCreate, TablespaceId, TablespaceRecord,
        TablespaceUpdate, TenantCreate, TenantId, TenantRecord, TenantUpdate, UserId,
    },
    types::{PropertyUpdate, Timestamp},
};
use nanograph_raft::ConsensusRouter;
use std::collections::{HashMap, HashSet};
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
    container_metacaches: Arc<RwLock<HashMap<ContainerId, Arc<RwLock<ContainerMetadataCache>>>>>,
    /// Raft router for distributed mode
    raft_router: Option<Arc<ConsensusRouter>>,
}

impl KeyValueDatabaseContext {
    /// Register a storage engine.
    pub fn register_engine(
        &self,
        engine_type: nanograph_kvt::StorageEngineType,
        engine: Arc<dyn nanograph_kvt::KeyValueShardStore>,
    ) -> KeyValueResult<()> {
        let mut shard_manager = self
            .shard_manager
            .write()
            .map_err(|_| KeyValueError::LockPoisoned)?;
        shard_manager.register_engine(engine_type, engine)
    }

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
        let system_metacache = Arc::new(RwLock::new(SystemMetadataCache::new(
            config.node_id,
            ShardId::from(0),
            config.cache_ttl,
        )));
        let container_metacaches = Arc::new(RwLock::new(HashMap::new()));
        Self {
            node_id: config.node_id,
            shard_manager,
            system_metacache,
            container_metacaches,
            raft_router: None,
        }
    }

    /// Bootstrap the standalone context by creating the system shard.
    pub async fn bootstrap_standalone(&self) -> KeyValueResult<()> {
        let mut shard_manager = self.shard_manager.write().unwrap();

        // Register default engines if they are not already registered
        let art_engine = Arc::new(nanograph_art::ArtKeyValueStore::new());
        shard_manager.register_engine(nanograph_kvt::StorageEngineType::new("ART"), art_engine)?;

        // Create system shard (Shard 0)
        let config = nanograph_core::object::ShardCreate::new(
            TableId::from(0),
            ShardIndex::from(0),
            nanograph_kvt::StorageEngineType::new("ART"),
        );
        shard_manager.create_shard(config).await?;

        Ok(())
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
    pub(crate) fn new_distributed(
        config: KeyValueDatabaseConfig,
        raft_router: Arc<ConsensusRouter>,
    ) -> Self {
        let shard_manager = Arc::new(RwLock::new(KeyValueShardManager::new_standalone()));
        let system_metacache = Arc::new(RwLock::new(SystemMetadataCache::new(
            config.node_id,
            ShardId::from(0),
            config.cache_ttl,
        )));
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
    #[tracing::instrument(skip(self))]
    pub async fn initialize_cluster(
        &self,
        principal: &SecurityPrincipal,
        config: ClusterCreate,
    ) -> KeyValueResult<ClusterMetadata> {
        if !principal.has_system_permission(&Permission::ClusterAlter) {
            return Err(KeyValueError::PermissionDenied {
                user_id: principal.user_id,
                permission: Permission::ClusterAlter,
                resource: ResourceScope::System,
            });
        }
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
        if !principal.has_system_permission(&Permission::ClusterView) {
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
    /// - Requires [`Permission::ClusterAlter`] on [`ResourceScope::System`]
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
        if !principal.has_system_permission(&Permission::ClusterAlter) {
            return Err(KeyValueError::PermissionDenied {
                user_id: principal.user_id,
                permission: Permission::ClusterAlter,
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
        if !principal.has_system_permission(&Permission::ClusterView) {
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
    /// - Requires [`Permission::RegionView`] on [`ResourceScope::System`]
    ///
    /// # Parameters
    /// - `principal`: [`SecurityPrincipal`] for authorization
    /// - `region_id`: [`RegionId`] to look up
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
        region_id: RegionId,
    ) -> KeyValueResult<Option<RegionMetadata>> {
        if !principal.has_system_permission(&Permission::ClusterView) {
            return Err(KeyValueError::PermissionDenied {
                user_id: principal.user_id,
                permission: Permission::ClusterView,
                resource: ResourceScope::System,
            });
        }
        let lock = self.system_metacache.write().unwrap();
        let region = lock
            .get_region_record(&region_id)
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
    /// - Requires [`Permission::ClusterAlter`] on [`ResourceScope::System`]
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
        if !principal.has_permission(&Permission::ClusterAlter) {
            return Err(KeyValueError::PermissionDenied {
                user_id: principal.user_id,
                permission: Permission::ClusterAlter,
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

    /// Update an existing region's metadata.
    ///
    /// # What it does
    /// Updates region properties such as name based on the provided configuration.
    ///
    /// # How it works
    /// 1. Acquires write lock on system_metacache
    /// 2. Retrieves existing region record
    /// 3. Updates name if present in config
    /// 4. Increments version and updates last_modified timestamp
    /// 5. Stores updated record in cache
    /// 6. Persists to system shard (ShardId 0) using [`SystemKeys::region_key`]
    ///
    /// # Access Control
    /// - Requires [`Permission::ClusterAlter`] on [`ResourceScope::System`]
    ///
    /// # Parameters
    /// - `principal`: [`SecurityPrincipal`] for authorization
    /// - `region`: [`RegionId`] of the region to update
    /// - `config`: [`RegionUpdate`] containing fields to update
    ///
    /// # Returns
    /// - `Ok(RegionMetadata)` with the updated region information
    /// - `Err(KeyValueError::InvalidValue)` if region not found
    /// - `Err(KeyValueError)` if lock poisoned or persistence fails
    #[tracing::instrument(skip(self))]
    pub async fn update_region(
        &self,
        principal: &SecurityPrincipal,
        region_id: &RegionId,
        config: RegionUpdate,
    ) -> KeyValueResult<RegionMetadata> {
        if !principal.has_system_permission(&Permission::ClusterAlter) {
            return Err(KeyValueError::PermissionDenied {
                user_id: principal.user_id,
                permission: Permission::ClusterAlter,
                resource: ResourceScope::System,
            });
        }
        let mut cache = self
            .system_metacache
            .write()
            .map_err(|_| KeyValueError::LockPoisoned)?;

        if let Some(mut region_record) = cache.get_region_record(region_id).cloned() {
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
            let key = SystemKeys::region_key(self.node_id.cluster_id(), *region_id);
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
                region_id
            )))
        }
    }

    /// Remove a region from the cluster.
    ///
    /// # What it does
    /// Deletes a region record from the cache and persistent storage.
    ///
    /// # How it works
    /// 1. Acquires write lock on system_metacache
    /// 2. Clears the region record from cache
    /// 3. Generates storage key using [`SystemKeys::region_key`]
    /// 4. Deletes the record from the system shard (ShardId 0) via shard manager
    ///
    /// # Access Control
    /// - Requires [`Permission::ClusterAlter`] on [`ResourceScope::System`]
    ///
    /// # Parameters
    /// - `principal`: [`SecurityPrincipal`] for authorization
    /// - `region_id`: [`RegionId`] of the region to remove
    ///
    /// # Returns
    /// - `Ok(())` if removal succeeds
    /// - `Err(KeyValueError)` if lock poisoned or deletion fails
    #[tracing::instrument(skip(self))]
    pub async fn remove_region(
        &self,
        principal: &SecurityPrincipal,
        region_id: &RegionId,
    ) -> KeyValueResult<()> {
        // Check permissions
        if !principal.has_system_permission(&Permission::ClusterAlter) {
            return Err(KeyValueError::PermissionDenied {
                user_id: principal.user_id,
                permission: Permission::ClusterAlter,
                resource: ResourceScope::System,
            });
        }
        // Remove from cache
        {
            let mut cache = self
                .system_metacache
                .write()
                .map_err(|_| KeyValueError::LockPoisoned)?;
            cache.clear_region_record(region_id);
        }

        // Generate Key
        let key = SystemKeys::region_key(self.node_id.cluster_id(), *region_id);

        // Delete record from system shard
        let shard_manager = self
            .shard_manager
            .read()
            .map_err(|_| KeyValueError::LockPoisoned)?;
        shard_manager.delete(ShardId::from(0), &key).await?;

        Ok(())
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
    ///
    /// # TODO
    /// - Implement proper server enumeration from disk if not in cache
    /// - Add pagination support for large numbers of servers
    #[tracing::instrument(skip(self))]
    pub async fn get_servers(
        &self,
        principal: &SecurityPrincipal,
    ) -> KeyValueResult<impl IntoIterator<Item = ServerMetadata>> {
        if !principal.has_system_permission(&Permission::ClusterView) {
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
    /// - `region_id`: [`RegionId`] to filter servers by
    ///
    /// # Returns
    /// An iterator over [`ServerMetadata`] for matching servers
    ///
    /// # TODO
    /// - Implement proper server enumeration from disk if not in cache
    #[tracing::instrument(skip(self))]
    pub async fn get_servers_by_region(
        &self,
        principal: &SecurityPrincipal,
        region_id: &RegionId,
    ) -> KeyValueResult<impl IntoIterator<Item = NodeId>> {
        if !principal.has_permission(&Permission::ClusterView) {
            return Err(KeyValueError::PermissionDenied {
                user_id: principal.user_id,
                permission: Permission::ClusterView,
                resource: ResourceScope::System,
            });
        }
        let mut result = HashSet::new();
        {
            // TODO: Implement Better Get Server By Region Logic (possibly an extra in memory index)
            let lock = self.system_metacache.read().unwrap();
            let servers = lock
                .list_server_records()
                .filter(|record| record.id.region_id() == *region_id)
                .cloned()
                .map(|record| record.id)
                .collect::<Vec<_>>();
            result.extend(servers);
        }
        {
            // TODO: Read from disk index
        }
        Ok(result)
    }

    /// Get metadata about a specific server.
    ///
    /// # What it does
    /// Retrieves metadata for a single server by its node identifier.
    ///
    /// # How it works
    /// 1. Checks system_metacache for the server record
    /// 2. If not found, generates storage key via [`SystemKeys::server_key`]
    /// 3. Reads from system shard (ShardId 0)
    /// 4. Deserializes and updates cache if found
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
    /// - `Err(KeyValueError)` if lock poisoned or disk read fails
    #[tracing::instrument(skip(self))]
    pub async fn get_server(
        &self,
        principal: &SecurityPrincipal,
        node_id: &NodeId,
    ) -> KeyValueResult<Option<ServerMetadata>> {
        if !principal.has_system_permission(&Permission::ClusterView) {
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
                .write()
                .map_err(|_| KeyValueError::LockPoisoned)?;
            if let Some(server_meta) = cache.get_server_record(&node_id) {
                return Ok(Some(ServerMetadata::from(server_meta.clone())));
            }
        }
        {
            // Read from disk
            let key = SystemKeys::server_key(
                self.node_id.cluster_id(),
                node_id.region_id(),
                node_id.server_id(),
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
    }

    /// Add a new server to the cluster.
    ///
    /// # What it does
    /// Registers a new server with the provided configuration.
    ///
    /// # How it works
    /// 1. Generates a new [`ServerId`]
    /// 2. Creates [`ServerRecord`] with current timestamp and version 1
    /// 3. Stores in system_metacache
    /// 4. Serializes and persists to system shard using [`SystemKeys::server_key`]
    ///
    /// # Access Control
    /// - Requires [`Permission::ClusterAlter`] on [`ResourceScope::System`]
    ///
    /// # Parameters
    /// - `principal`: [`SecurityPrincipal`] for authorization
    /// - `config`: [`ServerCreate`] containing region and server details
    ///
    /// # Returns
    /// - `Ok(ServerMetadata)` with the created server information
    /// - `Err(KeyValueError)` if lock poisoned or persistence fails
    ///
    /// # TODO
    /// - Generate proper unique server ID instead of hardcoded 0
    #[tracing::instrument(skip(self))]
    pub async fn add_server(
        &self,
        principal: &SecurityPrincipal,
        config: ServerCreate,
    ) -> KeyValueResult<ServerMetadata> {
        // Check Permissions
        if !principal.has_system_permission(&Permission::ClusterAlter) {
            return Err(KeyValueError::PermissionDenied {
                user_id: principal.user_id,
                permission: Permission::ClusterAlter,
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

    /// Update an existing server's metadata.
    ///
    /// # What it does
    /// Updates server properties such as name based on the provided configuration.
    ///
    /// # How it works
    /// 1. Acquires write lock on system_metacache
    /// 2. Retrieves existing server record
    /// 3. Updates name if present in config
    /// 4. Increments version and updates last_modified timestamp
    /// 5. Stores updated record in cache
    /// 6. Persists to system shard (ShardId 0) using [`SystemKeys::server_key`]
    ///
    /// # Access Control
    /// - Requires [`Permission::ClusterAlter`] on [`ResourceScope::System`]
    ///
    /// # Parameters
    /// - `principal`: [`SecurityPrincipal`] for authorization
    /// - `server`: [`NodeId`] of the server to update
    /// - `config`: [`ServerUpdate`] containing fields to update
    ///
    /// # Returns
    /// - `Ok(ServerMetadata)` with the updated server information
    /// - `Err(KeyValueError::InvalidValue)` if server not found
    /// - `Err(KeyValueError)` if lock poisoned or persistence fails
    #[tracing::instrument(skip(self))]
    pub async fn update_server(
        &self,
        principal: &SecurityPrincipal,
        node_id: &NodeId,
        config: ServerUpdate,
    ) -> KeyValueResult<ServerMetadata> {
        if !principal.has_system_permission(&Permission::ClusterAlter) {
            return Err(KeyValueError::PermissionDenied {
                user_id: principal.user_id,
                permission: Permission::ClusterAlter,
                resource: ResourceScope::System,
            });
        }
        let mut cache = self
            .system_metacache
            .write()
            .map_err(|_| KeyValueError::LockPoisoned)?;

        if let Some(mut server_record) = cache.get_server_record(&node_id).cloned() {
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
                node_id.region_id(),
                node_id.server_id(),
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
                node_id
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
    /// - Requires [`Permission::ClusterAlter`] on [`ResourceScope::System`]
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
        node_id: &NodeId,
    ) -> KeyValueResult<()> {
        if !principal.has_system_permission(&Permission::ClusterAlter) {
            return Err(KeyValueError::PermissionDenied {
                user_id: principal.user_id,
                permission: Permission::ClusterAlter,
                resource: ResourceScope::System,
            });
        }
        // Remove from cache
        {
            let mut cache = self
                .system_metacache
                .write()
                .map_err(|_| KeyValueError::LockPoisoned)?;
            cache.clear_server_record(&node_id);
        }

        // Delete from system shard
        let key =
            SystemKeys::server_key(self.cluster_id(), node_id.region_id(), node_id.server_id());

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
    /// - Requires [`Permission::UserManagement`] on [`ResourceScope::System`]
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
        if !principal.has_system_permission(&Permission::UserManagement) {
            return Err(KeyValueError::PermissionDenied {
                user_id: principal.user_id,
                permission: Permission::UserManagement,
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
    /// - Requires [`Permission::UserManagement`] on [`ResourceScope::System`]
    ///
    /// # Parameters
    /// - `principal`: [`SecurityPrincipal`] for authorization
    /// - `user_id`: [`UserId`] to look up
    ///
    /// # Returns
    /// - `Some(SystemUserMetadata)` if user exists
    /// - `None` if user not found
    /// - `Err(KeyValueError)` if lock poisoned or deserialization fails
    #[tracing::instrument(skip(self))]
    pub async fn get_system_user(
        &self,
        principal: &SecurityPrincipal,
        user_id: &UserId,
    ) -> KeyValueResult<Option<SystemUserMetadata>> {
        if !principal.has_system_permission(&Permission::UserManagement) {
            return Err(KeyValueError::PermissionDenied {
                user_id: principal.user_id,
                permission: Permission::UserManagement,
                resource: ResourceScope::System,
            });
        }
        // Check cache first
        {
            let cache = self
                .system_metacache
                .read()
                .map_err(|_| KeyValueError::LockPoisoned)?;
            if let Some(user_meta) = cache.get_system_user_record(&user_id) {
                return Ok(Some(SystemUserMetadata::from(user_meta.clone())));
            }
        }

        // Read from disk
        let key = SystemKeys::system_user_key(*user_id);
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
    /// - Requires [`Permission::UserManagement`] on [`ResourceScope::System`]
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
    pub async fn get_user_by_username(
        &self,
        principal: &SecurityPrincipal,
        username: &str,
    ) -> KeyValueResult<Option<UserId>> {
        if !principal.has_system_permission(&Permission::UserManagement) {
            return Err(KeyValueError::PermissionDenied {
                user_id: principal.user_id,
                permission: Permission::UserManagement,
                resource: ResourceScope::System,
            });
        }
        {
            // Search in cache by name (UserMetadata doesn't have a login field, using name as identifier)
            let cache = self
                .system_metacache
                .read()
                .map_err(|_| KeyValueError::LockPoisoned)?;
            let user = cache
                .list_system_user_records()
                .find(|u| u.username == username)
                .cloned()
                .map(|u| u.user_id);
            if user.is_some() {
                return Ok(user);
            }
        }
        {
            // Lookup on Disk
        }
        Ok(None)
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
    /// - Requires [`Permission::UserManagement`] on [`ResourceScope::System`]
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
        if !principal.has_system_permission(&Permission::UserManagement) {
            return Err(KeyValueError::PermissionDenied {
                user_id: principal.user_id,
                permission: Permission::UserManagement,
                resource: ResourceScope::System,
            });
        }
        let now = Timestamp::now();

        // TODO: Generate proper user ID
        let user_id = UserId::from(0);

        // Create user metadata
        let user = SystemUserRecord {
            user_id: user_id,
            username: config.username.clone(),
            version: 1,
            created_at: now,
            last_modified: now,
            group_ids: Vec::new(),
            role_ids: Vec::new(),
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
    /// - Requires [`Permission::UserManagement`] on [`ResourceScope::System`]
    ///
    /// # Parameters
    /// - `principal`: [`SecurityPrincipal`] for authorization
    /// - `user_id`: [`UserId`] to update
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
        user_id: &UserId,
        config: SystemUserUpdate,
    ) -> KeyValueResult<SystemUserMetadata> {
        if !principal.has_system_permission(&Permission::UserManagement) {
            return Err(KeyValueError::PermissionDenied {
                user_id: principal.user_id,
                permission: Permission::UserManagement,
                resource: ResourceScope::System,
            });
        }
        let mut cache = self
            .system_metacache
            .write()
            .map_err(|_| KeyValueError::LockPoisoned)?;

        if let Some(mut user_record) = cache.get_system_user_record(user_id).cloned() {
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
            let key = SystemKeys::system_user_key(*user_id);
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
                user_id
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
    /// - Requires [`Permission::UserManagement`] on [`ResourceScope::System`]
    ///
    /// # Parameters
    /// - `principal`: [`SecurityPrincipal`] for authorization
    /// - `user_id`: [`UserId`] to remove
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
        user_id: &UserId,
    ) -> KeyValueResult<()> {
        if !principal.has_system_permission(&Permission::UserManagement) {
            return Err(KeyValueError::PermissionDenied {
                user_id: principal.user_id,
                permission: Permission::UserManagement,
                resource: ResourceScope::System,
            });
        }
        // Remove from cache
        {
            let mut cache = self
                .system_metacache
                .write()
                .map_err(|_| KeyValueError::LockPoisoned)?;
            cache.clear_system_user_record(user_id);
        }

        // Delete from system shard
        let key = SystemKeys::system_user_key(*user_id);

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
    ///
    /// # TODO
    /// - Implement pagination support for large numbers of tenants
    /// - Add disk fallback if not in cache
    #[tracing::instrument(skip(self))]
    pub async fn get_tenants(
        &self,
        principal: &SecurityPrincipal,
    ) -> KeyValueResult<impl IntoIterator<Item = (TenantId, String)>> {
        if !principal.has_system_permission(&Permission::TenantList) {
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
            .map(|x| (x.id, x.name.clone()))
            .collect::<Vec<_>>();
        Ok(tenants)
    }

    /// Get metadata for a specific tenant.
    ///
    /// # What it does
    /// Retrieves metadata for a single tenant by ID, checking cache first then disk.
    ///
    /// # How it works
    /// 1. Checks system_metacache for cached tenant record
    /// 2. If not in cache, reads from system shard using [`SystemKeys::tenant_key`]
    /// 3. Deserializes the stored metadata
    /// 4. Updates cache with the retrieved metadata
    ///
    /// # Access Control
    /// - Requires [`Permission::TenantView`] on [`ResourceScope::System`]
    ///
    /// # Parameters
    /// - `principal`: [`SecurityPrincipal`] for authorization
    /// - `tenant_id`: [`TenantId`] to look up
    ///
    /// # Returns
    /// - `Some(TenantMetadata)` if tenant exists
    /// - `None` if tenant not found
    /// - `Err(KeyValueError)` if lock poisoned or disk read fails
    #[tracing::instrument(skip(self))]
    pub async fn get_tenant(
        &self,
        principal: &SecurityPrincipal,
        tenant_id: &TenantId,
    ) -> KeyValueResult<Option<TenantMetadata>> {
        // Users can view tenant details IF they are a system user OR have tenant permission
        if !principal.has_system_permission(&Permission::TenantView) {
            return Err(KeyValueError::PermissionDenied {
                user_id: principal.user_id,
                permission: Permission::TenantView,
                resource: ResourceScope::Tenant(tenant_id.clone()),
            });
        }

        // Check cache first
        {
            let cache = self
                .system_metacache
                .write()
                .map_err(|_| KeyValueError::LockPoisoned)?;
            if let Some(tenant_meta) = cache.get_tenant_record(tenant_id) {
                return Ok(Some(TenantMetadata::from(tenant_meta.clone())));
            }
        }

        // Read from disk
        let key = SystemKeys::tenant_key(*tenant_id);
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

    /// Get tenant metadata by name.
    ///
    /// # What it does
    /// Finds a tenant by their name string.
    ///
    /// # How it works
    /// Iterates through cached tenant records to find one matching the name.
    ///
    /// # Access Control
    /// - Requires [`Permission::TenantView`] on [`ResourceScope::System`]
    ///
    /// # Parameters
    /// - `principal`: [`SecurityPrincipal`] for authorization
    /// - `name`: Tenant name to search for
    ///
    /// # Returns
    /// - `Some(TenantMetadata)` if tenant with that name exists
    /// - `None` if no matching tenant found
    /// - `Err(KeyValueError)` if lock poisoned
    ///
    /// # TODO
    /// - Add index for name-based lookups
    /// - Implement disk fallback if not in cache
    #[tracing::instrument(skip(self))]
    pub async fn get_tenant_by_name(
        &self,
        principal: &SecurityPrincipal,
        name: &str,
    ) -> KeyValueResult<Option<TenantId>> {
        if !principal.has_system_permission(&Permission::TenantView) {
            return Err(KeyValueError::PermissionDenied {
                user_id: principal.user_id,
                permission: Permission::TenantView,
                resource: ResourceScope::System,
            });
        }
        {
            // Search in cache
            let cache = self
                .system_metacache
                .read()
                .map_err(|_| KeyValueError::LockPoisoned)?;
            let tenant = cache
                .list_tenant_records()
                .find(|t| t.name == name)
                .cloned()
                .map(|record| record.id);
            if tenant.is_some() {
                return Ok(tenant);
            }
        }
        {
            // Lookup on Disk
        }
        Ok(None)
    }

    /// Create a new tenant.
    ///
    /// # What it does
    /// Registers a new tenant with the provided configuration.
    ///
    /// # How it works
    /// 1. Generates a new [`TenantId`]
    /// 2. Creates [`TenantRecord`] with current timestamp and version 1
    /// 3. Stores in system_metacache
    /// 4. Serializes and persists to system shard using [`SystemKeys::tenant_key`]
    ///
    /// # Access Control
    /// - Requires [`Permission::TenantCreate`] on [`ResourceScope::System`]
    ///
    /// # Parameters
    /// - `principal`: [`SecurityPrincipal`] for authorization
    /// - `config`: [`TenantCreate`] containing name, options, and metadata
    ///
    /// # Returns
    /// - `Ok(TenantMetadata)` with the created tenant information
    /// - `Err(KeyValueError)` if lock poisoned or persistence fails
    ///
    /// # TODO
    /// - Generate proper unique tenant ID instead of hardcoded 0
    #[tracing::instrument(skip(self))]
    pub async fn create_tenant(
        &self,
        principal: &SecurityPrincipal,
        config: TenantCreate,
    ) -> KeyValueResult<TenantMetadata> {
        if !principal.has_system_permission(&Permission::TenantCreate) {
            return Err(KeyValueError::PermissionDenied {
                user_id: principal.user_id,
                permission: Permission::TenantCreate,
                resource: ResourceScope::System,
            });
        }
        let now = Timestamp::now();

        // Generate proper tenant ID
        let mut cache = self
            .system_metacache
            .write()
            .map_err(|_| KeyValueError::LockPoisoned)?;

        let tenant_id = TenantId::from(
            cache
                .list_tenant_records()
                .map(|t| t.id.as_u32())
                .max()
                .map(|id| id + 1)
                .unwrap_or(0),
        );

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
        cache.set_tenant_record(tenant.clone());
        drop(cache);

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

    /// Update an existing tenant's metadata.
    ///
    /// # What it does
    /// Updates tenant metadata with new values from the config.
    ///
    /// # How it works
    /// 1. Acquires write lock on system_metacache
    /// 2. Retrieves existing tenant record
    /// 3. Updates name if provided in config
    /// 4. Applies PropertyUpdate operations to options and metadata maps
    /// 5. Increments version and updates last_modified timestamp
    /// 6. Stores updated metadata back to cache
    /// 7. Serializes and persists to system shard
    ///
    /// # Access Control
    /// - Requires [`Permission::TenantAlter`] on [`ResourceScope::System`]
    ///
    /// # Parameters
    /// - `principal`: [`SecurityPrincipal`] for authorization
    /// - `tenant_id`: [`TenantId`] of the tenant to update
    /// - `config`: [`TenantUpdate`] containing optional updates
    ///
    /// # Returns
    /// - `Ok(TenantMetadata)` with updated tenant information
    /// - `Err(KeyValueError::InvalidValue)` if tenant not found
    /// - `Err(KeyValueError)` if lock poisoned or persistence fails
    #[tracing::instrument(skip(self))]
    pub async fn update_tenant(
        &self,
        principal: &SecurityPrincipal,
        tenant_id: &TenantId,
        config: TenantUpdate,
    ) -> KeyValueResult<TenantMetadata> {
        if !principal.has_system_permission(&Permission::TenantAlter) {
            return Err(KeyValueError::PermissionDenied {
                user_id: principal.user_id,
                permission: Permission::TenantAlter,
                resource: ResourceScope::Tenant(tenant_id.clone()),
            });
        }
        let mut cache = self
            .system_metacache
            .write()
            .map_err(|_| KeyValueError::LockPoisoned)?;

        if let Some(mut tenant_record) = cache.get_tenant_record(tenant_id).cloned() {
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
            let key = SystemKeys::tenant_key(*tenant_id);
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
                tenant_id
            )))
        }
    }

    /// Delete a tenant.
    ///
    /// # What it does
    /// Deletes a tenant record from the cache and persistent storage.
    ///
    /// # How it works
    /// 1. Acquires write lock on system_metacache
    /// 2. Clears the tenant record from cache
    /// 3. Generates storage key using [`SystemKeys::tenant_key`]
    /// 4. Deletes the record from the system shard (ShardId 0) via shard manager
    ///
    /// # Access Control
    /// - Requires [`Permission::TenantDelete`] on [`ResourceScope::System`]
    ///
    /// # Parameters
    /// - `principal`: [`SecurityPrincipal`] for authorization
    /// - `tenant_id`: [`TenantId`] of the tenant to delete
    ///
    ///
    /// # Returns
    /// - `Ok(())` if deletion succeeds
    /// - `Err(KeyValueError)` if lock poisoned or deletion fails
    ///
    /// # TODO
    /// - Check if any databases exist for this tenant and prevent deletion if so
    #[tracing::instrument(skip(self))]
    pub async fn delete_tenant(
        &self,
        principal: &SecurityPrincipal,
        tenant_id: &TenantId,
    ) -> KeyValueResult<()> {
        if !principal.has_system_permission(&Permission::TenantDelete) {
            return Err(KeyValueError::PermissionDenied {
                user_id: principal.user_id,
                permission: Permission::TenantDelete,
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
            cache.clear_tenant_record(tenant_id);
        }

        // Delete from system shard
        let key = SystemKeys::tenant_key(*tenant_id);

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
    /// - Requires [`Permission::UserManagement`] on [`ResourceScope::Tenant`]
    ///
    /// # Parameters
    /// - `principal`: [`SecurityPrincipal`] for authorization
    /// - `tenant_id`: [`TenantId`] for tenant scope
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
        tenant_id: &TenantId,
    ) -> KeyValueResult<impl IntoIterator<Item = TenantUserMetadata>> {
        if !principal.has_tenant_permission(&Permission::UserManagement, tenant_id) {
            return Err(KeyValueError::PermissionDenied {
                user_id: principal.user_id,
                permission: Permission::UserManagement,
                resource: ResourceScope::Tenant(tenant_id.clone()),
            });
        }

        // TODO: Return Tenant Users
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
    /// - Requires [`Permission::UserManagement`] on [`ResourceScope::Tenant`]
    ///
    /// # Parameters
    /// - `principal`: [`SecurityPrincipal`] for authorization
    /// - `tenant_id`: [`TenantId`] of the tenant to which the user belongs
    /// - `user_id`: [`UserId`] to look up
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
        tenant_id: &TenantId,
        user_id: &UserId,
    ) -> KeyValueResult<Option<TenantUserMetadata>> {
        if !principal.has_tenant_permission(&Permission::UserManagement, tenant_id) {
            return Err(KeyValueError::PermissionDenied {
                user_id: principal.user_id,
                permission: Permission::UserManagement,
                resource: ResourceScope::Tenant(tenant_id.clone()),
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

        let system_user_record = if let Some(system_user_record) =
            system_cache.get_system_user_record(user_id).cloned()
        {
            system_user_record
        } else {
            let system_user_key = SystemKeys::system_user_key(*user_id);
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
        let tenant_user_record = if let Some(tenant_user_record) = system_cache
            .get_tenant_user_record(tenant_id, user_id)
            .cloned()
        {
            tenant_user_record
        } else {
            let tenant_user_key = SystemKeys::tenant_user_key(*tenant_id, *user_id);
            if let Some(value) = shard_manager
                .get(ShardId::from(0), &tenant_user_key)
                .await?
            {
                let tenant_user_record: TenantUserRecord = deserialize(&value)?;
                system_cache.set_tenant_user_record(tenant_id, tenant_user_record.clone());
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
    /// - Requires [`Permission::UserManagement`] on [`ResourceScope::Tenant`]
    ///
    /// # Parameters
    /// - `principal`: [`SecurityPrincipal`] for authorization
    /// - `tenant_id`: [`TenantId`] for the tenant to create the user in
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
        tenant_id: &TenantId,
        config: TenantUserCreate,
    ) -> KeyValueResult<TenantUserMetadata> {
        if !principal.has_tenant_permission(&Permission::UserManagement, tenant_id) {
            return Err(KeyValueError::PermissionDenied {
                user_id: principal.user_id,
                permission: Permission::UserManagement,
                resource: ResourceScope::Tenant(tenant_id.clone()),
            });
        }
        let now = Timestamp::now();

        // TODO: First lookup system user and use existing record if it exists.

        // TODO: Generate proper user ID
        let user_id = UserId::from(0);

        // Create System User metadata
        let system_user_record = SystemUserRecord {
            user_id,
            username: config.username.clone(),
            version: 1,
            created_at: now,
            last_modified: now,
            group_ids: Vec::new(),
            role_ids: Vec::new(),
            grants: Vec::new(), // Start with no permission grants - grant via roles/groups
            enabled: true,
            password_hash: None, // TODO: Add password hashing
            options: config.options.clone(),
            metadata: config.metadata.clone(),
        };

        // Create tenant user metadata
        let tenant_user_record = TenantUserRecord {
            user_id,
            tenant_id: tenant_id.clone(),
            version: 1,
            created_at: now,
            last_modified: now,
            group_ids: vec![],
            role_ids: vec![],
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
            cache.set_tenant_user_record(tenant_id, tenant_user_record.clone());
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
    /// - Requires [`Permission::UserManagement`] on [`ResourceScope::Tenant`]
    ///
    /// # Parameters
    /// - `principal`: [`SecurityPrincipal`] for authorization
    /// - `tenant_id`: [`TenantId`] for tenant context
    /// - `user_id`: [`UserId`] to update
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
        tenant_id: &TenantId,
        user_id: &UserId,
        config: TenantUserUpdate,
    ) -> KeyValueResult<TenantUserMetadata> {
        if !principal.has_tenant_permission(&Permission::UserManagement, tenant_id) {
            return Err(KeyValueError::PermissionDenied {
                user_id: principal.user_id,
                permission: Permission::UserManagement,
                resource: ResourceScope::Tenant(tenant_id.clone()),
            });
        }
        let mut cache = self
            .system_metacache
            .write()
            .map_err(|_| KeyValueError::LockPoisoned)?;

        if let Some(mut system_user_record) = cache.get_system_user_record(user_id).cloned() {
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
            let key = SystemKeys::system_user_key(*user_id);
            let value = serialize(&system_user_record)?;

            let shard_manager = self
                .shard_manager
                .read()
                .map_err(|_| KeyValueError::LockPoisoned)?;
            shard_manager.put(ShardId::from(0), &key, &value).await?;

            self.get_tenant_user(principal, tenant_id, user_id)
                .await?
                .ok_or_else(|| {
                    KeyValueError::Internal(format!(
                        "Failed to find updated user record for user {}",
                        user_id
                    ))
                })
        } else {
            Err(KeyValueError::InvalidValue(format!(
                "User not found: {:?}",
                user_id
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
    /// - Requires [`Permission::UserManagement`] on [`ResourceScope::System`]
    ///
    /// # Parameters
    /// - `principal`: [`SecurityPrincipal`] for authorization
    /// - `tenant_id`: [`TenantId`] to remove user from
    /// - `user_id`: [`UserId`] to remove
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
        tenant_id: &TenantId,
        user_id: &UserId,
    ) -> KeyValueResult<()> {
        if !principal.has_tenant_permission(&Permission::UserManagement, tenant_id) {
            return Err(KeyValueError::PermissionDenied {
                user_id: principal.user_id,
                permission: Permission::UserManagement,
                resource: ResourceScope::Tenant(tenant_id.clone()),
            });
        }
        // Remove tenant user from cache
        {
            let mut cache = self
                .system_metacache
                .write()
                .map_err(|_| KeyValueError::LockPoisoned)?;
            cache.clear_tenant_user_record(tenant_id, user_id);
        }
        {
            let tenant_user_key = SystemKeys::tenant_user_key(*tenant_id, *user_id);
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
                cache.clear_system_user_record(user_id);
            }
            {
                let system_user_key = SystemKeys::tenant_user_key(*tenant_id, *user_id);
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
    /// - `tenant_id`: [`TenantId`] to filter databases by
    ///
    /// # Returns
    /// An iterator over [`DatabaseMetadata`] for all matching databases
    ///
    /// # TODO
    /// - Implement proper database enumeration from disk if not in cache
    /// - Add pagination support for large numbers of databases
    #[tracing::instrument(skip(self))]
    pub async fn get_databases(
        &self,
        principal: &SecurityPrincipal,
        tenant_id: &TenantId,
    ) -> KeyValueResult<impl IntoIterator<Item = (DatabaseId, String)>> {
        if !principal.has_tenant_permission(&Permission::DatabaseList, tenant_id) {
            return Err(KeyValueError::PermissionDenied {
                user_id: principal.user_id,
                permission: Permission::DatabaseView,
                resource: ResourceScope::Tenant(tenant_id.clone()),
            });
        }
        let cache = self
            .system_metacache
            .read()
            .map_err(|_| KeyValueError::LockPoisoned)?;
        let records = cache
            .list_database_records()
            .filter(|d| &d.tenant_id == tenant_id)
            .cloned()
            .map(|db| (db.database_id, db.name))
            .collect::<Vec<_>>();
        Ok(records)
    }

    /// Get metadata for a specific database.
    ///
    /// # What it does
    /// Retrieves metadata for a single database by ID, checking cache first then disk.
    ///
    /// # How it works
    /// 1. Checks system_metacache for cached database record
    /// 2. If not in cache, reads from system shard using [`SystemKeys::database_key`]
    /// 3. Deserializes the stored metadata
    /// 4. Updates cache with the retrieved metadata
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
    /// - `Some(DatabaseMetadata)` if database exists and belongs to the tenant
    /// - `None` if database not found
    /// - `Err(KeyValueError)` if lock poisoned or disk read fails
    #[tracing::instrument(skip(self))]
    pub async fn get_database(
        &self,
        principal: &SecurityPrincipal,
        tenant_id: &TenantId,
        database_id: &DatabaseId,
    ) -> KeyValueResult<Option<DatabaseMetadata>> {
        if !principal.has_database_permission(&Permission::DatabaseView, tenant_id, database_id) {
            return Err(KeyValueError::PermissionDenied {
                user_id: principal.user_id,
                permission: Permission::DatabaseView,
                resource: ResourceScope::Database(*database_id),
            });
        }
        // Check cache first
        {
            let cache = self
                .system_metacache
                .read()
                .map_err(|_| KeyValueError::LockPoisoned)?;
            if let Some(database_record) =
                cache.get_database_record(&ContainerId::from_parts(*tenant_id, *database_id))
            {
                if &database_record.tenant_id == tenant_id {
                    return Ok(Some(DatabaseMetadata::from(database_record.clone())));
                }
            }
        }
        {
            // Read from disk
            let key = SystemKeys::database_key(*tenant_id, *database_id);
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

    /// Get metadata for a specific database.
    ///
    /// # What it does
    /// Retrieves metadata for a single database by ID, checking cache first then disk.
    ///
    /// # How it works
    /// 1. Checks system_metacache for cached database record
    /// 2. If not in cache, reads from system shard using [`SystemKeys::database_key`]
    /// 3. Deserializes the stored metadata
    /// 4. Updates cache with the retrieved metadata
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
    /// - `Some(DatabaseMetadata)` if database exists and belongs to the tenant
    /// - `None` if database not found
    /// - `Err(KeyValueError)` if lock poisoned or disk read fails
    #[tracing::instrument(skip(self))]
    pub async fn get_database_by_name(
        &self,
        principal: &SecurityPrincipal,
        tenant_id: &TenantId,
        name: &str,
    ) -> KeyValueResult<Option<DatabaseId>> {
        if !principal.has_tenant_permission(&Permission::DatabaseList, tenant_id) {
            return Err(KeyValueError::PermissionDenied {
                user_id: principal.user_id,
                permission: Permission::DatabaseList,
                resource: ResourceScope::Tenant(*tenant_id),
            });
        }
        // TODO: Implement name to ID lookup
        Ok(None)
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
    /// # Access Control
    /// - Requires [`Permission::DatabaseCreate`] on [`ResourceScope::Tenant`]
    ///
    /// # Parameters
    /// - `principal`: [`SecurityPrincipal`] for authorization
    /// - `tenant_id`: [`TenantId`] that will own the database
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
        tenant_id: &TenantId,
        config: DatabaseCreate,
    ) -> KeyValueResult<DatabaseMetadata> {
        if !principal.has_tenant_permission(&Permission::DatabaseCreate, tenant_id) {
            return Err(KeyValueError::PermissionDenied {
                user_id: principal.user_id,
                permission: Permission::DatabaseCreate,
                resource: ResourceScope::Tenant(tenant_id.clone()),
            });
        }
        let now = Timestamp::now();

        // Generate proper database ID
        let mut cache = self
            .system_metacache
            .write()
            .map_err(|_| KeyValueError::LockPoisoned)?;

        let database_id = DatabaseId::from(
            cache
                .list_database_records()
                .filter(|d| d.tenant_id == *tenant_id)
                .map(|d| d.database_id.as_u32())
                .max()
                .map(|id| id + 1)
                .unwrap_or(0),
        );

        // Create database metadata
        // TODO: Create root namespace for database
        let root_namespace = NamespaceId::from(0);

        let database_record = DatabaseRecord {
            database_id,
            tenant_id: *tenant_id,
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
        cache.set_database_record(database_record.clone());
        drop(cache);

        // Persist to system shard
        let key = SystemKeys::database_key(*tenant_id, database_id);
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
    /// # Access Control
    /// - Requires [`Permission::DatabaseAlter`] on [`ResourceScope::Tenant`]
    ///
    /// # Parameters
    /// - `principal`: [`SecurityPrincipal`] for authorization
    /// - `tenant_id`: [`TenantId`] that should own the database
    /// - `database_id`: [`DatabaseId`] to update
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
        tenant_id: &TenantId,
        database_id: &DatabaseId,
        config: DatabaseUpdate,
    ) -> KeyValueResult<DatabaseMetadata> {
        if !principal.has_database_permission(&Permission::DatabaseAlter, tenant_id, database_id) {
            return Err(KeyValueError::PermissionDenied {
                user_id: principal.user_id,
                permission: Permission::DatabaseAlter,
                resource: ResourceScope::Database(*database_id),
            });
        }
        let mut cache = self
            .system_metacache
            .write()
            .map_err(|_| KeyValueError::LockPoisoned)?;

        if let Some(mut database_record) = cache
            .get_database_record(&ContainerId::from_parts(*tenant_id, *database_id))
            .cloned()
        {
            // Verify tenant matches
            if &database_record.tenant_id != tenant_id {
                return Err(KeyValueError::InvalidValue(format!(
                    "Database {:?} does not belong to tenant {:?}",
                    database_id, tenant_id
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
            let key = SystemKeys::database_key(*tenant_id, *database_id);
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
                database_id
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
    /// # Access Control
    /// - Requires [`Permission::DatabaseDelete`] on [`ResourceScope::Tenant`]
    ///
    /// # Parameters
    /// - `principal`: [`SecurityPrincipal`] for authorization
    /// - `tenant_id`: [`TenantId`] that owns the database
    /// - `database_id`: [`DatabaseId`] to delete
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
        tenant_id: &TenantId,
        database_id: &DatabaseId,
    ) -> KeyValueResult<()> {
        if !principal.has_tenant_permission(&Permission::DatabaseDelete, tenant_id) {
            return Err(KeyValueError::PermissionDenied {
                user_id: principal.user_id,
                permission: Permission::DatabaseDelete,
                resource: ResourceScope::Tenant(tenant_id.clone()),
            });
        }
        // Remove from cache
        {
            let mut cache = self
                .system_metacache
                .write()
                .map_err(|_| KeyValueError::LockPoisoned)?;
            cache.clear_database_record(&ContainerId::from_parts(*tenant_id, *database_id));
        }

        // Delete from system shard
        let key = SystemKeys::database_key(*tenant_id, *database_id);

        let shard_manager = self
            .shard_manager
            .read()
            .map_err(|_| KeyValueError::LockPoisoned)?;
        shard_manager.delete(ShardId::from(0), &key).await?;

        Ok(())
    }

    /**********************************************************************************************\
     * Tablespace Management                                                                      *
    \**********************************************************************************************/

    /// Get metadata about all tablespaces assigned to a tenant.
    ///
    /// # What it does
    /// Returns an iterator over all tablespace records in the system assigned to the tenant.
    ///
    /// # How it works
    /// Reads from system_metacache and returns all cached tablespace records.
    ///
    /// # Access Control
    /// - Requires [`Permission::TablespaceList`] on [`ResourceScope::Tenant`]
    ///
    /// # Parameters
    /// - `principal`: [`SecurityPrincipal`] for authorization
    /// - `tenant_id`: [`TenantId`]
    ///
    /// # Returns
    /// An iterator over [`TablespaceRecord`] for all tablespaces
    ///
    /// # TODO
    /// - Implement proper tablespace enumeration from disk if not in cache
    /// - Add pagination support for large numbers of tablespaces
    #[tracing::instrument(skip(self))]
    pub async fn get_tenant_tablespaces(
        &self,
        principal: &SecurityPrincipal,
        tenant_id: &TenantId,
    ) -> KeyValueResult<impl IntoIterator<Item = (TablespaceId, String)>> {
        if !principal.has_tenant_permission(&Permission::TablespaceList, tenant_id) {
            return Err(KeyValueError::PermissionDenied {
                user_id: principal.user_id,
                permission: Permission::TablespaceView,
                resource: ResourceScope::System,
            });
        }
        // Return Tenant Assigned Tablespaces
        let cache = self.system_metacache.read().unwrap();
        // TODO: Filter by tablespaces assigned to tenant
        let tablespaces = cache
            .list_tablespace_records()
            .cloned()
            .map(|r| (r.id, r.name))
            .collect::<Vec<_>>();
        Ok(tablespaces)
    }

    /// Get metadata about all tablespaces.
    ///
    /// # What it does
    /// Returns an iterator over all tablespace records in the system.
    ///
    /// # How it works
    /// Reads from system_metacache and returns all cached tablespace records.
    ///
    /// # Access Control
    /// - Requires [`Permission::TablespaceList`] on [`ResourceScope::Tenant`]
    ///
    /// # Parameters
    /// - `principal`: [`SecurityPrincipal`] for authorization
    ///
    /// # Returns
    /// An iterator over [`TablespaceRecord`] for all tablespaces
    ///
    /// # TODO
    /// - Implement proper tablespace enumeration from disk if not in cache
    /// - Add pagination support for large numbers of tablespaces
    #[tracing::instrument(skip(self))]
    pub async fn get_tablespaces(
        &self,
        principal: &SecurityPrincipal,
    ) -> KeyValueResult<impl IntoIterator<Item = (TablespaceId, String)>> {
        if !principal.has_system_permission(&Permission::TablespaceList) {
            return Err(KeyValueError::PermissionDenied {
                user_id: principal.user_id,
                permission: Permission::TablespaceView,
                resource: ResourceScope::System,
            });
        }
        // Return All Tablespaces
        let cache = self.system_metacache.read().unwrap();
        let tablespaces = cache
            .list_tablespace_records()
            .cloned()
            .map(|r| (r.id, r.name))
            .collect::<Vec<_>>();
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
    /// # Access Control
    /// - Requires [`Permission::TablespaceView`] on [`ResourceScope::System`]
    ///
    /// # Parameters
    /// - `principal`: [`SecurityPrincipal`] for authorization
    /// - `tablespace_id`: [`TablespaceId`] to look up
    ///
    /// # Returns
    /// - `Some(TablespaceRecord)` if tablespace exists
    /// - `None` if tablespace not found
    ///
    /// # TODO
    /// - Add disk fallback if not in cache
    #[tracing::instrument(skip(self))]
    pub async fn get_tablespace(
        &self,
        principal: &SecurityPrincipal,
        tablespace_id: &TablespaceId,
    ) -> KeyValueResult<Option<TablespaceRecord>> {
        if !principal.has_system_permission(&Permission::TablespaceView) {
            return Err(KeyValueError::PermissionDenied {
                user_id: principal.user_id,
                permission: Permission::TablespaceView,
                resource: ResourceScope::Tablespace(*tablespace_id),
            });
        }
        let cache = self.system_metacache.write().unwrap();
        Ok(cache.get_tablespace_record(tablespace_id).cloned())
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
        if !principal.has_system_permission(&Permission::TablespaceView) {
            return Err(KeyValueError::PermissionDenied {
                user_id: principal.user_id,
                permission: Permission::TablespaceView,
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
    /// # Access Control
    /// - Requires [`Permission::TablespaceManagement`] on [`ResourceScope::System`]
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
        if !principal.has_permission(&Permission::TablespaceManagement) {
            return Err(KeyValueError::PermissionDenied {
                user_id: principal.user_id,
                permission: Permission::TablespaceManagement,
                resource: ResourceScope::System,
            });
        }
        let now = Timestamp::now();

        let mut cache = self.system_metacache.write().unwrap();

        // Get actual new tablespace ID
        let tablespace_id = TablespaceId::new(
            cache
                .list_tablespace_records()
                .map(|t| t.id.as_u32())
                .max()
                .map(|id| id + 1)
                .unwrap_or(0),
        );

        // Create tablespace metadata
        let tablespace = TablespaceRecord {
            id: tablespace_id,
            name: config.name.clone(),
            storage_path: config.storage_path.clone(),
            tier: config.tier.clone(),
            tenants: Default::default(),
            version: 1,
            created_at: now,
            last_modified: now,
            options: config.options.clone(),
            metadata: config.metadata.clone(),
        };

        // Store in cache
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
    /// # Access Control
    /// - Requires [`Permission::TablespaceManagement`] on [`ResourceScope::System`]
    ///
    /// # Parameters
    /// - `principal`: [`SecurityPrincipal`] for authorization
    /// - `tablespace_id`: [`TablespaceId`] to update
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
        tablespace_id: &TablespaceId,
        config: TablespaceUpdate,
    ) -> KeyValueResult<TablespaceRecord> {
        if !principal.has_system_permission(&Permission::TablespaceManagement) {
            return Err(KeyValueError::PermissionDenied {
                user_id: principal.user_id,
                permission: Permission::TablespaceManagement,
                resource: ResourceScope::System,
            });
        }
        let mut cache = self.system_metacache.write().unwrap();

        if let Some(mut tablespace) = cache.get_tablespace_record(&tablespace_id).cloned() {
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
                tablespace_id
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
    /// # Access Control
    /// - Requires [`Permission::TablespaceManagement`] on [`ResourceScope::System`]
    ///
    /// # Parameters
    /// - `principal`: [`SecurityPrincipal`] for authorization
    /// - `tablespace_id`: [`TablespaceId`] to delete
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
        tablespace_id: &TablespaceId,
    ) -> KeyValueResult<()> {
        if !principal.has_system_permission(&Permission::TablespaceManagement) {
            return Err(KeyValueError::PermissionDenied {
                user_id: principal.user_id,
                permission: Permission::TablespaceManagement,
                resource: ResourceScope::System,
            });
        }
        let mut cache = self.system_metacache.write().unwrap();
        cache.clear_tablespace_record(tablespace_id);

        // TODO: Persist deletion to system shard via Raft if in distributed mode
        // TODO: Update shard manager's path resolver to remove tablespace config

        Ok(())
    }

    /**********************************************************************************************\
     * Database Management                                                                        *
    \**********************************************************************************************/

    /// Get object metadata by path.
    ///
    /// # What it does
    /// Finds a namespace by its path string.
    ///
    /// # How it works
    /// 1. Retrieves container metadata cache
    /// 2. Iterates through namespace records to find one matching the path
    ///
    /// # Access Control
    /// - Requires [`Permission::DatabaseAccess`] on [`ResourceScope::Database`] or [`ResourceScope::AllDatabases`]
    /// AND
    /// - Requires [`Permission::NamespaceList`] on [`ResourceScope::Namespace`] or [`ResourceScope::AllNamespaces`]
    ///
    /// # Parameters
    /// - `principal`: [`SecurityPrincipal`] for authorization
    /// - `container_id`: [`ContainerId`] to search in
    /// - `path`: Namespace path to search for
    ///
    /// # Returns
    /// - `Some(NamespaceMetadata)` if namespace with that path exists
    /// - `None` if no matching namespace found
    ///
    /// # Errors
    /// - `Err(KeyValueError::InvalidValue)` if container not found
    /// - `Err(KeyValueError::LockPoisoned)` if lock poisoned
    ///
    /// # TODO
    /// - Implement Disk Indexing
    #[tracing::instrument(skip(self))]
    pub async fn get_object_by_path(
        &self,
        principal: &SecurityPrincipal,
        container_id: &ContainerId,
        path: &str,
    ) -> KeyValueResult<Option<(ObjectId, ObjectType)>> {
        // First security check if user has database access
        if !principal.has_database_permission(
            &Permission::DatabaseAccess,
            &container_id.tenant(),
            &container_id.database(),
        ) {
            return Err(KeyValueError::PermissionDenied {
                user_id: principal.user_id,
                permission: Permission::NamespaceList,
                resource: ResourceScope::Database(container_id.database()),
            });
        }
        // Get container metadata cache
        let container_caches = self
            .container_metacaches
            .read()
            .map_err(|_| KeyValueError::LockPoisoned)?;

        let container_cache = container_caches.get(&container_id).ok_or_else(|| {
            KeyValueError::InvalidValue(format!("Container not found: {:?}", container_id))
        })?;

        let mut cache = container_cache
            .write()
            .map_err(|_| KeyValueError::LockPoisoned)?;

        // Find objects by path
        // TODO: Figure out how to do this when not loaded into the cache
        if let Some(object) = cache.path_resolver().get_path_reference(path) {
            if let Some(parent_id) = object.parent() {
                // Second security check if user has namespace access to parent
                if !principal.has_namespace_permission(
                    &Permission::DatabaseAccess,
                    &container_id.tenant(),
                    &container_id.database(),
                    &NamespaceId::from(parent_id),
                ) {
                    return Err(KeyValueError::PermissionDenied {
                        user_id: principal.user_id,
                        permission: Permission::NamespaceList,
                        resource: ResourceScope::Database(container_id.database()),
                    });
                }
                // Return the object
                Ok(Some((object.object_id(), object.object_type())))
            } else {
                Ok(None)
            }
        } else {
            Ok(None)
        }
    }

    /// Get all objects belonging to a specific namespace.
    ///
    /// # What it does
    /// Returns an iterator over metadata for all objects (tables, etc.) within a namespace.
    ///
    /// # How it works
    /// 1. Retrieves container metadata cache
    /// 2. (TODO) Queries the name resolver or iterates objects to find those in the namespace
    /// 3. Returns metadata for matching objects
    ///
    /// # Access Control
    /// - Requires [`Permission::NamespaceList`] on the specified [`ResourceScope::Namespace`]
    ///
    /// # Parameters
    /// - `principal`: [`SecurityPrincipal`] for authorization
    /// - `container_id`: [`ContainerId`] containing the namespace
    /// - `namespace_id`: [`NamespaceId`] to list objects for
    ///
    /// # Returns
    /// An iterator over `(ObjectId, ObjectType, ObjectMetadata)` tuples
    ///
    /// # Errors
    /// - `Err(KeyValueError::InvalidValue)` if container not found
    /// - `Err(KeyValueError::LockPoisoned)` if lock poisoned
    ///
    /// # TODO
    /// - Implement proper object enumeration when namespace hierarchy is finalized
    /// - Use NameResolver for efficient lookup
    #[tracing::instrument(skip(self))]
    pub async fn get_objects_by_namespace(
        &self,
        principal: &SecurityPrincipal,
        container_id: &ContainerId,
        namespace_id: &NamespaceId,
    ) -> KeyValueResult<impl IntoIterator<Item = (ObjectId, ObjectType, ObjectMetadata)>> {
        if !principal.has_namespace_permission(
            &Permission::NamespaceList,
            &container_id.tenant(),
            &container_id.database(),
            &namespace_id,
        ) {
            return Err(KeyValueError::PermissionDenied {
                user_id: principal.user_id,
                permission: Permission::NamespaceList,
                resource: ResourceScope::Namespace(*namespace_id),
            });
        }
        // Get container metadata cache
        let container_caches = self
            .container_metacaches
            .read()
            .map_err(|_| KeyValueError::LockPoisoned)?;

        let container_cache = container_caches.get(container_id).ok_or_else(|| {
            KeyValueError::InvalidValue(format!("Container not found: {:?}", container_id))
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
    /// # Access Control
    /// - Requires [`Permission::NamespaceView`] on [`ResourceScope::Database`]
    ///
    /// # Parameters
    /// - `principal`: [`SecurityPrincipal`] for authorization
    /// - `container_id`: [`ContainerId`] to get namespaces from
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
        container_id: &ContainerId,
    ) -> KeyValueResult<impl IntoIterator<Item = NamespaceRecord>> {
        if !principal.has_database_permission(
            &Permission::NamespaceView,
            &container_id.tenant(),
            &container_id.database(),
        ) {
            return Err(KeyValueError::PermissionDenied {
                user_id: principal.user_id,
                permission: Permission::NamespaceList,
                resource: ResourceScope::Database(container_id.database()),
            });
        }
        // Get container metadata cache
        let container_caches = self
            .container_metacaches
            .read()
            .map_err(|_| KeyValueError::LockPoisoned)?;

        let container_cache = container_caches.get(container_id).ok_or_else(|| {
            KeyValueError::InvalidValue(format!("Container not found: {:?}", container_id))
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
    /// # Access Control
    /// - Requires [`Permission::NamespaceList`] on [`ResourceScope::Database`]
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
        container_id: &ContainerId,
        prefix: &str,
    ) -> KeyValueResult<impl IntoIterator<Item = NamespaceRecord>> {
        if !principal.has_database_permission(
            &Permission::NamespaceList,
            &container_id.tenant(),
            &container_id.database(),
        ) {
            return Err(KeyValueError::PermissionDenied {
                user_id: principal.user_id,
                permission: Permission::NamespaceList,
                resource: ResourceScope::Database(container_id.database()),
            });
        }
        // Get container metadata cache
        let container_caches = self
            .container_metacaches
            .read()
            .map_err(|_| KeyValueError::LockPoisoned)?;

        let container_cache = container_caches.get(container_id).ok_or_else(|| {
            KeyValueError::InvalidValue(format!("Container not found: {:?}", container_id))
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
    /// # Access Control
    /// - Requires [`Permission::SystemSecurityManage`] on [`ResourceScope::System`]
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
        container_id: &ContainerId,
        namespace_id: &NamespaceId,
    ) -> KeyValueResult<Option<NamespaceRecord>> {
        if !principal.has_namespace_permission(
            &Permission::NamespaceView,
            &container_id.tenant(),
            &container_id.database(),
            &namespace_id,
        ) {
            return Err(KeyValueError::PermissionDenied {
                user_id: principal.user_id,
                permission: Permission::NamespaceList,
                resource: ResourceScope::Database(container_id.database()),
            });
        }
        // Get container metadata cache
        let container_caches = self
            .container_metacaches
            .read()
            .map_err(|_| KeyValueError::LockPoisoned)?;

        let container_cache = container_caches.get(container_id).ok_or_else(|| {
            KeyValueError::InvalidValue(format!("Container not found: {:?}", container_id))
        })?;

        let cache = container_cache
            .read()
            .map_err(|_| KeyValueError::LockPoisoned)?;

        // Get namespace from cache
        Ok(cache.get_namespace_record(namespace_id).cloned())
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
    /// # Access Control
    /// - Requires [`Permission::SystemSecurityManage`] on [`ResourceScope::System`]
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
        container_id: &ContainerId,
        config: NamespaceCreate,
    ) -> KeyValueResult<NamespaceRecord> {
        if !principal.has_namespace_permission(
            &Permission::NamespaceCreate,
            &container_id.tenant(),
            &container_id.database(),
            &config.parent,
        ) {
            return Err(KeyValueError::PermissionDenied {
                user_id: principal.user_id,
                permission: Permission::NamespaceCreate,
                resource: ResourceScope::Database(container_id.database()),
            });
        }
        // Get container metadata cache
        let container_caches = self
            .container_metacaches
            .read()
            .map_err(|_| KeyValueError::LockPoisoned)?;

        let container_cache = container_caches.get(container_id).ok_or_else(|| {
            KeyValueError::InvalidValue(format!("Container not found: {:?}", container_id))
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
            default_tablespace: None,
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

    /// Update an existing namespace's metadata.
    ///
    /// # What it does
    /// Updates namespace metadata with new values from the config.
    ///
    /// # How it works
    /// 1. Retrieves container metadata cache
    /// 2. Retrieves existing namespace record
    /// 3. Updates name if provided in config
    /// 4. Applies PropertyUpdate operations to options and metadata maps
    /// 5. Increments version and updates last_modified timestamp
    /// 6. Stores updated metadata back to cache
    ///
    /// # Access Control
    /// - Requires [`Permission::NamespaceAlter`] on [`ResourceScope::Namespace`]
    /// OR
    /// - Requires [`Permission::NamespaceAlter`] on [`ResourceScope::AllNamespaces`]
    ///
    /// # Parameters
    /// - `principal`: [`SecurityPrincipal`] for authorization
    /// - `container`: [`ContainerId`] containing the namespace
    /// - `namespace`: [`NamespaceId`] to update
    /// - `config`: [`NamespaceUpdate`] containing optional updates
    ///
    /// # Returns
    /// - `Ok(NamespaceRecord)` with updated namespace information
    /// - `Err(KeyValueError::InvalidValue)` if namespace not found
    /// - `Err(KeyValueError)` if lock poisoned
    ///
    /// # TODO
    /// - Persist to container metadata shard
    #[tracing::instrument(skip(self))]
    pub async fn update_namespace(
        &self,
        principal: &SecurityPrincipal,
        container_id: &ContainerId,
        namespace_id: &NamespaceId,
        config: NamespaceUpdate,
    ) -> KeyValueResult<NamespaceRecord> {
        if !principal.has_namespace_permission(
            &Permission::NamespaceAlter,
            &container_id.tenant(),
            &container_id.database(),
            namespace_id,
        ) {
            return Err(KeyValueError::PermissionDenied {
                user_id: principal.user_id,
                permission: Permission::NamespaceAlter,
                resource: ResourceScope::Namespace(*namespace_id),
            });
        }
        // Get container metadata cache
        let container_caches = self
            .container_metacaches
            .read()
            .map_err(|_| KeyValueError::LockPoisoned)?;

        let container_cache = container_caches.get(container_id).ok_or_else(|| {
            KeyValueError::InvalidValue(format!("Container not found: {:?}", container_id))
        })?;

        let mut cache = container_cache
            .write()
            .map_err(|_| KeyValueError::LockPoisoned)?;

        // Get existing namespace metadata
        let mut namespace_meta = cache
            .get_namespace_record(namespace_id)
            .cloned()
            .ok_or_else(|| {
                KeyValueError::InvalidValue(format!("Namespace not found: {:?}", namespace_id))
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

    /// Delete a namespace.
    ///
    /// # What it does
    /// Deletes a namespace record from the container cache and persistent storage.
    ///
    /// # How it works
    /// 1. Retrieves container metadata cache
    /// 2. (TODO) Check if any objects exist in the namespace
    /// 3. Removes namespace record from cache
    ///
    /// # Access Control
    /// - Requires [`Permission::NamespaceDelete`] on the container
    ///
    /// # Parameters
    /// - `principal`: [`SecurityPrincipal`] for authorization
    /// - `container`: [`ContainerId`] containing the namespace
    /// - `namespace_id`: [`NamespaceId`] of the namespace to delete
    ///
    /// # Returns
    /// - `Ok(())` if deletion succeeds
    /// - `Err(KeyValueError::InvalidValue)` if container not found
    /// - `Err(KeyValueError)` if lock poisoned
    ///
    /// # TODO
    /// - Check if any tables or sub-namespaces exist for this namespace and prevent deletion if so
    /// - Persist deletion to container metadata shard
    #[tracing::instrument(skip(self))]
    pub async fn delete_namespace(
        &self,
        principal: &SecurityPrincipal,
        container_id: &ContainerId,
        namespace_id: &NamespaceId,
    ) -> KeyValueResult<()> {
        if !principal.has_namespace_permission(
            &Permission::NamespaceDelete,
            &container_id.tenant(),
            &container_id.database(),
            namespace_id,
        ) {
            return Err(KeyValueError::PermissionDenied {
                user_id: principal.user_id,
                permission: Permission::NamespaceDelete,
                resource: ResourceScope::Database(container_id.database()),
            });
        }
        // Get container metadata cache
        let container_caches = self
            .container_metacaches
            .read()
            .map_err(|_| KeyValueError::LockPoisoned)?;

        let container_cache = container_caches.get(container_id).ok_or_else(|| {
            KeyValueError::InvalidValue(format!("Container not found: {:?}", container_id))
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
    /// # Access Control
    /// - Requires [`Permission::SystemSecurityManage`] on [`ResourceScope::System`]
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
        container_id: &ContainerId,
    ) -> KeyValueResult<impl IntoIterator<Item = TableRecord>> {
        if !principal.has_database_permission(
            &Permission::TableRead,
            &container_id.tenant(),
            &container_id.database(),
        ) {
            return Err(KeyValueError::PermissionDenied {
                user_id: principal.user_id,
                permission: Permission::TableRead,
                resource: ResourceScope::Database(container_id.database()),
            });
        }
        // Get container metadata cache
        let container_caches = self
            .container_metacaches
            .read()
            .map_err(|_| KeyValueError::LockPoisoned)?;

        let container_cache = container_caches.get(container_id).ok_or_else(|| {
            KeyValueError::InvalidValue(format!("Container not found: {:?}", container_id))
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
    /// # Access Control
    /// - Requires [`Permission::SystemSecurityManage`] on [`ResourceScope::System`]
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
        container_id: &ContainerId,
        namespace_id: &NamespaceId,
    ) -> KeyValueResult<impl IntoIterator<Item = TableRecord>> {
        // TODO: Fix Permissions
        if !principal.has_permission(&Permission::NamespaceList) {
            return Err(KeyValueError::PermissionDenied {
                user_id: principal.user_id,
                permission: Permission::NamespaceList,
                resource: ResourceScope::Database(container_id.database()),
            });
        }
        // Get container metadata cache
        let container_caches = self
            .container_metacaches
            .read()
            .map_err(|_| KeyValueError::LockPoisoned)?;

        let container_cache = container_caches.get(container_id).ok_or_else(|| {
            KeyValueError::InvalidValue(format!("Container not found: {:?}", container_id))
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
                table
                    .path
                    .contains(&format!("ns_{}", namespace_id.as_u64()))
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
    /// # Access Control
    /// - Requires [`Permission::SystemSecurityManage`] on [`ResourceScope::System`]
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
        container_id: &ContainerId,
        prefix: &str,
    ) -> KeyValueResult<impl IntoIterator<Item = TableRecord>> {
        // TODO: Fix Permissions
        if !principal.has_permission(&Permission::NamespaceList) {
            return Err(KeyValueError::PermissionDenied {
                user_id: principal.user_id,
                permission: Permission::NamespaceList,
                resource: ResourceScope::Database(container_id.database()),
            });
        }
        // Get container metadata cache
        let container_caches = self
            .container_metacaches
            .read()
            .map_err(|_| KeyValueError::LockPoisoned)?;

        let container_cache = container_caches.get(container_id).ok_or_else(|| {
            KeyValueError::InvalidValue(format!("Container not found: {:?}", container_id))
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
    /// # Access Control
    /// - Requires [`Permission::SystemSecurityManage`] on [`ResourceScope::System`]
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
        container_id: &ContainerId,
        table_id: &TableId,
    ) -> KeyValueResult<Option<TableRecord>> {
        if !principal.has_database_permission(
            &Permission::TableRead,
            &container_id.tenant(),
            &container_id.database(),
        ) {
            return Err(KeyValueError::PermissionDenied {
                user_id: principal.user_id,
                permission: Permission::TableRead,
                resource: ResourceScope::Database(container_id.database()),
            });
        }
        // Get container metadata cache
        let container_caches = self
            .container_metacaches
            .read()
            .map_err(|_| KeyValueError::LockPoisoned)?;

        let container_cache = container_caches.get(container_id).ok_or_else(|| {
            KeyValueError::InvalidValue(format!("Container not found: {:?}", container_id))
        })?;

        let cache = container_cache
            .read()
            .map_err(|_| KeyValueError::LockPoisoned)?;

        // Get table from cache
        Ok(cache.get_table_record(table_id).cloned())
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
    /// # Access Control
    /// - Requires [`Permission::SystemSecurityManage`] on [`ResourceScope::System`]
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
        container_id: &ContainerId,
        config: TableCreate,
    ) -> KeyValueResult<TableRecord> {
        if !principal.has_database_permission(
            &Permission::TableCreate,
            &container_id.tenant(),
            &container_id.database(),
        ) {
            return Err(KeyValueError::PermissionDenied {
                user_id: principal.user_id,
                permission: Permission::TableCreate,
                resource: ResourceScope::Database(container_id.database()),
            });
        }
        // TODO: Get/Create actual new table ID
        let table_id = {
            let container_caches = self
                .container_metacaches
                .read()
                .map_err(|_| KeyValueError::LockPoisoned)?;

            let container_cache = container_caches.get(container_id);
            if let Some(cc) = container_cache {
                let cache = cc.read().map_err(|_| KeyValueError::LockPoisoned)?;
                TableId::new(
                    cache
                        .list_table_records()
                        .map(|t| t.id.as_u64() as u32)
                        .max()
                        .map(|id| id + 1)
                        .unwrap_or(0),
                )
            } else {
                TableId::new(0)
            }
        };

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
            // Standalone mode: Create shards locally
            let shard_manager = self.shard_manager.read().unwrap();
            let now = Timestamp::now();

            let shard_ids = match config.sharding_config {
                TableSharding::Single => {
                    let shard_id = ShardId::from_parts(table_id, ShardIndex::new(0));
                    let shard_config = ShardCreate::new(table_id, ShardIndex::new(0), config.engine_type.clone());
                    shard_manager.create_shard(shard_config).await?;
                    vec![shard_id]
                }
                TableSharding::Multiple { shard_count, .. } => {
                    let mut shards = Vec::new();
                    for shard_index in 0..shard_count {
                        let shard_id = ShardId::from_parts(table_id, ShardIndex::new(shard_index));
                        let shard_config = ShardCreate::new(table_id, ShardIndex::new(shard_index), config.engine_type.clone());
                        shard_manager.create_shard(shard_config).await?;
                        shards.push(shard_id);
                    }
                    shards
                }
            };

            // Create table record
            let table_record = TableRecord {
                id: table_id,
                name: config.name.clone(),
                path: config.path.clone(),
                version: 1,
                created_at: now,
                last_modified: now,
                engine_type: config.engine_type.clone(),
                sharding: config.sharding_config.clone(),
                options: config.options.clone(),
                metadata: config.metadata.clone(),
            };

            // Store in container cache
            let mut container_caches = self.container_metacaches.write().unwrap();
            let container_cache = container_caches
                .entry(*container_id)
                .or_insert_with(|| {
                    // System Shard 0 is used for container metadata
                    Arc::new(RwLock::new(ContainerMetadataCache::new(*container_id, ShardId::from(0), Duration::from_secs(60))))
                });
            
            let mut cache = container_cache.write().unwrap();
            cache.set_table_record(table_record.clone());

            // Create shard records in cache
            for shard_id in shard_ids {
                let shard_record = ShardRecord {
                    id: shard_id,
                    name: format!("{}-shard-{}", config.name, shard_id.index()),
                    version: 1,
                    engine_type: config.engine_type.clone(),
                    created_at: now,
                    last_modified: now,
                    range: (vec![], vec![0xFF; 32]), // Default full range
                    leader: Some(self.node_id),
                    replicas: vec![self.node_id],
                    status: Default::default(),
                    term: 1,
                    size_bytes: 0,
                };
                cache.set_shard_record(shard_record);
            }

            Ok(table_record)
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
    /// # Access Control
    /// - Requires [`Permission::TableAlter`] on [`ResourceScope::Table`]
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
        container_id: &ContainerId,
        table_id: &TableId,
        config: TableUpdate,
    ) -> KeyValueResult<TableRecord> {
        if !principal.has_table_permission(
            &Permission::TableAlter,
            &container_id.tenant(),
            &container_id.database(),
            table_id,
        ) {
            return Err(KeyValueError::PermissionDenied {
                user_id: principal.user_id,
                permission: Permission::TableAlter,
                resource: ResourceScope::Table(*table_id),
            });
        }
        // Get container metadata cache
        let container_caches = self
            .container_metacaches
            .read()
            .map_err(|_| KeyValueError::LockPoisoned)?;

        let container_cache = container_caches.get(container_id).ok_or_else(|| {
            KeyValueError::InvalidValue(format!("Container not found: {:?}", container_id))
        })?;

        let mut cache = container_cache
            .write()
            .map_err(|_| KeyValueError::LockPoisoned)?;

        // Get existing table metadata
        let mut table_meta = cache.get_table_record(table_id).cloned().ok_or_else(|| {
            KeyValueError::InvalidValue(format!("Table not found: {:?}", table_id))
        })?;

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
    /// # Access Control
    /// - Requires [`Permission::SystemSecurityManage`] on [`ResourceScope::System`]
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
        container_id: &ContainerId,
        table_id: &TableId,
    ) -> KeyValueResult<()> {
        if !principal.has_database_permission(
            &Permission::TableDrop,
            &container_id.tenant(),
            &container_id.database(),
        ) {
            return Err(KeyValueError::PermissionDenied {
                user_id: principal.user_id,
                permission: Permission::TableDrop,
                resource: ResourceScope::Database(container_id.database()),
            });
        }
        // Get container metadata cache
        let container_caches = self
            .container_metacaches
            .read()
            .map_err(|_| KeyValueError::LockPoisoned)?;

        let container_cache = container_caches.get(container_id).ok_or_else(|| {
            KeyValueError::InvalidValue(format!("Container not found: {:?}", container_id))
        })?;

        let mut cache = container_cache
            .write()
            .map_err(|_| KeyValueError::LockPoisoned)?;

        // Verify table exists
        if cache.get_table_record(table_id).is_none() {
            return Err(KeyValueError::InvalidValue(format!(
                "Table not found: {:?}",
                table_id
            )));
        }

        // Remove from cache
        cache.clear_table_record(*table_id);

        // TODO: Delete all shards associated with this table
        // TODO: Persist deletion to container metadata shard
        // let container_shard = cache.metadata_shard_id();
        // self.shard_manager.delete(container_shard, key).await?;

        Ok(())
    }

    /**********************************************************************************************\
     * Data Management                                                                            *
    \**********************************************************************************************/

    /// Write a key-value pair to a table.
    ///
    /// # What it does
    /// Inserts or updates a value for a given key in the specified table.
    ///
    /// # How it works
    /// 1. Determines the target shard using [`get_shard_for_key`]
    /// 2. If in distributed mode: routes the write through [`ConsensusRouter`] via Raft
    /// 3. If in standalone mode: writes directly to the shard via [`KeyValueShardManager`]
    ///
    /// # Access Control
    /// - Requires [`Permission::NamespaceList`] (TODO: correct to Write permission)
    ///
    /// # Parameters
    /// - `principal`: [`SecurityPrincipal`] for authorization
    /// - `container`: [`ContainerId`] containing the table
    /// - `table`: [`TableId`] of the target table
    /// - `key`: Key bytes to write
    /// - `value`: Value bytes to write
    ///
    /// # Returns
    /// - `Ok(())` if write succeeds
    /// - `Err(KeyValueError)` if consensus fails, lock poisoned, or write fails
    ///
    /// # TODO
    /// - Implement proper write permissions check
    /// - Optimize for batch writes in distributed mode
    #[tracing::instrument(skip(self))]
    pub async fn put(
        &self,
        principal: &SecurityPrincipal,
        container_id: &ContainerId,
        table_id: &TableId,
        key: &[u8],
        value: &[u8],
    ) -> KeyValueResult<()> {
        if !principal.has_table_permission(
            &Permission::TableWrite,
            &container_id.tenant(),
            &container_id.database(),
            table_id,
        ) {
            return Err(KeyValueError::PermissionDenied {
                user_id: principal.user_id,
                permission: Permission::TableWrite,
                resource: ResourceScope::Table(*table_id),
            });
        }
        let shard_id = self.get_shard_for_key(container_id, table_id, key)?;

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

    /// Read a value by key from a table.
    ///
    /// # What it does
    /// Retrieves the value associated with the given key from the specified table.
    ///
    /// # How it works
    /// 1. Determines the target shard using [`get_shard_for_key`]
    /// 2. If in distributed mode: routes the read through [`ConsensusRouter`]
    /// 3. If in standalone mode: reads directly from the shard via [`KeyValueShardManager`]
    ///
    /// # Access Control
    /// - Requires [`Permission::NamespaceList`] (TODO: correct to Read permission)
    ///
    /// # Parameters
    /// - `principal`: [`SecurityPrincipal`] for authorization
    /// - `container`: [`ContainerId`] containing the table
    /// - `table`: [`TableId`] of the target table
    /// - `key`: Key bytes to look up
    ///
    /// # Returns
    /// - `Ok(Some(Vec<u8>))` if key exists
    /// - `Ok(None)` if key not found
    /// - `Err(KeyValueError)` if consensus fails, lock poisoned, or read fails
    ///
    /// # TODO
    /// - Implement proper read permissions check
    /// - Implement local reads for distributed mode if consistency allow it
    #[tracing::instrument(skip(self))]
    pub async fn get(
        &self,
        principal: &SecurityPrincipal,
        container_id: &ContainerId,
        table_id: &TableId,
        key: &[u8],
    ) -> KeyValueResult<Option<Vec<u8>>> {
        if !principal.has_table_permission(
            &Permission::TableRead,
            &container_id.tenant(),
            &container_id.database(),
            table_id,
        ) {
            return Err(KeyValueError::PermissionDenied {
                user_id: principal.user_id,
                permission: Permission::TableRead,
                resource: ResourceScope::Table(*table_id),
            });
        }
        let shard_id = self.get_shard_for_key(container_id, table_id, key)?;

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

    /// Delete a key-value pair from a table.
    ///
    /// # What it does
    /// Removes the mapping for a given key from the specified table.
    ///
    /// # How it works
    /// 1. Determines the target shard using [`get_shard_for_key`]
    /// 2. If in distributed mode: routes the delete through [`ConsensusRouter`] via Raft
    /// 3. If in standalone mode: deletes directly from the shard via [`KeyValueShardManager`]
    ///
    /// # Access Control
    /// - Requires [`Permission::NamespaceList`] (TODO: correct to Write permission)
    ///
    /// # Parameters
    /// - `principal`: [`SecurityPrincipal`] for authorization
    /// - `container`: [`ContainerId`] containing the table
    /// - `table`: [`TableId`] of the target table
    /// - `key`: Key bytes to delete
    ///
    /// # Returns
    /// - `Ok(true)` if delete succeeds
    /// - `Err(KeyValueError)` if consensus fails, lock poisoned, or deletion fails
    ///
    /// # TODO
    /// - Implement proper write permissions check
    #[tracing::instrument(skip(self))]
    pub async fn delete(
        &self,
        principal: &SecurityPrincipal,
        container_id: &ContainerId,
        table_id: &TableId,
        key: &[u8],
    ) -> KeyValueResult<bool> {
        if !principal.has_table_permission(
            &Permission::TableDelete,
            &container_id.tenant(),
            &container_id.database(),
            table_id,
        ) {
            return Err(KeyValueError::PermissionDenied {
                user_id: principal.user_id,
                permission: Permission::TableDelete,
                resource: ResourceScope::Table(*table_id),
            });
        }
        let shard_id = self.get_shard_for_key(container_id, table_id, key)?;

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

    /// Perform multiple put operations in a single batch.
    ///
    /// # What it does
    /// Writes multiple key-value pairs to the same table efficiently.
    ///
    /// # How it works
    /// 1. For each key, determines target shard
    /// 2. If in distributed mode: routes batch through [`ConsensusRouter`] (TODO)
    /// 3. If in standalone mode: writes directly to the shard(s) via [`KeyValueShardManager`]
    ///
    /// # Access Control
    /// - Requires [`Permission::NamespaceList`] (TODO: correct to Write permission)
    ///
    /// # Parameters
    /// - `principal`: [`SecurityPrincipal`] for authorization
    /// - `container`: [`ContainerId`] containing the table
    /// - `table`: [`TableId`] of the target table
    /// - `pairs`: Slice of key-value byte pairs to write
    ///
    /// # Returns
    /// - `Ok(())` if all writes succeed
    /// - `Err(KeyValueError)` if lock poisoned or any write fails
    ///
    /// # TODO
    /// - Implement proper write permissions check
    /// - Implement batch support in distributed mode (Raft)
    /// - Optimize for cases where pairs span multiple shards
    #[tracing::instrument(skip(self))]
    pub async fn batch_put(
        &self,
        principal: &SecurityPrincipal,
        container_id: &ContainerId,
        table_id: &TableId,
        pairs: &[(&[u8], &[u8])],
    ) -> KeyValueResult<()> {
        if !principal.has_table_permission(
            &Permission::TableWrite,
            &container_id.tenant(),
            &container_id.database(),
            table_id,
        ) {
            return Err(KeyValueError::PermissionDenied {
                user_id: principal.user_id,
                permission: Permission::TableWrite,
                resource: ResourceScope::Table(*table_id),
            });
        }
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
                let shard_id = self.get_shard_for_key(container_id, table_id, key)?;
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
        container_id: &ContainerId,
        table_id: &TableId,
        key: &[u8],
    ) -> KeyValueResult<ShardId> {
        // Get container metadata cache
        let container_caches = self
            .container_metacaches
            .read()
            .map_err(|_| KeyValueError::LockPoisoned)?;

        let container_cache = container_caches.get(container_id).ok_or_else(|| {
            KeyValueError::InvalidValue(format!("Container not found: {:?}", container_id))
        })?;

        let cache = container_cache
            .read()
            .map_err(|_| KeyValueError::LockPoisoned)?;

        // Get table metadata to determine sharding strategy
        let table_meta = cache.get_table_record(table_id).ok_or_else(|| {
            KeyValueError::InvalidValue(format!("Table not found: {:?}", table_id))
        })?;

        match &table_meta.sharding {
            TableSharding::Single => {
                // Single shard table - always use shard index 0
                Ok(ShardId::from_parts(*table_id, ShardIndex::new(0)))
            }
            TableSharding::Multiple {
                shard_count,
                partitioner,
                ..
            } => {
                // Multi-shard table - use partitioner's built-in logic
                let shard_index = partitioner.get_shard_index(key, *shard_count);
                Ok(ShardId::from_parts(*table_id, shard_index))
            }
        }
    }
}
