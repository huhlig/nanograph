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

use crate::object::{DatabaseId, FunctionId, NamespaceId, ObjectId, TableId, TenantId};
use crate::types::{PropertyUpdate, Timestamp};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// User identifier (database)
///
/// Represents a user with access to data.
/// Uses a 32-bit identifier for compactness and to avoid overflow.
#[derive(
    Copy, Clone, Debug, Default, Ord, PartialOrd, Eq, PartialEq, Hash, Serialize, Deserialize,
)]
pub struct UserId(pub ObjectId);

impl UserId {
    /// Create a new cluster identifier.
    pub fn new(id: u32) -> Self {
        Self(id)
    }

    /// Return the cluster identifier as a u32.
    pub fn as_u64(&self) -> u32 {
        self.0
    }
}

impl From<u32> for UserId {
    fn from(id: u32) -> Self {
        Self(id)
    }
}

impl std::fmt::Display for UserId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "User({})", self.0)
    }
}

/// Configuration for SuperUser creation
#[derive(Clone, Debug)]
pub struct UserCreate {
    /// Name of the User
    pub username: String,
    /// Configuration Options for the SuperUser
    pub options: HashMap<String, String>,
    /// SuperUser Metadata (Informative)
    pub metadata: HashMap<String, String>,
}

impl UserCreate {
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
pub struct UserUpdate {
    /// Name of the SuperUser
    pub username: Option<String>,
    /// Configuration Options for the SuperUser
    pub options: Vec<PropertyUpdate>,
    /// SuperUser Metadata (Informative)
    pub metadata: Vec<PropertyUpdate>,
}

impl UserUpdate {
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
pub struct UserMetadata {
    /// Unique identifier for the User
    pub id: UserId,
    /// Username
    pub username: String,
    /// Version of the User Record
    pub version: u64,
    /// Timestamp when the user was created
    pub created_at: Timestamp,
    /// Timestamp when the user was last modified
    pub last_modified: Timestamp,
    /// Groups this user belongs to
    pub groups: Vec<GroupId>,
    /// Roles assigned directly to this user
    pub roles: Vec<RoleId>,
    /// Direct permission grants for the user (in addition to group/role permissions)
    pub grants: Vec<PermissionGrant>,
    /// Whether the user account is enabled
    pub enabled: bool,
    /// Optional password hash (for authentication)
    pub password_hash: Option<String>,
    /// Optional email address
    pub email: Option<String>,
    /// Configuration Options for the User
    pub options: HashMap<String, String>,
    /// User Metadata (Informative)
    pub metadata: HashMap<String, String>,
}

impl UserMetadata {
    /// Get all effective permissions for this user (direct + groups + roles)
    pub fn effective_permissions(
        &self,
        groups: &[GroupMetadata],
        roles: &[RoleMetadata],
    ) -> Vec<Permission> {
        let mut perms = self
            .grants
            .iter()
            .map(|g| g.permission.clone())
            .collect::<Vec<_>>();

        // Add permissions from groups
        for group_id in &self.groups {
            if let Some(group) = groups.iter().find(|g| g.id == *group_id) {
                perms.extend(group.grants.iter().map(|g| g.permission.clone()));

                // Add permissions from group's roles
                for role_id in &group.roles {
                    if let Some(role) = roles.iter().find(|r| r.id == *role_id) {
                        perms.extend(role.grants.iter().map(|g| g.permission.clone()));
                    }
                }
            }
        }

        // Add permissions from direct roles
        for role_id in &self.roles {
            if let Some(role) = roles.iter().find(|r| r.id == *role_id) {
                perms.extend(role.grants.iter().map(|g| g.permission.clone()));
            }
        }

        // Deduplicate permissions
        perms.sort_by_key(|p| format!("{:?}", p));
        perms.dedup();
        perms
    }

    /// Check if user has a specific permission
    pub fn has_permission(
        &self,
        permission: &Permission,
        groups: &[GroupMetadata],
        roles: &[RoleMetadata],
    ) -> bool {
        let effective = self.effective_permissions(groups, roles);
        effective.iter().any(|p| p.implies(permission))
    }

    /// Check if user has any system-level permissions
    pub fn has_system_permissions(&self, groups: &[GroupMetadata], roles: &[RoleMetadata]) -> bool {
        let effective = self.effective_permissions(groups, roles);
        effective.iter().any(|p| p.is_system_permission())
    }

    /// Check if user has any data-level permissions
    pub fn has_data_permissions(&self, groups: &[GroupMetadata], roles: &[RoleMetadata]) -> bool {
        let effective = self.effective_permissions(groups, roles);
        effective.iter().any(|p| p.is_data_permission())
    }
}

/// Group identifier
#[derive(
    Copy, Clone, Debug, Default, Ord, PartialOrd, Eq, PartialEq, Hash, Serialize, Deserialize,
)]
pub struct GroupId(pub ObjectId);

impl GroupId {
    pub fn new(id: u32) -> Self {
        Self(id)
    }

    pub fn as_u32(&self) -> u32 {
        self.0
    }
}

impl From<u32> for GroupId {
    fn from(id: u32) -> Self {
        Self(id)
    }
}

impl std::fmt::Display for GroupId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Group({})", self.0)
    }
}

/// Role identifier
#[derive(
    Copy, Clone, Debug, Default, Ord, PartialOrd, Eq, PartialEq, Hash, Serialize, Deserialize,
)]
pub struct RoleId(pub ObjectId);

impl RoleId {
    pub fn new(id: u32) -> Self {
        Self(id)
    }

    pub fn as_u64(&self) -> u32 {
        self.0
    }
}

impl From<u32> for RoleId {
    fn from(id: u32) -> Self {
        Self(id)
    }
}

impl std::fmt::Display for RoleId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Role({})", self.0)
    }
}

/// Group metadata - collection of users with shared permissions
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GroupMetadata {
    /// Unique identifier for the Group
    pub id: GroupId,
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
    pub roles: Vec<RoleId>,
    /// Direct permission grants for the group
    pub grants: Vec<PermissionGrant>,
    /// Configuration Options for the Group
    pub options: HashMap<String, String>,
    /// Group Metadata (Informative)
    pub metadata: HashMap<String, String>,
}

/// Role metadata - named collection of permissions
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RoleMetadata {
    /// Unique identifier for the Role
    pub id: RoleId,
    /// Name of the Role
    pub name: String,
    /// Version of the Role Record
    pub version: u64,
    /// Timestamp when the role was created
    pub created_at: Timestamp,
    /// Timestamp when the role was last modified
    pub last_modified: Timestamp,
    /// List of permission grants for this role
    pub grants: Vec<PermissionGrant>,
    /// Configuration Options for the Role
    pub options: HashMap<String, String>,
    /// Role Metadata (Informative)
    pub metadata: HashMap<String, String>,
}

/// Permission types for fine-grained access control
#[derive(Clone, Debug, Eq, PartialEq, Hash, Serialize, Deserialize)]
pub enum Permission {
    // System-level permissions
    /// Manage cluster configuration
    SystemClusterManage,
    /// View cluster status
    SystemClusterView,
    /// Manage regions
    SystemRegionManage,
    /// Manage servers/nodes
    SystemServerManage,
    /// Manage users
    SystemUserManage,
    /// Manage groups
    SystemGroupManage,
    /// Manage roles
    SystemRoleManage,
    /// Manage tenants
    SystemTenantManage,
    /// View system metrics
    SystemMetricsView,
    /// Manage system configuration
    SystemConfigManage,

    // Tenant-level permissions
    /// Create databases in tenant
    TenantDatabaseCreate,
    /// Delete databases in tenant
    TenantDatabaseDelete,
    /// Manage tenant configuration
    TenantConfigManage,
    /// View tenant information
    TenantView,

    // Database-level permissions
    /// Create namespaces in database
    DatabaseNamespaceCreate,
    /// Delete namespaces in database
    DatabaseNamespaceDelete,
    /// Create tables in database
    DatabaseTableCreate,
    /// Delete tables in database
    DatabaseTableDelete,
    /// Manage database configuration
    DatabaseConfigManage,
    /// View database schema
    DatabaseSchemaView,

    // Table-level permissions
    /// Read data from table
    TableRead,
    /// Write data to table
    TableWrite,
    /// Delete data from table
    TableDelete,
    /// Alter table structure
    TableAlter,
    /// Drop table
    TableDrop,
    /// Create indexes on table
    TableIndexCreate,
    /// Drop indexes from table
    TableIndexDrop,

    // Namespace-level permissions
    /// Create objects in namespace
    NamespaceObjectCreate,
    /// Delete objects from namespace
    NamespaceObjectDelete,
    /// Manage namespace configuration
    NamespaceConfigManage,

    // Function-level permissions
    /// Execute functions
    FunctionExecute,
    /// Create functions
    FunctionCreate,
    /// Delete functions
    FunctionDelete,
    /// Manage function configuration
    FunctionManage,

    // Special permissions
    /// Grant permissions to other users
    GrantPermission,
    /// Revoke permissions from other users
    RevokePermission,
    /// Superuser - all permissions
    Superuser,
}

impl Permission {
    /// Check if this permission implies another permission
    pub fn implies(&self, other: &Permission) -> bool {
        match self {
            Permission::Superuser => true, // Superuser has all permissions
            _ => self == other,
        }
    }

    /// Check if this is a system-level permission
    pub fn is_system_permission(&self) -> bool {
        matches!(
            self,
            Permission::SystemClusterManage
                | Permission::SystemClusterView
                | Permission::SystemRegionManage
                | Permission::SystemServerManage
                | Permission::SystemUserManage
                | Permission::SystemGroupManage
                | Permission::SystemRoleManage
                | Permission::SystemTenantManage
                | Permission::SystemMetricsView
                | Permission::SystemConfigManage
        )
    }

    /// Check if this is a data-level permission
    pub fn is_data_permission(&self) -> bool {
        matches!(
            self,
            Permission::TableRead
                | Permission::TableWrite
                | Permission::TableDelete
                | Permission::TableAlter
                | Permission::TableDrop
                | Permission::TableIndexCreate
                | Permission::TableIndexDrop
                | Permission::FunctionExecute
        )
    }
}

/// Resource scope for permission grants
///
/// Defines the scope at which a permission applies. Supports both specific
/// resources and wildcard grants for all resources within a scope.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ResourceScope {
    /// System-wide scope (for system administration)
    System,

    /// Specific tenant
    Tenant(TenantId),
    /// All tenants (wildcard)
    AllTenants,

    /// Specific database
    Database(DatabaseId),
    /// All databases in a specific tenant
    AllInTenant(TenantId),
    /// All databases (wildcard)
    AllDatabases,

    /// Specific table
    Table(TableId),
    /// All tables in a specific database
    AllInDatabase(DatabaseId),
    /// All tables (wildcard)
    AllTables,

    /// Specific namespace
    Namespace(NamespaceId),
    /// All namespaces in a specific database
    AllNamespacesInDatabase(DatabaseId),
    /// All namespaces (wildcard)
    AllNamespaces,

    /// Specific function
    Function(FunctionId),
    /// All functions in a specific database
    AllFunctionsInDatabase(DatabaseId),
    /// All functions (wildcard)
    AllFunctions,
}

impl ResourceScope {
    /// Check if this scope matches a specific tenant
    pub fn matches_tenant(&self, tenant_id: TenantId) -> bool {
        match self {
            ResourceScope::System => true,
            ResourceScope::AllTenants => true,
            ResourceScope::Tenant(id) => *id == tenant_id,
            _ => false,
        }
    }

    /// Check if this scope matches a specific database
    pub fn matches_database(&self, database_id: DatabaseId, tenant_id: TenantId) -> bool {
        match self {
            ResourceScope::System => true,
            ResourceScope::AllDatabases => true,
            ResourceScope::AllTenants => true,
            ResourceScope::Tenant(tid) => *tid == tenant_id,
            ResourceScope::AllInTenant(tid) => *tid == tenant_id,
            ResourceScope::Database(did) => *did == database_id,
            _ => false,
        }
    }

    /// Check if this scope matches a specific table
    pub fn matches_table(
        &self,
        table_id: TableId,
        database_id: DatabaseId,
        tenant_id: TenantId,
    ) -> bool {
        match self {
            ResourceScope::System => true,
            ResourceScope::AllTables => true,
            ResourceScope::AllDatabases => true,
            ResourceScope::AllTenants => true,
            ResourceScope::Tenant(tid) => *tid == tenant_id,
            ResourceScope::AllInTenant(tid) => *tid == tenant_id,
            ResourceScope::Database(did) => *did == database_id,
            ResourceScope::AllInDatabase(did) => *did == database_id,
            ResourceScope::Table(tid) => *tid == table_id,
            _ => false,
        }
    }

    /// Check if this scope matches a specific namespace
    pub fn matches_namespace(
        &self,
        namespace_id: NamespaceId,
        database_id: DatabaseId,
        tenant_id: TenantId,
    ) -> bool {
        match self {
            ResourceScope::System => true,
            ResourceScope::AllNamespaces => true,
            ResourceScope::AllDatabases => true,
            ResourceScope::AllTenants => true,
            ResourceScope::Tenant(tid) => *tid == tenant_id,
            ResourceScope::AllInTenant(tid) => *tid == tenant_id,
            ResourceScope::Database(did) => *did == database_id,
            ResourceScope::AllNamespacesInDatabase(did) => *did == database_id,
            ResourceScope::Namespace(nid) => *nid == namespace_id,
            _ => false,
        }
    }

    /// Check if this scope matches a specific function
    pub fn matches_function(
        &self,
        function_id: FunctionId,
        database_id: DatabaseId,
        tenant_id: TenantId,
    ) -> bool {
        match self {
            ResourceScope::System => true,
            ResourceScope::AllFunctions => true,
            ResourceScope::AllDatabases => true,
            ResourceScope::AllTenants => true,
            ResourceScope::Tenant(tid) => *tid == tenant_id,
            ResourceScope::AllInTenant(tid) => *tid == tenant_id,
            ResourceScope::Database(did) => *did == database_id,
            ResourceScope::AllFunctionsInDatabase(did) => *did == database_id,
            ResourceScope::Function(fid) => *fid == function_id,
            _ => false,
        }
    }
}

/// Permission grant - combines a permission with a resource scope
///
/// This structure allows fine-grained access control by specifying exactly
/// which resources a permission applies to. For example:
/// - TableRead on Table(123) - can read only table 123
/// - TableRead on AllInDatabase(456) - can read all tables in database 456
/// - TableRead on AllTables - can read all tables system-wide
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PermissionGrant {
    /// The permission being granted
    pub permission: Permission,
    /// The scope at which this permission applies
    pub scope: ResourceScope,
}

impl PermissionGrant {
    /// Create a new permission grant
    pub fn new(permission: Permission, scope: ResourceScope) -> Self {
        Self { permission, scope }
    }

    /// Check if this grant allows a specific permission on a tenant
    pub fn allows_tenant(&self, permission: &Permission, tenant_id: TenantId) -> bool {
        self.permission.implies(permission) && self.scope.matches_tenant(tenant_id)
    }

    /// Check if this grant allows a specific permission on a database
    pub fn allows_database(
        &self,
        permission: &Permission,
        database_id: DatabaseId,
        tenant_id: TenantId,
    ) -> bool {
        self.permission.implies(permission) && self.scope.matches_database(database_id, tenant_id)
    }

    /// Check if this grant allows a specific permission on a table
    pub fn allows_table(
        &self,
        permission: &Permission,
        table_id: TableId,
        database_id: DatabaseId,
        tenant_id: TenantId,
    ) -> bool {
        self.permission.implies(permission)
            && self.scope.matches_table(table_id, database_id, tenant_id)
    }

    /// Check if this grant allows a specific permission on a namespace
    pub fn allows_namespace(
        &self,
        permission: &Permission,
        namespace_id: NamespaceId,
        database_id: DatabaseId,
        tenant_id: TenantId,
    ) -> bool {
        self.permission.implies(permission)
            && self
                .scope
                .matches_namespace(namespace_id, database_id, tenant_id)
    }

    /// Check if this grant allows a specific permission on a function
    pub fn allows_function(
        &self,
        permission: &Permission,
        function_id: FunctionId,
        database_id: DatabaseId,
        tenant_id: TenantId,
    ) -> bool {
        self.permission.implies(permission)
            && self
                .scope
                .matches_function(function_id, database_id, tenant_id)
    }
}
