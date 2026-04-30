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

//! Ordered index traits for range queries and sorted access

use crate::error::IndexResult;
use crate::index::{IndexEntry, IndexStore};
use async_trait::async_trait;
use std::ops::Bound;

/// Trait for indexes that maintain sorted order
///
/// Ordered indexes support efficient range queries, sorted scans,
/// and operations that depend on key ordering.
#[async_trait]
pub trait OrderedIndex: IndexStore {
    /// Perform a range scan with specified bounds
    ///
    /// # Arguments
    /// * `start` - Lower bound (inclusive, exclusive, or unbounded)
    /// * `end` - Upper bound (inclusive, exclusive, or unbounded)
    /// * `limit` - Maximum number of results to return
    /// * `reverse` - Scan in reverse order (descending)
    ///
    /// # Returns
    /// * `Ok(Vec<IndexEntry>)` - Entries within the range
    /// * `Err(IndexError)` - If the scan fails
    async fn range_scan(
        &self,
        start: Bound<Vec<u8>>,
        end: Bound<Vec<u8>>,
        limit: Option<usize>,
        reverse: bool,
    ) -> IndexResult<Vec<IndexEntry>>;

    /// Get the minimum key in the index
    ///
    /// # Returns
    /// * `Ok(Some(key))` - The minimum key if index is not empty
    /// * `Ok(None)` - If the index is empty
    async fn min_key(&self) -> IndexResult<Option<Vec<u8>>>;

    /// Get the maximum key in the index
    ///
    /// # Returns
    /// * `Ok(Some(key))` - The maximum key if index is not empty
    /// * `Ok(None)` - If the index is empty
    async fn max_key(&self) -> IndexResult<Option<Vec<u8>>>;

    /// Scan entries with a common prefix
    ///
    /// # Arguments
    /// * `prefix` - The prefix to match
    /// * `limit` - Maximum number of results to return
    ///
    /// # Returns
    /// * `Ok(Vec<IndexEntry>)` - Entries with matching prefix
    async fn prefix_scan(
        &self,
        prefix: &[u8],
        limit: Option<usize>,
    ) -> IndexResult<Vec<IndexEntry>>;

    /// Count entries in a range
    ///
    /// # Arguments
    /// * `start` - Lower bound
    /// * `end` - Upper bound
    ///
    /// # Returns
    /// * `Ok(count)` - Number of entries in range
    async fn count_range(&self, start: Bound<Vec<u8>>, end: Bound<Vec<u8>>) -> IndexResult<u64>;
}

/// Trait for indexes that enforce uniqueness constraints
///
/// Unique indexes ensure that no two entries have the same indexed value.
#[async_trait]
pub trait UniqueIndex: IndexStore {
    /// Check if a value exists and return its primary key
    ///
    /// # Arguments
    /// * `indexed_value` - The value to look up
    ///
    /// # Returns
    /// * `Ok(Some(primary_key))` - If the value exists
    /// * `Ok(None)` - If the value doesn't exist
    async fn lookup_unique(&self, indexed_value: &[u8]) -> IndexResult<Option<Vec<u8>>>;

    /// Validate uniqueness before insert
    ///
    /// # Arguments
    /// * `indexed_value` - The value to check
    ///
    /// # Returns
    /// * `Ok(())` - If the value is unique
    /// * `Err(IndexError::UniqueViolation)` - If the value already exists
    async fn validate_unique(&self, indexed_value: &[u8]) -> IndexResult<()>;
}

// Index implementations
pub mod btree;
pub mod hash;
