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

use std::fmt;
use std::io;

/// LSM Tree specific errors
#[derive(Debug)]
pub enum LSMError {
    /// MemTable is full and cannot accept more writes
    MemTableFull {
        current_size: usize,
        max_size: usize,
    },

    /// Flush operation failed
    FlushFailed { reason: String },

    /// Compaction operation failed
    CompactionFailed { level: usize, reason: String },

    /// SSTable is corrupted
    SSTableCorrupted { file_number: u64, reason: String },

    /// Bloom filter error
    BloomFilterError { reason: String },

    /// Index is corrupted
    IndexCorrupted { file_number: u64, reason: String },

    /// Footer validation failed
    InvalidFooter {
        file_number: u64,
        expected_magic: u64,
        found_magic: u64,
    },

    /// Block checksum mismatch
    ChecksumMismatch {
        file_number: u64,
        block_offset: u64,
        expected: u32,
        found: u32,
    },

    /// File I/O error
    IoError {
        operation: String,
        path: String,
        source: io::Error,
    },

    /// Invalid configuration
    InvalidConfiguration { parameter: String, reason: String },

    /// Level not found
    LevelNotFound { level: usize },

    /// SSTable not found
    SSTableNotFound { file_number: u64 },

    /// Concurrent operation conflict
    ConcurrentOperationConflict { operation: String },

    /// Recovery failed
    RecoveryFailed { reason: String },
}

impl LSMError {
    /// Get a human-readable context for the error
    pub fn context(&self) -> String {
        match self {
            LSMError::MemTableFull {
                current_size,
                max_size,
            } => {
                format!(
                    "MemTable is full ({} bytes / {} bytes max). Flush is needed.",
                    current_size, max_size
                )
            }
            LSMError::FlushFailed { reason } => {
                format!("Failed to flush memtable to disk: {}", reason)
            }
            LSMError::CompactionFailed { level, reason } => {
                format!("Compaction failed at level {}: {}", level, reason)
            }
            LSMError::SSTableCorrupted {
                file_number,
                reason,
            } => {
                format!("SSTable {:06} is corrupted: {}", file_number, reason)
            }
            LSMError::BloomFilterError { reason } => {
                format!("Bloom filter error: {}", reason)
            }
            LSMError::IndexCorrupted {
                file_number,
                reason,
            } => {
                format!(
                    "Index in SSTable {:06} is corrupted: {}",
                    file_number, reason
                )
            }
            LSMError::InvalidFooter {
                file_number,
                expected_magic,
                found_magic,
            } => {
                format!(
                    "Invalid footer in SSTable {:06}: expected magic {:016x}, found {:016x}",
                    file_number, expected_magic, found_magic
                )
            }
            LSMError::ChecksumMismatch {
                file_number,
                block_offset,
                expected,
                found,
            } => {
                format!(
                    "Checksum mismatch in SSTable {:06} at offset {}: expected {:08x}, found {:08x}",
                    file_number, block_offset, expected, found
                )
            }
            LSMError::IoError {
                operation,
                path,
                source,
            } => {
                format!("I/O error during {}: {} ({})", operation, path, source)
            }
            LSMError::InvalidConfiguration { parameter, reason } => {
                format!("Invalid configuration for {}: {}", parameter, reason)
            }
            LSMError::LevelNotFound { level } => {
                format!("Level {} not found in LSM tree", level)
            }
            LSMError::SSTableNotFound { file_number } => {
                format!("SSTable {:06} not found", file_number)
            }
            LSMError::ConcurrentOperationConflict { operation } => {
                format!("Concurrent operation conflict: {}", operation)
            }
            LSMError::RecoveryFailed { reason } => {
                format!("Recovery failed: {}", reason)
            }
        }
    }

    /// Check if the error is recoverable
    pub fn is_recoverable(&self) -> bool {
        match self {
            LSMError::MemTableFull { .. } => true, // Can flush and retry
            LSMError::ConcurrentOperationConflict { .. } => true, // Can retry
            LSMError::IoError { .. } => false,     // Usually not recoverable
            LSMError::SSTableCorrupted { .. } => false,
            LSMError::IndexCorrupted { .. } => false,
            LSMError::ChecksumMismatch { .. } => false,
            LSMError::InvalidFooter { .. } => false,
            _ => false,
        }
    }

    /// Get the severity level of the error
    pub fn severity(&self) -> ErrorSeverity {
        match self {
            LSMError::MemTableFull { .. } => ErrorSeverity::Warning,
            LSMError::ConcurrentOperationConflict { .. } => ErrorSeverity::Warning,
            LSMError::FlushFailed { .. } => ErrorSeverity::Error,
            LSMError::CompactionFailed { .. } => ErrorSeverity::Error,
            LSMError::SSTableCorrupted { .. } => ErrorSeverity::Critical,
            LSMError::IndexCorrupted { .. } => ErrorSeverity::Critical,
            LSMError::ChecksumMismatch { .. } => ErrorSeverity::Critical,
            LSMError::InvalidFooter { .. } => ErrorSeverity::Critical,
            LSMError::RecoveryFailed { .. } => ErrorSeverity::Critical,
            _ => ErrorSeverity::Error,
        }
    }
}

/// Error severity levels
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ErrorSeverity {
    Warning,
    Error,
    Critical,
}

impl fmt::Display for LSMError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.context())
    }
}

impl std::error::Error for LSMError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            LSMError::IoError { source, .. } => Some(source),
            _ => None,
        }
    }
}

impl From<io::Error> for LSMError {
    fn from(err: io::Error) -> Self {
        LSMError::IoError {
            operation: "unknown".to_string(),
            path: "unknown".to_string(),
            source: err,
        }
    }
}

/// Convert LSMError to KeyValueError
impl From<LSMError> for nanograph_kvt::KeyValueError {
    fn from(err: LSMError) -> Self {
        match err {
            LSMError::MemTableFull { .. } => nanograph_kvt::KeyValueError::OutOfMemory,
            LSMError::SSTableCorrupted { reason, .. }
            | LSMError::IndexCorrupted { reason, .. }
            | LSMError::FlushFailed { reason }
            | LSMError::CompactionFailed { reason, .. }
            | LSMError::RecoveryFailed { reason } => {
                nanograph_kvt::KeyValueError::StorageCorruption(reason)
            }
            LSMError::IoError { source, .. } => {
                nanograph_kvt::KeyValueError::StorageCorruption(source.to_string())
            }
            LSMError::ConcurrentOperationConflict { .. } => {
                nanograph_kvt::KeyValueError::WriteConflict
            }
            _ => nanograph_kvt::KeyValueError::StorageCorruption(err.to_string()),
        }
    }
}

/// Result type for LSM operations
pub type LSMResult<T> = Result<T, LSMError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_context() {
        let err = LSMError::MemTableFull {
            current_size: 64 * 1024 * 1024,
            max_size: 64 * 1024 * 1024,
        };
        assert!(err.context().contains("MemTable is full"));
        assert!(err.is_recoverable());
        assert_eq!(err.severity(), ErrorSeverity::Warning);
    }

    #[test]
    fn test_error_severity() {
        let warning = LSMError::MemTableFull {
            current_size: 100,
            max_size: 100,
        };
        assert_eq!(warning.severity(), ErrorSeverity::Warning);

        let critical = LSMError::SSTableCorrupted {
            file_number: 1,
            reason: "test".to_string(),
        };
        assert_eq!(critical.severity(), ErrorSeverity::Critical);
        assert!(!critical.is_recoverable());
    }

    #[test]
    fn test_error_conversion() {
        let lsm_err = LSMError::MemTableFull {
            current_size: 100,
            max_size: 100,
        };
        let kv_err: nanograph_kvt::KeyValueError = lsm_err.into();
        assert!(matches!(kv_err, nanograph_kvt::KeyValueError::OutOfMemory));
    }
}
