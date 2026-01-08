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

/// Metrics for B+Tree operations
#[derive(Debug)]
pub struct BTreeMetrics {
    // Read operations
    pub reads: AtomicU64,
    pub read_hits: AtomicU64,
    pub read_misses: AtomicU64,
    
    // Write operations
    pub writes: AtomicU64,
    pub updates: AtomicU64,
    pub deletes: AtomicU64,
    
    // Node operations
    pub node_splits: AtomicU64,
    pub node_merges: AtomicU64,
    pub node_reads: AtomicU64,
    pub node_writes: AtomicU64,
    
    // Tree structure
    pub height_changes: AtomicU64,
    pub rebalances: AtomicU64,
    
    // Scan operations
    pub scans: AtomicU64,
    pub scan_entries_returned: AtomicU64,
}

impl BTreeMetrics {
    pub fn new() -> Self {
        Self {
            reads: AtomicU64::new(0),
            read_hits: AtomicU64::new(0),
            read_misses: AtomicU64::new(0),
            writes: AtomicU64::new(0),
            updates: AtomicU64::new(0),
            deletes: AtomicU64::new(0),
            node_splits: AtomicU64::new(0),
            node_merges: AtomicU64::new(0),
            node_reads: AtomicU64::new(0),
            node_writes: AtomicU64::new(0),
            height_changes: AtomicU64::new(0),
            rebalances: AtomicU64::new(0),
            scans: AtomicU64::new(0),
            scan_entries_returned: AtomicU64::new(0),
        }
    }

    // Read metrics
    pub fn record_read(&self, found: bool) {
        self.reads.fetch_add(1, Ordering::Relaxed);
        if found {
            self.read_hits.fetch_add(1, Ordering::Relaxed);
        } else {
            self.read_misses.fetch_add(1, Ordering::Relaxed);
        }
    }

    pub fn get_reads(&self) -> u64 {
        self.reads.load(Ordering::Relaxed)
    }

    pub fn get_read_hits(&self) -> u64 {
        self.read_hits.load(Ordering::Relaxed)
    }

    pub fn get_read_misses(&self) -> u64 {
        self.read_misses.load(Ordering::Relaxed)
    }

    pub fn get_hit_rate(&self) -> f64 {
        let reads = self.get_reads();
        if reads == 0 {
            return 0.0;
        }
        self.get_read_hits() as f64 / reads as f64
    }

    // Write metrics
    pub fn record_write(&self, is_update: bool) {
        self.writes.fetch_add(1, Ordering::Relaxed);
        if is_update {
            self.updates.fetch_add(1, Ordering::Relaxed);
        }
    }

    pub fn record_delete(&self) {
        self.deletes.fetch_add(1, Ordering::Relaxed);
    }

    pub fn get_writes(&self) -> u64 {
        self.writes.load(Ordering::Relaxed)
    }

    pub fn get_updates(&self) -> u64 {
        self.updates.load(Ordering::Relaxed)
    }

    pub fn get_deletes(&self) -> u64 {
        self.deletes.load(Ordering::Relaxed)
    }

    // Node metrics
    pub fn record_node_split(&self) {
        self.node_splits.fetch_add(1, Ordering::Relaxed);
    }

    pub fn record_node_merge(&self) {
        self.node_merges.fetch_add(1, Ordering::Relaxed);
    }

    pub fn record_node_read(&self) {
        self.node_reads.fetch_add(1, Ordering::Relaxed);
    }

    pub fn record_node_write(&self) {
        self.node_writes.fetch_add(1, Ordering::Relaxed);
    }

    pub fn get_node_splits(&self) -> u64 {
        self.node_splits.load(Ordering::Relaxed)
    }

    pub fn get_node_merges(&self) -> u64 {
        self.node_merges.load(Ordering::Relaxed)
    }

    pub fn get_node_reads(&self) -> u64 {
        self.node_reads.load(Ordering::Relaxed)
    }

    pub fn get_node_writes(&self) -> u64 {
        self.node_writes.load(Ordering::Relaxed)
    }

    // Tree structure metrics
    pub fn record_height_change(&self) {
        self.height_changes.fetch_add(1, Ordering::Relaxed);
    }

    pub fn record_rebalance(&self) {
        self.rebalances.fetch_add(1, Ordering::Relaxed);
    }

    pub fn get_height_changes(&self) -> u64 {
        self.height_changes.load(Ordering::Relaxed)
    }

    pub fn get_rebalances(&self) -> u64 {
        self.rebalances.load(Ordering::Relaxed)
    }

    // Scan metrics
    pub fn record_scan(&self, entries_returned: u64) {
        self.scans.fetch_add(1, Ordering::Relaxed);
        self.scan_entries_returned.fetch_add(entries_returned, Ordering::Relaxed);
    }

    pub fn get_scans(&self) -> u64 {
        self.scans.load(Ordering::Relaxed)
    }

    pub fn get_scan_entries_returned(&self) -> u64 {
        self.scan_entries_returned.load(Ordering::Relaxed)
    }

    pub fn get_avg_scan_size(&self) -> f64 {
        let scans = self.get_scans();
        if scans == 0 {
            return 0.0;
        }
        self.get_scan_entries_returned() as f64 / scans as f64
    }

    // Reset all metrics
    pub fn reset(&self) {
        self.reads.store(0, Ordering::Relaxed);
        self.read_hits.store(0, Ordering::Relaxed);
        self.read_misses.store(0, Ordering::Relaxed);
        self.writes.store(0, Ordering::Relaxed);
        self.updates.store(0, Ordering::Relaxed);
        self.deletes.store(0, Ordering::Relaxed);
        self.node_splits.store(0, Ordering::Relaxed);
        self.node_merges.store(0, Ordering::Relaxed);
        self.node_reads.store(0, Ordering::Relaxed);
        self.node_writes.store(0, Ordering::Relaxed);
        self.height_changes.store(0, Ordering::Relaxed);
        self.rebalances.store(0, Ordering::Relaxed);
        self.scans.store(0, Ordering::Relaxed);
        self.scan_entries_returned.store(0, Ordering::Relaxed);
    }

    // Get a snapshot of all metrics
    pub fn snapshot(&self) -> BTreeMetricsSnapshot {
        BTreeMetricsSnapshot {
            reads: self.get_reads(),
            read_hits: self.get_read_hits(),
            read_misses: self.get_read_misses(),
            writes: self.get_writes(),
            updates: self.get_updates(),
            deletes: self.get_deletes(),
            node_splits: self.get_node_splits(),
            node_merges: self.get_node_merges(),
            node_reads: self.get_node_reads(),
            node_writes: self.get_node_writes(),
            height_changes: self.get_height_changes(),
            rebalances: self.get_rebalances(),
            scans: self.get_scans(),
            scan_entries_returned: self.get_scan_entries_returned(),
        }
    }
}

impl Default for BTreeMetrics {
    fn default() -> Self {
        Self::new()
    }
}

/// Snapshot of B+Tree metrics at a point in time
#[derive(Debug, Clone)]
pub struct BTreeMetricsSnapshot {
    pub reads: u64,
    pub read_hits: u64,
    pub read_misses: u64,
    pub writes: u64,
    pub updates: u64,
    pub deletes: u64,
    pub node_splits: u64,
    pub node_merges: u64,
    pub node_reads: u64,
    pub node_writes: u64,
    pub height_changes: u64,
    pub rebalances: u64,
    pub scans: u64,
    pub scan_entries_returned: u64,
}

impl BTreeMetricsSnapshot {
    pub fn hit_rate(&self) -> f64 {
        if self.reads == 0 {
            return 0.0;
        }
        self.read_hits as f64 / self.reads as f64
    }

    pub fn avg_scan_size(&self) -> f64 {
        if self.scans == 0 {
            return 0.0;
        }
        self.scan_entries_returned as f64 / self.scans as f64
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_metrics() {
        let metrics = BTreeMetrics::new();

        // Test read metrics
        metrics.record_read(true);
        metrics.record_read(true);
        metrics.record_read(false);

        assert_eq!(metrics.get_reads(), 3);
        assert_eq!(metrics.get_read_hits(), 2);
        assert_eq!(metrics.get_read_misses(), 1);
        assert!((metrics.get_hit_rate() - 0.666).abs() < 0.01);

        // Test write metrics
        metrics.record_write(false);
        metrics.record_write(true);
        metrics.record_delete();

        assert_eq!(metrics.get_writes(), 2);
        assert_eq!(metrics.get_updates(), 1);
        assert_eq!(metrics.get_deletes(), 1);

        // Test node metrics
        metrics.record_node_split();
        metrics.record_node_merge();

        assert_eq!(metrics.get_node_splits(), 1);
        assert_eq!(metrics.get_node_merges(), 1);

        // Test scan metrics
        metrics.record_scan(10);
        metrics.record_scan(20);

        assert_eq!(metrics.get_scans(), 2);
        assert_eq!(metrics.get_scan_entries_returned(), 30);
        assert_eq!(metrics.get_avg_scan_size(), 15.0);

        // Test snapshot
        let snapshot = metrics.snapshot();
        assert_eq!(snapshot.reads, 3);
        assert_eq!(snapshot.writes, 2);
        assert_eq!(snapshot.scans, 2);

        // Test reset
        metrics.reset();
        assert_eq!(metrics.get_reads(), 0);
        assert_eq!(metrics.get_writes(), 0);
    }
}

// Made with Bob
