# Index Trait Implementation Plan

## Overview

This document provides the detailed implementation plan for the index trait hierarchy, including specific trait definitions, method signatures, and implementation steps.

## Phase 1: Ordered Index Traits

### File: `src/traits/ordered.rs`

```rust
//! Ordered index traits for range queries and sorted access

use crate::error::IndexResult;
use crate::store::{IndexEntry, IndexStore};
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
    async fn count_range(
        &self,
        start: Bound<Vec<u8>>,
        end: Bound<Vec<u8>>,
    ) -> IndexResult<u64>;
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
    async fn lookup_unique(
        &self,
        indexed_value: &[u8],
    ) -> IndexResult<Option<Vec<u8>>>;

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
```

**Implementation Steps**:
1. Create `src/traits/` directory
2. Create `src/traits/mod.rs` with module exports
3. Create `src/traits/ordered.rs` with trait definitions
4. Update `BTreeIndex` to implement `OrderedIndex`
5. Update `HashIndex` to implement `UniqueIndex`
6. Add unit tests for trait implementations

---

## Phase 2: Text Search Traits

### File: `src/traits/text.rs`

```rust
//! Text search index traits for full-text search

use crate::error::IndexResult;
use crate::store::{IndexEntry, IndexStore};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

/// Entry with relevance score
#[derive(Debug, Clone)]
pub struct ScoredEntry {
    /// The index entry
    pub entry: IndexEntry,
    /// Relevance score (higher is more relevant)
    pub score: f64,
    /// Text highlights showing matched terms
    pub highlights: Vec<TextHighlight>,
}

/// Highlighted text fragment
#[derive(Debug, Clone)]
pub struct TextHighlight {
    /// The matched text fragment
    pub fragment: String,
    /// Start position in original text
    pub start: usize,
    /// End position in original text
    pub end: usize,
    /// Matched terms in this fragment
    pub matched_terms: Vec<String>,
}

/// Boolean query operators
#[derive(Debug, Clone)]
pub enum BooleanQuery {
    /// Match all terms (AND)
    And(Vec<Term>),
    /// Match any term (OR)
    Or(Vec<Term>),
    /// Exclude term (NOT)
    Not(Box<BooleanQuery>),
    /// Nested query
    Nested(Box<BooleanQuery>),
}

/// Search term
#[derive(Debug, Clone)]
pub struct Term {
    /// The term text
    pub text: String,
    /// Optional boost factor
    pub boost: Option<f64>,
}

impl Term {
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            boost: None,
        }
    }

    pub fn with_boost(mut self, boost: f64) -> Self {
        self.boost = Some(boost);
        self
    }
}

/// Tokenizer configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenizerConfig {
    /// Convert to lowercase
    pub lowercase: bool,
    /// Remove punctuation
    pub remove_punctuation: bool,
    /// Apply stemming
    pub stemming: bool,
    /// Remove stop words
    pub remove_stop_words: bool,
    /// Stop words list
    pub stop_words: Vec<String>,
}

impl Default for TokenizerConfig {
    fn default() -> Self {
        Self {
            lowercase: true,
            remove_punctuation: true,
            stemming: false,
            remove_stop_words: false,
            stop_words: vec![],
        }
    }
}

/// Scoring algorithm
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScoringAlgorithm {
    /// Term Frequency-Inverse Document Frequency
    TfIdf,
    /// Best Matching 25 (Okapi BM25)
    Bm25,
    /// Simple term frequency
    TermFrequency,
}

/// Trait for full-text search indexes
#[async_trait]
pub trait TextSearchIndex: IndexStore {
    /// Search for documents matching query terms
    ///
    /// # Arguments
    /// * `query` - Search query string
    /// * `limit` - Maximum number of results
    ///
    /// # Returns
    /// * `Ok(Vec<ScoredEntry>)` - Results sorted by relevance
    async fn search(
        &self,
        query: &str,
        limit: Option<usize>,
    ) -> IndexResult<Vec<ScoredEntry>>;

    /// Search for exact phrase
    ///
    /// # Arguments
    /// * `phrase` - Exact phrase to match
    /// * `limit` - Maximum number of results
    ///
    /// # Returns
    /// * `Ok(Vec<ScoredEntry>)` - Results containing the phrase
    async fn phrase_search(
        &self,
        phrase: &str,
        limit: Option<usize>,
    ) -> IndexResult<Vec<ScoredEntry>>;

    /// Boolean search with AND/OR/NOT operators
    ///
    /// # Arguments
    /// * `query` - Boolean query structure
    /// * `limit` - Maximum number of results
    ///
    /// # Returns
    /// * `Ok(Vec<ScoredEntry>)` - Results matching the boolean query
    async fn boolean_search(
        &self,
        query: BooleanQuery,
        limit: Option<usize>,
    ) -> IndexResult<Vec<ScoredEntry>>;

    /// Get tokenizer configuration
    fn tokenizer_config(&self) -> &TokenizerConfig;

    /// Get scoring algorithm
    fn scoring_algorithm(&self) -> ScoringAlgorithm;

    /// Get term statistics
    ///
    /// # Arguments
    /// * `term` - The term to get statistics for
    ///
    /// # Returns
    /// * `Ok(Some(stats))` - Statistics if term exists
    /// * `Ok(None)` - If term doesn't exist
    async fn term_stats(&self, term: &str) -> IndexResult<Option<TermStats>>;
}

/// Statistics for a term
#[derive(Debug, Clone)]
pub struct TermStats {
    /// Number of documents containing the term
    pub document_frequency: u64,
    /// Total occurrences across all documents
    pub total_frequency: u64,
    /// Average positions per document
    pub avg_positions: f64,
}
```

**Implementation Steps**:
1. Create `src/traits/text.rs` with trait definitions
2. Add supporting types (ScoredEntry, BooleanQuery, etc.)
3. Update `FullTextIndex` to implement `TextSearchIndex`
4. Implement tokenization logic
5. Implement scoring algorithms (TF-IDF, BM25)
6. Add comprehensive tests

---

## Phase 3: Spatial Index Traits

### File: `src/traits/spatial.rs`

```rust
//! Spatial index traits for geometric queries

use crate::error::IndexResult;
use crate::store::{IndexEntry, IndexStore};
use async_trait::async_trait;

/// A point in 2D space
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Point {
    pub x: f64,
    pub y: f64,
}

impl Point {
    pub fn new(x: f64, y: f64) -> Self {
        Self { x, y }
    }

    /// Calculate Euclidean distance to another point
    pub fn distance_to(&self, other: &Point) -> f64 {
        let dx = self.x - other.x;
        let dy = self.y - other.y;
        (dx * dx + dy * dy).sqrt()
    }

    /// Calculate Manhattan distance to another point
    pub fn manhattan_distance_to(&self, other: &Point) -> f64 {
        (self.x - other.x).abs() + (self.y - other.y).abs()
    }
}

/// A bounding box in 2D space
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BoundingBox {
    pub min: Point,
    pub max: Point,
}

impl BoundingBox {
    pub fn new(min: Point, max: Point) -> Self {
        Self { min, max }
    }

    /// Check if this box contains a point
    pub fn contains(&self, point: &Point) -> bool {
        point.x >= self.min.x
            && point.x <= self.max.x
            && point.y >= self.min.y
            && point.y <= self.max.y
    }

    /// Check if this box intersects another box
    pub fn intersects(&self, other: &BoundingBox) -> bool {
        self.min.x <= other.max.x
            && self.max.x >= other.min.x
            && self.min.y <= other.max.y
            && self.max.y >= other.min.y
    }

    /// Calculate the area of this box
    pub fn area(&self) -> f64 {
        (self.max.x - self.min.x) * (self.max.y - self.min.y)
    }

    /// Expand box to include a point
    pub fn expand_to_include(&mut self, point: &Point) {
        self.min.x = self.min.x.min(point.x);
        self.min.y = self.min.y.min(point.y);
        self.max.x = self.max.x.max(point.x);
        self.max.y = self.max.y.max(point.y);
    }
}

/// Entry with distance information
#[derive(Debug, Clone)]
pub struct DistancedEntry {
    /// The index entry
    pub entry: IndexEntry,
    /// Distance from query point
    pub distance: f64,
}

/// Distance metric for spatial queries
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DistanceMetric {
    /// Euclidean distance (L2 norm)
    Euclidean,
    /// Manhattan distance (L1 norm)
    Manhattan,
    /// Haversine distance (great-circle distance on sphere)
    Haversine,
}

/// Trait for spatial indexes
#[async_trait]
pub trait SpatialIndex: IndexStore {
    /// Query by bounding box
    ///
    /// # Arguments
    /// * `bbox` - The bounding box to search within
    ///
    /// # Returns
    /// * `Ok(Vec<IndexEntry>)` - All entries within the bounding box
    async fn query_bbox(
        &self,
        bbox: BoundingBox,
    ) -> IndexResult<Vec<IndexEntry>>;

    /// K-nearest neighbors search
    ///
    /// # Arguments
    /// * `point` - The query point
    /// * `k` - Number of nearest neighbors to return
    ///
    /// # Returns
    /// * `Ok(Vec<DistancedEntry>)` - K nearest entries sorted by distance
    async fn query_knn(
        &self,
        point: Point,
        k: usize,
    ) -> IndexResult<Vec<DistancedEntry>>;

    /// Radius search (all points within distance)
    ///
    /// # Arguments
    /// * `center` - The center point
    /// * `radius` - Maximum distance from center
    ///
    /// # Returns
    /// * `Ok(Vec<DistancedEntry>)` - All entries within radius
    async fn query_radius(
        &self,
        center: Point,
        radius: f64,
    ) -> IndexResult<Vec<DistancedEntry>>;

    /// Point-in-polygon test
    ///
    /// # Arguments
    /// * `polygon` - Vertices of the polygon (closed)
    ///
    /// # Returns
    /// * `Ok(Vec<IndexEntry>)` - All entries inside the polygon
    async fn query_polygon(
        &self,
        polygon: &[Point],
    ) -> IndexResult<Vec<IndexEntry>>;

    /// Get distance metric used by this index
    fn distance_metric(&self) -> DistanceMetric;

    /// Get spatial statistics
    async fn spatial_stats(&self) -> IndexResult<SpatialStats>;
}

/// Statistics for spatial index
#[derive(Debug, Clone)]
pub struct SpatialStats {
    /// Total number of points
    pub point_count: u64,
    /// Bounding box of all points
    pub total_bounds: BoundingBox,
    /// Average density (points per unit area)
    pub avg_density: f64,
    /// Number of R-Tree nodes (if applicable)
    pub node_count: Option<u64>,
}
```

**Implementation Steps**:
1. Create `src/traits/spatial.rs` with trait definitions
2. Move Point and BoundingBox from `spatial.rs` to traits
3. Update `SpatialIndex` implementation to use new trait
4. Add distance metric support
5. Implement polygon queries
6. Add spatial statistics
7. Add comprehensive tests

---

## Phase 4: Vector Index Traits

### File: `src/traits/vector.rs`

```rust
//! Vector similarity index traits

use crate::error::IndexResult;
use crate::store::{IndexEntry, IndexStore};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

/// Entry with similarity information
#[derive(Debug, Clone)]
pub struct SimilarityEntry {
    /// The index entry
    pub entry: IndexEntry,
    /// Distance from query vector (lower is more similar)
    pub distance: f32,
    /// Similarity score (higher is more similar, 0.0-1.0)
    pub similarity_score: f32,
}

/// Distance metric for vector similarity
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum VectorDistanceMetric {
    /// Euclidean distance (L2 norm)
    Euclidean,
    /// Cosine distance (1 - cosine similarity)
    Cosine,
    /// Dot product (inner product)
    DotProduct,
    /// Manhattan distance (L1 norm)
    Manhattan,
}

/// Build parameters for ANN indexes
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnnBuildParams {
    /// Number of neighbors per node (HNSW)
    pub m: Option<usize>,
    /// Construction time parameter (HNSW)
    pub ef_construction: Option<usize>,
    /// Number of clusters (IVF)
    pub n_lists: Option<usize>,
    /// Number of trees (Annoy)
    pub n_trees: Option<usize>,
    /// Max degree (Vamana)
    pub max_degree: Option<usize>,
}

/// Search parameters for ANN indexes
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnnSearchParams {
    /// Search time parameter (HNSW)
    pub ef_search: Option<usize>,
    /// Number of clusters to probe (IVF)
    pub n_probe: Option<usize>,
    /// Search depth (Annoy)
    pub search_k: Option<usize>,
}

/// Base trait for all vector indexes
#[async_trait]
pub trait VectorIndex: IndexStore {
    /// K-nearest neighbors search
    ///
    /// # Arguments
    /// * `query_vector` - The query vector
    /// * `k` - Number of nearest neighbors to return
    ///
    /// # Returns
    /// * `Ok(Vec<SimilarityEntry>)` - K nearest vectors
    async fn search_knn(
        &self,
        query_vector: &[f32],
        k: usize,
    ) -> IndexResult<Vec<SimilarityEntry>>;

    /// Range search (all vectors within distance threshold)
    ///
    /// # Arguments
    /// * `query_vector` - The query vector
    /// * `radius` - Maximum distance threshold
    ///
    /// # Returns
    /// * `Ok(Vec<SimilarityEntry>)` - All vectors within radius
    async fn search_range(
        &self,
        query_vector: &[f32],
        radius: f32,
    ) -> IndexResult<Vec<SimilarityEntry>>;

    /// Get vector dimensionality
    fn dimensions(&self) -> usize;

    /// Get distance metric
    fn distance_metric(&self) -> VectorDistanceMetric;

    /// Get vector statistics
    async fn vector_stats(&self) -> IndexResult<VectorStats>;
}

/// Statistics for vector index
#[derive(Debug, Clone)]
pub struct VectorStats {
    /// Number of vectors
    pub vector_count: u64,
    /// Vector dimensionality
    pub dimensions: usize,
    /// Average vector magnitude
    pub avg_magnitude: f32,
    /// Index-specific statistics
    pub index_specific: Option<String>,
}

/// Trait for exact vector search
#[async_trait]
pub trait ExactVectorIndex: VectorIndex {
    /// Guaranteed exact k-NN results
    ///
    /// # Arguments
    /// * `query_vector` - The query vector
    /// * `k` - Number of nearest neighbors
    ///
    /// # Returns
    /// * `Ok(Vec<SimilarityEntry>)` - Exact k nearest vectors
    async fn exact_search_knn(
        &self,
        query_vector: &[f32],
        k: usize,
    ) -> IndexResult<Vec<SimilarityEntry>>;
}

/// Trait for approximate vector search
#[async_trait]
pub trait ApproximateVectorIndex: VectorIndex {
    /// Approximate k-NN with recall target
    ///
    /// # Arguments
    /// * `query_vector` - The query vector
    /// * `k` - Number of nearest neighbors
    /// * `recall_target` - Target recall (0.0-1.0)
    ///
    /// # Returns
    /// * `Ok(Vec<SimilarityEntry>)` - Approximate k nearest vectors
    async fn approximate_search_knn(
        &self,
        query_vector: &[f32],
        k: usize,
        recall_target: f32,
    ) -> IndexResult<Vec<SimilarityEntry>>;

    /// Get build parameters
    fn build_params(&self) -> &AnnBuildParams;

    /// Get search parameters
    fn search_params(&self) -> &AnnSearchParams;

    /// Estimate recall for given search parameters
    async fn estimate_recall(
        &self,
        search_params: &AnnSearchParams,
    ) -> IndexResult<f32>;
}

/// Trait for mutable vector indexes
#[async_trait]
pub trait MutableVectorIndex: ApproximateVectorIndex {
    /// Add vector to index (incremental)
    ///
    /// # Arguments
    /// * `vector` - The vector to add
    /// * `primary_key` - Primary key for the vector
    ///
    /// # Returns
    /// * `Ok(())` - If successful
    async fn add_vector(
        &mut self,
        vector: &[f32],
        primary_key: Vec<u8>,
    ) -> IndexResult<()>;

    /// Remove vector from index
    ///
    /// # Arguments
    /// * `primary_key` - Primary key of vector to remove
    ///
    /// # Returns
    /// * `Ok(())` - If successful
    async fn remove_vector(
        &mut self,
        primary_key: &[u8],
    ) -> IndexResult<()>;

    /// Update vector in place
    ///
    /// # Arguments
    /// * `primary_key` - Primary key of vector to update
    /// * `new_vector` - New vector values
    ///
    /// # Returns
    /// * `Ok(())` - If successful
    async fn update_vector(
        &mut self,
        primary_key: &[u8],
        new_vector: &[f32],
    ) -> IndexResult<()>;
}

/// Trait for immutable vector indexes
#[async_trait]
pub trait ImmutableVectorIndex: ApproximateVectorIndex {
    /// Build index from complete dataset (one-time)
    ///
    /// # Arguments
    /// * `vectors` - All vectors to index
    ///
    /// # Returns
    /// * `Ok(())` - If build succeeds
    async fn build_from_vectors(
        &mut self,
        vectors: Vec<(Vec<u8>, Vec<f32>)>,
    ) -> IndexResult<()>;

    /// Check if index is built and ready
    fn is_built(&self) -> bool;

    /// Rebuild index (expensive operation)
    ///
    /// # Returns
    /// * `Ok(())` - If rebuild succeeds
    async fn rebuild(&mut self) -> IndexResult<()>;
}
```

**Implementation Steps**:
1. Create `src/traits/vector.rs` with trait definitions
2. Add supporting types (SimilarityEntry, distance metrics, etc.)
3. Create stub implementations for each vector index type
4. Implement VectorFlat (exact search)
5. Implement VectorHNSW (mutable ANN)
6. Implement VectorAnnoy (immutable ANN)
7. Add comprehensive tests and benchmarks

---

## Implementation Timeline

### Week 1: Foundation
- [x] Design document
- [ ] Create traits module structure
- [ ] Implement OrderedIndex trait
- [ ] Update BTreeIndex and HashIndex

### Week 2: Text and Spatial
- [ ] Implement TextSearchIndex trait
- [ ] Update FullTextIndex
- [ ] Implement SpatialIndex trait
- [ ] Update RTreeIndex

### Week 3: Vector Base
- [ ] Implement VectorIndex base trait
- [ ] Implement ExactVectorIndex
- [ ] Implement VectorFlat

### Week 4: Vector ANN
- [ ] Implement ApproximateVectorIndex
- [ ] Implement MutableVectorIndex
- [ ] Implement ImmutableVectorIndex
- [ ] Create VectorHNSW stub

### Week 5: Testing and Documentation
- [ ] Comprehensive unit tests
- [ ] Integration tests
- [ ] Performance benchmarks
- [ ] API documentation
- [ ] Usage examples

---

## Testing Requirements

### Unit Tests
- Each trait method tested independently
- Mock implementations for testing
- Edge cases and error conditions
- Concurrent access patterns

### Integration Tests
- Cross-trait interactions
- Index lifecycle testing
- Query correctness validation
- Performance regression tests

### Benchmarks
- Index build time
- Query latency (p50, p95, p99)
- Memory usage
- Throughput (queries/second)

---

## Success Criteria

1. ✅ All traits compile without errors
2. ✅ All existing indexes implement appropriate traits
3. ✅ All tests pass (unit + integration)
4. ✅ Benchmarks show acceptable performance
5. ✅ Documentation is complete and accurate
6. ✅ Examples demonstrate all major use cases

---

**Document Version**: 1.0  
**Last Updated**: 2026-01-26  
**Status**: Implementation Ready