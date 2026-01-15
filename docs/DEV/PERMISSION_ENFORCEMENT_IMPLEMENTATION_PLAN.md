# Permission Enforcement Implementation Plan

**Status:** Planning  
**Priority:** High  
**Estimated Effort:** 2-3 weeks  
**Target Completion:** TBD

## Overview

This document outlines the implementation plan for adding permission enforcement to the Nanograph key-value database layer. The implementation will add security checks to `KeyValueDatabaseContext` to enforce the permission model defined in `docs/SECURITY_MODEL.md`.

## Goals

1. Enforce permissions at a single point (KeyValueDatabaseContext)
2. Support multi-user concurrent access safely
3. Maintain performance with permission caching
4. Provide comprehensive audit logging
5. Ensure backward compatibility where possible

## Architecture Summary

```
API Layer (Manager/Container/Table Handles)
    ↓ (passes UserMetadata per-operation)
KeyValueDatabaseContext
    ↓ (checks permissions before operations)
Storage Layer (no security logic)
```

See `docs/SECURITY_MODEL.md` for detailed architecture documentation.

## Implementation Phases

### Phase 1: Core Infrastructure (Week 1)

#### 1.1 Update Error Types

**File:** `nanograph-kvt/src/error.rs`

**Tasks:**
- [ ] Add `Unauthenticated` error variant
- [ ] Add `PermissionDenied` error variant with user_id, permission, resource fields
- [ ] Add `UserDisabled` error variant with user_id field
- [ ] Implement Display for new errors
- [ ] Add error conversion from security module
- [ ] Update error documentation

**Estimated Time:** 2 hours

#### 1.2 Add Permission Cache Structure

**File:** `nanograph-kvm/src/cache/permission.rs` (new file)

**Tasks:**
- [ ] Create `PermissionCache` struct with groups, roles, grants, timestamps
- [ ] Create `PermissionCacheManager` struct
- [ ] Implement cache get/put/invalidate methods
- [ ] Add TTL-based expiration logic
- [ ] Add cleanup_expired method
- [ ] Write unit tests for cache

**Estimated Time:** 4 hours

#### 1.3 Update KeyValueDatabaseContext Structure

**File:** `nanograph-kvm/src/context.rs`

**Tasks:**
- [ ] Add `permission_cache: Arc<PermissionCacheManager>` field
- [ ] Update `new_standalone` constructor
- [ ] Update `new_distributed` constructor
- [ ] Add configuration for cache TTL
- [ ] Update documentation

**Estimated Time:** 2 hours

### Phase 2: Permission Checking Logic (Week 1)

#### 2.1 Implement Permission Check Methods

**File:** `nanograph-kvm/src/context.rs`

**Tasks:**
- [ ] Implement `check_permission(user, permission, scope)` method
- [ ] Implement `load_user_permissions(user)` with caching
- [ ] Implement `load_user_groups(user)` method
- [ ] Implement `load_user_roles(user)` method
- [ ] Add `log_permission_check` for audit logging
- [ ] Write unit tests for permission checking

**Estimated Time:** 6 hours

#### 2.2 Update System Metadata Cache

**File:** `nanograph-kvm/src/cache/system.rs`

**Tasks:**
- [ ] Add `get_group(group_id)` method
- [ ] Add `get_role(role_id)` method
- [ ] Ensure groups/roles are cached on load
- [ ] Add cache invalidation on group/role updates
- [ ] Write tests for group/role retrieval

**Estimated Time:** 3 hours

### Phase 3: Update Context Methods (Week 2)

#### 3.1 Cluster Management Methods

**File:** `nanograph-kvm/src/context.rs`

**Tasks:**
- [ ] Update `initialize_cluster` - add user param, check SystemClusterManage
- [ ] Update `get_cluster` - add user param, check SystemClusterView
- [ ] Update `update_cluster` - add user param, check SystemClusterManage

**Estimated Time:** 1 hour

#### 3.2 Region Management Methods

**Tasks:**
- [ ] Update `get_regions` - add user param, check SystemClusterView
- [ ] Update `get_region` - add user param, check SystemClusterView
- [ ] Update `add_region` - add user param, check SystemRegionManage
- [ ] Update `update_region` - add user param, check SystemRegionManage
- [ ] Update `remove_region` - add user param, check SystemRegionManage

**Estimated Time:** 1.5 hours

#### 3.3 Server Management Methods

**Tasks:**
- [ ] Update `get_servers` - add user param, check SystemClusterView
- [ ] Update `get_server` - add user param, check SystemClusterView
- [ ] Update `add_server` - add user param, check SystemServerManage
- [ ] Update `update_server` - add user param, check SystemServerManage
- [ ] Update `remove_server` - add user param, check SystemServerManage

**Estimated Time:** 1.5 hours

#### 3.4 User Management Methods

**Tasks:**
- [ ] Update `get_users` - add user param, check SystemUserManage
- [ ] Update `get_user` - add user param, check SystemUserManage
- [ ] Update `create_user` - add user param, check SystemUserManage
- [ ] Update `update_user` - add user param, check SystemUserManage
- [ ] Update `remove_user` - add user param, check SystemUserManage

**Estimated Time:** 1.5 hours

#### 3.5 Tenant Management Methods

**Tasks:**
- [ ] Update `get_tenants` - add user param, check SystemTenantManage or TenantView
- [ ] Update `get_tenant` - add user param, check TenantView
- [ ] Update `create_tenant` - add user param, check SystemTenantManage
- [ ] Update `update_tenant` - add user param, check SystemTenantManage
- [ ] Update `delete_tenant` - add user param, check SystemTenantManage

**Estimated Time:** 1.5 hours

#### 3.6 Database Management Methods

**Tasks:**
- [ ] Update `get_databases` - add user param, check TenantView
- [ ] Update `get_database` - add user param, check TenantView
- [ ] Update `create_database` - add user param, check TenantDatabaseCreate
- [ ] Update `update_database` - add user param, check DatabaseConfigManage
- [ ] Update `delete_database` - add user param, check TenantDatabaseDelete

**Estimated Time:** 1.5 hours

#### 3.7 Tablespace Management Methods

**Tasks:**
- [ ] Update `get_tablespaces` - add user param, check SystemClusterView
- [ ] Update `get_tablespace` - add user param, check SystemClusterView
- [ ] Update `create_tablespace` - add user param, check SystemConfigManage
- [ ] Update `update_tablespace` - add user param, check SystemConfigManage
- [ ] Update `delete_tablespace` - add user param, check SystemConfigManage

**Estimated Time:** 1.5 hours

#### 3.8 Namespace Management Methods

**Tasks:**
- [ ] Update `get_namespaces` - add user param, check DatabaseSchemaView
- [ ] Update `get_namespace` - add user param, check DatabaseSchemaView
- [ ] Update `create_namespace` - add user param, check DatabaseNamespaceCreate
- [ ] Update `update_namespace` - add user param, check NamespaceConfigManage
- [ ] Update `delete_namespace` - add user param, check DatabaseNamespaceDelete

**Estimated Time:** 1.5 hours

#### 3.9 Table Management Methods

**Tasks:**
- [ ] Update `get_tables` - add user param, check DatabaseSchemaView
- [ ] Update `get_table` - add user param, check DatabaseSchemaView
- [ ] Update `create_table` - add user param, check DatabaseTableCreate
- [ ] Update `update_table` - add user param, check TableAlter
- [ ] Update `delete_table` - add user param, check DatabaseTableDelete

**Estimated Time:** 1.5 hours

#### 3.10 Data Operations Methods

**Tasks:**
- [ ] Update `put` - add user param, check TableWrite
- [ ] Update `get` - add user param, check TableRead
- [ ] Update `delete` - add user param, check TableDelete
- [ ] Update `batch_put` - add user param, check TableWrite

**Estimated Time:** 1 hour

### Phase 4: Update API Layer (Week 2)

#### 4.1 Update KeyValueDatabaseManager

**File:** `nanograph-kvm/src/manager.rs`

**Tasks:**
- [ ] Add user parameter to all public methods
- [ ] Pass user to all context method calls
- [ ] Update method documentation
- [ ] Update examples in doc comments

**Estimated Time:** 4 hours

#### 4.2 Update ContainerHandle

**File:** `nanograph-kvm/src/container.rs`

**Tasks:**
- [ ] Add `user: UserMetadata` field to struct
- [ ] Update `new` constructor to accept user
- [ ] Pass user to all context method calls
- [ ] Update documentation

**Estimated Time:** 3 hours

#### 4.3 Update TableHandle

**File:** `nanograph-kvm/src/table.rs`

**Tasks:**
- [ ] Add `user: UserMetadata` field to struct
- [ ] Update `new` constructor to accept user
- [ ] Pass user to all context method calls
- [ ] Update documentation

**Estimated Time:** 2 hours

### Phase 5: Testing (Week 3)

#### 5.1 Unit Tests

**File:** `nanograph-kvm/tests/permission_tests.rs` (new file)

**Tasks:**
- [ ] Test permission check allowed scenarios
- [ ] Test permission check denied scenarios
- [ ] Test permission inheritance from groups
- [ ] Test permission inheritance from roles
- [ ] Test Superuser permission
- [ ] Test permission caching
- [ ] Test cache expiration
- [ ] Test cache invalidation
- [ ] Test concurrent permission checks

**Estimated Time:** 8 hours

#### 5.2 Integration Tests

**File:** `nanograph-kvm/tests/security_integration_tests.rs` (new file)

**Tasks:**
- [ ] Test multi-user isolation
- [ ] Test tenant isolation
- [ ] Test permission escalation prevention
- [ ] Test complete operation flows with permissions
- [ ] Test audit logging

**Estimated Time:** 6 hours

#### 5.3 Performance Tests

**File:** `nanograph-kvm/benches/permission_benchmarks.rs` (new file)

**Tasks:**
- [ ] Benchmark permission check with warm cache
- [ ] Benchmark permission check with cold cache
- [ ] Benchmark full operation with permission check
- [ ] Benchmark concurrent access
- [ ] Verify performance impact is acceptable (target: less than 5%)

**Estimated Time:** 4 hours

### Phase 6: Documentation and Migration (Week 3)

#### 6.1 Update Documentation

**Tasks:**
- [x] Update SECURITY_MODEL.md with enforcement architecture
- [ ] Update API documentation in code
- [ ] Create migration guide for existing code
- [ ] Update examples to include user context
- [ ] Create security best practices guide

**Estimated Time:** 4 hours

#### 6.2 Migration Support

**File:** `docs/DEV/PERMISSION_MIGRATION_GUIDE.md` (new file)

**Tasks:**
- [ ] Document breaking changes
- [ ] Provide before/after migration examples
- [ ] Create compatibility layer if needed
- [ ] Update all examples in repository

**Estimated Time:** 3 hours

## Testing Checklist

- [ ] All unit tests pass
- [ ] All integration tests pass
- [ ] Performance benchmarks acceptable
- [ ] Security tests pass
- [ ] Multi-user tests pass
- [ ] Documentation complete
- [ ] Examples updated
- [ ] Migration guide complete

## Success Criteria

1. All operations enforce permissions
2. Single enforcement point (no duplication)
3. Multi-user safe (no race conditions)
4. Performance impact less than 5%
5. Comprehensive audit logging
6. All tests passing
7. Documentation complete

## Risks and Mitigations

| Risk | Impact | Mitigation |
|------|--------|------------|
| Performance degradation | High | Implement aggressive caching |
| Breaking changes | High | Provide migration guide and examples |
| Security gaps | Critical | Comprehensive security testing |
| Cache invalidation bugs | Medium | Conservative TTL, manual invalidation |
| Concurrent access issues | High | Thorough multi-threading tests |

## Timeline

- **Week 1:** Core infrastructure and permission checking logic
- **Week 2:** Update all context methods and API layer
- **Week 3:** Testing, documentation, and migration support

**Total Estimated Time:** 2-3 weeks (80-120 hours)

## Dependencies

- `nanograph-core` security types (already implemented)
- `nanograph-kvt` error types (needs update)
- System metadata cache (needs group/role support)

## Follow-up Work

After initial implementation:

1. Add permission templates for common roles
2. Implement time-based permissions
3. Add conditional permissions (IP, time-of-day)
4. Enhance audit logging with structured events
5. Add permission analytics dashboard
6. Implement permission delegation

## References

- [Security Model Documentation](../SECURITY_MODEL.md)
- [Multi-Tenancy Implementation Guide](MULTI_TENANCY_IMPLEMENTATION_GUIDE.md)