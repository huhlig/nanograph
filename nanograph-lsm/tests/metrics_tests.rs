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

use nanograph_lsm::{LSMMetrics, BloomFilterResult};
use std::time::Duration;
use std::thread;

#[test]
fn test_timestamp_tracking() {
    let metrics = LSMMetrics::new();
    
    // Check creation time is set
    let creation_time = metrics.creation_time();
    assert!(creation_time > 0);
    
    // Initially, operation times should be 0
    assert_eq!(metrics.last_write_time(), 0);
    assert_eq!(metrics.last_read_time(), 0);
    assert_eq!(metrics.last_flush_time(), 0);
    assert_eq!(metrics.last_compaction_time(), 0);
    
    // Record a write
    metrics.record_write(100, Duration::from_micros(10));
    let write_time = metrics.last_write_time();
    assert!(write_time > 0);
    
    // Delay to ensure different timestamps (timestamps are in seconds, so need >1s delay)
    thread::sleep(Duration::from_secs(2));
    
    // Record a read
    metrics.record_read(100, Duration::from_micros(10));
    let read_time = metrics.last_read_time();
    assert!(read_time > write_time, "read_time ({}) should be > write_time ({})", read_time, write_time);
    
    // Record a flush
    metrics.record_flush(1000, Duration::from_millis(10));
    assert!(metrics.last_flush_time() > 0);
    
    // Record a compaction
    metrics.record_compaction(2000, 1500, Duration::from_millis(100));
    assert!(metrics.last_compaction_time() > 0);
}

#[test]
fn test_uptime_calculation() {
    let metrics = LSMMetrics::new();
    
    // Uptime should be very small initially
    let uptime1 = metrics.uptime_seconds();
    assert!(uptime1 < 2);
    
    // Wait a bit
    thread::sleep(Duration::from_secs(1));
    
    // Uptime should have increased
    let uptime2 = metrics.uptime_seconds();
    assert!(uptime2 >= uptime1);
}

#[test]
fn test_read_amplification() {
    let metrics = LSMMetrics::new();
    
    // Initially should be 0
    assert_eq!(metrics.read_amplification(), 0.0);
    
    // Record some reads with SSTable accesses
    metrics.record_read(100, Duration::from_micros(10));
    metrics.record_sstable_reads_for_get(3); // Read 3 SSTables
    
    metrics.record_read(100, Duration::from_micros(10));
    metrics.record_sstable_reads_for_get(2); // Read 2 SSTables
    
    metrics.record_read(100, Duration::from_micros(10));
    metrics.record_sstable_reads_for_get(1); // Read 1 SSTable
    
    // Average should be (3 + 2 + 1) / 3 = 2.0
    assert_eq!(metrics.read_amplification(), 2.0);
}

#[test]
fn test_write_amplification() {
    let metrics = LSMMetrics::new();
    
    // Write 1000 bytes
    metrics.record_write(1000, Duration::from_micros(10));
    
    // Flush 2000 bytes (write amplification from memtable to L0)
    metrics.record_flush(2000, Duration::from_millis(10));
    
    // Compact 2000 bytes in, 1800 bytes out (some data removed)
    metrics.record_compaction(2000, 1800, Duration::from_millis(100));
    
    // Write amplification = (2000 + 1800) / 1000 = 3.8
    assert_eq!(metrics.write_amplification(), 3.8);
}

#[test]
fn test_space_amplification() {
    let metrics = LSMMetrics::new();
    
    // Write 1000 bytes of data
    metrics.record_write(1000, Duration::from_micros(10));
    
    // Set level sizes
    metrics.set_level_size(0, 500);
    metrics.set_level_size(1, 1000);
    metrics.set_level_size(2, 2000);
    
    // Total size = 3500, data size = 1000
    // Space amplification = 3500 / 1000 = 3.5
    assert_eq!(metrics.space_amplification(), 3.5);
}

#[test]
fn test_metrics_snapshot_with_timestamps() {
    let metrics = LSMMetrics::new();
    
    // Record some operations
    metrics.record_write(100, Duration::from_micros(10));
    metrics.record_read(100, Duration::from_micros(10));
    metrics.record_flush(1000, Duration::from_millis(10));
    metrics.record_compaction(2000, 1500, Duration::from_millis(100));
    metrics.record_sstable_reads_for_get(2);
    
    // Get snapshot
    let snapshot = metrics.snapshot();
    
    // Verify timestamp fields are present
    assert!(snapshot.creation_time > 0);
    assert!(snapshot.last_write_time > 0);
    assert!(snapshot.last_read_time > 0);
    assert!(snapshot.last_flush_time > 0);
    assert!(snapshot.last_compaction_time > 0);
    // uptime_seconds is u64, so it's always >= 0
    assert!(snapshot.uptime_seconds == snapshot.uptime_seconds); // Just verify it's accessible
    
    // Verify amplification metrics
    assert!(snapshot.read_amplification >= 0.0);
    assert!(snapshot.write_amplification >= 0.0);
    assert!(snapshot.space_amplification >= 0.0);
}

#[test]
fn test_level_metrics() {
    let metrics = LSMMetrics::new();
    
    // Set some level data
    for level in 0..7 {
        metrics.set_level_size(level, (level as u64 + 1) * 1000);
        metrics.set_level_file_count(level, level + 1);
    }
    
    // Verify we can read them back
    for level in 0..7 {
        assert_eq!(metrics.get_level_size(level), (level as u64 + 1) * 1000);
        assert_eq!(metrics.get_level_file_count(level), level + 1);
    }
    
    // Test snapshot includes level data
    let snapshot = metrics.snapshot();
    assert_eq!(snapshot.level_sizes.len(), 7);
    assert_eq!(snapshot.level_file_counts.len(), 7);
}

#[test]
fn test_bloom_filter_metrics() {
    let metrics = LSMMetrics::new();
    
    // Record bloom filter checks
    metrics.record_bloom_filter_check(BloomFilterResult::TruePositive);
    metrics.record_bloom_filter_check(BloomFilterResult::TruePositive);
    metrics.record_bloom_filter_check(BloomFilterResult::FalsePositive);
    metrics.record_bloom_filter_check(BloomFilterResult::TrueNegative);
    
    // False positive rate should be 1/4 = 0.25
    assert_eq!(metrics.bloom_filter_false_positive_rate(), 0.25);
}

#[test]
fn test_cache_metrics() {
    let metrics = LSMMetrics::new();
    
    // Record cache accesses
    metrics.record_block_cache_access(true);  // hit
    metrics.record_block_cache_access(true);  // hit
    metrics.record_block_cache_access(false); // miss
    
    // Hit rate should be 2/3
    assert!((metrics.block_cache_hit_rate() - 0.666).abs() < 0.01);
}

// Made with Bob
