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
use nanograph_core::object::{ContainerId, SecurityPrincipal, TableId, TableRecord, TableUpdate};
use nanograph_kvt::KeyValueResult;
use std::sync::Arc;

/// A handle for performing operations on a specific table.
///
/// `TableHandle` provides a convenient interface for working with a single table,
/// allowing you to read, write, and delete key-value pairs without repeatedly
/// specifying the container and table IDs.
///
/// # Usage
///
/// You typically obtain a `TableHandle` from a `ContainerHandle`:
///
/// ```ignore
/// let table = container.get_table_handle(&table_id).await?;
/// ```
///
/// # Thread Safety
///
/// `TableHandle` is safe to clone and share across threads. All operations are
/// internally synchronized and can be called concurrently.
///
/// # Performance
///
/// - Operations are automatically routed to the correct shard based on the key
/// - In distributed mode, writes go through Raft consensus for consistency
/// - Reads can be served from local replicas in distributed mode
/// - Batch operations are more efficient than individual operations
pub struct TableHandle {
    context: Arc<KeyValueDatabaseContext>,
    principal: SecurityPrincipal,
    container_id: ContainerId,
    table_id: TableId,
}

impl TableHandle {
    pub(crate) fn new(
        context: Arc<KeyValueDatabaseContext>,
        principal: SecurityPrincipal,
        container_id: ContainerId,
        table_id: TableId,
    ) -> Self {
        TableHandle {
            context,
            principal,
            container_id,
            table_id,
        }
    }

    /// Get the table ID associated with this handle.
    pub fn table_id(&self) -> TableId {
        self.table_id
    }

    /// Get the container ID associated with this handle.
    pub fn container_id(&self) -> &ContainerId {
        &self.container_id
    }

    /// Get metadata for this table.
    pub async fn get_metadata(&self) -> KeyValueResult<Option<TableRecord>> {
        self.context
            .get_table(&self.principal, &self.container_id, &self.table_id)
            .await
    }

    /// Update this table's configuration.
    pub async fn update(&self, config: TableUpdate) -> KeyValueResult<TableRecord> {
        self.context
            .update_table(&self.principal, &self.container_id, &self.table_id, config)
            .await
    }

    /// Delete this table.
    pub async fn delete_table(&self) -> KeyValueResult<()> {
        self.context
            .delete_table(&self.principal, &self.container_id, &self.table_id)
            .await
    }

    //
    // Data Operations
    //

    /// Store a key-value pair in the table.
    ///
    /// # Arguments
    ///
    /// * `key` - The key as a byte slice
    /// * `value` - The value as a byte slice
    ///
    /// # Returns
    ///
    /// * `Ok(())` - The operation succeeded
    /// * `Err(KeyValueError)` - The operation failed
    ///
    /// # Example
    ///
    /// ```ignore
    /// table.put(b"user:123", b"John Doe").await?;
    /// ```
    pub async fn put(&self, key: &[u8], value: &[u8]) -> KeyValueResult<()> {
        self.context
            .table_entry_put(
                &self.principal,
                &self.container_id,
                &self.table_id,
                key,
                value,
            )
            .await
    }

    /// Retrieve a value from the table by its key.
    ///
    /// # Arguments
    ///
    /// * `key` - The key to look up
    ///
    /// # Returns
    ///
    /// * `Ok(Some(value))` - The key exists and its value is returned
    /// * `Ok(None)` - The key does not exist
    /// * `Err(KeyValueError)` - The operation failed
    ///
    /// # Example
    ///
    /// ```ignore
    /// if let Some(value) = table.get(b"user:123").await? {
    ///     println!("Found: {:?}", value);
    /// }
    /// ```
    pub async fn get(&self, key: &[u8]) -> KeyValueResult<Option<Vec<u8>>> {
        self.context
            .table_entry_get(&self.principal, &self.container_id, &self.table_id, key)
            .await
    }

    /// Delete a key-value pair from the table.
    ///
    /// # Arguments
    ///
    /// * `key` - The key to delete
    ///
    /// # Returns
    ///
    /// * `Ok(true)` - The key was deleted
    /// * `Ok(false)` - The key did not exist
    /// * `Err(KeyValueError)` - The operation failed
    ///
    /// # Example
    ///
    /// ```ignore
    /// if table.delete(b"user:123").await? {
    ///     println!("Key deleted");
    /// }
    /// ```
    pub async fn delete(&self, key: &[u8]) -> KeyValueResult<bool> {
        self.context
            .table_entry_delete(&self.principal, &self.container_id, &self.table_id, key)
            .await
    }

    /// Store multiple key-value pairs in a single batch operation.
    ///
    /// This is more efficient than calling `put()` multiple times, especially
    /// in distributed mode where it reduces network round-trips.
    ///
    /// # Arguments
    ///
    /// * `pairs` - A slice of (key, value) tuples to store
    ///
    /// # Returns
    ///
    /// * `Ok(())` - All pairs were stored successfully
    /// * `Err(KeyValueError)` - The operation failed
    ///
    /// # Example
    ///
    /// ```ignore
    /// let pairs = vec![
    ///     (b"user:1" as &[u8], b"Alice" as &[u8]),
    ///     (b"user:2", b"Bob"),
    ///     (b"user:3", b"Charlie"),
    /// ];
    /// table.batch_put(&pairs).await?;
    /// ```
    pub async fn batch_put(&self, pairs: &[(&[u8], &[u8])]) -> KeyValueResult<()> {
        self.context
            .table_entry_batch_put(&self.principal, &self.container_id, &self.table_id, pairs)
            .await
    }
}
