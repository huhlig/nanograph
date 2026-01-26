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

use crate::object::TenantId;
use crate::types::{PropertyUpdate, Timestamp};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Tablespace identifier (cluster-wide)
///
/// Represents a logical storage location that can be configured
/// differently on each node in the cluster. Tablespaces enable:
/// - Storage tiering (hot/warm/cold/archive)
/// - Multi-tenant isolation
/// - Capacity management across multiple volumes
/// - Heterogeneous storage configurations per node
#[derive(
    Copy, Clone, Debug, Default, Ord, PartialOrd, Eq, PartialEq, Hash, Serialize, Deserialize,
)]
pub struct TablespaceId(pub u32);

impl TablespaceId {
    /// Create a new tablespace identifier.
    pub fn new(id: u32) -> Self {
        Self(id)
    }

    /// Return the tablespace identifier as a u32.
    pub fn as_u32(&self) -> u32 {
        self.0
    }

    /// Default tablespace (always ID 0)
    /// Used when no explicit tablespace is specified
    pub const DEFAULT: TablespaceId = TablespaceId(0);
}

impl From<u32> for TablespaceId {
    fn from(id: u32) -> Self {
        Self(id)
    }
}

impl std::fmt::Display for TablespaceId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Tablespace({:X})", self.0)
    }
}

/// Storage tier classification
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum StorageTier {
    /// Hot tier - fastest storage (NVMe, RAM)
    Hot,
    /// Warm tier - balanced storage (SSD)
    Warm,
    /// Cold tier - slower storage (HDD)
    Cold,
    /// Archive tier - archival storage (object storage, tape)
    Archive,
}

impl From<&str> for StorageTier {
    fn from(value: &str) -> Self {
        match value.to_uppercase().as_str() {
            "HOT" => StorageTier::Hot,
            "WARM" => StorageTier::Warm,
            "COLD" => StorageTier::Cold,
            "ARCHIVE" => StorageTier::Archive,
            _ => StorageTier::Warm,
        }
    }
}

/// Configuration for Tablespace creation
#[derive(Clone, Debug)]
pub struct TablespaceCreate {
    /// Name of the Tablespace
    pub name: String,
    /// Storage tier (Hot, Warm, Cold, Archive)
    pub tier: StorageTier,
    /// Configuration Options for the Tablespace
    pub options: HashMap<String, String>,
    /// Tablespace Metadata (Informative)
    pub metadata: HashMap<String, String>,
}

impl TablespaceCreate {
    /// Create a new Tablespace creation configuration.
    ///
    /// # Arguments
    ///
    /// * `id`: The unique identifier for the new Tablespace.
    /// * `name`: The name of the new Tablespace.
    /// * `tier`: The storage tier (Hot, Warm, Cold, Archive).
    pub fn new(name: impl Into<String>, tier: impl Into<StorageTier>) -> Self {
        Self {
            name: name.into(),
            tier: tier.into(),
            options: HashMap::new(),
            metadata: HashMap::new(),
        }
    }
    /// Add or update a configuration option for the Tablespace.
    ///
    /// # Arguments
    ///
    /// * `key`: The key of the option to set.
    /// * `value`: The value to assign to the option.
    pub fn add_option(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.options.insert(key.into(), value.into());
        self
    }
    /// Clear a configuration option from the Tablespace.
    ///
    /// # Arguments
    ///
    /// * `key`: The key of the option to clear.
    pub fn clear_option(mut self, key: impl Into<String>) -> Self {
        self.options.remove(&key.into());
        self
    }
    /// Add or update informative metadata for the Tablespace.
    ///
    /// # Arguments
    ///
    /// * `key`: The key of the metadata entry to set.
    /// * `value`: The value to assign to the metadata entry.
    pub fn add_metadata(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.metadata.insert(key.into(), value.into());
        self
    }
    /// Clear informative metadata from the Tablespace.
    ///
    /// # Arguments
    ///
    /// * `key`: The key of the metadata entry to clear.
    pub fn clear_metadata(mut self, key: impl Into<String>) -> Self {
        self.metadata.remove(&key.into());
        self
    }
}

/// Configuration for Tablespace update
#[derive(Clone, Debug, Default)]
pub struct TablespaceUpdate {
    /// Storage tier (Hot, Warm, Cold, Archive)
    pub tier: Option<StorageTier>,
    /// Configuration Options for the Tablespace
    pub options: Vec<PropertyUpdate>,
    /// Tablespace Metadata (Informative)
    pub metadata: Vec<PropertyUpdate>,
}

impl TablespaceUpdate {
    /// Change the storage tier of the Tablespace.
    ///
    /// # Arguments
    ///
    /// * `tier`: The new storage tier for the Tablespace.
    pub fn set_tier(mut self, tier: impl Into<StorageTier>) -> Self {
        self.tier = Some(tier.into());
        self
    }
    /// Add or update a configuration option for the Namespace.
    ///
    /// # Arguments
    ///
    /// * `key`: The key of the option to set.
    /// * `value`: The value to assign to the option.
    pub fn add_option(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.options
            .push(PropertyUpdate::Set(key.into(), value.into()));
        self
    }
    /// Clear a configuration option from the Namespace.
    ///
    /// # Arguments
    ///
    /// * `key`: The key of the option to clear.
    pub fn clear_option(mut self, key: impl Into<String>) -> Self {
        let key = key.into();
        self.options.retain(|opt| match opt {
            PropertyUpdate::Set(k, _) => k != &key,
            PropertyUpdate::Clear(k) => k != &key,
        });
        self
    }
    /// Add or update informative metadata for the Namespace.
    ///
    /// # Arguments
    ///
    /// * `key`: The key of the metadata entry to set.
    /// * `value`: The value to assign to the metadata entry.
    pub fn add_metadata(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.metadata
            .push(PropertyUpdate::Set(key.into(), value.into()));
        self
    }
    /// Clear informative metadata from the Namespace.
    ///
    /// # Arguments
    ///
    /// * `key`: The key of the metadata entry to clear.
    pub fn clear_metadata(mut self, key: impl Into<String>) -> Self {
        let key = key.into();
        self.metadata.retain(|k| k.key() != key);
        self
    }
}

/// Metadata for a Tablespace.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TablespaceMetadata {
    /// Unique identifier for the Tablespace
    pub id: TablespaceId,
    /// Name of the Tablespace
    pub name: String,
    /// Storage tier (Hot, Warm, Cold, Archive)
    pub tier: StorageTier,
    /// Timestamp when the tablespace was created
    pub created_at: Timestamp,
    /// Timestamp when the tablespace was last modified
    pub last_modified: Timestamp,
    /// Tenants assigned to this tablespace
    pub tenants: Vec<TenantId>,
    /// Local path for this tablespace on this node if exists
    pub local_path: Option<String>,
    /// Configuration Options for the Tablespace
    pub options: HashMap<String, String>,
    /// Tablespace Metadata (Informative)
    pub metadata: HashMap<String, String>,
}

impl From<(TablespaceRecord, Option<LocalTablespaceRecord>)> for TablespaceMetadata {
    fn from(
        (global_record, local_record): (TablespaceRecord, Option<LocalTablespaceRecord>),
    ) -> Self {
        Self {
            id: global_record.id,
            name: global_record.name,
            tier: global_record.tier,
            created_at: global_record.created_at,
            last_modified: global_record.updated_at,
            tenants: vec![],
            local_path: local_record.map(|r| r.base_path),
            options: global_record.options,
            metadata: global_record.metadata,
        }
    }
}

/// Metadata Record for a Tablespace.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TablespaceRecord {
    /// Unique identifier for the Tablespace
    pub id: TablespaceId,
    /// Version of the Tablespace Record
    pub version: u64,
    /// Timestamp when the tablespace was created
    pub created_at: Timestamp,
    /// Timestamp when the tablespace was last modified
    pub updated_at: Timestamp,
    /// Name of the Tablespace
    pub name: String,
    /// Storage tier (Hot, Warm, Cold, Archive)
    pub tier: StorageTier,
    /// Tenants assigned to this tablespace
    pub tenants: Vec<TenantId>,
    /// Configuration Options for the Tablespace
    pub options: HashMap<String, String>,
    /// Tablespace Metadata (Informative)
    pub metadata: HashMap<String, String>,
}

/// Local Tablespace Configuration
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LocalTablespaceRecord {
    /// Unique identifier for the Tablespace
    pub id: TablespaceId,
    /// Name of the Tablespace
    pub name: String,
    /// Base path for this tablespace on this node
    pub base_path: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tablespace_id() {
        let id = TablespaceId::new(0x12345678);
        assert_eq!(id.as_u32(), 0x12345678);
        assert_eq!(TablespaceId::from(0x12345678), id);
        assert_eq!(format!("{}", id), "Tablespace(12345678)");
    }
}
