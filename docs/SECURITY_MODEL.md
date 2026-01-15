# Security Model

## Overview

Nanograph implements a comprehensive, resource-scoped permission system that provides fine-grained access control across all system resources. The security model is based on three key concepts:

1. **Permissions** - Define what actions can be performed
2. **Resource Scopes** - Define which resources permissions apply to
3. **Permission Grants** - Combine permissions with scopes for precise access control

## Core Concepts

### Permission Grant

A `PermissionGrant` combines a permission with a resource scope:

```rust
pub struct PermissionGrant {
    pub permission: Permission,  // What action can be performed
    pub scope: ResourceScope,     // On which resources
}
```

**Example:**
```rust
// Allow reading from a specific table
PermissionGrant::new(Permission::TableRead, ResourceScope::Table(table_123))

// Allow creating tables in all databases within a tenant
PermissionGrant::new(Permission::DatabaseTableCreate, ResourceScope::AllInTenant(tenant_456))
```

### Resource Scopes

Resource scopes define the breadth of access a permission grant provides:

| Scope | Description | Example Use Case |
|-------|-------------|------------------|
| `System` | System-wide access | Superuser, cluster management |
| `Tenant(TenantId)` | Specific tenant | Tenant administrator |
| `AllTenants` | All tenants | Multi-tenant administrator |
| `Database(DatabaseId)` | Specific database | Database owner |
| `AllInTenant(TenantId)` | All databases in tenant | Tenant-wide database admin |
| `AllDatabases` | All databases system-wide | Global database administrator |
| `Table(TableId)` | Specific table | Application with table-specific access |
| `AllInDatabase(DatabaseId)` | All tables in database | Database-wide data access |
| `AllTables` | All tables system-wide | Global data reader |
| `Namespace(NamespaceId)` | Specific namespace | Namespace owner |
| `AllNamespacesInDatabase(DatabaseId)` | All namespaces in database | Database schema manager |
| `AllNamespaces` | All namespaces system-wide | Global namespace administrator |
| `Function(FunctionId)` | Specific function | Function executor |
| `AllFunctionsInDatabase(DatabaseId)` | All functions in database | Database function manager |
| `AllFunctions` | All functions system-wide | Global function administrator |

### Permission Hierarchy

Permissions are organized hierarchically by scope:

```
System Level
├── SystemClusterManage
├── SystemTenantManage
├── SystemUserManage
├── SystemGroupManage
├── SystemRoleManage
└── Superuser

Tenant Level
├── TenantDatabaseCreate
├── TenantDatabaseDelete
└── TenantConfigManage

Database Level
├── DatabaseTableCreate
├── DatabaseTableDelete
├── DatabaseNamespaceCreate
├── DatabaseNamespaceDelete
└── DatabaseConfigManage

Resource Level
├── TableRead
├── TableWrite
├── TableDelete
├── TableAlter
├── NamespaceObjectCreate
└── FunctionExecute
```

## Permission Types

### System-Level Permissions

Control cluster-wide operations:

- **SystemClusterManage** - Manage cluster configuration
- **SystemClusterView** - View cluster status
- **SystemRegionManage** - Manage regions
- **SystemServerManage** - Manage servers/nodes
- **SystemUserManage** - Manage users
- **SystemGroupManage** - Manage groups
- **SystemRoleManage** - Manage roles
- **SystemTenantManage** - Create, modify, delete tenants
- **SystemMetricsView** - View system metrics
- **SystemConfigManage** - Manage system configuration

### Tenant-Level Permissions

Control tenant-scoped operations:

- **TenantDatabaseCreate** - Create databases in tenant
- **TenantDatabaseDelete** - Delete databases in tenant
- **TenantConfigManage** - Manage tenant configuration
- **TenantView** - View tenant information

### Database-Level Permissions

Control database-scoped operations:

- **DatabaseNamespaceCreate** - Create namespaces in database
- **DatabaseNamespaceDelete** - Delete namespaces in database
- **DatabaseTableCreate** - Create tables in database
- **DatabaseTableDelete** - Delete tables in database
- **DatabaseConfigManage** - Manage database configuration
- **DatabaseSchemaView** - View database schema

### Table-Level Permissions

Control table data and structure:

- **TableRead** - Read data from table
- **TableWrite** - Write data to table
- **TableDelete** - Delete data from table
- **TableAlter** - Alter table structure
- **TableDrop** - Drop table
- **TableIndexCreate** - Create indexes on table
- **TableIndexDrop** - Drop indexes from table

### Namespace-Level Permissions

Control namespace operations:

- **NamespaceObjectCreate** - Create objects in namespace
- **NamespaceObjectDelete** - Delete objects from namespace
- **NamespaceConfigManage** - Manage namespace configuration

### Function-Level Permissions

Control function operations:

- **FunctionExecute** - Execute functions
- **FunctionCreate** - Create functions
- **FunctionDelete** - Delete functions
- **FunctionManage** - Manage function configuration

### Special Permissions

- **GrantPermission** - Grant permissions to other users
- **RevokePermission** - Revoke permissions from other users
- **Superuser** - All permissions (implies all other permissions)

## User, Group, and Role Model

### Users

Users are the primary security principals. They can be granted permissions through:

1. **Direct grants** - Permissions assigned directly to the user
2. **Group membership** - Permissions inherited from groups
3. **Role assignment** - Permissions inherited from roles

```rust
pub struct UserMetadata {
    pub id: UserId,
    pub name: String,
    pub groups: Vec<GroupId>,           // Groups user belongs to
    pub roles: Vec<RoleId>,             // Roles assigned to user
    pub grants: Vec<PermissionGrant>,   // Direct permission grants
    pub enabled: bool,
    // ... other fields
}
```

### Groups

Groups are collections of users with shared permissions:

```rust
pub struct GroupMetadata {
    pub id: GroupId,
    pub name: String,
    pub members: Vec<UserId>,           // Users in this group
    pub roles: Vec<RoleId>,             // Roles assigned to group
    pub grants: Vec<PermissionGrant>,   // Direct permission grants
    // ... other fields
}
```

### Roles

Roles are named collections of permission grants:

```rust
pub struct RoleMetadata {
    pub id: RoleId,
    pub name: String,
    pub grants: Vec<PermissionGrant>,   // Permission grants for this role
    // ... other fields
}
```

### Permission Inheritance

Users inherit permissions through multiple paths:

```
User Effective Permissions = 
    User Direct Grants
    + Group Grants (for all groups user belongs to)
    + Group Role Grants (for all roles assigned to user's groups)
    + User Role Grants (for all roles assigned directly to user)
```

## Common Use Cases

### 1. System Administrator

Full system access:

```rust
// Grant superuser permission with system-wide scope
PermissionGrant::new(Permission::Superuser, ResourceScope::System)
```

### 2. Tenant Administrator

Manage a specific tenant and all its databases:

```rust
// View tenant
PermissionGrant::new(Permission::TenantView, ResourceScope::Tenant(tenant_id))

// Create/delete databases in tenant
PermissionGrant::new(Permission::TenantDatabaseCreate, ResourceScope::Tenant(tenant_id))
PermissionGrant::new(Permission::TenantDatabaseDelete, ResourceScope::Tenant(tenant_id))

// Manage all databases in tenant
PermissionGrant::new(Permission::DatabaseConfigManage, ResourceScope::AllInTenant(tenant_id))
```

### 3. Database Owner

Full control over a specific database:

```rust
// Create/delete tables
PermissionGrant::new(Permission::DatabaseTableCreate, ResourceScope::Database(db_id))
PermissionGrant::new(Permission::DatabaseTableDelete, ResourceScope::Database(db_id))

// Create/delete namespaces
PermissionGrant::new(Permission::DatabaseNamespaceCreate, ResourceScope::Database(db_id))
PermissionGrant::new(Permission::DatabaseNamespaceDelete, ResourceScope::Database(db_id))

// Read/write all tables in database
PermissionGrant::new(Permission::TableRead, ResourceScope::AllInDatabase(db_id))
PermissionGrant::new(Permission::TableWrite, ResourceScope::AllInDatabase(db_id))
```

### 4. Application User

Read/write access to specific tables:

```rust
// Read from specific tables
PermissionGrant::new(Permission::TableRead, ResourceScope::Table(users_table))
PermissionGrant::new(Permission::TableRead, ResourceScope::Table(orders_table))

// Write to specific tables
PermissionGrant::new(Permission::TableWrite, ResourceScope::Table(orders_table))
PermissionGrant::new(Permission::TableWrite, ResourceScope::Table(audit_log_table))
```

### 5. Read-Only Analyst

Read access to all tables in a database:

```rust
// Read all tables in database
PermissionGrant::new(Permission::TableRead, ResourceScope::AllInDatabase(analytics_db))

// View database schema
PermissionGrant::new(Permission::DatabaseSchemaView, ResourceScope::Database(analytics_db))
```

### 6. Multi-Tenant Service Account

Access across multiple tenants:

```rust
// Read from all tables in all tenants
PermissionGrant::new(Permission::TableRead, ResourceScope::AllTenants)

// Execute functions in all databases
PermissionGrant::new(Permission::FunctionExecute, ResourceScope::AllFunctions)
```

## Permission Checking

### Checking Permissions

The `UserMetadata` type provides methods to check permissions:

```rust
// Check permission on a specific tenant
user.has_tenant_permission(&Permission::TenantView, tenant_id, &groups, &roles)

// Check permission on a specific database
user.has_database_permission(&Permission::DatabaseTableCreate, db_id, tenant_id, &groups, &roles)

// Check permission on a specific table
user.has_table_permission(&Permission::TableRead, table_id, db_id, tenant_id, &groups, &roles)

// Check permission on a specific namespace
user.has_namespace_permission(&Permission::NamespaceObjectCreate, ns_id, db_id, tenant_id, &groups, &roles)

// Check permission on a specific function
user.has_function_permission(&Permission::FunctionExecute, fn_id, db_id, tenant_id, &groups, &roles)
```

### Effective Grants

Get all effective permission grants for a user:

```rust
let effective_grants = user.effective_grants(&groups, &roles);
```

This returns all grants from:
- User's direct grants
- All groups the user belongs to
- All roles assigned to the user
- All roles assigned to the user's groups

## Best Practices

### 1. Principle of Least Privilege

Grant only the minimum permissions necessary:

```rust
// ❌ Too broad
PermissionGrant::new(Permission::TableWrite, ResourceScope::AllTables)

// ✅ Specific and minimal
PermissionGrant::new(Permission::TableWrite, ResourceScope::Table(specific_table))
```

### 2. Use Roles for Common Permission Sets

Define roles for common job functions:

```rust
// Define "Database Developer" role
let db_developer_role = RoleMetadata {
    name: "Database Developer".to_string(),
    grants: vec![
        PermissionGrant::new(Permission::DatabaseTableCreate, ResourceScope::AllInDatabase(db_id)),
        PermissionGrant::new(Permission::TableRead, ResourceScope::AllInDatabase(db_id)),
        PermissionGrant::new(Permission::TableWrite, ResourceScope::AllInDatabase(db_id)),
        PermissionGrant::new(Permission::DatabaseSchemaView, ResourceScope::Database(db_id)),
    ],
    // ... other fields
};
```

### 3. Use Groups for Team-Based Access

Organize users into groups:

```rust
// Create "Analytics Team" group
let analytics_group = GroupMetadata {
    name: "Analytics Team".to_string(),
    members: vec![user1_id, user2_id, user3_id],
    grants: vec![
        PermissionGrant::new(Permission::TableRead, ResourceScope::AllInDatabase(analytics_db)),
        PermissionGrant::new(Permission::FunctionExecute, ResourceScope::AllFunctionsInDatabase(analytics_db)),
    ],
    // ... other fields
};
```

### 4. Audit Permission Changes

Track who grants/revokes permissions:

```rust
// Require GrantPermission to modify permissions
if !user.has_system_permissions(&groups, &roles) {
    return Err("Insufficient permissions to grant access");
}
```

### 5. Regular Permission Reviews

Periodically review and revoke unnecessary permissions:

```rust
// List all effective grants for a user
let grants = user.effective_grants(&groups, &roles);
for grant in grants {
    println!("User has {:?} on {:?}", grant.permission, grant.scope);
}
```

## Security Considerations

### 1. Superuser Access

The `Superuser` permission grants all permissions. Use sparingly:

```rust
// Only for true system administrators
PermissionGrant::new(Permission::Superuser, ResourceScope::System)
```

### 2. Grant/Revoke Permissions

Control who can modify permissions:

```rust
// Only users with GrantPermission can grant permissions
PermissionGrant::new(Permission::GrantPermission, ResourceScope::System)
```

### 3. Wildcard Scopes

Be cautious with wildcard scopes like `AllTenants`, `AllDatabases`, `AllTables`:

```rust
// ⚠️ Very broad - use only when necessary
PermissionGrant::new(Permission::TableRead, ResourceScope::AllTables)

// ✅ More controlled
PermissionGrant::new(Permission::TableRead, ResourceScope::AllInDatabase(specific_db))
```

### 4. Separation of Duties

Separate administrative and data access permissions:

```rust
// System admin - no data access
PermissionGrant::new(Permission::SystemClusterManage, ResourceScope::System)

// Data user - no admin access
PermissionGrant::new(Permission::TableRead, ResourceScope::Table(table_id))
```

## Implementation Details

### Permission Matching

The `ResourceScope` type implements matching logic to determine if a scope covers a specific resource:

```rust
impl ResourceScope {
    pub fn matches_table(&self, table_id: TableId, database_id: DatabaseId, tenant_id: TenantId) -> bool {
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
}
```

### Permission Implication

The `Superuser` permission implies all other permissions:

```rust
impl Permission {
    pub fn implies(&self, other: &Permission) -> bool {
        match self {
            Permission::Superuser => true,  // Superuser has all permissions
            _ => self == other,
        }
    }
}
```

## Permission Enforcement Architecture

### Overview

Permission checks are enforced at the **KeyValueDatabaseContext** layer, providing a single enforcement point for all database operations. This architecture supports both standalone (single-user) and distributed (multi-user) deployments.

### Design Principles

1. **Single Enforcement Point** - All operations flow through KeyValueDatabaseContext
2. **Stateless Context** - Context is shared across users; user identity passed per-operation
3. **Thread-Safe** - No shared mutable user state
4. **Consistent Enforcement** - Same checks regardless of entry point (Manager, Container, Table)

### Architecture Layers

```
┌─────────────────────────────────────────────────────────────┐
│                    Application/Client Layer                  │
│  • Authenticates user                                        │
│  • Obtains UserMetadata                                      │
└─────────────────────────────────────────────────────────────┘
                            │
                            ▼
┌─────────────────────────────────────────────────────────────┐
│              API Layer (Entry Points)                        │
│  • KeyValueDatabaseManager (cluster/tenant/database ops)    │
│  • ContainerHandle (namespace/table management)             │
│  • TableHandle (data operations)                            │
│                                                              │
│  Each handle stores the UserMetadata it's bound to          │
└─────────────────────────────────────────────────────────────┘
                            │
                            ▼ (passes user per-operation)
┌─────────────────────────────────────────────────────────────┐
│         KeyValueDatabaseContext (Enforcement Layer)          │
│  ✓ Receives user parameter on every method                  │
│  ✓ Checks permissions before operations                     │
│  ✓ Loads user's groups and roles                            │
│  ✓ Evaluates effective grants                               │
│  ✓ Returns PermissionDenied on failure                      │
│  ✓ Logs all permission checks for audit                     │
└─────────────────────────────────────────────────────────────┘
                            │
                            ▼ (after permission check passes)
┌─────────────────────────────────────────────────────────────┐
│              Storage Layer (No Security Logic)               │
│  • KeyValueStore trait                                       │
│  • Storage engines (LSM/B+Tree/ART)                         │
│  • Shards and physical storage                              │
└─────────────────────────────────────────────────────────────┘
```

### Implementation Pattern

#### 1. KeyValueDatabaseContext Methods

Every public method in `KeyValueDatabaseContext` follows this pattern:

```rust
impl KeyValueDatabaseContext {
    pub async fn create_database(
        &self,
        user: &UserMetadata,           // User context passed per-operation
        tenant: &TenantId,
        config: DatabaseCreate,
    ) -> KeyValueResult<DatabaseMetadata> {
        // 1. CHECK PERMISSION FIRST
        self.check_permission(
            user,
            &Permission::TenantDatabaseCreate,
            ResourceScope::Tenant(*tenant)
        )?;
        
        // 2. PERFORM OPERATION (existing implementation)
        // ... create database logic ...
    }
    
    pub async fn put(
        &self,
        user: &UserMetadata,           // User context passed per-operation
        container: &ContainerId,
        table: &TableId,
        key: &[u8],
        value: &[u8],
    ) -> KeyValueResult<()> {
        // 1. CHECK PERMISSION FIRST
        self.check_permission(
            user,
            &Permission::TableWrite,
            ResourceScope::Table(*table)
        )?;
        
        // 2. PERFORM OPERATION (existing implementation)
        // ... write data logic ...
    }
}
```

#### 2. Permission Check Helper

```rust
impl KeyValueDatabaseContext {
    /// Check if user has permission for an operation
    fn check_permission(
        &self,
        user: &UserMetadata,
        permission: &Permission,
        scope: ResourceScope,
    ) -> KeyValueResult<()> {
        // Load user's groups and roles (with caching)
        let groups = self.load_user_groups(user)?;
        let roles = self.load_user_roles(user)?;
        
        // Get all effective grants
        let grants = user.effective_grants(&groups, &roles);
        
        // Check if any grant allows this operation
        let has_permission = grants.iter().any(|grant| {
            grant.permission.implies(permission) && 
            grant.scope.matches(&scope)
        });
        
        // Log the check for audit trail
        self.log_permission_check(user, permission, &scope, has_permission);
        
        if !has_permission {
            return Err(KeyValueError::PermissionDenied {
                user_id: user.id,
                permission: permission.clone(),
                resource: format!("{:?}", scope),
            });
        }
        
        Ok(())
    }
    
    /// Load user's groups with caching
    fn load_user_groups(&self, user: &UserMetadata) -> KeyValueResult<Vec<GroupMetadata>> {
        // Check cache first
        if let Some(cached) = self.get_cached_groups(user.id) {
            return Ok(cached);
        }
        
        // Load from system metadata cache
        let groups = user.groups.iter()
            .filter_map(|gid| self.system_metacache.read().unwrap().get_group(gid))
            .collect();
        
        // Cache for future use
        self.cache_groups(user.id, &groups);
        
        Ok(groups)
    }
    
    /// Load user's roles with caching
    fn load_user_roles(&self, user: &UserMetadata) -> KeyValueResult<Vec<RoleMetadata>> {
        // Similar to load_user_groups
        // ...
    }
}
```

#### 3. API Layer (Handles)

Handles store the user they're bound to and pass it to context:

```rust
pub struct ContainerHandle {
    container_id: ContainerId,
    user: UserMetadata,              // Bound to specific user
    context: Arc<KeyValueDatabaseContext>,
    metadata_cache: Arc<RwLock<ContainerMetadataCache>>,
}

impl ContainerHandle {
    pub async fn create_table(
        &self,
        config: TableCreate,
    ) -> KeyValueResult<TableId> {
        // Pass stored user to context
        self.context.create_table(
            &self.user,              // User from handle
            &self.container_id,
            config
        ).await
    }
}

pub struct TableHandle {
    container_id: ContainerId,
    table_id: TableId,
    user: UserMetadata,              // Bound to specific user
    context: Arc<KeyValueDatabaseContext>,
}

impl TableHandle {
    pub async fn put(&self, key: &[u8], value: &[u8]) -> KeyValueResult<()> {
        // Pass stored user to context
        self.context.put(
            &self.user,              // User from handle
            &self.container_id,
            &self.table_id,
            key,
            value
        ).await
    }
}
```

### Multi-User Safety

The architecture is designed for concurrent multi-user access:

1. **Stateless Context** - `KeyValueDatabaseContext` has no user state
2. **Per-Operation User** - User identity passed with each operation
3. **User-Bound Handles** - Each handle is bound to a specific user
4. **Thread-Safe** - No race conditions on user identity

Example with multiple concurrent users:

```rust
// Shared context (one instance for entire server)
let context = Arc::new(KeyValueDatabaseContext::new_standalone(config));

// User Alice's request
let alice = authenticate_user("alice", password)?;
let alice_container = ContainerHandle::new(container_id, alice, context.clone());
alice_container.create_table(table_config).await?;  // Uses alice's permissions

// User Bob's request (concurrent)
let bob = authenticate_user("bob", password)?;
let bob_container = ContainerHandle::new(container_id, bob, context.clone());
bob_container.create_table(table_config).await?;    // Uses bob's permissions

// No interference - each operation carries its own user context
```

### Performance Optimization

To avoid repeated permission lookups:

1. **Cache Groups/Roles** - Cache user's groups and roles in context
2. **Cache Effective Grants** - Cache computed permission grants
3. **TTL-Based Expiration** - Expire cached permissions after a timeout
4. **Invalidation on Change** - Clear cache when permissions are modified

```rust
struct PermissionCache {
    groups: Vec<GroupMetadata>,
    roles: Vec<RoleMetadata>,
    effective_grants: Vec<PermissionGrant>,
    cached_at: Timestamp,
    expires_at: Timestamp,
}

impl KeyValueDatabaseContext {
    permission_cache: Arc<RwLock<HashMap<UserId, Arc<PermissionCache>>>>,
}
```

### Audit Logging

All permission checks are logged for security audit:

```rust
fn log_permission_check(
    &self,
    user: &UserMetadata,
    permission: &Permission,
    scope: &ResourceScope,
    allowed: bool,
) {
    tracing::info!(
        user_id = %user.id,
        user_name = %user.name,
        permission = ?permission,
        scope = ?scope,
        allowed = allowed,
        "Permission check"
    );
}
```

### Error Handling

Permission denied errors include full context:

```rust
pub enum KeyValueError {
    PermissionDenied {
        user_id: UserId,
        permission: Permission,
        resource: String,
    },
    // ... other errors
}
```

### Testing Strategy

1. **Unit Tests** - Test `check_permission()` with various grants
2. **Integration Tests** - Test full operation flow with permissions
3. **Multi-User Tests** - Test concurrent access with different users
4. **Security Tests** - Attempt unauthorized operations
5. **Performance Tests** - Verify caching effectiveness

## Future Enhancements

Potential future additions to the security model:

1. **Time-based permissions** - Grants that expire after a certain time
2. **Conditional permissions** - Grants based on context (IP address, time of day, etc.)
3. **Permission delegation** - Allow users to delegate their permissions temporarily
4. **Audit logging** - Comprehensive logging of all permission checks and changes
5. **Permission templates** - Pre-defined permission sets for common scenarios
6. **Dynamic permissions** - Permissions computed at runtime based on data attributes

## Related Documentation

- [ADR-0010: Authentication, Authorization, and Access Control](ADR/ADR-0010-Authentication-Authorization-Access-Control.md)
- [Multi-Tenancy and Isolation](ADR/ADR-0025-Multi-Tenancy-and-Isolation-FINAL.md)
- [Resource Quotas and Limits](ADR/ADR-0026-Resource-Quotas-and-Limits.md)