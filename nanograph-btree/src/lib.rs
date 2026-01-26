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

//! # nanograph-btree
//!
//! B+Tree storage engine implementation for Nanograph.
//!
//! This crate provides an in-memory B+Tree implementation that conforms to the
//! `KeyValueStore` trait. B+Trees are well-suited for range queries and maintain
//! sorted order of keys.
//!
//! ## Features
//!
//! - **B+Tree Structure**: All data stored in leaf nodes, internal nodes only for routing
//! - **Efficient Range Scans**: Linked leaf nodes enable fast sequential access
//! - **MVCC Support**: Snapshot isolation for transactions
//! - **Metrics**: Comprehensive performance tracking
//! - **Configurable**: Adjustable node size and tree parameters
//!
//! ## Architecture
//!
//! The B+Tree implementation consists of:
//!
//! - **Nodes**: Internal nodes (routing) and leaf nodes (data storage)
//! - **Tree**: Main B+Tree structure with insert, search, delete operations
//! - **Iterator**: Efficient forward and reverse range scans
//! - **Transactions**: MVCC-based transaction support
//! - **Metrics**: Performance monitoring and statistics
//!
//! ## Examples
//!
//! ### Basic KeyValueStore Usage
//!
//! ```rust
//! use nanograph_btree::BTreeKeyValueStore;
//! use nanograph_kvt::{KeyValueShardStore, TableId, IndexNumber};
//!
//! # #[tokio::main]
//! # async fn main() {
//! // Create store with default configuration
//! let store = BTreeKeyValueStore::default();
//!
//! // Create a shard
//! let table_id = TableId::new(1);
//! let shard_index = IndexNumber::new(0);
//! let shard = store.create_shard(table_id, shard_index).await.unwrap();
//!
//! // Insert data
//! store.put(shard, b"key1", b"value1").await.unwrap();
//! store.put(shard, b"key2", b"value2").await.unwrap();
//!
//! // Retrieve data
//! let value = store.get(shard, b"key1").await.unwrap();
//! assert_eq!(value, Some(b"value1".to_vec()));
//!
//! // Delete data
//! store.delete(shard, b"key1").await.unwrap();
//! assert_eq!(store.get(shard, b"key1").await.unwrap(), None);
//! # }
//! ```
//!
//! ### Direct B+Tree Usage
//!
//! ```rust
//! use nanograph_btree::{BPlusTree, BPlusTreeConfig};
//!
//! // Create tree with custom configuration
//! let config = BPlusTreeConfig {
//!     max_keys: 4,
//!     min_keys: 2,
//! };
//! let tree = BPlusTree::new(config);
//!
//! // Insert key-value pairs
//! tree.insert(b"apple".to_vec(), b"red fruit".to_vec()).unwrap();
//! tree.insert(b"banana".to_vec(), b"yellow fruit".to_vec()).unwrap();
//! tree.insert(b"cherry".to_vec(), b"red fruit".to_vec()).unwrap();
//!
//! // Search for values
//! let value = tree.get(b"banana").unwrap();
//! assert_eq!(value, Some(b"yellow fruit".to_vec()));
//!
//! // Get tree statistics
//! let stats = tree.stats();
//! println!("Tree height: {}", stats.height);
//! println!("Total keys: {}", stats.num_keys);
//! ```
//!
//! ### Range Scans
//!
//! ```rust
//! use nanograph_btree::BTreeKeyValueStore;
//! use nanograph_kvt::{KeyValueShardStore, KeyRange, TableId, IndexNumber};
//! use std::collections::Bound;
//! use futures::StreamExt;
//!
//! # #[tokio::main]
//! # async fn main() {
//! let store = BTreeKeyValueStore::default();
//! let table_id = TableId::new(1);
//! let shard_index = IndexNumber::new(0);
//! let shard = store.create_shard(table_id, shard_index).await.unwrap();
//!
//! // Insert sorted data
//! store.put(shard, b"apple", b"1").await.unwrap();
//! store.put(shard, b"banana", b"2").await.unwrap();
//! store.put(shard, b"cherry", b"3").await.unwrap();
//! store.put(shard, b"date", b"4").await.unwrap();
//!
//! // Scan a range
//! let range = KeyRange::new(
//!     Bound::Included(b"banana".to_vec()),
//!     Bound::Included(b"date".to_vec())
//! );
//! let mut iter = store.scan(shard, range).await.unwrap();
//!
//! let mut results = Vec::new();
//! while let Some(result) = iter.next().await {
//!     let (key, value) = result.unwrap();
//!     results.push((key, value));
//! }
//!
//! assert_eq!(results.len(), 3); // banana, cherry, date
//! # }
//! ```
//!
//! ### Transactions with MVCC
//!
//! ```rust
//! use nanograph_btree::BTreeKeyValueStore;
//! use nanograph_kvt::{KeyValueShardStore, TableId, IndexNumber};
//!
//! # #[tokio::main]
//! # async fn main() {
//! let store = BTreeKeyValueStore::default();
//! let table_id = TableId::new(1);
//! let shard_index = IndexNumber::new(0);
//! let shard = store.create_shard(table_id, shard_index).await.unwrap();
//!
//! // Start a transaction
//! let txn = store.begin_transaction().await.unwrap();
//!
//! // Perform operations within transaction
//! txn.put(shard, b"account1", b"100").await.unwrap();
//! txn.put(shard, b"account2", b"200").await.unwrap();
//!
//! // Read within transaction
//! let balance = txn.get(shard, b"account1").await.unwrap();
//! assert_eq!(balance, Some(b"100".to_vec()));
//!
//! // Commit transaction
//! txn.commit().await.unwrap();
//!
//! // Verify committed data
//! let value = store.get(shard, b"account1").await.unwrap();
//! assert_eq!(value, Some(b"100".to_vec()));
//! # }
//! ```

pub mod config;
pub mod error;
pub mod iterator;
pub mod kvstore;
pub mod metrics;
pub mod mvcc;
pub mod mvcc_node;
pub mod mvcc_transaction;
pub mod mvcc_tree;
pub mod node;
pub mod persistence;
pub mod transaction;
pub mod tree;
pub mod wal_record;

// Re-export main types
pub use config::BTreeStorageConfig;
pub use error::{BTreeError, BTreeResult};
pub use kvstore::BTreeKeyValueStore;
pub use metrics::{BTreeMetrics, BTreeMetricsSnapshot};
pub use node::{BPlusTreeNode, BTreeNodeId, InternalNode, LeafNode};
pub use persistence::{BTreePersistence, TreeMetadata};
pub use transaction::{BTreeTransaction, TransactionManager};
pub use tree::{BPlusTree, BPlusTreeConfig, BTreeStats};
