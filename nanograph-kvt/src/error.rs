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

use nanograph_core::object::{Permission, ResourceScope, ShardId, TablespaceId, UserId};
use thiserror::Error;

/// Result Type for KeyValue Operations
pub type KeyValueResult<T> = Result<T, KeyValueError>;

/// Error Type for KeyValue Operations
#[derive(Debug, Error)]
pub enum KeyValueError {
    // Core errors - all implementations
    /// Out Of Memory
    #[error("Out of memory")]
    OutOfMemory,
    /// Key not found in the tree
    #[error("Key not found")]
    KeyNotFound,
    /// Key already exists in the tree
    #[error("Key already exists")]
    KeyExists,
    /// Invalid key (e.g., empty key)
    #[error("Invalid key: {0}")]
    InvalidKey(String),
    /// Invalid value (e.g., bad value)
    #[error("Invalid value: {0}")]
    InvalidValue(String),

    // I/O errors (disk-backed implementations)
    /// VFS IO Error
    #[error("VFS IO error: {0}")]
    IoError(#[from] nanograph_vfs::FileSystemError),
    /// VFS IO Error
    #[error("std::io error: {0}")]
    StdIoError(#[from] std::io::Error),
    /// WAL IO Error
    #[error("WAL error: {0}")]
    WalError(#[from] nanograph_wal::WriteAheadLogError),
    /// Storage Corruption Error
    #[error("Storage Corruption Error: {0}")]
    StorageCorruption(String),

    // Concurrency
    #[error("Lock Timeout")]
    LockTimeout,
    #[error("Write conflict")]
    WriteConflict,
    #[error("Lock poisoned")]
    LockPoisoned,
    #[error("Snapshot expired: transaction exceeded deadline")]
    SnapshotExpired,

    // Operational
    /// attempted to write on a read-only instance
    #[error("Read Only")]
    ReadOnly,
    /// attempted an operation on an already closed store
    #[error("Store Closed")]
    Closed,

    // Capacity limits
    /// Storage Full
    #[error("Storage Full")]
    StorageFull,
    /// Value too Large
    #[error("Key too large: {size}/{max}")]
    KeyTooLarge { size: usize, max: usize },
    /// Value too Large
    #[error("Value too large: {size}/{max}")]
    ValueTooLarge { size: usize, max: usize },

    // Network & Consensus Errors
    /// Consensus Error
    #[error("Consensus error: {0}")]
    Consensus(String),
    /// Shard not found
    #[error("Shard not found: {0}")]
    ShardNotFound(ShardId),
    /// Invalid Operation
    #[error("Invalid operation: {0}")]
    InvalidOperation(String),

    // Snapshot errors
    /// Invalid snapshot format
    #[error("Invalid snapshot format")]
    InvalidSnapshotFormat,
    /// Unsupported snapshot version
    #[error("Unsupported snapshot version")]
    UnsupportedSnapshotVersion,

    // Serialization
    /// Serialization error
    #[error("Serialization error: {0}")]
    Serialization(String),
    /// Deserialization error
    #[error("Deserialization error: {0}")]
    Deserialization(String),

    // Security
    #[error(
        "Permission denied: {user_id} does not have permission {permission} on resource {resource}"
    )]
    PermissionDenied {
        user_id: UserId,
        permission: Permission,
        resource: ResourceScope,
    },
    #[error("Tablespace quota exceeded: {tablespace_id} (max: {size})")]
    TablespaceQuotaExceeded {
        tablespace_id: TablespaceId,
        size: usize,
    },

    /// Internal error
    #[error("Internal error: {0}")]
    Internal(String),
    #[error("Tablespace not found: {0}")]
    TablespaceNotFound(TablespaceId),
}
