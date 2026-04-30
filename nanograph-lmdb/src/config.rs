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

use serde::{Deserialize, Serialize};

/// LMDB storage configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LMDBConfig {
    /// Maximum size of the database in bytes
    /// Default: 1GB
    pub max_db_size: usize,

    /// Maximum number of databases (shards)
    /// Default: 128
    pub max_dbs: u32,

    /// Maximum number of readers
    /// Default: 126
    pub max_readers: u32,

    /// Use write-ahead logging (sync mode)
    /// Default: true (safer but slower)
    pub use_writemap: bool,

    /// Sync to disk on commit
    /// Default: true (safer but slower)
    pub sync_on_commit: bool,

    /// Use read-only mode
    /// Default: false
    pub read_only: bool,

    /// Create database if it doesn't exist
    /// Default: true
    pub create_if_missing: bool,
}

impl Default for LMDBConfig {
    fn default() -> Self {
        Self {
            max_db_size: 1024 * 1024 * 1024, // 1GB
            max_dbs: 128,
            max_readers: 126,
            use_writemap: false,
            sync_on_commit: true,
            read_only: false,
            create_if_missing: true,
        }
    }
}

impl LMDBConfig {
    /// Create a new configuration with default values
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the maximum database size
    pub fn with_max_db_size(mut self, size: usize) -> Self {
        self.max_db_size = size;
        self
    }

    /// Set the maximum number of databases
    pub fn with_max_dbs(mut self, max_dbs: u32) -> Self {
        self.max_dbs = max_dbs;
        self
    }

    /// Set the maximum number of readers
    pub fn with_max_readers(mut self, max_readers: u32) -> Self {
        self.max_readers = max_readers;
        self
    }

    /// Enable or disable writemap mode
    pub fn with_writemap(mut self, use_writemap: bool) -> Self {
        self.use_writemap = use_writemap;
        self
    }

    /// Enable or disable sync on commit
    pub fn with_sync_on_commit(mut self, sync: bool) -> Self {
        self.sync_on_commit = sync;
        self
    }

    /// Set read-only mode
    pub fn with_read_only(mut self, read_only: bool) -> Self {
        self.read_only = read_only;
        self
    }

    /// Set create if missing flag
    pub fn with_create_if_missing(mut self, create: bool) -> Self {
        self.create_if_missing = create;
        self
    }
}

/// LMDB storage configuration with resolved paths
/// This is used by the shard manager after tablespace resolution
#[derive(Debug, Clone)]
pub struct LMDBStorageConfig {
    /// Base directory for LMDB database files
    pub data_dir: String,

    /// LMDB configuration options
    pub config: LMDBConfig,
}

impl LMDBStorageConfig {
    /// Create a new storage configuration
    pub fn new(data_dir: String, config: LMDBConfig) -> Self {
        Self { data_dir, config }
    }
}

// Made with Bob
