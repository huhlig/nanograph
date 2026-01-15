---
parent: ADR
nav_order: 0025
title: Multi-Tenancy and Isolation (Revised)
status: proposed
date: 2026-01-11
deciders: Hans W. Uhlig
---

# ADR-0025: Multi-Tenancy and Isolation (Revised)

## Status

Proposed

## Context

Nanograph needs multi-tenant support with efficient key encoding and clear separation of concerns:
- **Tenant**: Security isolation boundary
- **Database**: Top-level data container within tenant
- **Namespace/Table**: Flat internal logical organization (both use ObjectId)

## Decision

Implement **simplified hierarchical multi-tenancy** with efficient key encoding:

### Hierarchy

```
Cluster
  └─ Tenant (32-bit) - Security isolation boundary
      └─ Database (32-bit) - Data AND Compute container
          └─ Namespace (64-bit ObjectId) - Logical grouping
          └─ Table (64-bit ObjectId) - Data storage
```

**Key Insights:**
- Namespaces and Tables are peers at the same level, both identified by ObjectId
- Namespaces provide logical grouping but don't nest
- **Database is the unit of both data AND compute isolation**

### Key Encoding (16 bytes prefix)

```
Key Format: {tenant_id:4}{database_id:4}{object_id:8}{user_key}
            |<--- 16 bytes prefix --->|<-- variable -->|

Example:
  Tenant 1, DB 2, Table 42, Key "user:123"
  → 0x00000001_00000002_000000000000002A_user:123
  
  Tenant 1, DB 2, Namespace 100
  → 0x00000001_00000002_0000000000000064
```

**Benefits:**
- 50% smaller prefix (16 bytes vs 32 bytes)
- Simpler hierarchy (3 levels vs 5)
- Flat namespace/table structure (easier to manage)
- Still provides complete isolation

## Design

### 1. Core Types

```rust
// nanograph-core/src/types.rs

/// Tenant identifier - security isolation boundary (32-bit)
#[derive(Copy, Clone, Debug, Ord, PartialOrd, Eq, PartialEq, Hash)]
pub struct TenantId(pub u32);

impl TenantId {
    pub fn new(id: u32) -> Self {
        Self(id)
    }
    
    pub fn as_u32(&self) -> u32 {
        self.0
    }
    
    /// System tenant (ID 0) for internal metadata
    pub const SYSTEM: TenantId = TenantId(0);
}

/// Database identifier - top-level data container (32-bit)
#[derive(Copy, Clone, Debug, Ord, PartialOrd, Eq, PartialEq, Hash)]
pub struct DatabaseId(pub u32);

impl DatabaseId {
    pub fn new(id: u32) -> Self {
        Self(id)
    }
    
    pub fn as_u32(&self) -> u32 {
        self.0
    }
    
    /// Default database (ID 0) created with each tenant
    pub const DEFAULT: DatabaseId = DatabaseId(0);
}

/// ObjectId (64-bit) - Used for both Namespaces and Tables
/// Already defined in types.rs as: pub type ObjectId = u64;

/// Namespace identifier (uses ObjectId)
#[derive(Copy, Clone, Debug, Ord, PartialOrd, Eq, PartialEq, Hash)]
pub struct NamespaceId(pub ObjectId);

/// Table identifier (uses ObjectId)
#[derive(Copy, Clone, Debug, Ord, PartialOrd, Eq, PartialEq, Hash)]
pub struct TableId(pub ObjectId);

/// Hierarchical resource path
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct ResourcePath {
    pub tenant: TenantId,
    pub database: DatabaseId,
    pub object: Option<ObjectId>, // Namespace or Table
}

impl ResourcePath {
    /// Parse from string: "tenant.database.object"
    pub fn parse(path: &str) -> Result<Self, ParseError> {
        let parts: Vec<&str> = path.split('.').collect();
        
        if parts.len() < 2 {
            return Err(ParseError::InvalidFormat);
        }
        
        let tenant = parts[0].parse::<u32>()
            .map(TenantId::new)
            .map_err(|_| ParseError::InvalidTenantId)?;
            
        let database = parts[1].parse::<u32>()
            .map(DatabaseId::new)
            .map_err(|_| ParseError::InvalidDatabaseId)?;
        
        let object = if parts.len() > 2 {
            Some(parts[2].parse::<u64>()
                .map_err(|_| ParseError::InvalidObjectId)?)
        } else {
            None
        };
        
        Ok(Self {
            tenant,
            database,
            object,
        })
    }
    
    /// Convert to string representation
    pub fn to_string(&self) -> String {
        let mut path = format!("{}.{}", self.tenant.0, self.database.0);
        
        if let Some(obj) = self.object {
            path.push_str(&format!(".{}", obj));
        }
        
        path
    }
}
```

### 2. Key Encoding (16-byte prefix)

```rust
// nanograph-kvt/src/key_encoding.rs

/// Encode a key with tenant/database/object prefix (16 bytes)
pub fn encode_key(
    tenant: TenantId,
    database: DatabaseId,
    object: ObjectId, // Namespace or Table
    user_key: &[u8],
) -> Vec<u8> {
    let mut encoded = Vec::with_capacity(16 + user_key.len());
    
    // Tenant ID (4 bytes)
    encoded.extend_from_slice(&tenant.as_u32().to_be_bytes());
    
    // Database ID (4 bytes)
    encoded.extend_from_slice(&database.as_u32().to_be_bytes());
    
    // Object ID (8 bytes) - Namespace or Table
    encoded.extend_from_slice(&object.to_be_bytes());
    
    // User key (variable length)
    encoded.extend_from_slice(user_key);
    
    encoded
}

/// Decode a key to extract tenant/database/object and user key
pub fn decode_key(encoded: &[u8]) -> Result<(TenantId, DatabaseId, ObjectId, &[u8]), DecodeError> {
    if encoded.len() < 16 {
        return Err(DecodeError::TooShort);
    }
    
    let tenant = TenantId::new(u32::from_be_bytes(encoded[0..4].try_into().unwrap()));
    let database = DatabaseId::new(u32::from_be_bytes(encoded[4..8].try_into().unwrap()));
    let object = u64::from_be_bytes(encoded[8..16].try_into().unwrap());
    let user_key = &encoded[16..];
    
    Ok((tenant, database, object, user_key))
}

/// Create a range prefix for tenant-scoped scans (4 bytes)
pub fn tenant_prefix(tenant: TenantId) -> Vec<u8> {
    tenant.as_u32().to_be_bytes().to_vec()
}

/// Create a range prefix for database-scoped scans (8 bytes)
pub fn database_prefix(tenant: TenantId, database: DatabaseId) -> Vec<u8> {
    let mut prefix = Vec::with_capacity(8);
    prefix.extend_from_slice(&tenant.as_u32().to_be_bytes());
    prefix.extend_from_slice(&database.as_u32().to_be_bytes());
    prefix
}

/// Create a range prefix for object-scoped scans (16 bytes)
pub fn object_prefix(
    tenant: TenantId,
    database: DatabaseId,
    object: ObjectId,
) -> Vec<u8> {
    let mut prefix = Vec::with_capacity(16);
    prefix.extend_from_slice(&tenant.as_u32().to_be_bytes());
    prefix.extend_from_slice(&database.as_u32().to_be_bytes());
    prefix.extend_from_slice(&object.to_be_bytes());
    prefix
}
```

### 3. Metadata Structure

```rust
// nanograph-kvt/src/types.rs

/// Tenant metadata (security isolation)
#[derive(Debug, Clone)]
pub struct TenantMetadata {
    pub id: TenantId,
    pub name: String,
    pub created_at: Timestamp,
    pub last_modified: Timestamp,
    pub isolation_mode: IsolationMode,
    pub databases: Vec<DatabaseId>,
    pub status: TenantStatus,
}

/// Database metadata (data AND compute container)
#[derive(Debug, Clone)]
pub struct DatabaseMetadata {
    pub id: DatabaseId,
    pub tenant: TenantId,
    pub name: String,
    pub created_at: Timestamp,
    pub last_modified: Timestamp,
    
    // Data isolation
    pub storage_quota: Option<u64>,
    pub namespaces: Vec<NamespaceId>,
    pub tables: Vec<TableId>,
    
    // Compute isolation
    pub compute_quotas: ComputeQuotas,
    pub max_connections: Option<u32>,
    pub max_concurrent_queries: Option<u32>,
    pub max_query_time_ms: Option<u64>,
    pub max_memory_bytes: Option<u64>,
}

/// Compute quotas per database
#[derive(Debug, Clone)]
pub struct ComputeQuotas {
    /// Maximum CPU time per query (milliseconds)
    pub max_query_time_ms: Option<u64>,
    
    /// Maximum memory per query
    pub max_query_memory_bytes: Option<u64>,
    
    /// Maximum concurrent queries
    pub max_concurrent_queries: Option<u32>,
    
    /// Maximum IOPS (operations per second)
    pub max_iops: Option<u32>,
    
    /// Maximum connections
    pub max_connections: Option<u32>,
}

/// Namespace metadata (logical grouping, same level as tables)
#[derive(Debug, Clone)]
pub struct NamespaceMetadata {
    pub id: NamespaceId,
    pub database: DatabaseId,
    pub tenant: TenantId,
    pub name: String,
    pub created_at: Timestamp,
    pub last_modified: Timestamp,
}

/// Table metadata (data storage, same level as namespaces)
#[derive(Debug, Clone)]
pub struct TableMetadata {
    pub id: TableId,
    pub database: DatabaseId,
    pub tenant: TenantId,
    pub name: String,
    pub namespace: Option<NamespaceId>, // Optional: table can belong to namespace
    pub created_at: Timestamp,
    pub last_modified: Timestamp,
    pub engine_type: StorageEngineType,
    pub sharding: TableSharding,
}
```

### 4. Simplified Hierarchy Benefits

**Tenant (32-bit) - Security Isolation**
- Security boundary
- Authentication and authorization scope
- Access control enforcement
- Audit logging scope
- Billing and cost allocation unit

**Database (32-bit) - Data AND Compute Container**
- **Data isolation**: Logical separation of data
- **Compute isolation**: Resource quotas (CPU, memory, IOPS, connections)
- **Operational unit**: Backup/restore, migration, replication
- **Query scope**: Queries execute within database context
- **Connection pool**: Per-database connection limits
- **Transaction scope**: Transactions are database-scoped

**Namespace/Table (64-bit ObjectId) - Logical Organization**
- Flat structure (no nesting)
- Namespaces provide logical grouping
- Tables can optionally belong to namespaces
- Both use same ObjectId space
- Simpler to manage and reason about

### 5. Example Usage

```rust
// Create tenant
let tenant = db.create_tenant(TenantConfig {
    name: "acme-corp".to_string(),
    isolation_mode: IsolationMode::Logical,
    resource_quotas: ResourceQuotas::starter_tier(),
}).await?;

// Create database
let database = db.create_database(tenant, DatabaseConfig {
    name: "production".to_string(),
    storage_quota: Some(100 * 1024 * 1024 * 1024), // 100 GB
}).await?;

// Create namespace (optional logical grouping)
let namespace = db.create_namespace(tenant, database, NamespaceConfig {
    name: "users".to_string(),
}).await?;

// Create table (can be in namespace or standalone)
let table = db.create_table(tenant, database, TableConfig {
    name: "accounts".to_string(),
    namespace: Some(namespace), // Optional
    engine_type: StorageEngineType::LSM,
    sharding: TableSharding::Single,
}).await?;

// Write data (16-byte prefix)
db.put(tenant, database, table.id.0, b"user:123", b"data").await?;

// Read data
let value = db.get(tenant, database, table.id.0, b"user:123").await?;
```

## Consequences

### Positive

* **Efficient**: 16-byte prefix vs 32-byte (50% reduction)
* **Simple**: 3-level hierarchy vs 5-level
* **Flexible**: Namespaces optional, tables can be standalone
* **Scalable**: 32-bit tenant/database = 4B each, 64-bit objects = 18 quintillion
* **Clear**: Tenant=security, Database=data container, Namespace/Table=organization
* **Fast**: Smaller keys = better cache utilization

### Negative

* **No nested namespaces**: Flat structure only (acceptable trade-off)
* **Shared ObjectId space**: Namespaces and tables share ID space (not an issue)

### Capacity

- **Tenants**: 4,294,967,296 (4.3 billion)
- **Databases per tenant**: 4,294,967,296 (4.3 billion)
- **Objects per database**: 18,446,744,073,709,551,616 (18 quintillion)

## Implementation Notes

### Key Encoding Efficiency

```
Old design: 32 bytes prefix
  8 bytes tenant
  8 bytes database  
  8 bytes namespace
  8 bytes table

New design: 16 bytes prefix (50% reduction!)
  4 bytes tenant
  4 bytes database
  8 bytes object (namespace or table)
```

### Resolver Simplification

The flat namespace/table structure simplifies the resolver:

```
Old: tenant.database.namespace.table (4 levels)
New: tenant.database.object (3 levels)
```

Objects (namespaces and tables) are peers, making the hierarchy easier to manage.

## Related ADRs

* [ADR-0026: Resource Quotas and Limits](ADR-0026-Resource-Quotas-and-Limits.md)
* [ADR-0010: Authentication, Authorization, and Access Control](ADR-0010-Authentication-Authorization-Access-Control.md)
* [ADR-0007: Clustering, Sharding, Replication, and Consensus](ADR-0007-Clustering-Sharding-Replication-Consensus.md)

---

**Revision Notes:**
- Reduced key prefix from 32 bytes to 16 bytes (50% improvement)
- Simplified hierarchy from 5 levels to 3 levels
- Made namespaces and tables peers (flat structure)
- Tenant and Database use 32-bit IDs (sufficient capacity)
- Objects (namespaces/tables) use 64-bit ObjectId
- Clearer separation of concerns: Tenant=security, Database=data, Objects=organization