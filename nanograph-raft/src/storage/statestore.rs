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

use crate::error::{ConsensusError, ConsensusResult};
use crate::types::{ConsensusSnapshot, ConsensusTypeConfig, Operation, OperationResponse};
use futures_core::Stream;
use futures_util::StreamExt;
use futures_util::future::BoxFuture;
use nanograph_core::object::KeyRange;
use nanograph_core::{object::ShardId, types::Timestamp};
use nanograph_kvt::KeyValueShardStore;
use nanograph_vfs::{File, FileSystem};
use crate::storage::snapshot::SnapshotManager;
use openraft::storage::{EntryResponder, RaftSnapshotBuilder, RaftStateMachine};
use openraft::type_config::alias::LogIdOf;
use openraft::{EntryPayload, LogId, OptionalSend, Snapshot, SnapshotMeta, StoredMembership, Vote};
use serde::{Deserialize, Serialize};
use std::fmt::Debug;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Storage adapter that implements openraft's RaftStorage trait
///
/// This bridges openraft's consensus layer with Nanograph's KeyValueStore trait.
/// Each shard has its own storage adapter backed by a storage engine (LSM, B+Tree, etc).
#[derive(Clone)]
pub struct ConsensusStateStore {
    /// Storage Shard identifier
    shard_id: ShardId,
    /// Underlying storage engine
    shard_storage: Arc<dyn KeyValueShardStore>,
    /// Snapshot storage engine
    snapshot_storage: Arc<SnapshotManager>,
    /// Current Raft state (term, voted_for, etc)
    raft_state: Arc<RwLock<RaftState>>,
    /// Cached snapshot metadata (ID and meta) - we store the ID to open fresh file handles
    snapshot_meta: Arc<RwLock<Option<(String, SnapshotMeta<ConsensusTypeConfig>)>>>,
}

impl ConsensusStateStore {
    /// Create a new storage adapter
    pub fn new(
        shard_id: ShardId,
        shard_storage: Arc<dyn KeyValueShardStore>,
        snapshot_storage: Arc<SnapshotManager>,
    ) -> Self {
        Self {
            shard_id,
            shard_storage,
            snapshot_storage,
            raft_state: Arc::new(RwLock::new(RaftState::default())),
            snapshot_meta: Arc::new(RwLock::new(None)),
        }
    }

    pub async fn load_state(&self) -> ConsensusResult<()> {
        let key = format!("raft_state_{}", self.shard_id);
        if let Some(data) = self
            .shard_storage
            .get(self.shard_id, key.as_bytes())
            .await
            .map_err(|e| ConsensusError::Storage {
                message: e.to_string(),
            })?
        {
            let state: RaftState =
                postcard::from_bytes(&data).map_err(|e| ConsensusError::Serialization {
                    message: e.to_string(),
                })?;
            *self.raft_state.write().await = state.clone();
        }
        Ok(())
    }
    pub async fn save_state(&self) -> ConsensusResult<()> {
        let state = self.raft_state.read().await;
        let data = postcard::to_stdvec(&*state).map_err(|e| ConsensusError::Serialization {
            message: e.to_string(),
        })?;
        let key = format!("raft_state_{}", self.shard_id);
        self.shard_storage
            .put(self.shard_id, key.as_bytes(), &data)
            .await
            .map_err(|e| ConsensusError::Storage {
                message: e.to_string(),
            })?;
        Ok(())
    }

    /// Get a value from the storage engine
    pub async fn get_value(&self, key: &[u8]) -> ConsensusResult<Option<Vec<u8>>> {
        self.shard_storage
            .get(self.shard_id, key)
            .await
            .map_err(|e| ConsensusError::Storage {
                message: e.to_string(),
            })
    }

    /// Apply an operation to the storage engine
    pub async fn apply_operation(
        &self,
        operation: &Operation,
    ) -> ConsensusResult<OperationResponse> {
        self.apply_operation_recursive(operation).await
    }

    fn apply_operation_recursive<'a>(
        &'a self,
        operation: &'a Operation,
    ) -> BoxFuture<'a, ConsensusResult<OperationResponse>> {
        Box::pin(async move {
            match operation {
                Operation::Put { key, value } => {
                    self.shard_storage
                        .put(self.shard_id, key, value)
                        .await
                        .map_err(|e| ConsensusError::Storage {
                            message: e.to_string(),
                        })?;

                    Ok(OperationResponse {
                        success: true,
                        value: None,
                        error: None,
                    })
                }

                Operation::Delete { key } => {
                    self.shard_storage
                        .delete(self.shard_id, key)
                        .await
                        .map_err(|e| ConsensusError::Storage {
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
                    for op in operations {
                        self.apply_operation_recursive(op).await?;
                    }

                    Ok(OperationResponse {
                        success: true,
                        value: None,
                        error: None,
                    })
                }
            }
        })
    }
}

impl RaftStateMachine<ConsensusTypeConfig> for ConsensusStateStore {
    type SnapshotBuilder = Self;

    async fn applied_state(
        &mut self,
    ) -> Result<
        (
            Option<LogIdOf<ConsensusTypeConfig>>,
            StoredMembership<ConsensusTypeConfig>,
        ),
        std::io::Error,
    > {
        let state = self.raft_state.read().await;
        Ok((state.last_applied, state.last_membership.clone()))
    }

    async fn apply<S>(&mut self, mut entries: S) -> Result<(), std::io::Error>
    where
        S: Stream<Item=Result<EntryResponder<ConsensusTypeConfig>, std::io::Error>>
        + Unpin
        + OptionalSend,
    {
        while let Some(entry) = entries.next().await {
            match entry {
                Ok((entry, _responder)) => {
                    let log_id = entry.log_id;
                    match entry.payload {
                        EntryPayload::Normal(ref operation) => {
                            let _: OperationResponse = self
                                .apply_operation(operation)
                                .await
                                .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
                        }
                        EntryPayload::Membership(ref membership) => {
                            let mut state = self.raft_state.write().await;
                            state.last_membership =
                                StoredMembership::new(Some(log_id), membership.clone());
                        }
                        _ => {}
                    };

                    let mut state = self.raft_state.write().await;
                    state.last_applied = Some(log_id);
                }
                Err(e) => {
                    return Err(std::io::Error::new(std::io::ErrorKind::Other, e));
                }
            }
        }
        self.save_state()
            .await
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;

        Ok(())
    }

    async fn try_create_snapshot_builder(&mut self, _force: bool) -> Option<Self::SnapshotBuilder> {
        Some(self.clone())
    }
    async fn get_snapshot_builder(&mut self) -> Self::SnapshotBuilder {
        self.clone()
    }

    async fn begin_receiving_snapshot(&mut self) -> Result<Box<dyn File>, std::io::Error> {
        // Create a temporary in-memory file for receiving snapshot data
        let temp_path = format!("/tmp/snapshot_{}", nanograph_core::types::Timestamp::now());
        let fs = nanograph_vfs::MemoryFileSystem::new();
        let file = fs.create_file(&temp_path)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
        Ok(Box::new(file))
    }

    async fn install_snapshot(
        &mut self,
        meta: &SnapshotMeta<ConsensusTypeConfig>,
        mut snapshot: Box<dyn File>,
    ) -> Result<(), std::io::Error> {
        // Read the entire snapshot file into memory
        let size = snapshot.get_size()
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
        let mut buffer = vec![0u8; size as usize];
        
        // Seek to the beginning of the file first
        use std::io::{Read, Seek, SeekFrom, Write};
        snapshot.seek(SeekFrom::Start(0))?;
        snapshot.read_exact(&mut buffer)?;
        
        // Use snapshot_data to write the RAW snapshot file
        let snapshot_id = &meta.snapshot_id;
        let mut snapshot_data = self.snapshot_storage
            .snapshot_data(snapshot_id)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
        
        // Write the snapshot data
        snapshot_data.file_mut().write_all(&buffer)?;
        snapshot_data.file_mut().flush()?;
        drop(snapshot_data);
        
        // Now use snapshot_reader to read and install the data
        let (mut reader, _file_metadata) = self.snapshot_storage
            .snapshot_reader(snapshot_id)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;

        // Clear existing data
        self.shard_storage
            .clear(self.shard_id)
            .await
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;

        // Read all blocks until EOF
        loop {
            match reader.next_block() {
                Ok(block_iter) => {
                    for res in block_iter {
                        let (key, value) = res?;
                        self.shard_storage
                            .put(self.shard_id, &key, &value)
                            .await
                            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
                    }
                }
                Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => {
                    // End of snapshot file, this is expected
                    break;
                }
                Err(e) => {
                    // Other errors should be propagated
                    return Err(e);
                }
            }
        }

        // Update last applied
        {
            let mut state = self.raft_state.write().await;
            state.last_applied = meta.last_log_id;
            state.last_membership = meta.last_membership.clone();
        }

        self.save_state()
            .await
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;

        // Persist the snapshot metadata to storage for retrieval across clones
        let meta_key = format!("snapshot_meta_{}", self.shard_id);
        let meta_value = serde_json::to_vec(&(snapshot_id.clone(), meta.clone()))
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
        self.shard_storage
            .put(self.shard_id, meta_key.as_bytes(), &meta_value)
            .await
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
        
        // Also cache it in memory for faster access
        let mut cached_meta = self.snapshot_meta.write().await;
        *cached_meta = Some((snapshot_id.clone(), meta.clone()));

        Ok(())
    }

    async fn get_current_snapshot(&mut self) -> Result<Option<ConsensusSnapshot>, std::io::Error> {
        // First check the in-memory cache
        {
            let meta_guard = self.snapshot_meta.read().await;
            if let Some((snapshot_id, snapshot_meta)) = meta_guard.as_ref() {
                // Use snapshot_reader to get the actual snapshot file
                let (reader, _) = self.snapshot_storage
                    .snapshot_reader(snapshot_id)
                    .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
                
                // Return the snapshot with the file handle from the reader
                return Ok(Some(Snapshot {
                    meta: snapshot_meta.clone(),
                    snapshot: Box::new(reader.into_inner()),
                }));
            }
        }
        
        // If not in cache, try to load from persistent storage
        let meta_key = format!("snapshot_meta_{}", self.shard_id);
        if let Some(meta_value) = self.shard_storage
            .get(self.shard_id, meta_key.as_bytes())
            .await
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?
        {
            let (snapshot_id, snapshot_meta): (String, SnapshotMeta<ConsensusTypeConfig>) =
                serde_json::from_slice(&meta_value)
                    .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
            
            // Cache it for next time
            let mut cached_meta = self.snapshot_meta.write().await;
            *cached_meta = Some((snapshot_id.clone(), snapshot_meta.clone()));
            
            // Use snapshot_reader to get the actual snapshot file
            let (reader, _) = self.snapshot_storage
                .snapshot_reader(&snapshot_id)
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
            
            return Ok(Some(Snapshot {
                meta: snapshot_meta,
                snapshot: Box::new(reader.into_inner()),
            }));
        }
        
        Ok(None)
    }
}

impl RaftSnapshotBuilder<ConsensusTypeConfig> for ConsensusStateStore {
    async fn build_snapshot(&mut self) -> Result<Snapshot<ConsensusTypeConfig>, std::io::Error> {
        let (last_applied, last_membership) = self.applied_state().await?;
        
        // Create snapshot ID based on last_applied or use a default for empty snapshots
        let snapshot_id = if let Some(ref log_id) = last_applied {
            format!(
                "{}-{}-{}",
                log_id.leader_id,
                log_id.index,
                nanograph_core::types::Timestamp::now()
            )
        } else {
            format!("empty-{}", nanograph_core::types::Timestamp::now())
        };

        let metadata = SnapshotMeta {
            last_log_id: last_applied,
            last_membership,
            snapshot_id: snapshot_id.clone(),
        };

        let writer = self
            .snapshot_storage
            .snapshot_writer(metadata.clone())
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;

        let mut snapshot_writer = writer;

        // Iterate over all keys in the shard and write them to the snapshot
        let mut stream = self.shard_storage.scan(self.shard_id, KeyRange::all()).await.map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
        while let Some(res) = stream.next().await {
            let (key, value) = res.map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
            snapshot_writer
                .write_kv(&key, &value)
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
        }

        snapshot_writer
            .finish()
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;

        // Open a fresh file handle for the snapshot
        // Use snapshot_reader to get the snapshot file
        let (reader, _) = self.snapshot_storage
            .snapshot_reader(&snapshot_id)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
        let snapshot_file = Box::new(reader.into_inner());

        // Update last_snapshot in state
        if let Some(log_id) = last_applied {
            let mut state = self.raft_state.write().await;
            state.last_snapshot = Some(log_id);
        }

        // Persist the snapshot metadata to storage for retrieval across clones
        let meta_key = format!("snapshot_meta_{}", self.shard_id);
        let meta_value = serde_json::to_vec(&(snapshot_id.clone(), metadata.clone()))
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
        self.shard_storage
            .put(self.shard_id, meta_key.as_bytes(), &meta_value)
            .await
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
        
        // Also cache it in memory for faster access
        let mut cached_meta = self.snapshot_meta.write().await;
        *cached_meta = Some((snapshot_id.clone(), metadata.clone()));

        Ok(Snapshot {
            meta: metadata,
            snapshot: snapshot_file,
        })
    }
}

/// Raft state (persisted)
#[derive(Clone, Debug, Serialize, Deserialize)]
struct RaftState {
    /// Last vote
    vote: Option<Vote<ConsensusTypeConfig>>,

    /// Last applied log id
    last_applied: Option<LogId<ConsensusTypeConfig>>,

    /// Last membership
    last_membership: StoredMembership<ConsensusTypeConfig>,

    /// Last snapshot log id
    last_snapshot: Option<LogId<ConsensusTypeConfig>>,
}

impl Default for RaftState {
    fn default() -> Self {
        Self {
            vote: None,
            last_applied: None,
            last_membership: StoredMembership::default(),
            last_snapshot: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nanograph_core::object::{NodeId, ShardId};
    use nanograph_kvt::MemoryKeyValueShardStore;
    use nanograph_vfs::{MemoryFileSystem, Path};
    use openraft::storage::RaftSnapshotBuilder;
    use crate::storage::snapshot::SnapshotConfig;
    use crate::types::{ConsensusLeaderId, ConsensusLogId, NodeInfo};
    use openraft::vote::RaftLeaderId;

    async fn create_test_store() -> ConsensusStateStore {
        let shard_id = ShardId::new(1);
        let shard_storage = Arc::new(MemoryKeyValueShardStore::new());
        
        // Create the shard before using it
        shard_storage.create_shard(shard_id).await.unwrap();
        
        let fs = Arc::new(MemoryFileSystem::new());
        fs.create_directory_all("/snapshots").unwrap();
        let snapshot_storage = Arc::new(SnapshotManager::with_config(
            fs,
            Path::parse("/snapshots"),
            SnapshotConfig::default(),
        ));
        
        ConsensusStateStore::new(shard_id, shard_storage, snapshot_storage)
    }

    #[tokio::test]
    async fn test_state_store_creation() {
        let store = create_test_store().await;
        let (last_applied, last_membership) = store.clone().applied_state().await.unwrap();
        assert!(last_applied.is_none());
        assert!(last_membership.log_id().is_none());
    }

    #[tokio::test]
    async fn test_apply_put_operation() {
        let store = create_test_store().await;
        
        let operation = Operation::Put {
            key: b"test_key".to_vec(),
            value: b"test_value".to_vec(),
        };
        
        let response = store.apply_operation(&operation).await.unwrap();
        assert!(response.success);
        assert!(response.error.is_none());
        
        // Verify the value was stored
        let value = store.shard_storage
            .get(store.shard_id, b"test_key")
            .await
            .unwrap();
        assert_eq!(value, Some(b"test_value".to_vec()));
    }

    #[tokio::test]
    async fn test_apply_delete_operation() {
        let store = create_test_store().await;
        
        // First put a value
        store.shard_storage
            .put(store.shard_id, b"test_key", b"test_value")
            .await
            .unwrap();
        
        // Then delete it
        let operation = Operation::Delete {
            key: b"test_key".to_vec(),
        };
        
        let response = store.apply_operation(&operation).await.unwrap();
        assert!(response.success);
        
        // Verify the value was deleted
        let value = store.shard_storage
            .get(store.shard_id, b"test_key")
            .await
            .unwrap();
        assert!(value.is_none());
    }

    #[tokio::test]
    async fn test_apply_batch_operation() {
        let store = create_test_store().await;
        
        let operation = Operation::Batch {
            operations: vec![
                Operation::Put {
                    key: b"key1".to_vec(),
                    value: b"value1".to_vec(),
                },
                Operation::Put {
                    key: b"key2".to_vec(),
                    value: b"value2".to_vec(),
                },
                Operation::Delete {
                    key: b"key1".to_vec(),
                },
            ],
        };
        
        let response = store.apply_operation(&operation).await.unwrap();
        assert!(response.success);
        
        // Verify key1 was deleted
        let value1 = store.shard_storage
            .get(store.shard_id, b"key1")
            .await
            .unwrap();
        assert!(value1.is_none());
        
        // Verify key2 exists
        let value2 = store.shard_storage
            .get(store.shard_id, b"key2")
            .await
            .unwrap();
        assert_eq!(value2, Some(b"value2".to_vec()));
    }

    #[tokio::test]
    async fn test_build_snapshot() {
        let mut store = create_test_store().await;
        
        // Add some data
        store.shard_storage
            .put(store.shard_id, b"key1", b"value1")
            .await
            .unwrap();
        store.shard_storage
            .put(store.shard_id, b"key2", b"value2")
            .await
            .unwrap();
        
        // Update state with a log ID
        {
            let mut state = store.raft_state.write().await;
            state.last_applied = Some(ConsensusLogId::new(
                ConsensusLeaderId::new(1, NodeId::new(1)).to_committed(),
                10,
            ));
            state.last_membership = StoredMembership::new(
                None,
                {
                    let mut config = std::collections::BTreeSet::new();
                    config.insert(NodeId::new(1));
                    let mut nodes = std::collections::BTreeMap::new();
                    nodes.insert(NodeId::new(1), NodeInfo::default());
                    openraft::Membership::new(vec![config], nodes)
                        .expect("Failed to create membership")
                },
            );
        }
        
        // Build snapshot
        let snapshot = store.build_snapshot().await.unwrap();
        
        assert!(snapshot.meta.last_log_id.is_some());
        assert_eq!(snapshot.meta.last_log_id.unwrap().index, 10);
    }

    #[tokio::test]
    async fn test_install_and_get_snapshot() {
        let mut store = create_test_store().await;
        
        // Build a snapshot first
        store.shard_storage
            .put(store.shard_id, b"original_key", b"original_value")
            .await
            .unwrap();
        
        {
            let mut state = store.raft_state.write().await;
            state.last_applied = Some(ConsensusLogId::new(
                ConsensusLeaderId::new(1, NodeId::new(1)).to_committed(),
                5,
            ));
            state.last_membership = StoredMembership::new(
                None,
                {
                    let mut config = std::collections::BTreeSet::new();
                    config.insert(NodeId::new(1));
                    let mut nodes = std::collections::BTreeMap::new();
                    nodes.insert(NodeId::new(1), NodeInfo::default());
                    openraft::Membership::new(vec![config], nodes)
                        .expect("Failed to create membership")
                },
            );
        }
        
        let snapshot = store.build_snapshot().await.unwrap();
        let meta = snapshot.meta.clone();
        let snapshot_file = snapshot.snapshot;
        
        // Create a new store to install the snapshot into
        let mut new_store = create_test_store().await;
        
        // Install the snapshot
        new_store.install_snapshot(&meta, snapshot_file).await.unwrap();
        
        // Verify the data was installed
        let value = new_store.shard_storage
            .get(new_store.shard_id, b"original_key")
            .await
            .unwrap();
        assert_eq!(value, Some(b"original_value".to_vec()));
        
        // Verify state was updated
        let (last_applied, _) = new_store.applied_state().await.unwrap();
        assert_eq!(last_applied, meta.last_log_id);
        
        // Verify we can retrieve the snapshot
        let retrieved_snapshot = new_store.get_current_snapshot().await.unwrap();
        assert!(retrieved_snapshot.is_some());
        let retrieved = retrieved_snapshot.unwrap();
        assert_eq!(retrieved.meta.snapshot_id, meta.snapshot_id);
    }

    #[tokio::test]
    async fn test_state_persistence() {
        let store = create_test_store().await;
        
        // Update state
        {
            let mut state = store.raft_state.write().await;
            state.last_applied = Some(ConsensusLogId::new(
                ConsensusLeaderId::new(1, NodeId::new(1)).to_committed(),
                42,
            ));
        }
        
        // Save state
        store.save_state().await.unwrap();
        
        // Load state in a new store instance
        let new_store = ConsensusStateStore::new(
            store.shard_id,
            store.shard_storage.clone(),
            store.snapshot_storage.clone(),
        );
        new_store.load_state().await.unwrap();
        
        // Verify state was loaded
        let state = new_store.raft_state.read().await;
        assert!(state.last_applied.is_some());
        assert_eq!(state.last_applied.unwrap().index, 42);
    }

    #[tokio::test]
    async fn test_empty_snapshot() {
        let mut store = create_test_store().await;
        
        // Build snapshot with no data
        let snapshot = store.build_snapshot().await.unwrap();
        
        // Snapshot ID should indicate it's empty
        assert!(snapshot.meta.snapshot_id.starts_with("empty-"));
        assert!(snapshot.meta.last_log_id.is_none());
    }

    #[tokio::test]
    async fn test_snapshot_with_large_data() {
        let mut store = create_test_store().await;
        
        // Add many keys
        for i in 0..100 {
            let key = format!("key_{:03}", i);
            let value = format!("value_{:03}", i);
            store.shard_storage
                .put(store.shard_id, key.as_bytes(), value.as_bytes())
                .await
                .unwrap();
        }
        
        {
            let mut state = store.raft_state.write().await;
            state.last_applied = Some(ConsensusLogId::new(
                ConsensusLeaderId::new(1, NodeId::new(1)).to_committed(),
                100,
            ));
            state.last_membership = StoredMembership::new(
                None,
                {
                    let mut config = std::collections::BTreeSet::new();
                    config.insert(NodeId::new(1));
                    let mut nodes = std::collections::BTreeMap::new();
                    nodes.insert(NodeId::new(1), NodeInfo::default());
                    openraft::Membership::new(vec![config], nodes)
                        .expect("Failed to create membership")
                },
            );
        }
        
        // Build snapshot
        let snapshot = store.build_snapshot().await.unwrap();
        assert!(snapshot.meta.last_log_id.is_some());
        
        // Install in new store
        let mut new_store = create_test_store().await;
        let meta = snapshot.meta.clone();
        new_store.install_snapshot(&meta, snapshot.snapshot).await.unwrap();
        
        // Verify all keys were installed
        for i in 0..100 {
            let key = format!("key_{:03}", i);
            let expected_value = format!("value_{:03}", i);
            let value = new_store.shard_storage
                .get(new_store.shard_id, key.as_bytes())
                .await
                .unwrap();
            assert_eq!(value, Some(expected_value.as_bytes().to_vec()));
        }
    }
}
