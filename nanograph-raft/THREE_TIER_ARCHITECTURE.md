# Three-Tier Raft Group Architecture

## Overview

Nanograph uses a **three-tier hierarchical Raft group architecture** to manage metadata and data at different scopes with appropriate consistency guarantees and performance characteristics.

## Architecture Tiers

### Tier 1: System Metadata Raft Group

**Scope**: Cluster-wide (1 group per cluster)

**Purpose**: Manages global system configuration and cluster topology

**Shard**: Dedicated `system_shard` (ShardId)

**Data Managed**:
- **Cluster**: Configuration, version, global settings
- **Regions**: Geographic/data center locations
- **Servers**: Nanograph instance registrations
- **Tenants**: Multi-tenancy root entities
- **System Users**: Global administrative users
- **Database Registry**: References to all databases (not full metadata)

**Characteristics**:
- **Update Frequency**: Very low (minutes to hours)
- **Data Size**: Small (KB to low MB)
- **Replication**: Fully replicated across all nodes
- **Consistency**: Strong (linearizable)
- **Availability**: High (survives minority failures)

**Implementation**: `SystemMetastore` in `nanograph-kvm/src/metastore.rs`

```rust
pub struct SystemMetastore {
    cluster: ClusterMetadata,
    regions: HashMap<RegionId, RegionMetadata>,
    servers: HashMap<ServerId, ServerMetadata>,
    tenants: HashMap<TenantId, TenantMetadata>,
    databases: HashMap<DatabaseId, DatabaseMetadata>,  // References only
    system_users: HashMap<UserId, UserMetadata>,
    system_shard: ShardId,  // Dedicated shard
    shard_manager: Arc<RwLock<KeyValueShardManager>>,
    consensus_router: Option<Arc<ConsensusRouter>>,
}
```

**Operations**:
- Add/remove regions
- Register/deregister servers
- Create/delete tenants
- Manage system users
- Update cluster configuration

### Tier 2: Database Metadata Raft Groups

**Scope**: Per-database (N groups, one per database)

**Purpose**: Manages database-specific schema and configuration

**Shard**: Dedicated `metadata_shard` per database (ShardId per container)

**Container**: `ContainerId` = `TenantId` + `DatabaseId`

**Data Managed**:
- **Namespaces**: Schema/namespace definitions
- **Tables**: Table configurations and properties
- **Shard Metadata**: Information about data shards for this database
- **Database Users**: Database-level permissions
- **Shard Assignments**: Which nodes host which shards
- **Name Resolver**: Hierarchical object path resolution

**Characteristics**:
- **Update Frequency**: Low (seconds to minutes)
- **Data Size**: Medium (MB range)
- **Replication**: Fully replicated per database
- **Consistency**: Strong (linearizable)
- **Isolation**: Changes in one database don't affect others

**Implementation**: `DatabaseMetastore` in `nanograph-kvm/src/metastore.rs`

```rust
pub struct DatabaseMetastore {
    container: ContainerId,  // Tenant + Database
    namespaces: HashMap<NamespaceId, NamespaceMetadata>,
    tables: HashMap<TableId, TableMetadata>,
    shards: HashMap<ShardId, ShardMetadata>,
    database_users: HashMap<UserId, UserMetadata>,
    
    // Consensus metadata
    metadata_shard: Option<ShardId>,  // Dedicated shard for this DB
    consensus_router: Option<Arc<ConsensusRouter>>,
    shard_assignments: BTreeMap<ShardId, Vec<NodeId>>,
    
    // Name resolver
    resolver_nodes: BTreeMap<ObjectId, Node>,
    resolver_paths: BTreeMap<String, ObjectId>,
    available_nodes: BTreeSet<ObjectId>,
    next_resolver_id: ObjectId,
}
```

**Operations**:
- Create/drop namespaces
- Create/alter/drop tables
- Manage database users
- Update shard assignments
- Resolve hierarchical paths

### Tier 3: Data Shard Raft Groups

**Scope**: Per-shard (M groups, many per database)

**Purpose**: Manages actual user data with high throughput

**Shard**: One Raft group per data shard

**Data Managed**:
- User key-value pairs
- Application data
- Document data
- Graph data

**Characteristics**:
- **Update Frequency**: Very high (milliseconds)
- **Data Size**: Large (GB to TB per shard)
- **Replication**: Configurable (typically 3-5 replicas)
- **Consistency**: Configurable (linearizable, lease, or follower reads)
- **Scalability**: Horizontal (add more shards)

**Implementation**: `ShardRaftGroup` in `nanograph-raft/src/shard_group.rs`

```rust
pub struct ShardRaftGroup {
    shard_id: ShardId,
    local_node_id: NodeId,
    storage: Arc<RaftStorageAdapter>,
    config: ReplicationConfig,
    role: Arc<RwLock<RaftRole>>,
    peers: Arc<RwLock<Vec<NodeId>>>,
    leader: Arc<RwLock<Option<NodeId>>>,
    lease_expiry: Arc<RwLock<Option<std::time::Instant>>>,
}
```

**Operations**:
- PUT key-value pairs
- GET key-value pairs
- DELETE keys
- Batch operations
- Range scans

## Hierarchical Relationships

```
┌─────────────────────────────────────────────────────────────┐
│           System Metadata Raft Group (Tier 1)               │
│                                                              │
│  Manages:                                                    │
│  • Cluster configuration                                     │
│  • Regions (us-east, eu-west, ap-south)                     │
│  • Servers (node1, node2, node3, ...)                       │
│  • Tenants (tenant_a, tenant_b, ...)                        │
│  • System users                                              │
│                                                              │
│  Shard: system_shard (ShardId::new(0))                      │
└────────────────────────┬────────────────────────────────────┘
                         │
                         │ References
                         │
        ┌────────────────┼────────────────┐
        │                │                │
        ▼                ▼                ▼
┌──────────────┐  ┌──────────────┐  ┌──────────────┐
│ DB1 Metadata │  │ DB2 Metadata │  │ DB3 Metadata │
│ Raft Group   │  │ Raft Group   │  │ Raft Group   │
│  (Tier 2)    │  │  (Tier 2)    │  │  (Tier 2)    │
│              │  │              │  │              │
│ Container:   │  │ Container:   │  │ Container:   │
│ tenant_a+db1 │  │ tenant_a+db2 │  │ tenant_b+db1 │
│              │  │              │  │              │
│ Manages:     │  │ Manages:     │  │ Manages:     │
│ • Namespaces │  │ • Namespaces │  │ • Namespaces │
│ • Tables     │  │ • Tables     │  │ • Tables     │
│ • DB users   │  │ • DB users   │  │ • DB users   │
│              │  │              │  │              │
│ Shard: db1   │  │ Shard: db2   │  │ Shard: db3   │
└──────┬───────┘  └──────┬───────┘  └──────┬───────┘
       │                 │                 │
       │ Manages         │ Manages         │ Manages
       │                 │                 │
   ┌───┴────┬────┐   ┌───┴────┬────┐   ┌───┴────┬────┐
   ▼        ▼    ▼   ▼        ▼    ▼   ▼        ▼    ▼
┌─────┐  ┌─────┐  ┌─────┐  ┌─────┐  ┌─────┐  ┌─────┐
│Shard│  │Shard│  │Shard│  │Shard│  │Shard│  │Shard│
│  0  │  │  1  │  │  2  │  │  3  │  │  4  │  │  5  │
│     │  │     │  │     │  │     │  │     │  │     │
│Raft │  │Raft │  │Raft │  │Raft │  │Raft │  │Raft │
│Group│  │Group│  │Group│  │Group│  │Group│  │Group│
│     │  │     │  │     │  │     │  │     │  │     │
│Tier3│  │Tier3│  │Tier3│  │Tier3│  │Tier3│  │Tier3│
└─────┘  └─────┘  └─────┘  └─────┘  └─────┘  └─────┘
  User Data Shards (High Volume, Horizontally Scalable)
```

## Why Three Tiers?

### 1. Scope Isolation

**Problem**: Different types of metadata have different scopes and update patterns.

**Solution**: Separate Raft groups for different scopes.

- **System changes** (adding a region) shouldn't require consensus from every database
- **Database changes** (creating a table) shouldn't require consensus from every data shard
- **Data operations** should be isolated to their specific shard

### 2. Multi-Tenancy

**Problem**: Tenant isolation and security boundaries.

**Solution**: Each database (tenant+database) has its own metadata Raft group.

- Tenant A's schema changes don't affect Tenant B
- Metadata for different tenants is physically separated
- Better security and compliance (data residency, access control)

### 3. Performance and Scalability

**Problem**: Different data types have vastly different performance requirements.

**Solution**: Optimize each tier for its workload.

| Tier | Update Rate | Latency Target | Scalability |
|------|-------------|----------------|-------------|
| System Metadata | 1-10/min | 10-100ms | Vertical (more powerful nodes) |
| Database Metadata | 1-100/sec | 5-50ms | Horizontal (more databases) |
| Data Shards | 1000-100K/sec | 1-10ms | Horizontal (more shards) |

### 4. Failure Isolation

**Problem**: Failures in one component shouldn't cascade to others.

**Solution**: Independent Raft groups with isolated failure domains.

- System metadata failure: Cluster management affected, but data operations continue
- Database metadata failure: Only that database's schema operations affected
- Data shard failure: Only that shard's data affected

### 5. Operational Simplicity

**Problem**: Managing thousands of Raft groups is complex.

**Solution**: Hierarchical organization with clear responsibilities.

- System admins manage Tier 1 (cluster topology)
- Database admins manage Tier 2 (schemas)
- Applications use Tier 3 (data)
- Clear separation of concerns

## Data Flow Patterns

### Pattern 1: Create Table (Metadata Operation)

```
Application
    │
    │ 1. create_table("users", config)
    ▼
KeyValueDatabaseManager
    │
    │ 2. Determine database container
    │    container = tenant_a + db1
    ▼
Database Metadata Raft Group (Tier 2)
    │
    │ 3. Propose: CreateTable { name: "users", ... }
    │ 4. Replicate to followers
    │ 5. Commit when quorum reached
    │ 6. Apply to metadata shard
    ▼
System Metadata Raft Group (Tier 1)
    │
    │ 7. Propose: CreateShards { table_id, shard_count, ... }
    │ 8. Replicate to followers
    │ 9. Commit when quorum reached
    │ 10. Update shard assignments
    ▼
Data Shard Raft Groups (Tier 3)
    │
    │ 11. Initialize new shard Raft groups
    │ 12. Assign to nodes based on placement strategy
    ▼
Response: TableId
```

### Pattern 2: Write Data (Data Operation)

```
Application
    │
    │ 1. put(table_id, key, value)
    ▼
KeyValueDatabaseManager
    │
    │ 2. Lookup table metadata (cached)
    │ 3. Determine shard: hash(key) % shard_count
    │    shard_id = ShardId::from_parts(table_id, shard_index)
    ▼
ConsensusRouter
    │
    │ 4. Route to shard Raft group
    ▼
Data Shard Raft Group (Tier 3)
    │
    │ 5. Propose: Put { key, value }
    │ 6. Replicate to followers
    │ 7. Commit when quorum reached
    │ 8. Apply to storage engine
    │ 9. Write to WAL
    │ 10. Fsync to disk
    ▼
Response: Success
```

### Pattern 3: Add Server (System Operation)

```
Admin
    │
    │ 1. add_server(server_info)
    ▼
System Metadata Raft Group (Tier 1)
    │
    │ 2. Propose: AddServer { id, address, capacity, ... }
    │ 3. Replicate to all nodes
    │ 4. Commit when quorum reached
    │ 5. Apply to system metadata shard
    │ 6. Update cluster topology
    ▼
All Nodes
    │
    │ 7. Receive metadata update
    │ 8. Update local metadata cache
    │ 9. Trigger shard rebalancing (if needed)
    ▼
Rebalancing Coordinator
    │
    │ 10. Calculate new shard distribution
    │ 11. Migrate shards to new server
    │ 12. Update shard assignments in System Metadata
    ▼
Response: Success
```

## Consistency Guarantees

### Tier 1: System Metadata

- **Consistency**: Linearizable (strongest)
- **Reason**: Critical for cluster correctness
- **Read Strategy**: Always from leader with ReadIndex
- **Write Strategy**: Quorum consensus required

### Tier 2: Database Metadata

- **Consistency**: Linearizable (strongest)
- **Reason**: Schema consistency is critical
- **Read Strategy**: Leader reads with ReadIndex
- **Write Strategy**: Quorum consensus required

### Tier 3: Data Shards

- **Consistency**: Configurable per operation
- **Options**:
  - **Linearizable**: ReadIndex protocol (strongest, slower)
  - **Lease**: Leader lease-based reads (fast, requires clock sync)
  - **Follower**: Stale reads from any replica (fastest, potentially stale)
- **Write Strategy**: Always quorum consensus

## Implementation Status

### Completed

- ✅ System Metadata Raft Group structure (`SystemMetastore`)
- ✅ Database Metadata Raft Group structure (`DatabaseMetastore`)
- ✅ Data Shard Raft Group implementation (`ShardRaftGroup`)
- ✅ Raft storage adapter (`RaftStorageAdapter`)
- ✅ Basic routing logic (`ConsensusRouter`)

### In Progress

- 🔄 Network layer for Raft RPC
- 🔄 Full openraft integration
- 🔄 Metadata persistence to dedicated shards
- 🔄 Database metadata Raft group management

### Planned

- ⏳ Cross-tier coordination protocols
- ⏳ Shard rebalancing across tiers
- ⏳ Metadata caching and invalidation
- ⏳ Multi-region support
- ⏳ Disaster recovery procedures

## References

- [ARCHITECTURE_INTEGRATION.md](ARCHITECTURE_INTEGRATION.md) - Integration details
- [LOGICAL_ARCHITECTURE.md](LOGICAL_ARCHITECTURE.md) - Logical hierarchy
- [ADR-0007](../docs/ADR/ADR-0007-Clustering-Sharding-Replication-Consensus.md) - Consensus decision
- [SystemMetastore](../nanograph-kvm/src/cache.rs) - Implementation
- [DatabaseMetastore](../nanograph-kvm/src/cache.rs) - Implementation
- [ShardRaftGroup](src/shard_group.rs) - Implementation