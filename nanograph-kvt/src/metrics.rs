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

//! Pluggable metrics system for storage engines
//!
//! This module provides a metrics-based approach to table statistics that allows
//! each storage engine to register its own custom metrics without modifying the
//! core nanograph-kvt crate.
//!
//! # Design Philosophy
//!
//! Instead of hardcoding engine-specific statistics in enums or structs, we use
//! the `metrics` crate to allow engines to register arbitrary metrics. This makes
//! the system fully pluggable - new engines can expose their own metrics without
//! any code changes to this crate.
//!
//! # Metric Naming Convention
//!
//! All metrics follow the pattern: `nanograph.storage.{engine_type}.{metric_name}`
//!
//! Common labels:
//! - `table_id`: The table identifier
//! - `table_name`: Human-readable table name  
//! - `engine`: Storage engine type (lsm, btree, art, custom)
//!
//! # Example: LSM Engine
//!
//! ```rust,no_run
//! use nanograph_kvt::metrics::EngineMetrics;
//! use nanograph_kvt::KeyValueTableId;
//!
//! # struct LsmEngine;
//! # impl LsmEngine {
//! #     fn compaction_count(&self) -> u64 { 0 }
//! #     fn memtable_size(&self) -> usize { 0 }
//! #     fn calculate_write_amplification(&self) -> f64 { 1.0 }
//! # }
//! impl EngineMetrics for LsmEngine {
//!     fn register_metrics(&self, table_id: KeyValueTableId, table_name: &str) {
//!         let labels = [
//!             ("table_id", table_id.0.to_string()),
//!             ("table_name", table_name.to_string()),
//!             ("engine", "lsm".to_string()),
//!         ];
//!
//!         // Register LSM-specific metrics
//!         metrics::describe_counter!(
//!             "nanograph.storage.lsm.compactions_total",
//!             "Total number of compactions performed"
//!         );
//!
//!         metrics::describe_gauge!(
//!             "nanograph.storage.lsm.memtable_bytes",
//!             "Current memtable size in bytes"
//!         );
//!
//!         metrics::describe_gauge!(
//!             "nanograph.storage.lsm.write_amplification",
//!             "Write amplification factor"
//!         );
//!     }
//!
//!     fn update_metrics(&self, table_id: KeyValueTableId) {
//!         let labels = [
//!             ("table_id", table_id.0.to_string()),
//!             ("engine", "lsm".to_string()),
//!         ];
//!
//!         // Update metric values
//!         metrics::counter!("nanograph.storage.lsm.compactions_total", &labels)
//!             .increment(self.compaction_count());
//!
//!         metrics::gauge!("nanograph.storage.lsm.memtable_bytes", &labels)
//!             .set(self.memtable_size() as f64);
//!
//!         metrics::gauge!("nanograph.storage.lsm.write_amplification", &labels)
//!             .set(self.calculate_write_amplification());
//!     }
//!
//!     fn metric_names(&self) -> Vec<String> {
//!         vec![
//!             "nanograph.storage.lsm.compactions_total".to_string(),
//!             "nanograph.storage.lsm.memtable_bytes".to_string(),
//!             "nanograph.storage.lsm.write_amplification".to_string(),
//!             "nanograph.storage.lsm.levels".to_string(),
//!             "nanograph.storage.lsm.sstables_per_level".to_string(),
//!         ]
//!     }
//! }
//! ```
//!
//! # Example: Custom Engine
//!
//! ```rust,no_run
//! use nanograph_kvt::metrics::EngineMetrics;
//! use nanograph_kvt::KeyValueTableId;
//!
//! # struct MyCustomEngine;
//! impl EngineMetrics for MyCustomEngine {
//!     fn register_metrics(&self, table_id: KeyValueTableId, table_name: &str) {
//!         let labels = [
//!             ("table_id", table_id.0.to_string()),
//!             ("engine", "my-custom-engine".to_string()),
//!         ];
//!
//!         // Register custom metrics - no changes to nanograph-kvt needed!
//!         metrics::describe_histogram!(
//!             "nanograph.storage.my_custom.query_latency_ms",
//!             "Query latency in milliseconds"
//!         );
//!
//!         metrics::describe_counter!(
//!             "nanograph.storage.my_custom.cache_hits",
//!             "Number of cache hits"
//!         );
//!     }
//!
//!     fn update_metrics(&self, table_id: KeyValueTableId) {
//!         // Update custom metrics
//!         // ...
//!     }
//!
//!     fn metric_names(&self) -> Vec<String> {
//!         vec![
//!             "nanograph.storage.my_custom.query_latency_ms".to_string(),
//!             "nanograph.storage.my_custom.cache_hits".to_string(),
//!         ]
//!     }
//! }
//! ```

use crate::kvstore::KeyValueTableId;

/// Helper trait for storage engines to register their metrics
///
/// Engines implement this to register engine-specific metrics using the
/// `metrics` crate. This allows for fully pluggable statistics without
/// modifying the core nanograph-kvt crate.
pub trait EngineMetrics: Send + Sync {
    /// Register all metrics for this engine
    ///
    /// Called when a table is created. Engines should register all their
    /// metrics here with appropriate labels and descriptions.
    ///
    /// # Arguments
    ///
    /// * `table_id` - The table identifier
    /// * `table_name` - Human-readable table name
    fn register_metrics(&self, table_id: KeyValueTableId, table_name: &str);

    /// Update metrics for this table
    ///
    /// Called periodically or on-demand to update metric values.
    /// Engines should update all their registered metrics here.
    ///
    /// # Arguments
    ///
    /// * `table_id` - The table identifier
    fn update_metrics(&self, table_id: KeyValueTableId);

    /// Get engine-specific metric names
    ///
    /// Returns a list of all metrics this engine exposes.
    /// Useful for introspection and monitoring setup.
    fn metric_names(&self) -> Vec<String>;
}

/// Common metric names used across all engines
pub mod common {
    /// Total number of keys in the table (gauge)
    pub const KEY_COUNT: &str = "nanograph.storage.common.key_count";

    /// Total bytes used by the table (gauge)
    pub const TOTAL_BYTES: &str = "nanograph.storage.common.total_bytes";

    /// Bytes used for key-value data (gauge)
    pub const DATA_BYTES: &str = "nanograph.storage.common.data_bytes";

    /// Bytes used for indexes and metadata (gauge)
    pub const INDEX_BYTES: &str = "nanograph.storage.common.index_bytes";

    /// Number of get operations (counter)
    pub const GET_OPS: &str = "nanograph.storage.common.get_ops_total";

    /// Number of put operations (counter)
    pub const PUT_OPS: &str = "nanograph.storage.common.put_ops_total";

    /// Number of delete operations (counter)
    pub const DELETE_OPS: &str = "nanograph.storage.common.delete_ops_total";

    /// Number of scan operations (counter)
    pub const SCAN_OPS: &str = "nanograph.storage.common.scan_ops_total";

    /// Get operation latency (histogram, milliseconds)
    pub const GET_LATENCY_MS: &str = "nanograph.storage.common.get_latency_ms";

    /// Put operation latency (histogram, milliseconds)
    pub const PUT_LATENCY_MS: &str = "nanograph.storage.common.put_latency_ms";
}

/// Example LSM-specific metric names
pub mod lsm {
    /// Number of LSM levels (gauge)
    pub const LEVELS: &str = "nanograph.storage.lsm.levels";

    /// Number of SSTables (gauge, with level label)
    pub const SSTABLES: &str = "nanograph.storage.lsm.sstables";

    /// Memtable size in bytes (gauge)
    pub const MEMTABLE_BYTES: &str = "nanograph.storage.lsm.memtable_bytes";

    /// Total compactions performed (counter)
    pub const COMPACTIONS_TOTAL: &str = "nanograph.storage.lsm.compactions_total";

    /// Write amplification factor (gauge)
    pub const WRITE_AMPLIFICATION: &str = "nanograph.storage.lsm.write_amplification";

    /// Read amplification factor (gauge)
    pub const READ_AMPLIFICATION: &str = "nanograph.storage.lsm.read_amplification";

    /// Bloom filter false positive rate (gauge)
    pub const BLOOM_FP_RATE: &str = "nanograph.storage.lsm.bloom_false_positive_rate";
}

/// Example B+Tree-specific metric names
pub mod btree {
    /// Tree height (gauge)
    pub const HEIGHT: &str = "nanograph.storage.btree.height";

    /// Total number of nodes (gauge)
    pub const TOTAL_NODES: &str = "nanograph.storage.btree.total_nodes";

    /// Number of leaf nodes (gauge)
    pub const LEAF_NODES: &str = "nanograph.storage.btree.leaf_nodes";

    /// Average node utilization (gauge, 0.0-1.0)
    pub const NODE_UTILIZATION: &str = "nanograph.storage.btree.node_utilization";

    /// Total node splits (counter)
    pub const SPLITS_TOTAL: &str = "nanograph.storage.btree.splits_total";

    /// Total node merges (counter)
    pub const MERGES_TOTAL: &str = "nanograph.storage.btree.merges_total";
}

/// Example ART-specific metric names
pub mod art {
    /// Maximum trie depth (gauge)
    pub const MAX_DEPTH: &str = "nanograph.storage.art.max_depth";

    /// Average trie depth (gauge)
    pub const AVG_DEPTH: &str = "nanograph.storage.art.avg_depth";

    /// Number of Node4 nodes (gauge)
    pub const NODE4_COUNT: &str = "nanograph.storage.art.node4_count";

    /// Number of Node16 nodes (gauge)
    pub const NODE16_COUNT: &str = "nanograph.storage.art.node16_count";

    /// Number of Node48 nodes (gauge)
    pub const NODE48_COUNT: &str = "nanograph.storage.art.node48_count";

    /// Number of Node256 nodes (gauge)
    pub const NODE256_COUNT: &str = "nanograph.storage.art.node256_count";

    /// Memory usage in bytes (gauge)
    pub const MEMORY_BYTES: &str = "nanograph.storage.art.memory_bytes";

    /// Path compressions performed (counter)
    pub const PATH_COMPRESSIONS: &str = "nanograph.storage.art.path_compressions_total";
}

// Made with Bob
