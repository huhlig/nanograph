# Type Unification: KeyValueTableId vs ShardId

## Problem

Currently we have confusing overlapping types:
- `KeyValueTableId` - Used in KeyValueStore trait
- `ShardId` - Used for distributed sharding
- `TableId` - Logical table identifier (u128)
- `ShardIdentifier` - Combines TableId + shard_index

## Analysis

### What KeyValueStore Actually Operates On

```rust
trait KeyValueStore {
    async fn get(&self, table: KeyValueTableId, key: &[u8]) -> Result<Option<Vec<u8>>>;
    //                        ^^^^^^^^^^^^^^^^
    //                        This is misleading!
}
```

**Reality**: A `KeyValueStore` instance (LSM, B+Tree, etc.) operates on a **single shard**, not a logical table.

- One LSM engine = one shard
- One B+Tree engine = one shard  
- A logical table may have multiple shards
- Each shard is a separate storage engine instance

### What the Parameter Really Represents

When you call `store.get(table, key)`:
- In single-node mode: `table` identifies which storage engine
- In distributed mode: `table` identifies which **shard**
- The parameter should be called `shard`, not `table`!

## Solution: Unify Types

### Recommendation

**Replace `KeyValueTableId` with `ShardId`** throughout the codebase.

```rust
// BEFORE (Confusing)
pub struct KeyValueTableId(pub u128);

trait KeyValueStore {
    async fn get(&self, table: KeyValueTableId, key: &[u8]) -> Result<Option<Vec<u8>>>;
}

// AFTER (Clear)
pub struct ShardId(pub u64);  // Already exists!

trait KeyValueStore {
    async fn get(&self, shard: ShardId, key: &[u8]) -> Result<Option<Vec<u8>>>;
    //                        ^^^^^
    //                        Much clearer!
}
```

### Type Hierarchy (Corrected)

```
Logical Layer:
  TableId(u128) - Identifies a logical table
    └─ ShardIdentifier { table_id: TableId, shard_index: u32 }
        └─ to_shard_id() → ShardId(u64)

Physical Layer:
  ShardId(u64) - Identifies a physical shard (storage engine instance)
    └─ Used by KeyValueStore trait
    └─ Used by WAL (one WAL per shard)
    └─ Used by Raft (one Raft group per shard)
```

### Mapping

```
Logical Table "users" (TableId=11111, 4 shards)
  ├─ Shard 0: ShardIdentifier{table_id=11111, index=0} → ShardId(1001)
  ├─ Shard 1: ShardIdentifier{table_id=11111, index=1} → ShardId(1002)
  ├─ Shard 2: ShardIdentifier{table_id=11111, index=2} → ShardId(1003)
  └─ Shard 3: ShardIdentifier{table_id=11111, index=3} → ShardId(1004)

Each ShardId corresponds to:
  - One KeyValueStore instance (LSM/B+Tree)
  - One WAL segment
  - One Raft group (in distributed mode)
```

## Implementation Plan

### Direct Replacement (No Backward Compatibility)

Since we're in active development, make a clean break:

### Step 1: Remove KeyValueTableId, Use ShardId

```rust
// In nanograph-kvt/src/kvstore.rs
// DELETE KeyValueTableId entirely

// ShardId already exists in types.rs - use it directly!
use crate::types::ShardId;

#[async_trait]
pub trait KeyValueStore: Send + Sync {
    // Use ShardId directly, rename parameter from 'table' to 'shard'
    async fn get(&self, shard: ShardId, key: &[u8]) -> KeyValueResult<Option<Vec<u8>>>;
    async fn put(&self, shard: ShardId, key: &[u8], value: &[u8]) -> KeyValueResult<()>;
    async fn delete(&self, shard: ShardId, key: &[u8]) -> KeyValueResult<bool>;
    // ... etc
}
```

### Step 2: Update Manager

Rename `KeyValueTableManager` to `ShardManager`:

```rust
pub struct ShardManager {
    /// Shard metadata: shard_id -> (metadata, engine_type)
    shards: Arc<RwLock<HashMap<ShardId, (ShardMetadata, StorageEngineType)>>>,
    
    /// Logical table to shards mapping
    table_shards: Arc<RwLock<HashMap<TableId, Vec<ShardId>>>>,
}

impl ShardManager {
    /// Create a new shard for a table
    pub async fn create_shard(
        &self,
        table_id: TableId,
        shard_index: u32,
        config: ShardConfig,
    ) -> Result<ShardId> {
        let shard_ident = ShardIdentifier::new(table_id, shard_index);
        let shard_id = shard_ident.to_shard_id();
        
        // Create storage engine for this shard
        let engine = self.create_engine(config)?;
        
        // Register shard
        self.shards.write().await.insert(shard_id, (metadata, engine_type));
        
        Ok(shard_id)
    }
    
    /// Get all shards for a logical table
    pub fn get_table_shards(&self, table_id: TableId) -> Result<Vec<ShardId>> {
        self.table_shards.read().unwrap()
            .get(&table_id)
            .cloned()
            .ok_or(Error::TableNotFound)
    }
}
```

## Benefits

1. **Clarity**: `ShardId` clearly indicates physical storage
2. **Consistency**: Same type used throughout (Raft, WAL, Storage)
3. **Correctness**: Reflects actual architecture (one engine = one shard)
4. **Simplicity**: Fewer types to understand
5. **Scalability**: Clear path to distributed sharding
6. **Clean Break**: No deprecated code or backward compatibility burden

## Implementation Steps

### 1. Delete KeyValueTableId

```rust
// In nanograph-kvt/src/kvstore.rs
// DELETE this entire section:
// pub struct KeyValueTableId(pub u128);
// impl KeyValueTableId { ... }
```

### 2. Import ShardId

```rust
// In nanograph-kvt/src/kvstore.rs
use crate::types::ShardId;  // Already defined in types.rs!
```

### 3. Update All Trait Methods

Replace all occurrences of `KeyValueTableId` with `ShardId` and rename parameter from `table` to `shard`.

### 4. Update All Implementations

Update LSM, B+Tree, and any other implementations to use `ShardId`.

### 5. Update Manager

Rename `KeyValueTableManager` → `ShardManager` and update all references.

## Conclusion

**`KeyValueTableId` and `ShardId` represent the same concept** - a physical storage unit.

They should be unified as `ShardId` because:
- A KeyValueStore operates on a single shard
- The name "table" is misleading (implies logical table)
- ShardId is already used in distributed layer
- Unification simplifies the architecture

**Recommendation**: Deprecate `KeyValueTableId`, use `ShardId` everywhere.