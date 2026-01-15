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

use crate::object::ClusterId;
use crate::types::{PropertyUpdate, Timestamp};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Region identifier (geographic/data center)
///
/// Each region is a full replica of all data for data locality.
/// Examples: us-east-1, eu-west-1, ap-south-1
/// Uses a 32-bit identifier for compactness and to avoid overflow.
#[derive(
    Copy, Clone, Debug, Default, Ord, PartialOrd, Eq, PartialEq, Hash, Serialize, Deserialize,
)]
pub struct RegionId(pub u32);

impl RegionId {
    /// Create a new region identifier.
    pub fn new(id: u32) -> Self {
        Self(id)
    }

    /// Return the region identifier as a u32.
    pub fn as_u32(&self) -> u32 {
        self.0
    }
}

impl From<u32> for RegionId {
    fn from(id: u32) -> Self {
        Self(id)
    }
}

impl std::fmt::Display for RegionId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Region({})", self.0)
    }
}

/// Configuration for Region creation
#[derive(Clone, Debug)]
pub struct RegionCreate {
    /// Name of the region
    pub name: String,
    /// Identifier of the cluster this region belongs to
    pub cluster: ClusterId,
    /// Cluster Options
    pub options: HashMap<String, String>,
    /// Region Metadata (Informative)
    pub metadata: HashMap<String, String>,
}

impl RegionCreate {
    /// Create a new RegionCreate instance with a name and cluster ID.
    ///
    /// # Arguments
    ///
    /// * `name`: The name of the region.
    /// * `cluster`: The identifier of the cluster this region belongs to.
    pub fn new(name: impl Into<String>, cluster: ClusterId) -> Self {
        Self {
            name: name.into(),
            cluster,
            options: HashMap::new(),
            metadata: HashMap::new(),
        }
    }
    /// Add a configuration option to the region.
    ///
    /// # Arguments
    ///
    /// * `key`: The key of the option.
    /// * `value`: The value of the option.
    pub fn add_option(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.options.insert(key.into(), value.into());
        self
    }
    /// Remove a configuration option from the region.
    ///
    /// # Arguments
    ///
    /// * `key`: The key of the option to remove.
    pub fn clear_option(mut self, key: impl Into<String>) -> Self {
        self.options.remove(&key.into());
        self
    }
    /// Add informative metadata to the region.
    ///
    /// # Arguments
    ///
    /// * `key`: The key of the metadata entry.
    /// * `value`: The value of the metadata entry.
    pub fn add_metadata(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.metadata.insert(key.into(), value.into());
        self
    }
    /// Remove informative metadata from the region.
    ///
    /// # Arguments
    ///
    /// * `key`: The key of the metadata entry to remove.
    pub fn clear_metadata(mut self, key: impl Into<String>) -> Self {
        self.metadata.remove(&key.into());
        self
    }
}

/// Configuration for Region update
#[derive(Clone, Debug, Default)]
pub struct RegionUpdate {
    /// Name of the region
    pub name: Option<String>,
    /// Configuration Options for the Region
    pub options: Vec<PropertyUpdate>,
    /// Region Metadata (Informative)
    pub metadata: Vec<PropertyUpdate>,
}

impl RegionUpdate {
    /// Set the name of the region.
    ///
    /// # Arguments
    ///
    /// * `name`: The new name for the region.
    pub fn set_name(mut self, name: String) -> Self {
        self.name = Some(name);
        self
    }
    /// Add or update a configuration option for the region.
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
    /// Clear a configuration option from the region.
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
    /// Add or update informative metadata for the region.
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
    /// Clear informative metadata from the region.
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

/// Metadata for a region.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RegionMetadata {
    /// Unique identifier for the region
    pub id: RegionId,
    /// Name of the region
    pub name: String,
    /// Version of the Region Record
    pub version: u64,
    /// Identifier of the cluster this region belongs to
    pub cluster: ClusterId,
    /// Timestamp when the Region was created
    pub created_at: Timestamp,
    /// Timestamp when the schema was last modified
    pub last_modified: Timestamp,
    /// Configuration Options for the Region
    pub options: HashMap<String, String>,
    /// Region Metadata (Informative)
    pub metadata: HashMap<String, String>,
}
