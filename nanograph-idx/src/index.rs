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

// Index implementations
pub mod ordered;
pub mod spatial;
pub mod store;
pub mod text;
pub mod vector;

use crate::IndexResult;
use async_trait::async_trait;
use nanograph_core::object::{IndexId, IndexRecord};
use std::ops::Bound;

/// An entry in an index
#[derive(Debug, Clone)]
pub struct IndexEntry {
    /// The indexed value(s)
    pub indexed_value: Vec<u8>,
    /// The primary key of the table row
    pub primary_key: Vec<u8>,
    /// Optional included columns (for covering indexes)
    pub included_columns: Option<Vec<u8>>,
}

/// Query parameters for index scans
#[derive(Debug, Clone)]
pub struct IndexQuery {
    /// Start bound for range queries
    pub start: Bound<Vec<u8>>,
    /// End bound for range queries
    pub end: Bound<Vec<u8>>,
    /// Maximum number of results to return
    pub limit: Option<usize>,
    /// Scan in reverse order
    pub reverse: bool,
}

impl IndexQuery {
    /// Create a query that returns all entries
    pub fn all() -> Self {
        Self {
            start: Bound::Unbounded,
            end: Bound::Unbounded,
            limit: None,
            reverse: false,
        }
    }

    /// Create a query for a specific value
    pub fn exact(value: Vec<u8>) -> Self {
        Self {
            start: Bound::Included(value.clone()),
            end: Bound::Included(value),
            limit: None,
            reverse: false,
        }
    }

    /// Create a range query
    pub fn range(start: Bound<Vec<u8>>, end: Bound<Vec<u8>>) -> Self {
        Self {
            start,
            end,
            limit: None,
            reverse: false,
        }
    }

    /// Set a limit on the number of results
    pub fn with_limit(mut self, limit: usize) -> Self {
        self.limit = Some(limit);
        self
    }

    /// Set reverse scan order
    pub fn reversed(mut self) -> Self {
        self.reverse = true;
        self
    }
}

/// Statistics about an index
#[derive(Debug, Clone)]
pub struct IndexStats {
    /// Number of entries in the index
    pub entry_count: u64,
    /// Size of the index in bytes
    pub size_bytes: u64,
    /// Number of levels (for tree-based indexes)
    pub levels: Option<u32>,
    /// Average entry size in bytes
    pub avg_entry_size: u64,
    /// Index fragmentation percentage (0-100)
    pub fragmentation: Option<f64>,
}

/// Trait for index store implementations
///
/// This trait defines the interface that all index implementations must provide.
/// It supports building indexes from table data, querying indexes, and maintaining
/// indexes during table updates.
#[async_trait]
pub trait IndexStore: Send + Sync {
    /// Get the index metadata
    fn metadata(&self) -> &IndexRecord;

    /// Get the index ID
    fn index_id(&self) -> IndexId {
        self.metadata().index_id
    }

    /// Build the index from table data
    ///
    /// This is called when an index is first created or rebuilt.
    /// The implementation should scan the table and populate the index.
    ///
    /// # Arguments
    /// * `table_data` - Iterator over (primary_key, row_data) pairs
    ///
    /// # Returns
    /// * `Ok(())` if the build succeeds
    /// * `Err(IndexError)` if the build fails
    async fn build<I>(&mut self, table_data: I) -> IndexResult<()>
    where
        I: Iterator<Item = (Vec<u8>, Vec<u8>)> + Send;

    /// Insert an entry into the index
    ///
    /// Called when a row is inserted into the table.
    ///
    /// # Arguments
    /// * `entry` - The index entry to insert
    ///
    /// # Returns
    /// * `Ok(())` if the insert succeeds
    /// * `Err(IndexError::UniqueViolation)` if a unique constraint is violated
    async fn insert(&mut self, entry: IndexEntry) -> IndexResult<()>;

    /// Update an entry in the index
    ///
    /// Called when a row is updated in the table and the indexed columns change.
    ///
    /// # Arguments
    /// * `old_entry` - The old index entry to remove
    /// * `new_entry` - The new index entry to insert
    ///
    /// # Returns
    /// * `Ok(())` if the update succeeds
    /// * `Err(IndexError::UniqueViolation)` if a unique constraint is violated
    async fn update(&mut self, old_entry: IndexEntry, new_entry: IndexEntry) -> IndexResult<()> {
        self.delete(&old_entry.primary_key).await?;
        self.insert(new_entry).await
    }

    /// Delete an entry from the index
    ///
    /// Called when a row is deleted from the table.
    ///
    /// # Arguments
    /// * `primary_key` - The primary key of the row to delete
    ///
    /// # Returns
    /// * `Ok(())` if the delete succeeds
    async fn delete(&mut self, primary_key: &[u8]) -> IndexResult<()>;

    /// Query the index
    ///
    /// Returns an iterator over index entries matching the query.
    ///
    /// # Arguments
    /// * `query` - The query parameters
    ///
    /// # Returns
    /// * `Ok(Vec<IndexEntry>)` with matching entries
    /// * `Err(IndexError)` if the query fails
    async fn query(&self, query: IndexQuery) -> IndexResult<Vec<IndexEntry>>;

    /// Get a specific entry by primary key
    ///
    /// # Arguments
    /// * `primary_key` - The primary key to look up
    ///
    /// # Returns
    /// * `Ok(Some(IndexEntry))` if the entry exists
    /// * `Ok(None)` if the entry doesn't exist
    async fn get(&self, primary_key: &[u8]) -> IndexResult<Option<IndexEntry>>;

    /// Check if an indexed value exists (for unique constraints)
    ///
    /// # Arguments
    /// * `indexed_value` - The value to check
    ///
    /// # Returns
    /// * `Ok(true)` if the value exists
    /// * `Ok(false)` if the value doesn't exist
    async fn exists(&self, indexed_value: &[u8]) -> IndexResult<bool>;

    /// Get statistics about the index
    ///
    /// # Returns
    /// * `Ok(IndexStats)` with index statistics
    async fn stats(&self) -> IndexResult<IndexStats>;

    /// Optimize the index
    ///
    /// This may trigger compaction, rebalancing, or other maintenance operations.
    async fn optimize(&mut self) -> IndexResult<()>;

    /// Flush any pending changes to durable storage
    async fn flush(&mut self) -> IndexResult<()>;
}
