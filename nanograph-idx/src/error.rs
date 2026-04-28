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

/// Result type for index operations
pub type IndexResult<T> = Result<T, IndexError>;

/// Errors that can occur during index operations
#[derive(Debug, Error)]
pub enum IndexError {
    /// Index not found
    #[error("Index not found: {0}")]
    NotFound(String),

    /// Index already exists
    #[error("Index already exists: {0}")]
    AlreadyExists(String),

    /// Unique constraint violation
    #[error("Unique constraint violation: {0}")]
    UniqueViolation(String),

    /// Invalid index configuration
    #[error("Invalid index configuration: {0}")]
    InvalidConfig(String),

    /// Index build failed
    #[error("Index build failed: {0}")]
    BuildFailed(String),

    /// Index query failed
    #[error("Index query failed: {0}")]
    QueryFailed(String),

    /// Index is not ready (still building)
    #[error("Index is not ready: {0}")]
    NotReady(String),

    /// Storage error
    #[error("Storage error: {0}")]
    Storage(String),

    /// Serialization error
    #[error("Serialization error: {0}")]
    Serialization(String),

    /// I/O error
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// Key-value store error
    #[error("Key-value store error: {0}")]
    KeyValueStore(#[from] nanograph_kvt::KeyValueError),
}
