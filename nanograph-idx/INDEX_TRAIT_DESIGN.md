# Index Trait Design Document

## Overview

This document describes the trait hierarchy for Nanograph's index implementations, supporting 11 distinct index types across multiple categories with different operational characteristics.

## Index Type Categories

### 1. Ordered Indexes
- **Secondary (B-Tree)**: Range queries, sorted scans, prefix matching
- **Unique (Hash)**: Uniqueness constraints, O(1) point lookups

### 2. Text Search Indexes
- **FullText (Inverted Index)**: Text search, keyword matching, relevance scoring

### 3. Spatial Indexes
- **Spatial (R-Tree)**: Geographic queries, bounding box searches, nearest neighbor

### 4. Vector Similarity Indexes (Planned)
- **VectorFlat**: Brute-force exact search (small datasets <10K)
- **VectorHNSW**: Graph-based approximate search (high recall)
- **VectorIVF**: Clustering-based search (memory efficient)
- **VectorIVFPQ**: Compressed vectors (very large datasets)
- **VectorAnnoy**: Tree-based static datasets (read-heavy)
- **VectorVamana**: Disk-based billion-scale search

## Trait Hierarchy

```
IndexStore (base trait - all indexes)
    ├── OrderedIndex (range queries, sorted access)
    │   ├── BTreeIndex (Secondary)
    │   └── HashIndex (Unique) - partial implementation
    │
    ├── TextSearchIndex (text analysis, relevance)
    │   └── FullTextIndex
    │
    ├── SpatialIndex (geometric queries)
    │   └── RTreeIndex (Spatial)
    │
    └── VectorIndex (similarity search)
        ├── ExactVectorIndex (brute-force)
        │   └── VectorFlat
        │
        └── ApproximateVectorIndex (ANN search)
            ├── MutableVectorIndex (dynamic updates)
            │   ├── VectorHNSW
            │   ├── VectorIVF
            │   └── VectorIVFPQ
            │
            └── ImmutableVectorIndex (static, read-only)
                ├── VectorAnnoy
                └── VectorVamana
```

## Trait Definitions

### 1. Base Trait: `IndexStore`

**Purpose**: Common operations for all index types

**Operations**:
- Metadata access
- Build from table data
- Insert/update/delete entries
- Basic query interface
- Statistics and maintenance
- Persistence (flush)

**Already Implemented**: Yes (in [`store.rs`](nanograph-idx/src/store.rs:90))

---

### 2. Ordered Index Traits

#### `OrderedIndex`

**Purpose**: Indexes that maintain sorted order and support range queries

**Key Operations**:
```rust
pub trait OrderedIndex: IndexStore {
    /// Range scan with bounds
    async fn range_scan(
        &self,
        start: Bound<Vec<u8>>,
        end: Bound<Vec<u8>>,
        limit: Option<usize>,
        reverse: bool,
    ) -> IndexResult<Vec<IndexEntry>>;
    
    /// Get the minimum key
    async fn min_key(&self) -> IndexResult<Option<Vec<u8>>>;
    
    /// Get the maximum key
    async fn max_key(&self) -> IndexResult<Option<Vec<u8>>>;
    
    /// Prefix scan (for string keys)
    async fn prefix_scan(
        &self,
        prefix: &[u8],
        limit: Option<usize>,
    ) -> IndexResult<Vec<IndexEntry>>;
}
```

**Implementations**:
- `BTreeIndex` (Secondary) - full support
- `HashIndex` (Unique) - limited support (exact match only)

**Characteristics**:
- **Approximate Search**: No (exact matches and ranges)
- **Mutability**: Mutable (supports dynamic updates)
- **Ordering**: Maintains sort order

---

### 3. Text Search Traits

#### `TextSearchIndex`

**Purpose**: Full-text search with tokenization and relevance scoring

**Key Operations**:
```rust
pub trait TextSearchIndex: IndexStore {
    /// Search for documents matching query terms
    async fn search(
        &self,
        query: &str,
        limit: Option<usize>,
    ) -> IndexResult<Vec<ScoredEntry>>;
    
    /// Phrase search (exact phrase matching)
    async fn phrase_search(
        &self,
        phrase: &str,
        limit: Option<usize>,
    ) -> IndexResult<Vec<ScoredEntry>>;
    
    /// Boolean search (AND, OR, NOT operators)
    async fn boolean_search(
        &self,
        query: BooleanQuery,
        limit: Option<usize>,
    ) -> IndexResult<Vec<ScoredEntry>>;
    
    /// Get tokenization configuration
    fn tokenizer_config(&self) -> &TokenizerConfig;
    
    /// Get scoring algorithm (TF-IDF, BM25, etc.)
    fn scoring_algorithm(&self) -> ScoringAlgorithm;
}

pub struct ScoredEntry {
    pub entry: IndexEntry,
    pub score: f64,
    pub highlights: Vec<TextHighlight>,
}
```

**Implementations**:
- `FullTextIndex` (Inverted Index)

**Characteristics**:
- **Approximate Search**: Yes (fuzzy matching, stemming)
- **Mutability**: Mutable (supports dynamic updates)
- **Ordering**: By relevance score

---

### 4. Spatial Index Traits

#### `SpatialIndex`

**Purpose**: Geometric and geographic queries

**Key Operations**:
```rust
pub trait SpatialIndex: IndexStore {
    /// Query by bounding box
    async fn query_bbox(
        &self,
        bbox: BoundingBox,
    ) -> IndexResult<Vec<IndexEntry>>;
    
    /// K-nearest neighbors search
    async fn query_knn(
        &self,
        point: Point,
        k: usize,
    ) -> IndexResult<Vec<DistancedEntry>>;
    
    /// Radius search (all points within distance)
    async fn query_radius(
        &self,
        center: Point,
        radius: f64,
    ) -> IndexResult<Vec<DistancedEntry>>;
    
    /// Point-in-polygon test
    async fn query_polygon(
        &self,
        polygon: &[Point],
    ) -> IndexResult<Vec<IndexEntry>>;
    
    /// Get distance metric
    fn distance_metric(&self) -> DistanceMetric;
}

pub struct DistancedEntry {
    pub entry: IndexEntry,
    pub distance: f64,
}
```

**Implementations**:
- `RTreeIndex` (Spatial)

**Characteristics**:
- **Approximate Search**: No (exact geometric queries)
- **Mutability**: Mutable (supports dynamic updates)
- **Ordering**: By distance or spatial proximity

---

### 5. Vector Similarity Traits

#### `VectorIndex` (Base)

**Purpose**: Common operations for all vector similarity indexes

**Key Operations**:
```rust
pub trait VectorIndex: IndexStore {
    /// K-nearest neighbors search
    async fn search_knn(
        &self,
        query_vector: &[f32],
        k: usize,
    ) -> IndexResult<Vec<SimilarityEntry>>;
    
    /// Range search (all vectors within distance threshold)
    async fn search_range(
        &self,
        query_vector: &[f32],
        radius: f32,
    ) -> IndexResult<Vec<SimilarityEntry>>;
    
    /// Get vector dimensionality
    fn dimensions(&self) -> usize;
    
    /// Get distance metric (Euclidean, Cosine, Dot Product)
    fn distance_metric(&self) -> VectorDistanceMetric;
}

pub struct SimilarityEntry {
    pub entry: IndexEntry,
    pub distance: f32,
    pub similarity_score: f32,
}
```

#### `ExactVectorIndex`

**Purpose**: Brute-force exact nearest neighbor search

**Key Operations**:
```rust
pub trait ExactVectorIndex: VectorIndex {
    /// Guaranteed exact k-NN results
    async fn exact_search_knn(
        &self,
        query_vector: &[f32],
        k: usize,
    ) -> IndexResult<Vec<SimilarityEntry>>;
}
```

**Implementations**:
- `VectorFlat`

**Characteristics**:
- **Approximate Search**: No (exact results)
- **Mutability**: Mutable
- **Performance**: O(n) search time, best for <10K vectors

#### `ApproximateVectorIndex`

**Purpose**: Fast approximate nearest neighbor (ANN) search

**Key Operations**:
```rust
pub trait ApproximateVectorIndex: VectorIndex {
    /// Approximate k-NN with recall guarantee
    async fn approximate_search_knn(
        &self,
        query_vector: &[f32],
        k: usize,
        recall_target: f32, // 0.0-1.0
    ) -> IndexResult<Vec<SimilarityEntry>>;
    
    /// Get index build parameters
    fn build_params(&self) -> &AnnBuildParams;
    
    /// Get search parameters
    fn search_params(&self) -> &AnnSearchParams;
}
```

#### `MutableVectorIndex`

**Purpose**: Vector indexes supporting dynamic updates

**Key Operations**:
```rust
pub trait MutableVectorIndex: ApproximateVectorIndex {
    /// Add vector to index (incremental)
    async fn add_vector(
        &mut self,
        vector: &[f32],
        primary_key: Vec<u8>,
    ) -> IndexResult<()>;
    
    /// Remove vector from index
    async fn remove_vector(
        &mut self,
        primary_key: &[u8],
    ) -> IndexResult<()>;
    
    /// Update vector in place
    async fn update_vector(
        &mut self,
        primary_key: &[u8],
        new_vector: &[f32],
    ) -> IndexResult<()>;
}
```

**Implementations**:
- `VectorHNSW` - graph-based, high recall
- `VectorIVF` - clustering-based, memory efficient
- `VectorIVFPQ` - compressed, very large datasets

**Characteristics**:
- **Approximate Search**: Yes (configurable recall)
- **Mutability**: Mutable (dynamic updates)
- **Performance**: Sub-linear search time

#### `ImmutableVectorIndex`

**Purpose**: Static vector indexes optimized for read-heavy workloads

**Key Operations**:
```rust
pub trait ImmutableVectorIndex: ApproximateVectorIndex {
    /// Build index from complete dataset (one-time)
    async fn build_from_vectors(
        &mut self,
        vectors: Vec<(Vec<u8>, Vec<f32>)>,
    ) -> IndexResult<()>;
    
    /// Check if index is built and ready
    fn is_built(&self) -> bool;
    
    /// Rebuild index (expensive operation)
    async fn rebuild(&mut self) -> IndexResult<()>;
}
```

**Implementations**:
- `VectorAnnoy` - tree-based, Spotify-style recommendations
- `VectorVamana` - disk-based, billion-scale datasets

**Characteristics**:
- **Approximate Search**: Yes (configurable recall)
- **Mutability**: Immutable (requires rebuild for updates)
- **Performance**: Optimized for static datasets

---

## Design Decisions

### 1. Trait Composition

**Decision**: Use trait inheritance for specialization

**Rationale**:
- Clear hierarchy showing relationships
- Allows generic code over trait bounds
- Enables progressive feature addition
- Maintains backward compatibility

### 2. Async Operations

**Decision**: All index operations are async

**Rationale**:
- Supports I/O operations (disk, network)
- Enables concurrent index operations
- Aligns with Tokio runtime
- Future-proof for distributed indexes

### 3. Separate Mutable/Immutable Traits

**Decision**: Distinguish mutable vs immutable vector indexes

**Rationale**:
- Some algorithms (Annoy, Vamana) require full rebuild
- Prevents incorrect usage patterns
- Enables optimization for static datasets
- Clear API contract

### 4. Approximate vs Exact Search

**Decision**: Separate traits for exact and approximate search

**Rationale**:
- Different performance guarantees
- Different use cases and expectations
- Allows algorithm-specific optimizations
- Clear documentation of behavior

### 5. Distance Metrics as Configuration

**Decision**: Distance metrics are part of index configuration, not trait methods

**Rationale**:
- Metrics are set at index creation time
- Changing metrics requires rebuild
- Simplifies trait interface
- Allows metric-specific optimizations

---

## Implementation Strategy

### Phase 1: Core Traits (Current)
1. ✅ `IndexStore` - base trait (already implemented)
2. 🚧 `OrderedIndex` - range queries
3. 🚧 `TextSearchIndex` - full-text search
4. 🚧 `SpatialIndex` - geometric queries

### Phase 2: Vector Base (Next)
5. ⏳ `VectorIndex` - common vector operations
6. ⏳ `ExactVectorIndex` - brute-force search
7. ⏳ `ApproximateVectorIndex` - ANN search

### Phase 3: Vector Specialization (Future)
8. ⏳ `MutableVectorIndex` - dynamic updates
9. ⏳ `ImmutableVectorIndex` - static datasets

### Phase 4: Implementations (Future)
10. ⏳ Implement all 11 index types
11. ⏳ Add comprehensive tests
12. ⏳ Add benchmarks

---

## Usage Examples

### Example 1: Ordered Index (B-Tree)

```rust
use nanograph_idx::{BTreeIndex, OrderedIndex};

let index = BTreeIndex::new(metadata)?;

// Range scan
let results = index.range_scan(
    Bound::Included(b"a".to_vec()),
    Bound::Excluded(b"z".to_vec()),
    Some(100),
    false,
).await?;

// Prefix scan
let users = index.prefix_scan(b"user:", Some(50)).await?;
```

### Example 2: Text Search Index

```rust
use nanograph_idx::{FullTextIndex, TextSearchIndex};

let index = FullTextIndex::new(metadata)?;

// Simple search
let results = index.search("rust database", Some(10)).await?;

// Boolean search
let query = BooleanQuery::and(vec![
    Term::new("rust"),
    Term::new("database"),
]);
let results = index.boolean_search(query, Some(10)).await?;
```

### Example 3: Spatial Index

```rust
use nanograph_idx::{RTreeIndex, SpatialIndex, Point, BoundingBox};

let index = RTreeIndex::new(metadata)?;

// Bounding box query
let bbox = BoundingBox::new(
    Point::new(37.0, -122.0),
    Point::new(38.0, -121.0),
);
let locations = index.query_bbox(bbox).await?;

// K-nearest neighbors
let center = Point::new(37.7749, -122.4194); // San Francisco
let nearest = index.query_knn(center, 10).await?;
```

### Example 4: Vector Index (Mutable)

```rust
use nanograph_idx::{VectorHNSW, MutableVectorIndex};

let mut index = VectorHNSW::new(metadata, 128)?; // 128 dimensions

// Add vectors dynamically
index.add_vector(&embedding1, b"doc1".to_vec()).await?;
index.add_vector(&embedding2, b"doc2".to_vec()).await?;

// Search
let similar = index.approximate_search_knn(&query_vector, 10, 0.95).await?;
```

### Example 5: Vector Index (Immutable)

```rust
use nanograph_idx::{VectorAnnoy, ImmutableVectorIndex};

let mut index = VectorAnnoy::new(metadata, 128)?;

// Build from complete dataset
let vectors = vec![
    (b"doc1".to_vec(), embedding1),
    (b"doc2".to_vec(), embedding2),
    // ... millions more
];
index.build_from_vectors(vectors).await?;

// Search (fast, read-only)
let similar = index.approximate_search_knn(&query_vector, 10, 0.95).await?;
```

---

## Testing Strategy

### Unit Tests
- Each trait method tested independently
- Mock implementations for trait testing
- Edge cases and error conditions

### Integration Tests
- Cross-trait interactions
- Index lifecycle (create, build, query, maintain)
- Concurrent operations

### Performance Tests
- Benchmark each index type
- Compare approximate vs exact search
- Measure build time vs query time tradeoffs

### Correctness Tests
- Verify exact indexes return correct results
- Measure recall for approximate indexes
- Test uniqueness constraints
- Validate spatial queries

---

## Future Enhancements

### 1. Composite Indexes
- Multi-column indexes
- Covering indexes with included columns

### 2. Distributed Indexes
- Sharded indexes across nodes
- Replicated indexes for high availability

### 3. Adaptive Indexes
- Auto-tuning based on query patterns
- Dynamic index selection

### 4. Specialized Vector Indexes
- GPU-accelerated search
- Quantization strategies
- Hybrid exact/approximate search

---

## References

- [B-Tree Index Design](https://en.wikipedia.org/wiki/B-tree)
- [Inverted Index for Full-Text Search](https://en.wikipedia.org/wiki/Inverted_index)
- [R-Tree for Spatial Indexing](https://en.wikipedia.org/wiki/R-tree)
- [HNSW Algorithm](https://arxiv.org/abs/1603.09320)
- [IVF and Product Quantization](https://arxiv.org/abs/1702.08734)
- [Annoy by Spotify](https://github.com/spotify/annoy)
- [DiskANN/Vamana](https://arxiv.org/abs/1907.05046)

---

**Document Version**: 1.0  
**Last Updated**: 2026-01-26  
**Status**: Design Phase