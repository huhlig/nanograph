# Security Model

## Overview

Nanograph implements a comprehensive, resource-scoped permission system that provides fine-grained access control across
all system resources. The security model is based on three key concepts:

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

| Scope                                 | Description                | Example Use Case                       |
|---------------------------------------|----------------------------|----------------------------------------|
| `System`                              | System-wide access         | Superuser, cluster management          |
| `Tenant(TenantId)`                    | Specific tenant            | Tenant administrator                   |
| `AllTenants`                          | All tenants                | Multi-tenant administrator             |
| `Database(DatabaseId)`                | Specific database          | Database owner                         |
| `AllInTenant(TenantId)`               | All databases in tenant    | Tenant-wide database admin             |
| `AllDatabases`                        | All databases system-wide  | Global database administrator          |
| `Table(TableId)`                      | Specific table             | Application with table-specific access |
| `AllInDatabase(DatabaseId)`           | All tables in database     | Database-wide data access              |
| `AllTables`                           | All tables system-wide     | Global data reader                     |
| `Namespace(NamespaceId)`              | Specific namespace         | Namespace owner                        |
| `AllNamespacesInDatabase(DatabaseId)` | All namespaces in database | Database schema manager                |
| `AllNamespaces`                       | All namespaces system-wide | Global namespace administrator         |
| `Function(FunctionId)`                | Specific function          | Function executor                      |
| `AllFunctionsInDatabase(DatabaseId)`  | All functions in database  | Database function manager              |
| `AllFunctions`                        | All functions system-wide  | Global function administrator          |

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

### Multi-Tenant Architecture

Nanograph implements a **hybrid security model** that supports both system-wide and tenant-scoped security principals:

- **System-Level**: Global user identities, system administrators, and cross-tenant operations
- **Tenant-Level**: Tenant-specific groups, roles, and permissions for complete isolation

This architecture ensures:
1. **Shared User Identity**: Users have a single system-wide identity (`SystemUserRecord`)
2. **Tenant Isolation**: Each tenant has independent groups, roles, and permissions
3. **Flexible Access**: Users can belong to multiple tenants with different permissions in each

### Users

#### System User Record

The global user identity shared across all tenants:

```rust
pub struct SystemUserRecord {
    pub id: UserId,
    pub username: String,
    pub version: u64,
    pub created_at: Timestamp,
    pub last_modified: Timestamp,
    pub enabled: bool,
    pub password_hash: String,
    pub groups: Vec<GroupId>,           // System-level groups (for admins)
    pub roles: Vec<RoleId>,             // System-level roles (for admins)
    pub grants: Vec<PermissionGrant>,   // System-wide permission grants
    pub options: HashMap<String, String>,
    pub metadata: HashMap<String, String>,
}
```

**Use Cases for System-Level Groups/Roles:**
- System administrators with cross-tenant access
- Platform operators managing the cluster
- Service accounts with system-wide permissions

#### Tenant User Record

Tenant-specific overlay that extends the system user with tenant-scoped permissions:

```rust
pub struct TenantUserRecord {
    pub user: UserId,                   // References SystemUserRecord
    pub tenant: TenantId,               // Scoped to specific tenant
    pub version: u64,
    pub created_at: Timestamp,
    pub last_modified: Timestamp,
    pub groups: Vec<GroupId>,           // Tenant-scoped groups
    pub roles: Vec<RoleId>,             // Tenant-scoped roles
    pub grants: Vec<PermissionGrant>,   // Tenant-scoped direct grants
    pub options: HashMap<String, String>,
    pub metadata: HashMap<String, String>,
}
```

**Key Properties:**
- One `SystemUserRecord` can have multiple `TenantUserRecord` entries (one per tenant)
- Each tenant sees the user with different groups, roles, and permissions
- Complete isolation between tenants

**Example:**
```rust
// Alice's system identity
SystemUserRecord { id: user_123, username: "alice", ... }

// Alice in Tenant A (Engineering company)
TenantUserRecord {
    user: user_123,
    tenant: tenant_a,
    groups: [engineering_team, database_admins],
    roles: [developer, db_owner],
}

// Alice in Tenant B (Consulting client)
TenantUserRecord {
    user: user_123,
    tenant: tenant_b,
    groups: [consultants],
    roles: [read_only_analyst],
}
```

### Groups

#### System Groups (Optional)

System-wide groups for administrators:

```rust
pub struct SystemGroupRecord {
    pub id: GroupId,
    pub name: String,
    pub version: u64,
    pub created_at: Timestamp,
    pub last_modified: Timestamp,
    pub members: Vec<UserId>,           // System users
    pub roles: Vec<RoleId>,             // System roles
    pub grants: Vec<PermissionGrant>,   // System-wide grants
    pub options: HashMap<String, String>,
    pub metadata: HashMap<String, String>,
}
```

**Use Cases:**
- "Platform Administrators" - manage all tenants
- "System Operators" - cluster management
- "Security Auditors" - cross-tenant audit access

#### Tenant Groups (Primary)

Tenant-scoped groups for organizational structure within a tenant:

```rust
pub struct TenantGroupRecord {
    pub id: GroupId,
    pub tenant_id: TenantId,            // Scoped to specific tenant
    pub name: String,
    pub version: u64,
    pub created_at: Timestamp,
    pub last_modified: Timestamp,
    pub members: Vec<UserId>,           // Users in this tenant
    pub roles: Vec<RoleId>,             // Tenant-scoped roles
    pub grants: Vec<PermissionGrant>,   // Tenant-scoped grants
    pub options: HashMap<String, String>,
    pub metadata: HashMap<String, String>,
}
```

**Benefits:**
- **Isolation**: Groups in Tenant A are invisible to Tenant B
- **Flexibility**: Each tenant defines its own organizational structure
- **Security**: No cross-tenant information leakage
- **Independence**: Tenants manage their own groups without coordination

**Example Tenant Groups:**
```rust
// Tenant A (Enterprise)
TenantGroupRecord { tenant_id: tenant_a, name: "Engineering", ... }
TenantGroupRecord { tenant_id: tenant_a, name: "Sales", ... }
TenantGroupRecord { tenant_id: tenant_a, name: "Finance", ... }

// Tenant B (Startup)
TenantGroupRecord { tenant_id: tenant_b, name: "Developers", ... }
TenantGroupRecord { tenant_id: tenant_b, name: "Admins", ... }

// Tenant C (Agency)
TenantGroupRecord { tenant_id: tenant_c, name: "Client-ProjectX", ... }
TenantGroupRecord { tenant_id: tenant_c, name: "Client-ProjectY", ... }
```

### Roles

#### System Roles (Optional)

System-wide roles for administrators:

```rust
pub struct SystemRoleRecord {
    pub id: RoleId,
    pub name: String,
    pub version: u64,
    pub created_at: Timestamp,
    pub last_modified: Timestamp,
    pub grants: Vec<PermissionGrant>,   // System-wide grants
    pub options: HashMap<String, String>,
    pub metadata: HashMap<String, String>,
}
```

**Example System Roles:**
- "Superuser" - full system access
- "Cluster Administrator" - cluster management
- "Tenant Manager" - create/delete tenants

#### Tenant Roles (Primary)

Tenant-scoped roles for permission management within a tenant:

```rust
pub struct TenantRoleRecord {
    pub id: RoleId,
    pub tenant_id: TenantId,            // Scoped to specific tenant
    pub name: String,
    pub version: u64,
    pub created_at: Timestamp,
    pub last_modified: Timestamp,
    pub grants: Vec<PermissionGrant>,   // Tenant-scoped grants
    pub options: HashMap<String, String>,
    pub metadata: HashMap<String, String>,
}
```

**Benefits:**
- **Natural Scoping**: Permissions automatically scoped to tenant resources
- **Reusability**: Common roles (e.g., "Database Owner") defined per-tenant
- **Flexibility**: Each tenant customizes roles for their needs

**Example Tenant Roles:**
```rust
// Tenant A roles
TenantRoleRecord {
    tenant_id: tenant_a,
    name: "Database Owner",
    grants: vec![
        PermissionGrant::new(
            Permission::DatabaseTableCreate,
            ResourceScope::AllInTenant(tenant_a)
        ),
        PermissionGrant::new(
            Permission::TableRead,
            ResourceScope::AllInTenant(tenant_a)
        ),
        PermissionGrant::new(
            Permission::TableWrite,
            ResourceScope::AllInTenant(tenant_a)
        ),
    ],
}

TenantRoleRecord {
    tenant_id: tenant_a,
    name: "Read-Only Analyst",
    grants: vec![
        PermissionGrant::new(
            Permission::TableRead,
            ResourceScope::AllInTenant(tenant_a)
        ),
        PermissionGrant::new(
            Permission::DatabaseSchemaView,
            ResourceScope::AllInTenant(tenant_a)
        ),
    ],
}
```

### Permission Inheritance

#### System-Level Permission Resolution

For system administrators:

```
System User Effective Permissions =
    System User Direct Grants
    + System Group Grants (for all system groups user belongs to)
    + System Group Role Grants (for all system roles assigned to user's groups)
    + System User Role Grants (for all system roles assigned directly to user)
```

#### Tenant-Level Permission Resolution

For tenant users (most common):

```
Tenant User Effective Permissions =
    System User Direct Grants (if any system-level grants)
    + Tenant User Direct Grants
    + Tenant Group Grants (for all tenant groups user belongs to)
    + Tenant Group Role Grants (for all tenant roles assigned to user's groups)
    + Tenant User Role Grants (for all tenant roles assigned directly to user)
```

#### SecurityPrincipal Creation

The `SecurityPrincipal` resolves all permissions at authentication time:

```rust
impl SecurityPrincipal {
    /// Create principal for tenant-specific access
    pub fn from_tenant_user(
        system_user: &SystemUserRecord,
        tenant_user: &TenantUserRecord,
        tenant_groups: &[TenantGroupRecord],
        tenant_roles: &[TenantRoleRecord],
    ) -> Self {
        let mut grants = Vec::new();
        
        // 1. System-level grants (if user is also a system admin)
        grants.extend(system_user.grants.clone());
        
        // 2. Tenant user direct grants
        grants.extend(tenant_user.grants.clone());
        
        // 3. Tenant group grants
        for group_id in &tenant_user.groups {
            if let Some(group) = tenant_groups.iter().find(|g| g.id == *group_id) {
                grants.extend(group.grants.clone());
                
                // 4. Tenant roles from groups
                for role_id in &group.roles {
                    if let Some(role) = tenant_roles.iter().find(|r| r.id == *role_id) {
                        grants.extend(role.grants.clone());
                    }
                }
            }
        }
        
        // 5. Direct tenant roles
        for role_id in &tenant_user.roles {
            if let Some(role) = tenant_roles.iter().find(|r| r.id == *role_id) {
                grants.extend(role.grants.clone());
            }
        }
        
        // Deduplicate and return
        grants.sort_by_key(|g| format!("{:?}", g));
        grants.dedup();
        
        Self {
            user_id: system_user.id,
            username: system_user.username.clone(),
            effective_grants: grants,
            created_at: Timestamp::now(),
        }
    }
}
```

### Storage Strategy

#### Storage Location: System Metadata Cache (Tier 1)

All security metadata (system and tenant-level) is stored in the **System Metadata Cache** within the `system_shard` (Tier 1 of the three-tier Raft architecture).

**Why Tier 1?**
- Cross-database scope (users access multiple databases)
- Required for authentication (before database access)
- Low update frequency (minutes to hours)
- Small data size (KB to MB even with thousands of users)
- Cluster-wide visibility needed

**Implementation:**
```rust
pub struct SystemMetadataCache {
    shard: ShardId,  // system_shard
    
    // System-level security (administrators)
    system_users: HashMap<UserId, SystemUserRecord>,
    system_roles: HashMap<SystemRoleId, SystemRoleRecord>,
    system_groups: HashMap<SystemGroupId, SystemGroupRecord>,
    
    // Tenant-level security (regular users)
    tenant_users: HashMap<(TenantId, UserId), TenantUserRecord>,
    tenant_roles: HashMap<(TenantId, TenantRoleId), TenantRoleRecord>,
    tenant_groups: HashMap<(TenantId, TenantGroupId), TenantGroupRecord>,
    
    // Other system metadata...
}
```

#### Key Patterns

Tenant-scoped entities use composite keys for natural isolation:

```
System Users:     system:user:{user_id}
Tenant Users:     tenant:{tenant_id}:user:{user_id}
System Groups:    system:group:{group_id}
Tenant Groups:    tenant:{tenant_id}:group:{group_id}
System Roles:     system:role:{role_id}
Tenant Roles:     tenant:{tenant_id}:role:{role_id}
```

**Benefits:**
- Natural tenant isolation at storage level
- Efficient tenant-scoped queries
- Clear ownership boundaries
- Prevents accidental cross-tenant access
- Single source of truth (all in system_shard)
- Atomic updates across security metadata

#### Indexing

```
User to Tenants:  user:{user_id}:tenants -> [tenant_id, ...]
Tenant Members:   tenant:{tenant_id}:users -> [user_id, ...]
Group Members:    tenant:{tenant_id}:group:{group_id}:members -> [user_id, ...]
```

**Note:** See [Tenant Security Storage Strategy](DEV/TENANT_SECURITY_STORAGE_STRATEGY.md) for detailed implementation guidance on storage, caching, and loading strategies.

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

## Security Principal

### Overview

The security model uses a **SecurityPrincipal** abstraction to separate authentication/authorization from metadata storage. A `SecurityPrincipal` represents an authenticated entity with all permissions pre-resolved at authentication time.

### Key Benefits

1. **Performance** - Permissions resolved once, not on every check
2. **Separation of Concerns** - Metadata objects are just data; principals enforce security
3. **Immutability** - Principal's permissions don't change during a session
4. **Auditability** - Clear snapshot of active permissions at authentication time
5. **Thread Safety** - Immutable principals can be safely shared
6. **Multi-Tenant Support** - Clean separation between system and tenant contexts

### SecurityPrincipal Structure

```rust
pub struct SecurityPrincipal {
    /// User ID this principal represents
    pub user_id: UserId,
    /// Username for logging/audit
    pub username: String,
    /// Optional tenant ID if this is a tenant-scoped principal
    pub tenant_id: Option<TenantId>,
    /// All effective permission grants (pre-resolved)
    effective_grants: Vec<PermissionGrant>,
    /// Timestamp when principal was created
    pub created_at: Timestamp,
}
```

### Creating a SecurityPrincipal

The `SecurityPrincipal` supports three creation patterns to handle different authentication scenarios:

#### 1. System Principal (Administrators)

For users with system-wide administrative access:

```rust
// Authenticate user and load system-level metadata
let system_user = authenticate_user(username, password)?;
let system_groups = load_system_groups(&system_user.groups)?;
let system_roles = load_system_roles(&system_user.roles)?;

// Create system-level principal
let principal = SecurityPrincipal::from_system_user(
    &system_user,
    &system_groups,
    &system_roles,
);
```

**Permission Resolution:**
- System user direct grants
- System group grants (for all groups user belongs to)
- System group role grants (for all roles assigned to user's groups)
- System user role grants (for all roles assigned directly to user)

#### 2. Tenant Principal (Regular Users)

For users accessing a specific tenant (most common case):

```rust
// Authenticate user and load tenant-specific metadata
let system_user = authenticate_user(username, password)?;
let tenant_user = load_tenant_user(system_user.id, tenant_id)?;
let tenant_groups = load_tenant_groups(tenant_id, &tenant_user.groups)?;
let tenant_roles = load_tenant_roles(tenant_id, &tenant_user.roles)?;

// Create tenant-scoped principal
let principal = SecurityPrincipal::from_tenant_user(
    &system_user,
    &tenant_user,
    &tenant_groups,
    &tenant_roles,
);
```

**Permission Resolution:**
- System user direct grants (if any system-level permissions)
- Tenant user direct grants
- Tenant group grants (for all tenant groups user belongs to)
- Tenant group role grants (for all tenant roles assigned to user's groups)
- Tenant user role grants (for all tenant roles assigned directly to user)

#### 3. Hybrid Principal (System Admin with Tenant Access)

For users who are both system administrators and need tenant-specific access:

```rust
// Load both system and tenant metadata
let system_user = authenticate_user(username, password)?;
let system_groups = load_system_groups(&system_user.groups)?;
let system_roles = load_system_roles(&system_user.roles)?;
let tenant_user = load_tenant_user(system_user.id, tenant_id)?;
let tenant_groups = load_tenant_groups(tenant_id, &tenant_user.groups)?;
let tenant_roles = load_tenant_roles(tenant_id, &tenant_user.roles)?;

// Create hybrid principal with both system and tenant permissions
let principal = SecurityPrincipal::from_hybrid_user(
    &system_user,
    &system_groups,
    &system_roles,
    &tenant_user,
    &tenant_groups,
    &tenant_roles,
);
```

**Permission Resolution:**
- All system-level permissions (as in System Principal)
- All tenant-level permissions (as in Tenant Principal)
- Combined and deduplicated

### Tenant Context Switching

Users can access multiple tenants by switching context:

```rust
// User initially authenticated for Tenant A
let principal_a = SecurityPrincipal::from_tenant_user(...);

// User switches to Tenant B
let principal_b = principal_a.switch_tenant(
    &system_user,
    &tenant_b_user,
    &tenant_b_groups,
    &tenant_b_roles,
);
```

**Session Management Strategy:**

1. **Initial Login**: Create principal for default/specified tenant
2. **Cache Principal**: Store in session (not recreated per request)
3. **Tenant Switch**: Create new principal for different tenant, cache separately
4. **Multi-Tenant Cache**: Session manager tracks `(SessionId, TenantId) -> Principal`
5. **Invalidation**: Only when permissions change, not per-request

### Principal Methods

```rust
impl SecurityPrincipal {
    // Tenant context queries
    pub fn tenant_context(&self) -> Option<TenantId>;
    pub fn is_system_principal(&self) -> bool;
    pub fn is_tenant_principal(&self) -> bool;
    pub fn has_tenant_access(&self, tenant_id: TenantId) -> bool;
    
    // Permission checks (existing methods)
    pub fn has_permission(&self, permission: &Permission) -> bool;
    pub fn has_tenant_permission(&self, permission: &Permission, tenant_id: TenantId) -> bool;
    pub fn has_database_permission(&self, permission: &Permission, database_id: DatabaseId, tenant_id: TenantId) -> bool;
    pub fn has_table_permission(&self, permission: &Permission, table_id: TableId, database_id: DatabaseId, tenant_id: TenantId) -> bool;
    pub fn is_superuser(&self) -> bool;
}
```

## Permission Checking

### Checking Permissions with SecurityPrincipal

Once you have a `SecurityPrincipal`, permission checks are simple and efficient:

```rust
// Check permission on a specific tenant
principal.has_tenant_permission(&Permission::TenantView, tenant_id)

// Check permission on a specific database
principal.has_database_permission(&Permission::DatabaseTableCreate, db_id, tenant_id)

// Check permission on a specific table
principal.has_table_permission(&Permission::TableRead, table_id, db_id, tenant_id)

// Check permission on a specific namespace
principal.has_namespace_permission(&Permission::NamespaceObjectCreate, ns_id, db_id, tenant_id)

// Check permission on a specific function
principal.has_function_permission(&Permission::FunctionExecute, fn_id, db_id, tenant_id)

// Check if superuser
if principal.is_superuser() {
    // Allow all operations
}
```

### Effective Grants

Get all effective permission grants from a principal:

```rust
let grants = principal.grants();
for grant in grants {
    println!("Permission: {:?}, Scope: {:?}", grant.permission, grant.scope);
}
```

### Legacy UserMetadata Methods

For backward compatibility, `UserMetadata` still has permission checking methods, but they require passing groups and roles on every call:

```rust
// Legacy approach (less efficient)
user.has_table_permission(&Permission::TableRead, table_id, db_id, tenant_id, &groups, &roles)

// Preferred approach (more efficient)
let principal = SecurityPrincipal::from_user(&user, &groups, &roles);
principal.has_table_permission(&Permission::TableRead, table_id, db_id, tenant_id)
```

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
if ! user.has_system_permissions( & groups, & roles) {
return Err("Insufficient permissions to grant access");
}
```

### 5. Regular Permission Reviews

Periodically review and revoke unnecessary permissions:

```rust
// List all effective grants for a user
let grants = user.effective_grants( & groups, & roles);
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

Permission checks are enforced at the **KeyValueDatabaseContext** layer, providing a single enforcement point for all
database operations. This architecture supports both standalone (single-user) and distributed (multi-user) deployments.

### Design Principles

1. **Single Enforcement Point** - All operations flow through KeyValueDatabaseContext
2. **Stateless Context** - Context is shared across users; user identity passed per-operation
3. **Thread-Safe** - No shared mutable user state
4. **Consistent Enforcement** - Same checks regardless of entry point (Manager, Container, Table)

### Architecture Layers

```
┌─────────────────────────────────────────────────────────────┐
│                    Application/Client Layer                 │
│  • Authenticates user                                       │
│  • Obtains UserMetadata                                     │
└─────────────────────────────────────────────────────────────┘
                            │
                            ▼
┌─────────────────────────────────────────────────────────────┐
│              API Layer (Entry Points)                       │
│  • KeyValueDatabaseManager (cluster/tenant/database ops)    │
│  • ContainerHandle (namespace/table management)             │
│  • TableHandle (data operations)                            │
│                                                             │
│  Each handle stores the UserMetadata it's bound to          │
└─────────────────────────────────────────────────────────────┘
                            │
                            ▼ (passes user per-operation)
┌─────────────────────────────────────────────────────────────┐
│         KeyValueDatabaseContext (Enforcement Layer)         │
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
│              Storage Layer (No Security Logic)              │
│  • KeyValueStore trait                                      │
│  • Storage engines (LSM/B+Tree/ART)                         │
│  • Shards and physical storage                              │
└─────────────────────────────────────────────────────────────┘
```

### Implementation Pattern with SecurityPrincipal

#### 1. KeyValueDatabaseContext Methods

Every public method in `KeyValueDatabaseContext` accepts a `SecurityPrincipal`:

```rust
impl KeyValueDatabaseContext {
    pub async fn create_database(
        &self,
        principal: &SecurityPrincipal,  // Security principal passed per-operation
        tenant: &TenantId,
        config: DatabaseCreate,
    ) -> KeyValueResult<DatabaseMetadata> {
        // 1. CHECK PERMISSION FIRST (simple and efficient)
        if !principal.has_tenant_permission(&Permission::TenantDatabaseCreate, *tenant) {
            return Err(KeyValueError::PermissionDenied {
                user_id: principal.user_id,
                permission: Permission::TenantDatabaseCreate,
                resource: format!("Tenant({})", tenant),
            });
        }

        // 2. LOG FOR AUDIT
        self.log_permission_check(principal, &Permission::TenantDatabaseCreate, tenant, true);

        // 3. PERFORM OPERATION
        // ... create database logic ...
    }

    pub async fn put(
        &self,
        principal: &SecurityPrincipal,  // Security principal passed per-operation
        container: &ContainerId,
        table: &TableId,
        key: &[u8],
        value: &[u8],
    ) -> KeyValueResult<()> {
        // 1. RESOLVE TABLE METADATA (to get database_id and tenant_id)
        let table_meta = self.get_table_metadata(container, table)?;
        
        // 2. CHECK PERMISSION
        if !principal.has_table_permission(
            &Permission::TableWrite,
            *table,
            table_meta.database_id,
            table_meta.tenant_id
        ) {
            return Err(KeyValueError::PermissionDenied {
                user_id: principal.user_id,
                permission: Permission::TableWrite,
                resource: format!("Table({})", table),
            });
        }

        // 3. LOG FOR AUDIT
        self.log_permission_check(principal, &Permission::TableWrite, table, true);

        // 4. PERFORM OPERATION
        // ... write data logic ...
    }
}
```

#### 2. Permission Check Helper (Simplified)

```rust
impl KeyValueDatabaseContext {
    /// Check if principal has permission for an operation
    /// This is now much simpler since principal has pre-resolved permissions
    fn check_permission(
        &self,
        principal: &SecurityPrincipal,
        permission: &Permission,
        resource: impl std::fmt::Display,
    ) -> KeyValueResult<()> {
        // Check is now a simple method call on principal
        let has_permission = match resource {
            tenant_id if is_tenant => principal.has_tenant_permission(permission, tenant_id),
            (db_id, tenant_id) if is_database => principal.has_database_permission(permission, db_id, tenant_id),
            // ... other resource types
        };

        // Log the check for audit trail
        self.log_permission_check(principal, permission, &resource, has_permission);

        if !has_permission {
            return Err(KeyValueError::PermissionDenied {
                user_id: principal.user_id,
                permission: permission.clone(),
                resource: format!("{}", resource),
            });
        }

        Ok(())
    }
}
```

#### 3. API Layer (Handles)

Handles store the security principal they're bound to:

```rust
pub struct ContainerHandle {
    container_id: ContainerId,
    principal: Arc<SecurityPrincipal>,  // Bound to specific principal
    context: Arc<KeyValueDatabaseContext>,
    metadata_cache: Arc<RwLock<ContainerMetadataCache>>,
}

impl ContainerHandle {
    /// Create a new container handle bound to a security principal
    pub fn new(
        container_id: ContainerId,
        principal: Arc<SecurityPrincipal>,
        context: Arc<KeyValueDatabaseContext>,
    ) -> Self {
        Self {
            container_id,
            principal,
            context,
            metadata_cache: Arc::new(RwLock::new(ContainerMetadataCache::new())),
        }
    }

    pub async fn create_table(
        &self,
        config: TableCreate,
    ) -> KeyValueResult<TableId> {
        // Pass stored principal to context
        self.context.create_table(
            &self.principal,         // Principal from handle
            &self.container_id,
            config
        ).await
    }
}

pub struct TableHandle {
    container_id: ContainerId,
    table_id: TableId,
    principal: Arc<SecurityPrincipal>,  // Bound to specific principal
    context: Arc<KeyValueDatabaseContext>,
}

impl TableHandle {
    /// Create a new table handle bound to a security principal
    pub fn new(
        container_id: ContainerId,
        table_id: TableId,
        principal: Arc<SecurityPrincipal>,
        context: Arc<KeyValueDatabaseContext>,
    ) -> Self {
        Self {
            container_id,
            table_id,
            principal,
            context,
        }
    }

    pub async fn put(&self, key: &[u8], value: &[u8]) -> KeyValueResult<()> {
        // Pass stored principal to context
        self.context.put(
            &self.principal,         // Principal from handle
            &self.container_id,
            &self.table_id,
            key,
            value
        ).await
    }
}
```

### Multi-User Safety with SecurityPrincipal

The architecture is designed for concurrent multi-user access:

1. **Stateless Context** - `KeyValueDatabaseContext` has no user state
2. **Per-Operation Principal** - Security principal passed with each operation
3. **Principal-Bound Handles** - Each handle is bound to a specific principal
4. **Thread-Safe** - Principals are immutable and can be safely shared
5. **No Permission Resolution Overhead** - Permissions pre-resolved at authentication

Example with multiple concurrent users:

```rust
// Shared context (one instance for entire server)
let context = Arc::new(KeyValueDatabaseContext::new_standalone(config));

// User Alice's request
let alice_user = authenticate_user("alice", password)?;
let alice_groups = load_user_groups(&alice_user)?;
let alice_roles = load_user_roles(&alice_user)?;
let alice_principal = Arc::new(SecurityPrincipal::from_user(&alice_user, &alice_groups, &alice_roles));

let alice_container = ContainerHandle::new(container_id, alice_principal.clone(), context.clone());
alice_container.create_table(table_config).await?;  // Uses alice's permissions

// User Bob's request (concurrent)
let bob_user = authenticate_user("bob", password)?;
let bob_groups = load_user_groups(&bob_user)?;
let bob_roles = load_user_roles(&bob_user)?;
let bob_principal = Arc::new(SecurityPrincipal::from_user(&bob_user, &bob_groups, &bob_roles));

let bob_container = ContainerHandle::new(container_id, bob_principal.clone(), context.clone());
bob_container.create_table(table_config).await?;    // Uses bob's permissions

// No interference - each operation carries its own principal with pre-resolved permissions
```

### Performance Optimization with SecurityPrincipal

The `SecurityPrincipal` design provides significant performance benefits:

1. **One-Time Resolution** - Permissions resolved once at authentication, not on every check
2. **No Repeated Lookups** - No need to load groups/roles on every operation
3. **Efficient Checks** - Simple iteration over pre-resolved grants
4. **Principal Caching** - Cache principals by session ID for reuse

#### Principal Caching Strategy

```rust
/// Cache for active security principals
struct PrincipalCache {
    /// Map session ID to principal
    principals: Arc<RwLock<HashMap<SessionId, Arc<SecurityPrincipal>>>>,
    /// TTL for cached principals (e.g., 1 hour)
    ttl: Duration,
}

impl PrincipalCache {
    /// Get or create a principal for a session
    pub fn get_or_create(
        &self,
        session_id: SessionId,
        user: &UserMetadata,
        groups: &[GroupMetadata],
        roles: &[RoleMetadata],
    ) -> Arc<SecurityPrincipal> {
        let mut cache = self.principals.write().unwrap();
        
        // Check if principal exists and is not expired
        if let Some(principal) = cache.get(&session_id) {
            if principal.created_at.elapsed() < self.ttl {

### Authentication Flow and Session Management

#### Authentication Process

When a user authenticates, the system determines which type of principal to create:

```rust
async fn authenticate(
    username: &str, 
    password: &str, 
    tenant_id: Option<TenantId>
) -> Result<Session> {
    // 1. Verify credentials
    let system_user = verify_credentials(username, password).await?;
    
    if !system_user.enabled {
        return Err(AuthError::AccountDisabled);
    }
    
    // 2. Determine principal type based on context
    let principal = match tenant_id {
        Some(tid) => {
            // User specified tenant - create tenant principal
            let tenant_user = load_tenant_user(system_user.id, tid).await?;
            let tenant_groups = load_tenant_groups(tid, &tenant_user.groups).await?;
            let tenant_roles = load_tenant_roles(tid, &tenant_user.roles).await?;
            
            SecurityPrincipal::from_tenant_user(
                &system_user,
                &tenant_user,
                &tenant_groups,
                &tenant_roles,
            )
        }
        None if has_system_permissions(&system_user) => {
            // User has system permissions - create system principal
            let system_groups = load_system_groups(&system_user.groups).await?;
            let system_roles = load_system_roles(&system_user.roles).await?;
            
            SecurityPrincipal::from_system_user(
                &system_user,
                &system_groups,
                &system_roles,
            )
        }
        None => {
            // User has no system permissions and no tenant specified
            return Err(AuthError::TenantRequired);
        }
    };
    
    // 3. Create and cache session
    let session = Session {
        id: SessionId::new(),
        user_id: system_user.id,
        principal: Arc::new(principal),
        created_at: Timestamp::now(),
        last_activity: Timestamp::now(),
        expires_at: Timestamp::now() + Duration::from_secs(3600),
    };
    
    session_cache.insert(session.id, session.clone());
    Ok(session)
}
```

#### Multi-Tenant Session Management

The session manager maintains separate principal caches for different tenant contexts:

```rust
pub struct SessionManager {
    // Primary session cache: SessionId -> Session
    sessions: RwLock<HashMap<SessionId, Session>>,
    
    // Multi-tenant principal cache: (SessionId, TenantId) -> SecurityPrincipal
    tenant_principals: RwLock<HashMap<(SessionId, TenantId), Arc<SecurityPrincipal>>>,
}

impl SessionManager {
    /// Get or create tenant principal for a session
    pub async fn get_tenant_principal(
        &self,
        session_id: SessionId,
        tenant_id: TenantId,
    ) -> Result<Arc<SecurityPrincipal>> {
        // Check cache first
        {
            let cache = self.tenant_principals.read().unwrap();
            if let Some(principal) = cache.get(&(session_id, tenant_id)) {
                return Ok(principal.clone());
            }
        }
        
        // Create new principal for this tenant
        let session = self.get_session(session_id)?;
        let system_user = load_system_user(session.user_id).await?;
        let tenant_user = load_tenant_user(session.user_id, tenant_id).await?;
        let tenant_groups = load_tenant_groups(tenant_id, &tenant_user.groups).await?;
        let tenant_roles = load_tenant_roles(tenant_id, &tenant_user.roles).await?;
        
        let principal = Arc::new(SecurityPrincipal::from_tenant_user(
            &system_user,
            &tenant_user,
            &tenant_groups,
            &tenant_roles,
        ));
        
        // Cache for future use
        {
            let mut cache = self.tenant_principals.write().unwrap();
            cache.insert((session_id, tenant_id), principal.clone());
        }
        
        Ok(principal)
    }
    
    /// Invalidate all sessions for a user (when permissions change)
    pub fn invalidate_user_sessions(&self, user_id: UserId) {
        let mut sessions = self.sessions.write().unwrap();
        sessions.retain(|_, session| session.user_id != user_id);
        
        let mut tenant_principals = self.tenant_principals.write().unwrap();
        tenant_principals.retain(|(session_id, _), _| {
            sessions.get(session_id)
                .map(|s| s.user_id != user_id)
                .unwrap_or(false)
        });
    }
    
    /// Invalidate tenant principals when tenant permissions change
    pub fn invalidate_tenant_principals(&self, tenant_id: TenantId) {
        let mut tenant_principals = self.tenant_principals.write().unwrap();
        tenant_principals.retain(|(_, tid), _| *tid != tenant_id);
    }
}
```

#### Caching Strategy

**Principals are created once and cached, not recreated per request:**

1. **Initial Login**: Create principal for default/specified tenant → Cache in session
2. **Subsequent Requests**: Reuse cached principal from session
3. **Tenant Switch**: Create new principal for different tenant → Cache separately
4. **Multi-Tenant Access**: Session manager maintains `(SessionId, TenantId) -> Principal` mapping
5. **Invalidation**: Only when permissions change (user/group/role updates), not per-request

**Performance Benefits:**
- Permission resolution happens once at authentication
- No database lookups for groups/roles on every request
- Fast tenant switching with cached principals
- Minimal memory overhead (principals are immutable and shared via Arc)

#### Example: Multi-Tenant User Workflow

```rust
// 1. Alice logs in (consulting company employee)
let session = authenticate("alice", "password", None).await?;
// Creates system principal if Alice is admin, or requires tenant selection

// 2. Alice selects Client A's tenant
let client_a_principal = session_manager
    .get_tenant_principal(session.id, client_a_tenant_id)
    .await?;

// 3. Alice works with Client A's data
let table = manager.get_table(
    client_a_principal.clone(),
    container_id,
    table_id,
).await?;

// 4. Alice switches to Client B
let client_b_principal = session_manager
    .get_tenant_principal(session.id, client_b_tenant_id)
    .await?;

// 5. Both principals are cached - switching is instant
// No database queries needed for subsequent switches
```

#### Permission Change Handling

When permissions change, invalidate affected principals:

```rust
// User's permissions changed (e.g., added to new group)
async fn update_user_groups(user_id: UserId, new_groups: Vec<GroupId>) -> Result<()> {
    // Update database
    update_user_groups_in_db(user_id, new_groups).await?;
    
    // Invalidate all sessions for this user
    session_manager.invalidate_user_sessions(user_id);
    
    // User must re-authenticate to get new permissions
    Ok(())
}

// Tenant role permissions changed
async fn update_tenant_role(tenant_id: TenantId, role_id: RoleId, grants: Vec<PermissionGrant>) -> Result<()> {
    // Update database
    update_role_in_db(tenant_id, role_id, grants).await?;
    
    // Invalidate all tenant principals for this tenant
    session_manager.invalidate_tenant_principals(tenant_id);
    
    // Users in this tenant must switch context or re-authenticate
    Ok(())
}
```

                return principal.clone();
            }
        }
        
        // Create new principal
        let principal = Arc::new(SecurityPrincipal::from_user(user, groups, roles));
        cache.insert(session_id, principal.clone());
        principal
    }
    
    /// Invalidate a principal (e.g., on logout or permission change)
    pub fn invalidate(&self, session_id: SessionId) {
        self.principals.write().unwrap().remove(&session_id);
    }
    
    /// Invalidate all principals for a user (e.g., when permissions change)
    pub fn invalidate_user(&self, user_id: UserId) {
        let mut cache = self.principals.write().unwrap();
        cache.retain(|_, principal| principal.user_id != user_id);
    }
}
```

#### Session Management

```rust
pub struct Session {
    pub id: SessionId,
    pub principal: Arc<SecurityPrincipal>,
    pub created_at: Timestamp,
    pub last_activity: Timestamp,
}

impl KeyValueDatabaseContext {
    /// Authenticate user and create session with principal
    pub async fn authenticate(
        &self,
        username: &str,
        password: &str,
    ) -> KeyValueResult<Session> {
        // 1. Verify credentials
        let user = self.verify_credentials(username, password)?;
        
        // 2. Load groups and roles
        let groups = self.load_user_groups(&user)?;
        let roles = self.load_user_roles(&user)?;
        
        // 3. Create security principal
        let principal = Arc::new(SecurityPrincipal::from_user(&user, &groups, &roles));
        
        // 4. Create session
        let session = Session {
            id: SessionId::new(),
            principal,
            created_at: Timestamp::now(),
            last_activity: Timestamp::now(),
        };
        
        // 5. Cache session
        self.session_cache.insert(session.id, session.clone());
        
        Ok(session)
    }
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
- [SecurityPrincipal Implementation Guide](DEV/SECURITY_PRINCIPAL_IMPLEMENTATION_GUIDE.md) - Step-by-step guide for implementing SecurityPrincipal throughout the codebase
- [SecurityPrincipal Tenant Strategy](DEV/SECURITY_PRINCIPAL_TENANT_STRATEGY.md) - Detailed authentication flow, session management, and multi-tenant caching strategy
- [Tenant Security Storage Strategy](DEV/TENANT_SECURITY_STORAGE_STRATEGY.md) - Storage location, key patterns, and caching for tenant security metadata