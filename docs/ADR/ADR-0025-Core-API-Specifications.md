---
parent: ADR
nav_order: 0025
title: Core API Specifications
status: proposed
date: 2026-01-07
deciders: Hans W. Uhlig
---

# ADR-0025: Core API Specifications

## Status

Proposed

## Context

Nanograph requires well-defined, stable APIs for all data models (KV, document, graph, vector) to ensure consistency across implementations, enable SDK development, and provide clear contracts for users. Without concrete API specifications, implementation details may leak into public interfaces, making future changes difficult and breaking backward compatibility.

The API must support both embedded (in-process) and standalone (networked) deployment modes while maintaining identical semantics.

## Decision

Define comprehensive, versioned API specifications for all core operations across all data models, with clear separation between:

1. **Storage APIs** - Low-level KV operations
2. **Document APIs** - JSON document operations
3. **Graph APIs** - Node and edge operations
4. **Vector APIs** - Embedding and similarity search operations
5. **Transaction APIs** - ACID transaction boundaries
6. **Admin APIs** - Cluster management and operations

All APIs will be:
- **Strongly typed** with explicit error handling
- **Versioned** using semantic versioning
- **Async-first** for non-blocking operations
- **Composable** allowing operations to be combined
- **Idempotent** where possible for retry safety

## Decision Drivers

* Need for stable public contracts
* SDK generation requirements
* Backward compatibility guarantees
* Clear separation of concerns
* Support for both embedded and networked modes
* Type safety and compile-time guarantees

## Design

### 1. Storage (KV) API

#### Core Operations

```rust
pub trait KvStore {
    /// Get a value by key
    async fn get(&self, table: TableId, key: &[u8]) -> Result<Option<Vec<u8>>>;
    
    /// Put a key-value pair
    async fn put(&self, table: TableId, key: &[u8], value: &[u8]) -> Result<()>;
    
    /// Delete a key
    async fn delete(&self, table: TableId, key: &[u8]) -> Result<bool>;
    
    /// Check if key exists
    async fn exists(&self, table: TableId, key: &[u8]) -> Result<bool>;
    
    /// Batch get multiple keys
    async fn batch_get(&self, table: TableId, keys: &[&[u8]]) -> Result<Vec<Option<Vec<u8>>>>;
    
    /// Batch put multiple key-value pairs
    async fn batch_put(&self, table: TableId, pairs: &[(&[u8], &[u8])]) -> Result<()>;
    
    /// Range scan with optional bounds
    async fn scan(&self, table: TableId, range: Range) -> Result<KvIterator>;
    
    /// Get approximate table size
    async fn table_size(&self, table: TableId) -> Result<TableStats>;
}

pub struct Range {
    pub start: Bound<Vec<u8>>,
    pub end: Bound<Vec<u8>>,
    pub limit: Option<usize>,
    pub reverse: bool,
}

pub struct TableStats {
    pub key_count: u64,
    pub total_bytes: u64,
    pub last_modified: Timestamp,
}
```

#### Iterator Interface

```rust
pub trait KvIterator: Stream<Item = Result<(Vec<u8>, Vec<u8>)>> {
    /// Seek to a specific key
    async fn seek(&mut self, key: &[u8]) -> Result<()>;
    
    /// Get current position
    fn position(&self) -> Option<Vec<u8>>;
}
```

### 2. Document API

```rust
pub trait DocumentStore {
    /// Insert a new document
    async fn insert(&self, collection: CollectionId, doc: Document) -> Result<DocumentId>;
    
    /// Get document by ID
    async fn get(&self, collection: CollectionId, id: DocumentId) -> Result<Option<Document>>;
    
    /// Update entire document
    async fn update(&self, collection: CollectionId, id: DocumentId, doc: Document) -> Result<()>;
    
    /// Partial update using JSON patch
    async fn patch(&self, collection: CollectionId, id: DocumentId, patch: JsonPatch) -> Result<()>;
    
    /// Delete document
    async fn delete(&self, collection: CollectionId, id: DocumentId) -> Result<bool>;
    
    /// Query documents with filter
    async fn query(&self, collection: CollectionId, filter: Filter) -> Result<DocumentIterator>;
    
    /// Create secondary index
    async fn create_index(&self, collection: CollectionId, spec: IndexSpec) -> Result<IndexId>;
    
    /// List all indexes
    async fn list_indexes(&self, collection: CollectionId) -> Result<Vec<IndexInfo>>;
}

pub struct Document {
    pub id: Option<DocumentId>,
    pub data: serde_json::Value,
    pub metadata: DocumentMetadata,
}

pub struct Filter {
    pub conditions: Vec<Condition>,
    pub limit: Option<usize>,
    pub offset: Option<usize>,
    pub sort: Option<SortSpec>,
}

pub enum Condition {
    Eq(String, serde_json::Value),
    Ne(String, serde_json::Value),
    Gt(String, serde_json::Value),
    Lt(String, serde_json::Value),
    In(String, Vec<serde_json::Value>),
    And(Vec<Condition>),
    Or(Vec<Condition>),
}
```

### 3. Graph API

```rust
pub trait GraphStore {
    /// Create a node
    async fn create_node(&self, graph: GraphId, properties: Properties) -> Result<NodeId>;
    
    /// Get node by ID
    async fn get_node(&self, graph: GraphId, id: NodeId) -> Result<Option<Node>>;
    
    /// Update node properties
    async fn update_node(&self, graph: GraphId, id: NodeId, properties: Properties) -> Result<()>;
    
    /// Delete node (and optionally its edges)
    async fn delete_node(&self, graph: GraphId, id: NodeId, cascade: bool) -> Result<bool>;
    
    /// Create an edge between nodes
    async fn create_edge(&self, graph: GraphId, from: NodeId, to: NodeId, 
                        edge_type: String, properties: Properties) -> Result<EdgeId>;
    
    /// Get edge by ID
    async fn get_edge(&self, graph: GraphId, id: EdgeId) -> Result<Option<Edge>>;
    
    /// Delete edge
    async fn delete_edge(&self, graph: GraphId, id: EdgeId) -> Result<bool>;
    
    /// Get neighbors of a node
    async fn neighbors(&self, graph: GraphId, node: NodeId, 
                      direction: Direction, edge_type: Option<String>) -> Result<Vec<NodeId>>;
    
    /// Traverse graph with bounded depth
    async fn traverse(&self, graph: GraphId, start: NodeId, 
                     spec: TraversalSpec) -> Result<TraversalIterator>;
    
    /// Find paths between nodes
    async fn find_paths(&self, graph: GraphId, from: NodeId, to: NodeId,
                       max_depth: usize) -> Result<Vec<Path>>;
}

pub struct Node {
    pub id: NodeId,
    pub properties: Properties,
    pub labels: Vec<String>,
}

pub struct Edge {
    pub id: EdgeId,
    pub from: NodeId,
    pub to: NodeId,
    pub edge_type: String,
    pub properties: Properties,
}

pub enum Direction {
    Outgoing,
    Incoming,
    Both,
}

pub struct TraversalSpec {
    pub max_depth: usize,
    pub direction: Direction,
    pub edge_types: Option<Vec<String>>,
    pub node_filter: Option<Filter>,
    pub edge_filter: Option<Filter>,
}
```

### 4. Vector API

```rust
pub trait VectorStore {
    /// Insert vector with associated data
    async fn insert(&self, collection: CollectionId, vector: Vector, 
                   metadata: Metadata) -> Result<VectorId>;
    
    /// Get vector by ID
    async fn get(&self, collection: CollectionId, id: VectorId) -> Result<Option<VectorEntry>>;
    
    /// Update vector
    async fn update(&self, collection: CollectionId, id: VectorId, vector: Vector) -> Result<()>;
    
    /// Delete vector
    async fn delete(&self, collection: CollectionId, id: VectorId) -> Result<bool>;
    
    /// Similarity search (ANN)
    async fn search(&self, collection: CollectionId, query: Vector, 
                   params: SearchParams) -> Result<Vec<SearchResult>>;
    
    /// Hybrid search (vector + filters)
    async fn hybrid_search(&self, collection: CollectionId, query: Vector,
                          filter: Filter, params: SearchParams) -> Result<Vec<SearchResult>>;
    
    /// Create vector index
    async fn create_index(&self, collection: CollectionId, 
                         spec: VectorIndexSpec) -> Result<IndexId>;
    
    /// Get index build status
    async fn index_status(&self, collection: CollectionId, 
                         index: IndexId) -> Result<IndexStatus>;
}

pub struct Vector {
    pub dimensions: usize,
    pub values: Vec<f32>,
}

pub struct SearchParams {
    pub k: usize,
    pub metric: DistanceMetric,
    pub ef_search: Option<usize>, // HNSW-specific
    pub nprobe: Option<usize>,    // IVF-specific
}

pub enum DistanceMetric {
    Cosine,
    Euclidean,
    DotProduct,
}

pub struct SearchResult {
    pub id: VectorId,
    pub distance: f32,
    pub metadata: Metadata,
}

pub struct VectorIndexSpec {
    pub index_type: VectorIndexType,
    pub dimensions: usize,
    pub metric: DistanceMetric,
    pub parameters: IndexParameters,
}

pub enum VectorIndexType {
    HNSW,
    IVF,
    Flat,
}
```

### 5. Transaction API

```rust
pub trait TransactionManager {
    /// Begin a new transaction
    async fn begin(&self, options: TxOptions) -> Result<Transaction>;
    
    /// Begin read-only transaction
    async fn begin_readonly(&self) -> Result<ReadTransaction>;
}

pub trait Transaction {
    /// Get within transaction
    async fn get(&self, table: TableId, key: &[u8]) -> Result<Option<Vec<u8>>>;
    
    /// Put within transaction
    async fn put(&self, table: TableId, key: &[u8], value: &[u8]) -> Result<()>;
    
    /// Delete within transaction
    async fn delete(&self, table: TableId, key: &[u8]) -> Result<()>;
    
    /// Commit transaction
    async fn commit(self) -> Result<CommitResult>;
    
    /// Rollback transaction
    async fn rollback(self) -> Result<()>;
}

pub struct TxOptions {
    pub isolation: IsolationLevel,
    pub timeout: Option<Duration>,
    pub read_only: bool,
}

pub enum IsolationLevel {
    SnapshotIsolation,
    ReadCommitted,
}

pub struct CommitResult {
    pub timestamp: Timestamp,
    pub affected_shards: Vec<ShardId>,
}
```

### 6. Admin API

```rust
pub trait AdminApi {
    /// Get cluster status
    async fn cluster_status(&self) -> Result<ClusterStatus>;
    
    /// List all nodes
    async fn list_nodes(&self) -> Result<Vec<NodeInfo>>;
    
    /// Add node to cluster
    async fn add_node(&self, config: NodeConfig) -> Result<NodeId>;
    
    /// Remove node from cluster
    async fn remove_node(&self, node: NodeId) -> Result<()>;
    
    /// List all shards
    async fn list_shards(&self) -> Result<Vec<ShardInfo>>;
    
    /// Rebalance shards
    async fn rebalance(&self, strategy: RebalanceStrategy) -> Result<RebalanceJob>;
    
    /// Create backup
    async fn create_backup(&self, spec: BackupSpec) -> Result<BackupId>;
    
    /// Restore from backup
    async fn restore_backup(&self, backup: BackupId, options: RestoreOptions) -> Result<()>;
    
    /// Get metrics
    async fn metrics(&self, filter: MetricsFilter) -> Result<Metrics>;
}

pub struct ClusterStatus {
    pub healthy: bool,
    pub node_count: usize,
    pub shard_count: usize,
    pub replication_lag: Duration,
}
```

### API Versioning

All APIs include version information:

```rust
pub const API_VERSION: &str = "1.0.0";

pub struct ApiVersion {
    pub major: u32,
    pub minor: u32,
    pub patch: u32,
}
```

Version compatibility rules:
- **Major version** changes indicate breaking changes
- **Minor version** changes add backward-compatible features
- **Patch version** changes are bug fixes only

### Error Handling

Standardized error types across all APIs:

```rust
pub enum Error {
    NotFound(String),
    AlreadyExists(String),
    InvalidArgument(String),
    PermissionDenied(String),
    ResourceExhausted(String),
    Unavailable(String),
    Internal(String),
    Timeout(Duration),
    Conflict(String),
    Aborted(String),
}

pub type Result<T> = std::result::Result<T, Error>;
```

## Consequences

### Positive

* **Clear contracts** - Well-defined interfaces for all operations
* **Type safety** - Compile-time guarantees for API usage
* **SDK generation** - Enables automatic client library generation
* **Backward compatibility** - Versioning allows controlled evolution
* **Testability** - Mock implementations for testing
* **Documentation** - Self-documenting through type signatures
* **Consistency** - Uniform patterns across all data models

### Negative

* **Initial overhead** - Requires upfront design effort
* **Rigidity** - Changes require careful versioning
* **Abstraction cost** - May hide some low-level optimizations
* **Learning curve** - Users must understand multiple API surfaces

### Risks

* **API bloat** - Risk of adding too many convenience methods
* **Breaking changes** - Difficult to change once stabilized
* **Performance overhead** - Abstraction layers may impact performance

## Alternatives Considered

### 1. Single Unified API

**Rejected** - Would create a monolithic interface mixing concerns and making it harder to use specific features.

### 2. SQL-like Query Language

**Deferred** - Can be layered on top of these APIs in the future. Starting with programmatic APIs provides more flexibility.

### 3. REST-only API

**Rejected** - Doesn't support efficient streaming or embedded mode. gRPC provides better performance and type safety.

### 4. Callback-based API

**Rejected** - Async/await provides better ergonomics and composability in modern Rust.

## Implementation Notes

### Phase 1: Core KV API (Week 8)
- Implement basic KV operations
- Add transaction support
- Create comprehensive test suite

### Phase 2: Document API (Weeks 15-17)
- Build on KV foundation
- Add JSON encoding/decoding
- Implement filtering and indexing

### Phase 3: Graph API (Weeks 18-20)
- Implement node/edge storage
- Add traversal algorithms
- Optimize for common patterns

### Phase 4: Vector API (Weeks 23-25)
- Integrate vector index
- Implement similarity search
- Add hybrid search capabilities

### Phase 5: Admin API (Weeks 37-42)
- Add cluster management
- Implement backup/restore
- Create monitoring endpoints

## Related ADRs

* [ADR-0006: Key-Value, Document, and Graph Support](ADR-0006-Key-Value-Document-Graph-Support.md)
* [ADR-0008: Indexing Options](ADR-0008-Indexing-Options.md)
* [ADR-0012: Transaction Model and Isolation Levels](ADR-0012-Transaction-Model-and-Isolation-Levels.md)
* [ADR-0015: Query Interface Strategy](ADR-0015-Query-Interface-Strategy.md)
* [ADR-0022: SDK Design and Language Bindings](ADR-0022-SDK-Design-and-Language-Bindings.md)

## References

* Rust async/await patterns
* gRPC service definitions
* Database API design best practices
* Semantic versioning specification

---

**Next Steps:**
1. Review and approve API specifications
2. Create protobuf definitions for gRPC
3. Implement core KV API
4. Generate SDK stubs
5. Write API documentation and examples