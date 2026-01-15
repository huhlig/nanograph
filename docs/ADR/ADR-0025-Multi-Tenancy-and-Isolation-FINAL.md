---
parent: ADR
nav_order: 0025
title: Multi-Tenancy and Isolation (Final Design)
status: proposed
date: 2026-01-11
deciders: Hans W. Uhlig
---

# ADR-0025: Multi-Tenancy and Isolation (Final Design)

## Status

Proposed

## Context

Nanograph needs multi-tenancy with efficient metadata management and flexible compute isolation. Key requirements:
- **Global metadata**: Tenant, Database, User information accessible cluster-wide
- **Container-scoped metadata**: Namespaces, Tables, Functions only on assigned nodes
- **Efficient key encoding**: 16-byte prefix for data keys
- **Clear separation**: Control plane vs data plane

## Decision

Implement **two-tier metadata architecture** with separate shard groups:

### 1. Global Metadata Shard Group (Control Plane)
**Replicated across ALL nodes in cluster**
- Tenants
- Databases (containers)
- System Users
- Container assignments (which nodes host which databases)

### 2. Container Metadata Shard Groups (Data Plane)
**Replicated only on nodes assigned to that database/container**
- Namespaces
- Tables
- Functions
- Indexes
- Database-specific configuration

## Architecture

### Hierarchy

```
Cluster
  └─ Global Metadata Shard Group (ALL nodes)
      ├─ Tenant (32-bit) - Security isolation
      ├─ Database/Container (32-bit) - Data + Compute isolation
      └─ System Users - Authentication
  
  └─ Container Metadata Shard Groups (ASSIGNED nodes only)
      ├─ Namespace (64-bit ObjectId) - Logical grouping
      ├─ Table (64-bit ObjectId) - Data storage
      └─ Function (64-bit ObjectId) - Stored procedures
```

### Shard Group Design

```
┌─────────────────────────────────────────────────────────┐
│                    Cluster                               │
├─────────────────────────────────────────────────────────┤
│                                                          │
│  ┌────────────────────────────────────────────────┐    │
│  │   Global Metadata Shard Group                  │    │
│  │   (Replicated on ALL nodes)                    │    │
│  │                                                 │    │
│  │   - Tenants                                     │    │
│  │   - Databases/Containers                        │    │
│  │   - System Users                                │    │
│  │   - Container → Node Assignments                │    │
│  └────────────────────────────────────────────────┘    │
│                                                          │
│  ┌────────────────────────────────────────────────┐    │
│  │   Container Metadata Shard Group (DB 1)        │    │
│  │   (Replicated on nodes: [1, 2, 3])             │    │
│  │                                                 │    │
│  │   - Namespaces for DB 1                         │    │
│  │   - Tables for DB 1                             │    │
│  │   - Functions for DB 1                          │    │
│  └────────────────────────────────────────────────┘    │
│                                                          │
│  ┌────────────────────────────────────────────────┐    │
│  │   Container Metadata Shard Group (DB 2)        │    │
│  │   (Replicated on nodes: [4, 5, 6])             │    │
│  │                                                 │    │
│  │   - Namespaces for DB 2                         │    │
│  │   - Tables for DB 2                             │    │
│  │   - Functions for DB 2                          │    │
│  └────────────────────────────────────────────────┘    │
│                                                          │
│  ┌────────────────────────────────────────────────┐    │
│  │   Data Shard Groups (per table)                │    │
│  │   (Replicated based on table config)            │    │
│  │                                                 │    │
│  │   - Actual user data                            │    │
│  │   - Partitioned by key                          │    │
│  └────────────────────────────────────────────────┘    │
└─────────────────────────────────────────────────────────┘
```

## Design Details

### 1. Global Metadata Shard Group

**Purpose:** Cluster-wide control plane metadata

**Replication:** Fully replicated on ALL nodes

**Contents:**
```rust
// Stored in special system tables with reserved IDs

// System Table 0: Tenants
struct TenantRecord {
    id: TenantId,              // 32-bit
    name: String,
    status: TenantStatus,
    created_at: Timestamp,
    resource_quotas: ResourceQuotas,
}

// System Table 1: Databases/Containers
struct DatabaseRecord {
    id: DatabaseId,            // 32-bit
    tenant: TenantId,
    name: String,
    status: DatabaseStatus,
    created_at: Timestamp,
    compute_quotas: ComputeQuotas,
    assigned_nodes: Vec<NodeId>,  // Which nodes host this database
}

// System Table 2: System Users
struct UserRecord {
    id: UserId,
    tenant: TenantId,
    username: String,
    auth_data: AuthData,
    permissions: Vec<Permission>,
}

// System Table 3: Container Assignments
struct ContainerAssignment {
    database: DatabaseId,
    nodes: Vec<NodeId>,
    replication_factor: u32,
    status: AssignmentStatus,
}
```

**Access Pattern:**
- Read frequently (every request needs tenant/database validation)
- Write infrequently (only on admin operations)
- Small dataset (thousands to millions of records)
- Cached aggressively in memory on all nodes

### 2. Container Metadata Shard Groups

**Purpose:** Database-specific metadata

**Replication:** Only on nodes assigned to that database

**Contents:**
```rust
// Each database has its own metadata shard group

// Container Metadata Table 0: Namespaces
struct NamespaceRecord {
    id: ObjectId,              // 64-bit
    database: DatabaseId,
    name: String,
    created_at: Timestamp,
}

// Container Metadata Table 1: Tables
struct TableRecord {
    id: ObjectId,              // 64-bit
    database: DatabaseId,
    namespace: Option<ObjectId>,
    name: String,
    engine_type: StorageEngineType,
    sharding_config: ShardingConfig,
    created_at: Timestamp,
}

// Container Metadata Table 2: Functions
struct FunctionRecord {
    id: ObjectId,              // 64-bit
    database: DatabaseId,
    name: String,
    language: FunctionLanguage,
    code: Vec<u8>,
    created_at: Timestamp,
}

// Container Metadata Table 3: Indexes
struct IndexRecord {
    id: ObjectId,
    table: ObjectId,
    name: String,
    index_type: IndexType,
    columns: Vec<String>,
}
```

**Access Pattern:**
- Read frequently (query planning, execution)
- Write occasionally (DDL operations)
- Medium dataset (thousands to millions per database)
- Cached in memory on assigned nodes only

### 3. Data Shard Groups

**Purpose:** Actual user data

**Replication:** Based on table configuration

**Key Encoding:** 16-byte prefix
```
{tenant:4}{database:4}{object:8}{user_key}
```

## Benefits

### 1. **Efficient Metadata Distribution**

**Global Metadata:**
- Small, frequently accessed
- Replicated everywhere for fast access
- No network hops for tenant/database validation

**Container Metadata:**
- Larger, database-specific
- Only on nodes that need it
- Reduces memory footprint on other nodes

### 2. **Flexible Container Assignment**

```rust
// Assign database to specific nodes
cluster.assign_database(
    database_id,
    nodes: vec![node1, node2, node3],
    replication_factor: 3,
);

// Container metadata only exists on these nodes
// Data shards can be placed on these or other nodes
```

### 3. **Isolation Levels**

**Tenant Level (Security):**
- Authentication and authorization
- Billing and cost allocation
- Audit logging scope

**Database Level (Data + Compute):**
- Data isolation via key prefixing
- Compute isolation via quotas
- Node assignment for physical isolation
- Independent backup/restore

**Object Level (Organization):**
- Namespaces for logical grouping
- Tables for data storage
- Functions for compute

### 4. **Scalability**

**Add nodes for specific databases:**
```rust
// Database 1 is getting heavy load
cluster.add_nodes_to_database(
    database_id: 1,
    new_nodes: vec![node7, node8],
);

// Container metadata replicates to new nodes
// Data shards can be rebalanced to new nodes
```

**Remove nodes:**
```rust
// Drain node before removal
cluster.drain_node(node5);

// Container metadata and data shards migrate away
// Global metadata remains on all other nodes
```

## Implementation

### 1. Shard Group Types

```rust
pub enum ShardGroupType {
    /// Global metadata (ALL nodes)
    GlobalMetadata,
    
    /// Container metadata (ASSIGNED nodes only)
    ContainerMetadata { database: DatabaseId },
    
    /// Data shards (per table configuration)
    Data { table: TableId },
}

pub struct ShardGroup {
    pub id: ShardGroupId,
    pub group_type: ShardGroupType,
    pub replicas: Vec<NodeId>,
    pub leader: Option<NodeId>,
    pub raft_group: RaftGroup,
}
```

### 2. Metadata Access

```rust
impl ClusterManager {
    /// Access global metadata (always local)
    pub fn get_tenant(&self, tenant_id: TenantId) -> Option<TenantRecord> {
        self.global_metadata.get_tenant(tenant_id)
    }
    
    /// Access container metadata (local if assigned, remote otherwise)
    pub async fn get_table(
        &self,
        database: DatabaseId,
        table: ObjectId,
    ) -> Result<TableRecord> {
        if self.is_database_assigned_locally(database) {
            // Local access - fast
            self.container_metadata.get_table(database, table)
        } else {
            // Remote access - route to assigned node
            self.route_to_database_node(database, |node| {
                node.get_table(database, table)
            }).await
        }
    }
}
```

### 3. Container Assignment

```rust
pub struct ContainerAssignment {
    pub database: DatabaseId,
    pub nodes: Vec<NodeId>,
    pub replication_factor: u32,
    pub status: AssignmentStatus,
}

impl ClusterManager {
    /// Assign database to specific nodes
    pub async fn assign_database(
        &mut self,
        database: DatabaseId,
        nodes: Vec<NodeId>,
        replication_factor: u32,
    ) -> Result<()> {
        // 1. Update global metadata
        self.global_metadata.set_assignment(ContainerAssignment {
            database,
            nodes: nodes.clone(),
            replication_factor,
            status: AssignmentStatus::Pending,
        })?;
        
        // 2. Create container metadata shard group on assigned nodes
        self.create_container_metadata_shard_group(
            database,
            nodes.clone(),
            replication_factor,
        ).await?;
        
        // 3. Mark assignment as active
        self.global_metadata.update_assignment_status(
            database,
            AssignmentStatus::Active,
        )?;
        
        Ok(())
    }
}
```

### 4. Query Routing

```rust
impl QueryRouter {
    pub async fn execute_query(
        &self,
        tenant: TenantId,
        database: DatabaseId,
        query: Query,
    ) -> Result<QueryResult> {
        // 1. Validate tenant (global metadata - local)
        let tenant_record = self.global_metadata.get_tenant(tenant)?;
        
        // 2. Validate database (global metadata - local)
        let db_record = self.global_metadata.get_database(database)?;
        
        // 3. Check if we're on an assigned node
        if !db_record.assigned_nodes.contains(&self.node_id) {
            // Route to assigned node
            return self.route_to_database_node(database, query).await;
        }
        
        // 4. Access container metadata (local)
        let tables = self.container_metadata.get_tables_for_query(&query)?;
        
        // 5. Execute query on data shards
        self.execute_on_data_shards(tables, query).await
    }
}
```

## Consequences

### Positive

* **Efficient metadata access**: Global metadata always local, no network hops
* **Flexible isolation**: Can assign databases to specific nodes
* **Reduced memory**: Container metadata only where needed
* **Better scalability**: Add nodes for specific databases
* **Clear separation**: Control plane vs data plane
* **Fast tenant validation**: Global metadata cached everywhere

### Negative

* **More complex**: Two-tier metadata architecture
* **Coordination overhead**: Container assignments need coordination
* **Migration complexity**: Moving databases between nodes

### Trade-offs

* **Global metadata size**: Must fit in memory on all nodes (acceptable - small dataset)
* **Container metadata routing**: Remote access requires network hop (rare - usually local)

## Migration Strategy

### Phase 1: Global Metadata
1. Create global metadata shard group
2. Migrate tenant and database records
3. Replicate to all nodes

### Phase 2: Container Metadata
1. Create container metadata shard groups per database
2. Migrate namespace, table, function records
3. Assign to appropriate nodes

### Phase 3: Data Migration
1. Re-encode keys with 16-byte prefix
2. Migrate data to new shard groups
3. Update routing tables

## Related ADRs

* [ADR-0026: Resource Quotas and Limits](ADR-0026-Resource-Quotas-and-Limits.md)
* [ADR-0007: Clustering, Sharding, Replication, and Consensus](ADR-0007-Clustering-Sharding-Replication-Consensus.md)
* [ADR-0010: Authentication, Authorization, and Access Control](ADR-0010-Authentication-Authorization-Access-Control.md)

---

**Key Innovation:** Separating global metadata (control plane) from container metadata (data plane) enables efficient cluster-wide operations while maintaining flexible database-level isolation and assignment.