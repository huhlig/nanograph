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
    pub shard_id: u64,
    pub integrity: IntegrityAlgorithm,
    pub compression: CompressionAlgorithm,
    pub encryption: EncryptionAlgorithm,
    pub encryption_key: Option<EncryptionKey>,
    pub memtable_size: usize,
    pub block_size: usize,
    pub durability: Durability,
}

impl Default for LSMTreeOptions {
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
        }
    }
}

impl LSMTreeOptions {
    /// Create options with a specific shard ID
    pub fn with_shard_id(mut self, shard_id: u64) -> Self {
        self.shard_id = shard_id;
        self
    }
}
