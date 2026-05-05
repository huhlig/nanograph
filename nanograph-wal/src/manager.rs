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

use crate::config::WriteAheadLogConfig;
use crate::lsn::LogSequenceNumber;
use crate::metrics;
use crate::reader::WriteAheadLogReader;
use crate::result::{WriteAheadLogError, WriteAheadLogResult};
use crate::walfile::WriteAheadLogFile;
use crate::writer::WriteAheadLogWriter;
use nanograph_vfs::{DynamicFileSystem, Path};
use std::sync::{Arc, Mutex};

/// Write Ahead Log Manager.
/// Manages multiple WAL segments and provides access to writers and readers.
///
/// # Examples
///
/// ```rust
/// # use nanograph_wal::{WriteAheadLogManager, WriteAheadLogConfig};
/// # use nanograph_vfs::MemoryFileSystem;
/// # let fs = MemoryFileSystem::new();
/// # let config = WriteAheadLogConfig::new(0);
/// let manager = WriteAheadLogManager::new(fs, "/wal", config)?;
/// let writer = manager.writer()?;
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub struct WriteAheadLogManager {
    #[allow(dead_code)]
    file_system: Arc<dyn DynamicFileSystem>,
    #[allow(dead_code)]
    root_folder: Path,
    #[allow(dead_code)]
    shard_id: u128,
    active_segment: Arc<Mutex<WriteAheadLogFile>>,
    archived_segments: Arc<Mutex<Vec<WriteAheadLogFile>>>,
}

impl WriteAheadLogManager {
    /// Initialize the WAL manager.
    /// Opens or creates the specified directory and initializes the active segment.
    pub fn new<FS: DynamicFileSystem + 'static>(
        file_system: FS,
        directory: impl Into<Path>,
        config: WriteAheadLogConfig,
    ) -> WriteAheadLogResult<Self> {
        let root_folder = directory.into().clone();
        if !file_system
            .exists(&root_folder.to_string())
            .map_err(WriteAheadLogError::FileSystem)?
        {
            file_system
                .create_directory_all(&root_folder.to_string())
                .map_err(WriteAheadLogError::FileSystem)?;
        }

        let shard_id = config.shard_id;
        let segment_id = 0;
        let segment_path = root_folder.join(&Path::parse(&format!("segment_{}.wal", segment_id)));

        let active_segment = if file_system
            .exists(&segment_path.to_string())
            .map_err(WriteAheadLogError::FileSystem)?
        {
            let file = file_system
                .open_file(&segment_path.to_string())
                .map_err(WriteAheadLogError::FileSystem)?;
            WriteAheadLogFile::open_existing(file)?
        } else {
            let file = file_system
                .create_file(&segment_path.to_string())
                .map_err(WriteAheadLogError::FileSystem)?;
            WriteAheadLogFile::create(
                file,
                shard_id,
                segment_id,
                0,
                config.checksum,
                config.compression,
                config.encryption,
                0,
            )?
        };

        // Record initial metrics
        metrics::record_active_segments(shard_id, 1);
        let tail_lsn = active_segment.tail_lsn();
        let lsn_value = ((tail_lsn.segment_id as u128) << 64) | (tail_lsn.offset as u128);
        metrics::record_current_lsn(shard_id, lsn_value);

        Ok(Self {
            file_system: Arc::new(file_system),
            root_folder,
            shard_id,
            active_segment: Arc::new(Mutex::new(active_segment)),
            archived_segments: Arc::new(Mutex::new(Vec::new())),
        })
    }

    /// Create a new writer for the current active segment.
    pub fn writer(&self) -> WriteAheadLogResult<WriteAheadLogWriter> {
        Ok(WriteAheadLogWriter::new(self.active_segment.clone()))
    }

    /// Create a reader starting from a given LSN.
    /// Will search through active and archived segments for the record.
    pub fn reader_from(&self, lsn: LogSequenceNumber) -> WriteAheadLogResult<WriteAheadLogReader> {
        let result = (|| {
            let active = self.active_segment.lock().map_err(|_| WriteAheadLogError::LockPoisoned)?;
            if active.segment_id() == lsn.segment_id {
                return active.reader_from_offset(lsn.offset);
            }

            let archived = self.archived_segments.lock().map_err(|_| WriteAheadLogError::LockPoisoned)?;
            for segment in archived.iter() {
                if segment.segment_id() == lsn.segment_id {
                    return segment.reader_from_offset(lsn.offset);
                }
            }

            Err(WriteAheadLogError::InvalidLsn)
        })();

        metrics::record_operation("reader_from", self.shard_id, result.is_ok());
        result
    }

    /// Get the oldest available Log Sequence Number.
    ///
    /// The head LSN represents the point from which records are available.
    /// It changes when the WAL is truncated.
    pub fn head_lsn(&self) -> WriteAheadLogResult<LogSequenceNumber> {
        let archived = self.archived_segments.lock().map_err(|_| WriteAheadLogError::LockPoisoned)?;
        if let Some(first) = archived.first() {
            // Return the start LSN from the segment (which accounts for the header)
            Ok(first.start_lsn())
        } else {
            let active = self.active_segment.lock().map_err(|_| WriteAheadLogError::LockPoisoned)?;
            // Return the start LSN from the segment (which accounts for the header)
            Ok(active.start_lsn())
        }
    }

    /// Get the newest committed Log Sequence Number.
    ///
    /// The tail LSN represents the point where the next record will be written.
    pub fn tail_lsn(&self) -> WriteAheadLogResult<LogSequenceNumber> {
        let active = self.active_segment.lock().map_err(|_| WriteAheadLogError::LockPoisoned)?;
        Ok(active.tail_lsn())
    }

    /// Remove segments strictly before the specified LSN.
    pub fn truncate_before(&self, lsn: LogSequenceNumber) -> WriteAheadLogResult<()> {
        let mut archived = self.archived_segments.lock().map_err(|_| WriteAheadLogError::LockPoisoned)?;
        let before_count = archived.len();

        // Collect segment IDs to delete before modifying the vector
        let segments_to_delete: Vec<u64> = archived
            .iter()
            .filter(|s| s.segment_id() < lsn.segment_id)
            .map(|s| s.segment_id())
            .collect();

        // Remove segments from the archived list
        archived.retain(|s| s.segment_id() >= lsn.segment_id);
        let after_count = archived.len();

        // Drop the lock before doing file I/O
        drop(archived);

        // Delete the actual segment files from the filesystem
        let mut delete_errors: Vec<(u64, nanograph_vfs::FileSystemError)> = Vec::new();
        for segment_id in segments_to_delete {
            let segment_path = self
                .root_folder
                .join(&Path::parse(&format!("segment_{}.wal", segment_id)));
            if let Err(e) = self.file_system.remove_file(&segment_path.to_string()) {
                tracing::warn!(
                    shard_id = self.shard_id,
                    segment_id = segment_id,
                    error = ?e,
                    "Failed to delete WAL segment file"
                );
                delete_errors.push((segment_id, e));
            } else {
                tracing::debug!(
                    shard_id = self.shard_id,
                    segment_id = segment_id,
                    "Deleted WAL segment file"
                );
            }
        }

        // Update metrics
        metrics::record_active_segments(self.shard_id, after_count + 1); // +1 for active segment

        let success = delete_errors.is_empty();
        metrics::record_operation("truncate", self.shard_id, success);

        if !delete_errors.is_empty() {
            tracing::error!(
                shard_id = self.shard_id,
                failed_deletes = delete_errors.len(),
                "Some WAL segment files could not be deleted"
            );
            // Return error for the first failed deletion
            return Err(WriteAheadLogError::wrap_error(std::io::Error::new(
                std::io::ErrorKind::Other,
                format!(
                    "Failed to delete WAL segment {}: {:?}",
                    delete_errors[0].0, delete_errors[0].1
                ),
            )));
        }

        tracing::debug!(
            shard_id = self.shard_id,
            removed_segments = before_count - after_count,
            "Truncated WAL segments"
        );
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{
        CompressionAlgorithm, Durability, EncryptionAlgorithm, IntegrityAlgorithm,
    };
    use crate::writer::WriteAheadLogRecord;
    use nanograph_vfs::MemoryFileSystem;

    #[test]
    fn test_manager_basic_flow() {
        let fs = MemoryFileSystem::new();
        let config = WriteAheadLogConfig {
            shard_id: 0,
            max_segment_size: 1024,
            sync_on_rotate: true,
            checksum: IntegrityAlgorithm::None,
            compression: CompressionAlgorithm::None,
            encryption: EncryptionAlgorithm::None,
            encryption_key: None,
        };
        let manager = WriteAheadLogManager::new(fs, "/wal", config).unwrap();
        let mut writer = manager.writer().unwrap();

        let record = WriteAheadLogRecord {
            kind: 1,
            payload: b"test_data",
        };
        let lsn = writer.append(record, Durability::Buffered).unwrap();

        let mut reader = manager.reader_from(lsn).unwrap();
        let entry = reader.next().unwrap().unwrap();
        assert_eq!(entry.kind, 1);
        assert_eq!(entry.payload, b"test_data");
        assert_eq!(entry.lsn, lsn);
    }
}
