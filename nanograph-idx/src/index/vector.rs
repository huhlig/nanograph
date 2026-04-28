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

//! Vector similarity index traits

mod flat;
mod hnsw;
mod ivf;
mod annoy;
mod vamana;

use crate::error::IndexResult;
use crate::index::IndexEntry;
use crate::index::IndexStore;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

/// Entry with similarity information
#[derive(Debug, Clone)]
pub struct SimilarityEntry {
    /// The index entry
    pub entry: IndexEntry,
    /// Distance from query vector (lower is more similar)
    pub distance: f32,
    /// Similarity score (higher is more similar, 0.0-1.0)
    pub similarity_score: f32,
}

/// Distance metric for vector similarity
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum VectorDistanceMetric {
    /// Euclidean distance (L2 norm)
    Euclidean,
    /// Cosine distance (1 - cosine similarity)
    Cosine,
    /// Dot product (inner product)
    DotProduct,
    /// Manhattan distance (L1 norm)
    Manhattan,
}

/// Build parameters for ANN indexes
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnnBuildParams {
    /// Number of neighbors per node (HNSW)
    pub m: Option<usize>,
    /// Construction time parameter (HNSW)
    pub ef_construction: Option<usize>,
    /// Number of clusters (IVF)
    pub n_lists: Option<usize>,
    /// Number of trees (Annoy)
    pub n_trees: Option<usize>,
    /// Max degree (Vamana)
    pub max_degree: Option<usize>,
}

/// Search parameters for ANN indexes
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnnSearchParams {
    /// Search time parameter (HNSW)
    pub ef_search: Option<usize>,
    /// Number of clusters to probe (IVF)
    pub n_probe: Option<usize>,
    /// Search depth (Annoy)
    pub search_k: Option<usize>,
}

/// Base trait for all vector indexes
#[async_trait]
pub trait VectorIndex: IndexStore {
    /// K-nearest neighbors search
    ///
    /// # Arguments
    /// * `query_vector` - The query vector
    /// * `k` - Number of nearest neighbors to return
    ///
    /// # Returns
    /// * `Ok(Vec<SimilarityEntry>)` - K nearest vectors
    async fn search_knn(&self, query_vector: &[f32], k: usize)
    -> IndexResult<Vec<SimilarityEntry>>;

    /// Range search (all vectors within distance threshold)
    ///
    /// # Arguments
    /// * `query_vector` - The query vector
    /// * `radius` - Maximum distance threshold
    ///
    /// # Returns
    /// * `Ok(Vec<SimilarityEntry>)` - All vectors within radius
    async fn search_range(
        &self,
        query_vector: &[f32],
        radius: f32,
    ) -> IndexResult<Vec<SimilarityEntry>>;

    /// Get vector dimensionality
    fn dimensions(&self) -> usize;

    /// Get distance metric
    fn distance_metric(&self) -> VectorDistanceMetric;

    /// Get vector statistics
    async fn vector_stats(&self) -> IndexResult<VectorStats>;
}

/// Statistics for vector index
#[derive(Debug, Clone)]
pub struct VectorStats {
    /// Number of vectors
    pub vector_count: u64,
    /// Vector dimensionality
    pub dimensions: usize,
    /// Average vector magnitude
    pub avg_magnitude: f32,
    /// Index-specific statistics
    pub index_specific: Option<String>,
}

/// Trait for exact vector search
#[async_trait]
pub trait ExactVectorIndex: VectorIndex {
    /// Guaranteed exact k-NN results
    ///
    /// # Arguments
    /// * `query_vector` - The query vector
    /// * `k` - Number of nearest neighbors
    ///
    /// # Returns
    /// * `Ok(Vec<SimilarityEntry>)` - Exact k nearest vectors
    async fn exact_search_knn(
        &self,
        query_vector: &[f32],
        k: usize,
    ) -> IndexResult<Vec<SimilarityEntry>>;
}

/// Trait for approximate vector search
#[async_trait]
pub trait ApproximateVectorIndex: VectorIndex {
    /// Approximate k-NN with recall target
    ///
    /// # Arguments
    /// * `query_vector` - The query vector
    /// * `k` - Number of nearest neighbors
    /// * `recall_target` - Target recall (0.0-1.0)
    ///
    /// # Returns
    /// * `Ok(Vec<SimilarityEntry>)` - Approximate k nearest vectors
    async fn approximate_search_knn(
        &self,
        query_vector: &[f32],
        k: usize,
        recall_target: f32,
    ) -> IndexResult<Vec<SimilarityEntry>>;

    /// Get build parameters
    fn build_params(&self) -> &AnnBuildParams;

    /// Get search parameters
    fn search_params(&self) -> &AnnSearchParams;

    /// Estimate recall for given search parameters
    async fn estimate_recall(&self, search_params: &AnnSearchParams) -> IndexResult<f32>;
}

/// Trait for mutable vector indexes
#[async_trait]
pub trait MutableVectorIndex: ApproximateVectorIndex {
    /// Add vector to index (incremental)
    ///
    /// # Arguments
    /// * `vector` - The vector to add
    /// * `primary_key` - Primary key for the vector
    ///
    /// # Returns
    /// * `Ok(())` - If successful
    async fn add_vector(&mut self, vector: &[f32], primary_key: Vec<u8>) -> IndexResult<()>;

    /// Remove vector from index
    ///
    /// # Arguments
    /// * `primary_key` - Primary key of vector to remove
    ///
    /// # Returns
    /// * `Ok(())` - If successful
    async fn remove_vector(&mut self, primary_key: &[u8]) -> IndexResult<()>;

    /// Update vector in place
    ///
    /// # Arguments
    /// * `primary_key` - Primary key of vector to update
    /// * `new_vector` - New vector values
    ///
    /// # Returns
    /// * `Ok(())` - If successful
    async fn update_vector(&mut self, primary_key: &[u8], new_vector: &[f32]) -> IndexResult<()>;
}

/// Trait for immutable vector indexes
#[async_trait]
pub trait ImmutableVectorIndex: ApproximateVectorIndex {
    /// Build index from complete dataset (one-time)
    ///
    /// # Arguments
    /// * `vectors` - All vectors to index
    ///
    /// # Returns
    /// * `Ok(())` - If build succeeds
    async fn build_from_vectors(&mut self, vectors: Vec<(Vec<u8>, Vec<f32>)>) -> IndexResult<()>;

    /// Check if index is built and ready
    fn is_built(&self) -> bool;

    /// Rebuild index (expensive operation)
    ///
    /// # Returns
    /// * `Ok(())` - If rebuild succeeds
    async fn rebuild(&mut self) -> IndexResult<()>;
}

// Made with Bob
