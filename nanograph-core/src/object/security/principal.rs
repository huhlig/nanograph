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

use crate::object::{
    DatabaseId, FunctionId, NamespaceId, Permission, PermissionGrant,
    TableId, TenantId, UserId,
};
use crate::object::security::{
    SystemGroupRecord, SystemRoleRecord, SystemUserRecord,
    TenantGroupRecord, TenantRoleRecord, TenantUserRecord,
};
use crate::types::Timestamp;

/// Security Principal - represents an authenticated entity with resolved permissions
///
/// A SecurityPrincipal is created from user metadata by resolving all groups and roles
/// at authentication time. This provides:
/// 1. Better performance - permissions are resolved once, not on every check
/// 2. Cleaner separation - metadata objects are just data, principals enforce security
/// 3. Immutability - a principal's permissions don't change during a session
/// 4. Auditability - clear snapshot of what permissions were active
///
/// ## Multi-Tenant Support
///
/// The SecurityPrincipal supports both system-level and tenant-scoped permissions:
/// - System principals: Created from SystemUserRecord with system groups/roles
/// - Tenant principals: Created from SystemUserRecord + TenantUserRecord with tenant groups/roles
/// - Hybrid principals: System users can have both system and tenant permissions
#[derive(Clone, Debug)]
pub struct SecurityPrincipal {
    /// User ID this principal represents
    pub user_id: UserId,
    /// Username for logging/audit
    pub username: String,
    /// Optional tenant ID if this is a tenant-scoped principal
    pub tenant_id: Option<TenantId>,
    /// All effective permission grants (pre-resolved from user, groups, and roles)
    effective_grants: Vec<PermissionGrant>,
    /// Timestamp when this principal was created (for session tracking)
    pub created_at: Timestamp,
}

impl SecurityPrincipal {
    /// Create a system-level SecurityPrincipal from SystemUserRecord
    ///
    /// This creates a principal with system-wide permissions, typically for administrators.
    /// Resolves permissions from system groups and system roles.
    pub fn from_system_user(
        system_user: &SystemUserRecord,
        system_groups: &[SystemGroupRecord],
        system_roles: &[SystemRoleRecord],
    ) -> Self {
        let effective_grants = Self::resolve_system_grants(
            system_user,
            system_groups,
            system_roles,
        );

        Self {
            user_id: system_user.id,
            username: system_user.username.clone(),
            tenant_id: None,
            effective_grants,
            created_at: Timestamp::now(),
        }
    }

    /// Create a tenant-scoped SecurityPrincipal from SystemUserRecord and TenantUserRecord
    ///
    /// This creates a principal with tenant-specific permissions, the most common case.
    /// Resolves permissions from:
    /// 1. System user direct grants (if any system-level permissions)
    /// 2. Tenant user direct grants
    /// 3. Tenant groups and their roles
    /// 4. Direct tenant roles
    pub fn from_tenant_user(
        system_user: &SystemUserRecord,
        tenant_user: &TenantUserRecord,
        tenant_groups: &[TenantGroupRecord],
        tenant_roles: &[TenantRoleRecord],
    ) -> Self {
        let effective_grants = Self::resolve_tenant_grants(
            system_user,
            tenant_user,
            tenant_groups,
            tenant_roles,
        );

        Self {
            user_id: system_user.id,
            username: system_user.username.clone(),
            tenant_id: Some(tenant_user.tenant),
            effective_grants,
            created_at: Timestamp::now(),
        }
    }

    /// Create a hybrid SecurityPrincipal with both system and tenant permissions
    ///
    /// This is for users who are both system administrators and have tenant-specific access.
    /// Combines system-level and tenant-level permissions.
    pub fn from_hybrid_user(
        system_user: &SystemUserRecord,
        system_groups: &[SystemGroupRecord],
        system_roles: &[SystemRoleRecord],
        tenant_user: &TenantUserRecord,
        tenant_groups: &[TenantGroupRecord],
        tenant_roles: &[TenantRoleRecord],
    ) -> Self {
        let mut effective_grants = Self::resolve_system_grants(
            system_user,
            system_groups,
            system_roles,
        );

        let tenant_grants = Self::resolve_tenant_grants(
            system_user,
            tenant_user,
            tenant_groups,
            tenant_roles,
        );

        effective_grants.extend(tenant_grants);

        // Deduplicate combined grants
        effective_grants.sort_by_key(|g| format!("{:?}", g));
        effective_grants.dedup();

        Self {
            user_id: system_user.id,
            username: system_user.username.clone(),
            tenant_id: Some(tenant_user.tenant),
            effective_grants,
            created_at: Timestamp::now(),
        }
    }

    /// Switch tenant context for a user
    ///
    /// Creates a new SecurityPrincipal for the same user but in a different tenant context.
    /// This is used when a user needs to access resources in a different tenant they have access to.
    ///
    /// ## Authentication Flow
    ///
    /// 1. **Initial Login**: User authenticates with username/password
    ///    - System loads SystemUserRecord
    ///    - If user has system permissions, create system principal
    ///    - If user specifies a tenant (or has a default), create tenant principal
    ///
    /// 2. **Tenant Switching**: User switches to different tenant
    ///    - Verify user has access to target tenant (check TenantUserRecord exists)
    ///    - Create new principal for that tenant context
    ///    - Cache the new principal in session
    ///
    /// 3. **Multi-Tenant Sessions**: User can have multiple active tenant contexts
    ///    - Each tenant context has its own SecurityPrincipal
    ///    - Session manager tracks: SessionId -> (UserId, TenantId) -> SecurityPrincipal
    ///
    /// ## Example Usage
    ///
    /// ```rust,ignore
    /// // User logs in
    /// let session = authenticate("alice", "password").await?;
    /// let principal = session.principal; // Default tenant or system principal
    ///
    /// // User switches to different tenant
    /// let tenant_b_principal = principal.switch_tenant(
    ///     &system_user,
    ///     tenant_b_id,
    ///     &tenant_b_user,
    ///     &tenant_b_groups,
    ///     &tenant_b_roles,
    /// )?;
    ///
    /// // Cache the new principal
    /// session_manager.cache_tenant_principal(session_id, tenant_b_id, tenant_b_principal);
    /// ```
    pub fn switch_tenant(
        &self,
        system_user: &SystemUserRecord,
        tenant_user: &TenantUserRecord,
        tenant_groups: &[TenantGroupRecord],
        tenant_roles: &[TenantRoleRecord],
    ) -> Self {
        // Verify this is the same user
        assert_eq!(self.user_id, system_user.id, "Cannot switch tenant for different user");
        assert_eq!(system_user.id, tenant_user.user, "User ID mismatch");

        Self::from_tenant_user(system_user, tenant_user, tenant_groups, tenant_roles)
    }

    /// Check if this principal has access to a specific tenant
    ///
    /// Returns true if:
    /// - This is a system principal (tenant_id is None) with system-wide permissions
    /// - This is a tenant principal for the specified tenant
    pub fn has_tenant_access(&self, tenant_id: TenantId) -> bool {
        match self.tenant_id {
            None => self.is_superuser(), // System principals need superuser for cross-tenant access
            Some(principal_tenant) => principal_tenant == tenant_id,
        }
    }

    /// Get the tenant context for this principal
    pub fn tenant_context(&self) -> Option<TenantId> {
        self.tenant_id
    }

    /// Check if this is a system-level principal (not tenant-scoped)
    pub fn is_system_principal(&self) -> bool {
        self.tenant_id.is_none()
    }

    /// Check if this is a tenant-scoped principal
    pub fn is_tenant_principal(&self) -> bool {
        self.tenant_id.is_some()
    }

    /// Resolve system-level permission grants
    fn resolve_system_grants(
        system_user: &SystemUserRecord,
        system_groups: &[SystemGroupRecord],
        system_roles: &[SystemRoleRecord],
    ) -> Vec<PermissionGrant> {
        let mut grants = system_user.grants.clone();

        // Add grants from system groups
        for group_id in &system_user.groups {
            if let Some(group) = system_groups.iter().find(|g| g.id == *group_id) {
                grants.extend(group.grants.clone());

                // Add grants from group's system roles
                for role_id in &group.roles {
                    if let Some(role) = system_roles.iter().find(|r| r.id == *role_id) {
                        grants.extend(role.grants.clone());
                    }
                }
            }
        }

        // Add grants from direct system roles
        for role_id in &system_user.roles {
            if let Some(role) = system_roles.iter().find(|r| r.id == *role_id) {
                grants.extend(role.grants.clone());
            }
        }

        // Deduplicate grants
        grants.sort_by_key(|g| format!("{:?}", g));
        grants.dedup();
        grants
    }

    /// Resolve tenant-level permission grants
    fn resolve_tenant_grants(
        system_user: &SystemUserRecord,
        tenant_user: &TenantUserRecord,
        tenant_groups: &[TenantGroupRecord],
        tenant_roles: &[TenantRoleRecord],
    ) -> Vec<PermissionGrant> {
        let mut grants = Vec::new();

        // 1. Include system user direct grants (for hybrid scenarios)
        grants.extend(system_user.grants.clone());

        // 2. Add tenant user direct grants
        // Note: TenantUserRecord doesn't have grants field in current implementation
        // This would need to be added to TenantUserRecord if direct grants are needed

        // 3. Add grants from tenant groups
        for group_id in &tenant_user.groups {
            if let Some(group) = tenant_groups.iter().find(|g| g.id == *group_id) {
                grants.extend(group.grants.clone());

                // 4. Add grants from group's tenant roles
                for role_id in &group.roles {
                    if let Some(role) = tenant_roles.iter().find(|r| r.id == *role_id) {
                        grants.extend(role.grants.clone());
                    }
                }
            }
        }

        // 5. Add grants from direct tenant roles
        for role_id in &tenant_user.roles {
            if let Some(role) = tenant_roles.iter().find(|r| r.id == *role_id) {
                grants.extend(role.grants.clone());
            }
        }

        // Deduplicate grants
        grants.sort_by_key(|g| format!("{:?}", g));
        grants.dedup();
        grants
    }

    /// Get all effective permission grants
    pub fn grants(&self) -> &[PermissionGrant] {
        &self.effective_grants
    }

    /// Get all effective permissions (without scopes)
    pub fn permissions(&self) -> Vec<Permission> {
        let mut perms: Vec<Permission> = self
            .effective_grants
            .iter()
            .map(|g| g.permission.clone())
            .collect();

        // Deduplicate permissions
        perms.sort_by_key(|p| format!("{:?}", p));
        perms.dedup();
        perms
    }

    /// Check if principal has a specific permission (without resource scope)
    pub fn has_permission(&self, permission: &Permission) -> bool {
        self.effective_grants
            .iter()
            .any(|grant| grant.permission.implies(permission))
    }

    /// Check if principal has permission on a specific tenant
    pub fn has_tenant_permission(&self, permission: &Permission, tenant_id: TenantId) -> bool {
        self.effective_grants
            .iter()
            .any(|grant| grant.allows_tenant(permission, tenant_id))
    }

    /// Check if principal has permission on a specific database
    pub fn has_database_permission(
        &self,
        permission: &Permission,
        database_id: DatabaseId,
        tenant_id: TenantId,
    ) -> bool {
        self.effective_grants
            .iter()
            .any(|grant| grant.allows_database(permission, database_id, tenant_id))
    }

    /// Check if principal has permission on a specific table
    pub fn has_table_permission(
        &self,
        permission: &Permission,
        table_id: TableId,
        database_id: DatabaseId,
        tenant_id: TenantId,
    ) -> bool {
        self.effective_grants
            .iter()
            .any(|grant| grant.allows_table(permission, table_id, database_id, tenant_id))
    }

    /// Check if principal has permission on a specific namespace
    pub fn has_namespace_permission(
        &self,
        permission: &Permission,
        namespace_id: NamespaceId,
        database_id: DatabaseId,
        tenant_id: TenantId,
    ) -> bool {
        self.effective_grants
            .iter()
            .any(|grant| grant.allows_namespace(permission, namespace_id, database_id, tenant_id))
    }

    /// Check if principal has permission on a specific function
    pub fn has_function_permission(
        &self,
        permission: &Permission,
        function_id: FunctionId,
        database_id: DatabaseId,
        tenant_id: TenantId,
    ) -> bool {
        self.effective_grants
            .iter()
            .any(|grant| grant.allows_function(permission, function_id, database_id, tenant_id))
    }

    /// Check if principal is a superuser (has Superuser permission)
    pub fn is_superuser(&self) -> bool {
        self.has_permission(&Permission::Superuser)
    }
}
