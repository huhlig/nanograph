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

//! Adapter for integrating DistributedIndex with nanograph-raft
//!
//! This module provides an implementation of the `ConsensusGroup` trait
//! that wraps `TableShardRaftGroup` from nanograph-raft, enabling
//! distributed indexes to use Raft consensus for replication.
//!
//! This module is only available when the `raft` feature is enabled.

#![cfg(feature = "raft")]

use crate::distributed::ConsensusGroup;
use nanograph_raft::group::TableShardRaftGroup;
use std::sync::Arc;

/// Adapter that implements ConsensusGroup for TableShardRaftGroup
///
/// This allows DistributedIndex to work with the actual Raft implementation
/// from nanograph-raft without tight coupling.
///
/// # Example
///
/// ```ignore
/// use nanograph_idx::{DistributedIndex, FullTextIndex};
/// use nanograph_idx::raft_adapter::RaftConsensusAdapter;
/// use nanograph_raft::group::TableShardRaftGroup;
///
/// // Create Raft group
/// let raft_group = TableShardRaftGroup::new(...).await?;
/// let adapter = RaftConsensusAdapter::new(raft_group);
///
/// // Create distributed index
/// let local_index = FullTextIndex::new(...).await?;
/// let distributed = DistributedIndex::new(local_index, Arc::new(adapter));
/// ```
pub struct RaftConsensusAdapter {
    raft_group: Arc<TableShardRaftGroup>,
}

impl RaftConsensusAdapter {
    /// Create a new adapter wrapping a Raft group
    pub fn new(raft_group: Arc<TableShardRaftGroup>) -> Self {
        Self { raft_group }
    }

    /// Get reference to the underlying Raft group
    pub fn raft_group(&self) -> &Arc<TableShardRaftGroup> {
        &self.raft_group
    }
}

impl ConsensusGroup for RaftConsensusAdapter {
    fn is_leader(&self) -> impl std::future::Future<Output = bool> + Send {
        let raft_group = self.raft_group.clone();
        async move { raft_group.is_leader().await }
    }

    fn propose(&self, data: Vec<u8>) -> impl std::future::Future<Output = Result<(), String>> + Send {
        let raft_group = self.raft_group.clone();
        async move {
            // Convert the serialized index command into a Raft Operation
            // For now, we'll use a simple Put operation with a special key prefix
            // In production, you might want a more sophisticated encoding
            let operation = nanograph_raft::types::Operation::Put {
                key: b"index_command".to_vec(),
                value: data,
            };

            raft_group
                .propose_write(operation)
                .await
                .map(|_| ())
                .map_err(|e| format!("Raft proposal failed: {}", e))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Note: Full integration tests would require a running Raft cluster
    // These are placeholder tests for the structure

    #[test]
    fn test_adapter_creation() {
        // This test would require setting up a mock Raft group
        // For now, it's a placeholder to show the intended structure
    }
}

// Made with Bob
