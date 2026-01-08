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

use std::sync::atomic::{AtomicU64, Ordering};

pub mod consts {
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

/// Metrics for ART operations
#[derive(Debug)]
pub struct ArtMetrics {
    pub total_reads: AtomicU64,
    pub total_writes: AtomicU64,
    pub total_deletes: AtomicU64,
    pub cache_hits: AtomicU64,
    pub cache_misses: AtomicU64,
}

impl ArtMetrics {
    pub fn new() -> Self {
        Self {
            total_reads: AtomicU64::new(0),
            total_writes: AtomicU64::new(0),
            total_deletes: AtomicU64::new(0),
            cache_hits: AtomicU64::new(0),
            cache_misses: AtomicU64::new(0),
        }
    }

    pub fn record_read(&self, hit: bool) {
        self.total_reads.fetch_add(1, Ordering::Relaxed);
        if hit {
            self.cache_hits.fetch_add(1, Ordering::Relaxed);
        } else {
            self.cache_misses.fetch_add(1, Ordering::Relaxed);
        }
    }

    pub fn record_write(&self, _is_update: bool) {
        self.total_writes.fetch_add(1, Ordering::Relaxed);
    }

    pub fn record_delete(&self) {
        self.total_deletes.fetch_add(1, Ordering::Relaxed);
    }

    pub fn record_scan(&self, _count: usize) {
        // Could track scan operations if needed
    }

    pub fn snapshot(&self) -> MetricsSnapshot {
        MetricsSnapshot {
            total_reads: self.total_reads.load(Ordering::Relaxed),
            total_writes: self.total_writes.load(Ordering::Relaxed),
            total_deletes: self.total_deletes.load(Ordering::Relaxed),
            cache_hits: self.cache_hits.load(Ordering::Relaxed),
            cache_misses: self.cache_misses.load(Ordering::Relaxed),
        }
    }
}

impl Default for ArtMetrics {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone)]
pub struct MetricsSnapshot {
    pub total_reads: u64,
    pub total_writes: u64,
    pub total_deletes: u64,
    pub cache_hits: u64,
    pub cache_misses: u64,
}
