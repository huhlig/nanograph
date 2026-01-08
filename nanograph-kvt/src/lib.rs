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
//! # Example
//!
//! ```rust,no_run
//! use nanograph_kvt::{KeyValueStore, KeyValueTableId, KeyRange};
//!
//! async fn example(store: impl KeyValueStore) -> Result<(), Box<dyn std::error::Error>> {
//!     let table = store.create_table("my_table").await?;
//!
//!     // Simple put/get
//!     store.put(table, b"key1", b"value1").await?;
//!     let value = store.get(table, b"key1").await?;
//!
//!     // Range scan
//!     let range = KeyRange::prefix(b"prefix_".to_vec());
//!     let mut iter = store.scan(table, range).await?;
//!
//!     // Transaction
//!     let txn = store.begin_transaction().await?;
//!     txn.put(table, b"key2", b"value2").await?;
//!     txn.commit().await?;
//!
//!     Ok(())
//! }
//! ```

mod iterator;
mod kvstore;
mod manager;
pub mod metrics;
mod result;
mod transaction;
mod types;

// Re-export all public types
pub use self::iterator::KeyValueIterator;
pub use self::kvstore::KeyValueStore;
pub use self::kvstore::KeyValueTableId;
pub use self::manager::{KeyValueTableManager, StorageEngineType, TableConfig, TableMetadata};
pub use self::metrics::EngineMetrics;
pub use self::result::{KeyValueError, KeyValueResult};
pub use self::transaction::Transaction;
pub use self::transaction::{Timestamp, TransactionId};
pub use self::types::{
    ArtStats, BTreeStats, EngineStats, KeyRange, LsmStats, ShardId, StatValue, TableStats,
};