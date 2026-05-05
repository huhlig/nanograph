//
// Copyright 2019-2026 Hans W. Uhlig. All Rights Reserved.
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

use crate::lsn::LogSequenceNumber;
use nanograph_vfs::FileSystemError;

/// Result Type for WAL operations
pub type WriteAheadLogResult<T> = Result<T, WriteAheadLogError>;

/// Error Type for WAL operations
#[derive(Debug)]
pub enum WriteAheadLogError {
    /// Error from the underlying file system
    FileSystem(FileSystemError),
    /// Data corruption detected at the given LSN
    Corruption {
        /// LSN where corruption was detected
        lsn: LogSequenceNumber,
    },
    /// The provided Log Sequence Number is invalid or not found
    InvalidLsn,
    /// Version mismatch between the WAL and the segment file
    VersionMismatch,
    /// An unexpected internal error occurred
    WrappedError(Box<dyn std::error::Error + Send + Sync>),
    /// Checksum verification failed
    ChecksumMismatch,
    /// A mutex lock was poisoned due to a panic in another thread
    LockPoisoned,
}

impl WriteAheadLogError {
    /// Wrap a generic error into a `WriteAheadLogError`
    #[must_use]
    pub fn wrap_error<E: std::error::Error + Send + Sync + 'static>(err: E) -> WriteAheadLogError {
        WriteAheadLogError::WrappedError(Box::new(err))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nanograph_vfs::FileSystemError;

    #[test]
    fn test_error_display() {
        let err = WriteAheadLogError::InvalidLsn;
        assert!(format!("{}", err).contains("InvalidLsn"));

        let fs_err = WriteAheadLogError::FileSystem(FileSystemError::PathMissing);
        assert!(format!("{}", fs_err).contains("FileSystem"));
    }

    #[test]
    fn test_wrap_error() {
        let std_err = std::io::Error::new(std::io::ErrorKind::Other, "test error");
        let err = WriteAheadLogError::wrap_error(std_err);
        if let WriteAheadLogError::WrappedError(e) = err {
            assert_eq!(format!("{}", e), "test error");
        } else {
            panic!("Expected WrappedError");
        }
    }
}

impl std::fmt::Display for WriteAheadLogError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Debug::fmt(self, f)
    }
}

impl std::error::Error for WriteAheadLogError {}
