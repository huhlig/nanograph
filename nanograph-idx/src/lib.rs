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

//! # Nanograph Index Implementations
//!
//! This crate provides various index implementations for the Nanograph database system,
//! enabling efficient data retrieval through secondary indexes, unique constraints,
//! full-text search, and spatial queries.
//!
//! ## Implemented Index Types
//!
//! - **Secondary Indexes (B-Tree)**: For range queries and sorted scans
//! - **Unique Indexes (Hash)**: For fast lookups with uniqueness constraints
//! - **Full-Text Indexes (Inverted Index)**: For text search and keyword matching
//! - **Spatial Indexes (R-Tree)**: For geographic and geometric queries
//!
//! ## Planned Vector Similarity Search Indexes
//!
//! The following vector index types are defined in `IndexType` but not yet implemented.
//! These will support AI/ML workloads with high-dimensional embeddings:
//!
//! - **VectorFlat**: Brute-force exact search (small datasets)
//! - **VectorHNSW**: Graph-based approximate search (fast, high recall)
//! - **VectorIVF**: Clustering-based search (memory efficient)
//! - **VectorIVFPQ**: Compressed vectors (very large datasets)
//! - **VectorAnnoy**: Tree-based search (static datasets, recommendations)
//! - **VectorVamana**: Disk-based search (billion-scale datasets)
//!
//! ## Example
//!
//! ```ignore
//! use nanograph_idx::IndexStore;
//! use nanograph_core::object::{IndexCreate, IndexType};
//!
//! // Create an index configuration
//! let config = IndexCreate::new(
//!     "users_email_idx",
//!     IndexType::Unique,
//!     vec!["email".to_string()],
//! );
//!
//! // Build the index (implementation-specific)
//! // let index = HashIndex::new(config)?;
//! ```

mod distributed;
mod error;
mod index;
mod persistence;
#[cfg(feature = "raft")]
mod raft_adapter;
mod serialization;

pub use self::distributed::{ConsensusGroup, DistributedIndex, IndexCommand, IndexCommandResponse};
pub use self::error::{IndexError, IndexResult};
pub use self::index::{
    IndexEntry, IndexQuery, IndexStats, IndexStore,
    ordered::{OrderedIndex, UniqueIndex},
    text::{
        BooleanQuery, ScoredEntry, ScoringAlgorithm, Term, TermStats, TextHighlight,
        TextSearchIndex, TokenizerConfig,
    },
};
pub use self::persistence::{CacheStats, PersistenceConfig, PersistentIndexStore};
pub use self::serialization::{
    CURRENT_VERSION, SerializedIndexEntry, batch_deserialize_entries, batch_serialize_entries,
    deserialize_entry, deserialize_metadata, estimate_entry_size, serialize_entry,
    serialize_metadata,
};

// Re-export index implementations
pub mod btree {
    pub use crate::index::ordered::btree::BTreeIndex;
}

pub mod hash {
    pub use crate::index::ordered::hash::HashIndex;
}

pub mod fulltext {
    pub use crate::index::text::fulltext::FullTextIndex;
}

// Re-export Raft adapter (optional, only if nanograph-raft is available)
#[cfg(feature = "raft")]
pub mod raft_adapter {
    pub use crate::raft_adapter::RaftConsensusAdapter;
}
