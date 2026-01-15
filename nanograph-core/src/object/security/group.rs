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

use crate::object::{ObjectId, PermissionGrant, UserId};
use crate::types::{PropertyUpdate, Timestamp};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use crate::object::security::{SystemRoleId, TenantRoleId};

/// Group identifier
#[derive(
    Copy, Clone, Debug, Default, Ord, PartialOrd, Eq, PartialEq, Hash, Serialize, Deserialize,
)]
pub struct TenantGroupId(pub ObjectId);

impl TenantGroupId {
    pub fn new(id: u32) -> Self {
        Self(id)
    }

    pub fn as_u32(&self) -> u32 {
        self.0
    }
}

impl From<u32> for TenantGroupId {
    fn from(id: u32) -> Self {
        Self(id)
    }
}

impl std::fmt::Display for TenantGroupId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Group({})", self.0)
    }
}

/// Configuration for Tenant User creation
#[derive(Clone, Debug)]
pub struct TenantGroupCreate {
    /// Configuration Options for the Cluster
    pub options: HashMap<String, String>,
    /// Cluster Metadata (Informative)
    pub metadata: HashMap<String, String>,
}

impl TenantGroupCreate {
    /// Create a new Tenant User creation configuration.
    pub fn new() -> Self {
        Self {
            options: Default::default(),
            metadata: Default::default(),
        }
    }
    /// Add or update a configuration option for the Tenant user.
    ///
    /// # Arguments
    ///
    /// * `key`: The key of the option to set.
    /// * `value`: The value to assign to the option.
    pub fn add_option(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.options.insert(key.into(), value.into());
        self
    }
    /// Clear a configuration option from the Tenant.
    ///
    /// # Arguments
    ///
    /// * `key`: The key of the option to clear.
    pub fn clear_option(mut self, key: impl Into<String>) -> Self {
        self.options.remove(&key.into());
        self
    }
    /// Add or update informative metadata for the Tenant user.
    ///
    /// # Arguments
    ///
    /// * `key`: The key of the metadata entry to set.
    /// * `value`: The value to assign to the metadata entry.
    pub fn add_metadata(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.metadata.insert(key.into(), value.into());
        self
    }
    /// Clear informative metadata from the Tenant.
    ///
    /// # Arguments
    ///
    /// * `key`: The key of the metadata entry to clear.
    pub fn clear_metadata(mut self, key: impl Into<String>) -> Self {
        self.metadata.remove(&key.into());
        self
    }
}

/// Configuration for Tenant User Updates
#[derive(Clone, Debug, Default)]
pub struct TenantGroupUpdate {
    /// Configuration Options for the Cluster
    pub options: Vec<PropertyUpdate>,
    /// Cluster Metadata (Informative)
    pub metadata: Vec<PropertyUpdate>,
}

impl TenantGroupUpdate {
    /// Add or update a configuration option for the Tenant.
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
    /// Clear a configuration option from the Tenant.
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
    /// Add or update informative metadata for the Tenant.
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
    /// Clear informative metadata from the Tenant.
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

/// Group metadata - collection of users with shared permissions
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TenantGroupMetadata {
    /// Unique identifier for the Group
    pub id: TenantGroupId,
    /// Name of the Group
    pub name: String,
    /// Timestamp when the group was created
    pub created_at: Timestamp,
    /// Timestamp when the group was last modified
    pub last_modified: Timestamp,
    /// List of users in this group
    pub members: Vec<UserId>,
    /// List of roles assigned to this group
    pub roles: Vec<TenantRoleId>,
    /// Direct permission grants for the group
    pub grants: Vec<PermissionGrant>,
    /// Configuration Options for the Group
    pub options: HashMap<String, String>,
    /// Group Metadata (Informative)
    pub metadata: HashMap<String, String>,
}

impl From<TenantGroupRecord> for TenantGroupMetadata {
    fn from(record: TenantGroupRecord) -> Self {
        Self {
            id: record.id,
            name: record.name,
            created_at: record.created_at,
            last_modified: record.last_modified,
            members: record.members,
            roles: record.roles,
            grants: record.grants,
            options: record.options,
            metadata: record.metadata,
        }
    }
}

/// Group metadata - collection of users with shared permissions
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TenantGroupRecord {
    /// Unique identifier for the Group
    pub id: TenantGroupId,
    /// Name of the Group
    pub name: String,
    /// Version of the Group Record
    pub version: u64,
    /// Timestamp when the group was created
    pub created_at: Timestamp,
    /// Timestamp when the group was last modified
    pub last_modified: Timestamp,
    /// List of users in this group
    pub members: Vec<UserId>,
    /// List of roles assigned to this group
    pub roles: Vec<TenantRoleId>,
    /// Direct permission grants for the group
    pub grants: Vec<PermissionGrant>,
    /// Configuration Options for the Group
    pub options: HashMap<String, String>,
    /// Group Metadata (Informative)
    pub metadata: HashMap<String, String>,
}

/// Group identifier
#[derive(
    Copy, Clone, Debug, Default, Ord, PartialOrd, Eq, PartialEq, Hash, Serialize, Deserialize,
)]
pub struct SystemGroupId(pub ObjectId);

impl SystemGroupId {
    pub fn new(id: u32) -> Self {
        Self(id)
    }

    pub fn as_u32(&self) -> u32 {
        self.0
    }
}

impl From<u32> for SystemGroupId {
    fn from(id: u32) -> Self {
        Self(id)
    }
}

impl std::fmt::Display for SystemGroupId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Group({})", self.0)
    }
}

/// Configuration for Tenant User creation
#[derive(Clone, Debug)]
pub struct SystemGroupCreate {
    /// Configuration Options for the Cluster
    pub options: HashMap<String, String>,
    /// Cluster Metadata (Informative)
    pub metadata: HashMap<String, String>,
}

impl SystemGroupCreate {
    /// Create a new Tenant User creation configuration.
    pub fn new() -> Self {
        Self {
            options: Default::default(),
            metadata: Default::default(),
        }
    }
    /// Add or update a configuration option for the Tenant user.
    ///
    /// # Arguments
    ///
    /// * `key`: The key of the option to set.
    /// * `value`: The value to assign to the option.
    pub fn add_option(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.options.insert(key.into(), value.into());
        self
    }
    /// Clear a configuration option from the Tenant.
    ///
    /// # Arguments
    ///
    /// * `key`: The key of the option to clear.
    pub fn clear_option(mut self, key: impl Into<String>) -> Self {
        self.options.remove(&key.into());
        self
    }
    /// Add or update informative metadata for the Tenant user.
    ///
    /// # Arguments
    ///
    /// * `key`: The key of the metadata entry to set.
    /// * `value`: The value to assign to the metadata entry.
    pub fn add_metadata(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.metadata.insert(key.into(), value.into());
        self
    }
    /// Clear informative metadata from the Tenant.
    ///
    /// # Arguments
    ///
    /// * `key`: The key of the metadata entry to clear.
    pub fn clear_metadata(mut self, key: impl Into<String>) -> Self {
        self.metadata.remove(&key.into());
        self
    }
}

/// Configuration for Tenant User Updates
#[derive(Clone, Debug, Default)]
pub struct SystemGroupUpdate {
    /// Configuration Options for the Cluster
    pub options: Vec<PropertyUpdate>,
    /// Cluster Metadata (Informative)
    pub metadata: Vec<PropertyUpdate>,
}

impl SystemGroupUpdate {
    /// Add or update a configuration option for the Tenant.
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
    /// Clear a configuration option from the Tenant.
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
    /// Add or update informative metadata for the Tenant.
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
    /// Clear informative metadata from the Tenant.
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

/// Group metadata - collection of users with shared permissions
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SystemGroupMetadata {
    /// Unique identifier for the Group
    pub id: SystemGroupId,
    /// Name of the Group
    pub name: String,
    /// Timestamp when the group was created
    pub created_at: Timestamp,
    /// Timestamp when the group was last modified
    pub last_modified: Timestamp,
    /// List of users in this group
    pub members: Vec<UserId>,
    /// List of roles assigned to this group
    pub roles: Vec<SystemRoleId>,
    /// Direct permission grants for the group
    pub grants: Vec<PermissionGrant>,
    /// Configuration Options for the Group
    pub options: HashMap<String, String>,
    /// Group Metadata (Informative)
    pub metadata: HashMap<String, String>,
}

impl From<SystemGroupRecord> for SystemGroupMetadata {
    fn from(record: SystemGroupRecord) -> Self {
        Self {
            id: record.id,
            name: record.name,
            created_at: record.created_at,
            last_modified: record.last_modified,
            members: record.members,
            roles: record.roles,
            grants: record.grants,
            options: record.options,
            metadata: record.metadata,
        }
    }
}

/// Group metadata - collection of users with shared permissions
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SystemGroupRecord {
    /// Unique identifier for the Group
    pub id: SystemGroupId,
    /// Name of the Group
    pub name: String,
    /// Version of the Group Record
    pub version: u64,
    /// Timestamp when the group was created
    pub created_at: Timestamp,
    /// Timestamp when the group was last modified
    pub last_modified: Timestamp,
    /// List of users in this group
    pub members: Vec<UserId>,
    /// List of roles assigned to this group
    pub roles: Vec<SystemRoleId>,
    /// Direct permission grants for the group
    pub grants: Vec<PermissionGrant>,
    /// Configuration Options for the Group
    pub options: HashMap<String, String>,
    /// Group Metadata (Informative)
    pub metadata: HashMap<String, String>,
}
