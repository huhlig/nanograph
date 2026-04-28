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

//! Text search index traits for full-text search

pub mod fulltext;

use crate::error::IndexResult;
use crate::index::{IndexEntry, IndexStore};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

/// Entry with relevance score
#[derive(Debug, Clone)]
pub struct ScoredEntry {
    /// The index entry
    pub entry: IndexEntry,
    /// Relevance score (higher is more relevant)
    pub score: f64,
    /// Text highlights showing matched terms
    pub highlights: Vec<TextHighlight>,
}

/// Highlighted text fragment
#[derive(Debug, Clone)]
pub struct TextHighlight {
    /// The matched text fragment
    pub fragment: String,
    /// Start position in the original text
    pub start: usize,
    /// End position in the original text
    pub end: usize,
    /// Matched terms in this fragment
    pub matched_terms: Vec<String>,
}

/// Boolean query operators
#[derive(Debug, Clone)]
pub enum BooleanQuery {
    /// Match all terms (AND)
    And(Vec<Term>),
    /// Match any term (OR)
    Or(Vec<Term>),
    /// Exclude term (NOT)
    Not(Box<BooleanQuery>),
    /// Nested query
    Nested(Box<BooleanQuery>),
}

/// Search term
#[derive(Debug, Clone)]
pub struct Term {
    /// The term text
    pub text: String,
    /// Optional boost factor
    pub boost: Option<f64>,
}

impl Term {
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            boost: None,
        }
    }

    pub fn with_boost(mut self, boost: f64) -> Self {
        self.boost = Some(boost);
        self
    }
}

/// Tokenizer configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenizerConfig {
    /// Convert to lowercase
    pub lowercase: bool,
    /// Remove punctuation
    pub remove_punctuation: bool,
    /// Apply stemming
    pub stemming: bool,
    /// Remove stop words
    pub remove_stop_words: bool,
    /// Stop words list
    pub stop_words: Vec<String>,
}

impl Default for TokenizerConfig {
    fn default() -> Self {
        Self {
            lowercase: true,
            remove_punctuation: true,
            stemming: false,
            remove_stop_words: false,
            stop_words: vec![],
        }
    }
}

/// Scoring algorithm
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScoringAlgorithm {
    /// Term Frequency-Inverse Document Frequency
    TfIdf,
    /// Best Matching 25 (Okapi BM25)
    Bm25,
    /// Simple term frequency
    TermFrequency,
}

/// Trait for full-text search indexes
#[async_trait]
pub trait TextSearchIndex: IndexStore {
    /// Search for documents matching query terms
    ///
    /// # Arguments
    /// * `query` - Search query string
    /// * `limit` - Maximum number of results
    ///
    /// # Returns
    /// * `Ok(Vec<ScoredEntry>)` - Results sorted by relevance
    async fn search(&self, query: &str, limit: Option<usize>) -> IndexResult<Vec<ScoredEntry>>;

    /// Search for exact phrase
    ///
    /// # Arguments
    /// * `phrase` - Exact phrase to match
    /// * `limit` - Maximum number of results
    ///
    /// # Returns
    /// * `Ok(Vec<ScoredEntry>)` - Results containing the phrase
    async fn phrase_search(
        &self,
        phrase: &str,
        limit: Option<usize>,
    ) -> IndexResult<Vec<ScoredEntry>>;

    /// Boolean search with AND/OR/NOT operators
    ///
    /// # Arguments
    /// * `query` - Boolean query structure
    /// * `limit` - Maximum number of results
    ///
    /// # Returns
    /// * `Ok(Vec<ScoredEntry>)` - Results matching the boolean query
    async fn boolean_search(
        &self,
        query: BooleanQuery,
        limit: Option<usize>,
    ) -> IndexResult<Vec<ScoredEntry>>;

    /// Get tokenizer configuration
    fn tokenizer_config(&self) -> &TokenizerConfig;

    /// Get scoring algorithm
    fn scoring_algorithm(&self) -> ScoringAlgorithm;

    /// Get term statistics
    ///
    /// # Arguments
    /// * `term` - The term to get statistics for
    ///
    /// # Returns
    /// * `Ok(Some(stats))` - Statistics if term exists
    /// * `Ok(None)` - If term doesn't exist
    async fn term_stats(&self, term: &str) -> IndexResult<Option<TermStats>>;
}

/// Statistics for a term
#[derive(Debug, Clone)]
pub struct TermStats {
    /// Number of documents containing the term
    pub document_frequency: u64,
    /// Total occurrences across all documents
    pub total_frequency: u64,
    /// Average positions per document
    pub avg_positions: f64,
}


