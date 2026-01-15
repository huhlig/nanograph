---
parent: ADR
nav_order: 0025
title: Multi-Tenancy and Isolation
status: proposed
date: 2026-01-11
deciders: Hans W. Uhlig
---

# ADR-0025: Multi-Tenancy and Isolation

## Status

Proposed

## Context

Nanograph needs to support multiple tenants (organizations, applications, or users) sharing the same infrastructure while maintaining strong isolation guarantees. This is critical for:

1. **SaaS Deployments** - Multiple customers on shared infrastructure
2. **Enterprise Use Cases** - Departmental isolation within organizations
3. **Cost Efficiency** - Resource sharing while maintaining security
4. **Operational Simplicity** - Unified management across tenants
5. **Compliance** - Data residency and regulatory requirements

Current architecture has:
- Namespace concept (underutilized)
- ABAC/PBAC authentication (ADR-0010)
- Sharding and replication (ADR-0007)
- No explicit tenant boundaries

Without proper multi-tenancy:
- Security risks from data leakage
- No resource isolation (noisy neighbor problems)
- Difficult billing and quota management
- Complex compliance and audit requirements

## Decision

Implement **hierarchical multi-tenancy** with three levels of isolation:

### 1. Tenant as Primary Isolation Boundary

```
Cluster
  └─ Tenant (NEW - Primary isolation boundary)
      └─ Database (NEW - Logical grouping within tenant)
          └─ Namespace (Existing - Schema/organization)
              └─ Table (Existing - Data container)
                  └─ Shard (Existing - Physical partition)
```

### 2. Three Isolation Dimensions

#### A. User Isolation (Identity & Access)
- Tenant-scoped authentication
- ABAC policies enforced at tenant boundary
- Per-tenant API keys and credentials
- Separate audit logs per tenant

#### B. Data Isolation (Storage & Metadata)
- Tenant ID prefix on all keys
- Logical isolation via key prefixing (default)
- Optional physical isolation via dedicated shards
- Separate metadata per tenant

#### C. Compute Isolation (Resources & Performance)
- Per-tenant resource quotas
- Query execution budgets
- Rate limiting at tenant level
- Optional dedicated node pools

### 3. Isolation Modes

Support multiple isolation levels based on tenant requirements:

```rust
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
```

## Decision Drivers

* **Security First** - Complete data isolation between tenants
* **Scalability** - Support thousands of tenants efficiently
* **Flexibility** - Multiple isolation levels for different needs
* **Economics** - Enable SaaS pricing and cost allocation
* **Operations** - Tenant-level management and monitoring
* **Compliance** - Meet regulatory requirements (GDPR, HIPAA, etc.)

## Design

### 1. Core Type System

```rust
// nanograph-core/src/types.rs

/// Tenant identifier - top-level isolation boundary
#[derive(Copy, Clone, Debug, Ord, PartialOrd, Eq, PartialEq, Hash)]
pub struct TenantId(pub u64);

/// Database identifier - logical grouping within tenant
#[derive(Copy, Clone, Debug, Ord, PartialOrd, Eq, PartialEq, Hash)]
pub struct DatabaseId(pub u64);

/// Hierarchical resource identifier
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct ResourcePath {
    pub tenant: TenantId,
    pub database: DatabaseId,
    pub namespace: Option<NamespaceId>,
    pub table: Option<TableId>,
}

impl ResourcePath {
    /// Parse from string: "tenant.database.namespace.table"
    pub fn parse(path: &str) -> Result<Self, ParseError> {
        // Implementation
    }
    
    /// Convert to string representation
    pub fn to_string(&self) -> String {
        // Implementation
    }
}
```

### 2. Key Encoding Strategy

All keys are prefixed with tenant and database identifiers:

```
Key Format: {tenant_id:8}{database_id:8}{namespace_id:8}{table_id:8}{user_key}
            |<-------- 32 bytes prefix -------->|<-- variable -->|

Example:
  Tenant 1, DB 2, NS 3, Table 4, Key "user:123"
  → 0x0000000000000001_0000000000000002_0000000000000003_0000000000000004_user:123
```

Benefits:
- Automatic data isolation at storage layer
- Efficient tenant-scoped range scans
- No cross-tenant data leakage possible
- Enables tenant-level backup/restore

### 3. Metadata Structure

```rust
// nanograph-kvt/src/types.rs

/// Tenant configuration and metadata
pub struct TenantMetadata {
    pub id: TenantId,
    pub name: String,
    pub created_at: Timestamp,
    pub isolation_mode: IsolationMode,
    pub resource_quotas: ResourceQuotas,
    pub databases: Vec<DatabaseId>,
    pub status: TenantStatus,
}

pub enum TenantStatus {
    Active,
    Suspended,
    Migrating,
    Archived,
}

/// Database configuration within a tenant
pub struct DatabaseMetadata {
    pub id: DatabaseId,
    pub tenant: TenantId,
    pub name: String,
    pub created_at: Timestamp,
    pub namespaces: Vec<NamespaceId>,
    pub storage_quota: Option<u64>,
}

/// Resource quotas per tenant
pub struct ResourceQuotas {
    /// Maximum storage in bytes
    pub max_storage_bytes: Option<u64>,
    
    /// Maximum number of databases
    pub max_databases: Option<u32>,
    
    /// Maximum number of tables
    pub max_tables: Option<u32>,
    
    /// Maximum concurrent connections
    pub max_connections: Option<u32>,
    
    /// Maximum IOPS (operations per second)
    pub max_iops: Option<u32>,
    
    /// Maximum query execution time (milliseconds)
    pub max_query_time_ms: Option<u64>,
}
```

### 4. Access Control Integration

Extend ADR-0010 ABAC with tenant context:

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

pub enum AccessDecision {
    Allow,
    Deny { reason: String },
}
```

### 5. Database Manager Updates

```rust
// nanograph-kvm/src/container

impl KeyValueDatabaseManager {
    /// Create a new tenant
    pub async fn create_tenant(
        &self,
        name: String,
        config: TenantConfig,
    ) -> KeyValueResult<TenantId> {
        // Validate tenant name uniqueness
        // Allocate tenant ID
        // Create tenant metadata
        // Initialize default database
        // Set up resource tracking
    }
    
    /// Create a database within a tenant
    pub async fn create_database(
        &self,
        tenant: TenantId,
        name: String,
        config: DatabaseConfig,
    ) -> KeyValueResult<DatabaseId> {
        // Verify tenant exists and is active
        // Check tenant quotas
        // Allocate database ID
        // Create database metadata
        // Update tenant metadata
    }
    
    /// Put with tenant context
    pub async fn put(
        &self,
        tenant: TenantId,
        database: DatabaseId,
        table: TableId,
        key: &[u8],
        value: &[u8],
    ) -> KeyValueResult<()> {
        // Verify tenant access
        // Check resource quotas
        // Encode key with tenant prefix
        // Route to appropriate shard
        // Update usage metrics
    }
    
    /// Get with tenant context
    pub async fn get(
        &self,
        tenant: TenantId,
        database: DatabaseId,
        table: TableId,
        key: &[u8],
    ) -> KeyValueResult<Option<Vec<u8>>> {
        // Verify tenant access
        // Encode key with tenant prefix
        // Route to appropriate shard
        // Update usage metrics
    }
}
```

### 6. Resource Tracking and Enforcement

```rust
pub struct ResourceTracker {
    tenant_usage: Arc<RwLock<HashMap<TenantId, TenantUsage>>>,
}

pub struct TenantUsage {
    pub storage_bytes: AtomicU64,
    pub table_count: AtomicU32,
    pub connection_count: AtomicU32,
    pub iops_current: AtomicU32,
    pub last_updated: Timestamp,
}

impl ResourceTracker {
    /// Check if operation is allowed under quotas
    pub fn check_quota(
        &self,
        tenant: TenantId,
        operation: Operation,
    ) -> Result<(), QuotaExceeded> {
        let usage = self.tenant_usage.read().unwrap();
        let tenant_usage = usage.get(&tenant)?;
        let quotas = self.get_tenant_quotas(tenant)?;
        
        match operation {
            Operation::Write { size } => {
                if let Some(max) = quotas.max_storage_bytes {
                    if tenant_usage.storage_bytes.load(Ordering::Relaxed) + size > max {
                        return Err(QuotaExceeded::Storage);
                    }
                }
            }
            Operation::Connect => {
                if let Some(max) = quotas.max_connections {
                    if tenant_usage.connection_count.load(Ordering::Relaxed) >= max {
                        return Err(QuotaExceeded::Connections);
                    }
                }
            }
            // ... other operations
        }
        
        Ok(())
    }
    
    /// Update usage metrics
    pub fn record_usage(&self, tenant: TenantId, metric: UsageMetric) {
        // Update atomic counters
        // Trigger alerts if approaching limits
    }
}
```

### 7. Shard Assignment Strategies

```rust
pub enum ShardAssignmentStrategy {
    /// All tenants share shards (default)
    Shared,
    
    /// Tenant gets dedicated shards
    Dedicated {
        shard_count: u32,
        replication_factor: u32,
    },
    
    /// Hybrid: small tenants share, large get dedicated
    Hybrid {
        threshold_storage_gb: u64,
        threshold_iops: u32,
    },
}

impl ShardManager {
    /// Assign shards based on tenant requirements
    pub fn assign_shards(
        &mut self,
        tenant: TenantId,
        strategy: ShardAssignmentStrategy,
    ) -> Result<Vec<ShardId>> {
        match strategy {
            ShardAssignmentStrategy::Shared => {
                // Use existing shared shards
                self.get_shared_shards()
            }
            ShardAssignmentStrategy::Dedicated { shard_count, replication_factor } => {
                // Create dedicated shards for this tenant
                self.create_dedicated_shards(tenant, shard_count, replication_factor)
            }
            ShardAssignmentStrategy::Hybrid { .. } => {
                // Evaluate tenant size and assign accordingly
                self.evaluate_and_assign(tenant)
            }
        }
    }
}
```

### 8. Migration and Rebalancing

```rust
pub struct TenantMigration {
    pub tenant: TenantId,
    pub from_mode: IsolationMode,
    pub to_mode: IsolationMode,
    pub status: MigrationStatus,
}

impl TenantMigration {
    /// Migrate tenant from shared to dedicated shards
    pub async fn execute(&mut self) -> Result<()> {
        match (&self.from_mode, &self.to_mode) {
            (IsolationMode::Logical, IsolationMode::DedicatedShards) => {
                // 1. Create new dedicated shards
                let new_shards = self.create_dedicated_shards().await?;
                
                // 2. Copy tenant data to new shards
                self.copy_tenant_data(new_shards).await?;
                
                // 3. Switch routing to new shards
                self.update_routing().await?;
                
                // 4. Clean up old data
                self.cleanup_old_data().await?;
            }
            // ... other migration paths
            _ => unimplemented!(),
        }
        
        Ok(())
    }
}
```

## Consequences

### Positive

* **Strong Security** - Complete data isolation between tenants
* **Flexible Isolation** - Multiple modes for different requirements
* **Resource Control** - Quotas prevent noisy neighbor problems
* **Cost Allocation** - Per-tenant usage tracking enables billing
* **Operational Efficiency** - Tenant-level management and monitoring
* **Compliance Ready** - Supports data residency and audit requirements
* **Scalability** - Efficient support for thousands of tenants
* **Migration Path** - Can upgrade isolation level as tenant grows

### Negative

* **Key Overhead** - 32-byte prefix on every key (mitigated by compression)
* **Complexity** - Additional layer in hierarchy
* **Metadata Size** - Per-tenant metadata storage
* **Migration Cost** - Moving tenants between isolation modes
* **Testing Burden** - Must test multi-tenant scenarios

### Risks

* **Quota Enforcement** - Must be bulletproof to prevent abuse
* **Metadata Bottleneck** - Tenant metadata access must be fast
* **Migration Failures** - Tenant migrations must be atomic
* **Key Encoding Bugs** - Errors could cause data leakage

## Alternatives Considered

### 1. No Multi-Tenancy (Single Tenant per Cluster)

**Rejected** - Prohibitively expensive for SaaS deployments, poor resource utilization.

### 2. Application-Level Isolation Only

**Rejected** - Relies on application correctness, no defense in depth, difficult to audit.

### 3. Database-Level Isolation (No Tenant Concept)

**Rejected** - Insufficient for true multi-tenancy, no cross-database resource management.

### 4. Schema-Based Isolation (PostgreSQL-style)

**Rejected** - Too coarse-grained, difficult to enforce quotas, limited isolation.

### 5. Separate Clusters per Tenant

**Rejected** - Operationally complex, expensive, doesn't scale to many tenants.

## Implementation Plan

### Phase 1: Core Types and Metadata (Week 1-2)
- Add TenantId and DatabaseId types
- Extend Metastore with tenant/database metadata
- Update resolver to support tenant.database.namespace.table paths
- Add tenant configuration storage

### Phase 2: Key Encoding and Storage (Week 3-4)
- Implement key prefixing with tenant/database IDs
- Update all storage engines to handle prefixed keys
- Add tenant-scoped range scan support
- Test data isolation guarantees

### Phase 3: Access Control Integration (Week 5-6)
- Extend ABAC with tenant context
- Add tenant boundary enforcement
- Implement tenant-aware routing
- Add audit logging per tenant

### Phase 4: Resource Management (Week 7-8)
- Implement quota tracking
- Add admission control
- Create resource monitoring
- Add alerting for quota violations

### Phase 5: Advanced Isolation (Week 9-10)
- Implement dedicated shard assignment
- Add tenant migration support
- Create hybrid isolation mode
- Add tenant lifecycle management

### Phase 6: Operations and Tooling (Week 11-12)
- Tenant management CLI
- Monitoring dashboards
- Backup/restore per tenant
- Migration tools

## Related ADRs

* [ADR-0007: Clustering, Sharding, Replication, and Consensus](ADR-0007-Clustering-Sharding-Replication-Consensus.md)
* [ADR-0010: Authentication, Authorization, and Access Control](ADR-0010-Authentication-Authorization-Access-Control.md)
* [ADR-0026: Resource Quotas and Limits](ADR-0026-Resource-Quotas-and-Limits.md) (to be created)
* [ADR-0011: Observability, Telemetry, and Auditing](ADR-0011-Observability-Telemetry-Auditing.md)

## References

* AWS DynamoDB multi-tenancy patterns
* Azure Cosmos DB tenant isolation
* Salesforce multi-tenant architecture
* Stripe API tenant design
* PostgreSQL row-level security

---

**Next Steps:**
1. Review and approve ADR
2. Create ADR-0026 for Resource Quotas
3. Update implementation plan
4. Begin Phase 1 implementation
5. Create migration guide for existing deployments