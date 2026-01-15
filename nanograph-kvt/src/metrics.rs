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
//! use nanograph_kvt::ShardId;
//!
//! # struct LsmEngine;
//! # impl LsmEngine {
//! #     fn compaction_count(&self) -> u64 { 0 }
//! #     fn memtable_size(&self) -> usize { 0 }
//! #     fn calculate_write_amplification(&self) -> f64 { 1.0 }
//! # }
//! impl EngineMetrics for LsmEngine {
//!     fn register_metrics(&self, table_id: ShardId, table_name: &str) {
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
//!     fn update_metrics(&self, table_id: ShardId) {
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
//! use nanograph_kvt::ShardId;
//!
//! # struct MyCustomEngine;
//! impl EngineMetrics for MyCustomEngine {
//!     fn register_metrics(&self, table_id: ShardId, table_name: &str) {
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
//!     fn update_metrics(&self, table_id: ShardId) {
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

use nanograph_core::object::ShardId;
use nanograph_core::types::Timestamp;
use std::collections::HashMap;

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
    fn register_metrics(&self, table_id: ShardId, table_name: &str);

    /// Update metrics for this table
    ///
    /// Called periodically or on-demand to update metric values.
    /// Engines should update all their registered metrics here.
    ///
    /// # Arguments
    ///
    /// * `table_id` - The table identifier
    fn update_metrics(&self, table_id: ShardId);

    /// Get engine-specific metric names
    ///
    /// Returns a list of all metrics this engine exposes.
    /// Useful for introspection and monitoring setup.
    fn metric_names(&self) -> Vec<String>;
}

/// Common table statistics shared across all storage engines
#[derive(Debug, Clone)]
pub struct ShardStats {
    /// Approximate number of keys in the table
    pub key_count: u64,

    /// Total bytes used by the table (data + metadata + indexes)
    pub total_bytes: u64,

    /// Bytes used for actual key-value data
    pub data_bytes: u64,

    /// Bytes used for indexes and metadata
    pub index_bytes: u64,

    /// Last modification timestamp
    pub last_modified: Option<Timestamp>,

    /// Engine-specific statistics
    pub engine_stats: EngineStats,
}

impl ShardStats {
    /// Get Stat by name
    pub fn get(&self, key: &str) -> StatValue {
        match key {
            "key_count" => StatValue::None,
            "total_bytes" => StatValue::None,
            "data_bytes" => StatValue::None,
            "index_bytes" => StatValue::None,
            "last_modified" => StatValue::None,
            _ => self.engine_stats.get(key),
        }
    }
    pub fn keys(&self) -> impl Iterator<Item = String> {
        let mut names = vec![
            String::from("key_count"),
            String::from("total_bytes"),
            String::from("data_bytes"),
            String::from("index_bytes"),
            String::from("last_modified"),
        ];
        names.extend(self.engine_stats.keys());
        names.into_iter()
    }
    /// Get Iterator of Stats
    pub fn iter(&self) -> impl Iterator<Item = (String, StatValue)> {
        let mut stats = vec![
            (String::from("key_count"), StatValue::U64(self.key_count)),
            (
                String::from("total_bytes"),
                StatValue::U64(self.total_bytes),
            ),
            (String::from("data_bytes"), StatValue::U64(self.data_bytes)),
            (
                String::from("index_bytes"),
                StatValue::U64(self.index_bytes),
            ),
            (
                String::from("last_modified"),
                StatValue::Timestamp(self.last_modified.unwrap_or(Timestamp::epoch())),
            ),
        ];
        stats.extend(self.engine_stats.iter());
        stats.into_iter()
    }
}

/// Shard Engine statistics
#[derive(Clone, Debug, Default)]
pub struct EngineStats(HashMap<String, StatValue>);

impl EngineStats {
    /// Insert an engine-specific statistic.
    pub fn insert(&mut self, key: &str, value: StatValue) {
        self.0.insert(key.to_owned(), value);
    }
    /// Retrieve an engine-specific statistic.
    pub fn get(&self, key: &str) -> StatValue {
        self.0.get(key).cloned().unwrap_or_default()
    }
    /// Iterator of engine-specific statistic names
    pub fn keys(&self) -> impl Iterator<Item = String> {
        self.0.keys().cloned()
    }
    /// Iterator of engine-specific statistics
    pub fn iter(&self) -> impl Iterator<Item = (String, StatValue)> {
        self.0.iter().map(|(k, v)| (k.clone(), v.clone()))
    }
}

/// Generic statistic value for extensibility
#[derive(Clone, Debug, Default)]
pub enum StatValue {
    /// None value
    #[default]
    None,
    /// Unsigned 64-bit integer
    U64(u64),
    /// Signed 64-bit integer
    I64(i64),
    /// 64-bit floating point
    F64(f64),
    /// Boolean value
    Bool(bool),
    /// String value
    String(String),
    /// List of statistic values
    List(Vec<StatValue>),
    /// Map of statistic values
    Map(HashMap<String, StatValue>),
    /// Timestamp
    Timestamp(Timestamp),
}

impl StatValue {
    /// Create a StatValue from a u64.
    pub fn from_u64(value: u64) -> Self {
        Self::U64(value)
    }
    /// Create a StatValue from a usize.
    pub fn from_usize(value: usize) -> Self {
        Self::U64(value as u64)
    }
    /// Create a StatValue from an i64.
    pub fn from_i64(value: i64) -> Self {
        Self::I64(value)
    }
    /// Create a StatValue from an f64.
    pub fn from_f64(value: f64) -> Self {
        Self::F64(value)
    }
    /// Create a StatValue from a bool.
    pub fn from_bool(value: bool) -> Self {
        Self::Bool(value)
    }
    /// Create a StatValue from a string.
    pub fn from_string(value: impl Into<String>) -> Self {
        Self::String(value.into())
    }
    /// Create a StatValue from an iterator of StatValues.
    pub fn from_list(values: impl IntoIterator<Item = Self>) -> Self {
        Self::List(values.into_iter().collect())
    }
    /// Create a StatValue from an iterator of key-value pairs.
    pub fn from_map(values: impl IntoIterator<Item = (String, Self)>) -> Self {
        Self::Map(values.into_iter().collect())
    }
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
