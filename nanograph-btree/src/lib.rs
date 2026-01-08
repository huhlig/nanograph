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
//! ## Example
//!
//! ```rust
//! use nanograph_btree::{BTreeKeyValueStore, tree::BPlusTreeConfig};
//! use nanograph_kvt::KeyValueStore;
//!
//! #[tokio::main]
//! async fn main() {
//!     // Create store with default configuration
//!     let store = BTreeKeyValueStore::default();
//!     
//!     // Create a table
//!     let table = store.create_table("my_table").await.unwrap();
//!     
//!     // Insert data
//!     store.put(table, b"key1", b"value1").await.unwrap();
//!     
//!     // Retrieve data
//!     let value = store.get(table, b"key1").await.unwrap();
//!     assert_eq!(value, Some(b"value1".to_vec()));
//! }
//! ```

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

// Re-export main types
pub use error::{BTreeError, BTreeResult};
pub use kvstore::BTreeKeyValueStore;
pub use metrics::{BTreeMetrics, BTreeMetricsSnapshot};
pub use node::{BPlusTreeNode, InternalNode, LeafNode, NodeId};
pub use persistence::{BTreePersistence, TreeMetadata};
pub use transaction::{BTreeTransaction, TransactionManager};
pub use tree::{BPlusTree, BPlusTreeConfig, BTreeStats};

// Made with Bob
