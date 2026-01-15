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

impl std::fmt::Display for PermissionGrant {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} on {}", self.permission, self.scope)
    }
}

/// Permission types for fine-grained access control
#[derive(Clone, Debug, Eq, PartialEq, Hash, Serialize, Deserialize)]
pub enum Permission {
    // Cluster Operations
    /// Manage cluster configuration
    ClusterManage,
    /// View cluster status
    ClusterView,

    // Metrics Operations
    /// View system metrics
    MetricsView,

    // Security Operations
    /// Manage all users globally
    SecurityManage,

    // Configuration Operations
    /// Alter Configuration
    ConfigManage,
    /// View configuration
    ConfigView,

    // Tenant Operations
    /// Create a new tenant
    TenantCreate,
    /// Delete a tenant
    TenantDelete,
    /// Manage tenants
    TenantManage,
    /// View system tenants
    TenantView,

    // Database Operations
    /// Create databases in tenant
    DatabaseCreate,
    /// Delete databases in tenant
    DatabaseDelete,

    // Namespace-level permissions
    /// Create Namespaces
    NamespaceCreate,
    /// Delete Namespaces
    NamespaceDelete,
    /// View objects in namespace
    NamespaceObjectView,

    // Table-level permissions
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
    /// Tenant Superuser - all permissions within tenant
    TenantSuperuser,
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

    /// Specific Tablespace
    Tablespace(TablespaceId),
    /// All Tablespaces (wildcard)
    AllTablespaces,
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
            ResourceScope::Database(did) => *did == database_id,
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
            ResourceScope::Database(did) => *did == database_id,
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
            ResourceScope::Database(did) => *did == database_id,
            ResourceScope::Function(fid) => *fid == function_id,
            _ => false,
        }
    }
}

impl std::fmt::Display for ResourceScope {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Debug::fmt(self, f)
    }
}
