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

mod cache;
mod compaction;
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
