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

use crate::types::{PropertyUpdate, Timestamp};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use crate::object::TenantId;

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
        write!(f, "Tablespace({})", self.0)
    }
}

/// Configuration for Tablespace creation
#[derive(Clone, Debug)]
pub struct TablespaceCreate {
    /// Name of the Tablespace
    pub name: String,
    /// Storage path for the tablespace
    pub storage_path: String,
    /// Storage tier (Hot, Warm, Cold, Archive)
    pub tier: String,
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
    /// * `storage_path`: The filesystem path for the tablespace.
    /// * `tier`: The storage tier (Hot, Warm, Cold, Archive).
    pub fn new(
        name: impl Into<String>,
        storage_path: impl Into<String>,
        tier: impl Into<String>,
    ) -> Self {
        Self {
            name: name.into(),
            storage_path: storage_path.into(),
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
    /// Name of the Tablespace
    pub name: Option<String>,
    /// Storage path for the tablespace
    pub storage_path: Option<String>,
    /// Storage tier (Hot, Warm, Cold, Archive)
    pub tier: Option<String>,
    /// Configuration Options for the Tablespace
    pub options: Vec<PropertyUpdate>,
    /// Tablespace Metadata (Informative)
    pub metadata: Vec<PropertyUpdate>,
}

impl TablespaceUpdate {
    /// Set the name of the Tablespace.
    ///
    /// # Arguments
    ///
    /// * `name`: The new name for the Tablespace.
    pub fn set_name(mut self, name: String) -> Self {
        self.name = Some(name);
        self
    }

    /// Set the storage path of the Tablespace.
    ///
    /// # Arguments
    ///
    /// * `storage_path`: The new storage path for the Tablespace.
    pub fn set_storage_path(mut self, storage_path: String) -> Self {
        self.storage_path = Some(storage_path);
        self
    }

    /// Set the storage tier of the Tablespace.
    ///
    /// # Arguments
    ///
    /// * `tier`: The new storage tier for the Tablespace.
    pub fn set_tier(mut self, tier: String) -> Self {
        self.tier = Some(tier);
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
    /// Storage path for the tablespace
    pub storage_path: String,
    /// Storage tier (Hot, Warm, Cold, Archive)
    pub tier: String,
    /// Timestamp when the tablespace was created
    pub created_at: Timestamp,
    /// Timestamp when the tablespace was last modified
    pub last_modified: Timestamp,
    /// Tenants assigned to this tablespace
    pub tenants: Vec<TenantId>,
    /// Configuration Options for the Tablespace
    pub options: HashMap<String, String>,
    /// Tablespace Metadata (Informative)
    pub metadata: HashMap<String, String>,
}

impl From<TablespaceRecord> for TablespaceMetadata {
    fn from(record: TablespaceRecord) -> Self {
        Self {
            id: record.id,
            name: record.name,
            storage_path: record.storage_path,
            tier: record.tier,
            created_at: record.created_at,
            last_modified: record.last_modified,
            tenants: vec![],
            options: record.options,
            metadata: record.metadata,
        }
    }
}

/// Metadata Record for a Tablespace.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TablespaceRecord {
    /// Unique identifier for the Tablespace
    pub id: TablespaceId,
    /// Name of the Tablespace
    pub name: String,
    /// Storage path for the tablespace
    pub storage_path: String,
    /// Storage tier (Hot, Warm, Cold, Archive)
    pub tier: String,
    /// Version of the Tablespace Record
    pub version: u64,
    /// Timestamp when the tablespace was created
    pub created_at: Timestamp,
    /// Timestamp when the tablespace was last modified
    pub last_modified: Timestamp,
    /// Tenants assigned to this tablespace
    pub tenants: Vec<TenantId>,
    /// Configuration Options for the Tablespace
    pub options: HashMap<String, String>,
    /// Tablespace Metadata (Informative)
    pub metadata: HashMap<String, String>,
}
