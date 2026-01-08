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

//! Metrics collection for the Virtual File System.
//!
//! This module provides standardized metrics for tracking filesystem operations,
//! performance, and resource usage across all VFS implementations.

use metrics::{counter, gauge, histogram};

/// Records a filesystem operation counter.
///
/// # Arguments
/// * `operation` - The operation name (e.g., "read", "write", "create")
/// * `filesystem` - The filesystem type (e.g., "memory", "local", "overlay")
/// * `success` - Whether the operation succeeded
#[inline]
pub fn record_operation(operation: &'static str, filesystem: &'static str, success: bool) {
    let status = if success { "success" } else { "failure" };
    counter!(
        "nanograph_vfs_operations_total",
        "operation" => operation,
        "filesystem" => filesystem,
        "status" => status
    )
    .increment(1);
}

/// Records bytes read from a file.
///
/// # Arguments
/// * `filesystem` - The filesystem type
/// * `bytes` - Number of bytes read
#[inline]
pub fn record_bytes_read(filesystem: &'static str, bytes: u64) {
    counter!(
        "nanograph_vfs_bytes_read_total",
        "filesystem" => filesystem
    )
    .increment(bytes);
}

/// Records bytes written to a file.
///
/// # Arguments
/// * `filesystem` - The filesystem type
/// * `bytes` - Number of bytes written
#[inline]
pub fn record_bytes_written(filesystem: &'static str, bytes: u64) {
    counter!(
        "nanograph_vfs_bytes_written_total",
        "filesystem" => filesystem
    )
    .increment(bytes);
}

/// Records the duration of a filesystem operation.
///
/// # Arguments
/// * `operation` - The operation name
/// * `filesystem` - The filesystem type
/// * `duration_micros` - Duration in microseconds
#[inline]
pub fn record_operation_duration(
    operation: &'static str,
    filesystem: &'static str,
    duration_micros: u64,
) {
    histogram!(
        "nanograph_vfs_operation_duration_microseconds",
        "operation" => operation,
        "filesystem" => filesystem
    )
    .record(duration_micros as f64);
}

/// Records the number of currently open files.
///
/// # Arguments
/// * `filesystem` - The filesystem type
/// * `count` - Number of open files
#[inline]
pub fn record_open_files(filesystem: &'static str, count: i64) {
    gauge!(
        "nanograph_vfs_open_files",
        "filesystem" => filesystem
    )
    .set(count as f64);
}

/// Records file size.
///
/// # Arguments
/// * `filesystem` - The filesystem type
/// * `size` - File size in bytes
#[inline]
pub fn record_file_size(filesystem: &'static str, size: u64) {
    histogram!(
        "nanograph_vfs_file_size_bytes",
        "filesystem" => filesystem
    )
    .record(size as f64);
}

/// Records cache hit/miss for cached filesystems.
///
/// # Arguments
/// * `filesystem` - The filesystem type
/// * `hit` - Whether it was a cache hit
#[inline]
pub fn record_cache_access(filesystem: &'static str, hit: bool) {
    let result = if hit { "hit" } else { "miss" };
    counter!(
        "nanograph_vfs_cache_accesses_total",
        "filesystem" => filesystem,
        "result" => result
    )
    .increment(1);
}

/// Records mount/unmount operations.
///
/// # Arguments
/// * `operation` - "mount" or "unmount"
/// * `success` - Whether the operation succeeded
#[inline]
pub fn record_mount_operation(operation: &'static str, success: bool) {
    let status = if success { "success" } else { "failure" };
    counter!(
        "nanograph_vfs_mount_operations_total",
        "operation" => operation,
        "status" => status
    )
    .increment(1);
}

/// Records layer access in overlay filesystems.
///
/// # Arguments
/// * `layer_index` - The layer index (0 = top)
/// * `operation` - The operation type
#[inline]
pub fn record_layer_access(layer_index: usize, operation: &'static str) {
    let layer_str = match layer_index {
        0 => "0",
        1 => "1",
        2 => "2",
        3 => "3",
        4 => "4",
        _ => "5+",
    };
    counter!(
        "nanograph_vfs_overlay_layer_accesses_total",
        "layer" => layer_str,
        "operation" => operation
    )
    .increment(1);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_metrics_recording() {
        // These tests just ensure the metrics functions don't panic
        record_operation("read", "memory", true);
        record_bytes_read("memory", 1024);
        record_bytes_written("memory", 2048);
        record_operation_duration("write", "local", 150);
        record_open_files("memory", 5);
        record_file_size("local", 4096);
        record_cache_access("overlay", true);
        record_mount_operation("mount", true);
        record_layer_access(0, "read");
    }
}

// Made with Bob
