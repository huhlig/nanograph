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

//! Integration tests for distributed full-text search index

use nanograph_core::object::{IndexId, IndexRecord, IndexStatus, IndexType, ObjectId, ShardId};
use nanograph_core::types::Timestamp;
use nanograph_idx::{
    ConsensusGroup, DistributedIndex, IndexCommand, IndexCommandResponse, IndexEntry, IndexStore,
    PersistenceConfig, ScoringAlgorithm, TokenizerConfig, fulltext::FullTextIndex,
};
use nanograph_kvt::metrics::ShardStats;
use nanograph_kvt::{KeyValueError, KeyValueIterator, KeyValueShardStore};
use nanograph_vfs::{DynamicFileSystem, Path};
use std::collections::HashMap;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};
use tokio::sync::RwLock;

/// Mock consensus group for testing
struct MockConsensusGroup {
    is_leader: Arc<RwLock<bool>>,
    commands: Arc<RwLock<Vec<IndexCommand>>>,
}

impl MockConsensusGroup {
    fn new(is_leader: bool) -> Self {
        Self {
            is_leader: Arc::new(RwLock::new(is_leader)),
            commands: Arc::new(RwLock::new(Vec::new())),
        }
    }

    async fn set_leader(&self, is_leader: bool) {
        *self.is_leader.write().await = is_leader;
    }

    async fn get_commands(&self) -> Vec<IndexCommand> {
        self.commands.read().await.clone()
    }
}

impl ConsensusGroup for MockConsensusGroup {
    fn is_leader(&self) -> impl std::future::Future<Output = bool> + Send {
        let is_leader = self.is_leader.clone();
        async move { *is_leader.read().await }
    }

    fn propose(
        &self,
        data: Vec<u8>,
    ) -> impl std::future::Future<Output = Result<(), String>> + Send {
        let commands = self.commands.clone();
        async move {
            let command: IndexCommand = bincode::deserialize(&data)
                .map_err(|e| format!("Failed to deserialize command: {}", e))?;
            commands.write().await.push(command);
            Ok(())
        }
    }
}

/// Mock key-value store (reused from fulltext tests)
struct MockKeyValueStore {
    data: Arc<RwLock<HashMap<Vec<u8>, Vec<u8>>>>,
}

impl MockKeyValueStore {
    fn new() -> Self {
        Self {
            data: Arc::new(RwLock::new(HashMap::new())),
        }
    }
}

struct MockIterator {
    items: Vec<(Vec<u8>, Vec<u8>)>,
    index: usize,
}

impl futures_core::Stream for MockIterator {
    type Item = Result<(Vec<u8>, Vec<u8>), KeyValueError>;

    fn poll_next(mut self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        if self.index < self.items.len() {
            let item = self.items[self.index].clone();
            self.index += 1;
            Poll::Ready(Some(Ok(item)))
        } else {
            Poll::Ready(None)
        }
    }
}

impl KeyValueIterator for MockIterator {
    fn seek(&mut self, key: &[u8]) -> Result<(), KeyValueError> {
        self.index = self
            .items
            .iter()
            .position(|(k, _)| k.as_slice() >= key)
            .unwrap_or(self.items.len());
        Ok(())
    }

    fn position(&self) -> Option<Vec<u8>> {
        if self.index < self.items.len() {
            Some(self.items[self.index].0.clone())
        } else {
            None
        }
    }

    fn valid(&self) -> bool {
        self.index < self.items.len()
    }
}

#[async_trait::async_trait]
impl KeyValueShardStore for MockKeyValueStore {
    async fn get(&self, _shard_id: ShardId, key: &[u8]) -> Result<Option<Vec<u8>>, KeyValueError> {
        Ok(self.data.read().await.get(key).cloned())
    }

    async fn put(&self, _shard_id: ShardId, key: &[u8], value: &[u8]) -> Result<(), KeyValueError> {
        self.data.write().await.insert(key.to_vec(), value.to_vec());
        Ok(())
    }

    async fn delete(&self, _shard_id: ShardId, key: &[u8]) -> Result<bool, KeyValueError> {
        Ok(self.data.write().await.remove(key).is_some())
    }

    async fn exists(&self, _shard_id: ShardId, key: &[u8]) -> Result<bool, KeyValueError> {
        Ok(self.data.read().await.contains_key(key))
    }

    async fn batch_get(
        &self,
        shard: ShardId,
        keys: &[&[u8]],
    ) -> Result<Vec<Option<Vec<u8>>>, KeyValueError> {
        let mut results = Vec::new();
        for key in keys {
            results.push(self.get(shard, key).await?);
        }
        Ok(results)
    }

    async fn batch_put(
        &self,
        shard: ShardId,
        pairs: &[(&[u8], &[u8])],
    ) -> Result<(), KeyValueError> {
        for (key, value) in pairs {
            self.put(shard, key, value).await?;
        }
        Ok(())
    }

    async fn batch_delete(&self, shard: ShardId, keys: &[&[u8]]) -> Result<usize, KeyValueError> {
        let mut count = 0;
        for key in keys {
            if self.delete(shard, key).await? {
                count += 1;
            }
        }
        Ok(count)
    }

    async fn scan(
        &self,
        _shard_id: ShardId,
        range: nanograph_core::object::KeyRange,
    ) -> Result<Box<dyn KeyValueIterator + Send>, KeyValueError> {
        use std::ops::Bound;

        let data = self.data.read().await;
        let mut items: Vec<_> = data.iter().map(|(k, v)| (k.clone(), v.clone())).collect();
        items.sort_by(|a, b| a.0.cmp(&b.0));

        let filtered: Vec<_> = items
            .into_iter()
            .filter(|(k, _)| {
                let start_ok = match &range.start {
                    Bound::Included(s) => k >= s,
                    Bound::Excluded(s) => k > s,
                    Bound::Unbounded => true,
                };
                let end_ok = match &range.end {
                    Bound::Included(e) => k <= e,
                    Bound::Excluded(e) => k < e,
                    Bound::Unbounded => true,
                };
                start_ok && end_ok
            })
            .take(range.limit.unwrap_or(usize::MAX))
            .collect();

        Ok(Box::new(MockIterator {
            items: filtered,
            index: 0,
        }))
    }

    async fn key_count(&self, _shard: ShardId) -> Result<u64, KeyValueError> {
        Ok(self.data.read().await.len() as u64)
    }

    async fn shard_stats(&self, shard: ShardId) -> Result<ShardStats, KeyValueError> {
        Ok(ShardStats {
            key_count: self.key_count(shard).await?,
            total_bytes: 0,
            data_bytes: 0,
            index_bytes: 0,
            last_modified: Some(nanograph_core::types::Timestamp::now()),
            engine_stats: nanograph_kvt::metrics::EngineStats::default(),
        })
    }

    async fn begin_transaction(
        &self,
    ) -> Result<Arc<dyn nanograph_kvt::Transaction>, KeyValueError> {
        Err(KeyValueError::InvalidOperation(
            "Transactions not supported in mock".to_string(),
        ))
    }

    fn create_shard(
        &self,
        _shard: ShardId,
        _vfs: Arc<dyn DynamicFileSystem>,
        _data_path: Path,
        _wal_path: Path,
    ) -> Result<(), KeyValueError> {
        Ok(())
    }

    async fn drop_shard(&self, _shard: ShardId) -> Result<(), KeyValueError> {
        self.data.write().await.clear();
        Ok(())
    }

    async fn clear(&self, _shard: ShardId) -> Result<(), KeyValueError> {
        self.data.write().await.clear();
        Ok(())
    }

    async fn list_shards(&self) -> Result<Vec<ShardId>, KeyValueError> {
        Ok(vec![])
    }

    async fn shard_exists(&self, _shard: ShardId) -> Result<bool, KeyValueError> {
        Ok(true)
    }

    async fn flush(&self) -> Result<(), KeyValueError> {
        Ok(())
    }

    async fn compact(&self, _shard: Option<ShardId>) -> Result<(), KeyValueError> {
        Ok(())
    }
}

fn create_test_metadata() -> IndexRecord {
    IndexRecord {
        index_id: IndexId(ObjectId::new(1)),
        name: "test_distributed_fulltext_idx".to_string(),
        version: 0,
        index_type: IndexType::FullText,
        created_at: Timestamp::now(),
        updated_at: Timestamp::now(),
        columns: vec!["content".to_string()],
        key_extractor: None,
        options: HashMap::new(),
        metadata: HashMap::new(),
        status: IndexStatus::Building,
        sharding: nanograph_core::object::IndexSharding::Single,
    }
}

fn create_test_config() -> PersistenceConfig {
    PersistenceConfig {
        shard_id: ShardId::default(),
        index_id: IndexId(ObjectId::new(1)),
        cache_size: 100,
        durability: nanograph_wal::Durability::None,
        enable_wal: false,
    }
}

#[tokio::test]
async fn test_distributed_index_creation() {
    let metadata = create_test_metadata();
    let store = Arc::new(MockKeyValueStore::new());
    let config = create_test_config();
    let tokenizer_config = TokenizerConfig::default();

    let local_index = FullTextIndex::new(
        metadata,
        store,
        None,
        config,
        tokenizer_config,
        ScoringAlgorithm::TfIdf,
    )
    .await
    .unwrap();

    let consensus = Arc::new(MockConsensusGroup::new(true));
    let _distributed_index = DistributedIndex::new(local_index, consensus);

    // Test passes if creation succeeds
}

#[tokio::test]
async fn test_distributed_insert_as_leader() {
    let metadata = create_test_metadata();
    let store = Arc::new(MockKeyValueStore::new());
    let config = create_test_config();
    let tokenizer_config = TokenizerConfig::default();

    let local_index = FullTextIndex::new(
        metadata,
        store,
        None,
        config,
        tokenizer_config,
        ScoringAlgorithm::TfIdf,
    )
    .await
    .unwrap();

    let consensus = Arc::new(MockConsensusGroup::new(true));
    let mut distributed_index = DistributedIndex::new(local_index, consensus.clone());

    // Insert as leader
    let entry = IndexEntry {
        indexed_value: b"The quick brown fox".to_vec(),
        primary_key: b"doc1".to_vec(),
        included_columns: None,
    };

    let result = distributed_index.insert(entry).await;
    assert!(result.is_ok());

    // Verify command was proposed
    let commands = consensus.get_commands().await;
    assert_eq!(commands.len(), 1);
    match &commands[0] {
        IndexCommand::Insert { primary_key, .. } => {
            assert_eq!(primary_key, b"doc1");
        }
        _ => panic!("Expected Insert command"),
    }
}

#[tokio::test]
async fn test_distributed_delete_as_leader() {
    let metadata = create_test_metadata();
    let store = Arc::new(MockKeyValueStore::new());
    let config = create_test_config();
    let tokenizer_config = TokenizerConfig::default();

    let local_index = FullTextIndex::new(
        metadata,
        store,
        None,
        config,
        tokenizer_config,
        ScoringAlgorithm::TfIdf,
    )
    .await
    .unwrap();

    let consensus = Arc::new(MockConsensusGroup::new(true));
    let mut distributed_index = DistributedIndex::new(local_index, consensus.clone());

    // Delete as leader
    let result = distributed_index.delete(b"doc1").await;
    assert!(result.is_ok());

    // Verify command was proposed
    let commands = consensus.get_commands().await;
    assert_eq!(commands.len(), 1);
    match &commands[0] {
        IndexCommand::Delete { primary_key } => {
            assert_eq!(primary_key, b"doc1");
        }
        _ => panic!("Expected Delete command"),
    }
}

#[tokio::test]
async fn test_distributed_command_application() {
    let metadata = create_test_metadata();
    let store = Arc::new(MockKeyValueStore::new());
    let config = create_test_config();
    let tokenizer_config = TokenizerConfig::default();

    let local_index = FullTextIndex::new(
        metadata,
        store,
        None,
        config,
        tokenizer_config,
        ScoringAlgorithm::TfIdf,
    )
    .await
    .unwrap();

    let consensus = Arc::new(MockConsensusGroup::new(true));
    let distributed_index = DistributedIndex::new(local_index, consensus);

    // Apply insert command directly (simulating Raft commit)
    let command = IndexCommand::Insert {
        indexed_value: b"Test document".to_vec(),
        primary_key: b"doc1".to_vec(),
        included_columns: None,
    };

    let response = distributed_index.apply_command(command).await.unwrap();
    match response {
        IndexCommandResponse::Ok => {}
        IndexCommandResponse::Error(e) => panic!("Command failed: {}", e),
    }
}

#[tokio::test]
async fn test_distributed_leader_check() {
    let metadata = create_test_metadata();
    let store = Arc::new(MockKeyValueStore::new());
    let config = create_test_config();
    let tokenizer_config = TokenizerConfig::default();

    let local_index = FullTextIndex::new(
        metadata,
        store,
        None,
        config,
        tokenizer_config,
        ScoringAlgorithm::TfIdf,
    )
    .await
    .unwrap();

    let consensus = Arc::new(MockConsensusGroup::new(true));
    let distributed_index = DistributedIndex::new(local_index, consensus.clone());

    // Check leader status
    assert!(distributed_index.is_leader().await);

    // Change leader status
    consensus.set_leader(false).await;
    assert!(!distributed_index.is_leader().await);
}

// Made with Bob
