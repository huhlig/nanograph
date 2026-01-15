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

//! Configuration for LSM storage with tablespace support

use crate::options::LSMTreeOptions;

/// Configuration for LSM storage with tablespace-resolved paths
///
/// This configuration is created by the shard manager after resolving
/// tablespace paths for a specific shard.
#[derive(Clone, Debug)]
pub struct LSMStorageConfig {
    /// Data directory path (resolved from tablespace)
    pub data_dir: String,

    /// WAL directory path (resolved from tablespace)
    pub wal_dir: String,

    /// LSM tree options
    pub options: LSMTreeOptions,
}

impl LSMStorageConfig {
    /// Create a new LSM storage configuration
    pub fn new(data_dir: String, wal_dir: String) -> Self {
        Self {
            data_dir,
            wal_dir,
            options: LSMTreeOptions::default(),
        }
    }

    /// Create with custom LSM options
    pub fn with_options(data_dir: String, wal_dir: String, options: LSMTreeOptions) -> Self {
        Self {
            data_dir,
            wal_dir,
            options,
        }
    }
}

impl Default for LSMStorageConfig {
    fn default() -> Self {
        Self {
            data_dir: "/lsm/data".to_string(),
            wal_dir: "/lsm/wal".to_string(),
            options: LSMTreeOptions::default(),
        }
    }
}
