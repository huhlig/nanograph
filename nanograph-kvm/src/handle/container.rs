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

use crate::context::KeyValueDatabaseContext;
use nanograph_core::object::{
    ContainerId, DatabaseId, DatabaseMetadata, NamespaceCreate, NamespaceId, NamespaceRecord,
    NamespaceUpdate, ObjectId, ObjectMetadata, ObjectType, SecurityPrincipal, TableCreate, TableId,
    TableRecord, TableUpdate, TenantId,
};
use nanograph_kvt::{KeyValueError, KeyValueResult};

use crate::handle::table::TableHandle;
use std::sync::Arc;

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
    context: Arc<KeyValueDatabaseContext>,
    principal: SecurityPrincipal,
    container_id: ContainerId,
}

impl ContainerHandle {
    pub(crate) fn new(
        context: Arc<KeyValueDatabaseContext>,
        principal: SecurityPrincipal,
        container_id: ContainerId,
    ) -> ContainerHandle {
        ContainerHandle {
            context,
            principal,
            container_id,
        }
    }

    /// Get the container ID associated with this handle.
    pub fn container_id(&self) -> &ContainerId {
        &self.container_id
    }

    /// Get the tenant ID for this container.
    pub fn tenant_id(&self) -> TenantId {
        self.container_id.tenant()
    }

    /// Get the database ID for this container.
    pub fn database_id(&self) -> DatabaseId {
        self.container_id.database()
    }

    /// Get metadata for this database.
    pub async fn get_metadata(&self) -> KeyValueResult<Option<DatabaseMetadata>> {
        self.context
            .get_database(
                &self.principal,
                &self.container_id.tenant(),
                &self.container_id.database(),
            )
            .await
    }

    /// Get the root namespace ID for this container.
    pub async fn get_root_namespace(&self) -> KeyValueResult<NamespaceId> {
        if let Some(database_metadata) = self.get_metadata().await? {
            Ok(database_metadata.root_namespace)
        } else {
            Err(KeyValueError::InvalidKey("invalid container".to_string()))
        }
    }

    /// Get an object (table, namespace, etc.) by its path within the container.
    pub async fn get_object_by_path(
        &self,
        path: &str,
    ) -> KeyValueResult<Option<(ObjectId, ObjectType)>> {
        self.context
            .get_object_by_path(&self.principal, &self.container_id, path)
            .await
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
            .get_objects_by_namespace(&self.principal, &self.container_id, namespace)
            .await
    }

    /// List all namespaces in this container.
    ///
    /// # Returns
    ///
    /// An iterator over all namespace metadata records
    pub async fn get_namespaces(
        &self,
    ) -> KeyValueResult<impl IntoIterator<Item = NamespaceRecord>> {
        self.context
            .get_namespaces(&self.principal, &self.container_id)
            .await
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
    ) -> KeyValueResult<impl IntoIterator<Item = NamespaceRecord>> {
        self.context
            .get_namespaces_by_prefix(&self.principal, &self.container_id, prefix)
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
    ) -> KeyValueResult<Option<NamespaceRecord>> {
        self.context
            .get_namespace(&self.principal, &self.container_id, namespace)
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
    ) -> KeyValueResult<NamespaceRecord> {
        self.context
            .create_namespace(&self.principal, &self.container_id, config)
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
    ) -> KeyValueResult<NamespaceRecord> {
        self.context
            .update_namespace(&self.principal, &self.container_id, namespace, config)
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
            .delete_namespace(&self.principal, &self.container_id, namespace)
            .await
    }

    /// List all tables in this container.
    ///
    /// # Returns
    ///
    /// An iterator over all table metadata records
    pub async fn get_tables(&self) -> KeyValueResult<impl IntoIterator<Item = TableRecord>> {
        self.context
            .get_tables(&self.principal, &self.container_id)
            .await
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
            self.context.clone(),
            self.principal.clone(),
            self.container_id.clone(),
            table.clone(),
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
    ) -> KeyValueResult<impl IntoIterator<Item = TableRecord>> {
        self.context
            .get_tables_by_prefix(&self.principal, &self.container_id, prefix)
            .await
    }

    /// List tables in a specific namespace.
    pub async fn get_tables_by_namespace(
        &self,
        namespace: &NamespaceId,
    ) -> KeyValueResult<impl IntoIterator<Item = TableRecord>> {
        self.context
            .get_tables_by_namespace(&self.principal, &self.container_id, namespace)
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
    pub async fn get_table(&self, table: &TableId) -> KeyValueResult<Option<TableRecord>> {
        self.context
            .get_table(&self.principal, &self.container_id, table)
            .await
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
        let record = self
            .context
            .create_table(&self.principal, &self.container_id, config)
            .await?;
        Ok(record.id)
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
    ) -> KeyValueResult<TableRecord> {
        self.context
            .update_table(&self.principal, &self.container_id, table, config)
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
        self.context
            .delete_table(&self.principal, &self.container_id, table)
            .await
    }

    /**********************************************************************************************\
     * Data Management                                                                            *
    \**********************************************************************************************/

    /// Put a key-value pair into a table
    /// TODO: Figure out how to handle distributed mode
    /// TODO: Figure out how to deal with tenants and containers
    pub async fn put(&self, table: &TableId, key: &[u8], value: &[u8]) -> KeyValueResult<()> {
        self.context
            .put(&self.principal, &self.container_id, table, key, value)
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
        self.context
            .get(&self.principal, &self.container_id, table, key)
            .await
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
        self.context
            .delete(&self.principal, &self.container_id, table, key)
            .await
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
            .batch_put(&self.principal, &self.container_id, table, pairs)
            .await
    }
}
