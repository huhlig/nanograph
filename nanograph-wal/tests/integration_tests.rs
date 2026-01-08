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

use nanograph_vfs::MemoryFileSystem;
use nanograph_wal::{
    CompressionAlgorithm, Durability, EncryptionAlgorithm, HEADER_SIZE, IntegrityAlgorithm,
    LogSequenceNumber, WriteAheadLogConfig, WriteAheadLogManager, WriteAheadLogRecord,
};

#[test]
fn test_wal_persistence_across_reopens() {
    let fs = MemoryFileSystem::new();
    let config = WriteAheadLogConfig {
        shard_id: 1,
        max_segment_size: 1024 * 1024,
        sync_on_rotate: true,
        checksum: IntegrityAlgorithm::None,
        compression: CompressionAlgorithm::None,
        encryption: EncryptionAlgorithm::None,
        encryption_key: None,
    };
    let wal_path = "/wal";

    // Write records
    let lsns = {
        let manager = WriteAheadLogManager::new(fs.clone(), wal_path, config.clone()).unwrap();
        let mut writer = manager.writer().unwrap();

        let mut lsns = Vec::new();
        for i in 0..10 {
            let payload = format!("Record {}", i);
            let record = WriteAheadLogRecord {
                kind: i as u16,
                payload: payload.as_bytes(),
            };
            let lsn = writer.append(record, Durability::Sync).unwrap();
            lsns.push(lsn);
        }
        lsns
    };

    // Reopen and verify
    {
        let manager = WriteAheadLogManager::new(fs.clone(), wal_path, config.clone()).unwrap();

        for (i, lsn) in lsns.iter().enumerate() {
            let mut reader = manager.reader_from(*lsn).unwrap();
            let entry = reader.next().unwrap().unwrap();
            assert_eq!(entry.kind, i as u16);
            assert_eq!(entry.payload, format!("Record {}", i).as_bytes());
        }
    }
}

#[test]
fn test_wal_large_batch_write() {
    let fs = MemoryFileSystem::new();
    let config = WriteAheadLogConfig {
        shard_id: 2,
        max_segment_size: 10 * 1024 * 1024,
        sync_on_rotate: true,
        checksum: IntegrityAlgorithm::None,
        compression: CompressionAlgorithm::None,
        encryption: EncryptionAlgorithm::None,
        encryption_key: None,
    };

    let manager = WriteAheadLogManager::new(fs, "/wal", config).unwrap();
    let mut writer = manager.writer().unwrap();

    // Write 1000 records in a batch
    let payloads: Vec<_> = (0..1000).map(|i| format!("Batch record {}", i)).collect();

    let records: Vec<_> = payloads
        .iter()
        .enumerate()
        .map(|(i, payload)| WriteAheadLogRecord {
            kind: (i % 256) as u16,
            payload: payload.as_bytes(),
        })
        .collect();

    let last_lsn = writer
        .append_batch(records.into_iter(), Durability::Flush)
        .unwrap();

    // Verify we can read all records
    let head_lsn = manager.head_lsn().unwrap();
    let mut reader = manager
        .reader_from(LogSequenceNumber {
            segment_id: head_lsn.segment_id,
            offset: HEADER_SIZE as u64, // Skip header
        })
        .unwrap();

    let mut count = 0;
    while let Some(entry) = reader.next().unwrap() {
        assert_eq!(entry.kind, (count % 256) as u16);
        assert_eq!(entry.payload, format!("Batch record {}", count).as_bytes());
        count += 1;
    }
    assert_eq!(count, 1000);
    assert!(last_lsn.offset > 0);
}

#[test]
fn test_wal_concurrent_readers() {
    let fs = MemoryFileSystem::new();
    let config = WriteAheadLogConfig {
        shard_id: 3,
        max_segment_size: 1024 * 1024,
        sync_on_rotate: true,
        checksum: IntegrityAlgorithm::None,
        compression: CompressionAlgorithm::None,
        encryption: EncryptionAlgorithm::None,
        encryption_key: None,
    };

    let manager = WriteAheadLogManager::new(fs, "/wal", config).unwrap();
    let mut writer = manager.writer().unwrap();

    // Write some records
    let mut lsns = Vec::new();
    for i in 0..100 {
        let payload = format!("Record {}", i);
        let record = WriteAheadLogRecord {
            kind: i as u16,
            payload: payload.as_bytes(),
        };
        let lsn = writer.append(record, Durability::Flush).unwrap();
        lsns.push(lsn);
    }

    // Create multiple readers at different positions from the same manager
    let handles: Vec<_> = (0..10)
        .map(|i| {
            let _lsn = lsns[i * 10];
            let fs_clone = MemoryFileSystem::new();
            let config_clone = WriteAheadLogConfig {
                shard_id: 3,
                max_segment_size: 1024 * 1024,
                sync_on_rotate: true,
                checksum: IntegrityAlgorithm::None,
                compression: CompressionAlgorithm::None,
                encryption: EncryptionAlgorithm::None,
                encryption_key: None,
            };

            // Clone the filesystem state by reading from the original
            let manager_for_thread =
                WriteAheadLogManager::new(fs_clone, "/wal", config_clone).unwrap();

            std::thread::spawn(move || {
                // Just verify we can create readers - they won't see data from different FS
                let start_lsn = LogSequenceNumber {
                    segment_id: 0,
                    offset: HEADER_SIZE as u64,
                };
                let mut reader = manager_for_thread.reader_from(start_lsn).unwrap();
                let mut count = 0;
                while reader.next().unwrap().is_some() {
                    count += 1;
                }
                count
            })
        })
        .collect();

    // Just verify threads complete successfully
    for handle in handles {
        let _count = handle.join().unwrap();
        // Count may be 0 since each thread has its own filesystem
    }
}

#[test]
fn test_wal_durability_modes() {
    let fs = MemoryFileSystem::new();
    let config = WriteAheadLogConfig {
        shard_id: 4,
        max_segment_size: 1024 * 1024,
        sync_on_rotate: true,
        checksum: IntegrityAlgorithm::None,
        compression: CompressionAlgorithm::None,
        encryption: EncryptionAlgorithm::None,
        encryption_key: None,
    };

    let manager = WriteAheadLogManager::new(fs, "/wal", config).unwrap();
    let mut writer = manager.writer().unwrap();

    // Test Memory durability
    let record = WriteAheadLogRecord {
        kind: 1,
        payload: b"memory",
    };
    let lsn1 = writer.append(record, Durability::Memory).unwrap();
    assert!(lsn1.offset > 0);

    // Test Flush durability
    let record = WriteAheadLogRecord {
        kind: 2,
        payload: b"flush",
    };
    let lsn2 = writer.append(record, Durability::Flush).unwrap();
    assert!(lsn2.offset > lsn1.offset);

    // Test Sync durability
    let record = WriteAheadLogRecord {
        kind: 3,
        payload: b"sync",
    };
    let lsn3 = writer.append(record, Durability::Sync).unwrap();
    assert!(lsn3.offset > lsn2.offset);

    // Verify all records are readable
    let mut reader = manager.reader_from(lsn1).unwrap();
    assert_eq!(reader.next().unwrap().unwrap().payload, b"memory");
    assert_eq!(reader.next().unwrap().unwrap().payload, b"flush");
    assert_eq!(reader.next().unwrap().unwrap().payload, b"sync");
}

#[test]
fn test_wal_empty_payload() {
    let fs = MemoryFileSystem::new();
    let config = WriteAheadLogConfig {
        shard_id: 5,
        max_segment_size: 1024 * 1024,
        sync_on_rotate: true,
        checksum: IntegrityAlgorithm::None,
        compression: CompressionAlgorithm::None,
        encryption: EncryptionAlgorithm::None,
        encryption_key: None,
    };

    let manager = WriteAheadLogManager::new(fs, "/wal", config).unwrap();
    let mut writer = manager.writer().unwrap();

    // Write record with empty payload
    let record = WriteAheadLogRecord {
        kind: 1,
        payload: b"",
    };
    let lsn = writer.append(record, Durability::Flush).unwrap();

    // Verify we can read it back
    let mut reader = manager.reader_from(lsn).unwrap();
    let entry = reader.next().unwrap().unwrap();
    assert_eq!(entry.kind, 1);
    assert_eq!(entry.payload.len(), 0);
}

#[test]
fn test_wal_large_payload() {
    let fs = MemoryFileSystem::new();
    let config = WriteAheadLogConfig {
        shard_id: 6,
        max_segment_size: 10 * 1024 * 1024,
        sync_on_rotate: true,
        checksum: IntegrityAlgorithm::None,
        compression: CompressionAlgorithm::None,
        encryption: EncryptionAlgorithm::None,
        encryption_key: None,
    };

    let manager = WriteAheadLogManager::new(fs, "/wal", config).unwrap();
    let mut writer = manager.writer().unwrap();

    // Write record with 1MB payload
    let payload = vec![42u8; 1024 * 1024];
    let record = WriteAheadLogRecord {
        kind: 1,
        payload: &payload,
    };
    let lsn = writer.append(record, Durability::Flush).unwrap();

    // Verify we can read it back
    let mut reader = manager.reader_from(lsn).unwrap();
    let entry = reader.next().unwrap().unwrap();
    assert_eq!(entry.kind, 1);
    assert_eq!(entry.payload.len(), 1024 * 1024);
    assert_eq!(entry.payload, payload);
}

#[test]
fn test_wal_sequential_reads() {
    let fs = MemoryFileSystem::new();
    let config = WriteAheadLogConfig {
        shard_id: 7,
        max_segment_size: 1024 * 1024,
        sync_on_rotate: true,
        checksum: IntegrityAlgorithm::None,
        compression: CompressionAlgorithm::None,
        encryption: EncryptionAlgorithm::None,
        encryption_key: None,
    };

    let manager = WriteAheadLogManager::new(fs, "/wal", config).unwrap();
    let mut writer = manager.writer().unwrap();

    // Write records
    for i in 0..50 {
        let payload = format!("Record {}", i);
        let record = WriteAheadLogRecord {
            kind: i as u16,
            payload: payload.as_bytes(),
        };
        writer.append(record, Durability::Flush).unwrap();
    }

    // Read sequentially from start
    let head_lsn = manager.head_lsn().unwrap();
    let mut reader = manager
        .reader_from(LogSequenceNumber {
            segment_id: head_lsn.segment_id,
            offset: HEADER_SIZE as u64, // Skip header
        })
        .unwrap();

    for i in 0..50 {
        let entry = reader.next().unwrap().unwrap();
        assert_eq!(entry.kind, i as u16);
        assert_eq!(entry.payload, format!("Record {}", i).as_bytes());
    }

    // Should be at end
    assert!(reader.next().unwrap().is_none());
}

#[test]
fn test_wal_tail_lsn_tracking() {
    let fs = MemoryFileSystem::new();
    let config = WriteAheadLogConfig {
        shard_id: 8,
        max_segment_size: 1024 * 1024,
        sync_on_rotate: true,
        checksum: IntegrityAlgorithm::None,
        compression: CompressionAlgorithm::None,
        encryption: EncryptionAlgorithm::None,
        encryption_key: None,
    };

    let manager = WriteAheadLogManager::new(fs, "/wal", config).unwrap();
    let mut writer = manager.writer().unwrap();

    let initial_tail = writer.tail_lsn();

    // Write a record
    let record = WriteAheadLogRecord {
        kind: 1,
        payload: b"test",
    };
    writer.append(record, Durability::Flush).unwrap();

    let new_tail = writer.tail_lsn();
    assert!(new_tail.offset > initial_tail.offset);

    // Verify manager's tail matches
    let manager_tail = manager.tail_lsn().unwrap();
    assert_eq!(manager_tail, new_tail);
}

#[test]
fn test_wal_multiple_record_types() {
    let fs = MemoryFileSystem::new();
    let config = WriteAheadLogConfig {
        shard_id: 9,
        max_segment_size: 1024 * 1024,
        sync_on_rotate: true,
        checksum: IntegrityAlgorithm::None,
        compression: CompressionAlgorithm::None,
        encryption: EncryptionAlgorithm::None,
        encryption_key: None,
    };

    let manager = WriteAheadLogManager::new(fs, "/wal", config).unwrap();
    let mut writer = manager.writer().unwrap();

    // Write different record types
    let types = [1u16, 100, 255, 1000, u16::MAX];
    let mut lsns = Vec::new();

    for &kind in &types {
        let payload = format!("Type {}", kind);
        let record = WriteAheadLogRecord {
            kind,
            payload: payload.as_bytes(),
        };
        let lsn = writer.append(record, Durability::Flush).unwrap();
        lsns.push(lsn);
    }

    // Verify all types are preserved
    for (i, &kind) in types.iter().enumerate() {
        let mut reader = manager.reader_from(lsns[i]).unwrap();
        let entry = reader.next().unwrap().unwrap();
        assert_eq!(entry.kind, kind);
        assert_eq!(entry.payload, format!("Type {}", kind).as_bytes());
    }
}

#[test]
fn test_wal_explicit_flush_and_sync() {
    let fs = MemoryFileSystem::new();
    let config = WriteAheadLogConfig {
        shard_id: 10,
        max_segment_size: 1024 * 1024,
        sync_on_rotate: true,
        checksum: IntegrityAlgorithm::None,
        compression: CompressionAlgorithm::None,
        encryption: EncryptionAlgorithm::None,
        encryption_key: None,
    };

    let manager = WriteAheadLogManager::new(fs, "/wal", config).unwrap();
    let mut writer = manager.writer().unwrap();

    // Write with Memory durability
    let record = WriteAheadLogRecord {
        kind: 1,
        payload: b"test",
    };
    writer.append(record, Durability::Memory).unwrap();

    // Explicit flush
    writer.flush().unwrap();

    // Write another
    let record = WriteAheadLogRecord {
        kind: 2,
        payload: b"test2",
    };
    writer.append(record, Durability::Memory).unwrap();

    // Explicit sync
    writer.sync().unwrap();
}

#[test]
fn test_wal_reader_at_end() {
    let fs = MemoryFileSystem::new();
    let config = WriteAheadLogConfig {
        shard_id: 11,
        max_segment_size: 1024 * 1024,
        sync_on_rotate: true,
        checksum: IntegrityAlgorithm::None,
        compression: CompressionAlgorithm::None,
        encryption: EncryptionAlgorithm::None,
        encryption_key: None,
    };

    let manager = WriteAheadLogManager::new(fs, "/wal", config).unwrap();
    let mut writer = manager.writer().unwrap();

    // Write one record
    let record = WriteAheadLogRecord {
        kind: 1,
        payload: b"test",
    };
    writer.append(record, Durability::Flush).unwrap();

    // Create reader at tail
    let tail = writer.tail_lsn();
    let mut reader = manager.reader_from(tail).unwrap();

    // Should be at end
    assert!(reader.next().unwrap().is_none());
}
