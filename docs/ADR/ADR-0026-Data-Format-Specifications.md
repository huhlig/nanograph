---
parent: ADR
nav_order: 0026
title: Data Format Specifications
status: proposed
date: 2026-01-07
deciders: Hans W. Uhlig
---

# ADR-0026: Data Format Specifications

## Status

Proposed

## Context

Nanograph requires well-defined, versioned on-disk data formats for all persistent structures (WAL entries, SSTable files, index files, metadata) to ensure:

1. **Data durability** - Correct recovery after crashes
2. **Backward compatibility** - Ability to read older format versions
3. **Forward compatibility** - Graceful handling of newer formats
4. **Debuggability** - Ability to inspect files with external tools
5. **Performance** - Efficient encoding/decoding
6. **Integrity** - Detection of corruption

All formats must be versioned and documented to support long-term data preservation and migration.

## Decision

Define comprehensive binary format specifications for all on-disk structures with the following principles:

1. **Magic numbers** - All files start with format-specific magic bytes
2. **Version headers** - Explicit version information in every file
3. **Checksums** - CRC32C checksums for data integrity
4. **Alignment** - Proper byte alignment for performance
5. **Extensibility** - Reserved fields for future additions
6. **Self-describing** - Metadata embedded in files

## Decision Drivers

* Need for data durability guarantees
* Support for rolling upgrades
* Ability to debug production issues
* Performance requirements
* Corruption detection
* Long-term data preservation

## Design

### 1. Write-Ahead Log (WAL) Format

#### File Structure

```
WAL File Layout:
┌─────────────────────────────────────┐
│ File Header (64 bytes)              │
├─────────────────────────────────────┤
│ Entry 1                             │
├─────────────────────────────────────┤
│ Entry 2                             │
├─────────────────────────────────────┤
│ ...                                 │
├─────────────────────────────────────┤
│ Entry N                             │
└─────────────────────────────────────┘
```

#### File Header (64 bytes)

```rust
struct WalFileHeader {
    magic: [u8; 8],           // "NANOWAL\0"
    version: u32,             // Format version (1)
    flags: u32,               // Feature flags
    created_at: u64,          // Unix timestamp (microseconds)
    sequence_start: u64,      // First sequence number in file
    checksum: u32,            // CRC32C of header (excluding this field)
    reserved: [u8; 28],       // Reserved for future use
}
```

#### WAL Entry Format

```
Entry Layout:
┌─────────────────────────────────────┐
│ Entry Header (32 bytes)             │
├─────────────────────────────────────┤
│ Key Length (varint)                 │
├─────────────────────────────────────┤
│ Value Length (varint)               │
├─────────────────────────────────────┤
│ Key Data (variable)                 │
├─────────────────────────────────────┤
│ Value Data (variable)               │
├─────────────────────────────────────┤
│ Entry Checksum (4 bytes)            │
└─────────────────────────────────────┘
```

```rust
struct WalEntryHeader {
    sequence: u64,            // Monotonic sequence number
    timestamp: u64,           // Unix timestamp (microseconds)
    entry_type: u8,           // Put=1, Delete=2, Batch=3
    table_id: u32,            // Target table
    flags: u8,                // Entry flags
    reserved: [u8; 10],       // Reserved
}

enum WalEntryType {
    Put = 1,
    Delete = 2,
    BatchStart = 3,
    BatchEnd = 4,
    Checkpoint = 5,
}
```

#### Checksum Calculation

```rust
fn calculate_entry_checksum(header: &WalEntryHeader, key: &[u8], value: &[u8]) -> u32 {
    let mut hasher = crc32c::Hasher::new();
    hasher.update(&header.to_bytes());
    hasher.update(key);
    hasher.update(value);
    hasher.finalize()
}
```

### 2. SSTable Format (LSM)

#### File Structure

```
SSTable File Layout:
┌─────────────────────────────────────┐
│ File Header (128 bytes)             │
├─────────────────────────────────────┤
│ Data Block 1                        │
├─────────────────────────────────────┤
│ Data Block 2                        │
├─────────────────────────────────────┤
│ ...                                 │
├─────────────────────────────────────┤
│ Data Block N                        │
├─────────────────────────────────────┤
│ Index Block                         │
├─────────────────────────────────────┤
│ Bloom Filter Block                  │
├─────────────────────────────────────┤
│ Metadata Block                      │
├─────────────────────────────────────┤
│ Footer (64 bytes)                   │
└─────────────────────────────────────┘
```

#### File Header (128 bytes)

```rust
struct SstFileHeader {
    magic: [u8; 8],           // "NANOSST\0"
    version: u32,             // Format version (1)
    compression: u8,          // None=0, Snappy=1, LZ4=2, Zstd=3
    checksum_type: u8,        // CRC32C=1, XXHash=2
    flags: u16,               // Feature flags
    block_size: u32,          // Target block size in bytes
    key_count: u64,           // Total number of keys
    min_sequence: u64,        // Minimum sequence number
    max_sequence: u64,        // Maximum sequence number
    created_at: u64,          // Unix timestamp (microseconds)
    table_id: u32,            // Table identifier
    level: u8,                // LSM level (0-7)
    reserved: [u8; 59],       // Reserved for future use
}
```

#### Data Block Format

```
Data Block Layout:
┌─────────────────────────────────────┐
│ Block Header (16 bytes)             │
├─────────────────────────────────────┤
│ Entry 1 (key-value pair)            │
├─────────────────────────────────────┤
│ Entry 2                             │
├─────────────────────────────────────┤
│ ...                                 │
├─────────────────────────────────────┤
│ Restart Points Array                │
├─────────────────────────────────────┤
│ Restart Count (4 bytes)             │
├─────────────────────────────────────┤
│ Block Checksum (4 bytes)            │
└─────────────────────────────────────┘
```

```rust
struct DataBlockHeader {
    uncompressed_size: u32,   // Size before compression
    compressed_size: u32,     // Size after compression (0 if uncompressed)
    entry_count: u32,         // Number of entries in block
    flags: u32,               // Block flags
}

// Entry format uses prefix compression
struct BlockEntry {
    shared_prefix_len: varint,  // Bytes shared with previous key
    unshared_len: varint,       // Bytes unique to this key
    value_len: varint,          // Value length
    key_suffix: [u8],           // Unshared portion of key
    value: [u8],                // Value data
}
```

#### Index Block Format

```rust
struct IndexBlock {
    entries: Vec<IndexEntry>,
    checksum: u32,
}

struct IndexEntry {
    last_key: Vec<u8>,        // Last key in data block
    offset: u64,              // File offset to data block
    size: u32,                // Data block size
}
```

#### Bloom Filter Block

```rust
struct BloomFilterBlock {
    bits_per_key: u32,        // Bloom filter density
    num_probes: u32,          // Number of hash functions
    data_len: u32,            // Bit array length
    data: Vec<u8>,            // Bit array
    checksum: u32,            // CRC32C checksum
}
```

#### Footer (64 bytes)

```rust
struct SstFooter {
    index_offset: u64,        // Offset to index block
    index_size: u32,          // Index block size
    bloom_offset: u64,        // Offset to bloom filter
    bloom_size: u32,          // Bloom filter size
    metadata_offset: u64,     // Offset to metadata
    metadata_size: u32,       // Metadata size
    checksum: u32,            // Footer checksum
    magic: [u8; 8],           // "NANOSST\0" (validation)
    reserved: [u8; 12],       // Reserved
}
```

### 3. Vector Index Format (HNSW)

#### File Structure

```
HNSW Index File Layout:
┌─────────────────────────────────────┐
│ File Header (128 bytes)             │
├─────────────────────────────────────┤
│ Graph Metadata                      │
├─────────────────────────────────────┤
│ Level 0 Nodes                       │
├─────────────────────────────────────┤
│ Level 1 Nodes                       │
├─────────────────────────────────────┤
│ ...                                 │
├─────────────────────────────────────┤
│ Level N Nodes                       │
├─────────────────────────────────────┤
│ Vector Data                         │
├─────────────────────────────────────┤
│ Footer (64 bytes)                   │
└─────────────────────────────────────┘
```

#### File Header (128 bytes)

```rust
struct HnswFileHeader {
    magic: [u8; 8],           // "NANOHNSW"
    version: u32,             // Format version (1)
    dimensions: u32,          // Vector dimensions
    metric: u8,               // Cosine=1, L2=2, DotProduct=3
    max_level: u8,            // Maximum graph level
    m: u16,                   // Max connections per node
    ef_construction: u32,     // Construction parameter
    node_count: u64,          // Total number of nodes
    entry_point: u64,         // Entry point node ID
    created_at: u64,          // Unix timestamp
    flags: u32,               // Feature flags
    reserved: [u8; 56],       // Reserved
}
```

#### Node Format

```rust
struct HnswNode {
    node_id: u64,             // Unique node identifier
    level: u8,                // Node level in hierarchy
    neighbor_count: u16,      // Number of neighbors
    neighbors: Vec<u64>,      // Neighbor node IDs
    vector_offset: u64,       // Offset to vector data
}
```

#### Vector Data Format

```rust
struct VectorEntry {
    node_id: u64,             // Node identifier
    dimensions: u32,          // Vector dimensions
    data: Vec<f32>,           // Vector components
    metadata_len: u32,        // Metadata length
    metadata: Vec<u8>,        // Associated metadata
}
```

### 4. B-Tree Index Format

#### File Structure

```
B-Tree Index File Layout:
┌─────────────────────────────────────┐
│ File Header (128 bytes)             │
├─────────────────────────────────────┤
│ Root Node                           │
├─────────────────────────────────────┤
│ Internal Nodes                      │
├─────────────────────────────────────┤
│ Leaf Nodes                          │
├─────────────────────────────────────┤
│ Footer (64 bytes)                   │
└─────────────────────────────────────┘
```

#### File Header (128 bytes)

```rust
struct BTreeFileHeader {
    magic: [u8; 8],           // "NANOBTREE"
    version: u32,             // Format version (1)
    order: u32,               // B-tree order (max keys per node)
    key_count: u64,           // Total number of keys
    node_count: u64,          // Total number of nodes
    root_offset: u64,         // Offset to root node
    height: u32,              // Tree height
    created_at: u64,          // Unix timestamp
    flags: u32,               // Feature flags
    reserved: [u8; 60],       // Reserved
}
```

#### Node Format

```rust
struct BTreeNode {
    node_type: u8,            // Internal=1, Leaf=2
    key_count: u32,           // Number of keys in node
    keys: Vec<Vec<u8>>,       // Sorted keys
    values: Vec<NodePointer>, // Child pointers or values
    checksum: u32,            // Node checksum
}

enum NodePointer {
    Child(u64),               // Offset to child node
    Value(Vec<u8>),           // Actual value (leaf only)
}
```

### 5. Metadata Format

#### Cluster Metadata

```rust
struct ClusterMetadata {
    version: u32,             // Metadata version
    cluster_id: [u8; 16],     // UUID
    node_count: u32,          // Number of nodes
    nodes: Vec<NodeMetadata>,
    shard_count: u32,         // Number of shards
    shards: Vec<ShardMetadata>,
    checksum: u32,
}

struct NodeMetadata {
    node_id: u64,
    address: String,
    role: NodeRole,           // Leader=1, Follower=2
    status: NodeStatus,       // Active=1, Inactive=2
    last_heartbeat: u64,
}

struct ShardMetadata {
    shard_id: u64,
    range_start: Vec<u8>,     // Key range start (inclusive)
    range_end: Vec<u8>,       // Key range end (exclusive)
    replicas: Vec<u64>,       // Node IDs
    leader: u64,              // Current leader node ID
    status: ShardStatus,      // Active=1, Rebalancing=2
}
```

### 6. Snapshot Format

```rust
struct SnapshotHeader {
    magic: [u8; 8],           // "NANOSNAP"
    version: u32,             // Format version
    snapshot_id: [u8; 16],    // UUID
    timestamp: u64,           // Creation timestamp
    sequence: u64,            // WAL sequence at snapshot
    table_count: u32,         // Number of tables
    total_size: u64,          // Total snapshot size
    compression: u8,          // Compression type
    checksum: u32,            // Header checksum
    reserved: [u8; 48],
}

struct SnapshotManifest {
    tables: Vec<TableSnapshot>,
    indexes: Vec<IndexSnapshot>,
    metadata: ClusterMetadata,
}

struct TableSnapshot {
    table_id: u32,
    sst_files: Vec<String>,   // List of SST file names
    key_count: u64,
    total_size: u64,
}
```

### 7. Encoding Utilities

#### Variable-Length Integer (Varint)

```rust
// Encode unsigned integer using LEB128
fn encode_varint(value: u64) -> Vec<u8> {
    let mut result = Vec::new();
    let mut v = value;
    loop {
        let mut byte = (v & 0x7F) as u8;
        v >>= 7;
        if v != 0 {
            byte |= 0x80;
        }
        result.push(byte);
        if v == 0 {
            break;
        }
    }
    result
}

fn decode_varint(data: &[u8]) -> Result<(u64, usize)> {
    let mut result = 0u64;
    let mut shift = 0;
    for (i, &byte) in data.iter().enumerate() {
        if i > 9 {
            return Err(Error::InvalidVarint);
        }
        result |= ((byte & 0x7F) as u64) << shift;
        if byte & 0x80 == 0 {
            return Ok((result, i + 1));
        }
        shift += 7;
    }
    Err(Error::IncompleteVarint)
}
```

### 8. Version Migration

#### Version Compatibility Matrix

| File Type | V1 | V2 | V3 |
|-----------|----|----|-----|
| WAL       | ✓  | →  | →   |
| SSTable   | ✓  | →  | →   |
| HNSW      | ✓  | →  | →   |
| B-Tree    | ✓  | →  | →   |

Legend:
- ✓ = Current version
- → = Forward compatible (can read)
- ✗ = Incompatible (requires migration)

#### Migration Strategy

```rust
trait FormatMigration {
    fn can_migrate(&self, from_version: u32, to_version: u32) -> bool;
    fn migrate(&self, input: &[u8], from_version: u32) -> Result<Vec<u8>>;
}

struct WalMigration;

impl FormatMigration for WalMigration {
    fn can_migrate(&self, from: u32, to: u32) -> bool {
        from < to && to - from <= 2  // Support 2 version jumps
    }
    
    fn migrate(&self, input: &[u8], from: u32) -> Result<Vec<u8>> {
        match from {
            1 => self.migrate_v1_to_v2(input),
            2 => self.migrate_v2_to_v3(input),
            _ => Err(Error::UnsupportedVersion),
        }
    }
}
```

## Consequences

### Positive

* **Data integrity** - Checksums detect corruption
* **Debuggability** - Well-documented formats enable inspection
* **Compatibility** - Versioning supports upgrades
* **Performance** - Efficient binary encoding
* **Extensibility** - Reserved fields allow evolution
* **Portability** - Platform-independent formats

### Negative

* **Complexity** - Multiple format specifications to maintain
* **Storage overhead** - Headers and checksums add space
* **Migration burden** - Version upgrades require careful handling
* **Testing complexity** - Must test all format versions

### Risks

* **Format bugs** - Errors in format can cause data loss
* **Version skew** - Mixed versions during rolling upgrades
* **Corruption detection** - False positives/negatives in checksums

## Alternatives Considered

### 1. Protocol Buffers for All Formats

**Rejected** - Too much overhead for hot path (WAL, SSTable). Better for metadata.

### 2. JSON for Metadata

**Rejected** - Binary formats are more compact and faster to parse.

### 3. No Versioning

**Rejected** - Makes upgrades impossible without downtime.

### 4. Single Format Version

**Rejected** - Different components evolve at different rates.

## Implementation Notes

### Phase 1: WAL Format (Week 4)
- Implement WAL encoding/decoding
- Add checksum validation
- Create format documentation

### Phase 2: SSTable Format (Week 6)
- Implement SSTable writer/reader
- Add compression support
- Optimize block layout

### Phase 3: Index Formats (Weeks 22, 25)
- Implement B-tree format
- Implement HNSW format
- Add index building tools

### Phase 4: Metadata Format (Week 12)
- Define cluster metadata schema
- Implement serialization
- Add validation

## Related ADRs

* [ADR-0003: Virtual File System Abstraction](ADR-0003-Virtual-File-System-Abstraction.md)
* [ADR-0004: Storage File Formats](ADR-0004-Storage-File-Formats.md)
* [ADR-0005: Write Ahead Log Support](ADR-0005-Write-Ahead-Log-Support.md)
* [ADR-0008: Indexing Options](ADR-0008-Indexing-Options.md)
* [ADR-0021: Upgrade Migration and Backward Compatibility](ADR-0021-Upgrade-Migration-and-Backward-Compatibility.md)

## References

* LevelDB SSTable format
* RocksDB file formats
* Protocol Buffers encoding
* CRC32C algorithm
* LEB128 variable-length encoding

---

**Next Steps:**
1. Review and approve format specifications
2. Implement encoding/decoding libraries
3. Create format validation tools
4. Write format documentation
5. Build format inspection utilities