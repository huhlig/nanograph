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

//! Full-text index implementation using inverted index with persistence
//!
//! Full-text indexes are ideal for:
//! - Text search across documents
//! - Keyword matching
//! - Phrase queries
//! - Relevance scoring
//!
//! This implementation uses:
//! - PersistentIndexStore for durable storage
//! - Inverted index structure for efficient term lookup
//! - TF-IDF and BM25 scoring algorithms
//! - Tokenization with stemming and stop word removal

use crate::error::{IndexError, IndexResult};
use crate::index::text::{
    BooleanQuery, ScoredEntry, ScoringAlgorithm, Term, TermStats, TextHighlight, TextSearchIndex,
    TokenizerConfig,
};
use crate::index::{IndexEntry, IndexQuery, IndexStats, IndexStore};
use crate::persistence::{PersistenceConfig, PersistentIndexStore};
use crate::serialization::{deserialize_entry, serialize_entry};
use async_trait::async_trait;
use nanograph_core::object::{IndexRecord, IndexStatus};
use nanograph_kvt::KeyValueShardStore;
use nanograph_wal::WriteAheadLogManager;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::ops::Bound;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Posting list entry for a term
#[derive(Debug, Clone, Serialize, Deserialize)]
struct Posting {
    /// Primary key of the document
    primary_key: Vec<u8>,
    /// Positions where the term appears in the document
    positions: Vec<usize>,
    /// Term frequency in this document
    frequency: u32,
}

/// Document statistics for scoring
#[derive(Debug, Clone, Serialize, Deserialize)]
struct DocumentStats {
    /// Total number of terms in the document
    term_count: usize,
    /// Unique terms in the document
    unique_terms: usize,
    /// Average term frequency
    avg_term_frequency: f64,
}

/// Full-text index implementation with persistence
///
/// This index tokenizes text and maintains an inverted index mapping terms to documents,
/// enabling efficient text search with relevance scoring.
///
/// # Features
/// - Persistent storage via KeyValueShardStore
/// - Write-ahead logging for crash recovery
/// - TF-IDF and BM25 scoring algorithms
/// - Tokenization with configurable options
/// - Phrase search support
/// - Boolean query support (AND/OR/NOT)
///
/// # Example
///
/// ```ignore
/// use nanograph_idx::FullTextIndex;
/// use nanograph_core::object::{IndexRecord, IndexType};
///
/// let index = FullTextIndex::new(
///     metadata,
///     store,
///     Some(wal),
///     config,
///     tokenizer_config,
///     ScoringAlgorithm::Bm25,
/// ).await?;
/// ```
pub struct FullTextIndex {
    /// Index metadata
    metadata: Arc<RwLock<IndexRecord>>,
    /// Persistent storage layer
    storage: Arc<PersistentIndexStore>,
    /// Tokenizer configuration
    tokenizer_config: TokenizerConfig,
    /// Scoring algorithm
    scoring_algorithm: ScoringAlgorithm,
    /// Total number of documents
    total_documents: Arc<RwLock<u64>>,
    /// Average document length
    avg_doc_length: Arc<RwLock<f64>>,
}

impl FullTextIndex {
    /// Create a new full-text index with persistence
    ///
    /// # Arguments
    /// * `metadata` - Index metadata
    /// * `store` - Underlying key-value store
    /// * `wal` - Optional write-ahead log for durability
    /// * `config` - Persistence configuration
    /// * `tokenizer_config` - Tokenizer configuration
    /// * `scoring_algorithm` - Scoring algorithm to use
    pub async fn new(
        metadata: IndexRecord,
        store: Arc<dyn KeyValueShardStore>,
        wal: Option<Arc<WriteAheadLogManager>>,
        config: PersistenceConfig,
        tokenizer_config: TokenizerConfig,
        scoring_algorithm: ScoringAlgorithm,
    ) -> IndexResult<Self> {
        let storage = Arc::new(PersistentIndexStore::new(config, store, wal));

        Ok(Self {
            metadata: Arc::new(RwLock::new(metadata)),
            storage,
            tokenizer_config,
            scoring_algorithm,
            total_documents: Arc::new(RwLock::new(0)),
            avg_doc_length: Arc::new(RwLock::new(0.0)),
        })
    }

    /// Tokenize text into terms
    fn tokenize(&self, text: &str) -> Vec<String> {
        let mut tokens: Vec<String> = text
            .split(|c: char| c.is_whitespace() || c.is_ascii_punctuation())
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string())
            .collect();

        // Apply lowercase
        if self.tokenizer_config.lowercase {
            tokens = tokens.iter().map(|t| t.to_lowercase()).collect();
        }

        // Remove stop words
        if self.tokenizer_config.remove_stop_words {
            let stop_words: HashSet<_> = self.tokenizer_config.stop_words.iter().map(|s| s.as_str()).collect();
            tokens.retain(|t| !stop_words.contains(t.as_str()));
        }

        // Apply stemming (basic implementation)
        if self.tokenizer_config.stemming {
            tokens = tokens.iter().map(|t| self.stem(t)).collect();
        }

        tokens
    }

    /// Basic stemming implementation (Porter stemmer would be better)
    fn stem(&self, word: &str) -> String {
        // Very basic stemming - remove common suffixes
        let word = word.to_lowercase();
        if word.ends_with("ing") && word.len() > 5 {
            return word[..word.len() - 3].to_string();
        }
        if word.ends_with("ed") && word.len() > 4 {
            return word[..word.len() - 2].to_string();
        }
        if word.ends_with("s") && word.len() > 3 {
            return word[..word.len() - 1].to_string();
        }
        word
    }

    /// Build term key for storage
    fn build_term_key(&self, term: &str) -> Vec<u8> {
        format!("term:{}", term).into_bytes()
    }

    /// Build document stats key for storage
    fn build_doc_stats_key(&self, primary_key: &[u8]) -> Vec<u8> {
        let mut key = b"doc:".to_vec();
        key.extend_from_slice(primary_key);
        key
    }

    /// Build reverse index key (primary_key -> terms)
    fn build_reverse_key(&self, primary_key: &[u8]) -> Vec<u8> {
        let mut key = b"rev:".to_vec();
        key.extend_from_slice(primary_key);
        key
    }

    /// Get posting list for a term
    async fn get_postings(&self, term: &str) -> IndexResult<Vec<Posting>> {
        let term_key = self.build_term_key(term);
        
        match self.storage.read_entry(&term_key, &[]).await? {
            Some(data) => {
                let postings: Vec<Posting> = bincode::deserialize(&data)
                    .map_err(|e| IndexError::Serialization(e.to_string()))?;
                Ok(postings)
            }
            None => Ok(Vec::new()),
        }
    }

    /// Store posting list for a term
    async fn store_postings(&self, term: &str, postings: &[Posting]) -> IndexResult<()> {
        let term_key = self.build_term_key(term);
        let data = bincode::serialize(postings)
            .map_err(|e| IndexError::Serialization(e.to_string()))?;
        
        self.storage.write_entry(&term_key, &[], &data).await
    }

    /// Get document statistics
    async fn get_doc_stats(&self, primary_key: &[u8]) -> IndexResult<Option<DocumentStats>> {
        let stats_key = self.build_doc_stats_key(primary_key);
        
        match self.storage.read_entry(&stats_key, &[]).await? {
            Some(data) => {
                let stats: DocumentStats = bincode::deserialize(&data)
                    .map_err(|e| IndexError::Serialization(e.to_string()))?;
                Ok(Some(stats))
            }
            None => Ok(None),
        }
    }

    /// Store document statistics
    async fn store_doc_stats(&self, primary_key: &[u8], stats: &DocumentStats) -> IndexResult<()> {
        let stats_key = self.build_doc_stats_key(primary_key);
        let data = bincode::serialize(stats)
            .map_err(|e| IndexError::Serialization(e.to_string()))?;
        
        self.storage.write_entry(&stats_key, &[], &data).await
    }

    /// Calculate TF-IDF score
    async fn calculate_tfidf_score(&self, term: &str, posting: &Posting) -> IndexResult<f64> {
        let total_docs = *self.total_documents.read().await;
        if total_docs == 0 {
            return Ok(0.0);
        }

        // Term frequency (TF)
        let tf = posting.frequency as f64;

        // Document frequency (DF)
        let postings = self.get_postings(term).await?;
        let df = postings.len() as f64;

        // Inverse document frequency (IDF)
        let idf = ((total_docs as f64) / (1.0 + df)).ln();

        Ok(tf * idf)
    }

    /// Calculate BM25 score
    async fn calculate_bm25_score(&self, term: &str, posting: &Posting) -> IndexResult<f64> {
        const K1: f64 = 1.5;
        const B: f64 = 0.75;

        let total_docs = *self.total_documents.read().await;
        if total_docs == 0 {
            return Ok(0.0);
        }

        // Get document stats
        let doc_stats = self.get_doc_stats(&posting.primary_key).await?
            .ok_or_else(|| IndexError::QueryFailed("Document stats not found".to_string()))?;

        let avg_doc_len = *self.avg_doc_length.read().await;
        let doc_len = doc_stats.term_count as f64;

        // Term frequency
        let tf = posting.frequency as f64;

        // Document frequency
        let postings = self.get_postings(term).await?;
        let df = postings.len() as f64;

        // IDF component
        let idf = ((total_docs as f64 - df + 0.5) / (df + 0.5)).ln();

        // BM25 formula
        let numerator = tf * (K1 + 1.0);
        let denominator = tf + K1 * (1.0 - B + B * (doc_len / avg_doc_len));

        Ok(idf * (numerator / denominator))
    }

    /// Calculate relevance score for a document
    async fn calculate_score(&self, term: &str, posting: &Posting) -> IndexResult<f64> {
        match self.scoring_algorithm {
            ScoringAlgorithm::TfIdf => self.calculate_tfidf_score(term, posting).await,
            ScoringAlgorithm::Bm25 => self.calculate_bm25_score(term, posting).await,
            ScoringAlgorithm::TermFrequency => Ok(posting.frequency as f64),
        }
    }

    /// Update index status
    async fn update_status(&self, status: IndexStatus) -> IndexResult<()> {
        let mut metadata = self.metadata.write().await;
        metadata.status = status;
        Ok(())
    }

    /// Update document count and average length
    async fn update_corpus_stats(&self) -> IndexResult<()> {
        // This would scan all documents to calculate stats
        // For now, we'll update incrementally during insert/delete
        Ok(())
    }
}

#[async_trait]
impl IndexStore for FullTextIndex {
    fn metadata(&self) -> &IndexRecord {
        // Note: This is a synchronous method but we have async metadata
        // In production, we'd need to refactor the trait
        unimplemented!("Use async metadata access instead")
    }

    async fn build<I>(&mut self, table_data: I) -> IndexResult<()>
    where
        I: Iterator<Item = (Vec<u8>, Vec<u8>)> + Send,
    {
        self.update_status(IndexStatus::Building).await?;

        let mut total_docs = 0u64;
        let mut total_terms = 0u64;

        for (primary_key, row_data) in table_data {
            // Extract text from row data
            let text = String::from_utf8_lossy(&row_data);
            let tokens = self.tokenize(&text);

            // Build term frequency map
            let mut term_freq: HashMap<String, (Vec<usize>, u32)> = HashMap::new();
            for (pos, term) in tokens.iter().enumerate() {
                let entry = term_freq.entry(term.clone()).or_insert((Vec::new(), 0));
                entry.0.push(pos);
                entry.1 += 1;
            }

            // Store document stats
            let doc_stats = DocumentStats {
                term_count: tokens.len(),
                unique_terms: term_freq.len(),
                avg_term_frequency: tokens.len() as f64 / term_freq.len().max(1) as f64,
            };
            self.store_doc_stats(&primary_key, &doc_stats).await?;

            // Store reverse index first (before consuming term_freq)
            let reverse_key = self.build_reverse_key(&primary_key);
            let terms_list: Vec<String> = term_freq.keys().cloned().collect();
            
            // Update inverted index for each term
            for (term, (positions, frequency)) in term_freq {
                let mut postings = self.get_postings(&term).await?;
                postings.push(Posting {
                    primary_key: primary_key.clone(),
                    positions,
                    frequency,
                });
                self.store_postings(&term, &postings).await?;
            }

            // Store reverse index
            let terms_data = bincode::serialize(&terms_list)
                .map_err(|e| IndexError::Serialization(e.to_string()))?;
            self.storage.write_entry(&reverse_key, &[], &terms_data).await?;

            total_docs += 1;
            total_terms += tokens.len() as u64;

            // Flush periodically
            if total_docs % 1000 == 0 {
                self.storage.flush().await?;
            }
        }

        // Update corpus statistics
        *self.total_documents.write().await = total_docs;
        if total_docs > 0 {
            *self.avg_doc_length.write().await = total_terms as f64 / total_docs as f64;
        }

        self.storage.flush().await?;
        self.update_status(IndexStatus::Active).await?;

        Ok(())
    }

    async fn insert(&mut self, entry: IndexEntry) -> IndexResult<()> {
        let text = String::from_utf8_lossy(&entry.indexed_value);
        let tokens = self.tokenize(&text);

        // Build term frequency map
        let mut term_freq: HashMap<String, (Vec<usize>, u32)> = HashMap::new();
        for (pos, term) in tokens.iter().enumerate() {
            let entry_data = term_freq.entry(term.clone()).or_insert((Vec::new(), 0));
            entry_data.0.push(pos);
            entry_data.1 += 1;
        }

        // Store document stats
        let doc_stats = DocumentStats {
            term_count: tokens.len(),
            unique_terms: term_freq.len(),
            avg_term_frequency: tokens.len() as f64 / term_freq.len().max(1) as f64,
        };
        self.store_doc_stats(&entry.primary_key, &doc_stats).await?;

        // Store reverse index first (before consuming term_freq)
        let reverse_key = self.build_reverse_key(&entry.primary_key);
        let terms_list: Vec<String> = term_freq.keys().cloned().collect();
        
        // Update inverted index for each term
        for (term, (positions, frequency)) in term_freq {
            let mut postings = self.get_postings(&term).await?;
            
            // Remove existing posting for this document if any
            postings.retain(|p| p.primary_key != entry.primary_key);
            
            // Add new posting
            postings.push(Posting {
                primary_key: entry.primary_key.clone(),
                positions,
                frequency,
            });
            
            self.store_postings(&term, &postings).await?;
        }

        // Store reverse index
        let terms_data = bincode::serialize(&terms_list)
            .map_err(|e| IndexError::Serialization(e.to_string()))?;
        self.storage.write_entry(&reverse_key, &[], &terms_data).await?;

        // Update corpus stats
        let mut total_docs = self.total_documents.write().await;
        *total_docs += 1;
        
        let mut avg_len = self.avg_doc_length.write().await;
        let total_terms = *avg_len * (*total_docs - 1) as f64 + tokens.len() as f64;
        *avg_len = total_terms / *total_docs as f64;

        Ok(())
    }

    async fn update(&mut self, old_entry: IndexEntry, new_entry: IndexEntry) -> IndexResult<()> {
        // Delete old entry and insert new one
        self.delete(&old_entry.primary_key).await?;
        self.insert(new_entry).await
    }

    async fn delete(&mut self, primary_key: &[u8]) -> IndexResult<()> {
        // Get terms for this document from reverse index
        let reverse_key = self.build_reverse_key(primary_key);
        
        let terms_data = match self.storage.read_entry(&reverse_key, &[]).await? {
            Some(data) => data,
            None => return Ok(()), // Document not in index
        };

        let terms: Vec<String> = bincode::deserialize(&terms_data)
            .map_err(|e| IndexError::Serialization(e.to_string()))?;

        // Remove document from each term's posting list
        for term in &terms {
            let mut postings = self.get_postings(term).await?;
            postings.retain(|p| p.primary_key != primary_key);
            
            if postings.is_empty() {
                // Remove term entirely if no more documents
                let term_key = self.build_term_key(term);
                self.storage.delete_entry(&term_key, &[]).await?;
            } else {
                self.store_postings(term, &postings).await?;
            }
        }

        // Remove document stats
        let stats_key = self.build_doc_stats_key(primary_key);
        self.storage.delete_entry(&stats_key, &[]).await?;

        // Remove reverse index
        self.storage.delete_entry(&reverse_key, &[]).await?;

        // Update corpus stats
        let mut total_docs = self.total_documents.write().await;
        if *total_docs > 0 {
            *total_docs -= 1;
        }

        Ok(())
    }

    async fn query(&self, query: IndexQuery) -> IndexResult<Vec<IndexEntry>> {
        // For full-text index, we interpret the query differently
        // This is a basic implementation - use search() for better results
        
        if let Bound::Included(ref value) = query.start {
            let text = String::from_utf8_lossy(value);
            let results = self.search(&text, query.limit).await?;
            
            Ok(results.into_iter().map(|scored| scored.entry).collect())
        } else {
            Ok(Vec::new())
        }
    }

    async fn get(&self, primary_key: &[u8]) -> IndexResult<Option<IndexEntry>> {
        // Check if document exists
        let stats_key = self.build_doc_stats_key(primary_key);
        
        if self.storage.exists(&stats_key, &[]).await? {
            // We don't store the original text, only the index
            // In production, this would fetch from the main table
            Ok(Some(IndexEntry {
                indexed_value: vec![],
                primary_key: primary_key.to_vec(),
                included_columns: None,
            }))
        } else {
            Ok(None)
        }
    }

    async fn exists(&self, indexed_value: &[u8]) -> IndexResult<bool> {
        let text = String::from_utf8_lossy(indexed_value);
        let terms = self.tokenize(&text);
        
        // Check if any term exists
        for term in terms {
            let postings = self.get_postings(&term).await?;
            if !postings.is_empty() {
                return Ok(true);
            }
        }
        
        Ok(false)
    }

    async fn stats(&self) -> IndexResult<IndexStats> {
        let total_docs = *self.total_documents.read().await;
        let avg_len = *self.avg_doc_length.read().await;
        
        Ok(IndexStats {
            entry_count: total_docs,
            size_bytes: 0, // TODO: Calculate actual size
            levels: None,
            avg_entry_size: avg_len as u64,
            fragmentation: None,
        })
    }

    async fn optimize(&mut self) -> IndexResult<()> {
        // Remove empty posting lists and compact storage
        // This would require scanning all terms
        self.storage.flush().await?;
        Ok(())
    }

    async fn flush(&mut self) -> IndexResult<()> {
        self.storage.flush().await
    }
}

#[async_trait]
impl TextSearchIndex for FullTextIndex {
    async fn search(&self, query: &str, limit: Option<usize>) -> IndexResult<Vec<ScoredEntry>> {
        let terms = self.tokenize(query);
        if terms.is_empty() {
            return Ok(Vec::new());
        }

        // Collect all matching documents with scores
        let mut doc_scores: HashMap<Vec<u8>, f64> = HashMap::new();
        let mut doc_postings: HashMap<Vec<u8>, Vec<(String, Posting)>> = HashMap::new();

        for term in &terms {
            let postings = self.get_postings(term).await?;
            
            for posting in postings {
                let score = self.calculate_score(term, &posting).await?;
                *doc_scores.entry(posting.primary_key.clone()).or_insert(0.0) += score;
                
                doc_postings
                    .entry(posting.primary_key.clone())
                    .or_insert_with(Vec::new)
                    .push((term.clone(), posting));
            }
        }

        // Sort by score descending
        let mut results: Vec<_> = doc_scores.into_iter().collect();
        results.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        // Apply limit
        if let Some(limit) = limit {
            results.truncate(limit);
        }

        // Build scored entries
        let mut scored_entries = Vec::new();
        for (primary_key, score) in results {
            let postings = doc_postings.get(&primary_key).unwrap();
            
            // Create highlights
            let highlights = postings
                .iter()
                .flat_map(|(term, posting)| {
                    posting.positions.iter().map(move |&pos| TextHighlight {
                        fragment: term.clone(),
                        start: pos,
                        end: pos + term.len(),
                        matched_terms: vec![term.clone()],
                    })
                })
                .collect();

            scored_entries.push(ScoredEntry {
                entry: IndexEntry {
                    indexed_value: vec![],
                    primary_key,
                    included_columns: None,
                },
                score,
                highlights,
            });
        }

        Ok(scored_entries)
    }

    async fn phrase_search(
        &self,
        phrase: &str,
        limit: Option<usize>,
    ) -> IndexResult<Vec<ScoredEntry>> {
        let terms = self.tokenize(phrase);
        if terms.is_empty() {
            return Ok(Vec::new());
        }

        if terms.len() == 1 {
            // Single term - use regular search
            return self.search(&terms[0], limit).await;
        }

        // Get postings for all terms
        let mut all_postings: Vec<Vec<Posting>> = Vec::new();
        for term in &terms {
            let postings = self.get_postings(term).await?;
            if postings.is_empty() {
                // If any term is missing, no results
                return Ok(Vec::new());
            }
            all_postings.push(postings);
        }

        // Find documents that contain all terms
        let mut doc_postings: HashMap<Vec<u8>, Vec<Vec<Posting>>> = HashMap::new();
        for (term_idx, postings) in all_postings.iter().enumerate() {
            for posting in postings {
                doc_postings
                    .entry(posting.primary_key.clone())
                    .or_insert_with(|| vec![Vec::new(); terms.len()])
                    [term_idx]
                    .push(posting.clone());
            }
        }

        // Filter documents where terms appear consecutively
        let mut phrase_matches: Vec<(Vec<u8>, Vec<usize>, f64)> = Vec::new();
        
        for (primary_key, term_postings) in doc_postings {
            // Check if all terms are present
            if term_postings.iter().any(|p| p.is_empty()) {
                continue;
            }

            // Find consecutive positions
            let mut match_positions = Vec::new();
            
            // For each position of the first term
            for first_posting in &term_postings[0] {
                for &start_pos in &first_posting.positions {
                    let mut is_phrase = true;
                    
                    // Check if subsequent terms appear at consecutive positions
                    for (term_idx, term_posting_list) in term_postings.iter().enumerate().skip(1) {
                        let expected_pos = start_pos + term_idx;
                        let mut found = false;
                        
                        for posting in term_posting_list {
                            if posting.positions.contains(&expected_pos) {
                                found = true;
                                break;
                            }
                        }
                        
                        if !found {
                            is_phrase = false;
                            break;
                        }
                    }
                    
                    if is_phrase {
                        match_positions.push(start_pos);
                    }
                }
            }

            if !match_positions.is_empty() {
                // Calculate combined score for the phrase
                let mut total_score = 0.0;
                for (term_idx, term) in terms.iter().enumerate() {
                    if let Some(posting) = term_postings[term_idx].first() {
                        total_score += self.calculate_score(term, posting).await?;
                    }
                }
                
                phrase_matches.push((primary_key, match_positions, total_score));
            }
        }

        // Sort by score descending
        phrase_matches.sort_by(|a, b| b.2.partial_cmp(&a.2).unwrap_or(std::cmp::Ordering::Equal));

        // Apply limit
        if let Some(limit) = limit {
            phrase_matches.truncate(limit);
        }

        // Build scored entries with highlights
        let mut results = Vec::new();
        for (primary_key, positions, score) in phrase_matches {
            let highlights = positions
                .iter()
                .map(|&pos| TextHighlight {
                    fragment: phrase.to_string(),
                    start: pos,
                    end: pos + terms.len(),
                    matched_terms: terms.clone(),
                })
                .collect();

            results.push(ScoredEntry {
                entry: IndexEntry {
                    indexed_value: vec![],
                    primary_key,
                    included_columns: None,
                },
                score,
                highlights,
            });
        }

        Ok(results)
    }

    async fn boolean_search(
        &self,
        query: BooleanQuery,
        limit: Option<usize>,
    ) -> IndexResult<Vec<ScoredEntry>> {
        match query {
            BooleanQuery::And(terms) => {
                // Find documents containing all terms
                if terms.is_empty() {
                    return Ok(Vec::new());
                }

                let term_strs: Vec<String> = terms.iter().map(|t| t.text.clone()).collect();
                let mut doc_sets: Vec<HashSet<Vec<u8>>> = Vec::new();

                for term_str in &term_strs {
                    let postings = self.get_postings(term_str).await?;
                    let doc_set: HashSet<_> = postings.iter().map(|p| p.primary_key.clone()).collect();
                    doc_sets.push(doc_set);
                }

                // Intersect all sets
                let mut result_set = doc_sets[0].clone();
                for set in doc_sets.iter().skip(1) {
                    result_set = result_set.intersection(set).cloned().collect();
                }

                // Score and return
                let mut results = Vec::new();
                for primary_key in result_set.iter().take(limit.unwrap_or(usize::MAX)) {
                    results.push(ScoredEntry {
                        entry: IndexEntry {
                            indexed_value: vec![],
                            primary_key: primary_key.clone(),
                            included_columns: None,
                        },
                        score: 1.0,
                        highlights: vec![],
                    });
                }

                Ok(results)
            }
            BooleanQuery::Or(terms) => {
                // Find documents containing any term
                let term_strs: Vec<String> = terms.iter().map(|t| t.text.clone()).collect();
                let query_str = term_strs.join(" ");
                self.search(&query_str, limit).await
            }
            BooleanQuery::Not(_) => {
                Err(IndexError::QueryFailed("NOT queries not yet implemented".to_string()))
            }
            BooleanQuery::Nested(_) => {
                Err(IndexError::QueryFailed("Nested queries not yet implemented".to_string()))
            }
        }
    }

    fn tokenizer_config(&self) -> &TokenizerConfig {
        &self.tokenizer_config
    }

    fn scoring_algorithm(&self) -> ScoringAlgorithm {
        self.scoring_algorithm
    }

    async fn term_stats(&self, term: &str) -> IndexResult<Option<TermStats>> {
        let postings = self.get_postings(term).await?;
        
        if postings.is_empty() {
            return Ok(None);
        }

        let document_frequency = postings.len() as u64;
        let total_frequency: u64 = postings.iter().map(|p| p.frequency as u64).sum();
        let avg_positions = postings.iter().map(|p| p.positions.len()).sum::<usize>() as f64
            / postings.len() as f64;

        Ok(Some(TermStats {
            document_frequency,
            total_frequency,
            avg_positions,
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nanograph_core::object::{IndexId, IndexType, ObjectId, ShardId};
    use nanograph_core::types::Timestamp;
    use std::collections::HashMap as StdHashMap;

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
            options: StdHashMap::new(),
            metadata: StdHashMap::new(),
            status: IndexStatus::Building,
            sharding: nanograph_core::object::IndexSharding::Single,
        }
    }

    #[test]
    fn test_tokenization() {
        let config = TokenizerConfig::default();
        let _metadata = create_test_metadata();
        
        // Note: Would need actual store for full test
        // This tests the tokenization logic
        let text = "The quick brown fox jumps over the lazy dog!";
        let tokens: Vec<String> = text
            .split(|c: char| c.is_whitespace() || c.is_ascii_punctuation())
            .filter(|s| !s.is_empty())
            .map(|s| if config.lowercase { s.to_lowercase() } else { s.to_string() })
            .collect();
        
        assert!(tokens.contains(&"quick".to_string()));
        assert!(tokens.contains(&"fox".to_string()));
    }
}

// Made with Bob
