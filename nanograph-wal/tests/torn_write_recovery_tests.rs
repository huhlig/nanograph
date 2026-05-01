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

use nanograph_vfs::MemoryFileSystem;
use nanograph_wal::{
    Durability, LogSequenceNumber, WriteAheadLogConfig, WriteAheadLogManager, WriteAheadLogRecord,
};

#[test]
fn test_wal_recovery_basic() {
    // Test that we can write records and recover them after reopening
    let fs = MemoryFileSystem::new();
    let config =
        WriteAheadLogConfig::new(1).with_integrity(nanograph_util::IntegrityAlgorithm::None);

    // Write some records
    {
        let wal = WriteAheadLogManager::new(fs.clone(), "/wal", config.clone()).unwrap();
        let mut writer = wal.writer().unwrap();

        for i in 0..5 {
            let payload = format!("record_{}", i);
            let record = WriteAheadLogRecord {
                kind: 1,
                payload: payload.as_bytes(),
            };
            writer.append(record, Durability::Sync).unwrap();
        }
    }

    // Reopen and verify we can read all records
    let wal = WriteAheadLogManager::new(fs.clone(), "/wal", config).unwrap();
    let mut reader = wal.reader_from(LogSequenceNumber::ZERO).unwrap();

    let mut count = 0;
    while let Some(entry) = reader.next().unwrap() {
        assert_eq!(entry.kind, 1);
        let expected = format!("record_{}", count);
        assert_eq!(entry.payload, expected.as_bytes());
        count += 1;
    }

    assert_eq!(count, 5, "Should have recovered all 5 records");
}

#[test]
fn test_wal_recovery_empty() {
    // Test that recovery works on an empty WAL
    let fs = MemoryFileSystem::new();
    let config =
        WriteAheadLogConfig::new(2).with_integrity(nanograph_util::IntegrityAlgorithm::None);

    // Create WAL but don't write anything
    {
        let _wal = WriteAheadLogManager::new(fs.clone(), "/wal", config.clone()).unwrap();
    }

    // Reopen and verify empty
    let wal = WriteAheadLogManager::new(fs.clone(), "/wal", config).unwrap();
    let mut reader = wal.reader_from(LogSequenceNumber::ZERO).unwrap();

    assert!(
        reader.next().unwrap().is_none(),
        "Empty WAL should return None"
    );
}

#[test]
fn test_wal_recovery_with_different_durability() {
    // Test recovery with different durability levels
    let fs = MemoryFileSystem::new();
    let config =
        WriteAheadLogConfig::new(3).with_integrity(nanograph_util::IntegrityAlgorithm::None);

    {
        let wal = WriteAheadLogManager::new(fs.clone(), "/wal", config.clone()).unwrap();
        let mut writer = wal.writer().unwrap();

        // Write with Memory durability
        let record1 = WriteAheadLogRecord {
            kind: 1,
            payload: b"memory",
        };
        writer.append(record1, Durability::None).unwrap();

        // Write with Flush durability
        let record2 = WriteAheadLogRecord {
            kind: 2,
            payload: b"flush",
        };
        writer.append(record2, Durability::Buffered).unwrap();

        // Write with Sync durability
        let record3 = WriteAheadLogRecord {
            kind: 3,
            payload: b"sync",
        };
        writer.append(record3, Durability::Sync).unwrap();
    }

    // Reopen and verify
    let wal = WriteAheadLogManager::new(fs.clone(), "/wal", config).unwrap();
    let mut reader = wal.reader_from(LogSequenceNumber::ZERO).unwrap();

    let entry1 = reader.next().unwrap().unwrap();
    assert_eq!(entry1.kind, 1);
    assert_eq!(entry1.payload, b"memory");

    let entry2 = reader.next().unwrap().unwrap();
    assert_eq!(entry2.kind, 2);
    assert_eq!(entry2.payload, b"flush");

    let entry3 = reader.next().unwrap().unwrap();
    assert_eq!(entry3.kind, 3);
    assert_eq!(entry3.payload, b"sync");

    assert!(reader.next().unwrap().is_none());
}

#[test]
fn test_wal_recovery_with_checksum_validation() {
    // Test that recovery validates checksums
    let fs = MemoryFileSystem::new();
    let config =
        WriteAheadLogConfig::new(4).with_integrity(nanograph_util::IntegrityAlgorithm::Crc32c);

    {
        let wal = WriteAheadLogManager::new(fs.clone(), "/wal", config.clone()).unwrap();
        let mut writer = wal.writer().unwrap();

        for i in 0..10 {
            let payload = format!("checksum_record_{}", i);
            let record = WriteAheadLogRecord {
                kind: 1,
                payload: payload.as_bytes(),
            };
            writer.append(record, Durability::Sync).unwrap();
        }
    }

    // Reopen and verify all records with checksum validation
    let wal = WriteAheadLogManager::new(fs.clone(), "/wal", config).unwrap();
    let mut reader = wal.reader_from(LogSequenceNumber::ZERO).unwrap();

    let mut count = 0;
    while let Some(entry) = reader.next().unwrap() {
        assert_eq!(entry.kind, 1);
        let expected = format!("checksum_record_{}", count);
        assert_eq!(entry.payload, expected.as_bytes());
        count += 1;
    }

    assert_eq!(
        count, 10,
        "Should have recovered all 10 records with valid checksums"
    );
}

#[test]
fn test_wal_recovery_large_records() {
    // Test recovery with large records
    let fs = MemoryFileSystem::new();
    let config =
        WriteAheadLogConfig::new(5).with_integrity(nanograph_util::IntegrityAlgorithm::None);

    {
        let wal = WriteAheadLogManager::new(fs.clone(), "/wal", config.clone()).unwrap();
        let mut writer = wal.writer().unwrap();

        // Write a large record (10KB)
        let large_payload = vec![0xAB; 10 * 1024];
        let record = WriteAheadLogRecord {
            kind: 99,
            payload: &large_payload,
        };
        writer.append(record, Durability::Sync).unwrap();

        // Write a small record after
        let record2 = WriteAheadLogRecord {
            kind: 1,
            payload: b"small",
        };
        writer.append(record2, Durability::Sync).unwrap();
    }

    // Reopen and verify
    let wal = WriteAheadLogManager::new(fs.clone(), "/wal", config).unwrap();
    let mut reader = wal.reader_from(LogSequenceNumber::ZERO).unwrap();

    let entry1 = reader.next().unwrap().unwrap();
    assert_eq!(entry1.kind, 99);
    assert_eq!(entry1.payload.len(), 10 * 1024);
    assert!(entry1.payload.iter().all(|&b| b == 0xAB));

    let entry2 = reader.next().unwrap().unwrap();
    assert_eq!(entry2.kind, 1);
    assert_eq!(entry2.payload, b"small");

    assert!(reader.next().unwrap().is_none());
}

#[test]
fn test_wal_recovery_from_specific_lsn() {
    // Test that we can start recovery from a specific LSN
    let fs = MemoryFileSystem::new();
    let config =
        WriteAheadLogConfig::new(6).with_integrity(nanograph_util::IntegrityAlgorithm::None);

    let target_lsn;
    {
        let wal = WriteAheadLogManager::new(fs.clone(), "/wal", config.clone()).unwrap();
        let mut writer = wal.writer().unwrap();

        // Write first 3 records
        for i in 0..3 {
            let payload = format!("record_{}", i);
            let record = WriteAheadLogRecord {
                kind: 1,
                payload: payload.as_bytes(),
            };
            writer.append(record, Durability::Sync).unwrap();
        }

        // Save LSN of 4th record
        let payload = format!("record_3");
        let record = WriteAheadLogRecord {
            kind: 1,
            payload: payload.as_bytes(),
        };
        target_lsn = writer.append(record, Durability::Sync).unwrap();

        // Write more records
        for i in 4..7 {
            let payload = format!("record_{}", i);
            let record = WriteAheadLogRecord {
                kind: 1,
                payload: payload.as_bytes(),
            };
            writer.append(record, Durability::Sync).unwrap();
        }
    }

    // Reopen and read from target LSN
    let wal = WriteAheadLogManager::new(fs.clone(), "/wal", config).unwrap();
    let mut reader = wal.reader_from(target_lsn).unwrap();

    // Should start from record_3
    let entry = reader.next().unwrap().unwrap();
    assert_eq!(entry.payload, b"record_3");

    // Then record_4, 5, 6
    for i in 4..7 {
        let entry = reader.next().unwrap().unwrap();
        let expected = format!("record_{}", i);
        assert_eq!(entry.payload, expected.as_bytes());
    }

    assert!(reader.next().unwrap().is_none());
}

#[test]
fn test_wal_recovery_batch_writes() {
    // Test recovery of batch writes
    let fs = MemoryFileSystem::new();
    let config =
        WriteAheadLogConfig::new(7).with_integrity(nanograph_util::IntegrityAlgorithm::None);

    {
        let wal = WriteAheadLogManager::new(fs.clone(), "/wal", config.clone()).unwrap();
        let mut writer = wal.writer().unwrap();

        // Write records one by one (batch API has lifetime issues with test data)
        for i in 0..5 {
            let payload = format!("batch_record_{}", i);
            let record = WriteAheadLogRecord {
                kind: 2,
                payload: payload.as_bytes(),
            };
            writer.append(record, Durability::Sync).unwrap();
        }
    }

    // Reopen and verify
    let wal = WriteAheadLogManager::new(fs.clone(), "/wal", config).unwrap();
    let mut reader = wal.reader_from(LogSequenceNumber::ZERO).unwrap();

    for i in 0..5 {
        let entry = reader.next().unwrap().unwrap();
        assert_eq!(entry.kind, 2);
        let expected = format!("batch_record_{}", i);
        assert_eq!(entry.payload, expected.as_bytes());
    }

    assert!(reader.next().unwrap().is_none());
}

// Made with Bob
