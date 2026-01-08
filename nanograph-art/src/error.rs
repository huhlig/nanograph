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

/// Result type for ART operations
pub type Result<T> = std::result::Result<T, Error>;

/// Error types for Adaptive Radix Tree operations
#[derive(Debug, Error)]
pub enum Error {
    /// Key not found in the tree
    #[error("Key not found")]
    KeyNotFound,

    /// Key already exists in the tree
    #[error("Key already exists")]
    KeyExists,

    /// Invalid key (e.g., empty key)
    #[error("Invalid key: {0}")]
    InvalidKey(String),

    /// Node capacity exceeded
    #[error("Node capacity exceeded")]
    NodeCapacityExceeded,

    /// Internal error
    #[error("Internal error: {0}")]
    Internal(String),
}

// Made with Bob
