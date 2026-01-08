# WAL Benchmarks

This directory contains comprehensive benchmarks for the Write-Ahead Log (WAL) implementation, focusing on the performance impact of various integrity, encryption, and compression settings.

## Overview

The benchmarks test different combinations of:
- **Integrity Algorithms**: None, CRC32C, XXHash32
- **Compression Algorithms**: None, LZ4, Zstd, Snappy
- **Encryption Algorithms**: None, AES-256-GCM, ChaCha20-Poly1305

## Running Benchmarks

Run all benchmarks:
```bash
cargo bench --package nanograph-wal
```

Run specific benchmark groups:
```bash
# Test different algorithm configurations
cargo bench --package nanograph-wal -- wal_append_configs

# Test varying payload sizes
cargo bench --package nanograph-wal -- wal_append_sizes

# Test read performance
cargo bench --package nanograph-wal -- wal_read_configs

# Test compression effectiveness
cargo bench --package nanograph-wal -- wal_compression_effectiveness
```

## Benchmark Groups

### 1. `bench_wal_append_with_configs`
Tests write performance with all combinations of integrity, compression, and encryption algorithms using 1KB payloads.

**Purpose**: Identify the overhead of each algorithm and their combinations.

**Key Metrics**:
- Throughput (bytes/sec)
- Latency per operation

**Configurations Tested**:
- Baseline (no protection)
- Individual algorithms (integrity only, compression only, encryption only)
- Pairwise combinations (integrity+compression, integrity+encryption, compression+encryption)
- Full protection (all three algorithms)

### 2. `bench_wal_append_varying_sizes`
Tests write performance across different payload sizes (64B to 16KB) with representative configurations.

**Purpose**: Understand how payload size affects performance with different protection levels.

**Configurations**:
- None-None-None (baseline)
- CRC32C-LZ4-AES256GCM (full protection)

**Payload Sizes**: 64B, 256B, 1KB, 4KB, 16KB

### 3. `bench_wal_read_with_configs`
Tests read performance for 1000 records with all algorithm combinations.

**Purpose**: Measure the overhead of decompression, decryption, and integrity verification during reads.

**Key Insights**:
- Encryption/decryption overhead
- Decompression overhead
- Integrity verification overhead

### 4. `bench_wal_batch_append_with_configs`
Tests batch write performance (100 records) with representative configurations.

**Purpose**: Evaluate batch operation efficiency with different protection levels.

**Configurations**:
- None-None-None (baseline)
- CRC32C-None-None (integrity only)
- None-LZ4-None (compression only)
- CRC32C-LZ4-AES256GCM (full protection)

### 5. `bench_wal_compression_effectiveness`
Tests compression performance with both highly compressible and random (incompressible) data.

**Purpose**: Understand when compression helps vs. hurts performance.

**Data Types**:
- **Compressible**: Repeated bytes (4KB of 0x42)
- **Random**: Pseudo-random data (incompressible)

**Algorithms Tested**: None, LZ4, Zstd, Snappy

### 6. `bench_wal_append_with_durability`
Tests write performance with different durability levels using a representative configuration.

**Purpose**: Measure the cost of different durability guarantees.

**Durability Levels**:
- **Memory**: No sync, fastest
- **Flush**: Flush to OS buffers
- **Sync**: Full fsync, slowest but most durable

### 7. `bench_wal_sequential_read_write`
Tests interleaved read/write operations with representative configurations.

**Purpose**: Measure the overhead of switching between read and write operations.

### 8. `bench_wal_concurrent_writes`
Tests concurrent write performance with 4 threads using representative configurations.

**Purpose**: Evaluate lock contention and concurrent write throughput.

## Interpreting Results

### Performance Expectations

**Integrity Algorithms** (fastest to slowest):
1. None (no overhead)
2. CRC32C (hardware accelerated on modern CPUs)
3. XXHash32 (very fast, but software-based)

**Compression Algorithms** (fastest to slowest):
1. None (no overhead)
2. Snappy (optimized for speed)
3. LZ4 (good balance of speed and ratio)
4. Zstd (best compression ratio, moderate speed)

**Encryption Algorithms** (fastest to slowest):
1. None (no overhead)
2. AES-256-GCM (hardware accelerated with AES-NI)
3. ChaCha20-Poly1305 (fast software implementation)

### Trade-offs

**Integrity**:
- ✅ Minimal overhead (< 5% typically)
- ✅ Detects corruption
- ❌ No confidentiality or compression

**Compression**:
- ✅ Reduces storage and I/O
- ✅ Can improve performance for large, compressible data
- ❌ CPU overhead (10-30% typically)
- ❌ Ineffective for random/encrypted data

**Encryption**:
- ✅ Provides confidentiality
- ✅ Includes authentication (AEAD)
- ❌ Moderate overhead (15-25% typically)
- ❌ Prevents compression (encrypt after compress!)

### Recommended Configurations

**Maximum Performance** (no protection):
```rust
checksum: IntegrityAlgorithm::None,
compression: CompressionAlgorithm::None,
encryption: EncryptionAlgorithm::None,
```

**Balanced** (integrity + fast compression):
```rust
checksum: IntegrityAlgorithm::Crc32c,
compression: CompressionAlgorithm::Lz4,
encryption: EncryptionAlgorithm::None,
```

**High Security** (integrity + encryption):
```rust
checksum: IntegrityAlgorithm::Crc32c,
compression: CompressionAlgorithm::None,
encryption: EncryptionAlgorithm::Aes256Gcm,
```

**Maximum Protection** (all features):
```rust
checksum: IntegrityAlgorithm::Crc32c,
compression: CompressionAlgorithm::Lz4,
encryption: EncryptionAlgorithm::Aes256Gcm,
```

**Best Compression** (for large, compressible data):
```rust
checksum: IntegrityAlgorithm::Crc32c,
compression: CompressionAlgorithm::Zstd,
encryption: EncryptionAlgorithm::None,
```

## Analysis Tips

### Comparing Configurations

1. **Baseline Comparison**: Always compare against the None-None-None baseline to understand absolute overhead.

2. **Incremental Analysis**: Compare configurations that differ by only one algorithm to isolate its impact.

3. **Workload Matching**: Choose configurations based on your data characteristics:
   - Highly compressible data → Use compression
   - Random/encrypted data → Skip compression
   - Security requirements → Use encryption
   - Corruption detection → Use integrity checks

### Performance Metrics

- **Throughput**: Higher is better (MB/s or ops/s)
- **Latency**: Lower is better (ns or µs per operation)
- **Overhead**: Calculate as `(protected_time - baseline_time) / baseline_time * 100%`

### Example Analysis

If baseline throughput is 1000 MB/s and CRC32C-LZ4-AES256GCM achieves 600 MB/s:
- Overhead: (1000 - 600) / 1000 = 40%
- This means full protection costs 40% performance
- Evaluate if the security/integrity benefits justify the cost

## Hardware Considerations

Performance varies significantly based on hardware:

- **CPU with AES-NI**: AES-256-GCM will be much faster
- **CPU without AES-NI**: ChaCha20-Poly1305 may be faster
- **Modern CPUs**: CRC32C is hardware accelerated
- **Fast Storage**: Compression overhead may outweigh I/O savings
- **Slow Storage**: Compression can improve overall performance

## Continuous Monitoring

Run benchmarks regularly to:
1. Detect performance regressions
2. Validate optimizations
3. Guide configuration decisions
4. Understand hardware-specific behavior

## Contributing

When adding new benchmarks:
1. Use descriptive names
2. Document the purpose and expected results
3. Include representative configurations
4. Consider both best-case and worst-case scenarios
5. Update this README with new benchmark descriptions

## See Also

- [WAL Implementation](../src/)
- [Utility Benchmarks](../../nanograph-util/benches/)
- [Architecture Documentation](../../docs/)