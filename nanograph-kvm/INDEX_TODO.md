# Index Implementation TODO

This document tracks the remaining work needed to complete the index functionality in nanograph-kvm.

## Completed ✓

- [x] Index metadata structures (IndexRecord, IndexCreate, IndexUpdate, IndexType, IndexStatus)
- [x] Index ID generation and management (IndexId, IndexNumber)
- [x] Index cache support in ContainerMetadataCache
- [x] Index key generation in ContainerKeys
- [x] Index CRUD operations in KeyValueDatabaseContext
- [x] Index API methods in KeyValueDatabaseManager
- [x] Index operations in TableHandle
- [x] Permission-based access control for index operations
- [x] Version tracking and timestamps for indexes

## High Priority

### 1. Index Storage Implementation
**Location**: New `nanograph-idx` crate

- [ ] Create `nanograph-idx` crate structure
- [ ] Define `IndexStore` trait for different index types
- [ ] Implement B-Tree index for secondary indexes
- [ ] Implement hash index for unique constraints
- [ ] Implement inverted index for full-text search
- [ ] Implement R-Tree index for spatial queries
- [ ] Add index shard creation and management
- [ ] Integrate index stores with KeyValueShardManager

### 2. Index Building Process
**Location**: `nanograph-kvm/src/context.rs`, `nanograph-idx`

- [ ] Implement async index builder
- [ ] Add background index building task
- [ ] Support incremental index building
- [ ] Add progress tracking for index builds
- [ ] Handle index build failures and retries
- [ ] Update IndexStatus during build lifecycle
- [ ] Add index validation after build

### 3. Index Query Operations
**Location**: `nanograph-kvm/src/context.rs`, `nanograph-kvm/src/handle/table.rs`

- [ ] Add `scan_by_index()` method to TableHandle
- [ ] Implement index-based range queries
- [ ] Add index-based point lookups
- [ ] Support composite index queries
- [ ] Implement index intersection for multi-index queries
- [ ] Add query optimization based on available indexes

### 4. Index Maintenance
**Location**: `nanograph-idx`, `nanograph-kvm/src/context.rs`

- [ ] Implement index rebuild operation
- [ ] Add index optimization/compaction
- [ ] Support online index rebuilds
- [ ] Add index statistics collection
- [ ] Implement index health checks
- [ ] Add automatic index repair

## Medium Priority

### 5. Index Synchronization
**Location**: `nanograph-kvm/src/context.rs`

- [ ] Keep indexes in sync with table writes
- [ ] Handle index updates in `put()` operation
- [ ] Handle index updates in `delete()` operation
- [ ] Handle index updates in `batch_put()` operation
- [ ] Add transactional index updates
- [ ] Implement write-ahead logging for index changes

### 6. Unique Index Constraints
**Location**: `nanograph-idx`

- [ ] Implement uniqueness checking
- [ ] Add constraint violation errors
- [ ] Support unique index on multiple columns
- [ ] Handle concurrent unique constraint checks
- [ ] Add deferred constraint checking

### 7. Full-Text Search
**Location**: `nanograph-idx`

- [ ] Implement tokenization
- [ ] Add stemming and lemmatization
- [ ] Support stop words
- [ ] Implement relevance scoring
- [ ] Add phrase queries
- [ ] Support fuzzy matching
- [ ] Add highlighting

### 8. Spatial Indexes
**Location**: `nanograph-idx`

- [ ] Implement R-Tree structure
- [ ] Add bounding box queries
- [ ] Support point-in-polygon queries
- [ ] Implement nearest neighbor search
- [ ] Add distance calculations
- [ ] Support different coordinate systems

## Low Priority

### 9. Advanced Features

- [ ] Partial indexes (filtered indexes)
- [ ] Expression indexes (computed columns)
- [ ] Covering indexes (include columns)
- [ ] Index-only scans
- [ ] Parallel index builds
- [ ] Index compression
- [ ] Bloom filters for index optimization

### 10. Monitoring and Diagnostics

- [ ] Index usage statistics
- [ ] Index size tracking
- [ ] Query performance metrics
- [ ] Index fragmentation analysis
- [ ] Index recommendation system
- [ ] Index impact analysis

### 11. Testing

- [ ] Unit tests for index creation/deletion
- [ ] Integration tests for index queries
- [ ] Performance benchmarks for different index types
- [ ] Stress tests for concurrent index operations
- [ ] Tests for index recovery after failures
- [ ] Tests for index consistency

### 12. Documentation

- [ ] API documentation for index operations
- [ ] Usage examples for each index type
- [ ] Performance tuning guide
- [ ] Index design best practices
- [ ] Migration guide for adding indexes to existing tables
- [ ] Troubleshooting guide

## Architecture Notes

### Index Storage Model

Each index will be stored as a separate shard with:
- **IndexId** as the shard identifier
- **Index entries** as key-value pairs where:
  - Key: indexed value(s) + primary key
  - Value: reference to table row or included columns

### Index Types and Use Cases

1. **Secondary Index (B-Tree)**
   - Range queries
   - Sorted scans
   - Prefix matching

2. **Unique Index (Hash)**
   - Fast point lookups
   - Uniqueness constraints
   - Primary key alternatives

3. **Full-Text Index (Inverted Index)**
   - Text search
   - Keyword matching
   - Relevance ranking

4. **Spatial Index (R-Tree)**
   - Geographic queries
   - Geometric operations
   - Proximity search

### Integration Points

1. **KeyValueShardManager**: Manage index shards alongside table shards
2. **KeyValueDatabaseContext**: Coordinate index operations with table operations
3. **TableHandle**: Provide user-facing index query API
4. **ConsensusManager**: Replicate index operations in distributed mode

## Dependencies

- `nanograph-kvt`: Core key-value trait definitions
- `nanograph-btree`: B-Tree implementation for secondary indexes
- `nanograph-lsm`: LSM tree for write-heavy indexes
- `nanograph-vfs`: Virtual file system for index storage
- `nanograph-util`: Utility functions and data structures

## Performance Considerations

- Index builds should not block table operations
- Index queries should be faster than full table scans
- Index updates should have minimal overhead on writes
- Index storage should be space-efficient
- Index operations should scale with data size

## Security Considerations

- Index operations respect table permissions
- Index data is encrypted at rest (if table is encrypted)
- Index queries are subject to rate limiting
- Index metadata is protected from unauthorized access