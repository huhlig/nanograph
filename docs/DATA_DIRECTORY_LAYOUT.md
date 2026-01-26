# Nanograph Data Directory Layout

## Overview

This document defines the comprehensive data directory layout for Nanograph, supporting shards, snapshots, WAL logs, and all file artifacts across distributed storage tiers.

## Root Directory Structure

```
<tablespace_base_path>/
├── system/                         # System Storage
│   └── metadata/                   # Container Metadata Shard  (Raft Group 0:0:0:0)
│       ├── data/                   # System Shard Files
│       ├── wal/                    # System Write Ahead Log
│       ├── logs/                   # Raft consensus logs
│       └── snapshots/              # Raft state snapshots
│
├── containers/                         # Container Storage
│   └── tenant_{tenant_id}/             # Tenant 
│       ├── metadata/                   # Tenant Metadata (Raft Group {tenant_id}:0:0:0)
│       │   ├── data/                   # Tenant Metadata Shard
│       │   ├── wal/                    # Tenant Metadata Shard Write Ahead Logs
│       │   ├── logs/                   # Tenant Metadata Raft Logs
│       │   └── snapshots/              # Tenant Metadata Snapshots
│       └── database_{database_id}/   
│           ├── metadata/                   # Database Metadata (Raft Group {tenant_id}:{database_id}:0:0)
│           │   ├── data/                   # Database Metadata Shard
│           │   ├── wal/                    # Database Metadata Shard Write Ahead Logs
│           │   ├── logs/                   # Database Metadata Raft Logs
│           │   └── snapshots/              # Database Metadata Snapshots
│           └── table_{table_id}/           # Table
│               ├── metadata/                   # Database Metadata (Raft Group {tenant_id}:{database_id}:{table_id}:0)
│               │   ├── data/                   # Database Metadata Shard
│               │   ├── wal/                    # Database Metadata Shard Write Ahead Logs
│               │   ├── logs/                   # Database Metadata Raft Logs
│               │   └── snapshots/              # Database Metadata Snapshots
│               ├── shard_{shard_number}/
│               │   ├── data/
│               │   │   ├── l0/              # LSM Level 0 SSTables
│               │   │   ├── l1/              # LSM Level 1 SSTables
│               │   │   ├── l2/              # LSM Level 2 SSTables
│               │   │   ├── l3/              # LSM Level 3 SSTables
│               │   │   ├── l4/              # LSM Level 4 SSTables
│               │   │   ├── l5/              # LSM Level 5 SSTables
│               │   │   ├── l6/              # LSM Level 6 SSTables
│               │   │   ├── memtable/        # Memtable snapshots (optional)
│               │   │   ├── MANIFEST         # Current version metadata
│               │   │   ├── CURRENT          # Points to active MANIFEST
│               │   │   └── LOCK             # Lock file
│               │   ├── wal/
│               │   │   ├── {sequence}.log   # WAL segments
│               │   │   └── CURRENT          # Current WAL file
│               │   ├── snapshots/
│               │   │   └── {timestamp}/
│               │   │       ├── data/        # Snapshot data files
│               │   │       ├── manifest.json
│               │   │       └── checksum.sha256
│               │   ├── temp/                # Temporary files during compaction
│               │   │   └── compaction_{id}/
│               │   └── raft/                    # Shard-level Raft (if replicated)
│               │       ├── logs/
│               │       ├── snapshots/
│               │       └── metadata/
│               └── index_{index_number}/
│                   ├── vector/              # Vector indexes (HNSW, IVF)
│                   │   ├── hnsw/
│                   │   │   ├── graph.bin
│                   │   │   ├── vectors.bin
│                   │   │   └── metadata.json
│                   │   └── ivf/
│                   │       ├── centroids.bin
│                   │       ├── inverted_lists/
│                   │       └── metadata.json
│                   ├── text/                # Full-text indexes
│                   │   ├── inverted_index.bin
│                   │   ├── positions.bin
│                   │   └── metadata.json
│                   └── btree/               # B+Tree secondary indexes
│                       ├── data/
│                       └── wal/
│
├── embeddings/                      # Embedding model artifacts
│   └── models/
│       └── {model_id}/
│           ├── {version}/
│           │   ├── model.bin
│           │   ├── config.json
│           │   ├── tokenizer.json
│           │   └── signature.sig    # Cryptographic signature
│           └── CURRENT              # Points to active version
│
├── cache/                           # Persistent cache (optional)
│   ├── block_cache/                 # Block cache persistence
│   ├── query_cache/                 # Query result cache
│   └── metadata_cache/              # Metadata cache
│
├── backups/                         # Local backup staging
│   └── {backup_id}/
│       ├── manifest.json
│       ├── data/
│       └── wal/
│
├── temp/                            # Temporary files
│   ├── compaction/
│   ├── snapshots/
│   └── imports/
│
└── logs/                            # Application logs (separate from WAL)
    ├── application.log
    ├── query.log
    ├── audit.log
    └── metrics.log
```

## Detailed Component Descriptions

### 1. System Metadata (`/system/`)

**Purpose**: Cluster-wide configuration, tablespace definitions, node registry

**Replication**: Raft Group 0 (system metadata tier)

**Files**:
- `manifest/`: System state versions
- `snapshots/`: Point-in-time system state
- `wal/`: System metadata write-ahead log
- `raft/logs/`: Raft consensus log entries
- `raft/snapshots/`: Raft state machine snapshots

**Key Data Stored**:
- Tablespace definitions and configurations
- Node registry and cluster membership
- Global configuration settings
- Security principals and credentials

### 2. Container Metadata (`/containers/{container_id}/`)

**Purpose**: Per-container metadata (databases, tables, shards)

**Replication**: One Raft group per container

**Structure**: Same as system metadata

**Key Data Stored**:
- Database definitions within container
- Table schemas and configurations
- Shard placement and replica assignments
- Container-level security policies

### 3. Tenant Data (`/tenants/{tenant_id}/`)

**Purpose**: Physical isolation of tenant data

**Hierarchy**:
```
tenants/
└── {tenant_id}/              # Unique tenant identifier
    └── databases/
        └── {database_id}/    # Database within tenant
            └── namespaces/
                └── {namespace_id}/  # Logical grouping
                    └── tables/
                        └── {table_id}/  # Table identifier
                            ├── metadata.json
                            └── shards/
                                └── {shard_index}/
```

**Benefits**:
- Clear tenant isolation at filesystem level
- Easy tenant data migration
- Simplified backup/restore per tenant
- Storage quota enforcement

### 4. Shard Data (`/tenants/{tenant_id}/.../shards/{shard_index}/`)

**Purpose**: Actual key-value data storage

**Engine-Specific Layouts**:

#### LSM Tree (`lsm/`)

```
lsm/
├── data/
│   ├── l0/                    # Level 0: Overlapping SSTables from memtable flushes
│   │   ├── 000001.sst        # SSTable format: [Data Blocks][Meta][Index][Footer]
│   │   ├── 000002.sst
│   │   └── ...
│   ├── l1/                    # Level 1: Non-overlapping, 10x size of L0
│   │   ├── 000010.sst
│   │   └── ...
│   ├── l2/                    # Level 2: 10x size of L1
│   ├── l3/                    # Level 3: 10x size of L2
│   ├── l4/                    # Level 4: 10x size of L3
│   ├── l5/                    # Level 5: 10x size of L4
│   ├── l6/                    # Level 6: 10x size of L5 (max level)
│   ├── memtable/              # Optional: Memtable snapshots for fast recovery
│   ├── MANIFEST-{seq}         # SSTable metadata, versions, compaction state
│   ├── CURRENT                # Points to active MANIFEST file
│   └── LOCK                   # Prevents concurrent access
├── wal/
│   ├── 000001.log             # Sequential WAL segments (64MB default)
│   ├── 000002.log
│   ├── 000003.log
│   └── CURRENT                # Points to active WAL file
├── snapshots/
│   └── {timestamp}/           # ISO 8601 format: 2026-01-22T05-35-00Z
│       ├── data/              # Frozen SSTable set at snapshot time
│       │   ├── l0/
│       │   ├── l1/
│       │   └── ...
│       ├── manifest.json      # Snapshot metadata (timestamp, size, checksums)
│       └── checksum.sha256    # Overall snapshot integrity
└── temp/
    └── compaction_{id}/       # Temporary files during compaction
        ├── {new_sstable}.tmp
        └── manifest.tmp
```

**LSM File Details**:
- **SSTable Format**: Data blocks (4KB), meta block (bloom filter), index block, footer (48 bytes)
- **Compression**: Per-block Snappy/LZ4/Zstd
- **Bloom Filters**: 10 bits per key, ~1% false positive rate
- **Block Cache**: LRU cache for frequently accessed blocks

#### B+Tree (`btree/`)

```
btree/
├── data/
│   ├── nodes/                 # B+Tree node files
│   │   ├── internal/          # Internal nodes (keys + pointers)
│   │   │   ├── node_000001.dat
│   │   │   └── ...
│   │   └── leaf/              # Leaf nodes (keys + values)
│   │       ├── node_000001.dat
│   │       └── ...
│   ├── versions/              # MVCC version chains
│   │   └── {key_hash}/
│   │       ├── v1.dat
│   │       ├── v2.dat
│   │       └── ...
│   ├── metadata.json          # Tree structure metadata (order, height, root)
│   └── LOCK
├── wal/
│   ├── {sequence}.log
│   └── CURRENT
└── snapshots/
    └── {timestamp}/
        ├── data/
        ├── manifest.json
        └── checksum.sha256
```

**B+Tree Details**:
- **Order**: 128 (default, configurable)
- **Node Size**: 4KB aligned
- **MVCC**: Multiple versions per key with commit timestamps
- **Cache**: Node cache for frequently accessed nodes

#### ART (Adaptive Radix Tree) (`art/`)

```
art/
├── data/
│   ├── nodes/                 # ART node files by type
│   │   ├── node4/             # 4-way nodes
│   │   │   ├── node_000001.dat
│   │   │   └── ...
│   │   ├── node16/            # 16-way nodes
│   │   ├── node48/            # 48-way nodes
│   │   └── node256/           # 256-way nodes
│   ├── leaves/                # Leaf values
│   │   └── leaf_000001.dat
│   ├── metadata.json          # Tree structure metadata
│   └── LOCK
├── wal/
│   ├── {sequence}.log
│   └── CURRENT
└── snapshots/
    └── {timestamp}/
        ├── data/
        ├── manifest.json
        └── checksum.sha256
```

**ART Details**:
- **Adaptive**: Nodes grow from 4→16→48→256 children
- **Path Compression**: Efficient for sparse key spaces
- **Cache**: Node cache with LRU eviction

### 5. Indexes (`/indexes/`)

**Purpose**: Secondary indexes separate from primary data

**Structure**:
```
indexes/
└── {tenant_id}/
    └── {database_id}/
        └── {namespace_id}/
            └── {table_id}/
                └── {index_id}/
                    ├── type.json          # Index type and configuration
                    ├── vector/
                    ├── text/
                    └── btree/
```

#### Vector Indexes

```
vector/
├── hnsw/                      # Hierarchical Navigable Small World
│   ├── graph.bin              # Graph structure (layers, connections)
│   ├── vectors.bin            # Actual vector data
│   ├── metadata.json          # M, efConstruction, distance metric
│   └── LOCK
└── ivf/                       # Inverted File Index
    ├── centroids.bin          # Cluster centroids
    ├── inverted_lists/        # One file per cluster
    │   ├── cluster_0000.bin
    │   ├── cluster_0001.bin
    │   └── ...
    ├── metadata.json          # nlist, nprobe, distance metric
    └── LOCK
```

**Vector Index Details**:
- **HNSW**: Best for high recall, moderate dataset size
- **IVF**: Best for large datasets, approximate search
- **Distance Metrics**: L2, cosine, dot product
- **Dimensions**: Up to 2048 (configurable)

#### Text Indexes

```
text/
├── inverted_index.bin         # Term → document postings
├── positions.bin              # Term positions for phrase queries
├── term_frequencies.bin       # TF-IDF data
├── metadata.json              # Tokenizer, stemmer, stop words
└── LOCK
```

#### B+Tree Secondary Indexes

```
btree/
├── data/
│   └── nodes/
│       ├── internal/
│       └── leaf/
├── wal/
└── metadata.json
```

### 6. Embeddings (`/embeddings/`)

**Purpose**: Embedding model artifacts and versioning

**Structure**:
```
embeddings/
└── models/
    └── {model_id}/            # e.g., "sentence-transformers-all-MiniLM-L6-v2"
        ├── v1/
        │   ├── model.bin      # Model weights
        │   ├── config.json    # Model configuration
        │   ├── tokenizer.json # Tokenizer configuration
        │   ├── vocab.txt      # Vocabulary
        │   └── signature.sig  # Cryptographic signature for verification
        ├── v2/
        │   └── ...
        └── CURRENT            # Points to active version (e.g., "v2")
```

**Model Management**:
- **Versioning**: Multiple versions coexist
- **Hot Swap**: Change CURRENT to switch versions
- **Verification**: Signatures prevent tampering
- **Metadata**: Tracks model provenance and performance

### 7. Snapshots

**Location**: Per-shard under `{engine_type}/snapshots/{timestamp}/`

**Contents**:
```
{timestamp}/                   # e.g., 2026-01-22T05-35-00Z
├── data/                      # Complete data files at snapshot time
│   ├── l0/
│   ├── l1/
│   └── ...
├── manifest.json              # Snapshot metadata
│   {
│     "timestamp": "2026-01-22T05:35:00Z",
│     "shard_id": "table_42_shard_0",
│     "engine_type": "lsm",
│     "size_bytes": 1073741824,
│     "file_count": 42,
│     "wal_position": 12345,
│     "checksums": { ... }
│   }
└── checksum.sha256            # Overall snapshot integrity
```

**Snapshot Types**:
- **Full**: Complete copy of all data
- **Incremental**: Only changes since last snapshot (future)
- **Consistent**: Point-in-time consistency guaranteed

**Naming Convention**: ISO 8601 UTC timestamp with hyphens replacing colons

### 8. WAL (Write-Ahead Log)

**Location**: Per-shard under `{engine_type}/wal/`

**Format**:
```
wal/
├── 000001.log                 # Sequential log segments
├── 000002.log
├── 000003.log
└── CURRENT                    # Points to active WAL file
```

**WAL Entry Format**:
```
[Checksum: 4 bytes]
[Length: 4 bytes]
[Type: 1 byte]                 # Put, Delete, Commit, etc.
[Sequence: 8 bytes]
[Timestamp: 8 bytes]
[Key Length: 4 bytes]
[Value Length: 4 bytes]
[Key: variable]
[Value: variable]
```

**WAL Properties**:
- **Durability**: fsync before acknowledging writes
- **Rotation**: New file when size threshold reached (64MB default)
- **Retention**: Until checkpoint/snapshot created
- **Recovery**: Replay from last checkpoint

### 9. Raft Consensus

**Location**: Multiple locations depending on scope

**System Metadata Raft** (`/system/raft/`):
```
raft/
├── logs/
│   ├── 000001.log             # Raft log entries
│   ├── 000002.log
│   └── CURRENT
├── snapshots/
│   └── {term}_{index}/        # Raft state machine snapshots
│       ├── state.bin
│       └── metadata.json
└── metadata/
    ├── hard_state.json        # Current term, voted for, commit index
    └── conf_state.json        # Cluster configuration
```

**Shard-Level Raft** (`/tenants/.../shards/{shard_index}/raft/`):
- Same structure as system Raft
- One Raft group per replicated shard
- Coordinates writes across replicas

### 10. Cache (`/cache/`)

**Purpose**: Persistent cache for faster restarts (optional)

**Structure**:
```
cache/
├── block_cache/               # LSM block cache persistence
│   ├── index.bin              # Cache index
│   └── blocks/
│       ├── block_000001.bin
│       └── ...
├── query_cache/               # Query result cache
│   └── results/
│       └── {query_hash}.bin
└── metadata_cache/            # Metadata cache
    └── schemas/
        └── {table_id}.json
```

**Cache Policies**:
- **LRU**: Least Recently Used eviction
- **Size Limits**: Configurable per cache type
- **Persistence**: Optional, for faster cold starts

### 11. Backups (`/backups/`)

**Purpose**: Local backup staging before remote transfer

**Structure**:
```
backups/
└── {backup_id}/               # UUID or timestamp
    ├── manifest.json          # Backup metadata
    │   {
    │     "backup_id": "uuid",
    │     "timestamp": "2026-01-22T05:35:00Z",
    │     "type": "full",
    │     "tables": [...],
    │     "size_bytes": 10737418240,
    │     "compression": "zstd"
    │   }
    ├── data/                  # Backup data files
    │   ├── system/
    │   ├── containers/
    │   └── tenants/
    └── wal/                   # WAL files for point-in-time recovery
        └── ...
```

**Backup Types**:
- **Full**: Complete database backup
- **Incremental**: Changes since last backup
- **Differential**: Changes since last full backup

### 12. Temporary Files (`/temp/`)

**Purpose**: Safe intermediate file management

**Structure**:
```
temp/
├── compaction/                # LSM compaction temporary files
│   └── {compaction_id}/
│       ├── new_sstable_1.tmp
│       ├── new_sstable_2.tmp
│       └── manifest.tmp
├── snapshots/                 # Snapshot creation staging
│   └── {snapshot_id}/
│       └── ...
└── imports/                   # Bulk data import staging
    └── {import_id}/
        ├── data.csv
        └── progress.json
```

**Cleanup Policy**:
- Automatic cleanup on success
- Retention on failure for debugging
- Periodic cleanup of abandoned files

### 13. Logs (`/logs/`)

**Purpose**: Application logs (separate from WAL)

**Structure**:
```
logs/
├── application.log            # General application logs
├── query.log                  # Query execution logs
├── audit.log                  # Security audit trail
├── metrics.log                # Performance metrics
└── error.log                  # Error logs
```

**Log Rotation**:
- Daily rotation by default
- Size-based rotation (100MB)
- Compression of old logs
- Retention policy (30 days default)

## File Naming Conventions

### SSTable Files (LSM)
- **Format**: `{sequence_number}.sst`
- **Example**: `000042.sst`
- **Sequence**: Monotonically increasing, zero-padded to 6 digits
- **Range**: 000001 to 999999

### WAL Files
- **Format**: `{sequence_number}.log`
- **Example**: `000001.log`
- **Rotation**: New file when size threshold reached
- **Sequence**: Monotonically increasing

### Snapshot Directories
- **Format**: `{timestamp}/`
- **Example**: `2026-01-22T05-35-00Z/`
- **Timestamp**: ISO 8601 UTC format with hyphens replacing colons
- **Sorting**: Lexicographic sorting gives chronological order

### Manifest Files
- **Format**: `MANIFEST-{sequence}`
- **Example**: `MANIFEST-000042`
- **CURRENT**: Text file containing active manifest filename
- **LOCK**: Empty file for filesystem-level locking

### Raft Files
- **Log Format**: `{sequence}.log`
- **Snapshot Format**: `{term}_{index}/`
- **Example**: `snapshot_5_12345/`

## Storage Tier Mapping

### Hot Tier (NVMe)

**Path**: `/mnt/nvme/nanograph/`

**Use Cases**:
- Real-time analytics
- Frequently accessed data
- System metadata
- Active WAL files

**Example**:
```
/mnt/nvme/nanograph/
├── system/
├── containers/
└── tenants/
    └── tenant_001/
        └── databases/
            └── realtime_db/
                └── namespaces/
                    └── default/
                        └── tables/
                            └── events/
                                └── shards/
                                    └── 0/
                                        └── lsm/
```

### Warm Tier (SSD)

**Path**: `/mnt/ssd/nanograph/`

**Use Cases**:
- Regular access patterns
- General purpose tables
- Secondary indexes
- Recent snapshots

**Example**:
```
/mnt/ssd/nanograph/
├── tenants/
│   └── tenant_001/
│       └── databases/
│           └── main_db/
└── indexes/
    └── tenant_001/
```

### Cold Tier (HDD)

**Path**: `/mnt/hdd/nanograph/`

**Use Cases**:
- Historical data
- Infrequent access
- Long-term backups
- Archived snapshots

**Example**:
```
/mnt/hdd/nanograph/
├── tenants/
│   └── tenant_001/
│       └── databases/
│           └── archive_db/
└── backups/
    └── 2026-01-01/
```

### Archive Tier (Object Storage)

**Path**: `s3://nanograph-archive/`

**Use Cases**:
- Compliance data
- Disaster recovery
- Long-term retention
- Cold backups

**Example**:
```
s3://nanograph-archive/
├── backups/
│   └── 2025-12-31/
└── snapshots/
    └── tenant_001/
```

## Path Resolution Examples

### Example 1: LSM Shard in Hot Tier

**Logical Path**: `/tenant_001/main_db/default/users/shard_0`

**Physical Path**: `/mnt/nvme/nanograph/tenants/tenant_001/databases/main_db/namespaces/default/tables/users/shards/0/lsm/`

**Files**:
```
/mnt/nvme/nanograph/tenants/tenant_001/databases/main_db/namespaces/default/tables/users/shards/0/lsm/
├── data/
│   ├── l0/
│   │   ├── 000001.sst
│   │   └── 000002.sst
│   ├── l1/
│   │   └── 000010.sst
│   ├── MANIFEST-000001
│   ├── CURRENT
│   └── LOCK
├── wal/
│   ├── 000001.log
│   └── CURRENT
└── snapshots/
    └── 2026-01-22T05-35-00Z/
```

### Example 2: B+Tree Shard in Warm Tier

**Logical Path**: `/tenant_002/analytics_db/reports/monthly_stats/shard_0`

**Physical Path**: `/mnt/ssd/nanograph/tenants/tenant_002/databases/analytics_db/namespaces/reports/tables/monthly_stats/shards/0/btree/`

**Files**:
```
/mnt/ssd/nanograph/tenants/tenant_002/databases/analytics_db/namespaces/reports/tables/monthly_stats/shards/0/btree/
├── data/
│   ├── nodes/
│   │   ├── internal/
│   │   │   └── node_000001.dat
│   │   └── leaf/
│   │       └── node_000001.dat
│   ├── versions/
│   ├── metadata.json
│   └── LOCK
├── wal/
│   └── 000001.log
└── snapshots/
```

### Example 3: Vector Index

**Logical Path**: `/tenant_001/main_db/default/documents/index_embeddings`

**Physical Path**: `/mnt/ssd/nanograph/indexes/tenant_001/main_db/default/documents/embeddings/vector/hnsw/`

**Files**:
```
/mnt/ssd/nanograph/indexes/tenant_001/main_db/default/documents/embeddings/vector/hnsw/
├── graph.bin
├── vectors.bin
├── metadata.json
└── LOCK
```

## Key Design Principles

### 1. Hierarchical Organization
- Clear tenant → database → namespace → table → shard hierarchy
- Mirrors logical data model in filesystem
- Easy to understand and navigate

### 2. Engine Isolation
- Each storage engine has its own directory structure
- No cross-contamination of files
- Engine-specific optimizations possible

### 3. Separation of Concerns
- Data, WAL, snapshots, and indexes in separate directories
- Independent lifecycle management
- Simplified backup and recovery

### 4. Tablespace Flexibility
- Same logical structure across all storage tiers
- Easy data migration between tiers
- Transparent to applications

### 5. Atomic Operations
- Use of CURRENT files for atomic updates
- Lock files prevent concurrent access
- Manifest files track state transitions

### 6. Scalability
- Supports millions of shards through hierarchical organization
- No single directory with too many files
- Efficient filesystem operations

### 7. Multi-Tenancy
- Physical isolation at tenant level
- Clear security boundaries
- Independent tenant management

### 8. Backup-Friendly
- Clear snapshot and backup directory structure
- Consistent point-in-time views
- Easy to identify and transfer

### 9. Monitoring
- Separate logs directory for observability
- Structured log files by type
- Easy integration with monitoring tools

### 10. Cleanup
- Temp directory for safe intermediate file management
- Clear ownership of temporary files
- Automatic cleanup policies

## Operational Considerations

### Disk Space Management

**Monitoring**:
- Track usage per tablespace
- Alert on threshold violations
- Automatic cleanup of old snapshots

**Quotas**:
- Per-tenant storage limits
- Per-table size limits
- Enforcement at write time

### Backup Strategy

**Full Backups**:
- Weekly full backups to archive tier
- Includes all data and metadata
- Compressed and encrypted

**Incremental Backups**:
- Daily incremental backups
- Only changed data since last backup
- Fast and space-efficient

**WAL Archiving**:
- Continuous WAL archiving
- Point-in-time recovery capability
- Retention based on RPO requirements

### Disaster Recovery

**Replication**:
- Synchronous replication for hot data
- Asynchronous replication for warm/cold data
- Cross-region replication for DR

**Recovery**:
- Restore from backup + replay WAL
- Automated recovery procedures
- Regular DR drills

### Performance Optimization

**Hot Tier**:
- NVMe for lowest latency
- Separate data and WAL on different devices
- Large block cache

**Warm Tier**:
- SSD for balanced performance
- Compression enabled
- Moderate block cache

**Cold Tier**:
- HDD for cost efficiency
- Aggressive compression
- Minimal cache

## Migration Procedures

### Moving Data Between Tiers

**Hot → Warm**:
1. Create snapshot in hot tier
2. Copy snapshot to warm tier
3. Update metadata to point to warm tier
4. Verify data integrity
5. Delete hot tier copy

**Warm → Cold**:
1. Ensure no active transactions
2. Create final snapshot
3. Copy to cold tier with compression
4. Update metadata
5. Archive or delete warm tier copy

### Tablespace Migration

**Process**:
1. Create new tablespace in target tier
2. Create table in new tablespace
3. Copy data shard by shard
4. Switch reads to new location
5. Switch writes to new location
6. Verify and cleanup old location

## Troubleshooting

### Common Issues

**Disk Full**:
- Check temp directory for abandoned files
- Review snapshot retention policy
- Trigger manual compaction
- Move data to larger tablespace

**Slow Queries**:
- Check block cache hit rate
- Review compaction status
- Analyze query patterns
- Consider adding indexes

**Corruption**:
- Verify checksums
- Restore from snapshot
- Replay WAL from last good state
- Contact support if persistent

### Diagnostic Tools

**File Inspection**:
```bash
# List SSTable files
ls -lh /mnt/nvme/nanograph/tenants/*/databases/*/namespaces/*/tables/*/shards/*/lsm/data/l*/*.sst

# Check WAL size
du -sh /mnt/nvme/nanograph/tenants/*/databases/*/namespaces/*/tables/*/shards/*/lsm/wal/

# Verify snapshot integrity
sha256sum -c /path/to/snapshot/checksum.sha256
```

**Metadata Inspection**:
```bash
# View manifest
cat /path/to/shard/lsm/data/CURRENT
cat /path/to/shard/lsm/data/MANIFEST-000001

# Check Raft state
cat /path/to/raft/metadata/hard_state.json
```

## Future Enhancements

### Planned Features

1. **Tiered Storage Automation**
   - Automatic data movement based on access patterns
   - Policy-based lifecycle management
   - Cost optimization

2. **Compression Improvements**
   - Per-level compression strategies
   - Dictionary compression for similar data
   - Transparent compression/decompression

3. **Snapshot Improvements**
   - Incremental snapshots
   - Snapshot streaming
   - Cross-region snapshot replication

4. **Index Enhancements**
   - Partitioned indexes for large tables
   - Covering indexes
   - Index-only scans

5. **Monitoring Enhancements**
   - Real-time storage metrics
   - Predictive capacity planning
   - Automated alerting

## References

- [Filesystem Storage Architecture](FILESYSTEM_STORAGE_ARCHITECTURE.md)
- [Tablespace Implementation Guide](TABLESPACE_IMPLEMENTATION_GUIDE.md)
- [LSM Architecture](../nanograph-lsm/ARCHITECTURE.md)
- [MVCC Design](MVCC_DESIGN.md)
- [WAL Implementation Status](WAL_IMPLEMENTATION_STATUS.md)

## Conclusion

This data directory layout provides a comprehensive, scalable, and maintainable structure for Nanograph's distributed storage system. It supports:

- **Multi-tenancy**: Physical isolation at tenant level
- **Storage tiering**: Flexible placement across hot/warm/cold tiers
- **Multiple engines**: LSM, B+Tree, and ART with engine-specific optimizations
- **Distributed consensus**: Raft integration at multiple levels
- **Backup and recovery**: Clear snapshot and backup structures
- **Operational excellence**: Monitoring, troubleshooting, and maintenance

The layout is designed to scale from single-node deployments to large distributed clusters while maintaining consistency, durability, and performance.