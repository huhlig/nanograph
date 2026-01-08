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

use crate::config::Durability;
use crate::lsn::LogSequenceNumber;
use crate::metrics;
use crate::result::WriteAheadLogResult;
use crate::walfile::WriteAheadLogFile;
use std::sync::{Arc, Mutex};
use std::time::Instant;

/// Writer for the Write Ahead Log
///
/// Provides methods to append records to the active WAL segment with different
/// durability guarantees.
///
/// # Examples
///
/// ```rust
/// # use nanograph_wal::{WriteAheadLogManager, WriteAheadLogConfig, WriteAheadLogRecord, Durability};
/// # use nanograph_vfs::MemoryFileSystem;
/// # let fs = MemoryFileSystem::new();
/// # let config = WriteAheadLogConfig::new(0);
/// # let wal = WriteAheadLogManager::new(fs, "/wal", config)?;
/// let mut writer = wal.writer()?;
///
/// let record = WriteAheadLogRecord {
///     kind: 1,
///     payload: b"my record data",
/// };
///
/// writer.append(record, Durability::Sync)?;
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub struct WriteAheadLogWriter {
    active_segment: Arc<Mutex<WriteAheadLogFile>>,
}

impl WriteAheadLogWriter {
    pub(crate) fn new(active_segment: Arc<Mutex<WriteAheadLogFile>>) -> Self {
        Self { active_segment }
    }

    /// Append a single WAL record with specified durability
    #[tracing::instrument(level = "trace", skip(self, record))]
    pub fn append(
        &mut self,
        record: WriteAheadLogRecord<'_>,
        durability: Durability,
    ) -> WriteAheadLogResult<LogSequenceNumber> {
        let start = Instant::now();
        let mut segment = self.active_segment.lock().unwrap();
        let shard_id = segment.shard_id();

        let result = segment.append(&record);
        let lsn = match result {
            Ok(lsn) => {
                metrics::record_bytes_written(shard_id, record.payload.len() as u64);
                metrics::record_records_appended(shard_id, 1);
                metrics::record_record_size(shard_id, record.payload.len());
                lsn
            }
            Err(e) => {
                metrics::record_operation("append", shard_id, false);
                return Err(e);
            }
        };

        let flush_result = match durability {
            Durability::Memory => Ok(()),
            Durability::Flush => {
                let result = segment.flush();
                if result.is_ok() {
                    metrics::record_flush(shard_id, true);
                } else {
                    metrics::record_flush(shard_id, false);
                }
                result
            }
            Durability::Sync => {
                let result = segment.sync();
                if result.is_ok() {
                    metrics::record_sync(shard_id, true);
                } else {
                    metrics::record_sync(shard_id, false);
                }
                result
            }
        };

        let duration = start.elapsed().as_micros() as u64;
        metrics::record_operation_duration("append", shard_id, duration);
        metrics::record_operation("append", shard_id, flush_result.is_ok());

        // Update current LSN
        let lsn_value = ((lsn.segment_id as u128) << 64) | (lsn.offset as u128);
        metrics::record_current_lsn(shard_id, lsn_value);

        flush_result?;
        Ok(lsn)
    }

    /// Append multiple records atomically with specified durability
    #[tracing::instrument(level = "trace", skip(self, records))]
    pub fn append_batch<'a>(
        &mut self,
        records: impl IntoIterator<Item = WriteAheadLogRecord<'a>>,
        durability: Durability,
    ) -> WriteAheadLogResult<LogSequenceNumber> {
        let start = Instant::now();
        let mut segment = self.active_segment.lock().unwrap();
        let shard_id = segment.shard_id();
        let mut last_lsn = LogSequenceNumber::ZERO;
        let mut record_count = 0u64;
        let mut total_bytes = 0u64;

        for record in records {
            match segment.append(&record) {
                Ok(lsn) => {
                    last_lsn = lsn;
                    record_count += 1;
                    total_bytes += record.payload.len() as u64;
                    metrics::record_record_size(shard_id, record.payload.len());
                }
                Err(e) => {
                    metrics::record_operation("append_batch", shard_id, false);
                    return Err(e);
                }
            }
        }

        metrics::record_bytes_written(shard_id, total_bytes);
        metrics::record_records_appended(shard_id, record_count);

        let flush_result = match durability {
            Durability::Memory => Ok(()),
            Durability::Flush => {
                let result = segment.flush();
                if result.is_ok() {
                    metrics::record_flush(shard_id, true);
                } else {
                    metrics::record_flush(shard_id, false);
                }
                result
            }
            Durability::Sync => {
                let result = segment.sync();
                if result.is_ok() {
                    metrics::record_sync(shard_id, true);
                } else {
                    metrics::record_sync(shard_id, false);
                }
                result
            }
        };

        let duration = start.elapsed().as_micros() as u64;
        metrics::record_operation_duration("append_batch", shard_id, duration);
        metrics::record_operation("append_batch", shard_id, flush_result.is_ok());

        // Update current LSN
        let lsn_value = ((last_lsn.segment_id as u128) << 64) | (last_lsn.offset as u128);
        metrics::record_current_lsn(shard_id, lsn_value);

        flush_result?;
        Ok(last_lsn)
    }

    /// Explicitly flush buffered writes to OS buffers
    #[tracing::instrument(level = "trace", skip(self))]
    pub fn flush(&mut self) -> WriteAheadLogResult<()> {
        let start = Instant::now();
        let mut segment = self.active_segment.lock().unwrap();
        let shard_id = segment.shard_id();
        let result = segment.flush();

        let duration = start.elapsed().as_micros() as u64;
        metrics::record_operation_duration("flush", shard_id, duration);
        metrics::record_flush(shard_id, result.is_ok());

        result
    }

    /// Force fsync on the active segment to ensure persistence
    #[tracing::instrument(level = "trace", skip(self))]
    pub fn sync(&mut self) -> WriteAheadLogResult<()> {
        let start = Instant::now();
        let mut segment = self.active_segment.lock().unwrap();
        let shard_id = segment.shard_id();
        let result = segment.sync();

        let duration = start.elapsed().as_micros() as u64;
        metrics::record_operation_duration("sync", shard_id, duration);
        metrics::record_sync(shard_id, result.is_ok());

        result
    }

    /// Get the current end-of-log position (Tail LSN)
    pub fn tail_lsn(&self) -> LogSequenceNumber {
        let segment = self.active_segment.lock().unwrap();
        segment.tail_lsn()
    }
}

/// A single record to be written to the Write Ahead Log
pub struct WriteAheadLogRecord<'a> {
    /// Application-defined record type identifier
    pub kind: u16,
    /// Binary payload of the record
    pub payload: &'a [u8],
}

impl std::fmt::Debug for WriteAheadLogRecord<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WriteAheadLogRecord")
            .field("kind", &self.kind)
            .field("payload_len", &self.payload.len())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Durability;
    use nanograph_util::{CompressionAlgorithm, EncryptionAlgorithm, IntegrityAlgorithm};
    use nanograph_vfs::{FileSystem, MemoryFileSystem};

    #[test]
    fn test_writer_append() {
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
        let mut writer = WriteAheadLogWriter::new(Arc::new(Mutex::new(wal_file)));

        let record = WriteAheadLogRecord {
            kind: 1,
            payload: b"test",
        };
        let lsn = writer.append(record, Durability::Memory).unwrap();
        assert_eq!(lsn.segment_id, 0);
        assert!(lsn.offset > 0);
    }

    #[test]
    fn test_writer_append_batch() {
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
        let mut writer = WriteAheadLogWriter::new(Arc::new(Mutex::new(wal_file)));

        let records = vec![
            WriteAheadLogRecord {
                kind: 1,
                payload: b"one",
            },
            WriteAheadLogRecord {
                kind: 2,
                payload: b"two",
            },
        ];
        let lsn = writer.append_batch(records, Durability::Flush).unwrap();
        assert_eq!(lsn.segment_id, 0);
        assert!(lsn.offset > 0);
    }
}
