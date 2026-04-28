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

use crate::object::shard::{Partitioner, ShardNumber, StorageEngineType};
use crate::object::{DatabaseId, ObjectId, TenantId};
use crate::types::{PropertyUpdate, Timestamp};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Table identifier
///
/// Uses ObjectId (u32) for globally unique identification within a database.
/// Names are stored separately in metadata and mapped to IDs.
///
/// **IMPORTANT**: TableId shares the same ObjectId allocation pool with IndexId,
/// FunctionId, and NamespaceId within a database. This prevents collisions when
/// constructing ShardIds for storage operations.
#[derive(
    Copy, Clone, Debug, Default, Ord, PartialOrd, Eq, PartialEq, Hash, Serialize, Deserialize,
)]
pub struct TableId(pub ObjectId);

impl TableId {
    /// Create a new table identifier.
    pub fn new(id: ObjectId) -> Self {
        Self(id)
    }

    pub fn object(&self) -> ObjectId {
        self.0
    }
}

impl From<u32> for TableId {
    fn from(id: u32) -> Self {
        Self(ObjectId::new(id))
    }
}

impl std::fmt::Display for TableId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Table({})", self.0)
    }
}

/// Configuration for table creation
#[derive(Debug, Clone)]
pub struct TableCreate {
    /// Name of the Table
    pub name: String,
    /// Path of the Table within the namespace hierarchy
    pub path: String,
    /// Engine Type
    pub engine_type: StorageEngineType,
    /// Sharding configuration
    pub sharding_config: TableSharding,
    /// Additional engine-specific options
    pub options: HashMap<String, String>,
    /// Table Metadata (Informative)
    pub metadata: HashMap<String, String>,
}

impl TableCreate {
    /// Create a new Table creation configuration.
    ///
    /// # Arguments
    ///
    /// * `name`: The name of the new Table.
    /// * `engine_type`: The storage engine type to use for the Table.
    pub fn new(
        name: impl Into<String>,
        path: impl Into<String>,
        engine_type: StorageEngineType,
    ) -> Self {
        Self {
            name: name.into(),
            path: path.into(),
            engine_type,
            sharding_config: TableSharding::Single,
            options: HashMap::new(),
            metadata: HashMap::new(),
        }
    }

    /// Add or update a configuration option for the Table.
    ///
    /// # Arguments
    ///
    /// * `key`: The key of the option to set.
    /// * `value`: The value to assign to the option.
    pub fn with_option(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.options.insert(key.into(), value.into());
        self
    }

    /// Set the sharding configuration for the Table.
    ///
    /// # Arguments
    ///
    /// * `shard_count`: The number of shards to create.
    /// * `partitioner`: The partitioning strategy to use.
    /// * `replication_factor`: The number of replicas for each shard.
    pub fn with_sharding(
        mut self,
        shard_count: u32,
        partitioner: Partitioner,
        replication_factor: usize,
    ) -> Self {
        self.sharding_config = TableSharding::Multiple {
            shard_count,
            partitioner,
            replication_factor,
        };
        self
    }
}

/// Configuration for table update
#[derive(Clone, Debug, Default)]
pub struct TableUpdate {
    /// New name for the Table
    pub name: Option<String>,
    /// New engine type for the Table
    pub engine_type: Option<StorageEngineType>,
    /// New sharding configuration for the Table
    pub sharding_config: Option<TableSharding>,
    /// Table configuration options to update
    pub options: Vec<PropertyUpdate>,
    /// Table metadata to update
    pub metadata: Vec<PropertyUpdate>,
}

impl TableUpdate {
    /// Set a new name for the Table.
    ///
    /// # Arguments
    ///
    /// * `name`: The new name to set.
    pub fn set_name(&mut self, name: impl Into<String>) -> &mut Self {
        self.name = Some(name.into());
        self
    }
    /// Set a new engine type for the Table.
    ///
    /// # Arguments
    ///
    /// * `engine_type`: The new engine type to set.
    pub fn set_engine_type(&mut self, engine_type: StorageEngineType) -> &mut Self {
        self.engine_type = Some(engine_type);
        self
    }
    /// Set a new sharding configuration for the Table.
    ///
    /// # Arguments
    ///
    /// * `sharding_config`: The new sharding configuration to set.
    pub fn set_sharding_config(&mut self, sharding_config: TableSharding) -> &mut Self {
        self.sharding_config = Some(sharding_config);
        self
    }
    /// Add or update a configuration option for the Table.
    ///
    /// # Arguments
    ///
    /// * `key`: The key of the option to set.
    /// * `value`: The value to assign to the option.
    pub fn set_option(&mut self, key: impl Into<String>, value: impl Into<String>) -> &mut Self {
        self.options
            .push(PropertyUpdate::Set(key.into(), value.into()));
        self
    }
    /// Clear a configuration option from the Table.
    ///
    /// # Arguments
    ///
    /// * `key`: The key of the option to clear.
    pub fn clear_option(&mut self, key: impl Into<String>) -> &mut Self {
        self.options.push(PropertyUpdate::Clear(key.into()));
        self
    }
}

/// Metadata for a table.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TableMetadata {
    /// Unique identifier for the table
    pub id: TableId,
    /// Name of the table
    pub name: String,
    /// Path of the table within the namespace hierarchy
    pub path: String,
    /// Timestamp when the table was created
    pub created_at: Timestamp,
    /// Type of storage engine used by the table
    pub engine_type: StorageEngineType,
    /// Timestamp when the table was last modified
    pub last_modified: Timestamp,
    /// Distributed table Config (Single or Multiple)
    pub sharding: TableSharding,
    /// Additional engine-specific options
    pub options: HashMap<String, String>,
    /// Table Metadata (Informative)
    pub metadata: HashMap<String, String>,
}

impl From<TableRecord> for TableMetadata {
    fn from(record: TableRecord) -> Self {
        Self {
            id: record.table_id,
            name: record.name,
            path: record.path,
            created_at: record.created_at,
            engine_type: record.engine_type,
            last_modified: record.updated_at,
            sharding: record.sharding,
            options: record.options,
            metadata: record.metadata,
        }
    }
}

/// Metadata for a table.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TableRecord {
    /// Unique identifier for the table
    pub table_id: TableId,
    /// Name of the table
    pub name: String,
    /// Path of the table within the namespace hierarchy
    pub path: String,
    /// Version of the Table Record
    pub version: u64,
    /// Timestamp when the table was created
    pub created_at: Timestamp,
    /// Type of storage engine used by the table
    pub engine_type: StorageEngineType,
    /// Timestamp when the table was last modified
    pub updated_at: Timestamp,
    /// Distributed table Config (Single or Multiple)
    pub sharding: TableSharding,
    /// Additional engine-specific options
    pub options: HashMap<String, String>,
    /// Table Metadata (Informative)
    pub metadata: HashMap<String, String>,
}

/// Table Sharding Configuration
#[derive(Clone, Debug, Default, Ord, PartialOrd, Eq, PartialEq, Serialize, Deserialize)]
pub enum TableSharding {
    /// Single Shard
    #[default]
    Single,
    /// Multiple Shards with Partitioning and Replication
    Multiple {
        /// Number of Shards
        shard_count: u32,
        /// Key Partitioner
        partitioner: Partitioner,
        /// Number of replicas per shard
        replication_factor: usize,
    },
}

/// Shard identifier for distributed data partitioning
///
/// Each shard represents a partition of the key space and is replicated
/// across multiple nodes using Raft consensus. The shard_id is used to:
/// - Identify WAL segments
/// - Route keys to the correct storage engine
/// - Coordinate replication and failover
#[derive(
    Copy, Clone, Debug, Default, Ord, PartialOrd, Eq, PartialEq, Hash, Serialize, Deserialize,
)]
pub struct TableShardId(pub u128);

impl TableShardId {
    /// Create a new shard identifier.
    pub fn new(id: u128) -> Self {
        Self(id)
    }

    /// Create a ShardId from a ContainerID (Tenant+Database), ObjectID and a ShardIndex.
    pub fn from_parts(
        tenant: TenantId,
        database: DatabaseId,
        table: TableId,
        shard_number: ShardNumber,
    ) -> Self {
        Self(
            (tenant.0 as u128) << 96
                | (database.0 as u128) << 64
                | (table.object().as_u32() as u128) << 32
                | (shard_number.0 as u128) << 00,
        )
    }

    pub fn tenant(&self) -> TenantId {
        TenantId((self.0 >> 96) as u32)
    }
    pub fn database(&self) -> DatabaseId {
        DatabaseId((self.0 >> 64) as u32)
    }

    /// Extract the TableId from the ShardId.
    pub fn table(&self) -> TableId {
        TableId(ObjectId::new((self.0 >> 32) as u32))
    }

    /// Extract the ShardNumber from the ShardId.
    pub fn shard_number(&self) -> ShardNumber {
        ShardNumber(self.0 as u32)
    }

    /// Get the underlying u64 value.
    pub fn as_u128(&self) -> u128 {
        self.0
    }
}

impl From<u128> for TableShardId {
    fn from(id: u128) -> Self {
        Self(id)
    }
}

impl std::fmt::Display for TableShardId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Shard({:X})", self.0)
    }
}
