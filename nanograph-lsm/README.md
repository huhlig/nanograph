# Nanograph LSM Tree

A high-performance Log-Structured Merge Tree (LSM Tree) implementation for the Nanograph database.

## Overview

The LSM Tree is a write-optimized data structure that provides:

- **Fast Writes**: O(log n) writes to in-memory memtable
- **Efficient Reads**: Bloom filters and multi-level indexing
- **Range Scans**: Sorted data structure enables efficient range queries
- **Compaction**: Background compaction reduces space amplification
- **MVCC Support**: Sequence numbers for multi-version concurrency control

## Architecture

```
┌─────────────┐
│  MemTable   │ ← Active writes
└──────┬──────┘
       │ Flush (when full)
       ↓
┌─────────────┐
│  Level 0    │ ← Overlapping SSTables
└──────┬──────┘
       │ Compaction
       ↓
┌─────────────┐
│  Level 1    │ ← Non-overlapping SSTables
└──────┬──────┘
       │ Compaction
       ↓
┌─────────────┐
│  Level 2+   │ ← Larger, non-overlapping SSTables
└─────────────┘
```

## Components

### MemTable

In-memory sorted data structure (currently BTreeMap, designed for skip list):

- Stores recent writes
- Provides fast lookups
- Flushes to SSTable when size threshold reached
- Supports MVCC with sequence numbers

### SSTable (Sorted String Table)

Immutable on-disk sorted tables:

```
[Data Blocks] [Meta Block] [Meta Index] [Index Block] [Footer]
```

**Features:**
- Prefix-compressed keys in data blocks
- Bloom filter for fast negative lookups (~1% false positive rate)
- Block-based index for efficient seeks
- Optional compression (Snappy, LZ4, Zstd)
- Checksums for data integrity


### Block Cache

Smart LRU cache with frequency-aware eviction:

- **Frequency Tracking**: Counts how often each block is accessed
- **Recency Tracking**: Tracks when blocks were last accessed
- **Smart Eviction**: Uses score `frequency / (1 + recency)` to select victims
- **Metrics**: Tracks hits, misses, and evictions

### Compaction

Multi-level compaction strategy:

- **Level 0**: 4+ files trigger compaction (overlapping allowed)
- **Level 1+**: Size-based triggers (10x growth per level)
- **Merge**: Combines overlapping SSTables
- **Deduplication**: Removes old versions and tombstones

## Usage

```rust
use nanograph_lsm::{LSMTreeEngine, LSMTreeOptions};
use nanograph_wal::{WriteAheadLogManager, WriteAheadLogConfig};
use nanograph_vfs::MemoryFileSystem;
use std::path::PathBuf;

// Create WAL
let fs = MemoryFileSystem::new();
let wal_config = WriteAheadLogConfig::new(0);
let wal = WriteAheadLogManager::new(fs, "/wal".into(), wal_config)?;

// Create LSM engine
let options = LSMTreeOptions::default();
let engine = LSMTreeEngine::new(PathBuf::from("/data"), options, wal)?;

// Write data
engine.put(b"key1".to_vec(), b"value1".to_vec())?;
engine.put(b"key2".to_vec(), b"value2".to_vec())?;

// Read data
let value = engine.get(b"key1")?;
assert_eq!(value, Some(b"value1".to_vec()));

// Delete data
engine.delete(b"key1".to_vec())?;

// Get comprehensive metrics
let metrics = engine.metrics();
let snapshot = metrics.snapshot();

// Print detailed metrics summary
snapshot.print_summary();

// Access specific metrics
println!("Write amplification: {:.2}x", snapshot.write_amplification);
println!("Read amplification: {:.2}x", snapshot.read_amplification);
println!("Space amplification: {:.2}x", snapshot.space_amplification);
println!("Uptime: {} seconds", snapshot.uptime_seconds);

// Get statistics
let stats = engine.stats();
println!("Total writes: {}", stats.total_writes);
println!("Total reads: {}", stats.total_reads);
println!("Memtable size: {} bytes", stats.memtable_size);
```

## Configuration

```rust
use nanograph_lsm::LSMTreeOptions;
use nanograph_util::{CompressionAlgorithm, IntegrityAlgorithm};

let options = LSMTreeOptions {
    // Memtable size before flush (default: 64MB)
    memtable_size: 64 * 1024 * 1024,
    
    // Block size for SSTables (default: 4KB)
    block_size: 4096,
    
    // Compression algorithm
    compression: CompressionAlgorithm::Snappy,
    
    // Integrity checking
    integrity: IntegrityAlgorithm::Crc32c,
    

## Metrics & Observability

The LSM tree provides comprehensive metrics for monitoring and debugging:

### Amplification Metrics
- **Write Amplification**: Total bytes written to disk / User data written
- **Read Amplification**: Average SSTables read per get operation
- **Space Amplification**: Total storage size / Logical data size

### Timestamp Tracking
- Creation time
- Last write, read, flush, and compaction timestamps
- System uptime

### Performance Counters
- Operation counts (reads, writes, deletes, flushes, compactions)
- Average latencies
- Hit rates (memtable, SSTable, bloom filter, cache)
- Level statistics (sizes, file counts per level)

### Cache Metrics
- Cache hits and misses
- Eviction count
- Hit rate percentage
- Cache utilization

    // Encryption (optional)
    encryption: EncryptionAlgorithm::None,
    encryption_key: None,
};
```

## Performance Characteristics

### Time Complexity

- **Write**: O(log n) - memtable insertion
- **Read**: O(log n + k) - where k is number of levels checked
- **Range Scan**: O(log n + r) - where r is result size
- **Compaction**: O(n log n) - merge sort

### Space Amplification

- **Worst Case**: ~11x (10x level ratio + memtable/L0)
- **Typical**: 1.1-1.5x with regular compaction

### Write Amplification

- **Leveled**: ~10-30x (depends on level ratio)
- **Mitigation**: Larger memtable, higher level ratio

## Tuning Guidelines

### Write-Heavy Workload

```rust
LSMTreeOptions {
    memtable_size: 128 * 1024 * 1024,  // 128MB
    block_size: 16384,                  // 16KB
    compression: CompressionAlgorithm::Lz4,
    ..Default::default()
}
```

### Read-Heavy Workload

```rust
LSMTreeOptions {
    memtable_size: 32 * 1024 * 1024,   // 32MB
    block_size: 4096,                   // 4KB
    compression: CompressionAlgorithm::Snappy,
    ..Default::default()
}
```

### Balanced Workload

```rust
LSMTreeOptions::default()  // 64MB memtable, 4KB blocks
```

## Implementation Status

### ✅ Completed

- [x] MemTable with MVCC support
- [x] SSTable format with bloom filters
- [x] Multi-level LSM tree structure
- [x] Write path (memtable → SSTable flush)
- [x] Read path (memtable → SSTables)
- [x] Leveled compaction strategy
- [x] Prefix compression in data blocks
- [x] Varint encoding for space efficiency

### 🚧 In Progress

- [ ] Full compaction implementation
- [ ] Block cache for hot data
- [ ] WAL integration for durability
- [ ] Metrics and monitoring
- [ ] KeyValueStore trait implementation

### 📋 Planned

- [ ] Universal compaction option
- [ ] Partitioned bloom filters
- [ ] Direct I/O support
- [ ] Column families
- [ ] Tiered storage (SSD/HDD)
- [ ] Incremental compaction
- [ ] Range deletion
- [ ] Merge operators

## Testing

```bash
# Run unit tests
cargo test

# Run with output
cargo test -- --nocapture

# Run specific test
cargo test test_engine_basic_operations
```

## Benchmarks

```bash
# Run benchmarks (when implemented)
cargo bench
```

## References

- [LevelDB Design](https://github.com/google/leveldb/blob/main/doc/impl.md)
- [RocksDB Architecture](https://github.com/facebook/rocksdb/wiki/RocksDB-Basics)
- [LSM-tree Paper](https://www.cs.umb.edu/~poneil/lsmtree.pdf)
- [WiscKey Paper](https://www.usenix.org/system/files/conference/fast16/fast16-papers-lu.pdf)

## License

Copyright 2026 Hans W. Uhlig, IBM. All Rights Reserved.

Licensed under the Apache License, Version 2.0.