---
parent: ADR
nav_order: 0007
title: Clustering, Sharding, Replication, and Consensus
status: accepted
date: 2026-01-05
deciders: Hans W. Uhlig
---

# ADR-0007: Clustering, Sharding, Replication, and Consensus

## Status

Accepted

## Context

Nanograph must support horizontal scaling to handle growing data volumes and provide fault tolerance for production deployments. Key requirements include:

1. **Horizontal scalability** - Add nodes to increase capacity
2. **Fault tolerance** - Survive node failures without data loss
3. **Strong consistency** - Maintain ACID guarantees across nodes
4. **Automatic failover** - Recover from failures without manual intervention
5. **Operational simplicity** - Minimize configuration and management complexity

Traditional approaches face trade-offs:
- **Master-slave replication** - Simple but limited scalability and manual failover
- **Multi-master** - Complex conflict resolution and weak consistency
- **Eventual consistency** - Simpler but violates ACID requirements
- **Two-phase commit** - Strong consistency but poor availability and performance

## Decision

Implement a **Raft-based sharded architecture** where:

1. **Data is partitioned into shards** using consistent hashing
2. **Each shard is a Raft group** with configurable replication factor
3. **Writes go through Raft consensus** for durability and consistency
4. **Reads can be served from leader or followers** based on consistency requirements
5. **Metadata is coordinated** through a separate Raft group
6. **Shard rebalancing** is supported for adding/removing nodes

## Decision Drivers

* **Strong consistency** - Raft provides linearizable operations
* **Proven technology** - Raft is well-understood and battle-tested
* **Simpler than Paxos** - Easier to implement and reason about
* **Good performance** - Acceptable latency for most workloads
* **Automatic failover** - Leader election handles failures

## Architecture

### Three-Tier Raft Group Topology

Nanograph uses a **three-tier hierarchical Raft group architecture** for optimal performance and isolation:

```
┌─────────────────────────────────────────────────────────────┐
│         Tier 1: System Metadata Raft Group (1)              │
│  Manages: Clusters, Regions, Servers, Tenants               │
│  Shard: system_shard                                         │
│  Update Frequency: Very Low (minutes-hours)                 │
└────────────────────────┬────────────────────────────────────┘
                         │
        ┌────────────────┼────────────────┐
        │                │                │
        ▼                ▼                ▼
┌──────────────┐  ┌──────────────┐  ┌──────────────┐
│ Tier 2: DB1  │  │ Tier 2: DB2  │  │ Tier 2: DB3  │
│ Metadata     │  │ Metadata     │  │ Metadata     │
│ Raft Group   │  │ Raft Group   │  │ Raft Group   │
│              │  │              │  │              │
│ Manages:     │  │ Manages:     │  │ Manages:     │
│ Namespaces   │  │ Namespaces   │  │ Namespaces   │
│ Tables       │  │ Tables       │  │ Tables       │
│ DB Users     │  │ DB Users     │  │ DB Users     │
│              │  │              │  │              │
│ Shard: db1   │  │ Shard: db2   │  │ Shard: db3   │
│ Update Freq: │  │ Update Freq: │  │ Update Freq: │
│ Low (sec-min)│  │ Low (sec-min)│  │ Low (sec-min)│
└──────┬───────┘  └──────┬───────┘  └──────┬───────┘
       │                 │                 │
   ┌───┴───┐         ┌───┴───┐         ┌───┴───┐
   ▼       ▼         ▼       ▼         ▼       ▼
┌─────┐ ┌─────┐   ┌─────┐ ┌─────┐   ┌─────┐ ┌─────┐
│Tier3│ │Tier3│   │Tier3│ │Tier3│   │Tier3│ │Tier3│
│Data │ │Data │   │Data │ │Data │   │Data │ │Data │
│Shard│ │Shard│   │Shard│ │Shard│   │Shard│ │Shard│
│  0  │ │  1  │   │  2  │ │  3  │   │  4  │ │  5  │
│     │ │     │   │     │ │     │   │     │ │     │
│User │ │User │   │User │ │User │   │User │ │User │
│Data │ │Data │   │Data │ │Data │   │Data │ │Data │
│     │ │     │   │     │ │     │   │     │ │     │
│High │ │High │   │High │ │High │   │High │ │High │
│Freq │ │Freq │   │Freq │ │Freq │   │Freq │ │Freq │
└─────┘ └─────┘   └─────┘ └─────┘   └─────┘ └─────┘
```

### Physical Node Distribution

```
            ┌──────────────┐ ┌──────────────┐ ┌──────────────┐
            │   Node 1     │ │   Node 2     │ │   Node 3     │
            ├──────────────┤ ├──────────────┤ ├──────────────┤
            │ System Meta  │ │ System Meta  │ │ System Meta  │
            │ (Follower)   │ │ (Leader)     │ │ (Follower)   │
            ├──────────────┤ ├──────────────┤ ├──────────────┤
            │ DB1 Meta     │ │ DB1 Meta     │ │ DB1 Meta     │
            │ (Leader)     │ │ (Follower)   │ │ (Follower)   │
            ├──────────────┤ ├──────────────┤ ├──────────────┤
            │ DB2 Meta     │ │ DB2 Meta     │ │ DB2 Meta     │
            │ (Follower)   │ │ (Leader)     │ │ (Follower)   │
            ├──────────────┤ ├──────────────┤ ├──────────────┤
            │ Shard 0      │ │ Shard 0      │ │ Shard 0      │
            │ (Leader)     │ │ (Follower)   │ │ (Follower)   │
            ├──────────────┤ ├──────────────┤ ├──────────────┤
            │ Shard 1      │ │ Shard 1      │ │ Shard 1      │
            │ (Follower)   │ │ (Leader)     │ │ (Follower)   │
            ├──────────────┤ ├──────────────┤ ├──────────────┤
            │ Shard 2      │ │ Shard 2      │ │ Shard 2      │
            │ (Follower)   │ │ (Follower)   │ │ (Leader)     │
            └──────────────┘ └──────────────┘ └──────────────┘
```

### Why Three Tiers?

1. **Scope Isolation**: System, database, and data operations have different scopes
2. **Multi-Tenancy**: Each database gets its own metadata Raft group
3. **Performance**: Optimize each tier for its workload characteristics
4. **Failure Isolation**: Failures in one tier don't cascade to others
5. **Scalability**: Add databases and shards independently

### Data Flow

#### Write Path

```
Client
  │
  │ 1. Write request
  ▼
┌─────────────┐
│   Router    │ 2. Hash key → Shard ID
└─────────────┘
  │
  │ 3. Forward to shard leader
  ▼
┌─────────────┐
│Shard Leader │ 4. Propose to Raft
└─────────────┘
  │
  │ 5. Replicate to followers
  ├──────────────┬──────────────┐
  ▼              ▼              ▼
┌──────────┐ ┌──────────┐ ┌──────────┐
│Follower 1│ │Follower 2│ │Follower 3│
└──────────┘ └──────────┘ └──────────┘
  │              │              │
  │ 6. Acknowledge
  └──────────────┴──────────────┘
  │
  │ 7. Commit when quorum reached
  ▼
┌─────────────┐
│Shard Leader │ 8. Apply to state machine
└─────────────┘
  │
  │ 9. Return success to client
  ▼
Client
```

#### Read Path (Linearizable)

```
Client
  │
  │ 1. Read request
  ▼
┌─────────────┐
│   Router    │ 2. Hash key → Shard ID
└─────────────┘
  │
  │ 3. Forward to shard leader
  ▼
┌─────────────┐
│Shard Leader │ 4. Check leadership (ReadIndex)
└─────────────┘
  │
  │ 5. Confirm with quorum
  ├──────────────┬──────────────┐
  ▼              ▼              ▼
┌──────────┐ ┌──────────┐ ┌──────────┐
│Follower 1│ │Follower 2│ │Follower 3│
└──────────┘ └──────────┘ └──────────┘
  │              │              │
  │ 6. Heartbeat responses
  └──────────────┴──────────────┘
  │
  │ 7. Read from local state
  ▼
┌─────────────┐
│Shard Leader │ 8. Return data
└─────────────┘
  │
  ▼
Client
```

#### Read Path (Follower Read)

```
Client
  │
  │ 1. Read request (stale_ok=true)
  ▼
┌─────────────┐
│   Router    │ 2. Hash key → Shard ID
└─────────────┘
  │
  │ 3. Forward to any replica
  ▼
┌─────────────┐
│   Follower  │ 4. Read from local state
└─────────────┘    (may be slightly stale)
  │
  │ 5. Return data
  ▼
Client
```

### Shard Distribution

```
Key Space: [0x00000000 ... 0xFFFFFFFF]
           │                          │
           ├──────────┬───────────────┤
           │          │               │
        Shard 0    Shard 1        Shard N
     [0x00-0x55] [0x56-0xAA]  [0xAB-0xFF]
           │          │               │
           ▼          ▼               ▼
    ┌──────────┐ ┌──────────┐   ┌──────────┐
    │ Replica  │ │ Replica  │   │ Replica  │
    │  Group   │ │  Group   │   │  Group   │
    │  (Raft)  │ │  (Raft)  │   │  (Raft)  │
    └──────────┘ └──────────┘   └──────────┘
         │            │               │
    ┌────┴────┐  ┌────┴────┐     ┌────┴────┐
    │ Node 1  │  │ Node 2  │     │ Node 3  │
    │ Node 2  │  │ Node 3  │     │ Node 1  │
    │ Node 3  │  │ Node 1  │     │ Node 2  │
    └─────────┘  └─────────┘     └─────────┘
```

### Failure Scenarios

#### Leader Failure

```
Before:
┌─────────┐     ┌─────────┐     ┌─────────┐
│ Leader  │────▶│Follower │────▶│Follower │
│ Node 1  │     │ Node 2  │     │ Node 3  │
└─────────┘     └─────────┘     └─────────┘
     ✗ CRASH

After Election:
┌─────────┐     ┌─────────┐     ┌─────────┐
│ Failed  │     │ Leader  │────▶│Follower │
│ Node 1  │     │ Node 2  │     │ Node 3  │
└─────────┘     └─────────┘     └─────────┘
                     ▲
                     │ Elected in ~1-2 seconds
```

#### Network Partition

```
Partition occurs:
┌─────────┐     │     ┌─────────┐     ┌─────────┐
│ Leader  │     │     │Follower │────▶│Follower │
│ Node 1  │     │     │ Node 2  │     │ Node 3  │
└─────────┘     │     └─────────┘     └─────────┘
  Minority      │         Majority
  (1 node)      │         (2 nodes)
                │
                │ Network partition
                │
                ▼
┌─────────┐           ┌─────────┐     ┌─────────┐
│ Steps   │           │ Leader  │────▶│Follower │
│  Down   │           │ Node 2  │     │ Node 3  │
└─────────┘           └─────────┘     └─────────┘
                           ▲
                           │ New leader elected
                           │ Continues serving requests
```

* **Active development** - Good library support (tikv/raft-rs)

## Design

### 1. Cluster Architecture

```
┌─────────────────────────────────────────────────────────┐
│                    Nanograph Cluster                     │
├─────────────────────────────────────────────────────────┤
│                                                          │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐  │
│  │   Node 1     │  │   Node 2     │  │   Node 3     │  │
│  ├──────────────┤  ├──────────────┤  ├──────────────┤  │
│  │ Shard 1 (L)  │  │ Shard 1 (F)  │  │ Shard 2 (L)  │  │
│  │ Shard 2 (F)  │  │ Shard 3 (L)  │  │ Shard 3 (F)  │  │
│  │ Shard 3 (F)  │  │ Shard 4 (L)  │  │ Shard 4 (F)  │  │
│  └──────────────┘  └──────────────┘  └──────────────┘  │
│                                                          │
│  L = Leader, F = Follower                               │
│                                                          │
│  ┌────────────────────────────────────────────────────┐ │
│  │         Metadata Raft Group (All Nodes)            │ │
│  └────────────────────────────────────────────────────┘ │
└─────────────────────────────────────────────────────────┘
```

### 2. Sharding Strategy

#### Hash-Based Partitioning

```rust
struct ShardingStrategy {
    shard_count: u32,
    hash_function: HashFunction,
}

impl ShardingStrategy {
    fn get_shard(&self, key: &[u8]) -> ShardId {
        let hash = self.hash_function.hash(key);
        ShardId(hash % self.shard_count)
    }
    
    fn get_shard_range(&self, shard_id: ShardId) -> (Vec<u8>, Vec<u8>) {
        // Calculate key range for this shard
        let range_size = u64::MAX / self.shard_count as u64;
        let start = range_size * shard_id.0 as u64;
        let end = start + range_size;
        
        (start.to_be_bytes().to_vec(), end.to_be_bytes().to_vec())
    }
}

enum HashFunction {
    Murmur3,
    XXHash,
    CityHash,
}
```

#### Consistent Hashing (Future)

For dynamic shard count changes:

```rust
struct ConsistentHashRing {
    virtual_nodes: BTreeMap<u64, ShardId>,
    replicas_per_shard: usize,
}

impl ConsistentHashRing {
    fn get_shard(&self, key: &[u8]) -> ShardId {
        let hash = hash_key(key);
        
        // Find first virtual node >= hash
        self.virtual_nodes
            .range(hash..)
            .next()
            .or_else(|| self.virtual_nodes.iter().next())
            .map(|(_, shard_id)| *shard_id)
            .unwrap()
    }
}
```

### 3. Raft Integration

#### Raft Group Per Shard

```rust
struct ShardRaftGroup {
    shard_id: ShardId,
    raft_node: RaftNode,
    storage: Box<dyn Storage>,
    peers: Vec<NodeId>,
    replication_factor: usize,
}

impl ShardRaftGroup {
    async fn propose_write(&mut self, entry: LogEntry) -> Result<()> {
        // Propose to Raft
        let proposal = self.raft_node.propose(entry.encode())?;
        
        // Wait for commit
        proposal.await?;
        
        // Apply to state machine
        self.storage.apply(entry)?;
        
        Ok(())
    }
    
    async fn read(&self, key: &[u8], consistency: ReadConsistency) -> Result<Option<Vec<u8>>> {
        match consistency {
            ReadConsistency::Linearizable => {
                // Read index to ensure we're up to date
                self.raft_node.read_index().await?;
                self.storage.get(key)
            }
            ReadConsistency::Lease => {
                // Leader lease-based read (faster but requires clock sync)
                if self.raft_node.is_leader() && self.has_valid_lease() {
                    self.storage.get(key)
                } else {
                    Err(Error::NotLeader)
                }
            }
            ReadConsistency::Follower => {
                // Stale read from follower
                self.storage.get(key)
            }
        }
    }
}

enum ReadConsistency {
    Linearizable,  // Strongest, slowest
    Lease,         // Fast leader reads
    Follower,      // Fastest, potentially stale
}
```

#### Log Entry Format

```rust
struct LogEntry {
    index: u64,
    term: u64,
    entry_type: EntryType,
    data: Vec<u8>,
}

enum EntryType {
    Put { key: Vec<u8>, value: Vec<u8> },
    Delete { key: Vec<u8> },
    Batch { operations: Vec<Operation> },
    ConfigChange { change: ConfigChange },
}

struct Operation {
    op_type: OpType,
    key: Vec<u8>,
    value: Option<Vec<u8>>,
}

enum OpType {
    Put,
    Delete,
}
```

### 4. Replication

#### Replication Factor Configuration

```rust
struct ReplicationConfig {
    factor: usize,           // Number of replicas (typically 3 or 5)
    min_sync_replicas: usize, // Minimum replicas for write ack (typically factor/2 + 1)
}

impl ReplicationConfig {
    fn quorum_size(&self) -> usize {
        self.factor / 2 + 1
    }
    
    fn can_tolerate_failures(&self) -> usize {
        self.factor - self.quorum_size()
    }
}
```

#### Replica Placement

```rust
struct ReplicaPlacement {
    strategy: PlacementStrategy,
}

enum PlacementStrategy {
    Random,
    RackAware { racks: Vec<RackId> },
    ZoneAware { zones: Vec<ZoneId> },
    Custom { placement_fn: Box<dyn Fn(ShardId) -> Vec<NodeId>> },
}

impl ReplicaPlacement {
    fn select_replicas(&self, shard_id: ShardId, nodes: &[NodeInfo]) -> Vec<NodeId> {
        match &self.strategy {
            PlacementStrategy::Random => {
                // Select random nodes
                nodes.choose_multiple(&mut rand::thread_rng(), self.replication_factor)
                    .map(|n| n.id)
                    .collect()
            }
            PlacementStrategy::RackAware { racks } => {
                // Ensure replicas are in different racks
                self.select_across_racks(nodes, racks)
            }
            PlacementStrategy::ZoneAware { zones } => {
                // Ensure replicas are in different availability zones
                self.select_across_zones(nodes, zones)
            }
            PlacementStrategy::Custom { placement_fn } => {
                placement_fn(shard_id)
            }
        }
    }
}
```

### 5. Metadata Management

#### Cluster Metadata

```rust
struct ClusterMetadata {
    version: u64,
    nodes: HashMap<NodeId, NodeInfo>,
    shards: HashMap<ShardId, ShardInfo>,
    shard_assignments: HashMap<ShardId, Vec<NodeId>>,
}

struct NodeInfo {
    id: NodeId,
    address: SocketAddr,
    status: NodeStatus,
    capacity: ResourceCapacity,
    last_heartbeat: Instant,
}

enum NodeStatus {
    Active,
    Draining,
    Inactive,
    Failed,
}

struct ShardInfo {
    id: ShardId,
    range: (Vec<u8>, Vec<u8>),
    leader: Option<NodeId>,
    replicas: Vec<NodeId>,
    status: ShardStatus,
}

enum ShardStatus {
    Active,
    Rebalancing,
    Splitting,
    Merging,
}
```

#### Metadata Raft Group

```rust
struct MetadataRaftGroup {
    raft_node: RaftNode,
    metadata: ClusterMetadata,
}

impl MetadataRaftGroup {
    async fn update_shard_assignment(&mut self, shard_id: ShardId, replicas: Vec<NodeId>) -> Result<()> {
        let change = MetadataChange::UpdateShardAssignment { shard_id, replicas };
        self.propose_change(change).await
    }
    
    async fn add_node(&mut self, node: NodeInfo) -> Result<()> {
        let change = MetadataChange::AddNode { node };
        self.propose_change(change).await
    }
    
    async fn remove_node(&mut self, node_id: NodeId) -> Result<()> {
        let change = MetadataChange::RemoveNode { node_id };
        self.propose_change(change).await
    }
}
```

### 6. Shard Rebalancing

#### Rebalancing Triggers

```rust
enum RebalanceTrigger {
    NodeAdded(NodeId),
    NodeRemoved(NodeId),
    LoadImbalance { threshold: f64 },
    Manual,
}

struct RebalanceCoordinator {
    metadata: Arc<RwLock<ClusterMetadata>>,
}

impl RebalanceCoordinator {
    async fn rebalance(&self, trigger: RebalanceTrigger) -> Result<RebalancePlan> {
        let metadata = self.metadata.read().await;
        
        // Calculate target distribution
        let target = self.calculate_target_distribution(&metadata)?;
        
        // Generate migration plan
        let plan = self.generate_migration_plan(&metadata, &target)?;
        
        // Execute migrations
        self.execute_plan(plan).await
    }
    
    fn calculate_target_distribution(&self, metadata: &ClusterMetadata) -> Result<Distribution> {
        let total_shards = metadata.shards.len();
        let active_nodes = metadata.nodes.values()
            .filter(|n| n.status == NodeStatus::Active)
            .count();
        
        let shards_per_node = total_shards / active_nodes;
        
        // Account for node capacity
        let mut distribution = Distribution::new();
        for node in metadata.nodes.values() {
            let target_shards = (shards_per_node as f64 * node.capacity.weight) as usize;
            distribution.set_target(node.id, target_shards);
        }
        
        Ok(distribution)
    }
}
```

#### Shard Migration

```rust
struct ShardMigration {
    shard_id: ShardId,
    from_node: NodeId,
    to_node: NodeId,
    status: MigrationStatus,
}

enum MigrationStatus {
    Pending,
    Copying,
    Syncing,
    Switching,
    Complete,
    Failed(String),
}

impl ShardMigration {
    async fn execute(&mut self) -> Result<()> {
        // 1. Start copying data
        self.status = MigrationStatus::Copying;
        self.copy_snapshot().await?;
        
        // 2. Sync incremental changes
        self.status = MigrationStatus::Syncing;
        self.sync_wal().await?;
        
        // 3. Switch leadership
        self.status = MigrationStatus::Switching;
        self.transfer_leadership().await?;
        
        // 4. Update metadata
        self.status = MigrationStatus::Complete;
        self.update_metadata().await?;
        
        Ok(())
    }
    
    async fn copy_snapshot(&self) -> Result<()> {
        // Copy SST files to target node
        let snapshot = self.create_snapshot().await?;
        self.transfer_snapshot(snapshot, self.to_node).await?;
        Ok(())
    }
    
    async fn sync_wal(&self) -> Result<()> {
        // Stream WAL entries to catch up
        let wal_stream = self.get_wal_stream().await?;
        self.apply_wal_stream(wal_stream, self.to_node).await?;
        Ok(())
    }
}
```

### 7. Failure Handling

#### Leader Election

```rust
impl ShardRaftGroup {
    fn handle_leader_change(&mut self, new_leader: NodeId) {
        if new_leader == self.local_node_id {
            // Became leader
            self.on_become_leader();
        } else {
            // Became follower
            self.on_become_follower(new_leader);
        }
    }
    
    fn on_become_leader(&mut self) {
        // Start accepting writes
        self.is_leader = true;
        
        // Establish leader lease
        self.establish_lease();
        
        // Notify clients
        self.broadcast_leadership();
    }
    
    fn on_become_follower(&mut self, leader: NodeId) {
        // Stop accepting writes
        self.is_leader = false;
        
        // Redirect clients to leader
        self.leader_address = self.get_node_address(leader);
    }
}
```

#### Split Brain Prevention

```rust
impl ShardRaftGroup {
    fn can_accept_write(&self) -> bool {
        // Must be leader
        if !self.is_leader {
            return false;
        }
        
        // Must have quorum
        let active_replicas = self.count_active_replicas();
        if active_replicas < self.quorum_size() {
            return false;
        }
        
        // Must have valid lease (optional, for lease-based reads)
        if self.config.use_leases && !self.has_valid_lease() {
            return false;
        }
        
        true
    }
}
```

### 8. Performance Optimizations

#### Batch Writes

```rust
impl ShardRaftGroup {
    async fn batch_propose(&mut self, entries: Vec<LogEntry>) -> Result<()> {
        // Combine multiple writes into single Raft proposal
        let batch_entry = LogEntry {
            entry_type: EntryType::Batch {
                operations: entries.into_iter()
                    .map(|e| e.into_operation())
                    .collect()
            },
            ..Default::default()
        };
        
        self.propose_write(batch_entry).await
    }
}
```

#### Pipeline Replication

```rust
impl ShardRaftGroup {
    async fn pipeline_writes(&mut self, entries: Vec<LogEntry>) -> Result<Vec<Result<()>>> {
        // Send multiple proposals without waiting
        let mut futures = Vec::new();
        
        for entry in entries {
            let future = self.raft_node.propose(entry.encode());
            futures.push(future);
        }
        
        // Wait for all to complete
        let results = futures::future::join_all(futures).await;
        
        Ok(results)
    }
}
```

## Consequences

### Positive

* **Strong consistency** - Linearizable operations within shards
* **Automatic failover** - Raft handles leader election
* **Horizontal scalability** - Add nodes to increase capacity
* **Fault tolerance** - Survives minority node failures
* **Predictable behavior** - Well-understood consensus algorithm
* **Operational simplicity** - Minimal manual intervention
* **Battle-tested** - Raft is proven in production systems

### Negative

* **Write latency** - Consensus adds network round-trips (typically 1-5ms)
* **Cross-shard operations** - No atomic transactions across shards initially
* **Rebalancing overhead** - Shard migration consumes resources
* **Metadata coordination** - Metadata Raft group is a potential bottleneck
* **Complexity** - More complex than single-node systems

### Risks

* **Network partitions** - Minority partitions become unavailable
* **Clock skew** - Lease-based reads require synchronized clocks
* **Cascading failures** - Multiple node failures can cause unavailability
* **Rebalancing storms** - Rapid node changes can cause instability

## Alternatives Considered

### 1. Paxos

**Rejected** - More complex to implement and reason about than Raft, with similar performance characteristics.

### 2. Multi-Master with CRDTs

**Rejected** - Provides eventual consistency, which violates ACID requirements for Nanograph.

### 3. Master-Slave Replication

**Rejected** - Limited scalability and requires manual failover.

### 4. Consistent Hashing without Consensus

**Rejected** - Cannot provide strong consistency guarantees.

### 5. Spanner-style TrueTime

**Deferred** - Requires specialized hardware (GPS/atomic clocks). May consider for future versions.

## Implementation Notes

### Phase 1: Single-Node Raft (Week 9)
- Integrate Raft library
- Implement basic log replication
- Add leader election

### Phase 2: Multi-Node Cluster (Week 10-11)
- Add node discovery
- Implement membership changes
- Create metadata Raft group

### Phase 3: Sharding (Week 12-13)
- Implement hash-based partitioning
- Add shard assignment logic
- Create routing layer

### Phase 4: Rebalancing (Week 14)
- Implement shard migration
- Add rebalancing coordinator
- Create monitoring tools

## Related ADRs

* [ADR-0005: Write Ahead Log Support](ADR-0005-Write-Ahead-Log-Support.md)
* [ADR-0012: Transaction Model and Isolation Levels](ADR-0012-Transaction-Model-and-Isolation-Levels.md)
* [ADR-0014: Compaction, Garbage Collection, and Rebalancing](ADR-0014-Compaction-Garbage-Collection-Rebalancing.md)

## References

* Raft consensus algorithm paper
* tikv/raft-rs library
* CockroachDB architecture
* etcd design documentation
* Consul architecture

---

**Next Steps:**
1. Evaluate Raft libraries (tikv/raft-rs recommended)
2. Design metadata schema
3. Implement single-shard Raft group
4. Add multi-node support
5. Implement shard rebalancing
