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
fn test_wal_full_read_write_cycle() {
    let fs = MemoryFileSystem::new();
    let config = WriteAheadLogConfig {
        shard_id: 42,
        max_segment_size: 1024 * 1024,
        sync_on_rotate: true,
        checksum: IntegrityAlgorithm::None,
        compression: CompressionAlgorithm::None,
        encryption: EncryptionAlgorithm::None,
        encryption_key: None,
    };
    let wal_path = "/wal";

    // 1. Initialize and write records
    {
        let manager = WriteAheadLogManager::new(fs.clone(), wal_path, config.clone()).unwrap();
        let mut writer = manager.writer().unwrap();

        let records = vec![
            WriteAheadLogRecord {
                kind: 1,
                payload: b"first record",
            },
            WriteAheadLogRecord {
                kind: 2,
                payload: b"second record with more data",
            },
            WriteAheadLogRecord {
                kind: 3,
                payload: &[0u8; 100], // 100 zero bytes
            },
        ];

        for (i, record) in records.iter().enumerate() {
            let lsn = writer
                .append(
                    WriteAheadLogRecord {
                        kind: record.kind,
                        payload: record.payload,
                    },
                    Durability::Buffered,
                )
                .unwrap();
            assert_eq!(lsn.segment_id, 0);
            if i == 0 {
                // Header size is defined in walfile.rs
                assert_eq!(lsn.offset, HEADER_SIZE as u64);
            }
        }

        // Verify tail LSN
        let tail_lsn = writer.tail_lsn().unwrap();
        assert!(tail_lsn.offset > HEADER_SIZE as u64);
    }

    // 2. Re-open and verify records
    {
        let manager = WriteAheadLogManager::new(fs.clone(), wal_path, config.clone()).unwrap();
        let head_lsn = manager.head_lsn().unwrap();
        assert_eq!(
            head_lsn,
            LogSequenceNumber {
                segment_id: 0,
                offset: 0
            }
        );

        // Currently reader_from(0, 0) should fail because header is there, or maybe it should skip it?
        // Let's check from the first record offset which we know is HEADER_SIZE
        let first_record_lsn = LogSequenceNumber {
            segment_id: 0,
            offset: HEADER_SIZE as u64,
        };
        let mut reader = manager.reader_from(first_record_lsn).unwrap();

        let entry1 = reader.next().unwrap().expect("Should have first record");
        assert_eq!(entry1.kind, 1);
        assert_eq!(entry1.payload, b"first record");

        let entry2 = reader.next().unwrap().expect("Should have second record");
        assert_eq!(entry2.kind, 2);
        assert_eq!(entry2.payload, b"second record with more data");

        let entry3 = reader.next().unwrap().expect("Should have third record");
        assert_eq!(entry3.kind, 3);
        assert_eq!(entry3.payload, &[0u8; 100]);

        assert!(reader.next().unwrap().is_none(), "Should be at end of log");
    }
}

#[test]
fn test_wal_interleaved_read_write() {
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
    let manager = WriteAheadLogManager::new(fs.clone(), "/wal", config).unwrap();
    let mut writer = manager.writer().unwrap();

    // Write one
    let lsn1 = writer
        .append(
            WriteAheadLogRecord {
                kind: 1,
                payload: b"one",
            },
            Durability::Buffered,
        )
        .unwrap();

    // Read one
    let mut reader = manager.reader_from(lsn1).unwrap();
    let entry1 = reader.next().unwrap().unwrap();
    assert_eq!(entry1.payload, b"one");

    // Write another
    let lsn2 = writer
        .append(
            WriteAheadLogRecord {
                kind: 2,
                payload: b"two",
            },
            Durability::Buffered,
        )
        .unwrap();

    // Read next from same reader
    let entry2 = reader.next().unwrap().unwrap();
    assert_eq!(entry2.payload, b"two");
    assert_eq!(entry2.lsn, lsn2);
}
