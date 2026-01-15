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

//! Error types for Raft consensus operations

use nanograph_core::{
    object::{NodeId, ShardId},
    types::Timestamp,
};

/// Result type for Raft operations
pub type ConsensusResult<T> = Result<T, ConsensusError>;

/// Errors that can occur during Raft operations
#[derive(Debug)]
pub enum ConsensusError {
    /// Not the leader for this shard
    NotLeader {
        shard_id: ShardId,
        leader: Option<NodeId>,
    },

    /// No quorum available
    NoQuorum {
        shard_id: ShardId,
        required: usize,
        available: usize,
    },

    /// Shard not found
    ShardNotFound { shard_id: ShardId },

    /// Node not found
    NodeNotFound { node_id: NodeId },

    /// Operation timeout
    Timeout {
        operation: String,
        timeout_ms: Timestamp,
    },

    /// Network error
    Network { message: String },

    /// Storage error
    Storage { message: String },

    /// Serialization error
    Serialization { message: String },

    /// Configuration error
    Configuration { message: String },

    /// Raft protocol error
    Protocol { message: String },

    /// Internal error
    Internal { message: String },
}

impl std::fmt::Display for ConsensusError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ConsensusError::NotLeader { shard_id, leader } => {
                write!(f, "Not leader for shard {}", shard_id)?;
                if let Some(leader_id) = leader {
                    write!(f, ", leader is node {}", leader_id)?;
                }
                Ok(())
            }
            ConsensusError::NoQuorum {
                shard_id,
                required,
                available,
            } => {
                write!(
                    f,
                    "No quorum for shard {}: required {}, available {}",
                    shard_id, required, available
                )
            }
            ConsensusError::ShardNotFound { shard_id } => {
                write!(f, "Shard {} not found", shard_id)
            }
            ConsensusError::NodeNotFound { node_id } => {
                write!(f, "Node {} not found", node_id)
            }
            ConsensusError::Timeout {
                operation,
                timeout_ms,
            } => {
                write!(
                    f,
                    "Operation '{}' timed out after {}ms",
                    operation, timeout_ms
                )
            }
            ConsensusError::Network { message } => {
                write!(f, "Network error: {}", message)
            }
            ConsensusError::Storage { message } => {
                write!(f, "Storage error: {}", message)
            }
            ConsensusError::Serialization { message } => {
                write!(f, "Serialization error: {}", message)
            }
            ConsensusError::Configuration { message } => {
                write!(f, "Configuration error: {}", message)
            }
            ConsensusError::Protocol { message } => {
                write!(f, "Raft protocol error: {}", message)
            }
            ConsensusError::Internal { message } => {
                write!(f, "Internal error: {}", message)
            }
        }
    }
}

impl std::error::Error for ConsensusError {}

// Conversion from openraft errors
impl From<openraft::error::RaftError<NodeId>> for ConsensusError {
    fn from(err: openraft::error::RaftError<NodeId>) -> Self {
        ConsensusError::Protocol {
            message: err.to_string(),
        }
    }
}
