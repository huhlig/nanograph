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

use crate::object::security::{SystemGroupId, SystemRoleId, TenantGroupId, TenantRoleId};
use crate::object::{PermissionGrant, TenantId, UserId};
use crate::types::{PropertyUpdate, Timestamp};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Configuration for SuperUser creation
#[derive(Clone, Debug)]
pub struct SystemUserCreate {
    /// Name of the User
    pub username: String,
    /// Configuration Options for the SuperUser
    pub options: HashMap<String, String>,
    /// SuperUser Metadata (Informative)
    pub metadata: HashMap<String, String>,
}

impl SystemUserCreate {
    /// Create a new SuperUser creation configuration.
    ///
    /// # Arguments
    ///
    /// * `name`: The name of the new SuperUser.
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            username: name.into(),
            options: HashMap::new(),
            metadata: HashMap::new(),
        }
    }
    /// Add or update a configuration option for the SuperUser.
    ///
    /// # Arguments
    ///
    /// * `key`: The key of the option to set.
    /// * `value`: The value to assign to the option.
    pub fn add_option(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.options.insert(key.into(), value.into());
        self
    }
    /// Clear a configuration option from the SuperUser.
    ///
    /// # Arguments
    ///
    /// * `key`: The key of the option to clear.
    pub fn clear_option(mut self, key: impl Into<String>) -> Self {
        self.options.remove(&key.into());
        self
    }
    /// Add or update informative metadata for the SuperUser.
    ///
    /// # Arguments
    ///
    /// * `key`: The key of the metadata entry to set.
    /// * `value`: The value to assign to the metadata entry.
    pub fn add_metadata(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.metadata.insert(key.into(), value.into());
        self
    }
    /// Clear informative metadata from the SuperUser.
    ///
    /// # Arguments
    ///
    /// * `key`: The key of the metadata entry to clear.
    pub fn clear_metadata(mut self, key: impl Into<String>) -> Self {
        self.metadata.remove(&key.into());
        self
    }
}

/// Configuration for SuperUser update
#[derive(Clone, Debug, Default)]
pub struct SystemUserUpdate {
    /// Name of the SuperUser
    pub username: Option<String>,
    /// Configuration Options for the SuperUser
    pub options: Vec<PropertyUpdate>,
    /// SuperUser Metadata (Informative)
    pub metadata: Vec<PropertyUpdate>,
}

impl SystemUserUpdate {
    /// Set the name of the SuperUser.
    ///
    /// # Arguments
    ///
    /// * `name`: The new name for the SuperUser.
    pub fn set_name(mut self, name: String) -> Self {
        self.username = Some(name);
        self
    }
    /// Add or update a configuration option for the SuperUser.
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
    /// Clear a configuration option from the SuperUser.
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
    /// Add or update informative metadata for the SuperUser.
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
    /// Clear informative metadata from the SuperUser.
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

/// User metadata with comprehensive access control
///
/// Users are granted permissions through:
/// 1. Direct permissions assigned to the user
/// 2. Permissions inherited from groups they belong to
/// 3. Permissions inherited from roles assigned to them or their groups
///
/// This flexible model allows for fine-grained access control without rigid user types.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SystemUserMetadata {
    /// Unique identifier for the User
    pub id: UserId,
    /// Username
    pub username: String,
    /// Timestamp when the user was created
    pub created_at: Timestamp,
    /// Timestamp when the user was last modified
    pub last_modified: Timestamp,
    /// Groups this user belongs to
    pub groups: Vec<SystemGroupId>,
    /// Roles assigned directly to this user
    pub roles: Vec<SystemRoleId>,
    /// Direct permission grants for the user (in addition to group/role permissions)
    pub grants: Vec<PermissionGrant>,
    /// Whether the user account is enabled
    pub enabled: bool,
    /// Configuration Options for the User
    pub options: HashMap<String, String>,
    /// User Metadata (Informative)
    pub metadata: HashMap<String, String>,
}

impl From<SystemUserRecord> for SystemUserMetadata {
    fn from(record: SystemUserRecord) -> Self {
        Self {
            id: record.user_id,
            username: record.username,
            created_at: record.created_at,
            last_modified: record.last_modified,
            groups: record.group_ids,
            roles: record.role_ids,
            grants: record.grants,
            enabled: record.enabled,
            options: record.options,
            metadata: record.metadata,
        }
    }
}

/// User metadata with comprehensive access control
///
/// Users are granted permissions through:
/// 1. Direct permissions assigned to the user
/// 2. Permissions inherited from groups they belong to
/// 3. Permissions inherited from roles assigned to them or their groups
///
/// This flexible model allows for fine-grained access control without rigid user types.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SystemUserRecord {
    /// Unique identifier for the User
    pub user_id: UserId,
    /// Username
    pub username: String,
    /// Version of the User Record
    pub version: u64,
    /// Timestamp when the user was created
    pub created_at: Timestamp,
    /// Timestamp when the user was last modified
    pub last_modified: Timestamp,
    /// Groups this user belongs to
    pub group_ids: Vec<SystemGroupId>,
    /// Roles assigned directly to this user
    pub role_ids: Vec<SystemRoleId>,
    /// Direct permission grants for the user (in addition to group/role permissions)
    pub grants: Vec<PermissionGrant>,
    /// Whether the user account is enabled
    pub enabled: bool,
    /// Optional password hash (for authentication)
    pub password_hash: Option<String>,
    /// Configuration Options for the User
    pub options: HashMap<String, String>,
    /// User Metadata (Informative)
    pub metadata: HashMap<String, String>,
}

/// Configuration for Tenant User creation
#[derive(Clone, Debug)]
pub struct TenantUserCreate {
    /// Username for the Tenant User.
    pub username: String,
    /// Configuration Options for the Cluster
    pub options: HashMap<String, String>,
    /// Cluster Metadata (Informative)
    pub metadata: HashMap<String, String>,
}

impl TenantUserCreate {
    /// Create a new Tenant User creation configuration.
    pub fn new(username: String) -> Self {
        Self {
            username,
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
pub struct TenantUserUpdate {
    /// Configuration Options for the Cluster
    pub options: Vec<PropertyUpdate>,
    /// Cluster Metadata (Informative)
    pub metadata: Vec<PropertyUpdate>,
}

impl TenantUserUpdate {
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

/// A Tenants User Metadata.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TenantUserMetadata {
    /// Unique identifier for the User
    pub user: UserId,
    /// Username
    pub username: String,
    /// Timestamp when the user was added to the tenant
    pub created_at: Timestamp,
    /// Timestamp when the users tenant data was last modified
    pub last_modified: Timestamp,
    /// Groups this user belongs to
    pub groups: Vec<TenantGroupId>,
    /// Roles assigned directly to this user
    pub roles: Vec<TenantRoleId>,
    /// Whether the user account is enabled
    pub enabled: bool,
    /// Configuration Options for the User
    pub options: HashMap<String, String>,
    /// User Metadata (Informative)
    pub metadata: HashMap<String, String>,
}

impl From<(SystemUserRecord, TenantUserRecord)> for TenantUserMetadata {
    fn from((user, tenant_user): (SystemUserRecord, TenantUserRecord)) -> Self {
        Self {
            user: user.user_id,
            username: user.username,
            created_at: tenant_user.created_at,
            last_modified: tenant_user.last_modified,
            groups: tenant_user.group_ids,
            roles: tenant_user.role_ids,
            enabled: user.enabled,
            options: tenant_user.options,
            metadata: tenant_user.metadata,
        }
    }
}

/// A Tenants User Metadata.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TenantUserRecord {
    /// Unique identifier for the User
    pub user_id: UserId,
    /// Unique identifier for the Tenant
    pub tenant_id: TenantId,
    /// Version of the Tenant Record
    pub version: u64,
    /// Timestamp when the schema was created
    pub created_at: Timestamp,
    /// Timestamp when the schema was last modified
    pub last_modified: Timestamp,
    /// Tenant Groups this user belongs to
    pub group_ids: Vec<TenantGroupId>,
    /// Tenant Roles assigned directly to this user
    pub role_ids: Vec<TenantRoleId>,
    /// Configuration Options for the Cluster
    pub options: HashMap<String, String>,
    /// Cluster Metadata (Informative)
    pub metadata: HashMap<String, String>,
}
