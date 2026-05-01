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

//! # Nanograph LMDB Storage Engine
//!
//! This crate provides an LMDB (Lightning Memory-Mapped Database) implementation
//! of the KeyValueShardStore trait, optimized for read-heavy workloads.
//!
//! ## Features
//!
//! - **Single-file format**: Data stored in data.mdb + lock.mdb files
//! - **Memory-mapped I/O**: Fast reads through memory mapping
//! - **ACID transactions**: Full ACID compliance with MVCC
//! - **Copy-on-write B+tree**: Efficient storage structure
//! - **Zero-copy reads**: Direct memory access without copying
//! - **Read-optimized**: Excellent performance for read-heavy workloads
//!
//! ## Architecture
//!
//! LMDB uses a memory-mapped B+tree structure with copy-on-write semantics.
//! Each shard gets its own LMDB environment (directory with data.mdb and lock.mdb).
//!
//! ## Use Cases
//!
//! Best suited for:
//! - Read-heavy workloads (90%+ reads)
//! - Small to medium datasets that fit in memory
//! - Applications requiring fast point lookups
//! - Embedded use cases
//! - Single-writer, multiple-reader scenarios
//!
//! Not recommended for:
//! - Write-heavy workloads (use LSM instead)
//! - Very large datasets (>100GB)
//! - High-concurrency write scenarios
//!
//! ## Examples
//!
//! ### Basic Usage
//!
//! ```rust,no_run
//! use nanograph_lmdb::LMDBKeyValueStore;
//! use nanograph_kvt::{KeyValueShardStore, ShardId};
//! use nanograph_vfs::{MemoryFileSystem, Path};
//! use std::sync::Arc;
//!
//! # #[tokio::main]
//! # async fn main() {
//! // Create LMDB store with default configuration
//! let store = LMDBKeyValueStore::new();
//!
//! // Create a shard
//! let shard_id = ShardId::new(1);
//! let vfs = Arc::new(MemoryFileSystem::new());
//! let data_path = Path::from("/data/shard1");
//! let wal_path = Path::from("/wal/shard1");
//! store.create_shard(shard_id, vfs, data_path, wal_path).unwrap();
//!
//! // Insert data
//! store.put(shard_id, b"key1", b"value1").await.unwrap();
//! store.put(shard_id, b"key2", b"value2").await.unwrap();
//!
//! // Retrieve data
//! let value = store.get(shard_id, b"key1").await.unwrap();
//! assert_eq!(value, Some(b"value1".to_vec()));
//!
//! // Delete data
//! store.delete(shard_id, b"key1").await.unwrap();
//! # }
//! ```
//!
//! ### Custom Configuration
//!
//! ```rust
//! use nanograph_lmdb::{LMDBKeyValueStore, LMDBConfig};
//!
//! let config = LMDBConfig::default()
//!     .with_max_db_size(2 * 1024 * 1024 * 1024) // 2GB
//!     .with_max_dbs(256)
//!     .with_sync_on_commit(false); // Faster but less safe
//!
//! let store = LMDBKeyValueStore::with_config(config);
//! ```
//!
//! ### Range Scans
//!
//! ```rust,no_run
//! use nanograph_lmdb::LMDBKeyValueStore;
//! use nanograph_kvt::{KeyValueShardStore, KeyRange, ShardId};
//!
//! # #[tokio::main]
//! # async fn main() {
//! # let store = LMDBKeyValueStore::new();
//! # let shard_id = ShardId::new(1);
//! // Scan a range
//! let range = KeyRange::prefix(b"product:".to_vec());
//! let mut iter = store.scan(shard_id, range).await.unwrap();
//!
//! while let Some(result) = iter.next().await {
//!     let (key, value) = result.unwrap();
//!     println!("{:?} => {:?}", key, value);
//! }
//! # }
//! ```

mod config;
mod error;
mod iterator;
mod kvstore;
mod transaction;

pub use self::config::{LMDBConfig, LMDBStorageConfig};
pub use self::error::{LMDBError, LMDBResult};
pub use self::iterator::LMDBIterator;
pub use self::kvstore::LMDBKeyValueStore;
pub use self::transaction::LMDBTransaction;
pub use nanograph_kvt::{KeyValueError, KeyValueResult};

// Made with Bob
