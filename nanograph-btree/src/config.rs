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

//! Configuration for B+Tree storage with tablespace support

/// Configuration for B+Tree storage with tablespace-resolved paths
///
/// This configuration is created by the shard manager after resolving
/// tablespace paths for a specific shard.
#[derive(Clone, Debug)]
pub struct BTreeStorageConfig {
    /// Data directory path (resolved from tablespace)
    pub data_dir: String,

    /// WAL directory path (resolved from tablespace)
    pub wal_dir: String,

    /// B+Tree order (number of keys per node)
    pub order: usize,

    /// Cache size in megabytes
    pub cache_size_mb: usize,
}

impl BTreeStorageConfig {
    /// Create a new B+Tree storage configuration
    pub fn new(data_dir: String, wal_dir: String) -> Self {
        Self {
            data_dir,
            wal_dir,
            order: 128,
            cache_size_mb: 128,
        }
    }

    /// Create with custom order and cache size
    pub fn with_options(
        data_dir: String,
        wal_dir: String,
        order: usize,
        cache_size_mb: usize,
    ) -> Self {
        Self {
            data_dir,
            wal_dir,
            order,
            cache_size_mb,
        }
    }
}

impl Default for BTreeStorageConfig {
    fn default() -> Self {
        Self {
            data_dir: "/btree/data".to_string(),
            wal_dir: "/btree/wal".to_string(),
            order: 128,
            cache_size_mb: 128,
        }
    }
}
