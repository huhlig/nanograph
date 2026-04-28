# Unified ObjectId Allocation Strategy

## Overview

All database objects (Tables, Indexes, Functions, Namespaces) within a database share a **unified ObjectId allocation pool**. This design prevents collisions when constructing ShardIds for storage operations.

## Problem Statement

### The Collision Issue

ShardId uses a u128 structure: `[TenantId:32][DatabaseId:32][ObjectId:32][ShardNumber:32]`

If Tables and Indexes had separate ID allocators:
- TableId(1) → ShardId with value X
- IndexId(1) → ShardId with value X (COLLISION!)

Both would map to the same storage key space, causing data corruption.

### The Solution

**Use a single ObjectId allocator per database** that assigns unique IDs to ALL object types:
- Tables
- Indexes  
- Functions
- Namespaces

This ensures that no two objects within a database can have the same ObjectId, preventing ShardId collisions.

## Implementation

### Core Type Definition

```rust
/// Object Identifier used by all Database Objects within a container.
///
/// **IMPORTANT**: ObjectIds are allocated from a unified pool per database.
/// This means Tables, Indexes, Functions, and Namespaces all share the same
/// ID space to prevent collisions when constructing ShardIds.
pub type ObjectId = u32;
```

### Object Type Tracking

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ObjectType {
    Namespace,
    Table,
    Index,
    Function,
}

#[derive(Clone, Debug)]
pub enum ObjectMetadata {
    Function(FunctionRecord),
    Index(IndexRecord),
    Namespace(NamespaceRecord),
    Table(TableRecord),
}
```

The system tracks object types separately in metadata, so it knows whether ObjectId(42) refers to a table, index, function, or namespace.

### ID Structures

#### TableId
```rust
/// Table identifier
///
/// **IMPORTANT**: TableId shares the same ObjectId allocation pool with IndexId,
/// FunctionId, and NamespaceId within a database.
pub struct TableId(pub ObjectId);
```

#### IndexId
```rust
/// Index identifier
///
/// **IMPORTANT**: IndexId shares the same ObjectId allocation pool with TableId,
/// FunctionId, and NamespaceId within a database.
///
/// The u128 structure allows indexes to be sharded:
/// `[TenantId:32][DatabaseId:32][IndexId(ObjectId):32][ShardNumber:32]`
pub struct IndexId(pub u128);

impl IndexId {
    pub fn from_parts(
        tenant: TenantId,
        database: DatabaseId,
        index: ObjectId,  // From unified pool
        shard: ShardNumber,
    ) -> Self { ... }
    
    pub fn to_shard_id(&self) -> ShardId {
        ShardId(self.0)
    }
}
```

## Allocation Strategy

### Distributed Allocation via Raft

In a distributed system, ObjectId allocation must be coordinated across all nodes to ensure uniqueness. This is achieved through **Raft consensus**.

#### Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                    Database Metadata Shard                   │
│                  (Replicated via Raft)                       │
├─────────────────────────────────────────────────────────────┤
│  Leader Node                                                 │
│  ┌────────────────────────────────────────────────────────┐ │
│  │ DatabaseObjectAllocator                                │ │
│  │   next_id: AtomicU32                                   │ │
│  │                                                        │ │
│  │ allocate() -> ObjectId                                 │ │
│  │   1. Increment next_id                                 │ │
│  │   2. Propose to Raft: AllocateObjectId(new_id)        │ │
│  │   3. Wait for commit                                   │ │
│  │   4. Return new_id                                     │ │
│  └────────────────────────────────────────────────────────┘ │
└─────────────────────────────────────────────────────────────┘
         │                    │                    │
         ▼                    ▼                    ▼
    Follower 1           Follower 2           Follower 3
    (Replica)            (Replica)            (Replica)
```

#### Implementation

```rust
struct DatabaseObjectAllocator {
    /// Current next ID (replicated state)
    next_id: AtomicU32,
    /// Raft group for this database's metadata
    raft_group: Arc<ShardGroup>,
}

impl DatabaseObjectAllocator {
    /// Allocate a new ObjectId (distributed)
    pub async fn allocate(&self) -> Result<ObjectId, AllocationError> {
        // Only the Raft leader can allocate
        if !self.raft_group.is_leader().await {
            return Err(AllocationError::NotLeader);
        }
        
        // Optimistically increment
        let new_id = self.next_id.fetch_add(1, Ordering::SeqCst);
        
        // Propose through Raft for durability and replication
        let command = MetadataCommand::AllocateObjectId {
            object_id: new_id
        };
        
        self.raft_group.propose(command).await?;
        
        // Wait for commit (ensures durability)
        self.raft_group.wait_for_commit().await?;
        
        Ok(new_id)
    }
    
    /// Apply committed allocation (called on all nodes)
    pub fn apply_allocation(&self, object_id: ObjectId) {
        // Update local state to match committed value
        self.next_id.store(object_id + 1, Ordering::SeqCst);
    }
}
```

#### Raft State Machine

The database metadata is stored in a Raft-replicated state machine:

```rust
enum MetadataCommand {
    AllocateObjectId { object_id: ObjectId },
    CreateTable { id: ObjectId, record: TableRecord },
    CreateIndex { id: ObjectId, record: IndexRecord },
    // ... other metadata operations
}

impl RaftStateMachine for DatabaseMetadata {
    fn apply(&mut self, command: MetadataCommand) -> Result<()> {
        match command {
            MetadataCommand::AllocateObjectId { object_id } => {
                // Update allocator state
                self.allocator.apply_allocation(object_id);
                Ok(())
            }
            MetadataCommand::CreateTable { id, record } => {
                self.objects.insert(id, (ObjectType::Table, record));
                Ok(())
            }
            // ... handle other commands
        }
    }
}
```

### Distributed Object Creation Flow

1. **Client Request**: Create table/index/function/namespace
2. **Route to Leader**: Request routed to current Raft leader
3. **Allocate ObjectId**: Leader allocates from unified pool via Raft
4. **Create Object Record**: Construct object with allocated ObjectId
5. **Propose to Raft**: Propose CreateObject command
6. **Replicate**: Raft replicates to majority of nodes
7. **Commit**: Once majority acknowledges, commit
8. **Apply**: All nodes apply the creation to their state
9. **Return**: Success response to client

#### Failure Scenarios

**Leader Failure During Allocation**:
- Raft elects new leader
- New leader has replicated state (including next_id)
- Client retries allocation with new leader
- No ObjectId is lost or duplicated

**Network Partition**:
- Minority partition cannot allocate (no quorum)
- Majority partition continues allocating
- When partition heals, minority catches up via Raft log

**Concurrent Allocations**:
- All go through single Raft leader
- Serialized by Raft log
- Each gets unique ObjectId

### Performance Considerations

#### Allocation Latency

```
Allocation Time = Raft Consensus Latency
                = Network RTT + Disk Sync
                ≈ 1-10ms (typical)
```

**Optimization: Batch Allocation**
```rust
impl DatabaseObjectAllocator {
    /// Pre-allocate a range of ObjectIds
    pub async fn allocate_range(&self, count: u32) -> Result<Range<ObjectId>, AllocationError> {
        let start = self.next_id.fetch_add(count, Ordering::SeqCst);
        let end = start + count;
        
        let command = MetadataCommand::AllocateObjectIdRange {
            start,
            end
        };
        
        self.raft_group.propose(command).await?;
        self.raft_group.wait_for_commit().await?;
        
        Ok(start..end)
    }
}
```

Benefits:
- Amortize Raft overhead across multiple allocations
- Useful for bulk operations (e.g., creating many indexes)
- Reduces latency for batch table creation

#### Caching Strategy

**Local ID Cache** (per node):
```rust
struct LocalIdCache {
    /// Pre-allocated range
    range: Range<ObjectId>,
    /// Next ID to use from range
    next: AtomicU32,
}

impl LocalIdCache {
    pub async fn allocate(&self, allocator: &DatabaseObjectAllocator) -> Result<ObjectId> {
        // Try local cache first
        let current = self.next.load(Ordering::SeqCst);
        if current < self.range.end {
            let id = self.next.fetch_add(1, Ordering::SeqCst);
            if id < self.range.end {
                return Ok(id);
            }
        }
        
        // Cache exhausted, allocate new range from Raft
        let new_range = allocator.allocate_range(1000).await?;
        self.range = new_range.clone();
        self.next.store(new_range.start + 1, Ordering::SeqCst);
        Ok(new_range.start)
    }
}
```

This reduces Raft round-trips by 1000x for high-throughput scenarios.

### Example

```rust
// Database allocator state: next_id = 100

// Create a table
let table_id = allocator.allocate(); // Returns 100
let table = TableRecord { id: TableId(100), ... };
metadata.store(100, ObjectType::Table, table);

// Create an index on that table
let index_id = allocator.allocate(); // Returns 101 (NOT 1!)
let index = IndexRecord { id: IndexId::from_parts(tenant, db, 101, 0), ... };
metadata.store(101, ObjectType::Index, index);

// No collision: TableId(100) and IndexId(101) are different
```

## ShardId Construction

### For Tables
```rust
let table_shard = ShardId::from_parts(
    tenant,
    database,
    table_id,      // ObjectId from unified pool
    shard_number
);
```

### For Indexes
```rust
let index_id = IndexId::from_parts(
    tenant,
    database,
    index_object_id,  // ObjectId from unified pool
    shard_number
);
let index_shard = index_id.to_shard_id();
```

Since `table_id` and `index_object_id` come from the same pool and are guaranteed unique, there can be no collision in the ShardId space.

## Metadata Resolution

The system maintains a metadata store that maps ObjectId → (ObjectType, ObjectRecord):

```rust
struct MetadataStore {
    objects: HashMap<ObjectId, (ObjectType, ObjectMetadata)>,
}

impl MetadataStore {
    fn get_object(&self, id: ObjectId) -> Option<&(ObjectType, ObjectMetadata)> {
        self.objects.get(&id)
    }
    
    fn get_type(&self, id: ObjectId) -> Option<ObjectType> {
        self.objects.get(&id).map(|(t, _)| *t)
    }
}
```

This allows the system to:
1. Determine if an ObjectId refers to a table or index
2. Retrieve the full object record
3. Validate object existence
4. Enforce referential integrity

## Benefits

### ✅ No Collisions
- Guaranteed unique ObjectIds across all object types
- Safe ShardId construction
- No data corruption risk

### ✅ Simple Allocation
- Single atomic counter per database
- No complex coordination
- Fast allocation

### ✅ Clear Semantics
- ObjectId uniquely identifies an object
- Object type tracked separately
- Easy to reason about

### ✅ Scalable
- 4 billion objects per database (u32)
- Sufficient for any realistic workload
- Can be extended to u64 if needed

## Migration Considerations

### Existing Systems

If migrating from separate allocators:

1. **Audit Existing IDs**: Identify all allocated TableIds and IndexIds
2. **Renumber Objects**: Assign new ObjectIds from unified pool
3. **Update Metadata**: Map old IDs to new ObjectIds
4. **Update ShardIds**: Reconstruct ShardIds with new ObjectIds
5. **Migrate Data**: Move data to new shard locations
6. **Verify Integrity**: Ensure no collisions or data loss

### New Systems

For new deployments:
1. Initialize database with unified allocator
2. All objects automatically use unified pool
3. No migration needed

## Testing Strategy

### Unit Tests
- Verify allocator returns unique IDs
- Test ObjectId → ObjectType mapping
- Validate ShardId construction

### Integration Tests
- Create tables and indexes in same database
- Verify no ShardId collisions
- Test metadata resolution

### Stress Tests
- Allocate millions of objects
- Verify uniqueness maintained
- Test concurrent allocation

## Future Enhancements

### Potential Improvements

1. **ID Ranges**: Reserve ranges for different object types (optional optimization)
2. **ID Recycling**: Reuse IDs from deleted objects (with caution)
3. **Distributed Allocation**: Coordinate allocation across nodes
4. **ID Namespaces**: Support multiple databases per tenant

### Backward Compatibility

Any changes must maintain:
- Unique ObjectIds within a database
- No ShardId collisions
- Metadata consistency

## Summary

The unified ObjectId allocation strategy is a **critical design decision** that:
- Prevents data corruption from ShardId collisions
- Simplifies object management
- Provides clear semantics
- Scales to billions of objects

All database objects **must** use the unified allocator to maintain system integrity.