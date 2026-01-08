# Nanograph Key-Value Traits (nanograph-kvt)

This crate defines the core traits and types for key-value storage in Nanograph. It provides a unified interface that can be implemented by different storage engines (LSM, B+Tree, ART, etc.).

## Overview

The `nanograph-kvt` crate is the foundation of Nanograph's storage layer. It defines:

- **`KeyValueStore` trait**: The main interface for key-value operations
- **`Transaction` trait**: ACID transaction support with snapshot isolation
- **`KvIterator` trait**: Streaming iteration over key-value pairs
- **Supporting types**: Error types, table IDs, ranges, and statistics

## Features

- ✅ **Async-first API**: All operations are async for non-blocking I/O
- ✅ **MVCC Support**: Built-in support for multi-version concurrency control
- ✅ **Transaction Support**: ACID transactions with snapshot isolation
- ✅ **Flexible Iteration**: Range scans with seeking and filtering
- ✅ **Batch Operations**: Efficient batch get/put/delete operations
- ✅ **Table Management**: Multi-table support with metadata
- ✅ **Comprehensive Error Handling**: Detailed error types for all failure modes

## Architecture

```
┌─────────────────────────────────────────────────────────┐
│                   Application Layer                      │
└─────────────────────────────────────────────────────────┘
                          │
                          ▼
┌─────────────────────────────────────────────────────────┐
│              KeyValueStore Trait (this crate)            │
│  • get/put/delete/exists                                │
│  • batch_get/batch_put/batch_delete                     │
│  • scan/scan_prefix                                     │
│  • begin_transaction                                    │
│  • create_table/drop_table                              │
└─────────────────────────────────────────────────────────┘
                          │
          ┌───────────────┼───────────────┐
          ▼               ▼               ▼
    ┌──────────┐    ┌──────────┐    ┌──────────┐
    │   LSM    │    │  B+Tree  │    │   ART    │
    │  Engine  │    │  Engine  │    │  Engine  │
    └──────────┘    └──────────┘    └──────────┘
```

## Core Types

### KeyValueStore Trait

The main interface for storage operations:

```rust
#[async_trait]
pub trait KeyValueStore: Send + Sync {
    // Basic operations
    async fn get(&self, table: KeyValueTableId, key: &[u8]) -> KeyValueResult<Option<Vec<u8>>>;
    async fn put(&self, table: KeyValueTableId, key: &[u8], value: &[u8]) -> KeyValueResult<()>;
    async fn delete(&self, table: KeyValueTableId, key: &[u8]) -> KeyValueResult<bool>;
    async fn exists(&self, table: KeyValueTableId, key: &[u8]) -> KeyValueResult<bool>;
    
    // Batch operations
    async fn batch_get(&self, table: KeyValueTableId, keys: &[&[u8]]) -> KeyValueResult<Vec<Option<Vec<u8>>>>;
    async fn batch_put(&self, table: KeyValueTableId, pairs: &[(&[u8], &[u8])]) -> KeyValueResult<()>;
    async fn batch_delete(&self, table: KeyValueTableId, keys: &[&[u8]]) -> KeyValueResult<usize>;
    
    // Range operations
    async fn scan(&self, table: KeyValueTableId, range: KeyRange) -> KeyValueResult<Box<dyn KvIterator + Send>>;
    async fn scan_prefix(&self, table: KeyValueTableId, prefix: &[u8], limit: Option<usize>) -> KeyValueResult<Box<dyn KvIterator + Send>>;
    
    // Statistics
    async fn key_count(&self, table: KeyValueTableId) -> KeyValueResult<u64>;
    async fn table_stats(&self, table: KeyValueTableId) -> KeyValueResult<TableStats>;
    
    // Transactions
    async fn begin_transaction(&self) -> KeyValueResult<Arc<dyn Transaction>>;
    
    // Table management
    async fn create_table(&self, name: &str) -> KeyValueResult<KeyValueTableId>;
    async fn drop_table(&self, table: KeyValueTableId) -> KeyValueResult<()>;
    async fn list_tables(&self) -> KeyValueResult<Vec<(KeyValueTableId, String)>>;
    async fn table_exists(&self, table: KeyValueTableId) -> KeyValueResult<bool>;
    
    // Maintenance
    async fn flush(&self) -> KeyValueResult<()>;
    async fn compact(&self, table: Option<KeyValueTableId>) -> KeyValueResult<()>;
}
```

### Transaction Trait

ACID transactions with snapshot isolation:

```rust
#[async_trait]
pub trait Transaction: Send + Sync {
    fn id(&self) -> TransactionId;
    fn snapshot_ts(&self) -> Timestamp;
    
    async fn get(&self, table: KeyValueTableId, key: &[u8]) -> KeyValueResult<Option<Vec<u8>>>;
    async fn put(&self, table: KeyValueTableId, key: &[u8], value: &[u8]) -> KeyValueResult<()>;
    async fn delete(&self, table: KeyValueTableId, key: &[u8]) -> KeyValueResult<bool>;
    async fn scan(&self, table: KeyValueTableId, range: KeyRange) -> KeyValueResult<Box<dyn KvIterator + Send>>;
    
    async fn commit(self: Arc<Self>) -> KeyValueResult<()>;
    async fn rollback(self: Arc<Self>) -> KeyValueResult<()>;
}
```

### KvIterator Trait

Streaming iteration with seeking:

```rust
pub trait KvIterator: Stream<Item = KeyValueResult<(Vec<u8>, Vec<u8>)>> {
    fn seek(&mut self, key: &[u8]) -> KeyValueResult<()>;
    fn position(&self) -> Option<Vec<u8>>;
    fn valid(&self) -> bool;
}
```

## Usage Examples

### Basic Operations

```rust
use nanograph_kvt::{KeyValueStore, KeyValueTableId};

async fn basic_example(store: impl KeyValueStore) -> Result<(), Box<dyn std::error::Error>> {
    // Create a table
    let table = store.create_table("users").await?;
    
    // Put a value
    store.put(table, b"user:1", b"Alice").await?;
    
    // Get a value
    if let Some(value) = store.get(table, b"user:1").await? {
        println!("User: {}", String::from_utf8_lossy(&value));
    }
    
    // Check existence
    if store.exists(table, b"user:1").await? {
        println!("User exists");
    }
    
    // Delete
    store.delete(table, b"user:1").await?;
    
    Ok(())
}
```

### Batch Operations

```rust
async fn batch_example(store: impl KeyValueStore) -> Result<(), Box<dyn std::error::Error>> {
    let table = store.create_table("products").await?;
    
    // Batch put
    let pairs = vec![
        (&b"product:1"[..], &b"Laptop"[..]),
        (&b"product:2"[..], &b"Mouse"[..]),
        (&b"product:3"[..], &b"Keyboard"[..]),
    ];
    store.batch_put(table, &pairs).await?;
    
    // Batch get
    let keys = vec![&b"product:1"[..], &b"product:2"[..], &b"product:3"[..]];
    let values = store.batch_get(table, &keys).await?;
    
    for (key, value) in keys.iter().zip(values.iter()) {
        if let Some(v) = value {
            println!("{}: {}", String::from_utf8_lossy(key), String::from_utf8_lossy(v));
        }
    }
    
    Ok(())
}
```

### Range Scans

```rust
use nanograph_kvt::{KeyRange, KvIterator};
use futures::StreamExt;

async fn scan_example(store: impl KeyValueStore) -> Result<(), Box<dyn std::error::Error>> {
    let table = store.create_table("logs").await?;
    
    // Scan all keys with a prefix
    let range = KeyRange::prefix(b"log:2024-01-".to_vec()).with_limit(100);
    let mut iter = store.scan(table, range).await?;
    
    while let Some(result) = iter.next().await {
        let (key, value) = result?;
        println!("{}: {}", String::from_utf8_lossy(&key), String::from_utf8_lossy(&value));
    }
    
    // Or use the convenience method
    let mut iter = store.scan_prefix(table, b"log:2024-01-", Some(100)).await?;
    while let Some(result) = iter.next().await {
        let (key, value) = result?;
        // Process entry
    }
    
    Ok(())
}
```

### Transactions

```rust
use std::sync::Arc;

async fn transaction_example(store: impl KeyValueStore) -> Result<(), Box<dyn std::error::Error>> {
    let table = store.create_table("accounts").await?;
    
    // Begin transaction
    let txn = store.begin_transaction().await?;
    
    // Read within transaction
    let balance = txn.get(table, b"account:alice").await?;
    
    // Write within transaction
    txn.put(table, b"account:alice", b"1000").await?;
    txn.put(table, b"account:bob", b"500").await?;
    
    // Commit (or rollback on error)
    txn.commit().await?;
    
    Ok(())
}
```

## Error Handling

The crate provides comprehensive error types:

```rust
pub enum KeyValueError {
    // Core errors
    KeyNotFound,
    OutOfMemory,
    InvalidKey(String),
    InvalidValue(String),
    
    // I/O errors
    IoError(FileSystemError),
    StorageCorruption(String),
    
    // Concurrency
    LockTimeout,
    WriteConflict,
    
    // Operational
    ReadOnly,
    Closed,
    
    // Capacity limits
    StorageFull,
    KeyTooLarge { size: usize, max: usize },
    ValueTooLarge { size: usize, max: usize },
}
```

## Design Principles

1. **Trait-based abstraction**: Storage engines implement the `KeyValueStore` trait
2. **Async-first**: All I/O operations are async for better concurrency
3. **Zero-copy where possible**: Use byte slices to avoid unnecessary allocations
4. **MVCC-ready**: Built-in support for multi-version concurrency control
5. **Composable**: Operations can be combined and nested
6. **Type-safe**: Strong typing prevents common errors

## Implementation Requirements

Storage engines implementing `KeyValueStore` must provide:

- **Atomicity**: Single-key operations are atomic
- **Durability**: Writes are persisted (via WAL or equivalent)
- **Consistency**: Reads see a consistent snapshot
- **Isolation**: Transactions provide snapshot isolation
- **Ordering**: Keys are sorted lexicographically

## Performance Considerations

- **Batch operations** are more efficient than individual operations
- **Range scans** provide streaming results to avoid loading all data into memory
- **Transactions** buffer writes until commit for better performance
- **Prefix scans** are optimized for common access patterns

## Related Crates

- `nanograph-lsm`: LSM-tree storage engine implementation
- `nanograph-wal`: Write-ahead log for durability
- `nanograph-vfs`: Virtual file system abstraction

## License

Copyright 2026 Hans W. Uhlig, IBM. All Rights Reserved.

Licensed under the Apache License, Version 2.0.