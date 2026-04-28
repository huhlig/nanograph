# LSM Tree Architecture

## Overview

This document describes the Log-Structured Merge Tree (LSM Tree) implementation for Nanograph. The LSM Tree is optimized for write-heavy workloads and provides efficient range scans.

## Architecture Improvements & Recommendations

### 1. **Multi-Level Architecture**
- **Level 0**: Multiple overlapping SSTables (direct memtable flushes)
- **Level 1+**: Non-overlapping SSTables with exponentially increasing size
- **Size Ratio**: 10x between levels (configurable)
- **Max Level**: 7 levels (configurable, supports ~100TB with 64MB L0 files)

### 2. **MemTable Design**
- **Data Structure**: Skip List (O(log n) operations, good cache locality)
- **Size Threshold**: 64MB default (configurable)
- **Immutable MemTable**: When threshold reached, becomes immutable and new memtable created
- **WAL Integration**: All writes logged to WAL before memtable insertion

### 3. **SSTable Format** (Inspired by RocksDB/LevelDB)

```
[Data Blocks] [Meta Block] [Meta Index Block] [Index Block] [Footer]
```

#### Data Block Structure:
```
[Compression Type] [Compressed Data] [CRC32 Checksum]
```

Where Compressed Data contains:
```
[Entry 1] [Entry 2] ... [Entry N] [Restart Points] [Num Restarts]
```
- **Entry Format**: `[shared_key_len][unshared_key_len][value_len][unshared_key][value]`
- **Restart Points**: Every 16 entries for efficient binary search
- **Compression**: Per-block Snappy/LZ4/Zstd
- **CRC32 Checksum**: Covers compression type byte and compressed data (4 bytes)

#### Meta Block:
```
[Bloom Filter Data] [CRC32 Checksum]
```
- **Bloom Filter**: 10 bits per key, ~1% false positive rate
- **Statistics**: Min/max key, entry count, timestamps
- **CRC32 Checksum**: Covers bloom filter data (4 bytes)

#### Index Block:
```
[Index Entries] [CRC32 Checksum]
```
- **Format**: `[last_key_in_block][block_offset][block_size]`
- **Purpose**: Binary search to locate data blocks
- **CRC32 Checksum**: Covers all index entries (4 bytes)

#### Footer (48 bytes):
```
[Meta Index Handle: 8+8] [Index Handle: 8+8] [Magic: 8] [Version: 4] [Checksum: 4]
```

### 4. **Compaction Strategy** (Leveled Compaction)

#### Trigger Conditions:
- **Level 0**: 4+ SSTables (overlapping allowed)
- **Level N**: Total size > `base_size * 10^N`

#### Compaction Process:
1. Select SSTables from Level N
2. Find overlapping SSTables in Level N+1
3. Merge-sort all entries
4. Write new SSTables to Level N+1
5. Delete old SSTables atomically

#### Optimizations:
- **Parallel Compaction**: Multiple non-overlapping compactions
- **Subcompaction**: Split large compactions into smaller parallel tasks
- **Trivial Move**: Move non-overlapping L0 files directly to L1

### 5. **Read Path Optimization**

```
Query → MemTable → Immutable MemTable → L0 SSTables → L1+ SSTables
         ↓              ↓                    ↓              ↓
      Found?         Found?          Bloom Filter    Bloom Filter
                                          ↓              ↓
                                     Index Block    Index Block
                                          ↓              ↓
                                     Data Block     Data Block
```

#### Optimizations:
- **Block Cache**: LRU cache for frequently accessed blocks (default 8MB)
- **Bloom Filters**: Skip SSTables that definitely don't contain key
- **Table Cache**: Keep SSTable metadata in memory
- **Parallel Lookups**: Check multiple L0 SSTables concurrently

### 6. **Write Path**

```
Write → WAL → MemTable → [Threshold?] → Immutable MemTable → Background Flush → SSTable (L0)
```

#### Write Optimizations:
- **Batch Writes**: Group multiple writes into single WAL entry
- **Write Buffer**: Async WAL writes with fsync batching
- **Memtable Switch**: Fast atomic pointer swap

### 7. **Concurrency Model**

- **Read-Write Concurrency**: Lock-free reads, writes use memtable lock
- **Compaction**: Background threads, non-blocking to reads/writes
- **Version Control**: MVCC-style versioning for consistent snapshots
- **Manifest File**: Tracks current SSTable set, atomic updates

### 8. **Additional Features**

#### Bloom Filter:
- **Type**: Blocked Bloom Filter (better cache locality)
- **Size**: 10 bits per key
- **Hash Functions**: 7 (optimal for 1% FPR)

#### Compression:
- **Algorithms**: None, Snappy (default), LZ4, Zstd
- **Granularity**: Per-block (4KB default)
- **Trade-off**: CPU vs I/O and storage

#### Checksums:
- **Algorithm**: CRC32C (hardware accelerated via nanograph-util)
- **Coverage**: Block-level checksums for all blocks (data, index, meta/bloom filter)
- **Format**: 4-byte CRC32 appended to each block
- **Verification**: Automatic on read, detects bit rot and partial reads
- **Benefits**: Fail individual block reads instead of entire SSTable, better error isolation

## File Organization

```
/data/
  /tables/
    /{table_id}/
      /wal/              # Write-ahead logs
      /memtable/         # Memtable snapshots (optional)
      /l0/               # Level 0 SSTables
      /l1/               # Level 1 SSTables
      ...
      /l6/               # Level 6 SSTables
      MANIFEST          # Current version metadata
      CURRENT           # Points to active MANIFEST
```

## Performance Characteristics

### Time Complexity:
- **Write**: O(log n) memtable + O(1) WAL = O(log n)
- **Read**: O(log n) memtable + O(k * log m) SSTables where k = levels, m = entries
- **Range Scan**: O(log n + r) where r = result size
- **Compaction**: O(n log n) merge sort

### Space Amplification:
- **Worst Case**: ~11x (10x level ratio + 1x memtable/L0)
- **Typical**: 1.1-1.5x with regular compaction

### Write Amplification:
- **Leveled**: ~10-30x (depends on level ratio and data size)
- **Mitigation**: Larger memtable, higher level ratio, tiered compaction for L0

## Configuration Recommendations

### Write-Heavy Workload:
```rust
LSMTreeOptions {
    memtable_size: 128 * 1024 * 1024,  // 128MB
    block_size: 16384,                  // 16KB
    level0_file_num_compaction_trigger: 8,
    max_background_compactions: 4,
    compression: CompressionAlgorithm::Lz4,
}
```

### Read-Heavy Workload:
```rust
LSMTreeOptions {
    memtable_size: 32 * 1024 * 1024,   // 32MB
    block_size: 4096,                   // 4KB
    block_cache_size: 256 * 1024 * 1024, // 256MB
    bloom_filter_bits_per_key: 12,      // Lower FPR
    compression: CompressionAlgorithm::Snappy,
}
```

### Balanced Workload:
```rust
LSMTreeOptions {
    memtable_size: 64 * 1024 * 1024,   // 64MB (default)
    block_size: 4096,                   // 4KB
    block_cache_size: 128 * 1024 * 1024, // 128MB
    bloom_filter_bits_per_key: 10,
    compression: CompressionAlgorithm::Snappy,
}
```

## Future Enhancements

1. **Universal Compaction**: Alternative to leveled for write-heavy workloads
2. **Partitioned Index/Filters**: Reduce memory footprint for large SSTables
3. **Direct I/O**: Bypass page cache for large sequential scans
4. **Column Families**: Multiple logical tables sharing WAL and compaction
5. **Tiered Storage**: Hot data on SSD, cold data on HDD
6. **Incremental Compaction**: Reduce compaction latency spikes
7. **Range Deletion**: Efficient deletion of key ranges
8. **Merge Operator**: Application-defined merge semantics

## References

- [LevelDB Design](https://github.com/google/leveldb/blob/main/doc/impl.md)
- [RocksDB Architecture](https://github.com/facebook/rocksdb/wiki/RocksDB-Basics)
- [WiscKey Paper](https://www.usenix.org/system/files/conference/fast16/fast16-papers-lu.pdf)
- [LSM-tree Paper](https://www.cs.umb.edu/~poneil/lsmtree.pdf)