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

/// LMDB-specific errors
#[derive(Debug, Error)]
pub enum LMDBError {
    #[error("LMDB error: {0}")]
    LmdbError(#[from] lmdb::Error),

    #[error("Shard not found: {0}")]
    ShardNotFound(u128),

    #[error("Invalid configuration: {0}")]
    InvalidConfig(String),

    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("Serialization error: {0}")]
    SerializationError(String),

    #[error("Transaction error: {0}")]
    TransactionError(String),

    #[error("Database full")]
    DatabaseFull,

    #[error("Key too large")]
    KeyTooLarge,

    #[error("Value too large")]
    ValueTooLarge,
}

pub type LMDBResult<T> = Result<T, LMDBError>;

/// Convert LMDB errors to KeyValueError
impl From<LMDBError> for nanograph_kvt::KeyValueError {
    fn from(err: LMDBError) -> Self {
        match err {
            LMDBError::ShardNotFound(id) => {
                nanograph_kvt::KeyValueError::ShardNotFound(nanograph_kvt::ShardId::new(id))
            }
            LMDBError::InvalidConfig(msg) => nanograph_kvt::KeyValueError::InvalidValue(msg),
            LMDBError::IoError(e) => nanograph_kvt::KeyValueError::StorageCorruption(e.to_string()),
            LMDBError::SerializationError(msg) => nanograph_kvt::KeyValueError::InvalidValue(msg),
            LMDBError::TransactionError(msg) => nanograph_kvt::KeyValueError::StorageCorruption(format!("Transaction error: {}", msg)),
            LMDBError::DatabaseFull => nanograph_kvt::KeyValueError::StorageCorruption("Database full".to_string()),
            LMDBError::KeyTooLarge => nanograph_kvt::KeyValueError::InvalidKey("Key too large".to_string()),
            LMDBError::ValueTooLarge => nanograph_kvt::KeyValueError::InvalidValue("Value too large".to_string()),
            LMDBError::LmdbError(e) => nanograph_kvt::KeyValueError::StorageCorruption(e.to_string()),
        }
    }
}

// Made with Bob
