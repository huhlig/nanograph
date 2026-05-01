# nanograph-lmdb

LMDB (Lightning Memory-Mapped Database) storage engine for Nanograph, implementing the `KeyValueShardStore` trait.

## Overview

This crate provides a read-optimized storage backend using LMDB, a high-performance embedded database that uses memory-mapped files for fast data access. LMDB is ideal for read-heavy workloads where data fits in memory and fast point lookups are critical.

## Features

- **Single-file format**: Data stored in `data.mdb` + `lock.mdb` files per shard
- **Memory-mapped I/O**: Direct memory access without copying for fast reads
- **ACID transactions**: Full ACID compliance with MVCC (Multi-Version Concurrency Control)
- **Copy-on-write B+tree**: Efficient storage structure with minimal write amplification
- **Zero-copy reads**: Direct memory access without data copying
- **Read-optimized**: Excellent performance for read-heavy workloads (90%+ reads)
- **Embedded**: No separate server process required

## Architecture

LMDB uses a memory-mapped B+tree structure with copy-on-write semantics:

- Each shard gets its own LMDB environment (directory with `data.mdb` and `lock.mdb`)
- Reads are lock-free and use memory-mapped pages
- Writes use copy-on-write to maintain consistency
- Single-writer, multiple-reader model (SWMR)

## Use Cases

### Best Suited For

- **Read-heavy workloads**: 90%+ read operations
- **Small to medium datasets**: Data that fits in available memory (< 100GB)
- **Fast point lookups**: Direct key-value access with minimal latency
- **Embedded applications**: No separate database server needed
- **Single-writer scenarios**: One writer with many concurrent readers

### Not Recommended For

- **Write-heavy workloads**: Use LSM engine instead
- **Very large datasets**: > 100GB (memory mapping limitations)
- **High-concurrency writes**: Single-writer limitation
- **Distributed systems**: No built-in replication (use with Raft layer)

## Installation

Add to your `Cargo.toml`:

```toml
[dependencies]
nanograph-lmdb = "0.1.0"
```

## Usage

### Basic Operations

```rust
use nanograph_lmdb::LMDBKeyValueStore;
use nanograph_kvt::{KeyValueShardStore, ShardId};
use nanograph_vfs::{Path, MemoryFileSystem};
use std::sync::Arc;

#[tokio::main]
async fn main() {
    // Create LMDB store
    let store = LMDBKeyValueStore::new();

    // Create a shard
    let shard_id = ShardId::new(1);
    let vfs = Arc::new(MemoryFileSystem::new());
    let data_path = Path::from("/data/shard1");
    let wal_path = Path::from("/wal/shard1");
    
    store.create_shard(shard_id, vfs, data_path, wal_path).unwrap();

    // Put data
    store.put(shard_id, b"key1", b"value1").await.unwrap();

    // Get data
    let value = store.get(shard_id, b"key1").await.unwrap();
    assert_eq!(value, Some(b"value1".to_vec()));

    // Delete data
    store.delete(shard_id, b"key1").await.unwrap();
}
```

### Custom Configuration

```rust
use nanograph_lmdb::{LMDBKeyValueStore, LMDBConfig};

let config = LMDBConfig::default()
    .with_max_db_size(2 * 1024 * 1024 * 1024) // 2GB
    .with_max_dbs(256)                         // Support 256 shards
    .with_max_readers(126)                     // 126 concurrent readers
    .with_sync_on_commit(false);               // Faster but less safe

let store = LMDBKeyValueStore::with_config(config);
```

### Batch Operations

```rust
// Batch put
let pairs = vec![
    (&b"key1"[..], &b"value1"[..]),
    (&b"key2"[..], &b"value2"[..]),
    (&b"key3"[..], &b"value3"[..]),
];
store.batch_put(shard_id, &pairs).await.unwrap();

// Batch get
let keys = vec![&b"key1"[..], &b"key2"[..], &b"key3"[..]];
let values = store.batch_get(shard_id, &keys).await.unwrap();

// Batch delete
let delete_keys = vec![&b"key1"[..], &b"key2"[..]];
let count = store.batch_delete(shard_id, &delete_keys).await.unwrap();
```

### Range Scans

```rust
use nanograph_kvt::KeyRange;

// Scan with prefix
let range = KeyRange::prefix(b"product:".to_vec());
let mut iter = store.scan(shard_id, range).await.unwrap();

while let Some(result) = iter.next().await {
    let (key, value) = result.unwrap();
    println!("{:?} => {:?}", key, value);
}

// Scan with limit
let range = KeyRange::all().with_limit(100);
let mut iter = store.scan(shard_id, range).await.unwrap();
```

### Transactions

LMDB supports ACID transactions with snapshot isolation. The transaction wrapper manages multiple LMDB environments to provide cross-shard transaction support:

```rust
// Begin a transaction
let txn = store.begin_transaction().await.unwrap();

// Perform operations within the transaction
txn.put(shard_id, b"key1", b"updated_value").await.unwrap();
txn.put(shard_id, b"key2", b"new_value").await.unwrap();

// Transaction sees its own writes
let value = txn.get(shard_id, b"key1").await.unwrap();
assert_eq!(value, Some(b"updated_value".to_vec()));

// Store doesn't see uncommitted changes
let value = store.get(shard_id, b"key1").await.unwrap();
assert_eq!(value, Some(b"old_value".to_vec()));

// Commit the transaction
txn.commit().await.unwrap();

// Now store sees the committed changes
let value = store.get(shard_id, b"key1").await.unwrap();
assert_eq!(value, Some(b"updated_value".to_vec()));
```

**Transaction Implementation Notes:**

- Transactions buffer writes in memory until commit
- Cross-shard transactions are supported through the wrapper
- On commit, writes are applied atomically to each shard's LMDB environment
- Rollback simply discards the buffered writes
- Snapshot isolation: transactions see a consistent view of data at transaction start time

### Statistics

```rust
// Get key count
let count = store.key_count(shard_id).await.unwrap();

// Get detailed statistics
let stats = store.shard_stats(shard_id).await.unwrap();
println!("Keys: {}", stats.key_count);
println!("Total bytes: {}", stats.total_bytes);
println!("Page size: {:?}", stats.engine_stats.get("page_size"));
```

## Configuration Options

### LMDBConfig

- `max_db_size`: Maximum database size in bytes (default: 1GB)
- `max_dbs`: Maximum number of databases/shards (default: 128)
- `max_readers`: Maximum concurrent readers (default: 126)
- `use_writemap`: Use writable memory map (default: false)
- `sync_on_commit`: Sync to disk on commit (default: true)
- `read_only`: Open in read-only mode (default: false)
- `create_if_missing`: Create database if it doesn't exist (default: true)

## Performance Characteristics

### Read Performance

- **Point lookups**: O(log n) with memory-mapped access
- **Range scans**: O(log n + k) where k is result size
- **Batch reads**: Amortized cost across multiple keys

### Write Performance

- **Single writes**: O(log n) with copy-on-write overhead
- **Batch writes**: Amortized cost in single transaction
- **Write amplification**: Minimal due to copy-on-write

### Memory Usage

- **Memory-mapped**: Database size limited by address space
- **Page cache**: OS manages page cache automatically
- **Overhead**: Minimal per-key overhead in B+tree

## Comparison with LSM

| Feature | LMDB | LSM |
|---------|------|-----|
| Read Performance | Excellent | Good |
| Write Performance | Good | Excellent |
| Memory Usage | High (memory-mapped) | Medium |
| Disk Usage | Efficient | Higher (write amplification) |
| Compaction | Not needed | Required |
| Best For | Read-heavy | Write-heavy |

## Testing

Run tests:

```bash
cargo test
```

Run benchmarks:

```bash
cargo bench
```

## License

Licensed under the Apache License, Version 2.0. See LICENSE file for details.

## Contributing

Contributions are welcome! Please see CONTRIBUTING.md for guidelines.

## References

- [LMDB Documentation](http://www.lmdb.tech/doc/)
- [LMDB Paper](https://www.symas.com/lmdb)
- [lmdb-rkv Crate](https://docs.rs/lmdb-rkv/)