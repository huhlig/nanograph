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

//! Storage adapter for openraft integration

use crate::error::{RaftError, Result};
use crate::types::{Operation, OperationResponse};
use nanograph_kvt::{KeyValueShardStore, NodeId, ShardId};
use std::sync::Arc;
use tokio::sync::RwLock;

/// Snapshot data for a shard
#[derive(Clone, Debug)]
pub struct ShardSnapshot {
    /// Snapshot metadata
    pub meta: SnapshotMeta,

    /// Snapshot data (serialized key-value pairs)
    pub data: Vec<u8>,
}

/// Snapshot metadata
#[derive(Clone, Debug)]
pub struct SnapshotMeta {
    /// Last included index
    pub last_included_index: u64,

    /// Last included term
    pub last_included_term: u64,

    /// Shard ID
    pub shard_id: ShardId,

    /// Snapshot size in bytes
    pub size_bytes: u64,
}

/// Storage adapter that implements openraft's RaftStorage trait
///
/// This bridges openraft's consensus layer with Nanograph's KeyValueStore trait.
/// Each shard has its own storage adapter backed by a storage engine (LSM, B+Tree, etc).
pub struct RaftStorageAdapter {
    /// Underlying storage engine
    storage: Arc<RwLock<Box<dyn KeyValueShardStore>>>,

    /// Shard identifier
    shard_id: ShardId,

    /// Current Raft state (term, voted_for, etc)
    raft_state: Arc<RwLock<RaftState>>,

    /// Log entries (in-memory for now, will be backed by WAL)
    log_entries: Arc<RwLock<Vec<LogEntry>>>,
}

/// Raft state (persisted)
#[derive(Clone, Debug)]
struct RaftState {
    /// Current term
    term: u64,

    /// Node we voted for in current term
    voted_for: Option<NodeId>,

    /// Last applied index
    last_applied: u64,

    /// Commit index
    commit_index: u64,
}

impl Default for RaftState {
    fn default() -> Self {
        Self {
            term: 0,
            voted_for: None,
            last_applied: 0,
            commit_index: 0,
        }
    }
}

/// Log entry
#[derive(Clone, Debug)]
pub struct LogEntry {
    /// Log index
    pub index: u64,

    /// Term when entry was received
    pub term: u64,

    /// Operation to apply
    pub operation: Operation,
}

impl RaftStorageAdapter {
    /// Create a new storage adapter
    pub fn new(storage: Box<dyn KeyValueShardStore>, shard_id: ShardId) -> Self {
        Self {
            storage: Arc::new(RwLock::new(storage)),
            shard_id,
            raft_state: Arc::new(RwLock::new(RaftState::default())),
            log_entries: Arc::new(RwLock::new(Vec::new())),
        }
    }

    /// Apply an operation to the storage engine
    pub async fn apply_operation(&self, operation: &Operation) -> Result<OperationResponse> {
        match operation {
            Operation::Put { key, value } => {
                let storage = self.storage.write().await;
                storage
                    .put(self.shard_id, key, value)
                    .await
                    .map_err(|e| RaftError::Storage {
                        message: e.to_string(),
                    })?;

                Ok(OperationResponse {
                    success: true,
                    value: None,
                    error: None,
                })
            }

            Operation::Delete { key } => {
                let storage = self.storage.write().await;
                storage
                    .delete(self.shard_id, key)
                    .await
                    .map_err(|e| RaftError::Storage {
                        message: e.to_string(),
                    })?;

                Ok(OperationResponse {
                    success: true,
                    value: None,
                    error: None,
                })
            }

            Operation::Batch { operations } => {
                // Apply all operations in the batch
                // Use Box::pin to avoid infinite recursion
                for op in operations {
                    Box::pin(self.apply_operation(op)).await?;
                }

                Ok(OperationResponse {
                    success: true,
                    value: None,
                    error: None,
                })
            }
        }
    }

    /// Append log entries
    pub async fn append_entries(&self, entries: Vec<LogEntry>) -> Result<()> {
        let mut log = self.log_entries.write().await;
        log.extend(entries);
        Ok(())
    }

    /// Get log entry at index
    pub async fn get_log_entry(&self, index: u64) -> Result<Option<LogEntry>> {
        let log = self.log_entries.read().await;
        Ok(log.iter().find(|e| e.index == index).cloned())
    }

    /// Get log entries in range [start, end)
    pub async fn get_log_entries(&self, start: u64, end: u64) -> Result<Vec<LogEntry>> {
        let log = self.log_entries.read().await;
        Ok(log
            .iter()
            .filter(|e| e.index >= start && e.index < end)
            .cloned()
            .collect())
    }

    /// Get last log index
    pub async fn last_log_index(&self) -> Result<u64> {
        let log = self.log_entries.read().await;
        Ok(log.last().map(|e| e.index).unwrap_or(0))
    }

    /// Get last log term
    pub async fn last_log_term(&self) -> Result<u64> {
        let log = self.log_entries.read().await;
        Ok(log.last().map(|e| e.term).unwrap_or(0))
    }

    /// Truncate log from index onwards
    pub async fn truncate_log(&self, from_index: u64) -> Result<()> {
        let mut log = self.log_entries.write().await;
        log.retain(|e| e.index < from_index);
        Ok(())
    }

    /// Get current term
    pub async fn current_term(&self) -> Result<u64> {
        let state = self.raft_state.read().await;
        Ok(state.term)
    }

    /// Set current term
    pub async fn set_term(&self, term: u64) -> Result<()> {
        let mut state = self.raft_state.write().await;
        state.term = term;
        Ok(())
    }

    /// Get voted for
    pub async fn voted_for(&self) -> Result<Option<NodeId>> {
        let state = self.raft_state.read().await;
        Ok(state.voted_for)
    }

    /// Set voted for
    pub async fn set_voted_for(&self, node_id: Option<NodeId>) -> Result<()> {
        let mut state = self.raft_state.write().await;
        state.voted_for = node_id;
        Ok(())
    }

    /// Get commit index
    pub async fn commit_index(&self) -> Result<u64> {
        let state = self.raft_state.read().await;
        Ok(state.commit_index)
    }

    /// Set commit index and apply committed entries
    pub async fn set_commit_index(&self, index: u64) -> Result<()> {
        let mut state = self.raft_state.write().await;
        state.commit_index = index;

        // Apply entries from last_applied to commit_index
        let last_applied = state.last_applied;
        drop(state); // Release lock before applying

        if index > last_applied {
            let entries = self.get_log_entries(last_applied + 1, index + 1).await?;
            for entry in entries {
                self.apply_operation(&entry.operation).await?;
            }

            let mut state = self.raft_state.write().await;
            state.last_applied = index;
        }

        Ok(())
    }

    /// Create a snapshot of current state
    pub async fn create_snapshot(&self) -> Result<ShardSnapshot> {
        let state = self.raft_state.read().await;
        let last_applied = state.last_applied;
        let term = state.term;
        drop(state);

        // Get last applied log entry for term
        let last_entry = self.get_log_entry(last_applied).await?;
        let last_term = last_entry.map(|e| e.term).unwrap_or(term);

        // Serialize storage state (simplified - in production would use SSTable snapshots)
        let _storage = self.storage.read().await;
        let data = Vec::new(); // TODO: Implement actual snapshot serialization

        Ok(ShardSnapshot {
            meta: SnapshotMeta {
                last_included_index: last_applied,
                last_included_term: last_term,
                shard_id: self.shard_id,
                size_bytes: data.len() as u64,
            },
            data,
        })
    }

    /// Install a snapshot
    pub async fn install_snapshot(&self, snapshot: ShardSnapshot) -> Result<()> {
        // Truncate log up to snapshot
        self.truncate_log(snapshot.meta.last_included_index + 1)
            .await?;

        // Update state
        let mut state = self.raft_state.write().await;
        state.last_applied = snapshot.meta.last_included_index;
        state.commit_index = snapshot.meta.last_included_index;

        // TODO: Restore storage state from snapshot data

        Ok(())
    }
}

// Note: Full openraft::RaftStorage trait implementation will be added
// once we integrate the openraft dependency properly. This provides
// the foundation for that integration.
