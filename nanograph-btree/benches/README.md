# B+Tree Benchmarks

Comprehensive performance benchmarks for the nanograph-btree implementation.

## Overview

This benchmark suite measures the performance characteristics of the B+Tree implementation across various workloads and configurations. Benchmarks use [Criterion.rs](https://github.com/bheisler/criterion.rs) for statistical analysis and HTML report generation.

## Running Benchmarks

### Run All Benchmarks
```bash
cargo bench -p nanograph-btree
```

### Run Specific Benchmark Group
```bash
cargo bench -p nanograph-btree -- insert_sequential
cargo bench -p nanograph-btree -- get_existing
cargo bench -p nanograph-btree -- scan_full
```

### Run with Baseline Comparison
```bash
# Save current performance as baseline
cargo bench -p nanograph-btree -- --save-baseline main

# Compare against baseline
cargo bench -p nanograph-btree -- --baseline main
```

### Generate Reports Only
```bash
cargo bench -p nanograph-btree -- --profile-time=5
```

## Benchmark Categories

### 1. Insert Benchmarks

#### `bench_insert_sequential`
Measures sequential insertion performance with varying dataset sizes.

**Sizes**: 100, 1,000, 10,000 keys
**Pattern**: Keys inserted in ascending order
**Metrics**: Throughput (elements/sec)

**Use Case**: Bulk loading, time-series data

#### `bench_insert_reverse`
Measures reverse sequential insertion performance.

**Sizes**: 100, 1,000, 10,000 keys
**Pattern**: Keys inserted in descending order
**Metrics**: Throughput (elements/sec)

**Use Case**: Reverse chronological data, worst-case insertion

#### `bench_insert_random`
Measures random insertion performance with shuffled keys.

**Sizes**: 100, 1,000, 10,000 keys
**Pattern**: Keys inserted in random order (shuffled)
**Metrics**: Throughput (elements/sec)

**Use Case**: Real-world insertion patterns, cache behavior, tree rebalancing under random load

#### `bench_insert_with_splits`
Measures insertion performance with frequent node splits.

**Configuration**: Small node size (max_keys=8) to force splits
**Sizes**: 100, 1,000, 5,000 keys
**Metrics**: Throughput (elements/sec)

**Use Case**: Understanding split overhead

### 2. Get Benchmarks

#### `bench_get_existing`
Measures lookup performance for existing keys.

**Sizes**: 1,000, 10,000, 100,000 keys
**Pattern**: Sequential lookups of all keys
**Metrics**: Throughput (elements/sec)

**Use Case**: Read-heavy workloads, cache hit scenarios

#### `bench_get_missing`
Measures lookup performance for non-existent keys.

**Sizes**: 1,000, 10,000, 100,000 keys (tree size)
**Pattern**: 1,000 lookups of missing keys
**Metrics**: Throughput (elements/sec)

**Use Case**: Cache miss scenarios, negative lookups

#### `bench_get_random_access`
Measures random access pattern performance.

**Sizes**: 1,000, 10,000, 100,000 keys
**Pattern**: Pseudo-random key access (using prime number distribution)
**Metrics**: Throughput (elements/sec)

**Use Case**: Real-world access patterns, cache behavior

### 3. Delete Benchmarks

#### `bench_delete`
Measures deletion performance.

**Sizes**: 100, 1,000, 10,000 keys
**Pattern**: Sequential deletion of all keys
**Metrics**: Throughput (elements/sec)

**Use Case**: Cleanup operations, data expiration

### 4. Range Scan Benchmarks

#### `bench_scan_full`
Measures full table scan performance.

**Sizes**: 1,000, 10,000, 100,000 keys
**Pattern**: Iterate through all keys
**Metrics**: Throughput (elements/sec)

**Use Case**: Full table exports, analytics queries

#### `bench_scan_range`
Measures bounded range scan performance.

**Tree Size**: 100,000 keys
**Range Sizes**: 100, 1,000, 10,000 keys
**Pattern**: Scan subset of keys
**Metrics**: Throughput (elements/sec)

**Use Case**: Range queries, pagination

#### `bench_scan_with_limit`
Measures limited scan performance.

**Tree Size**: 100,000 keys
**Limits**: 10, 100, 1,000 keys
**Pattern**: Scan with early termination
**Metrics**: Throughput (elements/sec)

**Use Case**: Top-N queries, pagination

### 5. Batch Operation Benchmarks

#### `bench_batch_put`
Measures batch insertion performance.

**Batch Sizes**: 10, 100, 1,000 keys
**Pattern**: Single batch operation
**Metrics**: Throughput (elements/sec)

**Use Case**: Bulk imports, transaction commits

#### `bench_batch_get`
Measures batch retrieval performance.

**Tree Size**: 10,000 keys
**Batch Sizes**: 10, 100, 1,000 keys
**Pattern**: Single batch lookup
**Metrics**: Throughput (elements/sec)

**Use Case**: Multi-get operations, prefetching

### 6. Mixed Workload Benchmarks

#### `bench_mixed_read_write`
Measures performance under mixed workloads.

**Workloads**:
- 90% read / 10% write
- 50% read / 50% write

**Tree Size**: 10,000 keys
**Operations**: 100 operations per iteration
**Metrics**: Operations/sec

**Use Case**: Real-world application patterns

### 7. Tree Structure Benchmarks

#### `bench_tree_height_impact`
Measures impact of node size on performance.

**Node Sizes**: 8, 32, 128, 512 max_keys
**Dataset**: 10,000 keys
**Metrics**: Total insertion time, tree statistics

**Use Case**: Configuration tuning, understanding trade-offs

## Interpreting Results

### Criterion Output

Criterion provides detailed statistical analysis:

```
insert_sequential/100   time:   [45.234 µs 45.891 µs 46.612 µs]
                        thrpt:  [2.1453 Melem/s 2.1793 Melem/s 2.2107 Melem/s]
                        change: [-2.3421% +0.5234% +3.4567%] (p = 0.45 > 0.05)
                        No change in performance detected.
```

**Key Metrics**:
- **time**: Execution time (lower is better)
- **thrpt**: Throughput (higher is better)
- **change**: Performance change vs baseline
- **p-value**: Statistical significance

### HTML Reports

Open `target/criterion/report/index.html` for:
- Performance graphs
- Distribution plots
- Comparison charts
- Detailed statistics

### Performance Expectations

#### Insert Performance
- **Sequential**: ~100K-500K ops/sec
- **Reverse**: ~80K-400K ops/sec
- **Random**: ~50K-200K ops/sec
- **With Splits**: ~20K-100K ops/sec

#### Get Performance
- **Existing Keys**: ~500K-2M ops/sec
- **Missing Keys**: ~300K-1M ops/sec
- **Random Access**: ~200K-800K ops/sec

#### Scan Performance
- **Full Scan**: ~1M-5M elements/sec
- **Range Scan**: ~500K-3M elements/sec
- **Limited Scan**: ~2M-10M elements/sec

*Note: Actual performance depends on hardware, key/value sizes, and tree configuration.*

## Performance Tuning

### Node Size Configuration

**Small Nodes (max_keys < 32)**:
- ✅ Lower memory per node
- ✅ Faster splits
- ❌ Taller trees (more traversals)
- ❌ More nodes to manage

**Medium Nodes (max_keys 32-256)**:
- ✅ Balanced performance
- ✅ Good cache locality
- ✅ Reasonable tree height

**Large Nodes (max_keys > 256)**:
- ✅ Shorter trees
- ✅ Fewer splits
- ❌ Higher memory per node
- ❌ Slower node operations

### Workload-Specific Tuning

**Read-Heavy Workloads**:
- Use larger nodes (128-512)
- Minimize tree height
- Optimize for cache hits

**Write-Heavy Workloads**:
- Use medium nodes (64-128)
- Balance split overhead
- Consider write buffering

**Range Scan Workloads**:
- Use medium-large nodes (128-256)
- Optimize leaf node linking
- Consider prefetching

**Mixed Workloads**:
- Use default configuration (128)
- Monitor and adjust based on metrics
- Profile actual workload

## Profiling

### CPU Profiling
```bash
# Using perf (Linux)
cargo bench -p nanograph-btree -- --profile-time=60
perf record -g target/release/deps/btree_benchmarks-*
perf report

# Using flamegraph
cargo flamegraph --bench btree_benchmarks
```

### Memory Profiling
```bash
# Using valgrind
valgrind --tool=massif target/release/deps/btree_benchmarks-*
ms_print massif.out.*
```

### Benchmark Profiling
```bash
# Profile specific benchmark
cargo bench -p nanograph-btree -- insert_sequential --profile-time=30
```

## Regression Testing

### Automated Performance Checks

```bash
# Save baseline before changes
git checkout main
cargo bench -p nanograph-btree -- --save-baseline main

# Make changes
git checkout feature-branch

# Compare performance
cargo bench -p nanograph-btree -- --baseline main
```

### CI Integration

Add to CI pipeline:
```yaml
- name: Run Benchmarks
  run: cargo bench -p nanograph-btree -- --save-baseline ci-${{ github.sha }}

- name: Compare with Main
  run: cargo bench -p nanograph-btree -- --baseline ci-main
```

## Adding New Benchmarks

### Template

```rust
fn bench_new_feature(c: &mut Criterion) {
    let mut group = c.benchmark_group("feature_group");
    
    // Configure group
    group.throughput(Throughput::Elements(1000));
    group.sample_size(100);
    
    // Add benchmark
    group.bench_function("feature_name", |b| {
        // Setup (not measured)
        let data = setup_data();
        
        b.iter(|| {
            // Code to benchmark (measured)
            black_box(operation(data));
        });
    });
    
    group.finish();
}

// Add to criterion_group!
criterion_group!(
    benches,
    // ... existing benchmarks
    bench_new_feature,
);
```

### Best Practices

1. **Use `black_box`**: Prevent compiler optimizations
2. **Separate Setup**: Don't measure setup code
3. **Realistic Data**: Use representative datasets
4. **Multiple Sizes**: Test scalability
5. **Statistical Validity**: Use adequate sample sizes
6. **Document Purpose**: Explain what's being measured

## Troubleshooting

### Benchmarks Too Slow
- Reduce sample size: `group.sample_size(10)`
- Reduce measurement time: `group.measurement_time(Duration::from_secs(5))`
- Run specific benchmarks only

### Inconsistent Results
- Close other applications
- Disable CPU frequency scaling
- Run on dedicated hardware
- Increase sample size

### Out of Memory
- Reduce dataset sizes
- Run benchmarks individually
- Increase system memory
- Use smaller node sizes

## Resources

- [Criterion.rs Book](https://bheisler.github.io/criterion.rs/book/)
- [Rust Performance Book](https://nnethercote.github.io/perf-book/)
- [Benchmarking Best Practices](https://easyperf.net/blog/)

## License

Licensed under the Apache License, Version 2.0. See LICENSE for details.

