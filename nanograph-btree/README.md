# nanograph-btree

B+Tree storage engine implementation for Nanograph.

## Overview

This crate provides an in-memory B+Tree implementation that conforms to the `KeyValueStore` trait. B+Trees are well-suited for range queries and maintain sorted order of keys, making them ideal for workloads that require efficient sequential access.

## Features

- **B+Tree Structure**: All data stored in leaf nodes, internal nodes only contain routing keys
- **Efficient Range Scans**: Linked leaf nodes enable fast forward and reverse iteration
- **MVCC Support**: Snapshot isolation for concurrent transactions
- **Write-Ahead Logging (WAL)**: Active WAL writes for durability and crash recovery
  - Automatic WAL creation on shard initialization
  - WAL recovery on startup for crash consistency
  - Checkpointing for optimized recovery performance
- **Comprehensive Metrics**: Track reads, writes, splits, and other operations
- **Configurable**: Adjustable node size and tree parameters
- **In-Memory**: Fast access with no disk I/O overhead

## Architecture

### B+Tree Properties

Unlike traditional B-trees, B+Trees have the following characteristics:

1. **Data in Leaves Only**: All key-value pairs are stored in leaf nodes
2. **Internal Nodes for Routing**: Internal nodes only contain keys and child pointers
3. **Linked Leaves**: Leaf nodes are linked together for efficient range scans
4. **Balanced**: All leaf nodes are at the same depth

### Components

- **Node**: Internal and leaf node structures with split/merge operations
- **Tree**: Main B+Tree with insert, search, delete, and rebalancing
- **Iterator**: Efficient forward and reverse range scans using leaf links
- **Metrics**: Performance tracking for all operations
- **Transactions**: MVCC-based transaction support with write buffering

## Usage

### Basic Operations

```rust
use nanograph_btree::{BTreeKeyValueStore, tree::BPlusTreeConfig};
use nanograph_kvt::KeyValueStore;

#[tokio::main]
async fn main() {
    // Create store with default configuration
    let store = BTreeKeyValueStore::default();
    
    // Or with custom configuration
    let config = BPlusTreeConfig {
        max_keys: 256,  // Maximum keys per node
        min_keys: 128,  // Minimum keys per node
    };
    let store = BTreeKeyValueStore::new(config);
    
    // Create a table
    let table = store.create_table("my_table").await.unwrap();
    
    // Insert data
    store.put(table, b"key1", b"value1").await.unwrap();
    store.put(table, b"key2", b"value2").await.unwrap();
    
    // Retrieve data
    let value = store.get(table, b"key1").await.unwrap();
    assert_eq!(value, Some(b"value1".to_vec()));
    
    // Delete data
    store.delete(table, b"key1").await.unwrap();
}
```

### Range Scans

```rust
use nanograph_kvt::{KeyRange, KeyValueIterator};
use std::ops::Bound;

// Scan all keys
let range = KeyRange {
    start: Bound::Unbounded,
    end: Bound::Unbounded,
    limit: None,
    reverse: false,
};
let mut iter = store.scan(table, range).await.unwrap();

while let Some((key, value)) = iter.next().unwrap() {
    println!("{:?} => {:?}", key, value);
}

// Scan with bounds
let range = KeyRange {
    start: Bound::Included(b"key1".to_vec()),
    end: Bound::Excluded(b"key5".to_vec()),
    limit: Some(10),
    reverse: false,
};
let mut iter = store.scan(table, range).await.unwrap();
```

### Batch Operations

```rust
// Batch put
let pairs = vec![
    (&b"key1"[..], &b"value1"[..]),
    (&b"key2"[..], &b"value2"[..]),
    (&b"key3"[..], &b"value3"[..]),
];
store.batch_put(table, &pairs).await.unwrap();

// Batch get
let keys = vec![&b"key1"[..], &b"key2"[..], &b"key3"[..]];
let results = store.batch_get(table, &keys).await.unwrap();

// Batch delete
let delete_keys = vec![&b"key1"[..], &b"key2"[..]];
let deleted_count = store.batch_delete(table, &delete_keys).await.unwrap();
```

### Transactions

```rust
// Begin a transaction
let tx = store.begin_transaction().await.unwrap();

// Perform operations within transaction
// (Note: Full transactional semantics are work in progress)

// Commit the transaction
tx.commit().await.unwrap();

// Or rollback
// tx.rollback().await.unwrap();
```

### Statistics

```rust
// Get key count
let count = store.key_count(table).await.unwrap();
println!("Total keys: {}", count);

// Get detailed statistics
let stats = store.table_stats(table).await.unwrap();
println!("Keys: {}", stats.key_count);
println!("Total bytes: {}", stats.total_bytes);

if let nanograph_kvt::EngineStats::BTree(btree_stats) = stats.engine_stats {
    println!("Tree height: {}", btree_stats.height);
    println!("Internal nodes: {}", btree_stats.num_internal_nodes);
    println!("Leaf nodes: {}", btree_stats.num_leaf_nodes);
    println!("Fill factor: {:.2}%", btree_stats.fill_factor * 100.0);
}
```

## Configuration

### Tree Parameters

- **max_keys**: Maximum number of keys per node (default: 128)
  - Higher values reduce tree height but increase node size
  - Typical range: 64-512

- **min_keys**: Minimum number of keys per node (default: max_keys / 2)
  - Used for rebalancing operations
  - Should be at least max_keys / 2

### Choosing Parameters

- **Small keys/values**: Use larger max_keys (256-512) for better space efficiency
- **Large keys/values**: Use smaller max_keys (64-128) to avoid large nodes
- **Range-heavy workload**: Optimize for sequential access with moderate max_keys
- **Point-query workload**: Use larger max_keys to reduce tree height

## Performance Characteristics

### Time Complexity

- **Search**: O(log n) where n is the number of keys
- **Insert**: O(log n) amortized (may trigger splits)
- **Delete**: O(log n) amortized (may trigger merges)
- **Range Scan**: O(log n + k) where k is the number of results
- **Sequential Scan**: O(k) after initial seek

### Space Complexity

- **Memory**: O(n) where n is the number of keys
- **Node overhead**: Proportional to tree height (typically log n)

### Advantages

- Excellent range query performance
- Predictable performance (balanced tree)
- Good cache locality for sequential access
- Simple and well-understood algorithm

### Limitations

- In-memory only (no persistence in current implementation)
- No compression (stores full keys and values)
- Write amplification during splits
- Not optimized for very large datasets (consider LSM for that)

## Comparison with LSM

| Feature | B+Tree | LSM Tree |
|---------|--------|----------|
| Range Scans | Excellent | Good |
| Point Queries | Good | Good |
| Write Performance | Good | Excellent |
| Space Amplification | Low | Medium |
| Write Amplification | Medium | High |
| Complexity | Low | High |
| Best For | Range queries, sorted data | Write-heavy workloads |

## Future Enhancements

- [ ] Persistent storage with page management
- [ ] Concurrent access with fine-grained locking
- [ ] Bulk loading optimization
- [ ] Key compression for internal nodes
- [ ] Adaptive node sizing based on workload
- [ ] Full MVCC transaction support
- [ ] Node caching and eviction policies

## Testing

Run the test suite:

```bash
cargo test -p nanograph-btree
```

Run with output:

```bash
cargo test -p nanograph-btree -- --nocapture
```

## License

Licensed under the Apache License, Version 2.0. See LICENSE for details.

## Contributing

Contributions are welcome! Please see CONTRIBUTING.md for guidelines.
# Nanograph B+Tree

High-performance B+Tree implementation with MVCC support and automatic rebalancing.

## Features

- **Automatic Rebalancing**: Maintains tree balance after deletions
- **Node Borrowing**: Redistributes entries from siblings when possible
- **Node Merging**: Combines underflowing nodes with siblings
- **MVCC Support**: Multi-version concurrency control with timestamps
- **Write-Ahead Logging**: Active WAL with recovery and checkpointing
- **Persistence**: Optional disk-based storage
- **Range Scans**: Efficient iteration via leaf node links

## Rebalancing

The B+Tree automatically maintains balance after deletions:

### Underflow Handling
When a node has fewer than `min_keys` entries after deletion:

1. **Try Borrowing from Left Sibling**: If left sibling has > min_keys
2. **Try Borrowing from Right Sibling**: If right sibling has > min_keys  
3. **Merge with Left Sibling**: Combine nodes if borrowing not possible
4. **Merge with Right Sibling**: Alternative merge direction
5. **Propagate to Parent**: Recursively handle parent underflow

### Borrowing Operations
- Moves one entry from a sibling to the underflowing node
- Updates parent separator keys
- Maintains sort order

### Merging Operations
- Combines two nodes into one
- Removes separator key from parent
- Updates sibling links in leaf nodes
- May trigger parent underflow (recursive)

## Usage

```rust
use nanograph_btree::{BPlusTree, BPlusTreeConfig};

// Create tree with custom configuration
let config = BPlusTreeConfig {
    max_keys: 128,  // Maximum keys per node
    min_keys: 64,   // Minimum keys per node (triggers rebalancing)
};
let tree = BPlusTree::new(config);

// Insert data
tree.insert(b"key1".to_vec(), b"value1".to_vec())?;
tree.insert(b"key2".to_vec(), b"value2".to_vec())?;

// Read data
let value = tree.get(b"key1")?;

// Delete data (triggers automatic rebalancing if needed)
let deleted = tree.delete(b"key1")?;

// Range scan
let start_key = b"key1";
let end_key = b"key9";
// Use iterator for range scans
```

## Testing

Comprehensive tests for rebalancing:

```bash
# Run all tests
cargo test

# Run rebalancing-specific tests
cargo test --test rebalancing_tests

# Test with different configurations
cargo test test_delete_with_borrowing
cargo test test_delete_with_merging
```

## Performance

- Insert: O(log n)
- Search: O(log n)
- Delete: O(log n) with rebalancing
- Range Scan: O(log n + k) where k is result size

## License

Apache License 2.0
