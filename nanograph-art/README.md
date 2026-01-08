# Nanograph ART - Adaptive Radix Tree Storage Engine

A high-performance, production-ready Adaptive Radix Tree (ART) implementation for the Nanograph database system.

## Features

### Core Data Structure
- **Adaptive Node Sizing**: Automatically adjusts node types (Node4, Node16, Node48, Node256) based on the number of children
- **Path Compression**: Reduces tree height by compressing paths with single children
- **Prefix Keys**: Supports storing values in inner nodes for efficient prefix matching
- **O(k) Operations**: Insert, search, and delete operations run in O(k) time where k is the key length

### Storage Engine Capabilities
- **KeyValueShardStore Implementation**: Full integration with nanograph-kvt
- **Persistence**: VFS-based disk storage with JSON serialization
- **Write-Ahead Logging (WAL)**: Active WAL writes for durability and crash recovery
  - Automatic WAL creation on shard initialization
  - WAL recovery on startup for crash consistency
  - Checkpointing for optimized recovery performance
- **ACID Transactions**: Full transaction support with snapshot isolation
- **Shard Management**: Create, drop, and manage multiple shards
- **Metrics**: Comprehensive operation tracking and statistics

### Thread Safety
- **Arc-based Sharing**: Safe concurrent access across threads
- **RwLock Protection**: Read-write locks for optimal concurrency
- **Atomic Metrics**: Lock-free metric counters

## Installation

Add to your `Cargo.toml`:

```toml
[dependencies]
nanograph-art = "0.1.0"
```

## Quick Start

### Basic Usage

```rust
use nanograph_art::AdaptiveRadixTree;

// Create a new tree
let mut tree = AdaptiveRadixTree::new();

// Insert key-value pairs
tree.insert(b"hello".to_vec(), "world".to_string()).unwrap();
tree.insert(b"hi".to_vec(), "there".to_string()).unwrap();

// Retrieve values
assert_eq!(tree.get(b"hello"), Some("world".to_string()));

// Delete keys
tree.remove(b"hello").unwrap();

// Iterate over entries
for (key, value) in tree.iter() {
    println!("{:?} => {}", key, value);
}
```

### As a KeyValueStore

```rust
use nanograph_art::ArtKeyValueStore;
use nanograph_kvt::{KeyValueShardStore, TableId, ShardIndex};
use std::sync::Arc;

#[tokio::main]
async fn main() {
    // Create store
    let store = Arc::new(ArtKeyValueStore::new());
    
    // Initialize transaction manager
    store.init_tx_manager();
    
    // Create a shard
    let table_id = TableId::new(0);
    let shard_index = ShardIndex::new(0);
    let shard = store.create_shard(table_id, shard_index).await.unwrap();
    
    // Basic operations
    store.put(shard, b"key1", b"value1").await.unwrap();
    let value = store.get(shard, b"key1").await.unwrap();
    assert_eq!(value, Some(b"value1".to_vec()));
    
    // Batch operations
    let pairs = vec![
        (&b"key2"[..], &b"value2"[..]),
        (&b"key3"[..], &b"value3"[..]),
    ];
    store.batch_put(shard, &pairs).await.unwrap();
}
```

### Transactions

```rust
use nanograph_art::ArtKeyValueStore;
use nanograph_kvt::{KeyValueShardStore, Transaction};
use std::sync::Arc;

#[tokio::main]
async fn main() {
    let store = Arc::new(ArtKeyValueStore::new());
    store.init_tx_manager();
    
    let shard = store.create_shard(
        nanograph_kvt::TableId::new(0),
        nanograph_kvt::ShardIndex::new(0)
    ).await.unwrap();
    
    // Begin transaction
    let tx = store.begin_transaction().await.unwrap();
    
    // Perform operations within transaction
    tx.put(shard, b"key1", b"value1").await.unwrap();
    tx.put(shard, b"key2", b"value2").await.unwrap();
    
    // Read within transaction (sees uncommitted writes)
    let value = tx.get(shard, b"key1").await.unwrap();
    assert_eq!(value, Some(b"value1".to_vec()));
    
    // Commit transaction
    tx.commit().await.unwrap();
    
    // Or rollback to discard changes
    // tx.rollback().await.unwrap();
}
```

### Persistence

```rust
use nanograph_art::{AdaptiveRadixTree, ArtPersistence};
use nanograph_vfs::MemoryFileSystem;
use std::sync::Arc;

// Create persistence manager
let fs = Arc::new(MemoryFileSystem::new());
let persistence = ArtPersistence::new(fs, "/data".to_string()).unwrap();

// Create and populate tree
let mut tree = AdaptiveRadixTree::new();
tree.insert(b"key1".to_vec(), "value1".to_string()).unwrap();
tree.insert(b"key2".to_vec(), "value2".to_string()).unwrap();

// Save to disk
persistence.save_tree(&tree).unwrap();

// Load from disk
let loaded_tree: AdaptiveRadixTree<String> = persistence.load_tree().unwrap();
assert_eq!(loaded_tree.get(b"key1"), Some("value1".to_string()));
```

## Architecture

### Node Types

The ART uses four node types that adapt based on the number of children:

- **Node4**: 1-4 children, uses linear search
- **Node16**: 5-16 children, uses linear search with SIMD optimization potential
- **Node48**: 17-48 children, uses index array for O(1) lookup
- **Node256**: 49-256 children, direct array indexing

### Path Compression

Nodes store a partial key (prefix) to compress paths with single children, reducing tree height and improving cache locality.

### Transaction Isolation

Transactions provide snapshot isolation:
- Each transaction sees a consistent snapshot of data
- Writes are buffered until commit
- Reads within a transaction see uncommitted writes
- Conflicts are detected on commit

## Performance Characteristics

- **Insert**: O(k) where k is key length
- **Search**: O(k) where k is key length
- **Delete**: O(k) where k is key length
- **Range Scan**: O(n + k) where n is result size
- **Memory**: Adaptive - grows/shrinks based on data

## Comparison with Other Structures

| Feature | ART | B+Tree | LSM Tree |
|---------|-----|--------|----------|
| Point Queries | O(k) | O(log n) | O(log n) |
| Range Scans | Excellent | Excellent | Good |
| Memory Efficiency | Adaptive | Fixed | Variable |
| Write Amplification | Low | Medium | High |
| Prefix Matching | Native | Emulated | Emulated |

## Testing

Run the test suite:

```bash
cargo test
```

Run with output:

```bash
cargo test -- --nocapture
```

Run specific test:

```bash
cargo test test_transaction_isolation
```

## Benchmarks

```bash
cargo bench
```

## Contributing

Contributions are welcome! Please see [CONTRIBUTING.md](../CONTRIBUTING.md) for guidelines.

## License

Licensed under the Apache License, Version 2.0. See [LICENSE.md](../LICENSE.md) for details.

## References

- [The Adaptive Radix Tree: ARTful Indexing for Main-Memory Databases](https://db.in.tum.de/~leis/papers/ART.pdf)
- [Nanograph Architecture Documentation](../docs/ARCHITECTURE_APPENDICES.md)