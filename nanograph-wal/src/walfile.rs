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

use crate::config::IntegrityAlgorithm;
use crate::lsn::LogSequenceNumber;
use crate::reader::WriteAheadLogReader;
use crate::result::{WriteAheadLogError, WriteAheadLogResult};
use crate::writer::WriteAheadLogRecord;
use byteorder::{BigEndian, ByteOrder, WriteBytesExt};
use nanograph_util::{CompressionAlgorithm, EncryptionAlgorithm};
use nanograph_vfs::File;
use std::io::Write;
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const SEGMENT_MAGIC: u32 = 0x474e414e;
const RECORD_MAGIC: u32 = 0x474e414f;

/// Size of the Segment Header in Bytes
pub const HEADER_SIZE: usize = 4 + 2 + 8 + 8 + 8 + 8 + 1 + 1 + 1 + 16 + 4; // Magic + Version + ShardID + SegmentID + StartOffset + CreatedAt + Integrity + Compression + Encryption + EncryptionKeyID(u128) + Checksum

/// Write Ahead Log Segment Header.
/// This header is at the beginning of every WAL segment file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WriteAheadLogSegmentHeader {
    /// Segment Header Version
    pub version: u16,
    /// Data Shard Id
    pub shard_id: u64,
    /// WAL Segment Id
    pub segment_id: u64,
    /// WAL Segment Start Offset
    pub start_offset: u64,
    /// WAL Segment Created At Unix Timestamp in Milliseconds
    pub created_at_unix_ms: u64,
    /// Integrity Algorithm
    pub integrity: IntegrityAlgorithm,
    /// Compression Algorithm
    pub compression: CompressionAlgorithm,
    /// Encryption Algorithm
    pub encryption: EncryptionAlgorithm,
    /// Encryption Key Identifier (0 if no encryption)
    pub encryption_key_id: u128,
    /// Checksum of the header
    pub checksum: u32,
}

impl WriteAheadLogSegmentHeader {
    /// Get the starting Log Sequence Number for this segment
    pub fn start_lsn(&self) -> LogSequenceNumber {
        LogSequenceNumber {
            segment_id: self.segment_id,
            offset: self.start_offset,
        }
    }

    /// Get the WAL Segment Creation Time
    pub fn created_at(&self) -> SystemTime {
        UNIX_EPOCH + Duration::from_millis(self.created_at_unix_ms)
    }

    /// Encode the header into a byte buffer
    pub fn encode(&self, buffer: &mut [u8]) {
        BigEndian::write_u32(&mut buffer[0..4], SEGMENT_MAGIC);
        BigEndian::write_u16(&mut buffer[4..6], self.version);
        BigEndian::write_u64(&mut buffer[6..14], self.shard_id);
        BigEndian::write_u64(&mut buffer[14..22], self.segment_id);
        BigEndian::write_u64(&mut buffer[22..30], self.start_offset);
        BigEndian::write_u64(&mut buffer[30..38], self.created_at_unix_ms);
        buffer[38] = self.integrity.as_u8();
        buffer[39] = self.compression.as_u8();
        buffer[40] = self.encryption.as_u8();
        BigEndian::write_u128(&mut buffer[41..57], self.encryption_key_id);

        let checksum = self.integrity.hash(&buffer[0..57]).as_u32().unwrap_or(0);
        BigEndian::write_u32(&mut buffer[57..61], checksum);
    }

    /// Decode a header from a byte buffer
    pub fn decode(buffer: &[u8]) -> WriteAheadLogResult<Self> {
        if buffer.len() < HEADER_SIZE {
            return Err(WriteAheadLogError::InvalidLsn); // TODO: Better error
        }
        let magic = BigEndian::read_u32(&buffer[0..4]);
        if magic != SEGMENT_MAGIC {
            return Err(WriteAheadLogError::VersionMismatch);
        }

        let integrity =
            IntegrityAlgorithm::from_u8(buffer[38]).ok_or(WriteAheadLogError::VersionMismatch)?;
        let compression =
            CompressionAlgorithm::from_u8(buffer[39]).ok_or(WriteAheadLogError::VersionMismatch)?;
        let encryption =
            EncryptionAlgorithm::from_u8(buffer[40]).ok_or(WriteAheadLogError::VersionMismatch)?;

        let read_checksum = BigEndian::read_u32(&buffer[57..61]);

        // Verify checksum if integrity checking is enabled
        if integrity != IntegrityAlgorithm::None {
            let calc_checksum = integrity.hash(&buffer[0..57]).as_u32().unwrap_or(0);
            if read_checksum != calc_checksum {
                return Err(WriteAheadLogError::ChecksumMismatch);
            }
        }

        let header = Self {
            version: BigEndian::read_u16(&buffer[4..6]),
            shard_id: BigEndian::read_u64(&buffer[6..14]),
            segment_id: BigEndian::read_u64(&buffer[14..22]),
            start_offset: BigEndian::read_u64(&buffer[22..30]),
            created_at_unix_ms: BigEndian::read_u64(&buffer[30..38]),
            integrity,
            compression,
            encryption,
            encryption_key_id: BigEndian::read_u128(&buffer[41..57]),
            checksum: read_checksum,
        };

        Ok(header)
    }
}

/// Representation of a single Write Ahead Log segment file.
#[derive(Clone)]
pub struct WriteAheadLogFile {
    file: Arc<Mutex<dyn File>>,
    shard_id: u64,
    segment_id: u64,
    start_offset: u64,
    write_offset: u64,
    integrity: IntegrityAlgorithm,
}

impl WriteAheadLogFile {
    /// Create a new WAL segment file
    pub fn create<F: File + 'static>(
        mut file: F,
        shard_id: u64,
        segment_id: u64,
        start_offset: u64,
        integrity: IntegrityAlgorithm,
        compression: CompressionAlgorithm,
        encryption: EncryptionAlgorithm,
        encryption_key_id: u128,
    ) -> WriteAheadLogResult<Self> {
        let created_at_unix_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;
        let header = WriteAheadLogSegmentHeader {
            version: 1,
            shard_id,
            segment_id,
            start_offset,
            created_at_unix_ms,
            integrity,
            compression,
            encryption,
            encryption_key_id,
            checksum: 0,
        };
        let mut buffer = [0u8; HEADER_SIZE];
        header.encode(&mut buffer);

        // After encode, we need to update the checksum in the header struct if we want it to be accurate
        let header_with_checksum = WriteAheadLogSegmentHeader {
            checksum: BigEndian::read_u32(&buffer[57..61]),
            ..header
        };

        file.write_to_offset(0, &buffer)
            .map_err(WriteAheadLogError::FileSystem)?;

        Ok(Self {
            file: Arc::new(Mutex::new(file)),
            shard_id: header_with_checksum.shard_id,
            segment_id: header_with_checksum.segment_id,
            start_offset: header_with_checksum.start_offset,
            write_offset: HEADER_SIZE as u64,
            integrity: header_with_checksum.integrity,
        })
    }

    /// Open an existing WAL segment file and read its header
    pub fn open_existing<F: File + 'static>(mut file: F) -> WriteAheadLogResult<Self> {
        let mut buffer = [0u8; HEADER_SIZE];
        file.read_at_offset(0, &mut buffer)
            .map_err(WriteAheadLogError::FileSystem)?;
        let header = WriteAheadLogSegmentHeader::decode(&buffer)?;
        let size = file.get_size().map_err(WriteAheadLogError::FileSystem)?;
        Ok(Self {
            file: Arc::new(Mutex::new(file)),
            shard_id: header.shard_id,
            segment_id: header.segment_id,
            start_offset: header.start_offset,
            write_offset: size,
            integrity: header.integrity,
        })
    }

    /// Get the start LSN of this segment
    pub fn start_lsn(&self) -> LogSequenceNumber {
        LogSequenceNumber {
            segment_id: self.segment_id,
            offset: self.start_offset,
        }
    }

    /// Append a record to the WAL segment
    pub fn append(
        &mut self,
        record: &WriteAheadLogRecord<'_>,
    ) -> WriteAheadLogResult<LogSequenceNumber> {
        let lsn = LogSequenceNumber {
            segment_id: self.segment_id,
            offset: self.write_offset,
        };
        let payload_len = record.payload.len() as u32;
        let record_size = 4 + 2 + 4 + record.payload.len() + 4; // Magic + Kind + Len + Payload + Checksum
        let mut buffer = Vec::with_capacity(record_size);
        buffer.write_u32::<BigEndian>(RECORD_MAGIC).unwrap();
        buffer.write_u16::<BigEndian>(record.kind).unwrap();
        buffer.write_u32::<BigEndian>(payload_len).unwrap();
        buffer.write_all(record.payload).unwrap();

        let checksum = self.integrity.hash(&buffer).as_u32().unwrap_or(0);
        buffer.write_u32::<BigEndian>(checksum).unwrap();

        let mut file = self.file.lock().unwrap();

        file.write_to_offset(self.write_offset, &buffer)
            .map_err(WriteAheadLogError::FileSystem)?;
        self.write_offset += buffer.len() as u64;

        Ok(lsn)
    }

    /// Flush the WAL segment to the operating system's buffers
    pub fn flush(&mut self) -> WriteAheadLogResult<()> {
        let mut file = self.file.lock().unwrap();
        file.sync_data().map_err(WriteAheadLogError::FileSystem)?;
        Ok(())
    }

    /// Synchronize the WAL segment with stable storage
    pub fn sync(&mut self) -> WriteAheadLogResult<()> {
        let mut file = self.file.lock().unwrap();
        file.sync_all().map_err(WriteAheadLogError::FileSystem)?;
        Ok(())
    }

    /// Get the current tail LSN for this segment
    pub fn tail_lsn(&self) -> LogSequenceNumber {
        LogSequenceNumber {
            segment_id: self.segment_id,
            offset: self.write_offset,
        }
    }

    /// Get the segment ID
    pub fn segment_id(&self) -> u64 {
        self.segment_id
    }

    /// Get the shard ID
    pub fn shard_id(&self) -> u64 {
        self.shard_id
    }

    /// Create a reader for this segment starting at the given offset
    pub fn reader_from_offset(&self, offset: u64) -> WriteAheadLogResult<WriteAheadLogReader> {
        WriteAheadLogReader::from_file(self.file.clone(), self.segment_id, offset, self.integrity)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nanograph_vfs::{FileSystem, MemoryFileSystem};

    #[test]
    fn test_segment_header_serialization() {
        let mut header = WriteAheadLogSegmentHeader {
            version: 1,
            shard_id: 123,
            segment_id: 456,
            start_offset: 789,
            created_at_unix_ms: 1000000,
            integrity: IntegrityAlgorithm::XXHash32,
            compression: CompressionAlgorithm::None,
            encryption: EncryptionAlgorithm::None,
            encryption_key_id: 0,
            checksum: 0,
        };
        let mut buffer = [0u8; HEADER_SIZE];
        header.encode(&mut buffer);

        // Update header checksum from encoded buffer
        header.checksum = BigEndian::read_u32(&buffer[57..61]);

        let decoded = WriteAheadLogSegmentHeader::decode(&buffer).unwrap();
        assert_eq!(decoded.version, header.version);
        assert_eq!(decoded.shard_id, header.shard_id);
        assert_eq!(decoded.segment_id, header.segment_id);
        assert_eq!(decoded.start_offset, header.start_offset);
        assert_eq!(decoded.created_at_unix_ms, header.created_at_unix_ms);
        assert_eq!(decoded.checksum, header.checksum);

        assert_eq!(decoded.start_lsn().segment_id, header.segment_id);
        assert_eq!(decoded.start_lsn().offset, header.start_offset);
        assert!(decoded.created_at() <= SystemTime::now());
    }

    #[test]
    fn test_wal_file_append_and_read() {
        let fs = MemoryFileSystem::new();
        let file = fs.create_file("/test.wal").unwrap();
        let mut wal_file = WriteAheadLogFile::create(
            file,
            1,
            0,
            0,
            IntegrityAlgorithm::Crc32c,
            CompressionAlgorithm::None,
            EncryptionAlgorithm::None,
            0,
        )
        .unwrap();

        let record1 = WriteAheadLogRecord {
            kind: 1,
            payload: b"hello",
        };
        let lsn1 = wal_file.append(&record1).unwrap();
        assert_eq!(lsn1.segment_id, 0);
        assert_eq!(lsn1.offset, HEADER_SIZE as u64);

        let record2 = WriteAheadLogRecord {
            kind: 2,
            payload: b"world!",
        };
        let lsn2 = wal_file.append(&record2).unwrap();
        assert_eq!(lsn2.segment_id, 0);
        assert_eq!(lsn2.offset, lsn1.offset + 4 + 2 + 4 + 5 + 4);

        let mut reader = wal_file.reader_from_offset(lsn1.offset).unwrap();
        let entry1 = reader.next().unwrap().unwrap();
        assert_eq!(entry1.kind, 1);
        assert_eq!(entry1.payload, b"hello");
        assert_eq!(entry1.lsn, lsn1);

        let entry2 = reader.next().unwrap().unwrap();
        assert_eq!(entry2.kind, 2);
        assert_eq!(entry2.payload, b"world!");
        assert_eq!(entry2.lsn, lsn2);

        assert!(reader.next().unwrap().is_none());
    }

    #[test]
    fn test_different_integrity_algorithms() {
        let algorithms = vec![
            IntegrityAlgorithm::None,
            IntegrityAlgorithm::Crc32c,
            IntegrityAlgorithm::XXHash32,
        ];
        for algo in algorithms {
            let fs = MemoryFileSystem::new();
            let path = format!("/test_{:?}.wal", algo);
            let file = fs.create_file(&path).unwrap();
            let mut wal_file = WriteAheadLogFile::create(
                file,
                1,
                0,
                0,
                algo,
                CompressionAlgorithm::None,
                EncryptionAlgorithm::None,
                0,
            )
            .unwrap();

            let record = WriteAheadLogRecord {
                kind: 1,
                payload: b"data",
            };
            let lsn = wal_file.append(&record).unwrap();

            let mut reader = wal_file.reader_from_offset(lsn.offset).unwrap();
            let entry = reader.next().unwrap().unwrap();
            assert_eq!(entry.payload, b"data");

            // Verify it survives reopening
            drop(reader);
            drop(wal_file);
            let file = fs.open_file(&path).unwrap();
            let _wal_file = WriteAheadLogFile::open_existing(file).unwrap();
        }
    }
}
