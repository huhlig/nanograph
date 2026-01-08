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

use thiserror::Error;

/// B-tree specific errors
#[derive(Error, Debug)]
pub enum BTreeError {
    #[error("Key not found")]
    KeyNotFound,

    #[error("Node not found")]
    NodeNotFound,

    #[error("Node overflow: too many keys")]
    NodeOverflow,

    #[error("Node underflow: too few keys")]
    NodeUnderflow,

    #[error("Invalid node structure")]
    InvalidNode,

    #[error("Invalid operation: {0}")]
    InvalidOperation(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Serialization error: {0}")]
    Serialization(String),

    #[error("VFS error: {0}")]
    Vfs(#[from] nanograph_vfs::FileSystemError),

    #[error("WAL error: {0}")]
    Wal(#[from] nanograph_wal::WriteAheadLogError),

    #[error("Write conflict")]
    WriteConflict,

    #[error("Internal error: {0}")]
    Internal(String),
}

impl From<String> for BTreeError {
    fn from(s: String) -> Self {
        BTreeError::InvalidOperation(s)
    }
}

pub type BTreeResult<T> = Result<T, BTreeError>;

impl From<BTreeError> for nanograph_kvt::KeyValueError {
    fn from(err: BTreeError) -> Self {
        match err {
            BTreeError::KeyNotFound => nanograph_kvt::KeyValueError::KeyNotFound,
            BTreeError::Io(e) => nanograph_kvt::KeyValueError::IoError(
                nanograph_vfs::FileSystemError::IOError(e)
            ),
            BTreeError::Vfs(e) => nanograph_kvt::KeyValueError::IoError(e),
            BTreeError::WriteConflict => nanograph_kvt::KeyValueError::WriteConflict,
            other => nanograph_kvt::KeyValueError::StorageCorruption(other.to_string()),
        }
    }
}

// Made with Bob
