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

/// Cluster identifier (global)
///
/// Represents the entire Nanograph deployment across all regions.
/// Uses a 32-bit identifier for compactness and to avoid overflow.
#[derive(
    Copy, Clone, Debug, Default, Ord, PartialOrd, Eq, PartialEq, Hash, Serialize, Deserialize,
)]
pub struct ClusterId(pub u32);

impl ClusterId {
    /// Create a new cluster identifier.
    pub fn new(id: u32) -> Self {
        Self(id)
    }

    /// Return the cluster identifier as a u32.
    pub fn as_u32(&self) -> u32 {
        self.0
    }
}

impl From<u32> for ClusterId {
    fn from(id: u32) -> Self {
        Self(id)
    }
}

impl std::fmt::Display for ClusterId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Cluster({})", self.0)
    }
}

/// Configuration for Cluster creation
#[derive(Clone, Debug)]
pub struct ClusterCreate {
    /// Name of the cluster
    pub name: String,
    /// Configuration Options for the Cluster
    pub options: HashMap<String, String>,
    /// Cluster Metadata (Informative)
    pub metadata: HashMap<String, String>,
}

impl ClusterCreate {
    /// Create a new ClusterCreate instance with a name.
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            options: HashMap::new(),
            metadata: HashMap::new(),
        }
    }
    /// Add a configuration option to the cluster.
    ///
    /// # Arguments
    ///
    /// * `key`: The key of the option.
    /// * `value`: The value of the option.
    pub fn add_option(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.options.insert(key.into(), value.into());
        self
    }
    /// Remove a configuration option from the cluster.
    ///
    /// # Arguments
    ///
    /// * `key`: The key of the option to remove.
    pub fn clear_option(mut self, key: impl Into<String>) -> Self {
        self.options.remove(&key.into());
        self
    }
    /// Add informative metadata to the cluster.
    ///
    /// # Arguments
    ///
    /// * `key`: The key of the metadata entry.
    /// * `value`: The value of the metadata entry.
    pub fn add_metadata(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.metadata.insert(key.into(), value.into());
        self
    }
    /// Remove informative metadata from the cluster.
    ///
    /// # Arguments
    ///
    /// * `key`: The key of the metadata entry to remove.
    pub fn clear_metadata(mut self, key: impl Into<String>) -> Self {
        self.metadata.remove(&key.into());
        self
    }
}

/// Configuration for Cluster update
#[derive(Clone, Debug, Default)]
pub struct ClusterUpdate {
    /// Name of the cluster
    pub name: Option<String>,
    /// Configuration Options for the Cluster
    pub options: Vec<PropertyUpdate>,
    /// Cluster Metadata (Informative)
    pub metadata: Vec<PropertyUpdate>,
}

impl ClusterUpdate {
    /// Set the name of the cluster.
    ///
    /// # Arguments
    ///
    /// * `name`: The new name for the cluster.
    pub fn set_name(mut self, name: String) -> Self {
        self.name = Some(name);
        self
    }
    /// Add or update a configuration option for the cluster.
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
    /// Clear a configuration option from the cluster.
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
    /// Add or update informative metadata for the cluster.
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
    /// Clear informative metadata from the cluster.
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

/// Metadata for a cluster.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ClusterMetadata {
    /// Unique identifier for the cluster.
    pub id: ClusterId,
    /// Name of the cluster.
    pub name: String,
    /// Timestamp when the cluster was created.
    pub created_at: Timestamp,
    /// Timestamp when the cluster metadata was last modified.
    pub last_modified: Timestamp,
    /// Configuration Options for the Cluster.
    pub options: HashMap<String, String>,
    /// Cluster Metadata (Informative).
    pub metadata: HashMap<String, String>,
}

impl From<ClusterRecord> for ClusterMetadata {
    fn from(record: ClusterRecord) -> Self {
        Self {
            id: record.id,
            name: record.name,
            created_at: record.created_at,
            last_modified: record.last_modified,
            options: record.options,
            metadata: record.metadata,
        }
    }
}

/// Metadata Record for a cluster.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ClusterRecord {
    /// Unique identifier for the cluster.
    pub id: ClusterId,
    /// Name of the cluster.
    pub name: String,
    /// Metadata version (incremented on each change).
    pub version: u64,
    /// Timestamp when the cluster was created.
    pub created_at: Timestamp,
    /// Timestamp when the cluster metadata was last modified.
    pub last_modified: Timestamp,
    /// Configuration Options for the Cluster.
    pub options: HashMap<String, String>,
    /// Cluster Metadata (Informative).
    pub metadata: HashMap<String, String>,
}
