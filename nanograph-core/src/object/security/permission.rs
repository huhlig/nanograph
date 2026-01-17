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

use crate::object::{DatabaseId, FunctionId, NamespaceId, TableId, TablespaceId, TenantId};
use serde::{Deserialize, Serialize};

/// Permission grant - combines a permission with a resource scope
///
/// This structure allows fine-grained access control by specifying exactly
/// which resources a permission applies to. For example:
/// - TableRead on Table(123) - can read only table 123
/// - TableRead on AllInDatabase(456) - can read all tables in database 456
/// - TableRead on AllTables - can read all tables system-wide
///
/// # TODO
/// - Review granularity of permissions to ensure properly configurable scopes
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

    /// Check if this grant allows a specific permission on a tablespace
    pub fn allows_system(&self, permission: &Permission) -> bool {
        self.permission.implies(permission) && self.scope.matches_system()
    }

    /// Check if this grant allows a specific permission on a tablespace
    pub fn allows_tablespace(&self, permission: &Permission, tablespace_id: &TablespaceId) -> bool {
        self.permission.implies(permission) && self.scope.matches_tablespace(tablespace_id)
    }

    /// Check if this grant allows a specific permission on a tenant
    pub fn allows_tenant(&self, permission: &Permission, tenant_id: &TenantId) -> bool {
        self.permission.implies(permission) && self.scope.matches_tenant(tenant_id)
    }

    /// Check if this grant allows a specific permission on a database
    pub fn allows_database(
        &self,
        permission: &Permission,
        tenant_id: &TenantId,
        database_id: &DatabaseId,
    ) -> bool {
        self.permission.implies(permission) && self.scope.matches_database(tenant_id, database_id)
    }

    /// Check if this grant allows a specific permission on a table
    pub fn allows_table(
        &self,
        permission: &Permission,
        tenant_id: &TenantId,
        database_id: &DatabaseId,
        table_id: &TableId,
    ) -> bool {
        self.permission.implies(permission)
            && self.scope.matches_table(tenant_id, database_id, table_id)
    }

    /// Check if this grant allows a specific permission on a namespace
    pub fn allows_namespace(
        &self,
        permission: &Permission,
        tenant_id: &TenantId,
        database_id: &DatabaseId,
        namespace_id: &NamespaceId,
    ) -> bool {
        self.permission.implies(permission)
            && self
                .scope
                .matches_namespace(tenant_id, database_id, namespace_id)
    }

    /// Check if this grant allows a specific permission on a function
    pub fn allows_function(
        &self,
        permission: &Permission,
        tenant_id: &TenantId,
        database_id: &DatabaseId,
        function_id: &FunctionId,
    ) -> bool {
        self.permission.implies(permission)
            && self
                .scope
                .matches_function(tenant_id, database_id, function_id)
    }
}

impl std::fmt::Display for PermissionGrant {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} on {}", self.permission, self.scope)
    }
}

/// Permission types for fine-grained access control
#[derive(Clone, Debug, Eq, PartialEq, Hash, Serialize, Deserialize)]
pub enum Permission {
    // Cluster Operations
    /// Manage cluster configuration (Including Regions and Servers)
    ClusterAlter,
    /// View cluster status (Including Regions and Servers)
    ClusterView,

    // Metrics Operations
    /// View system metrics
    MetricsView,

    // Security Operations
    /// Manage all users globally
    UserManagement,

    // Configuration Operations
    /// Alter Configuration
    ConfigAlter,
    /// View configuration
    ConfigView,

    // Tablespace Operations
    /// Create, Update, & Delete Tablespace Configurations
    TablespaceManagement,
    /// Assign Tablespaces to Tenants, Databases, Tables, and Shards
    TablespaceAssign,
    /// View Tablespace Information
    TablespaceView,
    /// View List of Tablespaces
    TablespaceList,

    // Tenant Operations
    /// Create a new tenant
    TenantCreate,
    /// Delete a tenant
    TenantDelete,
    /// Alter tenants
    TenantAlter,
    /// View System Tenants
    TenantView,
    /// List System Tenants
    TenantList,

    // Database Operations
    /// User has access to database objects
    DatabaseAccess,
    /// Create databases in tenant
    DatabaseCreate,
    /// Delete databases in tenant
    DatabaseDelete,
    /// Update database configuration
    DatabaseAlter,
    /// View databases in tenant
    DatabaseView,
    /// List databases in tenant
    DatabaseList,

    // Namespace Operations
    /// Create Namespaces
    NamespaceCreate,
    /// Alter Namespaces
    NamespaceAlter,
    /// Delete Namespaces
    NamespaceDelete,
    /// View Namespace Information
    NamespaceView,
    /// List Objects in Namespace
    NamespaceList,

    // Table Operations
    /// Create new Tables
    TableCreate,
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

    // Function Operations
    /// Execute functions
    FunctionExecute,
    /// Create functions
    FunctionCreate,
    /// Delete functions
    FunctionDelete,
    /// Manage function configuration
    FunctionAlter,

    // Special permissions
    /// Grant permissions to other users
    GrantPermission,
    /// Revoke permissions from other users
    RevokePermission,
    /// Tenant Superuser - all permissions within tenant
    TenantSuperuser,
    /// Global Superuser - all permissions within all tenants
    GlobalSuperuser,
}

impl Permission {
    /// Check if this permission implies another permission
    pub fn implies(&self, other: &Permission) -> bool {
        match self {
            Permission::GlobalSuperuser => true, // Superuser has all permissions
            _ => self == other,
        }
    }
}

impl std::fmt::Display for Permission {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Debug::fmt(self, f)
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

    /// Specific Tablespace
    Tablespace(TablespaceId),
    /// All Tablespaces (wildcard)
    AllTablespaces,

    /// Specific tenant
    Tenant(TenantId),
    /// All tenants (wildcard)
    AllTenants,

    /// Specific database
    Database(DatabaseId),
    /// All databases (wildcard)
    AllDatabases,

    /// Specific namespace
    Namespace(NamespaceId),
    /// All namespaces (wildcard)
    AllNamespaces,

    /// Specific table
    Table(TableId),
    /// All tables (wildcard)
    AllTables,

    /// Specific function
    Function(FunctionId),
    /// All functions (wildcard)
    AllFunctions,
}

impl ResourceScope {
    /// Check if this scope matches the system scope
    pub fn matches_system(&self) -> bool {
        match self {
            ResourceScope::System => true,
            _ => false,
        }
    }
    /// Check if this scope matches a specific tablespace
    pub fn matches_tablespace(&self, tablespace_id: &TablespaceId) -> bool {
        match self {
            ResourceScope::System => true,
            ResourceScope::AllTablespaces => true,
            ResourceScope::Tablespace(id) => *id == *tablespace_id,
            _ => false,
        }
    }

    /// Check if this scope matches a specific tenant
    pub fn matches_tenant(&self, tenant_id: &TenantId) -> bool {
        match self {
            ResourceScope::System => true,
            ResourceScope::AllTenants => true,
            ResourceScope::Tenant(id) => *id == *tenant_id,
            _ => false,
        }
    }

    /// Check if this scope matches a specific database
    pub fn matches_database(&self, tenant_id: &TenantId, database_id: &DatabaseId) -> bool {
        match self {
            ResourceScope::System => true,
            ResourceScope::AllDatabases => true,
            ResourceScope::AllTenants => true,
            ResourceScope::Tenant(tid) => *tid == *tenant_id,
            ResourceScope::Database(did) => *did == *database_id,
            _ => false,
        }
    }

    /// Check if this scope matches a specific table
    pub fn matches_table(
        &self,
        tenant_id: &TenantId,
        database_id: &DatabaseId,
        table_id: &TableId,
    ) -> bool {
        match self {
            ResourceScope::System => true,
            ResourceScope::AllTables => true,
            ResourceScope::AllDatabases => true,
            ResourceScope::AllTenants => true,
            ResourceScope::Tenant(tid) => *tid == *tenant_id,
            ResourceScope::Database(did) => *did == *database_id,
            ResourceScope::Table(tid) => *tid == *table_id,
            _ => false,
        }
    }

    /// Check if this scope matches a specific namespace
    pub fn matches_namespace(
        &self,
        tenant_id: &TenantId,
        database_id: &DatabaseId,
        namespace_id: &NamespaceId,
    ) -> bool {
        match self {
            ResourceScope::System => true,
            ResourceScope::AllNamespaces => true,
            ResourceScope::AllDatabases => true,
            ResourceScope::AllTenants => true,
            ResourceScope::Tenant(tid) => *tid == *tenant_id,
            ResourceScope::Database(did) => *did == *database_id,
            ResourceScope::Namespace(nid) => *nid == *namespace_id,
            _ => false,
        }
    }

    /// Check if this scope matches a specific function
    pub fn matches_function(
        &self,
        tenant_id: &TenantId,
        database_id: &DatabaseId,
        function_id: &FunctionId,
    ) -> bool {
        match self {
            ResourceScope::System => true,
            ResourceScope::AllFunctions => true,
            ResourceScope::AllDatabases => true,
            ResourceScope::AllTenants => true,
            ResourceScope::Tenant(tid) => *tid == *tenant_id,
            ResourceScope::Database(did) => *did == *database_id,
            ResourceScope::Function(fid) => *fid == *function_id,
            _ => false,
        }
    }
}

impl std::fmt::Display for ResourceScope {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Debug::fmt(self, f)
    }
}
