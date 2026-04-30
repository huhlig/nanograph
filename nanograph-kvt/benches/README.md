# Unified Benchmark Suite for KeyValueShardStore Implementations

This directory contains a standardized benchmark framework for KeyValueShardStore implementations, enabling consistent performance comparisons across different storage engines.

## Structure

### `common.rs` - Reusable Benchmark Functions

The `common.rs` module provides generic benchmark functions that work with any `KeyValueShardStore` implementation:

- **Single Operations**: `bench_single_put`, `bench_single_get`
- **Batch Operations**: `bench_batch_put`, `bench_batch_get`
- **Scan Operations**: `bench_scan_full`, `bench_scan_prefix`, `bench_scan_range`
- **Mixed Workloads**: `bench_mixed_90_10`, `bench_mixed_50_50`
- **Convenience Functions**: `bench_single_operations`, `bench_batch_operations`, `bench_scan_operations`, `bench_mixed_workloads`, `bench_all`

### `kvstore_benchmarks.rs` - Memory Implementation Benchmarks

Benchmarks for the in-memory `MemoryKeyValueShardStore` implementation. Serves as a baseline for comparison.

### `comparative_benchmarks.rs` - Cross-Implementation Comparison

**This is the key file for comparing implementations!**

Runs identical benchmark scenarios across all KeyValueShardStore implementations:
- Memory (baseline)
- LMDB (persistent, ACID)
- LSM (write-optimized)

Run with:
```bash
cd nanograph-kvt
cargo bench --bench comparative_benchmarks
```

Results will show performance differences between implementations for each operation type.

## Benchmark Categories

### 1. Single Operations
- **put**: Insert with various value sizes (64B, 256B, 1KB, 4KB, 16KB)
- **get**: Retrieve with various value sizes (pre-populated dataset of 1000 keys)

**Metrics**: Throughput (ops/sec), Latency (µs/op)

### 2. Batch Operations
- **batch_put**: Batch insert with various batch sizes (10, 100, 1000)
- **batch_get**: Batch retrieve with various batch sizes
- **batch_delete**: Batch delete with various batch sizes

**Metrics**: Throughput (elements/sec), Latency (µs/batch)

### 3. Scan Operations
- **full_scan**: Scan entire dataset with various sizes (100, 1K, 10K keys)
- **prefix_scan**: Scan keys with common prefix (10K keys, 10 prefixes)
- **range_scan**: Scan specific key ranges (100, 1K, 5K keys)

**Metrics**: Throughput (elements/sec), Total scan time

### 4. Mixed Workloads
- **90/10 read/write**: Simulate read-heavy workload (1000 keys)
- **50/50 read/write**: Simulate balanced workload (1000 keys)

**Metrics**: Throughput (ops/sec), Latency (µs/op)

## Running Benchmarks

### Individual Implementation Benchmarks

Each storage engine has its own benchmarks in its respective crate:

```bash
# Memory (in nanograph-kvt)
cd nanograph-kvt
cargo bench --bench kvstore_benchmarks

# LMDB
cd nanograph-lmdb
cargo bench

# LSM
cd nanograph-lsm
cargo bench
```

### Comparative Benchmarks

To compare all implementations side-by-side:

```bash
cd nanograph-kvt
cargo bench --bench comparative_benchmarks
```

This will generate comparison reports in `target/criterion/` showing relative performance.

### Viewing Results

Criterion generates HTML reports with charts and statistical analysis:

```bash
# Open the main report
open target/criterion/report/index.html  # macOS
start target/criterion/report/index.html # Windows
xdg-open target/criterion/report/index.html # Linux
```

## Adding New Implementations

To benchmark a new KeyValueShardStore implementation:

1. **Add as dev-dependency** in `nanograph-kvt/Cargo.toml`:
   ```toml
   [dev-dependencies]
   your-kvstore = { path = "../your-kvstore" }
   ```

2. **Add setup function** in `comparative_benchmarks.rs`:
   ```rust
   fn setup_your_store() -> (YourStore, ShardId, TempDir) {
       // Initialize your store
       // ...
   }
   ```

3. **Add to all comparison functions**:
   ```rust
   fn compare_single_operations(c: &mut Criterion) {
       // ... existing implementations ...
       
       // Your implementation
       let (your_store, your_shard, _temp) = setup_your_store();
       common::bench_single_operations(c, "YourStore", &your_store, your_shard);
   }
   
   // Repeat for:
   // - compare_batch_operations
   // - compare_scan_operations
   // - compare_mixed_workloads
   ```

4. **Update this README** to list the new implementation in the "Current Implementations" section

5. **Run benchmarks**:
   ```bash
   cargo bench --bench comparative_benchmarks
   ```

## Benchmark Design Principles

1. **Identical Scenarios**: All implementations run the exact same test scenarios
2. **Consistent Metrics**: Same measurements (throughput, latency) across all tests
3. **Statistical Rigor**: Criterion provides confidence intervals and outlier detection
4. **Realistic Workloads**: Mix of operations reflects real-world usage patterns
5. **Scalability Testing**: Multiple dataset sizes to identify performance characteristics

## Performance Expectations

### Memory Store (Baseline)
- **Strengths**: Fastest for all operations, no I/O overhead
- **Weaknesses**: No persistence, limited by RAM

### LMDB
- **Strengths**: ACID guarantees, memory-mapped I/O, excellent read performance
- **Weaknesses**: Write amplification, lock contention under high concurrency

### LSM
- **Strengths**: Excellent write throughput, good compression, handles large datasets
- **Weaknesses**: Read amplification, compaction overhead, higher latency variance

## Interpreting Results

When comparing implementations, consider:

1. **Throughput vs Latency**: High throughput doesn't always mean low latency
2. **Workload Characteristics**: Some engines excel at reads, others at writes
3. **Dataset Size**: Performance may vary significantly with data volume
4. **Consistency Guarantees**: Faster engines may offer weaker guarantees
5. **Resource Usage**: Memory, disk I/O, and CPU utilization matter

## Best Practices

1. **Run on idle systems**: Background processes affect results
2. **Multiple runs**: Use criterion's statistical analysis (default)
3. **Warm-up**: First run may be slower due to cold caches
4. **Document changes**: Note any implementation-specific optimizations
5. **Version control**: Track performance over time with git

## References

- [Criterion.rs Documentation](https://bheisler.github.io/criterion.rs/book/)
- [KeyValueShardStore Trait](../src/kvstore.rs)
- [LMDB Benchmarks](../../nanograph-lmdb/benches/)
- [LSM Benchmarks](../../nanograph-lsm/benches/)