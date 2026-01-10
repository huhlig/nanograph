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

//! Nanograph Key-Value Traits
//!
//! This crate defines the core traits and types for key-value storage in Nanograph.
//! It provides a unified interface that can be implemented by different storage engines
//! (LSM, B+Tree, ART, etc.).
//!
//! # Features
//!
//! - **Async-first API**: All operations are async for non-blocking I/O
//! - **MVCC Support**: Built-in support for multi-version concurrency control
//! - **Transaction Support**: ACID transactions with snapshot isolation
//! - **Flexible Iteration**: Range scans with seeking and filtering
//! - **Batch Operations**: Efficient batch get/put/delete operations
//! - **Table Management**: Multi-table support with metadata
//!
//! # Examples
//!
//! ## Basic Store Operations
//!
//! ```rust,no_run
//! use nanograph_kvt::{KeyValueShardStore, ShardId, KeyRange, TableId, ShardIndex};
//!
//! async fn example(store: impl KeyValueShardStore) -> Result<(), Box<dyn std::error::Error>> {
//!     // Create a shard (table ID 1, shard index 0)
//!     let table_id = TableId::from(1u64);
//!     let shard = store.create_shard(table_id, ShardIndex::from(0u32)).await?;
//!
//!     // Simple put/get
//!     store.put(shard, b"key1", b"value1").await?;
//!     let value = store.get(shard, b"key1").await?;
//!     assert_eq!(value, Some(b"value1".to_vec()));
//!
//!     // Delete operation
//!     store.delete(shard, b"key1").await?;
//!     assert_eq!(store.get(shard, b"key1").await?, None);
//!
//!     Ok(())
//! }
//! ```
//!
//! ## Range Scans
//!
//! ```rust,no_run
//! use nanograph_kvt::{KeyValueShardStore, KeyRange, TableId, ShardIndex};
//! use futures::StreamExt;
//!
//! async fn range_example(store: impl KeyValueShardStore) -> Result<(), Box<dyn std::error::Error>> {
//!     let table_id = TableId::from(1u64);
//!     let shard = store.create_shard(table_id, ShardIndex::from(0u32)).await?;
//!
//!     // Insert data with common prefix
//!     store.put(shard, b"product:001", b"Widget A").await?;
//!     store.put(shard, b"product:002", b"Widget B").await?;
//!     store.put(shard, b"product:003", b"Widget C").await?;
//!
//!     // Scan with prefix
//!     let range = KeyRange::prefix(b"product:".to_vec());
//!     let mut iter = store.scan(shard, range).await?;
//!
//!     while let Some(result) = iter.next().await {
//!         let (key, value) = result?;
//!         println!("{:?} => {:?}", key, value);
//!     }
//!
//!     Ok(())
//! }
//! ```
//!
//! ## Transactions
//!
//! ```rust,no_run
//! use nanograph_kvt::{KeyValueShardStore, TableId, ShardIndex};
//!
//! async fn transaction_example(store: impl KeyValueShardStore) -> Result<(), Box<dyn std::error::Error>> {
//!     let table_id = TableId::from(1u64);
//!     let shard = store.create_shard(table_id, ShardIndex::from(0u32)).await?;
//!
//!     // Start a transaction
//!     let txn = store.begin_transaction().await?;
//!
//!     // Perform multiple operations atomically
//!     txn.put(shard, b"account:alice", b"1000").await?;
//!     txn.put(shard, b"account:bob", b"500").await?;
//!
//!     // Read within transaction
//!     let balance = txn.get(shard, b"account:alice").await?;
//!     assert_eq!(balance, Some(b"1000".to_vec()));
//!
//!     // Commit all changes atomically
//!     txn.commit().await?;
//!
//!     Ok(())
//! }
//! ```
//!
//! ## Batch Operations
//!
//! ```rust,no_run
//! use nanograph_kvt::{KeyValueShardStore, TableId, ShardIndex};
//!
//! async fn batch_example(store: impl KeyValueShardStore) -> Result<(), Box<dyn std::error::Error>> {
//!     let table_id = TableId::from(1u64);
//!     let shard = store.create_shard(table_id, ShardIndex::from(0u32)).await?;
//!
//!     // Batch put - need to convert to slice references
//!     let entries: &[(&[u8], &[u8])] = &[
//!         (b"key1", b"value1"),
//!         (b"key2", b"value2"),
//!         (b"key3", b"value3"),
//!     ];
//!     store.batch_put(shard, entries).await?;
//!
//!     // Batch get - need to convert to slice references
//!     let keys: &[&[u8]] = &[b"key1", b"key2", b"key3"];
//!     let values = store.batch_get(shard, keys).await?;
//!     assert_eq!(values.len(), 3);
//!
//!     Ok(())
//! }
//! ```
//!
//! ## Using KeyRange
//!
//! ```rust
//! use nanograph_kvt::KeyRange;
//! use std::collections::Bound;
//!
//! // Create a range with explicit bounds
//! let range = KeyRange::new(
//!     Bound::Included(b"a".to_vec()),
//!     Bound::Included(b"z".to_vec())
//! );
//!
//! // Create a range from start to end (more convenient)
//! let range2 = KeyRange::from_to(b"a".to_vec(), b"z".to_vec());
//!
//! // Create a prefix range
//! let prefix_range = KeyRange::prefix(b"user:".to_vec());
//!
//! // Create an unbounded range (all keys)
//! let all_range = KeyRange::all();
//! ```

mod config;
mod kviter;
mod kvstore;
pub mod metrics;
mod result;
mod transaction;
mod types;

// Re-export all public types
pub use self::config::{
    NamespaceConfig, NamespaceMetadata, ShardConfig, ShardMetadata, ShardState, ShardStatus,
    StorageEngineType, TableConfig, TableMetadata, TableSharding,
};
pub use self::kviter::KeyValueIterator;
pub use self::kvstore::KeyValueShardStore;
pub use self::metrics::EngineMetrics;
pub use self::result::{KeyValueError, KeyValueResult};
pub use self::transaction::Transaction;
pub use self::transaction::TransactionId;
pub use self::types::{HashFunction, KeyRange, Partitioner};
pub use nanograph_core::types::{ShardId, ShardIndex, TableId, Timestamp};
