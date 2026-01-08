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

use crate::{ShardIndex, StorageEngineType, TableId};
use std::collections::HashMap;

/// Configuration for Cluster creation
pub struct ClusterConfig {
    pub name: String,
}

impl ClusterConfig {
    pub fn new(name: impl Into<String>) -> Self {
        Self { name: name.into() }
    }
}

/// Configuration for Region creation
pub struct RegionConfig {
    pub name: String,
}

impl RegionConfig {
    pub fn new(name: impl Into<String>) -> Self {
        Self { name: name.into() }
    }
}

/// Configuration for Server creation
pub struct ServerConfig {
    pub name: String,
}

impl ServerConfig {
    pub fn new(name: impl Into<String>) -> Self {
        Self { name: name.into() }
    }
}

/// Configuration for Namespace creation
pub struct NamespaceConfig {
    pub name: String,
}

impl NamespaceConfig {
    pub fn new(name: impl Into<String>) -> Self {
        Self { name: name.into() }
    }
}

/// Configuration for table creation
#[derive(Debug, Clone)]
pub struct TableConfig {
    /// Name of the Table
    pub name: String,
    /// Engine Type
    pub engine_type: StorageEngineType,
    /// Number of shards to distribute data across (default: 1 for single-node)
    pub shard_count: u32,
    /// Number of replicas per shard (default: 1 for single-node)
    pub replication_factor: usize,
    /// Partitioning strategy (None for single-shard tables)
    pub partitioner: Option<crate::Partitioner>,
    /// Additional engine-specific options
    pub options: HashMap<String, String>,
}

impl TableConfig {
    pub fn new(name: impl Into<String>, engine_type: StorageEngineType) -> Self {
        Self {
            name: name.into(),
            engine_type,
            shard_count: 1,        // Default to single shard
            replication_factor: 1, // Default to no replication
            partitioner: None,     // No partitioner for single shard
            options: HashMap::new(),
        }
    }

    pub fn with_option(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.options.insert(key.into(), value.into());
        self
    }

    pub fn with_shards(mut self, shard_count: u32) -> Self {
        self.shard_count = shard_count;
        // Set default hash partitioner if sharding is enabled
        if shard_count > 1 && self.partitioner.is_none() {
            self.partitioner = Some(crate::Partitioner::default());
        }
        self
    }

    pub fn with_partitioner(mut self, partitioner: crate::Partitioner) -> Self {
        self.partitioner = Some(partitioner);
        self
    }

    pub fn with_replication(mut self, replication_factor: usize) -> Self {
        self.replication_factor = replication_factor;
        self
    }
}

/// Configuration for shard creation
#[derive(Debug, Clone)]
pub struct ShardConfig {
    /// Table ID for which the shard is being created
    pub table: TableId,
    /// Shard Index for which the shard is being created
    pub index: ShardIndex,
    /// Storage engine type for the shard
    pub engine_type: StorageEngineType,
    /// Number of replicas per shard (default: 1 for single-node)
    pub replication_factor: usize,
}

impl ShardConfig {
    pub fn new(table: TableId, index: ShardIndex, engine_type: StorageEngineType) -> Self {
        Self {
            table,
            index,
            engine_type,
            replication_factor: 1, // Default to no replication
        }
    }
    pub fn with_replication(mut self, replication_factor: usize) -> Self {
        self.replication_factor = replication_factor;
        self
    }
}
