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

//! Integration tests for full-text search index

use nanograph_core::object::{IndexId, IndexRecord, IndexStatus, IndexType, ObjectId, ShardId};
use nanograph_core::types::Timestamp;
use nanograph_idx::{
    BooleanQuery, IndexEntry, IndexStore, PersistenceConfig, ScoringAlgorithm, Term,
    TextSearchIndex, TokenizerConfig, fulltext::FullTextIndex,
};
use nanograph_kvt::metrics::ShardStats;
use nanograph_kvt::{KeyValueError, KeyValueIterator, KeyValueShardStore};
use nanograph_vfs::{DynamicFileSystem, Path};
use std::collections::HashMap;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};
use tokio::sync::RwLock;

/// Mock key-value store for testing
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

/// Simple iterator for mock store
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
        // Find first item >= key
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

        // Filter by range
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
        name: "test_fulltext_idx".to_string(),
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
        durability: nanograph_wal::Durability::Memory,
        enable_wal: false,
    }
}

#[tokio::test]
async fn test_fulltext_index_creation() {
    let metadata = create_test_metadata();
    let store = Arc::new(MockKeyValueStore::new());
    let config = create_test_config();
    let tokenizer_config = TokenizerConfig::default();

    let result = FullTextIndex::new(
        metadata,
        store,
        None,
        config,
        tokenizer_config,
        ScoringAlgorithm::TfIdf,
    )
    .await;

    assert!(result.is_ok());
}

#[tokio::test]
async fn test_fulltext_insert_and_search() {
    let metadata = create_test_metadata();
    let store = Arc::new(MockKeyValueStore::new());
    let config = create_test_config();
    let tokenizer_config = TokenizerConfig::default();

    let mut index = FullTextIndex::new(
        metadata,
        store,
        None,
        config,
        tokenizer_config,
        ScoringAlgorithm::TfIdf,
    )
    .await
    .unwrap();

    // Insert documents
    let doc1 = IndexEntry {
        indexed_value: b"The quick brown fox jumps over the lazy dog".to_vec(),
        primary_key: b"doc1".to_vec(),
        included_columns: None,
    };

    let doc2 = IndexEntry {
        indexed_value: b"A fast brown fox runs through the forest".to_vec(),
        primary_key: b"doc2".to_vec(),
        included_columns: None,
    };

    let doc3 = IndexEntry {
        indexed_value: b"The lazy cat sleeps all day".to_vec(),
        primary_key: b"doc3".to_vec(),
        included_columns: None,
    };

    assert!(index.insert(doc1).await.is_ok());
    assert!(index.insert(doc2).await.is_ok());
    assert!(index.insert(doc3).await.is_ok());

    // Search for "fox"
    let results = index.search("fox", Some(10)).await.unwrap();
    assert_eq!(results.len(), 2);
    assert!(results.iter().any(|r| r.entry.primary_key == b"doc1"));
    assert!(results.iter().any(|r| r.entry.primary_key == b"doc2"));

    // Search for "lazy"
    let results = index.search("lazy", Some(10)).await.unwrap();
    assert_eq!(results.len(), 2);
    assert!(results.iter().any(|r| r.entry.primary_key == b"doc1"));
    assert!(results.iter().any(|r| r.entry.primary_key == b"doc3"));

    // Search for "forest"
    let results = index.search("forest", Some(10)).await.unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].entry.primary_key, b"doc2");
}

#[tokio::test]
async fn test_fulltext_delete() {
    let metadata = create_test_metadata();
    let store = Arc::new(MockKeyValueStore::new());
    let config = create_test_config();
    let tokenizer_config = TokenizerConfig::default();

    let mut index = FullTextIndex::new(
        metadata,
        store,
        None,
        config,
        tokenizer_config,
        ScoringAlgorithm::TfIdf,
    )
    .await
    .unwrap();

    // Insert documents
    let doc1 = IndexEntry {
        indexed_value: b"The quick brown fox".to_vec(),
        primary_key: b"doc1".to_vec(),
        included_columns: None,
    };

    let doc2 = IndexEntry {
        indexed_value: b"The lazy fox".to_vec(),
        primary_key: b"doc2".to_vec(),
        included_columns: None,
    };

    index.insert(doc1).await.unwrap();
    index.insert(doc2).await.unwrap();

    // Verify both documents are searchable
    let results = index.search("fox", Some(10)).await.unwrap();
    assert_eq!(results.len(), 2);

    // Delete doc1
    index.delete(b"doc1").await.unwrap();

    // Verify only doc2 is searchable
    let results = index.search("fox", Some(10)).await.unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].entry.primary_key, b"doc2");

    // Verify "quick" is no longer searchable
    let results = index.search("quick", Some(10)).await.unwrap();
    assert_eq!(results.len(), 0);
}

#[tokio::test]
async fn test_fulltext_update() {
    let metadata = create_test_metadata();
    let store = Arc::new(MockKeyValueStore::new());
    let config = create_test_config();
    let tokenizer_config = TokenizerConfig::default();

    let mut index = FullTextIndex::new(
        metadata,
        store,
        None,
        config,
        tokenizer_config,
        ScoringAlgorithm::TfIdf,
    )
    .await
    .unwrap();

    // Insert document
    let doc1 = IndexEntry {
        indexed_value: b"The quick brown fox".to_vec(),
        primary_key: b"doc1".to_vec(),
        included_columns: None,
    };

    index.insert(doc1.clone()).await.unwrap();

    // Update document
    let doc1_updated = IndexEntry {
        indexed_value: b"The slow brown bear".to_vec(),
        primary_key: b"doc1".to_vec(),
        included_columns: None,
    };

    index.update(doc1, doc1_updated).await.unwrap();

    // Verify old terms are not searchable
    let results = index.search("fox", Some(10)).await.unwrap();
    assert_eq!(results.len(), 0);

    let results = index.search("quick", Some(10)).await.unwrap();
    assert_eq!(results.len(), 0);

    // Verify new terms are searchable
    let results = index.search("bear", Some(10)).await.unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].entry.primary_key, b"doc1");

    let results = index.search("slow", Some(10)).await.unwrap();
    assert_eq!(results.len(), 1);
}

#[tokio::test]
async fn test_fulltext_boolean_and_search() {
    let metadata = create_test_metadata();
    let store = Arc::new(MockKeyValueStore::new());
    let config = create_test_config();
    let tokenizer_config = TokenizerConfig::default();

    let mut index = FullTextIndex::new(
        metadata,
        store,
        None,
        config,
        tokenizer_config,
        ScoringAlgorithm::TfIdf,
    )
    .await
    .unwrap();

    // Insert documents
    let doc1 = IndexEntry {
        indexed_value: b"The quick brown fox".to_vec(),
        primary_key: b"doc1".to_vec(),
        included_columns: None,
    };

    let doc2 = IndexEntry {
        indexed_value: b"The lazy brown dog".to_vec(),
        primary_key: b"doc2".to_vec(),
        included_columns: None,
    };

    let doc3 = IndexEntry {
        indexed_value: b"The quick red fox".to_vec(),
        primary_key: b"doc3".to_vec(),
        included_columns: None,
    };

    index.insert(doc1).await.unwrap();
    index.insert(doc2).await.unwrap();
    index.insert(doc3).await.unwrap();

    // Search for documents with both "quick" AND "brown"
    let query = BooleanQuery::And(vec![Term::new("quick"), Term::new("brown")]);
    let results = index.boolean_search(query, Some(10)).await.unwrap();

    assert_eq!(results.len(), 1);
    assert_eq!(results[0].entry.primary_key, b"doc1");
}

#[tokio::test]
async fn test_fulltext_boolean_or_search() {
    let metadata = create_test_metadata();
    let store = Arc::new(MockKeyValueStore::new());
    let config = create_test_config();
    let tokenizer_config = TokenizerConfig::default();

    let mut index = FullTextIndex::new(
        metadata,
        store,
        None,
        config,
        tokenizer_config,
        ScoringAlgorithm::TfIdf,
    )
    .await
    .unwrap();

    // Insert documents
    let doc1 = IndexEntry {
        indexed_value: b"The quick brown fox".to_vec(),
        primary_key: b"doc1".to_vec(),
        included_columns: None,
    };

    let doc2 = IndexEntry {
        indexed_value: b"The lazy brown dog".to_vec(),
        primary_key: b"doc2".to_vec(),
        included_columns: None,
    };

    let doc3 = IndexEntry {
        indexed_value: b"The red cat".to_vec(),
        primary_key: b"doc3".to_vec(),
        included_columns: None,
    };

    index.insert(doc1).await.unwrap();
    index.insert(doc2).await.unwrap();
    index.insert(doc3).await.unwrap();

    // Search for documents with "fox" OR "dog"
    let query = BooleanQuery::Or(vec![Term::new("fox"), Term::new("dog")]);
    let results = index.boolean_search(query, Some(10)).await.unwrap();

    assert_eq!(results.len(), 2);
    assert!(results.iter().any(|r| r.entry.primary_key == b"doc1"));
    assert!(results.iter().any(|r| r.entry.primary_key == b"doc2"));
}

#[tokio::test]
async fn test_fulltext_term_stats() {
    let metadata = create_test_metadata();
    let store = Arc::new(MockKeyValueStore::new());
    let config = create_test_config();
    let tokenizer_config = TokenizerConfig::default();

    let mut index = FullTextIndex::new(
        metadata,
        store,
        None,
        config,
        tokenizer_config,
        ScoringAlgorithm::TfIdf,
    )
    .await
    .unwrap();

    // Insert documents
    let doc1 = IndexEntry {
        indexed_value: b"fox fox fox".to_vec(),
        primary_key: b"doc1".to_vec(),
        included_columns: None,
    };

    let doc2 = IndexEntry {
        indexed_value: b"fox dog".to_vec(),
        primary_key: b"doc2".to_vec(),
        included_columns: None,
    };

    index.insert(doc1).await.unwrap();
    index.insert(doc2).await.unwrap();

    // Get stats for "fox"
    let stats = index.term_stats("fox").await.unwrap();
    assert!(stats.is_some());

    let stats = stats.unwrap();
    assert_eq!(stats.document_frequency, 2); // Appears in 2 documents
    assert_eq!(stats.total_frequency, 4); // Total of 4 occurrences
}

#[tokio::test]
async fn test_fulltext_scoring_algorithms() {
    let metadata = create_test_metadata();
    let store = Arc::new(MockKeyValueStore::new());
    let config = create_test_config();
    let tokenizer_config = TokenizerConfig::default();

    // Test with TF-IDF
    let mut index_tfidf = FullTextIndex::new(
        metadata.clone(),
        store.clone(),
        None,
        config.clone(),
        tokenizer_config.clone(),
        ScoringAlgorithm::TfIdf,
    )
    .await
    .unwrap();

    let doc = IndexEntry {
        indexed_value: b"test document".to_vec(),
        primary_key: b"doc1".to_vec(),
        included_columns: None,
    };

    index_tfidf.insert(doc.clone()).await.unwrap();
    let results = index_tfidf.search("test", Some(10)).await.unwrap();
    assert_eq!(results.len(), 1);

    // Test with BM25
    let store2 = Arc::new(MockKeyValueStore::new());
    let mut index_bm25 = FullTextIndex::new(
        metadata,
        store2,
        None,
        config,
        tokenizer_config,
        ScoringAlgorithm::Bm25,
    )
    .await
    .unwrap();

    index_bm25.insert(doc).await.unwrap();
    let results = index_bm25.search("test", Some(10)).await.unwrap();
    assert_eq!(results.len(), 1);
}

#[tokio::test]
async fn test_fulltext_tokenizer_config() {
    let metadata = create_test_metadata();
    let store = Arc::new(MockKeyValueStore::new());
    let config = create_test_config();

    // Test with stop words
    let mut tokenizer_config = TokenizerConfig::default();
    tokenizer_config.remove_stop_words = true;
    tokenizer_config.stop_words = vec!["the".to_string(), "a".to_string()];

    let mut index = FullTextIndex::new(
        metadata,
        store,
        None,
        config,
        tokenizer_config,
        ScoringAlgorithm::TfIdf,
    )
    .await
    .unwrap();

    let doc = IndexEntry {
        indexed_value: b"The quick brown fox".to_vec(),
        primary_key: b"doc1".to_vec(),
        included_columns: None,
    };

    index.insert(doc).await.unwrap();

    // "the" should be filtered out
    let results = index.search("the", Some(10)).await.unwrap();
    assert_eq!(results.len(), 0);

    // Other terms should work
    let results = index.search("quick", Some(10)).await.unwrap();
    assert_eq!(results.len(), 1);
}

#[tokio::test]
async fn test_fulltext_exists() {
    let metadata = create_test_metadata();
    let store = Arc::new(MockKeyValueStore::new());
    let config = create_test_config();
    let tokenizer_config = TokenizerConfig::default();

    let mut index = FullTextIndex::new(
        metadata,
        store,
        None,
        config,
        tokenizer_config,
        ScoringAlgorithm::TfIdf,
    )
    .await
    .unwrap();

    let doc = IndexEntry {
        indexed_value: b"The quick brown fox".to_vec(),
        primary_key: b"doc1".to_vec(),
        included_columns: None,
    };

    index.insert(doc).await.unwrap();

    // Check if terms exist
    assert!(index.exists(b"fox").await.unwrap());
    assert!(index.exists(b"quick").await.unwrap());
    assert!(!index.exists(b"elephant").await.unwrap());
}

#[tokio::test]
async fn test_fulltext_stats() {
    let metadata = create_test_metadata();
    let store = Arc::new(MockKeyValueStore::new());
    let config = create_test_config();
    let tokenizer_config = TokenizerConfig::default();

    let mut index = FullTextIndex::new(
        metadata,
        store,
        None,
        config,
        tokenizer_config,
        ScoringAlgorithm::TfIdf,
    )
    .await
    .unwrap();

    // Insert multiple documents
    for i in 0..5 {
        let doc = IndexEntry {
            indexed_value: format!("Document number {}", i).into_bytes(),
            primary_key: format!("doc{}", i).into_bytes(),
            included_columns: None,
        };
        index.insert(doc).await.unwrap();
    }

    let stats = index.stats().await.unwrap();
    assert_eq!(stats.entry_count, 5);
}

#[tokio::test]
async fn test_fulltext_phrase_search() {
    let metadata = create_test_metadata();
    let store = Arc::new(MockKeyValueStore::new());
    let config = create_test_config();
    let tokenizer_config = TokenizerConfig::default();

    let mut index = FullTextIndex::new(
        metadata,
        store,
        None,
        config,
        tokenizer_config,
        ScoringAlgorithm::TfIdf,
    )
    .await
    .unwrap();

    // Insert documents with specific phrases
    let doc1 = IndexEntry {
        indexed_value: b"The quick brown fox jumps over the lazy dog".to_vec(),
        primary_key: b"doc1".to_vec(),
        included_columns: None,
    };

    let doc2 = IndexEntry {
        indexed_value: b"A brown fox and a quick rabbit".to_vec(),
        primary_key: b"doc2".to_vec(),
        included_columns: None,
    };

    let doc3 = IndexEntry {
        indexed_value: b"The lazy cat sleeps all day".to_vec(),
        primary_key: b"doc3".to_vec(),
        included_columns: None,
    };

    index.insert(doc1).await.unwrap();
    index.insert(doc2).await.unwrap();
    index.insert(doc3).await.unwrap();

    // Search for exact phrase "quick brown"
    let results = index.phrase_search("quick brown", Some(10)).await.unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].entry.primary_key, b"doc1");

    // Search for phrase "brown fox"
    let results = index.phrase_search("brown fox", Some(10)).await.unwrap();
    assert_eq!(results.len(), 2);
    assert!(results.iter().any(|r| r.entry.primary_key == b"doc1"));
    assert!(results.iter().any(|r| r.entry.primary_key == b"doc2"));

    // Search for phrase that doesn't exist as consecutive terms
    let results = index.phrase_search("fox lazy", Some(10)).await.unwrap();
    assert_eq!(results.len(), 0);
}

// Made with Bob
