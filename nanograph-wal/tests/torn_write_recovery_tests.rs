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

//! Tests for torn-write recovery scenarios
//!
//! These tests verify that the WAL correctly handles partial writes
//! (torn writes) that can occur during crashes or power failures.

use nanograph_vfs::{File, FileSystem, MemoryFileSystem};
use nanograph_wal::{
    Durability, LogSequenceNumber, WriteAheadLogConfig, WriteAheadLogManager, WriteAheadLogRecord,
};

#[test]
fn test_torn_write_at_end_of_segment() {
    // This test simulates a torn write at the end of a WAL segment
    // and verifies that recovery stops at the last valid record.

    let fs = MemoryFileSystem::new();
    let config = WriteAheadLogConfig::new(1);
    let wal = WriteAheadLogManager::new(fs.clone(), "/wal", config).unwrap();

    // Write several complete records
    let mut writer = wal.writer().unwrap();
    let mut lsns = Vec::new();

    for i in 0..5 {
        let payload = format!("record_{}", i);
        let record = WriteAheadLogRecord {
            kind: 1,
            payload: payload.as_bytes(),
        };
        let lsn = writer.append(record, Durability::Sync).unwrap();
        lsns.push(lsn);
    }

    drop(writer);

    // Simulate a torn write by truncating the last record
    // Get the current segment file and corrupt it
    let segment_path = "/wal/00000000.log";
    let mut file = fs.open_file(segment_path).unwrap();
    let size = file.get_size().unwrap();

    // Truncate to remove part of the last record (remove last 10 bytes)
    file.set_size(size - 10).unwrap();
    drop(file);

    // Now try to read back - should recover first 4 records, stop at torn write
    let mut reader = wal.reader_from(LogSequenceNumber::ZERO).unwrap();

    // Should successfully read first 4 records
    for i in 0..4 {
        let entry = reader.next().unwrap();
        assert!(entry.is_some(), "Expected record {} to be present", i);
        let entry = entry.unwrap();
        assert_eq!(entry.kind, 1);
        assert_eq!(
            entry.payload,
            format!("record_{}", i).as_bytes(),
            "Record {} payload mismatch",
            i
        );
    }

    // The 5th record should either be None (EOF) or return an error (corruption)
    // Both are acceptable for torn write handling
    let result = reader.next();
    match result {
        Ok(None) => {
            // EOF reached - acceptable
        }
        Err(_) => {
            // Corruption detected - also acceptable
        }
        Ok(Some(_)) => {
            panic!("Should not successfully read the torn record");
        }
    }
}

#[test]
fn test_recovery_with_multiple_segments_and_torn_write() {
    // Test recovery across multiple segments with a torn write in the last segment

    let fs = MemoryFileSystem::new();
    let mut config = WriteAheadLogConfig::new(2);
    // Use small segment size to force multiple segments
    config.max_segment_size = 1024; // 1KB segments

    let wal = WriteAheadLogManager::new(fs.clone(), "/wal", config).unwrap();
    let mut writer = wal.writer().unwrap();

    // Write enough records to span multiple segments
    let mut all_lsns = Vec::new();
    for i in 0..20 {
        let payload = format!("record_{:04}_with_some_padding_to_increase_size", i);
        let record = WriteAheadLogRecord {
            kind: 1,
            payload: payload.as_bytes(),
        };
        let lsn = writer.append(record, Durability::Sync).unwrap();
        all_lsns.push((lsn, payload));
    }

    drop(writer);

    // Find the last segment file
    let files = fs.list_directory("/wal").unwrap();
    let mut segment_files: Vec<_> = files
        .iter()
        .filter(|f| f.ends_with(".log"))
        .collect();
    segment_files.sort();

    if let Some(last_segment) = segment_files.last() {
        let segment_path = format!("/wal/{}", last_segment);
        let mut file = fs.open_file(&segment_path).unwrap();
        let size = file.get_size().unwrap();

        // Corrupt the last segment by truncating it
        if size > 50 {
            file.set_size(size - 30).unwrap();
        }
        drop(file);
    }

    // Recover and verify we get all complete records
    let mut reader = wal.reader_from(LogSequenceNumber::ZERO).unwrap();
    let mut recovered_count = 0;

    loop {
        match reader.next() {
            Ok(Some(entry)) => {
                assert_eq!(entry.kind, 1);
                recovered_count += 1;
            }
            Ok(None) => break,
            Err(_) => break, // Torn write detected
        }
    }

    // Should recover most records (at least 15 out of 20)
    assert!(
        recovered_count >= 15,
        "Expected to recover at least 15 records, got {}",
        recovered_count
    );
    assert!(
        recovered_count < 20,
        "Should not recover all records due to torn write"
    );
}

#[test]
fn test_empty_segment_recovery() {
    // Test that recovery handles empty segments gracefully

    let fs = MemoryFileSystem::new();
    let config = WriteAheadLogConfig::new(3);
    let wal = WriteAheadLogManager::new(fs.clone(), "/wal", config.clone()).unwrap();

    // Create WAL but don't write anything
    drop(wal);

    // Reopen and try to read
    let wal2 = WriteAheadLogManager::new(fs.clone(), "/wal", config).unwrap();
    let mut reader = wal2.reader_from(LogSequenceNumber::ZERO).unwrap();

    // Should return None immediately
    let result = reader.next().unwrap();
    assert!(result.is_none(), "Empty WAL should return None");
}

#[test]
fn test_partial_header_corruption() {
    // Test recovery when a record header is partially written

    let fs = MemoryFileSystem::new();
    let config = WriteAheadLogConfig::new(4);
    let wal = WriteAheadLogManager::new(fs.clone(), "/wal", config).unwrap();

    // Write a few records
    let mut writer = wal.writer().unwrap();
    for i in 0..3 {
        let payload = format!("record_{}", i);
        let record = WriteAheadLogRecord {
            kind: 1,
            payload: payload.as_bytes(),
        };
        writer.append(record, Durability::Sync).unwrap();
    }
    drop(writer);

    // Corrupt by truncating to leave a partial header
    let segment_path = "/wal/00000000.log";
    let mut file = fs.open_file(segment_path).unwrap();
    let size = file.get_size().unwrap();

    // Truncate to leave only 5 bytes of what would be the next record header
    // (header is 10 bytes: magic(4) + kind(2) + len(4))
    file.set_size(size + 5).unwrap();
    drop(file);

    // Recovery should handle this gracefully
    let mut reader = wal.reader_from(LogSequenceNumber::ZERO).unwrap();

    // Should read the 3 complete records
    for i in 0..3 {
        let entry = reader.next().unwrap();
        assert!(entry.is_some(), "Expected record {} to be present", i);
    }

    // Next read should return None or error (both acceptable)
    let result = reader.next();
    match result {
        Ok(None) => {}, // Expected
        Err(_) => {},   // Also acceptable
        Ok(Some(_)) => panic!("Partial header should result in None or error"),
    }
}

#[test]
fn test_checksum_mismatch_stops_recovery() {
    // Test that a checksum mismatch is detected and stops recovery

    let fs = MemoryFileSystem::new();
    let config = WriteAheadLogConfig::new(5).with_integrity(
        nanograph_util::IntegrityAlgorithm::Crc32c
    );
    let wal = WriteAheadLogManager::new(fs.clone(), "/wal", config).unwrap();

    // Write records
    let mut writer = wal.writer().unwrap();
    for i in 0..5 {
        let payload = format!("record_{}", i);
        let record = WriteAheadLogRecord {
            kind: 1,
            payload: payload.as_bytes(),
        };
        writer.append(record, Durability::Sync).unwrap();
    }
    drop(writer);

    // Corrupt a byte in the middle of a record to cause checksum mismatch
    let segment_path = "/wal/00000000.log";
    let mut file = fs.open_file(segment_path).unwrap();
    
    // Read the file
    let size = file.get_size().unwrap();
    let mut buffer = vec![0u8; size as usize];
    file.read_at_offset(0, &mut buffer).unwrap();
    
    // Corrupt a byte in the middle (skip header, corrupt payload)
    if buffer.len() > 100 {
        buffer[100] ^= 0xFF; // Flip all bits
    }
    
    // Write back
    file.write_to_offset(0, &buffer).unwrap();
    drop(file);

    // Try to read - should detect corruption
    let mut reader = wal.reader_from(LogSequenceNumber::ZERO).unwrap();
    
    let mut found_corruption = false;
    loop {
        match reader.next() {
            Ok(Some(_)) => continue,
            Ok(None) => break,
            Err(_e) => {
                // Should get a corruption error
                found_corruption = true;
                break;
            }
        }
    }

    assert!(
        found_corruption,
        "Should have detected checksum mismatch"
    );
}

// Made with Bob
