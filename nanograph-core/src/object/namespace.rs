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

use crate::object::{ObjectId, TablespaceId};
use crate::types::{PropertyUpdate, Timestamp};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Namespace identifier
///
/// Names are stored separately in metadata and mapped to IDs.
#[derive(
    Copy, Clone, Debug, Default, Ord, PartialOrd, Eq, PartialEq, Hash, Serialize, Deserialize,
)]
pub struct NamespaceId(pub ObjectId);

impl NamespaceId {
    /// Create a new namespace identifier.
    pub fn new(id: ObjectId) -> Self {
        Self(id)
    }

    pub fn object(&self) -> ObjectId {
        self.0
    }
}

impl From<u32> for NamespaceId {
    fn from(id: u32) -> Self {
        Self(ObjectId::new(id))
    }
}

impl std::fmt::Display for NamespaceId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Namespace({})", self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_namespace_id() {
        let id = NamespaceId::new(ObjectId::new(0x12345678));
        assert_eq!(id.object().as_u32(), 0x12345678);
        assert_eq!(NamespaceId::from(0x12345678), id);
        assert_eq!(format!("{}", id), "Namespace(12345678)");
    }
}

/// Configuration for Namespace creation
#[derive(Clone, Debug)]
pub struct NamespaceCreate {
    /// Name of the Namespace
    pub name: String,
    /// Parent Namespace for this Namespace
    pub parent: NamespaceId,
    /// Configuration Options for the Cluster
    pub options: HashMap<String, String>,
    /// Cluster Metadata (Informative)
    pub metadata: HashMap<String, String>,
}

impl NamespaceCreate {
    /// Create a new Namespace creation configuration.
    ///
    /// # Arguments
    ///
    /// * `name`: The name of the new Namespace.
    pub fn new(name: impl Into<String>, parent: NamespaceId) -> Self {
        Self {
            name: name.into(),
            parent,
            options: HashMap::new(),
            metadata: HashMap::new(),
        }
    }
    /// Add or update a configuration option for the Namespace.
    ///
    /// # Arguments
    ///
    /// * `key`: The key of the option to set.
    /// * `value`: The value to assign to the option.
    pub fn add_option(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.options.insert(key.into(), value.into());
        self
    }
    /// Clear a configuration option from the Namespace.
    ///
    /// # Arguments
    ///
    /// * `key`: The key of the option to clear.
    pub fn clear_option(mut self, key: impl Into<String>) -> Self {
        self.options.remove(&key.into());
        self
    }
    /// Add or update informative metadata for the Namespace.
    ///
    /// # Arguments
    ///
    /// * `key`: The key of the metadata entry to set.
    /// * `value`: The value to assign to the metadata entry.
    pub fn add_metadata(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.metadata.insert(key.into(), value.into());
        self
    }
    /// Clear informative metadata from the Namespace.
    ///
    /// # Arguments
    ///
    /// * `key`: The key of the metadata entry to clear.
    pub fn clear_metadata(mut self, key: impl Into<String>) -> Self {
        self.metadata.remove(&key.into());
        self
    }
}

/// Configuration for Namespace update
#[derive(Clone, Debug, Default)]
pub struct NamespaceUpdate {
    /// Name of the Namespace
    pub name: Option<String>,
    /// Configuration Options for the Namespace
    pub options: Vec<PropertyUpdate>,
    /// Namespace Metadata (Informative)
    pub metadata: Vec<PropertyUpdate>,
}

impl NamespaceUpdate {
    /// Set the name of the Namespace.
    ///
    /// # Arguments
    ///
    /// * `name`: The new name for the Namespace.
    pub fn set_name(mut self, name: String) -> Self {
        self.name = Some(name);
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

/// Metadata for a Namespace.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct NamespaceMetadata {
    /// Unique identifier for the Namespace
    pub id: NamespaceId,
    /// Name of the Namespace
    pub name: String,
    /// Path of the namespace within the namespace hierarchy
    pub path: String,
    /// Timestamp when the namespace was created
    pub created_at: Timestamp,
    /// Timestamp when the namespace was last modified
    pub last_modified: Timestamp,
    /// Default Tablespace for the Namespace
    pub default_tablespace: Option<TablespaceId>,
    /// Configuration Options for the Namespace
    pub options: HashMap<String, String>,
    /// Namespace Metadata (Informative)
    pub metadata: HashMap<String, String>,
}

/// Metadata for a Namespace.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct NamespaceRecord {
    /// Unique identifier for the Namespace
    pub id: NamespaceId,
    /// Name of the Namespace
    pub name: String,
    /// Version of the Namespace Record
    pub version: u64,
    /// Path of the namespace within the namespace hierarchy
    pub path: String,
    /// Timestamp when the namespace was created
    pub created_at: Timestamp,
    /// Timestamp when the namespace was last modified
    pub last_modified: Timestamp,
    /// Default Tablespace for the Namespace
    pub default_tablespace: Option<TablespaceId>,
    /// Configuration Options for the Namespace
    pub options: HashMap<String, String>,
    /// Namespace Metadata (Informative)
    pub metadata: HashMap<String, String>,
}
