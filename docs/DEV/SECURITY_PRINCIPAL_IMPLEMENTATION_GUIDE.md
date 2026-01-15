# SecurityPrincipal Implementation Guide

## Overview

This guide provides step-by-step instructions for implementing the `SecurityPrincipal` abstraction throughout the Nanograph codebase. The SecurityPrincipal separates authentication/authorization from metadata storage, providing better performance, cleaner code, and improved security.

## Architecture Changes

### Before: Permission Checks Against Metadata

```rust
// OLD APPROACH - Inefficient
impl KeyValueDatabaseContext {
    pub async fn put(
        &self,
        user: &UserMetadata,           // Metadata object
        container: &ContainerId,
        table: &TableId,
        key: &[u8],
        value: &[u8],
    ) -> KeyValueResult<()> {
        // Load groups and roles on EVERY operation
        let groups = self.load_user_groups(user)?;
        let roles = self.load_user_roles(user)?;
        
        // Check permission
        if !user.has_table_permission(&Permission::TableWrite, *table, db_id, tenant_id, &groups, &roles) {
            return Err(KeyValueError::PermissionDenied { ... });
        }
        
        // Perform operation
        // ...
    }
}
```

### After: Permission Checks Against Principal

```rust
// NEW APPROACH - Efficient
impl KeyValueDatabaseContext {
    pub async fn put(
        &self,
        principal: &SecurityPrincipal,  // Security principal
        container: &ContainerId,
        table: &TableId,
        key: &[u8],
        value: &[u8],
    ) -> KeyValueResult<()> {
        // Get table metadata to resolve IDs
        let table_meta = self.get_table_metadata(container, table)?;
        
        // Check permission (no loading needed - already resolved)
        if !principal.has_table_permission(
            &Permission::TableWrite,
            *table,
            table_meta.database_id,
            table_meta.tenant_id
        ) {
            return Err(KeyValueError::PermissionDenied { ... });
        }
        
        // Perform operation
        // ...
    }
}
```

## Implementation Steps

### Phase 1: Core Types (✅ COMPLETED)

- [x] Create `SecurityPrincipal` struct in `nanograph-core/src/object/security.rs`
- [x] Implement `from_user()` method to resolve permissions
- [x] Implement permission checking methods
- [x] Export `SecurityPrincipal` from `nanograph-core/src/object.rs`
- [x] Update `SECURITY_MODEL.md` documentation

### Phase 2: Context Layer Updates

#### 2.1 Update KeyValueDatabaseContext Signature

**File:** `nanograph-kvm/src/context.rs`

Change all public methods to accept `&SecurityPrincipal` instead of `&UserMetadata`:

```rust
// Before
pub async fn create_database(
    &self,
    user: &UserMetadata,
    tenant: &TenantId,
    config: DatabaseCreate,
) -> KeyValueResult<DatabaseMetadata>

// After
pub async fn create_database(
    &self,
    principal: &SecurityPrincipal,
    tenant: &TenantId,
    config: DatabaseCreate,
) -> KeyValueResult<DatabaseMetadata>
```

Methods to update:
- `create_database()`
- `delete_database()`
- `create_table()`
- `delete_table()`
- `put()`
- `get()`
- `delete()`
- `scan()`
- All other operations

#### 2.2 Simplify Permission Checking

Remove the helper methods that load groups/roles:

```rust
// REMOVE these methods
fn load_user_groups(&self, user: &UserMetadata) -> KeyValueResult<Vec<GroupMetadata>>
fn load_user_roles(&self, user: &UserMetadata) -> KeyValueResult<Vec<RoleMetadata>>
```

Simplify permission checks:

```rust
// Before
fn check_permission(
    &self,
    user: &UserMetadata,
    permission: &Permission,
    scope: ResourceScope,
) -> KeyValueResult<()> {
    let groups = self.load_user_groups(user)?;
    let roles = self.load_user_roles(user)?;
    let grants = user.effective_grants(&groups, &roles);
    // ... complex logic
}

// After
fn check_table_permission(
    &self,
    principal: &SecurityPrincipal,
    permission: &Permission,
    table_id: TableId,
    database_id: DatabaseId,
    tenant_id: TenantId,
) -> KeyValueResult<()> {
    if !principal.has_table_permission(permission, table_id, database_id, tenant_id) {
        return Err(KeyValueError::PermissionDenied {
            user_id: principal.user_id,
            permission: permission.clone(),
            resource: format!("Table({})", table_id),
        });
    }
    Ok(())
}
```

### Phase 3: API Handle Updates

#### 3.1 Update ContainerHandle

**File:** `nanograph-kvm/src/container.rs`

```rust
// Before
pub struct ContainerHandle {
    container_id: ContainerId,
    user: UserMetadata,
    context: Arc<KeyValueDatabaseContext>,
    metadata_cache: Arc<RwLock<ContainerMetadataCache>>,
}

// After
pub struct ContainerHandle {
    container_id: ContainerId,
    principal: Arc<SecurityPrincipal>,  // Changed from UserMetadata
    context: Arc<KeyValueDatabaseContext>,
    metadata_cache: Arc<RwLock<ContainerMetadataCache>>,
}

impl ContainerHandle {
    // Update constructor
    pub fn new(
        container_id: ContainerId,
        principal: Arc<SecurityPrincipal>,  // Changed parameter
        context: Arc<KeyValueDatabaseContext>,
    ) -> Self {
        Self {
            container_id,
            principal,
            context,
            metadata_cache: Arc::new(RwLock::new(ContainerMetadataCache::new())),
        }
    }
    
    // Update all methods to pass principal
    pub async fn create_table(&self, config: TableCreate) -> KeyValueResult<TableId> {
        self.context.create_table(
            &self.principal,  // Changed from &self.user
            &self.container_id,
            config
        ).await
    }
}
```

#### 3.2 Update TableHandle

**File:** `nanograph-kvm/src/table.rs`

```rust
// Before
pub struct TableHandle {
    container_id: ContainerId,
    table_id: TableId,
    user: UserMetadata,
    context: Arc<KeyValueDatabaseContext>,
}

// After
pub struct TableHandle {
    container_id: ContainerId,
    table_id: TableId,
    principal: Arc<SecurityPrincipal>,  // Changed from UserMetadata
    context: Arc<KeyValueDatabaseContext>,
}

impl TableHandle {
    // Update constructor
    pub fn new(
        container_id: ContainerId,
        table_id: TableId,
        principal: Arc<SecurityPrincipal>,  // Changed parameter
        context: Arc<KeyValueDatabaseContext>,
    ) -> Self {
        Self {
            container_id,
            table_id,
            principal,
            context,
        }
    }
    
    // Update all methods
    pub async fn put(&self, key: &[u8], value: &[u8]) -> KeyValueResult<()> {
        self.context.put(
            &self.principal,  // Changed from &self.user
            &self.container_id,
            &self.table_id,
            key,
            value
        ).await
    }
}
```

#### 3.3 Update KeyValueDatabaseManager

**File:** `nanograph-kvm/src/manager.rs`

```rust
impl KeyValueDatabaseManager {
    // Update methods that create handles
    pub async fn get_container(
        &self,
        principal: Arc<SecurityPrincipal>,  // Changed from UserMetadata
        container_id: ContainerId,
    ) -> KeyValueResult<ContainerHandle> {
        ContainerHandle::new(
            container_id,
            principal,  // Pass principal
            self.context.clone(),
        )
    }
    
    pub async fn get_table(
        &self,
        principal: Arc<SecurityPrincipal>,  // Changed from UserMetadata
        container_id: ContainerId,
        table_id: TableId,
    ) -> KeyValueResult<TableHandle> {
        TableHandle::new(
            container_id,
            table_id,
            principal,  // Pass principal
            self.context.clone(),
        )
    }
}
```

### Phase 4: Authentication and Session Management

#### 4.1 Add Session Types

**File:** `nanograph-core/src/types.rs`

```rust
/// Session identifier
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash, Serialize, Deserialize)]
pub struct SessionId(pub u128);

impl SessionId {
    pub fn new() -> Self {
        use std::time::{SystemTime, UNIX_EPOCH};
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        Self(nanos)
    }
}

/// Active user session
#[derive(Clone, Debug)]
pub struct Session {
    pub id: SessionId,
    pub principal: Arc<SecurityPrincipal>,
    pub created_at: Timestamp,
    pub last_activity: Timestamp,
    pub expires_at: Timestamp,
}
```

#### 4.2 Add Authentication Methods

**File:** `nanograph-kvm/src/context.rs`

```rust
impl KeyValueDatabaseContext {
    /// Authenticate user and create session with security principal
    pub async fn authenticate(
        &self,
        username: &str,
        password: &str,
    ) -> KeyValueResult<Session> {
        // 1. Verify credentials
        let user = self.verify_credentials(username, password).await?;
        
        if !user.enabled {
            return Err(KeyValueError::AuthenticationFailed {
                reason: "User account is disabled".to_string(),
            });
        }
        
        // 2. Load groups and roles from system metadata cache
        let groups = self.load_user_groups_for_auth(&user).await?;
        let roles = self.load_user_roles_for_auth(&user).await?;
        
        // 3. Create security principal with resolved permissions
        let principal = Arc::new(SecurityPrincipal::from_user(&user, &groups, &roles));
        
        // 4. Create session
        let now = Timestamp::now();
        let session = Session {
            id: SessionId::new(),
            principal,
            created_at: now,
            last_activity: now,
            expires_at: now + Duration::from_secs(3600), // 1 hour
        };
        
        // 5. Cache session
        self.session_cache.write().unwrap().insert(session.id, session.clone());
        
        // 6. Log authentication
        tracing::info!(
            user_id = %user.id,
            username = %user.username,
            session_id = ?session.id,
            "User authenticated successfully"
        );
        
        Ok(session)
    }
    
    /// Get session by ID
    pub fn get_session(&self, session_id: SessionId) -> KeyValueResult<Session> {
        let cache = self.session_cache.read().unwrap();
        cache.get(&session_id)
            .cloned()
            .ok_or_else(|| KeyValueError::SessionNotFound { session_id })
    }
    
    /// Invalidate session (logout)
    pub fn invalidate_session(&self, session_id: SessionId) -> KeyValueResult<()> {
        self.session_cache.write().unwrap().remove(&session_id);
        tracing::info!(session_id = ?session_id, "Session invalidated");
        Ok(())
    }
    
    /// Invalidate all sessions for a user (e.g., when permissions change)
    pub fn invalidate_user_sessions(&self, user_id: UserId) -> KeyValueResult<()> {
        let mut cache = self.session_cache.write().unwrap();
        cache.retain(|_, session| session.principal.user_id != user_id);
        tracing::info!(user_id = %user_id, "All user sessions invalidated");
        Ok(())
    }
}
```

### Phase 5: Testing

#### 5.1 Unit Tests

**File:** `nanograph-core/src/object/security.rs`

```rust
#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_security_principal_creation() {
        let user = create_test_user();
        let groups = vec![create_test_group()];
        let roles = vec![create_test_role()];
        
        let principal = SecurityPrincipal::from_user(&user, &groups, &roles);
        
        assert_eq!(principal.user_id, user.id);
        assert_eq!(principal.username, user.username);
        assert!(!principal.grants().is_empty());
    }
    
    #[test]
    fn test_principal_permission_checks() {
        let principal = create_test_principal_with_table_read();
        
        assert!(principal.has_table_permission(
            &Permission::TableRead,
            TableId::new(1),
            DatabaseId::new(1),
            TenantId::new(1)
        ));
        
        assert!(!principal.has_table_permission(
            &Permission::TableWrite,
            TableId::new(1),
            DatabaseId::new(1),
            TenantId::new(1)
        ));
    }
    
    #[test]
    fn test_superuser_principal() {
        let principal = create_superuser_principal();
        
        assert!(principal.is_superuser());
        assert!(principal.has_table_permission(
            &Permission::TableWrite,
            TableId::new(999),
            DatabaseId::new(999),
            TenantId::new(999)
        ));
    }
}
```

#### 5.2 Integration Tests

**File:** `nanograph-kvm/tests/security_principal_tests.rs`

```rust
#[tokio::test]
async fn test_authentication_creates_principal() {
    let context = create_test_context().await;
    
    // Create test user
    let user = create_test_user_in_db(&context).await;
    
    // Authenticate
    let session = context.authenticate(&user.username, "password").await.unwrap();
    
    // Verify principal was created
    assert_eq!(session.principal.user_id, user.id);
    assert!(!session.principal.grants().is_empty());
}

#[tokio::test]
async fn test_permission_enforcement_with_principal() {
    let context = create_test_context().await;
    
    // Create user with limited permissions
    let user = create_limited_user(&context).await;
    let session = context.authenticate(&user.username, "password").await.unwrap();
    
    // Try to create table (should succeed)
    let result = context.create_table(
        &session.principal,
        &ContainerId::new(1),
        TableCreate::new("test_table", "/", StorageEngineType::from("lsm"))
    ).await;
    assert!(result.is_ok());
    
    // Try to delete database (should fail)
    let result = context.delete_database(
        &session.principal,
        &DatabaseId::new(1)
    ).await;
    assert!(matches!(result, Err(KeyValueError::PermissionDenied { .. })));
}
```

## Migration Checklist

- [ ] Phase 1: Core Types (✅ COMPLETED)
- [ ] Phase 2: Context Layer
  - [ ] Update `KeyValueDatabaseContext` method signatures
  - [ ] Simplify permission checking logic
  - [ ] Remove group/role loading helpers
- [ ] Phase 3: API Handles
  - [ ] Update `ContainerHandle`
  - [ ] Update `TableHandle`
  - [ ] Update `KeyValueDatabaseManager`
- [ ] Phase 4: Authentication
  - [ ] Add `Session` and `SessionId` types
  - [ ] Implement `authenticate()` method
  - [ ] Implement session management
  - [ ] Add session caching
- [ ] Phase 5: Testing
  - [ ] Add unit tests for `SecurityPrincipal`
  - [ ] Add integration tests
  - [ ] Update existing tests
- [ ] Phase 6: Documentation
  - [ ] Update API documentation
  - [ ] Update examples
  - [ ] Update README files

## Benefits Summary

1. **Performance**: Permissions resolved once at authentication, not on every operation
2. **Simplicity**: Cleaner code with fewer parameters and no repeated lookups
3. **Security**: Clear separation between metadata and security enforcement
4. **Auditability**: Immutable snapshot of permissions at authentication time
5. **Thread Safety**: Immutable principals can be safely shared across threads

## Next Steps

1. Begin Phase 2: Update `KeyValueDatabaseContext`
2. Create feature branch: `feature/security-principal-implementation`
3. Implement changes incrementally with tests
4. Review and merge when complete