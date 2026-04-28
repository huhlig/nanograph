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

use crate::object::security::SubjectId;
use crate::object::{PermissionGrant, TenantId};
use crate::types::{PropertyUpdate, Timestamp};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Tenant Role identifier
#[derive(
    Copy, Clone, Debug, Default, Ord, PartialOrd, Eq, PartialEq, Hash, Serialize, Deserialize,
)]
pub struct TenantRoleId(pub SubjectId);

impl TenantRoleId {
    pub fn new(id: SubjectId) -> Self {
        Self(id)
    }

    pub fn subject(&self) -> SubjectId {
        self.0
    }
}

impl From<u32> for TenantRoleId {
    fn from(id: u32) -> Self {
        Self(SubjectId::new(id))
    }
}

impl std::fmt::Display for TenantRoleId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "TenantRole({})", self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tenant_role_id() {
        let id = TenantRoleId::new(SubjectId::new(0x12345678));
        assert_eq!(id.subject().as_u32(), 0x12345678);
        assert_eq!(TenantRoleId::from(0x12345678), id);
        assert_eq!(format!("{}", id), "TenantRole(12345678)");
    }

    #[test]
    fn test_system_role_id() {
        let id = SystemRoleId::new(SubjectId::new(0xABCDEF01));
        assert_eq!(id.subject().as_u32(), 0xABCDEF01);
        assert_eq!(SystemRoleId::from(0xABCDEF01), id);
        assert_eq!(format!("{}", id), "SystemRole(ABCDEF01)");
    }
}

/// Configuration for Tenant User creation
#[derive(Clone, Debug)]
pub struct TenantRoleCreate {
    /// Configuration Options for the Cluster
    pub options: HashMap<String, String>,
    /// Cluster Metadata (Informative)
    pub metadata: HashMap<String, String>,
}

impl TenantRoleCreate {
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
pub struct TenantRoleUpdate {
    /// Configuration Options for the Cluster
    pub options: Vec<PropertyUpdate>,
    /// Cluster Metadata (Informative)
    pub metadata: Vec<PropertyUpdate>,
}

impl TenantRoleUpdate {
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

/// TenantRole metadata - named collection of permissions
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TenantRoleMetadata {
    /// Unique identifier for the TenantRole
    pub id: TenantRoleId,
    /// Name of the TenantRole
    pub name: String,
    /// Timestamp when the TenantRole was created
    pub created_at: Timestamp,
    /// Timestamp when the TenantRole was last modified
    pub last_modified: Timestamp,
    /// List of permission grants for this TenantRole
    pub grants: Vec<PermissionGrant>,
    /// Configuration Options for the TenantRole
    pub options: HashMap<String, String>,
    /// TenantRole Metadata (Informative)
    pub metadata: HashMap<String, String>,
}

impl From<TenantRoleRecord> for TenantRoleMetadata {
    fn from(record: TenantRoleRecord) -> Self {
        Self {
            id: record.id,
            name: record.name,
            created_at: record.created_at,
            last_modified: record.last_modified,
            grants: record.grants,
            options: record.options,
            metadata: record.metadata,
        }
    }
}

/// TenantRole metadata - named collection of permissions
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TenantRoleRecord {
    /// Unique identifier for the TenantRole
    pub id: TenantRoleId,
    /// Tenant Identifier for the TenantRole
    pub tenant: TenantId,
    /// Name of the TenantRole
    pub name: String,
    /// Version of the TenantRole Record
    pub version: u64,
    /// Timestamp when the TenantRole was created
    pub created_at: Timestamp,
    /// Timestamp when the TenantRole was last modified
    pub last_modified: Timestamp,
    /// List of permission grants for this TenantRole
    pub grants: Vec<PermissionGrant>,
    /// Configuration Options for the TenantRole
    pub options: HashMap<String, String>,
    /// TenantRole Metadata (Informative)
    pub metadata: HashMap<String, String>,
}

/// Tenant Role identifier
#[derive(
    Copy, Clone, Debug, Default, Ord, PartialOrd, Eq, PartialEq, Hash, Serialize, Deserialize,
)]
pub struct SystemRoleId(pub SubjectId);

impl SystemRoleId {
    pub fn new(id: SubjectId) -> Self {
        Self(id)
    }

    pub fn subject(&self) -> SubjectId {
        self.0
    }
}

impl From<u32> for SystemRoleId {
    fn from(id: u32) -> Self {
        Self(SubjectId::new(id))
    }
}

impl std::fmt::Display for SystemRoleId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "SystemRole({})", self.0)
    }
}

/// Configuration for Tenant User creation
#[derive(Clone, Debug)]
pub struct SystemRoleCreate {
    /// Configuration Options for the Cluster
    pub options: HashMap<String, String>,
    /// Cluster Metadata (Informative)
    pub metadata: HashMap<String, String>,
}

impl SystemRoleCreate {
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
pub struct SystemRoleUpdate {
    /// Configuration Options for the Cluster
    pub options: Vec<PropertyUpdate>,
    /// Cluster Metadata (Informative)
    pub metadata: Vec<PropertyUpdate>,
}

impl SystemRoleUpdate {
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

/// SystemRole metadata - named collection of permissions
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SystemRoleMetadata {
    /// Unique identifier for the SystemRole
    pub id: SystemRoleId,
    /// Name of the SystemRole
    pub name: String,
    /// Timestamp when the SystemRole was created
    pub created_at: Timestamp,
    /// Timestamp when the SystemRole was last modified
    pub last_modified: Timestamp,
    /// List of permission grants for this SystemRole
    pub grants: Vec<PermissionGrant>,
    /// Configuration Options for the SystemRole
    pub options: HashMap<String, String>,
    /// SystemRole Metadata (Informative)
    pub metadata: HashMap<String, String>,
}

impl From<SystemRoleRecord> for SystemRoleMetadata {
    fn from(record: SystemRoleRecord) -> Self {
        Self {
            id: record.id,
            name: record.name,
            created_at: record.created_at,
            last_modified: record.last_modified,
            grants: record.grants,
            options: record.options,
            metadata: record.metadata,
        }
    }
}

/// SystemRole metadata - named collection of permissions
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SystemRoleRecord {
    /// Unique identifier for the SystemRole
    pub id: SystemRoleId,
    /// Name of the SystemRole
    pub name: String,
    /// Version of the SystemRole Record
    pub version: u64,
    /// Timestamp when the SystemRole was created
    pub created_at: Timestamp,
    /// Timestamp when the SystemRole was last modified
    pub last_modified: Timestamp,
    /// List of permission grants for this SystemRole
    pub grants: Vec<PermissionGrant>,
    /// Configuration Options for the SystemRole
    pub options: HashMap<String, String>,
    /// SystemRole Metadata (Informative)
    pub metadata: HashMap<String, String>,
}
