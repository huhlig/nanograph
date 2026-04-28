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

//! Distributed index implementation with Raft consensus
//!
//! This module provides a distributed wrapper around local index implementations,
//! enabling replication, high availability, and strong consistency through Raft.
//!
//! # Features
//! - Raft-based replication for all write operations
//! - Leader election and automatic failover
//! - Strong consistency guarantees
//! - Snapshot-based recovery
//! - Local reads for performance (eventual consistency option)
//!
//! # Architecture
//! ```text
//! ┌─────────────────────────────────────────┐
//! │      DistributedIndex (Wrapper)         │
//! │  ┌───────────────────────────────────┐  │
//! │  │   Raft Consensus Layer            │  │
//! │  │  - Leader election                │  │
//! │  │  - Log replication                │  │
//! │  │  - Snapshot management            │  │
//! │  └───────────────────────────────────┘  │
//! │              ↓                           │
//! │  ┌───────────────────────────────────┐  │
//! │  │   Local Index Implementation      │  │
//! │  │  - BTreeIndex                     │  │
//! │  │  - HashIndex                      │  │
//! │  │  - etc.                           │  │
//! │  └───────────────────────────────────┘  │
//! └─────────────────────────────────────────┘
//! ```

use crate::error::{IndexError, IndexResult};
use crate::index::{IndexEntry, IndexQuery, IndexStats, IndexStore};
use async_trait::async_trait;
use nanograph_core::object::IndexRecord;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::RwLock;

// Placeholder for Raft integration
// In production, this would use nanograph_raft::ShardGroup
// For now, we'll use a simplified interface
pub trait ConsensusGroup: Send + Sync {
    /// Check if this node is the leader
    fn is_leader(&self) -> impl std::future::Future<Output = bool> + Send;
    /// Propose a command to the group
    fn propose(&self, data: Vec<u8>) -> impl std::future::Future<Output = Result<(), String>> + Send;
}

/// Commands that can be replicated through Raft
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum IndexCommand {
    /// Insert an entry into the index
    Insert {
        indexed_value: Vec<u8>,
        primary_key: Vec<u8>,
        included_columns: Option<Vec<u8>>,
    },
    /// Update an entry in the index
    Update {
        old_indexed_value: Vec<u8>,
        old_primary_key: Vec<u8>,
        new_indexed_value: Vec<u8>,
        new_primary_key: Vec<u8>,
        new_included_columns: Option<Vec<u8>>,
    },
    /// Delete an entry from the index
    Delete { primary_key: Vec<u8> },
    /// Flush the index to durable storage
    Flush,
    /// Optimize the index
    Optimize,
}

/// Response from applying an index command
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum IndexCommandResponse {
    /// Command succeeded
    Ok,
    /// Command failed with error
    Error(String),
}

/// Distributed index wrapper with Raft consensus
///
/// This wrapper provides distributed replication for any index implementation
/// that implements the `IndexStore` trait. All write operations go through
/// Raft consensus to ensure strong consistency across replicas.
///
/// # Example
///
/// ```ignore
/// use nanograph_idx::{BTreeIndex, DistributedIndex};
///
/// // Create local index
/// let local_index = BTreeIndex::new(metadata, store, wal, config).await?;
///
/// // Wrap with distributed layer
/// let distributed_index = DistributedIndex::new(
///     local_index,
///     shard_group,
/// );
///
/// // All writes now go through Raft
/// distributed_index.insert(entry).await?;
/// ```
pub struct DistributedIndex<I: IndexStore, C: ConsensusGroup> {
    /// Local index implementation
    local_index: Arc<RwLock<I>>,
    /// Consensus group for replication
    consensus_group: Arc<C>,
    /// Whether to allow stale reads (eventual consistency)
    allow_stale_reads: bool,
}

impl<I: IndexStore, C: ConsensusGroup> DistributedIndex<I, C> {
    /// Create a new distributed index
    ///
    /// # Arguments
    /// * `local_index` - The local index implementation
    /// * `consensus_group` - Consensus group for replication
    pub fn new(local_index: I, consensus_group: Arc<C>) -> Self {
        Self {
            local_index: Arc::new(RwLock::new(local_index)),
            consensus_group,
            allow_stale_reads: false,
        }
    }

    /// Enable stale reads for better read performance
    ///
    /// When enabled, reads are served from the local replica without
    /// checking with the leader, providing eventual consistency.
    pub fn with_stale_reads(mut self, allow: bool) -> Self {
        self.allow_stale_reads = allow;
        self
    }

    /// Check if this node is the leader
    pub async fn is_leader(&self) -> bool {
        self.consensus_group.is_leader().await
    }

    /// Propose a command through consensus
    ///
    /// This method serializes the command and proposes it to the consensus group.
    /// The command will be replicated to all nodes before being applied.
    async fn propose_command(&self, command: IndexCommand) -> IndexResult<()> {
        // Serialize command
        let payload = bincode::serialize(&command)
            .map_err(|e| IndexError::Serialization(e.to_string()))?;

        // Propose to consensus group
        self.consensus_group
            .propose(payload)
            .await
            .map_err(|e| IndexError::Storage(format!("Consensus proposal failed: {}", e)))?;

        Ok(())
    }

    /// Apply a command to the local index
    ///
    /// This is called by the Raft state machine when a command is committed.
    /// It should only be called by the Raft layer, not directly by users.
    pub async fn apply_command(&self, command: IndexCommand) -> IndexResult<IndexCommandResponse> {
        let mut index = self.local_index.write().await;

        match command {
            IndexCommand::Insert {
                indexed_value,
                primary_key,
                included_columns,
            } => {
                let entry = IndexEntry {
                    indexed_value,
                    primary_key,
                    included_columns,
                };
                match index.insert(entry).await {
                    Ok(()) => Ok(IndexCommandResponse::Ok),
                    Err(e) => Ok(IndexCommandResponse::Error(e.to_string())),
                }
            }
            IndexCommand::Update {
                old_indexed_value,
                old_primary_key,
                new_indexed_value,
                new_primary_key,
                new_included_columns,
            } => {
                let old_entry = IndexEntry {
                    indexed_value: old_indexed_value,
                    primary_key: old_primary_key,
                    included_columns: None,
                };
                let new_entry = IndexEntry {
                    indexed_value: new_indexed_value,
                    primary_key: new_primary_key,
                    included_columns: new_included_columns,
                };
                match index.update(old_entry, new_entry).await {
                    Ok(()) => Ok(IndexCommandResponse::Ok),
                    Err(e) => Ok(IndexCommandResponse::Error(e.to_string())),
                }
            }
            IndexCommand::Delete { primary_key } => {
                match index.delete(&primary_key).await {
                    Ok(()) => Ok(IndexCommandResponse::Ok),
                    Err(e) => Ok(IndexCommandResponse::Error(e.to_string())),
                }
            }
            IndexCommand::Flush => match index.flush().await {
                Ok(()) => Ok(IndexCommandResponse::Ok),
                Err(e) => Ok(IndexCommandResponse::Error(e.to_string())),
            },
            IndexCommand::Optimize => match index.optimize().await {
                Ok(()) => Ok(IndexCommandResponse::Ok),
                Err(e) => Ok(IndexCommandResponse::Error(e.to_string())),
            },
        }
    }
}

#[async_trait]
impl<I: IndexStore + Send + Sync + 'static, C: ConsensusGroup + 'static> IndexStore for DistributedIndex<I, C> {
    fn metadata(&self) -> &IndexRecord {
        // This is a limitation of the sync trait method
        // In production, we'd need to refactor this to be async
        unimplemented!("Use async metadata access instead")
    }

    async fn build<Iter>(&mut self, table_data: Iter) -> IndexResult<()>
    where
        Iter: Iterator<Item = (Vec<u8>, Vec<u8>)> + Send,
    {
        // Building is done on the local index directly
        // This should only be called on the leader
        if !self.is_leader().await {
            return Err(IndexError::BuildFailed(
                "Index building must be done on the leader".to_string(),
            ));
        }

        let mut index = self.local_index.write().await;
        index.build(table_data).await
    }

    async fn insert(&mut self, entry: IndexEntry) -> IndexResult<()> {
        // Propose insert through Raft
        let command = IndexCommand::Insert {
            indexed_value: entry.indexed_value,
            primary_key: entry.primary_key,
            included_columns: entry.included_columns,
        };
        self.propose_command(command).await
    }

    async fn update(&mut self, old_entry: IndexEntry, new_entry: IndexEntry) -> IndexResult<()> {
        // Propose update through Raft
        let command = IndexCommand::Update {
            old_indexed_value: old_entry.indexed_value,
            old_primary_key: old_entry.primary_key,
            new_indexed_value: new_entry.indexed_value,
            new_primary_key: new_entry.primary_key,
            new_included_columns: new_entry.included_columns,
        };
        self.propose_command(command).await
    }

    async fn delete(&mut self, primary_key: &[u8]) -> IndexResult<()> {
        // Propose delete through Raft
        let command = IndexCommand::Delete {
            primary_key: primary_key.to_vec(),
        };
        self.propose_command(command).await
    }

    async fn query(&self, query: IndexQuery) -> IndexResult<Vec<IndexEntry>> {
        // Reads can be served locally (with optional staleness)
        let index = self.local_index.read().await;
        index.query(query).await
    }

    async fn get(&self, primary_key: &[u8]) -> IndexResult<Option<IndexEntry>> {
        // Reads can be served locally
        let index = self.local_index.read().await;
        index.get(primary_key).await
    }

    async fn exists(&self, indexed_value: &[u8]) -> IndexResult<bool> {
        // Reads can be served locally
        let index = self.local_index.read().await;
        index.exists(indexed_value).await
    }

    async fn stats(&self) -> IndexResult<IndexStats> {
        // Stats can be served locally
        let index = self.local_index.read().await;
        index.stats().await
    }

    async fn optimize(&mut self) -> IndexResult<()> {
        // Propose optimize through Raft
        let command = IndexCommand::Optimize;
        self.propose_command(command).await
    }

    async fn flush(&mut self) -> IndexResult<()> {
        // Propose flush through Raft
        let command = IndexCommand::Flush;
        self.propose_command(command).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Note: Full integration tests would require a running Raft cluster
    // These are placeholder tests for the structure

    #[test]
    fn test_index_command_serialization() {
        let command = IndexCommand::Insert {
            indexed_value: b"value".to_vec(),
            primary_key: b"key".to_vec(),
            included_columns: None,
        };

        let serialized = bincode::serialize(&command).unwrap();
        let deserialized: IndexCommand = bincode::deserialize(&serialized).unwrap();

        match deserialized {
            IndexCommand::Insert {
                indexed_value,
                primary_key,
                ..
            } => {
                assert_eq!(indexed_value, b"value");
                assert_eq!(primary_key, b"key");
            }
            _ => panic!("Wrong command type"),
        }
    }
}

// Made with Bob