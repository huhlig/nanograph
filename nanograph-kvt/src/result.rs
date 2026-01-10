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

use nanograph_vfs::FileSystemError;

/// Result Type for KeyValue Operations
pub type KeyValueResult<T> = Result<T, KeyValueError>;

/// Error Type for KeyValue Operations
#[derive(Debug)]
pub enum KeyValueError {
    // Core errors - all implementations
    KeyNotFound,
    OutOfMemory,
    InvalidKey(String),
    InvalidValue(String),

    // I/O errors (disk-backed implementations)
    IoError(FileSystemError),
    StorageCorruption(String),

    // Concurrency
    LockTimeout,
    WriteConflict,

    // Operational
    ReadOnly, // attempted write on read-only instance
    Closed,   // operation on closed store

    // Capacity limits
    StorageFull,
    KeyTooLarge { size: usize, max: usize },
    ValueTooLarge { size: usize, max: usize },

    // Network & Consensus Errors
    Consensus(String),
}

impl std::fmt::Display for KeyValueError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            KeyValueError::KeyNotFound => write!(f, "Key not found"),
            KeyValueError::OutOfMemory => write!(f, "Out of memory"),
            KeyValueError::InvalidKey(key) => write!(f, "Invalid key: {}", key),
            KeyValueError::InvalidValue(key) => write!(f, "Invalid value: {}", key),
            KeyValueError::IoError(err) => write!(f, "IO error: {}", err),
            KeyValueError::StorageCorruption(err) => write!(f, "Storage error: {}", err),
            KeyValueError::LockTimeout => write!(f, "Lock timeout"),
            KeyValueError::WriteConflict => write!(f, "Write conflict"),
            KeyValueError::ReadOnly => write!(f, "Read only"),
            KeyValueError::Closed => write!(f, "Key closed"),
            KeyValueError::StorageFull => write!(f, "Storage full"),
            KeyValueError::KeyTooLarge { size, max } => {
                write!(f, "Key is too large: {} > {}", size, max)
            }
            KeyValueError::ValueTooLarge { size, max } => {
                write!(f, "Value is too large: {} > {}", size, max)
            }
            KeyValueError::Consensus(msg) => {
                write!(f, "Consensus error: {}", msg)
            }
        }
    }
}

impl std::error::Error for KeyValueError {}
