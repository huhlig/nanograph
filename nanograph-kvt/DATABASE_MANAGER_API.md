# KeyValueDatabaseManager API Design

## Overview

The `KeyValueDatabaseManager` is the top-level API for interacting with Nanograph's key-value storage. It provides:
- **Logical table operations** - Work with tables by name, not physical shards
- **Automatic routing** - Routes keys to the correct shard based on partitioning strategy
- **Metadata management** - Maintains cluster, region, server, namespace, and table metadata
- **Name resolution** - Maps human-readable names to internal IDs

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│              KeyValueDatabaseManager                         │
│  (Logical table operations, name resolution, routing)        │
├─────────────────────────────────────────────────────────────┤
│                                                              │
│  ┌──────────────────┐         ┌──────────────────┐         │
│  │  MetadataCache   │         │ KeyValueShard    │         │
│  │                  │         │    Manager       │         │
│  │  - Name → ID     │         │                  │         │
│  │  - Table config  │         │  - Shard ops     │         │
│  │  - Partitioners  │         │  - Engine mgmt   │         │
│  └──────────────────┘         └──────────────────┘         │
│                                                              │
└─────────────────────────────────────────────────────────────┘
                         │
         ┌───────────────┼───────────────┐
         ▼               ▼               ▼
    ┌─────────┐     ┌─────────┐     ┌─────────┐
    │ Shard 0 │     │ Shard 1 │     │ Shard N │
    │ (LSM)   │     │ (LSM)   │     │ (LSM)   │
    └─────────┘     └─────────┘     └─────────┘
```

## Core API

### Initialization

```rust
use nanograph_kvt::{
    KeyValueDatabaseManager, ClusterConfig, RegionConfig, 
    ServerConfig, StorageEngineType
};

// Create database manager
let manager = KeyValueDatabaseManager::new();

// Initialize cluster hierarchy
let cluster_id = manager.create_cluster(
    ClusterConfig::new("production")
).await?;

let region_id = manager.create_region(
    cluster_id,
    RegionConfig::new("us-east-1")
).await?;

let server_id = manager.create_server(
    region_id,
    ServerConfig::new("server-1")
).await?;

// Register storage engines
manager.register_engine(
    StorageEngineType::new("lsm"),
    Arc::new(lsm_store)
).await?;
```

### Namespace Operations

```rust
// Create namespace (like a database/schema)
let namespace_id = manager.create_namespace(
    "app_db",
    NamespaceConfig::new("app_db")
).await?;

// List namespaces
let namespaces = manager.list_namespaces().await?;

// Get namespace by name
let namespace_id = manager.get_namespace_id("app_db").await?;

// Drop namespace (and all tables within)
manager.drop_namespace("app_db").await?;
```

### Table Operations

```rust
use nanograph_kvt::{TableConfig, Partitioner, HashFunction};

// Create single-shard table
let table_id = manager.create_table(
    namespace_id,
    TableConfig::new("users", StorageEngineType::new("lsm"))
).await?;

// Create multi-shard table with hash partitioning
let table_id = manager.create_table(
    namespace_id,
    TableConfig::new("events", StorageEngineType::new("lsm"))
        .with_shards(4)
        .with_partitioner(Partitioner::Hash {
            hash_fn: HashFunction::Murmur3
        })
        .with_replication(3)
).await?;

// Get table by name
let table_id = manager.get_table_id(namespace_id, "users").await?;

// List tables in namespace
let tables = manager.list_tables(namespace_id).await?;

// Get table metadata
let metadata = manager.get_table_metadata(table_id).await?;

// Drop table
manager.drop_table(namespace_id, "users").await?;
```

### Key-Value Operations (Logical Table Level)

These operations work with table names and automatically route to the correct shard:

```rust
// Put - automatically routes to correct shard
manager.put(
    namespace_id,
    "users",           // table name
    b"user:123",       // key
    b"Alice"           // value
).await?;

// Get - automatically routes to correct shard
let value = manager.get(
    namespace_id,
    "users",
    b"user:123"
).await?;

// Delete
let deleted = manager.delete(
    namespace_id,
    "users",
    b"user:123"
).await?;

// Exists
let exists = manager.exists(
    namespace_id,
    "users",
    b"user:123"
).await?;
```

### Batch Operations

```rust
// Batch put - routes each key to its shard
manager.batch_put(
    namespace_id,
    "users",
    &[
        (b"user:1", b"Alice"),
        (b"user:2", b"Bob"),
        (b"user:3", b"Charlie"),
    ]
).await?;

// Batch get - gathers from multiple shards
let values = manager.batch_get(
    namespace_id,
    "users",
    &[b"user:1", b"user:2", b"user:3"]
).await?;

// Batch delete
let deleted_count = manager.batch_delete(
    namespace_id,
    "users",
    &[b"user:1", b"user:2"]
).await?;
```

### Range Scans

```rust
use nanograph_kvt::KeyRange;

// Scan with prefix (may span multiple shards)
let range = KeyRange::prefix(b"user:".to_vec());
let mut iter = manager.scan(
    namespace_id,
    "users",
    range
).await?;

while let Some(result) = iter.next().await {
    let (key, value) = result?;
    // Process entry
}

// Convenience method for prefix scans
let mut iter = manager.scan_prefix(
    namespace_id,
    "users",
    b"user:",
    Some(100)  // limit
).await?;
```

### Transactions

```rust
// Begin transaction
let txn = manager.begin_transaction().await?;

// Operations within transaction
txn.put(namespace_id, "users", b"user:123", b"Alice").await?;
txn.put(namespace_id, "users", b"user:456", b"Bob").await?;

let value = txn.get(namespace_id, "users", b"user:123").await?;

// Commit or rollback
txn.commit().await?;
// or
txn.rollback().await?;
```

### Statistics and Monitoring

```rust
// Get table statistics
let stats = manager.table_stats(namespace_id, "users").await?;
println!("Keys: {}", stats.key_count);
println!("Size: {} bytes", stats.total_bytes);

// Get shard-level statistics
let shard_stats = manager.shard_stats(shard_id).await?;

// Get all shards for a table
let shards = manager.get_table_shards(table_id).await?;
```

### Maintenance Operations

```rust
// Flush all data to disk
manager.flush().await?;

// Compact specific table
manager.compact_table(namespace_id, "users").await?;

// Compact specific shard
manager.compact_shard(shard_id).await?;

// Compact all tables
manager.compact_all().await?;
```

## Internal Routing Logic

The manager handles routing transparently:

```rust
impl KeyValueDatabaseManager {
    async fn put(&self, namespace: NamespaceId, table_name: &str, key: &[u8], value: &[u8]) 
        -> KeyValueResult<()> 
    {
        // 1. Resolve table name to TableId
        let table_id = self.metadata.get_table_id(namespace, table_name)?;
        
        // 2. Get table metadata (includes partitioner and shard_count)
        let table_meta = self.metadata.get_table_metadata(&table_id)?;
        
        // 3. Determine shard index using partitioner
        let shard_index = if table_meta.shard_count == 1 {
            ShardIndex(0)
        } else {
            table_meta.partitioner
                .as_ref()
                .unwrap()
                .get_shard_index(key, table_meta.shard_count)
        };
        
        // 4. Create ShardId from TableId + ShardIndex
        let shard_id = ShardId::from_parts(table_id, shard_index);
        
        // 5. Route to shard manager
        self.shard_manager.put(shard_id, key, value).await
    }
}
```

## MetadataCache Structure

```rust
pub struct MetadataCache {
    // Cluster hierarchy
    clusters: HashMap<ClusterId, ClusterMetadata>,
    regions: HashMap<RegionId, RegionMetadata>,
    servers: HashMap<ServerId, ServerMetadata>,
    
    // Logical hierarchy
    namespaces: HashMap<NamespaceId, NamespaceMetadata>,
    tables: HashMap<TableId, TableMetadata>,
    shards: HashMap<ShardId, ShardMetadata>,
    
    // Name resolution
    namespace_names: HashMap<String, NamespaceId>,
    table_names: HashMap<(NamespaceId, String), TableId>,
    
    // Table to shards mapping
    table_shards: HashMap<TableId, Vec<ShardId>>,
    
    // ID generators
    next_namespace_id: AtomicU64,
    next_table_id: AtomicU64,
}
```

## Error Handling

```rust
pub enum KeyValueError {
    // Existing errors...
    
    // New database-level errors
    NamespaceNotFound(String),
    NamespaceAlreadyExists(String),
    TableNotFound(String),
    TableAlreadyExists(String),
    InvalidTableName(String),
    InvalidNamespaceName(String),
}
```

## Usage Examples

### Simple Application

```rust
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize
    let manager = KeyValueDatabaseManager::new();
    let cluster_id = manager.create_cluster(ClusterConfig::new("prod")).await?;
    let region_id = manager.create_region(cluster_id, RegionConfig::new("us-east")).await?;
    let server_id = manager.create_server(region_id, ServerConfig::new("srv1")).await?;
    
    // Register engine
    let lsm_store = LSMKeyValueStore::new();
    manager.register_engine(StorageEngineType::new("lsm"), Arc::new(lsm_store)).await?;
    
    // Create namespace and table
    let ns = manager.create_namespace("myapp", NamespaceConfig::new("myapp")).await?;
    manager.create_table(
        ns,
        TableConfig::new("users", StorageEngineType::new("lsm"))
    ).await?;
    
    // Use it
    manager.put(ns, "users", b"user:1", b"Alice").await?;
    let value = manager.get(ns, "users", b"user:1").await?;
    
    Ok(())
}
```

### Multi-Shard Application

```rust
// Create sharded table for high throughput
let ns = manager.create_namespace("analytics", NamespaceConfig::new("analytics")).await?;

manager.create_table(
    ns,
    TableConfig::new("events", StorageEngineType::new("lsm"))
        .with_shards(16)  // 16 shards for parallelism
        .with_partitioner(Partitioner::Hash {
            hash_fn: HashFunction::XXHash  // Fast hash for high throughput
        })
        .with_replication(3)  // 3 replicas for fault tolerance
).await?;

// Write events - automatically distributed across shards
for i in 0..1000 {
    let key = format!("event:{}", i);
    let value = format!("data:{}", i);
    manager.put(ns, "events", key.as_bytes(), value.as_bytes()).await?;
}
```

## Benefits

1. **Simple API** - Users work with table names, not shard IDs
2. **Automatic Routing** - Manager handles key-to-shard mapping
3. **Flexible Partitioning** - Support multiple strategies per table
4. **Metadata Management** - Centralized metadata with caching
5. **Hierarchical Organization** - Cluster → Region → Server → Namespace → Table
6. **Future-Proof** - Ready for distributed Raft integration

## Next Steps

1. Implement `KeyValueDatabaseManager` struct
2. Implement `MetadataCache` with persistence
3. Add routing logic for all operations
4. Update tests to use database manager
5. Add documentation and examples