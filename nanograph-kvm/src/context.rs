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
use crate::shardmgr::KeyValueShardManager;
use crate::utility::{SystemKeys, deserialize};
use nanograph_core::{
    object::{
        ClusterId, ContainerId, DatabaseCreate, DatabaseId, DatabaseMetadata, DatabaseUpdate,
        NamespaceCreate, NamespaceId, NamespaceMetadata, NamespaceUpdate, NodeId, ObjectId,
        ObjectMetadata, ObjectType, RegionId, ServerId, ShardId, ShardIndex, ShardMetadata,
        TableCreate, TableId, TableMetadata, TableSharding, TableUpdate, TablespaceCreate,
        TablespaceId, TablespaceMetadata, TablespaceUpdate, TenantCreate, TenantId, TenantMetadata,
        TenantUpdate, UserCreate, UserId, UserMetadata, UserUpdate,
    },
    types::{PropertyUpdate, Timestamp},
};
use nanograph_kvt::{KeyValueError, KeyValueResult};
use nanograph_raft::{
    ClusterCreate, ClusterMetadata, ClusterUpdate, ConsensusRouter, RegionCreate, RegionMetadata,
    RegionUpdate, ServerCreate, ServerMetadata, ServerUpdate,
};
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
    /// Cluster ID,
    cluster_id: ClusterId,
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
    /// Initializes a KeyValueDatabaseContext for standalone (non-distributed) operation.
    /// Creates a new shard manager, system metadata cache, and empty container metadata cache map.
    ///
    /// # How it works
    /// 1. Creates a standalone KeyValueShardManager for local shard storage
    /// 2. Initializes SystemMetadataCache with shard 0 for system metadata
    /// 3. Creates empty HashMap for container-specific metadata caches
    /// 4. Sets raft_router to None (no distributed consensus)
    ///
    /// # Parameters
    /// - `config`: KeyValueDatabaseConfig containing node_id and other configuration
    ///
    /// # Returns
    /// A new KeyValueDatabaseContext configured for single-node operation
    pub fn new_standalone(config: KeyValueDatabaseConfig) -> Self {
        let shard_manager = Arc::new(RwLock::new(KeyValueShardManager::new_standalone()));
        let system_metacache = Arc::new(RwLock::new(SystemMetadataCache::new(ShardId::from(0))));
        let container_metacaches = Arc::new(RwLock::new(HashMap::new()));
        Self {
            cluster_id: config.node_id.cluster_id(),
            shard_manager,
            system_metacache,
            container_metacaches,
            raft_router: None,
        }
    }

    /// Create a new database context in distributed mode.
    ///
    /// # What it does
    /// Initializes a KeyValueDatabaseContext for distributed operation with Raft consensus.
    /// Similar to standalone mode but includes a Raft router for coordinating operations across nodes.
    ///
    /// # How it works
    /// 1. Creates a standalone KeyValueShardManager (will be coordinated via Raft)
    /// 2. Initializes SystemMetadataCache with shard 0 for system metadata
    /// 3. Creates empty HashMap for container-specific metadata caches
    /// 4. Stores the provided raft_router for distributed consensus operations
    ///
    /// # Parameters
    /// - `config`: KeyValueDatabaseConfig containing node_id and other configuration
    /// - `raft_router`: Arc-wrapped ConsensusRouter for distributed coordination
    ///
    /// # Returns
    /// A new KeyValueDatabaseContext configured for distributed operation
    pub fn new_distributed(
        config: KeyValueDatabaseConfig,
        raft_router: Arc<ConsensusRouter>,
    ) -> Self {
        let shard_manager = Arc::new(RwLock::new(KeyValueShardManager::new_standalone()));
        let system_metacache = Arc::new(RwLock::new(SystemMetadataCache::new(ShardId::from(0))));
        let container_metacaches = Arc::new(RwLock::new(HashMap::new()));
        Self {
            cluster_id: config.node_id.cluster_id(),
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

    /// Get the local node ID (if in distributed mode).
    ///
    /// # What it does
    /// Returns the NodeId of this node in a distributed cluster.
    ///
    /// # How it works
    /// Queries the raft_router for the node_id if it exists.
    ///
    /// # Returns
    /// - `Some(NodeId)` if running in distributed mode
    /// - `None` if running in standalone mode
    pub fn node_id(&self) -> Option<NodeId> {
        self.raft_router
            .as_ref()
            .map(|raft_router| raft_router.node_id())
    }

    /// Get the cluster ID.
    ///
    /// # What it does
    /// Returns the ClusterId that this context belongs to.
    ///
    /// # How it works
    /// Returns the cluster_id field that was set during construction from the config.
    ///
    /// # Returns
    /// The ClusterId for this database context
    pub fn cluster_id(&self) -> ClusterId {
        self.cluster_id
    }

    /// Get the Raft router (if in distributed mode).
    ///
    /// # What it does
    /// Returns a reference to the ConsensusRouter for distributed operations.
    ///
    /// # How it works
    /// Returns a reference to the raft_router if it exists.
    ///
    /// # Returns
    /// - `Some(&Arc<ConsensusRouter>)` if running in distributed mode
    /// - `None` if running in standalone mode
    pub fn consensus_router(&self) -> Option<&Arc<ConsensusRouter>> {
        self.raft_router.as_ref()
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
    /// 1. Creates ClusterMetadata with current timestamp and version 1
    /// 2. Stores metadata in system_metacache for fast access
    /// 3. Serializes and persists metadata to system shard (ShardId 0)
    /// 4. Uses SystemKeys::cluster_key for the storage key
    ///
    /// # Parameters
    /// - `config`: ClusterCreate containing name, options, and metadata
    ///
    /// # Returns
    /// - `Ok(())` if cluster initialization succeeds
    /// - `Err(KeyValueError)` if lock poisoned or persistence fails
    #[tracing::instrument(skip(self))]
    pub async fn initialize_cluster(&self, config: ClusterCreate) -> KeyValueResult<()> {
        let now = Timestamp::now();

        // Create cluster metadata
        let cluster = ClusterMetadata {
            id: self.cluster_id,
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
        let key = SystemKeys::cluster_key(self.cluster_id);
        let value = crate::utility::serialize(&cluster)?;

        let shard_manager = self
            .shard_manager
            .read()
            .map_err(|_| KeyValueError::LockPoisoned)?;
        shard_manager.put(ShardId::from(0), &key, &value).await?;

        Ok(())
    }

    /// Get the cluster metadata.
    ///
    /// # What it does
    /// Retrieves the metadata for this cluster, checking cache first then disk.
    ///
    /// # How it works
    /// 1. First checks system_metacache for cached cluster record
    /// 2. If not in cache, reads from system shard (ShardId 0) using SystemKeys::cluster_key
    /// 3. Deserializes the stored metadata
    /// 4. Returns error if cluster metadata not found
    ///
    /// # Returns
    /// - `Ok(ClusterMetadata)` with the cluster information
    /// - `Err(KeyValueError)` if not found, lock poisoned, or deserialization fails
    #[tracing::instrument(skip(self))]
    pub async fn get_cluster(&self) -> KeyValueResult<ClusterMetadata> {
        {
            // First Check the cache
            let lock = self
                .system_metacache
                .read()
                .map_err(|_| KeyValueError::LockPoisoned)?;
            if let Some(record) = lock.get_cluster_record() {
                return Ok(record.clone());
            }
        }
        {
            // Second read from Disk
            let lock = self
                .shard_manager
                .read()
                .map_err(|_| KeyValueError::LockPoisoned)?;
            if let Some(value) = lock
                .get(ShardId::from(0), &SystemKeys::cluster_key(self.cluster_id))
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
    /// # Parameters
    /// - `config`: ClusterUpdate containing optional name and other updates
    ///
    /// # Returns
    /// - `Ok(())` if update succeeds
    /// - `Err(KeyValueError::InvalidValue)` if cluster not initialized
    /// - `Err(KeyValueError)` if lock poisoned or persistence fails
    #[tracing::instrument(skip(self))]
    pub async fn update_cluster(&self, config: ClusterUpdate) -> KeyValueResult<()> {
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
            let key = SystemKeys::cluster_key(self.cluster_id);
            let value = crate::utility::serialize(&cluster)?;

            let shard_manager = self
                .shard_manager
                .read()
                .map_err(|_| KeyValueError::LockPoisoned)?;
            shard_manager.put(ShardId::from(0), &key, &value).await?;

            Ok(())
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
    /// # Returns
    /// An iterator over RegionMetadata for all regions
    ///
    /// # TODO
    /// - Implement proper region enumeration from disk if not in cache
    /// - Add pagination support for large numbers of regions
    pub async fn get_regions(&self) -> KeyValueResult<impl IntoIterator<Item = RegionMetadata>> {
        let lock = self.system_metacache.read().unwrap();
        Ok(lock.list_region_records().cloned().collect::<Vec<_>>())
    }

    /// Get metadata about a specific region.
    ///
    /// # What it does
    /// Retrieves metadata for a single region by its ID.
    ///
    /// # How it works
    /// Reads from system_metacache to find the region record.
    ///
    /// # Parameters
    /// - `region`: RegionId to look up
    ///
    /// # Returns
    /// - `Some(RegionMetadata)` if region exists
    /// - `None` if region not found
    ///
    /// # TODO
    /// - Implement fallback to disk if not in cache
    pub async fn get_region(&self, region: RegionId) -> KeyValueResult<Option<RegionMetadata>> {
        let lock = self.system_metacache.read().unwrap();
        Ok(lock.get_region_record(&region).cloned())
    }

    /// Get region metadata by name.
    ///
    /// # What it does
    /// Finds a region by its name string.
    ///
    /// # How it works
    /// Iterates through cached region records to find one matching the name.
    ///
    /// # Parameters
    /// - `name`: Region name to search for
    ///
    /// # Returns
    /// - `Some(RegionMetadata)` if region with that name exists
    /// - `None` if no matching region found
    ///
    /// # TODO
    /// - Implement proper region lookup from disk when cache is not available
    /// - Add index for name-based lookups
    pub async fn get_region_by_name(&self, name: &str) -> KeyValueResult<Option<RegionMetadata>> {
        let lock = self.system_metacache.read().unwrap();
        Ok(lock.list_region_records().cloned().find(|r| r.name == name))
    }

    /// Add a new region to the cluster.
    ///
    /// # What it does
    /// Creates a new region with the provided configuration.
    ///
    /// # How it works
    /// 1. Generates a new RegionId (currently hardcoded to 0)
    /// 2. Creates RegionMetadata with current timestamp and version 1
    /// 3. Stores in system_metacache
    /// 4. Serializes and persists to system shard using SystemKeys::region_key
    ///
    /// # Parameters
    /// - `config`: RegionCreate containing name, cluster, options, and metadata
    ///
    /// # Returns
    /// - `Ok(RegionMetadata)` with the created region information
    /// - `Err(KeyValueError)` if lock poisoned or persistence fails
    ///
    /// # TODO
    /// - Generate proper unique region ID instead of hardcoded 0
    pub async fn add_region(&self, config: RegionCreate) -> KeyValueResult<RegionMetadata> {
        let now = Timestamp::now();

        // TODO: Generate proper region ID
        let region_id = RegionId::from(0);

        // Create region metadata
        let region = RegionMetadata {
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
        let key = SystemKeys::region_key(self.cluster_id, region_id);
        let value = crate::utility::serialize(&region)?;

        let shard_manager = self
            .shard_manager
            .read()
            .map_err(|_| KeyValueError::LockPoisoned)?;
        shard_manager.put(ShardId::from(0), &key, &value).await?;

        Ok(region)
    }
    /// TODO: Add Documentation
    #[tracing::instrument(skip(self))]
    pub async fn update_region(
        &self,
        region: &RegionId,
        config: RegionUpdate,
    ) -> KeyValueResult<RegionMetadata> {
        let mut cache = self
            .system_metacache
            .write()
            .map_err(|_| KeyValueError::LockPoisoned)?;

        if let Some(mut region_meta) = cache.get_region_record(region).cloned() {
            // Update fields
            if let Some(name) = config.name {
                region_meta.name = name;
            }

            // Update version and timestamp
            region_meta.version += 1;
            region_meta.last_modified = Timestamp::now();

            // Store in cache
            cache.set_region_record(region_meta.clone());
            drop(cache);

            // Persist to system shard
            let key = SystemKeys::region_key(self.cluster_id, *region);
            let value = crate::utility::serialize(&region_meta)?;

            let shard_manager = self
                .shard_manager
                .read()
                .map_err(|_| KeyValueError::LockPoisoned)?;
            shard_manager.put(ShardId::from(0), &key, &value).await?;

            Ok(region_meta)
        } else {
            Err(KeyValueError::InvalidValue(format!(
                "Region not found: {:?}",
                region
            )))
        }
    }
    #[tracing::instrument(skip(self))]
    pub async fn remove_region(&self, region: &RegionId) -> KeyValueResult<()> {
        // Remove from cache
        {
            let mut cache = self
                .system_metacache
                .write()
                .map_err(|_| KeyValueError::LockPoisoned)?;
            cache.clear_region_record(region);
        }

        // Delete from system shard
        let key = SystemKeys::region_key(self.cluster_id, *region);

        let shard_manager = self
            .shard_manager
            .read()
            .map_err(|_| KeyValueError::LockPoisoned)?;
        shard_manager.delete(ShardId::from(0), &key).await?;

        Ok(())
    }

    #[tracing::instrument(skip(self))]
    pub async fn get_servers(&self) -> KeyValueResult<impl IntoIterator<Item = ServerMetadata>> {
        // TODO: Implement Proper Get Server Logic
        let lock = self.system_metacache.read().unwrap();
        Ok(lock.list_server_records().cloned().collect::<Vec<_>>())
    }

    #[tracing::instrument(skip(self))]
    pub async fn get_servers_by_region(
        &self,
        region_id: &RegionId,
    ) -> KeyValueResult<impl IntoIterator<Item = ServerMetadata>> {
        // TODO: Implement Proper Get Server By Region Logic
        let lock = self.system_metacache.read().unwrap();
        Ok(lock.list_server_records().cloned().collect::<Vec<_>>())
    }

    #[tracing::instrument(skip(self))]
    pub async fn get_server(&self, server: &NodeId) -> KeyValueResult<Option<ServerMetadata>> {
        // Check cache first
        {
            let cache = self
                .system_metacache
                .read()
                .map_err(|_| KeyValueError::LockPoisoned)?;
            if let Some(server_meta) = cache.get_server_record(&server.server_id()) {
                return Ok(Some(server_meta.clone()));
            }
        }

        // Read from disk
        let key = SystemKeys::server_key(self.cluster_id, server.region_id(), server.server_id());
        let shard_manager = self
            .shard_manager
            .read()
            .map_err(|_| KeyValueError::LockPoisoned)?;

        if let Some(value) = shard_manager.get(ShardId::from(0), &key).await? {
            let server_meta: ServerMetadata = deserialize(&value)?;

            // Update cache
            let mut cache = self
                .system_metacache
                .write()
                .map_err(|_| KeyValueError::LockPoisoned)?;
            cache.set_server_record(server_meta.clone());

            Ok(Some(server_meta))
        } else {
            Ok(None)
        }
    }

    #[tracing::instrument(skip(self))]
    pub async fn add_server(&self, config: ServerCreate) -> KeyValueResult<ServerMetadata> {
        let now = Timestamp::now();

        // Generate NodeId from region and cluster
        let node_id = NodeId::from_parts(config.cluster, config.region, ServerId::from(0));

        // Create server metadata
        let server = ServerMetadata {
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
        let key = SystemKeys::server_key(self.cluster_id, config.region, ServerId::from(0));
        let value = crate::utility::serialize(&server)?;

        let shard_manager = self
            .shard_manager
            .read()
            .map_err(|_| KeyValueError::LockPoisoned)?;
        shard_manager.put(ShardId::from(0), &key, &value).await?;

        Ok(server)
    }

    #[tracing::instrument(skip(self))]
    pub async fn update_server(
        &self,
        server: &NodeId,
        config: ServerUpdate,
    ) -> KeyValueResult<ServerMetadata> {
        let mut cache = self
            .system_metacache
            .write()
            .map_err(|_| KeyValueError::LockPoisoned)?;

        if let Some(mut server_meta) = cache.get_server_record(&server.server_id()).cloned() {
            // Update fields
            if let Some(name) = config.name {
                server_meta.name = name;
            }

            // Update version and timestamp
            server_meta.version += 1;
            server_meta.last_modified = Timestamp::now();

            // Store in cache
            cache.set_server_record(server_meta.clone());
            drop(cache);

            // Persist to system shard
            let key =
                SystemKeys::server_key(self.cluster_id, server.region_id(), server.server_id());
            let value = crate::utility::serialize(&server_meta)?;

            let shard_manager = self
                .shard_manager
                .read()
                .map_err(|_| KeyValueError::LockPoisoned)?;
            shard_manager.put(ShardId::from(0), &key, &value).await?;

            Ok(server_meta)
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
    /// 2. Deletes persisted metadata from system shard using SystemKeys::server_key
    ///
    /// # Parameters
    /// - `server`: NodeId to remove
    ///
    /// # Returns
    /// - `Ok(())` if removal succeeds
    /// - `Err(KeyValueError)` if lock poisoned or deletion fails
    ///
    /// # TODO
    /// - Add validation to prevent deletion if server is hosting shards
    /// - Implement graceful server removal with shard migration
    #[tracing::instrument(skip(self))]
    pub async fn remove_server(&self, server: &NodeId) -> KeyValueResult<()> {
        // Remove from cache
        {
            let mut cache = self
                .system_metacache
                .write()
                .map_err(|_| KeyValueError::LockPoisoned)?;
            cache.clear_server_record(&server.server_id());
        }

        // Delete from system shard
        let key = SystemKeys::server_key(self.cluster_id, server.region_id(), server.server_id());

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
    /// # Returns
    /// An iterator over UserMetadata for all users
    ///
    /// # TODO
    /// - Implement pagination support for large numbers of users
    /// - Add disk fallback if not in cache
    #[tracing::instrument(skip(self))]
    pub async fn get_users(&self) -> KeyValueResult<impl IntoIterator<Item = UserMetadata>> {
        let cache = self
            .system_metacache
            .read()
            .map_err(|_| KeyValueError::LockPoisoned)?;
        Ok(cache.list_user_records().cloned().collect::<Vec<_>>())
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
    /// # Parameters
    /// - `user`: UserId to look up
    ///
    /// # Returns
    /// - `Some(UserMetadata)` if user exists
    /// - `None` if user not found
    /// - `Err(KeyValueError)` if lock poisoned or deserialization fails
    #[tracing::instrument(skip(self))]
    pub async fn get_user(&self, user: UserId) -> KeyValueResult<Option<UserMetadata>> {
        // Check cache first
        {
            let cache = self
                .system_metacache
                .read()
                .map_err(|_| KeyValueError::LockPoisoned)?;
            if let Some(user_meta) = cache.get_user_record(&user) {
                return Ok(Some(user_meta.clone()));
            }
        }

        // Read from disk
        let key = SystemKeys::user_key(user);
        let shard_manager = self
            .shard_manager
            .read()
            .map_err(|_| KeyValueError::LockPoisoned)?;

        if let Some(value) = shard_manager.get(ShardId::from(0), &key).await? {
            let user_meta: UserMetadata = deserialize(&value)?;

            // Update cache
            let mut cache = self
                .system_metacache
                .write()
                .map_err(|_| KeyValueError::LockPoisoned)?;
            cache.set_user_record(user_meta.clone());

            Ok(Some(user_meta))
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
    /// # Parameters
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
        username: &str,
    ) -> KeyValueResult<Option<UserMetadata>> {
        // Search in cache by name (UserMetadata doesn't have a login field, using name as identifier)
        let cache = self
            .system_metacache
            .read()
            .map_err(|_| KeyValueError::LockPoisoned)?;
        let user = cache
            .list_user_records()
            .find(|u| u.username == username)
            .cloned();
        Ok(user)
    }

    /// Create a new user.
    ///
    /// # What it does
    /// Creates a new user with the provided configuration.
    ///
    /// # How it works
    /// 1. Generates a new UserId (currently hardcoded to 0)
    /// 2. Creates UserMetadata with current timestamp, version 1, and empty groups/roles/grants
    /// 3. Stores in system_metacache
    /// 4. Serializes and persists to system shard using SystemKeys::user_key
    ///
    /// # Parameters
    /// - `config`: UserCreate containing name, options, and metadata
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
    pub async fn create_user(&self, config: UserCreate) -> KeyValueResult<UserMetadata> {
        let now = Timestamp::now();

        // TODO: Generate proper user ID
        let user_id = UserId::from(0);

        // Create user metadata
        let user = UserMetadata {
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
            email: None,
            options: config.options.clone(),
            metadata: config.metadata.clone(),
        };

        // Store in cache
        {
            let mut cache = self
                .system_metacache
                .write()
                .map_err(|_| KeyValueError::LockPoisoned)?;
            cache.set_user_record(user.clone());
        }

        // Persist to system shard
        let key = SystemKeys::user_key(user_id);
        let value = crate::utility::serialize(&user)?;

        let shard_manager = self
            .shard_manager
            .read()
            .map_err(|_| KeyValueError::LockPoisoned)?;
        shard_manager.put(ShardId::from(0), &key, &value).await?;

        Ok(user)
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
    /// # Parameters
    /// - `user`: UserId to update
    /// - `config`: UserUpdate containing optional name, options, and metadata updates
    ///
    /// # Returns
    /// - `Ok(UserMetadata)` with updated user information
    /// - `Err(KeyValueError::InvalidValue)` if user not found
    /// - `Err(KeyValueError)` if lock poisoned or persistence fails
    #[tracing::instrument(skip(self))]
    pub async fn update_user(
        &self,
        user: &UserId,
        config: UserUpdate,
    ) -> KeyValueResult<UserMetadata> {
        let mut cache = self
            .system_metacache
            .write()
            .map_err(|_| KeyValueError::LockPoisoned)?;

        if let Some(mut user_meta) = cache.get_user_record(user).cloned() {
            // Update fields
            if let Some(username) = config.username {
                user_meta.username = username;
            }

            // Apply option updates
            for opt_update in &config.options {
                match opt_update {
                    PropertyUpdate::Set(key, value) => {
                        user_meta.options.insert(key.clone(), value.clone());
                    }
                    PropertyUpdate::Clear(key) => {
                        user_meta.options.remove(key);
                    }
                }
            }

            // Apply metadata updates
            for meta_update in &config.metadata {
                match meta_update {
                    PropertyUpdate::Set(key, value) => {
                        user_meta.metadata.insert(key.clone(), value.clone());
                    }
                    PropertyUpdate::Clear(key) => {
                        user_meta.metadata.remove(key);
                    }
                }
            }

            // Update version and timestamp
            user_meta.version += 1;
            user_meta.last_modified = Timestamp::now();

            // Store in cache
            cache.set_user_record(user_meta.clone());
            drop(cache);

            // Persist to system shard
            let key = SystemKeys::user_key(*user);
            let value = crate::utility::serialize(&user_meta)?;

            let shard_manager = self
                .shard_manager
                .read()
                .map_err(|_| KeyValueError::LockPoisoned)?;
            shard_manager.put(ShardId::from(0), &key, &value).await?;

            Ok(user_meta)
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
    /// # Parameters
    /// - `user`: UserId to remove
    ///
    /// # Returns
    /// - `Ok(())` if removal succeeds
    /// - `Err(KeyValueError)` if lock poisoned or deletion fails
    ///
    /// # TODO
    /// - Add validation to check for user's active sessions
    /// - Implement cascade deletion or transfer of user-owned resources
    #[tracing::instrument(skip(self))]
    pub async fn remove_user(&self, user: &UserId) -> KeyValueResult<()> {
        // Remove from cache
        {
            let mut cache = self
                .system_metacache
                .write()
                .map_err(|_| KeyValueError::LockPoisoned)?;
            cache.clear_user_record(user);
        }

        // Delete from system shard
        let key = SystemKeys::user_key(*user);

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
    /// TODO Add Documentation
    #[tracing::instrument(skip(self))]
    pub async fn get_tenants(&self) -> KeyValueResult<impl IntoIterator<Item = TenantMetadata>> {
        let cache = self
            .system_metacache
            .read()
            .map_err(|_| KeyValueError::LockPoisoned)?;
        Ok(cache.list_tenant_records().cloned().collect::<Vec<_>>())
    }

    #[tracing::instrument(skip(self))]
    pub async fn get_tenant(&self, tenant: &TenantId) -> KeyValueResult<Option<TenantMetadata>> {
        // Check cache first
        {
            let cache = self
                .system_metacache
                .read()
                .map_err(|_| KeyValueError::LockPoisoned)?;
            if let Some(tenant_meta) = cache.get_tenant_record(tenant) {
                return Ok(Some(tenant_meta.clone()));
            }
        }

        // Read from disk
        let key = SystemKeys::tenant_key(*tenant);
        let shard_manager = self
            .shard_manager
            .read()
            .map_err(|_| KeyValueError::LockPoisoned)?;

        if let Some(value) = shard_manager.get(ShardId::from(0), &key).await? {
            let tenant_meta: TenantMetadata = deserialize(&value)?;

            // Update cache
            let mut cache = self
                .system_metacache
                .write()
                .map_err(|_| KeyValueError::LockPoisoned)?;
            cache.set_tenant_record(tenant_meta.clone());

            Ok(Some(tenant_meta))
        } else {
            Ok(None)
        }
    }

    #[tracing::instrument(skip(self))]
    pub async fn get_tenant_by_name(&self, name: &str) -> KeyValueResult<Option<TenantId>> {
        // Search in cache
        let cache = self
            .system_metacache
            .read()
            .map_err(|_| KeyValueError::LockPoisoned)?;
        let tenant = cache
            .list_tenant_records()
            .find(|t| t.name == name)
            .map(|t| t.id);
        Ok(tenant)
    }

    #[tracing::instrument(skip(self))]
    pub async fn create_tenant(&self, config: TenantCreate) -> KeyValueResult<TenantMetadata> {
        let now = Timestamp::now();

        // TODO: Generate proper tenant ID
        let tenant_id = TenantId::from(0);

        // Create tenant metadata
        let tenant = TenantMetadata {
            id: tenant_id,
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
            cache.set_tenant_record(tenant.clone());
        }

        // Persist to system shard
        let key = SystemKeys::tenant_key(tenant_id);
        let value = crate::utility::serialize(&tenant)?;

        let shard_manager = self
            .shard_manager
            .read()
            .map_err(|_| KeyValueError::LockPoisoned)?;
        shard_manager.put(ShardId::from(0), &key, &value).await?;

        Ok(tenant)
    }

    #[tracing::instrument(skip(self))]
    pub async fn update_tenant(
        &self,
        tenant: &TenantId,
        config: TenantUpdate,
    ) -> KeyValueResult<TenantMetadata> {
        let mut cache = self
            .system_metacache
            .write()
            .map_err(|_| KeyValueError::LockPoisoned)?;

        if let Some(mut tenant_meta) = cache.get_tenant_record(tenant).cloned() {
            // Update fields
            if let Some(name) = config.name {
                tenant_meta.name = name;
            }

            // Apply option updates
            for opt_update in &config.options {
                match opt_update {
                    PropertyUpdate::Set(key, value) => {
                        tenant_meta.options.insert(key.clone(), value.clone());
                    }
                    PropertyUpdate::Clear(key) => {
                        tenant_meta.options.remove(key);
                    }
                }
            }

            // Apply metadata updates
            for meta_update in &config.metadata {
                match meta_update {
                    PropertyUpdate::Set(key, value) => {
                        tenant_meta.metadata.insert(key.clone(), value.clone());
                    }
                    PropertyUpdate::Clear(key) => {
                        tenant_meta.metadata.remove(key);
                    }
                }
            }

            // Update version and timestamp
            tenant_meta.version += 1;
            tenant_meta.last_modified = Timestamp::now();

            // Store in cache
            cache.set_tenant_record(tenant_meta.clone());
            drop(cache);

            // Persist to system shard
            let key = SystemKeys::tenant_key(*tenant);
            let value = crate::utility::serialize(&tenant_meta)?;

            let shard_manager = self
                .shard_manager
                .read()
                .map_err(|_| KeyValueError::LockPoisoned)?;
            shard_manager.put(ShardId::from(0), &key, &value).await?;

            Ok(tenant_meta)
        } else {
            Err(KeyValueError::InvalidValue(format!(
                "Tenant not found: {:?}",
                tenant
            )))
        }
    }

    #[tracing::instrument(skip(self))]
    pub async fn delete_tenant(&self, tenant: &TenantId) -> KeyValueResult<()> {
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

    #[tracing::instrument(skip(self))]
    pub async fn get_databases(
        &self,
        tenant: &TenantId,
    ) -> KeyValueResult<impl IntoIterator<Item = DatabaseMetadata>> {
        let cache = self
            .system_metacache
            .read()
            .map_err(|_| KeyValueError::LockPoisoned)?;
        Ok(cache
            .list_database_records()
            .filter(|d| &d.tenant == tenant)
            .cloned()
            .collect::<Vec<_>>())
    }

    #[tracing::instrument(skip(self))]
    pub async fn get_database(
        &self,
        tenant: &TenantId,
        database: &DatabaseId,
    ) -> KeyValueResult<Option<DatabaseMetadata>> {
        // Check cache first
        {
            let cache = self
                .system_metacache
                .read()
                .map_err(|_| KeyValueError::LockPoisoned)?;
            if let Some(db_meta) = cache.get_database_record(database) {
                if &db_meta.tenant == tenant {
                    return Ok(Some(db_meta.clone()));
                }
            }
        }

        // Read from disk
        let key = SystemKeys::database_key(*tenant, *database);
        let shard_manager = self
            .shard_manager
            .read()
            .map_err(|_| KeyValueError::LockPoisoned)?;

        if let Some(value) = shard_manager.get(ShardId::from(0), &key).await? {
            let db_meta: DatabaseMetadata = deserialize(&value)?;

            // Update cache
            let mut cache = self
                .system_metacache
                .write()
                .map_err(|_| KeyValueError::LockPoisoned)?;
            cache.set_database_record(db_meta.clone());

            Ok(Some(db_meta))
        } else {
            Ok(None)
        }
    }

    /// Create a new database for a tenant.
    ///
    /// # What it does
    /// Creates a new database with the provided configuration.
    ///
    /// # How it works
    /// 1. Generates a new DatabaseId (currently hardcoded to 0)
    /// 2. Creates a root namespace for the database (currently hardcoded to 0)
    /// 3. Creates DatabaseMetadata with current timestamp and version 1
    /// 4. Stores in system_metacache
    /// 5. Serializes and persists to system shard using SystemKeys::database_key
    ///
    /// # Parameters
    /// - `tenant`: TenantId that will own the database
    /// - `config`: DatabaseCreate containing name, options, and metadata
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
        tenant: &TenantId,
        config: DatabaseCreate,
    ) -> KeyValueResult<DatabaseMetadata> {
        let now = Timestamp::now();

        // TODO: Generate proper database ID
        let database_id = DatabaseId::from(0);

        // Create database metadata
        // TODO: Create root namespace for database
        let root_namespace = NamespaceId::from(0);

        let database = DatabaseMetadata {
            id: database_id,
            tenant: *tenant,
            name: config.name.clone(),
            version: 1,
            created_at: now,
            last_modified: now,
            root_namespace,
            options: config.options.clone(),
            metadata: config.metadata.clone(),
        };

        // Store in cache
        {
            let mut cache = self
                .system_metacache
                .write()
                .map_err(|_| KeyValueError::LockPoisoned)?;
            cache.set_database_record(database.clone());
        }

        // Persist to system shard
        let key = SystemKeys::database_key(*tenant, database_id);
        let value = crate::utility::serialize(&database)?;

        let shard_manager = self
            .shard_manager
            .read()
            .map_err(|_| KeyValueError::LockPoisoned)?;
        shard_manager.put(ShardId::from(0), &key, &value).await?;

        Ok(database)
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
    /// - `tenant`: TenantId that should own the database
    /// - `database`: DatabaseId to update
    /// - `config`: DatabaseUpdate containing optional name, options, and metadata updates
    ///
    /// # Returns
    /// - `Ok(DatabaseMetadata)` with updated database information
    /// - `Err(KeyValueError::InvalidValue)` if database not found or belongs to different tenant
    /// - `Err(KeyValueError)` if lock poisoned or persistence fails
    #[tracing::instrument(skip(self))]
    pub async fn update_database(
        &self,
        tenant: &TenantId,
        database: &DatabaseId,
        config: DatabaseUpdate,
    ) -> KeyValueResult<DatabaseMetadata> {
        let mut cache = self
            .system_metacache
            .write()
            .map_err(|_| KeyValueError::LockPoisoned)?;

        if let Some(mut db_meta) = cache.get_database_record(database).cloned() {
            // Verify tenant matches
            if &db_meta.tenant != tenant {
                return Err(KeyValueError::InvalidValue(format!(
                    "Database {:?} does not belong to tenant {:?}",
                    database, tenant
                )));
            }

            // Update fields
            if let Some(name) = config.name {
                db_meta.name = name;
            }

            // Apply option updates
            for opt_update in &config.options {
                match opt_update {
                    PropertyUpdate::Set(key, value) => {
                        db_meta.options.insert(key.clone(), value.clone());
                    }
                    PropertyUpdate::Clear(key) => {
                        db_meta.options.remove(key);
                    }
                }
            }

            // Apply metadata updates
            for meta_update in &config.metadata {
                match meta_update {
                    PropertyUpdate::Set(key, value) => {
                        db_meta.metadata.insert(key.clone(), value.clone());
                    }
                    PropertyUpdate::Clear(key) => {
                        db_meta.metadata.remove(key);
                    }
                }
            }

            // Update version and timestamp
            db_meta.version += 1;
            db_meta.last_modified = Timestamp::now();

            // Store in cache
            cache.set_database_record(db_meta.clone());
            drop(cache);

            // Persist to system shard
            let key = SystemKeys::database_key(*tenant, *database);
            let value = crate::utility::serialize(&db_meta)?;

            let shard_manager = self
                .shard_manager
                .read()
                .map_err(|_| KeyValueError::LockPoisoned)?;
            shard_manager.put(ShardId::from(0), &key, &value).await?;

            Ok(db_meta)
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
    /// - `tenant`: TenantId that owns the database
    /// - `database`: DatabaseId to delete
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
        tenant: &TenantId,
        database: &DatabaseId,
    ) -> KeyValueResult<()> {
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

    #[tracing::instrument(skip(self))]
    pub async fn get_tablespaces(
        &self,
    ) -> KeyValueResult<impl IntoIterator<Item = TablespaceMetadata>> {
        let cache = self.system_metacache.read().unwrap();
        let tablespaces: Vec<TablespaceMetadata> =
            cache.list_tablespace_records().cloned().collect();
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
    /// - `tablespace`: TablespaceId to look up
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
        tablespace: &TablespaceId,
    ) -> KeyValueResult<Option<TablespaceMetadata>> {
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
    pub async fn get_tablespace_by_name(&self, name: &str) -> KeyValueResult<Option<TablespaceId>> {
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
    /// - `config`: TablespaceCreate containing name, storage_path, tier, options, and metadata
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
        config: TablespaceCreate,
    ) -> KeyValueResult<TablespaceMetadata> {
        let now = Timestamp::now();

        // TODO: Get/Create actual new tablespace ID
        let tablespace_id = TablespaceId::new(0);

        // Create tablespace metadata
        let tablespace = TablespaceMetadata {
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
    /// - `tablespace`: TablespaceId to update
    /// - `config`: TablespaceUpdate containing optional name, storage_path, tier updates
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
        tablespace: &TablespaceId,
        config: TablespaceUpdate,
    ) -> KeyValueResult<TablespaceMetadata> {
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
    /// - `tablespace`: TablespaceId to delete
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
    pub async fn delete_tablespace(&self, tablespace: &TablespaceId) -> KeyValueResult<()> {
        let mut cache = self.system_metacache.write().unwrap();
        cache.clear_tablespace_record(tablespace);

        // TODO: Persist deletion to system shard via Raft if in distributed mode
        // TODO: Update shard manager's path resolver to remove tablespace config

        Ok(())
    }

    /**********************************************************************************************\
     * Database Management                                                                        *
    \**********************************************************************************************/

    #[tracing::instrument(skip(self))]
    pub async fn get_objects_by_namespace(
        &self,
        container: &ContainerId,
        namespace: &NamespaceId,
    ) -> KeyValueResult<impl IntoIterator<Item = (ObjectId, ObjectType, ObjectMetadata)>> {
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
    /// - `container`: ContainerId to get namespaces from
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
        container: &ContainerId,
    ) -> KeyValueResult<impl IntoIterator<Item = NamespaceMetadata>> {
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
        let namespaces: Vec<NamespaceMetadata> = cache.list_namespace_records().cloned().collect();

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
    /// - `container`: ContainerId to search in
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
        container: &ContainerId,
        prefix: &str,
    ) -> KeyValueResult<impl IntoIterator<Item = NamespaceMetadata>> {
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
        let namespaces: Vec<NamespaceMetadata> = cache
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
    /// - `container`: ContainerId containing the namespace
    /// - `namespace`: NamespaceId to look up
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
        container: &ContainerId,
        namespace: &NamespaceId,
    ) -> KeyValueResult<Option<NamespaceMetadata>> {
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
    /// - `container`: ContainerId to search in
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
        container: &ContainerId,
        path: &str,
    ) -> KeyValueResult<Option<NamespaceMetadata>> {
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
    /// - `container`: ContainerId to create namespace in
    /// - `config`: NamespaceCreate containing name, options, and metadata
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
        container: &ContainerId,
        config: NamespaceCreate,
    ) -> KeyValueResult<NamespaceMetadata> {
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
        let namespace_meta = NamespaceMetadata {
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

    /// Update a namespace
    /// TODO: Figure out best API for namespaces
    #[tracing::instrument(skip(self))]
    pub async fn update_namespace(
        &self,
        container: &ContainerId,
        namespace: &NamespaceId,
        config: NamespaceUpdate,
    ) -> KeyValueResult<NamespaceMetadata> {
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

    /// Remove a namespace
    /// TODO: Figure out best API for namespaces
    #[tracing::instrument(skip(self))]
    pub async fn delete_namespace(
        &self,
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
    /// - `container`: ContainerId to get tables from
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
        container: &ContainerId,
    ) -> KeyValueResult<impl IntoIterator<Item = TableMetadata>> {
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
        let tables: Vec<TableMetadata> = cache.list_table_records().cloned().collect();

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
    /// - `container`: ContainerId to search in
    /// - `namespace`: NamespaceId to filter tables by
    ///
    /// # Returns
    /// An iterator over TableMetadata for tables in the namespace
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
        container: &ContainerId,
        namespace: &NamespaceId,
    ) -> KeyValueResult<impl IntoIterator<Item = TableMetadata>> {
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
        let tables: Vec<TableMetadata> = cache
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
    /// - `container`: ContainerId to search in
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
        container: &ContainerId,
        prefix: &str,
    ) -> KeyValueResult<impl IntoIterator<Item = TableMetadata>> {
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
        let tables: Vec<TableMetadata> = cache
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
    /// - `container_id`: ContainerId containing the table
    /// - `table`: TableId to look up
    ///
    /// # Returns
    /// - `Some(TableMetadata)` if table exists
    /// - `None` if table not found
    ///
    /// # Errors
    /// - `Err(KeyValueError::InvalidValue)` if container not found
    /// - `Err(KeyValueError::LockPoisoned)` if lock poisoned
    #[tracing::instrument(skip(self))]
    pub async fn get_table(
        &self,
        container_id: &ContainerId,
        table: &TableId,
    ) -> KeyValueResult<Option<TableMetadata>> {
        // Get container metadata cache
        let container_caches = self
            .container_metacaches
            .read()
            .map_err(|_| KeyValueError::LockPoisoned)?;

        let container_cache = container_caches
            .get(&container_id.database())
            .ok_or_else(|| {
                KeyValueError::InvalidValue(format!("Container not found: {:?}", container_id))
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
    /// - `container_id`: ContainerId to create table in
    /// - `config`: TableCreate containing name, path, engine_type, sharding_config, options, metadata
    ///
    /// # Returns
    /// - `Ok(TableId)` with the created table ID (currently hardcoded to 0)
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
        container_id: &ContainerId,
        config: TableCreate,
    ) -> KeyValueResult<TableId> {
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
                    let _shard_metadata = ShardMetadata {
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
                    let _table_metadata = TableMetadata {
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

                    // TODO: Return actual table ID
                    Ok(TableId::new(0))
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
                        let _shard_metadata = ShardMetadata {
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
                    let _table_metadata = TableMetadata {
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
                    Ok(TableId::new(0))
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
    /// - `container`: ContainerId containing the table
    /// - `table`: TableId to update
    /// - `config`: TableUpdate containing optional name, engine_type, sharding_config, options, metadata updates
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
        container: &ContainerId,
        table: &TableId,
        config: TableUpdate,
    ) -> KeyValueResult<TableMetadata> {
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
    /// - `container`: ContainerId containing the table
    /// - `table`: TableId to delete
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

    /// Put a key-value pair into a table
    /// TODO: Figure out how to handle distributed mode
    /// TODO: Figure out how to deal with tenants and containers
    #[tracing::instrument(skip(self))]
    pub async fn put(
        &self,
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

    /// Get a value from a table
    #[tracing::instrument(skip(self))]
    pub async fn get(
        &self,
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

    /// Delete a key from a table
    #[tracing::instrument(skip(self))]
    pub async fn delete(
        &self,
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
    /// - `container`: ContainerId containing the table
    /// - `table`: TableId to determine shard for
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
