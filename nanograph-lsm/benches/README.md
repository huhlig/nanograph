# Benchmarks for nanograph-lsm

This directory contains performance benchmarks for the LSM Tree implementation.

## Benchmark Suite

### Write Benchmarks
- **Sequential Writes** - Measures throughput for sequential key writes
- **Random Writes** - Measures throughput for random key writes
- **Large Values** - Tests performance with 1KB, 10KB, and 100KB values

### Read Benchmarks
- **Sequential Reads** - Measures throughput for sequential key reads
- **Random Reads** - Measures throughput for random key reads

### Mixed Workloads
- **Mixed Workload** - 33% writes, 33% reads, 33% updates

### Maintenance Operations
- **Deletes** - Measures deletion performance
- **Memtable Flush** - Measures flush operation latency

### Scalability Tests
- Tests with 100, 1K, 10K, and 100K operations

## Running Benchmarks

```bash
# Run all benchmarks
cargo bench --bench lsm_benchmarks

# Run benchmarks and save baseline
cargo bench --bench lsm_benchmarks -- --save-baseline main

# Compare against baseline
cargo bench --bench lsm_benchmarks -- --baseline main
```

## Benchmark Output

The benchmarks report:
- **Throughput**: Operations per second
- **Latency**: Microseconds per operation
- **Total Time**: Total benchmark duration in milliseconds

Example output:
```
=== LSM Tree Benchmarks ===

Benchmark                      |      Throughput |      Latency |     Total Time
------------------------------+----------------+-------------+---------------
Sequential Writes (10K)        |   50000.00 ops/sec |    20.00 µs/op |   200.00 ms total
Random Writes (10K)            |   45000.00 ops/sec |    22.22 µs/op |   222.22 ms total
Sequential Reads (10K)         |  100000.00 ops/sec |    10.00 µs/op |   100.00 ms total
```

## Performance Targets

Target performance metrics:
- **Write Throughput**: >50K ops/sec for small values
- **Read Latency (p99)**: <1ms
- **Memtable Flush**: <100ms for 64MB
- **Space Amplification**: <1.5x
- **Write Amplification**: <20x

## Profiling

For detailed profiling:

```bash
# Profile with perf (Linux)
cargo bench --bench lsm_benchmarks --profile release -- --profile-time=10

# Profile with Instruments (macOS)
cargo instruments --bench lsm_benchmarks --template time

# Profile with flamegraph
cargo flamegraph --bench lsm_benchmarks
```

## Optimization Tips

When optimizing based on benchmark results:

1. **Write Performance**
   - Increase memtable size for better batching
   - Use faster compression algorithms (LZ4 vs Zstd)
   - Tune WAL sync strategy

2. **Read Performance**
   - Increase block cache size
   - Optimize bloom filter parameters
   - Reduce number of levels through compaction

3. **Mixed Workloads**
   - Balance memtable size vs flush frequency
   - Tune compaction triggers
   - Consider universal compaction for write-heavy loads

## Made with Bob