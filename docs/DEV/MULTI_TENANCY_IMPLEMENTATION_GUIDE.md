# Multi-Tenancy Implementation Guide

**Version:** 1.0  
**Date:** 2026-01-11  
**Status:** Planning  

---

## Overview

This guide provides detailed implementation instructions for adding multi-tenancy, security and resource isolation to Nanograph. It complements [ADR-0025](../ADR/ADR-0025-Multi-Tenancy-and-Isolation.md) and [ADR-0026](../ADR/ADR-0026-Resource-Quotas-and-Limits.md).

**Goal:** Enable secure multi-tenant deployments with complete data, user, and compute isolation.

**Timeline:** 4 weeks (Phase 2.5 in Implementation Plan)

---

## Architecture Overview

### Hierarchical Structure

```
Cluster
  └─ Tenant (NEW - Primary Security Isolation Boundary)
      └─ Database (NEW - Primary Compute Isolation Boundary)
          └─ Namespace (Existing - Schema/organization)
              └─ Table (Existing - Data container)
                  └─ Shard (Existing - Physical partition)
```

### Key Encoding Strategy

**Keys are NOT prefixed with tenant/database/namespace/table identifiers!**

Since each shard is already scoped to a specific table within a specific container (tenant + database), and shards are physically separate key-value stores, the isolation is provided by the shard itself.

```
Key Format: {user_key}
            |<-- application-defined -->|

Example:
  User key "user:123" → stored as-is: "user:123"
  
Physical Isolation:
  Tenant 1, DB 2, Table 4, Shard 0 → /data/shard_4_0/
  Tenant 2, DB 3, Table 5, Shard 0 → /data/shard_5_0/
```

**Why No Prefixes?**

1. **Shard-Level Isolation**: Each shard is a separate key-value store
   - Shard ID encodes: `ShardId::from_parts(table_id, shard_index)`
   - Table ID encodes container: Table belongs to specific tenant+database
   - Physical separation provides isolation

2. **Performance Benefits**:
   - Zero prefix overhead (saves 24 bytes per key)
   - Faster key comparisons
   - Better cache locality
   - Simpler key encoding/decoding

3. **Namespace Handling**:
   - Namespaces are entirely virtual and for logical organization
   - Example: `"namespace:users:123"` or `"ns1/users/123"`
   - Application-level convention, not system-level requirement

**Metadata Storage**:
- System metadata (tenants, databases, tables) stored in special system shards
- System shard keys DO use prefixes for organization:
  - `"tenant:{tenant_id}"` → TenantMetadata
  - `"database:{tenant_id}:{database_id}"` → DatabaseMetadata
  - `"table:{table_id}"` → TableMetadata

---

## API Design: Layered Facade Architecture

### Design Decision

The multi-tenant API uses a **Layered Facade Architecture** with three distinct layers:

1. **Inner Core** (`KeyValueDatabaseManagerCore`) - Internal implementation with full access
2. **Outer Facade** (`KeyValueDatabaseManager`) - Public API with limited, safe operations
3. **Container Handle** (`ContainerHandle`) - Scoped facade for container-level operations

This provides:

1. **Security by Design**: Public API cannot bypass tenant isolation
2. **Cleaner API**: Container context is bound at handle creation
3. **Better Encapsulation**: Internal methods are not exposed publicly
4. **Type Safety**: Compile-time guarantee that operations are scoped correctly
5. **Flexibility**: Easy to add administrative operations without exposing internals
6. **Natural Fit**: Aligns with existing `ContainerMetadataCache` architecture

### Architecture Layers

```
┌─────────────────────────────────────────────────────────┐
│  Public API Layer                                       │
│  ┌───────────────────────────────────────────────────┐ │
│  │ KeyValueDatabaseManager (Outer Facade)            │ │
│  │ - System management (tenants, databases)          │ │
│  │ - Container handle factory                        │ │
│  │ - Administrative operations                       │ │
│  └───────────────────────────────────────────────────┘ │
│                          │                              │
│                          ▼                              │
│  ┌───────────────────────────────────────────────────┐ │
│  │ ContainerHandle (Container Facade)                │ │
│  │ - Scoped data operations (put, get, delete)       │ │
│  │ - Table management within container               │ │
│  │ - Automatic tenant/database context               │ │
│  └───────────────────────────────────────────────────┘ │
└─────────────────────────────────────────────────────────┘
                          │
                          ▼
┌─────────────────────────────────────────────────────────┐
│  Internal Implementation Layer                          │
│  ┌───────────────────────────────────────────────────┐ │
│  │ KeyValueDatabaseManagerCore (Inner Core)          │ │
│  │ - Raw data operations (no validation)             │ │
│  │ - Direct shard access                             │ │
│  │ - Internal metadata operations                    │ │
│  │ - System-level operations                         │ │
│  └───────────────────────────────────────────────────┘ │
└─────────────────────────────────────────────────────────┘
```

### Layer 1: Inner Core (Internal Implementation)

```rust
/// Internal core implementation with full access to all operations
///
/// This struct is NOT exposed publicly. It contains all the actual implementation
/// logic and has unrestricted access to shards, metadata, and system operations.
pub(crate) struct KeyValueDatabaseManagerCore {
    /// Cluster ID
    cluster_id: ClusterId,
    
    /// Local Shard Storage Manager
    shard_manager: Arc<RwLock<KeyValueShardManager>>,
    
    /// System Metadata Cache
    system_metacache: Arc<RwLock<SystemMetadataCache>>,
    
    /// Database Metadata Cache
    container_metacaches: Arc<RwLock<HashMap<DatabaseId, Arc<RwLock<ContainerMetadataCache>>>>>,
    
    /// Raft router for distributed mode
    raft_router: Option<Arc<ConsensusRouter>>,
}

impl KeyValueDatabaseManagerCore {
    /// Internal method: put with explicit container context
    ///
    /// This method has NO validation - it's the caller's responsibility
    /// to ensure tenant access, quotas, etc.
    pub(crate) async fn put_internal(
        &self,
        container_id: ContainerId,
        namespace: NamespaceId,
        table: TableId,
        key: &[u8],
        value: &[u8],
    ) -> KeyValueResult<()> {
        // Keys are stored as-is - no encoding needed
        // Shard already provides tenant/database/table isolation
        let shard_id = self.get_shard_for_key(table, key)?;
        
        if let Some(router) = &self.raft_router {
            router.put(key.to_vec(), value.to_vec()).await
                .map_err(|e| KeyValueError::Consensus(format!("Raft put failed: {}", e)))
        } else {
            let shard_manager = self.shard_manager.read().unwrap();
            shard_manager.put(shard_id, key, value).await
        }
    }
    
    /// Internal method: get with explicit container context
    ///
    /// Keys are stored as-is - no decoding needed.
    pub(crate) async fn get_internal(
        &self,
        container_id: ContainerId,
        namespace: NamespaceId,
        table: TableId,
        key: &[u8],
    ) -> KeyValueResult<Option<Vec<u8>>> {
        let shard_id = self.get_shard_for_key(table, key)?;
        
        if let Some(router) = &self.raft_router {
            router.get(key).await
                .map_err(|e| KeyValueError::Consensus(format!("Raft get failed: {}", e)))
        } else {
            let shard_manager = self.shard_manager.read().unwrap();
            shard_manager.get(shard_id, key).await
        }
    }
    
    /// Internal method: delete with explicit container context
    pub(crate) async fn delete_internal(
        &self,
        container_id: ContainerId,
        namespace: NamespaceId,
        table: TableId,
        key: &[u8],
    ) -> KeyValueResult<bool> {
        let shard_id = self.get_shard_for_key(table, key)?;
        
        if let Some(router) = &self.raft_router {
            router.delete(key.to_vec()).await
                .map_err(|e| KeyValueError::Consensus(format!("Raft delete failed: {}", e)))?;
            Ok(true)
        } else {
            let shard_manager = self.shard_manager.read().unwrap();
            shard_manager.delete(shard_id, key).await
        }
    }
    
    // ... other internal methods for table creation, shard management, etc.
}
```

### Layer 2: Outer Facade (Public API)

```rust
/// Public facade for database management
///
/// This is the main entry point for users. It provides safe, validated
/// access to database operations and enforces tenant isolation.
pub struct KeyValueDatabaseManager {
    /// Internal core implementation (not exposed)
    core: Arc<KeyValueDatabaseManagerCore>,
    
    /// Usage tracker for quotas and metrics
    usage_tracker: Arc<UsageTracker>,
    
    /// Access control policy
    access_policy: Arc<dyn AccessPolicy>,
}

impl KeyValueDatabaseManager {
    /// Create a new database manager in single-node mode
    pub fn new_standalone(config: KeyValueDatabaseConfig) -> Self {
        let core = Arc::new(KeyValueDatabaseManagerCore::new_standalone(config));
        let usage_tracker = Arc::new(UsageTracker::new());
        let access_policy = Arc::new(DefaultAccessPolicy::new());
        
        Self {
            core,
            usage_tracker,
            access_policy,
        }
    }
    
    /// Get a container handle for a specific tenant and database
    ///
    /// This is the PRIMARY way to access data operations.
    /// It validates tenant access and creates a scoped handle.
    pub fn container(
        &self,
        tenant: TenantId,
        database: DatabaseId,
    ) -> KeyValueResult<ContainerHandle> {
        // Verify tenant exists and is active
        self.verify_tenant_access(tenant)?;
        
        // Verify database exists and belongs to tenant
        self.verify_database_access(tenant, database)?;
        
        // Create container ID
        let container_id = ContainerId::from_parts(tenant, database);
        
        // Get or create container metadata cache
        let cache = self.core.get_or_create_container_cache(container_id)?;
        
        Ok(ContainerHandle::new(
            container_id,
            self.core.clone(),
            cache,
            self.usage_tracker.clone(),
        ))
    }
    
    // System-level operations (tenant/database management)
    
    /// Create a new tenant
    pub async fn create_tenant(&self, config: TenantCreate) -> KeyValueResult<TenantMetadata> {
        // Administrative operation - requires superuser access
        // Implementation delegates to core
        self.core.create_tenant_internal(config).await
    }
    
    /// Get tenant metadata
    pub async fn get_tenant(&self, tenant: TenantId) -> KeyValueResult<Option<TenantMetadata>> {
        self.core.get_tenant_internal(tenant).await
    }
    
    /// Create a database within a tenant
    pub async fn create_database(
        &self,
        tenant: TenantId,
        config: DatabaseCreate,
    ) -> KeyValueResult<DatabaseMetadata> {
        // Verify tenant access
        self.verify_tenant_access(tenant)?;
        
        // Check quotas
        self.check_database_quota(tenant)?;
        
        // Delegate to core
        self.core.create_database_internal(tenant, config).await
    }
    
    // NOTE: Data operations (put, get, delete) are NOT exposed here
    // They must go through ContainerHandle for proper isolation
    
    fn verify_tenant_access(&self, tenant: TenantId) -> KeyValueResult<()> {
        let cache = self.core.system_metacache.read().unwrap();
        let tenant_meta = cache.get_tenant_record(&tenant)
            .ok_or(KeyValueError::TenantNotFound(tenant))?;
        
        if tenant_meta.status != TenantStatus::Active {
            return Err(KeyValueError::TenantSuspended(tenant));
        }
        
        Ok(())
    }
    
    fn verify_database_access(&self, tenant: TenantId, database: DatabaseId) -> KeyValueResult<()> {
        let cache = self.core.system_metacache.read().unwrap();
        let db_meta = cache.get_database_record(&database)
            .ok_or(KeyValueError::DatabaseNotFound(database))?;
        
        if db_meta.tenant != tenant {
            return Err(KeyValueError::AccessDenied(
                format!("Database {} does not belong to tenant {}", database, tenant)
            ));
        }
        
        Ok(())
    }
}
```

### Layer 3: Container Handle (Scoped Facade)

```rust
/// Container-scoped facade for database operations
///
/// A Container represents a specific Database within a Tenant.
/// All operations through this handle are automatically scoped to the container.
pub struct ContainerHandle {
    /// Container ID (encapsulates TenantId + DatabaseId)
    container_id: ContainerId,
    
    /// Reference to the internal core (for actual operations)
    core: Arc<KeyValueDatabaseManagerCore>,
    
    /// Container-specific metadata cache
    metadata_cache: Arc<RwLock<ContainerMetadataCache>>,
    
    /// Usage tracker for quotas
    usage_tracker: Arc<UsageTracker>,
}

impl ContainerHandle {
    /// Put a key-value pair into a table
    ///
    /// No need to pass container_id - it's already bound to this handle
    pub async fn put(
        &self,
        namespace: NamespaceId,
        table: TableId,
        key: &[u8],
        value: &[u8],
    ) -> KeyValueResult<()> {
        // Internally uses self.container_id for key encoding
        self.manager.put_internal(
            self.container_id,
            namespace,
            table,
            key,
            value,
        ).await
    }
    
    /// Get a value from a table
    pub async fn get(
        &self,
        namespace: NamespaceId,
        table: TableId,
        key: &[u8],
    ) -> KeyValueResult<Option<Vec<u8>>> {
        self.manager.get_internal(
            self.container_id,
            namespace,
            table,
            key,
        ).await
    }
    
    /// Delete a key from a table
    pub async fn delete(
        &self,
        namespace: NamespaceId,
        table: TableId,
        key: &[u8],
    ) -> KeyValueResult<bool> {
        self.manager.delete_internal(
            self.container_id,
            namespace,
            table,
            key,
        ).await
    }
    
    /// Create a new table in this container
    pub async fn create_table(
        &self,
        path: &str,
        name: &str,
        config: TableCreate,
    ) -> KeyValueResult<TableId> {
        self.manager.create_table_internal(
            self.container_id,
            path,
            name,
            config,
        ).await
    }
    
    /// List all tables in this container
    pub async fn list_tables(&self) -> KeyValueResult<Vec<TableMetadata>> {
        let cache = self.metadata_cache.read().unwrap();
        Ok(cache.list_table_records().cloned().collect())
    }
    
    /// Get table metadata
    pub async fn get_table(&self, table_id: TableId) -> KeyValueResult<Option<TableMetadata>> {
        let cache = self.metadata_cache.read().unwrap();
        Ok(cache.get_table_record(&table_id).cloned())
    }
    
    /// Get the container ID
    pub fn container_id(&self) -> ContainerId {
        self.container_id
    }
    
    /// Get the tenant ID
    pub fn tenant_id(&self) -> TenantId {
        self.container_id.tenant()
    }
    
    /// Get the database ID
    pub fn database_id(&self) -> DatabaseId {
        self.container_id.database()
    }
}
```

### Factory Method on KeyValueDatabaseManager

```rust
impl KeyValueDatabaseManager {
    /// Get a container handle for a specific tenant and database
    ///
    /// This validates tenant access and creates/retrieves the container cache.
    pub fn container(
        &self,
        tenant: TenantId,
        database: DatabaseId,
    ) -> KeyValueResult<ContainerHandle> {
        // Verify tenant exists and is active
        self.verify_tenant_access(tenant)?;
        
        // Verify database exists and belongs to tenant
        self.verify_database_access(tenant, database)?;
        
        // Create container ID
        let container_id = ContainerId::from_parts(tenant, database);
        
        // Get or create container metadata cache
        let cache = self.get_or_create_container_cache(container_id)?;
        
        Ok(ContainerHandle {
            container_id,
            manager: Arc::new(self.clone()),
            metadata_cache: cache,
        })
    }
    
    /// Internal method: put with explicit container context
    async fn put_internal(
        &self,
        container_id: ContainerId,
        namespace: NamespaceId,
        table: TableId,
        key: &[u8],
        value: &[u8],
    ) -> KeyValueResult<()> {
        // Extract tenant and database from container
        let tenant = container_id.tenant();
        let database = container_id.database();
        
        // Check resource quotas
        self.check_quotas(tenant, Operation::Write {
            size: key.len() + value.len()
        })?;
        
        // Encode key with tenant/database/namespace prefix
        // Note: table is NOT included - shard is already table-specific
        let encoded_key = key_encoding::encode_key(
            tenant,
            database,
            namespace,
            key,
        );
        
        // Get shard and perform write
        let shard_id = self.get_shard_for_key(table, &encoded_key)?;
        
        if let Some(router) = &self.raft_router {
            router.put(encoded_key, value.to_vec()).await
                .map_err(|e| KeyValueError::Consensus(format!("Raft put failed: {}", e)))
        } else {
            let shard_manager = self.shard_manager.read().unwrap();
            shard_manager.put(shard_id, &encoded_key, value).await
        }?;
        
        // Update usage metrics
        self.record_usage(tenant, Operation::Write {
            size: key.len() + value.len()
        });
        
        Ok(())
    }
    
    // Similar internal methods for get, delete, etc.
}
```

### Usage Examples

```rust
// Example 1: Basic operations with container handle
async fn example_basic_usage(
    db_manager: &KeyValueDatabaseManager,
    tenant_id: TenantId,
    database_id: DatabaseId,
) -> KeyValueResult<()> {
    // Get container handle (validates access once)
    let container = db_manager.container(tenant_id, database_id)?;
    
    // Clean API - no need to pass container_id repeatedly
    container.put(namespace, table, b"key1", b"value1").await?;
    container.put(namespace, table, b"key2", b"value2").await?;
    
    let value = container.get(namespace, table, b"key1").await?;
    assert_eq!(value, Some(b"value1".to_vec()));
    
    container.delete(namespace, table, b"key1").await?;
    
    Ok(())
}

// Example 2: Multiple containers
async fn example_multi_container(
    db_manager: &KeyValueDatabaseManager,
) -> KeyValueResult<()> {
    // Different containers for different tenants
    let tenant1_container = db_manager.container(TenantId::new(1), DatabaseId::DEFAULT)?;
    let tenant2_container = db_manager.container(TenantId::new(2), DatabaseId::DEFAULT)?;
    
    // Operations are automatically isolated by container
    tenant1_container.put(ns, table, b"key", b"tenant1_value").await?;
    tenant2_container.put(ns, table, b"key", b"tenant2_value").await?;
    
    // Each tenant sees only their own data
    let val1 = tenant1_container.get(ns, table, b"key").await?;
    let val2 = tenant2_container.get(ns, table, b"key").await?;
    
    assert_eq!(val1, Some(b"tenant1_value".to_vec()));
    assert_eq!(val2, Some(b"tenant2_value".to_vec()));
    
    Ok(())
}

// Example 3: Table management
async fn example_table_management(
    container: &ContainerHandle,
) -> KeyValueResult<()> {
    // Create table in this container
    let table_id = container.create_table(
        "/analytics",
        "events",
        TableCreate {
            engine_type: StorageEngineType::from("lsm"),
            sharding_config: TableSharding::Single,
            ..Default::default()
        },
    ).await?;
    
    // List all tables in this container
    let tables = container.list_tables().await?;
    println!("Container has {} tables", tables.len());
    
    // Get table metadata
    let table_meta = container.get_table(table_id).await?;
    
    Ok(())
}
```

### Benefits of This Design

1. **Cleaner API Surface**
   - Methods have fewer parameters
   - Container context is implicit, not explicit
   - Reduces cognitive load for API users

2. **Better Type Safety**
   - Container context validated once at handle creation
   - Impossible to accidentally use wrong container ID
   - Compile-time guarantees about operation scope

3. **Performance Optimization**
   - Container validation happens once, not per operation
   - Metadata cache is bound to handle
   - Reduces lock contention on shared caches

4. **Natural Separation of Concerns**
   - System-level operations: `KeyValueDatabaseManager`
   - Container-level operations: `ContainerHandle`
   - Clear API boundaries

5. **Aligns with Existing Architecture**
   - Matches `ContainerMetadataCache` design
   - Follows hierarchical structure (Tenant → Database → Namespace → Table)
   - Integrates naturally with key encoding strategy

6. **Extensibility**
   - Easy to add container-specific features (quotas, rate limiting)
   - Can add container-level caching strategies
   - Supports future container-level optimizations

---

## Implementation Phases

### Phase 1: Core Types and Metadata (Week 1)

#### 1.1 Add Core Types to `nanograph-core/src/types.rs`

```rust
/// Tenant identifier - top-level isolation boundary
#[derive(Copy, Clone, Debug, Ord, PartialOrd, Eq, PartialEq, Hash)]
pub struct TenantId(pub u64);

impl TenantId {
    pub fn new(id: u64) -> Self {
        Self(id)
    }
    
    pub fn as_u64(&self) -> u64 {
        self.0
    }
    
    /// System tenant (ID 0) for internal metadata
    pub const SYSTEM: TenantId = TenantId(0);
}

impl From<u64> for TenantId {
    fn from(id: u64) -> Self {
        Self(id)
    }
}

impl std::fmt::Display for TenantId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Tenant({})", self.0)
    }
}

/// Database identifier - logical grouping within tenant
#[derive(Copy, Clone, Debug, Ord, PartialOrd, Eq, PartialEq, Hash)]
pub struct DatabaseId(pub u64);

impl DatabaseId {
    pub fn new(id: u64) -> Self {
        Self(id)
    }
    
    pub fn as_u64(&self) -> u64 {
        self.0
    }
    
    /// Default database (ID 0) created with each tenant
    pub const DEFAULT: DatabaseId = DatabaseId(0);
}

impl From<u64> for DatabaseId {
    fn from(id: u64) -> Self {
        Self(id)
    }
}

impl std::fmt::Display for DatabaseId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Database({})", self.0)
    }
}

/// Hierarchical resource path
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct ResourcePath {
    pub tenant: TenantId,
    pub database: DatabaseId,
    pub namespace: Option<NamespaceId>,
    pub table: Option<TableId>,
}

impl ResourcePath {
    /// Create a new resource path
    pub fn new(tenant: TenantId, database: DatabaseId) -> Self {
        Self {
            tenant,
            database,
            namespace: None,
            table: None,
        }
    }
    
    /// Parse from string: "tenant.database.namespace.table"
    pub fn parse(path: &str) -> Result<Self, ParseError> {
        let parts: Vec<&str> = path.split('.').collect();
        
        if parts.len() < 2 {
            return Err(ParseError::InvalidFormat);
        }
        
        let tenant = parts[0].parse::<u64>()
            .map(TenantId::new)
            .map_err(|_| ParseError::InvalidTenantId)?;
            
        let database = parts[1].parse::<u64>()
            .map(DatabaseId::new)
            .map_err(|_| ParseError::InvalidDatabaseId)?;
        
        let namespace = if parts.len() > 2 {
            Some(parts[2].parse::<u64>()
                .map(NamespaceId::new)
                .map_err(|_| ParseError::InvalidNamespaceId)?)
        } else {
            None
        };
        
        let table = if parts.len() > 3 {
            Some(parts[3].parse::<u64>()
                .map(TableId::new)
                .map_err(|_| ParseError::InvalidTableId)?)
        } else {
            None
        };
        
        Ok(Self {
            tenant,
            database,
            namespace,
            table,
        })
    }
    
    /// Convert to string representation
    pub fn to_string(&self) -> String {
        let mut path = format!("{}.{}", self.tenant.0, self.database.0);
        
        if let Some(ns) = self.namespace {
            path.push_str(&format!(".{}", ns.0));
        }
        
        if let Some(tbl) = self.table {
            path.push_str(&format!(".{}", tbl.0));
        }
        
        path
    }
}

#[derive(Debug, Clone)]
pub enum ParseError {
    InvalidFormat,
    InvalidTenantId,
    InvalidDatabaseId,
    InvalidNamespaceId,
    InvalidTableId,
}
```

#### 1.2 Add Metadata Types to `nanograph-kvt/src/types.rs`

```rust
use nanograph_core::types::{TenantId, DatabaseId, NamespaceId, Timestamp};

/// Tenant configuration and metadata
#[derive(Debug, Clone)]
pub struct TenantMetadata {
    pub id: TenantId,
    pub name: String,
    pub created_at: Timestamp,
    pub last_modified: Timestamp,
    pub isolation_mode: IsolationMode,
    pub resource_quotas: ResourceQuotas,
    pub databases: Vec<DatabaseId>,
    pub status: TenantStatus,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TenantStatus {
    Active,
    Suspended,
    Migrating,
    Archived,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IsolationMode {
    /// Shared shards with logical key separation (default)
    Logical,
    
    /// Dedicated shards for tenant data
    DedicatedShards,
    
    /// Dedicated nodes for tenant workloads
    DedicatedNodes,
    
    /// Complete physical isolation (separate cluster)
    PhysicalCluster,
}

/// Database configuration within a tenant
#[derive(Debug, Clone)]
pub struct DatabaseMetadata {
    pub id: DatabaseId,
    pub tenant: TenantId,
    pub name: String,
    pub created_at: Timestamp,
    pub last_modified: Timestamp,
    pub namespaces: Vec<NamespaceId>,
    pub storage_quota: Option<u64>,
}

/// Tenant configuration for creation
#[derive(Debug, Clone)]
pub struct TenantConfig {
    pub name: String,
    pub isolation_mode: IsolationMode,
    pub resource_quotas: ResourceQuotas,
}

/// Database configuration for creation
#[derive(Debug, Clone)]
pub struct DatabaseConfig {
    pub name: String,
    pub storage_quota: Option<u64>,
}
```

#### 1.3 Update Metastore in `nanograph-kvm/src/metastore.rs`

Add tenant and database management methods:

```rust
impl Metastore {
    /// Create a new tenant
    pub fn create_tenant(&mut self, config: TenantConfig) -> Result<TenantId, MetastoreError> {
        // Generate unique tenant ID
        let tenant_id = self.allocate_tenant_id()?;
        
        // Create tenant metadata
        let metadata = TenantMetadata {
            id: tenant_id,
            name: config.name.clone(),
            created_at: Timestamp::now(),
            last_modified: Timestamp::now(),
            isolation_mode: config.isolation_mode,
            resource_quotas: config.resource_quotas,
            databases: vec![DatabaseId::DEFAULT],
            status: TenantStatus::Active,
        };
        
        // Store tenant metadata
        self.tenants.insert(tenant_id, metadata);
        
        // Create default database
        self.create_database_internal(
            tenant_id,
            DatabaseConfig {
                name: "default".to_string(),
                storage_quota: None,
            },
        )?;
        
        Ok(tenant_id)
    }
    
    /// Get tenant metadata
    pub fn get_tenant(&self, tenant_id: TenantId) -> Option<&TenantMetadata> {
        self.tenants.get(&tenant_id)
    }
    
    /// Update tenant metadata
    pub fn update_tenant(&mut self, tenant_id: TenantId, metadata: TenantMetadata) -> Result<(), MetastoreError> {
        if !self.tenants.contains_key(&tenant_id) {
            return Err(MetastoreError::TenantNotFound(tenant_id));
        }
        
        self.tenants.insert(tenant_id, metadata);
        Ok(())
    }
    
    /// Create a database within a tenant
    pub fn create_database(
        &mut self,
        tenant_id: TenantId,
        config: DatabaseConfig,
    ) -> Result<DatabaseId, MetastoreError> {
        // Verify tenant exists
        let tenant = self.tenants.get_mut(&tenant_id)
            .ok_or(MetastoreError::TenantNotFound(tenant_id))?;
        
        // Check tenant quotas
        if let Some(max_dbs) = tenant.resource_quotas.storage.max_databases {
            if tenant.databases.len() >= max_dbs as usize {
                return Err(MetastoreError::QuotaExceeded);
            }
        }
        
        self.create_database_internal(tenant_id, config)
    }
    
    fn create_database_internal(
        &mut self,
        tenant_id: TenantId,
        config: DatabaseConfig,
    ) -> Result<DatabaseId, MetastoreError> {
        // Generate unique database ID
        let database_id = self.allocate_database_id()?;
        
        // Create database metadata
        let metadata = DatabaseMetadata {
            id: database_id,
            tenant: tenant_id,
            name: config.name,
            created_at: Timestamp::now(),
            last_modified: Timestamp::now(),
            namespaces: vec![],
            storage_quota: config.storage_quota,
        };
        
        // Store database metadata
        self.databases.insert(database_id, metadata);
        
        // Update tenant's database list
        if let Some(tenant) = self.tenants.get_mut(&tenant_id) {
            tenant.databases.push(database_id);
        }
        
        Ok(database_id)
    }
    
    fn allocate_tenant_id(&mut self) -> Result<TenantId, MetastoreError> {
        // Simple incrementing ID for now
        // TODO: Use distributed ID generation in production
        let id = self.next_tenant_id;
        self.next_tenant_id += 1;
        Ok(TenantId::new(id))
    }
    
    fn allocate_database_id(&mut self) -> Result<DatabaseId, MetastoreError> {
        let id = self.next_database_id;
        self.next_database_id += 1;
        Ok(DatabaseId::new(id))
    }
}
```

#### 1.4 Testing

Create `nanograph-kvm/tests/tenant_tests.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_create_tenant() {
        let mut metastore = Metastore::new();
        
        let config = TenantConfig {
            name: "test-tenant".to_string(),
            isolation_mode: IsolationMode::Logical,
            resource_quotas: ResourceQuotas::default(),
        };
        
        let tenant_id = metastore.create_tenant(config).unwrap();
        
        let tenant = metastore.get_tenant(tenant_id).unwrap();
        assert_eq!(tenant.name, "test-tenant");
        assert_eq!(tenant.status, TenantStatus::Active);
        assert_eq!(tenant.databases.len(), 1); // Default database
    }
    
    #[test]
    fn test_create_database() {
        let mut metastore = Metastore::new();
        
        let tenant_id = metastore.create_tenant(TenantConfig::default()).unwrap();
        
        let db_config = DatabaseConfig {
            name: "my-database".to_string(),
            storage_quota: Some(1024 * 1024 * 1024), // 1 GB
        };
        
        let db_id = metastore.create_database(tenant_id, db_config).unwrap();
        
        let db = metastore.get_database(db_id).unwrap();
        assert_eq!(db.name, "my-database");
        assert_eq!(db.tenant, tenant_id);
    }
    
    #[test]
    fn test_tenant_isolation() {
        let mut metastore = Metastore::new();
        
        let tenant1 = metastore.create_tenant(TenantConfig::default()).unwrap();
        let tenant2 = metastore.create_tenant(TenantConfig::default()).unwrap();
        
        assert_ne!(tenant1, tenant2);
        
        let db1 = metastore.create_database(tenant1, DatabaseConfig::default()).unwrap();
        let db2 = metastore.create_database(tenant2, DatabaseConfig::default()).unwrap();
        
        assert_ne!(db1, db2);
        assert_eq!(metastore.get_database(db1).unwrap().tenant, tenant1);
        assert_eq!(metastore.get_database(db2).unwrap().tenant, tenant2);
    }
}
```

---

### Phase 2: Key Encoding and Storage (Week 2)

#### 2.1 System Metadata Key Encoding

Create `nanograph-kvt/src/system_keys.rs` for system metadata keys:

```rust
use nanograph_core::types::{TenantId, DatabaseId, TableId, TablespaceId};

/// System metadata key encoding
///
/// These keys are used in system shards to store cluster metadata.
/// User data keys are NOT prefixed - they're stored as-is in their respective shards.
pub struct SystemKeys;

impl SystemKeys {
    /// Key for tenant metadata
    pub fn tenant_key(tenant_id: TenantId) -> Vec<u8> {
        format!("tenant:{}", tenant_id.as_u64()).into_bytes()
    }
    
    /// Key for database metadata
    pub fn database_key(tenant_id: TenantId, database_id: DatabaseId) -> Vec<u8> {
        format!("database:{}:{}", tenant_id.as_u64(), database_id.as_u64()).into_bytes()
    }
    
    /// Key for table metadata
    pub fn table_key(table_id: TableId) -> Vec<u8> {
        format!("table:{}", table_id.as_u64()).into_bytes()
    }
    
    /// Key for tablespace metadata
    pub fn tablespace_key(tablespace_id: TablespaceId) -> Vec<u8> {
        format!("tablespace:{}", tablespace_id.as_u64()).into_bytes()
    }
    
    /// Key for cluster metadata
    pub fn cluster_key() -> Vec<u8> {
        b"cluster:metadata".to_vec()
    }
    
    /// Prefix for listing all tenants
    pub fn tenant_prefix() -> Vec<u8> {
        b"tenant:".to_vec()
    }
    
    /// Prefix for listing databases in a tenant
    pub fn database_prefix(tenant_id: TenantId) -> Vec<u8> {
        format!("database:{}:", tenant_id.as_u64()).into_bytes()
    }
    
    /// Prefix for listing all tables
    pub fn table_prefix() -> Vec<u8> {
        b"table:".to_vec()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_system_keys() {
        let tenant_id = TenantId::new(1);
        let database_id = DatabaseId::new(2);
        let table_id = TableId::new(3);
        
        let tenant_key = SystemKeys::tenant_key(tenant_id);
        assert_eq!(tenant_key, b"tenant:1");
        
        let db_key = SystemKeys::database_key(tenant_id, database_id);
        assert_eq!(db_key, b"database:1:2");
        
        let table_key = SystemKeys::table_key(table_id);
        assert_eq!(table_key, b"table:3");
    }
    
    #[test]
    fn test_prefix_generation() {
        let tenant_id = TenantId::new(1);
        
        let tenant_pfx = SystemKeys::tenant_prefix();
        let db_pfx = SystemKeys::database_prefix(tenant_id);
        
        assert_eq!(tenant_pfx, b"tenant:");
        assert_eq!(db_pfx, b"database:1:");
        
        // Verify prefix matching
        let db_key = SystemKeys::database_key(tenant_id, DatabaseId::new(2));
        assert!(db_key.starts_with(&db_pfx));
    }
}
```

**Key Points:**
- User data keys are stored as-is (no encoding needed)
- Only system metadata keys use prefixes for organization
- Simpler, faster, and more efficient than prefixing all keys

#### 2.2 Implement Container Handle

Create `nanograph-kvm/src/container.rs`:

```rust
use crate::cache::ContainerMetadataCache;
use crate::database::KeyValueDatabaseManager;
use nanograph_core::types::{ContainerId, DatabaseId, NamespaceId, TableId, TenantId};
use nanograph_kvt::{KeyValueResult, TableCreate, TableMetadata, TableUpdate};
use std::sync::{Arc, RwLock};

/// Container-scoped facade for database operations
///
/// A Container represents a specific Database within a Tenant.
/// All operations through this handle are automatically scoped to the container.
pub struct ContainerHandle {
    /// Container ID (encapsulates TenantId + DatabaseId)
    container_id: ContainerId,
    
    /// Reference to the underlying database manager
    manager: Arc<KeyValueDatabaseManager>,
    
    /// Container-specific metadata cache
    metadata_cache: Arc<RwLock<ContainerMetadataCache>>,
}

impl ContainerHandle {
    /// Create a new container handle (internal use only)
    pub(crate) fn new(
        container_id: ContainerId,
        core: Arc<KeyValueDatabaseManagerCore>,
        metadata_cache: Arc<RwLock<ContainerMetadataCache>>,
        usage_tracker: Arc<UsageTracker>,
    ) -> Self {
        Self {
            container_id,
            core,
            metadata_cache,
            usage_tracker,
        }
    }
    
    /// Put a key-value pair into a table
    ///
    /// This method enforces quotas and delegates to the internal core.
    pub async fn put(
        &self,
        namespace: NamespaceId,
        table: TableId,
        key: &[u8],
        value: &[u8],
    ) -> KeyValueResult<()> {
        let tenant = self.container_id.tenant();
        
        // Check quotas BEFORE operation
        self.usage_tracker.check_quota(tenant, Operation::Write {
            size: key.len() + value.len()
        })?;
        
        // Delegate to internal core (no validation needed - already done)
        self.core.put_internal(
            self.container_id,
            namespace,
            table,
            key,
            value,
        ).await?;
        
        // Record usage AFTER successful operation
        self.usage_tracker.record(tenant, Operation::Write {
            size: key.len() + value.len()
        });
        
        Ok(())
    }
    
    /// Get a value from a table
    pub async fn get(
        &self,
        namespace: NamespaceId,
        table: TableId,
        key: &[u8],
    ) -> KeyValueResult<Option<Vec<u8>>> {
        let tenant = self.container_id.tenant();
        
        // Delegate to internal core
        let result = self.core.get_internal(
            self.container_id,
            namespace,
            table,
            key,
        ).await?;
        
        // Record usage
        self.usage_tracker.record(tenant, Operation::Read);
        
        Ok(result)
    }
    
    /// Delete a key from a table
    pub async fn delete(
        &self,
        namespace: NamespaceId,
        table: TableId,
        key: &[u8],
    ) -> KeyValueResult<bool> {
        let tenant = self.container_id.tenant();
        
        // Delegate to internal core
        let result = self.core.delete_internal(
            self.container_id,
            namespace,
            table,
            key,
        ).await?;
        
        // Record usage
        self.usage_tracker.record(tenant, Operation::Delete);
        
        Ok(result)
    }
    
    /// Batch put operations
    pub async fn batch_put(
        &self,
        namespace: NamespaceId,
        table: TableId,
        pairs: &[(&[u8], &[u8])],
    ) -> KeyValueResult<()> {
        let tenant = self.container_id.tenant();
        let total_size: usize = pairs.iter().map(|(k, v)| k.len() + v.len()).sum();
        
        // Check quotas for entire batch
        self.usage_tracker.check_quota(tenant, Operation::Write {
            size: total_size
        })?;
        
        // Delegate to internal core
        self.core.batch_put_internal(
            self.container_id,
            namespace,
            table,
            pairs,
        ).await?;
        
        // Record usage
        self.usage_tracker.record(tenant, Operation::Write {
            size: total_size
        });
        
        Ok(())
    }
    
    /// Create a new table in this container
    pub async fn create_table(
        &self,
        path: &str,
        name: &str,
        config: TableCreate,
    ) -> KeyValueResult<TableId> {
        self.core.create_table_internal(
            self.container_id,
            path,
            name,
            config,
        ).await
    }
    
    /// Update table metadata
    pub async fn update_table(
        &self,
        table_id: TableId,
        config: TableUpdate,
    ) -> KeyValueResult<TableMetadata> {
        self.core.update_table_internal(
            self.container_id,
            table_id,
            config,
        ).await
    }
    
    /// Delete a table
    pub async fn delete_table(&self, table_id: TableId) -> KeyValueResult<()> {
        self.core.delete_table_internal(
            self.container_id,
            table_id,
        ).await
    }
    
    /// List all tables in this container
    pub async fn list_tables(&self) -> KeyValueResult<Vec<TableMetadata>> {
        let cache = self.metadata_cache.read().unwrap();
        Ok(cache.list_table_records().cloned().collect())
    }
    
    /// Get table metadata
    pub async fn get_table(&self, table_id: TableId) -> KeyValueResult<Option<TableMetadata>> {
        let cache = self.metadata_cache.read().unwrap();
        Ok(cache.get_table_record(&table_id).cloned())
    }
    
    /// Get the container ID
    pub fn container_id(&self) -> ContainerId {
        self.container_id
    }
    
    /// Get the tenant ID
    pub fn tenant_id(&self) -> TenantId {
        self.container_id.tenant()
    }
    
    /// Get the database ID
    pub fn database_id(&self) -> DatabaseId {
        self.container_id.database()
    }
}
```

### Key Security Properties

1. **No Direct Data Access**: The public `KeyValueDatabaseManager` does NOT expose `put`, `get`, or `delete` methods
2. **Forced Validation**: All data operations must go through `ContainerHandle`, which validates access at creation
3. **Internal Core Hidden**: `KeyValueDatabaseManagerCore` is `pub(crate)` - not accessible outside the crate
4. **Quota Enforcement**: `ContainerHandle` enforces quotas before delegating to core
5. **Audit Trail**: All operations are tracked through `UsageTracker`
6. **Separation of Concerns**: System operations on outer facade, data operations through container handles

#### 2.3 Implementation Notes

The layered architecture ensures:

- **Security**: No way to bypass tenant isolation from public API
- **Maintainability**: Clear separation between public API and internal implementation
- **Testability**: Can test each layer independently
- **Flexibility**: Easy to add new features without breaking existing API
- **Performance**: Validation happens once at handle creation, not per operation

---

### Phase 3: Resource Quotas (Week 3)

See [ADR-0026](../ADR/ADR-0026-Resource-Quotas-and-Limits.md) for complete implementation details.

Key components:
1. `ResourceQuotas` types
2. `UsageTracker` with atomic counters
3. `RateLimiter` implementation
4. Quota enforcement in all operations
5. Monitoring and alerting

---

### Phase 4: Access Control Integration (Week 4)

#### 4.1 Extend ABAC with Tenant Context

Update `nanograph-api/src/auth.rs`:

```rust
pub struct AccessContext {
    /// Authenticated user/service
    pub principal: Principal,
    
    /// Tenant context
    pub tenant: TenantId,
    
    /// Database context (optional)
    pub database: Option<DatabaseId>,
    
    /// Requested resource
    pub resource: ResourcePath,
    
    /// Requested action
    pub action: Action,
}

pub trait AccessPolicy {
    /// Evaluate if access is allowed
    fn evaluate(&self, context: &AccessContext) -> AccessDecision;
}

impl DefaultAccessPolicy {
    fn evaluate(&self, context: &AccessContext) -> AccessDecision {
        // Check tenant membership
        if !self.is_tenant_member(&context.principal, context.tenant) {
            return AccessDecision::Deny {
                reason: "Not a member of this tenant".to_string(),
            };
        }
        
        // Check resource permissions
        if !self.has_permission(&context.principal, &context.resource, &context.action) {
            return AccessDecision::Deny {
                reason: "Insufficient permissions".to_string(),
            };
        }
        
        AccessDecision::Allow
    }
}
```

---

## Testing Strategy

### Unit Tests
- Key encoding/decoding
- Tenant metadata operations
- Quota calculations
- Rate limiting logic

### Integration Tests
- Multi-tenant data isolation
- Cross-tenant access prevention
- Quota enforcement
- Tenant lifecycle (create, suspend, delete)

### Performance Tests
- Overhead of key prefixing
- Quota check latency
- Multi-tenant throughput

### Security Tests
- Tenant boundary violations
- Privilege escalation attempts
- Resource exhaustion attacks

---

## Migration Guide

### For Existing Deployments

1. **Backup all data**
2. **Create system tenant** (ID 0) for existing data
3. **Migrate existing tables** to system tenant
4. **Update client code** to include tenant context
5. **Test thoroughly** before production deployment

### Example Migration Script

```rust
async fn migrate_to_multi_tenant(db: &KeyValueDatabaseManager) -> Result<()> {
    // Create system tenant
    let system_tenant = db.create_tenant(TenantConfig {
        name: "system".to_string(),
        isolation_mode: IsolationMode::Logical,
        resource_quotas: ResourceQuotas::unlimited(),
    }).await?;
    
    // Migrate existing tables
    for table in db.list_tables().await? {
        db.migrate_table_to_tenant(table, system_tenant).await?;
    }
    
    Ok(())
}
```

---

## Monitoring and Operations

### Key Metrics

- `tenant_storage_bytes{tenant_id}` - Storage usage per tenant
- `tenant_iops{tenant_id}` - Operations per second per tenant
- `tenant_connections{tenant_id}` - Active connections per tenant
- `quota_violations{tenant_id, resource}` - Quota violation count
- `tenant_query_latency{tenant_id}` - Query latency per tenant

### Alerts

- Tenant approaching storage quota (80%, 95%)
- Tenant rate limit exceeded
- Tenant suspended due to quota violation
- Cross-tenant access attempt detected

---

## Security Considerations

1. **Key Prefix Validation** - Always validate tenant ID in keys
2. **Metadata Protection** - Restrict access to tenant metadata
3. **Audit Logging** - Log all tenant operations
4. **Quota Enforcement** - Never bypass quota checks
5. **Isolation Testing** - Regular security audits

---

## Performance Optimization

1. **Cache tenant metadata** in memory
2. **Batch quota updates** to reduce contention
3. **Use approximate counting** for high-frequency metrics
4. **Optimize key encoding** with SIMD instructions
5. **Pre-allocate buffers** for key encoding

---

## References

- [ADR-0025: Multi-Tenancy and Isolation](../ADR/ADR-0025-Multi-Tenancy-and-Isolation.md)
- [ADR-0026: Resource Quotas and Limits](../ADR/ADR-0026-Resource-Quotas-and-Limits.md)
- [ADR-0010: Authentication, Authorization, and Access Control](../ADR/ADR-0010-Authentication-Authorization-Access-Control.md)
- [Implementation Plan](IMPLEMENTATION_PLAN.md)

---

## Summary: Layered Architecture with Shard-Level Isolation

The multi-tenant architecture provides optimal security, performance, and developer experience through three key design decisions:

### 1. Shard-Level Physical Isolation

**No Key Prefixing Required**
- User keys stored as-is in shards (zero overhead)
- Each shard is a physically separate key-value store
- Shard ID encodes table: `ShardId::from_parts(table_id, shard_index)`
- Table metadata links to container (tenant + database)
- Physical separation provides complete isolation

**Benefits:**
- ✅ Zero prefix overhead (no wasted bytes per key)
- ✅ Maximum performance (no encoding/decoding)
- ✅ Simpler implementation (no key transformation logic)
- ✅ Better cache locality (shorter keys)
- ✅ Stronger isolation (physical separation vs logical prefixes)

### 2. Three-Layer Facade Architecture

**Layer 1: Inner Core** (`KeyValueDatabaseManagerCore`)
- Internal implementation (`pub(crate)` - not exposed)
- Raw data operations with no validation
- Direct shard access
- Trusts caller for security

**Layer 2: Outer Facade** (`KeyValueDatabaseManager`)
- Public API for system management
- Tenant/database/tablespace operations
- Container handle factory
- **Does NOT expose data operations** (security by design)

**Layer 3: Container Handle** (`ContainerHandle`)
- Scoped facade for container-level operations
- Validates quotas before delegating to core
- Records usage metrics
- Automatic tenant/database context

**Security Properties:**
- ✅ Impossible to bypass tenant isolation from public API
- ✅ Container validation happens once at handle creation
- ✅ Internal core hidden from external access
- ✅ Quota enforcement before operations
- ✅ Complete audit trail through usage tracker

### 3. Developer Experience

**Clean API:**
```rust
// Get container handle (validates access once)
let container = db_manager.container(tenant_id, database_id)?;

// Clean operations - no repetitive parameters
container.put(namespace, table, b"key", b"value").await?;
let value = container.get(namespace, table, b"key").await?;
```

**Type Safety:**
- Container context bound at handle creation
- Compile-time guarantees about operation scope
- Impossible to use wrong container ID

**Performance:**
- Single validation per handle (not per operation)
- Bound metadata cache per handle
- Zero key encoding overhead
- Reduced lock contention

### Implementation Path

1. **Phase 1**: Core types and metadata (Week 1)
   - Add TenantId, DatabaseId, ContainerId types
   - Implement system metadata storage
   - Create metadata caches

2. **Phase 2**: Layered architecture (Week 2)
   - Implement KeyValueDatabaseManagerCore (internal)
   - Implement KeyValueDatabaseManager (public facade)
   - Implement ContainerHandle
   - No key encoding needed!

3. **Phase 3**: Resource quotas (Week 3)
   - Implement UsageTracker
   - Add quota enforcement in ContainerHandle
   - Monitoring and alerting

4. **Phase 4**: Access control integration (Week 4)
   - Extend ABAC with tenant context
   - Integrate with authentication system
   - Audit logging

### Key Advantages

This architecture provides:

1. **Maximum Performance**: Zero key encoding overhead
2. **Strongest Isolation**: Physical shard separation
3. **Best Security**: Impossible to bypass tenant boundaries
4. **Cleanest API**: Container-scoped operations
5. **Type Safety**: Compile-time guarantees
6. **Simplest Implementation**: No complex key encoding logic

The combination of shard-level physical isolation and layered facade architecture ensures Nanograph's multi-tenant system is secure, performant, and maintainable.

---

**Document Version:** 1.1
**Last Updated:** 2026-01-14
**Status:** Ready for Implementation