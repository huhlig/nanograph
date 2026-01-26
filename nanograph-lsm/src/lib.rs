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

//! # Nanograph Log Structured Merge Tree
//!
//! This crate provides a Log-Structured Merge Tree (LSM Tree) implementation
//! optimized for write-heavy workloads with efficient range scans.
//!
//! ## Features
//!
//! - **MemTable**: In-memory sorted data structure with MVCC support
//! - **SSTable**: Immutable on-disk sorted tables with bloom filters
//! - **Leveled Compaction**: Multi-level compaction strategy
//! - **Compression**: Optional block-level compression
//! - **Integrity**: Checksums for data integrity
//! - **Metrics**: Comprehensive performance monitoring
//! - **Error Handling**: Detailed error types with recovery information
//! - **KeyValueStore**: Full implementation of the KeyValueStore trait
//! - **Transactions**: MVCC-based transactions with snapshot isolation
//! - **Block Cache**: LRU cache for hot data blocks
//! - **Async Support**: Async/await interface with tokio
//!
//! ## Architecture
//!
//! See `ARCHITECTURE.md` for detailed design documentation.
//!
//! ## Examples
//!
//! ### Basic Usage with KeyValueStore
//!
//! ```rust,no_run
//! use nanograph_lsm::LSMKeyValueStore;
//! use nanograph_kvt::{KeyValueShardStore, TableId, IndexNumber};
//!
//! # #[tokio::main]
//! # async fn main() {
//! // Create LSM store with default options
//! let store = LSMKeyValueStore::new();
//!
//! // Create a shard with table ID and shard index
//! let table_id = TableId::from(1u64);
//! let shard_index = IndexNumber::from(0u32);
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
//! # }
//! ```
//!
//! ### Configuring LSM Options
//!
//! ```rust
//! use nanograph_lsm::LSMTreeOptions;
//! use nanograph_util::CompressionAlgorithm;
//!
//! let mut options = LSMTreeOptions::default();
//! options.memtable_size = 4 * 1024 * 1024; // 4MB
//! options.compression = CompressionAlgorithm::Zstd;
//! options.block_size = 8192; // 8KB blocks
//!
//! println!("MemTable size: {} bytes", options.memtable_size);
//! println!("Block size: {} bytes", options.block_size);
//! ```
//!
//! ### Range Scans
//!
//! ```rust,no_run
//! use nanograph_lsm::LSMKeyValueStore;
//! use nanograph_kvt::{KeyValueShardStore, KeyRange, TableId, IndexNumber};
//! use futures::StreamExt;
//!
//! # #[tokio::main]
//! # async fn main() {
//! let store = LSMKeyValueStore::new();
//! let table_id = TableId::from(1u64);
//! let shard_index = IndexNumber::from(0u32);
//! let shard = store.create_shard(table_id, shard_index).await.unwrap();
//!
//! // Insert sorted data
//! store.put(shard, b"product:001", b"Widget A").await.unwrap();
//! store.put(shard, b"product:002", b"Widget B").await.unwrap();
//! store.put(shard, b"product:003", b"Widget C").await.unwrap();
//!
//! // Scan a range
//! let range = KeyRange::prefix(b"product:".to_vec());
//! let mut iter = store.scan(shard, range).await.unwrap();
//!
//! while let Some(result) = iter.next().await {
//!     let (key, value) = result.unwrap();
//!     println!("{:?} => {:?}", key, value);
//! }
//! # }
//! ```
//!
//! ### Transactions with MVCC
//!
//! ```rust,no_run
//! use nanograph_lsm::LSMKeyValueStore;
//! use nanograph_kvt::{KeyValueShardStore, TableId, IndexNumber};
//!
//! # #[tokio::main]
//! # async fn main() {
//! let store = LSMKeyValueStore::new();
//! let table_id = TableId::from(1u64);
//! let shard_index = IndexNumber::from(0u32);
//! let shard = store.create_shard(table_id, shard_index).await.unwrap();
//!
//! // Start a transaction
//! let txn = store.begin_transaction().await.unwrap();
//!
//! // Perform operations within transaction
//! txn.put(shard, b"account:alice", b"1000").await.unwrap();
//! txn.put(shard, b"account:bob", b"500").await.unwrap();
//!
//! // Read within transaction sees uncommitted changes
//! let balance = txn.get(shard, b"account:alice").await.unwrap();
//! assert_eq!(balance, Some(b"1000".to_vec()));
//!
//! // Commit transaction
//! txn.commit().await.unwrap();
//! # }
//! ```
//!
//! ### Monitoring with Metrics
//!
//! ```rust,no_run
//! use nanograph_lsm::{LSMKeyValueStore, LSMMetrics};
//!
//! # #[tokio::main]
//! # async fn main() {
//! let store = LSMKeyValueStore::new();
//! let metrics = LSMMetrics::new();
//!
//! // Perform operations...
//! // Metrics are automatically collected
//!
//! let snapshot = metrics.snapshot();
//! println!("Total writes: {}", snapshot.total_writes);
//! println!("Total reads: {}", snapshot.total_reads);
//! println!("Block cache hit rate: {:.2}%", snapshot.block_cache_hit_rate * 100.0);
//! println!("Write amplification: {:.2}", snapshot.write_amplification);
//! # }
//! ```

mod cache;
mod compaction;
mod config;
mod engine;
mod error;
mod iterator;
mod kvstore;
mod memtable;
mod metrics;
mod options;
mod sstable;
mod transaction;
mod wal_record;

pub use self::cache::{BlockCache, BlockKey, CacheStats};
pub use self::compaction::{CompactionStrategy, CompactionTask};
pub use self::config::LSMStorageConfig;
pub use self::engine::{EngineStats, LSMTreeEngine, LevelStats};
pub use self::error::{ErrorSeverity, LSMError, LSMResult};
pub use self::iterator::LSMIterator;
pub use self::kvstore::LSMKeyValueStore;
pub use self::memtable::{Entry, MemTable};
pub use self::metrics::{BloomFilterResult, LSMMetrics, MetricsSnapshot};
pub use self::options::LSMTreeOptions;
pub use self::sstable::{DataBlock, SSTable, SSTableMetadata};
pub use self::transaction::{LSMTransaction, TransactionManager};
pub use self::wal_record::{WalRecordKind, decode_delete, decode_put, encode_delete, encode_put};
pub use nanograph_kvt::{KeyValueError, KeyValueResult};
