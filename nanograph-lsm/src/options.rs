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

use nanograph_util::{
    CompressionAlgorithm, EncryptionAlgorithm, EncryptionKey, IntegrityAlgorithm,
};
use nanograph_wal::Durability;

/// LSM Tree Configuration Options
#[derive(Debug, Clone)]
pub struct LSMTreeOptions {
    /// Shard identifier for this LSM tree instance
    /// Used to identify WAL segments and coordinate distributed operations
    pub shard_id: u128,
    /// Integrity algorithm for data blocks
    pub integrity: IntegrityAlgorithm,
    /// Compression algorithm for data blocks
    pub compression: CompressionAlgorithm,
    /// Encryption algorithm for data blocks
    pub encryption: EncryptionAlgorithm,
    /// Optional encryption key
    pub encryption_key: Option<EncryptionKey>,
    /// Maximum size of the memtable in bytes before flushing
    pub memtable_size: usize,
    /// Size of data blocks in bytes
    pub block_size: usize,
    /// Durability level for write operations
    pub durability: Durability,
    /// Value size threshold for blob separation (WiscKey-style)
    /// Values larger than this threshold are stored in separate blob log files
    /// Default: 4096 bytes (4KB)
    pub value_separation_threshold: usize,
    /// Enable value separation (WiscKey-style blob log)
    /// When enabled, values larger than value_separation_threshold are stored separately
    pub enable_value_separation: bool,
    /// Blob log garbage collection threshold
    /// Files with live data ratio below this threshold are candidates for GC
    /// Default: 0.5 (50%)
    pub blob_gc_threshold: f64,
}

impl Default for LSMTreeOptions {
    /// Create default LSM tree options
    fn default() -> Self {
        Self {
            shard_id: 0, // Default to shard 0 for single-node deployments
            integrity: IntegrityAlgorithm::None,
            compression: CompressionAlgorithm::None,
            encryption: EncryptionAlgorithm::None,
            encryption_key: None,
            memtable_size: 64 * 1024 * 1024, // 64MB
            block_size: 4096,                // 4KB
            durability: Durability::Flush,   // Default to Flush for balance
            value_separation_threshold: 4096, // 4KB - values larger than this go to blob log
            enable_value_separation: true,   // Enable WiscKey-style value separation by default
            blob_gc_threshold: 0.5,          // GC blob files with <50% live data
        }
    }
}

impl LSMTreeOptions {
    /// Create options with a specific shard ID
    pub fn with_shard_id(mut self, shard_id: u128) -> Self {
        self.shard_id = shard_id;
        self
    }
}
