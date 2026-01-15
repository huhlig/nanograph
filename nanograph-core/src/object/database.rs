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

use crate::object::{NamespaceId, TenantId};
use crate::types::{PropertyUpdate, Timestamp};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Database Identifier
#[derive(
    Copy, Clone, Debug, Default, Ord, PartialOrd, Eq, PartialEq, Hash, Serialize, Deserialize,
)]
pub struct DatabaseId(pub u32);

impl DatabaseId {
    /// Create a new database identifier.
    pub fn new(id: u32) -> Self {
        Self(id)
    }

    /// Return the database identifier as a u64.
    pub fn as_u32(&self) -> u32 {
        self.0
    }
}

impl From<u32> for DatabaseId {
    fn from(id: u32) -> Self {
        Self(id)
    }
}

impl std::fmt::Display for DatabaseId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Database({})", self.0)
    }
}

/// Configuration for Database creation
#[derive(Clone, Debug)]
pub struct DatabaseCreate {
    /// Name of the Database
    pub name: String,
    /// Configuration Options for the Database
    pub options: HashMap<String, String>,
    /// Database Metadata (Informative)
    pub metadata: HashMap<String, String>,
}

impl DatabaseCreate {
    /// Create a new Database creation configuration.
    ///
    /// # Arguments
    ///
    /// * `name`: The name of the new Database.
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            options: HashMap::new(),
            metadata: HashMap::new(),
        }
    }
    /// Add or update a configuration option for the Database.
    ///
    /// # Arguments
    ///
    /// * `key`: The key of the option to set.
    /// * `value`: The value to assign to the option.
    pub fn add_option(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.options.insert(key.into(), value.into());
        self
    }
    /// Clear a configuration option from the Database.
    ///
    /// # Arguments
    ///
    /// * `key`: The key of the option to clear.
    pub fn clear_option(mut self, key: impl Into<String>) -> Self {
        self.options.remove(&key.into());
        self
    }
    /// Add or update informative metadata for the Database.
    ///
    /// # Arguments
    ///
    /// * `key`: The key of the metadata entry to set.
    /// * `value`: The value to assign to the metadata entry.
    pub fn add_metadata(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.metadata.insert(key.into(), value.into());
        self
    }
    /// Clear informative metadata from the Database.
    ///
    /// # Arguments
    ///
    /// * `key`: The key of the metadata entry to clear.
    pub fn clear_metadata(mut self, key: impl Into<String>) -> Self {
        self.metadata.remove(&key.into());
        self
    }
}

/// Configuration for Database creation
#[derive(Clone, Debug, Default)]
pub struct DatabaseUpdate {
    /// Name of the Database
    pub name: Option<String>,
    /// Configuration Options for the Database
    pub options: Vec<PropertyUpdate>,
    /// Database Metadata (Informative)
    pub metadata: Vec<PropertyUpdate>,
}

impl DatabaseUpdate {
    /// Set the name of the Database.
    ///
    /// # Arguments
    ///
    /// * `name`: The new name for the Database.
    pub fn set_name(mut self, name: String) -> Self {
        self.name = Some(name);
        self
    }
    /// Add or update a configuration option for the Database.
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
    /// Clear a configuration option from the Database.
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
    /// Add or update informative metadata for the Database.
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
    /// Clear informative metadata from the Database.
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

/// Metadata for a Database.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DatabaseMetadata {
    /// Unique identifier for the Database
    pub id: DatabaseId,
    /// Tenant Id
    pub tenant: TenantId,
    /// Name of the Database
    pub name: String,
    /// Version of the Database Record
    pub version: u64,
    /// Timestamp when the schema was created
    pub created_at: Timestamp,
    /// Timestamp when the schema was last modified
    pub last_modified: Timestamp,
    /// Root Namespace of Database
    pub root_namespace: NamespaceId,
    /// Configuration Options for the Cluster
    pub options: HashMap<String, String>,
    /// Cluster Metadata (Informative)
    pub metadata: HashMap<String, String>,
}
