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

use nanograph_kvt::ShardId;
use thiserror::Error;

pub type ArtResult<T> = Result<T, ArtError>;

#[derive(Debug, Error)]
pub enum ArtError {
    #[error("Invalid Key: {0}")]
    InvalidKey(String),

    #[error("Node capacity exceeded")]
    NodeCapacityExceeded,

    #[error("Shard not found: {0}")]
    ShardNotFound(ShardId),

    #[error("Storage Corruption: {0}")]
    StorageCorruption(String),

    #[error("Internal Error: {0}")]
    Internal(String),
}

impl From<ArtError> for nanograph_kvt::KeyValueError {
    fn from(err: ArtError) -> Self {
        match err {
            ArtError::InvalidKey(v) => nanograph_kvt::KeyValueError::InvalidKey(v),
            ArtError::NodeCapacityExceeded => {
                nanograph_kvt::KeyValueError::Internal("Node capacity exceeded".to_string())
            }
            ArtError::ShardNotFound(e) => nanograph_kvt::KeyValueError::ShardNotFound(e),
            ArtError::StorageCorruption(e) => nanograph_kvt::KeyValueError::StorageCorruption(e),
            other => nanograph_kvt::KeyValueError::Internal(other.to_string()),
        }
    }
}
