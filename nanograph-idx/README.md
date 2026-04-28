# Nanograph Index Implementations

This crate provides various index implementations for the Nanograph database system, enabling efficient data retrieval through secondary indexes, unique constraints, full-text search, and spatial queries.

## Overview

The `nanograph-idx` crate implements different types of indexes that can be used with Nanograph tables:

- **Secondary Indexes (B-Tree)**: For range queries and sorted scans
- **Unique Indexes (Hash)**: For fast lookups with uniqueness constraints
- **Full-Text Indexes (Inverted Index)**: For text search and keyword matching
- **Spatial Indexes (R-Tree)**: For geographic and geometric queries

## Architecture

### Index Store Trait

All index implementations conform to the `IndexStore` trait, which provides a unified interface for:

- Building indexes from table data
- Querying indexes
- Maintaining indexes during table updates
- Managing index storage and lifecycle

### Index Storage Model

Each index is stored as a separate shard with:
- **IndexId** as the shard identifier
- **Index entries** as key-value pairs where:
  - Key: indexed value(s) + primary key
  - Value: reference to table row or included columns

## Index Types

### 1. Secondary Index (B-Tree)

B-Tree indexes are ideal for:
- Range queries (`WHERE age BETWEEN 18 AND 65`)
- Sorted scans (`ORDER BY created_at`)
- Prefix matching (`WHERE name LIKE 'John%'`)

**Example:**
```rust
use nanograph_idx::BTreeIndex;
use nanograph_core::object::{IndexCreate, IndexType};

let config = IndexCreate::new(
    "users_age_idx",
    IndexType::Secondary,
    vec!["age".to_string()],
);

let index = BTreeIndex::new(config)?;
```

### 2. Unique Index (Hash)

Hash indexes provide:
- Fast O(1) point lookups
- Uniqueness constraint enforcement
- Efficient equality checks

**Example:**
```rust
use nanograph_idx::HashIndex;
use nanograph_core::object::{IndexCreate, IndexType};

let config = IndexCreate::new(
    "users_email_idx",
    IndexType::Unique,
    vec!["email".to_string()],
);

let index = HashIndex::new(config)?;
```

### 3. Full-Text Index (Inverted Index)

Full-text indexes enable:
- Text search across documents
- Keyword matching with relevance scoring
- Phrase queries and fuzzy matching

**Example:**
```rust
use nanograph_idx::FullTextIndex;
use nanograph_core::object::{IndexCreate, IndexType};

let config = IndexCreate::new(
    "documents_content_idx",
    IndexType::FullText,
    vec!["content".to_string()],
);

let index = FullTextIndex::new(config)?;
```

### 4. Spatial Index (R-Tree)

Spatial indexes support:
- Geographic queries (point-in-polygon, bounding box)
- Nearest neighbor search
- Distance calculations

**Example:**
```rust
use nanograph_idx::SpatialIndex;
use nanograph_core::object::{IndexCreate, IndexType};

let config = IndexCreate::new(
    "locations_coords_idx",
    IndexType::Spatial,
    vec!["latitude".to_string(), "longitude".to_string()],
);

let index = SpatialIndex::new(config)?;
```

## Usage

### Creating an Index

```rust
use nanograph_kvm::KeyValueDatabaseManager;
use nanograph_core::object::{IndexCreate, IndexType};

// Get a table handle
let table = manager.get_table_handle(&principal, &container_id, &table_id).await?;

// Create an index
let config = IndexCreate::new(
    "users_email_idx",
    IndexType::Unique,
    vec!["email".to_string()],
);

let index = table.create_index(config).await?;
println!("Created index: {}", index.name);
```

### Querying with an Index

```rust
// Query using the index (future implementation)
let results = table.scan_by_index(&index_id, range).await?;
```

### Maintaining Indexes

Indexes are automatically maintained during table operations:
- **INSERT**: New entries are added to all indexes
- **UPDATE**: Index entries are updated if indexed columns change
- **DELETE**: Corresponding index entries are removed

## Performance Considerations

### Index Selection

Choose the right index type for your use case:

| Use Case | Recommended Index | Reason |
|----------|------------------|---------|
| Equality lookups | Hash (Unique) | O(1) lookup time |
| Range queries | B-Tree (Secondary) | Efficient range scans |
| Text search | Inverted (FullText) | Optimized for text matching |
| Geographic queries | R-Tree (Spatial) | Spatial data structures |

### Index Build Performance

- Index builds are performed asynchronously
- Large tables may take time to index initially
- Use `IndexStatus` to track build progress

### Query Performance

- Indexes can significantly speed up queries (10-1000x)
- Multiple indexes can be used together (index intersection)
- Query planner automatically selects optimal indexes

## Implementation Status

### Completed ✓
- Index metadata structures and types
- Index lifecycle management (create, update, delete)
- Index cache integration
- Permission-based access control

### In Progress 🚧
- B-Tree index implementation
- Hash index implementation
- Index building process
- Index query operations

### Planned 📋
- Full-text index implementation
- Spatial index implementation
- Index optimization and maintenance
- Advanced query features

## Testing

Run the test suite:
```bash
cargo nextest run -p nanograph-idx
```

Run benchmarks:
```bash
cargo bench -p nanograph-idx
```

## Contributing

When implementing new index types:

1. Implement the `IndexStore` trait
2. Add appropriate tests
3. Include benchmarks
4. Update documentation

## License

Licensed under the Apache License, Version 2.0.