# Filesystem Storage Architecture and Tablespace Design

## Overview

This document explains how Nanograph handles filesystem storage pathing and the tablespace concept that enables flexible storage management in a distributed database system.

## Original Question

**"How is filesystem storage pathing handled and do we need to introduce the concept of tablespaces for tables to exist in?"**

**Answer**: Yes, we have introduced tablespaces to enable:
1. **Storage Tiering**: Different tables/shards can use different storage tiers (NVMe, SSD, HDD, Object Storage)
2. **Multi-Tenancy Isolation**: Physical separation of tenant data
3. **Node-Specific Configuration**: Each node can have different storage paths for the same tablespace
4. **Flexible Storage Management**: Easy migration between storage tiers without application changes

## Filesystem Storage Pathing

### Path Structure

All storage paths follow a consistent hierarchical structure:

```
<tablespace_base_path>/<engine_type>/table_<table_id>/shard_<shard_id>/{data|wal}
```

**Example:**
```
/mnt/nvme/hot/lsm/table_42/shard_0/data     # LSM data files
/mnt/nvme/hot/lsm/table_42/shard_0/wal      # LSM WAL files
/mnt/ssd/warm/btree/table_100/shard_5/data  # B+Tree data files
/mnt/hdd/cold/art/table_200/shard_10/wal    # ART WAL files
```

### Path Resolution Process

1. **Table Creation**: User specifies tablespace ID (or uses DEFAULT)
2. **Shard Creation**: System resolves paths using `StoragePathResolver`
3. **Path Lookup**: Resolver checks node-local tablespace configuration
4. **Directory Creation**: VFS creates necessary directories
5. **Engine Initialization**: Storage engine uses resolved paths

### VFS Abstraction

All filesystem operations go through the Virtual File System (VFS) layer:

```rust
pub trait DynamicFileSystem {
    fn create_directory_all(&self, path: &str) -> Result<()>;
    fn read_file(&self, path: &str) -> Result<Vec<u8>>;
    fn write_file(&self, path: &str, data: &[u8]) -> Result<()>;
    // ... more methods
}
```

**Benefits:**
- **Testability**: Use `MemoryFileSystem` for tests
- **Flexibility**: Support different backends (Local, Network, Object Storage)
- **Monitoring**: Wrap with `MonitoredFileSystem` for metrics
- **Abstraction**: No direct filesystem access in storage engines

## Tablespace Concept

### What is a Tablespace?

A **tablespace** is a logical storage location that maps to physical storage paths on each node. It provides:

1. **Logical Naming**: Reference storage by name (e.g., "hot", "warm", "cold")
2. **Physical Mapping**: Each node maps tablespace to local paths
3. **Storage Tiering**: Classify storage by performance characteristics
4. **Flexibility**: Move data between tiers without application changes

### Tablespace Configuration

Each node maintains its own tablespace configuration:

```rust
pub struct TablespaceConfig {
    pub id: TablespaceId,           // Unique identifier
    pub base_path: PathBuf,         // Node-local base path
    pub tier: StorageTier,          // Hot/Warm/Cold/Archive
    pub available: bool,            // Is this tablespace available on this node?
}
```

**Example Configuration:**

```toml
# Node 1 (has NVMe and SSD)
[[tablespaces]]
id = 0
name = "default"
base_path = "/mnt/nvme/nanograph"
tier = "hot"
available = true

[[tablespaces]]
id = 1
name = "warm_storage"
base_path = "/mnt/ssd/nanograph"
tier = "warm"
available = true

# Node 2 (has SSD and HDD)
[[tablespaces]]
id = 0
name = "default"
base_path = "/mnt/ssd/nanograph"
tier = "warm"
available = true

[[tablespaces]]
id = 2
name = "cold_storage"
base_path = "/mnt/hdd/nanograph"
tier = "cold"
available = true
```

### Storage Tiers

```rust
pub enum StorageTier {
    Hot,      // Fastest: NVMe, RAM disk
    Warm,     // Balanced: SSD
    Cold,     // Slower: HDD
    Archive,  // Archival: Object storage, tape
}
```

**Use Cases:**
- **Hot**: Frequently accessed data, real-time analytics
- **Warm**: Regular access, general purpose
- **Cold**: Infrequent access, historical data
- **Archive**: Long-term retention, compliance

## Hierarchical Data Organization

### Complete Hierarchy

```
Cluster (Global)
├── Region (Geographic)
│   ├── Server/Node (Physical Machine)
│   │   ├── Tablespace (Storage Location)
│   │   │   ├── Tenant (Multi-tenancy Isolation)
│   │   │   │   ├── Database (Container)
│   │   │   │   │   ├── Namespace (Logical Grouping)
│   │   │   │   │   │   ├── Table (Data Structure)
│   │   │   │   │   │   │   ├── Shard (Horizontal Partition)
│   │   │   │   │   │   │   │   └── Storage Engine (LSM/B+Tree/ART)
│   │   │   │   │   │   │   │       ├── Data Files
│   │   │   │   │   │   │   │       └── WAL Files
```

### Relationship to Filesystem

- **Cluster/Region/Server**: Logical organization (not in filesystem)
- **Tablespace**: Maps to base directory on each node
- **Tenant/Database/Namespace**: Could be in path, but currently not (metadata only)
- **Table/Shard**: Directly in filesystem path
- **Storage Engine**: Determines subdirectory structure

## Storage Engine Integration

### Engine-Specific Configurations

Each storage engine has its own configuration structure:

**LSM:**
```rust
pub struct LSMStorageConfig {
    pub data_dir: String,
    pub wal_dir: String,
    pub options: LSMOptions,
}
```

**B+Tree:**
```rust
pub struct BTreeStorageConfig {
    pub data_dir: String,
    pub wal_dir: String,
    pub order: usize,
    pub cache_size_mb: Option<usize>,
}
```

**ART:**
```rust
pub struct ARTStorageConfig {
    pub data_dir: PathBuf,
    pub wal_dir: PathBuf,
    pub max_entries: usize,
    pub cache_size_mb: Option<usize>,
}
```

### Tablespace-Aware Shard Creation

```rust
// 1. Resolve paths from tablespace
let data_path = path_resolver.resolve_data_path(
    tablespace_id,
    table_id,
    shard_id,
    engine_type,
)?;

let wal_path = path_resolver.resolve_wal_path(
    tablespace_id,
    table_id,
    shard_id,
    engine_type,
)?;

// 2. Create engine-specific config
let config = LSMStorageConfig::new(
    data_path.to_string_lossy().to_string(),
    wal_path.to_string_lossy().to_string(),
);

// 3. Create shard with config
engine.create_shard_with_config(shard_id, vfs, config)?;
```

## Distributed Coordination

### Raft Integration

Tablespace metadata is coordinated through Raft consensus:

1. **System Metadata Tier**: Stores tablespace definitions
2. **Database Metadata Tier**: Stores table-to-tablespace mappings
3. **Data Tier**: Actual shard data in tablespaces

### Node-Local vs. Global

- **Global (Raft)**: Tablespace definitions, table assignments
- **Node-Local**: Physical paths, availability status
- **Coordination**: Raft ensures consistent tablespace assignments
- **Flexibility**: Each node can have different physical paths

## Use Cases

### 1. Storage Tiering

```sql
-- Create table in hot storage for real-time data
CREATE TABLE realtime_events (
    event_id BIGINT,
    timestamp TIMESTAMP,
    data JSONB
) TABLESPACE hot_nvme;

-- Create table in cold storage for historical data
CREATE TABLE historical_events (
    event_id BIGINT,
    timestamp TIMESTAMP,
    data JSONB
) TABLESPACE cold_hdd;
```

### 2. Multi-Tenancy Isolation

```sql
-- Tenant A gets dedicated NVMe storage
CREATE TABLESPACE tenant_a_storage
    LOCATION '/mnt/nvme/tenant_a'
    TIER 'hot';

-- Tenant B gets shared SSD storage
CREATE TABLESPACE tenant_b_storage
    LOCATION '/mnt/ssd/tenant_b'
    TIER 'warm';
```

### 3. Data Lifecycle Management

```sql
-- Move table from hot to warm storage
ALTER TABLE user_sessions
    SET TABLESPACE warm_ssd;

-- Archive old data to cold storage
ALTER TABLE audit_logs
    SET TABLESPACE cold_hdd
    WHERE created_at < NOW() - INTERVAL '1 year';
```

### 4. Disaster Recovery

```sql
-- Create tablespace on replicated storage
CREATE TABLESPACE replicated_storage
    LOCATION '/mnt/replicated/nanograph'
    TIER 'warm'
    REPLICATION 'synchronous';
```

## Implementation Status

### ✅ Completed

1. **Core Types**: `TablespaceId` in nanograph-core
2. **Storage Configs**: LSM, B+Tree, ART configurations
3. **Path Resolver**: Complete path resolution system
4. **VFS Integration**: All engines use VFS abstraction
5. **Shard Manager**: Infrastructure for tablespace-aware creation
6. **Documentation**: ADR and implementation guide

### 🚧 In Progress

1. **Trait Enhancement**: Add `create_shard_with_config` to `KeyValueShardStore` trait
2. **Database Manager**: Tablespace management APIs
3. **Configuration**: File-based tablespace configuration
4. **Testing**: Comprehensive test suite

### 📋 Planned

1. **CLI Tools**: Tablespace management commands
2. **Monitoring**: Tablespace usage metrics
3. **Migration Tools**: Move data between tablespaces
4. **Auto-Tiering**: Automatic data movement based on access patterns

## Best Practices

### 1. Tablespace Naming

- Use descriptive names: `hot_nvme`, `warm_ssd`, `cold_hdd`
- Include tier in name for clarity
- Consider tenant/purpose: `tenant_a_hot`, `analytics_warm`

### 2. Path Organization

- Use consistent base paths across nodes
- Separate data and WAL when possible
- Consider RAID configuration for each tier

### 3. Capacity Planning

- Monitor tablespace usage
- Set up alerts for capacity thresholds
- Plan for growth in each tier

### 4. Performance Optimization

- Place hot data on fastest storage
- Use appropriate storage engine for each tier
- Consider compression for cold storage

## Conclusion

The tablespace concept provides Nanograph with:

1. **Flexibility**: Easy storage management without application changes
2. **Performance**: Optimal storage tier for each workload
3. **Cost Efficiency**: Use expensive storage only where needed
4. **Scalability**: Add storage tiers as needs grow
5. **Multi-Tenancy**: Physical isolation between tenants

The filesystem storage pathing is handled through a combination of:
- **Tablespace abstraction**: Logical storage locations
- **Path resolution**: Consistent path structure
- **VFS layer**: Abstracted filesystem operations
- **Node-local configuration**: Flexible physical mappings

This architecture enables Nanograph to efficiently manage storage across distributed nodes while providing flexibility for different workload requirements.