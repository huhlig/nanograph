# Shard ID Collision Issue - RESOLVED ✅

## Problem Statement

The initial implementation had a potential collision between table data shards and index shards.

### Initial Problematic Structure
```rust
// Both used identical u128 layout:
ShardId:  [TenantId:32][DatabaseId:32][TableId:32][ShardNumber:32]
IndexId:  [TenantId:32][DatabaseId:32][TableId:32][IndexNumber:32]
```

### Collision Scenario
If TableId and IndexId were allocated from **separate pools**:
- Table with ID 1: `ShardId::from_parts(tenant, db, TableId(1), shard_0)`
- Index with ID 1: `IndexId::from_parts(tenant, db, IndexId(1), shard_0)`
- **Both would produce the same u128 value!** → COLLISION

This would cause index data and table data to collide in the storage layer.

## Solution: Unified ObjectId Allocation ✅

### The Fix

**All database objects (Tables, Indexes, Functions, Namespaces) share a single ObjectId allocation pool per database.**

```rust
// Unified ObjectId type
pub type ObjectId = u32;

// All objects use ObjectIds from the same pool
pub struct TableId(pub ObjectId);
pub struct IndexId(pub u128);  // Contains ObjectId in bits 32-63

// Object type tracked separately
pub enum ObjectType {
    Namespace,
    Table,
    Index,
    Function,
}
```

### How It Works

1. **Single Allocator Per Database**
   ```rust
   struct DatabaseObjectAllocator {
       next_id: AtomicU32,
   }
   ```

2. **Unified Allocation**
   ```rust
   // Create table
   let table_object_id = allocator.allocate(); // Returns 100
   let table = TableRecord { id: TableId(100), ... };
   
   // Create index
   let index_object_id = allocator.allocate(); // Returns 101 (NOT 1!)
   let index = IndexRecord {
       id: IndexId::from_parts(tenant, db, 101, shard_0),
       ...
   };
   ```

3. **No Collision**
   - TableId(100) → ShardId with ObjectId=100
   - IndexId(101) → ShardId with ObjectId=101
   - Different ObjectIds = No collision!

### Implementation Changes

#### nanograph-core/src/object.rs
```rust
/// Object Identifier used by all Database Objects within a container.
///
/// **IMPORTANT**: ObjectIds are allocated from a unified pool per database.
/// This prevents collisions when constructing ShardIds.
pub type ObjectId = u32;

pub enum ObjectType {
    Namespace,
    Table,
    Index,      // ← Added
    Function,
}

pub enum ObjectMetadata {
    Function(FunctionRecord),
    Index(IndexRecord),    // ← Added
    Namespace(NamespaceRecord),
    Table(TableRecord),
}
```

#### nanograph-core/src/object/table.rs
```rust
/// Table identifier
///
/// **IMPORTANT**: TableId shares the same ObjectId allocation pool with IndexId.
pub struct TableId(pub ObjectId);

/// Index identifier
///
/// **IMPORTANT**: IndexId shares the same ObjectId allocation pool with TableId.
pub struct IndexId(pub u128);

impl IndexId {
    pub fn from_parts(
        tenant: TenantId,
        database: DatabaseId,
        index: ObjectId,      // ← From unified pool
        shard: ShardNumber,
    ) -> Self { ... }
    
    pub fn to_shard_id(&self) -> ShardId {
        ShardId(self.0)
    }
}
```

### Benefits

✅ **No Collisions**: Guaranteed unique ObjectIds across all object types
✅ **Simple Allocation**: Single atomic counter per database
✅ **Clear Semantics**: ObjectId uniquely identifies an object
✅ **Scalable**: 4 billion objects per database (u32)
✅ **Type Safety**: Object type tracked separately in metadata

### Documentation

See `nanograph-core/OBJECT_ID_ALLOCATION.md` for complete details on:
- Allocation strategy
- Metadata tracking
- Migration considerations
- Testing approach

## Proposed Solutions

### Option 1: Type Discriminator Bit (Recommended)
Use the high bit of the u128 to distinguish entity types:

```rust
// Bit layout: [Type:1][Reserved:31][TenantId:32][DatabaseId:32][TableId:32][Number:32]
// Type bit: 0 = Table Shard, 1 = Index

ShardId:  [0][000...][TenantId:32][DatabaseId:32][TableId:32][ShardNumber:32]
IndexId:  [1][000...][TenantId:32][DatabaseId:32][TableId:32][IndexNumber:32]
```

**Pros:**
- Clean separation at the bit level
- No range restrictions
- Easy to implement
- Future-proof (31 reserved bits for other entity types)

**Cons:**
- Requires modifying ShardId and IndexId structures
- Need to update all from_parts() methods

### Option 2: Reserved Shard Number Ranges
Reserve the upper half of the u32 range for indexes:

```rust
// ShardNumber: 0x00000000 - 0x7FFFFFFF (2 billion table shards)
// IndexNumber: 0x80000000 - 0xFFFFFFFF (2 billion indexes)

const INDEX_SHARD_OFFSET: u32 = 0x80000000;

impl IndexId {
    pub fn to_shard_id(&self) -> ShardId {
        ShardId(self.0 | (INDEX_SHARD_OFFSET as u128))
    }
}
```

**Pros:**
- Minimal code changes
- No structural changes needed

**Cons:**
- Reduces available shard/index space by half
- Implicit convention (easy to violate)
- Confusing semantics

### Option 3: Separate Index Shard Structure
Create a distinct shard structure for indexes:

```rust
pub struct IndexShardId(pub u128);

impl IndexShardId {
    pub fn from_parts(
        tenant: TenantId,
        database: DatabaseId,
        table: TableId,
        index: IndexNumber,
        shard: ShardNumber,  // Indexes can also be sharded!
    ) -> Self {
        // [TenantId:32][DatabaseId:24][TableId:24][IndexNumber:24][ShardNumber:24]
        Self(
            (tenant.0 as u128) << 96
                | ((database.0 & 0xFFFFFF) as u128) << 72
                | ((table.0 & 0xFFFFFF) as u128) << 48
                | ((index.0 & 0xFFFFFF) as u128) << 24
                | ((shard.0 & 0xFFFFFF) as u128)
        )
    }
}
```

**Pros:**
- Indexes can be sharded independently
- Clear semantic separation
- Supports large-scale index distribution

**Cons:**
- More complex implementation
- Requires new type throughout codebase

## Recommendation

**Implement Option 1: Type Discriminator Bit**

This provides:
1. Clean separation with no collision risk
2. Future extensibility (can add more entity types)
3. Minimal performance impact
4. Clear semantics

## Implementation Plan

### Phase 1: Update Core Types
1. Modify `ShardId::from_parts()` to set type bit to 0
2. Modify `IndexId::from_parts()` to set type bit to 1
3. Add `IndexId::to_shard_id()` conversion method
4. Add validation methods to check type bits

### Phase 2: Update Persistence Layer
1. Update `PersistentIndexStore` to use `IndexId::to_shard_id()`
2. Ensure all index operations use the correct shard ID
3. Add tests to verify no collisions

### Phase 3: Documentation
1. Document the type bit convention
2. Update architecture diagrams
3. Add migration notes

## Example Implementation

```rust
impl ShardId {
    const TYPE_BIT_SHIFT: u32 = 127;
    const TYPE_TABLE: u128 = 0;
    
    pub fn from_parts(
        tenant: TenantId,
        database: DatabaseId,
        table: TableId,
        index: ShardNumber,
    ) -> Self {
        Self(
            (Self::TYPE_TABLE << Self::TYPE_BIT_SHIFT)
                | (tenant.0 as u128) << 96
                | (database.0 as u128) << 64
                | (table.0 as u128) << 32
                | (index.0 as u128)
        )
    }
    
    pub fn is_table_shard(&self) -> bool {
        (self.0 >> Self::TYPE_BIT_SHIFT) == Self::TYPE_TABLE
    }
}

impl IndexId {
    const TYPE_BIT_SHIFT: u32 = 127;
    const TYPE_INDEX: u128 = 1;
    
    pub fn from_parts(
        tenant: TenantId,
        database: DatabaseId,
        table: TableId,
        index: IndexNumber,
    ) -> Self {
        Self(
            (Self::TYPE_INDEX << Self::TYPE_BIT_SHIFT)
                | (tenant.0 as u128) << 96
                | (database.0 as u128) << 64
                | (table.0 as u128) << 32
                | (index.0 as u128)
        )
    }
    
    pub fn to_shard_id(&self) -> ShardId {
        ShardId(self.0)
    }
    
    pub fn is_index(&self) -> bool {
        (self.0 >> Self::TYPE_BIT_SHIFT) == Self::TYPE_INDEX
    }
}
```

## Testing Strategy

1. **Unit Tests**: Verify no collisions between ShardId and IndexId
2. **Integration Tests**: Test index storage with multiple indexes
3. **Stress Tests**: Create many shards and indexes, verify separation

## Migration Considerations

- Existing data will need type bits set correctly
- Migration script to update all ShardId/IndexId values
- Backward compatibility during transition period