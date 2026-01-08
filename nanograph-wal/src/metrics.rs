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

//! Metrics collection for the Write-Ahead Log.
//!
//! This module provides standardized metrics for tracking WAL operations,
//! performance, and resource usage.

use metrics::{counter, gauge, histogram};

/// Records a WAL operation counter.
///
/// # Arguments
/// * `operation` - The operation name (e.g., "append", "read", "flush")
/// * `shard_id` - The shard identifier
/// * `success` - Whether the operation succeeded
#[inline]
pub fn record_operation(operation: &'static str, shard_id: u64, success: bool) {
    let status = if success { "success" } else { "failure" };
    counter!(
        "nanograph_wal_operations_total",
        "operation" => operation,
        "shard_id" => shard_id.to_string(),
        "status" => status
    )
    .increment(1);
}

/// Records bytes written to the WAL.
///
/// # Arguments
/// * `shard_id` - The shard identifier
/// * `bytes` - Number of bytes written
#[inline]
pub fn record_bytes_written(shard_id: u64, bytes: u64) {
    counter!(
        "nanograph_wal_bytes_written_total",
        "shard_id" => shard_id.to_string()
    )
    .increment(bytes);
}

/// Records bytes read from the WAL.
///
/// # Arguments
/// * `shard_id` - The shard identifier
/// * `bytes` - Number of bytes read
#[inline]
pub fn record_bytes_read(shard_id: u64, bytes: u64) {
    counter!(
        "nanograph_wal_bytes_read_total",
        "shard_id" => shard_id.to_string()
    )
    .increment(bytes);
}

/// Records the duration of a WAL operation.
///
/// # Arguments
/// * `operation` - The operation name
/// * `shard_id` - The shard identifier
/// * `duration_micros` - Duration in microseconds
#[inline]
pub fn record_operation_duration(operation: &'static str, shard_id: u64, duration_micros: u64) {
    histogram!(
        "nanograph_wal_operation_duration_microseconds",
        "operation" => operation,
        "shard_id" => shard_id.to_string()
    )
    .record(duration_micros as f64);
}

/// Records the current number of active segments.
///
/// # Arguments
/// * `shard_id` - The shard identifier
/// * `count` - Number of active segments
#[inline]
pub fn record_active_segments(shard_id: u64, count: usize) {
    gauge!(
        "nanograph_wal_active_segments",
        "shard_id" => shard_id.to_string()
    )
    .set(count as f64);
}

/// Records the current WAL size in bytes.
///
/// # Arguments
/// * `shard_id` - The shard identifier
/// * `size` - Total WAL size in bytes
#[inline]
pub fn record_wal_size(shard_id: u64, size: u64) {
    gauge!(
        "nanograph_wal_size_bytes",
        "shard_id" => shard_id.to_string()
    )
    .set(size as f64);
}

/// Records the current Log Sequence Number (LSN).
///
/// # Arguments
/// * `shard_id` - The shard identifier
/// * `lsn` - Current LSN value
#[inline]
pub fn record_current_lsn(shard_id: u64, lsn: u128) {
    gauge!(
        "nanograph_wal_current_lsn",
        "shard_id" => shard_id.to_string()
    )
    .set(lsn as f64);
}

/// Records a flush operation.
///
/// # Arguments
/// * `shard_id` - The shard identifier
/// * `success` - Whether the flush succeeded
#[inline]
pub fn record_flush(shard_id: u64, success: bool) {
    let status = if success { "success" } else { "failure" };
    counter!(
        "nanograph_wal_flushes_total",
        "shard_id" => shard_id.to_string(),
        "status" => status
    )
    .increment(1);
}

/// Records a sync operation.
///
/// # Arguments
/// * `shard_id` - The shard identifier
/// * `success` - Whether the sync succeeded
#[inline]
pub fn record_sync(shard_id: u64, success: bool) {
    let status = if success { "success" } else { "failure" };
    counter!(
        "nanograph_wal_syncs_total",
        "shard_id" => shard_id.to_string(),
        "status" => status
    )
    .increment(1);
}

/// Records a segment rotation.
///
/// # Arguments
/// * `shard_id` - The shard identifier
/// * `success` - Whether the rotation succeeded
#[inline]
pub fn record_segment_rotation(shard_id: u64, success: bool) {
    let status = if success { "success" } else { "failure" };
    counter!(
        "nanograph_wal_segment_rotations_total",
        "shard_id" => shard_id.to_string(),
        "status" => status
    )
    .increment(1);
}

/// Records the number of records in a segment.
///
/// # Arguments
/// * `shard_id` - The shard identifier
/// * `count` - Number of records
#[inline]
pub fn record_segment_records(shard_id: u64, count: u64) {
    histogram!(
        "nanograph_wal_segment_records",
        "shard_id" => shard_id.to_string()
    )
    .record(count as f64);
}

/// Records the size of a segment.
///
/// # Arguments
/// * `shard_id` - The shard identifier
/// * `size` - Segment size in bytes
#[inline]
pub fn record_segment_size(shard_id: u64, size: u64) {
    histogram!(
        "nanograph_wal_segment_size_bytes",
        "shard_id" => shard_id.to_string()
    )
    .record(size as f64);
}

/// Records a checkpoint operation.
///
/// # Arguments
/// * `shard_id` - The shard identifier
/// * `success` - Whether the checkpoint succeeded
#[inline]
pub fn record_checkpoint(shard_id: u64, success: bool) {
    let status = if success { "success" } else { "failure" };
    counter!(
        "nanograph_wal_checkpoints_total",
        "shard_id" => shard_id.to_string(),
        "status" => status
    )
    .increment(1);
}

/// Records the number of records appended.
///
/// # Arguments
/// * `shard_id` - The shard identifier
/// * `count` - Number of records appended
#[inline]
pub fn record_records_appended(shard_id: u64, count: u64) {
    counter!(
        "nanograph_wal_records_appended_total",
        "shard_id" => shard_id.to_string()
    )
    .increment(count);
}

/// Records the number of records read.
///
/// # Arguments
/// * `shard_id` - The shard identifier
/// * `count` - Number of records read
#[inline]
pub fn record_records_read(shard_id: u64, count: u64) {
    counter!(
        "nanograph_wal_records_read_total",
        "shard_id" => shard_id.to_string()
    )
    .increment(count);
}

/// Records record size distribution.
///
/// # Arguments
/// * `shard_id` - The shard identifier
/// * `size` - Record size in bytes
#[inline]
pub fn record_record_size(shard_id: u64, size: usize) {
    histogram!(
        "nanograph_wal_record_size_bytes",
        "shard_id" => shard_id.to_string()
    )
    .record(size as f64);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_metrics_recording() {
        // These tests just ensure the metrics functions don't panic
        record_operation("append", 0, true);
        record_bytes_written(0, 1024);
        record_bytes_read(0, 2048);
        record_operation_duration("flush", 0, 150);
        record_active_segments(0, 5);
        record_wal_size(0, 1024 * 1024);
        record_current_lsn(0, 12345);
        record_flush(0, true);
        record_sync(0, true);
        record_segment_rotation(0, true);
        record_segment_records(0, 100);
        record_segment_size(0, 4096);
        record_checkpoint(0, true);
        record_records_appended(0, 10);
        record_records_read(0, 5);
        record_record_size(0, 256);
    }
}
