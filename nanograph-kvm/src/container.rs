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

use crate::cache::ContainerMetadataCache;
use crate::context::KeyValueDatabaseContext;
use nanograph_core::object::{
    ContainerId, NamespaceCreate, NamespaceId, NamespaceMetadata, NamespaceUpdate, ObjectId,
    ObjectMetadata, ObjectType, ShardId, TableCreate, TableId, TableMetadata, TableUpdate,
};
use nanograph_kvt::{KeyValueError, KeyValueResult};

use crate::table::TableHandle;
use std::sync::{Arc, RwLock};

/// A handle for managing a database container (tenant + database combination).
///
/// `ContainerHandle` provides access to namespaces, tables, and data within a specific
/// database. It encapsulates both the tenant and database identifiers, making it easier
/// to work with a specific database without repeatedly specifying these IDs.
///
/// # Hierarchy
///
/// The database hierarchy is:
/// - **Cluster** → **Tenant** → **Database (Container)** → **Namespace** → **Table** → **Key-Value Pairs**
///
/// # Usage
///
/// You typically obtain a `ContainerHandle` from a `KeyValueDatabaseManager`:
///
/// ```ignore
/// let container_id = ContainerId::new(tenant_id, database_id);
/// let container = manager.get_container(&container_id).await?;
/// ```
///
/// # Operations
///
/// A `ContainerHandle` allows you to:
/// - Manage namespaces (create, list, update, delete)
/// - Manage tables (create, list, update, delete)
/// - Perform data operations (put, get, delete, batch operations)
/// - Get table handles for focused table operations
///
/// # Thread Safety
///
/// `ContainerHandle` is safe to clone and share across threads. All operations are
/// internally synchronized.
pub struct ContainerHandle {
    container_id: ContainerId, // Encapsulates TenantId + DatabaseId
    context: Arc<KeyValueDatabaseContext>,
    metadata_cache: Arc<RwLock<ContainerMetadataCache>>,
}

impl ContainerHandle {
    pub(crate) fn new(
        container_id: ContainerId,
        shard_id: ShardId,
        context: Arc<KeyValueDatabaseContext>,
    ) -> ContainerHandle {
        ContainerHandle {
            container_id,
            context,
            metadata_cache: Arc::new(RwLock::new(ContainerMetadataCache::new(
                container_id,
                shard_id,
            ))),
        }
    }

    /// Get the root namespace ID for this container.
    ///
    /// Every database has a root namespace that serves as the top-level organizational unit.
    ///
    /// # Returns
    ///
    /// * `Ok(NamespaceId)` - The root namespace ID
    /// * `Err(KeyValueError)` - The container is invalid or not found
    pub async fn get_root_namespace(&self) -> KeyValueResult<NamespaceId> {
        // TODO: Figure out a better way to handle this
        if let Some(database_metadata) = self
            .context
            .get_database(&self.container_id.tenant(), &self.container_id.database())
            .await?
        {
            Ok(database_metadata.root_namespace)
        } else {
            Err(KeyValueError::InvalidKey("invalid container".to_string()))
        }
    }

    /// List all objects (tables, views, etc.) in a specific namespace.
    ///
    /// # Arguments
    ///
    /// * `namespace` - The namespace ID to query
    ///
    /// # Returns
    ///
    /// An iterator over tuples of (ObjectId, ObjectType, ObjectMetadata)
    pub async fn get_objects_by_namespace(
        &self,
        namespace: &NamespaceId,
    ) -> KeyValueResult<impl IntoIterator<Item = (ObjectId, ObjectType, ObjectMetadata)>> {
        self.context
            .get_objects_by_namespace(&self.container_id, namespace)
            .await
    }

    /// List all namespaces in this container.
    ///
    /// # Returns
    ///
    /// An iterator over all namespace metadata records
    pub async fn get_namespaces(
        &self,
    ) -> KeyValueResult<impl IntoIterator<Item = NamespaceMetadata>> {
        self.context.get_namespaces(&self.container_id).await
    }

    /// Find namespaces whose name or path starts with the given prefix.
    ///
    /// # Arguments
    ///
    /// * `prefix` - The prefix to match against namespace names and paths
    ///
    /// # Returns
    ///
    /// An iterator over matching namespace metadata records
    pub async fn get_namespaces_by_prefix(
        &self,
        prefix: &str,
    ) -> KeyValueResult<impl IntoIterator<Item = NamespaceMetadata>> {
        self.context
            .get_namespaces_by_prefix(&self.container_id, prefix)
            .await
    }

    /// Get metadata for a specific namespace.
    ///
    /// # Arguments
    ///
    /// * `namespace` - The namespace ID to look up
    ///
    /// # Returns
    ///
    /// * `Ok(Some(metadata))` - The namespace exists
    /// * `Ok(None)` - The namespace does not exist
    pub async fn get_namespace(
        &self,
        namespace: &NamespaceId,
    ) -> KeyValueResult<Option<NamespaceMetadata>> {
        self.context
            .get_namespace(&self.container_id, namespace)
            .await
    }

    /// Create a new namespace in this container.
    ///
    /// Namespaces provide logical organization for tables and other objects.
    ///
    /// # Arguments
    ///
    /// * `config` - Configuration for the new namespace (name, options, metadata)
    ///
    /// # Returns
    ///
    /// The created namespace metadata
    ///
    /// # Example
    ///
    /// ```ignore
    /// let config = NamespaceCreate {
    ///     name: "analytics".to_string(),
    ///     options: HashMap::new(),
    ///     metadata: HashMap::new(),
    /// };
    /// let namespace = container.create_namespace(config).await?;
    /// ```
    pub async fn create_namespace(
        &self,
        config: NamespaceCreate,
    ) -> KeyValueResult<NamespaceMetadata> {
        self.context
            .create_namespace(&self.container_id, config)
            .await
    }

    /// Update an existing namespace's metadata.
    ///
    /// # Arguments
    ///
    /// * `namespace` - The namespace ID to update
    /// * `config` - Update configuration (optional name, options, metadata changes)
    ///
    /// # Returns
    ///
    /// The updated namespace metadata
    pub async fn update_namespace(
        &self,
        namespace: &NamespaceId,
        config: NamespaceUpdate,
    ) -> KeyValueResult<NamespaceMetadata> {
        self.context
            .update_namespace(&self.container_id, namespace, config)
            .await
    }

    /// Delete a namespace from this container.
    ///
    /// # Arguments
    ///
    /// * `namespace` - The namespace ID to delete
    ///
    /// # Returns
    ///
    /// * `Ok(())` - The namespace was deleted
    /// * `Err(KeyValueError)` - The operation failed
    ///
    /// # Note
    ///
    /// Ensure the namespace is empty before deletion to avoid orphaned objects.
    pub async fn delete_namespace(&self, namespace: &NamespaceId) -> KeyValueResult<()> {
        self.context
            .delete_namespace(&self.container_id, namespace)
            .await
    }

    /// List all tables in this container.
    ///
    /// # Returns
    ///
    /// An iterator over all table metadata records
    pub async fn get_tables(&self) -> KeyValueResult<impl IntoIterator<Item = TableMetadata>> {
        self.context.get_tables(&self.container_id).await
    }

    /// Get a handle for performing operations on a specific table.
    ///
    /// This is the recommended way to work with a table, as it provides a
    /// convenient interface without repeatedly specifying the container and table IDs.
    ///
    /// # Arguments
    ///
    /// * `table` - The table ID
    ///
    /// # Returns
    ///
    /// A `TableHandle` for the specified table
    ///
    /// # Example
    ///
    /// ```ignore
    /// let table_handle = container.get_table_handle(&table_id).await?;
    /// table_handle.put(b"key", b"value").await?;
    /// ```
    pub async fn get_table_handle(&self, table: &TableId) -> KeyValueResult<TableHandle> {
        Ok(TableHandle::new(
            self.container_id.clone(),
            table.clone(),
            self.context.clone(),
        ))
    }

    /// Find tables whose name or path starts with the given prefix.
    ///
    /// # Arguments
    ///
    /// * `prefix` - The prefix to match against table names and paths
    ///
    /// # Returns
    ///
    /// An iterator over matching table metadata records
    pub async fn get_tables_by_prefix(
        &self,
        prefix: &str,
    ) -> KeyValueResult<impl IntoIterator<Item = TableMetadata>> {
        self.context
            .get_tables_by_prefix(&self.container_id, prefix)
            .await
    }

    /// Get metadata for a specific table.
    ///
    /// # Arguments
    ///
    /// * `table` - The table ID to look up
    ///
    /// # Returns
    ///
    /// * `Ok(Some(metadata))` - The table exists
    /// * `Ok(None)` - The table does not exist
    pub async fn get_table(&self, table: &TableId) -> KeyValueResult<Option<TableMetadata>> {
        self.context.get_table(&self.container_id, table).await
    }

    /// Create a new table in this container.
    ///
    /// # Arguments
    ///
    /// * `config` - Configuration including name, path, engine type, and sharding strategy
    ///
    /// # Returns
    ///
    /// The ID of the created table
    ///
    /// # Example
    ///
    /// ```ignore
    /// let config = TableCreate {
    ///     name: "users".to_string(),
    ///     path: "/analytics/users".to_string(),
    ///     engine_type: EngineType::BTree,
    ///     sharding_config: TableSharding::Single,
    ///     options: HashMap::new(),
    ///     metadata: HashMap::new(),
    /// };
    /// let table_id = container.create_table(config).await?;
    /// ```
    pub async fn create_table(&self, config: TableCreate) -> KeyValueResult<TableId> {
        self.context.create_table(&self.container_id, config).await
    }

    /// Update an existing table's metadata.
    ///
    /// # Arguments
    ///
    /// * `table` - The table ID to update
    /// * `config` - Update configuration (optional name, engine type, sharding, options, metadata)
    ///
    /// # Returns
    ///
    /// The updated table metadata
    pub async fn update_table(
        &self,
        table: &TableId,
        config: TableUpdate,
    ) -> KeyValueResult<TableMetadata> {
        self.context
            .update_table(&self.container_id, table, config)
            .await
    }

    /// Delete a table from this container.
    ///
    /// # Arguments
    ///
    /// * `table` - The table ID to delete
    ///
    /// # Returns
    ///
    /// * `Ok(())` - The table was deleted
    /// * `Err(KeyValueError)` - The operation failed
    ///
    /// # Warning
    ///
    /// This will delete all data in the table. Ensure you have backups if needed.
    pub async fn delete_table(&self, table: &TableId) -> KeyValueResult<()> {
        self.context.delete_table(&self.container_id, table).await
    }

    /**********************************************************************************************\
     * Data Management                                                                            *
    \**********************************************************************************************/

    /// Put a key-value pair into a table
    /// TODO: Figure out how to handle distributed mode
    /// TODO: Figure out how to deal with tenants and containers
    pub async fn put(&self, table: &TableId, key: &[u8], value: &[u8]) -> KeyValueResult<()> {
        self.context
            .put(&self.container_id, &table, key, value)
            .await
    }

    /// Retrieve a value from a table by its key.
    ///
    /// # Arguments
    ///
    /// * `table` - The table ID to read from
    /// * `key` - The key to look up
    ///
    /// # Returns
    ///
    /// * `Ok(Some(value))` - The key exists
    /// * `Ok(None)` - The key does not exist
    /// * `Err(KeyValueError)` - The operation failed
    pub async fn get(&self, table: &TableId, key: &[u8]) -> KeyValueResult<Option<Vec<u8>>> {
        self.context.get(&self.container_id, table, key).await
    }

    /// Delete a key-value pair from a table.
    ///
    /// # Arguments
    ///
    /// * `table` - The table ID to delete from
    /// * `key` - The key to delete
    ///
    /// # Returns
    ///
    /// * `Ok(true)` - The key was deleted
    /// * `Ok(false)` - The key did not exist
    /// * `Err(KeyValueError)` - The operation failed
    pub async fn delete(&self, table: &TableId, key: &[u8]) -> KeyValueResult<bool> {
        self.context.delete(&self.container_id, table, key).await
    }

    /// Store multiple key-value pairs in a table in a single batch operation.
    ///
    /// This is more efficient than calling `put()` multiple times.
    ///
    /// # Arguments
    ///
    /// * `table` - The table ID to write to
    /// * `pairs` - A slice of (key, value) tuples to store
    ///
    /// # Returns
    ///
    /// * `Ok(())` - All pairs were stored successfully
    /// * `Err(KeyValueError)` - The operation failed
    pub async fn batch_put(&self, table: &TableId, pairs: &[(&[u8], &[u8])]) -> KeyValueResult<()> {
        self.context
            .batch_put(&self.container_id, table, pairs)
            .await
    }
}
