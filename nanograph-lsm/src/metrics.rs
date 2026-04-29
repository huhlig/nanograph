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

use std::sync::Arc;
use std::sync::atomic::{AtomicI64, AtomicU64, AtomicUsize, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// Get current timestamp in seconds since UNIX epoch
fn current_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or(Duration::from_secs(0))
        .as_secs()
}

/// LSM Tree metrics collector
#[derive(Debug, Clone)]
pub struct LSMMetrics {
    inner: Arc<LSMMetricsInner>,
}

#[derive(Debug)]
struct LSMMetricsInner {
    // Write metrics
    total_writes: AtomicU64,
    total_write_bytes: AtomicU64,
    write_latency_sum_ns: AtomicU64,

    // Read metrics
    total_reads: AtomicU64,
    total_read_bytes: AtomicU64,
    read_latency_sum_ns: AtomicU64,

    // Memtable metrics
    memtable_hits: AtomicU64,
    memtable_size: AtomicUsize,

    // SSTable metrics
    sstable_reads: AtomicU64,
    sstable_hits: AtomicU64,

    // Bloom filter metrics
    bloom_filter_checks: AtomicU64,
    bloom_filter_true_positives: AtomicU64,
    bloom_filter_false_positives: AtomicU64,

    // Block cache metrics (when implemented)
    block_cache_hits: AtomicU64,
    block_cache_misses: AtomicU64,

    // Flush metrics
    total_flushes: AtomicU64,
    flush_duration_sum_ns: AtomicU64,
    bytes_flushed: AtomicU64,

    // Compaction metrics
    total_compactions: AtomicU64,
    compaction_duration_sum_ns: AtomicU64,
    bytes_compacted_in: AtomicU64,
    bytes_compacted_out: AtomicU64,

    // Level metrics
    level_sizes: [AtomicU64; 7],
    level_file_counts: [AtomicUsize; 7],

    // Timestamp tracking
    creation_time: AtomicU64,
    last_write_time: AtomicU64,
    last_read_time: AtomicU64,
    last_flush_time: AtomicU64,
    last_compaction_time: AtomicU64,

    // Read amplification tracking
    total_sstable_reads_for_gets: AtomicU64,

    // Transaction metrics
    active_transactions: AtomicUsize,
    min_active_snapshot_seq: AtomicI64,
}

impl LSMMetrics {
    pub fn new() -> Self {
        let now = current_timestamp();
        Self {
            inner: Arc::new(LSMMetricsInner {
                total_writes: AtomicU64::new(0),
                total_write_bytes: AtomicU64::new(0),
                write_latency_sum_ns: AtomicU64::new(0),
                total_reads: AtomicU64::new(0),
                total_read_bytes: AtomicU64::new(0),
                read_latency_sum_ns: AtomicU64::new(0),
                memtable_hits: AtomicU64::new(0),
                memtable_size: AtomicUsize::new(0),
                sstable_reads: AtomicU64::new(0),
                sstable_hits: AtomicU64::new(0),
                bloom_filter_checks: AtomicU64::new(0),
                bloom_filter_true_positives: AtomicU64::new(0),
                bloom_filter_false_positives: AtomicU64::new(0),
                block_cache_hits: AtomicU64::new(0),
                block_cache_misses: AtomicU64::new(0),
                total_flushes: AtomicU64::new(0),
                flush_duration_sum_ns: AtomicU64::new(0),
                bytes_flushed: AtomicU64::new(0),
                total_compactions: AtomicU64::new(0),
                compaction_duration_sum_ns: AtomicU64::new(0),
                bytes_compacted_in: AtomicU64::new(0),
                bytes_compacted_out: AtomicU64::new(0),
                level_sizes: Default::default(),
                level_file_counts: Default::default(),
                creation_time: AtomicU64::new(now),
                last_write_time: AtomicU64::new(0),
                last_read_time: AtomicU64::new(0),
                last_flush_time: AtomicU64::new(0),
                last_compaction_time: AtomicU64::new(0),
                total_sstable_reads_for_gets: AtomicU64::new(0),
                active_transactions: AtomicUsize::new(0),
                min_active_snapshot_seq: AtomicI64::new(i64::MAX),
            }),
        }
    }

    // Write metrics
    pub fn record_write(&self, bytes: usize, duration: Duration) {
        self.inner.total_writes.fetch_add(1, Ordering::Relaxed);
        self.inner
            .total_write_bytes
            .fetch_add(bytes as u64, Ordering::Relaxed);
        self.inner
            .write_latency_sum_ns
            .fetch_add(duration.as_nanos() as u64, Ordering::Relaxed);
        self.inner
            .last_write_time
            .store(current_timestamp(), Ordering::Relaxed);
    }

    pub fn total_writes(&self) -> u64 {
        self.inner.total_writes.load(Ordering::Relaxed)
    }

    pub fn avg_write_latency(&self) -> Duration {
        let total = self.inner.total_writes.load(Ordering::Relaxed);
        if total == 0 {
            return Duration::from_nanos(0);
        }
        let sum = self.inner.write_latency_sum_ns.load(Ordering::Relaxed);
        Duration::from_nanos(sum / total)
    }

    pub fn write_throughput_bytes_per_sec(&self, elapsed: Duration) -> f64 {
        let bytes = self.inner.total_write_bytes.load(Ordering::Relaxed);
        bytes as f64 / elapsed.as_secs_f64()
    }

    // Read metrics
    pub fn record_read(&self, bytes: usize, duration: Duration) {
        self.inner.total_reads.fetch_add(1, Ordering::Relaxed);
        self.inner
            .total_read_bytes
            .fetch_add(bytes as u64, Ordering::Relaxed);
        self.inner
            .read_latency_sum_ns
            .fetch_add(duration.as_nanos() as u64, Ordering::Relaxed);
        self.inner
            .last_read_time
            .store(current_timestamp(), Ordering::Relaxed);
    }

    /// Record SSTable reads for a get operation (for read amplification calculation)
    pub fn record_sstable_reads_for_get(&self, count: u64) {
        self.inner
            .total_sstable_reads_for_gets
            .fetch_add(count, Ordering::Relaxed);
    }

    pub fn total_reads(&self) -> u64 {
        self.inner.total_reads.load(Ordering::Relaxed)
    }

    pub fn avg_read_latency(&self) -> Duration {
        let total = self.inner.total_reads.load(Ordering::Relaxed);
        if total == 0 {
            return Duration::from_nanos(0);
        }
        let sum = self.inner.read_latency_sum_ns.load(Ordering::Relaxed);
        Duration::from_nanos(sum / total)
    }

    // Memtable metrics
    pub fn record_memtable_hit(&self) {
        self.inner.memtable_hits.fetch_add(1, Ordering::Relaxed);
    }

    pub fn set_memtable_size(&self, size: usize) {
        self.inner.memtable_size.store(size, Ordering::Relaxed);
    }

    pub fn memtable_hit_rate(&self) -> f64 {
        let total = self.inner.total_reads.load(Ordering::Relaxed);
        if total == 0 {
            return 0.0;
        }
        let hits = self.inner.memtable_hits.load(Ordering::Relaxed);
        hits as f64 / total as f64
    }

    // SSTable metrics
    pub fn record_sstable_read(&self, hit: bool) {
        self.inner.sstable_reads.fetch_add(1, Ordering::Relaxed);
        if hit {
            self.inner.sstable_hits.fetch_add(1, Ordering::Relaxed);
        }
    }

    pub fn sstable_hit_rate(&self) -> f64 {
        let total = self.inner.sstable_reads.load(Ordering::Relaxed);
        if total == 0 {
            return 0.0;
        }
        let hits = self.inner.sstable_hits.load(Ordering::Relaxed);
        hits as f64 / total as f64
    }

    // Bloom filter metrics
    pub fn record_bloom_filter_check(&self, result: BloomFilterResult) {
        self.inner
            .bloom_filter_checks
            .fetch_add(1, Ordering::Relaxed);
        match result {
            BloomFilterResult::TruePositive => {
                self.inner
                    .bloom_filter_true_positives
                    .fetch_add(1, Ordering::Relaxed);
            }
            BloomFilterResult::FalsePositive => {
                self.inner
                    .bloom_filter_false_positives
                    .fetch_add(1, Ordering::Relaxed);
            }
            BloomFilterResult::TrueNegative => {}
        }
    }

    pub fn bloom_filter_false_positive_rate(&self) -> f64 {
        let checks = self.inner.bloom_filter_checks.load(Ordering::Relaxed);
        if checks == 0 {
            return 0.0;
        }
        let false_positives = self
            .inner
            .bloom_filter_false_positives
            .load(Ordering::Relaxed);
        false_positives as f64 / checks as f64
    }

    // Block cache metrics
    pub fn record_block_cache_access(&self, hit: bool) {
        if hit {
            self.inner.block_cache_hits.fetch_add(1, Ordering::Relaxed);
        } else {
            self.inner
                .block_cache_misses
                .fetch_add(1, Ordering::Relaxed);
        }
    }

    pub fn block_cache_hit_rate(&self) -> f64 {
        let hits = self.inner.block_cache_hits.load(Ordering::Relaxed);
        let misses = self.inner.block_cache_misses.load(Ordering::Relaxed);
        let total = hits + misses;
        if total == 0 {
            return 0.0;
        }
        hits as f64 / total as f64
    }

    // Flush metrics
    pub fn record_flush(&self, bytes: u64, duration: Duration) {
        self.inner.total_flushes.fetch_add(1, Ordering::Relaxed);
        self.inner.bytes_flushed.fetch_add(bytes, Ordering::Relaxed);
        self.inner
            .flush_duration_sum_ns
            .fetch_add(duration.as_nanos() as u64, Ordering::Relaxed);
        self.inner
            .last_flush_time
            .store(current_timestamp(), Ordering::Relaxed);
    }

    pub fn total_flushes(&self) -> u64 {
        self.inner.total_flushes.load(Ordering::Relaxed)
    }

    pub fn avg_flush_duration(&self) -> Duration {
        let total = self.inner.total_flushes.load(Ordering::Relaxed);
        if total == 0 {
            return Duration::from_nanos(0);
        }
        let sum = self.inner.flush_duration_sum_ns.load(Ordering::Relaxed);
        Duration::from_nanos(sum / total)
    }

    // Compaction metrics
    pub fn record_compaction(&self, bytes_in: u64, bytes_out: u64, duration: Duration) {
        self.inner.total_compactions.fetch_add(1, Ordering::Relaxed);
        self.inner
            .bytes_compacted_in
            .fetch_add(bytes_in, Ordering::Relaxed);
        self.inner
            .bytes_compacted_out
            .fetch_add(bytes_out, Ordering::Relaxed);
        self.inner
            .compaction_duration_sum_ns
            .fetch_add(duration.as_nanos() as u64, Ordering::Relaxed);
        self.inner
            .last_compaction_time
            .store(current_timestamp(), Ordering::Relaxed);
    }

    pub fn total_compactions(&self) -> u64 {
        self.inner.total_compactions.load(Ordering::Relaxed)
    }

    pub fn write_amplification(&self) -> f64 {
        let bytes_written = self.inner.total_write_bytes.load(Ordering::Relaxed);
        if bytes_written == 0 {
            return 0.0;
        }
        let bytes_flushed = self.inner.bytes_flushed.load(Ordering::Relaxed);
        let bytes_compacted = self.inner.bytes_compacted_out.load(Ordering::Relaxed);
        let total_written = bytes_flushed + bytes_compacted;
        total_written as f64 / bytes_written as f64
    }

    /// Calculate read amplification (average number of SSTables read per get operation)
    pub fn read_amplification(&self) -> f64 {
        let total_reads = self.inner.total_reads.load(Ordering::Relaxed);
        if total_reads == 0 {
            return 0.0;
        }
        let sstable_reads = self
            .inner
            .total_sstable_reads_for_gets
            .load(Ordering::Relaxed);
        sstable_reads as f64 / total_reads as f64
    }

    pub fn space_amplification(&self) -> f64 {
        let total_size: u64 = (0..7)
            .map(|i| self.inner.level_sizes[i].load(Ordering::Relaxed))
            .sum();
        let data_size = self.inner.total_write_bytes.load(Ordering::Relaxed);
        if data_size == 0 {
            return 0.0;
        }
        total_size as f64 / data_size as f64
    }

    // Timestamp accessors
    pub fn creation_time(&self) -> u64 {
        self.inner.creation_time.load(Ordering::Relaxed)
    }

    pub fn last_write_time(&self) -> u64 {
        self.inner.last_write_time.load(Ordering::Relaxed)
    }

    pub fn last_read_time(&self) -> u64 {
        self.inner.last_read_time.load(Ordering::Relaxed)
    }

    pub fn last_flush_time(&self) -> u64 {
        self.inner.last_flush_time.load(Ordering::Relaxed)
    }

    pub fn last_compaction_time(&self) -> u64 {
        self.inner.last_compaction_time.load(Ordering::Relaxed)
    }

    pub fn uptime_seconds(&self) -> u64 {
        current_timestamp().saturating_sub(self.creation_time())
    }

    // Level metrics
    pub fn set_level_size(&self, level: usize, size: u64) {
        if level < 7 {
            self.inner.level_sizes[level].store(size, Ordering::Relaxed);
        }
    }

    pub fn set_level_file_count(&self, level: usize, count: usize) {
        if level < 7 {
            self.inner.level_file_counts[level].store(count, Ordering::Relaxed);
        }
    }

    pub fn get_level_size(&self, level: usize) -> u64 {
        if level < 7 {
            self.inner.level_sizes[level].load(Ordering::Relaxed)
        } else {
            0
        }
    }

    pub fn get_level_file_count(&self, level: usize) -> usize {
        if level < 7 {
            self.inner.level_file_counts[level].load(Ordering::Relaxed)
        } else {
            0
        }
    }

    // Transaction metrics
    
    /// Set the number of active transactions
    pub fn set_active_transactions(&self, count: usize) {
        self.inner.active_transactions.store(count, Ordering::Relaxed);
    }

    /// Get the number of active transactions
    pub fn active_transactions(&self) -> usize {
        self.inner.active_transactions.load(Ordering::Relaxed)
    }

    /// Set the minimum active snapshot sequence number (GC watermark)
    ///
    /// This represents the oldest snapshot timestamp among all active transactions.
    /// Data with timestamps older than this can be safely garbage collected during compaction.
    ///
    /// Use i64::MAX when there are no active transactions.
    pub fn set_min_active_snapshot_seq(&self, seq: i64) {
        self.inner.min_active_snapshot_seq.store(seq, Ordering::Relaxed);
    }

    /// Get the minimum active snapshot sequence number (GC watermark)
    ///
    /// Returns i64::MAX if there are no active transactions, indicating all data
    /// can potentially be garbage collected (subject to other retention policies).
    pub fn min_active_snapshot_seq(&self) -> i64 {
        self.inner.min_active_snapshot_seq.load(Ordering::Relaxed)
    }

    /// Get a snapshot of all metrics
    pub fn snapshot(&self) -> MetricsSnapshot {
        MetricsSnapshot {
            total_writes: self.total_writes(),
            total_reads: self.total_reads(),
            avg_write_latency: self.avg_write_latency(),
            avg_read_latency: self.avg_read_latency(),
            memtable_hit_rate: self.memtable_hit_rate(),
            sstable_hit_rate: self.sstable_hit_rate(),
            bloom_filter_fpr: self.bloom_filter_false_positive_rate(),
            block_cache_hit_rate: self.block_cache_hit_rate(),
            total_flushes: self.total_flushes(),
            total_compactions: self.total_compactions(),
            write_amplification: self.write_amplification(),
            read_amplification: self.read_amplification(),
            space_amplification: self.space_amplification(),
            level_sizes: (0..7).map(|i| self.get_level_size(i)).collect(),
            level_file_counts: (0..7).map(|i| self.get_level_file_count(i)).collect(),
            creation_time: self.creation_time(),
            last_write_time: self.last_write_time(),
            last_read_time: self.last_read_time(),
            last_flush_time: self.last_flush_time(),
            last_compaction_time: self.last_compaction_time(),
            uptime_seconds: self.uptime_seconds(),
            active_transactions: self.active_transactions(),
            min_active_snapshot_seq: self.min_active_snapshot_seq(),
        }
    }
}

impl Default for LSMMetrics {
    fn default() -> Self {
        Self::new()
    }
}

/// Bloom filter check result
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BloomFilterResult {
    TruePositive,  // Bloom said yes, key exists
    FalsePositive, // Bloom said yes, key doesn't exist
    TrueNegative,  // Bloom said no, key doesn't exist
}

/// Snapshot of metrics at a point in time
#[derive(Debug, Clone)]
pub struct MetricsSnapshot {
    pub total_writes: u64,
    pub total_reads: u64,
    pub avg_write_latency: Duration,
    pub avg_read_latency: Duration,
    pub memtable_hit_rate: f64,
    pub sstable_hit_rate: f64,
    pub bloom_filter_fpr: f64,
    pub block_cache_hit_rate: f64,
    pub total_flushes: u64,
    pub total_compactions: u64,
    pub write_amplification: f64,
    pub read_amplification: f64,
    pub space_amplification: f64,
    pub level_sizes: Vec<u64>,
    pub level_file_counts: Vec<usize>,
    pub creation_time: u64,
    pub last_write_time: u64,
    pub last_read_time: u64,
    pub last_flush_time: u64,
    pub last_compaction_time: u64,
    pub uptime_seconds: u64,
    pub active_transactions: usize,
    pub min_active_snapshot_seq: i64,
}

impl MetricsSnapshot {
    pub fn print_summary(&self) {
        println!("=== LSM Tree Metrics ===");
        println!("Uptime: {} seconds", self.uptime_seconds);
        println!();
        println!("Operations:");
        println!("  Total Writes: {}", self.total_writes);
        println!("  Total Reads: {}", self.total_reads);
        println!("  Avg Write Latency: {:?}", self.avg_write_latency);
        println!("  Avg Read Latency: {:?}", self.avg_read_latency);
        println!();
        println!("Transactions:");
        println!("  Active Transactions: {}", self.active_transactions);
        if self.min_active_snapshot_seq == i64::MAX {
            println!("  GC Watermark: None (no active transactions)");
        } else {
            println!("  GC Watermark (min snapshot seq): {}", self.min_active_snapshot_seq);
        }
        println!();
        println!("Hit Rates:");
        println!("  MemTable: {:.2}%", self.memtable_hit_rate * 100.0);
        println!("  SSTable: {:.2}%", self.sstable_hit_rate * 100.0);
        println!("  Block Cache: {:.2}%", self.block_cache_hit_rate * 100.0);
        println!("  Bloom Filter FPR: {:.2}%", self.bloom_filter_fpr * 100.0);
        println!();
        println!("Amplification Factors:");
        println!("  Write Amplification: {:.2}x", self.write_amplification);
        println!("  Read Amplification: {:.2}x", self.read_amplification);
        println!("  Space Amplification: {:.2}x", self.space_amplification);
        println!();
        println!("Maintenance:");
        println!("  Total Flushes: {}", self.total_flushes);
        println!("  Total Compactions: {}", self.total_compactions);
        if self.last_flush_time > 0 {
            println!(
                "  Last Flush: {} seconds ago",
                current_timestamp().saturating_sub(self.last_flush_time)
            );
        }
        if self.last_compaction_time > 0 {
            println!(
                "  Last Compaction: {} seconds ago",
                current_timestamp().saturating_sub(self.last_compaction_time)
            );
        }
        println!();
        println!("Levels:");
        for (i, (size, count)) in self
            .level_sizes
            .iter()
            .zip(&self.level_file_counts)
            .enumerate()
        {
            if *count > 0 {
                println!(
                    "  L{}: {} files, {} bytes ({:.2} MB)",
                    i,
                    count,
                    size,
                    *size as f64 / 1024.0 / 1024.0
                );
            }
        }
    }
}

/// Example LSM-specific metric names
pub mod consts {
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

    /// Number of active transactions (gauge)
    pub const ACTIVE_TRANSACTIONS: &str = "nanograph.storage.lsm.active_transactions";

    /// Minimum active snapshot sequence number - GC watermark (gauge)
    /// Data with timestamps older than this can be safely garbage collected during compaction.
    /// Value is i64::MAX when there are no active transactions.
    pub const MIN_ACTIVE_SNAPSHOT_SEQ: &str = "nanograph.storage.lsm.min_active_snapshot_seq";
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_write_metrics() {
        let metrics = LSMMetrics::new();

        metrics.record_write(100, Duration::from_micros(10));
        metrics.record_write(200, Duration::from_micros(20));

        assert_eq!(metrics.total_writes(), 2);
        assert_eq!(metrics.avg_write_latency(), Duration::from_micros(15));
    }

    #[test]
    fn test_hit_rates() {
        let metrics = LSMMetrics::new();

        metrics.record_read(100, Duration::from_micros(10));
        metrics.record_memtable_hit();
        metrics.record_read(100, Duration::from_micros(10));

        assert_eq!(metrics.memtable_hit_rate(), 0.5);
    }

    #[test]
    fn test_amplification() {
        let metrics = LSMMetrics::new();

        metrics.record_write(1000, Duration::from_micros(10));
        metrics.record_flush(2000, Duration::from_millis(10));
        metrics.record_compaction(2000, 3000, Duration::from_millis(100));

        assert_eq!(metrics.write_amplification(), 5.0); // (2000 + 3000) / 1000
    }

    #[test]
    fn test_snapshot() {
        let metrics = LSMMetrics::new();

        metrics.record_write(100, Duration::from_micros(10));
        metrics.set_level_size(0, 1000);
        metrics.set_level_file_count(0, 5);

        let snapshot = metrics.snapshot();
        assert_eq!(snapshot.total_writes, 1);
        assert_eq!(snapshot.level_sizes[0], 1000);
        assert_eq!(snapshot.level_file_counts[0], 5);
    }
}
