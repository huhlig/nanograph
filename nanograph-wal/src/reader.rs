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
use crate::manager::WriteAheadLogManager;
use crate::metrics;
use crate::result::{WriteAheadLogError, WriteAheadLogResult};
use byteorder::{BigEndian, ByteOrder};
use nanograph_vfs::File;
use std::sync::{Arc, Mutex};
use std::time::Instant;

const RECORD_MAGIC: u32 = 0x474e414f;

/// Reader for the Write Ahead Log
///
/// Provides an iterator-like interface to read records from a WAL segment.
pub struct WriteAheadLogReader {
    file: Arc<Mutex<dyn File>>,
    segment_id: u64,
    offset: u64,
    integrity: IntegrityAlgorithm,
}

impl WriteAheadLogReader {
    /// Create a reader for a specific segment file starting at the given offset
    pub(crate) fn from_file(
        file: Arc<Mutex<dyn File>>,
        segment_id: u64,
        offset: u64,
        integrity: IntegrityAlgorithm,
    ) -> WriteAheadLogResult<Self> {
        Ok(Self {
            file,
            segment_id,
            offset,
            integrity,
        })
    }

    /// Create a reader starting from a given LSN (inclusive)
    pub fn from_lsn(
        manager: &WriteAheadLogManager,
        lsn: LogSequenceNumber,
    ) -> WriteAheadLogResult<Self> {
        manager.reader_from(lsn)
    }

    /// Read the next record from the log
    /// Returns `Ok(Some(entry))` if a record was read, `Ok(None)` if end of file reached.
    #[tracing::instrument(level = "trace", skip(self))]
    pub fn next(&mut self) -> WriteAheadLogResult<Option<WriteAheadLogEntry>> {
        let start = Instant::now();
        let shard_id = self.segment_id as u128; // Using segment_id as shard_id for metrics

        let mut file = self.file.lock().map_err(|_| WriteAheadLogError::LockPoisoned)?;
        let size = file.get_size().map_err(WriteAheadLogError::FileSystem)?;
        if self.offset >= size {
            return Ok(None);
        }

        // Read record header: Magic(4) + Kind(2) + Len(4)
        let mut header = [0u8; 10];
        let bytes_read = file
            .read_at_offset(self.offset, &mut header)
            .map_err(WriteAheadLogError::FileSystem)?;

        if bytes_read < 10 {
            return Ok(None);
        }

        let magic = BigEndian::read_u32(&header[0..4]);
        if magic != RECORD_MAGIC {
            metrics::record_operation("read", shard_id, false);
            return Err(WriteAheadLogError::Corruption {
                lsn: LogSequenceNumber {
                    segment_id: self.segment_id,
                    offset: self.offset,
                },
            });
        }

        let kind = BigEndian::read_u16(&header[4..6]);
        let len = BigEndian::read_u32(&header[6..10]) as usize;

        let mut payload = vec![0u8; len];
        let bytes_read = file
            .read_at_offset(self.offset + 10, &mut payload)
            .map_err(WriteAheadLogError::FileSystem)?;

        if bytes_read < len {
            metrics::record_operation("read", shard_id, false);
            return Err(WriteAheadLogError::Corruption {
                lsn: LogSequenceNumber {
                    segment_id: self.segment_id,
                    offset: self.offset,
                },
            });
        }

        // Read checksum(4)
        let mut checksum_buf = [0u8; 4];
        let bytes_read = file
            .read_at_offset(self.offset + 10 + len as u64, &mut checksum_buf)
            .map_err(WriteAheadLogError::FileSystem)?;

        if bytes_read < 4 {
            metrics::record_operation("read", shard_id, false);
            return Err(WriteAheadLogError::Corruption {
                lsn: LogSequenceNumber {
                    segment_id: self.segment_id,
                    offset: self.offset,
                },
            });
        }

        let read_checksum = BigEndian::read_u32(&checksum_buf);

        // Verify checksum
        let mut full_record = Vec::with_capacity(header.len() + payload.len());
        full_record.extend_from_slice(&header);
        full_record.extend_from_slice(&payload);
        // Verify checksum if integrity checking is enabled
        if self.integrity != IntegrityAlgorithm::None {
            let calc_checksum = self.integrity.hash(&full_record).as_u32().unwrap_or(0);
            if read_checksum != calc_checksum {
                metrics::record_operation("read", shard_id, false);
                return Err(WriteAheadLogError::Corruption {
                    lsn: LogSequenceNumber {
                        segment_id: self.segment_id,
                        offset: self.offset,
                    },
                });
            }
        }

        let entry = WriteAheadLogEntry {
            lsn: LogSequenceNumber {
                segment_id: self.segment_id,
                offset: self.offset,
            },
            kind,
            payload,
        };

        self.offset += (10 + len + 4) as u64;

        // Record metrics
        let total_bytes = 10 + len + 4;
        metrics::record_bytes_read(shard_id, total_bytes as u64);
        metrics::record_records_read(shard_id, 1);
        metrics::record_record_size(shard_id, len);

        let duration = start.elapsed().as_micros() as u64;
        metrics::record_operation_duration("read", shard_id, duration);
        metrics::record_operation("read", shard_id, true);

        Ok(Some(entry))
    }
}

/// A single entry read from the Write Ahead Log
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WriteAheadLogEntry {
    /// LSN of this record
    pub lsn: LogSequenceNumber,
    /// Application-defined record type identifier
    pub kind: u16,
    /// Binary payload of the record
    pub payload: Vec<u8>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::walfile::WriteAheadLogFile;
    use nanograph_util::{CompressionAlgorithm, EncryptionAlgorithm};
    use nanograph_vfs::{FileSystem, MemoryFileSystem};

    #[test]
    fn test_reader_empty_file() {
        let fs = MemoryFileSystem::new();
        let file = fs.create_file("/test.wal").unwrap();
        let wal_file = WriteAheadLogFile::create(
            file,
            1,
            0,
            0,
            IntegrityAlgorithm::None,
            CompressionAlgorithm::None,
            EncryptionAlgorithm::None,
            0,
        )
        .unwrap();
        let mut reader = wal_file.reader_from_offset(200).unwrap(); // Far past end
        assert!(reader.next().unwrap().is_none());
    }

    #[test]
    fn test_reader_corruption() {
        let fs = MemoryFileSystem::new();
        let mut file = fs.create_file("/test.wal").unwrap();
        let wal_file = WriteAheadLogFile::create(
            file.clone(),
            1,
            0,
            0,
            IntegrityAlgorithm::Crc32c,
            CompressionAlgorithm::None,
            EncryptionAlgorithm::None,
            0,
        )
        .unwrap();

        // Write some garbage after the header
        let garbage = [0u8; 20];
        file.write_to_offset(42, &garbage).unwrap();

        let mut reader = wal_file.reader_from_offset(42).unwrap();
        let res = reader.next();
        assert!(matches!(res, Err(WriteAheadLogError::Corruption { .. })));
    }
}
