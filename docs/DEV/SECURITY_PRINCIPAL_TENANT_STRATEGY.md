# SecurityPrincipal Tenant Strategy

## Overview

This document explains how the `SecurityPrincipal` handles multi-tenant authentication, session management, and tenant context switching in Nanograph.

## Authentication Flow

### 1. Initial Login

When a user logs in, the system follows this flow:

```rust
// User provides credentials
async fn authenticate(username: &str, password: &str, tenant_id: Option<TenantId>) 
    -> Result<Session> 
{
    // 1. Verify credentials and load SystemUserRecord
    let system_user = verify_credentials(username, password).await?;
    
    if !system_user.enabled {
        return Err(AuthError::AccountDisabled);
    }
    
    // 2. Determine which principal to create
    let principal = if let Some(tenant_id) = tenant_id {
        // User specified a tenant - create tenant-scoped principal
        create_tenant_principal(&system_user, tenant_id).await?
    } else if !system_user.groups.is_empty() || !system_user.roles.is_empty() {
        // User has system-level permissions - create system principal
        create_system_principal(&system_user).await?
    } else {
        // User has no system permissions - need to specify tenant
        return Err(AuthError::TenantRequired);
    };
    
    // 3. Create session with the principal
    let session = Session {
        id: SessionId::new(),
        user_id: system_user.id,
        principal: Arc::new(principal),
        created_at: Timestamp::now(),
        last_activity: Timestamp::now(),
        expires_at: Timestamp::now() + Duration::from_secs(3600),
    };
    
    // 4. Cache the session
    session_cache.insert(session.id, session.clone());
    
    Ok(session)
}
```

### 2. Creating Principals

#### System Principal (Administrators)

```rust
async fn create_system_principal(system_user: &SystemUserRecord) 
    -> Result<SecurityPrincipal> 
{
    // Load system groups and roles
    let system_groups = load_system_groups(&system_user.groups).await?;
    let system_roles = load_system_roles(&system_user.roles).await?;
    
    // Create system-level principal
    Ok(SecurityPrincipal::from_system_user(
        system_user,
        &system_groups,
        &system_roles,
    ))
}
```

#### Tenant Principal (Regular Users)

```rust
async fn create_tenant_principal(
    system_user: &SystemUserRecord, 
    tenant_id: TenantId
) -> Result<SecurityPrincipal> 
{
    // Load tenant user record
    let tenant_user = load_tenant_user(system_user.id, tenant_id).await?;
    
    // Load tenant groups and roles
    let tenant_groups = load_tenant_groups(tenant_id, &tenant_user.groups).await?;
    let tenant_roles = load_tenant_roles(tenant_id, &tenant_user.roles).await?;
    
    // Create tenant-scoped principal
    Ok(SecurityPrincipal::from_tenant_user(
        system_user,
        &tenant_user,
        &tenant_groups,
        &tenant_roles,
    ))
}
```

## Session Management

### Session Structure

```rust
pub struct Session {
    pub id: SessionId,
    pub user_id: UserId,
    pub principal: Arc<SecurityPrincipal>,
    pub created_at: Timestamp,
    pub last_activity: Timestamp,
    pub expires_at: Timestamp,
}

pub struct SessionManager {
    // Primary session cache: SessionId -> Session
    sessions: RwLock<HashMap<SessionId, Session>>,
    
    // Multi-tenant principal cache: (SessionId, TenantId) -> SecurityPrincipal
    tenant_principals: RwLock<HashMap<(SessionId, TenantId), Arc<SecurityPrincipal>>>,
}
```

### Caching Strategy

**Principals are created once and cached, not recreated on every request:**

1. **Initial Login**: Create principal for default tenant or system context
2. **Cache in Session**: Store principal in session object
3. **Tenant Switch**: Create new principal for different tenant, cache separately
4. **Reuse**: All subsequent requests use cached principal

```rust
impl SessionManager {
    /// Get or create tenant principal for a session
    pub async fn get_tenant_principal(
        &self,
        session_id: SessionId,
        tenant_id: TenantId,
    ) -> Result<Arc<SecurityPrincipal>> {
        // Check if we already have a principal for this tenant
        {
            let cache = self.tenant_principals.read().unwrap();
            if let Some(principal) = cache.get(&(session_id, tenant_id)) {
                return Ok(principal.clone());
            }
        }
        
        // Need to create new principal for this tenant
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
        
        // Cache the new principal
        {
            let mut cache = self.tenant_principals.write().unwrap();
            cache.insert((session_id, tenant_id), principal.clone());
        }
        
        Ok(principal)
    }
}
```

## Tenant Context Switching

### Use Case: User Accesses Multiple Tenants

Alice works for a consulting company and has access to multiple client tenants:

```rust
// Alice logs in (no tenant specified, gets system principal if she's an admin)
let session = authenticate("alice", "password", None).await?;

// Alice wants to work with Client A's data
let client_a_principal = session_manager
    .get_tenant_principal(session.id, client_a_tenant_id)
    .await?;

// Use client_a_principal for all operations on Client A's data
let table_handle = manager.get_table(
    client_a_principal,
    container_id,
    table_id,
).await?;

// Later, Alice switches to Client B
let client_b_principal = session_manager
    .get_tenant_principal(session.id, client_b_tenant_id)
    .await?;

// Use client_b_principal for Client B's data
// The principals are cached, so switching is fast
```

### Explicit Tenant Switching

```rust
impl Session {
    /// Switch to a different tenant context
    pub async fn switch_tenant(
        &mut self,
        tenant_id: TenantId,
        session_manager: &SessionManager,
    ) -> Result<()> {
        // Get or create principal for the new tenant
        let new_principal = session_manager
            .get_tenant_principal(self.id, tenant_id)
            .await?;
        
        // Update the session's active principal
        self.principal = new_principal;
        self.last_activity = Timestamp::now();
        
        Ok(())
    }
}
```

## Permission Invalidation

When permissions change, invalidate cached principals:

```rust
impl SessionManager {
    /// Invalidate all sessions for a user (e.g., when permissions change)
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

## Best Practices

### 1. Default Tenant Selection

```rust
// Option A: User specifies tenant at login
authenticate("alice", "password", Some(tenant_a_id)).await?

// Option B: Use user's default tenant from profile
let default_tenant = system_user.metadata.get("default_tenant");
authenticate_with_default("alice", "password").await?

// Option C: Require tenant for non-admin users
if system_user.is_admin() {
    create_system_principal(&system_user).await?
} else {
    return Err(AuthError::TenantRequired);
}
```

### 2. API Design

```rust
// API endpoints should accept tenant context
POST /api/v1/auth/login
{
    "username": "alice",
    "password": "...",
    "tenant_id": "tenant-123"  // Optional
}

// Or use tenant in URL path
GET /api/v1/tenants/{tenant_id}/databases
Authorization: Bearer {session_token}

// Or use tenant switching endpoint
POST /api/v1/auth/switch-tenant
{
    "tenant_id": "tenant-456"
}
```

### 3. Performance Optimization

```rust
// Cache principals aggressively
// - Session lifetime: 1 hour (configurable)
// - Principal cache: Until session expires or permissions change
// - Tenant principal cache: Per-session, per-tenant

// Lazy loading: Only load tenant principals when needed
// Don't pre-load all tenant principals at login

// Background refresh: Optionally refresh principals before expiry
async fn refresh_principal_if_needed(session: &Session) {
    if session.expires_at - Timestamp::now() < Duration::from_secs(300) {
        // Refresh principal in background
        tokio::spawn(async move {
            refresh_session_principal(session.id).await;
        });
    }
}
```

## Summary

**Key Points:**

1. **Principals are created once per tenant context and cached**
2. **Not recreated on every request** - that would be inefficient
3. **Tenant switching creates new principals** but caches them
4. **Session manager tracks multiple tenant contexts** per user
5. **Invalidation happens when permissions change**, not on every request

This design provides:
- **Performance**: Permissions resolved once, cached for session lifetime
- **Flexibility**: Users can access multiple tenants
- **Security**: Each tenant context has isolated permissions
- **Auditability**: Clear snapshot of permissions at authentication time