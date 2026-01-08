# Nanograph Logical Architecture

## Hierarchical Structure

### Physical Hierarchy
```
Cluster (Global)
  └─ Region (Geographic/Data Center)
      └─ Server (Nanograph Instance)
          └─ Shard Replicas (Data Partitions)
```

### Logical Hierarchy
```
Cluster
  └─ Database
      └─ Schema
          └─ Table
              └─ Shards (Optional, per-table partitioning)
```

## Complete Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                          CLUSTER                                │
│  (Global coordination, cross-region replication)                │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│  ┌────────────────────┐  ┌────────────────────┐                 │
│  │   REGION: US-EAST  │  │   REGION: EU-WEST  │                 │
│  │  (Full replica)    │  │  (Full replica)    │                 │
│  ├────────────────────┤  ├────────────────────┤                 │
│  │                    │  │                    │                 │
│  │ ┌────────────────┐ │  │ ┌────────────────┐ │                 │
│  │ │  Server 1      │ │  │ │  Server 4      │ │                 │
│  │ │  - Shard 0 (L) │ │  │ │  - Shard 0 (F) │ │                 │
│  │ │  - Shard 1 (F) │ │  │ │  - Shard 1 (L) │ │                 │
│  │ └────────────────┘ │  │ └────────────────┘ │                 │
│  │                    │  │                    │                 │
│  │ ┌────────────────┐ │  │ ┌────────────────┐ │                 │
│  │ │  Server 2      │ │  │ │  Server 5      │ │                 │
│  │ │  - Shard 0 (F) │ │  │ │  - Shard 0 (F) │ │                 │
│  │ │  - Shard 1 (L) │ │  │ │  - Shard 1 (F) │ │                 │
│  │ └────────────────┘ │  │ └────────────────┘ │                 │
│  │                    │  │                    │                 │
│  │ ┌────────────────┐ │  │ ┌────────────────┐ │                 │
│  │ │  Server 3      │ │  │ │  Server 6      │ │                 │
│  │ │  - Shard 0 (F) │ │  │ │  - Shard 0 (L) │ │                 │
│  │ │  - Shard 1 (F) │ │  │ │  - Shard 1 (F) │ │                 │
│  │ └────────────────┘ │  │ └────────────────┘ │                 │
│  │                    │  │                    │                 │
│  │ Each region has    │  │ Each region has    │                 │
│  │ ALL shards for     │  │ ALL shards for     │                 │
│  │ data locality      │  │ data locality      │                 │
│  └────────────────────┘  └────────────────────┘                 │
│                                                                 │
│  L = Leader, F = Follower                                       │
│  Each shard has one leader per region                           │
└─────────────────────────────────────────────────────────────────┘
```

## Logical Data Model

```
Cluster "production"
  │
  ├─ Database "app_db"
  │   │
  │   ├─ Schema "public"
  │   │   │
  │   │   ├─ Table "users" (sharded by user_id, 4 shards)
  │   │   │   ├─ Shard 0: user_id % 4 == 0
  │   │   │   ├─ Shard 1: user_id % 4 == 1
  │   │   │   ├─ Shard 2: user_id % 4 == 2
  │   │   │   └─ Shard 3: user_id % 4 == 3
  │   │   │
  │   │   ├─ Table "sessions" (not sharded, single shard)
  │   │   │   └─ Shard 0: all data
  │   │   │
  │   │   └─ Table "events" (sharded by timestamp, 8 shards)
  │   │       ├─ Shard 0-7: time-based partitioning
  │   │
  │   └─ Schema "analytics"
  │       └─ Table "metrics" (sharded by metric_id, 16 shards)
  │
  └─ Database "cache_db"
      └─ Schema "public"
          └─ Table "cache" (sharded by key hash, 32 shards)
```

## Key Design Principles

### 1. Region-Level Replication
- **Each region is a full replica** of all data
- Provides data locality for reads
- Enables region-level failover
- Cross-region writes use consensus

### 2. Per-Table Sharding
- **Sharding is optional** and configured per table
- Small tables: single shard (no partitioning overhead)
- Large tables: multiple shards (horizontal scaling)
- Partitioner configured at table creation

### 3. Shard Distribution
- **All shards present in each region**
- Within a region, shards distributed across servers
- Each shard has one leader per region
- Followers provide read scaling

### 4. Hierarchical Naming
```
Fully Qualified Table Name:
  cluster.database.schema.table

Shard Identifier:
  cluster.database.schema.table.shard_N

Physical Location:
  region.server.shard_N
```

## Revised Type System

### Physical Hierarchy Types

```rust
/// Cluster identifier
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub struct ClusterId(pub u64);

/// Region identifier (geographic/data center)
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub struct RegionId(pub u64);

/// Server identifier (Nanograph instance)
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub struct ServerId(pub u64);

/// Region information
pub struct RegionInfo {
    pub id: RegionId,
    pub name: String,
    pub location: String, // e.g., "us-east-1", "eu-west-1"
    pub servers: Vec<ServerId>,
    pub status: RegionStatus,
}

pub enum RegionStatus {
    Active,
    Degraded,
    Offline,
}

/// Server information
pub struct ServerInfo {
    pub id: ServerId,
    pub region_id: RegionId,
    pub address: SocketAddr,
    pub capacity: ResourceCapacity,
    pub status: ServerStatus,
}
```

### Logical Hierarchy Types

```rust
/// Database identifier
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct DatabaseId(pub String);

/// Schema identifier
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct SchemaId {
    pub database: DatabaseId,
    pub name: String,
}

/// Table identifier
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct TableId {
    pub database: DatabaseId,
    pub schema: String,
    pub name: String,
}

impl TableId {
    pub fn fqn(&self) -> String {
        format!("{}.{}.{}", self.database.0, self.schema, self.name)
    }
}

/// Table configuration
pub struct TableConfig {
    pub id: TableId,
    pub partitioner: Option<Partitioner>,
    pub shard_count: u32, // 1 = not sharded
    pub replication_factor: usize, // per region
}

/// Partitioning strategy
pub enum Partitioner {
    Hash { key_field: String },
    Range { key_field: String, ranges: Vec<(Vec<u8>, Vec<u8>)> },
    List { key_field: String, values: Vec<Vec<u8>> },
    Time { key_field: String, interval: Duration },
}
```

### Shard Identification

```rust
/// Complete shard identifier
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct ShardIdentifier {
    pub table: TableId,
    pub shard_index: u32,
}

impl ShardIdentifier {
    pub fn fqn(&self) -> String {
        format!("{}.shard_{}", self.table.fqn(), self.shard_index)
    }
}

/// Shard placement (physical location)
pub struct ShardPlacement {
    pub shard: ShardIdentifier,
    pub region: RegionId,
    pub servers: Vec<ServerId>,
    pub leader: Option<ServerId>,
}
```

## Routing Architecture

### Multi-Level Routing

```rust
pub struct ClusterRouter {
    /// Local region ID
    local_region: RegionId,
    
    /// Local server ID
    local_server: ServerId,
    
    /// Region routers (one per region)
    regions: HashMap<RegionId, Arc<RegionRouter>>,
    
    /// Cluster metadata
    metadata: Arc<ClusterMetadata>,
}

impl ClusterRouter {
    /// Route operation to appropriate region
    pub async fn route_operation(&self, table: &TableId, key: &[u8]) -> Result<RegionId> {
        // Prefer local region for reads
        // Use consensus for writes across regions
        Ok(self.local_region)
    }
}

pub struct RegionRouter {
    /// Region ID
    region_id: RegionId,
    
    /// Server routers (one per server in region)
    servers: HashMap<ServerId, Arc<ServerRouter>>,
    
    /// Region metadata
    metadata: Arc<RegionMetadata>,
}

impl RegionRouter {
    /// Route to appropriate server in region
    pub async fn route_to_server(&self, shard: &ShardIdentifier) -> Result<ServerId> {
        // Find server hosting this shard
        self.metadata.get_shard_leader(shard).await
    }
}

pub struct ServerRouter {
    /// Server ID
    server_id: ServerId,
    
    /// Shard Raft groups on this server
    shards: HashMap<ShardIdentifier, Arc<ShardRaftGroup>>,
}

impl ServerRouter {
    /// Route to local shard
    pub async fn route_to_shard(&self, shard: &ShardIdentifier) -> Result<Arc<ShardRaftGroup>> {
        self.shards.get(shard)
            .cloned()
            .ok_or(RaftError::ShardNotFound)
    }
}
```

## Operation Flow

### Write Operation (Cross-Region)

```
1. Client → ClusterRouter.put(table="users", key="user:123", value=...)
                │
2. Determine table sharding
   table.shard_count = 4
   hash("user:123") % 4 = 2
   shard = users.shard_2
                │
3. Cross-region consensus (all regions must agree)
                │
   ┌────────────┼────────────┐
   │            │            │
   ▼            ▼            ▼
US-EAST      EU-WEST      AP-SOUTH
Region       Region       Region
   │            │            │
4. Each region's leader proposes to local Raft group
   │            │            │
   ▼            ▼            ▼
Shard 2      Shard 2      Shard 2
Leader       Leader       Leader
   │            │            │
5. Local Raft consensus within each region
   │            │            │
6. All regions commit → Success
```

### Read Operation (Local Region)

```
1. Client → ClusterRouter.get(table="users", key="user:123")
                │
2. Route to local region (data locality)
                │
3. Determine shard: users.shard_2
                │
4. Route to shard leader in local region
                │
5. Read from local storage (fast, no cross-region)
                │
6. Return value
```

## Metadata Management

### Cluster Metadata
- Database/Schema/Table definitions
- Table partitioning configurations
- Region topology
- Cross-region replication status

### Region Metadata
- Server membership
- Shard assignments within region
- Shard leaders within region
- Server health status

### Server Metadata
- Local shard replicas
- Storage engine instances
- Resource usage

## Configuration Example

```rust
// Create cluster
let cluster = ClusterRouter::new(cluster_id);

// Add regions
cluster.add_region(RegionInfo {
    id: RegionId::new(1),
    name: "us-east".to_string(),
    location: "us-east-1".to_string(),
    servers: vec![],
    status: RegionStatus::Active,
}).await?;

cluster.add_region(RegionInfo {
    id: RegionId::new(2),
    name: "eu-west".to_string(),
    location: "eu-west-1".to_string(),
    servers: vec![],
    status: RegionStatus::Active,
}).await?;

// Add servers to regions
cluster.add_server(ServerInfo {
    id: ServerId::new(1),
    region_id: RegionId::new(1),
    address: "10.0.1.1:9000".parse()?,
    capacity: ResourceCapacity::default(),
    status: ServerStatus::Active,
}).await?;

// Create database
cluster.create_database(DatabaseId("app_db".to_string())).await?;

// Create schema
cluster.create_schema(SchemaId {
    database: DatabaseId("app_db".to_string()),
    name: "public".to_string(),
}).await?;

// Create table with sharding
cluster.create_table(TableConfig {
    id: TableId {
        database: DatabaseId("app_db".to_string()),
        schema: "public".to_string(),
        name: "users".to_string(),
    },
    partitioner: Some(Partitioner::Hash {
        key_field: "user_id".to_string(),
    }),
    shard_count: 4,
    replication_factor: 3, // 3 replicas per region
}).await?;
```

## Benefits of This Architecture

1. **Data Locality**: Each region has all data, minimizing cross-region reads
2. **Flexible Sharding**: Per-table configuration, not forced on all tables
3. **Scalability**: Add servers within regions, add regions for geographic expansion
4. **Fault Tolerance**: Region-level failover, shard-level replication
5. **Performance**: Local reads, coordinated writes
6. **Operational Simplicity**: Clear hierarchy, predictable behavior

## Migration from Current Implementation

The current implementation provides the foundation (shard-level Raft groups). We need to add:

1. **Hierarchical identifiers**: ClusterId, RegionId, ServerId, DatabaseId, SchemaId, TableId
2. **Multi-level routing**: ClusterRouter → RegionRouter → ServerRouter
3. **Cross-region coordination**: Consensus across region leaders
4. **Metadata hierarchy**: Cluster, Region, and Server metadata
5. **Table-level sharding**: Partitioner configuration per table

This is a Phase 2+ enhancement that builds on the shard-level foundation already implemented.