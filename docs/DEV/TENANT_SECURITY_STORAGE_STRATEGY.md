# Tenant Security Metadata Storage Strategy

## Overview

This document explains where and how tenant users, groups, and roles are stored in Nanograph's three-tier architecture, and why no new abstraction level is needed.

## Storage Location: System Metadata Cache (Tier 1)

### Current Implementation

Tenant security metadata is **already stored in the System Metadata Cache** alongside system-level security metadata:

```rust
// From nanograph-kvm/src/cache/system.rs
pub struct SystemMetadataCache {
    shard: ShardId,  // system_shard
    
    // System-level security (for administrators)
    system_users: HashMap<UserId, SystemUserRecord>,
    system_roles: HashMap<SystemRoleId, SystemRoleRecord>,
    system_groups: HashMap<SystemGroupId, SystemGroupRecord>,
    
    // Tenant-level security (for regular users)
    tenant_users: HashMap<(TenantId, UserId), TenantUserRecord>,
    tenant_roles: HashMap<(TenantId, TenantRoleId), TenantRoleRecord>,
    tenant_groups: HashMap<(TenantId, TenantGroupId), TenantGroupRecord>,
    
    // Other system metadata
    cluster: Option<ClusterRecord>,
    regions: HashMap<RegionId, RegionRecord>,
    servers: HashMap<ServerId, ServerRecord>,
    tenants: HashMap<TenantId, TenantRecord>,
    databases: HashMap<DatabaseId, DatabaseRecord>,
    tablespaces: HashMap<TablespaceId, TablespaceRecord>,
}
```

### Why Tier 1 (System Metadata)?

**Tenant security metadata belongs in Tier 1 because:**

1. **Cross-Database Scope**: Users can access multiple databases within a tenant
2. **Authentication Requirement**: Must be available before database access
3. **Low Update Frequency**: User permissions change infrequently (minutes to hours)
4. **Small Data Size**: Even with thousands of users, metadata is KB to low MB
5. **Cluster-Wide Visibility**: System needs to know all users for authentication

## Three-Tier Architecture Review

### Tier 1: System Metadata Raft Group ✅ (Security Metadata Here)

**Scope**: Cluster-wide (1 group per cluster)

**Shard**: `system_shard` (ShardId::new(0))

**Data Managed**:
- ✅ **System Users**: Global administrators
- ✅ **System Groups**: Administrator groups
- ✅ **System Roles**: Administrator roles
- ✅ **Tenant Users**: Per-tenant user overlays
- ✅ **Tenant Groups**: Per-tenant organizational groups
- ✅ **Tenant Roles**: Per-tenant permission roles
- Cluster configuration
- Regions, Servers
- Tenants
- Database registry (references)

**Characteristics**:
- Update Frequency: Low (minutes to hours)
- Data Size: Small (KB to MB)
- Replication: Fully replicated
- Consistency: Strong (linearizable)

### Tier 2: Database Metadata Raft Groups ❌ (Not for Security)

**Scope**: Per-database (N groups)

**Shard**: `metadata_shard` per database

**Data Managed**:
- Namespaces
- Tables
- Shard metadata
- ❌ **NOT user/group/role data** (that's in Tier 1)

**Why NOT here?**
- Users need to authenticate BEFORE accessing a database
- Users can access multiple databases
- Would require duplicating user data across databases
- Permission changes would need to propagate to all databases

### Tier 3: Data Shard Raft Groups ❌ (Not for Metadata)

**Scope**: Per-shard (M groups)

**Data Managed**:
- User application data only
- No metadata

## Storage Key Patterns

### Physical Storage in system_shard

All security metadata is stored in the `system_shard` with composite keys:

```rust
// System-level (administrators)
"system:user:{user_id}"              -> SystemUserRecord
"system:group:{group_id}"            -> SystemGroupRecord
"system:role:{role_id}"              -> SystemRoleRecord

// Tenant-level (regular users)
"tenant:{tenant_id}:user:{user_id}"  -> TenantUserRecord
"tenant:{tenant_id}:group:{group_id}" -> TenantGroupRecord
"tenant:{tenant_id}:role:{role_id}"  -> TenantRoleRecord

// Indexes for efficient lookups
"user:{user_id}:tenants"             -> Vec<TenantId>
"tenant:{tenant_id}:users"           -> Vec<UserId>
"tenant:{tenant_id}:group:{group_id}:members" -> Vec<UserId>
```

### Benefits of This Pattern

1. **Natural Isolation**: Tenant prefix prevents cross-tenant access
2. **Efficient Queries**: Can list all users in a tenant with prefix scan
3. **Single Source of Truth**: All security metadata in one place
4. **Atomic Updates**: Can update user permissions atomically
5. **Consistent Caching**: Single cache invalidation point

## Authentication Flow with Storage

### 1. User Login

```rust
async fn authenticate(username: &str, password: &str, tenant_id: Option<TenantId>) 
    -> Result<Session> 
{
    // 1. Load from system_shard (Tier 1)
    let system_user = load_from_system_shard(
        format!("system:user:{}", username)
    ).await?;
    
    // 2. Verify credentials
    verify_password(&system_user, password)?;
    
    // 3. If tenant specified, load tenant overlay
    let principal = if let Some(tid) = tenant_id {
        // Load tenant-specific data from system_shard
        let tenant_user = load_from_system_shard(
            format!("tenant:{}:user:{}", tid, system_user.id)
        ).await?;
        
        let tenant_groups = load_tenant_groups_from_system_shard(
            tid, &tenant_user.groups
        ).await?;
        
        let tenant_roles = load_tenant_roles_from_system_shard(
            tid, &tenant_user.roles
        ).await?;
        
        SecurityPrincipal::from_tenant_user(
            &system_user,
            &tenant_user,
            &tenant_groups,
            &tenant_roles,
        )
    } else {
        // System principal for administrators
        let system_groups = load_system_groups_from_system_shard(
            &system_user.groups
        ).await?;
        
        let system_roles = load_system_roles_from_system_shard(
            &system_user.roles
        ).await?;
        
        SecurityPrincipal::from_system_user(
            &system_user,
            &system_groups,
            &system_roles,
        )
    };
    
    // 4. Create session with cached principal
    Ok(Session {
        id: SessionId::new(),
        principal: Arc::new(principal),
        // ...
    })
}
```

### 2. Loading from System Shard

```rust
impl SystemMetadataCache {
    /// Load tenant user from system_shard
    pub async fn load_tenant_user(
        &self,
        tenant_id: TenantId,
        user_id: UserId,
    ) -> Result<TenantUserRecord> {
        // Check cache first
        if let Some(record) = self.tenant_users.get(&(tenant_id, user_id)) {
            return Ok(record.clone());
        }
        
        // Load from system_shard storage
        let key = format!("tenant:{}:user:{}", tenant_id, user_id);
        let record = self.shard_manager
            .get(&self.shard, key.as_bytes())
            .await?
            .ok_or(Error::UserNotFound)?;
        
        let tenant_user: TenantUserRecord = deserialize(&record)?;
        
        // Cache for future use
        self.tenant_users.insert((tenant_id, user_id), tenant_user.clone());
        
        Ok(tenant_user)
    }
    
    /// Load tenant groups from system_shard
    pub async fn load_tenant_groups(
        &self,
        tenant_id: TenantId,
        group_ids: &[TenantGroupId],
    ) -> Result<Vec<TenantGroupRecord>> {
        let mut groups = Vec::new();
        
        for group_id in group_ids {
            // Check cache
            if let Some(record) = self.tenant_groups.get(&(tenant_id, *group_id)) {
                groups.push(record.clone());
                continue;
            }
            
            // Load from system_shard
            let key = format!("tenant:{}:group:{}", tenant_id, group_id.0);
            let record = self.shard_manager
                .get(&self.shard, key.as_bytes())
                .await?
                .ok_or(Error::GroupNotFound)?;
            
            let group: TenantGroupRecord = deserialize(&record)?;
            
            // Cache
            self.tenant_groups.insert((tenant_id, *group_id), group.clone());
            groups.push(group);
        }
        
        Ok(groups)
    }
    
    /// Similar methods for tenant_roles, system_users, system_groups, system_roles
}
```

## Caching Strategy

### Two-Level Cache

1. **System Metadata Cache** (in-memory, per-node)
   - Caches all security metadata from system_shard
   - Invalidated when permissions change
   - Shared across all databases on the node

2. **Session Principal Cache** (in-memory, per-session)
   - Caches resolved SecurityPrincipal objects
   - Invalidated when session expires or permissions change
   - Per-user, per-tenant

```rust
pub struct CachingStrategy {
    // Level 1: System metadata cache (shared)
    system_cache: Arc<RwLock<SystemMetadataCache>>,
    
    // Level 2: Session principal cache (per-session)
    session_principals: Arc<RwLock<HashMap<SessionId, Arc<SecurityPrincipal>>>>,
    tenant_principals: Arc<RwLock<HashMap<(SessionId, TenantId), Arc<SecurityPrincipal>>>>,
}
```

### Cache Invalidation

```rust
impl SystemMetadataCache {
    /// Invalidate user cache when permissions change
    pub fn invalidate_user(&mut self, user_id: UserId) {
        // Remove from system cache
        self.system_users.remove(&user_id);
        
        // Remove all tenant overlays for this user
        self.tenant_users.retain(|(_, uid), _| *uid != user_id);
    }
    
    /// Invalidate tenant cache when tenant permissions change
    pub fn invalidate_tenant(&mut self, tenant_id: TenantId) {
        // Remove all tenant-specific data
        self.tenant_users.retain(|(tid, _), _| *tid != tenant_id);
        self.tenant_groups.retain(|(tid, _), _| *tid != tenant_id);
        self.tenant_roles.retain(|(tid, _), _| *tid != tenant_id);
    }
    
    /// Invalidate group when group membership changes
    pub fn invalidate_group(&mut self, tenant_id: TenantId, group_id: TenantGroupId) {
        self.tenant_groups.remove(&(tenant_id, group_id));
    }
}
```

## Why No New Abstraction Level is Needed

### Current Architecture is Sufficient

✅ **System Metadata Cache already exists** - No new structure needed

✅ **Composite keys provide isolation** - `(TenantId, UserId)` naturally separates tenants

✅ **Single storage location** - All security metadata in system_shard

✅ **Efficient caching** - HashMap lookups are O(1)

✅ **Clear ownership** - Tier 1 owns all authentication/authorization data

### What Would a New Level Add?

❌ **Complexity**: Additional abstraction without clear benefit

❌ **Performance**: Extra indirection would slow lookups

❌ **Maintenance**: More code to maintain and test

❌ **Confusion**: Unclear where to look for security data

### Alternative Considered: Per-Tenant Metadata Shards

**Rejected because:**
- Would require N shards for N tenants (resource intensive)
- Authentication would need to query multiple shards
- Cross-tenant operations (admin tasks) would be complex
- Tenant creation/deletion would require shard lifecycle management
- No performance benefit (security metadata is small)

## Implementation Checklist

- [x] SystemMetadataCache structure supports tenant security metadata
- [x] Composite key pattern defined: `(TenantId, UserId)`, etc.
- [x] SecurityPrincipal supports tenant-scoped creation
- [ ] Implement load methods in SystemMetadataCache:
  - [ ] `load_tenant_user()`
  - [ ] `load_tenant_groups()`
  - [ ] `load_tenant_roles()`
  - [ ] `load_system_user()`
  - [ ] `load_system_groups()`
  - [ ] `load_system_roles()`
- [ ] Implement cache invalidation methods
- [ ] Add authentication flow using system_shard
- [ ] Add session management with principal caching
- [ ] Add tests for multi-tenant security

## Summary

**Storage Location**: System Metadata Cache (Tier 1) in `system_shard`

**Key Pattern**: Composite keys with tenant prefix for isolation

**No New Abstraction**: Current architecture is sufficient and optimal

**Benefits**:
- Single source of truth for all security metadata
- Natural tenant isolation via key prefixes
- Efficient caching with HashMap
- Simple invalidation strategy
- Minimal storage overhead
- Fast authentication (single shard lookup)

The existing three-tier architecture already provides the perfect home for tenant security metadata in Tier 1, with no need for additional abstraction levels.