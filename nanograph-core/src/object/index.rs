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

//! # Index Management Types
//!
//! This module provides types for managing database indexes in Nanograph.
//!
//! ## Overview
//!
//! Indexes are secondary data structures that improve query performance by providing
//! efficient access paths to data. Nanograph supports multiple index types optimized
//! for different use cases:
//!
//! - **Secondary indexes**: B-Tree based indexes for range queries
//! - **Unique indexes**: Hash-based indexes enforcing uniqueness constraints
//! - **Full-text indexes**: Inverted indexes for text search
//! - **Spatial indexes**: R-Tree based indexes for geometric queries
//! - **Vector indexes**: Specialized indexes for AI/ML similarity search
//!
//! ## Index Sharding
//!
//! Like tables, indexes can be sharded across multiple nodes for scalability.
//! Each index shard is identified by an [`IndexShardId`] which encodes:
//! - Tenant ID (32 bits)
//! - Database ID (32 bits)
//! - Index ID (32 bits) - from unified ObjectId pool
//! - Shard Number (32 bits)
//!
//! ## ObjectId Allocation
//!
//! **IMPORTANT**: Index IDs are allocated from a unified ObjectId pool shared with
//! tables, functions, and namespaces within a database. This prevents collisions
//! when constructing shard IDs for storage operations.
//!
//! ## Examples
//!
//! Creating a secondary index:
//! ```rust
//! use nanograph_core::object::{IndexCreate, IndexType};
//!
//! let index = IndexCreate::new(
//!     "user_email_idx",
//!     IndexType::Secondary,
//!     vec!["email".to_string()]
//! );
//! ```
//!
//! Creating a vector similarity index:
//! ```rust
//! use nanograph_core::object::{IndexCreate, IndexType};
//!
//! let index = IndexCreate::new(
//!     "embeddings_idx",
//!     IndexType::VectorHNSW,
//!     vec!["embedding".to_string()]
//! )
//! .with_option("ef_construction", "200")
//! .with_option("M", "16")
//! .with_metadata("description", "HNSW index for text embeddings");
//! ```

use crate::object::shard::{Partitioner, ShardStatus, StorageEngineType};
use crate::object::{ContainerId, DatabaseId, NodeId, ObjectId, ShardNumber, TenantId};
use crate::types::{PropertyUpdate, Timestamp};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Index identifier, unique within a database.
///
/// Uses [`ObjectId`] (u32) from the unified allocation pool shared with tables,
/// functions, and namespaces. This ensures no collisions when constructing
/// [`IndexShardId`] values for storage operations.
///
/// # Examples
///
/// ```rust
/// use nanograph_core::object::IndexId;
///
/// let id = IndexId::new(42);
/// assert_eq!(id.as_u32(), 42);
/// ```
#[derive(
    Copy, Clone, Debug, Default, Ord, PartialOrd, Eq, PartialEq, Hash, Serialize, Deserialize,
)]
pub struct IndexId(pub ObjectId);

impl IndexId {
    /// Create a new index ID.
    pub fn new(id: ObjectId) -> Self {
        Self(id)
    }

    ///
    pub fn object(&self) -> ObjectId {
        self.0
    }
}

impl From<ObjectId> for IndexId {
    fn from(id: ObjectId) -> Self {
        Self(id)
    }
}

impl std::fmt::Display for IndexId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "IndexId({})", self.0)
    }
}

/// Globally unique identifier for an index shard.
///
/// Uses u128 encoding to uniquely identify a shard across the entire cluster:
/// ```text
/// [TenantId:32][DatabaseId:32][IndexId:32][ShardNumber:32]
/// ```
///
/// This structure enables:
/// - Global uniqueness across all tenants and databases
/// - Efficient routing to the correct node
/// - Hierarchical organization of shards
///
/// # Examples
///
/// ```rust
/// use nanograph_core::object::{IndexShardId, IndexId, IndexShardNumber, TenantId, DatabaseId};
///
/// let shard_id = IndexShardId::from_parts(
///     TenantId(1),
///     DatabaseId(2),
///     IndexId(3),
///     IndexShardNumber(0)
/// );
///
/// assert_eq!(shard_id.tenant(), TenantId(1));
/// assert_eq!(shard_id.database(), DatabaseId(2));
/// assert_eq!(shard_id.index(), IndexId(3));
/// assert_eq!(shard_id.shard_number(), IndexShardNumber(0));
/// ```
#[derive(
    Copy, Clone, Debug, Default, Ord, PartialOrd, Eq, PartialEq, Hash, Serialize, Deserialize,
)]
pub struct IndexShardId(pub u128);

impl IndexShardId {
    /// Create a new index shard identifier from a u128.
    pub fn new(id: u128) -> Self {
        Self(id)
    }

    /// Create an IndexShardId from component parts.
    ///
    /// The `index` parameter should be an ObjectId allocated from the unified
    /// database object pool (shared with tables, functions, and namespaces).
    pub fn from_parts(
        tenant: TenantId,
        database: DatabaseId,
        index: IndexId,
        shard: ShardNumber,
    ) -> Self {
        Self(
            (tenant.0 as u128) << 96
                | (database.0 as u128) << 64
                | (index.object().as_u32() as u128) << 32
                | (shard.0 as u128) << 00,
        )
    }

    /// Get the tenant ID from this index shard ID
    pub fn tenant(&self) -> TenantId {
        TenantId((self.0 >> 96) as u32)
    }

    /// Get the database ID from this index shard ID
    pub fn database(&self) -> DatabaseId {
        DatabaseId((self.0 >> 64) as u32)
    }

    /// Extract the IndexId (index identifier) from the IndexShardId.
    ///
    /// This IndexId (ObjectId) is from the unified allocation pool shared with tables.
    pub fn index(&self) -> IndexId {
        IndexId(ObjectId::new((self.0 >> 32) as u32))
    }

    /// Extract the shard number from the IndexShardId.
    pub fn shard_number(&self) -> ShardNumber {
        ShardNumber(self.0 as u32)
    }

    /// Get the underlying u128 value.
    pub fn as_u128(&self) -> u128 {
        self.0
    }
}

impl From<u128> for IndexShardId {
    fn from(id: u128) -> Self {
        Self(id)
    }
}

impl std::fmt::Display for IndexShardId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "IndexShard({:X})", self.0)
    }
}

/// Index type enumeration.
///
/// Defines the various types of indexes supported by Nanograph, each optimized
/// for different query patterns and data types.
///
/// # Vector Index Types
///
/// Vector indexes are specialized for AI/ML workloads:
///
/// - **VectorFlat**: Exhaustive search, highest accuracy, best for <10K vectors
/// - **VectorHNSW**: Graph-based, fast approximate search with high recall
/// - **VectorIVF**: Clustering-based, memory-efficient for large datasets
/// - **VectorIVFPQ**: IVF with compression, best for very large datasets
/// - **VectorAnnoy**: Tree-based, ideal for static datasets and read-heavy workloads
/// - **VectorVamana**: DiskANN algorithm, optimized for billion-scale datasets
#[derive(Clone, Debug, Ord, PartialOrd, Eq, PartialEq, Hash, Serialize, Deserialize)]
pub enum IndexType {
    /// Secondary index on a single column/field (B-Tree based)
    Secondary,
    /// Unique index ensuring uniqueness constraint (Hash based)
    Unique,
    /// Full-text search index (Inverted index)
    FullText,
    /// Spatial/geometric index (R-Tree based)
    Spatial,

    // Vector Similarity Search Indexes for AI/ML workloads
    /// Flat/Brute-force vector index - exhaustive search, highest accuracy
    /// Best for: Small datasets (<10K vectors), exact nearest neighbor search
    VectorFlat,
    /// Hierarchical Navigable Small World (HNSW) graph-based index
    /// Best for: High-dimensional vectors, fast approximate search, high recall
    VectorHNSW,
    /// Inverted File (IVF) index with clustering
    /// Best for: Large datasets, memory-efficient approximate search
    VectorIVF,
    /// IVF with Product Quantization (compression)
    /// Best for: Very large datasets, memory-constrained environments
    VectorIVFPQ,
    /// Annoy (Approximate Nearest Neighbors Oh Yeah) - tree-based
    /// Best for: Static datasets, read-heavy workloads, Spotify-style recommendations
    VectorAnnoy,
    /// Vamana graph-based index (DiskANN algorithm)
    /// Best for: Billion-scale datasets, disk-based search, high throughput
    VectorVamana,
}

/// Configuration for creating a new index.
///
/// Provides a builder pattern for constructing index creation requests with
/// various options and metadata.
///
/// # Examples
///
/// Basic secondary index:
/// ```rust
/// use nanograph_core::object::{IndexCreate, IndexType};
///
/// let index = IndexCreate::new(
///     "user_email_idx",
///     IndexType::Secondary,
///     vec!["email".to_string()]
/// );
/// ```
///
/// Vector index with options:
/// ```rust
/// use nanograph_core::object::{IndexCreate, IndexType};
///
/// let index = IndexCreate::new(
///     "embeddings_idx",
///     IndexType::VectorHNSW,
///     vec!["embedding".to_string()]
/// )
/// .with_option("ef_construction", "200")
/// .with_option("M", "16");
/// ```
#[derive(Debug, Clone)]
pub struct IndexCreate {
    /// Name of the index
    pub name: String,
    /// Type of index
    pub index_type: IndexType,
    /// Column/field names to index (for structured data)
    pub columns: Vec<String>,
    /// Key extraction function (for unstructured data)
    /// This is a string representation of how to extract index keys from values
    pub key_extractor: Option<String>,
    /// Additional index-specific options
    pub options: HashMap<String, String>,
    /// Index metadata (informative)
    pub metadata: HashMap<String, String>,
    /// Index Sharding configuration
    pub sharding: IndexSharding,
}

impl IndexCreate {
    /// Create a new index creation configuration.
    ///
    /// # Arguments
    ///
    /// * `name`: The name of the new index.
    /// * `index_type`: The type of index to create.
    /// * `columns`: The columns/fields to index.
    pub fn new(name: impl Into<String>, index_type: IndexType, columns: Vec<String>) -> Self {
        Self {
            name: name.into(),
            index_type,
            columns,
            key_extractor: None,
            options: HashMap::new(),
            metadata: HashMap::new(),
            sharding: Default::default(),
        }
    }

    /// Set a key extractor function for the index.
    pub fn with_key_extractor(mut self, extractor: impl Into<String>) -> Self {
        self.key_extractor = Some(extractor.into());
        self
    }

    /// Add or update a configuration option for the index.
    pub fn with_option(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.options.insert(key.into(), value.into());
        self
    }

    /// Add metadata to the index.
    pub fn with_metadata(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.metadata.insert(key.into(), value.into());
        self
    }
}

/// Configuration for updating an existing index.
///
/// Allows modification of index properties, options, and metadata using
/// a builder pattern.
///
/// # Examples
///
/// ```rust
/// use nanograph_core::object::IndexUpdate;
///
/// let mut update = IndexUpdate::default();
/// update
///     .set_name("new_index_name")
///     .set_option("cache_size", "1GB")
///     .set_metadata("description", "Updated description");
/// ```
#[derive(Clone, Debug, Default)]
pub struct IndexUpdate {
    /// New name for the index
    pub name: Option<String>,
    /// Index configuration options to update
    pub options: Vec<PropertyUpdate>,
    /// Index metadata to update
    pub metadata: Vec<PropertyUpdate>,
}

impl IndexUpdate {
    /// Set a new name for the index.
    pub fn set_name(&mut self, name: impl Into<String>) -> &mut Self {
        self.name = Some(name.into());
        self
    }

    /// Add or update a configuration option for the index.
    pub fn set_option(&mut self, key: impl Into<String>, value: impl Into<String>) -> &mut Self {
        self.options
            .push(PropertyUpdate::Set(key.into(), value.into()));
        self
    }

    /// Clear a configuration option from the index.
    pub fn clear_option(&mut self, key: impl Into<String>) -> &mut Self {
        self.options.push(PropertyUpdate::Clear(key.into()));
        self
    }

    /// Add or update metadata for the index.
    pub fn set_metadata(&mut self, key: impl Into<String>, value: impl Into<String>) -> &mut Self {
        self.metadata
            .push(PropertyUpdate::Set(key.into(), value.into()));
        self
    }

    /// Clear metadata from the index.
    pub fn clear_metadata(&mut self, key: impl Into<String>) -> &mut Self {
        self.metadata.push(PropertyUpdate::Clear(key.into()));
        self
    }
}

/// Metadata for an index.
///
/// Contains all user-visible information about an index, excluding internal
/// implementation details. This is the type returned to clients when querying
/// index information.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexMetadata {
    /// Unique identifier for the index
    pub id: IndexId,
    /// Name of the index
    pub name: String,
    /// Type of index
    pub index_type: IndexType,
    /// Timestamp when the index was created
    pub created_at: Timestamp,
    /// Timestamp when the index was last modified
    pub last_modified: Timestamp,
    /// Column/field names indexed
    pub columns: Vec<String>,
    /// Key extraction function
    pub key_extractor: Option<String>,
    /// Additional index-specific options
    pub options: HashMap<String, String>,
    /// Index metadata (informative)
    pub metadata: HashMap<String, String>,
}

impl From<IndexRecord> for IndexMetadata {
    fn from(record: IndexRecord) -> Self {
        Self {
            id: record.index_id,
            name: record.name,
            index_type: record.index_type,
            created_at: record.created_at,
            last_modified: record.updated_at,
            columns: record.columns,
            key_extractor: record.key_extractor,
            options: record.options,
            metadata: record.metadata,
        }
    }
}

/// Complete record for an index.
///
/// Contains all information about an index, including internal state and version
/// information. This is the type stored in the metadata store.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexRecord {
    /// Unique identifier for the index
    pub index_id: IndexId,
    /// Name of the index
    pub name: String,
    /// Version of the index record
    pub version: u64,
    /// Type of index
    pub index_type: IndexType,
    /// Timestamp when the index was created
    pub created_at: Timestamp,
    /// Timestamp when the index was last modified
    pub updated_at: Timestamp,
    /// Column/field names indexed
    pub columns: Vec<String>,
    /// Key extraction function
    pub key_extractor: Option<String>,
    /// Additional index-specific options
    pub options: HashMap<String, String>,
    /// Index metadata (informative)
    pub metadata: HashMap<String, String>,
    /// Index build status
    pub status: IndexStatus,
    /// Index sharding configuration
    pub sharding: IndexSharding,
}

/// Index build and maintenance status.
///
/// Tracks the current operational state of an index.
#[derive(
    Copy, Clone, Debug, Default, Ord, PartialOrd, Eq, PartialEq, Hash, Serialize, Deserialize,
)]
pub enum IndexStatus {
    /// Index is being built
    Building,
    /// Index is active and ready for queries
    #[default]
    Active,
    /// Index is being rebuilt
    Rebuilding,
    /// Index is disabled
    Disabled,
}

/// Index sharding configuration.
///
/// Defines how an index is distributed across multiple shards for scalability
/// and parallel processing.
///
/// # Examples
///
/// Single shard (default):
/// ```rust
/// use nanograph_core::object::IndexSharding;
///
/// let sharding = IndexSharding::Single;
/// ```
///
/// Multiple shards with hash partitioning:
/// ```rust
/// use nanograph_core::object::{IndexSharding, Partitioner, HashFunction};
///
/// let sharding = IndexSharding::Multiple {
///     shard_count: 4,
///     partitioner: Partitioner::Hash { hash_fn: HashFunction::Murmur3 },
///     replication_factor: 3,
/// };
/// ```
#[derive(Clone, Debug, Default, Ord, PartialOrd, Eq, PartialEq, Serialize, Deserialize)]
pub enum IndexSharding {
    /// Single Shard
    #[default]
    Single,
    /// Multiple Shards with Partitioning and Replication
    Multiple {
        /// Number of Shards
        shard_count: u32,
        /// Key Partitioner
        partitioner: Partitioner,
        /// Number of replicas per shard
        replication_factor: usize,
    },
}

/// Configuration for creating an index shard.
///
/// Specifies the parameters needed to create a new shard for an index.
#[derive(Debug, Clone)]
pub struct IndexShardCreate {
    /// Container ID for which the shard is being created
    pub container: ContainerId,
    /// Index ID for which the shard is being created
    pub index: IndexId,
    /// Shard Number for which the shard is being created
    pub shard_number: ShardNumber,
    /// Storage engine type for the shard
    pub engine_type: StorageEngineType,
    /// Number of replicas per shard (default: 1 for single-node)
    pub replication_factor: usize,
}

impl IndexShardCreate {
    /// Create a new Shard creation configuration.
    ///
    /// # Arguments
    ///
    /// * `container`: The ID of the container the shard belongs to.
    /// * `index`: The ID of the index the shard belongs to.
    /// * `shard_number`: The shard number within the index.
    /// * `engine_type`: The storage engine type to use for the shard.
    pub fn new(
        container: ContainerId,
        index: IndexId,
        shard_number: ShardNumber,
        engine_type: StorageEngineType,
    ) -> Self {
        Self {
            container,
            index,
            shard_number,
            engine_type,
            replication_factor: 1, // Default to no replication
        }
    }
    /// Set the replication factor for the shard.
    ///
    /// # Arguments
    ///
    /// * `replication_factor`: The number of replicas for the shard.
    pub fn with_replication(mut self, replication_factor: usize) -> Self {
        self.replication_factor = replication_factor;
        self
    }
}

/// Configuration for updating an index shard.
///
/// Allows modification of shard properties such as replication factor.
#[derive(Debug, Clone)]
pub struct IndexShardUpdate {
    /// Number of replicas per shard
    pub replication_factor: usize,
}

impl IndexShardUpdate {
    /// Create a new Shard update configuration.
    ///
    /// # Arguments
    ///
    /// * `replication_factor`: The new replication factor for the shard.
    pub fn new(replication_factor: usize) -> Self {
        Self { replication_factor }
    }
}

/// Complete record for an index shard.
///
/// Contains all information about a shard including its location, status,
/// and operational metadata.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct IndexShardRecord {
    /// Unique identifier for the shard (IndexId + IndexShardNumber)
    pub id: IndexShardId,
    /// Name of the shard
    pub name: String,
    /// Version of the Shard Record
    pub version: u64,
    /// Type of storage engine used by the shard
    pub engine_type: StorageEngineType,
    /// Timestamp when the shard was created
    pub created_at: Timestamp,
    /// Timestamp when the shard was last modified
    pub last_modified: Timestamp,
    /// Range of keys covered by this shard
    pub range: (Vec<u8>, Vec<u8>),
    /// Current leader node (if known)
    pub leader: Option<NodeId>,
    /// All replica nodes for this shard
    pub replicas: Vec<NodeId>,
    /// Current shard status
    pub status: ShardStatus,
    /// Raft term (for debugging)
    pub term: u64,
    /// Approximate size in bytes
    pub size_bytes: u64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::object::shard::HashFunction;

    #[test]
    fn test_index_id_creation() {
        let id = IndexId::new(ObjectId::new(42));
        assert_eq!(id.object().as_u32(), 42);
    }

    #[test]
    fn test_index_id_from_object_id() {
        let object_id = ObjectId::new(100);
        let index_id = IndexId::from(object_id);
        assert_eq!(index_id.object().as_u32(), 100);
    }

    #[test]
    fn test_index_id_display() {
        let id = IndexId::new(ObjectId::new(255));
        assert_eq!(format!("{}", id), "IndexId(FF)");
    }

    #[test]
    fn test_index_id_ordering() {
        let id1 = IndexId::new(ObjectId::new(1));
        let id2 = IndexId::new(ObjectId::new(2));
        assert!(id1 < id2);
        assert_eq!(id1, id1);
    }

    #[test]
    fn test_index_shard_number_creation() {
        let shard = ShardNumber::new(5);
        assert_eq!(shard.as_u32(), 5);
        assert_eq!(shard.0, 5);
    }

    #[test]
    fn test_index_shard_number_from_u32() {
        let num: u32 = 10;
        let shard = ShardNumber::from(num);
        assert_eq!(shard.as_u32(), 10);
    }

    #[test]
    fn test_index_shard_number_display() {
        let shard = ShardNumber::new(16);
        assert_eq!(format!("{}", shard), "ShardNumber(10)");
    }

    #[test]
    fn test_index_shard_id_from_parts() {
        let tenant = TenantId(1);
        let database = DatabaseId(2);
        let index = IndexId(ObjectId::new(3));
        let shard = ShardNumber(4);

        let shard_id = IndexShardId::from_parts(tenant, database, index, shard);

        // Verify component extraction
        assert_eq!(shard_id.tenant(), tenant);
        assert_eq!(shard_id.database(), database);
        assert_eq!(shard_id.index(), index);
        assert_eq!(shard_id.shard_number(), shard);
    }

    #[test]
    fn test_index_shard_id_bit_layout() {
        // Test that the bit layout is correct: [TenantId:32][DatabaseId:32][IndexId:32][ShardNumber:32]
        let tenant = TenantId(0x12345678);
        let database = DatabaseId(0x9ABCDEF0);
        let index = IndexId(ObjectId::new(0x11223344));
        let shard = ShardNumber(0x55667788);

        let shard_id = IndexShardId::from_parts(tenant, database, index, shard);
        let expected: u128 = 0x123456789ABCDEF01122334455667788;

        assert_eq!(shard_id.as_u128(), expected);
    }

    #[test]
    fn test_index_shard_id_roundtrip() {
        let original_id: u128 = 0xFFEEDDCCBBAA99887766554433221100;
        let shard_id = IndexShardId::from(original_id);
        assert_eq!(shard_id.as_u128(), original_id);
    }

    #[test]
    fn test_index_shard_id_display() {
        let shard_id = IndexShardId::new(0x123);
        assert_eq!(format!("{}", shard_id), "IndexShard(123)");
    }

    #[test]
    fn test_index_create_basic() {
        let columns = vec!["name".to_string(), "email".to_string()];
        let create = IndexCreate::new("user_idx", IndexType::Secondary, columns.clone());

        assert_eq!(create.name, "user_idx");
        assert_eq!(create.index_type, IndexType::Secondary);
        assert_eq!(create.columns, columns);
        assert!(create.key_extractor.is_none());
        assert!(create.options.is_empty());
        assert!(create.metadata.is_empty());
    }

    #[test]
    fn test_index_create_with_key_extractor() {
        let create = IndexCreate::new("test_idx", IndexType::FullText, vec![])
            .with_key_extractor("extract_text");

        assert_eq!(create.key_extractor, Some("extract_text".to_string()));
    }

    #[test]
    fn test_index_create_with_options() {
        let create = IndexCreate::new("test_idx", IndexType::VectorHNSW, vec![])
            .with_option("ef_construction", "200")
            .with_option("M", "16");

        assert_eq!(
            create.options.get("ef_construction"),
            Some(&"200".to_string())
        );
        assert_eq!(create.options.get("M"), Some(&"16".to_string()));
    }

    #[test]
    fn test_index_create_with_metadata() {
        let create = IndexCreate::new("test_idx", IndexType::Spatial, vec![])
            .with_metadata("description", "Geospatial index")
            .with_metadata("version", "1.0");

        assert_eq!(
            create.metadata.get("description"),
            Some(&"Geospatial index".to_string())
        );
        assert_eq!(create.metadata.get("version"), Some(&"1.0".to_string()));
    }

    #[test]
    fn test_index_create_builder_chain() {
        let create = IndexCreate::new(
            "complex_idx",
            IndexType::VectorIVFPQ,
            vec!["embedding".to_string()],
        )
        .with_key_extractor("extract_vector")
        .with_option("nlist", "100")
        .with_option("m", "8")
        .with_metadata("model", "text-embedding-ada-002");

        assert_eq!(create.name, "complex_idx");
        assert_eq!(create.key_extractor, Some("extract_vector".to_string()));
        assert_eq!(create.options.len(), 2);
        assert_eq!(create.metadata.len(), 1);
    }

    #[test]
    fn test_index_update_set_name() {
        let mut update = IndexUpdate::default();
        update.set_name("new_name");

        assert_eq!(update.name, Some("new_name".to_string()));
    }

    #[test]
    fn test_index_update_set_option() {
        let mut update = IndexUpdate::default();
        update.set_option("key1", "value1");
        update.set_option("key2", "value2");

        assert_eq!(update.options.len(), 2);
        assert!(
            matches!(update.options[0], PropertyUpdate::Set(ref k, ref v) if k == "key1" && v == "value1")
        );
    }

    #[test]
    fn test_index_update_clear_option() {
        let mut update = IndexUpdate::default();
        update.clear_option("old_key");

        assert_eq!(update.options.len(), 1);
        assert!(matches!(update.options[0], PropertyUpdate::Clear(ref k) if k == "old_key"));
    }

    #[test]
    fn test_index_update_set_metadata() {
        let mut update = IndexUpdate::default();
        update.set_metadata("author", "Alice");

        assert_eq!(update.metadata.len(), 1);
        assert!(
            matches!(update.metadata[0], PropertyUpdate::Set(ref k, ref v) if k == "author" && v == "Alice")
        );
    }

    #[test]
    fn test_index_update_clear_metadata() {
        let mut update = IndexUpdate::default();
        update.clear_metadata("deprecated_field");

        assert_eq!(update.metadata.len(), 1);
        assert!(
            matches!(update.metadata[0], PropertyUpdate::Clear(ref k) if k == "deprecated_field")
        );
    }

    #[test]
    fn test_index_update_chaining() {
        let mut update = IndexUpdate::default();
        update
            .set_name("updated_idx")
            .set_option("opt1", "val1")
            .clear_option("opt2")
            .set_metadata("meta1", "val1")
            .clear_metadata("meta2");

        assert_eq!(update.name, Some("updated_idx".to_string()));
        assert_eq!(update.options.len(), 2);
        assert_eq!(update.metadata.len(), 2);
    }

    #[test]
    fn test_index_status_default() {
        let status = IndexStatus::default();
        assert_eq!(status, IndexStatus::Active);
    }

    #[test]
    fn test_index_status_ordering() {
        assert!(IndexStatus::Building < IndexStatus::Active);
        assert!(IndexStatus::Active < IndexStatus::Rebuilding);
        assert!(IndexStatus::Rebuilding < IndexStatus::Disabled);
    }

    #[test]
    fn test_index_type_variants() {
        // Ensure all index types can be created
        let types = vec![
            IndexType::Secondary,
            IndexType::Unique,
            IndexType::FullText,
            IndexType::Spatial,
            IndexType::VectorFlat,
            IndexType::VectorHNSW,
            IndexType::VectorIVF,
            IndexType::VectorIVFPQ,
            IndexType::VectorAnnoy,
            IndexType::VectorVamana,
        ];

        assert_eq!(types.len(), 10);
    }

    #[test]
    fn test_index_sharding_default() {
        let sharding = IndexSharding::default();
        assert_eq!(sharding, IndexSharding::Single);
    }

    #[test]
    fn test_index_sharding_multiple() {
        let sharding = IndexSharding::Multiple {
            shard_count: 4,
            partitioner: Partitioner::Hash {
                hash_fn: HashFunction::Murmur3,
            },
            replication_factor: 3,
        };

        match sharding {
            IndexSharding::Multiple {
                shard_count,
                replication_factor,
                ..
            } => {
                assert_eq!(shard_count, 4);
                assert_eq!(replication_factor, 3);
            }
            _ => panic!("Expected Multiple sharding"),
        }
    }

    #[test]
    fn test_index_shard_create_basic() {
        let container = ContainerId::from_parts(TenantId(1), DatabaseId(2));
        let index = IndexId::new(ObjectId::new(3));
        let shard_number = ShardNumber(0);
        let engine = StorageEngineType::new("lsm");

        let create = IndexShardCreate::new(container, index, shard_number, engine.clone());

        assert_eq!(create.container, container);
        assert_eq!(create.index, index);
        assert_eq!(create.shard_number, shard_number);
        assert_eq!(create.engine_type, engine);
        assert_eq!(create.replication_factor, 1);
    }

    #[test]
    fn test_index_shard_create_with_replication() {
        let container = ContainerId::from_parts(TenantId(1), DatabaseId(2));
        let create = IndexShardCreate::new(
            container,
            IndexId(ObjectId::new(1)),
            ShardNumber(0),
            StorageEngineType::new("lsm"),
        )
        .with_replication(3);

        assert_eq!(create.replication_factor, 3);
    }

    #[test]
    fn test_index_shard_update() {
        let update = IndexShardUpdate::new(5);
        assert_eq!(update.replication_factor, 5);
    }

    #[test]
    fn test_index_shard_status_default() {
        let status = ShardStatus::default();
        assert_eq!(status, ShardStatus::Active);
    }

    #[test]
    fn test_index_shard_status_ordering() {
        assert!(ShardStatus::Active < ShardStatus::Rebalancing);
        assert!(ShardStatus::Rebalancing < ShardStatus::Splitting);
        assert!(ShardStatus::Splitting < ShardStatus::Merging);
        assert!(ShardStatus::Merging < ShardStatus::Offline);
    }

    #[test]
    fn test_index_metadata_from_record() {
        use crate::types::Timestamp;

        let now = Timestamp::now();
        let record = IndexRecord {
            index_id: IndexId(ObjectId::new(1)),
            name: "test_index".to_string(),
            version: 1,
            index_type: IndexType::Secondary,
            created_at: now,
            updated_at: now,
            columns: vec!["col1".to_string()],
            key_extractor: Some("extractor".to_string()),
            options: HashMap::new(),
            metadata: HashMap::new(),
            status: IndexStatus::Active,
            sharding: Default::default(),
        };

        let metadata = IndexMetadata::from(record.clone());

        assert_eq!(metadata.id, record.index_id);
        assert_eq!(metadata.name, record.name);
        assert_eq!(metadata.index_type, record.index_type);
        assert_eq!(metadata.columns, record.columns);
        assert_eq!(metadata.key_extractor, record.key_extractor);
    }

    #[test]
    fn test_index_shard_id_zero_components() {
        let shard_id = IndexShardId::from_parts(
            TenantId(0),
            DatabaseId(0),
            IndexId(ObjectId::new(0)),
            ShardNumber(0),
        );
        assert_eq!(shard_id.as_u128(), 0);
    }

    #[test]
    fn test_index_shard_id_max_components() {
        let shard_id = IndexShardId::from_parts(
            TenantId(u32::MAX),
            DatabaseId(u32::MAX),
            IndexId(ObjectId::new(u32::MAX)),
            ShardNumber(u32::MAX),
        );
        assert_eq!(shard_id.as_u128(), u128::MAX);
    }
}
